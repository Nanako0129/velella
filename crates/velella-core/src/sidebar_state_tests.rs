//! Slice D rules through `State::apply`, read back through the sidebar functions. One world: user 1
//! (member role 555) in guild 100 (category 5; text 10 under it, text 11, voice 12), a 1:1 DM 500
//! with user 2 and a group DM 501 with users 2 and 3. Nothing is acked unless a test says so.
use crate::sidebar_tests::{ch, guild, ready};
use crate::state::State;
use crate::{
    channel_categories, dm_list, guild_entries, PresenceStatus, SidebarChannel, SidebarDm,
    SidebarEntry, SidebarGuild,
};
use serde_json::{json, Value};

const VIEW: &str = "1024";

fn lm(mut c: Value, last: u64) -> Value {
    c["last_message_id"] = json!(last.to_string());
    c
}

/// READY with the world above; `extra` keys (read_state, user_guild_settings, ...) are merged in.
fn world_with(extra: Value, guild_extra: Value) -> State {
    let mut g = guild(
        100,
        vec![
            ch(5, 4, "cat", 0, None),
            lm(ch(10, 0, "general", 0, Some(5)), 200),
            lm(ch(11, 0, "random", 1, None), 200),
            ch(12, 2, "Lounge", 2, None),
            json!({"id": "13", "type": 0, "name": "staff", "position": 3,
                "permission_overwrites": [
                    {"id": "100", "type": 0, "allow": "0", "deny": VIEW},
                    {"id": "555", "type": 0, "allow": VIEW, "deny": "0"}]}),
        ],
    );
    g["roles"] = json!([
        {"id": "100", "permissions": VIEW, "position": 0},
        {"id": "555", "permissions": "0", "position": 1}
    ]);
    for (k, v) in guild_extra.as_object().into_iter().flatten() {
        g[k] = v.clone();
    }
    let users = vec![
        json!({"id": "2", "username": "alice", "global_name": null, "avatar": null, "discriminator": "0"}),
        json!({"id": "3", "username": "bob", "global_name": null, "avatar": null, "discriminator": "0"}),
    ];
    let dms = json!([
        {"id": "500", "type": 1, "recipient_ids": ["2"], "last_message_id": "300"},
        {"id": "501", "type": 3, "recipient_ids": ["2", "3"], "last_message_id": "300"}
    ]);
    let mut d = ready(vec![g], Value::Null, dms, users);
    d["merged_members"] = json!([[{"user_id": "1", "roles": ["555"]}]]);
    for (k, v) in extra.as_object().into_iter().flatten() {
        d[k] = v.clone();
    }
    let mut s = State::default();
    assert!(s.apply("READY", &d));
    s
}

fn world(extra: Value) -> State {
    world_with(extra, json!({}))
}

fn item(s: &State, id: u64) -> SidebarChannel {
    channel_categories(s, 100)
        .into_iter()
        .flat_map(|c| c.channels)
        .find(|c| c.id == id)
        .unwrap_or_else(|| panic!("channel {id} not listed"))
}

fn rail_guild(s: &State) -> SidebarGuild {
    match guild_entries(s).remove(0) {
        SidebarEntry::Guild(g) => g,
        SidebarEntry::Folder { .. } => panic!("expected a plain guild"),
    }
}

fn dm(s: &State, id: u64) -> SidebarDm {
    dm_list(s).into_iter().find(|d| d.id == id).unwrap()
}

fn read_states(entries: Value) -> Value {
    json!({"read_state": {"entries": entries, "version": 1, "partial": false}})
}

fn settings(entries: Value) -> Value {
    json!({"user_guild_settings": {"entries": entries, "version": 1, "partial": false}})
}

/// Acks 10 and 11 at their newest message (200), and both DMs at theirs (300).
fn all_read() -> Value {
    read_states(json!([
        {"id": "10", "last_message_id": "200", "mention_count": 0},
        {"id": "11", "last_message_id": "200", "mention_count": 0},
        {"id": "500", "last_message_id": "300", "mention_count": 0},
        {"id": "501", "last_message_id": "300", "mention_count": 0}
    ]))
}

fn msg(id: u64, channel: u64, guild: Option<u64>, author: u64) -> Value {
    let mut m = json!({"id": id.to_string(), "channel_id": channel.to_string(), "type": 0,
        "author": {"id": author.to_string()}, "mentions": [], "mention_roles": [],
        "mention_everyone": false});
    if let Some(g) = guild {
        m["guild_id"] = json!(g.to_string());
    }
    m
}

