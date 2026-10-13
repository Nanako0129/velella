//! Offline fixture tests on hand-written synthetic JSON in the capabilities-253 READY shape.
use crate::state::State;
use serde_json::{json, Value};

trait Ap {
    fn ap(&mut self, event: &str, d: Value) -> bool;
}
impl Ap for State {
    fn ap(&mut self, event: &str, d: Value) -> bool {
        self.apply(event, &d)
    }
}

const ME: u64 = 1;
const VIEW: u64 = 1 << 10;
const ADMIN: u64 = 1 << 3;

fn user(id: u64, name: &str) -> Value {
    json!({"id": id.to_string(), "username": name, "global_name": format!("{name} G"), "avatar": format!("av{id}"), "discriminator": "0"})
}

fn guild(id: u64) -> Value {
    json!({"id": id.to_string(), "name": format!("g{id}"), "icon": null, "owner_id": "999",
        "roles": [{"id": id.to_string(), "permissions": "0", "position": 0}], "channels": []})
}

fn ready(guilds: Vec<Value>) -> Value {
    json!({
        "user": user(ME, "me"),
        "users": [user(2, "alice"), user(3, "bob")],
        "guilds": guilds,
        "private_channels": [
            {"id": "500", "type": 1, "recipient_ids": ["2"], "last_message_id": "9"},
            {"id": "501", "type": 3, "name": "club", "icon": "ic", "recipient_ids": ["2", "3"]}
        ],
        "user_settings": {"guild_folders": [{"id": null, "guild_ids": ["10"]}, {"id": 7, "name": "f", "guild_ids": ["20", "30"]}]},
        "session_id": "s", "resume_gateway_url": "wss://gateway-x.discord.gg/"
    })
}

fn applied(event: &str, d: Value) -> State {
    let mut s = State::default();
    assert!(s.ap(event, d));
    s
}

#[test]
fn ready_dm_with_only_recipient_ids_resolves_from_users() {
    let s = applied("READY", ready(vec![]));
    let dm = &s.private_channels[0];
    assert_eq!(dm.recipient_ids, vec![2]);
    let who = s.recipients(dm);
    assert_eq!(who.len(), 1);
    assert_eq!(who[0].username, "alice");
    assert_eq!(who[0].global_name.as_deref(), Some("alice G"));
    assert_eq!(s.recipients(&s.private_channels[1]).len(), 2);
    assert_eq!(s.private_channels[0].last_message_id, Some(9));
    assert_eq!(s.users[&ME].username, "me");
}

#[test]
fn guild_fields_fall_back_to_properties() {
    let g = json!({"id": "10", "properties": {"name": "nested", "icon": "ic", "owner_id": "42"}});
    let s = applied("READY", ready(vec![g]));
    let g = s.guild(10).unwrap();
    assert_eq!(g.name.as_deref(), Some("nested"));
    assert_eq!(g.icon.as_deref(), Some("ic"));
    assert_eq!(g.owner_id, Some(42));
    // Top-level fields still win when present.
    let s = applied("READY", ready(vec![guild(11)]));
    assert_eq!(s.guild(11).unwrap().name.as_deref(), Some("g11"));
}

fn viewer_guild(member_roles_entry: Value) -> State {
    let mut g = guild(100);
    g["roles"] = json!([
        {"id": "100", "permissions": "0"},
        {"id": "200", "permissions": VIEW.to_string()}
    ]);
    g["channels"] = json!([
        {"id": "300", "type": 0, "permission_overwrites": [{"id": "100", "type": 0, "allow": "0", "deny": VIEW.to_string()}]},
        {"id": "301", "type": 0, "permission_overwrites": []}
    ]);
    let mut r = ready(vec![g]);
    r["merged_members"] = json!([member_roles_entry]);
    applied("READY", r)
}

