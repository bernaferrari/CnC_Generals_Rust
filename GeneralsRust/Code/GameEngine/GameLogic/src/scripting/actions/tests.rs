//! Tests for script actions.
//!
//! Split from `scripting/actions.rs` for module-size parity.

use super::building::*;
use super::camera_ui::*;
use super::leftover::*;
use super::music_audio::*;
use super::named_unit::*;
use super::object_actions::*;
use super::player_command::*;
use super::player_economy::*;
use super::science_special::*;
use super::team_command::*;
use super::unit_actions::*;
use super::weather_radar::*;
use super::*;
use crate::ai::{AiCommandParams, AiCommandType, GuardMode};
use crate::common::PlayerIndex;
use crate::common::{AsciiString, CommandSourceType, Coord3D, LocomotorSetType, Relationship};
use crate::helpers::TheGameLogic;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object_manager::{ObjectCreationFlags, get_object_manager};
use crate::player::player_list;
use crate::scripting::engine::{get_named_object_tracker, get_script_engine};
use crate::scripting::{ScriptContext, ScriptResult, ScriptValue};
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::{GameLogicError, GameLogicResult};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

fn test_context() -> ScriptContext {
    ScriptContext {
        game_time: std::time::Duration::from_secs(0),
        active_player: Some(0),
        variables: HashMap::new(),
        game_state: crate::scripting::GameStateContext {
            map_name: "Test".to_string(),
            game_mode: "Test".to_string(),
            players: vec![],
            objectives: vec![],
        },
    }
}

fn reset_test_player(index: PlayerIndex, money: i32) {
    let mut list = player_list().write().unwrap();
    list.clear();
    let mut player = crate::player::Player::new(index);
    player.set_display_name(format!("Player{index}"));
    player.get_money_mut().set_money(money);
    list.add_player(Arc::new(RwLock::new(player)));
}

fn reset_test_players(count: PlayerIndex) {
    let mut list = player_list().write().unwrap();
    list.clear();
    for index in 0..count {
        let mut player = crate::player::Player::new(index);
        player.set_display_name(format!("Player{index}"));
        list.add_player(Arc::new(RwLock::new(player)));
    }
}

fn reset_test_script_engine() {
    let engine_lock = get_script_engine();
    let mut engine = engine_lock.write().unwrap();
    *engine = Some(crate::scripting::engine::ScriptEngine::new().unwrap());
}

fn reset_test_object_manager() {
    get_object_manager().write().unwrap().reset();
}

fn reset_test_team_factory() {
    get_team_factory().lock().unwrap().reset();
}

fn reset_test_named_object_tracker() {
    get_named_object_tracker().clear().unwrap();
}

fn reset_test_terrain() {
    get_terrain_logic().write().unwrap().reset();
}

fn ensure_test_template(name: &str) {
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};

    if get_thing_factory().unwrap().is_none() {
        init_thing_factory().unwrap();
    }
    let mut factory_guard = get_thing_factory().unwrap();
    let factory = factory_guard.as_mut().unwrap();
    if factory.find_template(name, false).is_none() {
        factory.new_template(name);
    }
}

#[tokio::test]
async fn test_action_registry() {
    let registry = ActionRegistry::new();

    let actions = registry.list_actions();
    assert!(actions.contains(&"create_unit".to_string()));
    assert!(actions.contains(&"move_unit".to_string()));
    assert!(actions.contains(&"play_sound".to_string()));
}

#[tokio::test]
async fn test_create_unit_action() {
    use game_engine::common::thing::thing_factory::{get_thing_factory, init_thing_factory};

    // Ensure a template exists for the requested unit type.
    // The fully-implemented `CreateUnitAction` now uses the real object factory path.
    let needs_init = get_thing_factory().unwrap().is_none();
    if needs_init {
        init_thing_factory().unwrap();
    }
    {
        let mut factory_guard = get_thing_factory().unwrap();
        if let Some(factory) = factory_guard.as_mut() {
            if factory.find_template("Tank", false).is_none() {
                factory.new_template("Tank");
            }
        }
    }

    let action = CreateUnitAction;
    let mut params = HashMap::new();
    params.insert("player".to_string(), ScriptValue::Int(1));
    params.insert(
        "unit_type".to_string(),
        ScriptValue::String("Tank".to_string()),
    );
    params.insert("x".to_string(), ScriptValue::Float(100.0));
    params.insert("y".to_string(), ScriptValue::Float(200.0));

    let context = ScriptContext {
        game_time: std::time::Duration::from_secs(0),
        active_player: Some(1),
        variables: HashMap::new(),
        game_state: crate::scripting::GameStateContext {
            map_name: "Test".to_string(),
            game_mode: "Test".to_string(),
            players: vec![],
            objectives: vec![],
        },
    };

    let result = action.execute(&params, &context).await.unwrap();
    assert!(matches!(result, ScriptResult::Success(_)));
}

