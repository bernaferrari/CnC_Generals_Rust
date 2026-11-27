//! Main regressions for the one-shot player science receipt, readiness check,
//! grant and purchase in ScriptConditions.cpp and ScriptActions.cpp.

use super::*;
use crate::command_executor::CommandExecutor;
use crate::command_system::{CommandResult, CommandType, GameCommand, ModifierKeys};
use game_engine::common::rts::science::{
    SCIENCE_INVALID, ScienceInfo, ScienceNameFacts, ScienceStore, ScienceType, get_science_store,
};
use generals_sim::scripting::program::{
    Condition, ConditionType, OrCondition, Parameter, ParameterType, Script, ScriptAction,
    ScriptActionType, ScriptList,
};
use gamelogic::scripting::engine::ScriptEngine;
use std::sync::Arc;
use std::time::SystemTime;

const SIDE: &str = "Local";
const SCIENCE: &str = "SCIENCE_EdgeOwnershipFixture";
const OBSERVER_A: &str = "science_edge_observer_a";
const OBSERVER_B: &str = "science_edge_observer_b";

fn single_condition(and: Condition) -> Box<OrCondition> {
    let mut result = OrCondition::new();
    result.set_first_and_condition(Some(Box::new(and)));
    Box::new(result)
}

fn truth_condition() -> Box<OrCondition> {
    single_condition(Condition::new(ConditionType::ConditionTrue))
}

fn science_condition(
    kind: ConditionType,
    side: &str,
    mask: i32,
    science: &str,
) -> Box<OrCondition> {
    let mut condition = Condition::new(kind);
    let mut side = Parameter::with_string(ParameterType::Side, side.into());
    side.int_value = mask;
    condition.add_parameter(side).unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Science,
            science.into(),
        ))
        .unwrap();
    single_condition(condition)
}

fn acquired_condition() -> Box<OrCondition> {
    science_condition(ConditionType::PlayerAcquiredScience, SIDE, 0, SCIENCE)
}

fn flag_action(name: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(ScriptActionType::SetFlag);
    action
        .add_parameter(Parameter::with_string(ParameterType::Flag, name.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();
    Box::new(action)
}

fn science_action(kind: ScriptActionType, side: &str, science: &str) -> Box<ScriptAction> {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, side.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Science,
            science.into(),
        ))
        .unwrap();
    Box::new(action)
}

fn script(name: &str, condition: Box<OrCondition>, action: Box<ScriptAction>) -> Box<Script> {
    let mut script = Script::new();
    script.script_name = name.into();
    script.is_one_shot = true;
    script.condition = Some(condition);
    script.action = Some(action);
    Box::new(script)
}

fn world_with_science(cost: i32) -> (GameLogic, ScienceType) {
    let mut store = ScienceStore::new();
    let mut definition = ScienceInfo::new(SCIENCE_INVALID, SCIENCE);
    definition.science_purchase_point_cost = cost;
    store.add_science(definition);
    let facts = Arc::new(ScienceNameFacts::from_store(&store));
    let science = facts.science(SCIENCE).unwrap();
    let object_source = crate::fixture_content::FixtureObject::new("EnemyScienceStructure")
        .kind_of("STRUCTURE SELECTABLE")
        .health(100.0)
        .ini();
    let content = crate::assets::ReceivingObjectContent::from_sources_with_sciences(
        [("science-edge-fixture.ini".into(), object_source)],
        Arc::clone(&facts),
    )
    .unwrap();

    let mut world = GameLogic::new();
    world.admit_receiving_module_content(content).unwrap();
    let mut player = Player::new(0, Team::USA, SIDE, true).with_script_identity(SIDE);
    player.science_purchase_points = 3;
    world.add_player(player);
    (world, science)
}

