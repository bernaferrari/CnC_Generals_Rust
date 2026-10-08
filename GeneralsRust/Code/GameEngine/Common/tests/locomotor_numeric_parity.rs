//! Public locomotor loader arithmetic, bounded to Common's nine converted fields.
//! Original callback outputs: fixtures/ini_numeric_original.txt. Field/type proof:
//! oracles/generate_locomotor_numeric.py. Raw fields and existing defaults are
//! compatibility controls, not claims of complete original locomotor parity.

use game_engine::common::ini::ini::INI;
use game_engine::common::ini::ini_locomotor::{
    LocomotorTemplate, get_locomotor_store, load_locomotors_from_str,
    parse_locomotor_template_definition,
};
use std::collections::HashMap;

const ORIGINAL: &str = include_str!("fixtures/ini_numeric_original.txt");
const FIELDS: [(&str, usize, fn(&LocomotorTemplate) -> f32); 9] = [
    ("Speed", 4, |t| t.max_speed),
    ("SpeedDamaged", 4, |t| t.max_speed_damaged),
    ("TurnRate", 3, |t| t.max_turn_rate),
    ("TurnRateDamaged", 3, |t| t.max_turn_rate_damaged),
    ("Acceleration", 5, |t| t.acceleration),
    ("AccelerationDamaged", 5, |t| t.acceleration_damaged),
    ("Lift", 5, |t| t.lift),
    ("LiftDamaged", 5, |t| t.lift_damaged),
    ("Braking", 5, |t| t.braking),
];

fn load(name: &str, body: &str, bulk: bool) -> LocomotorTemplate {
    let text = format!("Locomotor {name}\n{body}\nEnd\n");
    if bulk {
        assert_eq!(load_locomotors_from_str(&text).unwrap(), 1);
    } else {
        INI::new()
            .with_inline_source(&text, |ini| ini.parse_current_file())
            .unwrap();
    }
    get_locomotor_store().find_template(name).unwrap().clone()
}

fn check_field(index: usize) {
    let (field, column, get) = FIELDS[index];
    let mut differences = Vec::new();
    let mut count = 0;
    for (row_index, row) in ORIGINAL
        .lines()
        .filter(|line| !line.starts_with('#'))
        .enumerate()
    {
        let row: Vec<_> = row.split_whitespace().collect();
        let token = row[0];
        let expected = u32::from_str_radix(row[column], 16).unwrap();
        // Equal healthy/damaged inputs leave validate()'s negative fallback
        // unchanged, including its distinction between negatives and -0.
        let body = FIELDS
            .iter()
            .map(|(key, _, _)| format!("{key} = {token}"))
            .collect::<Vec<_>>()
            .join("\n");
        for bulk in [false, true] {
            let name = format!("Numeric{index}_{row_index}_{bulk}");
            let actual = get(&load(&name, &body, bulk)).to_bits();
            count += 1;
            if actual != expected {
                differences.push(format!(
                    "{field}={token} bulk={bulk}: Rust={actual:08x}, C++={expected:08x}"
                ));
            }
        }
    }
    assert_eq!(count, 354, "both loaders must execute all 177 tokens");
    assert!(
        differences.is_empty(),
        "{} / {count} bit mismatches; examples: {:?}",
        differences.len(),
        &differences[..differences.len().min(6)]
    );
}

#[test]
fn speed() {
    check_field(0);
}
#[test]
fn speed_damaged() {
    check_field(1);
}
#[test]
fn turn_rate() {
    check_field(2);
}
#[test]
fn turn_rate_damaged() {
    check_field(3);
}
#[test]
fn acceleration() {
    check_field(4);
}
#[test]
fn acceleration_damaged() {
    check_field(5);
}
#[test]
fn lift() {
    check_field(6);
}
#[test]
fn lift_damaged() {
    check_field(7);
}
#[test]
fn braking() {
    check_field(8);
}

