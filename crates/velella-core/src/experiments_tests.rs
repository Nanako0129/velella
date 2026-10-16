//! Slice X: the evaluation against experiments.md's hand-checkable vectors (section 6), payload
//! parsing, and Unknown for every input that is missing or unreadable. The hashes below were
//! computed with an independent Python implementation of MurmurHash3 x86 32 (seed 0).
use crate::experiments::{murmur3, BookmarkLimits, Gate, MenuGate};
use crate::state::State;
use serde_json::{json, Value};

/// Experiments.md 6b's guilds A (out of range) and B (in range), and a guild only an override names.
const A: u64 = 81_384_788_765_712_384;
const B: u64 = 613_425_648_685_547_541;
const OVERRIDDEN: u64 = 111_111_111_111_111_111;
const OFFICIAL: &str = "2026-03_guild_official_messages";
const TIDA: &str = "2025-11_tida_webform";
const V1: &str = "2026-03-message-bookmarks";
const V2: &str = "2026-07-message-bookmarks-v2";
const OBFUSCATION: &str = "2026-02-private-channel-hiding";
const CONVERSATIONS: &str = "2026-03-conversation-highlighting-utility";
const NOW: u64 = 1_790_000_000_000;
const DAY: u64 = 86_400_000;

fn guild(id: u64, features: &[&str]) -> Value {
    json!({"id": id.to_string(), "properties": {"features": features}})
}

fn ready(extra: Value) -> State {
    let mut d = json!({
        "user": {"id": "1", "username": "u", "flags": 0},
        "guilds": [], "private_channels": []
    });
    for (k, v) in extra.as_object().unwrap() {
        d[k] = v.clone();
    }
    let mut s = State::default();
    assert!(s.apply("READY", &d));
    s
}

fn apex(tuples: Value) -> Value {
    json!({"assignments": {"1": {"1": {"evaluation_id": "e", "assignments": tuples}}}})
}

fn apex_state(tuples: Value) -> State {
    ready(json!({"apex_experiments": apex(tuples)}))
}

/// The synthetic `2026-03_guild_official_messages` tuple of experiments.md 6b.
fn official(pops: Value, overrides: Value, hash_key: Value, holdout: (Value, Value)) -> Value {
    json!([
        3_747_270_261u64,
        hash_key,
        1,
        pops,
        overrides,
        [],
        holdout.0,
        holdout.1,
        0,
        0
    ])
}

/// A population: bucket ranges, then the filters that must all pass.
fn pop(ranges: Value, filters: Value) -> Value {
    json!([ranges, filters])
}

/// `[0, 5000)` is bucket 1 for guilds with the VERIFIED feature.
fn verified_pops() -> Value {
    json!([pop(
        json!([[1, [{"s": 0, "e": 5000}]]]),
        json!([[1_604_612_045u64, [[1_183_251_248u64, ["VERIFIED"]]]]])
    )])
}

fn synthetic(guilds: Vec<Value>) -> State {
    let tuple = official(
        verified_pops(),
        json!([{"b": 1, "k": [OVERRIDDEN.to_string()]}]),
        Value::Null,
        (Value::Null, Value::Null),
    );
    ready(json!({"guild_experiments": [tuple], "guilds": guilds}))
}

fn bucket(s: &State, g: Option<u64>) -> Option<i64> {
    s.bucket_at(OFFICIAL, g, NOW)
}

