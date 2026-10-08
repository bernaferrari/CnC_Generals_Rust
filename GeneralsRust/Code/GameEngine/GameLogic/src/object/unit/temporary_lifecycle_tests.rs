//! Installed-object temporary lifecycle regressions, AIStates.cpp:903-954,3297-3314.
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
fn machine(ai: &mut dyn AIUpdateInterface) -> &mut crate::ai::states::AIStateMachine {
    ai.unit_ai_for_test()
        .expect("actual cached UnitAI")
        .ai_state_machine
        .as_mut()
        .unwrap()
}
fn seeded_path(ai: &mut dyn AIUpdateInterface) {
    use crate::ai::states::follow_path::AIFollowExitProductionPathState;
    use crate::state_machine::cpp_state::CppStateAdapter;
    let machine = machine(ai);
    let state = machine
        .base
        .get_state_mut(AIStateType::FollowExitProductionPath as u32)
        .unwrap();
    let state = (state.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<CppStateAdapter<AIFollowExitProductionPathState>>()
        .expect("registered CppStateAdapter, never replaced");
    state.inner_mut_for_test().set_path(path().to_vec());
    assert_eq!(state.inner_mut_for_test().base.path, path());
    eprintln!(
        "DIAGNOSTIC ONLY: test seeded real installed adapter's nonempty path; production installer is separately tested"
    );
}
fn entered(ai: &mut dyn AIUpdateInterface) {
    ai.do_quick_exit(&path());
    let state = machine(ai);
    let registered = state
        .base
        .get_state_mut(AIStateType::FollowExitProductionPath as u32)
        .unwrap();
    let registered = (registered.as_mut() as &mut dyn std::any::Any)
        .downcast_mut::<crate::state_machine::cpp_state::CppStateAdapter<
            crate::ai::states::follow_path::AIFollowExitProductionPathState,
        >>()
        .unwrap();
    let actual_path = registered.inner_mut_for_test().base.path.clone();
    eprintln!(
        "WITNESS actual registered path={actual_path:?}; requested path={:?}",
        path()
    );
    eprintln!(
        "WITNESS after quickexit: temporary={:?}; ordinary={:?}",
        state.get_temporary_state(),
        state.get_current_state_id()
    );
    assert_eq!(
        state.get_temporary_state(),
        Some(AIStateType::FollowExitProductionPath as u32),
        "quickexit must actually admit the registered movement state"
    );
    assert_eq!(
        actual_path,
        path(),
        "registered movement state owns exact authored path"
    );
    drop(state);
    assert_eq!(
        ai.get_current_state_id(),
        Some(AIStateType::Busy as u32),
        "ordinary state stays suspended"
    );
    assert_eq!(ai.get_current_goal_path_index(), 0);
    assert!(ai.get_can_path_through_units());
    assert!(ai.is_moving());
    eprintln!(
        "WITNESS quick exit returned: Busy suspended; path index=0; through units=true; moving=true"
    );
}
#[test]
fn installed_quick_exit_enters_real_nonempty_follow_path() {
    if !child(concat!(
        module_path!(),
        "::installed_quick_exit_enters_real_nonempty_follow_path"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|_, handle| {
        entered(&mut *handle.lock().unwrap());
    });
}
#[test]
fn diagnostic_seeded_installed_temporary_follow_path_returns() {
    if !child(concat!(
        module_path!(),
        "::diagnostic_seeded_installed_temporary_follow_path_returns"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|_, handle| {
        let mut ai = handle.lock().unwrap();
        seeded_path(&mut *ai);
        eprintln!(
            "WITNESS calling installed set_temporary_state(FollowExitProductionPath) with explicitly test-populated registered path"
        );
        ai.set_temporary_state(AIStateType::FollowExitProductionPath, 300);
        eprintln!("WITNESS temporary replacement returned");
        assert_eq!(ai.get_current_goal_path_index(), 0);
    });
}
#[test]
fn installed_failed_entry_cleans_follow_path_synchronously() {
    if !child(concat!(
        module_path!(),
        "::installed_failed_entry_cleans_follow_path_synchronously"
    )) {
        return;
    }
    let _serial = crate::test_sync::lock();
    fixture(|owner, handle| {
        let mut ai = handle.lock().unwrap();
        // Intentional empty registered FollowPath is a stable failed-entry control,
        // independent of the quick-exit installer defect. Seed flags BEFORE entry.
        ai.set_precise_z_pos(true).unwrap();
        ai.set_can_path_through_units(true).unwrap();
        ai.set_current_goal_path_index(3).unwrap();
        ai.friend_starting_move();
        ai.set_temporary_state(AIStateType::FollowPath, 300);
        assert_eq!(
            machine(&mut *ai).get_temporary_state(),
            None,
            "empty registered FollowPath fails entry and is cleared"
        );
        eprintln!(
            "WITNESS failed entry returned: through_units={} index={}; pending={:?}/{:?}/{:?}/{:?}",
            ai.get_can_path_through_units(),
            ai.get_current_goal_path_index(),
            owner.read().unwrap().ai_pending_path_through_units,
            owner.read().unwrap().ai_pending_precise_z,
            owner.read().unwrap().ai_pending_path_extra,
            owner.read().unwrap().ai_pending_goal_path_index
        );
        assert!(
            !ai.get_can_path_through_units(),
            "failed-entry Normal exit cleanup must be synchronous on return"
        );
        assert_eq!(ai.get_current_goal_path_index(), -1);
        ai.with_cur_locomotor(&mut |l| assert!(!l.uses_precise_z_pos()));
        assert!(
            !ai.is_moving(),
            "failed-entry Normal exit ends movement synchronously"
        );
        let owner = owner.read().unwrap();
        assert!(!owner.ai_pending_ending_move);
        assert!(!owner.ai_pending_destroy_path);
        assert!(owner.ai_pending_path_through_units.is_none());
        assert!(owner.ai_pending_precise_z.is_none());
        assert!(owner.ai_pending_path_extra.is_none());
        assert!(owner.ai_pending_goal_path_index.is_none());
    });
}