#[test]
fn merged_members_roles_feed_view_channel() {
    // Own member holds role 200 (VIEW_CHANNEL); @everyone has nothing.
    let s = viewer_guild(json!([{"user_id": "1", "roles": ["200"]}]));
    let g = s.guild(100).unwrap();
    assert_eq!(g.member_roles, Some(vec![200]));
    assert_eq!(s.can_view_channel(&g.channels[1]), Some(true));
    // Channel 300 denies VIEW to @everyone, role 200 allows nothing there: role grant is wiped.
    assert_eq!(s.can_view_channel(&g.channels[0]), Some(false));
    // Without the role the same channel is invisible.
    let s = viewer_guild(json!([{"user_id": "1", "roles": []}]));
    assert_eq!(
        s.can_view_channel(&s.guild(100).unwrap().channels[1]),
        Some(false)
    );
    // Another user's entry in merged_members does not count as ours.
    let s = viewer_guild(json!([{"user_id": "77", "roles": ["200"]}]));
    assert_eq!(s.can_view_channel(&s.guild(100).unwrap().channels[1]), None);
}

#[test]
fn permission_rules() {
    let mut s = viewer_guild(json!([{"user_id": "1", "roles": ["200"]}]));
    let chan = |s: &State| s.guild(100).unwrap().channels[0].clone();
    // Role overwrite allow beats @everyone deny; member overwrite beats both.
    let ow = json!([
        {"id": "100", "type": 0, "allow": "0", "deny": VIEW.to_string()},
        {"id": "200", "type": 0, "allow": VIEW.to_string(), "deny": "0"}
    ]);
    assert!(s.ap(
        "CHANNEL_UPDATE",
        json!({"id": "300", "guild_id": "100", "type": 0, "permission_overwrites": ow})
    ));
    assert_eq!(s.can_view_channel(&chan(&s)), Some(true));
    let ow = json!([
        {"id": "200", "type": 0, "allow": "0", "deny": VIEW.to_string()},
        {"id": "1", "type": 1, "allow": VIEW.to_string(), "deny": "0"}
    ]);
    s.ap(
        "CHANNEL_UPDATE",
        json!({"id": "300", "guild_id": "100", "type": 0, "permission_overwrites": ow}),
    );
    assert_eq!(s.can_view_channel(&chan(&s)), Some(true));
    // ADMINISTRATOR sees through a deny.
    let ow = json!([{"id": "100", "type": 0, "allow": "0", "deny": VIEW.to_string()}]);
    s.ap(
        "CHANNEL_UPDATE",
        json!({"id": "300", "guild_id": "100", "type": 0, "permission_overwrites": ow}),
    );
    assert_eq!(s.can_view_channel(&chan(&s)), Some(false));
    s.ap(
        "GUILD_ROLE_UPDATE",
        json!({"guild_id": "100", "role": {"id": "200", "permissions": ADMIN.to_string()}}),
    );
    assert_eq!(s.can_view_channel(&chan(&s)), Some(true));
    // Owner sees everything even with no roles.
    s.ap("GUILD_UPDATE", json!({"id": "100", "owner_id": "1"}));
    s.ap(
        "GUILD_ROLE_DELETE",
        json!({"guild_id": "100", "role_id": "200"}),
    );
    assert_eq!(s.can_view_channel(&chan(&s)), Some(true));
}

#[test]
fn undecidable_permissions_are_none() {
    let s = viewer_guild(json!([{"user_id": "1", "roles": ["200"]}]));
    let base = s.guild(100).unwrap().clone();
    let probe = |edit: &dyn Fn(&mut State)| {
        let mut s = s.clone();
        edit(&mut s);
        s.can_view_channel(&s.guild(100).unwrap().channels[1])
    };
    assert_eq!(probe(&|_| {}), Some(true));
    assert_eq!(probe(&|s| s.guilds[0].owner_id = None), None); // owner unknown
    assert_eq!(probe(&|s| s.guilds[0].member_roles = None), None); // own roles unknown
    assert_eq!(probe(&|s| s.guilds[0].roles = None), None); // role table unknown
    assert_eq!(probe(&|s| s.guilds[0].member_roles = Some(vec![555])), None); // unknown role id
    assert_eq!(
        probe(&|s| s.guilds[0].channels[1].permission_overwrites = None),
        None
    );
    assert_eq!(probe(&|s| s.guilds[0].unavailable = true), None);
    assert_eq!(probe(&|s| s.user_id = None), None);
    // An unreadable overwrite entry makes the whole set undecidable.
    let mut s2 = s.clone();
    s2.ap("CHANNEL_UPDATE", json!({"id": "301", "guild_id": "100", "type": 0, "permission_overwrites": [{"id": "1", "type": 9}]}));
    assert_eq!(
        s2.can_view_channel(&s2.guild(100).unwrap().channels[1]),
        None
    );
    assert_eq!(base.channels.len(), 2);
}