#[test]
fn murmur3_matches_the_reference_values() {
    let h = |s: &str| murmur3(s.as_bytes());
    assert_eq!(h(""), 0);
    assert_eq!(h("hello"), 613_153_351);
    assert_eq!(
        h("The quick brown fox jumps over the lazy dog"),
        776_992_547
    );
    assert_eq!(h(OFFICIAL), 3_747_270_261);
    assert_eq!(h(TIDA), 4_055_210_228);
    assert_eq!(h(V1), 3_051_456_417);
    assert_eq!(h(V2), 367_271_379);
    assert_eq!(h(OBFUSCATION), 764_569_958);
    assert_eq!(h(CONVERSATIONS), 485_610_171);
    // The filter kinds and keys of experiments.md 2b.
    for (name, value) in [
        ("guild_ids", 3_013_771_838),
        ("guild_id_range", 2_404_720_969),
        ("guild_age_range_days", 3_730_341_874),
        ("guild_member_count_range", 2_918_402_255),
        ("guild_has_feature", 1_604_612_045),
        ("guild_hub_types", 4_148_745_523),
        ("guild_has_vanity_url", 188_952_590),
        ("guild_in_range_by_hash", 2_294_888_943),
        ("min_id", 3_399_957_344),
        ("max_id", 1_238_858_341),
        ("hash_key", 2_690_752_156),
        ("target", 1_982_804_121),
    ] {
        assert_eq!(h(name), value, "{name}");
    }
    // The 6b key strings and their `h mod 10000`.
    for (key, hash, x) in [
        (
            "2026-03_guild_official_messages:81384788765712384",
            437_655_232,
            5232,
        ),
        (
            "2026-03_guild_official_messages:613425648685547541",
            2_132_883_868,
            3868,
        ),
        ("abc:613425648685547541", 2_170_193_342, 3342),
        ("2026-03_guild_official_messages:", 3_418_241_754, 1754),
    ] {
        assert_eq!(h(key), hash, "{key}");
        assert_eq!(h(key) % 10_000, x, "{key}");
    }
}

#[test]
fn a_guild_in_range_out_of_range_filtered_out_and_overridden() {
    let s = synthetic(vec![guild(A, &["VERIFIED"]), guild(B, &["VERIFIED"])]);
    // A: 5232 is not in [0, 5000): CONTROL. B: 3868 is in range: bucket 1.
    assert_eq!(bucket(&s, Some(A)), Some(0));
    assert_eq!(bucket(&s, Some(B)), Some(1));
    assert_eq!(s.gate_at(OFFICIAL, Some(A), NOW), Gate::Disabled);
    assert_eq!(s.gate_at(OFFICIAL, Some(B), NOW), Gate::Enabled);
    // D: the override names the guild; nothing is hashed, so no guild data is needed either.
    assert_eq!(bucket(&s, Some(OVERRIDDEN)), Some(1));
    assert_eq!(s.gate_at(OFFICIAL, Some(OVERRIDDEN), NOW), Gate::Enabled);
    // C: the only population's filter fails (B without VERIFIED): not eligible.
    let s = synthetic(vec![guild(B, &[])]);
    assert_eq!(bucket(&s, Some(B)), Some(-1));
    assert_eq!(s.gate_at(OFFICIAL, Some(B), NOW), Gate::Disabled);
    // G: no guild: the feature filter needs one and fails.
    assert_eq!(bucket(&s, None), Some(-1));
    // A guild whose payload never arrived cannot be filtered: unknown, not off.
    assert_eq!(bucket(&s, Some(A)), None);
    assert_eq!(s.gate_at(OFFICIAL, Some(A), NOW), Gate::Unknown);
}

#[test]
fn a_holdout_excludes_only_when_the_held_experiment_lands_in_its_bucket() {
    let held = |bucket_range: i64| {
        json!([
            1_082_091_529u64,
            null,
            1,
            [pop(
                json!([[bucket_range, [{"s": 0, "e": 10000}]]]),
                json!([])
            )],
            [],
            [],
            null,
            null,
            0,
            0
        ])
    };
    let main = official(
        verified_pops(),
        json!([]),
        Value::Null,
        (json!("H"), json!(0)),
    );
    let with = |held: Option<Value>| {
        let mut exps = vec![main.clone()];
        exps.extend(held);
        ready(json!({"guild_experiments": exps, "guilds": [guild(B, &["VERIFIED"])]}))
    };
    // E: H puts B in bucket 0 == holdout_bucket 0: held out.
    assert_eq!(bucket(&with(Some(held(0))), Some(B)), Some(-1));
    // H puts B in bucket 1: not the excluding bucket; B keeps its bucket.
    assert_eq!(bucket(&with(Some(held(1))), Some(B)), Some(1));
    // H is not in the payload: no descriptor, nothing excludes.
    assert_eq!(bucket(&with(None), Some(B)), Some(1));
}

