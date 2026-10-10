//! Actual Main script walks: CPP ScriptConditions1029/1086, PartitionManager738.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::common::Relationship;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

const OBSERVER: &str = "OwnedSightingObserver";
const TARGET: &str = "OwnedSightingTarget";
const TYPE: &str = "OwnedSightingInfantry";
const TYPES: &str = "OwnedSightingTypes";

// Each fixture owns its thread's compatibility namespace. No serialization
// lock or process-wide engine/player installation is needed by these walks.
fn isolated(run: impl FnOnce() + Send + 'static) {
    std::thread::spawn(run).join().unwrap();
}

fn world() -> (GameLogic, ObjectId, ObjectId) {
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world.add_player(Player::new(1, Team::USA, "Viewer", true));
    world.add_player(Player::new(2, Team::China, "Opponent", false));
    world
        .players
        .get_mut(&1)
        .unwrap()
        .set_map_relationship(2, Relationship::Enemies);
    world
        .players
        .get_mut(&1)
        .unwrap()
        .set_map_relationship(1, Relationship::Allies);
    let mut template = ThingTemplate::new(TYPE);
    template.add_kind_of(KindOf::Infantry).set_health(100.0);
    world.templates.insert(TYPE.into(), template);
    let observer = world
        .create_object_for_player(TYPE, 1, Vec3::new(50.0, 0.0, 50.0))
        .unwrap();
    let target = world
        .create_object_for_player(TYPE, 2, Vec3::new(60.0, 900.0, 50.0))
        .unwrap();
    world.host_object_mut(observer).unwrap().name = OBSERVER.into();
    world.host_object_mut(observer).unwrap().vision_range = 20.0;
    world.host_object_mut(target).unwrap().name = TARGET.into();
    (world, observer, target)
}

fn enemy(unit: &str, alliance: i32, player: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::EnemySighted);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, unit.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, alliance))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
        .unwrap();
    condition
}

fn types(unit: &str, list: &str, player: &str) -> Condition {
    let mut condition = Condition::new(ConditionType::TypeSighted);
    for (kind, name) in [
        (ParameterType::Unit, unit),
        (ParameterType::ObjectType, list),
        (ParameterType::Side, player),
    ] {
        condition
            .add_parameter(Parameter::with_string(kind, name.into()))
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
            "Seen".into(),
        ))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "ObserveSighting".into();
    script.is_one_shot = false;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let engine = ScriptEngine::new().unwrap();
    engine.set_counter("Seen", 0).unwrap();
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
    engine.get_counter("Seen").unwrap().value
}

fn observed(world: &mut GameLogic, condition: Condition) -> bool {
    tick(&engine(condition), world) == 1
}

fn both(world: &mut GameLogic, expected: bool) {
    assert_eq!(
        observed(world, enemy(OBSERVER, 0, "Opponent")),
        expected,
        "EnemySighted"
    );
    assert_eq!(
        observed(world, types(OBSERVER, TYPE, "Opponent")),
        expected,
        "TypeSighted"
    );
}

#[test]
fn sighting_action_walk_calibration() {
    isolated(|| {
        let (mut world, _, _) = world();
        assert_eq!(
            tick(
                &engine(Condition::new(ConditionType::ConditionTrue)),
                &mut world
            ),
            1
        );
    });
}

#[test]
fn sighting_uses_driving_world_despite_foreign_registry_and_same_ids() {
    isolated(|| {
        let (mut first, observer, target) = world();
        let (mut second, other_observer, other_target) = world();
        assert_eq!((observer, target), (other_observer, other_target));
        second
            .host_object_mut(other_target)
            .unwrap()
            .set_position(Vec3::new(500.0, 0.0, 500.0));
        // Existing publication is deliberately hostile input, not the query owner.
        second.inject_host_script_query_snapshot();
        let foreign = std::sync::Arc::new(std::sync::RwLock::new(
            gamelogic::object::Object::new_for_xfer_load(0xfe42_1001, 100.0),
        ));
        let registry = &gamelogic::object::registry::OBJECT_REGISTRY;
        registry.register_object(0xfe42_1001, &foreign);
        both(&mut first, true);
        both(&mut second, false);
        second.reset();
        drop(second);
        let _inert = GameLogic::new();
        both(&mut first, true);
        first
            .host_object_mut(target)
            .unwrap()
            .set_position(Vec3::new(500.0, 0.0, 500.0));
        both(&mut first, false);
        // Never clear a foreign fixture's registry or unregister a replacement.
        if registry
            .get_object(0xfe42_1001)
            .is_some_and(|live| std::sync::Arc::ptr_eq(&live, &foreign))
        {
            registry.unregister_object(0xfe42_1001);
        }
    });
}

