//! Existing public appearance-phase boundary; no world admission or ticks.
//! The native extracted-source fixture keeps raw and reconstructed frame results
//! separate. Only the explicit host-adapter column is compared with Main.

use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_locomotor::{get_locomotor_store, load_locomotors_from_str};
use generals_main::game_logic::locomotor_bootstrap::{
    apply_host_locomotor_binding, resolve_host_locomotor_binding,
};
use generals_main::game_logic::{
    LocomotorAppearance, Object, ObjectId, Team, ThingTemplate, calc_slow_down_dist,
};

const ORIGINAL: &str = include_str!("fixtures/approach_original.txt");

fn bits(text: &str) -> u32 {
    u32::from_str_radix(text, 16).unwrap()
}

fn scalar(text: &str) -> f32 {
    f32::from_bits(bits(text))
}

fn rows(family: &str) -> Vec<Vec<&str>> {
    ORIGINAL
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>())
        .filter(|row| row.first() == Some(&family))
        .collect()
}

fn appearance(value: &str) -> LocomotorAppearance {
    match value {
        "TREADS" => LocomotorAppearance::Treads,
        "FOUR_WHEELS" => LocomotorAppearance::WheelsFour,
        "MOTORCYCLE" => LocomotorAppearance::Motorcycle,
        "TWO_LEGS" => LocomotorAppearance::LegsTwo,
        "CLIMBER" => LocomotorAppearance::Climber,
        "OTHER" => LocomotorAppearance::Other,
        "HOVER" => LocomotorAppearance::Hover,
        "WINGS" => LocomotorAppearance::Wings,
        other => panic!("unsupported fixture appearance {other}"),
    }
}

fn check_float(actual: f32, expected: &str, field: &str, differences: &mut Vec<String>) {
    let expected_value = scalar(expected);
    if (expected_value.is_nan() && !actual.is_nan())
        || (!expected_value.is_nan() && actual.to_bits() != bits(expected))
    {
        differences.push(format!("{field}: {:08x} != {expected}", actual.to_bits()));
    }
}

fn check_family(family: &str, expected_count: usize, bulk: Option<bool>) {
    let cases = rows(family);
    assert_eq!(cases.len(), expected_count);
    let mut failures = Vec::new();
    for (index, row) in cases.iter().enumerate() {
        assert_eq!(row.len(), 38, "{}", row[1]);
        let mut object = Object::new(
            ThingTemplate::new("ApproachFixture"),
            ObjectId(901),
            Team::USA,
        );
        let mut differences = Vec::new();
        if let Some(bulk) = bulk {
            assert_eq!(row[3], "load");
            let name = format!("Approach{index}_{bulk}");
            let braking = if row[4] == "omitted" {
                String::new()
            } else {
                format!("Braking = {}\n", row[4])
            };
            let text = format!(
                "Locomotor {name}\nSurfaces = GROUND\nSpeed = 60\nAcceleration = 30\nAppearance = {}\nMinSpeed = {}\n{braking}End\n",
                row[2], row[5]
            );
            if bulk {
                assert_eq!(load_locomotors_from_str(&text).unwrap(), 1);
            } else {
                INI::new()
                    .with_inline_source(&text, |ini| ini.parse_current_file())
                    .unwrap();
            }
            let template = get_locomotor_store().find_template(&name).unwrap().clone();
            check_float(
                template.braking,
                row[16],
                "parsed frame braking",
                &mut differences,
            );
            check_float(
                template.min_speed,
                row[14],
                "parsed host minimum",
                &mut differences,
            );
            let binding = resolve_host_locomotor_binding(&name).unwrap();
            apply_host_locomotor_binding(&mut object, &binding);
            check_float(binding.braking, row[15], "bound braking", &mut differences);
        } else {
            assert_eq!(row[3], "direct");
            // Explicit phase input, not an admitted World object or field repair.
            object.loco_appearance = appearance(row[2]);
            object.braking = scalar(row[15]);
            object.min_speed = scalar(row[14]);
        }
        // Loaded scalars are inspected exactly and never rewritten after binding.
        assert_eq!(object.loco_appearance, appearance(row[2]));
        check_float(
            object.braking,
            row[15],
            "initial host braking",
            &mut differences,
        );
        check_float(
            object.min_speed,
            row[14],
            "initial host minimum",
            &mut differences,
        );
        // These are ordinary pre-call appearance state, inherited from prior phases.
        object.no_slow_down_as_approaching_dest = row[7] == "1";
        object.is_braking = row[8] == "1";
        object.braking_factor = scalar(row[9]);
        object.donut_timer = row[10].parse().unwrap();
        let result = object.apply_cpp_approach_brake(
            scalar(row[11]),
            scalar(row[12]),
            scalar(row[13]),
            row[6].parse().unwrap(),
        );
        check_float(result, row[37], "host goal", &mut differences);
        check_float(object.braking_factor, row[34], "factor", &mut differences);
        if object.is_braking != (row[33] == "1") {
            differences.push(format!("flag: {} != {}", object.is_braking, row[33]));
        }
        if object.donut_timer != row[35].parse::<u32>().unwrap() {
            differences.push(format!("deadline: {} != {}", object.donut_timer, row[35]));
        }
        if !differences.is_empty() {
            failures.push(format!("{}: {}", row[1], differences.join(", ")));
        }
    }
    assert!(
        failures.is_empty(),
        "{family} bulk={bulk:?}:\n{}",
        failures.join("\n")
    );
}

