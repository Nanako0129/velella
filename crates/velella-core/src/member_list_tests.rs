//! Member-list rules through `State::apply`, on synthetic GUILD_MEMBER_LIST_UPDATE payloads.
//! The mock-gateway cases live in `session_tests::member_list`.
use crate::{
    member_sections,
    state::{MemberSub, State},
    MemberListSection, MemberListStatus,
};
use serde_json::{json, Value};

pub(crate) const GUILD: u64 = 10;
pub(crate) const CHANNEL: u64 = 20;

pub(crate) fn header(id: &str, count: u64) -> Value {
    json!({"group": {"id": id, "count": count}})
}

pub(crate) fn member(id: u64, name: &str) -> Value {
    json!({"member": {"user": {"id": id.to_string(), "username": name},
        "roles": [], "presence": {"status": "online"}}})
}

pub(crate) fn sync(list: &str, items: Vec<Value>) -> Value {
    let end = items.len().max(1) - 1;
    update(
        list,
        vec![json!({"op": "SYNC", "range": [0, end], "items": items})],
    )
}

pub(crate) fn update(list: &str, ops: Vec<Value>) -> Value {
    json!({"guild_id": GUILD.to_string(), "id": list, "member_count": 9, "ops": ops, "groups": []})
}

fn state(epoch: u64, channel: u64) -> State {
    State {
        member_sub: Some(MemberSub {
            epoch,
            guild_id: GUILD,
            channel_id: channel,
        }),
        ..State::default()
    }
}

fn rows(s: &State) -> Vec<MemberListSection> {
    let sub = s.member_sub.as_ref().unwrap();
    member_sections(s, sub.guild_id, sub.channel_id)
}

fn names(section: &MemberListSection) -> Vec<&str> {
    section.members.iter().map(|m| &*m.name).collect()
}

fn with_roles(s: &mut State, roles: Value) {
    let guild = json!({"id": GUILD.to_string(), "name": "g", "roles": roles, "channels": []});
    assert!(s.apply("GUILD_CREATE", &guild));
}

/// Selects another channel of the same guild, as `set_member_subscription` does.
fn switch(s: &mut State, epoch: u64, channel: u64) {
    s.member_sub = Some(MemberSub {
        epoch,
        guild_id: GUILD,
        channel_id: channel,
    });
}

#[test]
fn sync_fills_online_and_offline_groups() {
    let mut s = state(1, CHANNEL);
    let items = vec![
        header("online", 2),
        member(1, "m1"),
        member(2, "m2"),
        header("offline", 5),
        member(3, "m3"),
    ];
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &sync("everyone", items)));
    let got = rows(&s);
    assert_eq!(got.len(), 2);
    assert_eq!((got[0].title.as_str(), got[0].count), ("Online", 2));
    assert_eq!(names(&got[0]), ["m1", "m2"]);
    assert_eq!((got[1].title.as_str(), got[1].count), ("Offline", 5));
    assert_eq!(names(&got[1]), ["m3"]);
}

#[test]
fn insert_and_delete_count_header_items_in_the_indices() {
    let mut s = state(1, CHANNEL);
    let items = vec![
        header("online", 2),
        member(1, "m1"),
        member(2, "m2"),
        header("offline", 1),
        member(3, "m3"),
    ];
    s.apply("GUILD_MEMBER_LIST_UPDATE", &sync("everyone", items));
    // Index 2 is m2 (the header took index 0): inserting there pushes m2 and everything after.
    let ins = update(
        "everyone",
        vec![json!({"op": "INSERT", "index": 2, "item": member(4, "m4")})],
    );
    s.apply("GUILD_MEMBER_LIST_UPDATE", &ins);
    let got = rows(&s);
    assert_eq!(names(&got[0]), ["m1", "m4", "m2"]);
    assert_eq!(names(&got[1]), ["m3"]);
    // Index 3 is m2 now; the Offline header moved from 3 to 4 and back to 3.
    let del = update("everyone", vec![json!({"op": "DELETE", "index": 3})]);
    s.apply("GUILD_MEMBER_LIST_UPDATE", &del);
    let got = rows(&s);
    assert_eq!(names(&got[0]), ["m1", "m4"]);
    assert_eq!(names(&got[1]), ["m3"]);
    // UPDATE replaces in place (a presence change here).
    let mut idle = member(3, "m3");
    idle["member"]["presence"]["status"] = json!("idle");
    let upd = update(
        "everyone",
        vec![json!({"op": "UPDATE", "index": 4, "item": idle})],
    );
    s.apply("GUILD_MEMBER_LIST_UPDATE", &upd);
    assert_eq!(rows(&s)[1].members[0].status, MemberListStatus::Idle);
}

