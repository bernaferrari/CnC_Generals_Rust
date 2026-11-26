//! Real ScriptEngine player actions target the borrowed Main owner.
use super::script_execution_driver::HostScriptExecutionDriver;
use super::*;
use gamelogic::scripting::core::{Parameter, ParameterType, ScriptAction, ScriptActionType};
use gamelogic::scripting::engine::ScriptEngine;

fn player_action(kind: ScriptActionType, player: &str, value: i32) -> ScriptAction {
    let mut action = ScriptAction::new(kind);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, value))
        .unwrap();
    action
}

fn repair_action(player: &str, name: &str) -> ScriptAction {
    let mut action = ScriptAction::new(ScriptActionType::PlayerRepairNamedStructure);
    action
        .add_parameter(Parameter::with_string(ParameterType::Side, player.into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
        .unwrap();
    action
}

fn action_chain(player: &str) -> ScriptAction {
    let mut repair = repair_action(player, "ScriptRepairTarget");
    let mut skillset = player_action(ScriptActionType::PlayerSelectSkillset, player, 4);
    skillset.next_action = Some(Box::new(player_action(
        ScriptActionType::SetBaseConstructionSpeed,
        player,
        7,
    )));
    repair.next_action = Some(Box::new(skillset));
    repair
}

fn foreign_controller_snapshot(id: u32) -> Vec<u8> {
    use game_engine::common::system::Xfer;
    let mut bytes = Vec::new();
    let mut save =
        game_engine::system::xfer_save::XferSave::new(std::io::Cursor::new(&mut bytes), 1);
    save.open("foreign_ai_owner").unwrap();
    gamelogic::ai::integration::with_ai_integration_mut(|manager| {
        manager
            .with_ai_player_mut(id, |player| player.xfer(&mut save))
            .unwrap();
    })
    .unwrap();
    save.close().unwrap();
    drop(save);
    bytes
}

fn install_foreign_player(id: u32, name: &str) {
    let mut player = gamelogic::player::Player::new(id as i32);
    player.set_player_name_key(
        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name),
    );
    gamelogic::player::player_list()
        .write()
        .unwrap()
        .add_player(std::sync::Arc::new(std::sync::RwLock::new(player)));
    gamelogic::ai::integration::with_ai_integration_mut(|manager| {
        manager.ensure_ai_player(id, false);
        manager
            .with_ai_player_mut(id, |player| {
                player.select_skillset(9);
                player.set_team_delay_seconds(19.0);
            })
            .unwrap();
    })
    .unwrap();
}

fn owner_world() -> (GameLogic, ObjectId) {
    let mut world = GameLogic::new();
    world.add_player(
        Player::new(1, Team::USA, "Script AI owner display", false)
            .with_script_identity("ScriptAiOwner"),
    );
    world
        .ai_manager
        .add_ai_player(1, Team::USA, crate::ai::AIDifficulty::Medium);
    let mut template = ThingTemplate::new("ScriptAiStructure");
    template.add_kind_of(KindOf::Structure).set_health(100.0);
    world.templates.insert(template.name.clone(), template);
    let id = world
        .create_object_for_player("ScriptAiStructure", 1, glam::Vec3::ZERO)
        .unwrap();
    let object = world.host_object_mut(id).unwrap();
    object.name = "ScriptRepairTarget".into();
    object.health.current = 40.0;
    object.body_damage_state =
        crate::game_logic::host_enum_table_residual::HostBodyDamageType::Damaged;
    world.initialize_receiving_named_cache_for_fixture();
    (world, id)
}

#[test]
fn script_frame_without_supply_conditions_does_not_assess_enemies() {
    let (mut world, _) = owner_world();
    world.get_player_mut(1).unwrap().map_side.is_skirmish_ai = true;
    world.scripts_loaded = true;
    world.frame = 50;

    world.evaluate_and_execute_scripts(1.0 / 30.0);

    assert_eq!(world.ai_manager.ai_players[&1].frame_to_check_enemy, 0);
}

fn enemy_alias_world() -> GameLogic {
    use game_engine::common::game_common::Relationship;

    let (mut world, _) = owner_world();
    world.game_mode = GameMode::Skirmish;
    world.add_player(
        Player::new(2, Team::GLA, "Script enemy", false).with_script_identity("ScriptEnemy"),
    );
    let current = world.get_player_mut(1).unwrap();
    current.map_side.is_skirmish_ai = true;
    current.set_map_relationship(2, Relationship::Enemies);
    world
        .ai_manager
        .add_ai_player(2, Team::GLA, crate::ai::AIDifficulty::Medium);
    world
        .create_object_for_player("ScriptAiStructure", 2, Vec3::new(100.0, 0.0, 100.0))
        .expect("live enemy structure");
    world
}

#[test]
fn enemy_player_token_queries_the_receiving_ai_during_the_action() {
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Script, ScriptList, THIS_PLAYER_ENEMY,
    };
    use gamelogic::scripting::executor::ScriptContext;

    let mut first = enemy_alias_world();
    let peer = enemy_alias_world();
    first.frame = 149;
    let original_delay = peer.ai_manager.ai_players[&2]
        .capture_queue_persist()
        .team_seconds;
    assert_ne!(original_delay, 7.0);
    let mut branch = OrCondition::new();
    branch.set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    let mut script = Script::new();
    script.script_name = "SetEnemyTeamDelay".into();
    script.condition = Some(Box::new(branch));
    script.action = Some(Box::new(player_action(
        ScriptActionType::SetBaseConstructionSpeed,
        THIS_PLAYER_ENEMY,
        7,
    )));
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    first
        .with_owned_script_engine(|engine, world| {
            engine
                .set_script_list_for_player(1, Some(Box::new(list)))
                .unwrap();
            let context = ScriptContext::at_frame(world.frame);
            engine
                .update_with_driver(
                    context,
                    &mut HostScriptExecutionDriver::with_interpreter(world, engine),
                )
                .unwrap();
        })
        .unwrap();

    assert_eq!(first.ai_manager.ai_players[&1].enemy_player_id, Some(2));
    assert_eq!(first.ai_manager.ai_players[&1].frame_to_check_enemy, 299);
    assert_eq!(
        first.ai_manager.ai_players[&2]
            .capture_queue_persist()
            .team_seconds,
        7.0
    );
    assert_eq!(peer.ai_manager.ai_players[&1].frame_to_check_enemy, 0);
    assert_eq!(
        peer.ai_manager.ai_players[&2]
            .capture_queue_persist()
            .team_seconds,
        original_delay
    );
}

