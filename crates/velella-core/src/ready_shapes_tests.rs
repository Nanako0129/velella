//! READY and dispatch shapes of the official capabilities (menu plan slice ID-a) next to the
//! capabilities-253 shapes velella still receives: settings proto (bit 9), CLIENT_STATE_V2 guilds
//! (bit 10), PASSIVE_UPDATE_V2 (bit 14), INITIAL_GUILD, CHANNEL_SYNC, STATE_UPDATE,
//! VOICE_STATE_UPDATE_BATCH, and obfuscated channels (identify.md §1, §3, §5).
use crate::proto::Message;
use crate::settings_proto::tests::{folder_msg, folders_field, settings, status_field};
use crate::sidebar_tests::{ch, guild, ready};
use crate::state::State;
use crate::{channel_categories, dm_list, guild_entries, SidebarEntry};
use serde_json::{json, Value};

fn applied(d: &Value) -> State {
    let mut s = State::default();
    assert!(s.apply("READY", d));
    s
}

/// The rail as ids: `g` for a guild, `f<id>[..]` for a folder.
fn rail(s: &State) -> Vec<String> {
    guild_entries(s)
        .into_iter()
        .map(|e| match e {
            SidebarEntry::Guild(g) => g.id.to_string(),
            SidebarEntry::Folder { id, guilds, .. } => format!(
                "f{}{:?}",
                id.unwrap(),
                guilds.iter().map(|g| g.id).collect::<Vec<_>>()
            ),
        })
        .collect()
}

fn guilds(ids: &[u64]) -> Vec<Value> {
    ids.iter().map(|i| guild(*i, vec![])).collect()
}

fn with_proto(mut d: Value, b64: String) -> Value {
    d["user_settings_proto"] = json!(b64);
    d
}

fn proto_update(kind: u64, b64: String, partial: bool) -> Value {
    json!({"settings": {"type": kind, "proto": b64}, "partial": partial})
}

// ---- Bit 9: settings proto, with the legacy shape as the fallback ----

#[test]
fn proto_folders_order_the_rail_and_win_over_legacy() {
    let b64 = settings(&[(
        14,
        folders_field(
            &[
                folder_msg(None, None, &[30]),
                folder_msg(Some(0), Some("zero"), &[10, 20]),
            ],
            &[],
        ),
    )]);
    let legacy = json!([{"id": 9, "guild_ids": ["10", "20", "30"]}]);
    let s = applied(&with_proto(
        ready(guilds(&[10, 20, 30]), legacy, json!([]), vec![]),
        b64,
    ));
    assert_eq!(rail(&s), ["30", "f0[10, 20]"]);
    let st = s.user_settings.as_ref().unwrap();
    assert!(!st.developer_mode && st.render_reactions);
}

#[test]
fn a_broken_proto_leaves_ready_applied_and_legacy_folders_in_use() {
    let legacy = json!([{"id": 7, "guild_ids": ["10", "20"]}]);
    for broken in ["%%%", "cgUB", ""] {
        let d = with_proto(
            ready(guilds(&[10, 20]), legacy.clone(), json!([]), vec![]),
            broken.into(),
        );
        let s = applied(&d);
        assert_eq!(s.guilds.len(), 2, "READY applied with proto {broken:?}");
        if broken.is_empty() {
            // An empty proto is a valid one with no folders: proto wins, nothing grouped.
            assert!(s.user_settings.is_some());
            assert_eq!(rail(&s), ["20", "10"]);
        } else {
            assert!(
                s.user_settings.is_none(),
                "{broken:?} must leave settings unknown"
            );
            assert_eq!(rail(&s), ["f7[10, 20]"]);
        }
    }
}

#[test]
fn only_a_missing_user_fails_ready() {
    let mut d = ready(guilds(&[10]), Value::Null, json!([]), vec![]);
    for (k, v) in [
        ("user_settings_proto", json!(12)),
        ("auth_token", json!({"x": 1})),
        ("apex_experiments", json!("nope")),
        ("qos_token", json!([1])),
        ("read_state", json!("x")),
        ("user_guild_settings", json!(7)),
        ("private_channels", json!({"not": "an array"})),
    ] {
        d[k] = v;
    }
    d["guilds"]
        .as_array_mut()
        .unwrap()
        .push(json!({"id": "11", "data_mode": "partial", "properties": 5}));
    let s = applied(&d);
    assert_eq!(s.ready_guild_ids(), vec![10, 11]);
    assert!(s.guild(11).unwrap().unavailable);
    assert!(s.user_settings.is_none() && s.apex_experiments.is_empty());
    d.as_object_mut().unwrap().remove("user");
    assert!(!State::default().apply("READY", &d));
}