#[test]
fn unread_from_ready_read_state() {
    let s = world(read_states(json!([
        {"id": "10", "last_message_id": "100", "mention_count": 2},
        {"id": "11", "last_message_id": "200", "mention_count": 0}
    ])));
    let (behind, current) = (item(&s, 10), item(&s, 11));
    assert!(behind.unread && behind.mention_count == 2);
    assert!(!current.unread && current.mention_count == 0);
    assert!(!item(&s, 12).unread, "a channel without messages is read");
    let g = rail_guild(&s);
    assert!(g.unread && g.mention_count == 2 && !g.muted);
}

#[test]
fn missing_read_state_counts_as_unread() {
    let s = world(read_states(json!([])));
    let c = item(&s, 11);
    assert!(c.unread && c.mention_count == 0);
    // READY with no read_state key at all is the same.
    let s = world(json!({}));
    assert!(item(&s, 10).unread && rail_guild(&s).unread);
    assert!(dm(&s, 500).unread);
}

#[test]
fn message_ack_clears_unread_and_mentions() {
    let mut s = world(read_states(json!([
        {"id": "10", "last_message_id": "100", "mention_count": 2}
    ])));
    assert!(s.apply(
        "MESSAGE_ACK",
        &json!({"channel_id": "10", "message_id": "200", "mention_count": 0, "version": 5})
    ));
    let c = item(&s, 10);
    assert!(!c.unread && c.mention_count == 0);
    assert_eq!(rail_guild(&s).mention_count, 0);
}

#[test]
fn stale_ack_is_a_no_op() {
    let mut s = world(all_read());
    let before = s.revision;
    assert!(!s.apply(
        "MESSAGE_ACK",
        &json!({"channel_id": "10", "message_id": "150", "mention_count": 9})
    ));
    assert_eq!(s.revision, before);
    let c = item(&s, 10);
    assert!(!c.unread && c.mention_count == 0);
}

#[test]
fn channel_unread_update_raises_last_message_id() {
    let mut s = world(all_read());
    assert!(!item(&s, 11).unread);
    assert!(s.apply(
        "CHANNEL_UNREAD_UPDATE",
        &json!({"guild_id": "100", "channel_unread_updates": [
            {"id": "11", "last_message_id": "300"}, {"id": "10", "last_message_id": "100"}]})
    ));
    assert!(item(&s, 11).unread);
    assert!(!item(&s, 10).unread, "an older id never lowers or unreads");
}

#[test]
fn self_message_adds_no_mention_and_reads_the_channel() {
    let mut s = world(read_states(json!([
        {"id": "10", "last_message_id": "100", "mention_count": 2}
    ])));
    let mut m = msg(250, 10, Some(100), 1);
    m["mentions"] = json!([{"id": "1"}]);
    m["mention_everyone"] = json!(true);
    assert!(s.apply("MESSAGE_CREATE", &m));
    let c = item(&s, 10);
    assert!(!c.unread && c.mention_count == 0, "{c:?}");
}

#[test]
fn self_message_in_a_read_channel_leaves_it_read() {
    let mut s = world(all_read());
    s.apply("MESSAGE_CREATE", &msg(210, 11, Some(100), 1));
    assert!(!item(&s, 11).unread);
    s.apply("MESSAGE_CREATE", &msg(310, 500, None, 1));
    let d = dm(&s, 500);
    assert!(!d.unread && d.mention_count == 0);
    // Without any read-state entry the self message still reads the channel.
    let mut s = world(json!({}));
    s.apply("MESSAGE_CREATE", &msg(210, 11, Some(100), 1));
    assert!(!item(&s, 11).unread);
}

#[test]
fn dm_plain_message_adds_one_and_the_same_message_in_a_guild_adds_none() {
    let mut s = world(all_read());
    s.apply("MESSAGE_CREATE", &msg(310, 500, None, 2));
    let d = dm(&s, 500);
    assert!(d.unread && d.mention_count == 1);
    // A second one and a group DM message count too.
    s.apply("MESSAGE_CREATE", &msg(311, 500, None, 2));
    s.apply("MESSAGE_CREATE", &msg(310, 501, None, 3));
    assert_eq!(dm(&s, 500).mention_count, 2);
    assert_eq!(dm(&s, 501).mention_count, 1);
    // The same message in a guild channel: unread, no mention.
    s.apply("MESSAGE_CREATE", &msg(310, 11, Some(100), 2));
    let c = item(&s, 11);
    assert!(c.unread && c.mention_count == 0);
    // A message at or below the ack adds nothing.
    s.apply("MESSAGE_CREATE", &msg(300, 500, None, 2));
    assert_eq!(dm(&s, 500).mention_count, 2);
    // The DM list puts the newest conversation first.
    assert_eq!(dm_list(&s)[0].id, 500);
}

