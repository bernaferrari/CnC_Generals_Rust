#![allow(deprecated, unused_imports, dead_code)]

use super::*;

use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::dock::AIDockMachine;
use crate::ai::group::AIGroup;
use crate::ai::guard::{AIGuardMachine, GuardStateType};
use crate::ai::guard_retaliate::AIGuardRetaliateMachine;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::pathfind::Path;
use crate::ai::squad::Squad;
use crate::ai::tn_guard::{AITNGuardMachine, TNGuardStateType};
use crate::ai::{
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter, the_ai,
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::compat::{ClassicState, legacy_transition, register_classic_state};
use crate::control_bar::get_control_bar_bridge;
use crate::damage::DamageInfo;
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, get_game_logic_random_value};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE, PhysicsBehaviorExt,
};
use crate::object::production::AIFreeToExitType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::GRAVITY;
use crate::player::PlayerType;
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::engine::get_script_engine;
use crate::state_machine::*;
use crate::team::{Team, TeamID, TheTeamFactory};
use crate::terrain::get_terrain_logic;
use crate::waypoint::{Waypoint, WaypointId};
use crate::weapon::{
    NO_MAX_SHOTS_LIMIT, Weapon, WeaponChoiceCriteria, WeaponLockType, WeaponSlotType, WeaponStatus,
};
use game_engine::common::system::xfer_load::XferLoad;
use game_engine::common::system::xfer_save::XferSave;
use game_engine::common::system::{GeometryType, Snapshotable, Xfer};

use crate::common::INVALID_ID;

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicI32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Mutex as StdMutex, OnceLock, RwLock, Weak};

fn test_guard() -> std::sync::MutexGuard<'static, ()> {
    static TEST_LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
    TEST_LOCK
        .get_or_init(|| StdMutex::new(()))
        .lock()
        .unwrap_or_else(|err| err.into_inner())
}

fn set_frame(frame: u64) {
    let mut logic = crate::system::game_logic::get_game_logic()
        .lock()
        .expect("game logic lock poisoned");
    logic.set_current_frame(frame);
}

fn unique_missing_waypoint_name() -> String {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    format!("__codex_missing_waypoint_{id}__")
}

fn unique_polygon_trigger_id() -> i32 {
    static NEXT_ID: AtomicI32 = AtomicI32::new(1_000_000);
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

#[test]
fn add_to_goal_path_deduplicates_terminal_point() {
    let _guard = test_guard();
    let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-path");
    let p0 = Coord3D::new(1.0, 2.0, 3.0);
    let p1 = Coord3D::new(4.0, 5.0, 6.0);

    machine.add_to_goal_path(&p0);
    machine.add_to_goal_path(&p0);
    machine.add_to_goal_path(&p1);

    assert_eq!(machine.get_goal_path_size(), 2);
    assert_eq!(machine.get_goal_path_position(0), Some(&p0));
    assert_eq!(machine.get_goal_path_position(1), Some(&p1));
}

#[test]
fn set_state_returns_base_state_machine_result() {
    let _guard = test_guard();
    let mut expected_machine =
        AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-state-expected");
    let mut actual_machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-state-actual");

    let expected = expected_machine
        .base
        .set_current_state(MACHINE_DONE_STATE_ID);
    let actual = actual_machine.set_state(MACHINE_DONE_STATE_ID);

    assert_eq!(actual, expected);
}

#[test]
fn clear_uses_base_clear_semantics() {
    let _guard = test_guard();
    let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-clear");
    assert!(machine.get_current_state_id().is_some());

    machine.clear();

    assert_eq!(machine.get_current_state_id(), None);
    assert_eq!(machine.get_goal_path_size(), 0);
}

#[test]
fn set_goal_squad_copies_instead_of_aliasing() {
    let _guard = test_guard();
    let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-squad");
    let source = Arc::new(Mutex::new(Squad::new()));

    machine.set_goal_squad(Some(source.clone()));

    let stored = machine
        .get_goal_squad()
        .expect("goal squad should be set")
        .clone();
    assert!(!Arc::ptr_eq(&stored, &source));
}

#[test]
fn waypoint_load_with_missing_name_clears_existing_goal_waypoint() {
    let _guard = test_guard();
    let missing_name = unique_missing_waypoint_name();

    let mut source = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-waypoint-source");
    source.set_goal_waypoint(Some(Arc::new(Waypoint::new(
        777_001,
        Coord3D::new(10.0, 20.0, 30.0),
        missing_name.clone(),
    ))));

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer(&mut saver)
            .expect("source state machine should serialize");
    }

    let mut loaded = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-waypoint-loaded");
    loaded.set_goal_waypoint(Some(Arc::new(Waypoint::new(
        777_002,
        Coord3D::new(-1.0, -2.0, -3.0),
        "stale-waypoint".to_string(),
    ))));
    assert!(loaded.get_goal_waypoint().is_some());

    let bytes = save_cursor.into_inner();
    let mut loader = XferLoad::new(Cursor::new(bytes), 1);
    loaded
        .xfer(&mut loader)
        .expect("loaded state machine should deserialize");

    assert!(loaded.get_goal_waypoint().is_none());
    assert_eq!(loaded.base.get_goal_waypoint(), None);
}

