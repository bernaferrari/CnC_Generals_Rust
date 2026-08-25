//! Behavior suite extracted from the original test module.
use super::*;

#[test]
fn named_flash_sets_drawable_flash_count_for_presentation() {
    // C++ ScriptActions.cpp:2661-2666 frames / DRAWABLE_FRAMES_PER_FLASH (15).
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();

    const UNIT_ID: ObjectID = 8750;
    let unit = crate::object_manager::GameObjectInstance::new(
        UNIT_ID,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("flash unit");
    let drawable = Arc::new(RwLock::new(crate::object::drawable::Drawable::new(
        1,
        UNIT_ID,
        "FlashModel".to_string(),
        crate::object::drawable::DrawableType::Static,
    )));
    {
        let base_arc = unit.base();
        let mut base = base_arc.write().unwrap();
        base.set_drawable(Some(Arc::clone(&drawable)));
    }
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(unit, Coord3D::new(1.0, 1.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("FlashUnit".to_string(), UNIT_ID)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedFlash);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "FlashUnit".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_flash(&action).unwrap();

    let guard = drawable.read().unwrap();
    assert_eq!(
        guard.get_flash_count(),
        4,
        "2s * 30fps / 15 frames-per-flash"
    );

    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn named_custom_color_unpacks_argb_without_swapping_red_blue() {
    // C++ Color.h GameMakeColor: (a<<24)|(r<<16)|(g<<8)|b
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();

    const UNIT_ID: ObjectID = 8760;
    let unit = crate::object_manager::GameObjectInstance::new(
        UNIT_ID,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("color unit");
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(unit, Coord3D::new(1.0, 1.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ColorUnit".to_string(), UNIT_ID)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedCustomColor);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ColorUnit".to_string(),
        ))
        .unwrap();
    // Opaque red 0xFFFF0000
    action
        .add_parameter(Parameter::with_int(ParameterType::Color, -65536))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_custom_color(&action).unwrap();

    let color = get_object_manager()
        .read()
        .unwrap()
        .with_object(UNIT_ID, |o| {
            o.base().read().ok().map(|b| b.get_indicator_color())
        })
        .flatten()
        .expect("indicator color");
    assert_eq!(color.r, 255);
    assert_eq!(color.g, 0);
    assert_eq!(color.b, 0);

    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn named_set_attitude_reads_int_mood() {
    // C++ ScriptActions.cpp:6585 getInt() AttitudeType Aggressive=2.
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();

    const UNIT_ID: ObjectID = 8770;
    let (_, _, _, attitudes) = install_recording_named_unit(UNIT_ID, "MoodUnit", None);

    let mut action = ScriptAction::new(ScriptActionType::NamedSetAttitude);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MoodUnit".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::AiMood, 2))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_set_attitude(&action).unwrap();

    assert_eq!(
        *attitudes.lock().unwrap(),
        vec![crate::modules::AIAttitudeType::Aggressive]
    );

    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn named_attack_area_qualifies_my_inner_perimeter() {
    // C++ ScriptEngine.cpp:5888-5897 MyInnerPerimeter -> InnerPerimeter{mpStart+1}
    let _lock = crate::test_sync::lock();
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_terrain_logic().write().unwrap().reset();
    crate::player::player_list().write().unwrap().clear();

    let mut player = crate::player::Player::new(0);
    player.set_display_name("PerimeterOwner");
    player.set_mp_start_index(0);
    crate::player::player_list()
        .write()
        .unwrap()
        .add_player(Arc::new(RwLock::new(player)));

    let _ = initialize_script_engine();
    {
        let handle = get_script_engine();
        let mut slot = handle.write().unwrap();
        if let Some(engine) = slot.as_mut() {
            engine.set_external_eval_context(Some("PerimeterOwner".to_string()), None);
        }
    }

    get_terrain_logic().write().unwrap().add_trigger_area(
        crate::polygon_trigger::PolygonTrigger::new(
            9301,
            AsciiString::from("InnerPerimeter1"),
            vec![
                crate::common::ICoord3D::new(0, 0, 0),
                crate::common::ICoord3D::new(20, 0, 0),
                crate::common::ICoord3D::new(20, 20, 0),
                crate::common::ICoord3D::new(0, 20, 0),
            ],
        ),
    );

    const UNIT_ID: ObjectID = 8780;
    let (commands, _, _, _) = install_recording_named_unit(UNIT_ID, "PerimeterAttacker", None);

    let mut action = ScriptAction::new(ScriptActionType::NamedAttackArea);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "PerimeterAttacker".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            crate::scripting::engine::MY_INNER_PERIMETER.to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_attack_area(&action).unwrap();

    assert_eq!(commands.lock().unwrap()[0].0, AiCommandType::AttackArea);

    crate::player::player_list().write().unwrap().clear();
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_terrain_logic().write().unwrap().reset();
}

#[test]
fn team_follow_waypoints_honors_as_team_int() {
    // C++ ScriptActions.cpp:1803-1807 asTeam selects groupFollowWaypointPathAsTeam.
    let _lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    get_team_factory().lock().unwrap().reset();
    get_terrain_logic().write().unwrap().reset();

    let mut map_data = crate::system::map_loader::MapData::new();
    map_data.width = 4;
    map_data.height = 4;
    map_data.heightmap = vec![0; 16];
    map_data
        .waypoints
        .push(crate::system::map_loader::MapWaypoint {
            id: 9401,
            name: "TeamFollowWp".to_string(),
            location: crate::system::map_loader::Coord3D::new(10.0, 10.0, 0.0),
            path_label1: "TeamPath".to_string(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    get_terrain_logic().write().unwrap().load_map_data(map_data);

    const UNIT_ID: ObjectID = 8790;
    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let obj = Arc::new(RwLock::new(crate::object::Object::new_test(UNIT_ID, 100.0)));
    {
        let mut guard = obj.write().unwrap();
        guard.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingMoveAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
            cleared: Arc::new(Mutex::new(0)),
            attitudes: Arc::new(Mutex::new(Vec::new())),
        }))));
        let _ = guard.set_position(&Coord3D::new(4.0, 5.0, 0.0));
    }
    TheGameLogic::register_object(obj).expect("register team follower");

    {
        let mut factory = get_team_factory().lock().unwrap();
        let team = factory.create_team("FollowTeam").expect("team");
        team.write().unwrap().add_member(UNIT_ID);
    }

    let mut action = ScriptAction::new(ScriptActionType::TeamFollowWaypoints);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "FollowTeam".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "TeamPath".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_team_follow_waypoints(&action).unwrap();

    assert_eq!(
        commands.lock().unwrap()[0].0,
        AiCommandType::FollowWaypointPathAsTeam
    );

    crate::object::registry::OBJECT_REGISTRY.clear();
    get_team_factory().lock().unwrap().reset();
    get_terrain_logic().write().unwrap().reset();
}

#[test]
fn named_face_named_clears_waypoint_queue() {
    // C++ ScriptActions.cpp:6092 doNamedFaceNamed clearWaypointQueue.
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();

    const UNIT_ID: ObjectID = 8800;
    const TARGET_ID: ObjectID = 8801;
    let (_, _, cleared, _) = install_recording_named_unit(UNIT_ID, "FaceUnit", None);
    let target = crate::object_manager::GameObjectInstance::new(
        TARGET_ID,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("face target");
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(target, Coord3D::new(9.0, 9.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("FaceTarget".to_string(), TARGET_ID)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedFaceNamed);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "FaceUnit".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "FaceTarget".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_face_named(&action).unwrap();
    assert_eq!(*cleared.lock().unwrap(), 1);

    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn has_finished_media_fails_closed_without_handler() {
    // C++ ScriptConditions.cpp:1419-1437 queries TheScriptEngine; missing handler is not complete.
    // HAS_FINISHED_AUDIO without a live handler still uses leftover ScriptEngine
    // TheAudio length (C++ isAudioComplete), so it is not in this fail-closed set.
    let _test_lock = crate::test_sync::lock();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    for (kind, name) in [
        (ConditionType::HasFinishedVideo, "IntroMovie"),
        (ConditionType::HasFinishedSpeech, "Briefing"),
    ] {
        let mut condition = Condition::new(kind);
        condition
            .add_parameter(Parameter::with_string(ParameterType::Movie, name.into()))
            .unwrap();
        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::False,
            "{kind:?} must fail closed without an action handler"
        );
    }
}

#[test]
fn multiplayer_player_defeat_has_no_player_param() {
    // C++ ScriptConditions.cpp:1748-1750 — no params; missing local player is false, not error.
    let mut condition = Condition::new(ConditionType::MultiplayerPlayerDefeat);
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False
    );
}

#[test]
fn named_totally_dead_false_while_object_exists() {
    // C++ ScriptConditions.cpp:323-335 — getUnitNamed success is never totally dead.
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    get_named_object_tracker().clear().unwrap();

    const ID: u32 = 0x70_7A_11_DE;
    let obj = Arc::new(RwLock::new(crate::object::Object::new_test(ID, 100.0)));
    obj.write().unwrap().set_effectively_dead(true);
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &obj);
    get_named_object_tracker()
        .register_named_object("DeadHero".to_string(), ID)
        .unwrap();

    let mut condition = Condition::new(ConditionType::NamedTotallyDead);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "DeadHero".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "NAMED_TOTALLY_DEAD is false while the object still exists"
    );

    crate::object::registry::OBJECT_REGISTRY.unregister_object(ID);
    drop(obj);
    get_named_object_tracker().unregister_object(ID).unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True,
        "NAMED_TOTALLY_DEAD is true only after the named object is gone"
    );
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn named_totally_dead_false_if_name_never_existed() {
    // C++ ScriptConditions.cpp:334 — never-existed names are not totally dead.
    let _test_lock = crate::test_sync::lock();
    get_named_object_tracker().clear().unwrap();
    let mut condition = Condition::new(ConditionType::NamedTotallyDead);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NeverExisted".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False
    );
}

#[test]
fn unit_health_uses_initial_health_rounding() {
    // C++ ScriptConditions.cpp:934 (curHealth*100 + initialHealth/2)/initialHealth
    // When initial==max, 100% current health is 100.

    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    get_named_object_tracker().clear().unwrap();

    const ID: u32 = 0x11_11_EA_17;
    let obj = Arc::new(RwLock::new(crate::object::Object::new_test(ID, 100.0)));
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &obj);
    get_named_object_tracker()
        .register_named_object("HurtHero".to_string(), ID)
        .unwrap();

    let mut condition = Condition::new(ConditionType::UnitHealth);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "HurtHero".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 100))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True,
        "full initial health is 100 percent via the C++ integer formula"
    );

    crate::object::registry::OBJECT_REGISTRY.unregister_object(ID);
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn built_by_player_rejects_object_type_lists() {
    // C++ ScriptConditions.cpp:872-874 findTemplate(raw) must exist.
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine");
    let mut list = crate::object::object_types::ObjectTypes::new();
    list.add_object_type(crate::common::AsciiString::from("AmericaRanger"));
    let _ = with_script_engine_mut(|engine| {
        engine.set_object_types("HeroList".to_string(), list);
    });

    let mut condition = Condition::new(ConditionType::BuiltByPlayer);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "HeroList".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "type-list names are inert in retail BUILT_BY_PLAYER"
    );
}

