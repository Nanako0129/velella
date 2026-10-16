//! Slice F1: permission bits, the open channel's permission set, self state and the thread state
//! filled from them. Every state is built through the same dispatches the session applies.
use crate::permissions::*;
use crate::proto::Message;
use crate::rest_actions::{thread_state, ThreadState, Tri};
use crate::self_state::{GuildEmoji, Timeout};
use crate::settings_proto::tests::settings;
use crate::state::State;
use serde_json::{json, Value};

const ME: u64 = 1;
const GUILD: u64 = 100;
const OWNER: u64 = 999;
const CHANNEL: u64 = 300;
const THREAD: u64 = 400;
/// 2027-01-15T08:00:00Z.
const NOW: i64 = 1_800_000_000_000;
const FUTURE: &str = "2099-01-01T00:00:00.000000+00:00";
const PAST: &str = "2000-01-01T00:00:00+00:00";

fn bits(v: &[u64]) -> u64 {
    v.iter().fold(0, |a, b| a | b)
}

fn ow(id: u64, kind: u8, allow: u64, deny: u64) -> Value {
    json!({"id": id.to_string(), "type": kind, "allow": allow.to_string(), "deny": deny.to_string()})
}

fn role(id: u64, permissions: u64) -> Value {
    json!({"id": id.to_string(), "permissions": permissions.to_string(), "position": 1})
}

/// A READY with one guild (owner `owner`, text channel 300 with `overwrites`) in which the user
/// holds `my_roles` and `member` extras (for example a timeout), carried by `merged_members`.
fn world(owner: u64, roles: &[Value], my_roles: &[u64], member: Value, overwrites: Value) -> State {
    world_in(
        owner,
        roles,
        my_roles,
        member,
        overwrites,
        json!({"mfa_level": 0}),
    )
}

/// `world` with extra fields on the guild object (for example `mfa_level`).
fn world_in(
    owner: u64,
    roles: &[Value],
    my_roles: &[u64],
    member: Value,
    overwrites: Value,
    guild_extra: Value,
) -> State {
    let mut mine = json!({"user_id": ME.to_string(), "roles": my_roles.iter().map(u64::to_string).collect::<Vec<_>>()});
    for (k, v) in member.as_object().into_iter().flatten() {
        mine[k] = v.clone();
    }
    let mut guild = json!({
        "id": GUILD.to_string(), "name": "g", "owner_id": owner.to_string(), "roles": roles,
        "channels": [{"id": CHANNEL.to_string(), "type": 0, "permission_overwrites": overwrites}],
    });
    for (k, v) in guild_extra.as_object().into_iter().flatten() {
        guild[k] = v.clone();
    }
    let mut s = State::default();
    assert!(s.apply(
        "READY",
        &json!({
            "user": {"id": ME.to_string(), "username": "me", "premium_type": 2},
            "guilds": [guild],
            "merged_members": [[mine]],
            "private_channels": [{"id": "500", "type": 1, "recipient_ids": []}],
        })
    ));
    s
}

fn simple(roles: &[Value], my_roles: &[u64], overwrites: Value) -> State {
    world(OWNER, roles, my_roles, json!({}), overwrites)
}

fn perms(s: &State, channel: u64) -> ChannelPermissions {
    channel_permissions_at(s, channel, NOW)
}

fn thread_json(parent: u64, metadata: Value) -> Value {
    let mut t = json!({"id": THREAD.to_string(), "guild_id": GUILD.to_string(), "parent_id": parent.to_string(), "type": 11});
    if !metadata.is_null() {
        t["thread_metadata"] = metadata;
    }
    t
}

#[test]
fn the_table_has_every_bit_of_section_16() {
    assert_eq!(PERMISSION_TABLE.len(), 53);
    let named = |name: &str| PERMISSION_TABLE.iter().find(|(n, _)| *n == name).unwrap().1;
    assert_eq!(named("ADMINISTRATOR"), 3);
    assert_eq!(named("VIEW_CHANNEL"), 10);
    assert_eq!(named("MANAGE_THREADS"), 34);
    assert_eq!(named("SET_VOICE_CHANNEL_STATUS"), 48);
    assert_eq!(named("USE_EXTERNAL_APPS"), 50);
    assert_eq!(named("PIN_MESSAGES"), 51);
    assert_eq!(named("BYPASS_SLOWMODE"), 52);
    assert_eq!(named("MANAGE_OFFICIAL_MESSAGES"), 53);
    assert_eq!(MANAGE_OFFICIAL_MESSAGES, 0x20_0000_0000_0000);
    assert_eq!(PIN_MESSAGES, 0x8_0000_0000_0000);
    // Bit 47 is absent from the official table, and every other number up to 53 is present.
    assert!(PERMISSION_TABLE.iter().all(|(_, b)| *b != 47));
    assert_eq!(ALL_PERMISSIONS.count_ones(), 53);
    assert_eq!(ALL_PERMISSIONS & (1 << 47), 0);
}

