//! CPP ScriptConditions1443–1540: consume this engine's first matching event.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

const PLAYER: &str = "OwnedEventPlayer";
const SOURCE: &str = "OwnedEventSource";
const EVENT: &str = "OwnedEventName";

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world.add_player(Player::new(1, Team::USA, PLAYER, true));
    let mut template = ThingTemplate::new("OwnedEventInfantry");
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("OwnedEventInfantry", 1, Vec3::ZERO)
        .unwrap();
    world.host_object_mut(id).unwrap().name = SOURCE.into();
    (world, id)
}

fn condition(kind: ConditionType, player: &str, event: &str, source: Option<&str>) -> Condition {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            if matches!(
                kind,
                ConditionType::PlayerBuiltUpgrade | ConditionType::PlayerBuiltUpgradeFromNamed
            ) {
                ParameterType::Upgrade
            } else {
                ParameterType::SpecialPower
            },
            event.into(),
        ))
        .unwrap();
    if let Some(source) = source {
        condition
            .add_parameter(Parameter::with_string(ParameterType::Unit, source.into()))
            .unwrap();
    }
    condition
}

fn engine(condition: Condition) -> ScriptEngine {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "EventObserved".into(),
        ))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "ObserveOwnedPlayerEvent".into();
    script.is_one_shot = false;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("EventObserved", 0).unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn tick(engine: &ScriptEngine, world: &mut GameLogic) -> i32 {
    engine
        .update_with_driver(
            gamelogic::scripting::executor::ScriptContext::at_frame(world.frame),
            &mut HostScriptExecutionDriver::new(world),
        )
        .unwrap();
    world.frame += 1;
    engine.get_counter("EventObserved").unwrap().value
}

fn notify(
    engine: &ScriptEngine,
    kind: ConditionType,
    player: usize,
    event: &str,
    source: ObjectId,
) {
    match kind {
        ConditionType::PlayerTriggeredSpecialPower
        | ConditionType::PlayerTriggeredSpecialPowerFromNamed => {
            engine.notify_of_triggered_special_power(player, event, source.0)
        }
        ConditionType::PlayerMidwaySpecialPower
        | ConditionType::PlayerMidwaySpecialPowerFromNamed => {
            engine.notify_of_midway_special_power(player, event, source.0)
        }
        ConditionType::PlayerCompletedSpecialPower
        | ConditionType::PlayerCompletedSpecialPowerFromNamed => {
            engine.notify_of_completed_special_power(player, event, source.0)
        }
        ConditionType::PlayerBuiltUpgrade | ConditionType::PlayerBuiltUpgradeFromNamed => {
            engine.notify_of_completed_upgrade(player, event, source.0)
        }
        _ => panic!("not a player event"),
    }
}

#[test]
fn player_event_action_walk_calibration() {
    let (mut world, _) = world();
    assert_eq!(
        tick(
            &engine(Condition::new(ConditionType::ConditionTrue)),
            &mut world
        ),
        1
    );
}

#[test]
fn all_player_event_kinds_are_immediate_and_one_shot() {
    for (kind, source) in [
        (ConditionType::PlayerTriggeredSpecialPower, None),
        (ConditionType::PlayerMidwaySpecialPower, None),
        (ConditionType::PlayerCompletedSpecialPower, None),
        (ConditionType::PlayerBuiltUpgrade, None),
        (
            ConditionType::PlayerTriggeredSpecialPowerFromNamed,
            Some(SOURCE),
        ),
        (
            ConditionType::PlayerMidwaySpecialPowerFromNamed,
            Some(SOURCE),
        ),
        (
            ConditionType::PlayerCompletedSpecialPowerFromNamed,
            Some(SOURCE),
        ),
        (ConditionType::PlayerBuiltUpgradeFromNamed, Some(SOURCE)),
    ] {
        let (mut world, id) = world();
        let engine = engine(condition(kind, PLAYER, EVENT, source));
        notify(&engine, kind, 1, EVENT, id);
        assert_eq!(
            tick(&engine, &mut world),
            1,
            "{kind:?} must consume own event"
        );
        assert_eq!(
            tick(&engine, &mut world),
            1,
            "{kind:?} must not remain true"
        );
    }
}