#[test]
fn the_hash_key_the_ranges_and_unknown_filters_follow_the_official_evaluation() {
    let one = |hash_key: Value, ranges: Value, filters: Value| {
        let t = official(
            json!([pop(ranges, filters)]),
            json!([]),
            hash_key,
            (Value::Null, Value::Null),
        );
        ready(json!({"guild_experiments": [t], "guilds": [guild(B, &["VERIFIED"])]}))
    };
    let narrow = json!([[1, [{"s": 0, "e": 3500}]]]);
    // B hashes to 3868 under the experiment name (outside [0, 3500): CONTROL) and to 3342 under
    // the hash key "abc" (inside: bucket 1).
    assert_eq!(
        bucket(&one(Value::Null, narrow.clone(), json!([])), Some(B)),
        Some(0)
    );
    assert_eq!(
        bucket(&one(json!("abc"), narrow, json!([])), Some(B)),
        Some(1)
    );
    // H: a range whose bucket is -1 is not eligible.
    let ineligible = json!([[-1, [{"s": 0, "e": 5000}]]]);
    assert_eq!(
        bucket(&one(Value::Null, ineligible, json!([])), Some(B)),
        Some(-1)
    );
    // I: a filter kind the client does not know passes.
    let unknown_kind = json!([[12345, [[1, "x"]]]]);
    let any = json!([[1, [{"s": 0, "e": 5000}]]]);
    assert_eq!(
        bucket(&one(Value::Null, any, unknown_kind), Some(B)),
        Some(1)
    );
}

/// A range holds `start <= x < end`; guild B's x is exactly 3868.
#[test]
fn a_range_includes_its_start_and_excludes_its_end() {
    let in_range = |start: i64, end: i64| {
        let t = official(
            json!([pop(json!([[1, [{"s": start, "e": end}]]]), json!([]))]),
            json!([]),
            Value::Null,
            (Value::Null, Value::Null),
        );
        let s = ready(json!({"guild_experiments": [t]}));
        // Outside every range of the population is CONTROL (0), inside is bucket 1.
        bucket(&s, Some(B)) == Some(1)
    };
    assert!(in_range(3868, 3869));
    assert!(in_range(3868, 5000));
    assert!(!in_range(3869, 5000), "below the start");
    assert!(!in_range(0, 3868), "at the end");
    assert!(in_range(0, 3869));
}

#[test]
fn the_hash_range_filter_doubles_the_hash() {
    let filter = json!([
        2_294_888_943u64,
        [[2_690_752_156u64, OFFICIAL], [1_982_804_121u64, "5000"]]
    ]);
    let t = official(
        json!([pop(json!([[1, [{"s": 0, "e": 10000}]]]), json!([filter]))]),
        json!([]),
        Value::Null,
        (Value::Null, Value::Null),
    );
    let s = ready(json!({"guild_experiments": [t]}));
    // A: 2h = 875310464, mod 10000 = 464 < 5000: passes. B: 2h mod 10000 = 7736: fails.
    assert_eq!(bucket(&s, Some(A)), Some(1));
    assert_eq!(bucket(&s, Some(B)), Some(-1));
}

/// One population with `filter` and the whole range as bucket 1: 1 when the filter passes, -1 when
/// it fails, `None` when an input is missing.
fn filtered(filter: Value, g: Value, id: u64, now: u64) -> Option<i64> {
    let t = official(
        json!([pop(json!([[1, [{"s": 0, "e": 10000}]]]), json!([filter]))]),
        json!([]),
        Value::Null,
        (Value::Null, Value::Null),
    );
    ready(json!({"guild_experiments": [t], "guilds": [g]})).bucket_at(OFFICIAL, Some(id), now)
}