#[test]
fn the_owner_holds_everything_even_without_role_data() {
    let s = world(
        ME,
        &[role(GUILD, 0)],
        &[],
        json!({}),
        json!([ow(GUILD, 0, 0, VIEW_CHANNEL)]),
    );
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS)
    );
    // No roles, no member, no overwrites received: still the owner.
    let mut s = State::default();
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"}, "guilds": [
        {"id": "100", "owner_id": "1", "channels": [{"id": "300", "type": 0}]}]}),
    );
    // The elevated-MFA strip reaches the owner too, so even the owner needs the guild's level
    // (or MFA on the account) to be known.
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    s.apply(
        "GUILD_CREATE",
        &json!({"id": "100", "owner_id": "1", "mfa_level": 0, "channels": [{"id": "300", "type": 0}]}),
    );
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS)
    );
}

#[test]
fn an_administrator_role_holds_everything_whatever_the_overwrites_say() {
    let s = simple(
        &[role(GUILD, 0), role(200, ADMINISTRATOR)],
        &[200],
        json!([ow(GUILD, 0, 0, ALL_PERMISSIONS)]),
    );
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS)
    );
}

#[test]
fn a_role_allow_beats_the_everyone_deny() {
    let base = bits(&[VIEW_CHANNEL, SEND_MESSAGES, ADD_REACTIONS]);
    let s = simple(
        &[role(GUILD, base), role(200, 0)],
        &[200],
        json!([
            ow(GUILD, 0, 0, bits(&[SEND_MESSAGES, ADD_REACTIONS])),
            ow(200, 0, SEND_MESSAGES, 0)
        ]),
    );
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, SEND_MESSAGES]))
    );
}

#[test]
fn every_role_deny_applies_before_every_role_allow() {
    let s = simple(
        &[
            role(GUILD, bits(&[VIEW_CHANNEL, ADD_REACTIONS])),
            role(200, 0),
            role(201, 0),
        ],
        &[200, 201],
        json!([ow(200, 0, 0, ADD_REACTIONS), ow(201, 0, ADD_REACTIONS, 0)]),
    );
    assert_eq!(perms(&s, CHANNEL).can(ADD_REACTIONS), Some(true));
}

#[test]
fn a_member_overwrite_beats_the_role_overwrite() {
    let roles = [role(GUILD, VIEW_CHANNEL), role(200, 0)];
    // Role denies, member allows: allowed.
    let s = simple(
        &roles,
        &[200],
        json!([ow(200, 0, 0, SEND_MESSAGES), ow(ME, 1, SEND_MESSAGES, 0)]),
    );
    assert_eq!(perms(&s, CHANNEL).can(SEND_MESSAGES), Some(true));
    // Role allows, member denies: denied. Another member's overwrite is not ours.
    let s = simple(
        &roles,
        &[200],
        json!([
            ow(200, 0, SEND_MESSAGES, 0),
            ow(ME, 1, 0, SEND_MESSAGES),
            ow(77, 1, 0, VIEW_CHANNEL)
        ]),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(VIEW_CHANNEL));
}

#[test]
fn a_thread_takes_its_parents_permissions() {
    let base = bits(&[
        VIEW_CHANNEL,
        ADD_REACTIONS,
        SEND_MESSAGES_IN_THREADS,
        READ_MESSAGE_HISTORY,
    ]);
    let mut s = simple(
        &[role(GUILD, base)],
        &[],
        json!([ow(GUILD, 0, 0, ADD_REACTIONS)]),
    );
    s.apply(
        "THREAD_CREATE",
        &thread_json(CHANNEL, json!({"archived": false, "locked": false})),
    );
    // The parent's overwrite (no reactions) carries over; SEND_MESSAGES follows
    // SEND_MESSAGES_IN_THREADS, which the parent holds.
    assert_eq!(
        perms(&s, THREAD),
        ChannelPermissions::Bits(bits(&[
            VIEW_CHANNEL,
            SEND_MESSAGES_IN_THREADS,
            READ_MESSAGE_HISTORY,
            SEND_MESSAGES
        ]))
    );
    // The parent's own SEND_MESSAGES does not reach the thread without the threads bit.
    let mut s = simple(
        &[role(GUILD, bits(&[VIEW_CHANNEL, SEND_MESSAGES]))],
        &[],
        json!([]),
    );
    s.apply(
        "THREAD_CREATE",
        &thread_json(CHANNEL, json!({"archived": false, "locked": false})),
    );
    assert_eq!(perms(&s, THREAD), ChannelPermissions::Bits(VIEW_CHANNEL));
    // A thread whose parent is not in the guild is unknown.
    let mut s = simple(&[role(GUILD, VIEW_CHANNEL)], &[], json!([]));
    s.apply(
        "THREAD_CREATE",
        &thread_json(301, json!({"archived": false})),
    );
    assert_eq!(perms(&s, THREAD), ChannelPermissions::Unknown);
}

