//! C++ ScriptConditions.cpp:2233: live tech search and persistent result latch.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::common::Relationship;
use gamelogic::scripting::core::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;

fn condition(player: &str, area: &str, distance: f32) -> Condition {
    let mut condition = Condition::new(ConditionType::SkirmishTechBuildingWithinDistance);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
        .unwrap();
    condition
        .add_parameter(Parameter::with_real(ParameterType::Real, distance))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            area.into(),
        ))
        .unwrap();
    condition
}

fn observing_engine(condition: Condition, side: usize) -> ScriptEngine {
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(condition)));
    let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
    // CPP ScriptActions::addCounter: amount first, counter name second.
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "ObservedTech".into(),
        ))
        .unwrap();
    let mut script = Script::new();
    script.script_name = "ObserveOwnedTech".into();
    script.is_one_shot = false;
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(action));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    let mut engine = ScriptEngine::new().unwrap();
    engine.set_counter("ObservedTech", 0).unwrap();
    engine
        .set_script_list_for_player(side, Some(Box::new(list)))
        .unwrap();
    engine
}

fn tick(engine: &ScriptEngine, world: &mut GameLogic) -> i32 {
    let mut context = gamelogic::scripting::executor::ScriptContext::new();
    context.current_frame = world.frame;
    context.host_trigger_world = Arc::clone(&world.host_trigger_world);
    engine
        .update_with_driver(context, &mut HostScriptExecutionDriver::new(world))
        .unwrap();
    world.frame += 1;
    engine.get_counter("ObservedTech").unwrap().value
}