#[test]
fn every_filter_kind_reads_its_input() {
    let g = json!({"id": B.to_string(), "member_count": 50,
        "properties": {"features": ["VERIFIED", "COMMUNITY"], "vanity_url_code": "cats", "hub_type": 2}});
    let f = |kind: u64, pairs: Value| filtered(json!([kind, pairs]), g.clone(), B, NOW);
    // guild_ids
    assert_eq!(
        f(3_013_771_838, json!([[3_013_771_838u64, [B.to_string()]]])),
        Some(1)
    );
    assert_eq!(
        f(3_013_771_838, json!([[3_013_771_838u64, [A.to_string()]]])),
        Some(-1)
    );
    // guild_id_range: min_id 3399957344, max_id 1238858341; a null bound is open
    let range = |min: Value, max: Value| json!([[3_399_957_344u64, min], [1_238_858_341u64, max]]);
    assert_eq!(
        f(
            2_404_720_969,
            range(json!("100000000000000000"), Value::Null)
        ),
        Some(1)
    );
    assert_eq!(
        f(
            2_404_720_969,
            range(json!("700000000000000000"), Value::Null)
        ),
        Some(-1)
    );
    assert_eq!(
        f(
            2_404_720_969,
            range(Value::Null, json!("500000000000000000"))
        ),
        Some(-1)
    );
    // guild_age_range_days: whole days since the snowflake's timestamp
    let created = (B >> 22) + 1_420_070_400_000;
    let age = |now: u64, min: i64, max: i64| {
        filtered(
            json!([3_730_341_874u64, range(json!(min), json!(max))]),
            g.clone(),
            B,
            now,
        )
    };
    assert_eq!(age(created + 10 * DAY, 10, 10), Some(1));
    assert_eq!(age(created + 10 * DAY + DAY - 1, 10, 10), Some(1));
    assert_eq!(age(created + 10 * DAY - 1, 10, 10), Some(-1));
    assert_eq!(age(created + 11 * DAY, 10, 10), Some(-1));
    // guild_member_count_range
    assert_eq!(f(2_918_402_255, range(json!(10), json!(100))), Some(1));
    assert_eq!(f(2_918_402_255, range(json!(60), Value::Null)), Some(-1));
    // guild_has_feature reads the first pair's value, whatever its key
    assert_eq!(
        f(1_604_612_045, json!([[1_183_251_248u64, ["COMMUNITY"]]])),
        Some(1)
    );
    assert_eq!(
        f(1_604_612_045, json!([[7, ["PARTNERED", "COMMUNITY"]]])),
        Some(1)
    );
    assert_eq!(
        f(1_604_612_045, json!([[1_183_251_248u64, ["PARTNERED"]]])),
        Some(-1)
    );
    // guild_hub_types, guild_has_vanity_url
    assert_eq!(f(4_148_745_523, json!([[1, [1, 2]]])), Some(1));
    assert_eq!(f(4_148_745_523, json!([[1, [3]]])), Some(-1));
    assert_eq!(f(188_952_590, json!([[1, true]])), Some(1));
    assert_eq!(f(188_952_590, json!([[1, false]])), Some(-1));
}

#[test]
fn a_filter_input_that_is_missing_is_unknown() {
    // No member count in the payload: the count filter cannot be decided.
    let g = json!({"id": B.to_string(), "properties": {"features": []}});
    let count = json!([
        2_918_402_255u64,
        [[3_399_957_344u64, 1], [1_238_858_341u64, 100]]
    ]);
    assert_eq!(filtered(count, g, B, NOW), None);
    // A range bound that is present but not a number: the experiment is unreadable.
    let g = json!({"id": B.to_string(), "member_count": 5, "properties": {"features": []}});
    let bad = json!([2_918_402_255u64, [[3_399_957_344u64, "many"]]]);
    assert_eq!(filtered(bad, g, B, NOW), None);
    // A definite failure elsewhere wins over an unknown: the population does not apply.
    let g = json!({"id": B.to_string(), "properties": {"features": []}});
    let t = official(
        json!([pop(
            json!([[1, [{"s": 0, "e": 10000}]]]),
            json!([
                [2_918_402_255u64, [[3_399_957_344u64, 1]]],
                [1_604_612_045u64, [[1, ["VERIFIED"]]]]
            ])
        )]),
        json!([]),
        Value::Null,
        (Value::Null, Value::Null),
    );
    let s = ready(json!({"guild_experiments": [t], "guilds": [g]}));
    assert_eq!(bucket(&s, Some(B)), Some(-1));
}