#[test]
fn ai_do_command_polygon_updates_machine_goal_polygon() {
    let _guard = test_guard();
    let trigger_id = unique_polygon_trigger_id();
    let trigger_name = format!("__codex_test_trigger_{trigger_id}__");
    let trigger = PolygonTrigger::new(
        trigger_id,
        AsciiString::from(trigger_name.as_str()),
        vec![
            ICoord3D::new(0, 0, 0),
            ICoord3D::new(20, 0, 0),
            ICoord3D::new(20, 20, 0),
        ],
    );
    {
        let mut terrain = get_terrain_logic()
            .write()
            .expect("terrain logic write lock poisoned");
        terrain.add_trigger_area(trigger);
    }

    let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-polygon");
    let mut params = AiCommandParams::new(AiCommandType::GuardArea, CommandSourceType::FromScript);
    params.polygon = Some(trigger_id);

    machine
        .ai_do_command(&params)
        .expect("ai_do_command should succeed");

    assert_eq!(
        machine
            .goal_polygon
            .as_ref()
            .map(|polygon| polygon.get_id()),
        Some(trigger_id)
    );
    assert_eq!(
        machine
            .base
            .get_goal_polygon()
            .as_ref()
            .map(|polygon| polygon.get_id()),
        Some(trigger_id)
    );
}

#[test]
fn xfer_roundtrip_preserves_path_squad_temp_and_waypoint_lookup_rules() {
    let _guard = test_guard();
    set_frame(1_000);

    let missing_name = unique_missing_waypoint_name();
    let mut source = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-roundtrip-source");
    let path = vec![Coord3D::new(1.0, 2.0, 3.0), Coord3D::new(4.0, 5.0, 6.0)];
    source.set_goal_path(&path);
    source.set_goal_squad(Some(Arc::new(Mutex::new(Squad::new()))));
    source.set_goal_waypoint(Some(Arc::new(Waypoint::new(
        777_010,
        Coord3D::new(30.0, 40.0, 50.0),
        missing_name,
    ))));

    let temp_ret = source.set_temporary_state(AIStateType::Idle as u32, 45);
    assert_eq!(temp_ret, StateReturnType::Continue);
    let expected_temp_end = source.temporary_state_frame_end;

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer(&mut saver)
            .expect("source state machine should serialize");
    }

    let mut loaded = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-roundtrip-loaded");
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer(&mut loader)
        .expect("loaded state machine should deserialize");

    assert_eq!(loaded.get_goal_path_size(), path.len());
    assert_eq!(loaded.get_goal_path_position(0), Some(&path[0]));
    assert_eq!(loaded.get_goal_path_position(1), Some(&path[1]));
    assert!(loaded.get_goal_squad().is_some());
    assert!(loaded.base.get_goal_squad().is_some());
    assert_eq!(loaded.get_temporary_state(), Some(AIStateType::Idle as u32));
    assert_eq!(loaded.temporary_state_frame_end, expected_temp_end);
    assert!(loaded.get_goal_waypoint().is_none());
    assert_eq!(loaded.base.get_goal_waypoint(), None);
}