#[test]
fn team_the_player_not_remapped_outside_challenge() {
    // C++ ScriptEngine.cpp:5935-5939 remaps TEAM_THE_PLAYER only in Challenge.
    let evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.resolve_string_token(crate::scripting::core::TEAM_THE_PLAYER),
        crate::scripting::core::TEAM_THE_PLAYER
    );
}

#[test]
fn the_player_not_remapped_outside_challenge() {
    // C++ ScriptEngine.cpp:5809-5814 remaps THE_PLAYER only in Challenge.
    let evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.resolve_string_token(crate::scripting::core::THE_PLAYER),
        crate::scripting::core::THE_PLAYER
    );
}

#[test]
fn player_has_counts_dead_lost_ignores_dead() {
    // C++ HAS ignoreDead=FALSE (ScriptConditions.cpp:1792);
    // LOST ignoreDead=TRUE (ScriptConditions.cpp:2686).
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();

    let player = crate::player::Player::new(0);
    const ID: u32 = 0x0A5_DEAD;
    let template: Arc<dyn crate::common::ThingTemplate> = Arc::new(
        crate::common::DefaultThingTemplate::new("AmericaRanger".to_string()),
    );
    let obj = Arc::new(RwLock::new(crate::object::Object::new_test_from_template(
        ID,
        100.0,
        Arc::clone(&template),
    )));
    obj.write().unwrap().set_effectively_dead(true);
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &obj);
    let mut player = player;
    player.add_owned_object(ID);

    let templates = vec![Arc::clone(&template)];
    let mut counts = vec![0];
    player.count_objects_by_thing_template(&templates, false, true, &mut counts);
    assert_eq!(
        counts[0], 1,
        "HAS ignoreDead=false still counts a dead object"
    );
    player.count_objects_by_thing_template(&templates, true, true, &mut counts);
    assert_eq!(
        counts[0], 0,
        "LOST ignoreDead=true skips effectively-dead objects"
    );

    crate::object::registry::OBJECT_REGISTRY.unregister_object(ID);
}

#[test]
fn skirmish_value_in_area_excludes_inert() {
    // C++ ScriptConditions.cpp:2139 !KINDOF_INERT
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    player_list().write().unwrap().clear();

    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    {
        let mut guard = player.write().unwrap();
        guard.set_display_name("ValuePlayer");
    }
    player_list().write().unwrap().add_player(player);

    const ID: u32 = 0x1E_E47;
    let mut template = crate::common::DefaultThingTemplate::new("InertProp".to_string());
    template.add_kind_of(crate::common::KindOf::Inert);
    let obj = Arc::new(RwLock::new(crate::object::Object::new_test_from_template(
        ID,
        10.0,
        Arc::new(template),
    )));
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &obj);
    get_area_tracker()
        .register_area(crate::scripting::events::TriggerArea::new_circular(
            "ValueArea".to_string(),
            [0.0, 0.0, 0.0],
            50.0,
        ))
        .unwrap();
    let events = crate::scripting::engine::get_event_manager();
    get_area_tracker()
        .update_object_position_sync(ID, [0.0, 0.0, 0.0], &events)
        .unwrap();

    let mut condition = Condition::new(ConditionType::SkirmishValueInArea);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "ValuePlayer".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 4))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "ValueArea".into(),
        ))
        .unwrap();

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "inert objects do not add value"
    );

    crate::object::registry::OBJECT_REGISTRY.unregister_object(ID);
    player_list().write().unwrap().clear();
}

