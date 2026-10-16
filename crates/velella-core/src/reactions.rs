//! Read-only reactions of buffered messages (wave 3, track X). A message's `reactions` array is
//! parsed once; the four MESSAGE_REACTION_* dispatches then edit the list in place. Order is
//! first-seen, at most `MAX_REACTIONS` are kept (Discord's own limit), and a reaction whose count
//! reaches 0 is removed. Burst (super) reactions arrive in the same events and count as normal.
use crate::messages::{emoji_url, id as snowflake};
use serde_json::Value;

/// Distinct reactions kept per message.
pub(crate) const MAX_REACTIONS: usize = 20;

/// One emoji's reaction on a message. A custom emoji has an `id` and a CDN `url` built from it
/// (never from payload text); a unicode emoji is its `name` and has neither.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Reaction {
    pub id: Option<u64>,
    pub name: String,
    pub animated: bool,
    pub url: Option<String>,
    pub count: u32,
    /// The self user has reacted.
    pub me: bool,
}

/// The emoji object of a reaction or of a reaction event: (id, name, animated). A custom emoji
/// needs an id; a unicode one needs a non-empty name.
fn emoji(v: &Value) -> Option<(Option<u64>, String, bool)> {
    let id = v.get("id").filter(|i| !i.is_null()).map(snowflake);
    let name = v
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let animated = v.get("animated").and_then(Value::as_bool).unwrap_or(false);
    match id {
        Some(None) => None, // an id that is not a snowflake
        Some(Some(id)) => Some((Some(id), name, animated)),
        None if name.is_empty() => None,
        None => Some((None, name, false)),
    }
}

fn new(emoji: (Option<u64>, String, bool), count: u32, me: bool) -> Reaction {
    let (id, name, animated) = emoji;
    Reaction {
        url: id.map(|id| emoji_url(id, animated)),
        id,
        name,
        animated,
        count,
        me,
    }
}

/// The `reactions` array of a message object: entries with a usable emoji and a positive count,
/// the first `MAX_REACTIONS`.
pub(crate) fn parse(v: Option<&Value>) -> Vec<Reaction> {
    v.and_then(Value::as_array)
        .map(|list| {
            list.iter()
                .filter_map(|r| {
                    let count = r
                        .get("count")
                        .and_then(Value::as_u64)
                        .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
                        .filter(|n| *n > 0)?;
                    let me = r.get("me").and_then(Value::as_bool).unwrap_or(false);
                    Some(new(emoji(r.get("emoji")?)?, count, me))
                })
                .take(MAX_REACTIONS)
                .collect()
        })
        .unwrap_or_default()
}

/// Applies one reaction dispatch (`name`, payload `d`) to a message's list; `me` is the self user
/// id. Returns whether anything changed.
pub(crate) fn apply(list: &mut Vec<Reaction>, name: &str, d: &Value, me: Option<u64>) -> bool {
    if name == "MESSAGE_REACTION_REMOVE_ALL" {
        return !std::mem::take(list).is_empty();
    }
    if name == "MESSAGE_REACTION_ADD_MANY" {
        return add_many(list, d, me);
    }
    let Some(emoji) = d.get("emoji").and_then(emoji) else {
        return false;
    };
    let same = |r: &Reaction| r.id == emoji.0 && (emoji.0.is_some() || r.name == emoji.1);
    let at = list.iter().position(same);
    let own = me.is_some() && d.get("user_id").and_then(snowflake) == me;
    match (name, at) {
        ("MESSAGE_REACTION_ADD", Some(i)) => {
            list[i].count = list[i].count.saturating_add(1);
            list[i].me |= own;
        }
        ("MESSAGE_REACTION_ADD", None) if list.len() < MAX_REACTIONS => {
            list.push(new(emoji, 1, own));
        }
        ("MESSAGE_REACTION_REMOVE", Some(i)) => {
            list[i].count -= 1; // counts in the list are always > 0
            if own {
                list[i].me = false;
            }
            if list[i].count == 0 {
                list.remove(i);
            }
        }
        ("MESSAGE_REACTION_REMOVE_EMOJI", Some(i)) => {
            list.remove(i);
        }
        _ => return false,
    }
    true
}

/// MESSAGE_REACTION_ADD_MANY (bit 13, identify.md §1): `reactions: [{users, emoji}]`, one
/// MESSAGE_REACTION_ADD per listed user, as the official `addReactionBatch` does.
fn add_many(list: &mut Vec<Reaction>, d: &Value, me: Option<u64>) -> bool {
    let mut changed = false;
    for r in d
        .get("reactions")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let Some(emoji) = r.get("emoji") else {
            continue;
        };
        for user in r
            .get("users")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let one = serde_json::json!({"emoji": emoji, "user_id": user});
            changed |= apply(list, "MESSAGE_REACTION_ADD", &one, me);
        }
    }
    changed
}