fn world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.world_min = Vec3::ZERO;
    world.world_max = Vec3::splat(1000.0);
    world.add_player(Player::new(1, Team::USA, "Viewer", true));
    world.add_player(Player::new(2, Team::USA, "Other", false));
    world
        .players
        .get_mut(&1)
        .unwrap()
        .set_map_relationship(2, Relationship::Enemies);
    let trigger = gamelogic::polygon_trigger::PolygonTrigger::new(
        1,
        "HomeBase".into(),
        vec![
            gamelogic::common::ICoord3D::new(0, 0, 0),
            gamelogic::common::ICoord3D::new(20, 20, 0),
        ],
    );
    world
        .host_trigger_world
        .lock()
        .unwrap()
        .set_trigger_areas(&[trigger.clone()]);
    world
        .world_services
        .terrain()
        .write()
        .unwrap()
        .add_trigger_area(trigger);
    let mut template = ThingTemplate::new("OwnedTech");
    template
        .add_kind_of(KindOf::TechBuilding)
        .add_kind_of(KindOf::Structure)
        .set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("OwnedTech", 2, Vec3::new(10.0, 500.0, 10.0))
        .unwrap();
    world.host_object_mut(id).unwrap().team_instance_name = "OtherTechTeam".into();
    (world, id)
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn live_tech_condition_ignores_foreign_registry_and_world_lifecycle() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "live_tech_condition_ignores_foreign_registry_and_world_lifecycle",
        || {
            let (mut first, id) = world();
            let (mut second, other) = world();
            assert_eq!(id, other);
            second
                .players
                .get_mut(&1)
                .unwrap()
                .set_map_relationship(2, Relationship::Allies);
            let foreign = Arc::new(RwLock::new(gamelogic::object::Object::new_for_xfer_load(
                id.0, 100.0,
            )));
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(id.0, &foreign);
            let first_engine = observing_engine(condition("Viewer", "HomeBase", 0.0), 1);
            let second_engine = observing_engine(condition("Viewer", "HomeBase", 0.0), 1);
            assert_eq!(
                tick(&first_engine, &mut first),
                1,
                "live owner is authoritative despite a foreign Core registry member"
            );
            assert_eq!(tick(&second_engine, &mut second), 0);
            // Completed searches latch independently, including a false search.
            first
                .host_object_mut(id)
                .unwrap()
                .set_position(Vec3::splat(900.0));
            second
                .players
                .get_mut(&1)
                .unwrap()
                .set_map_relationship(2, Relationship::Enemies);
            assert_eq!(tick(&first_engine, &mut first), 2);
            assert_eq!(tick(&second_engine, &mut second), 0);
            second.reset();
            assert_eq!(tick(&first_engine, &mut first), 3);
            let viewer = first.players.remove(&1).unwrap();
            assert_eq!(
                tick(&first_engine, &mut first),
                3,
                "missing player takes precedence over a cached true result"
            );
            first.add_player(viewer);
            assert_eq!(tick(&first_engine, &mut first), 4);
            assert!(Arc::ptr_eq(
                &gamelogic::object::registry::OBJECT_REGISTRY
                    .get_object(id.0)
                    .unwrap(),
                &foreign
            ));
            assert_eq!(foreign.read().unwrap().get_health(), 100.0);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn live_tech_condition_retries_missing_trigger_but_latches_empty_search() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "live_tech_condition_retries_missing_trigger_but_latches_empty_search",
        || {
            let (mut world, id) = world();
            let engine = observing_engine(condition("Viewer", "LaterArea", 0.0), 1);
            assert_eq!(tick(&engine, &mut world), 0);
            let trigger = gamelogic::polygon_trigger::PolygonTrigger::new(
                2,
                "LaterArea".into(),
                vec![
                    gamelogic::common::ICoord3D::new(0, 0, 0),
                    gamelogic::common::ICoord3D::new(20, 20, 0),
                ],
            );
            world
                .host_trigger_world
                .lock()
                .unwrap()
                .set_trigger_areas(&[trigger]);
            assert_eq!(
                tick(&engine, &mut world),
                1,
                "missing trigger must not cache false"
            );
            world.objects.remove(&id);
            let empty = observing_engine(condition("Viewer", "LaterArea", 0.0), 1);
            assert_eq!(tick(&empty, &mut world), 0);
            world
                .create_object_for_player("OwnedTech", 2, Vec3::new(10.0, 0.0, 10.0))
                .unwrap();
            assert_eq!(
                tick(&empty, &mut world),
                0,
                "a valid empty search caches false"
            );
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn live_tech_search_preserves_center_distance_and_team_affiliation() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "live_tech_search_preserves_center_distance_and_team_affiliation",
        || {
            let (mut world, id) = world();
            let observed = |world: &mut GameLogic, distance| {
                tick(
                    &observing_engine(condition("Viewer", "HomeBase", distance), 1),
                    world,
                )
            };
            assert_eq!(
                observed(&mut world, 0.0),
                1,
                "altitude does not affect FROM_CENTER_2D"
            );
            world.players.get_mut(&1).unwrap().is_alive = false;
            world.players.get_mut(&2).unwrap().is_alive = false;
            world
                .players
                .get_mut(&1)
                .unwrap()
                .set_map_relationship(2, Relationship::Allies);
            assert_eq!(
                observed(&mut world, 0.0),
                0,
                "defeat does not erase stored affiliation"
            );
            world.players.get_mut(&1).unwrap().is_alive = true;
            world.players.get_mut(&2).unwrap().is_alive = true;
            world
                .players
                .get_mut(&1)
                .unwrap()
                .set_map_relationship(2, Relationship::Enemies);
            world.host_object_mut(id).unwrap().status.effectively_dead = true;
            assert_eq!(
                observed(&mut world, 0.0),
                1,
                "C++ tech filters do not reject dead objects"
            );
            world.host_object_mut(id).unwrap().status.effectively_dead = false;
            world
                .players
                .get_mut(&1)
                .unwrap()
                .set_team_relationship_override("OtherTechTeam", Relationship::Allies);
            assert_eq!(
                observed(&mut world, 0.0),
                0,
                "team override precedes player relation"
            );
            world
                .players
                .get_mut(&1)
                .unwrap()
                .set_team_relationship_override("OtherTechTeam", Relationship::Neutral);
            assert_eq!(
                observed(&mut world, 0.0),
                1,
                "neutral tech passes the not-allies filter"
            );
            world.host_object_mut(id).unwrap().owner_player_id = Some(1);
            assert_eq!(
                observed(&mut world, 0.0),
                0,
                "self ownership is excluded independently of affiliation"
            );
            world.host_object_mut(id).unwrap().owner_player_id = Some(2);
            // Radius sqrt(200) + extra =5 exactly. C++ uses strict center distance.
            let extra = 5.0 - 200.0_f32.sqrt();
            world
                .host_object_mut(id)
                .unwrap()
                .set_position(Vec3::new(15.0, 500.0, 10.0));
            assert_eq!(observed(&mut world, extra), 0);
            world
                .host_object_mut(id)
                .unwrap()
                .set_position(Vec3::new(14.0, 500.0, 10.0));
            assert_eq!(
                observed(&mut world, extra),
                1,
                "negative extra distance shrinks the positive search radius"
            );
            world
                .host_object_mut(id)
                .unwrap()
                .set_position(Vec3::new(-1.0, 0.0, 10.0));
            assert_eq!(observed(&mut world, 100.0), 0, "off-map tech is excluded");
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn owned_tech_aliases_use_explicit_current_player_and_enemy_perimeter() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "owned_tech_aliases_use_explicit_current_player_and_enemy_perimeter",
        || {
            use gamelogic::scripting::core::{LOCAL_PLAYER, THIS_PLAYER, THIS_PLAYER_ENEMY};
            use gamelogic::scripting::engine::{ScriptExecutionDriver, ScriptOwnerQuery};
            let (mut world, id) = world();
            world.players.get_mut(&1).unwrap().start_position = 2;
            world.players.get_mut(&2).unwrap().start_position = 5;
            world
                .ai_manager
                .add_ai_player(1, Team::USA, crate::ai::AIDifficulty::Easy);
            world
                .ai_manager
                .ai_players
                .get_mut(&1)
                .unwrap()
                .enemy_player_id = Some(2);
            let areas = ["InnerPerimeter3", "OuterPerimeter6", THIS_PLAYER].map(|name| {
                gamelogic::polygon_trigger::PolygonTrigger::new(
                    3,
                    name.into(),
                    vec![
                        gamelogic::common::ICoord3D::new(0, 0, 0),
                        gamelogic::common::ICoord3D::new(20, 20, 0),
                    ],
                )
            });
            world
                .host_trigger_world
                .lock()
                .unwrap()
                .set_trigger_areas(&areas);
            // Query capability receives the executing player explicitly. The
            // broader prepared-side -> host-player admission is a separate
            // migration; this fixture must not assume side index == host ID.
            let query = |world: &mut GameLogic, player: &str, area: &str, current: Option<&str>| {
                HostScriptExecutionDriver::new(world)
                    .tech_building_within_distance(player, 0.0, area, current)
            };
            for player in [THIS_PLAYER, LOCAL_PLAYER] {
                assert_eq!(
                    query(
                        &mut world,
                        player,
                        "[Skirmish]MyInnerPerimeter",
                        Some("Viewer")
                    ),
                    ScriptOwnerQuery::Present(true)
                );
            }
            assert_eq!(
                query(&mut world, "Viewer", THIS_PLAYER, Some("Viewer")),
                ScriptOwnerQuery::Present(true),
                "location names remain literal"
            );
            assert_eq!(
                query(
                    &mut world,
                    "Viewer",
                    "[Skirmish]EnemyOuterPerimeter",
                    Some("Viewer")
                ),
                ScriptOwnerQuery::Present(true)
            );
            // SIDE determines affiliation; perimeter still uses CURRENT player.
            world.host_object_mut(id).unwrap().owner_player_id = Some(1);
            world
                .players
                .get_mut(&2)
                .unwrap()
                .set_map_relationship(1, Relationship::Enemies);
            assert_eq!(
                query(
                    &mut world,
                    THIS_PLAYER_ENEMY,
                    "[Skirmish]MyInnerPerimeter",
                    Some("Viewer")
                ),
                ScriptOwnerQuery::Present(true)
            );
            assert_eq!(
                query(&mut world, THIS_PLAYER_ENEMY, "InnerPerimeter3", None),
                ScriptOwnerQuery::Missing
            );
            assert_eq!(
                query(&mut world, "PlyrUSA", "InnerPerimeter3", Some("Viewer")),
                ScriptOwnerQuery::Missing,
                "faction is not a player identity"
            );
            world.host_object_mut(id).unwrap().owner_player_id = Some(2);
            world.players.get_mut(&1).unwrap().map_side.map_player_name = "AuthoredViewer".into();
            assert_eq!(
                query(
                    &mut world,
                    THIS_PLAYER,
                    "InnerPerimeter3",
                    Some("AuthoredViewer")
                ),
                ScriptOwnerQuery::Present(true)
            );
            assert_eq!(
                query(
                    &mut world,
                    "Viewer",
                    "InnerPerimeter3",
                    Some("AuthoredViewer")
                ),
                ScriptOwnerQuery::Missing,
                "display label is not an authored name key"
            );
            // Populated names do not prove the prepared side -> host-ID map.
            // Keep all prepared modes on their existing route until admission
            // records that mapping, rather than inventing an index offset.
            world.players.get_mut(&2).unwrap().map_side.map_player_name = "AuthoredOther".into();
            for mode in [
                GameMode::Skirmish,
                GameMode::Multiplayer,
                GameMode::Lan,
                GameMode::Internet,
                GameMode::Replay,
            ] {
                world.game_mode = mode;
                let driver = HostScriptExecutionDriver::new(&mut world);
                assert_eq!(
                    driver.skirmish_player_exists("AuthoredViewer", Some("AuthoredViewer")),
                    ScriptOwnerQuery::Unavailable
                );
                assert_eq!(
                    driver.tech_building_within_distance(
                        "AuthoredViewer",
                        0.0,
                        "InnerPerimeter3",
                        Some("AuthoredViewer")
                    ),
                    ScriptOwnerQuery::Unavailable
                );
            }
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn true_condition_calibrates_runtime_counter_witness() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "true_condition_calibrates_runtime_counter_witness",
        || {
            let (mut world, _) = world();
            let engine = observing_engine(Condition::new(ConditionType::ConditionTrue), 1);
            assert_eq!(tick(&engine, &mut world), 1);
            assert_eq!(tick(&engine, &mut world), 2);
        },
    );
}
