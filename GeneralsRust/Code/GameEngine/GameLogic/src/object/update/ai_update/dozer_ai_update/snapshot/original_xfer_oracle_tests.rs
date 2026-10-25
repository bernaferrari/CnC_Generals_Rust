//! Primary codec bytes recorded from fifteen pinned original C++ Xfer bodies.
//! Full inherited AI payload, postprocessing and gameplay are outside this fixture.
use super::*;
fn decode_hex(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0);
    text.as_bytes()
        .chunks_exact(2)
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}
#[test]
fn primary_native_codec_matches_executed_original_cpp_xfer_bodies() {
    let output = include_str!("dozer_primary_original.jsonl");
    let mut count = 0;
    for line in output.lines() {
        let row: serde_json::Value = serde_json::from_str(line).expect("oracle JSONL");
        let name = row["case"].as_str().unwrap();
        let mut value = PrimaryMachine::new();
        value.state = match name {
            "idle" => PrimaryState::Idle,
            "repair" => PrimaryState::Repair,
            _ => panic!("unknown executed original oracle case {name}"),
        };
        value.control.initialized = true;
        value.control.sleep_until = 77;
        value.control.goal_object = 99;
        value.control.goal_position = Coord3D::new(11.0, 22.0, 33.0);
        value.control.locked = true;
        value.idle.idle_since = 31;
        value.idle.player = 2;
        value.idle.marked = true;
        let expected = decode_hex(row["primary_hex"].as_str().unwrap());
        assert_eq!(
            value.save_original_control_bytes().unwrap(),
            expected,
            "{name}"
        );
        let loaded = PrimaryMachine::load_original_control_bytes(&expected).unwrap();
        assert_eq!(
            loaded.save_original_control_bytes().unwrap(),
            expected,
            "{name} load"
        );
        assert_eq!(row["invalid_docks_roundtrip"], true);
        count += 1;
    }
    assert_eq!(count, 2, "both actual-original fixtures must execute");
}
