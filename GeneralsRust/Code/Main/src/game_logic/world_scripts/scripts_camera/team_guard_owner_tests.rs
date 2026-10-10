//! Actual ScriptEngine -> Main TeamGuard, CPP ScriptActions1882.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_TEAM,
};
use gamelogic::scripting::engine::ScriptEngine;

fn team(world: &GameLogic, name: &str, members: &[ObjectId], singleton: bool, active: bool) -> u32 {
    let mut factory = world.team_factory.lock().unwrap();
    if factory.find_team_prototype(name).is_none() {
        factory
            .init_team(name.into(), "".into(), singleton, None)
            .unwrap();
    }
    let roster = if active {
        factory.create_team(name)
    } else {
        factory.create_inactive_team(name)
    }
    .unwrap();
    let mut roster = roster.write().unwrap();
    for id in members {
        roster.add_member(id.0);
    }
    roster.get_id()
}

fn action(name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, name.into()))
        .unwrap();
    let mut counter = ScriptAction::new(ScriptActionType::IncrementCounter);
    counter
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    counter
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "TeamObserved".into(),
        ))
        .unwrap();
    action.next_action = Some(Box::new(counter));
    action
}

fn execute(world: &mut GameLogic, name: &str) -> i32 {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "OwnedTeamGuard".into();
    script.is_one_shot = true;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action(name)));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("TeamObserved", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
    engine.get_counter("TeamObserved").unwrap().value
}

