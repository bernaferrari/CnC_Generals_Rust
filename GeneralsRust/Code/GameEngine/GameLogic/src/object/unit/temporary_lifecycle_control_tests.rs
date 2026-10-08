//! Additional installed-state lifecycle controls; no native motion claim.
//! Real factory admission, original registered CppStateAdapter movement bodies.
use super::*;

fn terrain(test: impl FnOnce()) {
    let mut map = crate::system::map_loader::MapData::new();
    map.width = 32;
    map.height = 32;
    map.heightmap = vec![0; 32 * 32];
    map.boundaries = vec![crate::common::ICoord2D::new(32, 32)];
    let mut terrain = crate::terrain::TerrainLogic::new();
    terrain.load_map_data(map);
    let stores = Arc::new(crate::system::engine_stores::EngineStores::new_for_world());
    stores
        .ai()
        .read()
        .unwrap()
        .pathfinder()
        .unwrap()
        .write()
        .unwrap()
        .rebuild_from_terrain(&terrain);
    {
        let handle = stores.ai().read().unwrap().pathfinder().unwrap();
        let pf = handle.read().unwrap();
        assert!(pf.is_map_ready());
        assert!(
            pf.get_cell_type_at(&Coord3D::new(30.0, 30.0, 0.0))
                .is_some()
        );
        assert!(
            pf.get_cell_type_at(&Coord3D::new(120.0, 30.0, 0.0))
                .is_some()
        );
    }
    *crate::terrain::get_terrain_logic().write().unwrap() = terrain;
    crate::system::engine_stores::with_active_stores(&stores, test);
}
fn fixture(
    test: impl FnOnce(Arc<RwLock<crate::object::Object>>, Arc<Mutex<dyn AIUpdateInterface>>),
) {
    terrain(|| {
        definitions();
        let mut factory = ObjectFactory::new();
        let id = create(
            &mut factory,
            "EnterCapacityNoWeapon",
            Coord3D::new(30.0, 30.0, 0.0),
            None,
            ObjectCreationFlags::empty(),
        );
        let (owner, ai) = native_ai(&factory, id);
        let cached = owner.read().unwrap().get_ai_update_interface().unwrap();
        assert!(Arc::ptr_eq(&cached, &ai));
        {
            let mut ai = ai.lock().unwrap();
            let mut active = false;
            ai.with_cur_locomotor(&mut |_| active = true);
            assert!(active, "authored NORMAL locomotor must be installed");
            ai.execute_command(&AiCommandParams::new(
                AiCommandType::Busy,
                CommandSourceType::FromPlayer,
            ))
            .unwrap();
            assert_eq!(ai.get_current_state_id(), Some(AIStateType::Busy as u32));
        }
        eprintln!(
            "WITNESS factory admitted id={id}; cached native AI; real Busy state; loaded terrain/pathfinder; authored locomotor"
        );
        test(owner, ai);
    });
}
fn path() -> [Coord3D; 3] {
    [
        Coord3D::new(60.0, 30.0, 0.0),
        Coord3D::new(90.0, 30.0, 0.0),
        Coord3D::new(120.0, 30.0, 0.0),
    ]
}
fn machine(ai: &mut dyn AIUpdateInterface) -> Arc<Mutex<crate::ai::states::AIStateMachine>> {
    ai.unit_ai_for_test()
        .expect("actual cached UnitAI")
        .ai_state_machine
        .as_ref()
        .unwrap()
        .clone()
}

