//! Experiments and feature gates (menu plan slice X, D7, D14), from
//! refs/discord-ground-truth/experiments.md. The official client has two systems, and this file
//! follows both:
//!
//! - legacy: READY `experiments` (user buckets, precomputed by the server) and READY
//!   `guild_experiments` (guild buckets, computed here: hash key, populations, filters, overrides,
//!   holdout);
//! - apex: READY `apex_experiments` (kept raw in `State::apex_experiments`, which STATE_UPDATE also
//!   feeds) and the guild units in `guilds[].experiments` / GUILD_CREATE `guild.experiments`.
//!
//! Every answer is tri-state. An input that is missing or unreadable gives `Unknown`, never a
//! silent `Disabled`, and the menu hides what is `Unknown` (S-6). A shipped build has no local
//! override: the official client's dev overrides apply to staging and staff builds only
//! (experiments.md 3.6), which velella is not.
use crate::identify::now_ms;
use crate::state::{get_id, id, State};
use serde_json::Value;
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};

/// MurmurHash3 x86 32-bit, seed 0, of the UTF-8 bytes: the official `h()` (experiments.md 3.1).
pub(crate) const fn murmur3(data: &[u8]) -> u32 {
    const C1: u32 = 0xcc9e_2d51;
    const C2: u32 = 0x1b87_3593;
    let mut h = 0u32;
    let blocks = data.len() / 4;
    let mut i = 0;
    while i < blocks {
        let k = u32::from_le_bytes([
            data[4 * i],
            data[4 * i + 1],
            data[4 * i + 2],
            data[4 * i + 3],
        ]);
        let k = k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
        h = (h ^ k)
            .rotate_left(13)
            .wrapping_mul(5)
            .wrapping_add(0xe654_6b64);
        i += 1;
    }
    let tail = data.len() % 4;
    if tail > 0 {
        let mut k = 0u32;
        let mut j = 0;
        while j < tail {
            k |= (data[4 * blocks + j] as u32) << (8 * j);
            j += 1;
        }
        h ^= k.wrapping_mul(C1).rotate_left(15).wrapping_mul(C2);
    }
    h ^= data.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}

const fn hs(s: &str) -> u32 {
    murmur3(s.as_bytes())
}

const TIDA_WEBFORM: &str = "2025-11_tida_webform";
const OFFICIAL_MESSAGES: &str = "2026-03_guild_official_messages";
const BOOKMARKS_V1: &str = "2026-03-message-bookmarks";
const BOOKMARKS_V2: &str = "2026-07-message-bookmarks-v2";
const OBFUSCATION: &str = "2026-02-private-channel-hiding";
const CONVERSATIONS: &str = "2026-03-conversation-highlighting-utility";

/// Guild features and the user flag the gates read.
const FEATURE_VERIFIED: &str = "VERIFIED";
const FEATURE_CONVERSATIONS: &str = "CONVERSATIONS_EXTRACTION_PROCESSING";
const USER_FLAG_STAFF: u64 = 1;
/// Apex assignment flags (experiments.md 2.4).
const FLAG_IS_OVERRIDE: u64 = 1;
const FLAG_USE_AS_ELIGIBILITY: u64 = 8;

// Filter kinds, by `h(kind)` (experiments.md 2b); the test module checks them against the table.
const K_GUILD_IDS: u32 = hs("guild_ids");
const K_ID_RANGE: u32 = hs("guild_id_range");
const K_AGE_RANGE: u32 = hs("guild_age_range_days");
const K_MEMBER_RANGE: u32 = hs("guild_member_count_range");
const K_HAS_FEATURE: u32 = hs("guild_has_feature");
const K_HUB_TYPES: u32 = hs("guild_hub_types");
const K_HAS_VANITY: u32 = hs("guild_has_vanity_url");
const K_IN_RANGE_BY_HASH: u32 = hs("guild_in_range_by_hash");
const P_MIN: u32 = hs("min_id");
const P_MAX: u32 = hs("max_id");
const P_HASH_KEY: u32 = hs("hash_key");
const P_TARGET: u32 = hs("target");
const P_GUILD_IDS: u32 = hs("guild_ids");