#[test]
fn follow_path_state_snapshot_roundtrip_preserves_runtime_fields() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "follow-path-source");
    let mut source = AIFollowPathState::new(&source_machine, false);
    source.path = vec![
        Coord3D::new(2.0, 3.0, 4.0),
        Coord3D::new(5.0, 6.0, 7.0),
        Coord3D::new(8.0, 9.0, 10.0),
    ];
    source.index = 2;
    source.adjust_final = false;
    source.adjust_final_override = false;
    source.retry_count = 3;
    source.ignore_object_id = Some(4242);

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("follow path state should serialize");
    }

    let load_machine = StateMachine::new(None, "follow-path-loaded");
    let mut loaded = AIFollowPathState::new(&load_machine, false);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("follow path state should deserialize");

    assert_eq!(loaded.path.len(), 3);
    assert_eq!(loaded.path[0], Coord3D::new(2.0, 3.0, 4.0));
    assert_eq!(loaded.path[2], Coord3D::new(8.0, 9.0, 10.0));
    assert_eq!(loaded.index, 2);
    assert!(!loaded.adjust_final);
    assert!(!loaded.adjust_final_override);
    assert_eq!(loaded.retry_count, 3);
    assert_eq!(loaded.ignore_object_id, Some(4242));
}

#[test]
fn follow_exit_production_path_snapshot_delegates_to_base_state() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "follow-exit-source");
    let mut source = AIFollowExitProductionPathState::new(&source_machine);
    source.base.path = vec![
        Coord3D::new(11.0, 12.0, 13.0),
        Coord3D::new(14.0, 15.0, 16.0),
    ];
    source.base.index = 1;
    source.base.ignore_object_id = Some(99);

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("follow exit production state should serialize");
    }

    let load_machine = StateMachine::new(None, "follow-exit-loaded");
    let mut loaded = AIFollowExitProductionPathState::new(&load_machine);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("follow exit production state should deserialize");

    assert_eq!(loaded.base.path.len(), 2);
    assert_eq!(loaded.base.path[1], Coord3D::new(14.0, 15.0, 16.0));
    assert_eq!(loaded.base.index, 1);
    assert_eq!(loaded.base.ignore_object_id, Some(99));
}

#[test]
fn pick_up_crate_state_snapshot_roundtrip_preserves_delay_and_goal() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "pickup-crate-source");
    let mut source = AIPickUpCrateState::new(&source_machine);
    source.delay_counter = 2;
    source.goal_position = Coord3D::new(21.0, 22.0, 23.0);
    source.base.goal_position = Coord3D::new(1.0, 1.0, 1.0);

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("pick up crate state should serialize");
    }

    let load_machine = StateMachine::new(None, "pickup-crate-loaded");
    let mut loaded = AIPickUpCrateState::new(&load_machine);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("pick up crate state should deserialize");

    assert_eq!(loaded.delay_counter, 2);
    assert_eq!(loaded.goal_position, Coord3D::new(21.0, 22.0, 23.0));
    assert_eq!(loaded.base.goal_position, Coord3D::new(21.0, 22.0, 23.0));
}

#[test]
fn attack_pursue_state_snapshot_roundtrip_preserves_base_and_runtime_fields() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "attack-pursue-source");
    let mut source = AIAttackPursueTargetState::new(&source_machine, true, true, true);
    source.base.goal_position = Coord3D::new(30.0, 31.0, 32.0);
    source.base.path_goal_position = Coord3D::new(33.0, 34.0, 35.0);
    source.base.path_timestamp = 123;
    source.base.blocked_repath_timestamp = 456;
    source.base.adjust_destinations = false;
    source.base.waiting_for_path = true;
    source.base.goal_layer = 2;
    source.prev_victim_pos = Coord3D::new(40.0, 41.0, 42.0);
    source.approach_timestamp = 789;
    source.follow = false;
    source.attacking_object = false;
    source.stop_if_in_range = true;
    source.is_initial_approach = false;

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("attack pursue state should serialize");
    }

    let load_machine = StateMachine::new(None, "attack-pursue-loaded");
    let mut loaded = AIAttackPursueTargetState::new(&load_machine, true, true, false);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("attack pursue state should deserialize");

    assert_eq!(loaded.base.goal_position, Coord3D::new(30.0, 31.0, 32.0));
    assert_eq!(
        loaded.base.path_goal_position,
        Coord3D::new(33.0, 34.0, 35.0)
    );
    assert_eq!(loaded.base.path_timestamp, 123);
    assert_eq!(loaded.base.blocked_repath_timestamp, 456);
    assert!(!loaded.base.adjust_destinations);
    assert!(loaded.base.waiting_for_path);
    assert_eq!(loaded.base.goal_layer, 2);
    assert_eq!(loaded.prev_victim_pos, Coord3D::new(40.0, 41.0, 42.0));
    assert_eq!(loaded.approach_timestamp, 789);
    assert!(!loaded.follow);
    assert!(!loaded.attacking_object);
    assert!(loaded.stop_if_in_range);
    assert!(!loaded.is_initial_approach);
    assert!(!loaded.force_attacking);
}

