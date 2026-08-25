use super::super::engine::initialize_script_engine;
use super::*;

#[derive(Debug)]
struct CompletedWaypointAi {
    completed_waypoint_id: Arc<std::sync::atomic::AtomicU32>,
}

impl crate::modules::AIUpdateInterface for CompletedWaypointAi {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn is_moving(&self) -> bool {
        false
    }

    fn is_idle(&self) -> bool {
        true
    }

    fn set_movement_target(&mut self, _target: &crate::common::Coord3D) -> Result<(), String> {
        Ok(())
    }

    fn get_completed_waypoint_id(&self) -> Option<crate::waypoint::WaypointId> {
        Some(
            self.completed_waypoint_id
                .load(std::sync::atomic::Ordering::Relaxed),
        )
    }
}

struct TeamWaypointConditionFixture {
    member_id: u32,
    object: Option<Arc<RwLock<crate::object::Object>>>,
}

impl Drop for TeamWaypointConditionFixture {
    fn drop(&mut self) {
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.member_id);
        // Keep the last Object handle alive through unregister.  Its destructor
        // queries OBJECT_REGISTRY, so dropping it while unregister holds the
        // registry write lock would self-deadlock.
        drop(self.object.take());
        if let Ok(mut factory) = get_team_factory().lock() {
            factory.reset();
        }
        if let Ok(mut terrain) = crate::terrain::get_terrain_logic().write() {
            terrain.reset();
        }
    }
}

struct EnemySightedConditionFixture {
    object_ids: [u32; 3],
    // Retain the final handles until after registry removal.  Object::drop can
    // query the registry, so releasing its last Arc while that registry's write
    // lock is active would deadlock the test teardown.
    objects: Option<Vec<Arc<RwLock<crate::object::Object>>>>,
}

impl Drop for EnemySightedConditionFixture {
    fn drop(&mut self) {
        let _ = get_named_object_tracker().unregister_object(self.object_ids[0]);
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            for object_id in self.object_ids {
                logic.partition_manager_mut().remove_object(object_id);
            }
        }
        for object_id in self.object_ids {
            crate::object::registry::OBJECT_REGISTRY.unregister_object(object_id);
        }
        drop(self.objects.take());
        if let Ok(mut players) = crate::player::player_list().write() {
            players.clear();
        }
    }
}

struct PlayerScienceTokenConditionFixture {
    sentinel_id: u32,
    object: Option<Arc<RwLock<crate::object::Object>>>,
}

impl Drop for PlayerScienceTokenConditionFixture {
    fn drop(&mut self) {
        crate::object::registry::OBJECT_REGISTRY.unregister_object(self.sentinel_id);
        // Object::drop may query the registry, so release the last Arc only
        // after unregister has released its write guard.
        drop(self.object.take());
        if let Ok(mut players) = crate::player::player_list().write() {
            players.clear();
        }
    }
}

struct EvaluatorStateHandler {
    camera_finished: bool,
    music_finished: bool,
}

impl ScriptActionHandler for EvaluatorStateHandler {
    fn is_camera_movement_finished(&self) -> bool {
        self.camera_finished
    }

    fn has_music_track_completed(&self, _track: &str, _param: i32) -> bool {
        self.music_finished
    }
}

struct ScriptToggleRecordingHandler {
    updates: Arc<std::sync::Mutex<Vec<(String, bool)>>>,
}

impl ScriptActionHandler for ScriptToggleRecordingHandler {
    fn enable_script(&self, name: &str, enabled: bool) -> crate::GameLogicResult<()> {
        self.updates
            .lock()
            .expect("script toggle update mutex should not be poisoned")
            .push((name.to_string(), enabled));
        Ok(())
    }
}

#[tokio::test]
async fn test_script_evaluator_creation() {
    initialize_script_engine().unwrap();
    let engine = get_script_engine();
    let evaluator = ScriptEvaluator::new(engine);

    // Create a simple script to test
    let mut script = Script::new();
    script.set_name("test_script".to_string());

    // Should evaluate to true with no conditions
    let result = evaluator.evaluate_script(&mut script).unwrap();
    assert!(result);
}

#[tokio::test]
async fn test_counter_condition() {
    initialize_script_engine().unwrap();
    let engine = get_script_engine();
    let evaluator = ScriptEvaluator::new(engine.clone());

    // Set up a counter
    {
        let mut engine_guard = engine.write().unwrap();
        let engine = engine_guard.as_mut().unwrap();
        engine.set_counter("test_counter", 50).unwrap();
    }

    // Create counter condition: counter >= 40
    let mut condition = Condition::new(ConditionType::Counter);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "test_counter".to_string(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
        .unwrap(); // GreaterEqual
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 40))
        .unwrap();

    let result = evaluator.evaluate_condition(&mut condition).unwrap();
    assert!(result); // 50 >= 40 should be true
}

#[test]
fn condition_true_evaluates_when_object_registry_empty() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    assert!(crate::object::registry::OBJECT_REGISTRY.is_empty());
    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut condition = Condition::new(ConditionType::ConditionTrue);
    assert!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ EvaluateCondition(TRUE) does not depend on OBJECT_REGISTRY"
    );
}