#[tokio::test]
async fn test_parameter_extraction() {
    let mut params = HashMap::new();
    params.insert(
        "test_string".to_string(),
        ScriptValue::String("hello".to_string()),
    );
    params.insert("test_int".to_string(), ScriptValue::Int(42));
    params.insert("test_float".to_string(), ScriptValue::Float(3.14));

    assert_eq!(get_string_param(&params, "test_string").unwrap(), "hello");
    assert_eq!(get_int_param(&params, "test_int").unwrap(), 42);
    assert_eq!(get_float_param(&params, "test_float").unwrap(), 3.14);
}

#[tokio::test]
async fn set_player_resource_sets_money() {
    reset_test_player(0, 250);

    let action = SetPlayerResourceAction;
    let mut params = HashMap::new();
    params.insert("player".to_string(), ScriptValue::Int(0));
    params.insert(
        "resource_type".to_string(),
        ScriptValue::String("cash".to_string()),
    );
    params.insert("amount".to_string(), ScriptValue::Int(1200));

    action.execute(&params, &test_context()).await.unwrap();

    let list = player_list().read().unwrap();
    let player = list.get_player(0).unwrap().read().unwrap();
    assert_eq!(player.get_money().get_money(), 1200);
}

#[tokio::test]
async fn add_player_resource_updates_money_and_ignores_unknown_resources() {
    reset_test_player(0, 500);

    let action = AddPlayerResourceAction;
    let mut params = HashMap::new();
    params.insert("player".to_string(), ScriptValue::Int(0));
    params.insert(
        "resource_type".to_string(),
        ScriptValue::String("supplies".to_string()),
    );
    params.insert("amount".to_string(), ScriptValue::Int(300));

    action.execute(&params, &test_context()).await.unwrap();

    params.insert(
        "resource_type".to_string(),
        ScriptValue::String("oil".to_string()),
    );
    params.insert("amount".to_string(), ScriptValue::Int(999));
    action.execute(&params, &test_context()).await.unwrap();

    let list = player_list().read().unwrap();
    let player = list.get_player(0).unwrap().read().unwrap();
    assert_eq!(player.get_money().get_money(), 800);
}

#[tokio::test]
async fn named_money_actions_update_money_without_reentrant_deposit() {
    reset_test_player(0, 500);

    let mut params = HashMap::new();
    params.insert(
        "player".to_string(),
        ScriptValue::String("Player0".to_string()),
    );
    params.insert("amount".to_string(), ScriptValue::Int(250));
    GiveMoneyAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    params.insert("amount".to_string(), ScriptValue::Int(-1000));
    GiveMoneyAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    params.insert("amount".to_string(), ScriptValue::Int(1200));
    SetMoneyAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let list = player_list().read().unwrap();
    let player = list.get_player(0).unwrap().read().unwrap();
    assert_eq!(player.get_money().get_money(), 1200);
    assert_eq!(player.get_score_keeper().get_total_money_spent(), 750);
}

#[tokio::test]
async fn indexed_player_add_money_spends_only_available_money() {
    reset_test_player(0, 300);

    let mut params = HashMap::new();
    params.insert("player".to_string(), ScriptValue::Int(0));
    params.insert("amount".to_string(), ScriptValue::Int(-500));

    PlayerAddMoneyAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let list = player_list().read().unwrap();
    let player = list.get_player(0).unwrap().read().unwrap();
    assert_eq!(player.get_money().get_money(), 0);
    assert_eq!(player.get_score_keeper().get_total_money_spent(), 300);
}