#[test]
fn enemy_player_census_assesses_on_the_receiving_condition() {
    use gamelogic::scripting::core::{
        Condition, ConditionType, OrCondition, Script, THIS_PLAYER_ENEMY,
    };
    use gamelogic::scripting::executor::ScriptContext;

    fn run_condition(world: &mut GameLogic) -> bool {
        let mut condition = Condition::new(ConditionType::PlayerHasNOrFewerBuildings);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                THIS_PLAYER_ENEMY.into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 1))
            .unwrap();
        let mut branch = OrCondition::new();
        branch.set_first_and_condition(Some(Box::new(condition)));
        let mut script = Script::new();
        script.condition = Some(Box::new(branch));
        let passes = world
            .with_owned_script_engine(|engine, world| {
                engine.evaluate_conditions_with_driver(
                    &mut script,
                    None,
                    Some("ScriptAiOwner"),
                    ScriptContext::at_frame(world.frame),
                    &mut HostScriptExecutionDriver::with_interpreter(world, engine),
                )
            })
            .unwrap();
        let condition = script
            .condition
            .as_ref()
            .unwrap()
            .get_first_and_condition()
            .unwrap();
        assert_eq!(condition.get_parameter(0).unwrap().get_int(), 0);
        passes
    }

    let mut first = enemy_alias_world();
    first.frame = 149;
    let mut peer = enemy_alias_world();
    peer.frame = 50;
    peer.create_object_for_player("ScriptAiStructure", 2, Vec3::new(150.0, 0.0, 150.0))
        .expect("second peer building");

    assert!(run_condition(&mut first));
    assert_eq!(first.ai_manager.ai_players[&1].enemy_player_id, Some(2));
    assert_eq!(first.ai_manager.ai_players[&1].frame_to_check_enemy, 299);
    assert_eq!(peer.ai_manager.ai_players[&1].frame_to_check_enemy, 0);
    assert!(!run_condition(&mut peer));
    assert_eq!(peer.ai_manager.ai_players[&1].enemy_player_id, Some(2));
    assert_eq!(peer.ai_manager.ai_players[&1].frame_to_check_enemy, 200);
    assert_eq!(first.ai_manager.ai_players[&1].frame_to_check_enemy, 299);
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn authored_player_ai_actions_ignore_foreign_same_id_controller_and_preserve_core_fallback() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "authored_player_ai_actions_ignore_foreign_same_id_controller_and_preserve_core_fallback",
        || {
            gamelogic::ai::integration::initialize_ai_integration().unwrap();
            install_foreign_player(1, "ScriptAiOwner");
            let (mut first, id) = owner_world();
            let (second, other_id) = owner_world();
            assert_eq!(id, other_id);
            let foreign = std::sync::Arc::new(std::sync::RwLock::new(
                gamelogic::object::Object::new_for_xfer_load(id.0, 100.0),
            ));
            foreign
                .read()
                .unwrap()
                .get_body_module()
                .unwrap()
                .lock()
                .unwrap()
                .set_damage_state(gamelogic::object::body::BodyDamageType::Damaged)
                .unwrap();
            gamelogic::object::registry::OBJECT_REGISTRY.register_object(id.0, &foreign);
            gamelogic::scripting::engine::get_named_object_tracker()
                .register_named_object("ScriptRepairTarget".into(), id.0)
                .unwrap();
            let before = foreign_controller_snapshot(1);
            let second_before = second.ai_manager.ai_players[&1].capture_queue_persist();
            let engine = ScriptEngine::new_in_published_world().unwrap();
            let actions = action_chain("ScriptAiOwner");
            engine.friend_execute_action_with_driver(
                &actions,
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut HostScriptExecutionDriver::new(&mut first),
            );
            let owned = first.ai_manager.ai_players[&1].capture_queue_persist();
            assert_eq!(owned.structures_to_repair, vec![id.0]);
            assert_eq!(
                owned.skillset_selector, 3,
                "script numbering is converted exactly once"
            );
            assert_eq!(
                owned.team_seconds, 7.0,
                "CPP construction-speed action sets the AI team delay"
            );
            assert_eq!(
                foreign_controller_snapshot(1),
                before,
                "installed foreign Core controller is untouched"
            );
            assert_eq!(
                serde_json::to_vec(&second.ai_manager.ai_players[&1].capture_queue_persist())
                    .unwrap(),
                serde_json::to_vec(&second_before).unwrap()
            );
            assert!(gamelogic::scripting::take_host_script_player_misc_requests().is_empty());
            assert!(
                gamelogic::scripting::take_host_set_base_construction_speed_requests().is_empty()
            );

            // A genuine standalone Core walk must still execute its controller.
            // Use each action separately to witness all three fallback effects.
            let mut prior = before;
            for action in [
                repair_action("ScriptAiOwner", "ScriptRepairTarget"),
                player_action(ScriptActionType::PlayerSelectSkillset, "ScriptAiOwner", 4),
                player_action(
                    ScriptActionType::SetBaseConstructionSpeed,
                    "ScriptAiOwner",
                    7,
                ),
            ] {
                engine.friend_execute_action(&action, None);
                let next = foreign_controller_snapshot(1);
                assert_ne!(
                    next, prior,
                    "standalone Core action retains its real controller effect"
                );
                prior = next;
            }
        },
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn missing_main_ai_player_is_authoritative_even_when_core_player_exists() {
    super::sequential_actor_tests::isolated(
        module_path!(),
        "missing_main_ai_player_is_authoritative_even_when_core_player_exists",
        || {
            gamelogic::ai::integration::initialize_ai_integration().unwrap();
            install_foreign_player(2, "OnlyCorePlayer");
            let (mut world, id) = owner_world();
            let before = foreign_controller_snapshot(2);
            let owned_before =
                serde_json::to_vec(&world.ai_manager.ai_players[&1].capture_queue_persist())
                    .unwrap();
            let engine = ScriptEngine::new_in_published_world().unwrap();
            engine.friend_execute_action_with_driver(
                &action_chain("OnlyCorePlayer"),
                None,
                gamelogic::scripting::executor::ScriptContext::new(),
                &mut HostScriptExecutionDriver::new(&mut world),
            );
            assert_eq!(foreign_controller_snapshot(2), before);
            assert_eq!(
                serde_json::to_vec(&world.ai_manager.ai_players[&1].capture_queue_persist())
                    .unwrap(),
                owned_before
            );
            assert!(world.host_object(id).is_some());
            assert!(gamelogic::scripting::take_host_script_player_misc_requests().is_empty());
            assert!(
                gamelogic::scripting::take_host_set_base_construction_speed_requests().is_empty()
            );
        },
    );
}

