//! Real script action -> owned trigger selection -> checked object-goal movement.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

// A distant tracking target must have a distinct authored type: CPP's
// Coord3D closest query permits the receiver itself when its type matches.
fn tracking_world() -> (GameLogic, ObjectId, ObjectId) {
    let (mut world, unit, _) = world();
    let mut template = world.templates["OwnedNamedInfantry"].clone();
    template.name = "OwnedNearestTarget".into();
    world.templates.insert(template.name.clone(), template);
    let target = world
        .create_object_for_player("OwnedNearestTarget", 2, Vec3::new(70.0, 0.0, 70.0))
        .unwrap();
    (world, unit, target)
}

fn trigger(world: &GameLogic) {
    let polygon = gamelogic::polygon_trigger::PolygonTrigger::new(
        9901,
        "NearestOwned".into(),
        vec![
            gamelogic::common::ICoord3D::new(0, 0, 0),
            gamelogic::common::ICoord3D::new(1000, 0, 0),
            gamelogic::common::ICoord3D::new(1000, 1000, 0),
            gamelogic::common::ICoord3D::new(0, 1000, 0),
        ],
    );
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .add_trigger_area(polygon);
}

fn execute(world: &mut GameLogic, team: bool, recipient: &str) {
    world.execute_nearest_object_script_for_test(
        team,
        recipient,
        "OwnedNearestTarget",
        "NearestOwned",
    );
}

impl GameLogic {
    pub(crate) fn execute_nearest_object_script_for_test(
        &mut self,
        team: bool,
        recipient: &str,
        object_type: &str,
        trigger: &str,
    ) {
        let world = self;
        let mut action = ScriptAction::new(if team {
            ScriptActionType::TeamMoveTowardsNearestObjectType
        } else {
            ScriptActionType::UnitMoveTowardsNearestObjectType
        });
        for (kind, value) in [
            (
                if team {
                    ParameterType::Team
                } else {
                    ParameterType::Unit
                },
                recipient,
            ),
            (ParameterType::ObjectType, object_type),
            (ParameterType::TriggerArea, trigger),
        ] {
            action
                .add_parameter(Parameter::with_string(kind, value.into()))
                .unwrap();
        }
        let mut condition = OrCondition::new();
        condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        let mut script = Script::new();
        script.script_name = "OwnedNearest".into();
        script.is_one_shot = true;
        script.condition = Some(Box::new(condition));
        script.action = Some(Box::new(action));
        let mut list = ScriptList::new();
        list.append_script(Box::new(script));
        let mut engine = ScriptEngine::new().unwrap();
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        engine
            .update_with_driver(
                gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
                &mut HostScriptExecutionDriver::new(world),
            )
            .unwrap();
    }
}

#[test]
fn nearest_actual_script_tracks_selected_identity_and_preserves_missing_goal() {
    let (mut world, unit, target) = tracking_world();
    trigger(&world);
    execute(&mut world, false, "NamedUnit");
    let destination = world.host_object(target).unwrap().get_position();
    let mover = world.host_object(unit).unwrap();
    assert_eq!(mover.movement_goal_object(), Some(target));
    assert_eq!(mover.requested_destination, Some(destination));
    assert_eq!(
        mover.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        mover.cur_locomotor_name.as_deref(),
        Some("BasicHumanLocomotor")
    );
    let moved = Vec3::new(180.0, 0.0, 170.0);
    world.set_fixture_object_position(target, moved);
    let before_frame = world.frame;
    for _ in 0..12 {
        world.update_with_dt(1.0 / 30.0);
    }
    assert_eq!(world.frame, before_frame + 12);
    assert_eq!(
        world.host_object(unit).unwrap().requested_destination,
        Some(moved)
    );
    world.objects.remove(&target);
    world.refresh_movement_goal_objects(&[unit]);
    assert_eq!(
        world.host_object(unit).unwrap().requested_destination,
        Some(moved)
    );
    assert_eq!(
        world.host_object(unit).unwrap().movement_goal_object(),
        Some(target)
    );
    assert_eq!(
        world
            .host_object(unit)
            .unwrap()
            .clone()
            .movement_goal_object(),
        Some(target)
    );
    assert!(world.unit_command_stop(unit));
    assert_eq!(
        world.host_object(unit).unwrap().movement_goal_object(),
        None
    );
}