#[tokio::test]
async fn timer_actions_update_script_engine_counters() {
    reset_test_script_engine();

    let mut params = HashMap::new();
    params.insert(
        "counter_name".to_string(),
        ScriptValue::String("TimerA".to_string()),
    );
    params.insert("milliseconds".to_string(), ScriptValue::Int(1500));

    StartTimerAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    {
        let engine = get_script_engine();
        let guard = engine.read().unwrap();
        let counter = guard
            .as_ref()
            .unwrap()
            .get_counter("TimerA")
            .expect("timer counter");
        assert_eq!(counter.value, 45);
        assert!(counter.is_countdown_timer);
    }

    params.remove("milliseconds");
    StopTimerAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let engine = get_script_engine();
    let guard = engine.read().unwrap();
    let counter = guard
        .as_ref()
        .unwrap()
        .get_counter("TimerA")
        .expect("timer counter");
    assert_eq!(counter.value, 45);
    assert!(!counter.is_countdown_timer);
}

#[tokio::test]
async fn display_timer_actions_create_counter_state() {
    reset_test_script_engine();

    let mut params = HashMap::new();
    params.insert(
        "timer".to_string(),
        ScriptValue::String("CounterA".to_string()),
    );
    params.insert("value".to_string(), ScriptValue::Int(7));

    SetTimerAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    params.clear();
    params.insert(
        "timer".to_string(),
        ScriptValue::String("CountdownA".to_string()),
    );
    params.insert("seconds".to_string(), ScriptValue::Int(3));

    CountdownTimerAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let engine = get_script_engine();
    let guard = engine.read().unwrap();
    let engine = guard.as_ref().unwrap();
    let counter = engine.get_counter("CounterA").expect("counter");
    assert_eq!(counter.value, 7);
    assert!(!counter.is_countdown_timer);

    let countdown = engine.get_counter("CountdownA").expect("countdown");
    assert_eq!(countdown.value, 90);
    assert!(countdown.is_countdown_timer);
}

#[tokio::test]
async fn set_team_alliance_sets_one_way_player_relationship() {
    reset_test_players(2);

    let mut params = HashMap::new();
    params.insert("player1".to_string(), ScriptValue::Int(0));
    params.insert("player2".to_string(), ScriptValue::Int(1));
    params.insert(
        "relation".to_string(),
        ScriptValue::String("enemy".to_string()),
    );

    SetTeamAllianceAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let list = player_list().read().unwrap();
    let player0 = list.get_player(0).unwrap().read().unwrap();
    let player1 = list.get_player(1).unwrap().read().unwrap();
    assert_eq!(player0.get_relationship(&player1), Relationship::Enemies);
    assert_eq!(player1.get_relationship(&player0), Relationship::Neutral);
}

#[tokio::test]
async fn destroy_building_queues_object_manager_removal() {
    reset_test_object_manager();

    let object =
        crate::object_manager::GameObjectInstance::new(700, None, None, ObjectCreationFlags::new())
            .expect("test object instance");
    {
        let manager = get_object_manager();
        manager
            .write()
            .unwrap()
            .register_object_instance(object, Coord3D::new(0.0, 0.0, 0.0))
            .unwrap();
    }

    let mut params = HashMap::new();
    params.insert("object_id".to_string(), ScriptValue::Int(700));

    DestroyBuildingAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let manager = get_object_manager();
    let mut manager = manager.write().unwrap();
    assert!(manager.get_object(700).is_some());
    manager.update(0).unwrap();
    assert!(manager.get_object(700).is_none());
}