#[test]
fn role_and_everyone_mentions() {
    let mut s = world(all_read());
    let mut role = msg(310, 11, Some(100), 2);
    role["mention_roles"] = json!(["555"]);
    s.apply("MESSAGE_CREATE", &role);
    assert_eq!(item(&s, 11).mention_count, 1);
    let mut other = msg(311, 11, Some(100), 2);
    other["mention_roles"] = json!(["777"]);
    s.apply("MESSAGE_CREATE", &other);
    assert_eq!(item(&s, 11).mention_count, 1, "a role the user lacks");
    let mut everyone = msg(312, 11, Some(100), 2);
    everyone["mention_everyone"] = json!(true);
    s.apply("MESSAGE_CREATE", &everyone);
    assert_eq!(item(&s, 11).mention_count, 2);
    let mut direct = msg(313, 11, Some(100), 2);
    direct["mentions"] = json!([{"id": "1"}]);
    s.apply("MESSAGE_CREATE", &direct);
    assert_eq!(item(&s, 11).mention_count, 3);
    assert_eq!(rail_guild(&s).mention_count, 3);

    // suppress_everyone / suppress_roles from the guild's settings switch those two off.
    s.apply(
        "USER_GUILD_SETTINGS_UPDATE",
        &json!({"guild_id": "100", "suppress_everyone": true, "suppress_roles": true,
            "channel_overrides": [], "version": 2}),
    );
    s.apply("MESSAGE_CREATE", &role.clone().tap_id(320));
    s.apply("MESSAGE_CREATE", &everyone.clone().tap_id(321));
    assert_eq!(item(&s, 11).mention_count, 3);
    s.apply("MESSAGE_CREATE", &direct.clone().tap_id(322));
    assert_eq!(item(&s, 11).mention_count, 4);
}

trait TapId {
    fn tap_id(self, id: u64) -> Self;
}
impl TapId for Value {
    fn tap_id(mut self, id: u64) -> Self {
        self["id"] = json!(id.to_string());
        self
    }
}

fn mute(channel: u64) -> Value {
    json!({"channel_id": channel.to_string(), "muted": true})
}

#[test]
fn child_of_a_muted_category_is_muted() {
    let s = world(settings(json!([
        {"guild_id": "100", "muted": false, "channel_overrides": [mute(5)]}
    ])));
    let (child, other) = (item(&s, 10), item(&s, 11));
    assert!(child.muted && !child.unread && child.mention_count == 0);
    assert!(!other.muted && other.unread);
    // The first override up the ancestry decides: an explicit unmute on the channel wins.
    let s = world(settings(json!([
        {"guild_id": "100", "channel_overrides": [mute(5),
            {"channel_id": "10", "muted": false}]}
    ])));
    assert!(!item(&s, 10).muted);
}

#[test]
fn muted_guild_is_not_unread() {
    let s = world(settings(json!([{"guild_id": "100", "muted": true}])));
    let g = rail_guild(&s);
    assert!(g.muted && !g.unread && g.mention_count == 0);
}

#[test]
fn expired_mute_is_not_muted_and_a_future_one_is() {
    let past = settings(json!([{"guild_id": "100", "muted": true,
        "mute_config": {"end_time": "2020-01-01T00:00:00.000000+00:00", "selected_time_window": 3600}}]));
    let g = rail_guild(&world(past));
    assert!(!g.muted && g.unread);
    let future = settings(json!([{"guild_id": "100", "muted": true,
        "mute_config": {"end_time": "2999-01-01T00:00:00+00:00"}}]));
    assert!(rail_guild(&world(future)).muted);
    // Channel overrides expire the same way.
    let s = world(settings(json!([{"guild_id": "100", "channel_overrides": [
        {"channel_id": "11", "muted": true, "mute_config": {"end_time": "2020-01-01T00:00:00Z"}}]}])));
    assert!(!item(&s, 11).muted);
}