#[test]
fn attack_approach_state_snapshot_roundtrip_preserves_base_and_runtime_fields() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "attack-approach-source");
    let mut source = AIAttackApproachTargetState::new(&source_machine, true, true, true);
    source.base.goal_position = Coord3D::new(50.0, 51.0, 52.0);
    source.base.path_goal_position = Coord3D::new(53.0, 54.0, 55.0);
    source.base.path_timestamp = 223;
    source.base.blocked_repath_timestamp = 556;
    source.base.adjust_destinations = false;
    source.base.waiting_for_path = true;
    source.base.goal_layer = 1;
    source.prev_victim_pos = Coord3D::new(60.0, 61.0, 62.0);
    source.approach_timestamp = 889;
    source.follow = false;
    source.attacking_object = false;
    source.stop_if_in_range = true;
    source.is_initial_approach = false;

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("attack approach state should serialize");
    }

    let load_machine = StateMachine::new(None, "attack-approach-loaded");
    let mut loaded = AIAttackApproachTargetState::new(&load_machine, true, true, false);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("attack approach state should deserialize");

    assert_eq!(loaded.base.goal_position, Coord3D::new(50.0, 51.0, 52.0));
    assert_eq!(
        loaded.base.path_goal_position,
        Coord3D::new(53.0, 54.0, 55.0)
    );
    assert_eq!(loaded.base.path_timestamp, 223);
    assert_eq!(loaded.base.blocked_repath_timestamp, 556);
    assert!(!loaded.base.adjust_destinations);
    assert!(loaded.base.waiting_for_path);
    assert_eq!(loaded.base.goal_layer, 1);
    assert_eq!(loaded.prev_victim_pos, Coord3D::new(60.0, 61.0, 62.0));
    assert_eq!(loaded.approach_timestamp, 889);
    assert!(!loaded.follow);
    assert!(!loaded.attacking_object);
    assert!(loaded.stop_if_in_range);
    assert!(!loaded.is_initial_approach);
    assert!(!loaded.force_attacking);
}

#[test]
fn attack_aim_state_snapshot_roundtrip_preserves_runtime_flags() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "attack-aim-source");
    let mut source = AIAttackAimAtTargetState::new(&source_machine, true, true);
    source.can_turn_in_place = true;
    source.set_locomotor = true;

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("attack aim state should serialize");
    }

    let load_machine = StateMachine::new(None, "attack-aim-loaded");
    let mut loaded = AIAttackAimAtTargetState::new(&load_machine, true, false);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("attack aim state should deserialize");

    assert!(loaded.can_turn_in_place);
    assert!(loaded.set_locomotor);
    assert!(!loaded.force_attacking);
    assert!(loaded.attacking_object);
}

#[test]
fn idle_state_snapshot_roundtrip_preserves_runtime_flags() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "idle-source");
    let mut source = AIIdleState::new(&source_machine, false);
    source.initial_sleep_offset = 17;
    source.should_look_for_targets = true;
    source.inited = true;

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("idle state should serialize");
    }

    let load_machine = StateMachine::new(None, "idle-loaded");
    let mut loaded = AIIdleState::new(&load_machine, false);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("idle state should deserialize");

    assert_eq!(loaded.initial_sleep_offset, 17);
    assert!(loaded.should_look_for_targets);
    assert!(loaded.inited);
}