#[test]
fn nothing_loaded_is_unknown_and_so_is_an_unknown_name() {
    let s = State::default();
    for gate in [
        MenuGate::Translate,
        MenuGate::TidaWebform,
        MenuGate::BookmarksV1,
        MenuGate::BookmarksV2,
        MenuGate::BookmarksVisible,
        MenuGate::ChannelObfuscation,
        MenuGate::ViewConversation,
    ] {
        assert_eq!(s.menu_gate(gate, Some(B)), Gate::Unknown, "{gate:?}");
    }
    assert_eq!(
        s.menu_gate(MenuGate::ReportToModerators, Some(B)),
        Gate::Unknown
    );
    assert_eq!(
        s.menu_gate(MenuGate::OfficialMessage, Some(B)),
        Gate::Unknown
    );
    // A DM has no guild: these are definite.
    assert_eq!(
        s.menu_gate(MenuGate::ReportToModerators, None),
        Gate::Disabled
    );
    assert_eq!(s.menu_gate(MenuGate::OfficialMessage, None), Gate::Disabled);
    assert_eq!(
        s.menu_gate(MenuGate::ViewConversation, None),
        Gate::Disabled
    );
    assert_eq!(s.bookmark_v2_limits(), None);

    // READY without any experiment field.
    let s = ready(json!({}));
    assert_eq!(s.menu_gate(MenuGate::TidaWebform, None), Gate::Unknown);
    assert_eq!(
        s.menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Unknown
    );
    assert_eq!(s.menu_gate(MenuGate::Translate, None), Gate::Disabled);
    assert_eq!(s.gate_at(OFFICIAL, Some(B), NOW), Gate::Unknown);

    // A name this build has no definition for, even when the payload lists it.
    let s = ready(
        json!({"experiments": [[1_082_091_529u64, 1, 1, 0, 0, 0, 0]],
                          "apex_experiments": apex(json!([[1_082_091_529u64, 1, 0, 1, 1]]))}),
    );
    assert_eq!(s.experiment_gate("H", None), Gate::Unknown);
    assert_eq!(s.experiment_bucket("H", None), None);
    assert_eq!(s.experiment_gate("", None), Gate::Unknown);
}

#[test]
fn the_user_flags_decide_translate() {
    let mut s = ready(json!({"user": {"id": "1", "flags": 1}}));
    assert_eq!(s.menu_gate(MenuGate::Translate, None), Gate::Enabled);
    // Another bit alone is not staff.
    assert_eq!(
        ready(json!({"user": {"id": "1", "flags": 64}})).menu_gate(MenuGate::Translate, None),
        Gate::Disabled
    );
    // The flags field missing from READY is unknown, not "not staff".
    assert_eq!(
        ready(json!({"user": {"id": "1"}})).menu_gate(MenuGate::Translate, None),
        Gate::Unknown
    );
    // USER_UPDATE of this user changes it; another user's does not.
    assert!(s.apply("USER_UPDATE", &json!({"id": "1", "flags": 0})));
    assert_eq!(s.menu_gate(MenuGate::Translate, None), Gate::Disabled);
    assert!(!s.apply("USER_UPDATE", &json!({"id": "2", "flags": 1})));
    assert_eq!(s.menu_gate(MenuGate::Translate, None), Gate::Disabled);
}

#[test]
fn legacy_user_experiments_use_the_servers_bucket() {
    let tuple = |bucket: i64, aa: i64| {
        json!([
            4_055_210_228u64,
            2,
            bucket,
            1,
            0,
            1234,
            aa,
            0,
            null,
            null,
            null
        ])
    };
    let state = |list: Value| ready(json!({"experiments": list}));
    // 6c: in treatment, A/A mode, absent from a loaded list.
    assert_eq!(
        state(json!([tuple(1, 0)])).menu_gate(MenuGate::TidaWebform, None),
        Gate::Enabled
    );
    assert_eq!(
        state(json!([tuple(1, 1)])).menu_gate(MenuGate::TidaWebform, None),
        Gate::Disabled
    );
    assert_eq!(
        state(json!([tuple(0, 0)])).menu_gate(MenuGate::TidaWebform, None),
        Gate::Disabled
    );
    assert_eq!(
        state(json!([tuple(-1, 0)])).menu_gate(MenuGate::TidaWebform, None),
        Gate::Disabled
    );
    assert_eq!(
        state(json!([tuple(2, 0)])).menu_gate(MenuGate::TidaWebform, None),
        Gate::Disabled
    );
    let s = state(json!([]));
    assert_eq!(s.menu_gate(MenuGate::TidaWebform, None), Gate::Disabled);
    assert_eq!(s.experiment_bucket(TIDA, None), Some(-1));
    assert_eq!(
        state(json!([tuple(1, 0)])).experiment_bucket(TIDA, None),
        Some(1)
    );
    // Unreadable: the field is not a list, or the one tuple is malformed.
    assert_eq!(
        ready(json!({"experiments": "x"})).menu_gate(MenuGate::TidaWebform, None),
        Gate::Unknown
    );
    assert_eq!(
        state(json!([[4_055_210_228u64, 2, "one"]])).menu_gate(MenuGate::TidaWebform, None),
        Gate::Unknown
    );
    // Another experiment's malformed tuple does not matter.
    assert_eq!(
        state(json!([[7, 2, "one"], tuple(1, 0)])).menu_gate(MenuGate::TidaWebform, None),
        Gate::Enabled
    );
}