#[test]
fn a_locked_thread_takes_sending_from_those_who_cannot_manage_threads() {
    let send = bits(&[VIEW_CHANNEL, SEND_MESSAGES_IN_THREADS]);
    let mut s = simple(&[role(GUILD, send)], &[], json!([]));
    s.apply(
        "THREAD_CREATE",
        &thread_json(CHANNEL, json!({"archived": false, "locked": true})),
    );
    assert_eq!(perms(&s, THREAD).can(SEND_MESSAGES), Some(false));
    // MANAGE_THREADS keeps it, and so does the owner.
    let mut s = simple(&[role(GUILD, send | MANAGE_THREADS)], &[], json!([]));
    s.apply(
        "THREAD_CREATE",
        &thread_json(CHANNEL, json!({"archived": false, "locked": true})),
    );
    assert_eq!(perms(&s, THREAD).can(SEND_MESSAGES), Some(true));
    // A thread whose metadata said nothing about `locked` is unknown to a member who cannot
    // manage threads, and known to one who can.
    let mut s = simple(&[role(GUILD, send)], &[], json!([]));
    s.apply("THREAD_CREATE", &thread_json(CHANNEL, Value::Null));
    assert_eq!(perms(&s, THREAD), ChannelPermissions::Unknown);
    let mut s = simple(&[role(GUILD, send | MANAGE_THREADS)], &[], json!([]));
    s.apply("THREAD_CREATE", &thread_json(CHANNEL, Value::Null));
    assert_eq!(perms(&s, THREAD).can(SEND_MESSAGES), Some(true));
}

#[test]
fn a_timeout_leaves_only_view_and_history() {
    let all_but_admin = ALL_PERMISSIONS & !ADMINISTRATOR;
    let timed = |until: Value| {
        world(
            OWNER,
            &[role(GUILD, all_but_admin)],
            &[],
            json!({"communication_disabled_until": until}),
            json!([]),
        )
    };
    assert_eq!(
        perms(&timed(json!(FUTURE)), CHANNEL),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, READ_MESSAGE_HISTORY]))
    );
    // Expired, or null: nothing stripped.
    for until in [json!(PAST), Value::Null] {
        assert_eq!(
            perms(&timed(until), CHANNEL),
            ChannelPermissions::Bits(all_but_admin)
        );
    }
    // Not a timestamp: neither known.
    assert_eq!(
        perms(&timed(json!("soon")), CHANNEL),
        ChannelPermissions::Unknown
    );
    // A member who never came with a member object has no timeout record at all.
    let mut s = timed(Value::Null);
    s.self_state.members.clear();
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    // The expiry is compared with the moment asked about.
    let s = timed(json!("2027-01-15T08:00:01Z"));
    assert_eq!(
        channel_permissions_at(&s, CHANNEL, NOW),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, READ_MESSAGE_HISTORY]))
    );
    assert_eq!(
        channel_permissions_at(&s, CHANNEL, NOW + 1000),
        ChannelPermissions::Bits(all_but_admin)
    );
}

#[test]
fn a_timeout_is_ignored_for_an_administrator_and_for_the_owner() {
    let member = json!({"communication_disabled_until": FUTURE});
    let admin = world(
        OWNER,
        &[role(GUILD, 0), role(200, ADMINISTRATOR)],
        &[200],
        member.clone(),
        json!([]),
    );
    assert_eq!(
        perms(&admin, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS)
    );
    let owner = world(ME, &[role(GUILD, 0)], &[], member, json!([]));
    assert_eq!(
        perms(&owner, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS)
    );
    // An unreadable timeout does not matter to them either.
    let owner = world(
        ME,
        &[role(GUILD, 0)],
        &[],
        json!({"communication_disabled_until": "soon"}),
        json!([]),
    );
    assert_eq!(
        perms(&owner, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS)
    );
}

#[test]
fn a_dm_is_private_with_an_empty_set() {
    let s = simple(
        &[role(GUILD, ALL_PERMISSIONS & !ADMINISTRATOR)],
        &[],
        json!([]),
    );
    assert_eq!(perms(&s, 500), ChannelPermissions::Private);
    assert_eq!(perms(&s, 500).can(MANAGE_MESSAGES), Some(false));
    assert_eq!(perms(&s, 500).can(0), Some(false));
}

#[test]
fn missing_data_is_unknown_never_a_guess() {
    let ok = bits(&[VIEW_CHANNEL, SEND_MESSAGES]);
    // A role the member holds that the guild never listed.
    let s = simple(&[role(GUILD, ok)], &[555], json!([]));
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    // No @everyone role.
    let s = simple(&[role(200, ok)], &[200], json!([]));
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    // A channel (or thread) velella does not hold.
    let s = simple(&[role(GUILD, ok)], &[], json!([]));
    assert_eq!(perms(&s, 12345), ChannelPermissions::Unknown);
    assert_eq!(perms(&s, 12345).can(VIEW_CHANNEL), None);
    // Overwrites the payload did not carry.
    let mut s = simple(&[role(GUILD, ok)], &[], json!([]));
    s.guilds[0].channels[0].permission_overwrites = None;
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    // A guild in an outage; no own member known; nobody signed in.
    let mut s = simple(&[role(GUILD, ok)], &[], json!([]));
    s.guilds[0].unavailable = true;
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    let mut s = simple(&[role(GUILD, ok)], &[], json!([]));
    s.guilds[0].member_roles = None;
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    let mut s = simple(&[role(GUILD, ok)], &[], json!([]));
    s.user_id = None;
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    // An obfuscated channel grants nothing.
    let mut s = simple(&[role(GUILD, ok)], &[], json!([]));
    s.guilds[0].channels[0].flags = 1 << 17;
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(0));
}