#[test]
fn named_destroyed_uses_host_snapshot_when_registry_empty() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("Hero".into(), 7)].into_iter().collect(),
        objects: vec![crate::scripting::HostScriptQueryObject {
            id: 7,
            name: "Hero".into(),
            team: 1,
            x: 0.0,
            z: 0.0,
            alive: false,
            ..Default::default()
        }],
        ..Default::default()
    });
    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut condition = Condition::new(ConditionType::NamedDestroyed);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    assert!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        "dead named host unit must satisfy NamedDestroyed"
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn named_destroyed_false_when_name_never_existed_like_cxx() {
    // C++ ScriptConditions::evaluateNamedUnitDestroyed (ScriptConditions.cpp:285)
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("OtherHero".into(), 8)].into_iter().collect(),
        objects: vec![crate::scripting::HostScriptQueryObject {
            id: 8,
            name: "OtherHero".into(),
            team: 1,
            x: 0.0,
            z: 0.0,
            alive: true,
            ..Default::default()
        }],
        ..Default::default()
    });
    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut condition = Condition::new(ConditionType::NamedDestroyed);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "TypoHero".into(),
        ))
        .unwrap();
    assert!(
        !evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ evaluateNamedUnitDestroyed: never existed → FALSE"
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn evaluator_uses_active_engine_before_its_private_handle() {
    let _test_lock = crate::test_sync::lock();
    const SENTINEL_ID: u32 = 0xE7A1_3001;
    const COUNTER_NAME: &str = "EvaluatorActiveEnginePrecedence";

    let sentinel = Arc::new(RwLock::new(crate::object::Object::new_test(
        SENTINEL_ID,
        1.0,
    )));
    crate::object::registry::OBJECT_REGISTRY.register_object(SENTINEL_ID, &sentinel);

    let mut private_engine = ScriptEngine::new().expect("private script engine");
    private_engine
        .set_counter(COUNTER_NAME, 11)
        .expect("private counter");
    private_engine.set_action_handler(Some(Arc::new(EvaluatorStateHandler {
        camera_finished: true,
        music_finished: true,
    })));
    let evaluator = ScriptEvaluator::new(ScriptEngineHandle::from_engine(private_engine));

    let mut private_camera = Condition::new(ConditionType::CameraMovementFinished);
    assert!(
        evaluator.evaluate_condition(&mut private_camera).unwrap(),
        "without a live update scope, an evaluator must use its injected engine handle"
    );
    let mut private_music = Condition::new(ConditionType::MusicTrackHasCompleted);
    private_music
        .add_parameter(Parameter::with_string(
            ParameterType::Music,
            "PrivateEngineTrack".to_string(),
        ))
        .unwrap();
    assert!(
        evaluator.evaluate_condition(&mut private_music).unwrap(),
        "private music state must not be read from the unrelated global engine"
    );

    initialize_script_engine().expect("global script engine");
    let previous_handler = {
        let global = get_script_engine();
        let mut global = global.write().expect("global engine lock");
        let engine = global.as_mut().expect("global script engine");
        engine.set_counter(COUNTER_NAME, 3).expect("global counter");
        let previous = engine.action_handler();
        engine.set_action_handler(Some(Arc::new(EvaluatorStateHandler {
            camera_finished: false,
            music_finished: false,
        })));
        previous
    };

    let active_result = with_script_engine_mut(|_| {
        let mut active_camera = Condition::new(ConditionType::CameraMovementFinished);
        let mut active_music = Condition::new(ConditionType::MusicTrackHasCompleted);
        active_music
            .add_parameter(Parameter::with_string(
                ParameterType::Music,
                "GlobalEngineTrack".to_string(),
            ))
            .unwrap();
        let mut active_counter = Condition::new(ConditionType::Counter);
        active_counter
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                COUNTER_NAME.to_string(),
            ))
            .unwrap();
        active_counter
            .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
            .unwrap();
        active_counter
            .add_parameter(Parameter::with_int(ParameterType::Int, 3))
            .unwrap();

        (
            evaluator.evaluate_condition(&mut active_camera),
            evaluator.evaluate_condition(&mut active_music),
            evaluator.evaluate_condition(&mut active_counter),
        )
    });

    {
        let global = get_script_engine();
        let mut global = global.write().expect("global engine lock");
        global
            .as_mut()
            .expect("global script engine")
            .set_action_handler(previous_handler);
    }
    crate::object::registry::OBJECT_REGISTRY.unregister_object(SENTINEL_ID);
    drop(sentinel);

    let Some((camera, music, counter)) = active_result else {
        panic!("global live engine should install an active evaluation scope");
    };
    assert!(!camera.unwrap());
    assert!(!music.unwrap());
    assert!(counter.unwrap());
}

