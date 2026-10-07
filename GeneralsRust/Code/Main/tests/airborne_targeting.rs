//! Fresh-template signed height semantics and the live public Main tick route.
//! Expected rows execute extracted original default/parse/getter/AI statements.
//! This does not establish full original game, override-loader or save provenance parity.

use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_locomotor::{
    LocomotorTemplate as CommonTemplate, get_locomotor_store, get_locomotor_store_mut,
    load_locomotors_from_str,
};
use gamelogic::locomotor::ini_bridge::{convert_named, from_common_ini_template};
use generals_main::game_logic::locomotor_bootstrap::{
    apply_host_locomotor_binding, resolve_host_locomotor_binding,
};
use generals_main::game_logic::object::ObjectType;
use generals_main::game_logic::{
    GameLogic, KindOf, LocomotorAppearance, Object, ObjectId, Team, ThingTemplate,
};
use generals_main::gameworld_shadow::{
    gameworld_movement_authority_live, shadow_coupled_tick_active,
};
use glam::Vec3;

const ORIGINAL: &str = include_str!("fixtures/airborne_targeting_original.txt");
const DT: f32 = 1.0 / 30.0;

fn bits(s: &str) -> u32 {
    u32::from_str_radix(s, 16).unwrap()
}
fn rows(kind: &str) -> Vec<Vec<&str>> {
    ORIGINAL
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>())
        .filter(|r| r.first() == Some(&kind))
        .collect()
}
fn load(prefix: &str, index: usize, token: &str, bulk: bool) -> CommonTemplate {
    let name = format!("Airborne{prefix}_{index}_{bulk}");
    let field = if token == "omitted" {
        String::new()
    } else {
        format!("AirborneTargetingHeight = {token}\n")
    };
    let text = format!(
        "Locomotor {name}\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 30\nAppearance = OTHER\nAllowAirborneMotiveForce = Yes\nZAxisBehavior = NO_Z_MOTIVE_FORCE\n{field}End\n"
    );
    if bulk {
        assert_eq!(load_locomotors_from_str(&text).unwrap(), 1);
    } else {
        INI::new()
            .with_inline_source(&text, |ini| ini.parse_current_file())
            .unwrap();
    }
    get_locomotor_store().find_template(&name).unwrap().clone()
}
fn object() -> Object {
    Object::new(
        ThingTemplate::new("AirborneFixture"),
        ObjectId(901),
        Team::USA,
    )
}
fn stamp(object: &mut Object, row: &[&str]) -> bool {
    object.ground_height = 0.0;
    object.set_position(Vec3::new(0.0, f32::from_bits(bits(row[4])), 0.0));
    object.status.airborne_target = row[5] == "1";
    object.stamp_airborne_target_from_locomotor();
    object.status.airborne_target
}
fn check_loaders(prefix: &str, read: impl Fn(&CommonTemplate) -> i32) {
    let mut errors = Vec::new();
    let mut count = 0;
    for (index, row) in rows("load").iter().enumerate() {
        for bulk in [false, true] {
            let template = load(prefix, index, row[1], bulk);
            let actual = read(&template);
            let expected: i32 = row[2].parse().unwrap();
            count += 1;
            if actual != expected {
                errors.push(format!("{} bulk={bulk}: {actual} != {expected}", row[1]));
            }
        }
    }
    assert_eq!(count, 30);
    println!(
        "{prefix}: {count} scalar comparisons, {} differences",
        errors.len()
    );
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn both_public_loaders_keep_signed_integer_and_fresh_default() {
    check_loaders("Common", |t| t.airborne_targeting_height);
}
#[test]
fn core_direct_bridge_keeps_literal_signed_integer() {
    check_loaders("Bridge", |t| {
        from_common_ini_template(t).airborne_targeting_height
    });
}
#[test]
fn core_named_bridge_keeps_literal_signed_integer() {
    check_loaders("NamedBridge", |t| {
        convert_named(t.name.as_str())
            .unwrap()
            .airborne_targeting_height
    });
}
#[test]
fn main_resolver_and_apply_keep_literal_signed_integer() {
    check_loaders("Main", |t| {
        let binding = resolve_host_locomotor_binding(t.name.as_str()).unwrap();
        let mut host = object();
        apply_host_locomotor_binding(&mut host, &binding);
        assert_eq!(
            host.airborne_targeting_height,
            binding.airborne_targeting_height
        );
        host.airborne_targeting_height
    });
}

#[test]
fn original_fixture_has_strict_equality_rounding_and_both_initial_flags() {
    assert_eq!(rows("load").len(), 15);
    let flags = rows("flag");
    assert_eq!(flags.len(), 142);
    for row in rows("load") {
        let threshold = bits(row[3]);
        for initial in ["0", "1"] {
            let equal = flags
                .iter()
                .find(|r| r[1] == row[1] && bits(r[4]) == threshold && r[5] == initial)
                .unwrap();
            assert_eq!(equal[6], "0", "strict greater-than at equality");
        }
        assert_eq!((row[2].parse::<i32>().unwrap() as f32).to_bits(), threshold);
    }
    let rounded = rows("load")
        .into_iter()
        .find(|r| r[1] == "16777217")
        .unwrap();
    assert_eq!(bits(rounded[3]), 16777216.0_f32.to_bits());
    let max = rows("load")
        .into_iter()
        .find(|r| r[1] == "2147483647")
        .unwrap();
    assert_eq!(bits(max[3]), 2147483648.0_f32.to_bits());
    assert!(
        flags.iter().any(|r| r[1] == "omitted" && r[6] == "1"),
        "MAX is a threshold, not unconditional never-airborne"
    );
}

#[test]
fn bound_main_stamp_matches_original_predicate_for_both_loaders() {
    let mut errors = Vec::new();
    let mut count = 0;
    for (index, row) in rows("flag").iter().enumerate() {
        for bulk in [false, true] {
            let template = load("Bound", index, row[1], bulk);
            let binding = resolve_host_locomotor_binding(template.name.as_str()).unwrap();
            let mut host = object();
            apply_host_locomotor_binding(&mut host, &binding);
            let actual = stamp(&mut host, row);
            count += 1;
            if actual != (row[6] == "1") {
                errors.push(format!("{row:?} bulk={bulk}: {actual}"));
            }
        }
    }
    assert_eq!(count, 284);
    println!(
        "bound predicate: {count} comparisons, {} differences",
        errors.len()
    );
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn standalone_object_new_resolves_named_template_without_admission() {
    let mut errors = Vec::new();
    for (index, row) in rows("flag").iter().enumerate() {
        let template = load("Standalone", index, row[1], true);
        let mut thing = ThingTemplate::new("StandaloneAirborne");
        thing.set_locomotor_name(template.name.as_str());
        let mut host = Object::new(thing, ObjectId(902), Team::USA);
        assert_eq!(host.airborne_targeting_height, i32::MAX);
        assert_eq!(
            host.cur_locomotor_name.as_deref(),
            Some(template.name.as_str())
        );
        if stamp(&mut host, row) != (row[6] == "1") {
            errors.push(format!("{row:?}"));
        }
    }
    println!(
        "standalone predicate: 142 comparisons, {} differences",
        errors.len()
    );
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn literal_zero_honored_without_name_missing_name_and_conflicting_name() {
    let template = load("ConflictingName", 0, "30", true);
    let mut errors = Vec::new();
    for name in [
        None,
        Some("AirborneAbsentName"),
        Some(template.name.as_str()),
    ] {
        for initial in [false, true] {
            let mut host = object();
            host.airborne_targeting_height = 0;
            host.cur_locomotor_name = name.map(str::to_owned);
            host.set_position(Vec3::Y);
            host.status.airborne_target = initial;
            host.stamp_airborne_target_from_locomotor();
            if !host.status.airborne_target {
                errors.push(format!("{name:?} initial={initial}"));
            }
        }
    }
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn literal_scalars_without_name_match_all_original_predicate_rows() {
    let mut errors = Vec::new();
    for row in rows("flag") {
        let mut host = object();
        host.airborne_targeting_height = row[2].parse().unwrap();
        assert!(host.cur_locomotor_name.is_none());
        if stamp(&mut host, &row) != (row[6] == "1") {
            errors.push(format!("{row:?}"));
        }
    }
    println!(
        "literal predicate: 142 comparisons, {} differences",
        errors.len()
    );
    assert!(errors.is_empty(), "{errors:?}");
}

#[test]
fn constructor_missing_serde_and_literal_clone_roundtrip_preserve_scalars() {
    assert_eq!(object().airborne_targeting_height, i32::MAX);
    assert_eq!(
        Object::new_simple(ObjectId(903), ObjectType::Vehicle, "AirborneSimple".into())
            .airborne_targeting_height,
        i32::MAX
    );
    let mut missing = serde_json::to_value(object()).unwrap();
    missing
        .as_object_mut()
        .unwrap()
        .remove("airborne_targeting_height");
    let restored: Object = serde_json::from_value(missing).unwrap();
    assert_eq!(restored.airborne_targeting_height, i32::MAX);
    for value in [0, -1, 30, i32::MIN, i32::MAX] {
        let mut saved = serde_json::to_value(object()).unwrap();
        saved["airborne_targeting_height"] = serde_json::json!(value);
        let restored: Object = serde_json::from_value(saved).unwrap();
        assert_eq!(restored.airborne_targeting_height, value);
        assert_eq!(restored.clone().airborne_targeting_height, value);
        let serialized = serde_json::to_string(&restored).unwrap();
        let roundtrip: Object = serde_json::from_str(&serialized).unwrap();
        assert_eq!(roundtrip.airborne_targeting_height, value);
    }
}

#[test]
fn max_named_fallback_ambiguity_is_preserved_not_redefined() {
    // MAX still conflates an unbound default, bound omission, and authored MAX.
    // A later store edit can affect all three. This controls existing behavior;
    // it does not claim original override-loader or authoritative save parity.
    for (index, token) in ["omitted", "2147483647"].into_iter().enumerate() {
        let template = load("MaxAmbiguity", index, token, true);
        let binding = resolve_host_locomotor_binding(template.name.as_str()).unwrap();
        let mut bound_thing = ThingTemplate::new("BoundMax");
        bound_thing.set_locomotor_name(template.name.as_str());
        let mut bound = Object::new(bound_thing, ObjectId(905), Team::USA);
        apply_host_locomotor_binding(&mut bound, &binding);
        assert_eq!(bound.airborne_targeting_height, i32::MAX);
        let mut thing = ThingTemplate::new("UnboundMax");
        thing.set_locomotor_name(template.name.as_str());
        let mut unbound = Object::new(thing, ObjectId(904), Team::USA);
        get_locomotor_store_mut()
            .find_template_mut(template.name.as_str())
            .unwrap()
            .airborne_targeting_height = -1;
        for host in [&mut bound, &mut unbound] {
            host.status.airborne_target = false;
            host.stamp_airborne_target_from_locomotor();
            assert!(host.status.airborne_target);
            assert_eq!(host.airborne_targeting_height, i32::MAX);
        }
    }
}

fn live_case(token: &str, height: f32, initial: bool) {
    assert!(!shadow_coupled_tick_active());
    assert!(!gameworld_movement_authority_live());
    let mut game = GameLogic::new();
    assert_eq!(game.get_frame(), 0);
    assert!(game.host_objects().is_empty());
    let warm = game.update_with_dt(DT);
    assert_eq!((warm.frame, warm.steps_run), (1, 1));
    let template = load(&format!("Live{token}"), usize::from(initial), token, true);
    let unit_name = format!("AirborneLiveUnit{token}_{initial}");
    let mut thing = ThingTemplate::new(&unit_name);
    thing
        .set_health(100.0)
        .add_kind_of(KindOf::Worker)
        .set_locomotor_name(template.name.as_str());
    game.templates.insert(unit_name.clone(), thing);
    let id = game
        .create_object(&unit_name, Team::USA, Vec3::new(0.0, height, 0.0))
        .unwrap();
    let expected_scalar: i32 = rows("load").into_iter().find(|r| r[1] == token).unwrap()[2]
        .parse()
        .unwrap();
    let admitted_scalar;
    {
        let host = game.host_object_mut(id).unwrap();
        admitted_scalar = host.airborne_targeting_height;
        assert_eq!(host.loco_appearance, LocomotorAppearance::Other);
        assert!(host.is_mobile() && host.can_move() && host.is_alive());
        assert!(!host.worker_ai_update);
        assert!(!host.stick_to_ground);
        host.status.airborne_target = initial;
        host.move_to(Vec3::new(1000.0, height, 0.0));
        assert!(host.movement.target_position.is_some());
    }
    let tick = game.update_with_dt(DT);
    assert_eq!((tick.frame, tick.steps_run), (2, 1));
    assert!(!tick.budget_hit);
    assert_eq!(tick.accumulated_time_seconds, 0.0);
    assert!(!shadow_coupled_tick_active());
    assert!(!gameworld_movement_authority_live());
    let host = game.host_object(id).unwrap();
    let actual_height = host.get_position().y - host.ground_height;
    println!(
        "live token={token} initial={initial} frame={} admitted={admitted_scalar} final_scalar={} height_bits={:08x} final_flag={}",
        tick.frame,
        host.airborne_targeting_height,
        actual_height.to_bits(),
        host.status.airborne_target
    );
    assert!(host.is_alive() && host.is_motive());
    assert!(host.status.moving && host.movement.target_position.is_some());
    assert!(
        host.get_position().x > 0.0,
        "ordinary movement really advanced"
    );
    if height > 0.0 {
        assert!(actual_height > 0.0, "positive-height witness must not land");
    } else {
        assert_eq!(actual_height, 0.0);
    }
    // Match actual sampled height bits to original execution, not initial pose.
    let flags = rows("flag");
    let original = flags
        .iter()
        .find(|r| {
            r[1] == token && bits(r[4]) == actual_height.to_bits() && (r[5] == "1") == initial
        })
        .expect("actual live sample must be present in frozen original fixture");
    assert_eq!(
        host.airborne_targeting_height, admitted_scalar,
        "no post-admission threshold repair"
    );
    assert_eq!(
        (admitted_scalar, host.status.airborne_target),
        (expected_scalar, original[6] == "1")
    );
}

#[test]
fn public_tick_explicit_zero_sets_target_at_positive_height() {
    live_case("0", 1.0, false);
}
#[test]
fn public_tick_omitted_clears_target_at_positive_height() {
    live_case("omitted", 1.0, true);
}
#[test]
fn public_tick_negative_sets_target_at_ground_height() {
    live_case("-1", 0.0, false);
}