#[test]
fn lazy_private_channel_resolves_all_four_user_fields_from_ready_users() {
    let mut s = applied("READY", ready(vec![]));
    let before = s.revision;
    let supplemental = json!({"lazy_private_channels": [{"id": "600", "type": 1, "recipient_ids": ["3"], "last_message_id": "55"}]});
    assert!(s.ap("READY_SUPPLEMENTAL", supplemental));
    assert!(s.revision > before);
    let dm = s
        .private_channels
        .iter()
        .find(|c| c.id == 600)
        .expect("lazy DM in state");
    assert_eq!(dm.recipient_ids, vec![3]);
    let bob = s.recipients(dm)[0];
    assert_eq!(bob.username, "bob");
    assert_eq!(bob.global_name.as_deref(), Some("bob G"));
    assert_eq!(bob.avatar.as_deref(), Some("av3"));
    assert_eq!(bob.discriminator.as_deref(), Some("0"));
}

#[test]
fn private_channel_create_inline_recipients_resolve_all_four_fields() {
    let mut s = applied("READY", ready(vec![]));
    let create = json!({"id": "700", "type": 1, "last_message_id": "70",
        "recipients": [{"id": "8", "username": "carol", "global_name": "Carol", "avatar": "avc", "discriminator": "0"}]});
    assert!(s.ap("CHANNEL_CREATE", create));
    let dm = s.private_channels.iter().find(|c| c.id == 700).unwrap();
    let carol = s.recipients(dm)[0];
    assert_eq!(
        (
            carol.username.as_str(),
            carol.global_name.as_deref(),
            carol.avatar.as_deref(),
            carol.discriminator.as_deref()
        ),
        ("carol", Some("Carol"), Some("avc"), Some("0"))
    );
    // The directory is lasting: it also serves a later lazy DM that only has the id.
    s.ap(
        "READY_SUPPLEMENTAL",
        json!({"lazy_private_channels": [{"id": "701", "type": 1, "recipient_ids": ["8"]}]}),
    );
    let lazy = s.private_channels.iter().find(|c| c.id == 701).unwrap();
    assert_eq!(s.recipients(lazy)[0].username, "carol");
    // CHANNEL_DELETE removes it.
    assert!(s.ap("CHANNEL_DELETE", json!({"id": "700", "type": 1})));
    assert!(s.private_channels.iter().all(|c| c.id != 700));
}

#[test]
fn absent_user_settings_means_no_guild_folders() {
    let mut r = ready(vec![]);
    r.as_object_mut().unwrap().remove("user_settings");
    assert_eq!(applied("READY", r).guild_folders, None);
    let with = applied("READY", ready(vec![]));
    let folders = with.guild_folders.unwrap();
    assert_eq!(folders.len(), 2);
    assert_eq!(
        (folders[0].id, folders[0].guild_ids.clone()),
        (None, vec![10])
    );
    assert_eq!(
        (
            folders[1].id,
            folders[1].name.as_deref(),
            folders[1].guild_ids.clone()
        ),
        (Some(7), Some("f"), vec![20, 30])
    );
}

