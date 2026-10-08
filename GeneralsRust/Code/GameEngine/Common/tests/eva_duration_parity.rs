//! Execute Common's actual local EVA block parser against frozen original C++
//! unsigned-duration results. No client registration, singleton, audio or ticks.
//! Common is the fallback parser; this does not claim live-client coverage.

use game_engine::common::ini::ini_eva_event::{EvaCheckInfo, EvaEventStore, EvaMessage};
use game_engine::common::ini::{INI, INIError, INIResult};

const ORIGINAL: &str = include_str!("fixtures/eva_duration_original.txt");

fn parse_local_event(fields: &str) -> INIResult<EvaCheckInfo> {
    let mut store = EvaEventStore::new();
    let mut ini = INI::new();
    let source = format!("EvaEvent LowPower\n{fields}End\n");
    ini.with_inline_source(&source, |ini| {
        ini.read_line()?;
        store.parse_eva_event_definition(ini)
    })?;
    Ok(store
        .get_check_info(EvaMessage::LowPower)
        .expect("the local parser must retain the event")
        .clone())
}

#[test]
fn local_eva_timing_fields_match_original_unsigned_duration() {
    let mut cases = 0;
    let mut differences = Vec::new();
    for line in ORIGINAL.lines().filter(|line| !line.starts_with('#')) {
        let row: Vec<_> = line.split_whitespace().collect();
        assert_eq!(row.len(), 6);
        let token = row[0];
        assert_eq!(
            token.parse::<u32>().unwrap(),
            row[1].parse::<u32>().unwrap()
        );
        let expected: u32 = row[5].parse().unwrap();
        let info = parse_local_event(&format!(
            "TimeBetweenChecksMS = {token}\nExpirationTimeMS = {token}\n"
        ))
        .unwrap();
        if info.frames_between_checks != expected || info.frames_to_expire != expected {
            differences.push(format!(
                "{token}: between={}, expire={}, C++={expected}",
                info.frames_between_checks, info.frames_to_expire
            ));
        }
        cases += 1;
    }
    assert_eq!(cases, 242, "the entire finite unsigned corpus must execute");
    assert!(
        differences.is_empty(),
        "{} / {cases} mismatches: {:?}",
        differences.len(),
        &differences[..differences.len().min(8)]
    );
}

#[test]
fn omitted_timing_fields_keep_cpp_constructor_defaults() {
    // Eva.cpp:183-184. These frame defaults are separate from an authored -1.
    let defaults = parse_local_event("").unwrap();
    assert_eq!(defaults.frames_between_checks, 900);
    assert_eq!(defaults.frames_to_expire, 150);

    let between = parse_local_event("TimeBetweenChecksMS = 1\n").unwrap();
    assert_eq!(between.frames_between_checks, 1);
    assert_eq!(between.frames_to_expire, 150);

    let expiration = parse_local_event("ExpirationTimeMS = 1\n").unwrap();
    assert_eq!(expiration.frames_between_checks, 900);
    assert_eq!(expiration.frames_to_expire, 1);
}

#[test]
fn local_parser_keeps_its_existing_duration_token_grammar() {
    // Rust extensions and grammar residuals, not C++ unsigned-parser parity.
    for (token, expected) in [
        ("1ms", 1),
        ("34ms", 2),
        ("1s", 30),
        ("0.034s", 2),
        ("1.5", 1),
        ("33.5", 2),
        ("1.25s", 38),
        ("NaN", 0),
        ("inf", u32::MAX),
    ] {
        let info = parse_local_event(&format!(
            "TimeBetweenChecksMS = {token}\nExpirationTimeMS = {token}\n"
        ))
        .unwrap();
        assert_eq!(info.frames_between_checks, expected, "{token}");
        assert_eq!(info.frames_to_expire, expected, "{token}");
    }
    for field in ["TimeBetweenChecksMS", "ExpirationTimeMS"] {
        for token in ["", "x", "1junk", "1f", "1F", "-0", "-1", "-1ms", "-1s"] {
            assert!(
                matches!(
                    parse_local_event(&format!("{field} = {token}\n")),
                    Err(INIError::InvalidData)
                ),
                "{field} = {token}"
            );
        }
    }
}
