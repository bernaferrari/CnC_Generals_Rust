//! Wire fixtures for C++ AIGuardRetaliate and its embedded pickup state.
use super::*;
use crate::ai::states::AIPickUpCrateState;
use game_engine::common::system::{Snapshotable, xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

fn append_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn append_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn append_coord(out: &mut Vec<u8>, value: Coord3D) {
    for component in [value.x, value.y, value.z] {
        out.extend_from_slice(&component.to_le_bytes());
    }
}

fn save<T: Snapshotable>(value: &mut T) -> Vec<u8> {
    let mut bytes = Vec::new();
    value
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    bytes
}

fn save_state<T: StateImplementation>(value: &mut T) -> Vec<u8> {
    let mut bytes = Vec::new();
    value
        .xfer_snapshot(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    bytes
}

fn pickup_fixture() -> Vec<u8> {
    let mut expected = vec![1, 1]; // pickup wrapper v1, AIInternalMoveToState v1
    append_coord(&mut expected, Coord3D::new(1.25, -2.5, 3.75));
    append_u32(&mut expected, 4); // PathfindLayerEnum is sizeof(enum), four bytes
    expected.push(1); // waiting for path
    append_coord(&mut expected, Coord3D::new(-4.5, 5.25, -6.0));
    append_u32(&mut expected, 0x1122_3344); // path timestamp
    append_u32(&mut expected, 0x5566_7788); // blocked repath timestamp
    expected.push(0); // adjust destinations
    append_i32(&mut expected, -17); // pickup delay
    append_coord(&mut expected, Coord3D::new(7.5, -8.25, 9.0));
    expected
}

fn pickup_with_fixture() -> AIPickUpCrateState {
    let machine = StateMachine::new(None, "pickup snapshot fixture");
    let mut pickup = AIPickUpCrateState::new(&machine);
    pickup.base.goal_position = Coord3D::new(1.25, -2.5, 3.75);
    pickup.base.goal_layer = 0x04;
    pickup.base.waiting_for_path = true;
    pickup.base.path_goal_position = Coord3D::new(-4.5, 5.25, -6.0);
    pickup.base.path_timestamp = 0x1122_3344;
    pickup.base.blocked_repath_timestamp = 0x5566_7788;
    pickup.base.adjust_destinations = false;
    pickup.delay_counter = -17;
    pickup.goal_position = Coord3D::new(7.5, -8.25, 9.0);
    pickup
}

#[test]
fn pickup_wire_matches_cpp_nested_versions_fields_and_enum_width() {
    let expected = pickup_fixture();
    let mut pickup = pickup_with_fixture();
    assert_eq!(save(&mut pickup), expected);

    let mut restored = AIPickUpCrateState::new(&StateMachine::new(None, "pickup restore"));
    restored
        .xfer(&mut XferLoad::new(Cursor::new(expected), 1))
        .unwrap();
    assert_eq!(restored.base.goal_position, Coord3D::new(1.25, -2.5, 3.75));
    assert_eq!(restored.base.goal_layer, 4);
    assert!(restored.base.waiting_for_path);
    assert_eq!(
        restored.base.path_goal_position,
        Coord3D::new(-4.5, 5.25, -6.0)
    );
    assert_eq!(restored.base.path_timestamp, 0x1122_3344);
    assert_eq!(restored.base.blocked_repath_timestamp, 0x5566_7788);
    assert!(!restored.base.adjust_destinations);
    assert_eq!(restored.delay_counter, -17);
    assert_eq!(restored.goal_position, Coord3D::new(7.5, -8.25, 9.0));
}

#[test]
fn guard_retaliate_embedded_pickup_uses_shared_pickup_wire_payload() {
    let machine = StateMachine::new(None, "retaliate pickup snapshot fixture");
    let mut state = AIGuardRetaliatePickUpCrateState::new(&machine);
    state.install_test_pickup(pickup_with_fixture());
    let expected = pickup_fixture();
    assert_eq!(save_state(&mut state), expected);
}

#[test]
fn pickup_rejects_unsupported_versions_and_truncated_inherited_payload() {
    let mut pickup = pickup_with_fixture();
    let mut unsupported_base_version = Cursor::new(vec![1, 2, 0, 0, 0]);
    assert!(
        pickup
            .xfer(&mut XferLoad::new(&mut unsupported_base_version, 1))
            .is_err()
    );

    let mut bytes = pickup_fixture();
    bytes.pop();
    let mut truncated = Cursor::new(bytes);
    assert!(pickup.xfer(&mut XferLoad::new(&mut truncated, 1)).is_err());
}

#[test]
fn retaliation_child_payloads_match_cpp_versions_and_only_declared_fields() {
    let machine = StateMachine::new(None, "retaliate child snapshots");
    let mut inner = AIGuardRetaliateInnerState::new(&machine);
    let mut outer = AIGuardRetaliateOuterState::new(&machine);
    let mut aggressor = AIGuardRetaliateAttackAggressorState::new(&machine);
    assert_eq!(save_state(&mut inner), [1]);
    assert_eq!(save_state(&mut outer), [1]);
    assert_eq!(save_state(&mut aggressor), [1]);

    let mut idle = AIGuardRetaliateIdleState::new(&machine);
    idle.next_enemy_scan_time = 0x1020_3040;
    let idle_bytes = save_state(&mut idle);
    let mut expected_idle = vec![1];
    append_u32(&mut expected_idle, 0x1020_3040);
    assert_eq!(idle_bytes, expected_idle);
    let mut restored_idle = AIGuardRetaliateIdleState::new(&machine);
    restored_idle
        .xfer_snapshot(&mut XferLoad::new(Cursor::new(idle_bytes), 1))
        .unwrap();
    assert_eq!(restored_idle.next_enemy_scan_time, 0x1020_3040);

    let mut returning = AIGuardRetaliateReturnState::new(&machine);
    returning.next_return_scan_time = 0xa1b2_c3d4;
    let mut expected_return = vec![1];
    append_u32(&mut expected_return, 0xa1b2_c3d4);
    assert_eq!(save_state(&mut returning), expected_return);
}

#[test]
fn unsupported_or_truncated_child_payloads_fail_without_postprocess_callbacks() {
    let machine = StateMachine::new(None, "invalid snapshot inputs");
    let mut idle = AIGuardRetaliateIdleState::new(&machine);
    let mut unsupported = Cursor::new(vec![2, 0, 0, 0, 0]);
    assert!(
        idle.xfer_snapshot(&mut XferLoad::new(&mut unsupported, 1))
            .is_err()
    );

    let mut returning = AIGuardRetaliateReturnState::new(&machine);
    let mut truncated = Cursor::new(vec![1, 0xAA, 0xBB]);
    assert!(
        returning
            .xfer_snapshot(&mut XferLoad::new(&mut truncated, 1))
            .is_err()
    );
}

// Access-only fixture adapter: OLD wraps this assignment in Some; wire
// assertions and the serializer exercised by the test remain identical.
trait TestPickupInstall {
    fn install_test_pickup(&mut self, pickup: AIPickUpCrateState);
}
impl TestPickupInstall for AIGuardRetaliatePickUpCrateState {
    fn install_test_pickup(&mut self, pickup: AIPickUpCrateState) {
        self.pickup = pickup;
    }
}

#[test]
fn pickup_roundtrip_preserves_raw_cpp_enum_width_without_truncation() {
    let mut expected = pickup_fixture();
    expected[14..18].copy_from_slice(&0xfedc_ba98u32.to_le_bytes());
    let mut pickup = AIPickUpCrateState::new(&StateMachine::new(None, "raw enum restore"));
    pickup
        .xfer(&mut XferLoad::new(Cursor::new(&expected), 1))
        .unwrap();
    assert_eq!(pickup.base.goal_layer as u32, 0xfedc_ba98);
    assert_eq!(save(&mut pickup), expected);
}

fn retaliate_v2_fixture(all_states: bool, current: u32) -> Vec<u8> {
    let mut bytes = vec![2, 1]; // AIGuardRetaliateMachine v2, StateMachine v1
    append_u32(&mut bytes, 0); // sleep_till
    append_u32(&mut bytes, 5005); // default is first-defined AttackAggressor
    append_u32(&mut bytes, current);
    bytes.push(u8::from(all_states));
    if all_states {
        append_i32(&mut bytes, 6);
        for id in [5000, 5001, 5002, 5003, 5004, 5005] {
            append_u32(&mut bytes, id); // C++ std::map state ID ordering
            match id {
                5000 | 5002 | 5005 => bytes.push(1),
                5001 => {
                    bytes.push(1);
                    append_u32(&mut bytes, 0x1020_3040);
                }
                5003 => {
                    bytes.push(1);
                    append_u32(&mut bytes, 0xa1b2_c3d4);
                }
                5004 => {
                    // AIPickUpCrateState v1 + AIInternalMoveToState v1, defaults.
                    bytes.extend_from_slice(&[1, 1]);
                    append_coord(&mut bytes, Coord3D::new(0.0, 0.0, 0.0));
                    append_u32(&mut bytes, 0);
                    bytes.push(0);
                    append_coord(&mut bytes, Coord3D::new(0.0, 0.0, 0.0));
                    append_u32(&mut bytes, 0);
                    append_u32(&mut bytes, 0);
                    bytes.push(1); // AIMoveToState adjust_destinations default
                    append_i32(&mut bytes, 0);
                    append_coord(&mut bytes, Coord3D::new(0.0, 0.0, 0.0));
                }
                _ => unreachable!(),
            }
        }
    } else {
        // current-only: Idle's state payload follows immediately.
        assert_eq!(current, 5001);
        bytes.push(1);
        append_u32(&mut bytes, 0x1020_3040);
    }
    append_u32(&mut bytes, 0); // goal_object_id / INVALID_ID
    append_coord(&mut bytes, Coord3D::new(0.0, 0.0, 0.0));
    bytes.extend_from_slice(&[0, 0]); // locked, default_state_inited
    append_u32(&mut bytes, 0x1234_5678); // retaliation nemesis
    append_coord(&mut bytes, Coord3D::new(1.5, -2.25, 3.0));
    bytes
}

#[test]
fn retaliate_v2_all_states_loads_cpp_sorted_children_then_saves_current_only() {
    let mut machine = AIGuardRetaliateMachine::new(Weak::new());
    let all_states = retaliate_v2_fixture(true, 5001);
    machine
        .xfer(&mut XferLoad::new(Cursor::new(all_states), 1))
        .unwrap();
    assert_eq!(machine.state_machine.get_current_state_id(), Some(5001));
    let idle = machine.state_machine.get_state_mut(5001).unwrap();
    assert_eq!(
        idle.as_ref()
            .as_any()
            .downcast_ref::<AIGuardRetaliateIdleState>()
            .unwrap()
            .next_enemy_scan_time,
        0x1020_3040
    );
    let returning = machine.state_machine.get_state_mut(5003).unwrap();
    assert_eq!(
        returning
            .as_ref()
            .as_any()
            .downcast_ref::<AIGuardRetaliateReturnState>()
            .unwrap()
            .next_return_scan_time,
        0xa1b2_c3d4
    );

    // Release-build C++ saves only the current state's payload, even though the
    // debug all-state selector is accepted on load.
    let expected = retaliate_v2_fixture(false, 5001);
    let mut saved = Vec::new();
    machine
        .xfer(&mut XferSave::new(Cursor::new(&mut saved), 1))
        .unwrap();
    assert_eq!(saved, expected);
}

#[test]
fn retaliate_v1_fixture_is_load_only_and_skips_nested_machine() {
    let mut bytes = vec![1]; // old machine envelope version
    append_u32(&mut bytes, 0x1234_5678);
    append_coord(&mut bytes, Coord3D::new(1.5, -2.25, 3.0));
    let mut machine = AIGuardRetaliateMachine::new(Weak::new());
    machine
        .xfer(&mut XferLoad::new(Cursor::new(bytes), 1))
        .unwrap();
    assert_eq!(machine.state_machine.get_current_state_id(), None);
    assert_eq!(machine.get_nemesis_id(), 0x1234_5678);
}

#[test]
fn native_guard_retaliate_wrapper_roundtrips_nested_machine_envelope() {
    let _serial = crate::test_sync::lock();
    let owner = Arc::new(RwLock::new(Object::new_test(0x7842, 100.0)));
    let parent = StateMachine::new(Some(Arc::downgrade(&owner)), "registered parent");
    let mut child = AIGuardRetaliateMachine::new(Arc::downgrade(&owner));
    child
        .xfer(&mut XferLoad::new(
            Cursor::new(retaliate_v2_fixture(false, 5001)),
            1,
        ))
        .unwrap();
    let mut registered = crate::ai::states::AIGuardRetaliateState::new(&parent);
    registered.guard_machine = Some(child);
    let mut expected = vec![1, 1]; // registered-state v1, has child machine
    expected.extend_from_slice(&retaliate_v2_fixture(false, 5001));
    let mut saved = Vec::new();
    registered
        .xfer(&mut XferSave::new(Cursor::new(&mut saved), 1))
        .unwrap();
    assert_eq!(saved, expected);

    let mut restored = crate::ai::states::AIGuardRetaliateState::new(&parent);
    restored
        .xfer(&mut XferLoad::new(Cursor::new(saved), 1))
        .unwrap();
    Snapshotable::load_post_process(&mut restored).unwrap();
    let loaded = restored.guard_machine.as_mut().unwrap();
    assert_eq!(loaded.state_machine.get_current_state_id(), Some(5001));
    assert_eq!(loaded.get_nemesis_id(), 0x1234_5678);
}