#[test]
fn set_cave_index_queues_host_when_dual_world_empty() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_set_cave_index_requests();
    let mut action = ScriptAction::new(ScriptActionType::SetCaveIndex);
    action
        .add_parameter(Parameter::with_string(ParameterType::Unit, "CaveB".into()))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        dispatcher.execute_action(&action).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_set_cave_index_requests(),
        vec![("CaveB".to_string(), 3)]
    );
}

#[test]
fn team_panic_queues_host_when_dual_world_empty() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_team_loco_set_requests();
    let mut action = ScriptAction::new(ScriptActionType::TeamPanic);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "TeamCivilians".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "PanicPath".into(),
        ))
        .unwrap();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        dispatcher.execute_action(&action).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_team_loco_set_requests(),
        vec![(
            "TeamCivilians".to_string(),
            "panic".to_string(),
            Some("PanicPath".to_string())
        )]
    );
}

#[test]
fn create_object_queues_host_when_dual_world_empty() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_script_create_requests();
    let mut action = ScriptAction::new(ScriptActionType::CreateObject);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaInfantryRanger".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_coord(
            ParameterType::Coord3D,
            crate::scripting::core::Coord3D::new(10.0, 20.0, 0.0),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_real(ParameterType::Angle, 1.5))
        .unwrap();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        dispatcher.execute_action(&action).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_script_create_requests(),
        vec![HostScriptCreateRequest::Object {
            name: None,
            thing: "AmericaInfantryRanger".to_string(),
            team: "teamAmerica".to_string(),
            x: 10.0,
            y: 20.0,
            z: 0.0,
            angle: 1.5,
        }]
    );
}

#[test]
fn named_team_kill_delete_damage_queue_host_when_dual_world_empty() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();

    let _ = take_host_script_kill_delete_damage_requests();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut named_delete = ScriptAction::new(ScriptActionType::NamedDelete);
    named_delete
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "Flyover".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&named_delete).unwrap(),
        ScriptActionResult::Success
    );

    let mut named_kill = ScriptAction::new(ScriptActionType::NamedKill);
    named_kill
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "Civilian".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&named_kill).unwrap(),
        ScriptActionResult::Success
    );

    let mut named_damage = ScriptAction::new(ScriptActionType::NamedDamage);
    named_damage
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    named_damage
        .add_parameter(Parameter::with_int(ParameterType::Int, 25))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&named_damage).unwrap(),
        ScriptActionResult::Success
    );

    let mut team_delete = ScriptAction::new(ScriptActionType::TeamDelete);
    team_delete
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_delete).unwrap(),
        ScriptActionResult::Success
    );

    let mut team_kill = ScriptAction::new(ScriptActionType::TeamKill);
    team_kill
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamChina".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_kill).unwrap(),
        ScriptActionResult::Success
    );

    let mut team_damage = ScriptAction::new(ScriptActionType::DamageMembersOfTeam);
    team_damage
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamGLA".into(),
        ))
        .unwrap();
    team_damage
        .add_parameter(Parameter::with_real(ParameterType::Real, 40.0))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_damage).unwrap(),
        ScriptActionResult::Success
    );

    let mut team_delete_living = ScriptAction::new(ScriptActionType::TeamDeleteLiving);
    team_delete_living
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamUSA".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_delete_living).unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        take_host_script_kill_delete_damage_requests(),
        vec![
            HostScriptKillDeleteDamageRequest::NamedDelete {
                unit: "Flyover".to_string()
            },
            HostScriptKillDeleteDamageRequest::NamedKill {
                unit: "Civilian".to_string()
            },
            HostScriptKillDeleteDamageRequest::NamedDamage {
                unit: "Hero".to_string(),
                amount: 25
            },
            HostScriptKillDeleteDamageRequest::TeamDelete {
                team: "teamAmerica".to_string(),
                ignore_dead: false
            },
            HostScriptKillDeleteDamageRequest::TeamKill {
                team: "teamChina".to_string()
            },
            HostScriptKillDeleteDamageRequest::TeamDamage {
                team: "teamGLA".to_string(),
                amount: 40.0
            },
            HostScriptKillDeleteDamageRequest::TeamDelete {
                team: "teamUSA".to_string(),
                ignore_dead: true
            },
        ]
    );
}

#[test]
fn player_kill_queues_host_even_when_leftover_player_missing() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_script_player_misc_requests();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut action = ScriptAction::new(ScriptActionType::PlayerKill);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrGLA".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&action).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_script_player_misc_requests(),
        vec![HostScriptPlayerMiscRequest::Kill {
            player: "PlyrGLA".to_string()
        }]
    );
}

#[test]
fn named_object_sound_queues_host_when_dual_world_empty() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_script_object_sound_requests();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut play = ScriptAction::new(ScriptActionType::SoundPlayNamed);
    play.add_parameter(Parameter::with_string(
        ParameterType::TextString,
        "UnitCheer".into(),
    ))
    .unwrap();
    play.add_parameter(Parameter::with_string(
        ParameterType::Unit,
        "NamedScout".into(),
    ))
    .unwrap();
    assert_eq!(
        dispatcher.execute_action(&play).unwrap(),
        ScriptActionResult::Success
    );

    let mut enable = ScriptAction::new(ScriptActionType::EnableObjectSound);
    enable
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedFactory".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&enable).unwrap(),
        ScriptActionResult::Success
    );

    let mut disable = ScriptAction::new(ScriptActionType::DisableObjectSound);
    disable
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedFactory".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&disable).unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        take_host_script_object_sound_requests(),
        vec![
            HostScriptObjectSoundRequest::Enable {
                unit: "NamedFactory".to_string(),
                enable: true
            },
            HostScriptObjectSoundRequest::Enable {
                unit: "NamedFactory".to_string(),
                enable: false
            },
        ]
    );
}

#[test]
fn team_set_attitude_queues_host_when_dual_world_empty() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_team_attitude_requests();
    let mut action = ScriptAction::new(ScriptActionType::TeamSetAttitude);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "AmericaTeamHeroes".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::AiMood, 2))
        .unwrap();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        dispatcher.execute_action(&action).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_team_attitude_requests(),
        vec![("AmericaTeamHeroes".to_string(), 2)]
    );
}

#[test]
fn build_team_queues_host_when_dual_world_empty() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_build_team_requests();
    get_team_factory().lock().unwrap().reset();
    get_team_factory().lock().unwrap().init_team(
        AsciiString::from("SquadHost"),
        AsciiString::from("PlyrAmerica"),
        false,
        None,
    );
    let mut action = ScriptAction::new(ScriptActionType::BuildTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "SquadHost".into(),
        ))
        .unwrap();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        dispatcher.execute_action(&action).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_build_team_requests(),
        vec![("PlyrAmerica".to_string(), "SquadHost".to_string())]
    );
    get_team_factory().lock().unwrap().reset();
}