fn ready_without_member(roles: Value) -> State {
    let mut s = State::default();
    assert!(s.apply(
        "READY",
        &json!({
            "user": {"id": "1", "username": "me"},
            "guilds": [{"id": "100", "owner_id": "999", "mfa_level": 0, "roles": roles, "channels": [{"id": "300", "type": 0, "permission_overwrites": []}]}],
        })
    ));
    s
}

#[test]
fn roles_that_arrive_only_in_ready_supplemental_are_unknown_then_known() {
    let mut s = ready_without_member(json!([role(GUILD, VIEW_CHANNEL), role(200, SEND_MESSAGES)]));
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    assert!(s.apply(
        "READY_SUPPLEMENTAL",
        &json!({
            "guilds": [{"id": "100", "voice_states": []}],
            "merged_members": [[{"user_id": "1", "roles": ["200"]}]],
        })
    ));
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, SEND_MESSAGES]))
    );
}

#[test]
fn a_timeout_carried_in_merged_members_strips_the_permissions() {
    // READY's merged_members.
    let s = world(
        OWNER,
        &[role(
            GUILD,
            bits(&[VIEW_CHANNEL, SEND_MESSAGES, READ_MESSAGE_HISTORY]),
        )],
        &[],
        json!({"communication_disabled_until": FUTURE}),
        json!([]),
    );
    assert_eq!(
        s.self_state.members[&GUILD].timeout,
        Timeout::Until(4_070_908_800_000)
    );
    assert_eq!(perms(&s, CHANNEL).can(SEND_MESSAGES), Some(false));
    // READY_SUPPLEMENTAL's merged_members, for a guild READY gave no member for.
    let mut s = ready_without_member(json!([role(GUILD, bits(&[VIEW_CHANNEL, SEND_MESSAGES]))]));
    s.apply(
        "READY_SUPPLEMENTAL",
        &json!({"guilds": [{"id": "100", "voice_states": []}], "merged_members": [[
            {"user_id": "1", "roles": [], "communication_disabled_until": FUTURE}]]}),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(VIEW_CHANNEL));
    // Another user's entry is not ours.
    let mut s = ready_without_member(json!([role(GUILD, SEND_MESSAGES)]));
    s.apply(
        "READY_SUPPLEMENTAL",
        &json!({"guilds": [{"id": "100", "voice_states": []}], "merged_members": [[
            {"user_id": "77", "roles": [], "communication_disabled_until": FUTURE}]]}),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
}

#[test]
fn guild_member_update_and_guild_create_change_the_timeout() {
    let roles = [role(GUILD, bits(&[VIEW_CHANNEL, SEND_MESSAGES]))];
    let mut s = world(OWNER, &roles, &[], json!({}), json!([]));
    assert_eq!(perms(&s, CHANNEL).can(SEND_MESSAGES), Some(true));
    let update = |user: &str, until: Value| json!({"guild_id": "100", "user": {"id": user}, "roles": [], "communication_disabled_until": until});
    // Someone else's update changes nothing; ours strips, then clears.
    s.apply("GUILD_MEMBER_UPDATE", &update("77", json!(FUTURE)));
    assert_eq!(perms(&s, CHANNEL).can(SEND_MESSAGES), Some(true));
    assert!(s.apply("GUILD_MEMBER_UPDATE", &update("1", json!(FUTURE))));
    assert_eq!(perms(&s, CHANNEL).can(SEND_MESSAGES), Some(false));
    assert!(s.apply("GUILD_MEMBER_UPDATE", &update("1", Value::Null)));
    assert_eq!(perms(&s, CHANNEL).can(SEND_MESSAGES), Some(true));
    // A GUILD_CREATE carries the member too.
    let mut s = State::default();
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"}, "guilds": []}),
    );
    s.apply(
        "GUILD_CREATE",
        &json!({"id": "100", "owner_id": "999", "mfa_level": 0, "roles": roles, "members": [
            {"user": {"id": "1"}, "roles": [], "communication_disabled_until": FUTURE}],
            "channels": [{"id": "300", "type": 0, "permission_overwrites": []}]}),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(VIEW_CHANNEL));
}

fn appearance(developer_mode: bool) -> Message {
    let mut a = Message::new();
    a.push_bool(2, developer_mode);
    a
}

fn text_and_images(render_reactions: bool) -> Message {
    let mut t = Message::new();
    t.push_wrapper_bool(13, render_reactions);
    t
}

#[test]
fn a_settings_proto_update_flips_developer_mode_and_the_reactions_setting() {
    let mut s = State::default();
    assert_eq!((s.developer_mode(), s.render_reactions()), (None, None));
    let base = settings(&[(13, appearance(false)), (6, text_and_images(false))]);
    assert!(s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"}, "guilds": [], "user_settings_proto": base})
    ));
    assert_eq!(
        (s.developer_mode(), s.render_reactions()),
        (Some(false), Some(false))
    );
    let flip = settings(&[(13, appearance(true))]);
    assert!(s.apply(
        "USER_SETTINGS_PROTO_UPDATE",
        &json!({"settings": {"type": 1, "proto": flip}, "partial": true})
    ));
    assert_eq!(
        (s.developer_mode(), s.render_reactions()),
        (Some(true), Some(false))
    );
    // A READY without a proto: both unknown, not defaults.
    let mut s = State::default();
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"}, "guilds": []}),
    );
    assert_eq!((s.developer_mode(), s.render_reactions()), (None, None));
    // A proto without the fields: the official defaults, known.
    let mut s = State::default();
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"}, "guilds": [], "user_settings_proto": settings(&[(13, Message::new())])}),
    );
    assert_eq!(
        (s.developer_mode(), s.render_reactions()),
        (Some(false), Some(true))
    );
}

