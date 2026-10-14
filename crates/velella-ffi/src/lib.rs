use std::fmt;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;
use velella_core::{Session, SessionEvent, Token};

mod decorations;
pub use decorations::{SidebarOccupant, SidebarStatus, SidebarThread};

uniffi::setup_scaffolding!();

mod markdown;
mod members;
mod messages;
pub use markdown::*;
pub use messages::*;

/// The one process-wide runtime. Exported async fns spawn onto it and await the
/// JoinHandle, so bodies run on `velella-rt` threads regardless of who polls the
/// returned future. (`uniffi::export(async_runtime = "tokio")` is deliberately
/// not used: async-compat polls the body on the caller's thread.)
///
/// `enable_all()`: sessions dial WebSockets on this runtime, which needs the IO driver as well as
/// timers (`owned_runtime_has_io` fails without it).
///
/// Log hygiene: shipped code installs NO logger of any kind — no `log` logger (env_logger, oslog,
/// ...), no `tracing-log`/`LogTracer` bridge, no tracing subscriber. tungstenite logs raw frames
/// through `log` at TRACE, and the IDENTIFY/RESUME frames carry the account token; with nothing
/// installed, every `log` and `tracing` record is dropped unemitted. Do not add one here or anywhere
/// else in the shipped build.
fn runtime() -> &'static Runtime {
    static RT: OnceLock<Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        Builder::new_multi_thread()
            .thread_name("velella-rt")
            .enable_all()
            .build()
            .expect("failed to build tokio runtime")
    })
}

/// Name of the thread the body ran on.
#[uniffi::export]
pub async fn runtime_probe() -> String {
    runtime()
        .spawn(async { std::thread::current().name().unwrap_or("").to_owned() })
        .await
        .expect("probe task failed")
}

#[uniffi::export(with_foreign)]
pub trait TickListener: Send + Sync {
    fn on_tick(&self, count: u64);
}

/// Dropping this aborts the tick task.
#[derive(uniffi::Object)]
pub struct Subscription(JoinHandle<()>);

impl Drop for Subscription {
    fn drop(&mut self) {
        self.0.abort();
        eprintln!("velella: subscription dropped");
    }
}

#[uniffi::export]
pub fn subscribe_ticks(listener: Arc<dyn TickListener>) -> Arc<Subscription> {
    start_ticks(listener, Duration::from_secs(1))
}

pub fn start_ticks(listener: Arc<dyn TickListener>, interval: Duration) -> Arc<Subscription> {
    let task = runtime().spawn(async move {
        let mut count = 0u64;
        let mut timer = tokio::time::interval(interval);
        timer.tick().await; // first tick completes immediately
        loop {
            timer.tick().await;
            count += 1;
            listener.on_tick(count);
        }
    });
    Arc::new(Subscription(task))
}

/// The S2 user-agent constant, so the login webview presents the same browser as IDENTIFY.
#[uniffi::export]
pub fn user_agent() -> String {
    velella_core::USER_AGENT.to_owned()
}

/// Session lifecycle callbacks. They run on a `velella-rt` thread and must not block.
#[uniffi::export(with_foreign)]
pub trait SessionListener: Send + Sync {
    fn on_ready(&self, generation: u64, user_id: u64, username: String);
    fn on_resumed(&self, generation: u64);
    fn on_reconnecting(&self, generation: u64);
    fn on_auth_failed(&self, generation: u64);
    fn on_closed(&self, generation: u64);
    /// A REST call got 401: REST stays off for this session, the gateway carries on. At most once
    /// per session and never after `on_auth_failed`/`on_closed`. Not a reason to delete the
    /// saved sign-in (that stays tied to `on_auth_failed`).
    fn on_rest_unauthorized(&self, generation: u64);
}

/// A running session. Dropping the last reference disconnects it.
#[derive(uniffi::Object)]
pub struct SessionHandle(velella_core::SessionHandle);

#[uniffi::export]
impl SessionHandle {
    pub fn disconnect(&self) {
        self.0.disconnect();
    }

    pub fn generation(&self) -> u64 {
        self.0.generation()
    }

