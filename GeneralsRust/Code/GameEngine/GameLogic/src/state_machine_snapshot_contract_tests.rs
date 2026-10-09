//! Common/StateMachine.cpp:799-868 container wire, including null-state recovery.
use super::*;
use game_engine::common::random_value::{
    get_game_logic_random_seed_state, set_game_logic_random_seed_state,
};
use game_engine::common::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Debug, Default)]
struct Callbacks {
    enters: AtomicUsize,
    exits: AtomicUsize,
}
#[derive(Debug)]
struct PayloadState {
    id: StateId,
    payload: u32,
    snapshot_calls: u32,
    callbacks: Arc<Callbacks>,
}
impl StateImplementation for PayloadState {
    fn set_id(&mut self, id: StateId) {
        self.id = id;
    }
    fn get_id(&self) -> StateId {
        self.id
    }
    fn on_enter(&mut self) -> StateReturnType {
        self.callbacks.enters.fetch_add(1, Ordering::Relaxed);
        StateReturnType::Continue
    }
    fn on_exit(&mut self, _: StateExitType) {
        self.callbacks.exits.fetch_add(1, Ordering::Relaxed);
    }
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }
    fn xfer_snapshot(&mut self, xfer: &mut dyn crate::common::xfer::Xfer) -> Result<(), String> {
        self.snapshot_calls += 1;
        xfer.xfer_unsigned_int(&mut self.payload)
            .map_err(|error| format!("{error:?}"))
    }
}
struct PreserveRng([u32; 6]);
impl Drop for PreserveRng {
    fn drop(&mut self) {
        set_game_logic_random_seed_state(self.0);
    }
}
fn machine(owner: &Arc<RwLock<Object>>, values: [u32; 2]) -> (StateMachine, Arc<Callbacks>) {
    let machine = StateMachine::new(Some(Arc::downgrade(owner)), "snapshot contract");
    define_payload_states(machine, values)
}
// The wire/error cases below only need the owner ID copied into State metadata;
// allocating Object would also run its process-global destructor callbacks.
fn machine_with_owner_id(
    owner_id: crate::common::ObjectID,
    values: [u32; 2],
) -> (StateMachine, Arc<Callbacks>) {
    define_payload_states(
        StateMachine::new_with_owner_id(owner_id, "snapshot contract"),
        values,
    )
}
fn define_payload_states(
    mut machine: StateMachine,
    values: [u32; 2],
) -> (StateMachine, Arc<Callbacks>) {
    let callbacks = Arc::new(Callbacks::default());
    // C++ std::map orders by ID, regardless of definition order. First defined
    // remains the default, so use the larger ID first to distinguish both rules.
    for (id, payload) in [(20, values[1]), (10, values[0])] {
        machine.define_state(
            id,
            Box::new(PayloadState {
                id,
                payload,
                snapshot_calls: 0,
                callbacks: callbacks.clone(),
            }),
            None,
            None,
            None,
        );
    }
    (machine, callbacks)
}
fn owner(health: f32) -> Arc<RwLock<Object>> {
    Arc::new(RwLock::new(Object::new_test(0x7AF12010, health)))
}
fn payload(machine: &mut StateMachine, id: StateId) -> u32 {
    machine
        .get_state_mut(id)
        .unwrap()
        .as_ref()
        .as_any()
        .downcast_ref::<PayloadState>()
        .unwrap()
        .payload
}
fn append_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}
fn prefix(current: StateId, all_states: bool) -> Vec<u8> {
    let mut bytes = vec![1];
    for value in [37u32, 20, current] {
        append_u32(&mut bytes, value);
    }
    bytes.push(u8::from(all_states));
    bytes
}
fn append_tail(bytes: &mut Vec<u8>) {
    append_u32(bytes, 0x1234ABCD);
    for value in [2.5f32, -3.5, 7.0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[1, 1]);
}
fn all_states_fixture(count: i32, ids: [StateId; 2]) -> Vec<u8> {
    let mut bytes = prefix(10, true);
    bytes.extend_from_slice(&count.to_le_bytes());
    for (id, value) in ids.into_iter().zip([0x76543210, 0xFEDCBA98]) {
        append_u32(&mut bytes, id);
        append_u32(&mut bytes, value);
    }
    append_tail(&mut bytes);
    append_u32(&mut bytes, 0xDEADBEEF);
    bytes
}
fn save(machine: &mut StateMachine) -> Vec<u8> {
    let mut bytes = Vec::new();
    machine
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    bytes
}
fn assert_no_callbacks(callbacks: &Callbacks) {
    assert_eq!(callbacks.enters.load(Ordering::Relaxed), 0);
    assert_eq!(callbacks.exits.load(Ordering::Relaxed), 0);
}

