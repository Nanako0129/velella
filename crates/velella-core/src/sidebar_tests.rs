//! Pure sidebar functions over synthetic READY states (user 1; every guild owned by 999, so
//! visibility comes from roles and overwrites).
use crate::state::State;
use crate::{
    channel_categories, dm_list, guild_entries, SidebarCategory, SidebarChannel, SidebarDm,
    SidebarEntry, SidebarGuild,
};
use serde_json::{json, Value};

const VIEW: u64 = 1 << 10;

pub(crate) fn guild(id: u64, channels: Vec<Value>) -> Value {
    json!({"id": id.to_string(), "name": format!("g{id}"), "icon": null, "owner_id": "999",
        "roles": [{"id": id.to_string(), "permissions": VIEW.to_string(), "position": 0}],
        "channels": channels})
}

fn user(id: u64, name: &str, discriminator: &str, avatar: Value) -> Value {
    json!({"id": id.to_string(), "username": name, "global_name": null, "avatar": avatar, "discriminator": discriminator})
}

pub(crate) fn ready(guilds: Vec<Value>, folders: Value, dms: Value, users: Vec<Value>) -> Value {
    let members: Vec<Value> = guilds
        .iter()
        .map(|_| json!([{"user_id": "1", "roles": []}]))
        .collect();
    let mut d = json!({
        "user": user(1, "me", "0", Value::Null), "users": users,
        "guilds": guilds, "merged_members": members, "private_channels": dms,
        "session_id": "s", "resume_gateway_url": "wss://gateway-x.discord.gg/"
    });
    if !folders.is_null() {
        d["user_settings"] = json!({"guild_folders": folders});
    }
    d
}

fn state(d: Value) -> State {
    let mut s = State::default();
    assert!(s.apply("READY", &d));
    s
}

fn plain(id: u64) -> SidebarEntry {
    SidebarEntry::Guild(sg(id))
}

/// Every channel in these READY fixtures has no `last_message_id`, so nothing is unread.
fn sg(id: u64) -> SidebarGuild {
    SidebarGuild {
        id,
        name: format!("g{id}"),
        icon_url: None,
        unread: false,
        mention_count: 0,
        muted: false,
    }
}

#[test]
fn guild_order_ungrouped_then_folders() {
    let guilds = [10, 20, 30, 40, 50].map(|i| guild(i, vec![])).to_vec();
    let folders = json!([
        {"id": null, "guild_ids": ["10"]},
        {"id": 7, "name": "f", "color": 16711680, "guild_ids": ["20", "999", "30"]},
        {"id": 8, "guild_ids": ["777"]}
    ]);
    let mut s = state(ready(guilds, folders, json!([]), vec![]));
    s.apply("GUILD_CREATE", &guild(60, vec![]));
    s.apply("GUILD_CREATE", &guild(61, vec![]));
    assert_eq!(
        guild_entries(&s),
        vec![
            // Ungrouped: post-READY newest first, then READY order.
            plain(61),
            plain(60),
            plain(40),
            plain(50),
            plain(10),
            SidebarEntry::Folder {
                id: Some(7),
                name: Some("f".into()),
                color: Some(0xff0000),
                guilds: vec![sg(20), sg(30)],
            },
        ]
    );
}

#[test]
fn folder_with_two_known_guilds_is_a_folder_with_its_color_and_null_color_is_none() {
    let guilds = vec![guild(10, vec![]), guild(20, vec![])];
    let folders = json!([{"id": 7, "name": "f", "color": null, "guild_ids": ["10", "20"]}]);
    let s = state(ready(guilds.clone(), folders, json!([]), vec![]));
    assert_eq!(
        guild_entries(&s),
        vec![SidebarEntry::Folder {
            id: Some(7),
            name: Some("f".into()),
            color: None,
            guilds: vec![sg(10), sg(20)],
        }]
    );
    let folders = json!([{"id": 7, "guild_ids": ["10", "20"]}]);
    let s = state(ready(guilds, folders, json!([]), vec![]));
    assert!(matches!(
        &guild_entries(&s)[0],
        SidebarEntry::Folder { color: None, .. }
    ));
}

#[test]
fn ready_parses_folder_color() {
    let folders = json!([
        {"id": 1, "color": 5793266, "guild_ids": ["10"]},
        {"id": 2, "color": null, "guild_ids": ["20"]},
        {"id": 3, "guild_ids": ["30"]}
    ]);
    let s = state(ready(vec![], folders, json!([]), vec![]));
    let colors: Vec<_> = s.guild_folders.unwrap().iter().map(|f| f.color).collect();
    assert_eq!(colors, vec![Some(5_793_266), None, None]);
}