#[tokio::test]
async fn test_flag_condition() {
    initialize_script_engine().unwrap();
    let engine = get_script_engine();
    let evaluator = ScriptEvaluator::new(engine.clone());

    // Set up a flag
    {
        let mut engine_guard = engine.write().unwrap();
        let engine = engine_guard.as_mut().unwrap();
        engine.set_flag("test_flag", true).unwrap();
    }

    // Create flag condition: flag == true
    let mut condition = Condition::new(ConditionType::Flag);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            "test_flag".to_string(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap(); // true

    let result = evaluator.evaluate_condition(&mut condition).unwrap();
    assert!(result);
}

#[test]
fn player_science_points_resolves_local_player_token_like_cpp() {
    let _test_lock = crate::test_sync::lock();
    const SENTINEL_ID: u32 = 0xE7A1_2001;
    const PLAYER_NAME: &str = "EvaluatorScienceLocalPlayer";

    let mut fixture = PlayerScienceTokenConditionFixture {
        sentinel_id: SENTINEL_ID,
        object: None,
    };
    crate::player::player_list().write().unwrap().clear();

    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    {
        let mut player_guard = player.write().unwrap();
        player_guard.set_display_name(PLAYER_NAME);
        player_guard.add_science_purchase_points(3);
    }
    {
        let mut players = crate::player::player_list().write().unwrap();
        players.add_player(player);
        players.set_local_player_index(0);
    }

    // evaluate_condition intentionally fails closed without a live object
    // registry.  A retained sentinel makes this a real evaluator path.
    let sentinel = Arc::new(RwLock::new(crate::object::Object::new_test(
        SENTINEL_ID,
        1.0,
    )));
    crate::object::registry::OBJECT_REGISTRY.register_object(SENTINEL_ID, &sentinel);
    fixture.object = Some(sentinel);

    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut condition = Condition::new(ConditionType::PlayerHasSciencepurchasepoints);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            crate::scripting::core::LOCAL_PLAYER.to_string(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();

    assert!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ ScriptConditions::playerFromParam resolves <Local Player> before evaluating science points"
    );
}

#[test]
fn player_all_destroyed_resolves_local_player_token_like_cpp() {
    let _test_lock = crate::test_sync::lock();
    const UNIT_ID: u32 = 0xE7A1_2002;

    let mut fixture = PlayerScienceTokenConditionFixture {
        sentinel_id: UNIT_ID,
        object: None,
    };
    crate::player::player_list().write().unwrap().clear();

    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    player
        .write()
        .unwrap()
        .set_display_name("EvaluatorEliminationLocalPlayer");
    {
        let mut players = crate::player::player_list().write().unwrap();
        players.add_player(Arc::clone(&player));
        players.set_local_player_index(0);
    }

    let unit = Arc::new(RwLock::new(crate::object::Object::new_test(UNIT_ID, 100.0)));
    crate::object::registry::OBJECT_REGISTRY.register_object(UNIT_ID, &unit);
    fixture.object = Some(unit);
    player.write().unwrap().add_owned_object(UNIT_ID);

    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut condition = Condition::new(ConditionType::PlayerAllDestroyed);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            crate::scripting::core::LOCAL_PLAYER.to_string(),
        ))
        .unwrap();

    assert!(
        !evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ evaluateAllDestroyed must inspect the local player's live unit, not treat <Local Player> as missing"
    );
}

#[test]
fn player_credits_resolves_side_tokens_and_missing_players_fail_closed_like_cpp() {
    let _test_lock = crate::test_sync::lock();
    const SENTINEL_ID: u32 = 0xE7A1_2003;

    let mut fixture = PlayerScienceTokenConditionFixture {
        sentinel_id: SENTINEL_ID,
        object: None,
    };
    crate::player::player_list().write().unwrap().clear();

    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    {
        let mut player_guard = player.write().unwrap();
        player_guard.set_display_name("EvaluatorCreditsLocalPlayer");
        player_guard.get_money_mut().set_money(100);
    }
    {
        let mut players = crate::player::player_list().write().unwrap();
        players.add_player(player);
        players.set_local_player_index(0);
    }

    let sentinel = Arc::new(RwLock::new(crate::object::Object::new_test(
        SENTINEL_ID,
        1.0,
    )));
    crate::object::registry::OBJECT_REGISTRY.register_object(SENTINEL_ID, &sentinel);
    fixture.object = Some(sentinel);

    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut local_player_condition = Condition::new(ConditionType::PlayerHasCredits);
    local_player_condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 50))
        .unwrap();
    local_player_condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 0))
        .unwrap(); // C++ Parameter::LESS_THAN
    local_player_condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            crate::scripting::core::LOCAL_PLAYER.to_string(),
        ))
        .unwrap();
    assert!(
        evaluator
            .evaluate_condition(&mut local_player_condition)
            .unwrap(),
        "C++ PlayerHasCredits resolves <Local Player> through playerFromParam"
    );

    let mut missing_player_condition = Condition::new(ConditionType::PlayerHasCredits);
    missing_player_condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    missing_player_condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap(); // C++ Parameter::EQUAL
    missing_player_condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "NoSuchEvaluatorPlayer".to_string(),
        ))
        .unwrap();
    assert!(
        !evaluator
            .evaluate_condition(&mut missing_player_condition)
            .unwrap(),
        "C++ returns false for an unresolved Side instead of comparing against invented zero credits"
    );
}

