use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList, THIS_PLAYER,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, KindOf, Player, Team, ThingTemplate};

fn script(action_type: ScriptActionType, factory: bool, factory_name: &str) -> ScriptEngine {
    let mut action = ScriptAction::new(action_type);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    if factory {
        action
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                factory_name.into(),
            ))
            .unwrap();
    }
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut authored = Script::new();
    authored.script_name = "PlayerConstructionOwner".into();
    authored.condition = Some(Box::new(condition));
    authored.action = Some(Box::new(action));
    authored.is_one_shot = true;
    let mut list = ScriptList::new();
    list.append_script(Box::new(authored));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn enable_then_conditionally_disable_base() -> ScriptEngine {
    let mut enable = ScriptAction::new(ScriptActionType::PlayerEnableUnitConstruction);
    enable
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    let mut disable_base = ScriptAction::new(ScriptActionType::PlayerDisableBaseConstruction);
    disable_base
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();

    let mut first_condition = OrCondition::new();
    first_condition
        .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut first = Script::new();
    first.script_name = "EnableUnitsImmediately".into();
    first.condition = Some(Box::new(first_condition));
    first.action = Some(Box::new(enable));
    first.is_one_shot = true;

    let mut can_build = Condition::new(ConditionType::SkirmishPlayerHasPrerequisiteToBuild);
    can_build
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    can_build
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "ProbeUnit".into(),
        ))
        .unwrap();
    let mut second_condition = OrCondition::new();
    second_condition.set_first_and_condition(Some(Box::new(can_build)));
    let mut second = Script::new();
    second.script_name = "ObserveImmediateEnable".into();
    second.condition = Some(Box::new(second_condition));
    second.action = Some(Box::new(disable_base));
    second.is_one_shot = true;

    let mut list = ScriptList::new();
    list.append_script(Box::new(first));
    list.append_script(Box::new(second));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    engine
}

fn world(prefix: &str) -> GameLogic {
    let mut world = GameLogic::new();
    let player_name = format!("{prefix}Player");
    let mut player =
        Player::new(1, Team::USA, &player_name, false).with_script_identity(&player_name);
    player.set_can_build_units(true);
    player.set_can_build_base(true);
    world.add_player(player);
    let mut factory = ThingTemplate::new("SharedFactory");
    factory.add_kind_of(KindOf::Structure);
    world.templates.insert(factory.name.clone(), factory);
    world
        .create_object_for_player("SharedFactory", 1, glam::Vec3::ZERO)
        .unwrap();
    world
}

fn run(world: &mut GameLogic, action: ScriptActionType, factory: bool) {
    world
        .update_local_script_engine_with_owner(&script(action, factory, "SharedFactory"))
        .unwrap();
}

#[test]
fn construction_toggles_are_synchronous_and_world_owned_for_colliding_player_ids() {
    let mut alpha = world("Alpha");
    let mut beta = world("Beta");
    assert_eq!(
        alpha.get_player(1).unwrap().id,
        beta.get_player(1).unwrap().id
    );

    run(
        &mut alpha,
        ScriptActionType::PlayerDisableUnitConstruction,
        false,
    );
    assert!(!alpha.get_player(1).unwrap().can_build_units);
    assert!(beta.get_player(1).unwrap().can_build_units);
    run(
        &mut beta,
        ScriptActionType::PlayerDisableBaseConstruction,
        false,
    );
    assert!(!beta.get_player(1).unwrap().can_build_base);
    assert!(alpha.get_player(1).unwrap().can_build_base);
    let alpha_factory = alpha.objects.registered_values().next().unwrap().id;
    let beta_factory = beta.objects.registered_values().next().unwrap().id;
    alpha
        .update_local_script_engine_with_owner(&script(
            ScriptActionType::PlayerDisableFactories,
            true,
            "sharedfactory",
        ))
        .unwrap();
    assert!(
        !alpha
            .host_object(alpha_factory)
            .unwrap()
            .is_script_disabled(),
        "C++ compares factory template names exactly"
    );
    run(&mut alpha, ScriptActionType::PlayerDisableFactories, true);
    assert!(
        alpha
            .host_object(alpha_factory)
            .unwrap()
            .is_script_disabled()
    );
    assert!(!beta.host_object(beta_factory).unwrap().is_script_disabled());

    run(
        &mut alpha,
        ScriptActionType::PlayerEnableUnitConstruction,
        false,
    );
    run(
        &mut beta,
        ScriptActionType::PlayerEnableBaseConstruction,
        false,
    );
    run(&mut alpha, ScriptActionType::PlayerEnableFactories, true);
    assert!(alpha.get_player(1).unwrap().can_build_units);
    assert!(beta.get_player(1).unwrap().can_build_base);
    assert!(
        !alpha
            .host_object(alpha_factory)
            .unwrap()
            .is_script_disabled()
    );
}

#[test]
fn missing_receiving_this_player_does_not_mutate_local_or_peer_player() {
    let mut alpha = world("Alpha");
    let beta = world("Beta");
    assert!(alpha.select_local_player(Some(1)));
    // The receiving script is assigned to side 1, which has no player. The
    // local/only Player ID 1 must not be substituted for the missing selector.
    let mut action = ScriptAction::new(ScriptActionType::PlayerDisableUnitConstruction);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            THIS_PLAYER.into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut authored = Script::new();
    authored.script_name = "MissingConstructionRecipient".into();
    authored.condition = Some(Box::new(condition));
    authored.action = Some(Box::new(action));
    authored.is_one_shot = true;
    let mut list = ScriptList::new();
    list.append_script(Box::new(authored));
    let engine = ScriptEngine::new().unwrap();
    engine
        .set_script_list_for_player(1, Some(Box::new(list)))
        .unwrap();
    alpha
        .update_local_script_engine_with_owner(&engine)
        .unwrap();
    assert!(alpha.get_player(1).unwrap().can_build_units);
    assert!(beta.get_player(1).unwrap().can_build_units);
}

#[test]
fn following_script_condition_observes_enable_in_the_same_world_walk() {
    let mut world = world("Immediate");
    world.get_player_mut(1).unwrap().set_can_build_units(false);
    let mut unit = ThingTemplate::new("ProbeUnit");
    unit.add_kind_of(KindOf::Infantry);
    world.templates.insert(unit.name.clone(), unit);

    world
        .update_local_script_engine_with_owner(&enable_then_conditionally_disable_base())
        .unwrap();

    assert!(world.get_player(1).unwrap().can_build_units);
    assert!(!world.get_player(1).unwrap().can_build_base);
}
