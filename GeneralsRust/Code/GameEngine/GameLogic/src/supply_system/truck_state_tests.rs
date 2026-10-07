//! Borrowed family dispatch and the pre-migration generic Xfer envelope.
use super::*;
use crate::common::Coord3D;
use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
use std::io::Cursor;

#[derive(Debug)]
struct DrivingAi {
    idle: bool,
    state: Option<u32>,
    command: Option<AiCommandType>,
    source: CommandSourceType,
    idle_reads: std::sync::atomic::AtomicUsize,
}
impl DrivingAi {
    fn new(idle: bool) -> Self {
        Self {
            idle,
            state: None,
            command: None,
            source: CommandSourceType::FromAi,
            idle_reads: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}
impl AIUpdateInterface for DrivingAi {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        !self.idle
    }
    fn is_idle(&self) -> bool {
        self.idle_reads
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        self.idle
    }
    fn set_movement_target(&mut self, _: &Coord3D) -> Result<(), String> {
        Ok(())
    }
    fn get_current_state_id(&self) -> Option<u32> {
        self.state
    }
    fn get_current_command(&self) -> Option<AiCommandType> {
        self.command
    }
    fn get_last_command_source(&self) -> CommandSourceType {
        self.source
    }
}

#[test]
fn supply_constructor_is_inert_and_same_id_drivers_borrow_distinct_trucks() {
    let mut a = SupplyTruckAIUpdate::new(SupplyTruckAIUpdateData::default(), 0x70AC_0001, 0);
    let mut b = SupplyTruckAIUpdate::new(SupplyTruckAIUpdateData::default(), 0x70AC_0001, 0);
    a.force_busy_state = true;
    b.force_busy_state = true;
    let mut ai_a = DrivingAi::new(true);
    let mut ai_b = DrivingAi::new(false);
    assert_eq!(
        ai_a.idle_reads.load(std::sync::atomic::Ordering::Relaxed),
        0
    );
    assert!(a.state_machine.is_none());
    assert_eq!(
        a.update_with_ai(&mut ai_a, false),
        StateReturnType::Continue
    );
    assert_eq!(a.get_state(), SupplyTruckState::Idle);
    assert!(
        !a.force_busy_state,
        "Busy entry clears only the current owner's latch"
    );
    assert!(b.force_busy_state);
    assert!(b.state_machine.is_none());
    assert_eq!(
        b.update_with_ai(&mut ai_b, false),
        StateReturnType::Continue
    );
    assert_eq!(b.get_state(), SupplyTruckState::Busy);
    assert!(!b.force_busy_state);
    assert_eq!(a.get_state(), SupplyTruckState::Idle);
}

#[test]
fn supply_docking_predicate_reads_actual_ai_state_instead_of_last_command() {
    let mut truck = SupplyTruckAIUpdate::new(SupplyTruckAIUpdateData::default(), 0x70AC_0002, 0);
    let mut ai = DrivingAi::new(false);
    ai.command = Some(AiCommandType::Dock);
    truck.update_with_ai(&mut ai, false);
    assert_eq!(
        truck.get_state(),
        SupplyTruckState::Busy,
        "a stale Dock command is not AI_DOCK (CPP706–724)"
    );
    ai.command = Some(AiCommandType::MoveToPosition);
    ai.state = Some(crate::ai::states::AIStateType::Dock as u32);
    truck.force_wanting_state = true;
    truck.update_with_ai(&mut ai, false);
    assert_eq!(truck.get_state(), SupplyTruckState::Docking);
    assert!(
        !truck.force_wanting_state,
        "Docking onEnter clears the live latch"
    );
}

#[test]
fn supply_ordered_forced_busy_precedes_wanting_and_idle_precedes_docking() {
    let mut truck = SupplyTruckAIUpdate::new(SupplyTruckAIUpdateData::default(), 0x70AC_0003, 0);
    let mut ai = DrivingAi::new(true);
    truck.update_with_ai(&mut ai, false);
    assert_eq!(truck.get_state(), SupplyTruckState::Idle);
    truck.force_busy_state = true;
    truck.force_wanting_state = true;
    truck.update_with_ai(&mut ai, false);
    // Idle -> Busy (clears busy) -> Idle -> Wanting (clears wanting), synchronously.
    assert_eq!(truck.get_state(), SupplyTruckState::Wanting);
    assert!(!truck.force_busy_state);
    assert!(!truck.force_wanting_state);
    let mut machine = SupplyTruckStateMachine::new(0x70AC_0004);
    ai.state = Some(crate::ai::states::AIStateType::Dock as u32);
    // Busy's idle predicate wins first, then Idle's docking predicate runs.
    machine.update(&mut truck, &mut ai, false);
    assert_eq!(machine.current_state_id(), Some(ST_DOCKING));
}

#[derive(Debug)]
struct EmptyState(u32);
impl crate::state_machine::StateImplementation for EmptyState {
    fn get_id(&self) -> u32 {
        self.0
    }
    fn set_id(&mut self, id: u32) {
        self.0 = id;
    }
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }
    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if matches!(self.0, ST_WANTING | ST_REGROUPING | ST_DOCKING) {
            let mut version = 1;
            xfer.xfer_version(&mut version, 1)
                .map_err(|error| error.to_string())?;
        }
        Ok(())
    }
}