    /// The account's locale tag (BCP 47) as of now, `None` until READY carried a valid one.
    pub fn locale(&self) -> Option<String> {
        self.0.snapshot().locale.clone()
    }

    /// Rail entries (guilds and folders in display order): the current snapshot at once, then
    /// again whenever it changes. Dropping the returned object stops the deliveries.
    pub fn subscribe_guilds(&self, listener: Arc<dyn GuildsListener>) -> Arc<SidebarSubscription> {
        let _context = runtime().enter();
        sidebar(
            self.0
                .subscribe(velella_core::guild_entries, move |entries| {
                    listener.on_guilds(entries.into_iter().map(Into::into).collect())
                }),
        )
    }

    /// The channel list of one guild (empty while the guild is unknown or after it is deleted).
    /// Every delivery names `guild_id`, so a late one from a dropped subscription can be told apart.
    pub fn subscribe_channels(
        &self,
        guild_id: u64,
        listener: Arc<dyn ChannelsListener>,
    ) -> Arc<SidebarSubscription> {
        let _context = runtime().enter();
        sidebar(self.0.subscribe(
            move |state| velella_core::channel_categories(state, guild_id),
            move |categories| {
                listener.on_channels(guild_id, categories.into_iter().map(Into::into).collect())
            },
        ))
    }

    pub fn subscribe_dms(&self, listener: Arc<dyn DmsListener>) -> Arc<SidebarSubscription> {
        let _context = runtime().enter();
        sidebar(self.0.subscribe(
            |state| (velella_core::dm_list(state), state.friends_pending()),
            move |(dms, friends_pending)| {
                listener.on_dms(dms.into_iter().map(Into::into).collect(), friends_pending)
            },
        ))
    }
}

/// Holds a core subscription; dropping it stops the deliveries.
#[derive(uniffi::Object)]
pub struct SidebarSubscription(#[allow(dead_code)] velella_core::Subscription);

fn sidebar(subscription: velella_core::Subscription) -> Arc<SidebarSubscription> {
    Arc::new(SidebarSubscription(subscription))
}

/// Sidebar callbacks run on a `velella-rt` thread and must not block.
#[uniffi::export(with_foreign)]
pub trait GuildsListener: Send + Sync {
    fn on_guilds(&self, entries: Vec<SidebarEntry>);
}

#[uniffi::export(with_foreign)]
pub trait ChannelsListener: Send + Sync {
    fn on_channels(&self, guild_id: u64, categories: Vec<SidebarCategory>);
}

#[uniffi::export(with_foreign)]
pub trait DmsListener: Send + Sync {
    /// `friends_pending`: incoming friend requests, for the Friends row badge.
    fn on_dms(&self, dms: Vec<SidebarDm>, friends_pending: u32);
}

#[derive(uniffi::Record)]
pub struct SidebarGuild {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
    pub unread: bool,
    pub mention_count: u32,
    pub muted: bool,
}

#[derive(uniffi::Enum)]
pub enum SidebarEntry {
    Guild(SidebarGuild),
    Folder {
        id: Option<u64>,
        name: Option<String>,
        color: Option<u32>,
        guilds: Vec<SidebarGuild>,
    },
}

#[derive(uniffi::Record)]
pub struct SidebarChannel {
    pub id: u64,
    pub kind: u8,
    pub name: String,
    pub unread: bool,
    pub mention_count: u32,
    pub muted: bool,
    pub private: bool,
    pub occupants: Vec<SidebarOccupant>,
    pub threads: Vec<SidebarThread>,
}

#[derive(uniffi::Record)]
pub struct SidebarCategory {
    pub id: Option<u64>,
    pub name: Option<String>,
    pub channels: Vec<SidebarChannel>,
}

#[derive(uniffi::Record)]
pub struct SidebarDm {
    pub id: u64,
    pub name: String,
    pub icon_url: Option<String>,
    pub unread: bool,
    pub mention_count: u32,
    pub status: Option<SidebarStatus>,
}

impl From<velella_core::SidebarGuild> for SidebarGuild {
    fn from(g: velella_core::SidebarGuild) -> Self {
        Self {
            id: g.id,
            name: g.name,
            icon_url: g.icon_url,
            unread: g.unread,
            mention_count: g.mention_count,
            muted: g.muted,
        }
    }
}

impl From<velella_core::SidebarEntry> for SidebarEntry {
    fn from(e: velella_core::SidebarEntry) -> Self {
        match e {
            velella_core::SidebarEntry::Guild(g) => Self::Guild(g.into()),
            velella_core::SidebarEntry::Folder {
                id,
                name,
                color,
                guilds,
            } => Self::Folder {
                id,
                name,
                color,
                guilds: guilds.into_iter().map(Into::into).collect(),
            },
        }
    }
}

impl From<velella_core::SidebarCategory> for SidebarCategory {
    fn from(c: velella_core::SidebarCategory) -> Self {
        Self {
            id: c.id,
            name: c.name,
            channels: c
                .channels
                .into_iter()
                .map(|c| SidebarChannel {
                    id: c.id,
                    kind: c.kind,
                    name: c.name,
                    unread: c.unread,
                    mention_count: c.mention_count,
                    muted: c.muted,
                    private: c.private,
                    occupants: c.occupants.into_iter().map(Into::into).collect(),
                    threads: c.threads.into_iter().map(Into::into).collect(),
                })
                .collect(),
        }
    }
}

impl From<velella_core::SidebarDm> for SidebarDm {
    fn from(d: velella_core::SidebarDm) -> Self {
        Self {
            id: d.id,
            name: d.name,
            icon_url: d.icon_url,
            unread: d.unread,
            mention_count: d.mention_count,
            status: d.status.map(Into::into),
        }
    }
}

/// Fixed text only: never carries (any part of) the rejected token.
#[derive(Debug, uniffi::Error)]
pub enum ConnectError {
    InvalidToken,
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid token")
    }
}