#[test]
fn wander_and_panic_state_snapshot_roundtrip_preserves_runtime_fields() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "wander-source");

    let mut wander = AIWanderState::new(&source_machine);
    wander.wait_frames = 23;
    wander.timer = -4;
    wander.core.group_offset = Coord2D::new(7.0, 8.0);
    wander.core.angle = 1.5;
    wander.core.frames_sleeping = 9;
    wander.core.append_goal_position = true;
    wander.core.goal_position = Coord3D::new(10.0, 11.0, 12.0);

    let mut wander_save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut wander_save_cursor, 1);
        wander
            .xfer_snapshot(&mut saver)
            .expect("wander state should serialize");
    }

    let load_machine = StateMachine::new(None, "wander-loaded");
    let mut wander_loaded = AIWanderState::new(&load_machine);
    let mut wander_loader = XferLoad::new(Cursor::new(wander_save_cursor.into_inner()), 1);
    wander_loaded
        .xfer_snapshot(&mut wander_loader)
        .expect("wander state should deserialize");

    assert_eq!(wander_loaded.wait_frames, 23);
    assert_eq!(wander_loaded.timer, -4);
    assert_eq!(wander_loaded.core.group_offset, Coord2D::new(7.0, 8.0));
    assert_eq!(wander_loaded.core.angle, 1.5);
    assert_eq!(wander_loaded.core.frames_sleeping, 9);
    assert!(wander_loaded.core.append_goal_position);
    assert_eq!(
        wander_loaded.core.goal_position,
        Coord3D::new(10.0, 11.0, 12.0)
    );

    let mut panic_state = AIPanicState::new(&source_machine);
    panic_state.wait_frames = 12;
    panic_state.timer = 6;
    panic_state.core.group_offset = Coord2D::new(-3.0, 4.0);
    panic_state.core.frames_sleeping = 2;

    let mut panic_save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut panic_save_cursor, 1);
        panic_state
            .xfer_snapshot(&mut saver)
            .expect("panic state should serialize");
    }

    let mut panic_loaded = AIPanicState::new(&load_machine);
    let mut panic_loader = XferLoad::new(Cursor::new(panic_save_cursor.into_inner()), 1);
    panic_loaded
        .xfer_snapshot(&mut panic_loader)
        .expect("panic state should deserialize");

    assert_eq!(panic_loaded.wait_frames, 12);
    assert_eq!(panic_loaded.timer, 6);
    assert_eq!(panic_loaded.core.group_offset, Coord2D::new(-3.0, 4.0));
    assert_eq!(panic_loaded.core.frames_sleeping, 2);
}

#[test]
fn exit_states_snapshot_roundtrip_preserves_entry_to_clear() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "exit-source");

    let mut exit_state = AIExitState::new(&source_machine);
    exit_state.entry_to_clear = 1337;
    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        exit_state
            .xfer_snapshot(&mut saver)
            .expect("exit state should serialize");
    }
    let load_machine = StateMachine::new(None, "exit-loaded");
    let mut loaded = AIExitState::new(&load_machine);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("exit state should deserialize");
    assert_eq!(loaded.entry_to_clear, 1337);

    let mut exit_instantly_state = AIExitInstantlyState::new(&source_machine);
    exit_instantly_state.entry_to_clear = 7331;
    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        exit_instantly_state
            .xfer_snapshot(&mut saver)
            .expect("exit instantly state should serialize");
    }
    let mut loaded = AIExitInstantlyState::new(&load_machine);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("exit instantly state should deserialize");
    assert_eq!(loaded.entry_to_clear, 7331);
}

#[test]
fn hunt_state_snapshot_roundtrip_preserves_scan_time_without_machine() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "hunt-source");
    let mut source = AIHuntState::new(&source_machine);
    source.next_enemy_scan_time = 54321;
    source.hunt_machine = None;

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("hunt state should serialize");
    }

    let load_machine = StateMachine::new(None, "hunt-loaded");
    let mut loaded = AIHuntState::new(&load_machine);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("hunt state should deserialize");

    assert_eq!(loaded.next_enemy_scan_time, 54321);
    assert!(loaded.hunt_machine.is_none());
}

