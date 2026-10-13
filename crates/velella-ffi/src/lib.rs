use std::fmt;
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use tokio::runtime::{Builder, Runtime};
use tokio::task::JoinHandle;
use velella_core::{Session, SessionEvent, Token};

uniffi::setup_scaffolding!();

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