/// Discord's snowflake epoch, and one day.
const DISCORD_EPOCH_MS: i128 = 1_420_070_400_000;
const DAY_MS: i128 = 86_400_000;
/// A holdout that names a holdout that names ... stops here (the official code recurses freely).
const MAX_HOLDOUT_DEPTH: u8 = 8;

/// The answer to "is this on for this account (and guild)".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    Enabled,
    Disabled,
    /// An input is missing or unreadable. The menu treats this as hidden (S-6).
    Unknown,
}

impl Gate {
    fn of(on: bool) -> Gate {
        if on {
            Gate::Enabled
        } else {
            Gate::Disabled
        }
    }

    /// Disabled wins, then Unknown; Enabled only when both are.
    fn and(self, other: Gate) -> Gate {
        match (self, other) {
            (Gate::Disabled, _) | (_, Gate::Disabled) => Gate::Disabled,
            (Gate::Enabled, Gate::Enabled) => Gate::Enabled,
            _ => Gate::Unknown,
        }
    }

    /// Enabled wins, then Unknown; Disabled only when both are.
    fn or(self, other: Gate) -> Gate {
        match (self, other) {
            (Gate::Enabled, _) | (_, Gate::Enabled) => Gate::Enabled,
            (Gate::Disabled, Gate::Disabled) => Gate::Disabled,
            _ => Gate::Unknown,
        }
    }
}

/// The gates the message menu, the hover bar and IDENTIFY ask for (experiments.md "Gates").
/// `guild` is the selected guild (`None` in a DM).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuGate {
    /// Staff user flag. The caller also needs non-empty message content.
    Translate,
    /// Apex `2026-03-conversation-highlighting-utility` and the guild feature
    /// `CONVERSATIONS_EXTRACTION_PROCESSING`. The caller also needs a conversation for the message
    /// (ConversationStore, not read here).
    ViewConversation,
    /// The selected guild's `moderator_reporting`: enabled and a report channel set.
    ReportToModerators,
    /// Legacy guild `2026-03_guild_official_messages` and the guild feature `VERIFIED`. The caller
    /// also needs MANAGE_OFFICIAL_MESSAGES (F1's permissions), the message type and the channel.
    OfficialMessage,
    /// Legacy user `2025-11_tida_webform`.
    TidaWebform,
    /// Apex `2026-03-message-bookmarks`. Using it needs Nitro tier 2 (the caller's).
    BookmarksV1,
    /// Apex `2026-07-message-bookmarks-v2`; limits through `State::bookmark_v2_limits`.
    BookmarksV2,
    /// The hover bar's `message_utilities` location: v1 or v2 (the item is visible at all).
    BookmarksVisible,
    /// Apex `2026-02-private-channel-hiding`: the obfuscation capabilities variant (D14).
    ChannelObfuscation,
}

/// `{b, r}` of the v2 bookmarks config: the limits of a non-Nitro account (tier 2 gets 200 and 50).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BookmarkLimits {
    pub bookmarks: u32,
    pub reminders: u32,
}

/// A legacy user assignment: the server's bucket, and A/A mode (config stays the default).
#[derive(Clone, Debug)]
struct LegacyUser {
    bucket: i64,
    aa: bool,
}

#[derive(Clone, Debug)]
enum Filter {
    GuildIds(Vec<u64>),
    IdRange(Option<i128>, Option<i128>),
    AgeDays(Option<i128>, Option<i128>),
    MemberCount(Option<i128>, Option<i128>),
    HasFeature(Vec<String>),
    HubTypes(Vec<i64>),
    HasVanity(bool),
    InRangeByHash {
        key: String,
        target: i64,
    },
    /// A kind this build does not know: skipped, i.e. it passes (experiments.md 2b).
    Skipped,
}

#[derive(Clone, Debug)]
struct Population {
    /// `(bucket, [start, end))` ranges per bucket, in order.
    buckets: Vec<(i64, Vec<(i64, i64)>)>,
    filters: Vec<Filter>,
}

#[derive(Clone, Debug)]
struct GuildExp {
    hash_key: Option<String>,
    populations: Vec<Population>,
    overrides: HashMap<u64, i64>,
    overrides_formatted: Vec<Vec<Population>>,
    holdout_name: Option<String>,
    holdout_bucket: Option<i64>,
    aa: bool,
}