fn emoji(id: u64, name: &str) -> Value {
    json!({"id": id.to_string(), "name": name, "animated": false, "available": true, "roles": [], "managed": false})
}

fn known(id: u64, name: &str) -> GuildEmoji {
    GuildEmoji {
        id,
        name: name.into(),
        animated: false,
        available: true,
        roles: vec![],
        managed: false,
    }
}

fn emojis_of(s: &State, guild: u64) -> Option<Vec<GuildEmoji>> {
    s.guild_emojis()
        .into_iter()
        .find(|(g, _)| *g == guild)
        .map(|(_, e)| e.to_vec())
}

#[test]
fn guild_emojis_come_from_ready_guild_create_and_are_replaced_by_an_update() {
    let mut s = State::default();
    let odd = json!({"id": "12", "name": "odd", "animated": true, "available": false, "roles": ["5", "6"], "managed": true});
    assert!(s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"}, "guilds": [
            {"id": "100", "emojis": [emoji(10, "a"), emoji(11, "b"), odd]},
            {"id": "101", "unavailable": true}]})
    ));
    assert_eq!(
        emojis_of(&s, 100),
        Some(vec![
            known(10, "a"),
            known(11, "b"),
            GuildEmoji {
                id: 12,
                name: "odd".into(),
                animated: true,
                available: false,
                roles: vec![5, 6],
                managed: true
            }
        ])
    );
    // A stub guild's emojis are unknown, which is not an empty list.
    assert_eq!(emojis_of(&s, 101), None);
    // GUILD_EMOJIS_UPDATE replaces the whole list: b is gone, c is new.
    assert!(s.apply(
        "GUILD_EMOJIS_UPDATE",
        &json!({"guild_id": "100", "emojis": [emoji(10, "a"), emoji(13, "c")]})
    ));
    assert_eq!(
        emojis_of(&s, 100),
        Some(vec![known(10, "a"), known(13, "c")])
    );
    // The same list again is no change; an empty list is known empty.
    assert!(!s.apply(
        "GUILD_EMOJIS_UPDATE",
        &json!({"guild_id": "100", "emojis": [emoji(10, "a"), emoji(13, "c")]})
    ));
    assert!(s.apply(
        "GUILD_EMOJIS_UPDATE",
        &json!({"guild_id": "100", "emojis": []})
    ));
    assert_eq!(emojis_of(&s, 100), Some(vec![]));
    // A GUILD_CREATE for a new guild, then a partial one that changes the held list.
    s.apply(
        "GUILD_CREATE",
        &json!({"id": "102", "emojis": [emoji(20, "x"), emoji(21, "y")]}),
    );
    assert_eq!(
        emojis_of(&s, 102),
        Some(vec![known(20, "x"), known(21, "y")])
    );
    s.apply(
        "GUILD_CREATE",
        &json!({"id": "102", "data_mode": "partial", "properties": null, "partial_updates": {
            "emojis": [emoji(22, "z"), emoji(20, "x2")], "deleted_emoji_ids": ["21"]}}),
    );
    assert_eq!(
        emojis_of(&s, 102),
        Some(vec![known(20, "x2"), known(22, "z")])
    );
    // A partial update never makes an unknown list look complete.
    s.apply("GUILD_CREATE", &json!({"id": "101", "data_mode": "partial", "properties": null, "partial_updates": {"emojis": [emoji(30, "q")]}}));
    assert_eq!(emojis_of(&s, 101), None);
    // A deleted guild's emojis are not offered.
    s.apply("GUILD_DELETE", &json!({"id": "102"}));
    assert_eq!(emojis_of(&s, 102), None);
    // READY starts over.
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"}, "guilds": [{"id": "100"}]}),
    );
    assert_eq!(emojis_of(&s, 100), None);
}

#[test]
fn premium_type_comes_from_ready_and_user_update() {
    let mut s = State::default();
    assert_eq!(s.self_state.premium_type, None);
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"}, "guilds": []}),
    );
    assert_eq!(s.self_state.premium_type, None);
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me", "premium_type": 2}, "guilds": []}),
    );
    assert_eq!(s.self_state.premium_type, Some(2));
    // Someone else's update, and an update without the field, change nothing.
    assert!(!s.apply("USER_UPDATE", &json!({"id": "77", "premium_type": 0})));
    assert!(!s.apply("USER_UPDATE", &json!({"id": "1", "username": "me2"})));
    assert_eq!(s.self_state.premium_type, Some(2));
    assert!(s.apply("USER_UPDATE", &json!({"id": "1", "premium_type": 0})));
    assert_eq!(s.self_state.premium_type, Some(0));
    assert!(!s.apply("USER_UPDATE", &json!({"id": "1", "premium_type": 0})));
}