#[test]
fn ai_player_build_actions_queue_host_when_dual_world_empty() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_ai_player_build_supply_center_requests();
    let _ = take_host_ai_player_build_upgrade_requests();
    let _ = take_host_ai_player_build_type_nearest_team_requests();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut supply = ScriptAction::new(ScriptActionType::AiPlayerBuildSupplyCenter);
    supply
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    supply
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaSupplyCenter".into(),
        ))
        .unwrap();
    supply
        .add_parameter(Parameter::with_int(ParameterType::Int, 1000))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&supply).unwrap(),
        ScriptActionResult::Success
    );

    let mut upgrade = ScriptAction::new(ScriptActionType::AiPlayerBuildUpgrade);
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    upgrade
        .add_parameter(Parameter::with_string(
            ParameterType::Upgrade,
            "Upgrade_AmericaSupplyLines".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&upgrade).unwrap(),
        ScriptActionResult::Success
    );

    let mut nearest = ScriptAction::new(ScriptActionType::AiPlayerBuildTypeNearestTeam);
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaPatriotBattery".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&nearest).unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        take_host_ai_player_build_supply_center_requests(),
        vec![(
            "PlyrAmerica".to_string(),
            "AmericaSupplyCenter".to_string(),
            1000
        )]
    );
    assert_eq!(
        take_host_ai_player_build_upgrade_requests(),
        vec![(
            "PlyrAmerica".to_string(),
            "Upgrade_AmericaSupplyLines".to_string()
        )]
    );
    assert_eq!(
        take_host_ai_player_build_type_nearest_team_requests(),
        vec![(
            "PlyrAmerica".to_string(),
            "AmericaPatriotBattery".to_string(),
            "USA_RangerSquad".to_string()
        )]
    );
}