#[test]
fn null_id_container_is_a_plain_guild_and_single_known_guild_folder_too() {
    let guilds = vec![guild(10, vec![]), guild(20, vec![]), guild(30, vec![])];
    let folders = json!([
        {"id": null, "guild_ids": ["30"]},
        {"id": 7, "name": "solo", "guild_ids": ["999", "10"]},
        {"id": null, "guild_ids": ["20"]}
    ]);
    let s = state(ready(guilds, folders, json!([]), vec![]));
    assert_eq!(guild_entries(&s), vec![plain(30), plain(10), plain(20)]);
}

#[test]
fn folder_whose_guilds_are_all_unknown_is_dropped() {
    let folders = json!([{"id": 7, "name": "ghost", "guild_ids": ["555", "556"]}]);
    let s = state(ready(vec![guild(10, vec![])], folders, json!([]), vec![]));
    assert_eq!(guild_entries(&s), vec![plain(10)]);
}

#[test]
fn guild_folders_none_gives_ready_order() {
    let guilds = vec![guild(30, vec![]), guild(10, vec![]), guild(20, vec![])];
    let s = state(ready(guilds, Value::Null, json!([]), vec![]));
    assert!(s.guild_folders.is_none());
    assert_eq!(guild_entries(&s), vec![plain(30), plain(10), plain(20)]);
}

#[test]
fn guild_icon_urls_and_unknown_name() {
    let mut a = guild(10, vec![]);
    a["icon"] = json!("abc123");
    let mut b = guild(20, vec![]);
    b["icon"] = json!("a_animated");
    let stub = json!({"id": "30", "unavailable": true});
    let s = state(ready(
        vec![a, b, guild(40, vec![]), stub],
        Value::Null,
        json!([]),
        vec![],
    ));
    let urls: Vec<_> = guild_entries(&s)
        .into_iter()
        .map(|e| match e {
            SidebarEntry::Guild(g) => (g.name, g.icon_url),
            SidebarEntry::Folder { .. } => unreachable!(),
        })
        .collect();
    assert_eq!(
        urls,
        vec![
            (
                "g10".into(),
                Some("https://cdn.discordapp.com/icons/10/abc123.png?size=64".into())
            ),
            (
                "g20".into(),
                Some("https://cdn.discordapp.com/icons/20/a_animated.png?size=64".into())
            ),
            ("g40".into(), None),
            ("Unknown server".into(), None),
        ]
    );
}

pub(crate) fn ch(id: u64, kind: u8, name: &str, position: i64, parent: Option<u64>) -> Value {
    let mut c = json!({"id": id.to_string(), "type": kind, "name": name, "position": position,
        "permission_overwrites": []});
    if let Some(p) = parent {
        c["parent_id"] = json!(p.to_string());
    }
    c
}

pub(crate) fn hidden(mut c: Value, guild_id: u64) -> Value {
    c["permission_overwrites"] =
        json!([{"id": guild_id.to_string(), "type": 0, "allow": "0", "deny": VIEW.to_string()}]);
    c
}

fn listed(id: u64, kind: u8, name: &str) -> SidebarChannel {
    SidebarChannel {
        id,
        kind,
        name: name.into(),
        unread: false,
        mention_count: 0,
        muted: false,
        private: false,
        occupants: vec![],
        threads: vec![],
    }
}