#[test]
fn treads_full_half_hold_and_factor() {
    check_family("treads", 12, None);
}

#[test]
fn wheels_and_motorcycle_frame_time_and_travel() {
    check_family("wheels", 8, None);
}

#[test]
fn strict_distance_and_factor_boundaries() {
    check_family("boundaries", 28, None);
}

#[test]
fn no_slow_down_only_blocks_new_latch() {
    check_family("no_slow", 12, None);
}

#[test]
fn signed_zero_negative_and_tiny_phase_inputs() {
    check_family("signed", 27, None);
}

#[test]
fn wheel_defined_refresh_equality_and_no_refresh_wrap() {
    check_family("timer", 11, None);
}

#[test]
fn generic_minimum_choices_preserve_inherited_state_and_wing_clear() {
    check_family("generic", 45, None);
}

#[test]
fn callback_loader_resolve_apply_to_public_approach() {
    check_family("binding", 43, Some(false));
}

#[test]
fn bulk_loader_resolve_apply_to_public_approach() {
    check_family("binding", 43, Some(true));
}

#[test]
fn shared_slowdown_preserves_original_division_and_early_return() {
    let controls = rows("helper");
    assert_eq!(controls.len(), 9);
    let mut failures = Vec::new();
    for row in controls {
        check_float(
            calc_slow_down_dist(scalar(row[2]), scalar(row[3]), scalar(row[4])),
            row[5],
            row[1],
            &mut failures,
        );
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn fixture_keeps_original_and_host_reconstructed_frames_distinct() {
    let loaded = rows("binding");
    assert!(
        loaded.iter().any(|r| r[20] != r[24]),
        "binding round-trip loss must stay visible"
    );
    assert!(
        loaded.iter().any(|r| r[19] != r[23]),
        "parsed minimum and host minimum differ"
    );
    let default = loaded
        .iter()
        .find(|r| r[1] == "default_braking_cap")
        .unwrap();
    assert_eq!(default[16], "47c34f80");
    assert_eq!(default[15], "4caba8e0");
    assert_ne!(default[20], default[24]);
    let boundary = loaded
        .iter()
        .find(|r| r[1] == "roundtrip_boundary")
        .unwrap();
    assert_ne!(boundary[20], boundary[24]);
    assert_ne!(
        boundary[26], boundary[32],
        "round-trip loss changes the original goal"
    );
}
