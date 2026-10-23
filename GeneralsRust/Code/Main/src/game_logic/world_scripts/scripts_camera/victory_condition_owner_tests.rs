//! Actual receiving victory evaluation, script consumers, and frozen defeat events.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::common::Relationship;
use gamelogic::scripting::core::{Condition, ConditionType, OrCondition, Script};

fn fixture(allied: bool) -> (GameLogic, ObjectId, ObjectId) {
    let (mut world, unit, target) = super::named_command_test_support::world();
    world.game_mode = GameMode::Skirmish;
    world.frame = 8;
    world.get_player_mut(1).unwrap().alliance_team = 1;
    world.get_player_mut(2).unwrap().alliance_team = 2;
    for (source, dest, name) in [(1, 2, "teamNamedVictim"), (2, 1, "teamNamedOwner")] {
        let player = world.get_player_mut(source).unwrap();
        player
            .map_side
            .relations
            .insert(dest, Relationship::Enemies);
        if allied {
            player.set_team_relationship_override(name, Relationship::Allies);
        }
    }
    (world, unit, target)
}
fn query(world: &mut GameLogic, kind: ConditionType) -> bool {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(kind))));
    let mut script = Script::new();
    script.condition = Some(Box::new(branch));
    world
        .with_owned_script_engine(|engine, receiving| {
            let frame = receiving.frame;
            let mut driver = HostScriptExecutionDriver::with_interpreter(receiving, engine);
            engine.evaluate_conditions_with_driver(
                &mut script,
                None,
                None,
                gamelogic::scripting::executor::ScriptContext::at_frame(frame),
                &mut driver,
            )
        })
        .unwrap()
}

#[test]
fn same_id_victory_conditions_survive_peer_reset_and_encoded_restore() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, unit, target) = fixture(true);
    let (mut peer, peer_unit, peer_target) = fixture(false);
    assert_eq!((unit, target), (peer_unit, peer_target));
    let builder = crate::save_load::snapshot::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&owner).unwrap();
    let bytes = bincode_legacy::serialize(&snapshot).unwrap();
    let decoded = crate::save_load::snapshot::decode_bincode_world_snapshot(&bytes).unwrap();
    assert!(owner.evaluate_victory_condition().is_some());
    assert!(peer.evaluate_victory_condition().is_none());
    assert!(query(&mut owner, ConditionType::MultiplayerAlliedVictory));
    assert!(!query(&mut peer, ConditionType::MultiplayerAlliedVictory));
    assert!(!query(&mut peer, ConditionType::MultiplayerAlliedDefeat));
    peer.reset();
    assert!(query(&mut owner, ConditionType::MultiplayerAlliedVictory));
    let (mut restored, _, _) = fixture(false);
    builder
        .restore_from_snapshot(&decoded, &mut restored)
        .unwrap();
    assert!(restored.evaluate_victory_condition().is_some());
    assert!(query(
        &mut restored,
        ConditionType::MultiplayerAlliedVictory
    ));
    assert!(!query(
        &mut restored,
        ConditionType::MultiplayerAlliedDefeat
    ));
}

#[test]
fn receiving_defeat_keeps_script_and_native_presentation_events() {
    let _serial = crate::game_logic::combat::tests::combat_test_guard();
    let (mut owner, unit, _) = fixture(false);
    let (mut peer, _, _) = fixture(false);
    // Remove only the local army. The enemy remains alive: local allied
    // defeat wins over the separate pre-final-alliance player-defeat condition.
    let (object, health_events) = owner.host_object_and_health_events_mut(unit).unwrap();
    object.take_damage_from_immediate(1_000_000.0, None, health_events);
    assert!(owner.evaluate_victory_condition().is_some());
    assert!(query(&mut owner, ConditionType::MultiplayerAlliedDefeat));
    assert!(!query(&mut owner, ConditionType::MultiplayerPlayerDefeat));
    assert!(!query(&mut peer, ConditionType::MultiplayerAlliedDefeat));
    let frame = crate::presentation_frame::PresentationFrame::build_with_victory_for_engine(
        &mut owner, 1, None,
    );
    assert!(frame.defeated_player_ids.contains(&1));
    assert!(peer.host_object(unit).unwrap().is_alive());
}