#[test]
fn channel_order_filter_and_headers() {
    const G: u64 = 100;
    let mut undecidable = ch(90, 0, "undecidable", 0, None);
    undecidable
        .as_object_mut()
        .unwrap()
        .remove("permission_overwrites");
    let channels = vec![
        // Categories out of order (position 5, then 1). Equal positions tie by id: tie-b (5) is
        // listed before tie-a (4) so a stable sort without the id key would keep them reversed.
        ch(2, 4, "later", 5, None),
        ch(1, 4, "first", 1, None),
        ch(3, 4, "empty-after-hiding", 2, None),
        ch(5, 4, "tie-b", 7, None),
        ch(4, 4, "tie-a", 7, None),
        // Root block: uncategorized, and one whose parent is not a known category.
        ch(10, 2, "root-voice", 0, None),
        ch(11, 0, "root-text", 3, None),
        ch(12, 5, "orphan-announce", 1, Some(4242)),
        // Category 1: voice at position 0 precedes text at position 9 in raw order only.
        ch(20, 2, "voice-first-by-position", 0, Some(1)),
        ch(21, 0, "text-late", 9, Some(1)),
        // media (24) before forum (22), both at position 4: only the id tiebreak orders them.
        ch(24, 16, "media", 4, Some(1)),
        ch(22, 15, "forum", 4, Some(1)),
        ch(23, 13, "stage", 1, Some(1)),
        // Excluded types: threads and a directory.
        ch(25, 11, "thread", 0, Some(1)),
        ch(26, 10, "thread", 0, Some(1)),
        ch(27, 14, "directory", 0, Some(1)),
        // Category 2 holds a text channel; the tie categories get one each so they are listed.
        ch(30, 0, "in-later", 0, Some(2)),
        ch(41, 0, "in-tie-b", 0, Some(5)),
        ch(40, 0, "in-tie-a", 0, Some(4)),
        // Category 3's only child is hidden: header absent.
        hidden(ch(31, 0, "secret", 0, Some(3)), G),
        // Hidden and undecidable root channels never show.
        hidden(ch(32, 0, "secret-root", 0, None), G),
        undecidable,
    ];
    let s = state(ready(
        vec![guild(G, channels)],
        Value::Null,
        json!([]),
        vec![],
    ));
    assert_eq!(
        channel_categories(&s, G),
        vec![
            SidebarCategory {
                id: None,
                name: None,
                channels: vec![
                    listed(12, 5, "orphan-announce"),
                    listed(11, 0, "root-text"),
                    listed(10, 2, "root-voice"),
                ],
            },
            SidebarCategory {
                id: Some(1),
                name: Some("first".into()),
                channels: vec![
                    listed(22, 15, "forum"),
                    listed(24, 16, "media"),
                    listed(21, 0, "text-late"),
                    listed(20, 2, "voice-first-by-position"),
                    listed(23, 13, "stage"),
                ],
            },
            SidebarCategory {
                id: Some(2),
                name: Some("later".into()),
                channels: vec![listed(30, 0, "in-later")],
            },
            SidebarCategory {
                id: Some(4),
                name: Some("tie-a".into()),
                channels: vec![listed(40, 0, "in-tie-a")],
            },
            SidebarCategory {
                id: Some(5),
                name: Some("tie-b".into()),
                channels: vec![listed(41, 0, "in-tie-b")],
            },
        ]
    );
    assert!(channel_categories(&s, 12345).is_empty());
}

#[test]
fn a_category_the_user_cannot_see_still_heads_its_visible_children() {
    const G: u64 = 100;
    let channels = vec![
        hidden(ch(1, 4, "private-cat", 0, None), G),
        ch(2, 0, "visible-child", 0, Some(1)),
    ];
    let s = state(ready(
        vec![guild(G, channels)],
        Value::Null,
        json!([]),
        vec![],
    ));
    let cats = channel_categories(&s, G);
    assert_eq!(cats.len(), 1);
    assert_eq!(cats[0].id, Some(1));
}

#[test]
fn dm_order_names_and_icons() {
    let users = vec![
        // Animated hash: still the static .png in M1.
        user(2, "alice", "0", json!("a_avh")),
        // Second-generation default: (id >> 22) % 6 = 5 (a % 5 would give 0).
        user(4_194_304 * 5, "bob", "0", Value::Null),
        // Legacy discriminator: 1235 % 5 = 0 (a % 6 would give 5).
        user(5, "carol", "1235", Value::Null),
    ];
    let mut users_named = users.clone();
    users_named[0]["global_name"] = json!("Alice G");
    let dms = json!([
        {"id": "500", "type": 1, "recipient_ids": ["2"], "last_message_id": "9"},
        {"id": "501", "type": 3, "name": "club", "icon": "icon9", "recipient_ids": ["2", "20971520"], "last_message_id": "30"},
        {"id": "502", "type": 3, "recipient_ids": ["2", "20971520"]},
        {"id": "503", "type": 1, "recipient_ids": ["20971520"], "last_message_id": "9"},
        {"id": "504", "type": 1, "recipient_ids": ["5"], "last_message_id": "9"},
        {"id": "505", "type": 3, "recipient_ids": ["77"]}
    ]);
    let s = state(ready(vec![], Value::Null, dms, users_named));
    let cdn = "https://cdn.discordapp.com";
    let dm = |id, name: &str, icon: Option<String>| SidebarDm {
        id,
        name: name.into(),
        icon_url: icon,
        // These DMs have a last_message_id (or none) and no read state: unread; 1:1 DMs offline.
        unread: ![502, 505].contains(&id),
        mention_count: 0,
        status: [500, 503, 504].contains(&id).then_some(crate::PresenceStatus::Offline),
    };
    assert_eq!(
        dm_list(&s),
        vec![
            // 30 first; then last_message 9 ties by id descending (504, 503, 500); none last.
            dm(
                501,
                "club",
                Some(format!("{cdn}/channel-icons/501/icon9.png?size=64"))
            ),
            dm(504, "carol", Some(format!("{cdn}/embed/avatars/0.png"))),
            dm(503, "bob", Some(format!("{cdn}/embed/avatars/5.png"))),
            dm(
                500,
                "Alice G",
                Some(format!("{cdn}/avatars/2/a_avh.png?size=64"))
            ),
            dm(505, "Group DM", None),
            dm(502, "Alice G, bob", None),
        ]
    );
}