#[test]
fn apex_assignments_by_variant_flags_and_config() {
    let on =
        |name_hash: u64, variant: i64| apex_state(json!([[name_hash, variant, 0, 1, variant]]));
    // Bookmarks v1: variants 1 and 2 on.
    assert_eq!(
        on(3_051_456_417, 1).menu_gate(MenuGate::BookmarksV1, None),
        Gate::Enabled
    );
    assert_eq!(
        on(3_051_456_417, 2).menu_gate(MenuGate::BookmarksV1, None),
        Gate::Enabled
    );
    assert_eq!(
        on(3_051_456_417, 0).menu_gate(MenuGate::BookmarksV1, None),
        Gate::Disabled
    );
    assert_eq!(on(3_051_456_417, 1).experiment_bucket(V1, None), Some(1));
    // Channel obfuscation: 1 and 2 on, 3 off, no tuple off.
    assert_eq!(
        on(764_569_958, 1).menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Enabled
    );
    assert_eq!(
        on(764_569_958, 2).menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Enabled
    );
    assert_eq!(
        on(764_569_958, 3).menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Disabled
    );
    assert_eq!(
        apex_state(json!([])).menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Disabled
    );
    // A null variant is the default; so is an eligibility marker (flags 8).
    let null_variant = apex_state(json!([[764_569_958u64, null, 0, 1, null]]));
    assert_eq!(
        null_variant.menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Disabled
    );
    assert_eq!(null_variant.experiment_bucket(OBFUSCATION, None), Some(-1));
    let marker = apex_state(json!([[764_569_958u64, 1, 8, 1, 1]]));
    assert_eq!(
        marker.menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Disabled
    );
    // The apex object present but without this user's unit: unknown. A tuple that does not read:
    // unknown.
    let other_user = ready(json!({"apex_experiments":
        {"assignments": {"1": {"2": {"evaluation_id": "e", "assignments": []}}}}}));
    assert_eq!(
        other_user.menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Unknown
    );
    let broken = apex_state(json!([[764_569_958u64, "one", 0, 1, 1]]));
    assert_eq!(
        broken.menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Unknown
    );
}

#[test]
fn bookmarks_v2_carry_their_limits_and_v1_or_v2_makes_them_visible() {
    let v2 =
        |config: Value, flags: i64| apex_state(json!([[367_271_379u64, 1, flags, 5, 1, config]]));
    let s = v2(json!("{\"b\":100,\"r\":10}"), 2);
    assert_eq!(s.menu_gate(MenuGate::BookmarksV2, None), Gate::Enabled);
    assert_eq!(
        s.bookmark_v2_limits(),
        Some(BookmarkLimits {
            bookmarks: 100,
            reminders: 10
        })
    );
    // A config missing a member falls back to the default: off, no limits.
    let s = v2(json!("{\"b\":100}"), 2);
    assert_eq!(s.menu_gate(MenuGate::BookmarksV2, None), Gate::Disabled);
    assert_eq!(s.bookmark_v2_limits(), None);
    // Not JSON, not a string, null: the same.
    for bad in [json!("not json"), json!({"b": 1, "r": 1}), Value::Null] {
        assert_eq!(
            v2(bad, 2).menu_gate(MenuGate::BookmarksV2, None),
            Gate::Disabled
        );
    }
    // UseAsEligibility: the default.
    let s = v2(json!("{\"b\":100,\"r\":10}"), 8);
    assert_eq!(s.menu_gate(MenuGate::BookmarksV2, None), Gate::Disabled);
    assert_eq!(s.bookmark_v2_limits(), None);
    // message_utilities: v1 or v2.
    let both_off = apex_state(json!([]));
    assert_eq!(
        both_off.menu_gate(MenuGate::BookmarksVisible, None),
        Gate::Disabled
    );
    let v1_only = apex_state(json!([[3_051_456_417u64, 2, 0, 1, 2]]));
    assert_eq!(
        v1_only.menu_gate(MenuGate::BookmarksVisible, None),
        Gate::Enabled
    );
    let v2_only = v2(json!("{\"b\":1,\"r\":1}"), 2);
    assert_eq!(
        v2_only.menu_gate(MenuGate::BookmarksVisible, None),
        Gate::Enabled
    );
    // One off and the other unknown stays unknown.
    assert_eq!(
        State::default().menu_gate(MenuGate::BookmarksVisible, None),
        Gate::Unknown
    );
}