impl std::error::Error for ConnectError {}

/// Validates the token immediately (Token zeroizes it on drop, also on rejection), then starts the
/// session on the owned runtime. No URL crosses this boundary: the gateway URL is fixed in core.
#[uniffi::export]
pub fn connect(
    token: String,
    listener: Arc<dyn SessionListener>,
) -> Result<Arc<SessionHandle>, ConnectError> {
    let token = Token::new(token).map_err(|_| ConnectError::InvalidToken)?;
    let _context = runtime().enter();
    let handle = Session::connect(token, move |event| match event {
        SessionEvent::Ready {
            generation,
            user_id,
            username,
        } => listener.on_ready(generation, user_id, username),
        SessionEvent::Resumed { generation } => listener.on_resumed(generation),
        SessionEvent::Reconnecting { generation } => listener.on_reconnecting(generation),
        SessionEvent::AuthFailed { generation } => listener.on_auth_failed(generation),
        SessionEvent::Closed { generation } => listener.on_closed(generation),
        SessionEvent::RestUnauthorized { generation } => listener.on_rest_unauthorized(generation),
    });
    Ok(Arc::new(SessionHandle(handle)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    #[test]
    fn async_runs_on_owned_runtime() {
        let name = futures::executor::block_on(runtime_probe());
        assert!(name.starts_with("velella-rt"), "ran on {name:?}");
    }

    /// `Session::connect` dials through tokio's IO driver; a time-only runtime panics there.
    #[test]
    fn owned_runtime_has_io() {
        let task = runtime().spawn(async {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let addr = listener.local_addr()?;
            let (client, accepted) =
                tokio::join!(tokio::net::TcpStream::connect(addr), listener.accept());
            client?;
            accepted?;
            Ok::<(), std::io::Error>(())
        });
        futures::executor::block_on(task)
            .expect("IO task panicked (runtime without IO driver?)")
            .expect("loopback connect failed");
    }

    struct NoEvents;
    impl SessionListener for NoEvents {
        fn on_ready(&self, _: u64, _: u64, _: String) {}
        fn on_resumed(&self, _: u64) {}
        fn on_reconnecting(&self, _: u64) {}
        fn on_auth_failed(&self, _: u64) {}
        fn on_closed(&self, _: u64) {}
        fn on_rest_unauthorized(&self, _: u64) {}
    }

    /// Only invalid tokens here: a valid one would start a real gateway dial.
    #[test]
    fn connect_error_text_is_fixed_and_never_echoes_the_input() {
        for bad in [
            "SYNTHETIC_TOKEN_MARKER with space",
            "SYNTH",
            "Bot SYNTHETIC_TOKEN_MARKER_0123456789",
        ] {
            let Err(err) = connect(bad.to_owned(), Arc::new(NoEvents)) else {
                panic!("accepted an invalid token");
            };
            assert_eq!(err.to_string(), "invalid token");
            assert!(!format!("{err} {err:?}").contains("SYNTH"));
        }
    }

    /// Subscriptions need a live session, which needs a valid token (a real gateway dial), so
    /// only the core-to-record conversion is exercised here; the driver is tested in velella-core.
    #[test]
    fn core_entries_convert_field_for_field() {
        let guild = velella_core::SidebarGuild {
            id: 1,
            name: "g".into(),
            icon_url: Some("u".into()),
            unread: true,
            mention_count: 4,
            muted: false,
        };
        let folder = velella_core::SidebarEntry::Folder {
            id: Some(7),
            name: None,
            color: Some(0xff0000),
            guilds: vec![guild.clone()],
        };
        match SidebarEntry::from(folder) {
            SidebarEntry::Folder {
                id: Some(7),
                name: None,
                color: Some(0xff0000),
                guilds,
            } => assert_eq!(
                (guilds[0].id, guilds[0].icon_url.as_deref()),
                (1, Some("u"))
            ),
            _ => panic!("folder did not convert"),
        }
        assert!(matches!(
            SidebarEntry::from(velella_core::SidebarEntry::Guild(guild)),
            SidebarEntry::Guild(SidebarGuild {
                id: 1,
                unread: true,
                mention_count: 4,
                muted: false,
                ..
            })
        ));
        let category = SidebarCategory::from(velella_core::SidebarCategory {
            id: None,
            name: None,
            channels: vec![velella_core::SidebarChannel {
                id: 2,
                kind: 13,
                name: "c".into(),
                unread: true,
                mention_count: 3,
                muted: true,
                private: true,
                occupants: vec![velella_core::SidebarOccupant {
                    id: 9,
                    name: "o".into(),
                    icon_url: None,
                }],
                threads: vec![velella_core::SidebarThread {
                    id: 8,
                    name: "t".into(),
                    unread: true,
                    mention_count: 1,
                    muted: false,
                }],
            }],
        });
        let c = &category.channels[0];
        assert_eq!(
            (category.id, c.kind, &*c.name, c.unread, c.mention_count),
            (None, 13, "c", true, 3)
        );
        assert!(c.muted && c.private);
        assert_eq!((c.occupants[0].id, &*c.occupants[0].name), (9, "o"));
        assert_eq!((c.threads[0].id, c.threads[0].mention_count), (8, 1));
    }

    #[test]
    fn core_dm_converts_field_for_field() {
        let dm = SidebarDm::from(velella_core::SidebarDm {
            id: 5,
            name: "d".into(),
            icon_url: None,
            unread: true,
            mention_count: 2,
            status: Some(velella_core::PresenceStatus::DoNotDisturb),
        });
        assert_eq!((dm.id, dm.unread, dm.mention_count), (5, true, 2));
        assert!(matches!(dm.status, Some(SidebarStatus::DoNotDisturb)));
    }

    #[test]
    fn user_agent_is_the_core_constant() {
        assert_eq!(user_agent(), velella_core::USER_AGENT);
    }

    struct Counter(Arc<AtomicU64>);
    impl TickListener for Counter {
        fn on_tick(&self, _count: u64) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn dropping_handle_stops_ticks() {
        let interval = Duration::from_millis(20);
        let ticks = Arc::new(AtomicU64::new(0));
        let handle = start_ticks(Arc::new(Counter(ticks.clone())), interval);
        std::thread::sleep(interval * 8);
        assert!(ticks.load(Ordering::SeqCst) > 1);
        drop(handle);
        std::thread::sleep(interval * 2); // let an in-flight tick land
        let settled = ticks.load(Ordering::SeqCst);
        std::thread::sleep(interval * 6);
        assert_eq!(ticks.load(Ordering::SeqCst), settled);
    }
}
