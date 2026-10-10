//! Real snapshot -> owned roster -> ScriptEngine TeamGuard continuation.
use super::named_command_test_support::world;
use super::*;
use crate::save_load::SnapshotBuilder;

fn definition(world: &GameLogic, singleton: bool) {
    world
        .team_factory
        .lock()
        .unwrap()
        .init_team("SavedRoster".into(), "".into(), singleton, None)
        .unwrap();
}

fn roster(world: &GameLogic, first: ObjectId, second: ObjectId, reversed: bool, active: bool) {
    let mut factory = world.team_factory.lock().unwrap();
    let prototype = factory.find_team_prototype("SavedRoster").unwrap();
    for (id, member) in [
        (90, if reversed { second } else { first }),
        (10, if reversed { first } else { second }),
    ] {
        let team = factory
            .create_team_on_prototype_with_id(&prototype, id)
            .unwrap();
        let mut team = team.write().unwrap();
        team.add_member(member.0);
        if active {
            team.set_active();
        }
    }
}

fn assert_guarded(world: &GameLogic, selected: ObjectId, untouched: ObjectId) {
    let unit = world.host_object(selected).unwrap();
    assert_eq!(unit.guard_position, Some(unit.get_position()));
    assert_eq!(unit.ai_state, AIState::GuardingArea);
    assert_eq!(
        unit.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(world.host_object(untouched).unwrap().guard_position, None);
}

#[test]
fn team_save_walk_calibration_restores_objects_without_a_roster() {
    let (source, first, _) = world();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let (mut loaded, _, _) = world();
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    assert_eq!(
        loaded.host_object(first).unwrap().get_position(),
        source.host_object(first).unwrap().get_position()
    );
    loaded.execute_team_guard_script_for_test("MissingRoster");
    assert_eq!(loaded.host_object(first).unwrap().guard_position, None);
}

#[test]
fn fresh_candidate_admits_missing_instances_in_cpp_saved_visitation_order() {
    let (source, first, second) = world();
    definition(&source, false);
    roster(&source, first, second, false, true);
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let (mut loaded, _, _) = world();
    definition(&loaded, false);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    let factory = loaded.team_factory.lock().unwrap();
    assert_eq!(
        factory
            .find_team_instances("SavedRoster")
            .iter()
            .map(|t| t.read().unwrap().get_id())
            .collect::<Vec<_>>(),
        [90, 10]
    );
    assert_eq!(
        factory
            .find_team_by_id(10)
            .unwrap()
            .read()
            .unwrap()
            .get_members(),
        [second.0]
    );
    drop(factory);
    loaded.execute_team_guard_script_for_test("SavedRoster");
    assert_guarded(&loaded, first, second);
    assert_eq!(
        loaded.host_object(second).unwrap().team_instance_name,
        "SavedRoster"
    );
}

#[test]
fn retained_snapshots_restore_their_own_rosters_in_either_order() {
    for reverse_restore in [false, true] {
        let (source_a, first, second) = world();
        definition(&source_a, false);
        roster(&source_a, first, second, false, true);
        let a = SnapshotBuilder::new()
            .create_world_snapshot(&source_a)
            .unwrap();
        let (source_b, first_b, second_b) = world();
        assert_eq!((first, second), (first_b, second_b));
        definition(&source_b, false);
        roster(&source_b, first_b, second_b, true, true);
        let b = SnapshotBuilder::new()
            .create_world_snapshot(&source_b)
            .unwrap();
        let (mut loaded_a, _, _) = world();
        definition(&loaded_a, false);
        let (mut loaded_b, _, _) = world();
        definition(&loaded_b, false);
        let builder = SnapshotBuilder::new();
        if reverse_restore {
            builder.restore_from_snapshot(&b, &mut loaded_b).unwrap();
            builder.restore_from_snapshot(&a, &mut loaded_a).unwrap();
        } else {
            builder.restore_from_snapshot(&a, &mut loaded_a).unwrap();
            builder.restore_from_snapshot(&b, &mut loaded_b).unwrap();
        }
        loaded_a.execute_team_guard_script_for_test("SavedRoster");
        loaded_b.execute_team_guard_script_for_test("SavedRoster");
        assert_guarded(&loaded_a, first, second);
        assert_guarded(&loaded_b, second, first);
        drop(loaded_b);
        drop(source_b);
        loaded_a.execute_team_guard_script_for_test("SavedRoster");
        assert_guarded(&loaded_a, first, second);
    }
}

#[test]
fn inactive_singleton_keeps_its_saved_head_and_does_not_activate_on_restore() {
    let (source, first, second) = world();
    definition(&source, true);
    roster(&source, first, second, false, false);
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let (mut loaded, _, _) = world();
    definition(&loaded, true);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    loaded.execute_team_guard_script_for_test("SavedRoster");
    assert_eq!(loaded.host_object(second).unwrap().guard_position, None);
    let factory = loaded.team_factory.lock().unwrap();
    let instance = factory
        .find_team_by_id(10)
        .expect("inactive saved instance");
    let instance = instance.read().unwrap();
    assert!(!instance.is_active());
    assert!(!instance.is_created());
    assert_eq!(instance.get_members(), [second.0]);
}

#[test]
fn missing_secondary_member_rejects_capture_instead_of_saving_a_partial_roster() {
    let (source, first, _) = world();
    definition(&source, false);
    roster(&source, first, ObjectId(999999), false, true);
    assert!(
        SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .is_err()
    );
}

#[test]
fn membership_cannot_belong_to_two_saved_instances() {
    let (source, first, _) = world();
    definition(&source, false);
    roster(&source, first, first, false, true);
    assert!(
        SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .is_err()
    );
}

#[test]
fn existing_instances_are_not_relinked_when_saved_data_is_applied() {
    let (source, first, second) = world();
    definition(&source, false);
    roster(&source, first, second, false, true);
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let (mut loaded, _, _) = world();
    definition(&loaded, false);
    roster(&loaded, first, second, true, false);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut loaded)
        .unwrap();
    loaded.execute_team_guard_script_for_test("SavedRoster");
    assert_guarded(&loaded, second, first);
    let factory = loaded.team_factory.lock().unwrap();
    assert_eq!(
        factory
            .find_team_instances("SavedRoster")
            .iter()
            .map(|t| t.read().unwrap().get_id())
            .collect::<Vec<_>>(),
        [10, 90]
    );
}

#[test]
fn rejected_missing_map_definition_leaves_running_owner_unchanged() {
    let (source, first, second) = world();
    definition(&source, false);
    roster(&source, first, second, false, true);
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&source)
        .unwrap();
    let (mut live, _, _) = world();
    live.frame = 543;
    live.host_object_mut(first)
        .unwrap()
        .set_position(Vec3::new(123.0, 0.0, 456.0));
    let before = live.host_object(first).unwrap().get_position();
    assert!(
        SnapshotBuilder::new()
            .restore_from_snapshot(&snapshot, &mut live)
            .is_err()
    );
    assert_eq!(live.frame, 543);
    assert_eq!(live.host_object(first).unwrap().get_position(), before);
    assert!(
        live.team_factory
            .lock()
            .unwrap()
            .find_team_instances("SavedRoster")
            .is_empty()
    );
}