#[tokio::test]
async fn spawn_reinforcements_creates_grid_formation() {
    reset_test_object_manager();
    ensure_test_template("TestReinforcement");

    let mut params = HashMap::new();
    params.insert("player".to_string(), ScriptValue::Int(0));
    params.insert(
        "unit_type".to_string(),
        ScriptValue::String("TestReinforcement".to_string()),
    );
    params.insert("count".to_string(), ScriptValue::Int(6));
    params.insert("x".to_string(), ScriptValue::Float(100.0));
    params.insert("y".to_string(), ScriptValue::Float(200.0));
    params.insert("spacing".to_string(), ScriptValue::Float(12.0));

    let result = SpawnReinforcementsAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let ScriptResult::Success(Some(ScriptValue::Array(ids))) = result else {
        panic!("expected created object id array");
    };
    assert_eq!(ids.len(), 6);

    let manager = get_object_manager();
    let manager = manager.read().unwrap();
    let expected = [
        Coord3D::new(100.0, 200.0, 0.0),
        Coord3D::new(112.0, 200.0, 0.0),
        Coord3D::new(124.0, 200.0, 0.0),
        Coord3D::new(136.0, 200.0, 0.0),
        Coord3D::new(148.0, 200.0, 0.0),
        Coord3D::new(100.0, 212.0, 0.0),
    ];

    for (value, expected_pos) in ids.iter().zip(expected.iter()) {
        let ScriptValue::ObjectId(object_id) = value else {
            panic!("expected object id");
        };
        let object = manager.get_object(*object_id).expect("created object");
        let object = object.read().unwrap();
        assert_eq!(*object.get_position(), *expected_pos);
    }
}

#[tokio::test]
async fn give_special_power_initializes_player_ready_timer() {
    reset_test_players(1);
    if let Some(mut store) = crate::object::special_power_template::get_special_power_store_mut() {
        store.reset();
    }

    let expected_frame = TheGameLogic::get_frame();
    let mut params = HashMap::new();
    params.insert("player".to_string(), ScriptValue::Int(0));
    params.insert(
        "power_name".to_string(),
        ScriptValue::String("TestScriptPower".to_string()),
    );

    GiveSpecialPowerAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let template = find_or_create_special_power_template(&AsciiString::from("TestScriptPower"));
    assert_eq!(template.get_name(), "TestScriptPower");

    let list = player_list().read().unwrap();
    let mut player = list.get_player(0).unwrap().write().unwrap();
    assert_eq!(
        player.get_or_start_special_power_ready_frame(&template),
        expected_frame
    );
}

#[tokio::test]
async fn weather_set_dispatches_visibility_to_action_handler() {
    use crate::scripting::engine::ScriptActionHandler;
    use std::sync::Mutex;

    struct RecordingWeatherHandler {
        calls: Arc<Mutex<Vec<bool>>>,
    }

    impl ScriptActionHandler for RecordingWeatherHandler {
        fn set_weather_visible(&self, visible: bool) -> GameLogicResult<()> {
            self.calls.lock().unwrap().push(visible);
            Ok(())
        }
    }

    reset_test_script_engine();
    let calls = Arc::new(Mutex::new(Vec::new()));
    {
        let engine_lock = get_script_engine();
        let mut engine = engine_lock.write().unwrap();
        engine
            .as_mut()
            .unwrap()
            .set_action_handler(Some(Arc::new(RecordingWeatherHandler {
                calls: Arc::clone(&calls),
            })));
    }

    let mut params = HashMap::new();
    params.insert(
        "weather_type".to_string(),
        ScriptValue::String("snow".to_string()),
    );
    params.insert("enabled".to_string(), ScriptValue::Bool(false));

    WeatherSetAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    assert_eq!(*calls.lock().unwrap(), vec![false]);
}

#[tokio::test]
async fn player_hunt_sets_player_hunt_flag() {
    reset_test_player(0, 0);

    let mut params = HashMap::new();
    params.insert("player".to_string(), ScriptValue::Int(0));

    PlayerHuntAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    let list = player_list().read().unwrap();
    let player = list.get_player(0).unwrap();
    assert!(player.read().unwrap().get_units_should_hunt());
}