#[tokio::test]
async fn player_has_credits_compares_threshold_to_player_money_like_cxx() {
    initialize_script_engine().unwrap();
    player_list().write().unwrap().clear();
    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    {
        let mut player_guard = player.write().unwrap();
        player_guard.set_display_name("CreditsEvaluatorPlayer");
        player_guard.get_money_mut().set_money(1000);
    }
    player_list().write().unwrap().add_player(player);

    let engine = get_script_engine();
    let evaluator = ScriptEvaluator::new(engine);

    let mut condition = Condition::new(ConditionType::PlayerHasCredits);
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 500))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 0))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "CreditsEvaluatorPlayer".to_string(),
        ))
        .unwrap();

    assert!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ evaluates threshold < player's credits, not player's credits < threshold"
    );
}

#[test]
fn player_money_actions_mutate_player_money_like_cxx() {
    initialize_script_engine().unwrap();
    player_list().write().unwrap().clear();
    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    {
        let mut player_guard = player.write().unwrap();
        player_guard.set_display_name("MoneyActionEvaluatorPlayer");
        player_guard.get_money_mut().set_money(500);
    }
    player_list().write().unwrap().add_player(player.clone());

    let engine = get_script_engine();
    let evaluator = ScriptEvaluator::new(engine);

    let mut set_action = ScriptAction::new(ScriptActionType::PlayerSetMoney);
    set_action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "MoneyActionEvaluatorPlayer".to_string(),
        ))
        .unwrap();
    set_action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1200))
        .unwrap();
    evaluator.execute_action(&set_action).unwrap();
    assert_eq!(player.read().unwrap().get_money().get_money(), 1200);

    let mut give_action = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    give_action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "MoneyActionEvaluatorPlayer".to_string(),
        ))
        .unwrap();
    give_action
        .add_parameter(Parameter::with_int(ParameterType::Int, -1500))
        .unwrap();
    evaluator.execute_action(&give_action).unwrap();
    assert_eq!(
        player.read().unwrap().get_money().get_money(),
        0,
        "C++ withdraws up to available money for negative give-money actions"
    );
}

#[test]
fn unit_type_area_condition_counts_dead_or_inert_crates_like_cxx() {
    assert!(
        ScriptEvaluator::counts_for_unit_type_area_condition(true, false, true),
        "C++ includes crates even when they are effectively dead"
    );
    assert!(
        ScriptEvaluator::counts_for_unit_type_area_condition(false, true, true),
        "C++ includes crates even when they are inert"
    );
    assert!(!ScriptEvaluator::counts_for_unit_type_area_condition(
        true, false, false
    ));
    assert!(!ScriptEvaluator::counts_for_unit_type_area_condition(
        false, true, false
    ));
}