#[test]
fn the_thread_state_comes_from_the_thread_and_the_permissions() {
    let mut s = simple(
        &[role(GUILD, VIEW_CHANNEL | MANAGE_THREADS)],
        &[],
        json!([]),
    );
    // Not a known thread: all Unknown.
    assert_eq!(thread_state(&s, THREAD), ThreadState::default());
    assert_eq!(thread_state(&s, CHANNEL), ThreadState::default());
    let mut member = thread_json(CHANNEL, json!({"archived": true, "locked": false}));
    member["member"] = json!({"user_id": "1"});
    s.apply("THREAD_CREATE", &member);
    assert_eq!(
        thread_state(&s, THREAD),
        ThreadState {
            archived: Tri::Yes,
            locked: Tri::No,
            joined: Tri::Yes,
            manage_threads: Tri::Yes
        }
    );
    // Metadata that did not say: Unknown, never false. No membership seen: not joined.
    s.apply("THREAD_UPDATE", &thread_json(CHANNEL, Value::Null));
    let t = &s.threads.get(THREAD).unwrap();
    assert!(!t.archived_known && t.locked.is_none());
    assert_eq!(
        thread_state(&s, THREAD),
        ThreadState {
            archived: Tri::Unknown,
            locked: Tri::Unknown,
            joined: Tri::Yes,
            manage_threads: Tri::Yes
        }
    );
    // Without MANAGE_THREADS: No; with the permissions unknown: Unknown.
    let mut s = simple(&[role(GUILD, VIEW_CHANNEL)], &[], json!([]));
    s.apply(
        "THREAD_CREATE",
        &thread_json(CHANNEL, json!({"archived": false, "locked": true})),
    );
    assert_eq!(
        thread_state(&s, THREAD),
        ThreadState {
            archived: Tri::No,
            locked: Tri::Yes,
            joined: Tri::No,
            manage_threads: Tri::No
        }
    );
    s.guilds[0].member_roles = None;
    assert_eq!(thread_state(&s, THREAD).manage_threads, Tri::Unknown);
}

// ---- The official masks beyond the timeout (W:9605796 `U`, W:9606189 `w`, W:9605590 `P`) ----

/// The union `Lti` of the official moderator list `eh` (W:11587777), written out from it.
fn moderator_mask() -> u64 {
    bits(&[
        KICK_MEMBERS,
        BAN_MEMBERS,
        ADMINISTRATOR,
        MANAGE_CHANNELS,
        MANAGE_GUILD,
        MANAGE_ROLES,
        MANAGE_MESSAGES,
        MANAGE_THREADS,
        MODERATE_MEMBERS,
    ])
}

fn set_mfa(s: &mut State, enabled: bool) {
    assert!(s.apply("USER_UPDATE", &json!({"id": "1", "mfa_enabled": enabled})));
}

#[test]
fn a_guild_requiring_mfa_strips_moderator_bits_from_a_user_without_it() {
    let member_bits = bits(&[
        VIEW_CHANNEL,
        SEND_MESSAGES,
        MANAGE_MESSAGES,
        MANAGE_THREADS,
        PIN_MESSAGES,
    ]);
    let elevated = json!({"mfa_level": 1});
    let build = |owner: u64, role_bits: u64| {
        world_in(
            owner,
            &[role(GUILD, role_bits)],
            &[],
            json!({}),
            json!([]),
            elevated.clone(),
        )
    };
    // Without MFA: MANAGE_MESSAGES and MANAGE_THREADS go, PIN_MESSAGES (not in the list) stays.
    let mut s = build(OWNER, member_bits);
    set_mfa(&mut s, false);
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, SEND_MESSAGES, PIN_MESSAGES]))
    );
    // With MFA nothing is stripped.
    set_mfa(&mut s, true);
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(member_bits));
    // The owner and an administrator lose the moderator list too, and keep the rest.
    let mut owner = build(ME, 0);
    set_mfa(&mut owner, false);
    assert_eq!(
        perms(&owner, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS & !moderator_mask())
    );
    let mut admin = build(OWNER, ADMINISTRATOR);
    set_mfa(&mut admin, false);
    assert_eq!(
        perms(&admin, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS & !moderator_mask())
    );
    // A guild that does not require MFA strips nothing.
    let mut s = world_in(
        OWNER,
        &[role(GUILD, member_bits)],
        &[],
        json!({}),
        json!([]),
        json!({"mfa_level": 0}),
    );
    set_mfa(&mut s, false);
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(member_bits));
}

#[test]
fn a_guild_update_turning_the_mfa_requirement_on_or_off_applies_at_once() {
    let member_bits = bits(&[VIEW_CHANNEL, SEND_MESSAGES, MANAGE_MESSAGES]);
    let mut s = world_in(
        OWNER,
        &[role(GUILD, member_bits)],
        &[],
        json!({}),
        json!([]),
        json!({"mfa_level": 0}),
    );
    set_mfa(&mut s, false);
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(member_bits));
    s.apply(
        "GUILD_UPDATE",
        &json!({"id": GUILD.to_string(), "mfa_level": 1}),
    );
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, SEND_MESSAGES]))
    );
    // CLIENT_STATE_V2 carries it in `properties`.
    s.apply(
        "GUILD_UPDATE",
        &json!({"id": GUILD.to_string(), "properties": {"mfa_level": 0}}),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(member_bits));
    // An update without the field keeps the held level.
    s.apply(
        "GUILD_UPDATE",
        &json!({"id": GUILD.to_string(), "name": "renamed"}),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(member_bits));
}