#[test]
fn user_guild_settings_update_toggles() {
    let mut s = world(json!({}));
    assert!(!rail_guild(&s).muted);
    assert!(s.apply(
        "USER_GUILD_SETTINGS_UPDATE",
        &json!({"guild_id": "100", "muted": true, "channel_overrides": [], "version": 2})
    ));
    assert!(rail_guild(&s).muted);
    // The update replaces the entry: the mute is gone, a channel override is in.
    assert!(s.apply(
        "USER_GUILD_SETTINGS_UPDATE",
        &json!({"guild_id": "100", "muted": false, "channel_overrides": [mute(11)], "version": 3})
    ));
    assert!(!rail_guild(&s).muted);
    assert!(item(&s, 11).muted);
    // The same update again changes nothing.
    assert!(!s.apply(
        "USER_GUILD_SETTINGS_UPDATE",
        &json!({"guild_id": "100", "muted": false, "channel_overrides": [mute(11)], "version": 3})
    ));
}

#[test]
fn muted_mentions_are_excluded_from_totals() {
    let entries = read_states(json!([
        {"id": "10", "last_message_id": "100", "mention_count": 2},
        {"id": "11", "last_message_id": "150", "mention_count": 3}
    ]));
    let mut both = entries.clone();
    assert_eq!(rail_guild(&world(both.take())).mention_count, 5);
    let mut muted = entries;
    for (k, v) in settings(json!([{"guild_id": "100", "channel_overrides": [mute(5)]}]))
        .as_object()
        .unwrap()
    {
        muted[k] = v.clone();
    }
    let s = world(muted);
    assert_eq!(rail_guild(&s).mention_count, 3);
    // A mention landing in the muted channel is recorded but not shown anywhere.
    let mut s = s;
    let mut m = msg(260, 10, Some(100), 2);
    m["mentions"] = json!([{"id": "1"}]);
    s.apply("MESSAGE_CREATE", &m);
    assert_eq!(item(&s, 10).mention_count, 0);
    assert_eq!(rail_guild(&s).mention_count, 3);
}

#[test]
fn private_channel_has_a_lock() {
    let s = world(json!({}));
    let staff = item(&s, 13);
    assert!(staff.private, "@everyone is denied VIEW_CHANNEL");
    assert!(!item(&s, 10).private && !item(&s, 12).private);
}

fn supplemental_voice() -> Value {
    json!({"guilds": [{"id": "100", "voice_states": [
        {"user_id": "2", "channel_id": "12", "member": {"nick": "Alicia", "user": {"id": "2", "username": "alice"}}},
        {"user_id": "3", "channel_id": "12"}]}]})
}

fn occupants(s: &State) -> Vec<(u64, String)> {
    item(s, 12)
        .occupants
        .into_iter()
        .map(|o| (o.id, o.name))
        .collect()
}

#[test]
fn occupants_come_from_ready_supplemental_only() {
    let mut s = world(json!({}));
    assert!(
        occupants(&s).is_empty(),
        "READY carries no voice states here"
    );
    assert!(s.apply("READY_SUPPLEMENTAL", &supplemental_voice()));
    // Nickname first, then the user directory.
    assert_eq!(occupants(&s), vec![(2, "Alicia".into()), (3, "bob".into())]);
    assert!(item(&s, 12).occupants[1].icon_url.is_some());
    assert!(item(&s, 11).occupants.is_empty());
}

#[test]
fn voice_join_and_leave() {
    let mut s = world(json!({}));
    s.apply("READY_SUPPLEMENTAL", &supplemental_voice());
    let leave = json!({"guild_id": "100", "user_id": "3", "channel_id": null});
    assert!(s.apply("VOICE_STATE_UPDATE", &leave));
    assert_eq!(occupants(&s), vec![(2, "Alicia".into())]);
    assert!(
        !s.apply("VOICE_STATE_UPDATE", &leave),
        "leaving twice changes nothing"
    );
    let join = json!({"guild_id": "100", "user_id": "1", "channel_id": "12",
        "member": {"nick": null, "user": {"id": "1", "username": "me"}}});
    assert!(s.apply("VOICE_STATE_UPDATE", &join));
    assert_eq!(occupants(&s), vec![(2, "Alicia".into()), (1, "me".into())]);
    // Moving away from the channel removes the user from it.
    s.apply(
        "VOICE_STATE_UPDATE",
        &json!({"guild_id": "100", "user_id": "2", "channel_id": "11"}),
    );
    assert_eq!(occupants(&s), vec![(1, "me".into())]);
}