#[test]
fn folders_with_and_without_ids_follow_the_official_rules() {
    // Legacy shape: id 0 is a folder, a single guild with an id is still a folder, an entry
    // without an id places only its first guild, unknown guilds and empty folders vanish.
    let legacy = json!([
        {"id": null, "guild_ids": ["10", "20"]},
        {"id": 0, "name": "zero", "guild_ids": ["30"]},
        {"id": 5, "guild_ids": ["777"]},
        {"id": "6", "guild_ids": ["40", "999", "10"]}
    ]);
    let s = applied(&ready(
        guilds(&[10, 20, 30, 40, 50]),
        legacy,
        json!([]),
        vec![],
    ));
    // 50 and 20 were never placed: on top, the most recently added first.
    assert_eq!(rail(&s), ["50", "20", "10", "f0[30]", "f6[40]"]);
}

#[test]
fn guild_positions_order_the_rail_only_without_folders() {
    let b64 = settings(&[(14, folders_field(&[], &[30, 999, 10]))]);
    let mut s = applied(&with_proto(
        ready(guilds(&[10, 20, 30]), Value::Null, json!([]), vec![]),
        b64,
    ));
    assert_eq!(rail(&s), ["20", "30", "10"]);
    assert!(s.apply("GUILD_CREATE", &guild(40, vec![])));
    assert_eq!(rail(&s), ["40", "20", "30", "10"]);
    // With a folder present, positions are ignored.
    let b64 = settings(&[(
        14,
        folders_field(&[folder_msg(None, None, &[10])], &[30, 20]),
    )]);
    let s = applied(&with_proto(
        ready(guilds(&[10, 20, 30]), Value::Null, json!([]), vec![]),
        b64,
    ));
    assert_eq!(rail(&s), ["30", "20", "10"]);
}

#[test]
fn settings_updates_by_type_and_partial() {
    let base = settings(&[
        (
            14,
            folders_field(&[folder_msg(Some(4), None, &[10, 20])], &[]),
        ),
        (11, status_field("idle", None)),
    ]);
    let mut s = applied(&with_proto(
        ready(guilds(&[10, 20]), Value::Null, json!([]), vec![]),
        base,
    ));
    let status = |s: &State| s.user_settings.as_ref().unwrap().status.clone();
    let rev = s.revision;
    // Type 2 (frecency) and unknown types leave folders and status alone.
    let frecency = settings(&[(1, Message::new())]);
    assert!(!s.apply(
        "USER_SETTINGS_PROTO_UPDATE",
        &proto_update(2, frecency.clone(), false)
    ));
    assert!(!s.apply(
        "USER_SETTINGS_PROTO_UPDATE",
        &proto_update(3, frecency, false)
    ));
    assert_eq!(
        (rail(&s), status(&s)),
        (vec!["f4[10, 20]".to_owned()], Some("idle".into()))
    );
    assert_eq!(s.revision, rev);
    // A broken type-1 update keeps the previous values.
    assert!(!s.apply(
        "USER_SETTINGS_PROTO_UPDATE",
        &proto_update(1, "%%".into(), true)
    ));
    assert_eq!(status(&s), Some("idle".into()));
    // Partial: the status field replaced whole, folders kept.
    let dnd = settings(&[(11, status_field("dnd", None))]);
    assert!(s.apply(
        "USER_SETTINGS_PROTO_UPDATE",
        &proto_update(1, dnd.clone(), true)
    ));
    assert_eq!(
        (rail(&s), status(&s)),
        (vec!["f4[10, 20]".to_owned()], Some("dnd".into()))
    );
    // Full: the whole proto replaced, so the folders are gone.
    assert!(s.apply("USER_SETTINGS_PROTO_UPDATE", &proto_update(1, dnd, false)));
    assert_eq!(rail(&s), ["20", "10"]);
}