#[test]
fn enter_state_snapshot_roundtrip_preserves_base_and_entry_runtime_fields() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "enter-source");
    let mut source = AIEnterState::new(&source_machine);
    source.base.goal_position = Coord3D::new(70.0, 71.0, 72.0);
    source.base.path_goal_position = Coord3D::new(73.0, 74.0, 75.0);
    source.base.path_timestamp = 612;
    source.base.blocked_repath_timestamp = 913;
    source.base.waiting_for_path = true;
    source.base.adjust_destinations = false;
    source.base.goal_layer = 1;
    source.entry_to_clear = 17;
    source.goal_position = Coord3D::new(81.0, 82.0, 83.0);

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("enter state should serialize");
    }

    let load_machine = StateMachine::new(None, "enter-loaded");
    let mut loaded = AIEnterState::new(&load_machine);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("enter state should deserialize");

    assert_eq!(loaded.base.goal_position, Coord3D::new(81.0, 82.0, 83.0));
    assert_eq!(
        loaded.base.path_goal_position,
        Coord3D::new(73.0, 74.0, 75.0)
    );
    assert_eq!(loaded.base.path_timestamp, 612);
    assert_eq!(loaded.base.blocked_repath_timestamp, 913);
    assert!(loaded.base.waiting_for_path);
    assert!(!loaded.base.adjust_destinations);
    assert_eq!(loaded.base.goal_layer, 1);
    assert_eq!(loaded.entry_to_clear, 17);
    assert_eq!(loaded.goal_position, Coord3D::new(81.0, 82.0, 83.0));
}

#[test]
fn rappel_into_state_snapshot_roundtrip_preserves_runtime_fields() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "rappel-source");
    let mut source = AIRappelIntoState::new(&source_machine);
    source.rappel_rate = -21.5;
    source.dest_z = 133.25;
    source.target_is_bldg = true;

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("rappel state should serialize");
    }

    let load_machine = StateMachine::new(None, "rappel-loaded");
    let mut loaded = AIRappelIntoState::new(&load_machine);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("rappel state should deserialize");

    assert_eq!(loaded.rappel_rate, -21.5);
    assert_eq!(loaded.dest_z, 133.25);
    assert!(loaded.target_is_bldg);
}

#[test]
fn combat_drop_state_snapshot_roundtrip_preserves_issued_command() {
    let _guard = test_guard();
    let source_machine = StateMachine::new(None, "combat-drop-source");
    let mut source = AICombatDropState::new(&source_machine);
    source.issued_command = true;

    let mut save_cursor = Cursor::new(Vec::<u8>::new());
    {
        let mut saver = XferSave::new(&mut save_cursor, 1);
        source
            .xfer_snapshot(&mut saver)
            .expect("combat drop state should serialize");
    }

    let load_machine = StateMachine::new(None, "combat-drop-loaded");
    let mut loaded = AICombatDropState::new(&load_machine);
    let mut loader = XferLoad::new(Cursor::new(save_cursor.into_inner()), 1);
    loaded
        .xfer_snapshot(&mut loader)
        .expect("combat drop state should deserialize");

    assert!(loaded.issued_command);
}

#[test]
fn temporary_state_frame_end_uses_saturating_add() {
    let _guard = test_guard();
    set_frame((u32::MAX as u64).saturating_sub(10));

    let mut machine = AIStateMachine::new(Weak::<RwLock<Object>>::new(), "ai-temp");
    let ret = machine.set_temporary_state(AIStateType::Idle as u32, u32::MAX);

    assert_eq!(ret, StateReturnType::Continue);
    assert_eq!(machine.temporary_state_frame_end, u32::MAX);
}

#[test]
fn idle_restake_plan_runs_for_ultra_accurate_and_loco_less() {
    // C++ AIIdleState::doInitIdleState (AIStates.cpp:1323-1347):
    // first updateGoal always runs; ultraAccurate only gates the snap.
    let pos = Coord3D::new(10.0, 20.0, 0.0);
    let ultra = idle_pathfinder_restake_plan(true, true, pos, true);
    assert!(ultra.first_restake, "ultra-accurate units still restake");
    assert!(!ultra.snap, "ultraAccurate gates only the snap");

    let loco_less = idle_pathfinder_restake_plan(true, true, pos, false);
    assert!(
        loco_less.first_restake,
        "loco-less ultraAccurate==false still restakes"
    );
    assert!(loco_less.snap);

    let zero = idle_pathfinder_restake_plan(true, true, Coord3D::new(0.0, 0.0, 0.0), false);
    assert!(!zero.first_restake);
    assert!(!zero.snap);

    let not_idle = idle_pathfinder_restake_plan(false, true, pos, false);
    assert!(!not_idle.first_restake);
}