#[test]
fn live_named_created_true_from_host_snapshot() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("Hero".into(), 7)].into_iter().collect(),
        objects: vec![live_host_named_object("Hero", 7, true)],
        ..Default::default()
    });
    let mut condition = Condition::new(ConditionType::NamedCreated);
    condition
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn live_enemy_and_type_sighted_use_host_snapshot() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();

    let mut looker = live_host_named_object("LookerScout", 7, true);
    looker.x = 0.0;
    looker.z = 0.0;
    looker.vision_range = 150.0;
    looker.team = 1;
    looker.owner_player = "PlyrAmerica".into();

    let mut enemy = live_host_named_object("EnemyRanger", 8, true);
    enemy.x = 40.0;
    enemy.z = 0.0;
    enemy.vision_range = 100.0;
    enemy.team = 0;
    enemy.owner_player = "PlyrGLA".into();
    enemy.template_name = "AmericaRanger".into();

    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("LookerScout".into(), 7)].into_iter().collect(),
        objects: vec![looker, enemy.clone()],
        ..Default::default()
    });

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut enemy_sighted = Condition::new(ConditionType::EnemySighted);
    enemy_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "LookerScout".into(),
        ))
        .unwrap();
    enemy_sighted
        .add_parameter(Parameter::with_int(ParameterType::Relation, 0))
        .unwrap();
    enemy_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrGLA".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut enemy_sighted).unwrap(),
        ScriptConditionResult::True,
        "ENEMY_SIGHTED must use host vision when OBJECT_REGISTRY is empty"
    );

    let mut type_sighted = Condition::new(ConditionType::TypeSighted);
    type_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "LookerScout".into(),
        ))
        .unwrap();
    type_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaRanger".into(),
        ))
        .unwrap();
    type_sighted
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrGLA".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut type_sighted).unwrap(),
        ScriptConditionResult::True,
        "TYPE_SIGHTED must use host vision when OBJECT_REGISTRY is empty"
    );

    enemy.stealthed_hidden = true;
    let mut looker = live_host_named_object("LookerScout", 7, true);
    looker.x = 0.0;
    looker.z = 0.0;
    looker.vision_range = 150.0;
    looker.team = 1;
    looker.owner_player = "PlyrAmerica".into();
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("LookerScout".into(), 7)].into_iter().collect(),
        objects: vec![looker, enemy],
        ..Default::default()
    });
    assert_eq!(
        evaluator.evaluate_condition(&mut enemy_sighted).unwrap(),
        ScriptConditionResult::False,
        "undetected stealth must fail host ENEMY_SIGHTED"
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut type_sighted).unwrap(),
        ScriptConditionResult::False,
        "undetected stealth must fail host TYPE_SIGHTED"
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn live_named_totally_dead_false_at_load_when_host_unit_lives() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    get_named_object_tracker().clear().unwrap();
    get_named_object_tracker()
        .register_named_object("MapHero".to_string(), 7)
        .unwrap();
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("MapHero".into(), 7)].into_iter().collect(),
        objects: vec![live_host_named_object("MapHero", 7, true)],
        ..Default::default()
    });
    let mut condition = Condition::new(ConditionType::NamedTotallyDead);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MapHero".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "NAMED_TOTALLY_DEAD must stay false while the host unit still exists"
    );
    crate::scripting::clear_host_script_query_snapshot();
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn live_named_destroyed_true_after_host_destroy_list() {
    // C++ evaluateNamedUnitDestroyed: after processDestroyList the pointer is
    // NULL and didUnitExist keeps the condition TRUE even if other units live.
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    get_named_object_tracker().clear().unwrap();
    get_named_object_tracker()
        .register_named_object("MapHero".to_string(), 7)
        .unwrap();
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("OtherScout".into(), 8)].into_iter().collect(),
        objects: vec![live_host_named_object("OtherScout", 8, true)],
        ..Default::default()
    });
    let mut condition = Condition::new(ConditionType::NamedDestroyed);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MapHero".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True,
        "NAMED_DESTROYED must stay true after host destroy-list removes the unit"
    );
    crate::scripting::clear_host_script_query_snapshot();
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn live_named_selected_true_from_host_snapshot() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    let mut selected = live_host_named_object("TutorialRanger", 11, true);
    selected.selected = true;
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("TutorialRanger".into(), 11)].into_iter().collect(),
        objects: vec![selected],
        ..Default::default()
    });
    let mut condition = Condition::new(ConditionType::NamedSelected);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "TutorialRanger".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True,
        "NAMED_SELECTED must read host UI selection when OBJECT_REGISTRY is empty"
    );
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("TutorialRanger".into(), 11)].into_iter().collect(),
        objects: vec![live_host_named_object("TutorialRanger", 11, true)],
        ..Default::default()
    });
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "NAMED_SELECTED is false when the host unit is not selected"
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn live_team_destroyed_false_when_host_members_live() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        objects: vec![live_host_named_object("Ranger", 7, true)],
        team_instance_ids: [("USA_RangerSquad".into(), vec![7])].into_iter().collect(),
        ..Default::default()
    });
    let mut destroyed = Condition::new(ConditionType::TeamDestroyed);
    destroyed
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    let mut has_units = Condition::new(ConditionType::TeamHasUnits);
    has_units
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    let mut created = Condition::new(ConditionType::TeamCreated);
    created
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut destroyed).unwrap(),
        ScriptConditionResult::False
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut has_units).unwrap(),
        ScriptConditionResult::True
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut created).unwrap(),
        ScriptConditionResult::True
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn live_named_entered_uses_host_trigger_flags() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    crate::terrain::get_terrain_logic()
        .write()
        .expect("terrain")
        .add_trigger_area(crate::polygon_trigger::PolygonTrigger::new(
            1814,
            crate::common::AsciiString::from("Wave18PolyPad"),
            vec![
                crate::common::ICoord3D::new(0, 0, 0),
                crate::common::ICoord3D::new(20, 0, 0),
                crate::common::ICoord3D::new(0, 20, 0),
            ],
        ));
    crate::system::game_logic::get_game_logic()
        .lock()
        .expect("logic")
        .set_current_frame(20);
    crate::scripting::update_host_object_trigger_flags(7, 18.0, 18.0, 19, false, Some("teamUSA"));
    crate::scripting::update_host_object_trigger_flags(7, 2.0, 2.0, 20, false, Some("teamUSA"));
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("Scout".into(), 7)].into_iter().collect(),
        objects: vec![live_host_named_object("Scout", 7, true)],
        team_instance_ids: [("teamUSA".into(), vec![7])].into_iter().collect(),
        ..Default::default()
    });
    let mut entered = Condition::new(ConditionType::NamedEnteredArea);
    entered
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Scout".into()))
        .unwrap();
    entered
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "Wave18PolyPad".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut entered).unwrap(),
        ScriptConditionResult::True
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn live_named_body_state_reads_host_snapshot() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("Hero".into(), 7)].into_iter().collect(),
        objects: vec![live_host_named_object("Hero", 7, true)],
        ..Default::default()
    });
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut health = Condition::new(ConditionType::UnitHealth);
    health
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    health
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    health
        .add_parameter(Parameter::with_int(ParameterType::Int, 100))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut health).unwrap(),
        ScriptConditionResult::True
    );

    let mut owned = Condition::new(ConditionType::NamedOwnedByPlayer);
    owned
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    owned
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut owned).unwrap(),
        ScriptConditionResult::True
    );

    let mut empty = Condition::new(ConditionType::NamedBuildingIsEmpty);
    empty
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut empty).unwrap(),
        ScriptConditionResult::True
    );

    let mut slots = Condition::new(ConditionType::NamedHasFreeContainerSlots);
    slots
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut slots).unwrap(),
        ScriptConditionResult::True
    );

    let mut dying = Condition::new(ConditionType::NamedDying);
    dying
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut dying).unwrap(),
        ScriptConditionResult::False
    );

    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn eval_player_has_comparison_unit_type_in_trigger_area_zero_matching() {
    // Given: live host census with no matching type in the trigger
    // When: PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA compares the count
    // Then: count is 0 so ==0 is true and >=1 is false
    let _test_lock = crate::test_sync::lock();
    install_live_hold_zone_census(vec![live_host_area_unit(
        1,
        "PlyrAmerica",
        "AmericaTankCrusader",
        5.0,
        5.0,
        &["Vehicle"],
    )]);
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut eq_zero = eval_player_unit_type_in_area(2, 0, "AmericaInfantryRanger", "HoldZone");
    let mut ge_one = eval_player_unit_type_in_area(3, 1, "AmericaInfantryRanger", "HoldZone");
    assert_eq!(
        evaluator.evaluate_condition(&mut eq_zero).unwrap(),
        ScriptConditionResult::True,
        "0 matching units in the trigger must compare as count == 0"
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut ge_one).unwrap(),
        ScriptConditionResult::False,
        "0 matching units must not satisfy >= 1"
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn eval_player_has_comparison_unit_type_in_trigger_area_n_units_ge_n() {
    // Given: N live host units of the scripted type inside the trigger
    // When: PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA asks >= N
    // Then: the condition is true (C++ evaluatePlayerHasUnitTypeInArea)
    let _test_lock = crate::test_sync::lock();
    install_live_hold_zone_census(vec![
        live_host_area_unit(
            1,
            "PlyrAmerica",
            "AmericaInfantryRanger",
            4.0,
            4.0,
            &["Infantry"],
        ),
        live_host_area_unit(
            2,
            "PlyrAmerica",
            "AmericaInfantryRanger",
            8.0,
            8.0,
            &["Infantry"],
        ),
        live_host_area_unit(
            3,
            "PlyrAmerica",
            "AmericaInfantryRanger",
            80.0,
            80.0,
            &["Infantry"],
        ),
        live_host_area_unit(
            4,
            "PlyrChina",
            "AmericaInfantryRanger",
            6.0,
            6.0,
            &["Infantry"],
        ),
    ]);
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut ge_two = eval_player_unit_type_in_area(3, 2, "AmericaInfantryRanger", "HoldZone");
    assert_eq!(
        evaluator.evaluate_condition(&mut ge_two).unwrap(),
        ScriptConditionResult::True,
        "two matching live units in the trigger must satisfy >= 2"
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn eval_player_has_comparison_unit_type_in_trigger_area_map_load_eq_zero_false() {
    // Given: map-load census with living units already standing in the zone
    // When: PLAYER_HAS_COMPARISON_UNIT_TYPE_IN_TRIGGER_AREA == 0
    // Then: false — leftover owned_objects must not report an empty world
    let _test_lock = crate::test_sync::lock();
    install_live_hold_zone_census(vec![live_host_area_unit(
        1,
        "PlyrAmerica",
        "AmericaInfantryRanger",
        5.0,
        5.0,
        &["Infantry"],
    )]);
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut eq_zero = eval_player_unit_type_in_area(2, 0, "AmericaInfantryRanger", "HoldZone");
    assert_eq!(
        evaluator.evaluate_condition(&mut eq_zero).unwrap(),
        ScriptConditionResult::False,
        "== 0 must stay false at map load while living units occupy the trigger"
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn eval_player_has_comparison_unit_kind_in_trigger_area_n_units_ge_n() {
    // Given: N live host infantry inside the trigger
    // When: PLAYER_HAS_COMPARISON_UNIT_KIND_IN_TRIGGER_AREA asks >= N
    // Then: true (C++ evaluatePlayerHasUnitKindInArea)
    let _test_lock = crate::test_sync::lock();
    const KINDOF_INFANTRY: i32 = 4;
    install_live_hold_zone_census(vec![
        live_host_area_unit(
            1,
            "PlyrAmerica",
            "AmericaInfantryRanger",
            4.0,
            4.0,
            &["Infantry"],
        ),
        live_host_area_unit(
            2,
            "PlyrAmerica",
            "AmericaInfantryMissileDefender",
            9.0,
            6.0,
            &["Infantry"],
        ),
    ]);
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut ge_two = eval_player_unit_kind_in_area(3, 2, KINDOF_INFANTRY, "HoldZone");
    let mut eq_zero = eval_player_unit_kind_in_area(2, 0, KINDOF_INFANTRY, "HoldZone");
    assert_eq!(
        evaluator.evaluate_condition(&mut ge_two).unwrap(),
        ScriptConditionResult::True
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut eq_zero).unwrap(),
        ScriptConditionResult::False,
        "kind == 0 must not fire at map load while infantry occupy the trigger"
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn live_skirmish_leftover_conditions_read_host_snapshot() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();

    let mut command_center = live_host_area_unit(
        1,
        "PlyrAmerica",
        "AmericaCommandCenter",
        5.0,
        5.0,
        &["Structure"],
    );
    command_center.special_power_ready = true;
    command_center.special_power_templates = vec!["SuperweaponDaisyCutter".into()];
    command_center.build_cost = 2000;
    command_center.garrisonable = true;
    command_center.contain_count = 2;
    command_center.captured = true;

    let mut unmanned =
        live_host_area_unit(2, "PlyrNeutral", "AmericaTankCrusader", 50.0, 50.0, &[]);
    unmanned.unmanned = true;
    unmanned.team = 3;

    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        objects: vec![command_center, unmanned],
        areas: [("HoldZone".into(), (0.0, 0.0, 10.0, 10.0))]
            .into_iter()
            .collect(),
        ..Default::default()
    });

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut power = Condition::new(ConditionType::SkirmishSpecialPowerReady);
    power
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    power
        .add_parameter(Parameter::with_string(
            ParameterType::SpecialPower,
            "SuperweaponDaisyCutter".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut power).unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_SPECIAL_POWER_READY must see host special_power_ready"
    );

    let mut value = Condition::new(ConditionType::SkirmishValueInArea);
    value
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    value
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
        .unwrap();
    value
        .add_parameter(Parameter::with_int(ParameterType::Int, 2000))
        .unwrap();
    value
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "HoldZone".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut value).unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_VALUE_IN_AREA must sum host build_cost"
    );

    let mut unowned = Condition::new(ConditionType::SkirmishUnownedFactionUnitExists);
    unowned
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    unowned
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    unowned
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut unowned).unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_UNOWNED_FACTION_UNIT_EXISTS must count host unmanned"
    );

    let mut garrisoned = Condition::new(ConditionType::SkirmishPlayerHasComparisonGarrisoned);
    garrisoned
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    garrisoned
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 3))
        .unwrap();
    garrisoned
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut garrisoned).unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_PLAYER_HAS_COMPARISON_GARRISONED must count host garrisonable+occupied"
    );

    let mut captured = Condition::new(ConditionType::SkirmishPlayerHasComparisonCapturedUnits);
    captured
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    captured
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    captured
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut captured).unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_PLAYER_HAS_COMPARISON_CAPTURED_UNITS must count host captured"
    );

    let mut units = Condition::new(ConditionType::SkirmishPlayerHasUnitsInArea);
    units
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    units
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "HoldZone".into(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut units).unwrap(),
        ScriptConditionResult::True,
        "SKIRMISH_PLAYER_HAS_UNITS_IN_AREA must see host objects in the pad"
    );

    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn bridge_broken_reads_host_named_state_only_on_damage_edge() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut broken = Condition::new(ConditionType::BridgeBroken);
    broken
        .add_parameter(Parameter::with_string(
            ParameterType::Bridge,
            "ConvoyBridge".into(),
        ))
        .unwrap();
    let mut repaired = Condition::new(ConditionType::BridgeRepaired);
    repaired
        .add_parameter(Parameter::with_string(
            ParameterType::Bridge,
            "ConvoyBridge".into(),
        ))
        .unwrap();

    assert_eq!(
        evaluator.evaluate_condition(&mut broken).unwrap(),
        ScriptConditionResult::False
    );

    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        any_bridges_damage_states_changed: true,
        named_bridge_broken: [("ConvoyBridge".into(), true)].into_iter().collect(),
        named_bridge_repaired: [("ConvoyBridge".into(), false)].into_iter().collect(),
        ..Default::default()
    });
    assert_eq!(
        evaluator.evaluate_condition(&mut broken).unwrap(),
        ScriptConditionResult::True
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut repaired).unwrap(),
        ScriptConditionResult::False
    );

    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        any_bridges_damage_states_changed: false,
        named_bridge_broken: [("ConvoyBridge".into(), true)].into_iter().collect(),
        named_bridge_repaired: [("ConvoyBridge".into(), false)].into_iter().collect(),
        ..Default::default()
    });
    assert_eq!(
        evaluator.evaluate_condition(&mut broken).unwrap(),
        ScriptConditionResult::False
    );

    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        any_bridges_damage_states_changed: true,
        named_bridge_broken: [("ConvoyBridge".into(), false)].into_iter().collect(),
        named_bridge_repaired: [("ConvoyBridge".into(), true)].into_iter().collect(),
        ..Default::default()
    });
    assert_eq!(
        evaluator.evaluate_condition(&mut repaired).unwrap(),
        ScriptConditionResult::True
    );

    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn player_set_give_money_queue_host_drain() {
    let _test_lock = crate::test_sync::lock();
    let _ = take_host_money_requests();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut set = ScriptAction::new(ScriptActionType::PlayerSetMoney);
    set.add_parameter(Parameter::with_string(
        ParameterType::Side,
        "PlyrAmerica".into(),
    ))
    .unwrap();
    set.add_parameter(Parameter::with_int(ParameterType::Int, 10_000))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&set).unwrap(),
        ScriptActionResult::Success
    );

    let mut give = ScriptAction::new(ScriptActionType::PlayerGiveMoney);
    give.add_parameter(Parameter::with_string(
        ParameterType::Side,
        "PlyrChina".into(),
    ))
    .unwrap();
    give.add_parameter(Parameter::with_int(ParameterType::Int, -500))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&give).unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        take_host_money_requests(),
        vec![
            HostScriptMoneyRequest::Set {
                player: "PlyrAmerica".into(),
                amount: 10_000,
            },
            HostScriptMoneyRequest::Give {
                player: "PlyrChina".into(),
                amount: -500,
            },
        ]
    );
}