#[tokio::test]
async fn team_guard_without_position_guards_each_member_at_own_position() {
    use crate::modules::AIUpdateInterface;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingAi {
        calls: Arc<Mutex<Vec<(AiCommandType, Coord3D, i32, CommandSourceType)>>>,
    }

    impl AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn execute_command(
            &mut self,
            command: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.calls.lock().unwrap().push((
                command.cmd,
                command.pos,
                command.int_value,
                command.cmd_source,
            ));
            Ok(())
        }
    }

    reset_test_object_manager();
    reset_test_team_factory();

    let team = {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("GuardTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("GuardTeam")
            .expect("team should be created")
    };

    let positions = [Coord3D::new(10.0, 20.0, 0.0), Coord3D::new(40.0, 80.0, 0.0)];
    let mut calls = Vec::new();
    for (idx, position) in positions.iter().enumerate() {
        let object_id = 8100 + idx as u32;
        let call_log = Arc::new(Mutex::new(Vec::new()));
        calls.push(Arc::clone(&call_log));

        let object = crate::object_manager::GameObjectInstance::new(
            object_id,
            None,
            Some(Arc::clone(&team)),
            ObjectCreationFlags::new(),
        )
        .expect("test object instance");
        {
            let instance = &object;
            instance
                .base()
                .write()
                .unwrap()
                .set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                    calls: call_log,
                }))));
        }
        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(object, *position)
            .unwrap();
        team.write().unwrap().add_member(object_id);
    }

    let mut params = HashMap::new();
    params.insert(
        "team_name".to_string(),
        ScriptValue::String("GuardTeam".to_string()),
    );

    TeamGuardAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    for (call_log, expected_pos) in calls.iter().zip(positions.iter()) {
        assert_eq!(
            *call_log.lock().unwrap(),
            vec![(
                AiCommandType::GuardPosition,
                *expected_pos,
                GuardMode::Normal.as_i32(),
                CommandSourceType::FromScript,
            )]
        );
    }
}

#[tokio::test]
async fn team_attack_area_dispatches_attack_area_to_team_members() {
    use crate::common::ICoord3D;
    use crate::modules::AIUpdateInterface;
    use crate::polygon_trigger::PolygonTrigger;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingAi {
        calls: Arc<Mutex<Vec<(AiCommandType, Coord3D, Option<i32>, CommandSourceType)>>>,
    }

    impl AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn execute_command(
            &mut self,
            command: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.calls.lock().unwrap().push((
                command.cmd,
                command.pos,
                command.polygon,
                command.cmd_source,
            ));
            Ok(())
        }
    }

    reset_test_object_manager();
    reset_test_team_factory();
    reset_test_terrain();

    let trigger_id = 6200;
    let expected_center = Coord3D::new(50.0, 70.0, 0.0);
    get_terrain_logic()
        .write()
        .unwrap()
        .add_trigger_area(PolygonTrigger::new(
            trigger_id,
            AsciiString::from("AttackZone"),
            vec![
                ICoord3D::new(10, 20, 0),
                ICoord3D::new(90, 20, 0),
                ICoord3D::new(90, 120, 0),
                ICoord3D::new(10, 120, 0),
            ],
        ));

    let team = {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("AttackAreaTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("AttackAreaTeam")
            .expect("team should be created")
    };

    let mut calls = Vec::new();
    for idx in 0..2 {
        let object_id = 8200 + idx as u32;
        let call_log = Arc::new(Mutex::new(Vec::new()));
        calls.push(Arc::clone(&call_log));

        let object = crate::object_manager::GameObjectInstance::new(
            object_id,
            None,
            Some(Arc::clone(&team)),
            ObjectCreationFlags::new(),
        )
        .expect("test object instance");
        {
            let instance = &object;
            instance
                .base()
                .write()
                .unwrap()
                .set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                    calls: call_log,
                }))));
        }
        get_object_manager()
            .write()
            .unwrap()
            .register_object_instance(object, Coord3D::new(idx as f32, 0.0, 0.0))
            .unwrap();
        team.write().unwrap().add_member(object_id);
    }

    let mut params = HashMap::new();
    params.insert(
        "team".to_string(),
        ScriptValue::String("AttackAreaTeam".to_string()),
    );
    params.insert(
        "area".to_string(),
        ScriptValue::String("AttackZone".to_string()),
    );

    TeamAttackAreaAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    for call_log in calls {
        assert_eq!(
            *call_log.lock().unwrap(),
            vec![(
                AiCommandType::AttackArea,
                expected_center,
                Some(trigger_id),
                CommandSourceType::FromScript,
            )]
        );
    }
}