#[test]
fn owned_supply_xfer_matches_real_generic_machine_and_restores_without_callbacks() {
    // Compare the real old serializer, not a second handwritten byte encoder.
    let mut old = crate::state_machine::StateMachine::new_with_owner_id(
        INVALID_ID,
        "SupplyTruckStateMachine",
    );
    for id in [ST_BUSY, ST_IDLE, ST_WANTING, ST_REGROUPING, ST_DOCKING] {
        old.define_state(
            id,
            Box::new(EmptyState(id)),
            Some(ST_BUSY),
            Some(ST_BUSY),
            None,
        );
    }
    old.init_default_state();
    let mut old_bytes = Vec::new();
    old.xfer(&mut XferSave::new(Cursor::new(&mut old_bytes), 1))
        .unwrap();
    let mut owned = SupplyTruckStateMachine::new(INVALID_ID);
    owned.state = Some(SupplyTruckState::Busy);
    owned.default_state_inited = true;
    let mut bytes = Vec::new();
    owned
        .xfer(&mut XferSave::new(Cursor::new(&mut bytes), 1))
        .unwrap();
    assert_eq!(
        bytes, old_bytes,
        "initialized StateMachine envelope must remain exact"
    );
    assert_eq!(bytes.len(), 32);
    let mut restored = SupplyTruckStateMachine::new(INVALID_ID);
    restored
        .xfer(&mut XferLoad::new(Cursor::new(bytes.as_slice()), 1))
        .unwrap();
    assert_eq!(restored.current_state_id(), Some(ST_BUSY));
    assert!(restored.default_state_inited);
    let mut round_trip = Vec::new();
    restored
        .xfer(&mut XferSave::new(Cursor::new(&mut round_trip), 1))
        .unwrap();
    assert_eq!(round_trip, bytes);
    for id in [
        ST_IDLE,
        ST_BUSY,
        ST_WANTING,
        ST_REGROUPING,
        ST_DOCKING,
        crate::state_machine::INVALID_STATE_ID,
        0xDEAD_BEEF,
    ] {
        for snapshot_all in [false, true] {
            let mut input = bytes.clone();
            input[9..13].copy_from_slice(&id.to_le_bytes());
            input[13] = u8::from(snapshot_all);
            if snapshot_all {
                input.splice(14..14, all_states_table());
            } else if matches!(id, ST_WANTING | ST_REGROUPING | ST_DOCKING) {
                // These three C++ states have a one-byte v1 payload (header44,62,79).
                input.insert(14, 1);
            }
            old.xfer(&mut XferLoad::new(Cursor::new(input.as_slice()), 1))
                .unwrap();
            restored
                .xfer(&mut XferLoad::new(Cursor::new(input.as_slice()), 1))
                .unwrap();
            assert_eq!(restored.current_state_id(), old.get_current_state_id());
            let mut old_restored = Vec::new();
            let mut owned_restored = Vec::new();
            old.xfer(&mut XferSave::new(Cursor::new(&mut old_restored), 1))
                .unwrap();
            restored
                .xfer(&mut XferSave::new(Cursor::new(&mut owned_restored), 1))
                .unwrap();
            assert_eq!(owned_restored, old_restored);
        }
    }
}