#[test]
fn named_team_unmanned_stealth_radar_queue_host_when_dual_world_empty() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_script_unmanned_requests();
    let _ = take_host_script_radar_event_requests();
    let _ = take_host_script_stealth_enabled_requests();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut named_unmanned = ScriptAction::new(ScriptActionType::NamedSetUnmannedStatus);
    named_unmanned
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "SnipedHumvee".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&named_unmanned).unwrap(),
        ScriptActionResult::Success
    );

    let mut team_unmanned = ScriptAction::new(ScriptActionType::TeamSetUnmannedStatus);
    team_unmanned
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_unmanned).unwrap(),
        ScriptActionResult::Success
    );

    let delete_all = ScriptAction::new(ScriptActionType::DeleteAllUnmanned);
    assert_eq!(
        dispatcher.execute_action(&delete_all).unwrap(),
        ScriptActionResult::Success
    );

    let mut named_stealth = ScriptAction::new(ScriptActionType::NamedSetStealthEnabled);
    named_stealth
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ColonelBurton".into(),
        ))
        .unwrap();
    named_stealth
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&named_stealth).unwrap(),
        ScriptActionResult::Success
    );

    let mut team_stealth = ScriptAction::new(ScriptActionType::TeamSetStealthEnabled);
    team_stealth
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamChina".into(),
        ))
        .unwrap();
    team_stealth
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_stealth).unwrap(),
        ScriptActionResult::Success
    );

    let mut object_radar = ScriptAction::new(ScriptActionType::ObjectCreateRadarEvent);
    object_radar
        .add_parameter(Parameter::with_string(ParameterType::Unit, "Hero".into()))
        .unwrap();
    object_radar
        .add_parameter(Parameter::with_int(ParameterType::Int, 4))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&object_radar).unwrap(),
        ScriptActionResult::Success
    );

    let mut team_radar = ScriptAction::new(ScriptActionType::TeamCreateRadarEvent);
    team_radar
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamGLA".into(),
        ))
        .unwrap();
    team_radar
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_radar).unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        take_host_script_unmanned_requests(),
        vec![
            HostScriptUnmannedRequest::Named {
                unit: "SnipedHumvee".to_string()
            },
            HostScriptUnmannedRequest::Team {
                team: "teamAmerica".to_string()
            },
            HostScriptUnmannedRequest::DeleteAll,
        ]
    );
    assert_eq!(
        take_host_script_stealth_enabled_requests(),
        vec![
            HostScriptStealthEnabledRequest::Named {
                unit: "ColonelBurton".to_string(),
                enabled: false,
            },
            HostScriptStealthEnabledRequest::Team {
                team: "teamChina".to_string(),
                enabled: true,
            },
        ]
    );
    assert_eq!(
        take_host_script_radar_event_requests(),
        vec![
            HostScriptRadarEventRequest::Object {
                unit: "Hero".to_string(),
                event_type: 4,
            },
            HostScriptRadarEventRequest::Team {
                team: "teamGLA".to_string(),
                event_type: 3,
            },
        ]
    );
}