#[test]
fn a_partial_settings_update_on_unknown_settings_is_ignored() {
    let mut s = applied(&ready(guilds(&[10]), Value::Null, json!([]), vec![]));
    let dnd = settings(&[(11, status_field("dnd", None))]);
    assert!(!s.apply(
        "USER_SETTINGS_PROTO_UPDATE",
        &proto_update(1, dnd.clone(), true)
    ));
    assert!(s.user_settings.is_none());
    assert!(s.apply("USER_SETTINGS_PROTO_UPDATE", &proto_update(1, dnd, false)));
    assert_eq!(s.user_settings.unwrap().status.as_deref(), Some("dnd"));
}

// ---- Bit 10: CLIENT_STATE_V2 guilds ----

fn v2_guild(id: u64, mode: &str, extra: Value) -> Value {
    let mut g = json!({"id": id.to_string(), "data_mode": mode, "version": 3,
        "properties": {"id": id.to_string(), "name": format!("p{id}"), "icon": "ic", "owner_id": "999"},
        "roles": [{"id": id.to_string(), "permissions": (1u64 << 10).to_string(), "position": 0}],
        "channels": [ch(id + 1, 0, "general", 0, None)]});
    for (k, v) in extra.as_object().cloned().unwrap_or_default() {
        g[k] = v;
    }
    g
}

#[test]
fn full_guilds_read_their_fields_from_properties() {
    let s = applied(&ready(
        vec![v2_guild(10, "full", json!({}))],
        Value::Null,
        json!([]),
        vec![],
    ));
    let g = s.guild(10).unwrap();
    assert_eq!(
        (g.name.as_deref(), g.owner_id, g.unavailable),
        (Some("p10"), Some(999), false)
    );
    let cats = channel_categories(&s, 10);
    assert_eq!(cats[0].channels[0].name, "general");
}

#[test]
fn a_partial_guild_without_a_held_guild_is_an_unavailable_stub() {
    let partial = json!({"id": "10", "data_mode": "partial", "properties": null,
        "partial_updates": {"channels": [ch(12, 0, "new", 1, None)]}});
    let s = applied(&ready(
        vec![partial.clone()],
        Value::Null,
        json!([]),
        vec![],
    ));
    let g = s.guild(10).unwrap();
    assert!(g.unavailable && g.channels.is_empty() && g.name.is_none());
    assert!(channel_categories(&s, 10).is_empty());
    // The same for GUILD_CREATE after READY.
    let mut s = applied(&ready(vec![], Value::Null, json!([]), vec![]));
    let mut late = partial;
    late["id"] = json!("20");
    assert!(s.apply("GUILD_CREATE", &late));
    assert!(s.guild(20).unwrap().unavailable);
    assert_eq!(s.post_ready_guild_ids(), vec![20]);
}

#[test]
fn a_partial_guild_applies_its_updates_to_the_held_guild() {
    let mut s = applied(&ready(
        vec![v2_guild(10, "full", json!({}))],
        Value::Null,
        json!([]),
        vec![],
    ));
    let partial = json!({"id": "10", "data_mode": "partial", "properties": null,
        "partial_updates": {
            "channels": [ch(12, 0, "added", 1, None)],
            "deleted_channel_ids": ["11"],
            "roles": [{"id": "7", "permissions": "0", "position": 1}],
            "deleted_role_ids": []
        },
        "channel_updates": [{"id": "12", "last_message_id": "500"}]});
    assert!(s.apply("GUILD_CREATE", &partial));
    let g = s.guild(10).unwrap();
    assert_eq!(
        g.name.as_deref(),
        Some("p10"),
        "null properties keep the held ones"
    );
    assert_eq!(
        g.channels.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![12]
    );
    assert_eq!(g.channels[0].last_message_id, Some(500));
    assert_eq!(g.roles.as_ref().unwrap().len(), 2);
    assert!(!g.unavailable);
    // A later READY with the partial form keeps the held guild too.
    let mut renamed = partial.clone();
    renamed["properties"] = json!({"name": "renamed"});
    renamed["partial_updates"] = json!({});
    assert!(s.apply(
        "READY",
        &ready(vec![renamed], Value::Null, json!([]), vec![])
    ));
    let g = s.guild(10).unwrap();
    assert_eq!((g.name.as_deref(), g.channels.len()), (Some("renamed"), 1));
    assert_eq!(s.ready_guild_ids(), vec![10]);
}

