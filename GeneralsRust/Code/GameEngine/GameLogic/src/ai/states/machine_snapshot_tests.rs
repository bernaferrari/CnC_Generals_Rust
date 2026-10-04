//! Source-derived retail wire: AIStates.cpp:733-795 and StateMachine.cpp:799-869.
//! This exercises the actual machine, not an original game binary or Object restore.
use super::*;
use crate::common::Coord3D;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
use std::sync::Weak;

fn original_v1_fixture(points: &[Coord3D]) -> Vec<u8> {
    let mut bytes = vec![1, 1]; // AIStateMachine then inherited StateMachine versions.
    bytes.extend_from_slice(&23u32.to_le_bytes()); // sleepTill
    bytes.extend_from_slice(&0u32.to_le_bytes()); // default AI_IDLE
    bytes.extend_from_slice(&0u32.to_le_bytes()); // current AI_IDLE
    bytes.push(0); // snapshotAllStates
    bytes.push(1); // AIIdleState version
    bytes.extend_from_slice(&7u16.to_le_bytes()); // initialSleepOffset
    bytes.extend_from_slice(&[1, 0]); // lookForTargets, inited; onEnter would change inited
    bytes.extend_from_slice(&91u32.to_le_bytes()); // goalObjectID
    for value in [5f32, 6f32, 7f32] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[1, 1]); // locked, defaultStateInited
    bytes.extend_from_slice(&(points.len() as i32).to_le_bytes());
    for point in points {
        for value in [point.x, point.y, point.z] {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    bytes.push(0); // empty waypoint ASCII string
    bytes.push(0); // no goal squad
    bytes.extend_from_slice(&999_999u32.to_le_bytes()); // Common/StateMachine.h INVALID_STATE_ID
    bytes.extend_from_slice(&29u32.to_le_bytes()); // temporaryStateFrameEnd
    bytes
}

fn assert_original_v1_round_trip(points: &[Coord3D]) {
    let expected = original_v1_fixture(points);
    let mut input = expected.clone();
    input.extend_from_slice(&0x7654_3210u32.to_le_bytes());
    let mut reader = Cursor::new(input);
    let mut machine = AIStateMachine::new(Weak::new(), "wire-fixture");
    {
        let mut load = XferLoad::new(&mut reader, 1);
        Snapshotable::xfer(&mut machine, &mut load).expect("load original source-derived v1");
        let mut sentinel = 0u32;
        load.xfer_unsigned_int(&mut sentinel).unwrap();
        assert_eq!(
            sentinel, 0x7654_3210,
            "consume exactly the original machine payload"
        );
    }
    assert_eq!(machine.get_goal_path_size(), points.len());
    for (index, point) in points.iter().enumerate() {
        assert_eq!(machine.get_goal_path_position(index), Some(point));
    }
    assert_eq!(machine.get_current_state_id(), Some(0));
    let mut bytes = Vec::new();
    Snapshotable::xfer(&mut machine, &mut XferSave::new(Cursor::new(&mut bytes), 1)).unwrap();
    assert_eq!(
        bytes, expected,
        "resave original Coord3D-only goal path, without replaying state callbacks"
    );
}

#[test]
fn original_machine_v1_empty_goal_path_resaves_exact_payload() {
    assert_original_v1_round_trip(&[]);
}

#[test]
fn original_machine_v1_goal_path_resaves_coordinates_without_path_node_metadata() {
    assert_original_v1_round_trip(&[Coord3D::new(1.25, 2.5, 3.75), Coord3D::new(4.5, 5.25, 6.0)]);
}
