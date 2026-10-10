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
    world.add_player(Player::new(1, Team::USA, "ScriptAiOwner", false));
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
    (world, id)
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
            let engine = ScriptEngine::new().unwrap();
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
            let engine = ScriptEngine::new().unwrap();
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