#[test]
fn distinct_authored_fields_reach_their_own_members() {
    let tokens = ["0.1", "45", "10", "999999", "1", "9.8", "30", "900", "90"];
    let body = FIELDS
        .iter()
        .zip(tokens)
        .map(|((field, _, _), token)| format!("{field}={token}"))
        .collect::<Vec<_>>()
        .join("\n");
    for bulk in [false, true] {
        let template = load(&format!("NumericDistinct{bulk}"), &body, bulk);
        for ((field, column, get), token) in FIELDS.iter().zip(tokens) {
            let row = ORIGINAL
                .lines()
                .find(|row| row.split_whitespace().next() == Some(token))
                .unwrap();
            let row: Vec<_> = row.split_whitespace().collect();
            assert_eq!(
                get(&template).to_bits(),
                u32::from_str_radix(row[*column], 16).unwrap(),
                "{field}"
            );
        }
    }
}

#[test]
fn omitted_fields_keep_original_braking_and_other_existing_defaults() {
    for bulk in [false, true] {
        let template = load(&format!("NumericDefaults{bulk}"), "", bulk);
        for (field, _, get) in FIELDS {
            let expected = if field == "Braking" {
                99999.0_f32.to_bits()
            } else {
                0
            };
            assert_eq!(get(&template).to_bits(), expected, "{field}");
        }
        // Suspension defaults are owned by Common, matching the C++ constructor.
        // Unrelated raw/default conventions remain unchanged in this slice.
        assert_eq!(template.min_turn_speed.to_bits(), 0);
        assert_eq!(template.speed_limit_z.to_bits(), 1_000_000.0_f32.to_bits());
        assert_eq!(template.pitch_stiffness.to_bits(), 0.1_f32.to_bits());
        assert_eq!(template.uniform_axial_damping.to_bits(), 1.0_f32.to_bits());
        assert_eq!(
            template.preferred_height_damping.to_bits(),
            1.0_f32.to_bits()
        );
        assert_eq!(template.close_enough_dist.to_bits(), 1.0_f32.to_bits());
        assert_eq!(template.wander_length_factor.to_bits(), 1.0_f32.to_bits());
    }
}

#[test]
fn damaged_fallback_and_authored_zero_keep_their_existing_branches() {
    for bulk in [false, true] {
        for (suffix, damaged, expected_zero) in [
            ("Omitted", "", None),
            ("Negative", "-1", None),
            ("Zero", "0", Some(0)),
            ("NegativeZero", "-0", Some(0x8000_0000)),
            ("Underflow", "-1.40129846e-45", Some(0x8000_0000)),
        ] {
            let mut body = "Speed=90\nTurnRate=90\nAcceleration=90\nLift=90".to_owned();
            if !damaged.is_empty() {
                for field in [
                    "SpeedDamaged",
                    "TurnRateDamaged",
                    "AccelerationDamaged",
                    "LiftDamaged",
                ] {
                    body.push_str(&format!("\n{field}={damaged}"));
                }
            }
            let template = load(&format!("NumericFallback{suffix}{bulk}"), &body, bulk);
            for (healthy, damaged) in [(0, 1), (2, 3), (4, 5), (6, 7)] {
                let expected =
                    expected_zero.unwrap_or_else(|| FIELDS[healthy].2(&template).to_bits());
                assert_eq!(FIELDS[damaged].2(&template).to_bits(), expected, "{suffix}");
            }
        }
    }
}

