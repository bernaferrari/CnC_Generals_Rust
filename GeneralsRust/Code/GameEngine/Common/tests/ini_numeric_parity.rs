//! Bitwise arithmetic parity for complete finite INI numeric tokens.
//! Expectations come from extracted original INI.cpp methods and GameCommon.h,
//! executed by tests/oracles/generate_ini_numeric.py; no retail assets are used.
//! This does not claim parity for the differing C++/Rust token grammars.

use game_engine::common::ascii_string::AsciiString;
use game_engine::common::ini::ini::{INI, INIError, INIResult};
use game_engine::common::ini::ini_game_data::get_global_data;
use game_engine::common::ini::ini_weapon::{WeaponTemplate, get_weapon_store};

const ORIGINAL: &str = include_str!("fixtures/ini_numeric_original.txt");

fn check_column(column: usize, parse: impl Fn(&str) -> INIResult<f32>) {
    let mut count = 0;
    let mut differences = Vec::new();
    for row in ORIGINAL.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<_> = row.split_whitespace().collect();
        assert_eq!(fields.len(), 8);
        let expected = u32::from_str_radix(fields[column], 16).unwrap();
        let actual = parse(fields[0]).unwrap().to_bits();
        count += 1;
        if actual != expected {
            differences.push(format!(
                "{}: Rust={actual:08x}, C++={expected:08x}",
                fields[0]
            ));
        }
    }
    assert_eq!(count, 177, "the entire original-source corpus must execute");
    assert!(
        differences.is_empty(),
        "{} / {count} bit mismatches; first examples: {:?}",
        differences.len(),
        &differences[..differences.len().min(8)]
    );
}

#[test]
fn real_values_are_not_scaled() {
    check_column(1, INI::parse_real);
}

#[test]
fn angles_multiply_by_the_rounded_radians_per_degree() {
    check_column(2, INI::parse_angle_real);
}

#[test]
fn angular_velocities_multiply_by_the_rounded_per_frame_factor() {
    check_column(3, INI::parse_angular_velocity_real);
}

#[test]
fn parsed_velocities_multiply_by_the_rounded_seconds_per_frame() {
    check_column(4, INI::parse_velocity_real);
}

#[test]
fn converted_velocities_multiply_by_the_rounded_seconds_per_frame() {
    check_column(4, |token| {
        INI::parse_real(token).map(INI::convert_velocity_secs_to_frames)
    });
}

#[test]
fn accelerations_multiply_by_the_squared_rounded_seconds_per_frame() {
    check_column(5, |token| {
        INI::parse_real(token).map(INI::convert_acceleration_secs_to_frames)
    });
}

#[test]
fn duration_conversion_keeps_its_existing_millisecond_factor() {
    // Exercise the arithmetic helper, not Rust's distinct duration token grammar.
    check_column(6, |token| {
        INI::parse_real(token).map(INI::convert_duration_msecs_to_frames)
    });
}

#[test]
fn percentages_keep_their_existing_division() {
    check_column(7, INI::parse_percent_to_real);
}

#[test]
fn arithmetic_fix_keeps_existing_token_validation() {
    for token in [
        "", "x", "1junk", "1f", "1F", "1%", "1s", "1ms", " 1", "1 ", "1e", "+", "--1",
    ] {
        assert_eq!(INI::parse_angle_real(token), Err(INIError::InvalidData));
        assert_eq!(
            INI::parse_angular_velocity_real(token),
            Err(INIError::InvalidData)
        );
        assert_eq!(INI::parse_velocity_real(token), Err(INIError::InvalidData));
    }
    assert_eq!(INI::parse_real("1f"), Ok(1.0));
    assert_eq!(INI::parse_percent_to_real("25%"), Ok(0.25));
    assert_eq!(INI::parse_duration_unsigned_int("1s"), Ok(30));
    assert_eq!(INI::parse_duration_real("1s"), Ok(30.0));
    assert_eq!(INI::parse_duration_real("-1"), Err(INIError::InvalidData));
}

#[test]
fn authored_weapon_and_gravity_units_reach_the_production_parsers() {
    // Weapon.cpp uses parseReal for ScatterRadius and parseVelocityReal for
    // authored WeaponSpeed, while the constructor's default is already per-frame.
    let defaults = WeaponTemplate::new(AsciiString::from("NumericDefaults"));
    assert_eq!(defaults.projectile_speed.to_bits(), 999999.0_f32.to_bits());

    let mut ini = INI::new();
    ini.with_inline_source(
        "GameData\n Gravity = -9.8\nEnd\n\
         Weapon NumericParity\n WeaponSpeed = 90\n AcceptableAimDelta = 10\n\
         ScatterRadius = 90\nEnd\n",
        |ini| ini.parse_current_file(),
    )
    .unwrap();

    let store = get_weapon_store().unwrap();
    let weapon = store
        .find_template(&AsciiString::from("NumericParity"))
        .unwrap();
    assert_eq!(weapon.scatter_radius.to_bits(), 0x42b4_0000);
    assert_eq!(weapon.projectile_speed.to_bits(), 0x4040_0001);
    assert_eq!(weapon.acceptable_aim_delta.to_bits(), 0x3e32_b8c2);
    let data = get_global_data().unwrap();
    assert_eq!(data.read().gravity.to_bits(), 0xbc32_6751);
}