#[test]
fn live_host_team_reached_waypoints_end_uses_snapshot_labels() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    let mut snap = crate::scripting::HostScriptQuerySnapshot::default();
    snap.team_instance_ids
        .insert("USA_RangerSquad".into(), vec![7]);
    let mut obj = live_host_named_object("Hero", 7, true);
    obj.waypoint_labels = vec!["HeroPath".into()];
    snap.objects.push(obj);
    crate::scripting::set_host_script_query_snapshot(snap);

    let mut condition = Condition::new(ConditionType::TeamReachedWaypointsEnd);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "HeroPath".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn live_host_skirmish_discovered_uses_snapshot() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    let mut snap = crate::scripting::HostScriptQuerySnapshot::default();
    snap.objects.push(crate::scripting::HostScriptQueryObject {
        id: 8,
        owner_player: "PlyrChina".into(),
        discovered_by: vec!["PlyrAmerica".into()],
        ..Default::default()
    });
    crate::scripting::set_host_script_query_snapshot(snap);

    let mut condition = Condition::new(ConditionType::SkirmishPlayerHasDiscoveredPlayer);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrChina".into(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True
    );
    crate::scripting::clear_host_script_query_snapshot();
}

#[test]
fn live_host_from_named_special_power_uses_host_id() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    initialize_script_engine().expect("script engine");
    player_list().write().unwrap().clear();
    let player = Arc::new(RwLock::new(crate::player::Player::new(1)));
    player.write().unwrap().set_display_name("PlyrAmerica");
    player_list().write().unwrap().add_player(player);

    get_named_object_tracker()
        .register_named_object("ParticleCannon".to_string(), 42)
        .expect("named");
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        named: [("ParticleCannon".into(), 42)].into_iter().collect(),
        objects: vec![crate::scripting::HostScriptQueryObject {
            id: 42,
            name: "ParticleCannon".into(),
            owner_player: "PlyrAmerica".into(),
            alive: true,
            ..Default::default()
        }],
        ..Default::default()
    });

    let completed = with_script_engine_mut(|engine| {
        engine.notify_of_triggered_special_power(1, "SuperweaponParticleUplinkCannon", 42);
        let mut condition = Condition::new(ConditionType::PlayerTriggeredSpecialPowerFromNamed);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "PlyrAmerica".into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::SpecialPower,
                "SuperweaponParticleUplinkCannon".into(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                "ParticleCannon".into(),
            ))
            .unwrap();
        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::True,
            "FROM_NAMED must accept host IDs when OBJECT_REGISTRY is empty"
        );
    });
    player_list().write().unwrap().clear();
    crate::scripting::clear_host_script_query_snapshot();
    get_named_object_tracker().clear().ok();
    assert_eq!(completed, Some(()));
}

#[test]
fn skirmish_approach_and_defense_host_queues_roundtrip() {
    let _ = take_host_skirmish_approach_path_requests();
    let _ = take_host_skirmish_base_defense_requests();
    request_host_skirmish_approach_path(HostScriptSkirmishApproachPathRequest {
        team: "teamAmerica".into(),
        path_label: "ApproachPath".into(),
        as_team: true,
        follow: true,
    });
    request_host_skirmish_base_defense(HostScriptSkirmishBaseDefenseRequest {
        player: "PlyrAmerica".into(),
        structure: Some("AmericaPatriotBattery".into()),
        flank: true,
    });
    let paths = take_host_skirmish_approach_path_requests();
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].team, "teamAmerica");
    assert_eq!(paths[0].path_label, "ApproachPath");
    assert!(paths[0].as_team && paths[0].follow);
    let defs = take_host_skirmish_base_defense_requests();
    assert_eq!(defs.len(), 1);
    assert_eq!(defs[0].player, "PlyrAmerica");
    assert_eq!(defs[0].structure.as_deref(), Some("AmericaPatriotBattery"));
    assert!(defs[0].flank);
    assert!(take_host_skirmish_approach_path_requests().is_empty());
    assert!(take_host_skirmish_base_defense_requests().is_empty());
}

#[test]
fn set_base_construction_speed_queues_host() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_set_base_construction_speed_requests();
    let mut action = ScriptAction::new(ScriptActionType::SetBaseConstructionSpeed);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        dispatcher.execute_action(&action).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_set_base_construction_speed_requests(),
        vec![("PlyrAmerica".to_string(), 3)]
    );
}

#[test]
fn set_train_held_queues_host() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_set_train_held_requests();
    let mut action = ScriptAction::new(ScriptActionType::SetTrainHeld);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "CivilianTrain".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        dispatcher.execute_action(&action).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_set_train_held_requests(),
        vec![("CivilianTrain".to_string(), true)]
    );
}