/// What the filters and the gates read of a guild, from its payload (`properties` or the top
/// level; `member_count` is top level).
#[derive(Clone, Debug, Default)]
struct GuildInfo {
    features: Vec<String>,
    member_count: Option<u64>,
    vanity: bool,
    hub_type: Option<i64>,
    /// `moderator_reporting` enabled with a report channel.
    reporting: bool,
}

/// One tuple of an apex guild unit (no config).
#[derive(Clone, Debug)]
struct GuildAssign {
    hash: u64,
    variant: Option<i64>,
}

/// Everything slice X keeps besides `State::apex_experiments`. Rebuilt by READY.
#[derive(Clone, Debug, Default)]
pub(crate) struct Experiments {
    /// `None` until READY carries a readable `experiments` array. A tuple that cannot be read is
    /// `None` under its key: that experiment is Unknown.
    user: Option<HashMap<u64, Option<LegacyUser>>>,
    guild_exps: Option<HashMap<u64, Option<GuildExp>>>,
    guild_units: HashMap<u64, Vec<GuildAssign>>,
    guilds: HashMap<u64, GuildInfo>,
    /// The signed-in user's `flags`.
    flags: Option<u64>,
    /// Server-pushed session overrides: name to (variant, stamp of the apex data when it arrived).
    overrides: HashMap<String, (i64, u64)>,
}

fn int(v: &Value) -> Option<i128> {
    v.as_i64()
        .map(i128::from)
        .or_else(|| v.as_u64().map(i128::from))
        .or_else(|| v.as_str()?.trim().parse().ok())
}

/// Guild fields sit on the guild object or in `properties` (CLIENT_STATE_V2).
fn field<'a>(d: &'a Value, key: &str) -> Option<&'a Value> {
    d.get(key)
        .filter(|v| !v.is_null())
        .or_else(|| d.get("properties")?.get(key).filter(|v| !v.is_null()))
}

fn parse_pop(v: &Value) -> Option<Population> {
    let [ranges, filters] = v.as_array()?.as_slice() else {
        return None;
    };
    let buckets = ranges
        .as_array()?
        .iter()
        .map(|b| {
            let b = b.as_array()?;
            let spans = b
                .get(1)?
                .as_array()?
                .iter()
                .map(|p| {
                    Some((
                        i64::try_from(int(p.get("s")?)?).ok()?,
                        i64::try_from(int(p.get("e")?)?).ok()?,
                    ))
                })
                .collect::<Option<Vec<_>>>()?;
            Some((i64::try_from(int(b.first()?)?).ok()?, spans))
        })
        .collect::<Option<Vec<_>>>()?;
    let filters = filters
        .as_array()?
        .iter()
        .map(parse_filter)
        .collect::<Option<Vec<_>>>()?;
    Some(Population { buckets, filters })
}

fn parse_filter(v: &Value) -> Option<Filter> {
    let a = v.as_array()?;
    let kind = u32::try_from(a.first()?.as_u64()?).unwrap_or(0);
    if ![
        K_GUILD_IDS,
        K_ID_RANGE,
        K_AGE_RANGE,
        K_MEMBER_RANGE,
        K_HAS_FEATURE,
        K_HUB_TYPES,
        K_HAS_VANITY,
        K_IN_RANGE_BY_HASH,
    ]
    .contains(&kind)
    {
        return Some(Filter::Skipped);
    }
    let pairs = a.get(1)?.as_array()?;
    let keyed = |key: u32| {
        pairs.iter().find_map(|p| {
            let p = p.as_array()?;
            (p.first()?.as_u64()? == u64::from(key)).then(|| p.get(1))
        })
    };
    // Absent or null is an open bound; a present value must read.
    let bound = |key: u32| match keyed(key).flatten() {
        None | Some(Value::Null) => Some(None),
        Some(v) => Some(Some(int(v)?)),
    };
    // Kinds that carry one list, bool or number read the first pair's value, whatever its key.
    let first = || pairs.first()?.as_array()?.get(1);
    Some(match kind {
        K_GUILD_IDS => Filter::GuildIds(
            keyed(P_GUILD_IDS)??
                .as_array()?
                .iter()
                .map(|g| u64::try_from(int(g)?).ok())
                .collect::<Option<_>>()?,
        ),
        K_ID_RANGE => Filter::IdRange(bound(P_MIN)?, bound(P_MAX)?),
        K_AGE_RANGE => Filter::AgeDays(bound(P_MIN)?, bound(P_MAX)?),
        K_MEMBER_RANGE => Filter::MemberCount(bound(P_MIN)?, bound(P_MAX)?),
        K_HAS_FEATURE => Filter::HasFeature(
            first()?
                .as_array()?
                .iter()
                .map(|f| f.as_str().map(str::to_owned))
                .collect::<Option<_>>()?,
        ),
        K_HUB_TYPES => Filter::HubTypes(
            first()?
                .as_array()?
                .iter()
                .map(|t| i64::try_from(int(t)?).ok())
                .collect::<Option<_>>()?,
        ),
        K_HAS_VANITY => Filter::HasVanity(first()?.as_bool()?),
        _ => Filter::InRangeByHash {
            key: keyed(P_HASH_KEY)??.as_str()?.to_owned(),
            target: i64::try_from(int(keyed(P_TARGET)??)?).ok()?,
        },
    })
}

