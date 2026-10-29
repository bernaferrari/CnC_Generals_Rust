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

#[test]
fn script_flag_and_disabled_mask_are_distinct_and_survive_restore() {
    let mut a = world("Alpha");
    let mut b = world("Beta");
    let id = a.objects.registered_values().next().unwrap().id;
    let peer_id = b.objects.registered_values().next().unwrap().id;
    assert_eq!(id, peer_id);
    // A parent can propagate a disabled type without changing a child's
    // independent script flag.
    let child = a.host_object_mut(id).unwrap();
    child.set_status_disabled_script_disabled(true);
    assert!(!child.is_script_disabled());
    assert!(child.status.disabled_script_disabled);
    // An independently set script flag can outlive the propagated mask.
    run(&mut b, ScriptActionType::PlayerDisableFactories, true);
    let scripted = b.host_object_mut(id).unwrap();
    scripted.set_status_disabled_script_disabled(false);
    assert!(scripted.is_script_disabled());
    assert!(!scripted.status.disabled_script_disabled);

    let builder = generals_main::save_load::snapshot::SnapshotBuilder::new();
    let saved_a = builder.create_world_snapshot(&a).unwrap();
    let saved_b = builder.create_world_snapshot(&b).unwrap();
    let mut restored_a = world("Alpha");
    let mut restored_b = world("Beta");
    builder
        .restore_from_snapshot(&saved_a, &mut restored_a)
        .unwrap();
    builder
        .restore_from_snapshot(&saved_b, &mut restored_b)
        .unwrap();
    let a_child = restored_a.host_object(id).unwrap();
    assert!(!a_child.is_script_disabled());
    assert!(a_child.status.disabled_script_disabled);
    let b_child = restored_b.host_object(id).unwrap();
    assert!(b_child.is_script_disabled());
    assert!(!b_child.status.disabled_script_disabled);
    run(
        &mut restored_b,
        ScriptActionType::PlayerEnableFactories,
        true,
    );
    run(
        &mut restored_b,
        ScriptActionType::PlayerDisableFactories,
        true,
    );
    assert!(
        restored_b
            .host_object(id)
            .unwrap()
            .status
            .disabled_script_disabled
    );

    // ODIS v1 contained only the disabled mask. Its only possible script
    // interpretation is the same bit; keep it readable after the split.
    let mut old = builder.create_world_snapshot(&restored_b).unwrap();
    let offset = old
        .lifecycle_tail
        .windows(4)
        .position(|bytes| bytes == b"ODIS")
        .expect("ODIS continuation present");
    let old_length = u32::from_le_bytes(
        old.lifecycle_tail[offset + 8..offset + 12]
            .try_into()
            .unwrap(),
    ) as usize;
    let payload = bincode_legacy::serialize(&vec![(id.0, false, false, 0u32, 0u32)]).unwrap();
    let mut v1 = b"ODIS".to_vec();
    v1.extend_from_slice(&1u32.to_le_bytes());
    v1.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    v1.extend_from_slice(&payload);
    old.lifecycle_tail
        .splice(offset..offset + 12 + old_length, v1);
    let mut legacy_receiver = world("Beta");
    builder
        .restore_from_snapshot(&old, &mut legacy_receiver)
        .unwrap();
    assert!(
        legacy_receiver
            .host_object(id)
            .unwrap()
            .is_script_disabled()
    );
    assert!(
        legacy_receiver
            .host_object(id)
            .unwrap()
            .status
            .disabled_script_disabled
    );
}

