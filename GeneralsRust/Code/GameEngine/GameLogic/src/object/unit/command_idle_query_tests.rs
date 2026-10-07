//! C++ AIUpdate.cpp:2875,3095,3171,3330. Real command/query dispatcher;
//! inert state bodies isolate borrowing and routing from unfinished movement.
use super::*;
use crate::state_machine::{StateImplementation, StateMachine, StateReturnType};

#[derive(Debug)]
struct NoopState {
    idle: bool,
}
impl StateImplementation for NoopState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Continue
    }
    fn is_idle(&self) -> bool {
        self.idle
    }
}
fn child(name: &str) -> bool {
    #[cfg(not(target_arch = "wasm32"))]
    {
        matches!(
            crate::test_process::run_bounded(
                name.strip_prefix("gamelogic::").unwrap_or(name),
                "GENERALS_COMMAND_IDLE_QUERY_CHILD"
            ),
            crate::test_process::TestProcess::Child
        )
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = name;
        true
    }
}
fn fixture(
    initial: AIStateType,
    idle_virtual: bool,
) -> (Arc<RwLock<Object>>, Arc<RwLock<Unit>>, UnitAIUpdate) {
    let (owner, unit, ai) =
        unit_ai_update_with_primary_weapon(0x7af10103, Coord3D::new(30.0, 30.0, 0.0), 100.0);
    let machine = ai.ai_state_machine.as_ref().unwrap();
    {
        let mut machine = machine.lock().unwrap();
        machine.base = StateMachine::new(Some(Arc::downgrade(&owner)), "query routing witness");
        for state in [
            AIStateType::Idle,
            AIStateType::Busy,
            AIStateType::MoveTo,
            AIStateType::FollowPath,
        ] {
            machine.base.define_state(
                state as u32,
                Box::new(NoopState {
                    idle: state == AIStateType::Idle && idle_virtual,
                }),
                None,
                None,
                None,
            );
        }
        assert_eq!(
            machine.base.set_current_state(initial as u32),
            StateReturnType::Continue
        );
        machine.set_goal_position(Coord3D::new(40.0, 50.0, 0.0));
    }
    (owner, unit, ai)
}
#[test]
fn explicit_idle_id_precedes_virtual_classification_cpp() {
    if !child(concat!(
        module_path!(),
        "::explicit_idle_id_precedes_virtual_classification_cpp"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let (_owner, unit, ai) = fixture(AIStateType::Idle, false);
    let mut unit = unit.write().unwrap();
    unit.movement_state = MovementState::Moving;
    unit.target_position = Some(Coord3D::new(90.0, 91.0, 0.0));
    assert!(
        ai.is_idle(),
        "C++ returns true for AI_IDLE before virtual classification"
    );
    assert!(ai.is_idle_unrestricted());
    assert!(!ai.is_moving());
}
#[test]
fn native_idle_query_does_not_reborrow_held_unit() {
    if !child(concat!(
        module_path!(),
        "::native_idle_query_does_not_reborrow_held_unit"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let (_owner, unit, ai) = fixture(AIStateType::Idle, true);
    let _unit = unit.write().unwrap();
    assert!(ai.is_idle());
    assert!(ai.is_idle_unrestricted());
}
#[test]
fn ai_move_command_borrows_busy_machine_and_enters_temporary_move() {
    if !child(concat!(
        module_path!(),
        "::ai_move_command_borrows_busy_machine_and_enters_temporary_move"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    with_flat_pathfind_map(|| {
        let (_owner, _unit, mut ai) = fixture(AIStateType::Busy, false);
        let mut command = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::MoveToPosition,
            CommandSourceType::FromAi,
        );
        command.pos = Coord3D::new(80.0, 90.0, 0.0);
        ai.data.blocked_frames = 123;
        ai.data.is_blocked = true;
        ai.data.blocked_and_stuck = true;
        ai.execute_command(&command).unwrap();
        assert_eq!(ai.data.blocked_frames, 0);
        assert!(!ai.data.is_blocked);
        assert!(!ai.data.blocked_and_stuck);
        let machine = ai.ai_state_machine.as_ref().unwrap().lock().unwrap();
        assert_eq!(
            machine.get_current_state_id(),
            Some(AIStateType::Busy as u32)
        );
        assert_eq!(
            machine.get_temporary_state(),
            Some(AIStateType::MoveTo as u32)
        );
        assert_eq!(machine.get_goal_position(), Some(command.pos));
    });
}
fn append(initial: AIStateType, moving: bool, waiting: bool, expected: &[Coord3D]) {
    let (_owner, _unit, mut ai) = fixture(initial, true);
    ai.data.cpp_is_moving = moving;
    ai.data.waiting_for_path = waiting;
    let mut command = crate::ai::AiCommandParams::new(
        crate::ai::AiCommandType::FollowPathAppend,
        CommandSourceType::FromPlayer,
    );
    command.pos = Coord3D::new(80.0, 90.0, 0.0);
    ai.execute_command(&command).unwrap();
    let machine = ai.ai_state_machine.as_ref().unwrap().lock().unwrap();
    assert_eq!(
        machine.get_current_state_id(),
        Some(AIStateType::FollowPath as u32)
    );
    assert_eq!(machine.goal_path, expected);
    assert_eq!(machine.get_goal_position(), Some(command.pos));
}
#[test]
fn idle_path_append_does_not_reborrow_unit_and_uses_single_new_point() {
    if !child(concat!(
        module_path!(),
        "::idle_path_append_does_not_reborrow_unit_and_uses_single_new_point"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    append(
        AIStateType::Idle,
        false,
        false,
        &[Coord3D::new(80.0, 90.0, 0.0)],
    );
}
#[test]
fn busy_without_locomotor_goal_is_not_effectively_moving_cpp() {
    if !child(concat!(
        module_path!(),
        "::busy_without_locomotor_goal_is_not_effectively_moving_cpp"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    append(
        AIStateType::Busy,
        false,
        false,
        &[Coord3D::new(80.0, 90.0, 0.0)],
    );
}
#[test]
fn movement_and_waiting_append_after_current_goal_cpp() {
    if !child(concat!(
        module_path!(),
        "::movement_and_waiting_append_after_current_goal_cpp"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let expected = [Coord3D::new(40.0, 50.0, 0.0), Coord3D::new(80.0, 90.0, 0.0)];
    append(AIStateType::Busy, true, false, &expected);
    append(AIStateType::Idle, false, true, &expected);
}

#[test]
fn waiting_query_reads_owned_flag_without_clock_or_unit_loans() {
    if !child(concat!(
        module_path!(),
        "::waiting_query_reads_owned_flag_without_clock_or_unit_loans"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let (_owner, unit, mut ai) = fixture(AIStateType::Busy, false);
    ai.data.queue_for_path_frame = u32::MAX;
    ai.data.waiting_for_path = false;
    let _unit = unit.write().unwrap();
    let _clock = crate::system::game_logic::get_game_logic().lock().unwrap();
    assert!(
        !ai.is_waiting_for_path(),
        "C++ flag is independent of queue deadline"
    );
    ai.data.waiting_for_path = true;
    assert!(ai.is_waiting_for_path());
}