#[tokio::test]
async fn named_attack_area_leaves_group_and_dispatches_attack_area() {
    use crate::common::ICoord3D;
    use crate::modules::AIUpdateInterface;
    use crate::polygon_trigger::PolygonTrigger;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingAi {
        commands: Arc<Mutex<Vec<(AiCommandType, Coord3D, Option<i32>, CommandSourceType)>>>,
        locomotors: Arc<Mutex<Vec<LocomotorSetType>>>,
    }

    impl AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn choose_locomotor_set(
            &mut self,
            set: LocomotorSetType,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.locomotors.lock().unwrap().push(set);
            Ok(())
        }

        fn execute_command(
            &mut self,
            command: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.commands.lock().unwrap().push((
                command.cmd,
                command.pos,
                command.polygon,
                command.cmd_source,
            ));
            Ok(())
        }
    }

    reset_test_object_manager();
    reset_test_named_object_tracker();
    reset_test_terrain();

    let trigger_id = 6300;
    let expected_center = Coord3D::new(60.0, 85.0, 0.0);
    get_terrain_logic()
        .write()
        .unwrap()
        .add_trigger_area(PolygonTrigger::new(
            trigger_id,
            AsciiString::from("NamedAttackZone"),
            vec![
                ICoord3D::new(20, 30, 0),
                ICoord3D::new(100, 30, 0),
                ICoord3D::new(100, 140, 0),
                ICoord3D::new(20, 140, 0),
            ],
        ));

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let object_id = 8300;
    let object = crate::object_manager::GameObjectInstance::new(
        object_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test object instance");

    {
        let __base_arc = object.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
        }))));
        base.enter_group(&crate::ai::AIGroup::new(77));
        assert_eq!(base.get_group_id(), Some(77));
    }

    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(object, Coord3D::new(5.0, 10.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("NamedAttacker".to_string(), object_id)
        .unwrap();

    let mut params = HashMap::new();
    params.insert(
        "unit_name".to_string(),
        ScriptValue::String("NamedAttacker".to_string()),
    );
    params.insert(
        "area".to_string(),
        ScriptValue::String("NamedAttackZone".to_string()),
    );

    NamedAttackAreaAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::AttackArea,
            expected_center,
            Some(trigger_id),
            CommandSourceType::FromScript,
        )]
    );
    assert_eq!(
        get_object_manager()
            .read()
            .unwrap()
            .with_object(object_id, |o| o.base().read().unwrap().get_group_id())
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn named_hunt_selects_normal_locomotor_before_hunt() {
    use crate::modules::AIUpdateInterface;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingAi {
        commands: Arc<Mutex<Vec<(AiCommandType, CommandSourceType)>>>,
        locomotors: Arc<Mutex<Vec<LocomotorSetType>>>,
    }

    impl AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn choose_locomotor_set(
            &mut self,
            set: LocomotorSetType,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.locomotors.lock().unwrap().push(set);
            Ok(())
        }

        fn execute_command(
            &mut self,
            command: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.commands
                .lock()
                .unwrap()
                .push((command.cmd, command.cmd_source));
            Ok(())
        }
    }

    reset_test_object_manager();
    reset_test_named_object_tracker();

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let object_id = 8350;
    let object = crate::object_manager::GameObjectInstance::new(
        object_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test hunter instance");

    {
        let instance = &object;
        instance
            .base()
            .write()
            .unwrap()
            .set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                commands: Arc::clone(&commands),
                locomotors: Arc::clone(&locomotors),
            }))));
    }

    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(object, Coord3D::new(12.0, 6.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("NamedHunter".to_string(), object_id)
        .unwrap();

    let mut params = HashMap::new();
    params.insert(
        "unit_name".to_string(),
        ScriptValue::String("NamedHunter".to_string()),
    );

    NamedHuntAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(AiCommandType::Hunt, CommandSourceType::FromScript)]
    );
}