#[test]
fn sighting_strict_planar_range_and_same_map_status() {
    isolated(|| {
        let (mut world, observer, target) = world();
        both(&mut world, true); // altitude ignored
        for (x, expected) in [(69.99, true), (70.0, false), (70.01, false)] {
            world
                .host_object_mut(target)
                .unwrap()
                .set_position(Vec3::new(x, 900.0, 50.0));
            both(&mut world, expected);
        }
        world
            .host_object_mut(observer)
            .unwrap()
            .set_position(Vec3::new(995.0, 0.0, 50.0));
        world
            .host_object_mut(target)
            .unwrap()
            .set_position(Vec3::new(1005.0, 0.0, 50.0));
        both(&mut world, false);
        world
            .host_object_mut(observer)
            .unwrap()
            .set_position(Vec3::new(1001.0, 0.0, 50.0));
        both(&mut world, true); // both off map is allowed
        world
            .host_object_mut(target)
            .unwrap()
            .set_position(Vec3::new(1000.0, 0.0, 50.0));
        both(&mut world, false); // extent edge is on map
    });
}

#[test]
fn sighting_candidate_flags_and_observer_lifetime_match_cpp_filters() {
    isolated(|| {
        let (mut world, observer, target) = world();
        world.host_object_mut(target).unwrap().status.stealthed = true;
        both(&mut world, false);
        world.host_object_mut(target).unwrap().status.detected = true;
        both(&mut world, true);
        world.host_object_mut(target).unwrap().status.detected = false;
        world.host_object_mut(target).unwrap().status.disguised = true;
        world.host_object_mut(target).unwrap().disguise_as_template =
            Some("ForeignDisguise".into());
        both(&mut world, true); // actual type/controller, not disguise
        world
            .host_object_mut(target)
            .unwrap()
            .status
            .effectively_dead = true;
        both(&mut world, false);
        world
            .host_object_mut(target)
            .unwrap()
            .status
            .effectively_dead = false;
        world
            .host_object_mut(observer)
            .unwrap()
            .status
            .effectively_dead = true;
        world.host_object_mut(observer).unwrap().health.current = 0.0;
        both(&mut world, true); // original filter applies only to candidates
        world.host_object_mut(target).unwrap().status.destroyed = true;
        both(&mut world, true); // queued destruction has not unregistered it
        world.host_object_mut(target).unwrap().status.destroyed = false;
        world.host_object_mut(target).unwrap().health.current = 0.0;
        both(&mut world, true); // health alone is not the effectively-dead bit
        world.host_object_mut(target).unwrap().owner_player_id = Some(1);
        assert!(!observed(&mut world, types(OBSERVER, TYPE, "Opponent")));
        world.objects.remove(&target);
        assert!(
            !observed(&mut world, types(OBSERVER, TYPE, "Viewer")),
            "self excluded"
        );
    });
}

#[test]
fn sighting_respects_enclosing_containment_and_non_enclosing_ancestors() {
    isolated(|| {
        let (mut world, _, target) = world();
        let carrier = world
            .create_object_for_player(TYPE, 1, Vec3::new(55.0, 0.0, 50.0))
            .unwrap();
        world.host_object_mut(target).unwrap().contained_by = Some(carrier);
        both(&mut world, false); // OpenContain removes enclosed riders
        world.host_object_mut(carrier).unwrap().paradrop_parachute = true;
        both(&mut world, true); // non-enclosing riders remain registered
        let outer = world
            .create_object_for_player(TYPE, 1, Vec3::new(55.0, 0.0, 50.0))
            .unwrap();
        world.host_object_mut(carrier).unwrap().contained_by = Some(outer);
        both(&mut world, false); // enclosing ancestor also removes descendants
        world.host_object_mut(outer).unwrap().paradrop_parachute = true;
        both(&mut world, true);
        world.host_object_mut(outer).unwrap().contained_by = Some(carrier);
        both(&mut world, false); // malformed cycle does not hang the query
        world.host_object_mut(outer).unwrap().contained_by = None;
        world.host_object_mut(target).unwrap().contained_by = None;
        both(&mut world, true); // restored partition membership
    });
}