#[test]
fn nearest_script_team_uses_owned_roster_and_foreign_world_cannot_move_goal() {
    let (mut world, unit, target) = tracking_world();
    trigger(&world);
    {
        let mut factory = world.team_factory.lock().unwrap();
        factory
            .init_team("NearestRoster".into(), "".into(), false, None)
            .unwrap();
        factory
            .create_team("NearestRoster")
            .unwrap()
            .write()
            .unwrap()
            .add_member(unit.0);
    }
    let (mut foreign, foreign_unit, foreign_target) = tracking_world();
    assert_eq!(unit, foreign_unit);
    assert_eq!(target, foreign_target);
    foreign.set_fixture_object_position(foreign_target, Vec3::splat(900.0));
    execute(&mut world, true, "NearestRoster");
    assert_eq!(
        world.host_object(unit).unwrap().movement_goal_object(),
        Some(target)
    );
    assert_eq!(
        world.host_object(unit).unwrap().requested_destination,
        Some(world.host_object(target).unwrap().get_position())
    );
    world.refresh_movement_goal_objects(&[unit]);
    assert_ne!(
        world.host_object(unit).unwrap().requested_destination,
        Some(foreign.host_object(foreign_target).unwrap().get_position())
    );
    assert!(world.unit_command_move_to(unit, Vec3::new(100.0, 0.0, 100.0)));
    assert_eq!(
        world.host_object(unit).unwrap().movement_goal_object(),
        None
    );
}

#[test]
fn nearest_actual_script_snapshot_preserves_moving_goal_continuation() {
    use crate::save_load::snapshot::SnapshotBuilder;
    let (mut source, unit, target) = tracking_world();
    trigger(&source);
    source.update_with_dt(1.0 / 30.0);
    execute(&mut source, false, "NamedUnit");
    for _ in 0..3 {
        source.update_with_dt(1.0 / 30.0);
    }
    let builder = SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, restored_unit, restored_target) = tracking_world();
    assert_eq!((unit, target), (restored_unit, restored_target));
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    let future = Vec3::new(160.0, 0.0, 140.0);
    source.set_fixture_object_position(target, future);
    restored.set_fixture_object_position(target, future);
    let initial = source.host_object(unit).unwrap().get_position();
    for _ in 0..18 {
        source.update_with_dt(1.0 / 30.0);
        restored.update_with_dt(1.0 / 30.0);
        let original = source.host_object(unit).unwrap();
        let loaded = restored.host_object(unit).unwrap();
        assert_eq!(original.movement_goal_object(), Some(target));
        assert_eq!(
            loaded.movement_goal_object(),
            original.movement_goal_object()
        );
        assert_eq!(loaded.requested_destination, original.requested_destination);
        assert_eq!(loaded.path_goal_position, original.path_goal_position);
        assert_eq!(loaded.movement.path, original.movement.path);
        assert_eq!(
            loaded.movement.current_path_index,
            original.movement.current_path_index
        );
        assert_eq!(loaded.get_position(), original.get_position());
    }
    assert_ne!(
        source.host_object(unit).unwrap().get_position(),
        initial,
        "admitted locomotor must advance the real mover"
    );
    assert_eq!(
        source.host_object(unit).unwrap().requested_destination,
        Some(future)
    );
    assert!(source.unit_command_stop(unit));
    assert!(restored.unit_command_stop(unit));
    for world in [&source, &restored] {
        let mover = world.host_object(unit).unwrap();
        assert_eq!(mover.movement_goal_object(), None);
        assert!(mover.movement.path.is_empty());
    }
}

#[test]
fn nearest_cpp_coordinate_query_can_select_receiver_itself() {
    let (mut world, unit, _) = world();
    trigger(&world);
    let own_position = world.host_object(unit).unwrap().get_position();
    world.execute_nearest_object_script_for_test(
        false,
        "NamedUnit",
        "OwnedNamedInfantry",
        "NearestOwned",
    );
    assert_eq!(
        world.host_object(unit).unwrap().movement_goal_object(),
        Some(unit)
    );
    assert_eq!(
        world.host_object(unit).unwrap().requested_destination,
        Some(own_position)
    );
}

#[test]
fn nearest_cpp_filters_allow_dead_and_queued_bodies_until_physical_removal() {
    let (mut world, unit, target) = tracking_world();
    trigger(&world);
    let farther = world
        .create_object_for_player("OwnedNearestTarget", 2, Vec3::new(300.0, 0.0, 300.0))
        .unwrap();
    world.host_object_mut(target).unwrap().health.current = 0.0;
    execute(&mut world, false, "NamedUnit");
    assert_eq!(
        world.host_object(unit).unwrap().movement_goal_object(),
        Some(target)
    );
    assert!(world.unit_command_stop(unit));
    world.host_object_mut(target).unwrap().status.destroyed = true;
    execute(&mut world, false, "NamedUnit");
    assert_eq!(
        world.host_object(unit).unwrap().movement_goal_object(),
        Some(target)
    );
    assert!(world.unit_command_stop(unit));
    world.objects.remove(&target);
    execute(&mut world, false, "NamedUnit");
    assert_eq!(
        world.host_object(unit).unwrap().movement_goal_object(),
        Some(farther)
    );
}