#[tokio::test]
async fn named_attack_named_leaves_group_and_dispatches_force_attack() {
    use crate::modules::AIUpdateInterface;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingAi {
        commands: Arc<Mutex<Vec<(AiCommandType, Option<u32>, i32, CommandSourceType)>>>,
        locomotors: Arc<Mutex<Vec<LocomotorSetType>>>,
    }

    impl AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn choose_locomotor_set(
            &mut self,
            set: LocomotorSetType,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.locomotors.lock().unwrap().push(set);
            Ok(())
        }

        fn execute_command(
            &mut self,
            command: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.commands.lock().unwrap().push((
                command.cmd,
                command.obj,
                command.int_value,
                command.cmd_source,
            ));
            Ok(())
        }
    }

    reset_test_object_manager();
    reset_test_named_object_tracker();

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let attacker_id = 8350;
    let target_id = 8351;
    let attacker = crate::object_manager::GameObjectInstance::new(
        attacker_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test attacker instance");
    let target = crate::object_manager::GameObjectInstance::new(
        target_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test target instance");

    {
        let __base_arc = attacker.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
        }))));
        base.enter_group(&crate::ai::AIGroup::new(87));
        assert_eq!(base.get_group_id(), Some(87));
    }

    let attacker_id = attacker.get_id();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(attacker, Coord3D::new(10.0, 4.0, 0.0))
        .unwrap();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(target, Coord3D::new(18.0, 4.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("NamedAttacker".to_string(), attacker_id)
        .unwrap();
    get_named_object_tracker()
        .register_named_object("NamedVictim".to_string(), target_id)
        .unwrap();

    let mut params = HashMap::new();
    params.insert(
        "attacker_name".to_string(),
        ScriptValue::String("NamedAttacker".to_string()),
    );
    params.insert(
        "target_name".to_string(),
        ScriptValue::String("NamedVictim".to_string()),
    );

    NamedAttackAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::ForceAttackObject,
            Some(target_id),
            -1,
            CommandSourceType::FromScript,
        )]
    );
    assert_eq!(
        get_object_manager()
            .read()
            .unwrap()
            .with_object(attacker_id, |o| o
                .base()
                .read()
                .ok()
                .and_then(|b| b.get_group_id()))
            .flatten(),
        None
    );
}