#[test]
fn sighting_reads_live_exact_object_types_and_authoritative_absence() {
    isolated(|| {
        let (mut world, _, target) = world();
        let engine = engine(types(OBSERVER, TYPES, "Opponent"));
        let mut list = gamelogic::object::object_types::ObjectTypes::new();
        list.add_object_type(TYPE.into());
        engine.set_object_types(TYPES.into(), list);
        assert_eq!(tick(&engine, &mut world), 1);
        engine.set_object_types(
            TYPES.into(),
            gamelogic::object::object_types::ObjectTypes::new(),
        );
        assert_eq!(
            tick(&engine, &mut world),
            1,
            "registered empty is authoritative"
        );
        // A candidate whose template equals the list name must not turn an
        // empty registered list into the singleton fallback.
        world.host_object_mut(target).unwrap().template_name = TYPES.into();
        assert_eq!(tick(&engine, &mut world), 1);
        let mut list = gamelogic::object::object_types::ObjectTypes::new();
        list.add_object_type(TYPES.into());
        engine.set_object_types(TYPES.into(), list);
        assert_eq!(tick(&engine, &mut world), 2, "live list mutation observed");
        assert!(!observed(
            &mut world,
            types(OBSERVER, &TYPES.to_ascii_lowercase(), "Opponent")
        ));
        assert!(!observed(
            &mut world,
            types("MissingObserver", TYPES, "Opponent")
        ));
        assert!(!observed(
            &mut world,
            types(OBSERVER, TYPES, "MissingPlayer")
        ));
        // Local-player resolution belongs to Main even when the standalone
        // global player list is empty or belongs to another fixture.
        world.host_object_mut(target).unwrap().owner_player_id = Some(1);
        assert!(observed(
            &mut world,
            types(OBSERVER, TYPES, gamelogic::scripting::core::LOCAL_PLAYER)
        ));
    });
}

#[test]
fn sighting_relationship_precedence_and_defection_use_owned_maps() {
    isolated(|| {
        let (mut world, observer, target) = world();
        world.host_object_mut(observer).unwrap().team_instance_name = "LookerTeam".into();
        world.host_object_mut(target).unwrap().team_instance_name = "TargetTeam".into();
        world.players.get_mut(&2).unwrap().is_alive = false;
        both(&mut world, true); // defeated player still has relationships
        world
            .players
            .get_mut(&1)
            .unwrap()
            .set_team_relationship_override("TargetTeam", Relationship::Allies);
        assert!(!observed(&mut world, enemy(OBSERVER, 0, "Opponent")));
        assert!(observed(&mut world, enemy(OBSERVER, 2, "Opponent")));
        assert!(
            observed(&mut world, types(OBSERVER, TYPE, "Opponent")),
            "type query ignores relation"
        );
        world
            .players
            .get_mut(&1)
            .unwrap()
            .set_team_instance_player_override("LookerTeam", 2, Relationship::Neutral);
        assert!(observed(&mut world, enemy(OBSERVER, 1, "Opponent")));
        world
            .players
            .get_mut(&1)
            .unwrap()
            .set_team_instance_team_override("LookerTeam", "TargetTeam", Relationship::Enemies);
        assert!(observed(&mut world, enemy(OBSERVER, 0, "Opponent")));
        let mut defector =
            crate::game_logic::host_defection_helper::HostDefectionHelperData::default();
        defector.set_undetected_defector(true);
        world.host_object_mut(target).unwrap().defection_helper = Some(defector.clone());
        assert!(observed(&mut world, enemy(OBSERVER, 2, "Opponent")));
        world.host_object_mut(observer).unwrap().defection_helper = Some(defector);
        assert!(
            observed(&mut world, enemy(OBSERVER, 1, "Opponent")),
            "observer defection wins"
        );
        world.host_object_mut(observer).unwrap().defection_helper = None;
        world.host_object_mut(target).unwrap().defection_helper = None;
        world
            .players
            .get_mut(&1)
            .unwrap()
            .map_side
            .relations
            .clear();
        world.players.get_mut(&1).unwrap().team_relations.clear();
        // A different raw player token selects absence rather than a foreign
        // Core player. Invalid relation ordinals never match a candidate.
        assert!(!observed(&mut world, enemy(OBSERVER, 99, "Opponent")));
    });
}