// C++ StateMachine.cpp799-868; SupplyTruckAIUpdate.h44,62,79 has v1
// payloads for Wanting/Regrouping/Docking. Busy/Idle .cpp278,322 are empty.
fn all_states_table() -> Vec<u8> {
    let mut table = 5i32.to_le_bytes().to_vec();
    for id in [ST_IDLE, ST_BUSY, ST_WANTING, ST_REGROUPING, ST_DOCKING] {
        table.extend(id.to_le_bytes());
        if matches!(id, ST_WANTING | ST_REGROUPING | ST_DOCKING) {
            table.push(1);
        }
    }
    table
}
fn all_states_packet() -> Vec<u8> {
    let mut bytes = vec![1];
    for value in [37u32, ST_BUSY, ST_WANTING] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.push(1);
    bytes.extend(all_states_table());
    bytes.extend(0x1234ABCDu32.to_le_bytes());
    for value in [2.5f32, -3.5, 7.0] {
        bytes.extend(value.to_le_bytes());
    }
    bytes.extend([1, 1]);
    assert_eq!(bytes.len(), 59);
    bytes
}
#[test]
fn all_state_table_loads_before_supply_tail_and_resaves_retail_without_callbacks() {
    let input = all_states_packet();
    let mut owned = SupplyTruckStateMachine::new(INVALID_ID);
    owned
        .xfer(&mut XferLoad::new(Cursor::new(input.as_slice()), 1))
        .unwrap();
    assert_eq!(owned.current_state_id(), Some(ST_WANTING));
    assert_eq!(owned.sleep_till, 37);
    assert_eq!(owned.goal_object_id, 0x1234ABCD);
    assert_eq!(owned.goal_position, Coord3D::new(2.5, -3.5, 7.0));
    assert!(owned.locked);
    assert!(owned.default_state_inited);
    let mut saved = Vec::new();
    owned
        .xfer(&mut XferSave::new(Cursor::new(&mut saved), 1))
        .unwrap();
    let mut retail = input.clone();
    retail[13] = 0;
    retail.drain(14..41);
    retail.insert(14, 1);
    assert_eq!(
        saved, retail,
        "retail save disables all-state mode but preserves the loaded tail exactly"
    );
}
#[test]
fn supply_all_state_count_mismatch_fails_before_tail() {
    for count in [-1i32, 0, 4, 6] {
        let mut input = all_states_packet();
        input[14..18].copy_from_slice(&count.to_le_bytes());
        let mut cursor = Cursor::new(input);
        let mut owned = SupplyTruckStateMachine::new(INVALID_ID);
        let result = owned.xfer(&mut XferLoad::new(&mut cursor, 1));
        assert!(result.is_err(), "must reject signed count {count}");
        assert_eq!(cursor.position(), 18);
    }
}
#[test]
fn supply_all_state_id_mismatch_fails_at_exact_id() {
    for (index, start) in [18, 22, 26, 31, 36].into_iter().enumerate() {
        let mut input = all_states_packet();
        input[start..start + 4].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
        let mut cursor = Cursor::new(input);
        let mut owned = SupplyTruckStateMachine::new(INVALID_ID);
        let result = owned.xfer(&mut XferLoad::new(&mut cursor, 1));
        assert!(
            result.is_err(),
            "must reject mismatched table index {index}"
        );
        assert_eq!(cursor.position(), (start + 4) as u64);
    }
}
#[test]
fn supply_missing_current_and_default_fails_before_selector_without_erasing_state() {
    let mut input = all_states_packet();
    input[5..9].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
    input[9..13].copy_from_slice(&0xDEAD_BEEFu32.to_le_bytes());
    let mut cursor = Cursor::new(input);
    let mut owned = SupplyTruckStateMachine::new(INVALID_ID);
    owned.state = Some(SupplyTruckState::Busy);
    assert!(owned.xfer(&mut XferLoad::new(&mut cursor, 1)).is_err());
    assert_eq!(
        cursor.position(),
        13,
        "C++ internalGetState throws before reading snapshotAllStates"
    );
    assert_eq!(owned.current_state_id(), Some(ST_BUSY));
}
#[test]
fn supply_null_retail_save_writes_invalid_then_heals_default_without_entry() {
    let mut owned = SupplyTruckStateMachine::new(INVALID_ID);
    let mut first = Vec::new();
    owned
        .xfer(&mut XferSave::new(Cursor::new(&mut first), 1))
        .unwrap();
    assert_eq!(
        &first[9..13],
        &crate::state_machine::INVALID_STATE_ID.to_le_bytes()
    );
    assert_eq!(owned.current_state_id(), Some(ST_BUSY));
    assert!(
        !owned.default_state_inited,
        "healing the state reference is not onEnter"
    );
    let mut second = Vec::new();
    owned
        .xfer(&mut XferSave::new(Cursor::new(&mut second), 1))
        .unwrap();
    assert_eq!(&second[9..13], &ST_BUSY.to_le_bytes());
    assert_eq!(first.len(), 32);
    assert_eq!(second.len(), 32);
    assert_eq!(&first[13..], &second[13..]);
}

#[test]
fn supply_all_state_future_payload_version_fails_at_exact_state() {
    for offset in [30, 35, 40] {
        let mut input = all_states_packet();
        input[offset] = 2;
        let mut cursor = Cursor::new(input);
        let mut owned = SupplyTruckStateMachine::new(INVALID_ID);
        assert!(owned.xfer(&mut XferLoad::new(&mut cursor, 1)).is_err());
        assert_eq!(cursor.position(), (offset + 1) as u64);
    }
}