use crate::ai::states::follow_path::AIFollowExitProductionPathState;
use crate::state_machine::{StateReturnType, cpp_state::CppStateAdapter};
fn step(ai: &mut dyn AIUpdateInterface) -> StateReturnType {
    let handle = machine(ai);
    let native = ai.unit_ai_for_test().unwrap();
    let result = handle
        .lock()
        .unwrap()
        .update_state_machine(native, |_, _, _| {});
    result
}
fn local_goal(ai: &mut dyn AIUpdateInterface) -> Coord3D {
    let handle = machine(ai);
    let mut machine = handle.lock().unwrap();
    let body = machine
        .base
        .get_state_mut(AIStateType::FollowExitProductionPath as u32)
        .unwrap();
    (body.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<CppStateAdapter<AIFollowExitProductionPathState>>()
        .unwrap()
        .inner_mut_for_test()
        .base
        .base
        .goal_position
}
#[test]
fn quickexit_preserves_lock_and_orders_replacement_callbacks() {
    if !child(concat!(
        module_path!(),
        "::quickexit_preserves_lock_and_orders_replacement_callbacks"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|owner, handle| {
        let mut ai = handle.lock().unwrap();
        let fsm = machine(&mut *ai);
        let sentinel = Coord3D::new(19.0, 23.0, 0.0);
        {
            let mut machine = fsm.lock().unwrap();
            machine.set_goal_position(sentinel);
            machine.lock();
        }
        ai.do_quick_exit(&path());
        assert!(fsm.lock().unwrap().is_locked());
        assert_eq!(
            fsm.lock().unwrap().get_goal_position(),
            Some(sentinel),
            "C++ enter leaves parent goal unchanged"
        );
        assert_eq!(local_goal(&mut *ai), path()[0]);
        assert_eq!(ai.get_desired_speed(), crate::modules::FAST_AS_POSSIBLE);
        assert_eq!(
            owner.read().unwrap().get_position(),
            &Coord3D::new(30.0, 30.0, 0.0)
        );
        ai.set_precise_z_pos(true).unwrap();
        let start = fsm.lock().unwrap().temporary_lifecycle_observations().len();
        let replacement = [
            Coord3D::new(70.0, 40.0, 0.0),
            Coord3D::new(110.0, 40.0, 0.0),
        ];
        ai.do_quick_exit(&replacement);
        assert!(fsm.lock().unwrap().is_locked());
        assert_eq!(local_goal(&mut *ai), replacement[0]);
        ai.with_cur_locomotor(&mut |l| assert!(!l.uses_precise_z_pos()));
        let machine = fsm.lock().unwrap();
        let events = &machine.temporary_lifecycle_observations()[start..];
        assert_eq!(
            events.iter().map(|e| e.phase).collect::<Vec<_>>(),
            [
                "before_reset_exit",
                "after_reset_exit",
                "reset_cleared",
                "before_enter",
                "after_enter"
            ]
        );
        let id = Some(AIStateType::FollowExitProductionPath as u32);
        assert_eq!(events[0].published_temporary, id);
        assert_eq!(events[1].published_temporary, id);
        assert_eq!(events[2].published_temporary, None);
        assert_eq!(events[3].published_temporary, id);
        assert_eq!(
            (events[1].path_index, events[1].through_units),
            (Some(-1), Some(false))
        );
        assert_eq!(
            (events[3].path_index, events[3].through_units),
            (Some(-1), Some(false))
        );
        assert_eq!(
            (events[4].path_index, events[4].through_units),
            (Some(0), Some(true))
        );
        assert!(
            events
                .iter()
                .all(|e| e.ordinary_state == Some(AIStateType::Busy as u32))
        );
        let object = owner.read().unwrap();
        assert!(!object.ai_pending_ending_move && !object.ai_pending_destroy_path);
        assert!(
            object.ai_pending_path_through_units.is_none() && object.ai_pending_precise_z.is_none()
        );
    });
}
#[test]
fn failed_entry_publishes_exits_normally_then_clears() {
    if !child(concat!(
        module_path!(),
        "::failed_entry_publishes_exits_normally_then_clears"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|_, handle| {
        let mut ai = handle.lock().unwrap();
        let fsm = machine(&mut *ai);
        ai.set_current_goal_path_index(8).unwrap();
        ai.set_can_path_through_units(true).unwrap();
        ai.set_temporary_state(AIStateType::FollowPath, 300);
        let machine = fsm.lock().unwrap();
        let e = machine.temporary_lifecycle_observations();
        assert_eq!(
            e.iter().map(|e| e.phase).collect::<Vec<_>>(),
            [
                "before_enter",
                "after_enter",
                "before_failed_exit",
                "after_failed_exit",
                "failed_cleared"
            ]
        );
        assert!(
            e[..4]
                .iter()
                .all(|e| e.published_temporary == Some(AIStateType::FollowPath as u32))
        );
        assert_eq!(e[4].published_temporary, None);
        assert_eq!(
            (e[3].path_index, e[3].through_units),
            (Some(-1), Some(false))
        );
    });
}
#[test]
fn temporary_timeout_is_strict_capped_and_resumes_ordinary_same_step() {
    if !child(concat!(
        module_path!(),
        "::temporary_timeout_is_strict_capped_and_resumes_ordinary_same_step"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    let _frame = super::super::RestoreAmbientFrame::set(10);
    fixture(|_, handle| {
        let mut ai = handle.lock().unwrap();
        ai.do_quick_exit(&path());
        ai.set_temporary_state(AIStateType::FollowExitProductionPath, u32::MAX);
        let fsm = machine(&mut *ai);
        assert_eq!(fsm.lock().unwrap().temporary_frame_end_for_test(), 1810);
        let before = fsm.lock().unwrap().temporary_lifecycle_observations().len();
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .set_current_frame(1810);
        assert_eq!(step(&mut *ai), StateReturnType::Continue);
        assert_eq!(
            fsm.lock().unwrap().temporary_lifecycle_observations().len(),
            before,
            "ordinary state remains suspended at exact deadline"
        );
        crate::system::game_logic::get_game_logic()
            .lock()
            .unwrap()
            .set_current_frame(1811);
        assert_eq!(step(&mut *ai), StateReturnType::Continue);
        let machine = fsm.lock().unwrap();
        let e = &machine.temporary_lifecycle_observations()[before..];
        assert_eq!(
            e.iter().map(|e| e.phase).collect::<Vec<_>>(),
            [
                "before_completion_exit",
                "after_completion_exit",
                "completion_cleared",
                "before_ordinary_update"
            ]
        );
        assert_eq!(
            e[0].published_temporary,
            Some(AIStateType::FollowExitProductionPath as u32)
        );
        assert_eq!(
            (e[1].path_index, e[1].through_units),
            (Some(-1), Some(false))
        );
        assert_eq!(e[2].published_temporary, None);
        assert_eq!(e[3].published_temporary, None);
        assert_eq!(
            machine.get_current_state_id(),
            Some(AIStateType::Busy as u32)
        );
    });
}
#[test]
fn diagnostic_failed_path_results_preserve_follow_segment_publication_timing() {
    if !child(concat!(
        module_path!(),
        "::diagnostic_failed_path_results_preserve_follow_segment_publication_timing"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|_, handle| {
        let mut ai = handle.lock().unwrap();
        let fsm = machine(&mut *ai);
        let points = [
            path()[0],
            path()[1],
            path()[2],
            Coord3D::new(150.0, 30.0, 0.0),
        ];
        let sentinel = Coord3D::new(10.0, 15.0, 0.0);
        fsm.lock().unwrap().set_goal_position(sentinel);
        ai.do_quick_exit(&points);
        assert_eq!(fsm.lock().unwrap().get_goal_position(), Some(sentinel));
        assert_eq!(local_goal(&mut *ai), points[0]);
        // Explicitly deliver a failed path result to the actual AI. This is
        // segment lifecycle evidence, not pathfinder success or motion.
        ai.unit_ai_for_test().unwrap().data.waiting_for_path = false;
        assert_eq!(step(&mut *ai), StateReturnType::Continue);
        assert_eq!(ai.get_current_goal_path_index(), 1);
        assert_eq!(fsm.lock().unwrap().get_goal_position(), Some(points[0]));
        assert_eq!(local_goal(&mut *ai), points[1]);
        ai.unit_ai_for_test().unwrap().data.waiting_for_path = false;
        assert_eq!(step(&mut *ai), StateReturnType::Continue);
        assert_eq!(ai.get_current_goal_path_index(), 2);
        assert_eq!(fsm.lock().unwrap().get_goal_position(), Some(points[1]));
        assert_eq!(local_goal(&mut *ai), points[2]);
    });
}
#[test]
fn quickexit_preserves_unlocked_machine_and_terminal_body_resumes_ordinary() {
    if !child(concat!(
        module_path!(),
        "::quickexit_preserves_unlocked_machine_and_terminal_body_resumes_ordinary"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|_, handle| {
        let mut ai = handle.lock().unwrap();
        let fsm = machine(&mut *ai);
        assert!(!fsm.lock().unwrap().is_locked());
        ai.do_quick_exit(&[path()[0]]);
        assert!(!fsm.lock().unwrap().is_locked());
        let before = fsm.lock().unwrap().temporary_lifecycle_observations().len();
        // Deliver failed native path-result input. The real FollowPath body
        // consumes the last segment and returns Success, before its deadline.
        ai.unit_ai_for_test().unwrap().data.waiting_for_path = false;
        assert_eq!(step(&mut *ai), StateReturnType::Continue);
        let machine = fsm.lock().unwrap();
        assert_eq!(machine.get_temporary_state(), None);
        let e = &machine.temporary_lifecycle_observations()[before..];
        assert_eq!(
            e.iter().map(|e| e.phase).collect::<Vec<_>>(),
            [
                "before_completion_exit",
                "after_completion_exit",
                "completion_cleared",
                "before_ordinary_update"
            ]
        );
        assert_eq!(
            e[0].published_temporary,
            Some(AIStateType::FollowExitProductionPath as u32)
        );
        assert_eq!(
            (e[1].path_index, e[1].through_units),
            (Some(-1), Some(false))
        );
        assert_eq!(
            machine.get_current_state_id(),
            Some(AIStateType::Busy as u32)
        );
    });
}

#[test]
fn real_busy_ai_move_command_uses_borrowed_temporary_move_to() {
    if !child(concat!(
        module_path!(),
        "::real_busy_ai_move_command_uses_borrowed_temporary_move_to"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|owner, handle| {
        let mut ai = handle.lock().unwrap();
        let fsm = machine(&mut *ai);
        let mut command =
            AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
        command.pos = Coord3D::new(80.0, 90.0, 0.0);
        ai.execute_command(&command).unwrap();
        let machine = fsm.lock().unwrap();
        assert_eq!(
            machine.get_temporary_state(),
            Some(AIStateType::MoveTo as u32)
        );
        assert_eq!(
            machine.get_current_state_id(),
            Some(AIStateType::Busy as u32)
        );
        assert_eq!(machine.get_goal_position(), Some(command.pos));
        assert_eq!(
            ai.unit_ai_for_test().unwrap().data.requested_destination,
            command.pos
        );
        assert_eq!(
            owner.read().unwrap().get_position(),
            &Coord3D::new(30.0, 30.0, 0.0)
        );
        // Request arguments/admission are checked; native queue/motion remains
        // outside this lifecycle patch because its legacy registry path is absent.
    });
}

#[test]
fn ordinary_follow_path_command_installs_registered_body_without_relocking_ai() {
    if !child(concat!(
        module_path!(),
        "::ordinary_follow_path_command_installs_registered_body_without_relocking_ai"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|_, handle| {
        let mut ai = handle.lock().unwrap();
        let fsm = machine(&mut *ai);
        let mut command =
            AiCommandParams::new(AiCommandType::FollowPath, CommandSourceType::FromPlayer);
        command.coords = path().to_vec();
        ai.execute_command(&command).unwrap();
        let mut machine = fsm.lock().unwrap();
        assert_eq!(machine.get_temporary_state(), None);
        assert_eq!(
            machine.get_current_state_id(),
            Some(AIStateType::FollowPath as u32)
        );
        let body = machine
            .base
            .get_state_mut(AIStateType::FollowPath as u32)
            .unwrap();
        let path_state = (body.as_mut() as &mut dyn std::any::Any)
            .downcast_mut::<CppStateAdapter<crate::ai::states::follow_path::AIFollowPathState>>()
            .unwrap()
            .inner_mut_for_test();
        assert_eq!(path_state.path, path());
        assert_eq!(path_state.base.goal_position, path()[0]);
        assert_eq!(ai.get_current_goal_path_index(), 0);
        assert!(!ai.get_can_path_through_units());
    });
}

#[test]
fn native_move_out_missing_path_fails_with_its_own_synchronous_cleanup() {
    if !child(concat!(
        module_path!(),
        "::native_move_out_missing_path_fails_with_its_own_synchronous_cleanup"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|owner, handle| {
        let mut ai = handle.lock().unwrap();
        let fsm = machine(&mut *ai);
        // This is genuine failed-entry coverage, not successful MoveOut motion.
        // Native path authority is intentionally not expanded in this patch.
        assert!(ai.get_path_destination().is_none());
        let native = ai.unit_ai_for_test().unwrap();
        native.data.move_out_of_way_1 = 123;
        native.data.move_out_of_way_2 = 456;
        ai.set_can_path_through_units(true).unwrap();
        ai.friend_starting_move();
        ai.set_temporary_state(AIStateType::MoveOutOfTheWay, 300);
        assert_eq!(fsm.lock().unwrap().get_temporary_state(), None);
        assert!(!ai.get_can_path_through_units());
        assert!(!ai.is_moving());
        let native = ai.unit_ai_for_test().unwrap();
        assert_eq!(
            (native.data.move_out_of_way_1, native.data.move_out_of_way_2),
            (crate::common::INVALID_ID, crate::common::INVALID_ID)
        );
        let owner = owner.read().unwrap();
        assert!(
            !owner.ai_pending_ending_move
                && !owner.ai_pending_destroy_path
                && !owner.ai_pending_clear_move_out
        );
        assert!(owner.ai_pending_path_through_units.is_none());
    });
}

#[test]
fn dirty_native_formation_recompute_uses_held_members_exact_identity() {
    if !child(concat!(
        module_path!(),
        "::dirty_native_formation_recompute_uses_held_members_exact_identity"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|owner, handle| {
        let id = owner.read().unwrap().get_id();
        assert!(Arc::ptr_eq(
            &crate::object::registry::OBJECT_REGISTRY
                .get_object(id)
                .unwrap(),
            &owner
        ));
        let group = crate::ai::the_ai().write().unwrap().create_group();
        let group_id;
        {
            let mut group = group.write().unwrap();
            group.add(id);
            group_id = group.get_id();
            assert_eq!(group.get_count(), 1);
            assert!(group.is_member(id));
            assert!(group.speed_dirty_for_test());
            assert!(group.speed_visits_for_test().is_empty());
        }
        {
            let mut owner = owner.write().unwrap();
            owner.enter_group(&crate::ai::group::AIGroup::new(group_id));
            owner.set_formation_id(crate::common::FormationID::new(17));
        }
        let cached = owner.read().unwrap().get_ai_update_interface().unwrap();
        assert!(Arc::ptr_eq(&cached, &handle));
        let mut ai = handle.lock().unwrap();
        assert!(cached.try_lock().is_err());
        assert_eq!(
            ai.get_speed(),
            0.0,
            "existing native speed getter has no legacy Unit; no fabricated speed"
        );
        eprintln!(
            "FORMATION_ENTRY exact native Object={id} group={group_id}; dirty nonempty group; installed AI held"
        );
        ai.do_quick_exit(&path());
        assert_eq!(ai.get_desired_speed(), 0.0);
        let mut group = group.write().unwrap();
        assert!(!group.speed_dirty_for_test());
        assert_eq!(group.speed_visits_for_test(), &[id]);
        assert_eq!(group.speed_borrowed_hits_for_test(), 1);
        assert_eq!(group.get_speed(), 0.0);
        assert_eq!(group.speed_borrowed_hits_for_test(), 1);
        assert_eq!(
            group.speed_visits_for_test(),
            &[id],
            "clean cache performs no member read"
        );
    });
}

#[test]
fn dirty_formation_move_to_command_uses_same_borrowed_speed_metric() {
    if !child(concat!(
        module_path!(),
        "::dirty_formation_move_to_command_uses_same_borrowed_speed_metric"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|owner, handle| {
        let id = owner.read().unwrap().get_id();
        let group = crate::ai::the_ai().write().unwrap().create_group();
        let group_id;
        {
            let mut group = group.write().unwrap();
            group.add(id);
            group_id = group.get_id();
            assert!(group.speed_dirty_for_test());
        }
        {
            let mut owner = owner.write().unwrap();
            owner.enter_group(&crate::ai::group::AIGroup::new(group_id));
            owner.set_formation_id(crate::common::FormationID::new(18));
        }
        assert!(Arc::ptr_eq(
            &owner.read().unwrap().get_ai_update_interface().unwrap(),
            &handle
        ));
        let mut ai = handle.lock().unwrap();
        let speed = ai.get_speed();
        let mut command =
            AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
        command.pos = path()[0];
        ai.execute_command(&command).unwrap();
        assert_eq!(ai.get_desired_speed(), speed);
        let group = group.read().unwrap();
        assert!(!group.speed_dirty_for_test());
        assert_eq!(group.speed_visits_for_test(), &[id]);
        assert_eq!(group.speed_borrowed_hits_for_test(), 1);
    });
}

#[test]
fn formation_recompute_rejects_mismatched_handles_and_preserves_ordinary_cache() {
    if !child(concat!(
        module_path!(),
        "::formation_recompute_rejects_mismatched_handles_and_preserves_ordinary_cache"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    terrain(|| {
        definitions();
        let mut factory = ObjectFactory::new();
        let first_id = create(
            &mut factory,
            "EnterCapacityNoWeapon",
            Coord3D::new(30.0, 30.0, 0.0),
            None,
            ObjectCreationFlags::empty(),
        );
        let second_id = create(
            &mut factory,
            "EnterCapacityNoWeapon",
            Coord3D::new(60.0, 60.0, 0.0),
            None,
            ObjectCreationFlags::empty(),
        );
        let (first, first_ai) = native_ai(&factory, first_id);
        let (second, second_ai) = native_ai(&factory, second_id);
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(!Arc::ptr_eq(&first_ai, &second_ai));
        let group = crate::ai::the_ai().write().unwrap().create_group();
        let mut group = group.write().unwrap();
        group.add(first_id);
        let other = second_ai.lock().unwrap();
        let other_speed = other.get_speed();
        // Same Object but wrong installed AI: the supplied context is rejected,
        // and the real member remains on the original locked-read path.
        assert_eq!(
            group.get_speed_with_borrowed_member(&first, &second_ai, other_speed),
            0.0
        );
        assert_eq!(group.speed_borrowed_hits_for_test(), 0);
        assert!(!group.speed_dirty_for_test());
        assert_eq!(group.speed_visits_for_test(), &[first_id]);
        group.recompute_group_speed();
        assert_eq!(
            group.get_speed_with_borrowed_member(&second, &second_ai, other_speed),
            0.0
        );
        assert_eq!(group.speed_borrowed_hits_for_test(), 0);
        drop(other);
        group.recompute_group_speed();
        assert_eq!(group.get_speed(), 0.0);
        assert_eq!(group.speed_visits_for_test().len(), 3);
        assert!(!group.speed_dirty_for_test());
        assert_eq!(group.get_speed(), 0.0);
        assert_eq!(group.speed_visits_for_test().len(), 3);
        group.recompute_group_speed();
        let actual = first_ai.lock().unwrap();
        let actual_speed = actual.get_speed();
        assert_eq!(
            group.get_speed_with_borrowed_member(&first, &first_ai, actual_speed),
            actual_speed
        );
        assert_eq!(group.speed_borrowed_hits_for_test(), 1);
        assert_eq!(group.speed_visits_for_test().len(), 4);
        assert!(!group.speed_dirty_for_test());
    });
}