fn parse_guild_exp(a: &[Value]) -> Option<GuildExp> {
    let opt_str = |v: Option<&Value>| match v {
        None | Some(Value::Null) => Some(None),
        Some(v) => Some(Some(v.as_str()?.to_owned())),
    };
    let mut overrides = HashMap::new();
    if let Some(list) = a.get(4).filter(|v| !v.is_null()) {
        for o in list.as_array()? {
            let b = i64::try_from(int(o.get("b")?)?).ok()?;
            for g in o.get("k")?.as_array()? {
                overrides.insert(id(g)?, b);
            }
        }
    }
    let overrides_formatted = match a.get(5).filter(|v| !v.is_null()) {
        None => Vec::new(),
        Some(list) => list
            .as_array()?
            .iter()
            .map(|pops| pops.as_array()?.iter().map(parse_pop).collect())
            .collect::<Option<_>>()?,
    };
    Some(GuildExp {
        hash_key: opt_str(a.get(1))?,
        populations: a
            .get(3)?
            .as_array()?
            .iter()
            .map(parse_pop)
            .collect::<Option<_>>()?,
        overrides,
        overrides_formatted,
        holdout_name: opt_str(a.get(6))?,
        holdout_bucket: match a.get(7).filter(|v| !v.is_null()) {
            None => None,
            Some(v) => Some(i64::try_from(int(v)?).ok()?),
        },
        aa: a.get(8).and_then(Value::as_u64) == Some(1),
    })
}

fn parse_legacy_user(a: &[Value]) -> Option<LegacyUser> {
    Some(LegacyUser {
        bucket: i64::try_from(int(a.get(2)?)?).ok()?,
        aa: a.get(6).and_then(Value::as_u64) == Some(1),
    })
}

/// A READY legacy array as a map from `h(name)` to the parsed tuple. A tuple without a readable
/// key cannot be told from the one we want, so it is left out.
fn parse_legacy<T>(
    list: &Value,
    parse: impl Fn(&[Value]) -> Option<T>,
) -> Option<HashMap<u64, Option<T>>> {
    Some(
        list.as_array()?
            .iter()
            .filter_map(|t| {
                let a = t.as_array()?;
                Some((a.first()?.as_u64()?, parse(a)))
            })
            .collect(),
    )
}

impl Experiments {
    /// READY: the legacy arrays, the user's flags and every guild it lists.
    pub(crate) fn ready(&mut self, d: &Value) {
        self.flags = d
            .get("user")
            .and_then(|u| u.get("flags"))
            .and_then(Value::as_u64);
        self.user = d
            .get("experiments")
            .and_then(|l| parse_legacy(l, parse_legacy_user));
        self.guild_exps = d
            .get("guild_experiments")
            .and_then(|l| parse_legacy(l, parse_guild_exp));
        for g in d
            .get("guilds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            self.guild(g);
        }
    }