#[test]
fn sighting_owned_factory_overrides_precede_host_player_maps() {
    isolated(|| {
        let (mut world, observer, target) = world();
        world.host_object_mut(observer).unwrap().team_instance_name = "FactoryLooker".into();
        world.host_object_mut(target).unwrap().team_instance_name = "FactoryTarget".into();
        let (source, destination) = {
            let mut factory = world.team_factory.lock().unwrap();
            for name in ["FactoryLooker", "FactoryTarget"] {
                factory.replace_team_prototype(gamelogic::team::TeamPrototype::new(name.into()));
            }
            (
                factory.create_inactive_team("FactoryLooker").unwrap(),
                factory.create_inactive_team("FactoryTarget").unwrap(),
            )
        };
        // The retained factory constructor reads a Core controller; explicitly
        // admitted owners below replace it. No global player installation.
        source.write().unwrap().set_controlling_player_id(Some(1));
        destination
            .write()
            .unwrap()
            .set_controlling_player_id(Some(2));
        let target_id = destination.read().unwrap().get_id();
        world
            .players
            .get_mut(&1)
            .unwrap()
            .set_team_instance_team_override(
                "FactoryLooker",
                "FactoryTarget",
                Relationship::Enemies,
            );
        source
            .write()
            .unwrap()
            .set_override_player_relationship(2, Relationship::Neutral);
        source
            .write()
            .unwrap()
            .set_override_team_relationship(target_id, Relationship::Allies);
        assert!(observed(&mut world, enemy(OBSERVER, 2, "Opponent")));
        assert!(observed(&mut world, types(OBSERVER, TYPE, "Opponent")));
        source
            .write()
            .unwrap()
            .remove_override_team_relationship(target_id);
        assert!(observed(&mut world, enemy(OBSERVER, 1, "Opponent")));
        source
            .write()
            .unwrap()
            .remove_override_player_relationship(2);
        assert!(observed(&mut world, enemy(OBSERVER, 0, "Opponent")));
    });
}

#[test]
fn sighting_default_team_uses_authored_player_name() {
    isolated(|| {
        let (mut world, observer, target) = world();
        world
            .host_object_mut(observer)
            .unwrap()
            .team_instance_name
            .clear();
        world
            .host_object_mut(target)
            .unwrap()
            .team_instance_name
            .clear();
        world.players.get_mut(&2).unwrap().map_side.map_player_name = "AuthoredOpponent".into();
        world
            .players
            .get_mut(&1)
            .unwrap()
            .set_team_relationship_override("teamAuthoredOpponent", Relationship::Allies);
        assert!(observed(&mut world, enemy(OBSERVER, 2, "AuthoredOpponent")));
        assert!(!observed(
            &mut world,
            enemy(OBSERVER, 0, "AuthoredOpponent")
        ));
        assert!(
            !observed(&mut world, types(OBSERVER, TYPE, "Opponent")),
            "display name is not authored identity"
        );
    });
}

// New-capability witness, separate from the unchanged-API OLD script walks.
#[test]
fn sighting_supplied_this_object_and_current_player_are_owned_identities() {
    isolated(|| {
        use gamelogic::scripting::core::{THIS_OBJECT, THIS_PLAYER};
        use gamelogic::scripting::engine::{
            ScriptExecutionDriver, ScriptOwnerQuery, ScriptSightingFilter,
        };
        let (mut first, observer, _) = world();
        let (mut foreign, foreign_observer, foreign_target) = world();
        assert_eq!(observer, foreign_observer);
        foreign
            .host_object_mut(foreign_target)
            .unwrap()
            .set_position(Vec3::splat(900.0));
        foreign.inject_host_script_query_snapshot();
        let names = [TYPE.into()];
        let driver = HostScriptExecutionDriver::new(&mut first);
        assert_eq!(
            driver.sighted(
                THIS_OBJECT,
                THIS_PLAYER,
                ScriptSightingFilter::Types(&names),
                Some("Opponent"),
                Some(observer.0)
            ),
            ScriptOwnerQuery::Present(true)
        );
        assert_eq!(
            driver.sighted(
                THIS_OBJECT,
                THIS_PLAYER,
                ScriptSightingFilter::Types(&names),
                Some("Viewer"),
                Some(observer.0)
            ),
            ScriptOwnerQuery::Present(false)
        );
        for id in [None, Some(u32::MAX)] {
            assert_eq!(
                driver.sighted(
                    THIS_OBJECT,
                    THIS_PLAYER,
                    ScriptSightingFilter::Types(&names),
                    Some("Opponent"),
                    id
                ),
                ScriptOwnerQuery::Missing
            );
        }
        assert_eq!(
            driver.sighted(
                THIS_OBJECT,
                THIS_PLAYER,
                ScriptSightingFilter::Types(&names),
                None,
                Some(observer.0)
            ),
            ScriptOwnerQuery::Missing
        );
        // This verifies supplied context. Root side admission and Challenge
        // classification retain their own owner debts (hq-fxwkm/hq-9bqdm).
    });
}