#[test]
fn a_groups_only_update_changes_header_counts() {
    let mut s = state(1, CHANNEL);
    let items = vec![
        header("online", 1),
        member(1, "m1"),
        header("offline", 5),
        member(2, "m2"),
    ];
    s.apply("GUILD_MEMBER_LIST_UPDATE", &sync("everyone", items));
    let mut counts = update("everyone", vec![]);
    counts["groups"] = json!([{"id": "online", "count": 3}, {"id": "offline", "count": 4}]);
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &counts));
    let got = rows(&s);
    assert_eq!((got[0].count, got[1].count), (3, 4));
    assert_eq!(names(&got[0]), ["m1"]);
}

#[test]
fn invalidate_of_a_sub_range_keeps_the_rows_outside_it() {
    let mut s = state(1, CHANNEL);
    let items = vec![
        header("online", 4),
        member(1, "m1"),
        member(2, "m2"),
        member(3, "m3"),
        member(4, "m4"),
    ];
    s.apply("GUILD_MEMBER_LIST_UPDATE", &sync("everyone", items));
    let inv = update(
        "everyone",
        vec![json!({"op": "INVALIDATE", "range": [2, 3]})],
    );
    s.apply("GUILD_MEMBER_LIST_UPDATE", &inv);
    let got = rows(&s);
    // m1 (index 1) stays under its header; m4 (index 4) follows a hole, so it forms the implicit group.
    assert_eq!(names(&got[0]), ["m1"]);
    assert_eq!(got[1].title, "Members");
    assert_eq!(names(&got[1]), ["m4"]);
}

#[test]
fn an_update_for_a_list_id_that_is_not_active_changes_nothing() {
    let mut s = state(1, CHANNEL);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(1, "m1")]),
    );
    let before = rows(&s);
    let revision = s.revision;
    let other = update(
        "other",
        vec![json!({"op": "INSERT", "index": 1, "item": member(2, "m2")})],
    );
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &other));
    // A SYNC that does not reach the subscribed rows cannot take over either.
    let far = update(
        "other",
        vec![json!({"op": "SYNC", "range": [100, 101], "items": [member(5, "x")]})],
    );
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &far));
    assert_eq!((rows(&s), s.revision), (before, revision));
}

#[test]
fn a_role_id_header_gets_the_role_name() {
    let mut s = state(1, CHANNEL);
    with_roles(
        &mut s,
        json!([{"id": "77", "name": "Mods", "position": 1, "color": 0}]),
    );
    let items = vec![
        header("77", 1),
        member(1, "m1"),
        header("78", 1),
        member(2, "m2"),
    ];
    s.apply("GUILD_MEMBER_LIST_UPDATE", &sync("everyone", items));
    let got = rows(&s);
    assert_eq!(got[0].title, "Mods");
    assert_eq!(got[1].title, "Members", "an unknown role id");
}

#[test]
fn colour_comes_from_the_highest_position_role_that_has_one() {
    let mut s = state(1, CHANNEL);
    with_roles(
        &mut s,
        json!([
            {"id": "1", "name": "A", "position": 2, "color": 0},
            {"id": "2", "name": "B", "position": 1, "color": 0xAA00BB},
            {"id": "3", "name": "C", "position": 0, "color": 0x112233}
        ]),
    );
    let mut m = member(1, "m1");
    m["member"]["roles"] = json!(["1", "2", "3"]);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), m, member(2, "plain")]),
    );
    let got = rows(&s);
    assert_eq!(got[0].members[0].color, Some(0xAA00BB));
    assert_eq!(got[0].members[1].color, None);
}