#[test]
fn all_states_load_restores_sorted_payloads_and_consumes_exact_goal_tail() {
    let _serial = crate::test_sync::lock();
    let _rng = PreserveRng(get_game_logic_random_seed_state());
    let seed = get_game_logic_random_seed_state();
    let first = owner(100.0);
    let second = owner(200.0);
    let (mut machine, callbacks) = machine(&first, [1, 2]);
    let (mut untouched, _) = self::machine(&second, [101, 202]);
    let mut reader = Cursor::new(all_states_fixture(2, [10, 20]));
    {
        let mut load = XferLoad::new(&mut reader, 1);
        machine.xfer(&mut load).unwrap();
        let mut sentinel = 0u32;
        game_engine::system::Xfer::xfer_unsigned_int(&mut load, &mut sentinel).unwrap();
        assert_eq!(sentinel, 0xDEADBEEF);
    }
    assert_eq!(payload(&mut machine, 10), 0x76543210);
    assert_eq!(payload(&mut machine, 20), 0xFEDCBA98);
    assert_eq!(payload(&mut untouched, 10), 101);
    assert_eq!(payload(&mut untouched, 20), 202);
    assert!(Arc::ptr_eq(&machine.get_owner().unwrap(), &first));
    assert_eq!(machine.get_current_state_id(), Some(10));
    assert_eq!(machine.get_goal_object_id(), 0x1234ABCD);
    assert_eq!(machine.get_goal_position(), Coord3D::new(2.5, -3.5, 7.0));
    assert!(machine.is_locked());
    assert_eq!(machine.init_default_state(), StateReturnType::Failure);
    assert_no_callbacks(&callbacks);
    assert_eq!(get_game_logic_random_seed_state(), seed);
    // Ordinary saves retain the C++ release layout (one active state).
    let mut expected = prefix(10, false);
    append_u32(&mut expected, 0x76543210);
    append_tail(&mut expected);
    assert_eq!(save(&mut machine), expected);
}
#[test]
fn all_states_count_mismatch_errors_before_any_state_payload_or_goal() {
    for count in [-1, 0, 1, 3, i32::MAX] {
        let (mut machine, callbacks) = machine_with_owner_id(0x7AF12010, [1, 2]);
        let mut reader = Cursor::new(all_states_fixture(count, [10, 20]));
        let error = machine
            .xfer(&mut XferLoad::new(&mut reader, 1))
            .unwrap_err();
        assert!(
            error.to_string().contains("state count mismatch"),
            "{error}"
        );
        assert_eq!(reader.position(), 18);
        assert_eq!(payload(&mut machine, 10), 1);
        assert_eq!(payload(&mut machine, 20), 2);
        assert_eq!(machine.get_goal_object_id(), crate::common::INVALID_ID);
        assert_no_callbacks(&callbacks);
    }
}
#[test]
fn all_states_id_mismatch_errors_before_the_mismatched_payload() {
    for ids in [[20, 10], [99, 20], [10, 10]] {
        let (mut machine, callbacks) = machine_with_owner_id(0x7AF12010, [1, 2]);
        let mut reader = Cursor::new(all_states_fixture(2, ids));
        let error = machine
            .xfer(&mut XferLoad::new(&mut reader, 1))
            .unwrap_err();
        assert!(error.to_string().contains("state ID mismatch"), "{error}");
        assert_eq!(reader.position(), if ids[0] == 10 { 30 } else { 22 });
        assert_eq!(payload(&mut machine, 20), 2);
        assert_eq!(machine.get_goal_object_id(), crate::common::INVALID_ID);
        assert_no_callbacks(&callbacks);
    }
}
#[test]
fn null_save_serializes_invalid_then_heals_default_without_state_entry() {
    let (mut machine, callbacks) = machine_with_owner_id(0x7AF12010, [1, 2]);
    assert_eq!(machine.get_current_state_id(), None);
    let first = save(&mut machine);
    assert_eq!(
        u32::from_le_bytes(first[9..13].try_into().unwrap()),
        INVALID_STATE_ID
    );
    assert_eq!(machine.get_current_state_id(), Some(20));
    let second = save(&mut machine);
    assert_eq!(u32::from_le_bytes(second[9..13].try_into().unwrap()), 20);
    assert_eq!(&first[13..], &second[13..]);
    assert_no_callbacks(&callbacks);
}
#[test]
fn retail_load_unknown_state_recovers_default_without_entry_and_preserves_tail() {
    for current in [INVALID_STATE_ID, 1234567] {
        let (mut machine, callbacks) = machine_with_owner_id(0x7AF12010, [1, 2]);
        let mut bytes = prefix(current, false);
        append_u32(&mut bytes, 0xAABBCCDD);
        append_tail(&mut bytes);
        append_u32(&mut bytes, 0xDEADBEEF);
        let mut reader = Cursor::new(bytes);
        {
            let mut load = XferLoad::new(&mut reader, 1);
            machine.xfer(&mut load).unwrap();
            let mut sentinel = 0;
            game_engine::system::Xfer::xfer_unsigned_int(&mut load, &mut sentinel).unwrap();
            assert_eq!(sentinel, 0xDEADBEEF);
        }
        assert_eq!(machine.get_current_state_id(), Some(20));
        assert_eq!(payload(&mut machine, 20), 0xAABBCCDD);
        assert_eq!(machine.get_goal_object_id(), 0x1234ABCD);
        assert_no_callbacks(&callbacks);
    }
}