#[test]
fn the_mfa_strip_is_unknown_when_its_inputs_are_missing() {
    let member_bits = bits(&[VIEW_CHANNEL, MANAGE_MESSAGES]);
    // The guild never said its level (and the user has no MFA on record): Unknown.
    let s = world_in(
        OWNER,
        &[role(GUILD, member_bits)],
        &[],
        json!({}),
        json!([]),
        json!({}),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    // A user known to have MFA needs no level.
    let mut s = s;
    set_mfa(&mut s, true);
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(member_bits));
    // An elevated guild and a user whose MFA state never arrived: Unknown.
    let s = world_in(
        OWNER,
        &[role(GUILD, member_bits)],
        &[],
        json!({}),
        json!([]),
        json!({"mfa_level": 1}),
    );
    assert_eq!(s.self_state.mfa_enabled, None);
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    // READY's user and a GUILD_CREATE with the level under `properties` feed both inputs.
    let mut s = State::default();
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me", "mfa_enabled": false}, "guilds": []}),
    );
    s.apply(
        "GUILD_CREATE",
        &json!({"id": "100", "properties": {"owner_id": "999", "mfa_level": 1}, "roles": [role(GUILD, member_bits)],
            "members": [{"user": {"id": "1"}, "roles": []}],
            "channels": [{"id": "300", "type": 0, "permission_overwrites": []}]}),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(VIEW_CHANNEL));
}

#[test]
fn a_pending_member_keeps_only_view_and_history_even_as_an_administrator() {
    let all_but_admin = ALL_PERMISSIONS & !ADMINISTRATOR;
    let pending = json!({"pending": true});
    let keeps = bits(&[VIEW_CHANNEL, READ_MESSAGE_HISTORY]);
    let s = world(
        OWNER,
        &[role(GUILD, all_but_admin)],
        &[],
        pending.clone(),
        json!([]),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(keeps));
    // A role may give less than the mask: the mask only takes away.
    let s = world(
        OWNER,
        &[role(GUILD, bits(&[VIEW_CHANNEL, SEND_MESSAGES]))],
        &[],
        pending.clone(),
        json!([]),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(VIEW_CHANNEL));
    // The official `w` applies it after the administrator shortcut: administrators too.
    let s = world(
        OWNER,
        &[role(GUILD, ADMINISTRATOR)],
        &[],
        pending.clone(),
        json!([]),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(keeps));
    // The owner never reaches `w` (W:9606751 returns first).
    let s = world(ME, &[role(GUILD, 0)], &[], pending.clone(), json!([]));
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS)
    );
    // A public stage channel gives a pending member more (`computeLurkerPermissionsAllowList`,
    // which needs the stage instance): not known here.
    let mut s = world(
        OWNER,
        &[role(GUILD, all_but_admin)],
        &[],
        pending,
        json!([]),
    );
    s.guilds[0].channels[0].kind = 13;
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Unknown);
    // Screening passed (a GUILD_MEMBER_UPDATE of the user): everything is back.
    let mut s = world(
        OWNER,
        &[role(GUILD, all_but_admin)],
        &[],
        json!({"pending": true}),
        json!([]),
    );
    assert!(s.apply(
        "GUILD_MEMBER_UPDATE",
        &json!({"guild_id": "100", "user": {"id": "1"}, "roles": [], "pending": false})
    ));
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(all_but_admin));
}

#[test]
fn a_quarantined_member_keeps_view_history_and_nickname_unless_administrator() {
    let all_but_admin = ALL_PERMISSIONS & !ADMINISTRATOR;
    let keeps = bits(&[VIEW_CHANNEL, READ_MESSAGE_HISTORY, CHANGE_NICKNAME]);
    // Each of the three AutoMod quarantine flags (module 340837): 128, 256, 1024.
    for flags in [128, 256, 1024, 256 | 2] {
        let s = world(
            OWNER,
            &[role(GUILD, all_but_admin)],
            &[],
            json!({"flags": flags}),
            json!([]),
        );
        assert_eq!(
            perms(&s, CHANNEL),
            ChannelPermissions::Bits(keeps),
            "flags {flags}"
        );
    }
    // Other member flags (512 is DM_SETTINGS_UPSELL_ACKNOWLEDGED, 1 DID_REJOIN) do nothing.
    for flags in [0, 1, 512] {
        let s = world(
            OWNER,
            &[role(GUILD, all_but_admin)],
            &[],
            json!({"flags": flags}),
            json!([]),
        );
        assert_eq!(
            perms(&s, CHANNEL),
            ChannelPermissions::Bits(all_but_admin),
            "flags {flags}"
        );
    }
    // An administrator, and the owner, are not touched (W:9605796: `!l`; W:9606751 owner first).
    let s = world(
        OWNER,
        &[role(GUILD, ADMINISTRATOR)],
        &[],
        json!({"flags": 256}),
        json!([]),
    );
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(ALL_PERMISSIONS)
    );
    // A GUILD_MEMBER_UPDATE of the user lifts it.
    let mut s = world(
        OWNER,
        &[role(GUILD, all_but_admin)],
        &[],
        json!({"flags": 256}),
        json!([]),
    );
    s.apply(
        "GUILD_MEMBER_UPDATE",
        &json!({"guild_id": "100", "user": {"id": "1"}, "roles": [], "flags": 0}),
    );
    assert_eq!(perms(&s, CHANNEL), ChannelPermissions::Bits(all_but_admin));
}