#[test]
fn team_nearest_and_partial_command_button_queue_host_when_dual_world_empty() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_script_use_command_button_requests();
    let _ = take_host_team_partial_command_button_requests();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut nearest =
        ScriptAction::new(ScriptActionType::TeamAllUseCommandbuttonOnNearestEnemyUnit);
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    nearest
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            "Command_Stop".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&nearest).unwrap(),
        ScriptActionResult::Success
    );

    let mut kindof = ScriptAction::new(ScriptActionType::TeamAllUseCommandbuttonOnNearestKindof);
    kindof
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    kindof
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            "Command_Hijack".into(),
        ))
        .unwrap();
    kindof
        .add_parameter(Parameter::with_string(
            ParameterType::KindOfParam,
            "VEHICLE".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&kindof).unwrap(),
        ScriptActionResult::Success
    );

    let mut partial = ScriptAction::new(ScriptActionType::TeamPartialUseCommandbutton);
    partial
        .add_parameter(Parameter::with_real(ParameterType::Real, 50.0))
        .unwrap();
    partial
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "teamAmerica".into(),
        ))
        .unwrap();
    partial
        .add_parameter(Parameter::with_string(
            ParameterType::CommandButton,
            "Command_Stop".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&partial).unwrap(),
        ScriptActionResult::Success
    );

    let queued = take_host_script_use_command_button_requests();
    assert!(
        queued.contains(&HostScriptUseCommandButtonRequest::TeamOnNearestEnemy {
            team: "teamAmerica".into(),
            button: "Command_Stop".into(),
        })
    );
    assert!(
        queued.contains(&HostScriptUseCommandButtonRequest::TeamOnNearestKindof {
            team: "teamAmerica".into(),
            button: "Command_Hijack".into(),
            kindof: "VEHICLE".into(),
        })
    );
    let partials = take_host_team_partial_command_button_requests();
    assert_eq!(partials.len(), 1);
    assert_eq!(partials[0].team, "teamAmerica");
    assert_eq!(partials[0].button, "Command_Stop");
    assert!((partials[0].percentage - 50.0).abs() < f32::EPSILON);
}

#[test]
fn idle_and_guard_for_framecount_queue_host_when_dual_world_empty() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_script_idle_requests();
    let _ = take_host_script_hunt_guard_requests();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut named_idle = ScriptAction::new(ScriptActionType::UnitIdleForFramecount);
    named_idle
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedRanger".into(),
        ))
        .unwrap();
    named_idle
        .add_parameter(Parameter::with_int(ParameterType::Int, 9))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&named_idle).unwrap(),
        ScriptActionResult::Pending(9.0)
    );

    let mut named_guard = ScriptAction::new(ScriptActionType::UnitGuardForFramecount);
    named_guard
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedRanger".into(),
        ))
        .unwrap();
    named_guard
        .add_parameter(Parameter::with_int(ParameterType::Int, 4))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&named_guard).unwrap(),
        ScriptActionResult::Pending(4.0)
    );

    let mut team_idle = ScriptAction::new(ScriptActionType::TeamIdleForFramecount);
    team_idle
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    team_idle
        .add_parameter(Parameter::with_int(ParameterType::Int, 12))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_idle).unwrap(),
        ScriptActionResult::Pending(12.0)
    );

    let mut team_guard = ScriptAction::new(ScriptActionType::TeamGuardForFramecount);
    team_guard
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_RangerSquad".into(),
        ))
        .unwrap();
    team_guard
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team_guard).unwrap(),
        ScriptActionResult::Pending(3.0)
    );

    assert_eq!(
        take_host_script_idle_requests(),
        vec![
            HostScriptIdleRequest::NamedStop {
                unit: "NamedRanger".into()
            },
            HostScriptIdleRequest::TeamStop {
                team: "USA_RangerSquad".into(),
                disband: false
            },
            HostScriptIdleRequest::TeamStop {
                team: "USA_RangerSquad".into(),
                disband: false
            },
        ]
    );
    assert_eq!(
        take_host_script_hunt_guard_requests(),
        vec![HostScriptHuntGuardRequest::NamedGuard {
            unit: "NamedRanger".into()
        }]
    );
}

#[test]
fn move_towards_nearest_queues_host_when_dual_world_empty() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_script_move_attack_requests();
    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut named = ScriptAction::new(ScriptActionType::UnitMoveTowardsNearestObjectType);
    named
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedScout".into(),
        ))
        .unwrap();
    named
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaCommandCenter".into(),
        ))
        .unwrap();
    named
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "HoldZone".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&named).unwrap(),
        ScriptActionResult::Success
    );

    let mut team = ScriptAction::new(ScriptActionType::TeamMoveTowardsNearestObjectType);
    team.add_parameter(Parameter::with_string(
        ParameterType::Team,
        "USA_Scout".into(),
    ))
    .unwrap();
    team.add_parameter(Parameter::with_string(
        ParameterType::ObjectType,
        "AmericaCommandCenter".into(),
    ))
    .unwrap();
    team.add_parameter(Parameter::with_string(
        ParameterType::TriggerArea,
        "HoldZone".into(),
    ))
    .unwrap();
    assert_eq!(
        dispatcher.execute_action(&team).unwrap(),
        ScriptActionResult::Success
    );

    assert_eq!(
        take_host_script_move_attack_requests(),
        vec![
            HostScriptMoveAttackRequest::NamedMoveTowardsNearest {
                unit: "NamedScout".into(),
                object_type: "AmericaCommandCenter".into(),
                trigger: "HoldZone".into(),
            },
            HostScriptMoveAttackRequest::TeamMoveTowardsNearest {
                team: "USA_Scout".into(),
                object_type: "AmericaCommandCenter".into(),
                trigger: "HoldZone".into(),
            },
        ]
    );
}

#[test]
fn team_wait_for_not_contained_uses_host_census_when_leftover_objects_missing() {
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    get_team_factory().lock().unwrap().reset();
    // Player path: leftover TeamFactory holds live host ids, leftover
    // OBJECT_REGISTRY is empty, so find_object_by_id misses. Census must
    // still use host snapshot contained_by (C++ evaluateTeamIsContained).
    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("USA_Contained"),
            AsciiString::default(),
            false,
            None,
        );
        let team = factory.create_team("USA_Contained").expect("leftover team");
        team.write().unwrap().add_member(7);
    }

    let mut contained = live_host_named_object("ContainedRanger", 7, true);
    contained.contained_by = 99;
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        objects: vec![contained],
        team_instance_ids: [("USA_Contained".into(), vec![7])].into_iter().collect(),
        ..Default::default()
    });

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut all = ScriptAction::new(ScriptActionType::TeamWaitForNotContainedAll);
    all.add_parameter(Parameter::with_string(
        ParameterType::Team,
        "USA_Contained".into(),
    ))
    .unwrap();
    assert_eq!(
        dispatcher.execute_action(&all).unwrap(),
        ScriptActionResult::Pending(1.0)
    );

    let mut partial = ScriptAction::new(ScriptActionType::TeamWaitForNotContainedPartial);
    partial
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "USA_Contained".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher.execute_action(&partial).unwrap(),
        ScriptActionResult::Pending(1.0)
    );

    let mut free = live_host_named_object("ContainedRanger", 7, true);
    free.contained_by = 0;
    crate::scripting::set_host_script_query_snapshot(crate::scripting::HostScriptQuerySnapshot {
        objects: vec![free],
        team_instance_ids: [("USA_Contained".into(), vec![7])].into_iter().collect(),
        ..Default::default()
    });
    assert_eq!(
        dispatcher.execute_action(&all).unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        dispatcher.execute_action(&partial).unwrap(),
        ScriptActionResult::Success
    );

    crate::scripting::clear_host_script_query_snapshot();
    get_team_factory().lock().unwrap().reset();
}