#[test]
fn display_name_presence_and_custom_status_come_from_the_item() {
    let mut s = state(1, CHANNEL);
    let nick = json!({"member": {"user": {"id": "1", "username": "u1", "global_name": "G1", "avatar": "abc"},
        "nick": "N1", "avatar": "gav", "roles": [],
        "presence": {"status": "dnd", "activities": [{"type": 0, "name": "game"}, {"type": 4, "state": "busy"}]}}});
    let global = json!({"member": {"user": {"id": "2", "username": "u2", "global_name": "G2", "avatar": "def"}, "roles": []}});
    let user = json!({"member": {"user": {"id": "3", "username": "u3"}, "roles": []}});
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 3), nick, global, user]),
    );
    let got = rows(&s);
    let m = &got[0].members;
    assert_eq!(names(&got[0]), ["N1", "G2", "u3"]);
    assert_eq!(
        (m[0].status, m[0].custom_status.as_deref()),
        (MemberListStatus::DoNotDisturb, Some("busy"))
    );
    assert_eq!(
        (m[1].status, m[1].custom_status.as_deref()),
        (MemberListStatus::Offline, None)
    );
    assert_eq!(
        m[0].avatar_url.as_deref(),
        Some("https://cdn.discordapp.com/guilds/10/users/1/avatars/gav.png?size=64")
    );
    assert_eq!(
        m[1].avatar_url.as_deref(),
        Some("https://cdn.discordapp.com/avatars/2/def.png?size=64")
    );
    assert_eq!(m[2].avatar_url, None);
}

#[test]
fn members_before_any_header_form_an_implicit_group() {
    let mut s = state(1, CHANNEL);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync(
            "everyone",
            vec![
                member(1, "m1"),
                member(2, "m2"),
                header("offline", 7),
                member(3, "m3"),
            ],
        ),
    );
    let got = rows(&s);
    assert_eq!(
        (got[0].title.as_str(), got[0].count, names(&got[0])),
        ("Members", 2, vec!["m1", "m2"])
    );
    assert_eq!((got[1].title.as_str(), got[1].count), ("Offline", 7));
}

#[test]
fn after_a_same_guild_switch_an_old_id_update_then_a_new_sync_shows_the_new_rows() {
    let mut s = state(1, CHANNEL);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("old", vec![header("online", 1), member(1, "old1")]),
    );
    switch(&mut s, 2, 21);
    assert!(rows(&s).is_empty(), "the view is empty until adoption");
    let late = update(
        "old",
        vec![json!({"op": "UPDATE", "index": 1, "item": member(9, "late")})],
    );
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &late));
    assert!(rows(&s).is_empty());
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("new", vec![header("online", 1), member(2, "new1")]),
    );
    assert_eq!(names(&rows(&s)[0]), ["new1"]);
}

#[test]
fn a_late_sync_with_the_old_id_is_ignored_when_it_had_been_adopted() {
    let mut s = state(1, CHANNEL);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("old", vec![header("online", 1), member(1, "old1")]),
    );
    switch(&mut s, 2, 21);
    assert!(!s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("old", vec![header("online", 1), member(9, "late")])
    ));
    assert!(rows(&s).is_empty());
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("new", vec![header("online", 1), member(2, "new1")]),
    );
    assert_eq!(names(&rows(&s)[0]), ["new1"]);
}

#[test]
fn a_new_covering_sync_replaces_a_list_that_was_never_adopted() {
    let mut s = state(1, CHANNEL);
    switch(&mut s, 2, 21);
    // The old id was never adopted, so nothing retires it: this SYNC is taken as the new list.
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("old", vec![header("online", 1), member(9, "late")]),
    );
    // Within the epoch a later covering SYNC with a different id replaces it.
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("new", vec![header("online", 1), member(2, "new1")]),
    );
    assert_eq!(names(&rows(&s)[0]), ["new1"]);
    // And the replaced list's updates no longer apply.
    let stale = update(
        "old",
        vec![json!({"op": "UPDATE", "index": 1, "item": member(9, "late")})],
    );
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &stale));
}