#[test]
fn team_reached_waypoints_end_requires_the_requested_path_like_cpp() {
    let _test_lock = crate::test_sync::lock();
    const MEMBER_ID: u32 = 0xE7A1_0001;
    const OTHER_PATH_WAYPOINT_ID: u32 = 0xE7A1_0002;
    const REQUESTED_PATH_WAYPOINT_ID: u32 = 0xE7A1_0003;
    const TEAM_NAME: &str = "EvaluatorWaypointPathTeam";
    const REQUESTED_PATH: &str = "RequestedCampaignPath";

    let mut fixture = TeamWaypointConditionFixture {
        member_id: MEMBER_ID,
        object: None,
    };
    get_team_factory().lock().unwrap().reset();
    crate::terrain::get_terrain_logic().write().unwrap().reset();

    let mut map_data = crate::system::map_loader::MapData::new();
    map_data.width = 2;
    map_data.height = 2;
    map_data.heightmap = vec![0; 4];
    map_data.waypoints = vec![
        crate::system::map_loader::MapWaypoint {
            id: OTHER_PATH_WAYPOINT_ID,
            name: "OtherPathEnd".to_string(),
            location: crate::system::map_loader::Coord3D::origin(),
            path_label1: "DifferentCampaignPath".to_string(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        },
        crate::system::map_loader::MapWaypoint {
            id: REQUESTED_PATH_WAYPOINT_ID,
            name: "RequestedPathEnd".to_string(),
            location: crate::system::map_loader::Coord3D::new(10.0, 0.0, 0.0),
            path_label1: "AnotherPath".to_string(),
            path_label2: REQUESTED_PATH.to_string(),
            path_label3: String::new(),
            bi_directional: false,
        },
    ];
    crate::terrain::get_terrain_logic()
        .write()
        .unwrap()
        .load_map_data(map_data);

    let completed_waypoint_id = Arc::new(std::sync::atomic::AtomicU32::new(OTHER_PATH_WAYPOINT_ID));
    let object = Arc::new(RwLock::new(crate::object::Object::new_test(
        MEMBER_ID, 100.0,
    )));
    fixture.object = Some(Arc::clone(&object));
    let ai: Arc<std::sync::Mutex<dyn crate::modules::AIUpdateInterface>> =
        Arc::new(std::sync::Mutex::new(CompletedWaypointAi {
            completed_waypoint_id: Arc::clone(&completed_waypoint_id),
        }));
    object.write().unwrap().set_ai_update_interface(Some(ai));
    crate::object::registry::OBJECT_REGISTRY.register_object(MEMBER_ID, &object);
    get_named_object_tracker()
        .register_named_object("EvaluatorWaypointNamedUnit".to_string(), MEMBER_ID)
        .unwrap();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from(TEAM_NAME),
            AsciiString::default(),
            false,
            None,
        );
        let team = factory.create_team(TEAM_NAME).unwrap();
        team.write().unwrap().add_member(MEMBER_ID);
    }

    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut condition = Condition::new(ConditionType::TeamReachedWaypointsEnd);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            TEAM_NAME.to_string(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            REQUESTED_PATH.to_string(),
        ))
        .unwrap();

    assert!(
        !evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ does not treat completion of a different waypoint path as success"
    );

    completed_waypoint_id.store(
        REQUESTED_PATH_WAYPOINT_ID,
        std::sync::atomic::Ordering::Relaxed,
    );
    let mut named_condition = Condition::new(ConditionType::NamedReachedWaypointsEnd);
    named_condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "EvaluatorWaypointNamedUnit".to_string(),
        ))
        .unwrap();
    named_condition
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "requestedcampaignpath".to_string(),
        ))
        .unwrap();
    assert!(
        !evaluator.evaluate_condition(&mut named_condition).unwrap(),
        "C++ compares waypoint path labels with case-sensitive AsciiString equality"
    );
    named_condition.parameters[1] = Some(Parameter::with_string(
        ParameterType::WaypointPath,
        REQUESTED_PATH.to_string(),
    ));
    assert!(
        evaluator.evaluate_condition(&mut named_condition).unwrap(),
        "the exact waypoint path label satisfies C++ NamedReachedWaypointsEnd"
    );
    assert!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        "a completed waypoint matching any C++ path-label slot must satisfy the condition"
    );
}