fn thread(id: u64, parent: u64, archived: bool) -> Value {
    json!({"id": id.to_string(), "guild_id": "100", "parent_id": parent.to_string(), "type": 11,
        "name": format!("t{id}"), "last_message_id": (id * 10).to_string(),
        "thread_metadata": {"archived": archived}})
}

fn threads(s: &State, parent: u64) -> Vec<u64> {
    item(s, parent).threads.into_iter().map(|t| t.id).collect()
}

#[test]
fn threads_show_only_when_active_joined_and_visible() {
    // READY threads count as joined.
    let mut s = world_with(json!({}), json!({"threads": [thread(20, 10, false)]}));
    assert_eq!(threads(&s, 10), vec![20]);

    // THREAD_CREATE and THREAD_LIST_SYNC without the user's membership stay hidden.
    s.apply("THREAD_CREATE", &thread(30, 10, false));
    s.apply(
        "THREAD_LIST_SYNC",
        &json!({"guild_id": "100", "threads": [
            thread(20, 10, false), thread(30, 10, false), thread(31, 10, false)]}),
    );
    assert_eq!(
        threads(&s, 10),
        vec![20],
        "an absent members array keeps what READY said"
    );
    // A sync lists the guild's whole active set, so a joined thread it omits is gone.
    s.apply("THREAD_CREATE", &thread(32, 10, false));
    s.apply(
        "THREAD_MEMBER_UPDATE",
        &json!({"id": "32", "guild_id": "100"}),
    );
    assert_eq!(threads(&s, 10), vec![32, 20]);
    s.apply(
        "THREAD_LIST_SYNC",
        &json!({"guild_id": "100", "threads": [
            thread(20, 10, false), thread(30, 10, false), thread(31, 10, false)]}),
    );
    assert_eq!(threads(&s, 10), vec![20], "thread 32 left the active set");
    // THREAD_MEMBER_UPDATE adds the user.
    assert!(s.apply(
        "THREAD_MEMBER_UPDATE",
        &json!({"id": "30", "guild_id": "100", "user_id": "1"})
    ));
    assert_eq!(threads(&s, 10), vec![30, 20], "most recent activity first");
    // THREAD_MEMBERS_UPDATE adds and removes the user too.
    s.apply(
        "THREAD_MEMBERS_UPDATE",
        &json!({"id": "31", "guild_id": "100", "added_members": [{"user_id": "1"}]}),
    );
    assert_eq!(threads(&s, 10), vec![31, 30, 20]);
    s.apply(
        "THREAD_MEMBERS_UPDATE",
        &json!({"id": "31", "guild_id": "100", "removed_member_ids": ["1"]}),
    );
    assert_eq!(threads(&s, 10), vec![30, 20]);
    // A present `members` array in LIST_SYNC is authoritative for the listed threads.
    s.apply(
        "THREAD_LIST_SYNC",
        &json!({"guild_id": "100", "channel_ids": ["10"],
            "threads": [thread(20, 10, false), thread(30, 10, false)],
            "members": [{"id": "30", "user_id": "1"}]}),
    );
    assert_eq!(threads(&s, 10), vec![30]);
    // Archived: hidden. Unarchived: back. Deleted: gone.
    s.apply("THREAD_UPDATE", &thread(30, 10, true));
    assert!(threads(&s, 10).is_empty());
    s.apply("THREAD_UPDATE", &thread(30, 10, false));
    assert_eq!(
        threads(&s, 10),
        vec![30],
        "THREAD_UPDATE keeps the joined flag"
    );
    s.apply(
        "THREAD_DELETE",
        &json!({"id": "30", "guild_id": "100", "parent_id": "10"}),
    );
    assert!(threads(&s, 10).is_empty());
}

#[test]
fn a_thread_of_a_channel_the_user_cannot_see_is_not_shown() {
    let mut s = world_with(json!({}), json!({"threads": [thread(20, 99, false)]}));
    assert!(channel_categories(&s, 100)
        .iter()
        .flat_map(|c| &c.channels)
        .all(|c| c.threads.is_empty()));
    // THREAD_LIST_SYNC for another parent leaves it alone.
    s.apply(
        "THREAD_LIST_SYNC",
        &json!({"guild_id": "100", "channel_ids": ["10"], "threads": []}),
    );
    assert!(item(&s, 10).threads.is_empty());
}