#[test]
fn initial_guild_is_merged_only_when_ready_lacks_it() {
    let mut s = State::default();
    assert!(!s.apply("INITIAL_GUILD", &v2_guild(30, "full", json!({}))));
    assert!(s.apply(
        "READY",
        &ready(guilds(&[10]), Value::Null, json!([]), vec![])
    ));
    assert_eq!(s.ready_guild_ids(), vec![10, 30]);
    assert_eq!(s.guild(30).unwrap().name.as_deref(), Some("p30"));
    // Consumed: the next READY does not bring it back.
    assert!(s.apply(
        "READY",
        &ready(guilds(&[10]), Value::Null, json!([]), vec![])
    ));
    assert_eq!(s.ready_guild_ids(), vec![10]);
    // When READY lists it, READY's copy is the one kept, once.
    let mut s = State::default();
    s.apply("INITIAL_GUILD", &v2_guild(10, "full", json!({})));
    assert!(s.apply(
        "READY",
        &ready(guilds(&[10]), Value::Null, json!([]), vec![])
    ));
    assert_eq!(s.ready_guild_ids(), vec![10]);
    assert_eq!(s.guild(10).unwrap().name.as_deref(), Some("g10"));
}

// ---- Bit 14 and the other dispatches ----

#[test]
fn passive_update_v2_raises_unread_like_channel_unread_update() {
    let g = guild(
        100,
        vec![ch(101, 0, "a", 0, None), ch(102, 0, "b", 1, None)],
    );
    let mut d = ready(vec![g], Value::Null, json!([]), vec![]);
    d["read_state"] = json!({"entries": [
        {"id": "101", "last_message_id": "50"}, {"id": "102", "last_message_id": "50"}]});
    d["users"] = json!([{"id": "2", "username": "x"}]);
    let mut s = applied(&d);
    let unread = |s: &State| {
        channel_categories(s, 100)[0]
            .channels
            .iter()
            .map(|c| c.unread)
            .collect::<Vec<_>>()
    };
    assert_eq!(unread(&s), [false, false]);
    assert!(s.apply(
        "PASSIVE_UPDATE_V2",
        &json!({"guild_id": "100",
        "updated_members": [{"user": {"id": "1"}, "roles": ["100"]}],
        "updated_channels": [{"id": "101", "last_message_id": "60", "last_pin_timestamp": null}],
        "updated_voice_states": [{"user_id": "2", "channel_id": "102"}],
        "removed_voice_states": []})
    ));
    assert_eq!(unread(&s), [true, false]);
    assert_eq!(s.guild(100).unwrap().member_roles, Some(vec![100]));
    assert_eq!(s.voice.in_channel(102).count(), 1);
    assert!(s.apply(
        "PASSIVE_UPDATE_V2",
        &json!({"guild_id": "100", "updated_members": [],
        "updated_channels": [], "updated_voice_states": [], "removed_voice_states": ["2"]})
    ));
    assert_eq!(s.voice.in_channel(102).count(), 0);
    // The 253 shape still works alongside it.
    assert!(s.apply(
        "CHANNEL_UNREAD_UPDATE",
        &json!({"guild_id": "100",
        "channel_unread_updates": [{"id": "102", "last_message_id": "70"}]})
    ));
    assert_eq!(unread(&s), [true, true]);
}

#[test]
fn channel_sync_merges_present_keys_unless_integrity_check() {
    let mut c = ch(101, 0, "keep", 3, None);
    c["last_message_id"] = json!("40");
    c["flags"] = json!(4);
    let mut s = applied(&ready(
        vec![guild(100, vec![c])],
        Value::Null,
        json!([]),
        vec![],
    ));
    let get = |s: &State| s.guild(100).unwrap().channels[0].clone();
    for check in [json!(true), json!("hash"), json!(1)] {
        assert!(!s.apply(
            "CHANNEL_SYNC",
            &json!({"guild_id": "100", "integrity_check": check,
            "channels": [{"id": "101", "name": "changed"}]})
        ));
    }
    assert_eq!(get(&s).name.as_deref(), Some("keep"));
    assert!(s.apply(
        "CHANNEL_SYNC",
        &json!({"guild_id": "100", "integrity_check": false,
        "channels": [{"id": "101", "name": "renamed"}, ch(102, 0, "new", 0, None)]})
    ));
    let c = get(&s);
    assert_eq!(
        (c.name.as_deref(), c.position, c.last_message_id, c.flags),
        (Some("renamed"), Some(3), Some(40), 0),
        "absent keys kept, flags taken from the payload"
    );
    assert_eq!(s.guild(100).unwrap().channels.len(), 2);
}