#[test]
fn factory_disable_visits_only_its_ordered_spawn_children_in_each_world() {
    use game_engine::common::ini::ini_locomotor::LocomotorStore;
    use generals_main::game_logic::{AIState, ObjectId};
    use generals_main::save_load::snapshot::{SnapshotBuilder, capture_lifecycle_tail};

    fn world_with_spawns(prefix: &str, has_ai: bool) -> (GameLogic, ObjectId, Vec<ObjectId>) {
        let mut world = GameLogic::new();
        let mut locomotors = LocomotorStore::new_owned();
        locomotors.apply_source("Locomotor GuardWalk\n Surfaces = GROUND\n Speed = 60\n Acceleration = 60\n TurnRate = 120\n Appearance = TWO_LEGS\nEnd\n").unwrap();
        world.admit_locomotor_definitions(locomotors).unwrap();
        let ai_module = if has_ai {
            " Behavior = AIUpdateInterface Brain\n End\n"
        } else {
            ""
        };
        world.admit_object_ini_sources([(
            "SpawnFactory.ini".into(),
            format!("Object SharedFactory\n KindOf = STRUCTURE SELECTABLE SPAWNS_ARE_THE_WEAPONS\n Body = ActiveBody FactoryBody\n MaxHealth = 1000\n InitialHealth = 1000\n End\n Behavior = SpawnBehavior FactorySpawn\n OneShot = Yes\n SpawnNumber = 2\n SpawnTemplateName = SpawnedGuard\n End\nEnd\nObject SpawnedGuard\n KindOf = INFANTRY SELECTABLE\n Locomotor = SET_NORMAL GuardWalk\n Body = ActiveBody GuardBody\n MaxHealth = 100\n InitialHealth = 100\n End\n{ai_module}End\n"),
        )]).unwrap();
        let name = format!("{prefix}Player");
        world.add_player(Player::new(1, Team::USA, &name, false).with_script_identity(&name));
        let factory = world
            .create_object_for_player("SharedFactory", 1, glam::Vec3::ZERO)
            .unwrap();
        let tail = capture_lifecycle_tail(&world).unwrap();
        let roster = tail
            .spawn_behavior_rosters
            .unwrap()
            .into_iter()
            .find(|roster| roster.producer_id == factory.0)
            .unwrap();
        let children: Vec<_> = roster
            .children
            .into_iter()
            .map(|child| ObjectId(child.child_id))
            .collect();
        assert_eq!(children.len(), 2);
        (world, factory, children)
    }

    let (mut alpha, factory, children) = world_with_spawns("Alpha", true);
    let (mut beta, peer_factory, peer_children) = world_with_spawns("Beta", false);
    assert_eq!((factory, &children), (peer_factory, &peer_children));
    let unrelated = alpha
        .create_object_for_player("SpawnedGuard", 1, glam::Vec3::ZERO)
        .unwrap();
    alpha.host_object_mut(unrelated).unwrap().producer_id = Some(factory);
    assert!(
        alpha
            .unit_command_set_ai_state(children[0], AIState::Moving)
            .unwrap()
    );
    assert_eq!(
        alpha.host_object(children[0]).unwrap().ai_state(),
        AIState::Moving
    );
    run(&mut alpha, ScriptActionType::PlayerDisableFactories, true);
    assert_eq!(
        alpha.host_object(children[0]).unwrap().ai_state(),
        AIState::Idle
    );
    for child in &children {
        let object = alpha.host_object(*child).unwrap();
        assert!(object.status.disabled_script_disabled);
        assert!(!object.is_script_disabled());
        assert!(
            !beta
                .host_object(*child)
                .unwrap()
                .status
                .disabled_script_disabled
        );
    }
    assert!(
        !alpha
            .host_object(unrelated)
            .unwrap()
            .status
            .disabled_script_disabled
    );
    run(&mut beta, ScriptActionType::PlayerDisableFactories, true);
    for child in &children {
        let object = beta.host_object(*child).unwrap();
        assert!(!object.has_ai_update_interface());
        assert!(object.status.disabled_script_disabled);
    }

    // The child's own script bit remains distinct when the parent's clear
    // removes only the propagated disabled mask.
    alpha
        .update_local_script_engine_with_owner(&script(
            ScriptActionType::PlayerDisableFactories,
            true,
            "SpawnedGuard",
        ))
        .unwrap();
    let snapshot = SnapshotBuilder::new()
        .create_world_snapshot(&alpha)
        .unwrap();
    let (mut restored, _, _) = world_with_spawns("Alpha", true);
    SnapshotBuilder::new()
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    run(&mut restored, ScriptActionType::PlayerEnableFactories, true);
    for child in &children {
        let object = restored.host_object(*child).unwrap();
        assert!(object.is_script_disabled());
        assert!(!object.status.disabled_script_disabled);
        assert!(
            beta.host_object(*child)
                .unwrap()
                .status
                .disabled_script_disabled
        );
    }
}