#[test]
fn thread_unread_follows_the_same_rules() {
    let mut s = world_with(
        read_states(json!([{"id": "20", "last_message_id": "200"}])),
        json!({"threads": [thread(20, 10, false)]}),
    );
    // last 200 acked 200
    assert!(!item(&s, 10).threads[0].unread);
    s.apply("MESSAGE_CREATE", &msg(250, 20, Some(100), 2));
    assert!(item(&s, 10).threads[0].unread);
    assert!(rail_guild(&s).unread);
}

#[test]
fn presence_comes_from_ready_supplemental_then_presence_update() {
    let mut s = world(json!({}));
    assert_eq!(
        dm(&s, 500).status,
        Some(PresenceStatus::Offline),
        "none known yet"
    );
    assert_eq!(dm(&s, 501).status, None, "group DMs carry none");
    assert!(s.apply(
        "READY_SUPPLEMENTAL",
        &json!({"merged_presences": {
            "friends": [{"user_id": "2", "status": "online", "activities": []}],
            "guilds": [[{"user_id": "3", "status": "idle"}, {"user_id": "77", "status": "online"}]]}})
    ));
    assert_eq!(dm(&s, 500).status, Some(PresenceStatus::Online));
    assert_eq!(dm(&s, 501).status, None);
    assert!(s.apply(
        "PRESENCE_UPDATE",
        &json!({"user": {"id": "2"}, "status": "dnd", "guild_id": "100"})
    ));
    assert_eq!(dm(&s, 500).status, Some(PresenceStatus::DoNotDisturb));
    s.apply(
        "PRESENCE_UPDATE",
        &json!({"user": {"id": "2"}, "status": "invisible"}),
    );
    assert_eq!(dm(&s, 500).status, Some(PresenceStatus::Offline));
    // Guild members who are not DM recipients are not tracked.
    assert!(!s.apply(
        "PRESENCE_UPDATE",
        &json!({"user": {"id": "77"}, "status": "online"})
    ));
}

#[test]
fn friends_count_adds_and_removes() {
    let mut s = world(json!({"relationships": [
        {"id": "7", "type": 3}, {"id": "8", "type": 1}, {"user": {"id": "9"}, "type": 3}]}));
    assert_eq!(s.friends_pending(), 2);
    assert!(s.apply(
        "RELATIONSHIP_ADD",
        &json!({"id": "10", "type": 3, "user": {"id": "10"}})
    ));
    assert_eq!(s.friends_pending(), 3);
    assert!(s.apply("RELATIONSHIP_REMOVE", &json!({"id": "7", "type": 3})));
    assert_eq!(s.friends_pending(), 2);
    // Accepting a request arrives as an ADD of another type.
    assert!(s.apply("RELATIONSHIP_ADD", &json!({"id": "9", "type": 1})));
    assert_eq!(s.friends_pending(), 1);
    assert!(!s.apply("RELATIONSHIP_REMOVE", &json!({"id": "555"})));
}

fn private_scope(entries: Value) -> Value {
    settings(json!([{"guild_id": null, "muted": false, "channel_overrides": entries}]))
}

#[test]
fn muted_group_dm_plain_message_adds_none_and_a_direct_mention_still_counts() {
    let mut extra = all_read();
    extra["user_guild_settings"] =
        private_scope(json!([{"channel_id": "501", "muted": true}]))["user_guild_settings"].clone();
    let mut s = world(extra);
    s.apply("MESSAGE_CREATE", &msg(310, 501, None, 2));
    let d = dm(&s, 501);
    assert!(!d.unread && d.mention_count == 0, "muted: no badge");
    assert_eq!(
        s.read.status(501, Some(310)),
        (true, 0),
        "and none recorded"
    );
    let mut direct = msg(311, 501, None, 3);
    direct["mentions"] = json!([{"id": "1"}]);
    s.apply("MESSAGE_CREATE", &direct);
    assert_eq!(
        s.read.status(501, Some(311)),
        (true, 1),
        "a direct mention still counts"
    );
    assert_eq!(
        dm(&s, 501).mention_count,
        0,
        "though a muted DM shows nothing"
    );
    // The unmuted DM still counts a plain message.
    s.apply("MESSAGE_CREATE", &msg(312, 500, None, 2));
    assert_eq!(dm(&s, 500).mention_count, 1);
}