    /// A guild object (READY, GUILD_CREATE, full or partial): its apex unit, and its info when the
    /// payload carries the guild's own fields (a partial guild without them keeps what was held).
    pub(crate) fn guild(&mut self, d: &Value) {
        let Some(gid) = get_id(d, "id") else {
            return;
        };
        if let Some(list) = d
            .get("experiments")
            .and_then(|b| b.get("assignments"))
            .and_then(Value::as_array)
        {
            self.guild_units.insert(
                gid,
                list.iter()
                    .filter_map(|t| {
                        let t = t.as_array()?;
                        Some(GuildAssign {
                            hash: t.first()?.as_u64()?,
                            variant: t.get(1).and_then(Value::as_i64),
                        })
                    })
                    .collect(),
            );
        }
        let Some(features) = field(d, "features").and_then(Value::as_array) else {
            return;
        };
        let info = self.guilds.entry(gid).or_default();
        info.features = features
            .iter()
            .filter_map(|f| f.as_str().map(str::to_owned))
            .collect();
        info.vanity = field(d, "vanity_url_code").is_some();
        info.hub_type = field(d, "hub_type").and_then(Value::as_i64);
        info.reporting = field(d, "moderator_reporting").is_some_and(|m| {
            m.get("moderator_reporting_enabled")
                .and_then(Value::as_bool)
                == Some(true)
                && m.get("moderator_report_channel_id")
                    .is_some_and(|c| !c.is_null())
        });
        if let Some(n) = d.get("member_count").and_then(Value::as_u64) {
            info.member_count = Some(n);
        }
    }
}

/// What a lookup of one apex user assignment found.
struct Assign<'a> {
    variant: Option<i64>,
    flags: u64,
    config: Option<&'a Value>,
}

/// How an experiment turns into an answer.
enum Def {
    LegacyUser,
    LegacyGuild,
    /// The assignment's variant and config to "on".
    Apex(fn(Option<i64>, Option<&Value>) -> bool),
}

fn definition(name: &str) -> Option<Def> {
    Some(match name {
        TIDA_WEBFORM => Def::LegacyUser,
        OFFICIAL_MESSAGES => Def::LegacyGuild,
        // Variations 1 and 2 are on (3 is off for the obfuscation experiment).
        BOOKMARKS_V1 | OBFUSCATION => Def::Apex(|v, _| matches!(v, Some(1 | 2))),
        BOOKMARKS_V2 => Def::Apex(|v, c| matches!(v, Some(1 | 2)) && v2_limits(c).is_some()),
        CONVERSATIONS => Def::Apex(|v, _| v == Some(2)),
        _ => return None,
    })
}

/// The v2 config is a JSON string `{"b": n, "r": n}`; the official constructor throws on a missing
/// member and the whole config then falls back to the default (experiments.md 4.7).
fn v2_limits(config: Option<&Value>) -> Option<BookmarkLimits> {
    let parsed: Value = serde_json::from_str(config?.as_str()?).ok()?;
    Some(BookmarkLimits {
        bookmarks: u32::try_from(parsed.get("b")?.as_u64()?).ok()?,
        reminders: u32::try_from(parsed.get("r")?.as_u64()?).ok()?,
    })
}

struct Eval {
    bucket: i64,
    on: bool,
}

fn in_range(v: i128, min: Option<i128>, max: Option<i128>) -> bool {
    !(min.is_some_and(|m| v < m) || max.is_some_and(|m| v > m))
}

fn guild_str(guild: Option<u64>) -> String {
    guild.map(|g| g.to_string()).unwrap_or_default()
}

impl Filter {
    /// `None`: an input this filter needs is not known, so neither is the answer.
    fn passes(
        &self,
        guild: Option<u64>,
        infos: &HashMap<u64, GuildInfo>,
        now: u64,
    ) -> Option<bool> {
        let info = || infos.get(&guild?);
        Some(match self {
            Filter::Skipped => true,
            Filter::GuildIds(ids) => guild.is_some_and(|g| ids.contains(&g)),
            Filter::IdRange(min, max) => in_range(i128::from(guild?), *min, *max),
            Filter::AgeDays(min, max) => {
                let created = i128::from(guild? >> 22) + DISCORD_EPOCH_MS;
                in_range((i128::from(now) - created).div_euclid(DAY_MS), *min, *max)
            }
            Filter::MemberCount(min, max) => {
                in_range(i128::from(info()?.member_count?), *min, *max)
            }
            // No guild at all is a definite no; a guild we hold no payload for is unknown.
            Filter::HasFeature(list) => match guild {
                None => false,
                Some(_) => {
                    let i = info()?;
                    list.iter().any(|f| i.features.contains(f))
                }
            },
            Filter::HubTypes(list) => match guild {
                None => false,
                Some(_) => info()?.hub_type.is_some_and(|t| list.contains(&t)),
            },
            Filter::HasVanity(want) => match guild {
                None => !*want,
                Some(_) => *want == info()?.vanity,
            },
            Filter::InRangeByHash { key, target } => {
                // The client doubles the already-unsigned hash before the modulo
                // (experiments.md 2b): `(2x) % 10000` for every x > 0.
                let x = u64::from(murmur3(format!("{key}:{}", guild_str(guild)).as_bytes()));
                i64::try_from(x * 2 % 10_000).ok()? < *target
            }
        })
    }
}