#[test]
fn insert_and_update_before_any_sync_add_no_rows() {
    let mut s = state(1, CHANNEL);
    let ins = update(
        "everyone",
        vec![json!({"op": "INSERT", "index": 0, "item": member(1, "m1")})],
    );
    let upd = update(
        "everyone",
        vec![json!({"op": "UPDATE", "index": 0, "item": member(1, "m1")})],
    );
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &ins));
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &upd));
    assert!(rows(&s).is_empty());
}

#[test]
fn ready_keeps_the_selection_and_the_next_sync_fills_the_list() {
    let mut s = state(1, CHANNEL);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(1, "m1")]),
    );
    let ready = json!({"user": {"id": "1", "username": "me"}, "guilds": [], "private_channels": [],
        "user_settings": {"guild_folders": []}});
    assert!(s.apply("READY", &ready));
    assert!(
        rows(&s).is_empty(),
        "READY rebuilds the state, the list is re-armed"
    );
    // The same id comes back after the re-sent subscription and must be adopted again.
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(2, "m2")]),
    );
    assert_eq!(names(&rows(&s)[0]), ["m2"]);
}

#[test]
fn a_selection_change_hides_the_list_before_any_update_arrives() {
    let mut s = state(1, CHANNEL);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(1, "m1")]),
    );
    switch(&mut s, 2, 21);
    assert!(member_sections(&s, GUILD, 21).is_empty());
    assert!(
        member_sections(&s, GUILD, CHANNEL).is_empty(),
        "not the selection any more"
    );
    s.member_sub = None;
    assert!(member_sections(&s, GUILD, 21).is_empty());
}

#[test]
fn no_selection_and_other_guilds_are_ignored() {
    let mut s = State::default();
    assert!(!s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(1, "m1")])
    ));
    let mut s = state(1, CHANNEL);
    let mut other = sync("everyone", vec![header("online", 1), member(1, "m1")]);
    other["guild_id"] = json!("11");
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &other));
    assert!(rows(&s).is_empty());
}

// ---- Channels with the same list identity share the list ----

const VIEW: u64 = 1 << 10;

fn channel(id: u64, overwrites: Option<Value>) -> Value {
    let mut c = json!({"id": id.to_string(), "type": 0, "guild_id": GUILD.to_string()});
    if let Some(o) = overwrites {
        c["permission_overwrites"] = o;
    }
    c
}

/// Guild 10 whose @everyone role can view; channels 20 and 21 have no overwrites, 22 denies
/// VIEW_CHANNEL to role 5, 23 has no overwrite data at all.
fn world(epoch: u64, channel_id: u64) -> State {
    let mut s = state(epoch, channel_id);
    let deny = json!([{"id": "5", "type": 0, "allow": "0", "deny": VIEW.to_string()}]);
    let guild = json!({"id": GUILD.to_string(), "name": "g", "roles": [{"id": "10", "permissions": VIEW.to_string()}],
        "channels": [channel(20, Some(json!([]))), channel(21, Some(json!([]))), channel(22, Some(deny)), channel(23, None)]});
    assert!(s.apply("GUILD_CREATE", &guild));
    s
}

#[test]
fn a_switch_between_channels_with_identical_overwrites_keeps_the_rows() {
    let mut s = world(1, 20);
    assert_eq!(s.channel_list_id(GUILD, 20).as_deref(), Some("everyone"));
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(1, "m1")]),
    );
    let before = rows(&s);
    switch(&mut s, 2, 21);
    // No new SYNC: the snapshot for the new channel shows the same rows.
    assert_eq!(member_sections(&s, GUILD, 21), before);
    assert!(
        member_sections(&s, GUILD, 20).is_empty(),
        "no longer the selection"
    );
    // Later updates keep applying to the kept list, and nothing was retired.
    let upd = update(
        "everyone",
        vec![json!({"op": "UPDATE", "index": 1, "item": member(2, "m2")})],
    );
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &upd));
    assert_eq!(names(&member_sections(&s, GUILD, 21)[0]), ["m2"]);
    // Back to the first channel: still the same list.
    switch(&mut s, 3, 20);
    assert_eq!(names(&member_sections(&s, GUILD, 20)[0]), ["m2"]);
}

