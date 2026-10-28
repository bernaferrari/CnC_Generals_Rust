use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use generals_main::game_logic::{GameLogic, GameMode, ObjectId, Player, Team};
use generals_main::save_load::{SnapshotBuilder, snapshot::WorldSnapshot};
use glam::Vec3;

const SOURCE: &str = "CostModifierOwnerSource";
const VETERAN: &str = "Upgrade_Veterancy_VETERAN";

fn world(percent: &str, create_source: bool) -> (GameLogic, Option<ObjectId>) {
    let mut world = GameLogic::new();
    world
        .admit_object_ini_sources([(
            "CostModifierOwner.ini".into(),
            format!(
                r#"
Object {SOURCE}
 KindOf = STRUCTURE SELECTABLE
 Body = ActiveBody SourceBody
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = CostModifierUpgrade AuthoredCost
  TriggeredBy = {VETERAN}
  EffectKindOf = VEHICLE AIRCRAFT
  Percentage = {percent}
 End
End
Object CostModifierVehicle
 KindOf = VEHICLE SELECTABLE
 BuildCost = 1000
 Body = ActiveBody VehicleBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
Object CostModifierDualKind
 KindOf = VEHICLE AIRCRAFT SELECTABLE
 BuildCost = 1000
 Body = ActiveBody DualBody
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#
            ),
        )])
        .expect("admit typed CostModifierUpgrade source and production targets");
    world.start_new_game(GameMode::SinglePlayer);
    world.add_player(Player::new(1, Team::USA, "CostOwnerA", true));
    world.add_player(Player::new(2, Team::China, "CostOwnerB", true));
    let source = create_source.then(|| {
        world
            .create_object_for_player(SOURCE, 1, Vec3::ZERO)
            .expect("create receiving upgrade source")
    });
    (world, source)
}

fn receive_veteran_upgrade(world: &mut GameLogic, source: ObjectId) {
    let object = world.host_object_mut(source).expect("source exists");
    object.name = "CostModifierOwnerNamedSource".into();

    let mut action = ScriptAction::new(ScriptActionType::NamedReceiveUpgrade);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "CostModifierOwnerNamedSource".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            VETERAN.into(),
        ))
        .unwrap();
    let mut condition = OrCondition::new();
    condition.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "ReceiveCostModifierTrigger".into();
    script.is_one_shot = true;
    script.delay_evaluation_seconds = 0;
    script.condition = Some(Box::new(condition));
    script.action = Some(Box::new(action));
    let mut scripts = ScriptList::new();
    scripts.append_script(Box::new(script));
    let engine = ScriptEngine::new().expect("script templates");
    engine
        .set_script_list_for_player(0, Some(Box::new(scripts)))
        .expect("install script on receiving side");
    world
        .update_local_script_engine_with_owner(&engine)
        .expect("execute named receiving upgrade action");
}

#[test]
fn authored_cost_modifier_isolated_across_capture_delete_and_encoded_restore() {
    let (mut first, first_source) = world("-10%", true);
    let (mut peer, peer_source) = world("-25%", true);
    let first_source = first_source.unwrap();
    let peer_source = peer_source.unwrap();
    assert_eq!(
        first_source, peer_source,
        "independent worlds reuse ObjectIds"
    );

    receive_veteran_upgrade(&mut first, first_source);
    receive_veteran_upgrade(&mut peer, peer_source);
    assert_eq!(
        first.modified_build_cost_supplies(1, "CostModifierVehicle", 1000),
        1000,
        "all authored EffectKindOf bits are required"
    );
    assert_eq!(
        first.modified_build_cost_supplies(1, "CostModifierDualKind", 1000),
        900
    );
    assert_eq!(
        peer.modified_build_cost_supplies(1, "CostModifierDualKind", 1000),
        750
    );

    assert!(first.transfer_object_to_player(first_source, 2));
    assert_eq!(
        first.modified_build_cost_supplies(1, "CostModifierDualKind", 1000),
        1000
    );
    assert_eq!(
        first.modified_build_cost_supplies(2, "CostModifierDualKind", 1000),
        900
    );
    assert_eq!(
        peer.modified_build_cost_supplies(1, "CostModifierDualKind", 1000),
        750,
        "capturing the same ObjectId in another world has no effect"
    );

    let builder = SnapshotBuilder::new();
    let snapshot = builder
        .create_world_snapshot(&first)
        .expect("capture snapshot");
    let encoded = bincode_legacy::serialize(&snapshot).expect("encode world snapshot");
    let decoded: WorldSnapshot =
        bincode_legacy::deserialize(&encoded).expect("decode world snapshot");
    let (mut restored, _) = world("-10%", false);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .expect("restore receiving world");
    assert_eq!(
        restored.modified_build_cost_supplies(2, "CostModifierDualKind", 1000),
        900
    );
    restored
        .destroy_object(first_source)
        .expect("delete restored source");
    assert_eq!(
        restored.modified_build_cost_supplies(2, "CostModifierDualKind", 1000),
        1000,
        "restored module latch removes its player contribution on deletion"
    );

    peer.destroy_object(peer_source)
        .expect("delete peer source");
    assert_eq!(
        peer.modified_build_cost_supplies(1, "CostModifierDualKind", 1000),
        1000
    );
}