struct GuildCtx<'a> {
    exps: &'a HashMap<u64, Option<GuildExp>>,
    infos: &'a HashMap<u64, GuildInfo>,
    now: u64,
}

impl GuildCtx<'_> {
    /// The first population whose filters all pass decides, by the range `x` falls in (CONTROL, 0,
    /// when in none); NOT_ELIGIBLE (-1) is `Some(None)`. Outer `None`: unknown.
    fn pops(&self, pops: &[Population], guild: Option<u64>, x: i64) -> Option<Option<i64>> {
        for pop in pops {
            let mut fails = false;
            let mut unknown = false;
            for f in &pop.filters {
                match f.passes(guild, self.infos, self.now) {
                    Some(false) => fails = true,
                    None => unknown = true,
                    Some(true) => {}
                }
            }
            if fails {
                continue;
            }
            if unknown {
                return None;
            }
            let b = pop
                .buckets
                .iter()
                .find(|(_, spans)| spans.iter().any(|&(s, e)| s <= x && x < e))
                .map_or(0, |(b, _)| *b);
            return Some((b != -1).then_some(b));
        }
        Some(None)
    }

    /// `getGuildExperimentDescriptor` (experiments.md 3b): the guild's bucket, `Some(None)` when
    /// the official descriptor is null (not eligible, or the experiment is not in the payload).
    fn bucket(&self, name: &str, guild: Option<u64>, depth: u8) -> Option<Option<i64>> {
        let Some(exp) = self.exps.get(&u64::from(hs(name))) else {
            return Some(None);
        };
        let exp = exp.as_ref()?;
        if let Some(&b) = guild.and_then(|g| exp.overrides.get(&g)) {
            return Some((b != -1).then_some(b));
        }
        let key = format!(
            "{}:{}",
            exp.hash_key.as_deref().unwrap_or(name),
            guild_str(guild)
        );
        let x = i64::from(murmur3(key.as_bytes()) % 10_000);
        for list in &exp.overrides_formatted {
            if let Some(b) = self.pops(list, guild, x)? {
                return Some(Some(b));
            }
        }
        let Some(b) = self.pops(&exp.populations, guild, x)? else {
            return Some(None);
        };
        if let (Some(holdout), Some(held)) = (&exp.holdout_name, exp.holdout_bucket) {
            if holdout != name {
                if depth >= MAX_HOLDOUT_DEPTH {
                    return None;
                }
                if self.bucket(holdout, guild, depth + 1)? == Some(held) {
                    return Some(None);
                }
            }
        }
        Some(Some(b))
    }
}