#[test]
fn enemy_and_type_sighted_honor_cxx_relation_and_stealth_filters() {
    let _test_lock = crate::test_sync::lock();
    const SOURCE_ID: u32 = 0xE7A1_1001;
    const CANDIDATE_ID: u32 = 0xE7A1_1002;
    const REGISTRY_SENTINEL_ID: u32 = 0xE7A1_1003;
    const SOURCE_TEAM_ID: u32 = 0xE7A1_1101;
    const CANDIDATE_TEAM_ID: u32 = 0xE7A1_1102;
    const SOURCE_NAME: &str = "EvaluatorEnemySightSource";
    const TARGET_PLAYER_NAME: &str = "EvaluatorEnemySightTarget";

    let mut fixture = EnemySightedConditionFixture {
        object_ids: [SOURCE_ID, CANDIDATE_ID, REGISTRY_SENTINEL_ID],
        objects: Some(Vec::new()),
    };
    crate::player::player_list().write().unwrap().clear();

    let source_player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    source_player
        .write()
        .unwrap()
        .set_display_name("EvaluatorEnemySightSourcePlayer");
    let target_player = Arc::new(RwLock::new(crate::player::Player::new(1)));
    target_player
        .write()
        .unwrap()
        .set_display_name(TARGET_PLAYER_NAME);
    {
        let mut players = crate::player::player_list().write().unwrap();
        players.add_player(source_player);
        players.add_player(target_player);
    }

    // The team controller setter refreshes every current member through the
    // registry.  Keep a harmless object registered while the teams are still
    // empty, then attach their members after controller setup.
    let registry_sentinel = Arc::new(RwLock::new(crate::object::Object::new_test(
        REGISTRY_SENTINEL_ID,
        1.0,
    )));
    fixture
        .objects
        .as_mut()
        .unwrap()
        .push(Arc::clone(&registry_sentinel));
    crate::object::registry::OBJECT_REGISTRY
        .register_object(REGISTRY_SENTINEL_ID, &registry_sentinel);

    let source = Arc::new(RwLock::new(crate::object::Object::new_test(
        SOURCE_ID, 100.0,
    )));
    let candidate = Arc::new(RwLock::new(crate::object::Object::new_test(
        CANDIDATE_ID,
        100.0,
    )));
    fixture
        .objects
        .as_mut()
        .unwrap()
        .extend([Arc::clone(&source), Arc::clone(&candidate)]);
    {
        let mut source_guard = source.write().unwrap();
        source_guard
            .set_position(&crate::common::Coord3D::new(0.0, 0.0, 0.0))
            .unwrap();
        source_guard.set_vision_range(100.0);
    }
    candidate
        .write()
        .unwrap()
        .set_position(&crate::common::Coord3D::new(25.0, 0.0, 0.0))
        .unwrap();

    let source_team = Arc::new(RwLock::new(crate::team::Team::new(
        AsciiString::from("EvaluatorEnemySightSourceTeam"),
        SOURCE_TEAM_ID,
    )));
    let candidate_team = Arc::new(RwLock::new(crate::team::Team::new(
        AsciiString::from("EvaluatorEnemySightCandidateTeam"),
        CANDIDATE_TEAM_ID,
    )));
    // Controllers are assigned before either object becomes a team member.  This keeps
    // Team::set_controlling_player_id from calling partition maintenance while its own
    // team write lock is held.
    source_team
        .write()
        .unwrap()
        .set_controlling_player_id(Some(0));
    candidate_team
        .write()
        .unwrap()
        .set_controlling_player_id(Some(1));
    // Attach the objects before registry registration: Object::set_team updates its
    // owner's object list by looking itself up in the registry, so registering first
    // would self-lock through that lookup while the object write guard is live.
    source
        .write()
        .unwrap()
        .set_team(Some(Arc::clone(&source_team)))
        .unwrap();
    candidate
        .write()
        .unwrap()
        .set_team(Some(Arc::clone(&candidate_team)))
        .unwrap();
    crate::object::registry::OBJECT_REGISTRY.register_object(SOURCE_ID, &source);
    crate::object::registry::OBJECT_REGISTRY.register_object(CANDIDATE_ID, &candidate);
    source_team
        .write()
        .unwrap()
        .set_override_team_relationship(CANDIDATE_TEAM_ID, crate::common::Relationship::Neutral);

    get_named_object_tracker()
        .register_named_object(SOURCE_NAME.to_string(), SOURCE_ID)
        .unwrap();
    {
        let mut logic = crate::system::game_logic::get_game_logic().lock().unwrap();
        logic
            .partition_manager_mut()
            .add_object(SOURCE_ID, (0.0, 0.0, 0.0));
        logic
            .partition_manager_mut()
            .add_object(CANDIDATE_ID, (25.0, 0.0, 0.0));
    }

    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut condition = Condition::new(ConditionType::EnemySighted);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            SOURCE_NAME.to_string(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Relation, 0))
        .unwrap(); // C++ Parameter::REL_ENEMY
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            TARGET_PLAYER_NAME.to_string(),
        ))
        .unwrap();

    assert!(
        !evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ filters EnemySighted candidates by the requested relationship"
    );

    source_team
        .write()
        .unwrap()
        .set_override_team_relationship(CANDIDATE_TEAM_ID, crate::common::Relationship::Enemies);
    assert!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        "an in-range, visible enemy owned by the requested player satisfies C++ EnemySighted"
    );

    candidate
        .write()
        .unwrap()
        .set_status(crate::common::ObjectStatusMaskType::STEALTHED, true);
    assert!(
        !evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ rejects stealthed candidates until they are detected or disguised"
    );

    candidate
        .write()
        .unwrap()
        .set_status(crate::common::ObjectStatusMaskType::DETECTED, true);
    assert!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        "C++ admits a detected stealthed candidate"
    );

    let mut type_condition = Condition::new(ConditionType::TypeSighted);
    type_condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            SOURCE_NAME.to_string(),
        ))
        .unwrap();
    type_condition
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "TestObject".to_string(),
        ))
        .unwrap();
    type_condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            TARGET_PLAYER_NAME.to_string(),
        ))
        .unwrap();
    assert!(
        evaluator.evaluate_condition(&mut type_condition).unwrap(),
        "the visible candidate's exact template type satisfies C++ TypeSighted"
    );

    candidate
        .write()
        .unwrap()
        .set_status(crate::common::ObjectStatusMaskType::DETECTED, false);
    assert!(
        !evaluator.evaluate_condition(&mut type_condition).unwrap(),
        "C++ TypeSighted also rejects an undetected stealthed candidate"
    );
}