#[test]
fn authored_superweapon_action_targets_and_consumes_only_receiving_world() {
    use crate::command_system::{CommandType, PowerTarget, SpecialPowerType};
    use crate::game_logic::{SpecialPowerModuleKind, SpecialPowerModuleMetadata};
    use game_engine::common::random_value::SessionRandom;

    fn world(target_x: f32) -> (GameLogic, ObjectId, ObjectId) {
        let mut world = GameLogic::new();
        world.add_player(Player::new(1, Team::USA, "AI", false).with_script_identity("ScriptAiOwner"));
        world.add_player(Player::new(2, Team::GLA, "Enemy", true).with_script_identity("Enemy"));
        world.ai_manager.add_ai_player(1, Team::USA, crate::ai::AIDifficulty::Medium);
        world.get_player_mut(1).unwrap().map_side.is_skirmish_ai = true;
        world.get_player_mut(1).unwrap().map_side.relations.insert(2, game_engine::common::game_common::Relationship::Enemies);
        world.get_player_mut(2).unwrap().map_side.relations.insert(1, game_engine::common::game_common::Relationship::Enemies);

        let mut cannon = ThingTemplate::new("ScriptCannon");
        cannon.set_health(4000.0);
        cannon.add_kind_of(KindOf::Structure);
        cannon.add_kind_of(KindOf::FSSuperweapon);
        cannon.special_power_modules.push(SpecialPowerModuleMetadata {
            source_index: 0,
            module_tag: Some("ModuleTag_SpecialPower".into()),
            module_kind: SpecialPowerModuleKind::OclSpecialPower,
            special_power_template: "SuperweaponParticleUplinkCannon".into(),
            special_power_template_id: 1,
            command_power: Some(SpecialPowerType::ParticleCannon),
            reload_time_frames: 0,
            required_science: None,
            public_timer: true,
            shared_n_sync: false,
            shortcut_power: false,
            update_module_starts_attack: false,
            starts_paused: false,
            scripted_special_power_only: false,
            cash_bounty: None,
        });
        world.templates.insert("ScriptCannon".into(), cannon);
        let mut target = ThingTemplate::new("ScriptTarget");
        target.set_health(1000.0);
        target.set_cost(500, 0);
        target.add_kind_of(KindOf::Structure);
        target.add_kind_of(KindOf::Attackable);
        world.templates.insert("ScriptTarget".into(), target);
        let caster = world.create_object_for_player("ScriptCannon", 1, Vec3::new(20.0, 0.0, 20.0)).unwrap();
        let enemy = world.create_object_for_player("ScriptTarget", 2, Vec3::new(target_x, 0.0, 80.0)).unwrap();
        assert_eq!(world.query_owned_ai_enemy(1), Some(2));
        (world, caster, enemy)
    }

    let (mut first, caster, target) = world(100.0);
    let (mut second, peer_caster, peer_target) = world(500.0);
    assert_eq!((caster, target), (peer_caster, peer_target));
    first.logic_random.seed_random(1234);
    second.logic_random.seed_random(9876);
    let expected_first = SessionRandom::default();
    expected_first.seed_random(1234);
    let expected_second = SessionRandom::default();
    expected_second.seed_random(9876);
    let mut action = ScriptAction::new(ScriptActionType::SkirmishFireSpecialPowerAtMostCost);
    action.add_parameter(Parameter::with_string(ParameterType::Side, "ScriptAiOwner".into())).unwrap();
    action.add_parameter(Parameter::with_string(ParameterType::SpecialPower, "SuperweaponParticleUplinkCannon".into())).unwrap();
    let engine = ScriptEngine::new_in_published_world().unwrap();

    let fire = |world: &mut GameLogic| {
        engine.friend_execute_action_with_driver(
            &action, None, gamelogic::scripting::executor::ScriptContext::new(),
            &mut HostScriptExecutionDriver::new(world),
        );
    };
    fire(&mut first);
    expected_first.next_int(1, 4);
    assert_eq!(first.logic_random.seed_words(), expected_first.seed_words());
    assert_eq!(second.logic_random.seed_words(), expected_second.seed_words());
    assert!(second.command_queue.is_empty());
    let command = first.command_queue.front().expect("script must queue the real special-power command");
    assert_eq!(command.selected_units, vec![caster]);
    match &command.command_type {
        CommandType::DoSpecialPower { power_type: SpecialPowerType::ParticleCannon, target: PowerTarget::Location(pos) } => {
            assert!((pos.x - 100.0).abs() < 0.01, "{pos:?}");
            assert!((pos.z - 80.0).abs() < 0.01, "{pos:?}");
        }
        other => panic!("unexpected command: {other:?}"),
    }
    fire(&mut second);
    expected_second.next_int(1, 4);
    assert_eq!(second.logic_random.seed_words(), expected_second.seed_words());
    let command = second.command_queue.front().expect("peer queues its own command");
    match &command.command_type {
        CommandType::DoSpecialPower { power_type: SpecialPowerType::ParticleCannon, target: PowerTarget::Location(pos) } => {
            assert!((pos.x - 500.0).abs() < 0.01, "{pos:?}");
            assert!((pos.z - 80.0).abs() < 0.01, "{pos:?}");
        }
        other => panic!("unexpected peer command: {other:?}"),
    }
    first.process_commands();
    assert_eq!(second.special_power_strikes().strike_count(), 0);
    assert!(first.special_power_strikes().honesty_queue_ok(crate::game_logic::HostSuperweaponKind::ParticleCannon));
    second.process_commands();
    assert!(second.special_power_strikes().honesty_queue_ok(crate::game_logic::HostSuperweaponKind::ParticleCannon));
}