impl State {
    /// The signed-in user's apex assignment of `name`: a session override first (it wins over the
    /// server, experiments.md 3.11), then the user unit of the newest apex object that lists it.
    /// Outer `None`: unknown (no apex data, no user unit, or an unreadable tuple); inner `None`:
    /// the unit is known and carries no assignment.
    fn apex_user(&self, name: &str) -> Option<Option<Assign<'_>>> {
        if let Some(&(variant, stamp)) = self.experiments.overrides.get(name) {
            if stamp == self.apex_stamp() {
                return Some(Some(Assign {
                    variant: Some(variant),
                    flags: FLAG_IS_OVERRIDE,
                    config: None,
                }));
            }
        }
        let uid = self.user_id?.to_string();
        // Each object overwrites the units it lists (STATE_UPDATE), so the newest listing wins.
        let block = self
            .apex_experiments
            .iter()
            .rev()
            .find_map(|o| o.get("assignments")?.get("1")?.get(&uid))?;
        let hash = u64::from(hs(name));
        let Some(t) = block
            .get("assignments")?
            .as_array()?
            .iter()
            .find(|t| t.get(0).and_then(Value::as_u64) == Some(hash))
        else {
            return Some(None);
        };
        let variant = match t.get(1)? {
            Value::Null => None,
            v => Some(i64::try_from(int(v)?).ok()?),
        };
        let flags = match t.get(2) {
            None | Some(Value::Null) => 0,
            Some(v) => v.as_u64()?,
        };
        Some(Some(Assign {
            variant,
            flags,
            config: t.get(5),
        }))
    }

    /// A fingerprint of the apex data held, to tell that a STATE_UPDATE arrived since an override
    /// did (the official client drops session overrides then, experiments.md 1d; `state_update`
    /// is not ours to hook).
    // ponytail: hashes the newest apex object (READY's, when nothing newer); only called while an
    // override exists, which is rare. A STATE_UPDATE identical to the previous one is missed.
    fn apex_stamp(&self) -> u64 {
        let mut s = DefaultHasher::new();
        self.apex_experiments.len().hash(&mut s);
        if let Some(last) = self.apex_experiments.last() {
            last.to_string().hash(&mut s);
        }
        s.finish()
    }

    fn evaluate(&self, name: &str, guild: Option<u64>, now: u64) -> Option<Eval> {
        match definition(name)? {
            Def::LegacyUser => {
                let user = self.experiments.user.as_ref()?;
                Some(match user.get(&u64::from(hs(name))) {
                    // Loaded, and the server assigned nothing: NOT_ELIGIBLE.
                    None => Eval {
                        bucket: -1,
                        on: false,
                    },
                    Some(u) => {
                        let u = u.as_ref()?;
                        Eval {
                            bucket: u.bucket,
                            on: !u.aa && u.bucket == 1,
                        }
                    }
                })
            }
            Def::LegacyGuild => {
                let ctx = GuildCtx {
                    exps: self.experiments.guild_exps.as_ref()?,
                    infos: &self.experiments.guilds,
                    now,
                };
                let bucket = ctx.bucket(name, guild, 0)?.unwrap_or(-1);
                let aa = ctx
                    .exps
                    .get(&u64::from(hs(name)))
                    .and_then(|e| e.as_ref())
                    .is_some_and(|e| e.aa);
                Some(Eval {
                    bucket,
                    on: !aa && bucket == 1,
                })
            }
            Def::Apex(on) => Some(match self.apex_user(name)? {
                None => Eval {
                    bucket: -1,
                    on: false,
                },
                Some(a) => Eval {
                    bucket: a.variant.unwrap_or(-1),
                    // Default config when the variant is null or the assignment is only an
                    // eligibility marker (experiments.md 3.13).
                    on: a.variant.is_some()
                        && a.flags & FLAG_USE_AS_ELIGIBILITY == 0
                        && on(a.variant, a.config),
                },
            }),
        }
    }

    /// The experiment's bucket for the account (and `guild`, for guild experiments). Legacy: the
    /// official bucket (-1 NOT_ELIGIBLE, 0 CONTROL, 1.. treatments). Apex: the assigned variant,
    /// -1 when none. `None` when it cannot be known, including a name this build has no
    /// definition for.
    pub fn experiment_bucket(&self, name: &str, guild: Option<u64>) -> Option<i64> {
        self.bucket_at(name, guild, now_ms())
    }

    /// `experiment_bucket` at a given time (the guild-age filter reads the clock).
    pub(crate) fn bucket_at(&self, name: &str, guild: Option<u64>, now: u64) -> Option<i64> {
        self.evaluate(name, guild, now).map(|e| e.bucket)
    }

    /// Whether the experiment's treatment is on for the account (and `guild`). `Unknown` when it
    /// cannot be known, including a name this build has no definition for.
    pub fn experiment_gate(&self, name: &str, guild: Option<u64>) -> Gate {
        self.gate_at(name, guild, now_ms())
    }

    pub(crate) fn gate_at(&self, name: &str, guild: Option<u64>, now: u64) -> Gate {
        self.evaluate(name, guild, now)
            .map_or(Gate::Unknown, |e| Gate::of(e.on))
    }

    fn guild_feature(&self, guild: Option<u64>, feature: &str) -> Gate {
        match guild {
            None => Gate::Disabled,
            Some(g) => self.experiments.guilds.get(&g).map_or(Gate::Unknown, |i| {
                Gate::of(i.features.iter().any(|f| f == feature))
            }),
        }
    }

    /// One of the menu's gates (experiments.md "Gates").
    pub fn menu_gate(&self, gate: MenuGate, guild: Option<u64>) -> Gate {
        match gate {
            MenuGate::Translate => self
                .experiments
                .flags
                .map_or(Gate::Unknown, |f| Gate::of(f & USER_FLAG_STAFF != 0)),
            MenuGate::ViewConversation => self
                .experiment_gate(CONVERSATIONS, None)
                .and(self.guild_feature(guild, FEATURE_CONVERSATIONS)),
            MenuGate::ReportToModerators => match guild {
                None => Gate::Disabled,
                Some(g) => self
                    .experiments
                    .guilds
                    .get(&g)
                    .map_or(Gate::Unknown, |i| Gate::of(i.reporting)),
            },
            MenuGate::OfficialMessage => match guild {
                None => Gate::Disabled,
                Some(_) => self
                    .experiment_gate(OFFICIAL_MESSAGES, guild)
                    .and(self.guild_feature(guild, FEATURE_VERIFIED)),
            },
            MenuGate::TidaWebform => self.experiment_gate(TIDA_WEBFORM, None),
            MenuGate::BookmarksV1 => self.experiment_gate(BOOKMARKS_V1, None),
            MenuGate::BookmarksV2 => self.experiment_gate(BOOKMARKS_V2, None),
            MenuGate::BookmarksVisible => self
                .experiment_gate(BOOKMARKS_V1, None)
                .or(self.experiment_gate(BOOKMARKS_V2, None)),
            MenuGate::ChannelObfuscation => self.experiment_gate(OBFUSCATION, None),
        }
    }

    /// The v2 bookmark limits for a non-Nitro account, when v2 is on.
    pub fn bookmark_v2_limits(&self) -> Option<BookmarkLimits> {
        if self.experiment_gate(BOOKMARKS_V2, None) != Gate::Enabled {
            return None;
        }
        v2_limits(self.apex_user(BOOKMARKS_V2)??.config)
    }

    /// An apex experiment of kind guild (experiments.md 3.12): needs the user's own assignment of
    /// the same name (an override is used as is; otherwise it must be an eligibility marker), then
    /// the guild unit's variant. Outer `None`: unknown. No gate uses one yet.
    pub fn guild_apex_variant(&self, name: &str, guild: u64) -> Option<Option<i64>> {
        let Some(user) = self.apex_user(name)? else {
            return Some(None);
        };
        if user.flags & FLAG_IS_OVERRIDE != 0 {
            return Some(user.variant);
        }
        if user.flags & FLAG_USE_AS_ELIGIBILITY == 0 {
            return Some(None);
        }
        let hash = u64::from(hs(name));
        Some(
            self.experiments
                .guild_units
                .get(&guild)?
                .iter()
                .find(|t| t.hash == hash)
                .and_then(|t| t.variant),
        )
    }

    /// EXPERIMENT_SESSION_OVERRIDE_CREATE / _DELETE (experiments.md 1d).
    pub(crate) fn session_override(&mut self, d: &Value, create: bool) -> bool {
        let Some(name) = d.get("experiment_name").and_then(Value::as_str) else {
            return false;
        };
        if !create {
            return self.experiments.overrides.remove(name).is_some();
        }
        let Some(variant) = d.get("variant_id").and_then(Value::as_i64) else {
            return false;
        };
        let stamp = self.apex_stamp();
        self.experiments
            .overrides
            .insert(name.to_owned(), (variant, stamp));
        true
    }

    /// USER_UPDATE of the signed-in user: its `flags` (the Translate gate reads the staff bit).
    pub(crate) fn user_flags_update(&mut self, d: &Value) -> bool {
        let Some(flags) = d.get("flags").and_then(Value::as_u64) else {
            return false;
        };
        if get_id(d, "id") != self.user_id || self.experiments.flags == Some(flags) {
            return false;
        }
        self.experiments.flags = Some(flags);
        true
    }
}