#[test]
fn enable_disable_actions_toggle_private_subroutine_group_before_following_call() {
    // C++ ScriptEngine::enableScript/disableScript mutates group state
    // before the next action in the same chain.  MissionScriptRuntime uses
    // a private evaluator engine, so this must not fall through to the
    // unrelated global engine or merely defer the state change to a hook.
    const GROUP_NAME: &str = "Deferred Until Explicitly Enabled";
    const FLAG_NAME: &str = "private_group_call_completed";

    let updates = Arc::new(std::sync::Mutex::new(Vec::new()));
    let mut member = Script::new();
    member.set_name("Private Group Member".to_string());
    let mut always_true = OrCondition::new();
    always_true
        .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
    member.set_or_condition(Some(Box::new(always_true)));
    let mut set_flag = ScriptAction::new(ScriptActionType::SetFlag);
    set_flag
        .add_parameter(Parameter::with_string(
            ParameterType::Flag,
            FLAG_NAME.to_string(),
        ))
        .expect("flag parameter should fit");
    set_flag
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .expect("boolean parameter should fit");
    member.set_action(Some(Box::new(set_flag)));

    let mut group = ScriptGroup::new();
    group.set_name(GROUP_NAME.to_string());
    group.set_active(false);
    group.set_subroutine(true);
    group.append_script(Box::new(member));

    let mut list = ScriptList::new();
    list.append_group(Box::new(group));

    let mut private_engine = ScriptEngine::new().expect("private script engine should initialize");
    private_engine.set_action_handler(Some(Arc::new(ScriptToggleRecordingHandler {
        updates: Arc::clone(&updates),
    })));
    private_engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .expect("private script list should install");
    let private_engine = ScriptEngineHandle::from_engine(private_engine);
    let evaluator = ScriptEvaluator::new(private_engine.clone());

    let mut enable = ScriptAction::new(ScriptActionType::EnableScript);
    enable
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            GROUP_NAME.to_string(),
        ))
        .expect("enable parameter should fit");
    let mut call_after_enable = ScriptAction::new(ScriptActionType::CallSubroutine);
    call_after_enable
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            GROUP_NAME.to_string(),
        ))
        .expect("call parameter should fit");
    enable.set_next_action(Some(Box::new(call_after_enable)));
    evaluator
        .execute_action_sequence(&enable)
        .expect("enable then call should execute on the private engine");

    {
        let engine = private_engine.read().expect("private engine lock");
        let engine = engine
            .as_ref()
            .expect("private engine should remain installed");
        assert!(
            engine
                .get_flag(FLAG_NAME)
                .expect("enabled group member should set its flag")
                .value,
            "ENABLE_SCRIPT must make an inactive subroutine group callable immediately"
        );
    }

    {
        let engine = private_engine.write().expect("private engine lock");
        engine
            .as_ref()
            .expect("private engine should remain installed")
            .set_flag(FLAG_NAME, false)
            .expect("flag reset should succeed");
    }

    let mut disable = ScriptAction::new(ScriptActionType::DisableScript);
    disable
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            GROUP_NAME.to_string(),
        ))
        .expect("disable parameter should fit");
    let mut call_after_disable = ScriptAction::new(ScriptActionType::CallSubroutine);
    call_after_disable
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            GROUP_NAME.to_string(),
        ))
        .expect("call parameter should fit");
    disable.set_next_action(Some(Box::new(call_after_disable)));
    evaluator
        .execute_action_sequence(&disable)
        .expect("disable then call should remain on the private engine");

    let engine = private_engine.read().expect("private engine lock");
    let engine = engine
        .as_ref()
        .expect("private engine should remain installed");
    assert!(
        !engine
            .get_flag(FLAG_NAME)
            .expect("flag should remain allocated")
            .value,
        "DISABLE_SCRIPT must make a subroutine group uncallable before the next action"
    );
    assert_eq!(
        updates
            .lock()
            .expect("script toggle update mutex should not be poisoned")
            .as_slice(),
        [
            (GROUP_NAME.to_string(), true),
            (GROUP_NAME.to_string(), false)
        ],
        "the private engine must forward each toggle to MissionScriptRuntime exactly once"
    );
}

#[test]
fn test_victory_action() {
    // Live MissionScriptRuntime uses ScriptEvaluator, not ScriptActionDispatcher
    // directly. C++ ScriptActions.cpp:191-210 doVictory must run: disable
    // input, Victorious.wnd, SetVictorious, startEndGameTimer.
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().unwrap();
    let prior_input = crate::helpers::TheGameLogic::is_input_enabled();
    crate::helpers::TheGameLogic::set_input_enabled(true);

    let engine = get_script_engine();
    {
        let guard = engine.write().unwrap();
        let engine = guard.as_ref().unwrap();
        engine.set_shown_mp_local_defeat_window(false);
        engine.close_windows(false);
        engine.set_campaign_victorious(false);
    }

    let evaluator = ScriptEvaluator::new(engine.clone());
    let action = ScriptAction::new(ScriptActionType::Victory);
    evaluator.execute_action(&action).unwrap();

    let engine_guard = engine.read().unwrap();
    let engine = engine_guard.as_ref().unwrap();
    assert!(engine.is_game_ending());
    assert!(
        engine.is_campaign_victorious(),
        "C++ ScriptActions.cpp:208 TheCampaignManager->SetVictorious(TRUE)"
    );
    assert_eq!(
        engine.current_win_lose_window().as_deref(),
        Some("Menus/Victorious.wnd"),
        "live evaluator must create Victorious.wnd via do_victory"
    );
    assert!(
        !crate::helpers::TheGameLogic::is_input_enabled(),
        "C++ doVictory calls doDisableInput"
    );
    crate::helpers::TheGameLogic::set_input_enabled(prior_input);
}

fn wave14_triangle_area() -> crate::polygon_trigger::PolygonTrigger {
    crate::polygon_trigger::PolygonTrigger::new(
        1411,
        crate::common::AsciiString::from("Wave14PolyPad"),
        vec![
            crate::common::ICoord3D::new(0, 0, 0),
            crate::common::ICoord3D::new(20, 0, 0),
            crate::common::ICoord3D::new(0, 20, 0),
        ],
    )
}