fn enemy_science_world(cost: i32) -> (GameLogic, ScienceType) {
    use game_engine::common::game_common::Relationship;

    let (mut world, science) = world_with_science(cost);
    world.game_mode = GameMode::Skirmish;
    let owner = world.get_player_mut(0).unwrap();
    owner.is_human = false;
    owner.map_side.is_skirmish_ai = true;
    owner.set_map_relationship(1, Relationship::Enemies);
    let mut enemy = Player::new(1, Team::GLA, "Enemy", false).with_script_identity("Enemy");
    enemy.science_purchase_points = 3;
    world.add_player(enemy);
    world
        .ai_manager
        .add_ai_player(0, Team::USA, crate::ai::AIDifficulty::Medium);
    world
        .ai_manager
        .add_ai_player(1, Team::GLA, crate::ai::AIDifficulty::Medium);

    world
        .create_object_for_player("EnemyScienceStructure", 1, Vec3::new(100.0, 0.0, 100.0))
        .unwrap();
    world.frame = 149;
    world
        .ai_manager
        .ai_players
        .get_mut(&0)
        .unwrap()
        .frame_to_check_enemy = 150;
    (world, science)
}

fn install_scripts(world: &mut GameLogic, scripts: ScriptList) {
    let engine = world.with_own_services(|| {
        assert_eq!(
            get_science_store()
                .expect("world CommonServices")
                .get_science_from_internal_name(SCIENCE),
            SCIENCE_INVALID,
        );
        let mut engine = ScriptEngine::new_in_published_world().expect("script engine");
        engine
            .set_script_list_for_player(0, Some(Box::new(scripts)))
            .expect("script list");
        engine
    });
    world.install_owned_script_engine(engine);
    world.scripts_loaded = true;
}