#[test]
fn raw_authored_fields_keep_the_downstream_conversion_contract() {
    let fields: [(&str, fn(&LocomotorTemplate) -> f32); 16] = [
        ("MinSpeed", |t| t.min_speed),
        ("MinTurnSpeed", |t| t.min_turn_speed),
        ("SpeedLimitZ", |t| t.speed_limit_z),
        ("Extra2DFriction", |t| t.extra_2d_friction),
        ("MaxThrustAngle", |t| t.max_thrust_angle),
        ("AccelerationPitchLimit", |t| t.accel_pitch_limit),
        ("DecelerationPitchLimit", |t| t.decel_pitch_limit),
        ("BounceAmount", |t| t.bounce_kick),
        ("FrontWheelTurnAngle", |t| t.wheel_turn_angle),
        ("SlideIntoPlaceTime", |t| {
            t.ultra_accurate_slide_into_place_factor
        }),
        ("ThrustRoll", |t| t.thrust_roll),
        ("ThrustWobbleRate", |t| t.wobble_rate),
        ("RudderCorrectionDegree", |t| t.rudder_correction_degree),
        ("ElevatorCorrectionRate", |t| t.elevator_correction_rate),
        ("PitchStiffness", |t| t.pitch_stiffness),
        ("PreferredHeight", |t| t.preferred_height),
    ];
    for (index, token) in ["90", "-0", "3.40282347e+38"].iter().enumerate() {
        let body = fields
            .iter()
            .map(|(field, _)| format!("{field}={token}"))
            .collect::<Vec<_>>()
            .join("\n");
        for bulk in [false, true] {
            let template = load(&format!("NumericRaw{index}{bulk}"), &body, bulk);
            for (field, get) in fields {
                assert_eq!(
                    get(&template).to_bits(),
                    token.parse::<f32>().unwrap().to_bits(),
                    "{field}"
                );
            }
        }
    }
}

#[test]
fn duplicate_fields_and_replaced_templates_convert_only_the_final_authored_values() {
    for bulk in [false, true] {
        let name = format!("NumericOverride{bulk}");
        for token in ["90", "10"] {
            let body = FIELDS
                .iter()
                .map(|(field, _, _)| format!("{field}=1\n{field}={token}"))
                .collect::<Vec<_>>()
                .join("\n");
            let template = load(&name, &body, bulk);
            let row = ORIGINAL
                .lines()
                .find(|row| row.split_whitespace().next() == Some(token))
                .unwrap();
            let row: Vec<_> = row.split_whitespace().collect();
            for (field, column, get) in FIELDS {
                assert_eq!(
                    get(&template).to_bits(),
                    u32::from_str_radix(row[column], 16).unwrap(),
                    "{field}"
                );
            }
        }
    }
}

#[test]
fn conversion_changes_do_not_expand_the_existing_token_grammar() {
    for (field, _, _) in FIELDS {
        for token in ["", "x", "1f", "1%", "1s", "1ms", "1junk", " 1", "1 ", "1e"] {
            let properties = HashMap::from([(field.to_owned(), token.to_owned())]);
            assert!(
                parse_locomotor_template_definition("NumericInvalid", &properties).is_err(),
                "{field}={token}"
            );
        }
    }
    let properties = HashMap::from([("UnknownNumericField".to_owned(), "1".to_owned())]);
    assert!(parse_locomotor_template_definition("NumericUnknown", &properties).is_err());
}

#[test]
fn original_corpus_detects_division_and_reassociated_factor_mutants() {
    let mutants: [(usize, fn(f32) -> f32); 6] = [
        (4, |v| v / 30.0),
        (5, |v| v / 900.0),
        (5, |v| (v * (1.0_f32 / 30.0)) * (1.0_f32 / 30.0)),
        (3, |v| v * std::f32::consts::PI / (180.0 * 30.0)),
        (3, |v| v * (std::f32::consts::PI / 180.0) * (1.0_f32 / 30.0)),
        (3, |v| v * ((1.0_f32 / 30.0) * std::f32::consts::PI) / 180.0),
    ];
    for (index, (column, mutant)) in mutants.iter().enumerate() {
        let mismatches = ORIGINAL
            .lines()
            .filter(|row| !row.starts_with('#'))
            .filter(|row| {
                let row: Vec<_> = row.split_whitespace().collect();
                mutant(row[0].parse().unwrap()).to_bits()
                    != u32::from_str_radix(row[*column], 16).unwrap()
            })
            .count();
        assert!(
            mismatches > 0,
            "factor-order mutant {index} escaped the corpus"
        );
    }
}