fn assert_guard(world: &GameLogic, id: ObjectId, position: Vec3) {
    let unit = world.host_object(id).unwrap();
    assert_eq!(unit.guard_position, Some(position));
    assert_eq!(unit.guard_mode, GuardMode::Normal);
    assert_eq!(unit.ai_state, AIState::GuardingArea);
    assert_eq!(
        unit.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
}

#[test]
fn team_guard_action_walk_calibration() {
    let (mut world, _, _) = world();
    assert_eq!(execute(&mut world, "MissingTeam"), 1);
}

#[test]
fn team_guard_members_keep_distinct_posts_formation_and_locomotor() {
    let (mut world, first, second) = world();
    team(&world, "GuardRoster", &[first, second], false, true);
    // The roster, not player/faction or writable name census, selects members.
    world.host_object_mut(first).unwrap().team_instance_name = "Unrelated".into();
    world.host_object_mut(second).unwrap().owner_player_id = None;
    let first_post = world.host_object(first).unwrap().get_position();
    let second_post = world.host_object(second).unwrap().get_position();
    assert_eq!(execute(&mut world, "GuardRoster"), 1);
    assert_guard(&world, first, first_post);
    assert_guard(&world, second, second_post);
    let unit = world.host_object(first).unwrap();
    assert_eq!(unit.formation_id, 19);
    assert_eq!(unit.formation_offset, glam::Vec2::new(4.0, 6.0));
    assert_eq!(
        unit.cur_locomotor_name.as_deref(),
        Some("RedguardLocomotor")
    );
    assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_PANIC"));
}

#[test]
fn team_guard_same_ids_are_owned_by_the_driving_world() {
    let (mut first, id, _) = world();
    let (mut second, second_id, _) = world();
    assert_eq!(id, second_id);
    team(&first, "GuardRoster", &[id], false, true);
    team(&second, "GuardRoster", &[id], false, true);
    let post = first.host_object(id).unwrap().get_position();
    second
        .host_object_mut(id)
        .unwrap()
        .set_position(Vec3::splat(500.0));
    assert_eq!(execute(&mut first, "GuardRoster"), 1);
    assert_guard(&first, id, post);
    assert_eq!(second.host_object(id).unwrap().guard_position, None);
    assert_eq!(execute(&mut second, "GuardRoster"), 1);
    assert_guard(&second, id, Vec3::splat(500.0));
    second.reset();
    assert_guard(&first, id, post);
}

#[test]
fn team_guard_calling_team_precedes_condition_team() {
    let (mut world, calling, condition) = world();
    let calling_id = team(&world, "CallingGuard", &[calling], true, false);
    let condition_id = team(&world, "ConditionGuard", &[condition], false, true);
    let post = world.host_object(calling).unwrap().get_position();
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_external_eval_context(None, Some(condition_id));
    engine.set_counter("TeamObserved", 0).unwrap();
    engine.friend_execute_action_with_driver(
        &action(THIS_TEAM),
        Some(calling_id),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert_eq!(engine.get_counter("TeamObserved").unwrap().value, 1);
    assert_guard(&world, calling, post);
    assert_eq!(world.host_object(condition).unwrap().guard_position, None);
    assert_eq!(engine.get_condition_team_id(), Some(condition_id));
    assert_eq!(engine.get_calling_team_id(), None);
}

#[test]
fn team_guard_missing_exact_identity_never_creates_or_uses_faction_census() {
    let (mut world, id, _) = world();
    world.host_object_mut(id).unwrap().team_instance_name = "MissingTeam".into();
    team(&world, "GuardRoster", &[id], false, true);
    let before = world.team_factory.lock().unwrap().get_all_teams().len();
    for name in ["MissingTeam", "guardroster", " GuardRoster ", THIS_TEAM] {
        assert_eq!(execute(&mut world, name), 1);
        assert_eq!(world.host_object(id).unwrap().guard_position, None);
    }
    assert_eq!(
        world.team_factory.lock().unwrap().get_all_teams().len(),
        before
    );
}

#[test]
fn team_guard_inactive_singleton_is_missing_outside_context() {
    let (mut world, id, _) = world();
    team(&world, "InactiveGuard", &[id], true, false);
    assert_eq!(execute(&mut world, "InactiveGuard"), 1);
    assert_eq!(world.host_object(id).unwrap().guard_position, None);
}

#[test]
fn team_guard_literal_selects_newest_instance_without_auto_creation() {
    let (mut world, first, last) = world();
    team(&world, "MultipleGuard", &[first], false, true);
    team(&world, "MultipleGuard", &[last], false, true);
    let post = world.host_object(last).unwrap().get_position();
    assert_eq!(execute(&mut world, "MultipleGuard"), 1);
    assert_guard(&world, last, post);
    assert_eq!(world.host_object(first).unwrap().guard_position, None);
}

#[test]
fn team_guard_skips_non_ai_and_rejected_members_without_named_preparation() {
    let setters: [fn(&mut Object); 4] = [
        |u| {
            u.template_mut()
                .set_authored_ai_update_interface(Some(false));
        },
        |u| u.status.effectively_dead = true,
        |u| u.health.current = 0.0,
        |u| {
            u.template_mut().add_kind_of(KindOf::Immobile);
        },
    ];
    for reject in setters {
        let (mut world, rejected, accepted) = world();
        team(
            &world,
            "GuardRoster",
            &[rejected, ObjectId(u32::MAX), accepted],
            false,
            true,
        );
        reject(world.host_object_mut(rejected).unwrap());
        let post = world.host_object(accepted).unwrap().get_position();
        assert_eq!(execute(&mut world, "GuardRoster"), 1);
        assert_guard(&world, accepted, post);
        let unit = world.host_object(rejected).unwrap();
        assert_eq!(unit.guard_position, None);
        assert_eq!(unit.formation_id, 19);
        assert_eq!(unit.jet_ai.cur_locomotor_set.as_deref(), Some("SET_PANIC"));
        assert_eq!(
            unit.last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_AI
        );
    }
}

#[test]
fn team_guard_reentry_exits_hunt_and_clears_previous_goal_immediately() {
    let (mut world, id, target) = world();
    team(&world, "GuardRoster", &[id], false, true);
    assert!(world.unit_command_patrol(id));
    let unit = world.host_object_mut(id).unwrap();
    unit.set_target(Some(target));
    unit.pending_move = Some(Vec3::splat(300.0));
    unit.set_guard_target(Some(target));
    unit.guard_area_trigger = Some("OldGuardArea".into());
    let post = unit.get_position();
    assert_eq!(execute(&mut world, "GuardRoster"), 1);
    assert_guard(&world, id, post);
    let unit = world.host_object(id).unwrap();
    assert_eq!(unit.target, None);
    assert_eq!(unit.guard_target, None);
    assert_eq!(unit.pending_move, None);
    assert_eq!(unit.guard_area_trigger, None);
    assert!(!unit.hunting);
    assert_eq!(unit.unit_ai_runtime.hunt_scan_deadline(), None);
}

#[test]
fn team_guard_snapshot_continues_without_reissuing_the_team_command() {
    let (mut source, id, _) = world();
    team(&source, "GuardRoster", &[id], false, true);
    assert_eq!(execute(&mut source, "GuardRoster"), 1);
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&source).unwrap();
    let (mut restored, _, _) = world();
    // Production loads map definitions before admitting saved instances.
    restored
        .team_factory
        .lock()
        .unwrap()
        .init_team("GuardRoster".into(), "".into(), false, None)
        .unwrap();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    for _ in 0..12 {
        let frame = source.frame;
        source.update_with_dt_budget(1.0 / 30.0, 1);
        restored.update_with_dt_budget(1.0 / 30.0, 1);
        assert_eq!(source.frame, frame + 1);
        assert_eq!(restored.frame, source.frame);
        let a = source.host_object(id).unwrap();
        let b = restored.host_object(id).unwrap();
        assert_eq!(a.ai_state, b.ai_state);
        assert_eq!(a.guard_position, b.guard_position);
        assert_eq!(a.last_command_source, b.last_command_source);
        assert_eq!(a.get_position(), b.get_position());
        assert_eq!(
            a.unit_ai_runtime.guard_scan_deadline(),
            b.unit_ai_runtime.guard_scan_deadline()
        );
    }
}