#[test]
fn live_named_inside_uses_point_in_trigger_not_aabb() {
    // Triangle (0,0)-(20,0)-(0,20): (18,18) is inside the AABB but outside C++ pointInTrigger.
    let _lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    crate::terrain::get_terrain_logic()
        .write()
        .expect("terrain")
        .add_trigger_area(wave14_triangle_area());

    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("Scout".into(), 7)].into_iter().collect(),
        objects: vec![crate::scripting::HostScriptQueryObject {
            id: 7,
            name: "Scout".into(),
            team: 1,
            x: 18.0,
            z: 18.0,
            alive: true,
            ..Default::default()
        }],
        ..Default::default()
    });

    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut inside = Condition::new(ConditionType::NamedInsideArea);
    inside
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Scout".into()))
        .unwrap();
    inside
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "Wave14PolyPad".into(),
        ))
        .unwrap();
    assert!(
        !evaluator.evaluate_condition(&mut inside).unwrap(),
        "AABB would include (18,18); C++ pointInTrigger must not"
    );

    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("Scout".into(), 7)].into_iter().collect(),
        objects: vec![crate::scripting::HostScriptQueryObject {
            id: 7,
            name: "Scout".into(),
            team: 1,
            x: 2.0,
            z: 2.0,
            alive: true,
            ..Default::default()
        }],
        ..Default::default()
    });
    assert!(
        evaluator.evaluate_condition(&mut inside).unwrap(),
        "host unit inside leftover polygon must match NAMED_INSIDE"
    );
    let mut outside = Condition::new(ConditionType::NamedOutsideArea);
    outside
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Scout".into()))
        .unwrap();
    outside
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "Wave14PolyPad".into(),
        ))
        .unwrap();
    assert!(
        !evaluator.evaluate_condition(&mut outside).unwrap(),
        "NAMED_OUTSIDE must not fail-open while the unit is inside"
    );
}

#[test]
fn live_named_entered_exited_use_two_frame_host_flags() {
    let _lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    crate::terrain::get_terrain_logic()
        .write()
        .expect("terrain")
        .add_trigger_area(wave14_triangle_area());
    crate::system::game_logic::get_game_logic()
        .lock()
        .expect("logic")
        .set_current_frame(20);

    crate::scripting::update_host_object_trigger_flags(7, 18.0, 18.0, 19, false, Some("teamUSA"));
    crate::scripting::update_host_object_trigger_flags(7, 2.0, 2.0, 20, false, Some("teamUSA"));
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("Scout".into(), 7)].into_iter().collect(),
        objects: vec![crate::scripting::HostScriptQueryObject {
            id: 7,
            name: "Scout".into(),
            team: 1,
            x: 2.0,
            z: 2.0,
            alive: true,
            ..Default::default()
        }],
        team_instance_ids: [("teamUSA".into(), vec![7])].into_iter().collect(),
        ..Default::default()
    });

    let evaluator = ScriptEvaluator::new(get_script_engine());
    let mut entered = Condition::new(ConditionType::NamedEnteredArea);
    entered
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Scout".into()))
        .unwrap();
    entered
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "Wave14PolyPad".into(),
        ))
        .unwrap();
    assert!(
        evaluator.evaluate_condition(&mut entered).unwrap(),
        "C++ Object::didEnter must fire on the live empty-registry path"
    );

    crate::system::game_logic::get_game_logic()
        .lock()
        .expect("logic")
        .set_current_frame(21);
    crate::scripting::update_host_object_trigger_flags(7, 18.0, 18.0, 21, false, Some("teamUSA"));
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("Scout".into(), 7)].into_iter().collect(),
        objects: vec![crate::scripting::HostScriptQueryObject {
            id: 7,
            name: "Scout".into(),
            team: 1,
            x: 18.0,
            z: 18.0,
            alive: true,
            ..Default::default()
        }],
        team_instance_ids: [("teamUSA".into(), vec![7])].into_iter().collect(),
        ..Default::default()
    });
    let mut exited = Condition::new(ConditionType::NamedExitedArea);
    exited
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Scout".into()))
        .unwrap();
    exited
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "Wave14PolyPad".into(),
        ))
        .unwrap();
    assert!(
        evaluator.evaluate_condition(&mut exited).unwrap(),
        "C++ Object::didExit must fire on the live empty-registry path"
    );

    let mut team_inside = Condition::new(ConditionType::TeamInsideAreaEntirely);
    team_inside
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamUSA".into(),
        ))
        .unwrap();
    team_inside
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "Wave14PolyPad".into(),
        ))
        .unwrap();
    team_inside
        .add_parameter(Parameter::with_int(ParameterType::SurfacesAllowed, 1))
        .unwrap();
    assert!(
        !evaluator.evaluate_condition(&mut team_inside).unwrap(),
        "team standing outside the triangle is not entirely inside"
    );
}