fn private_thread(joined: bool) -> Value {
    let mut t = json!({"id": THREAD.to_string(), "guild_id": GUILD.to_string(), "parent_id": CHANNEL.to_string(),
        "type": 12, "thread_metadata": {"archived": false, "locked": false}});
    if joined {
        t["member"] = json!({"user_id": "1"});
    }
    t
}

#[test]
fn a_private_thread_shows_nothing_to_a_member_who_has_not_joined_it() {
    let base = bits(&[VIEW_CHANNEL, SEND_MESSAGES_IN_THREADS, ADD_REACTIONS]);
    let mut s = simple(&[role(GUILD, base)], &[], json!([]));
    s.apply("THREAD_CREATE", &private_thread(false));
    assert_eq!(s.threads.get(THREAD).unwrap().kind, 12);
    assert_eq!(perms(&s, THREAD), ChannelPermissions::Bits(0));
    // Joined: the normal thread rule.
    let mut s = simple(&[role(GUILD, base)], &[], json!([]));
    s.apply("THREAD_CREATE", &private_thread(true));
    assert_eq!(
        perms(&s, THREAD),
        ChannelPermissions::Bits(base | SEND_MESSAGES)
    );
    // MANAGE_THREADS sees it without joining.
    let mut s = simple(&[role(GUILD, base | MANAGE_THREADS)], &[], json!([]));
    s.apply("THREAD_CREATE", &private_thread(false));
    assert_eq!(
        perms(&s, THREAD),
        ChannelPermissions::Bits(base | MANAGE_THREADS | SEND_MESSAGES)
    );
    // A public thread under the same parent is not affected by joining.
    let mut s = simple(&[role(GUILD, base)], &[], json!([]));
    s.apply(
        "THREAD_CREATE",
        &thread_json(CHANNEL, json!({"archived": false, "locked": false})),
    );
    assert_eq!(s.threads.get(THREAD).unwrap().kind, 11);
    assert_eq!(
        perms(&s, THREAD),
        ChannelPermissions::Bits(base | SEND_MESSAGES)
    );
}

#[test]
fn an_initial_guild_feeds_self_state_like_a_ready_guild() {
    // The real fast-connect sequence: INITIAL_GUILD first, then a READY that does not list it.
    let mut s = State::default();
    let roles = [
        role(
            GUILD,
            bits(&[VIEW_CHANNEL, SEND_MESSAGES, READ_MESSAGE_HISTORY]),
        ),
        role(200, 0),
    ];
    assert!(!s.apply(
        "INITIAL_GUILD",
        &json!({"id": "100", "owner_id": "999", "mfa_level": 0, "roles": roles,
            "members": [{"user": {"id": "1"}, "roles": ["200"], "communication_disabled_until": FUTURE}],
            "emojis": [emoji(10, "a")],
            "channels": [{"id": "300", "type": 0, "permission_overwrites": []}]})
    ));
    assert!(s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me", "premium_type": 2}, "guilds": [{"id": "101", "unavailable": true}]})
    ));
    // Roles, the timeout and the emojis all arrived with the initial guild.
    assert_eq!(s.guild(100).unwrap().member_roles, Some(vec![200]));
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, READ_MESSAGE_HISTORY]))
    );
    assert_eq!(emojis_of(&s, 100), Some(vec![known(10, "a")]));
    // The same sequence with the timeout cleared by a later READY_SUPPLEMENTAL for that guild.
    assert!(s.apply(
        "READY_SUPPLEMENTAL",
        &json!({"guilds": [{"id": "100", "voice_states": []}], "merged_members": [[{"user_id": "1", "roles": ["200"]}]]})
    ));
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, SEND_MESSAGES, READ_MESSAGE_HISTORY]))
    );
    // READY's own copy of a guild wins over a buffered one of the same id, for self state too.
    let mut s = State::default();
    s.apply(
        "INITIAL_GUILD",
        &json!({"id": "100", "owner_id": "999", "mfa_level": 0, "roles": roles,
            "members": [{"user": {"id": "1"}, "roles": [], "communication_disabled_until": FUTURE}],
            "channels": [{"id": "300", "type": 0, "permission_overwrites": []}]}),
    );
    s.apply(
        "READY",
        &json!({"user": {"id": "1", "username": "me"},
            "guilds": [{"id": "100", "owner_id": "999", "mfa_level": 0, "roles": roles,
                "channels": [{"id": "300", "type": 0, "permission_overwrites": []}]}],
            "merged_members": [[{"user_id": "1", "roles": []}]]}),
    );
    assert_eq!(
        perms(&s, CHANNEL),
        ChannelPermissions::Bits(bits(&[VIEW_CHANNEL, SEND_MESSAGES, READ_MESSAGE_HISTORY]))
    );
}