#[test]
fn view_conversation_needs_variant_two_and_the_guild_feature() {
    let with = |variant: i64, guilds: Value| {
        ready(
            json!({"apex_experiments": apex(json!([[485_610_171u64, variant, 0, 1, variant]])),
                     "guilds": guilds}),
        )
    };
    let feature = guild(B, &["CONVERSATIONS_EXTRACTION_PROCESSING"]);
    let gate = |s: &State| s.menu_gate(MenuGate::ViewConversation, Some(B));
    assert_eq!(gate(&with(2, json!([feature.clone()]))), Gate::Enabled);
    assert_eq!(gate(&with(1, json!([feature]))), Gate::Disabled);
    assert_eq!(gate(&with(2, json!([guild(B, &[])]))), Gate::Disabled);
    // The guild's payload never arrived: unknown.
    assert_eq!(gate(&with(2, json!([]))), Gate::Unknown);
}

#[test]
fn report_to_moderators_reads_the_selected_guilds_setting() {
    let reporting = |m: Value| {
        let mut g = guild(B, &[]);
        g["properties"]["moderator_reporting"] = m;
        ready(json!({"guilds": [g]}))
    };
    let gate = |s: &State| s.menu_gate(MenuGate::ReportToModerators, Some(B));
    let enabled = json!({"moderator_reporting_enabled": true, "moderator_report_channel_id": "9"});
    assert_eq!(gate(&reporting(enabled)), Gate::Enabled);
    let no_channel =
        json!({"moderator_reporting_enabled": true, "moderator_report_channel_id": null});
    assert_eq!(gate(&reporting(no_channel)), Gate::Disabled);
    let off = json!({"moderator_reporting_enabled": false, "moderator_report_channel_id": "9"});
    assert_eq!(gate(&reporting(off)), Gate::Disabled);
    assert_eq!(gate(&reporting(Value::Null)), Gate::Disabled);
    assert_eq!(gate(&ready(json!({}))), Gate::Unknown);
}

#[test]
fn the_official_message_gate_is_the_experiment_and_the_verified_feature() {
    let s = synthetic(vec![guild(B, &["VERIFIED"])]);
    assert_eq!(
        s.menu_gate(MenuGate::OfficialMessage, Some(B)),
        Gate::Enabled
    );
    // In range but not verified: off (the experiment's filter and the feature agree here, so use
    // an override to take the filter out).
    let s = synthetic(vec![guild(OVERRIDDEN, &[])]);
    assert_eq!(s.gate_at(OFFICIAL, Some(OVERRIDDEN), NOW), Gate::Enabled);
    assert_eq!(
        s.menu_gate(MenuGate::OfficialMessage, Some(OVERRIDDEN)),
        Gate::Disabled
    );
    assert_eq!(s.menu_gate(MenuGate::OfficialMessage, None), Gate::Disabled);
}

#[test]
fn a_guild_create_after_ready_feeds_the_guild_gates_and_units() {
    let mut s = synthetic(vec![]);
    assert_eq!(
        s.menu_gate(MenuGate::OfficialMessage, Some(B)),
        Gate::Unknown
    );
    assert!(s.apply("GUILD_CREATE", &guild(B, &["VERIFIED"])));
    assert_eq!(
        s.menu_gate(MenuGate::OfficialMessage, Some(B)),
        Gate::Enabled
    );
    // A partial GUILD_CREATE without properties keeps what was held.
    assert!(s.apply(
        "GUILD_CREATE",
        &json!({"id": B.to_string(), "data_mode": "partial", "properties": null})
    ));
    assert_eq!(
        s.menu_gate(MenuGate::OfficialMessage, Some(B)),
        Gate::Enabled
    );
}

