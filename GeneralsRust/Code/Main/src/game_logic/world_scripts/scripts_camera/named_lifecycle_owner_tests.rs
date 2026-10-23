//! CPP ScriptConditions274-335: pointer lifetime differs from effective death.
use super::*;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script,
};
use gamelogic::scripting::engine::ScriptEngine;

fn fixture() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(Player::new(1, Team::USA, "LifetimeOwner", false));
    let mut template = ThingTemplate::new("LifetimeInfantry");
    template.add_kind_of(KindOf::Infantry);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("LifetimeInfantry", 1, Vec3::ZERO)
        .unwrap();
    assert!(world.set_host_object_name(id, "Hero"));
    world.install_owned_script_engine(ScriptEngine::new().unwrap());
    (world, id)
}
fn query(world: &mut GameLogic, kind: ConditionType, name: &str) -> bool {
    let mut condition = Condition::new(kind);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut script = Script::new();
    script.condition = Some(Box::new(branch));
    world
        .with_owned_script_engine(|engine, world| {
            world.evaluate_owned_script_conditions(engine, &mut script, None)
        })
        .unwrap()
}

#[test]
fn live_dead_removed_and_never_created_names_are_session_owned() {
    let (mut world, id) = fixture();
    let (mut peer, peer_id) = fixture();
    assert_eq!(id, peer_id);
    assert!(query(&mut world, ConditionType::NamedCreated, "Hero"));
    assert!(!query(&mut world, ConditionType::NamedDestroyed, "Hero"));
    world.host_object_mut(id).unwrap().status.effectively_dead = true;
    assert!(query(&mut world, ConditionType::NamedCreated, "Hero"));
    assert!(query(&mut world, ConditionType::NamedDying, "Hero"));
    assert!(query(&mut world, ConditionType::NamedDestroyed, "Hero"));
    assert!(!query(&mut world, ConditionType::NamedTotallyDead, "Hero"));
    world.destroy_object(id);
    world.process_destroy_list();
    assert!(world.host_object(id).is_none());
    assert!(query(&mut world, ConditionType::NamedDestroyed, "Hero"));
    assert!(query(&mut world, ConditionType::NamedTotallyDead, "Hero"));
    assert!(!query(&mut world, ConditionType::NamedDying, "Hero"));
    assert!(!query(&mut world, ConditionType::NamedCreated, "Hero"));
    assert!(!query(
        &mut world,
        ConditionType::NamedDestroyed,
        "NeverCreated"
    ));
    assert!(!query(
        &mut world,
        ConditionType::NamedTotallyDead,
        "NeverCreated"
    ));
    assert!(!query(&mut peer, ConditionType::NamedDestroyed, "Hero"));
    assert!(query(&mut peer, ConditionType::NamedCreated, "Hero"));
}

#[test]
fn renaming_and_actual_snapshot_restore_preserve_history_but_reset_clears_it() {
    let (mut world, id) = fixture();
    assert!(world.set_host_object_name(id, "RenamedHero"));
    assert!(query(&mut world, ConditionType::NamedDestroyed, "Hero"));
    assert!(query(
        &mut world,
        ConditionType::NamedCreated,
        "RenamedHero"
    ));
    world.destroy_object(id);
    world.process_destroy_list();
    let builder = crate::save_load::SnapshotBuilder::new();
    let snapshot = builder.create_world_snapshot(&world).unwrap();
    assert!(world.named_unit_history.contains("Hero"));
    assert!(world.named_unit_history.contains("RenamedHero"));
    let (mut restored, _) = fixture();
    builder
        .restore_from_snapshot(&snapshot, &mut restored)
        .unwrap();
    assert!(restored.host_objects().registered_is_empty());
    assert!(
        restored.named_unit_history.contains("Hero"),
        "restored history: {:?}",
        restored.named_unit_history
    );
    assert!(restored.named_unit_history.contains("RenamedHero"));
    assert!(query(
        &mut restored,
        ConditionType::NamedTotallyDead,
        "Hero"
    ));
    assert!(query(
        &mut restored,
        ConditionType::NamedTotallyDead,
        "RenamedHero"
    ));
    assert!(!query(
        &mut restored,
        ConditionType::NamedDestroyed,
        "NeverCreated"
    ));
    restored.reset();
    restored.install_owned_script_engine(ScriptEngine::new().unwrap());
    assert!(!query(&mut restored, ConditionType::NamedDestroyed, "Hero"));
}