#[test]
fn state_update_and_voice_batch_are_kept() {
    let mut d = ready(vec![guild(100, vec![])], Value::Null, json!([]), vec![]);
    d["apex_experiments"] = json!({"assignments": {}, "installation": "i"});
    let mut s = applied(&d);
    assert!(s.apply(
        "STATE_UPDATE",
        &json!({"apex_experiments": {"assignments": {"1": {}}}})
    ));
    assert!(!s.apply("STATE_UPDATE", &json!({})));
    assert_eq!(s.apex_experiments.len(), 2);
    assert_eq!(s.apex_experiments[0]["installation"], "i");
    assert!(s.apply(
        "VOICE_STATE_UPDATE_BATCH",
        &json!({"voice_states": [
        {"guild_id": "100", "user_id": "2", "channel_id": "5"},
        {"guild_id": "100", "user_id": "3", "channel_id": "5"}]})
    ));
    assert_eq!(s.voice.in_channel(5).count(), 2);
}

// ---- Obfuscated channels (I-5) ----

#[test]
fn obfuscated_channels_and_dms_are_excluded_everywhere() {
    const OBF: u64 = 1 << 17;
    let mut hidden = ch(101, 0, "secret", 0, None);
    hidden["flags"] = json!(OBF);
    hidden["last_message_id"] = json!("9");
    let open = ch(102, 0, "open", 1, None);
    // Owned by the user: ownership would otherwise see everything.
    let mut g = guild(100, vec![hidden, open]);
    g["owner_id"] = json!("1");
    let dms = json!([
        {"id": "500", "type": 3, "name": "club", "recipient_ids": ["2"], "last_message_id": "9", "flags": OBF},
        {"id": "501", "type": 1, "recipient_ids": ["2"], "last_message_id": "9"}
    ]);
    let s = applied(&ready(
        vec![g],
        Value::Null,
        dms,
        vec![json!({"id": "2", "username": "x"})],
    ));
    let c = &s.guild(100).unwrap().channels[0];
    assert_eq!(s.can_view_channel(c), Some(false));
    // Only the open channel is listed; with no read state it is unread, the hidden one adds nothing.
    let cats = channel_categories(&s, 100);
    assert_eq!(
        cats[0].channels.iter().map(|c| c.id).collect::<Vec<_>>(),
        vec![102]
    );
    let dm = dm_list(&s);
    let club = dm.iter().find(|d| d.id == 500).unwrap();
    assert_eq!(
        (club.name.as_str(), club.unread, club.mention_count),
        ("No Access", false, 0)
    );
    assert!(dm.iter().find(|d| d.id == 501).unwrap().unread);
    // No op-37 target for it; the open channel still is one.
    assert_eq!(s.member_list_target(100, 101), None);
    assert_eq!(s.member_list_target(100, 102), Some((100, 102)));
    assert!(s.is_obfuscated(101) && s.is_obfuscated(500) && !s.is_obfuscated(102));
}

#[test]
fn obfuscated_channel_badges_stay_out_of_the_guild_total() {
    const OBF: u64 = 1 << 17;
    let mut hidden = ch(101, 0, "secret", 0, None);
    hidden["flags"] = json!(OBF);
    hidden["last_message_id"] = json!("9");
    let mut d = ready(
        vec![guild(100, vec![hidden])],
        Value::Null,
        json!([]),
        vec![],
    );
    d["read_state"] =
        json!({"entries": [{"id": "101", "last_message_id": "1", "mention_count": 3}]});
    let s = applied(&d);
    match &guild_entries(&s)[0] {
        SidebarEntry::Guild(g) => assert_eq!((g.unread, g.mention_count), (false, 0)),
        e => panic!("{e:?}"),
    }
}
