use super::super::named_command_test_support::{named_infantry, world_with};
use super::super::*;
use gamelogic::scripting::engine::ScriptEngine;
use generals_sim::scripting::program::{
    Condition, ConditionType, Parameter, ParameterType, Script, ScriptAction, ScriptActionType,
    THIS_TEAM,
};
use generals_sim::scripting::sequential_scripts::SequentialScript;

const POWER: &str = "ReadinessExactPower";
const BUTTON: &str = "ReadinessPowerButton";
const UPGRADE: &str = "Upgrade_ReadinessObject";
const PREREQUISITE: &str = "Upgrade_ReadinessPlayer";
const CONFLICT: &str = "Upgrade_ReadinessConflict";
const ROSTER: &str = "ReadinessRoster";

fn fixture() -> (GameLogic, ObjectId, ObjectId, ObjectId) {
    let (mut world, _, _) = world_with(
        &[
            named_infantry("ReadinessExact").module(format!(
                "Behavior = SpecialAbility ModuleTag_Exact\n SpecialPowerTemplate = {POWER}\nEnd"
            )),
            named_infantry("ReadinessAlias").module(
                "Behavior = SpecialAbility ModuleTag_Alias\n SpecialPowerTemplate = ReadinessAliasPower\nEnd",
            ),
            named_infantry("ReadinessUpgrade").module(format!(
                "Behavior = MaxHealthUpgrade ModuleTag_Health\n TriggeredBy = {UPGRADE} {PREREQUISITE}\n RequiresAllTriggers = Yes\n ConflictsWith = {CONFLICT}\n AddMaxHealth = 20\n ChangeType = ADD_CURRENT_HEALTH_TOO\nEnd"
            )),
        ],
        &[],
        &format!(
            "SpecialPower {POWER}\n Enum = SPECIAL_REMOTE_CHARGES\n ReloadTime = 15000\n SharedSyncedTimer = Yes\nEnd\n\
             SpecialPower ReadinessAliasPower\n Enum = SPECIAL_REMOTE_CHARGES\n ReloadTime = 15000\nEnd\n\
             CommandButton {BUTTON}\n Command = SPECIAL_POWER\n SpecialPower = {POWER}\nEnd\n\
             CommandButton ReadinessPlainButton\n Command = STOP\nEnd\n\
             CommandButton ReadinessUpgradeButton\n Command = OBJECT_UPGRADE\n Upgrade = {UPGRADE}\nEnd\n"
        ),
    );
    for (name, scope) in [
        (UPGRADE, "OBJECT"),
        (PREREQUISITE, "PLAYER"),
        (CONFLICT, "PLAYER"),
    ] {
        let mut ini = game_engine::common::ini::INI::new();
        ini.with_inline_source(&format!("Upgrade {name}\nType = {scope}\nEnd\n"), |ini| {
            ini.read_line()?;
            world.upgrade_definitions.parse_upgrade_definition(ini)
        })
        .unwrap();
    }
    let exact = world
        .create_object_for_player("ReadinessExact", 1, Vec3::ZERO)
        .unwrap();
    let alias = world
        .create_object_for_player("ReadinessAlias", 1, Vec3::ZERO)
        .unwrap();
    let upgrade = world
        .create_object_for_player("ReadinessUpgrade", 1, Vec3::ZERO)
        .unwrap();
    (world, exact, alias, upgrade)
}

fn roster(world: &GameLogic, name: &str, members: &[ObjectId]) -> u32 {
    let mut factory = world.team_factory.lock().unwrap();
    if factory.find_team_prototype(name).is_none() {
        factory
            .init_team_owned(name.into(), "NamedOwner".into(), false, None, Some(1))
            .unwrap();
    }
    let team = factory.create_team(name).unwrap();
    let mut team = team.write().unwrap();
    for member in members.iter().rev() {
        team.add_member(member.0);
    }
    team.get_id()
}

fn parameters(name: &str, button: &str) -> [Parameter; 3] {
    [
        Parameter::with_string(ParameterType::Side, "NoSuchSide".into()),
        Parameter::with_string(ParameterType::Team, name.into()),
        Parameter::with_string(ParameterType::CommandButton, button.into()),
    ]
}

fn condition(world: &mut GameLogic, all: bool, name: &str, button: &str) -> bool {
    let mut condition = Condition::new(if all {
        ConditionType::SkirmishCommandButtonReadyAll
    } else {
        ConditionType::SkirmishCommandButtonReadyPartial
    });
    for parameter in parameters(name, button) {
        condition.add_parameter(parameter).unwrap();
    }
    world.owned_script_condition_for_test(condition)
}

fn set_shared_timer(world: &mut GameLogic, seconds: f32) {
    world
        .get_player_mut(1)
        .unwrap()
        .authored_special_power_timers
        .insert(POWER.to_ascii_lowercase(), seconds);
}