fn engine_flag(world: &GameLogic, name: &str) -> bool {
    world
        .owned_script_engine
        .as_ref()
        .unwrap()
        .get_flag(name)
        .is_some_and(|flag| flag.value)
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn one_acquisition_edge_triggers_only_one_of_two_one_shot_observers() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "one_acquisition_edge_triggers_only_one_of_two_one_shot_observers",
        || {
            let (mut world, _) = world_with_science(0);
            let mut scripts = ScriptList::new();
            scripts.append_script(script(
                "grant_once",
                truth_condition(),
                science_action(ScriptActionType::PlayerGrantScience, SIDE, SCIENCE),
            ));
            scripts.append_script(script(
                "observe_edge_a_once",
                acquired_condition(),
                flag_action(OBSERVER_A),
            ));
            scripts.append_script(script(
                "grant_duplicate_once",
                truth_condition(),
                science_action(ScriptActionType::PlayerGrantScience, SIDE, SCIENCE),
            ));
            scripts.append_script(script(
                "observe_edge_b_once",
                acquired_condition(),
                flag_action(OBSERVER_B),
            ));
            install_scripts(&mut world, scripts);

            world.evaluate_and_execute_scripts(0.0);
            assert!(world.players[&0].has_unlocked_science(SCIENCE));
            assert!(engine_flag(&world, OBSERVER_A));
            assert!(!engine_flag(&world, OBSERVER_B));

            world.frame += 1;
            world.evaluate_and_execute_scripts(0.0);
            assert!(!engine_flag(&world, OBSERVER_B));
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn script_purchase_uses_receiving_cost_and_delivers_one_edge() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "script_purchase_uses_receiving_cost_and_delivers_one_edge",
        || {
            let (mut world, _) = world_with_science(2);
            let (mut peer, _) = world_with_science(4);
            let purchase_scripts = || {
                let mut scripts = ScriptList::new();
                scripts.append_script(script(
                    "purchase_once",
                    science_condition(ConditionType::PlayerCanPurchaseScience, SIDE, 0, SCIENCE),
                    science_action(ScriptActionType::PlayerPurchaseScience, SIDE, SCIENCE),
                ));
                scripts.append_script(script(
                    "observe_purchase",
                    acquired_condition(),
                    flag_action(OBSERVER_A),
                ));
                scripts.append_script(script(
                    "observe_duplicate",
                    acquired_condition(),
                    flag_action(OBSERVER_B),
                ));
                scripts
            };
            install_scripts(&mut world, purchase_scripts());
            install_scripts(&mut peer, purchase_scripts());

            peer.evaluate_and_execute_scripts(0.0);
            world.evaluate_and_execute_scripts(0.0);
            assert!(world.players[&0].has_unlocked_science(SCIENCE));
            assert_eq!(world.players[&0].science_purchase_points, 1);
            assert!(engine_flag(&world, OBSERVER_A));
            assert!(!engine_flag(&world, OBSERVER_B));
            assert!(!peer.players[&0].has_unlocked_science(SCIENCE));
            assert_eq!(peer.players[&0].science_purchase_points, 3);
            assert!(!engine_flag(&peer, OBSERVER_A));

            world.frame += 1;
            world.evaluate_and_execute_scripts(0.0);
            assert_eq!(world.players[&0].science_purchase_points, 1);
            assert!(!engine_flag(&world, OBSERVER_B));
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn this_player_enemy_science_condition_and_action_query_the_receiving_ai_once() {
    use generals_sim::scripting::program::THIS_PLAYER_ENEMY;

    super::sequential_actor_tests::isolated(
        module_path!(),
        "this_player_enemy_science_condition_and_action_query_the_receiving_ai_once",
        || {
            let (mut owner, science) = enemy_science_world(2);
            let (mut peer, _) = enemy_science_world(2);
            let mut scripts = ScriptList::new();
            scripts.append_script(script(
                "purchase_enemy_science",
                science_condition(
                    ConditionType::PlayerCanPurchaseScience,
                    THIS_PLAYER_ENEMY,
                    0,
                    SCIENCE,
                ),
                science_action(
                    ScriptActionType::PlayerGrantScience,
                    THIS_PLAYER_ENEMY,
                    SCIENCE,
                ),
            ));
            let mut fresh_purchase = script(
                "purchase_after_enemy_check_is_due",
                science_condition(
                    ConditionType::PlayerCanPurchaseScience,
                    THIS_PLAYER_ENEMY,
                    0,
                    SCIENCE,
                ),
                science_action(
                    ScriptActionType::PlayerGrantScience,
                    THIS_PLAYER_ENEMY,
                    SCIENCE,
                ),
            );
            fresh_purchase.is_active = false;
            scripts.append_script(fresh_purchase);
            scripts.append_script(script(
                "observe_enemy_science",
                science_condition(
                    ConditionType::PlayerAcquiredScience,
                    THIS_PLAYER_ENEMY,
                    0,
                    SCIENCE,
                ),
                flag_action(OBSERVER_A),
            ));
            install_scripts(&mut owner, scripts);

            let mut peer_scripts = ScriptList::new();
            peer_scripts.append_script(script(
                "grant_enemy_science",
                truth_condition(),
                science_action(
                    ScriptActionType::PlayerGrantScience,
                    THIS_PLAYER_ENEMY,
                    SCIENCE,
                ),
            ));
            install_scripts(&mut peer, peer_scripts);
            assert_eq!(owner.ai_manager.ai_players[&0].enemy_player_id, None);
            assert_eq!(peer.ai_manager.ai_players[&0].enemy_player_id, None);

            owner.evaluate_and_execute_scripts(0.0);
            assert_eq!(owner.ai_manager.ai_players[&0].enemy_player_id, None);
            assert_eq!(owner.ai_manager.ai_players[&0].frame_to_check_enemy, 150);
            assert!(!owner.players[&1].has_unlocked_science(SCIENCE));

            // C++ playerFromParam latches a failed lookup even for an enemy token.
            owner.frame = 150;
            owner.evaluate_and_execute_scripts(0.0);
            assert_eq!(owner.ai_manager.ai_players[&0].enemy_player_id, None);
            assert_eq!(owner.ai_manager.ai_players[&0].frame_to_check_enemy, 150);
            assert!(!owner.players[&1].has_unlocked_science(SCIENCE));

            // A fresh operand asks the receiving AI, then its action reuses
            // that AI's selection in the same frame.
            let engine = owner.owned_script_engine.as_ref().unwrap();
            assert!(engine.set_script_active_by_name("purchase_after_enemy_check_is_due", true));
            owner.frame = 151;
            owner.evaluate_and_execute_scripts(0.0);
            assert_eq!(owner.ai_manager.ai_players[&0].enemy_player_id, Some(1));
            assert_eq!(owner.ai_manager.ai_players[&0].frame_to_check_enemy, 301);
            assert!(owner.players[&1].has_unlocked_science(SCIENCE));
            assert!(!owner.players[&0].has_unlocked_science(SCIENCE));
            assert!(!engine_flag(&owner, OBSERVER_A));
            let engine = owner.owned_script_engine.as_ref().unwrap();
            assert!(engine.is_science_acquired(1, science, false));

            // This observer first resolves the enemy after the AI has acquired it.
            // The original observer still holds its failed SIDE lookup.
            let mut fresh = ScriptList::new();
            fresh.append_script(script(
                "observe_after_enemy_acquired",
                science_condition(
                    ConditionType::PlayerAcquiredScience,
                    THIS_PLAYER_ENEMY,
                    0,
                    SCIENCE,
                ),
                flag_action(OBSERVER_A),
            ));
            engine.set_script_list_for_player(0, Some(Box::new(fresh))).unwrap();
            owner.frame = 152;
            owner.evaluate_and_execute_scripts(0.0);
            assert!(engine_flag(&owner, OBSERVER_A));
            assert!(!owner.owned_script_engine.as_ref().unwrap().is_science_acquired(1, science, false));
            assert_eq!(peer.ai_manager.ai_players[&0].enemy_player_id, None);
            assert_eq!(peer.ai_manager.ai_players[&0].frame_to_check_enemy, 150);
            assert!(!peer.players[&1].has_unlocked_science(SCIENCE));

            // A separate same-ID world reaches the action without a science
            // condition having refreshed its dynamic enemy first.
            peer.frame = 150;
            peer.evaluate_and_execute_scripts(0.0);
            assert_eq!(peer.ai_manager.ai_players[&0].enemy_player_id, Some(1));
            assert_eq!(peer.ai_manager.ai_players[&0].frame_to_check_enemy, 300);
            assert!(peer.players[&1].has_unlocked_science(SCIENCE));
            assert!(!owner.players[&0].has_unlocked_science(SCIENCE));
            assert!(owner.players[&1].has_unlocked_science(SCIENCE));
            assert_eq!(owner.ai_manager.ai_players[&0].frame_to_check_enemy, 301);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn script_science_availability_gates_the_next_purchase_in_the_receiving_world() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "script_science_availability_gates_the_next_purchase_in_the_receiving_world",
        || {
            let (mut world, _) = world_with_science(2);
            let (peer, _) = world_with_science(2);
            let availability = |name: &str, side: &str, token: &str| {
                let mut action =
                    science_action(ScriptActionType::PlayerScienceAvailability, side, SCIENCE);
                action
                    .add_parameter(Parameter::with_string(
                        ParameterType::ScienceAvailability,
                        token.into(),
                    ))
                    .unwrap();
                script(name, truth_condition(), action)
            };
            let mut hidden = ScriptList::new();
            hidden.append_script(availability("hide", SIDE, "Hidden"));
            hidden.append_script(availability("missing_player", "ForeignSide", "Available"));
            hidden.append_script(script(
                "purchase_hidden",
                truth_condition(),
                science_action(ScriptActionType::PlayerPurchaseScience, SIDE, SCIENCE),
            ));
            install_scripts(&mut world, hidden);
            world.evaluate_and_execute_scripts(0.0);
            assert!(world.players[&0].is_science_hidden(SCIENCE));
            assert!(!world.players[&0].is_science_disabled(SCIENCE));
            assert!(!world.players[&0].has_unlocked_science(SCIENCE));
            assert_eq!(world.players[&0].science_purchase_points, 3);
            assert!(!peer.players[&0].is_science_hidden(SCIENCE));

            let mut disabled = ScriptList::new();
            disabled.append_script(availability("disable", SIDE, "Disabled"));
            disabled.append_script(script(
                "purchase_disabled",
                truth_condition(),
                science_action(ScriptActionType::PlayerPurchaseScience, SIDE, SCIENCE),
            ));
            install_scripts(&mut world, disabled);
            world.frame += 1;
            world.evaluate_and_execute_scripts(0.0);
            assert!(world.players[&0].is_science_disabled(SCIENCE));
            assert!(!world.players[&0].is_science_hidden(SCIENCE));
            assert_eq!(world.players[&0].science_purchase_points, 3);
            assert!(!peer.players[&0].is_science_disabled(SCIENCE));

            let mut available = ScriptList::new();
            available.append_script(availability("show", SIDE, "Available"));
            available.append_script(script(
                "purchase_available",
                truth_condition(),
                science_action(ScriptActionType::PlayerPurchaseScience, SIDE, SCIENCE),
            ));
            install_scripts(&mut world, available);
            world.frame += 1;
            world.evaluate_and_execute_scripts(0.0);
            assert!(!world.players[&0].is_science_disabled(SCIENCE));
            assert!(!world.players[&0].is_science_hidden(SCIENCE));
            assert!(world.players[&0].has_unlocked_science(SCIENCE));
            assert_eq!(world.players[&0].science_purchase_points, 1);
            assert_eq!(peer.players[&0].science_purchase_points, 3);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn player_command_receipt_survives_unknown_sources_until_valid_mask_consumes_it() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "player_command_receipt_survives_unknown_sources_until_valid_mask_consumes_it",
        || {
            let (mut world, science) = world_with_science(2);
            let mut scripts = ScriptList::new();
            for (name, side, mask, science) in [
                ("missing_player", "ForeignSide", 0, SCIENCE),
                ("missing_science", SIDE, 0, "SCIENCE_Unknown"),
                ("ambiguous_mask", SIDE, 3, SCIENCE),
                ("missing_mask_player", SIDE, 2, SCIENCE),
                (OBSERVER_A, "StoredNameIgnoredByMask", 1, SCIENCE),
                (OBSERVER_B, SIDE, 0, SCIENCE),
            ] {
                scripts.append_script(script(
                    name,
                    science_condition(ConditionType::PlayerAcquiredScience, side, mask, science),
                    flag_action(name),
                ));
            }
            install_scripts(&mut world, scripts);

            let command = GameCommand {
                command_type: CommandType::PurchaseScience {
                    science_name: SCIENCE.into(),
                },
                player_id: 0,
                command_id: 1,
                timestamp: SystemTime::now(),
                selected_units: Vec::new(),
                modifier_keys: ModifierKeys::default(),
            };
            let mut commands = CommandExecutor::new(&mut world, 0);
            assert_eq!(
                commands.execute_command(command.clone()).unwrap(),
                CommandResult::Success,
            );
            assert_eq!(
                commands.execute_command(command).unwrap(),
                CommandResult::InvalidCommand,
            );
            let engine = world.owned_script_engine.as_ref().unwrap();
            assert!(engine.is_science_acquired(0, science, false));

            world.evaluate_and_execute_scripts(0.0);
            for missing in [
                "missing_player",
                "missing_science",
                "ambiguous_mask",
                "missing_mask_player",
            ] {
                assert!(!engine_flag(&world, missing), "{missing}");
            }
            assert!(engine_flag(&world, OBSERVER_A));
            assert!(!engine_flag(&world, OBSERVER_B));
            assert_eq!(world.players[&0].science_purchase_points, 1);
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn owned_science_before_fresh_engine_has_no_acquisition_event() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "owned_science_before_fresh_engine_has_no_acquisition_event",
        || {
            let (mut world, _) = world_with_science(0);
            assert!(world.grant_player_science(0, SCIENCE));

            let mut scripts = ScriptList::new();
            scripts.append_script(script(
                "observe_no_new_acquisition",
                acquired_condition(),
                flag_action(OBSERVER_A),
            ));
            install_scripts(&mut world, scripts);
            world.evaluate_and_execute_scripts(0.0);

            assert!(world.players[&0].has_unlocked_science(SCIENCE));
            assert!(!engine_flag(&world, OBSERVER_A));
        },
    );
}
