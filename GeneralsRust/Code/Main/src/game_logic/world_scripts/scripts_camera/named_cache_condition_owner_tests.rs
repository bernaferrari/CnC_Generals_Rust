//! Actual named condition→canonical contain facts→receiving interpreter history.
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script,
};

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world
        .admit_receiving_module_content(
            crate::assets::ReceivingObjectContent::from_sources([(
                "NamedCondition.ini".into(),
                r#"
Object ReceivingNamedContain
 KindOf = STRUCTURE SELECTABLE
 Body = ActiveBody Health
  MaxHealth = 100
  InitialHealth = 100
 End
 Behavior = GarrisonContain Cargo
  ContainMax = 2
  AllowInsideKindOf = INFANTRY
 End
End
Object ReceivingNamedPassenger
 KindOf = INFANTRY SELECTABLE
 Body = ActiveBody Health
  MaxHealth = 100
  InitialHealth = 100
 End
End
"#
                .into(),
            )])
            .unwrap(),
        )
        .unwrap();
    world.add_player(Player::new(1, Team::USA, "ConditionOwner", true));
    let container = world
        .create_object_for_player("ReceivingNamedContain", 1, Vec3::ZERO)
        .unwrap();
    let passenger = world
        .create_object_for_player("ReceivingNamedPassenger", 1, Vec3::ZERO)
        .unwrap();
    world.set_host_object_name(container, "ContainSource");
    world.initialize_receiving_named_cache_for_fixture();
    world.set_host_object_name(container, "FieldOnlyContain");
    (world, container, passenger)
}
fn emptied(world: &mut GameLogic) -> bool {
    world
        .with_owned_script_engine(|engine, receiving| {
            let mut condition = Condition::new(ConditionType::UnitEmptied);
            condition
                .add_parameter(Parameter::with_string(
                    ParameterType::Unit,
                    "ContainSource".into(),
                ))
                .unwrap();
            let mut branch = OrCondition::new();
            branch.set_first_and_condition(Some(Box::new(condition)));
            let mut script = Script::new();
            script.condition = Some(Box::new(branch));
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
fn remove(world: &mut GameLogic, container: ObjectId, passenger: ObjectId) {
    let (object, ownership, partition) = world
        .host_object_ownership_partition_mut(container)
        .unwrap();
    assert!(object.remove_occupant_with_partition_owner(passenger, ownership, partition));
}
#[test]
fn receiving_emptied_history_preserves_first_observation_repeat_edge_peer_and_reset() {
    crate::game_logic::game_logic::pose_owner_tests::isolated_at(
        module_path!(),
        "receiving_emptied_history_preserves_first_observation_repeat_edge_peer_and_reset",
        || {
            let (mut owner, container, passenger) = world();
            let (mut peer, peer_container, _) = world();
            assert_eq!(container, peer_container);
            let foreign = std::sync::Arc::new(std::sync::RwLock::new(
                gamelogic::object::Object::new_for_xfer_load(container.0, 100.0),
            ));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(container.0, &foreign);
            gamelogic::scripting::engine::get_named_object_tracker()
                .register_named_object("ContainSource".into(), container.0)
                .unwrap();
            // Admit an actual resident passenger through the existing canonical
            // contain roster operation; this witness tests observation, not entry AI.
            assert!(
                owner
                    .host_object_mut(container)
                    .unwrap()
                    .add_occupant(passenger)
            );
            owner.frame = 10;
            peer.frame = 10;
            assert!(!emptied(&mut owner));
            assert!(!emptied(&mut peer));
            remove(&mut owner, container, passenger);
            owner.frame = 11;
            peer.frame = 11;
            assert!(emptied(&mut owner));
            assert!(emptied(&mut owner));
            assert!(!emptied(&mut peer));
            owner.frame = 12;
            assert!(!emptied(&mut owner));
            assert!(
                owner
                    .host_object_mut(container)
                    .unwrap()
                    .add_occupant(passenger)
            );
            owner.frame = 20;
            assert!(!emptied(&mut owner));
            remove(&mut owner, container, passenger);
            owner.owned_script_engine.as_mut().unwrap().reset();
            owner.initialize_receiving_named_cache_for_fixture();
            // Reset rebuilt from current roster: retain source name for this new
            // source phase by explicitly adding its current Object name.
            owner.set_host_object_name(container, "ContainSource");
            owner.add_receiving_named_script_object(container);
            owner.frame = 21;
            assert!(!emptied(&mut owner));
            assert_eq!(
                peer.host_object(peer_container).unwrap().garrison_count(),
                0
            );
            assert_eq!(
                gamelogic::scripting::engine::get_named_object_tracker()
                    .get_object_id("ContainSource")
                    .unwrap(),
                Some(container.0)
            );
            gamelogic::object::registry::OBJECT_REGISTRY.unregister_object(container.0);
        },
    );
}