#[test]
fn guild_order_is_ready_order_then_arrival_order() {
    let mut s = applied("READY", ready(vec![guild(30), guild(10), guild(20)]));
    assert_eq!(s.ready_guild_ids(), vec![30, 10, 20]);
    assert!(s.post_ready_guild_ids().is_empty());
    assert!(s.ap("GUILD_CREATE", guild(50)));
    assert!(s.ap("GUILD_CREATE", guild(40)));
    assert_eq!(s.ready_guild_ids(), vec![30, 10, 20]);
    assert_eq!(s.post_ready_guild_ids(), vec![50, 40]);
    // GUILD_UPDATE / re-create keep a guild where it was; DELETE removes it.
    s.ap("GUILD_CREATE", guild(10));
    assert_eq!(s.ready_guild_ids(), vec![30, 10, 20]);
    assert!(s.ap("GUILD_DELETE", json!({"id": "30"})));
    assert_eq!(s.ready_guild_ids(), vec![10, 20]);
    assert_eq!(
        s.guilds.iter().map(|g| g.id).collect::<Vec<_>>(),
        vec![10, 20, 50, 40]
    );
}

#[test]
fn unavailable_ready_stub_keeps_its_place_until_guild_create() {
    let mut s = applied(
        "READY",
        ready(vec![
            guild(30),
            json!({"id": "10", "unavailable": true}),
            guild(20),
        ]),
    );
    assert!(s.guild(10).unwrap().unavailable);
    s.ap("GUILD_CREATE", guild(10));
    assert!(!s.guild(10).unwrap().unavailable);
    assert_eq!(s.ready_guild_ids(), vec![30, 10, 20]);
}

#[test]
fn dispatches_bump_revision_and_unknown_ones_are_ignored() {
    let mut s = applied("READY", ready(vec![guild(100)]));
    let r0 = s.revision;
    assert!(!s.ap("TYPING_START", json!({"channel_id": "1"})));
    assert!(!s.ap("GUILD_CREATE", json!({"no_id": true})));
    assert!(!s.ap(
        "CHANNEL_CREATE",
        json!({"id": "5", "guild_id": "404", "type": 0})
    ));
    assert_eq!(s.revision, r0);
    assert!(s.ap("CHANNEL_CREATE", json!({"id": "5", "guild_id": "100", "type": 0, "name": "n", "position": 3, "parent_id": "4", "last_message_id": "8", "permission_overwrites": []})));
    assert_eq!(s.revision, r0 + 1);
    let c = &s.guild(100).unwrap().channels[0];
    assert_eq!(
        (
            c.name.as_deref(),
            c.position,
            c.parent_id,
            c.last_message_id,
            c.guild_id
        ),
        (Some("n"), Some(3), Some(4), Some(8), 100)
    );
    assert!(s.ap(
        "GUILD_ROLE_CREATE",
        json!({"guild_id": "100", "role": {"id": "7", "permissions": "1024", "position": 2}})
    ));
    assert_eq!(s.guild(100).unwrap().roles.as_ref().unwrap().len(), 2);
    // Self member update changes roles; someone else's does not.
    assert!(s.ap(
        "GUILD_MEMBER_UPDATE",
        json!({"guild_id": "100", "user": {"id": "1"}, "roles": ["7"]})
    ));
    assert_eq!(s.guild(100).unwrap().member_roles, Some(vec![7]));
    assert!(!s.ap(
        "GUILD_MEMBER_UPDATE",
        json!({"guild_id": "100", "user": {"id": "2"}, "roles": []})
    ));
    assert!(s.ap(
        "CHANNEL_DELETE",
        json!({"id": "5", "guild_id": "100", "type": 0})
    ));
    assert!(s.guild(100).unwrap().channels.is_empty());
}

#[test]
fn ready_without_a_user_is_rejected_and_numeric_ids_are_accepted() {
    let mut s = State::default();
    assert!(!s.ap("READY", json!({"guilds": []})));
    assert_eq!(s.revision, 0);
    let g = json!({"id": 77, "name": "n", "channels": [{"id": 78, "type": 0}]});
    let s = applied("READY", ready(vec![g]));
    assert_eq!(s.guild(77).unwrap().channels[0].id, 78);
}