#[test]
fn a_switch_to_a_different_computed_id_clears_ignores_the_old_id_and_adopts_the_matching_sync() {
    let mut s = world(1, 20);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(1, "old")]),
    );
    let hashed = s.channel_list_id(GUILD, 22).unwrap();
    assert_ne!(hashed, "everyone");
    switch(&mut s, 2, 22);
    assert!(member_sections(&s, GUILD, 22).is_empty());
    // A late SYNC with the old (retired) id is refused, even though a computed id exists.
    let late = sync("everyone", vec![header("online", 1), member(9, "late")]);
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &late));
    assert!(member_sections(&s, GUILD, 22).is_empty());
    let matching = sync(&hashed, vec![header("online", 1), member(2, "new")]);
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &matching));
    assert_eq!(names(&member_sections(&s, GUILD, 22)[0]), ["new"]);
    // Going back to a channel whose id equals a retired one adopts it again.
    switch(&mut s, 3, 20);
    assert!(member_sections(&s, GUILD, 20).is_empty());
    let back = sync("everyone", vec![header("online", 1), member(1, "back")]);
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &back));
    assert_eq!(names(&member_sections(&s, GUILD, 20)[0]), ["back"]);
}

#[test]
fn a_channel_without_a_computed_id_uses_the_fallback_rule() {
    let mut s = world(1, 20);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(1, "old")]),
    );
    assert_eq!(s.channel_list_id(GUILD, 23), None);
    switch(&mut s, 2, 23);
    assert!(
        member_sections(&s, GUILD, 23).is_empty(),
        "no computed id, so nothing is carried over"
    );
    // The adopted id was retired; the first covering SYNC with another id is taken,
    let late = sync("everyone", vec![header("online", 1), member(9, "late")]);
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &late));
    let one = sync("first", vec![header("online", 1), member(2, "one")]);
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &one));
    assert_eq!(names(&member_sections(&s, GUILD, 23)[0]), ["one"]);
    // and a later covering SYNC with a different id replaces it within the epoch.
    let two = sync("second", vec![header("online", 1), member(3, "two")]);
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &two));
    assert_eq!(names(&member_sections(&s, GUILD, 23)[0]), ["two"]);
}

#[test]
fn a_populated_sync_with_another_id_is_a_fallback_the_computed_id_replaces() {
    let mut s = world(1, 20);
    s.apply(
        "GUILD_MEMBER_LIST_UPDATE",
        &sync("everyone", vec![header("online", 1), member(1, "old")]),
    );
    let hashed = s.channel_list_id(GUILD, 22).unwrap();
    switch(&mut s, 2, 22);
    // Computed id X (`hashed`); nothing adopted yet, so a non-retired Y is taken.
    let y = sync("y", vec![header("online", 1), member(2, "fallback")]);
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &y));
    assert_eq!(names(&member_sections(&s, GUILD, 22)[0]), ["fallback"]);
    // After an adoption another id is ignored, a retired one too.
    let z = sync("z", vec![header("online", 1), member(3, "z")]);
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &z));
    let late = sync("everyone", vec![header("online", 1), member(9, "late")]);
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &late));
    assert_eq!(names(&member_sections(&s, GUILD, 22)[0]), ["fallback"]);
    // The computed id replaces the fallback at any time in the epoch.
    let x = sync(&hashed, vec![header("online", 1), member(4, "computed")]);
    assert!(s.apply("GUILD_MEMBER_LIST_UPDATE", &x));
    assert_eq!(names(&member_sections(&s, GUILD, 22)[0]), ["computed"]);
    // and from then on the fallback id no longer applies.
    let stale = update(
        "y",
        vec![json!({"op": "UPDATE", "index": 1, "item": member(5, "y")})],
    );
    assert!(!s.apply("GUILD_MEMBER_LIST_UPDATE", &stale));
}