#[test]
fn player_events_select_driving_world_and_engine_with_identical_ids() {
    let (mut first, id) = world();
    let (mut second, second_id) = world();
    assert_eq!(id, second_id);
    second.players.get_mut(&1).unwrap().name = "ForeignPlayer".into();
    second.host_object_mut(id).unwrap().name = "ForeignSource".into();
    let kind = ConditionType::PlayerTriggeredSpecialPowerFromNamed;
    let first_engine = engine(condition(kind, PLAYER, EVENT, Some(SOURCE)));
    let second_engine = engine(condition(kind, PLAYER, EVENT, Some(SOURCE)));
    notify(&first_engine, kind, 1, EVENT, id);
    notify(&second_engine, kind, 1, EVENT, id);
    assert_eq!(tick(&second_engine, &mut second), 0);
    assert_eq!(
        tick(&first_engine, &mut first),
        1,
        "must use driving owner, not shared PlayerList"
    );
    assert!(second_engine.is_special_power_triggered(1, EVENT, false, id.0));
    notify(&first_engine, kind, 1, EVENT, id);
    assert!(first_engine.is_special_power_triggered(1, EVENT, false, id.0));
    second.reset();
    assert_eq!(tick(&first_engine, &mut first), 2);
    assert_eq!(tick(&first_engine, &mut first), 2);
}

#[test]
fn missing_player_does_not_consume_event() {
    let (mut world, id) = world();
    let kind = ConditionType::PlayerBuiltUpgrade;
    let engine = engine(condition(kind, "AbsentPlayer", EVENT, None));
    notify(&engine, kind, 1, EVENT, id);
    assert_eq!(tick(&engine, &mut world), 0);
    assert!(engine.is_upgrade_complete(1, EVENT, false, id.0));
    world.players.get_mut(&1).unwrap().name = "AbsentPlayer".into();
    assert_eq!(tick(&engine, &mut world), 1);
    assert_eq!(tick(&engine, &mut world), 1);
}

#[test]
fn missing_or_wrong_source_does_not_consume_event() {
    let (mut world, id) = world();
    let kind = ConditionType::PlayerCompletedSpecialPowerFromNamed;
    let engine = engine(condition(kind, PLAYER, EVENT, Some("AbsentSource")));
    notify(&engine, kind, 1, EVENT, id);
    assert_eq!(tick(&engine, &mut world), 0);
    assert!(engine.is_special_power_complete(1, EVENT, false, id.0));
    world.host_object_mut(id).unwrap().name = "AbsentSource".into();
    assert_eq!(tick(&engine, &mut world), 1);
    assert_eq!(tick(&engine, &mut world), 1);
}

#[test]
fn duplicate_events_remove_only_first_matching_source() {
    let (mut world, id) = world();
    let kind = ConditionType::PlayerMidwaySpecialPowerFromNamed;
    let engine = engine(condition(kind, PLAYER, EVENT, Some(SOURCE)));
    let other = ObjectId(id.0 + 100);
    notify(&engine, kind, 1, EVENT, other);
    notify(&engine, kind, 1, EVENT, id);
    notify(&engine, kind, 1, EVENT, id);
    assert_eq!(tick(&engine, &mut world), 1);
    assert_eq!(tick(&engine, &mut world), 2);
    assert_eq!(tick(&engine, &mut world), 2);
    assert!(engine.is_special_power_midway(1, EVENT, false, other.0));
    assert!(!engine.is_special_power_midway(1, EVENT, false, id.0));
}

#[test]
fn unnamed_event_accepts_any_source_without_requiring_live_object() {
    let (mut world, _) = world();
    let kind = ConditionType::PlayerBuiltUpgrade;
    let engine = engine(condition(kind, PLAYER, EVENT, None));
    notify(&engine, kind, 1, EVENT, ObjectId(99999));
    assert_eq!(tick(&engine, &mut world), 1);
    assert_eq!(tick(&engine, &mut world), 1);
}