#[test]
fn fire_weapon_seeds_attack_common_target_only_when_empty() {
    // C++ AIAttackFireWeaponState::onEnter (AIStates.cpp:5153-5156).
    assert!(should_seed_attack_common_target(true, true, INVALID_ID));
    assert!(!should_seed_attack_common_target(true, true, 99));
    assert!(!should_seed_attack_common_target(true, false, INVALID_ID));
    assert!(!should_seed_attack_common_target(false, true, INVALID_ID));
}

#[test]
fn team_change_abort_clears_matching_team_target() {
    // C++ AIAttackState::update (AIStates.cpp:5620-5623 / 5605-5607).
    assert!(team_target_matches_victim(55, 55));
    assert!(!team_target_matches_victim(INVALID_ID, 55));
    assert!(!team_target_matches_victim(55, 99));
    let mut team = Team::new(AsciiString::from("AbortTeam"), 7);
    assert_eq!(team.get_team_target_object(), INVALID_ID);
    clear_team_target_object_if_victim(&mut team, 55);
    assert_eq!(team.get_team_target_object(), INVALID_ID);
}

#[test]
fn nested_attack_machine_picks_up_parent_goal_change() {
    // C++ AIAttackState::update (AIStates.cpp:5629-5633).
    let mut machine = AttackStateMachine::new(Weak::new(), "nested-goal", false, true, false);
    machine.set_goal_object(Some(11));
    assert_eq!(machine.get_goal_object_id(), 11);
    forward_parent_goal_to_nested_machine(&mut machine, 22);
    assert_eq!(machine.get_goal_object_id(), 22);
    forward_parent_goal_to_nested_machine(&mut machine, 22);
    assert_eq!(machine.get_goal_object_id(), 22);
    forward_parent_goal_to_nested_machine(&mut machine, INVALID_ID);
    assert_eq!(machine.get_goal_object_id(), 22);
}

#[test]
fn attack_on_exit_clears_leech_range_mode() {
    // C++ AIAttackState::onExit (AIStates.cpp:5690).
    let _guard = test_guard();
    let id = 9_101_001;
    let object = Arc::new(RwLock::new(Object::new_test(id, 100.0)));
    OBJECT_REGISTRY.register_object(id, &object);
    {
        let mut owner = object.write().expect("owner write");
        let mut tmpl = crate::weapon::WeaponTemplate::new("LeechTest".to_string());
        tmpl.leech_range_weapon = true;
        let mut set = crate::weapon::WeaponTemplateSet::new();
        set.weapon_templates[0] = Some(Arc::new(tmpl));
        owner.weapon_set.add_weapon_template_set(set);
        let flags = crate::weapon::WeaponSetFlags::new();
        owner
            .weapon_set
            .update_weapon_set(id, &flags)
            .expect("update weapon set");
        owner
            .weapon_set
            .get_weapon_in_slot_mut(WeaponSlotType::Primary)
            .expect("primary weapon")
            .set_leech_range_active(true);
        assert!(
            owner
                .weapon_set
                .get_weapon_in_slot(WeaponSlotType::Primary)
                .expect("primary")
                .has_leech_range()
        );
    }

    let machine = StateMachine::new(Some(Arc::downgrade(&object)), "leech-exit");
    let mut state = AIAttackObjectState::new(&machine, false, false);
    state
        .classic_on_exit(StateExitType::Normal)
        .expect("classic_on_exit");
    {
        let owner = object.read().expect("owner read");
        let weapon = owner
            .weapon_set
            .get_weapon_in_slot(WeaponSlotType::Primary)
            .expect("primary after exit");
        assert!(
            !weapon.has_leech_range(),
            "onExit must clear leech-range mode"
        );
    }
    OBJECT_REGISTRY.unregister_object(id);
}
