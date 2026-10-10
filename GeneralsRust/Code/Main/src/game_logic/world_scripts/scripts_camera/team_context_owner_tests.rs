//! Exact C++ Team* contexts through the actual Main script driver.
use super::named_command_test_support::world;
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_TEAM,
};
use gamelogic::scripting::engine::{ScriptEngine, ScriptExecutionDriver, ScriptOwnerQuery};

fn admit(world: &GameLogic, name: &str, member: ObjectId, owner: u32) -> u32 {
    let mut factory = world.team_factory.lock().unwrap();
    if factory.find_team_prototype(name).is_none() {
        factory
            .init_team(name.into(), "".into(), false, None)
            .unwrap();
    }
    let team = factory.create_inactive_team(name).unwrap();
    let mut team = team.write().unwrap();
    team.add_member(member.0);
    team.set_controlling_player_id(Some(owner));
    team.get_id()
}

fn guard(name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, name.into()))
        .unwrap();
    action
}

fn increment(name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Counter, name.into()))
        .unwrap();
    action
}

fn update(engine: &ScriptEngine, world: &mut GameLogic) {
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
}

fn sequence_engine() -> ScriptEngine {
    let mut body = guard(THIS_TEAM);
    body.next_action = Some(Box::new(increment("AfterGuard")));
    let mut script = Script::new();
    script.script_name = "ExactSequence".into();
    script.is_subroutine = true;
    script.action = Some(Box::new(body));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("AfterGuard", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn start(engine: &ScriptEngine, world: &mut GameLogic, id: u32) {
    let mut action = ScriptAction::new(ScriptActionType::TeamExecuteSequentialScriptLooping);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            THIS_TEAM.into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            "ExactSequence".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        Some(id),
        gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
        &mut HostScriptExecutionDriver::new(world),
    );
}

#[test]
fn literal_context_selects_older_same_name_instance_and_missing_id_cannot_retarget() {
    let (mut world, first, second) = world();
    let older = admit(&world, "SiblingRoster", first, 1);
    let newer = admit(&world, "SiblingRoster", second, 2);
    let engine = ScriptEngine::new().unwrap();
    engine.friend_execute_action_with_driver(
        &guard("SiblingRoster"),
        Some(older),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert_eq!(
        world.host_object(first).unwrap().ai_state,
        AIState::GuardingArea
    );
    assert_eq!(world.host_object(second).unwrap().guard_position, None);
    engine.friend_execute_action_with_driver(
        &guard(THIS_TEAM),
        Some(u32::MAX),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert_eq!(world.host_object(second).unwrap().guard_position, None);
    let driver = HostScriptExecutionDriver::new(&mut world);
    assert_eq!(
        driver.team_instances("SiblingRoster"),
        ScriptOwnerQuery::Present(vec![newer, older])
    );
    assert_eq!(driver.team_name(u32::MAX), ScriptOwnerQuery::Missing);
    assert_eq!(
        driver.team_current_player(older),
        ScriptOwnerQuery::Present(Some("NamedOwner".into()))
    );
    assert_eq!(
        driver.team_current_player(newer),
        ScriptOwnerQuery::Present(Some("NamedVictim".into()))
    );
}

#[test]
fn sequential_start_keeps_same_name_chains_distinct_and_stops_only_selected_context() {
    let (mut world, first, second) = world();
    let first_id = admit(&world, "SiblingRoster", first, 1);
    let second_id = admit(&world, "SiblingRoster", second, 2);
    world
        .host_object_mut(first)
        .unwrap()
        .set_ai_state(AIState::Moving);
    world
        .host_object_mut(second)
        .unwrap()
        .set_ai_state(AIState::Moving);
    let engine = sequence_engine();
    start(&engine, &mut world, first_id);
    assert_eq!(world.host_object(first).unwrap().ai_state, AIState::Idle);
    assert_eq!(world.host_object(second).unwrap().ai_state, AIState::Moving);
    assert_eq!(
        world.host_object(first).unwrap().last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(world.host_object(first).unwrap().formation_id, 19);
    assert_eq!(
        world
            .host_object(first)
            .unwrap()
            .cur_locomotor_name
            .as_deref(),
        Some("RedguardLocomotor")
    );
    start(&engine, &mut world, second_id);
    let before = engine.snapshot_sequential_scripts();
    assert_eq!(
        before.iter().map(|s| s.team_id).collect::<Vec<_>>(),
        vec![first_id, second_id]
    );
    assert!(before.iter().all(|s| s.times_to_loop == 2));
    update(&engine, &mut world);
    assert_eq!(
        world.host_object(first).unwrap().ai_state,
        AIState::GuardingArea
    );
    assert_eq!(
        world.host_object(second).unwrap().ai_state,
        AIState::GuardingArea
    );
    assert_eq!(engine.get_counter("AfterGuard").unwrap().value, 0);
    let mut stop = ScriptAction::new(ScriptActionType::TeamStopSequentialScript);
    stop.add_parameter(Parameter::with_string(
        ParameterType::Team,
        THIS_TEAM.into(),
    ))
    .unwrap();
    engine.friend_execute_action_with_driver(
        &stop,
        Some(first_id),
        gamelogic::scripting::executor::ScriptContext::at_frame(0),
        &mut HostScriptExecutionDriver::new(&mut world),
    );
    assert!(!engine.has_active_sequential_script_for_team(first_id));
    assert!(engine.has_active_sequential_script_for_team(second_id));
    assert_eq!(engine.get_calling_team_id(), None);
}

#[test]
fn same_ids_in_two_worlds_keep_context_sequence_effects_and_reset_independent() {
    let (mut first_world, first, _) = world();
    let (mut second_world, second, _) = world();
    let first_id = admit(&first_world, "SiblingRoster", first, 1);
    let second_id = admit(&second_world, "SiblingRoster", second, 2);
    assert_eq!((first_id, first), (second_id, second));
    second_world
        .host_object_mut(second)
        .unwrap()
        .set_position(Vec3::splat(500.0));
    let first_engine = sequence_engine();
    let second_engine = sequence_engine();
    start(&first_engine, &mut first_world, first_id);
    start(&second_engine, &mut second_world, second_id);
    update(&first_engine, &mut first_world);
    assert_eq!(
        second_world.host_object(second).unwrap().guard_position,
        None
    );
    update(&second_engine, &mut second_world);
    assert_eq!(
        first_world.host_object(first).unwrap().guard_position,
        Some(Vec3::new(50.0, 0.0, 50.0))
    );
    assert_eq!(
        second_world.host_object(second).unwrap().guard_position,
        Some(Vec3::splat(500.0))
    );
    let retained = first_engine.snapshot_sequential_scripts();
    second_world.reset();
    assert_eq!(first_engine.snapshot_sequential_scripts(), retained);
    assert_eq!(
        first_world.host_object(first).unwrap().ai_state,
        AIState::GuardingArea
    );
}

#[test]
fn exact_sequence_snapshot_restore_preserves_selected_sibling_identity() {
    let (mut source, first, second) = world();
    let older = admit(&source, "SiblingRoster", first, 1);
    let newer = admit(&source, "SiblingRoster", second, 2);
    let engine = sequence_engine();
    start(&engine, &mut source, older);
    let saved = engine.snapshot_sequential_scripts();
    assert_eq!(saved[0].team_id, older);
    assert_ne!(saved[0].team_id, newer);
    let (mut restored, first, second) = world();
    assert_eq!(admit(&restored, "SiblingRoster", first, 1), older);
    assert_eq!(admit(&restored, "SiblingRoster", second, 2), newer);
    let restored_engine = sequence_engine();
    restored_engine.restore_sequential_scripts(&saved);
    update(&restored_engine, &mut restored);
    assert_eq!(
        restored.host_object(first).unwrap().ai_state,
        AIState::GuardingArea
    );
    assert_eq!(restored.host_object(second).unwrap().guard_position, None);
}

#[test]
fn retained_missing_team_id_rejects_before_persist_tail_mutates_receiver() {
    use crate::save_load::snapshot::persist_v18::{
        SequentialScriptPersist, WorldPersistV18, restore_persist_v18,
    };
    let (mut world, id, _) = world();
    let before = world.host_object(id).unwrap().ai_state.clone();
    let mut persist = WorldPersistV18::default();
    persist.script_sequential.push(SequentialScriptPersist {
        team_id: u32::MAX,
        object_id: 0,
        script_name: "MissingTeamSequence".into(),
        current_instruction: -1,
        times_to_loop: 0,
        frames_to_wait: 0,
        dont_advance_instruction: false,
    });
    let error = restore_persist_v18(&persist, &mut world).unwrap_err();
    assert!(error.to_string().contains("missing team"));
    assert_eq!(world.host_object(id).unwrap().ai_state, before);
}

#[test]
fn sequence_idle_contains_cycle_is_bounded_and_still_visits_each_live_member() {
    let (mut world, first, second) = world();
    let id = admit(&world, "CycleRoster", first, 1);
    world.host_object_mut(first).unwrap().occupants = vec![second];
    world.host_object_mut(second).unwrap().occupants = vec![first, ObjectId(u32::MAX)];
    world
        .host_object_mut(first)
        .unwrap()
        .set_ai_state(AIState::Moving);
    world
        .host_object_mut(second)
        .unwrap()
        .set_ai_state(AIState::Moving);
    let engine = sequence_engine();
    start(&engine, &mut world, id);
    for member in [first, second] {
        let unit = world.host_object(member).unwrap();
        assert_eq!(unit.ai_state, AIState::Idle);
        assert_eq!(
            unit.last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
        );
    }
    assert!(engine.has_active_sequential_script_for_team(id));
}

fn attitude(engine: &ScriptEngine, world: &mut GameLogic, id: Option<u32>, raw: &str, mood: i32) {
    let mut action = ScriptAction::new(ScriptActionType::TeamSetAttitude);
    action
        .add_parameter(Parameter::with_string(ParameterType::Team, raw.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, mood))
        .unwrap();
    engine.friend_execute_action_with_driver(
        &action,
        id,
        gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
        &mut HostScriptExecutionDriver::new(world),
    );
}

#[test]
fn team_attitude_same_ids_in_two_worlds_select_only_exact_older_context() {
    let (mut first_world, older_member, newer_member) = world();
    let (mut foreign, foreign_member, _) = world();
    let older = admit(&first_world, "SiblingRoster", older_member, 1);
    let newer = admit(&first_world, "SiblingRoster", newer_member, 1);
    let foreign_id = admit(&foreign, "ForeignRoster", foreign_member, 1);
    assert_eq!(older, foreign_id);
    assert_ne!(older, newer);
    for id in [older_member, newer_member] {
        first_world.host_object_mut(id).unwrap().team_instance_name = "ForeignRoster".into();
    }
    let engine = ScriptEngine::new().unwrap();
    attitude(&engine, &mut first_world, Some(older), THIS_TEAM, -2);
    assert_eq!(
        first_world.host_object(older_member).unwrap().ai_attitude,
        -2
    );
    assert_eq!(
        first_world.host_object(newer_member).unwrap().ai_attitude,
        0
    );
    assert_eq!(foreign.host_object(foreign_member).unwrap().ai_attitude, 0);
    // Literal equality selects the same exact contextual instance, not head.
    attitude(&engine, &mut first_world, Some(older), "SiblingRoster", 2);
    assert_eq!(
        first_world.host_object(older_member).unwrap().ai_attitude,
        2
    );
    assert_eq!(
        first_world.host_object(newer_member).unwrap().ai_attitude,
        0
    );
    attitude(&engine, &mut foreign, Some(foreign_id), THIS_TEAM, -1);
    assert_eq!(foreign.host_object(foreign_member).unwrap().ai_attitude, -1);
    assert_eq!(
        first_world.host_object(older_member).unwrap().ai_attitude,
        2
    );
}

#[test]
fn team_attitude_modifies_dead_immobile_ai_but_skips_non_ai_and_missing_context() {
    let (mut world, first, second) = world();
    let id = admit(&world, "AttitudeRoster", first, 1);
    world
        .team_factory
        .lock()
        .unwrap()
        .find_team_by_id(id)
        .unwrap()
        .write()
        .unwrap()
        .add_member(second.0);
    world
        .host_object_mut(first)
        .unwrap()
        .status
        .effectively_dead = true;
    world
        .host_object_mut(first)
        .unwrap()
        .template_mut()
        .add_kind_of(KindOf::Immobile);
    world
        .host_object_mut(second)
        .unwrap()
        .template_mut()
        .set_authored_ai_update_interface(Some(false));
    let engine = ScriptEngine::new().unwrap();
    attitude(&engine, &mut world, Some(id), THIS_TEAM, 3);
    assert_eq!(world.host_object(first).unwrap().ai_attitude, 3);
    assert_eq!(world.host_object(second).unwrap().ai_attitude, 0);
    attitude(&engine, &mut world, Some(u32::MAX), THIS_TEAM, -2);
    assert_eq!(world.host_object(first).unwrap().ai_attitude, 3);
}

#[test]
fn sequence_script_idle_admits_immobile_ai_without_locomotor_preparation() {
    let (mut world, first, _) = world();
    let id = admit(&world, "IdleRoster", first, 1);
    let unit = world.host_object_mut(first).unwrap();
    unit.template_mut().add_kind_of(KindOf::Immobile);
    unit.set_ai_state(AIState::Moving);
    let engine = sequence_engine();
    start(&engine, &mut world, id);
    let unit = world.host_object(first).unwrap();
    assert_eq!(unit.ai_state, AIState::Idle);
    assert_eq!(
        unit.last_command_source,
        crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
    );
    assert_eq!(
        unit.cur_locomotor_name.as_deref(),
        Some("RedguardLocomotor")
    );
    assert_eq!(unit.formation_id, 19);
}

#[test]
fn sequence_script_idle_rejects_dead_sleep_surrender_and_projectile_members() {
    let rejects: [fn(&mut GameLogic, ObjectId); 4] = [
        |world, id| world.host_object_mut(id).unwrap().status.effectively_dead = true,
        |world, id| {
            world.get_player_mut(1).unwrap().is_human = false;
            world.host_object_mut(id).unwrap().set_ai_attitude_i8(-2);
        },
        |world, id| world.host_object_mut(id).unwrap().is_surrendered = true,
        |world, id| {
            world
                .host_object_mut(id)
                .unwrap()
                .template_mut()
                .add_kind_of(KindOf::Projectile);
        },
    ];
    for reject in rejects {
        let (mut world, rejected, accepted) = world();
        let team_id = admit(&world, "IdleRoster", rejected, 1);
        world
            .team_factory
            .lock()
            .unwrap()
            .find_team_by_id(team_id)
            .unwrap()
            .write()
            .unwrap()
            .add_member(accepted.0);
        for id in [rejected, accepted] {
            world
                .host_object_mut(id)
                .unwrap()
                .set_ai_state(AIState::Moving);
        }
        reject(&mut world, rejected);
        let before = world.host_object(rejected).unwrap().last_command_source;
        let engine = sequence_engine();
        start(&engine, &mut world, team_id);
        assert_eq!(
            world.host_object(rejected).unwrap().ai_state,
            AIState::Moving
        );
        assert_eq!(
            world.host_object(rejected).unwrap().last_command_source,
            before
        );
        assert_eq!(world.host_object(accepted).unwrap().ai_state, AIState::Idle);
        assert_eq!(
            world.host_object(accepted).unwrap().last_command_source,
            crate::game_logic::host_command_button_hunt::HUNT_CMD_FROM_SCRIPT
        );
    }
}