#[test]
fn exact_authored_names_and_literal_event_names_are_preserved() {
    let (mut world, id) = world();
    world.players.get_mut(&1).unwrap().map_side.map_player_name = "AuthoredEventPlayer".into();
    let kind = ConditionType::PlayerTriggeredSpecialPowerFromNamed;
    let display_engine = engine(condition(kind, PLAYER, EVENT, Some(SOURCE)));
    notify(&display_engine, kind, 1, EVENT, id);
    assert_eq!(tick(&display_engine, &mut world), 0);
    assert!(display_engine.is_special_power_triggered(1, EVENT, false, id.0));
    let own_engine = engine(condition(
        kind,
        "AuthoredEventPlayer",
        gamelogic::scripting::core::LOCAL_PLAYER,
        Some(SOURCE),
    ));
    notify(
        &own_engine,
        kind,
        1,
        gamelogic::scripting::core::LOCAL_PLAYER,
        id,
    );
    assert_eq!(tick(&own_engine, &mut world), 1);
    let wrong_case = engine(condition(kind, "authoredeventplayer", EVENT, Some(SOURCE)));
    notify(&wrong_case, kind, 1, EVENT, id);
    assert_eq!(tick(&wrong_case, &mut world), 0);
}

#[test]
fn local_player_alias_resolves_within_driving_world() {
    let (mut world, id) = world();
    let kind = ConditionType::PlayerBuiltUpgrade;
    let engine = engine(condition(
        kind,
        gamelogic::scripting::core::LOCAL_PLAYER,
        EVENT,
        None,
    ));
    notify(&engine, kind, 1, EVENT, id);
    assert_eq!(tick(&engine, &mut world), 1);
}

#[test]
fn pending_player_event_tail_restores_future_consumption() {
    let (mut world, id) = world();
    let kind = ConditionType::PlayerCompletedSpecialPowerFromNamed;
    let original = engine(condition(kind, PLAYER, EVENT, Some(SOURCE)));
    notify(&original, kind, 1, EVENT, id);
    notify(&original, kind, 1, EVENT, id);
    let tail = original.snapshot_xfer_tail();
    assert_eq!(tick(&original, &mut world), 1);
    let restored = engine(condition(kind, PLAYER, EVENT, Some(SOURCE)));
    restored.restore_xfer_tail(&tail);
    assert_eq!(tick(&restored, &mut world), 1);
    assert_eq!(tick(&restored, &mut world), 2);
    assert_eq!(tick(&restored, &mut world), 2);
    assert!(original.is_special_power_complete(1, EVENT, false, id.0));
}

#[test]
fn this_object_identity_and_prepared_binding_admission_are_explicit() {
    use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};
    let (mut world, id) = world();
    {
        let driver = HostScriptExecutionDriver::new(&mut world);
        let selected = driver.player_event_source(
            PLAYER,
            Some(gamelogic::scripting::core::THIS_OBJECT),
            None,
            Some(id.0),
        );
        assert!(
            matches!(selected, ScriptOwnerQuery::Present(source) if source.source_object == id.0 && source.player_index == 1)
        );
        assert_eq!(
            driver.player_event_source(
                PLAYER,
                Some(gamelogic::scripting::core::THIS_OBJECT),
                None,
                Some(99999)
            ),
            ScriptOwnerQuery::Missing
        );
        assert_eq!(
            driver.player_event_source(PLAYER, Some(""), None, None),
            ScriptOwnerQuery::Missing
        );
    }
    world.game_mode = GameMode::Skirmish;
    assert_eq!(
        HostScriptExecutionDriver::new(&mut world).player_event_source(
            PLAYER,
            Some(SOURCE),
            None,
            None
        ),
        ScriptOwnerQuery::Unavailable
    );
}