#[test]
fn muted_dm_is_excluded_from_the_rail_dm_sum() {
    let mut extra = all_read();
    extra["user_guild_settings"] =
        private_scope(json!([{"channel_id": "501", "muted": true}]))["user_guild_settings"].clone();
    let mut s = world(extra);
    s.apply("MESSAGE_CREATE", &msg(310, 500, None, 2));
    let mut direct = msg(311, 501, None, 3);
    direct["mentions"] = json!([{"id": "1"}]);
    s.apply("MESSAGE_CREATE", &direct);
    s.apply("MESSAGE_CREATE", &msg(312, 501, None, 3));
    let dms = dm_list(&s);
    let sum: u32 = dms.iter().map(|d| d.mention_count).sum();
    assert_eq!(sum, 1, "only the unmuted DM");
    assert!(dms.iter().any(|d| d.unread) && dms.iter().filter(|d| d.unread).count() == 1);
    // The whole private scope muted: every DM drops out.
    s.apply(
        "USER_GUILD_SETTINGS_UPDATE",
        &json!({"guild_id": "@me", "muted": true, "channel_overrides": [], "version": 4}),
    );
    assert!(dm_list(&s)
        .iter()
        .all(|d| !d.unread && d.mention_count == 0));
}

#[test]
fn muted_guild_reaches_the_rail_record() {
    let s = world(settings(json!([{"guild_id": "100", "muted": true}])));
    match &guild_entries(&s)[0] {
        SidebarEntry::Guild(g) => assert!(g.muted),
        SidebarEntry::Folder { .. } => panic!(),
    }
}

#[test]
fn channel_upsert_keeps_the_newest_message_id() {
    let mut s = world(all_read());
    s.apply("MESSAGE_CREATE", &msg(260, 11, Some(100), 2));
    // A CHANNEL_UPDATE without last_message_id must not forget it.
    s.apply(
        "CHANNEL_UPDATE",
        &ch(11, 0, "random", 1, None).tap_guild(100),
    );
    assert!(item(&s, 11).unread);
}

trait TapGuild {
    fn tap_guild(self, g: u64) -> Self;
}
impl TapGuild for Value {
    fn tap_guild(mut self, g: u64) -> Self {
        self["guild_id"] = json!(g.to_string());
        self
    }
}

#[test]
fn only_an_everyone_deny_makes_a_lock() {
    let mut s = world(json!({}));
    let with = |overwrites: Value| {
        json!({"id": "11", "type": 0, "name": "random", "position": 1, "guild_id": "100",
            "permission_overwrites": overwrites})
    };
    // A role the user lacks denying VIEW_CHANNEL is not a lock.
    s.apply(
        "CHANNEL_UPDATE",
        &with(json!([{"id": "777", "type": 0, "allow": "0", "deny": VIEW}])),
    );
    assert!(!item(&s, 11).private, "role deny");
    // @everyone explicitly allowed is not a lock.
    s.apply(
        "CHANNEL_UPDATE",
        &with(json!([{"id": "100", "type": 0, "allow": VIEW, "deny": "0"}])),
    );
    assert!(!item(&s, 11).private, "everyone allow");
    // A member overwrite whose id happens to equal the guild id is not the @everyone role.
    s.apply(
        "CHANNEL_UPDATE",
        &with(json!([{"id": "100", "type": 1, "allow": "0", "deny": VIEW}])),
    );
    assert!(!item(&s, 11).private, "member overwrite");
}

#[test]
fn a_dm_update_without_last_message_id_keeps_it() {
    let mut s = world(all_read());
    s.apply("MESSAGE_CREATE", &msg(310, 500, None, 2));
    assert!(dm(&s, 500).unread);
    // A private CHANNEL_UPDATE that omits last_message_id must not forget the newer one.
    s.apply(
        "CHANNEL_UPDATE",
        &json!({"id": "500", "type": 1, "recipient_ids": ["2"]}),
    );
    assert!(dm(&s, 500).unread);
}

#[test]
fn a_list_sync_that_drops_the_users_membership_reports_a_change() {
    let mut s = world_with(json!({}), json!({"threads": [thread(20, 10, false)]}));
    assert_eq!(threads(&s, 10), vec![20]);
    let before = s.revision;
    assert!(s.apply(
        "THREAD_LIST_SYNC",
        &json!({"guild_id": "100", "channel_ids": ["10"], "threads": [thread(20, 10, false)], "members": []}),
    ));
    assert!(threads(&s, 10).is_empty());
    assert!(s.revision > before, "subscribers must see the change");
}