#[tokio::test]
async fn named_attack_team_leaves_group_and_dispatches_attack_team() {
    use crate::modules::AIUpdateInterface;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingAi {
        commands: Arc<Mutex<Vec<(AiCommandType, Option<String>, i32, CommandSourceType)>>>,
        locomotors: Arc<Mutex<Vec<LocomotorSetType>>>,
    }

    impl AIUpdateInterface for RecordingAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn choose_locomotor_set(
            &mut self,
            set: LocomotorSetType,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.locomotors.lock().unwrap().push(set);
            Ok(())
        }

        fn execute_command(
            &mut self,
            command: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.commands.lock().unwrap().push((
                command.cmd,
                command.team.clone(),
                command.int_value,
                command.cmd_source,
            ));
            Ok(())
        }
    }

    reset_test_object_manager();
    reset_test_named_object_tracker();
    reset_test_team_factory();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("TargetTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("TargetTeam")
            .expect("target team should be created");
    }

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let object_id = 8400;
    let object = crate::object_manager::GameObjectInstance::new(
        object_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test object instance");

    {
        let __base_arc = object.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
        }))));
        base.enter_group(&crate::ai::AIGroup::new(88));
        assert_eq!(base.get_group_id(), Some(88));
    }

    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(object, Coord3D::new(8.0, 4.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("NamedTeamAttacker".to_string(), object_id)
        .unwrap();

    let mut params = HashMap::new();
    params.insert(
        "unit_name".to_string(),
        ScriptValue::String("NamedTeamAttacker".to_string()),
    );
    params.insert(
        "team_name".to_string(),
        ScriptValue::String("TargetTeam".to_string()),
    );

    NamedAttackTeamAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::AttackTeam,
            Some("TargetTeam".to_string()),
            -1,
            CommandSourceType::FromScript,
        )]
    );
    assert_eq!(
        get_object_manager()
            .read()
            .unwrap()
            .with_object(object_id, |o| o.base().read().unwrap().get_group_id())
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn create_explosion_dispatches_fx_at_position() {
    use crate::common::types::FXListManagerInterface;
    use crate::helpers::register_fx_list_manager;
    use game_engine::common::name_key_generator::NameKeyGenerator;
    use glam::Mat4;
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingFxManager {
        calls: Arc<Mutex<Vec<(u32, Coord3D)>>>,
    }

    impl FXListManagerInterface for RecordingFxManager {
        fn do_fx_pos(&self, fx_list: u32, position: &Coord3D, _matrix: Option<&Mat4>) {
            self.calls.lock().unwrap().push((fx_list, *position));
        }

        fn do_fx_obj(&self, _fx_list: u32, _object_id: u32) {}
    }

    let calls = Arc::new(Mutex::new(Vec::new()));
    assert!(register_fx_list_manager(Arc::new(RecordingFxManager {
        calls: Arc::clone(&calls),
    })));

    let mut params = HashMap::new();
    params.insert(
        "explosion_type".to_string(),
        ScriptValue::String("TestExplosionFX".to_string()),
    );
    params.insert("x".to_string(), ScriptValue::Float(10.0));
    params.insert("y".to_string(), ScriptValue::Float(20.0));
    params.insert("z".to_string(), ScriptValue::Float(5.0));
    params.insert("damage".to_string(), ScriptValue::Float(25.0));

    CreateExplosionAction
        .execute(&params, &test_context())
        .await
        .unwrap();

    assert_eq!(
        *calls.lock().unwrap(),
        vec![(
            NameKeyGenerator::name_to_key("TestExplosionFX"),
            Coord3D::new(10.0, 20.0, 5.0),
        )]
    );
}

#[tokio::test]
async fn enable_disable_execute_script_actions_call_live_script_engine() {
    // C++ ScriptEngine::enableScript/disableScript/executeScript
    // ScriptEngine.cpp:6797-6823 / CALL_SUBROUTINE execute path.
    reset_test_script_engine();

    {
        let engine = get_script_engine();
        let mut guard = engine.write().unwrap();
        let engine = guard.as_mut().unwrap();

        let mut member = crate::scripting::core::Script::new();
        member.script_name = "Toggle Group Member".to_string();
        member.is_subroutine = true;
        member.is_one_shot = false;
        member.condition = Some({
            let mut or_condition = crate::scripting::core::OrCondition::new();
            or_condition.set_first_and_condition(Some(Box::new(
                crate::scripting::core::Condition::new(
                    crate::scripting::core::ConditionType::ConditionTrue,
                ),
            )));
            Box::new(or_condition)
        });
        member.action = Some({
            let mut action = crate::scripting::core::ScriptAction::new(
                crate::scripting::core::ScriptActionType::SetFlag,
            );
            action
                .add_parameter(crate::scripting::core::Parameter::with_string(
                    crate::scripting::core::ParameterType::Flag,
                    "registry_toggle_fired".to_string(),
                ))
                .unwrap();
            action
                .add_parameter(crate::scripting::core::Parameter::with_int(
                    crate::scripting::core::ParameterType::Boolean,
                    1,
                ))
                .unwrap();
            Box::new(action)
        });

        let mut list = crate::scripting::core::ScriptList::new();
        list.append_script(Box::new(member));
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .unwrap();
        assert!(engine.set_script_active_by_name("Toggle Group Member", false));
    }

    let mut params = HashMap::new();
    params.insert(
        "script_name".to_string(),
        ScriptValue::String("Toggle Group Member".to_string()),
    );

    EnableScriptAction
        .execute(&params, &test_context())
        .await
        .unwrap();
    {
        let engine = get_script_engine();
        let guard = engine.read().unwrap();
        let is_active = guard
            .as_ref()
            .unwrap()
            .find_script_clone_by_name("Toggle Group Member")
            .map(|script| script.is_active)
            .expect("script present");
        assert!(is_active, "enable_script must mutate live ScriptEngine");
    }

    ExecuteScriptAction
        .execute(&params, &test_context())
        .await
        .unwrap();
    {
        let engine = get_script_engine();
        let guard = engine.read().unwrap();
        assert!(
            guard
                .as_ref()
                .unwrap()
                .get_flag("registry_toggle_fired")
                .unwrap()
                .value,
            "execute_script must run the live ScriptEngine subroutine"
        );
    }

    DisableScriptAction
        .execute(&params, &test_context())
        .await
        .unwrap();
    {
        let engine = get_script_engine();
        let guard = engine.read().unwrap();
        let is_active = guard
            .as_ref()
            .unwrap()
            .find_script_clone_by_name("Toggle Group Member")
            .map(|script| script.is_active)
            .expect("script present");
        assert!(!is_active, "disable_script must mutate live ScriptEngine");
    }
}