#[test]
fn command_button_conditions_read_the_receiving_exact_module_and_type() {
    let (mut world, exact, alias, unrelated) = fixture();
    let (mut peer, peer_exact, peer_alias, _) = fixture();
    assert_eq!((exact, alias), (peer_exact, peer_alias));
    assert_eq!(
        roster(&world, ROSTER, &[exact, alias]),
        roster(&peer, ROSTER, &[peer_exact, peer_alias])
    );
    roster(&world, "UnrelatedPowerRoster", &[unrelated]);
    set_shared_timer(&mut world, 12.0);
    set_shared_timer(&mut peer, 4.0);
    world.inject_host_script_query_snapshot();

    assert!(!condition(&mut world, false, ROSTER, BUTTON));
    set_shared_timer(&mut world, 0.0);
    world
        .host_object_mut(exact)
        .unwrap()
        .status
        .disabled_unmanned = true;
    assert!(condition(&mut world, false, ROSTER, BUTTON));
    assert!(!condition(&mut peer, false, ROSTER, BUTTON));
    assert!(
        !condition(&mut world, true, ROSTER, BUTTON),
        "same type, different template"
    );
    assert!(condition(&mut world, true, "UnrelatedPowerRoster", BUTTON));
    assert!(!condition(
        &mut world,
        false,
        "UnrelatedPowerRoster",
        BUTTON
    ));
    assert!(condition(&mut world, true, ROSTER, "ReadinessPlainButton"));
    assert!(!condition(
        &mut world,
        false,
        ROSTER,
        "ReadinessPlainButton"
    ));
    assert!(!condition(&mut world, true, ROSTER, "AbsentButton"));
    assert!(!condition(&mut world, true, "AbsentRoster", BUTTON));
}

#[test]
fn command_button_conditions_observe_combined_upgrade_masks_and_completion() {
    let (mut world, _, _, object) = fixture();
    let (_, button) = (roster(&world, ROSTER, &[object]), "ReadinessUpgradeButton");
    assert!(!condition(&mut world, false, ROSTER, button));
    world
        .get_player_mut(1)
        .unwrap()
        .add_completed_upgrade(PREREQUISITE);
    assert!(condition(&mut world, true, ROSTER, button));
    world
        .get_player_mut(1)
        .unwrap()
        .add_completed_upgrade(CONFLICT);
    assert!(!condition(&mut world, false, ROSTER, button));
    world
        .get_player_mut(1)
        .unwrap()
        .completed_upgrades
        .remove(CONFLICT);
    world.apply_upgrade_to_object(object, UPGRADE);
    assert!(
        world
            .host_object(object)
            .unwrap()
            .has_object_upgrade_complete(UPGRADE)
    );
    assert!(!condition(&mut world, false, ROSTER, button));
}

#[test]
fn command_button_waits_hold_the_exact_sequential_actor_until_ready() {
    for all in [true, false] {
        let (mut world, exact, alias, _) = fixture();
        let team = roster(&world, ROSTER, &[exact]);
        assert_ne!(team, roster(&world, ROSTER, &[alias]));
        let engine = ScriptEngine::new_in_published_world().unwrap();
        engine.set_counter("ReadinessResumed", 0).unwrap();
        let mut wait = ScriptAction::new(if all {
            ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll
        } else {
            ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial
        });
        for parameter in parameters(THIS_TEAM, BUTTON) {
            wait.add_parameter(parameter).unwrap();
        }
        let mut increment = ScriptAction::new(ScriptActionType::IncrementCounter);
        increment
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        increment
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "ReadinessResumed".into(),
            ))
            .unwrap();
        wait.next_action = Some(Box::new(increment));
        let mut script = Script::new();
        script.script_name = "ReadinessSequence".into();
        script.is_subroutine = true;
        script.action = Some(Box::new(wait));
        let mut sequence = SequentialScript::new();
        sequence.team_to_exec_on = Some(team);
        sequence.script_to_execute_sequentially = Some(Box::new(script));
        engine.append_sequential_script(sequence);

        set_shared_timer(&mut world, 6.0);
        for frame in 100..102 {
            world.frame = frame;
            world
                .update_local_script_engine_with_owner(&engine)
                .unwrap();
            assert_eq!(engine.get_counter("ReadinessResumed").unwrap().value, 0);
            let heads = engine.snapshot_sequential_scripts();
            assert_eq!(heads[0].current_instruction, 0);
            assert!(heads[0].dont_advance_instruction);
        }
        set_shared_timer(&mut world, 0.0);
        world.frame = 102;
        world
            .update_local_script_engine_with_owner(&engine)
            .unwrap();
        assert_eq!(engine.get_counter("ReadinessResumed").unwrap().value, 1);
        assert!(engine.snapshot_sequential_scripts().is_empty());
    }
}