#[derive(Debug)]
struct EmptyPayloadState {
    id: StateId,
    snapshot_calls: u32,
}
impl StateImplementation for EmptyPayloadState {
    fn get_id(&self) -> StateId {
        self.id
    }
    fn set_id(&mut self, id: StateId) {
        self.id = id;
    }
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }
    fn xfer_snapshot(&mut self, _: &mut dyn crate::common::xfer::Xfer) -> Result<(), String> {
        self.snapshot_calls += 1;
        Ok(())
    }
}
fn snapshot_calls(machine: &mut StateMachine, id: StateId) -> u32 {
    machine
        .get_state_mut(id)
        .unwrap()
        .as_ref()
        .as_any()
        .downcast_ref::<PayloadState>()
        .unwrap()
        .snapshot_calls
}
#[test]
fn all_states_dispatches_empty_and_nonempty_hooks_once_in_map_order() {
    let (mut machine, callbacks) = machine_with_owner_id(0x7AF12010, [1, 2]);
    machine.define_state(
        5,
        Box::new(EmptyPayloadState {
            id: 5,
            snapshot_calls: 0,
        }),
        None,
        None,
        None,
    );
    let mut bytes = prefix(10, true);
    bytes.extend_from_slice(&3i32.to_le_bytes());
    append_u32(&mut bytes, 5); // Empty snapshot still has a state ID and a hook.
    for (id, value) in [(10, 0x76543210), (20, 0xFEDCBA98)] {
        append_u32(&mut bytes, id);
        append_u32(&mut bytes, value);
    }
    append_tail(&mut bytes);
    append_u32(&mut bytes, 0xDEADBEEF);
    let mut reader = Cursor::new(bytes);
    {
        let mut load = XferLoad::new(&mut reader, 1);
        machine.xfer(&mut load).unwrap();
        let mut sentinel = 0;
        game_engine::system::Xfer::xfer_unsigned_int(&mut load, &mut sentinel).unwrap();
        assert_eq!(sentinel, 0xDEADBEEF);
    }
    assert_eq!(snapshot_calls(&mut machine, 10), 1);
    assert_eq!(snapshot_calls(&mut machine, 20), 1);
    assert_eq!(
        machine
            .get_state_mut(5)
            .unwrap()
            .as_ref()
            .as_any()
            .downcast_ref::<EmptyPayloadState>()
            .unwrap()
            .snapshot_calls,
        1
    );
    assert_eq!(payload(&mut machine, 10), 0x76543210);
    assert_eq!(payload(&mut machine, 20), 0xFEDCBA98);
    assert_no_callbacks(&callbacks);
}
#[test]
fn all_states_truncated_payload_propagates_xfer_error_before_goal_tail() {
    let (mut machine, callbacks) = machine_with_owner_id(0x7AF12010, [1, 2]);
    let mut bytes = all_states_fixture(2, [10, 20]);
    bytes.truncate(26); // First state restored; missing the next ID and body.
    let mut reader = Cursor::new(bytes);
    assert!(machine.xfer(&mut XferLoad::new(&mut reader, 1)).is_err());
    assert_eq!(payload(&mut machine, 10), 0x76543210);
    assert_eq!(payload(&mut machine, 20), 2);
    assert_eq!(machine.get_goal_object_id(), crate::common::INVALID_ID);
    assert_no_callbacks(&callbacks);
}

#[test]
fn load_without_current_or_default_state_errors_before_snapshot_flag() {
    for all_states in [false, true] {
        let mut machine = StateMachine::new_with_owner_id(0x7AF12010, "empty map");
        let mut bytes = prefix(10, all_states);
        bytes.extend_from_slice(&0i32.to_le_bytes());
        append_tail(&mut bytes);
        let mut reader = Cursor::new(bytes);
        let error = machine
            .xfer(&mut XferLoad::new(&mut reader, 1))
            .unwrap_err();
        assert!(
            error.to_string().contains("has no current/default state"),
            "{error}"
        );
        assert_eq!(
            reader.position(),
            13,
            "C++ internalGetState throws before snapshotAllStates"
        );
        assert_eq!(machine.get_current_state_id(), None);
        assert_eq!(machine.get_goal_object_id(), crate::common::INVALID_ID);
    }
}