#[test]
fn guild_units_follow_the_users_own_assignment() {
    let unit = |variant: i64| json!({"assignments": [[3_051_456_417u64, variant, 0, 1, variant]]});
    let mut g = guild(B, &[]);
    g["experiments"] = unit(2);
    let user = |flags: i64| apex(json!([[3_051_456_417u64, 1, flags, 1, 1]]));
    let state = |flags: i64| ready(json!({"apex_experiments": user(flags), "guilds": [g.clone()]}));
    // The user's tuple must be an eligibility marker (8); then the guild unit's variant decides.
    assert_eq!(state(8).guild_apex_variant(V1, B), Some(Some(2)));
    assert_eq!(state(0).guild_apex_variant(V1, B), Some(None));
    // A user override is used as is.
    assert_eq!(state(1).guild_apex_variant(V1, B), Some(Some(1)));
    // No unit for the guild: unknown. No user tuple at all: none.
    assert_eq!(state(8).guild_apex_variant(V1, A), None);
    let none = ready(json!({"apex_experiments": apex(json!([]))}));
    assert_eq!(none.guild_apex_variant(V1, B), Some(None));
    // GUILD_CREATE carries a unit too.
    let mut s = state(8);
    let mut created = guild(A, &[]);
    created["experiments"] = unit(1);
    s.apply("GUILD_CREATE", &created);
    assert_eq!(s.guild_apex_variant(V1, A), Some(Some(1)));
}

#[test]
fn a_state_update_overwrites_the_listed_unit_and_drops_session_overrides() {
    let mut s = apex_state(json!([[3_051_456_417u64, 0, 0, 1, 0]]));
    assert_eq!(s.menu_gate(MenuGate::BookmarksV1, None), Gate::Disabled);
    // STATE_UPDATE replaces the user's unit with the newest listing.
    assert!(s.apply(
        "STATE_UPDATE",
        &json!({"apex_experiments": apex(json!([[3_051_456_417u64, 1, 0, 2, 1]]))})
    ));
    assert_eq!(s.menu_gate(MenuGate::BookmarksV1, None), Gate::Enabled);
    // A session override wins over the server's assignment, and DELETE removes it.
    let override_ =
        |name: &str, variant: i64| json!({"experiment_name": name, "variant_id": variant});
    assert!(s.apply("EXPERIMENT_SESSION_OVERRIDE_CREATE", &override_(V1, 0)));
    assert_eq!(s.menu_gate(MenuGate::BookmarksV1, None), Gate::Disabled);
    assert!(s.apply(
        "EXPERIMENT_SESSION_OVERRIDE_DELETE",
        &json!({"experiment_name": V1})
    ));
    assert_eq!(s.menu_gate(MenuGate::BookmarksV1, None), Gate::Enabled);
    // An override works even before the unit is known.
    let mut empty = ready(json!({}));
    assert_eq!(empty.menu_gate(MenuGate::BookmarksV1, None), Gate::Unknown);
    assert!(empty.apply("EXPERIMENT_SESSION_OVERRIDE_CREATE", &override_(V1, 2)));
    assert_eq!(empty.menu_gate(MenuGate::BookmarksV1, None), Gate::Enabled);
    // A STATE_UPDATE after the override drops it (the update below lists the unit as off).
    assert!(empty.apply(
        "STATE_UPDATE",
        &json!({"apex_experiments": apex(json!([[3_051_456_417u64, 0, 0, 3, 0]]))})
    ));
    assert_eq!(empty.menu_gate(MenuGate::BookmarksV1, None), Gate::Disabled);
    // Unreadable override payloads change nothing.
    assert!(!empty.apply(
        "EXPERIMENT_SESSION_OVERRIDE_CREATE",
        &json!({"experiment_name": V1})
    ));
    assert!(!empty.apply(
        "EXPERIMENT_SESSION_OVERRIDE_DELETE",
        &json!({"experiment_name": V2})
    ));
}

#[test]
fn ready_replaces_everything_a_previous_one_loaded() {
    let mut s = apex_state(json!([[764_569_958u64, 1, 0, 1, 1]]));
    assert_eq!(
        s.menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Enabled
    );
    // The next READY carries no apex data: the old answer must not survive it.
    assert!(s.apply(
        "READY",
        &json!({"user": {"id": "1", "flags": 0}, "guilds": [], "private_channels": []})
    ));
    assert_eq!(
        s.menu_gate(MenuGate::ChannelObfuscation, None),
        Gate::Unknown
    );
}
