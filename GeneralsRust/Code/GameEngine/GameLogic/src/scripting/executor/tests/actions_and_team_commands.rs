//! Behavior suite extracted from the original test module.
use super::*;

#[test]
fn executor_named_attack_named_leaves_group_and_dispatches_force_attack() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let attacker_id = 8450;
    let target_id = 8451;
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
        base.enter_group(&crate::ai::AIGroup::new(91));
        assert_eq!(base.get_group_id(), Some(91));
    }

    let attacker_id = attacker.get_id();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(attacker, Coord3D::new(12.0, 4.0, 0.0))
        .unwrap();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(target, Coord3D::new(20.0, 4.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ExecutorAttacker".to_string(), attacker_id)
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ExecutorVictim".to_string(), target_id)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedAttackNamed);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorAttacker".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorVictim".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_attack_named(&action).unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::ForceAttackObject,
            Some(target_id),
            None,
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

#[test]
fn executor_team_attack_team_dispatches_attack_team() {
    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("ExecutorAttackers"),
            AsciiString::default(),
            false,
            None,
        );
        factory.init_team(
            AsciiString::from("ExecutorVictims"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("ExecutorAttackers")
            .expect("attacker team should be created");
        factory
            .create_team("ExecutorVictims")
            .expect("victim team should be created");
    }

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let attacker_id = 8460;
    let attacker = crate::object_manager::GameObjectInstance::new(
        attacker_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test attacker instance");

    {
        let instance = &attacker;
        instance
            .base()
            .write()
            .unwrap()
            .set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
                commands: Arc::clone(&commands),
                locomotors: Arc::clone(&locomotors),
            }))));
    }

    let attacker_id = attacker.get_id();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(attacker, Coord3D::new(14.0, 4.0, 0.0))
        .unwrap();
    {
        let factory = get_team_factory();
        let mut factory_guard = factory.lock().unwrap();
        factory_guard
            .find_team("ExecutorAttackers")
            .unwrap()
            .write()
            .unwrap()
            .add_member(attacker_id);
    }

    let mut action = ScriptAction::new(ScriptActionType::TeamAttackTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorAttackers".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorVictims".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_team_attack_team(&action).unwrap();

    assert_eq!(locomotors.lock().unwrap().len(), 0);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::AttackTeam,
            None,
            Some("ExecutorVictims".to_string()),
            -1,
            CommandSourceType::FromScript,
        )]
    );
}

#[test]
fn executor_named_attack_area_leaves_group_and_selects_normal_locomotor() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_terrain_logic().write().unwrap().reset();

    get_terrain_logic().write().unwrap().add_trigger_area(
        crate::polygon_trigger::PolygonTrigger::new(
            8470,
            AsciiString::from("ExecutorAttackArea"),
            vec![
                crate::common::ICoord3D::new(0, 0, 0),
                crate::common::ICoord3D::new(20, 0, 0),
                crate::common::ICoord3D::new(20, 20, 0),
                crate::common::ICoord3D::new(0, 20, 0),
            ],
        ),
    );

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let attacker_id = 8471;
    let attacker = crate::object_manager::GameObjectInstance::new(
        attacker_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test attacker instance");

    {
        let __base_arc = attacker.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
        }))));
        base.enter_group(&crate::ai::AIGroup::new(92));
        assert_eq!(base.get_group_id(), Some(92));
    }

    let attacker_id = attacker.get_id();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(attacker, Coord3D::new(4.0, 4.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ExecutorAreaAttacker".to_string(), attacker_id)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedAttackArea);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorAreaAttacker".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "ExecutorAttackArea".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_attack_area(&action).unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        commands.lock().unwrap()[0],
        (
            AiCommandType::AttackArea,
            None,
            None,
            0,
            CommandSourceType::FromScript,
        )
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

#[test]
fn executor_named_attack_team_validates_team_and_sets_max_shots() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_team_factory().lock().unwrap().reset();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("ExecutorTargetTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("ExecutorTargetTeam")
            .expect("target team should be created");
    }

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let attacker_id = 8480;
    let attacker = crate::object_manager::GameObjectInstance::new(
        attacker_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test attacker instance");

    {
        let __base_arc = attacker.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
        }))));
        base.enter_group(&crate::ai::AIGroup::new(93));
        assert_eq!(base.get_group_id(), Some(93));
    }

    let attacker_id = attacker.get_id();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(attacker, Coord3D::new(8.0, 4.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ExecutorTeamAttacker".to_string(), attacker_id)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedAttackTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorTeamAttacker".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorTargetTeam".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_attack_team(&action).unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::AttackTeam,
            None,
            Some("ExecutorTargetTeam".to_string()),
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

#[test]
fn executor_team_attack_named_ignores_stale_target_tracker_id() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_team_factory().lock().unwrap().reset();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("ExecutorSourceTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("ExecutorSourceTeam")
            .expect("source team should be created");
    }

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let member_id = 8490;
    let member = crate::object_manager::GameObjectInstance::new(
        member_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test team member instance");

    {
        let instance = &member;
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
        .register_object_instance(member, Coord3D::new(8.0, 8.0, 0.0))
        .unwrap();
    get_team_factory()
        .lock()
        .unwrap()
        .find_team("ExecutorSourceTeam")
        .unwrap()
        .write()
        .unwrap()
        .add_member(member_id);
    get_named_object_tracker()
        .register_named_object("MissingExecutorVictim".to_string(), 8491)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::TeamAttackNamed);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorSourceTeam".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MissingExecutorVictim".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_team_attack_named(&action).unwrap();

    assert!(commands.lock().unwrap().is_empty());
    assert!(locomotors.lock().unwrap().is_empty());
}

#[test]
fn executor_named_hunt_selects_normal_locomotor_before_hunt() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let hunter_id = 8495;
    let hunter = crate::object_manager::GameObjectInstance::new(
        hunter_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test hunter instance");

    {
        let __base_arc = hunter.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
        }))));
        base.enter_group(&crate::ai::AIGroup::new(95));
        assert_eq!(base.get_group_id(), Some(95));
    }

    let hunter_id = hunter.get_id();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(hunter, Coord3D::new(11.0, 6.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ExecutorHunter".to_string(), hunter_id)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedHunt);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorHunter".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_hunt(&action).unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::Hunt,
            None,
            None,
            0,
            CommandSourceType::FromScript,
        )]
    );
    assert_eq!(
        get_object_manager()
            .read()
            .unwrap()
            .with_object(hunter_id, |o| o
                .base()
                .read()
                .ok()
                .and_then(|b| b.get_group_id()))
            .flatten(),
        Some(95)
    );
}

#[test]
fn executor_named_stop_dispatches_direct_ai_without_player_owner() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let stopper_id = 8496;
    let stopper = crate::object_manager::GameObjectInstance::new(
        stopper_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test stopper instance");

    {
        let instance = &stopper;
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
        .register_object_instance(stopper, Coord3D::new(13.0, 6.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ExecutorStopper".to_string(), stopper_id)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedStop);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorStopper".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_stop(&action).unwrap();

    assert!(locomotors.lock().unwrap().is_empty());
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::Idle,
            None,
            None,
            0,
            CommandSourceType::FromScript,
        )]
    );
}

#[test]
fn executor_named_guard_leaves_group_selects_locomotor_and_sets_guard_mode() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let guard_id = 8500;
    let guard = crate::object_manager::GameObjectInstance::new(
        guard_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test guard instance");

    {
        let __base_arc = guard.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
        }))));
        base.enter_group(&crate::ai::AIGroup::new(94));
        assert_eq!(base.get_group_id(), Some(94));
    }

    let guard_id = guard.get_id();
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(guard, Coord3D::new(9.0, 5.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ExecutorGuard".to_string(), guard_id)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::NamedGuard);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorGuard".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_guard(&action).unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::GuardPosition,
            None,
            None,
            GuardMode::Normal.as_i32(),
            CommandSourceType::FromScript,
        )]
    );
    assert_eq!(
        get_object_manager()
            .read()
            .unwrap()
            .with_object(guard_id, |o| o
                .base()
                .read()
                .ok()
                .and_then(|b| b.get_group_id()))
            .flatten(),
        None
    );
}

#[test]
fn executor_team_guard_dispatches_direct_ai_without_player_owner() {
    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("ExecutorGuardTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("ExecutorGuardTeam")
            .expect("guard team should be created");
    }

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let member_id = 8510;
    let member = crate::object_manager::GameObjectInstance::new(
        member_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test guard team member instance");

    {
        let instance = &member;
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
        .register_object_instance(member, Coord3D::new(10.0, 10.0, 0.0))
        .unwrap();
    get_team_factory()
        .lock()
        .unwrap()
        .find_team("ExecutorGuardTeam")
        .unwrap()
        .write()
        .unwrap()
        .add_member(member_id);

    let mut action = ScriptAction::new(ScriptActionType::TeamGuard);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorGuardTeam".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_team_guard(&action).unwrap();

    assert!(locomotors.lock().unwrap().is_empty());
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::GuardPosition,
            None,
            None,
            GuardMode::Normal.as_i32(),
            CommandSourceType::FromScript,
        )]
    );
}

#[test]
fn executor_team_guard_for_framecount_dispatches_idle_like_cxx_switch() {
    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("ExecutorTimedGuardTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("ExecutorTimedGuardTeam")
            .expect("timed guard team should be created");
    }

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let member_id = 8515;
    let member = crate::object_manager::GameObjectInstance::new(
        member_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test timed guard team member instance");

    {
        let instance = &member;
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
        .register_object_instance(member, Coord3D::new(10.0, 12.0, 0.0))
        .unwrap();
    get_team_factory()
        .lock()
        .unwrap()
        .find_team("ExecutorTimedGuardTeam")
        .unwrap()
        .write()
        .unwrap()
        .add_member(member_id);

    let mut action = ScriptAction::new(ScriptActionType::TeamGuardForFramecount);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorTimedGuardTeam".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_int(ParameterType::Int, 7))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let result = dispatcher.execute_action(&action).unwrap();

    assert_eq!(result, ScriptActionResult::Pending(7.0));
    assert!(locomotors.lock().unwrap().is_empty());
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::Idle,
            None,
            None,
            0,
            CommandSourceType::FromScript,
        )]
    );
}

#[test]
fn executor_team_guard_object_ignores_stale_target_tracker_id() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_team_factory().lock().unwrap().reset();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("ExecutorObjectGuardTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("ExecutorObjectGuardTeam")
            .expect("object guard team should be created");
    }

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let member_id = 8520;
    let member = crate::object_manager::GameObjectInstance::new(
        member_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test object guard team member instance");

    {
        let instance = &member;
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
        .register_object_instance(member, Coord3D::new(11.0, 11.0, 0.0))
        .unwrap();
    get_team_factory()
        .lock()
        .unwrap()
        .find_team("ExecutorObjectGuardTeam")
        .unwrap()
        .write()
        .unwrap()
        .add_member(member_id);
    get_named_object_tracker()
        .register_named_object("MissingGuardTarget".to_string(), 8521)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::TeamGuardObject);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorObjectGuardTeam".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MissingGuardTarget".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_team_guard_object(&action).unwrap();

    assert!(commands.lock().unwrap().is_empty());
    assert!(locomotors.lock().unwrap().is_empty());
}

#[test]
fn executor_team_stop_and_disband_marks_members_recruitable_and_merges_default_team() {
    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();
    player_list().write().unwrap().clear();

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("ExecutorDisbandSource"),
            AsciiString::default(),
            false,
            None,
        );
        factory.init_team(
            AsciiString::from("ExecutorDefaultTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("ExecutorDisbandSource")
            .expect("source team should be created");
        factory
            .create_team("ExecutorDefaultTeam")
            .expect("default team should be created");
    }

    let default_team = get_team_factory()
        .lock()
        .unwrap()
        .find_team("ExecutorDefaultTeam")
        .expect("default team exists");
    let source_team = get_team_factory()
        .lock()
        .unwrap()
        .find_team("ExecutorDisbandSource")
        .expect("source team exists");

    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    player
        .write()
        .unwrap()
        .set_default_team(Some(default_team.clone()));
    player_list().write().unwrap().add_player(player);
    source_team
        .write()
        .unwrap()
        .set_controlling_player_id(Some(0));

    let commands = Arc::new(Mutex::new(Vec::new()));
    let recruitable = Arc::new(Mutex::new(Vec::new()));
    let member_id = 8530;
    let member = crate::object_manager::GameObjectInstance::new(
        member_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test member instance");
    {
        let instance = &member;
        instance
            .base()
            .write()
            .unwrap()
            .set_ai_update_interface(Some(Arc::new(Mutex::new(RecruitableRecordingAi {
                commands: Arc::clone(&commands),
                recruitable: Arc::clone(&recruitable),
            }))));
    }

    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(member, Coord3D::new(15.0, 15.0, 0.0))
        .unwrap();
    source_team.write().unwrap().add_member(member_id);

    let mut action = ScriptAction::new(ScriptActionType::TeamStopAndDisband);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorDisbandSource".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_team_stop_and_disband(&action).unwrap();

    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::Idle,
            None,
            None,
            0,
            CommandSourceType::FromScript,
        )]
    );
    assert_eq!(*recruitable.lock().unwrap(), vec![true]);
    assert!(default_team.read().unwrap().has_member(member_id));
    assert!(!source_team.read().unwrap().has_member(member_id));
    let team_ok = get_object_manager()
        .read()
        .unwrap()
        .with_object(member_id, |o| {
            o.base()
                .read()
                .ok()
                .and_then(|b| b.get_team())
                .map(|t| Arc::ptr_eq(&t, &default_team))
                .unwrap_or(false)
        })
        .unwrap_or(false);
    assert!(team_ok);
}

#[test]
fn executor_team_execute_sequential_script_requires_script_before_idle() {
    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();

    let script_engine_lock = get_script_engine();
    {
        let mut engine_guard = script_engine_lock.write().unwrap();
        *engine_guard = Some(ScriptEngine::new().expect("script engine"));
    }

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("ExecutorSequentialTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("ExecutorSequentialTeam")
            .expect("team should be created");
    }

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let member_id = 8540;
    let member = crate::object_manager::GameObjectInstance::new(
        member_id,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test member instance");
    {
        let instance = &member;
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
        .register_object_instance(member, Coord3D::new(16.0, 16.0, 0.0))
        .unwrap();
    get_team_factory()
        .lock()
        .unwrap()
        .find_team("ExecutorSequentialTeam")
        .unwrap()
        .write()
        .unwrap()
        .add_member(member_id);

    let mut action = ScriptAction::new(ScriptActionType::TeamExecuteSequentialScript);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorSequentialTeam".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            "MissingSequentialScript".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher
        .do_team_execute_sequential_script(&action)
        .unwrap();

    assert!(
        commands.lock().unwrap().is_empty(),
        "C++ doTeamStartSequentialScript returns before groupIdle when the script cannot be resolved"
    );
    assert!(locomotors.lock().unwrap().is_empty());
}

#[test]
fn executor_team_stop_sequential_script_requires_live_team() {
    get_team_factory().lock().unwrap().reset();

    let script_engine_lock = get_script_engine();
    {
        let mut engine_guard = script_engine_lock.write().unwrap();
        *engine_guard = Some(ScriptEngine::new().expect("script engine"));
        let engine = engine_guard.as_mut().unwrap();
        let mut missing_team_script = SequentialScript::new();
        missing_team_script.team_to_exec_on = Some("MissingSequentialTeam".to_string());
        engine.append_sequential_script(missing_team_script);
    }

    let mut action = ScriptAction::new(ScriptActionType::TeamStopSequentialScript);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "MissingSequentialTeam".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_team_stop_sequential_script(&action).unwrap();

    assert!(
        script_engine_lock
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .has_active_sequential_script_for_team("MissingSequentialTeam"),
        "C++ doTeamStopSequentialScript returns before removal when the team cannot be resolved"
    );

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from("MissingSequentialTeam"),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team("MissingSequentialTeam")
            .expect("team should be created");
    }

    dispatcher.do_team_stop_sequential_script(&action).unwrap();

    assert!(
        !script_engine_lock
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .has_active_sequential_script_for_team("MissingSequentialTeam")
    );
}

#[test]
fn condition_player_destroyed_n_buildings_player_matches_cxx_todo_false() {
    let mut condition = Condition::new(ConditionType::PlayerDestroyedNBuildingsPlayer);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "MissingDestroyedBuildingsPlayer".to_string(),
        ))
        .unwrap();
    condition
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "MissingDestroyedBuildingsOpponent".to_string(),
        ))
        .unwrap();

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "C++ resolves the side parameters, ignores N, and returns FALSE because the helper is unimplemented"
    );
}

#[test]
fn condition_mission_attempts_ignores_parameters_like_cxx_stub() {
    let mut condition = Condition::new(ConditionType::MissionAttempts);
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "C++ evaluateMissionAttempts does not read parameters and always returns false"
    );
}

#[test]
fn condition_player_has_credits_compares_threshold_to_player_money_like_cxx() {
    // C++ ScriptEngine.cpp:4414-4416 template [INT, COMPARISON, SIDE]
    // ScriptConditions.cpp:952-966 compares credits param to countMoney().
    player_list().write().unwrap().clear();
    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    {
        let mut player_guard = player.write().unwrap();
        player_guard.set_display_name("CreditsExecutorPlayer");
        player_guard.get_money_mut().set_money(1000);
    }
    player_list().write().unwrap().add_player(player);

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
            "CreditsExecutorPlayer".to_string(),
        ))
        .unwrap();

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::True,
        "C++ evaluates threshold < player's credits, not player's credits < threshold"
    );

    let mut missing = Condition::new(ConditionType::PlayerHasCredits);
    missing
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    missing
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 2))
        .unwrap();
    missing
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "NoSuchCreditsExecutorPlayer".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut missing).unwrap(),
        ScriptConditionResult::False,
        "C++ evaluatePlayerHasCredits returns false when playerFromParam fails"
    );
}

#[test]
fn player_conditions_use_host_census_instead_of_stale_leftover_player() {
    // Live host OBJECT_REGISTRY is empty; leftover Player money/energy/objects
    // are unsynced. C++ ScriptConditions read the same Player as the HUD.
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    crate::scripting::clear_host_script_query_snapshot();
    player_list().write().unwrap().clear();

    let leftover = Arc::new(RwLock::new(crate::player::Player::new(0)));
    {
        let mut player_guard = leftover.write().unwrap();
        player_guard.set_display_name("PlyrAmerica");
        player_guard.get_money_mut().set_money(0);
    }
    player_list().write().unwrap().add_player(leftover);

    let mut snap = crate::scripting::HostScriptQuerySnapshot::default();
    let mut template_counts = std::collections::HashMap::new();
    template_counts.insert("americawarfactory".into(), 1);
    template_counts.insert("americainfantryranger".into(), 2);
    template_counts.insert("americacommandcenter".into(), 1);
    let mut template_counts_ignore_dead = template_counts.clone();
    template_counts.insert("americatankcrusader".into(), 1);
    snap.player_census.insert(
        "plyramerica".into(),
        crate::scripting::HostScriptPlayerCensus {
            money: 2500,
            energy_production: 10,
            energy_consumption: 20,
            power_sabotaged: false,
            has_any_objects: true,
            has_any_build_facility: true,
            building_count: 3,
            faction_building_count: 2,
            template_counts,
            template_counts_ignore_dead,
            ..Default::default()
        },
    );
    crate::scripting::set_host_script_query_snapshot(snap);

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

    let mut credits = Condition::new(ConditionType::PlayerHasCredits);
    credits
        .add_parameter(Parameter::with_int(ParameterType::Int, 500))
        .unwrap();
    credits
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 0))
        .unwrap();
    credits
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut credits).unwrap(),
        ScriptConditionResult::True,
        "PLAYER_HAS_CREDITS must compare against host money, not leftover 0"
    );

    let mut power = Condition::new(ConditionType::PlayerHasPower);
    power
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut power).unwrap(),
        ScriptConditionResult::False,
        "PLAYER_HAS_POWER is false when host consumption exceeds production"
    );

    let mut no_power = Condition::new(ConditionType::PlayerHasNoPower);
    no_power
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut no_power).unwrap(),
        ScriptConditionResult::True,
        "PLAYER_HAS_NO_POWER is the inverse of host hasSufficientPower"
    );

    let mut destroyed = Condition::new(ConditionType::PlayerAllDestroyed);
    destroyed
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut destroyed).unwrap(),
        ScriptConditionResult::False,
        "PLAYER_ALL_DESTROYED is false while the host player still has objects"
    );

    let mut facilities = Condition::new(ConditionType::PlayerAllBuildfacilitiesDestroyed);
    facilities
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut facilities).unwrap(),
        ScriptConditionResult::False,
        "PLAYER_ALL_BUILDFACILITIES_DESTROYED is false while a factory remains"
    );

    let mut few = Condition::new(ConditionType::PlayerHasNOrFewerBuildings);
    few.add_parameter(Parameter::with_string(
        ParameterType::Side,
        "PlyrAmerica".to_string(),
    ))
    .unwrap();
    few.add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut few).unwrap(),
        ScriptConditionResult::False,
        "N_OR_FEWER_BUILDINGS 2 is false when host has 3 structures"
    );

    let mut enough = Condition::new(ConditionType::PlayerHasNOrFewerBuildings);
    enough
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    enough
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut enough).unwrap(),
        ScriptConditionResult::True,
        "N_OR_FEWER_BUILDINGS 3 is true when host has 3 structures"
    );

    let mut built = Condition::new(ConditionType::BuiltByPlayer);
    built
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaWarFactory".to_string(),
        ))
        .unwrap();
    built
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut built).unwrap(),
        ScriptConditionResult::True,
        "BUILT_BY_PLAYER must see host War Factory census, not leftover 0"
    );

    let mut has_rangers = Condition::new(ConditionType::PlayerHasObjectComparison);
    has_rangers
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    has_rangers
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 4))
        .unwrap();
    has_rangers
        .add_parameter(Parameter::with_int(ParameterType::Int, 0))
        .unwrap();
    has_rangers
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaInfantryRanger".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut has_rangers).unwrap(),
        ScriptConditionResult::True,
        "HAS_OBJECT_COMPARISON > 0 Rangers must use host census"
    );

    let mut few_cc = Condition::new(ConditionType::PlayerHasObjectComparison);
    few_cc
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            "PlyrAmerica".to_string(),
        ))
        .unwrap();
    few_cc
        .add_parameter(Parameter::with_int(ParameterType::Comparison, 0))
        .unwrap();
    few_cc
        .add_parameter(Parameter::with_int(ParameterType::Int, 1))
        .unwrap();
    few_cc
        .add_parameter(Parameter::with_string(
            ParameterType::ObjectType,
            "AmericaCommandCenter".to_string(),
        ))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut few_cc).unwrap(),
        ScriptConditionResult::False,
        "HAS fewer than 1 Command Center is false while host CC stands"
    );

    let mut lost = Condition::new(ConditionType::PlayerLostObjectType);
    lost.add_parameter(Parameter::with_string(
        ParameterType::Side,
        "PlyrAmerica".to_string(),
    ))
    .unwrap();
    lost.add_parameter(Parameter::with_string(
        ParameterType::ObjectType,
        "AmericaTankCrusader".to_string(),
    ))
    .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut lost).unwrap(),
        ScriptConditionResult::False,
        "LOST first census sample stores ignoreDead count and is not yet lost"
    );
    {
        let mut snap = crate::scripting::HostScriptQuerySnapshot::default();
        let mut template_counts = std::collections::HashMap::new();
        template_counts.insert("americatankcrusader".into(), 1);
        snap.player_census.insert(
            "plyramerica".into(),
            crate::scripting::HostScriptPlayerCensus {
                template_counts,
                template_counts_ignore_dead: std::collections::HashMap::new(),
                ..Default::default()
            },
        );
        crate::scripting::set_host_script_query_snapshot(snap);
    }
    assert_eq!(
        evaluator.evaluate_condition(&mut lost).unwrap(),
        ScriptConditionResult::True,
        "LOST fires when ignoreDead census drops vs ScriptEngine cache"
    );

    crate::scripting::clear_host_script_query_snapshot();
    player_list().write().unwrap().clear();
}

#[test]
fn named_destroyed_false_if_name_never_existed_like_cxx() {
    // C++ ScriptConditions::evaluateNamedUnitDestroyed (ScriptConditions.cpp:274-286)
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    get_named_object_tracker().clear().unwrap();
    crate::scripting::clear_host_script_query_snapshot();

    let mut condition = Condition::new(ConditionType::NamedDestroyed);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NeverSpawnedHero".into(),
        ))
        .unwrap();
    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "C++ evaluateNamedUnitDestroyed returns false when the name never existed"
    );
}

#[test]
fn named_destroyed_and_dying_use_effectively_dead_while_object_exists_like_cxx() {
    // C++ ScriptConditions.cpp:274-318 — existing unit uses isEffectivelyDead();
    // NAMED_DYING is false once the object is gone.
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    get_named_object_tracker().clear().unwrap();

    const ID: u32 = 0x5C01_D571;
    let obj = Arc::new(RwLock::new(crate::object::Object::new_test(ID, 100.0)));
    obj.write().unwrap().set_effectively_dead(true);
    crate::object::registry::OBJECT_REGISTRY.register_object(ID, &obj);
    get_named_object_tracker()
        .register_named_object("DyingHero".to_string(), ID)
        .unwrap();

    let mut destroyed = Condition::new(ConditionType::NamedDestroyed);
    destroyed
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "DyingHero".into(),
        ))
        .unwrap();
    let mut dying = Condition::new(ConditionType::NamedDying);
    dying
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "DyingHero".into(),
        ))
        .unwrap();

    let mut evaluator = ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
    assert_eq!(
        evaluator.evaluate_condition(&mut destroyed).unwrap(),
        ScriptConditionResult::True,
        "C++ evaluateNamedUnitDestroyed uses isEffectivelyDead while the object exists"
    );
    assert_eq!(
        evaluator.evaluate_condition(&mut dying).unwrap(),
        ScriptConditionResult::True,
        "C++ evaluateNamedUnitDying is isEffectivelyDead while the object exists"
    );

    crate::object::registry::OBJECT_REGISTRY.unregister_object(ID);
    drop(obj);
    get_named_object_tracker().unregister_object(ID).unwrap();

    assert_eq!(
        evaluator.evaluate_condition(&mut dying).unwrap(),
        ScriptConditionResult::False,
        "C++ evaluateNamedUnitDying is false once the object is gone"
    );
    get_named_object_tracker().clear().unwrap();
}

#[test]
fn active_script_counter_and_victory_reenter_without_relocking_the_global_engine() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    let prior_input = TheGameLogic::is_input_enabled();
    let prior_victory = TheVictoryConditions::is_local_allied_victory();
    TheVictoryConditions::set_local_allied_victory(false);

    let completed = with_script_engine_mut(|engine| {
        // `with_script_engine_mut` holds the global engine lock and
        // installs the active scoped engine.  The two calls below must
        // use that active engine immediately; re-locking the global
        // handle here would self-deadlock during a normal script update.
        engine
            .set_counter("ActiveExecutorCounterReentry", 3)
            .expect("counter should be allocated");

        let mut counter = Condition::new(ConditionType::Counter);
        counter
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "ActiveExecutorCounterReentry".to_string(),
            ))
            .expect("counter parameter");
        counter
            .add_parameter(Parameter::with_int(
                ParameterType::Comparison,
                ComparisonType::Equal as i32,
            ))
            .expect("comparison parameter");
        counter
            .add_parameter(Parameter::with_int(ParameterType::Int, 3))
            .expect("value parameter");

        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            evaluator.evaluate_condition(&mut counter).unwrap(),
            ScriptConditionResult::True
        );

        let victory = ScriptAction::new(ScriptActionType::Victory);
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            dispatcher.execute_action(&victory).unwrap(),
            ScriptActionResult::Success
        );
        assert!(engine.is_game_ending());
        assert!(
            engine.is_campaign_victorious(),
            "C++ ScriptActions.cpp:208 TheCampaignManager->SetVictorious(TRUE)"
        );
    });

    assert_eq!(completed, Some(()));
    assert!(
        !TheGameLogic::is_input_enabled(),
        "C++ Victory disables local input before starting the end-game timer"
    );
    assert!(
        !TheVictoryConditions::is_local_allied_victory(),
        "C++ doVictory does not touch TheVictoryConditions"
    );

    TheGameLogic::set_input_enabled(prior_input);
    TheVictoryConditions::set_local_allied_victory(prior_victory);

    with_script_engine_mut(|engine| engine.set_campaign_victorious(false));
}

#[test]
fn do_defeat_clears_campaign_victorious_flag() {
    // C++ ScriptActions.cpp:231-232 TheCampaignManager->SetVictorious(FALSE)
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");
    let prior_input = TheGameLogic::is_input_enabled();
    TheGameLogic::set_input_enabled(true);

    let completed = with_script_engine_mut(|engine| {
        engine.set_campaign_victorious(true);
        let defeat = ScriptAction::new(ScriptActionType::Defeat);
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            dispatcher.execute_action(&defeat).unwrap(),
            ScriptActionResult::Success
        );
        assert!(!engine.is_campaign_victorious());
        assert!(engine.is_game_ending());
    });
    assert_eq!(completed, Some(()));
    TheGameLogic::set_input_enabled(prior_input);
}

#[test]
fn do_victory_creates_victorious_window_layout() {
    // C++ ScriptActions.cpp:196-209 winCreateFromScript("Menus/Victorious.wnd")
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");
    let prior_input = TheGameLogic::is_input_enabled();
    TheGameLogic::set_input_enabled(true);

    let completed = with_script_engine_mut(|engine| {
        engine.set_shown_mp_local_defeat_window(false);
        engine.close_windows(false);
        let victory = ScriptAction::new(ScriptActionType::Victory);
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            dispatcher.execute_action(&victory).unwrap(),
            ScriptActionResult::Success
        );
        assert_eq!(
            engine.current_win_lose_window().as_deref(),
            Some("Menus/Victorious.wnd")
        );
        assert!(engine.is_game_ending());
    });
    assert_eq!(completed, Some(()));
    TheGameLogic::set_input_enabled(prior_input);
}

#[test]
fn do_defeat_creates_defeat_window_layout() {
    // C++ ScriptActions.cpp:220-229 winCreateFromScript("Menus/Defeat.wnd")
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");
    let prior_input = TheGameLogic::is_input_enabled();
    TheGameLogic::set_input_enabled(true);

    let completed = with_script_engine_mut(|engine| {
        engine.set_shown_mp_local_defeat_window(false);
        engine.close_windows(false);
        let defeat = ScriptAction::new(ScriptActionType::Defeat);
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            dispatcher.execute_action(&defeat).unwrap(),
            ScriptActionResult::Success
        );
        assert_eq!(
            engine.current_win_lose_window().as_deref(),
            Some("Menus/Defeat.wnd")
        );
    });
    assert_eq!(completed, Some(()));
    TheGameLogic::set_input_enabled(prior_input);
}

#[test]
fn do_local_defeat_creates_local_defeat_window_layout() {
    // C++ ScriptActions.cpp:244-247 winCreateFromScript("Menus/LocalDefeat.wnd")
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    let completed = with_script_engine_mut(|engine| {
        engine.set_shown_mp_local_defeat_window(false);
        engine.close_windows(false);
        let local_defeat = ScriptAction::new(ScriptActionType::Localdefeat);
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            dispatcher.execute_action(&local_defeat).unwrap(),
            ScriptActionResult::Success
        );
        assert_eq!(
            engine.current_win_lose_window().as_deref(),
            Some("Menus/LocalDefeat.wnd")
        );
        assert!(engine.has_shown_mp_local_defeat_window());
    });
    assert_eq!(completed, Some(()));
}

#[test]
fn active_world_actions_clone_the_handler_before_host_callback_reentry() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    let calls = Arc::new(Mutex::new(Vec::new()));
    let previous_handler = {
        let global = get_script_engine();
        let mut global = global.write().expect("script engine global lock");
        let engine = global.as_mut().expect("script engine should initialize");
        let previous = engine.action_handler();
        engine.set_action_handler(Some(Arc::new(ReentrantWorldActionHandler {
            calls: Arc::clone(&calls),
        })));
        previous
    };

    let completed = with_script_engine_mut(|engine| {
        engine
            .set_counter("WorldHandlerImmediateReentry", 0)
            .expect("reentry counter");
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

        let mut fullscreen_movie = ScriptAction::new(ScriptActionType::MoviePlayFullscreen);
        fullscreen_movie
            .add_parameter(Parameter::with_string(
                ParameterType::Movie,
                "campaign_intro.bik".to_string(),
            ))
            .expect("movie parameter");
        assert_eq!(
            dispatcher
                .execute_action(&fullscreen_movie)
                .expect("movie action"),
            ScriptActionResult::Success
        );

        assert_eq!(
            dispatcher
                .execute_action(&ScriptAction::new(ScriptActionType::RadarForceEnable))
                .expect("radar action"),
            ScriptActionResult::Success
        );

        let mut timer = ScriptAction::new(ScriptActionType::DisplayCountdownTimer);
        timer
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "CampaignTimer".to_string(),
            ))
            .expect("timer name parameter");
        timer
            .add_parameter(Parameter::with_string(
                ParameterType::LocalizedText,
                "GUI:Countdown".to_string(),
            ))
            .expect("timer text parameter");
        assert_eq!(
            dispatcher.execute_action(&timer).expect("timer action"),
            ScriptActionResult::Success
        );

        assert_eq!(
            dispatcher
                .execute_action(&ScriptAction::new(ScriptActionType::FreezeTime))
                .expect("freeze action"),
            ScriptActionResult::Success
        );
        assert_eq!(
            dispatcher
                .execute_action(&ScriptAction::new(ScriptActionType::UnfreezeTime))
                .expect("unfreeze action"),
            ScriptActionResult::Success
        );

        let mut visual_speed = ScriptAction::new(ScriptActionType::SetVisualSpeedMultiplier);
        visual_speed
            .add_parameter(Parameter::with_int(ParameterType::Int, 2))
            .expect("visual speed parameter");
        assert_eq!(
            dispatcher
                .execute_action(&visual_speed)
                .expect("visual speed action"),
            ScriptActionResult::Success
        );

        let mut weather = ScriptAction::new(ScriptActionType::ShowWeather);
        weather
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 0))
            .expect("weather parameter");
        assert_eq!(
            dispatcher.execute_action(&weather).expect("weather action"),
            ScriptActionResult::Success
        );

        assert_eq!(
            engine
                .get_counter("WorldHandlerImmediateReentry")
                .expect("reentry counter should remain allocated")
                .value,
            7,
            "each host callback re-enters before its script action returns"
        );
    });

    {
        let global = get_script_engine();
        let mut global = global.write().expect("script engine global lock");
        global
            .as_mut()
            .expect("script engine should initialize")
            .set_action_handler(previous_handler);
    }

    assert_eq!(completed, Some(()));
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            "movie:campaign_intro.bik",
            "radar:true",
            "timer:CampaignTimer:GUI:Countdown:true",
            "freeze",
            "unfreeze",
            "speed:2",
            "weather:false",
        ]
    );
}

#[test]
fn active_player_display_actions_clone_handler_before_reentry() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    let calls = Arc::new(Mutex::new(Vec::new()));
    let previous_handler = {
        let global = get_script_engine();
        let mut global = global.write().expect("script engine global lock");
        let engine = global.as_mut().expect("script engine should initialize");
        let previous = engine.action_handler();
        engine.set_action_handler(Some(Arc::new(ReentrantWorldActionHandler {
            calls: Arc::clone(&calls),
        })));
        previous
    };

    let completed = with_script_engine_mut(|engine| {
        engine
            .set_counter("WorldHandlerImmediateReentry", 0)
            .expect("reentry counter");
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

        let mut display = ScriptAction::new(ScriptActionType::DisplayText);
        display
            .add_parameter(Parameter::with_string(
                ParameterType::LocalizedText,
                "GUI:CampaignMessage".to_string(),
            ))
            .expect("display text parameter");
        assert_eq!(
            dispatcher.execute_action(&display).expect("display action"),
            ScriptActionResult::Success
        );

        let mut music = ScriptAction::new(ScriptActionType::MusicSetTrack);
        music
            .add_parameter(Parameter::with_string(
                ParameterType::Music,
                "CampaignCombat".to_string(),
            ))
            .expect("music parameter");
        music
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .expect("fade out parameter");
        music
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 0))
            .expect("fade in parameter");
        assert_eq!(
            dispatcher.execute_action(&music).expect("music action"),
            ScriptActionResult::Success
        );

        assert_eq!(engine.get_current_track_name(), "CampaignCombat");
        assert_eq!(
            engine
                .get_counter("WorldHandlerImmediateReentry")
                .expect("reentry counter should remain allocated")
                .value,
            2,
            "each callback re-enters before its script action returns"
        );
    });

    {
        let global = get_script_engine();
        let mut global = global.write().expect("script engine global lock");
        global
            .as_mut()
            .expect("script engine should initialize")
            .set_action_handler(previous_handler);
    }

    assert_eq!(completed, Some(()));
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            "text:GUI:CampaignMessage",
            "music:CampaignCombat:true:false",
        ]
    );
}

#[test]
fn active_camera_actions_snapshot_handler_and_mutate_fade_without_relocking() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    let calls = Arc::new(Mutex::new(Vec::new()));
    let previous_handler = {
        let global = get_script_engine();
        let mut global = global.write().expect("script engine global lock");
        let engine = global.as_mut().expect("script engine should initialize");
        let previous = engine.action_handler();
        engine.set_action_handler(Some(Arc::new(ReentrantWorldActionHandler {
            calls: Arc::clone(&calls),
        })));
        previous
    };

    let completed = with_script_engine_mut(|engine| {
        engine
            .set_counter("WorldHandlerImmediateReentry", 0)
            .expect("reentry counter");
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

        let mut zoom = ScriptAction::new(ScriptActionType::ZoomCamera);
        for value in [25.0, 1.5, 0.25, 0.5] {
            zoom.add_parameter(Parameter::with_real(ParameterType::Real, value))
                .expect("zoom parameter");
        }
        assert_eq!(
            dispatcher.execute_action(&zoom).expect("zoom action"),
            ScriptActionResult::Success
        );

        let mut fade = ScriptAction::new(ScriptActionType::CameraFadeAdd);
        for value in [0.2, 0.8] {
            fade.add_parameter(Parameter::with_real(ParameterType::Real, value))
                .expect("fade level parameter");
        }
        for value in [0, 3, 2] {
            fade.add_parameter(Parameter::with_int(ParameterType::Int, value))
                .expect("fade frame parameter");
        }
        assert_eq!(
            dispatcher.execute_action(&fade).expect("fade action"),
            ScriptActionResult::Success
        );
        assert_eq!(engine.get_fade(), TFade::Add);
        assert!(
            (engine.get_fade_value() - 0.8).abs() < f32::EPSILON,
            "C++ advances a zero-increase fade immediately into its hold value"
        );
        assert_eq!(
            engine
                .get_counter("WorldHandlerImmediateReentry")
                .expect("reentry counter should remain allocated")
                .value,
            1,
            "the host camera callback must re-enter before its action returns"
        );
    });

    {
        let global = get_script_engine();
        let mut global = global.write().expect("script engine global lock");
        global
            .as_mut()
            .expect("script engine should initialize")
            .set_action_handler(previous_handler);
    }

    assert_eq!(completed, Some(()));
    assert_eq!(*calls.lock().unwrap(), vec!["zoom:25:1.5:0.25:0.5"]);
}

#[test]
fn active_attack_priority_and_object_list_actions_mutate_the_live_engine() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    let completed = with_script_engine_mut(|engine| {
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

        let priority_set_name = "ActiveCampaignPrioritySet";
        let mut default_priority = ScriptAction::new(ScriptActionType::SetDefaultAttackPriority);
        default_priority
            .add_parameter(Parameter::with_string(
                ParameterType::AttackPrioritySet,
                priority_set_name.to_string(),
            ))
            .expect("priority set parameter");
        default_priority
            .add_parameter(Parameter::with_int(ParameterType::Int, 17))
            .expect("priority parameter");
        assert_eq!(
            dispatcher
                .execute_action(&default_priority)
                .expect("default-priority action"),
            ScriptActionResult::Success
        );
        let info = engine
            .get_attack_info(priority_set_name)
            .expect("C++ setDefaultAttackPriority creates its set");
        assert_eq!(info.get_name(), priority_set_name);
        assert_eq!(info.default_priority, 17);

        let list_name = "ActiveCampaignObjectList";
        let object_type = "CampaignTestObject";
        let mut add_to_list = ScriptAction::new(ScriptActionType::ObjectlistAddobjecttype);
        add_to_list
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectTypeList,
                list_name.to_string(),
            ))
            .expect("list parameter");
        add_to_list
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                object_type.to_string(),
            ))
            .expect("object type parameter");
        assert_eq!(
            dispatcher
                .execute_action(&add_to_list)
                .expect("add-list action"),
            ScriptActionResult::Success
        );
        assert!(
            engine
                .get_object_types(list_name)
                .expect("list must be created")
                .is_in_set(&AsciiString::from(object_type))
        );

        let mut remove_from_list = ScriptAction::new(ScriptActionType::ObjectlistRemoveobjecttype);
        remove_from_list
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectTypeList,
                list_name.to_string(),
            ))
            .expect("list parameter");
        remove_from_list
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                object_type.to_string(),
            ))
            .expect("object type parameter");
        assert_eq!(
            dispatcher
                .execute_action(&remove_from_list)
                .expect("remove-list action"),
            ScriptActionResult::Success
        );
        assert!(
            !engine
                .get_object_types(list_name)
                .expect("list remains allocated after removal")
                .is_in_set(&AsciiString::from(object_type))
        );

        let mut allow_bonuses = ScriptAction::new(ScriptActionType::ObjectAllowBonuses);
        allow_bonuses
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 0))
            .expect("difficulty-bonus parameter");
        assert_eq!(
            dispatcher
                .execute_action(&allow_bonuses)
                .expect("difficulty-bonus action"),
            ScriptActionResult::Success
        );
        assert!(!engine.get_objects_should_receive_difficulty_bonus());

        let mut choose_normal = ScriptAction::new(ScriptActionType::ChooseVictimAlwaysUsesNormal);
        choose_normal
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .expect("choose-victim parameter");
        assert_eq!(
            dispatcher
                .execute_action(&choose_normal)
                .expect("choose-victim action"),
            ScriptActionResult::Success
        );
        assert!(engine.get_choose_victim_always_uses_normal());
    });

    assert_eq!(completed, Some(()));
}

#[test]
fn active_skirmish_prerequisite_condition_reads_the_live_object_type_list() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    player_list().write().unwrap().clear();
    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    player
        .write()
        .unwrap()
        .set_display_name("ActiveSkirmishPrerequisitePlayer");
    player_list().write().unwrap().add_player(player);

    let completed = with_script_engine_mut(|engine| {
        // C++ objectTypesFromParam first resolves an ObjectTypes list by
        // exact name, then asks that list whether the player can build
        // any member.  An empty registered list therefore fails closed
        // while proving that this lookup works from the active engine.
        let list_name = "ActiveSkirmishEmptyPrerequisiteList";
        engine.set_object_types(
            list_name.to_string(),
            crate::object::object_types::ObjectTypes::with_list_name(AsciiString::from(list_name)),
        );

        let mut condition = Condition::new(ConditionType::SkirmishPlayerHasPrerequisiteToBuild);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "ActiveSkirmishPrerequisitePlayer".to_string(),
            ))
            .expect("player parameter");
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::ObjectType,
                list_name.to_string(),
            ))
            .expect("object type list parameter");

        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::False,
            "an empty C++ ObjectTypes list has no template the player can build"
        );
    });

    player_list().write().unwrap().clear();
    assert_eq!(completed, Some(()));
}

#[test]
fn active_named_actions_do_not_relock_the_engine_or_hold_host_callbacks() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    const OBJECT_ID: ObjectID = 86_220;
    const OBJECT_NAME: &str = "ActiveNamedActionObject";
    get_named_object_tracker()
        .register_named_object(OBJECT_NAME.to_string(), OBJECT_ID)
        .expect("named object registration");
    let mut named_object = crate::object::Object::new_test(OBJECT_ID, 100.0);
    named_object.set_name(AsciiString::from(OBJECT_NAME));

    let calls = Arc::new(Mutex::new(Vec::new()));
    let previous_handler = {
        let global = get_script_engine();
        let mut global = global.write().expect("script engine global lock");
        let engine = global.as_mut().expect("script engine should initialize");
        let previous = engine.action_handler();
        engine.set_action_handler(Some(Arc::new(ReentrantWorldActionHandler {
            calls: Arc::clone(&calls),
        })));
        previous
    };

    let completed = with_script_engine_mut(|engine| {
        engine
            .set_counter("WorldHandlerImmediateReentry", 0)
            .expect("reentry counter");

        let mut script_to_run = Script::new();
        script_to_run.script_name = "ActiveNamedSequentialTarget".to_string();
        let mut list = ScriptList::new();
        list.append_script(Box::new(script_to_run));
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .expect("sequential target script list");

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

        for action_type in [
            ScriptActionType::NamedHideSpecialPowerDisplay,
            ScriptActionType::NamedShowSpecialPowerDisplay,
        ] {
            let mut action = ScriptAction::new(action_type);
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::Unit,
                    OBJECT_NAME.to_string(),
                ))
                .expect("named display object parameter");
            assert_eq!(
                dispatcher
                    .execute_action(&action)
                    .expect("named display action"),
                ScriptActionResult::Success
            );
        }

        let mut topple = ScriptAction::new(ScriptActionType::NamedSetToppleDirection);
        topple
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                OBJECT_NAME.to_string(),
            ))
            .expect("topple object parameter");
        topple
            .add_parameter(Parameter::with_coord(
                ParameterType::Coord3D,
                crate::scripting::core::Coord3D::new(3.0, 4.0, 0.0),
            ))
            .expect("topple direction parameter");
        assert_eq!(
            dispatcher.execute_action(&topple).expect("topple action"),
            ScriptActionResult::Success
        );
        let mut adjusted = Coord3D::new(0.0, 0.0, 0.0);
        engine.adjust_topple_direction(&named_object, &mut adjusted);
        assert!((adjusted.x - 0.6).abs() < f32::EPSILON);
        assert!((adjusted.y - 0.8).abs() < f32::EPSILON);
        assert_eq!(adjusted.z, 0.0);

        let mut start = ScriptAction::new(ScriptActionType::UnitExecuteSequentialScript);
        start
            .add_parameter(Parameter::with_string(
                ParameterType::Unit,
                OBJECT_NAME.to_string(),
            ))
            .expect("sequential object parameter");
        start
            .add_parameter(Parameter::with_string(
                ParameterType::Script,
                "ActiveNamedSequentialTarget".to_string(),
            ))
            .expect("sequential target parameter");
        assert_eq!(
            dispatcher
                .execute_action(&start)
                .expect("start sequential action"),
            ScriptActionResult::Success
        );
        assert!(engine.has_active_sequential_script_for_object(OBJECT_ID));

        let mut stop = ScriptAction::new(ScriptActionType::UnitStopSequentialScript);
        stop.add_parameter(Parameter::with_string(
            ParameterType::Unit,
            OBJECT_NAME.to_string(),
        ))
        .expect("stop sequential object parameter");
        assert_eq!(
            dispatcher
                .execute_action(&stop)
                .expect("stop sequential action"),
            ScriptActionResult::Success
        );
        assert!(!engine.has_active_sequential_script_for_object(OBJECT_ID));

        assert_eq!(
            engine
                .get_counter("WorldHandlerImmediateReentry")
                .expect("reentry counter remains allocated")
                .value,
            2,
            "both host display callbacks re-enter before their script action returns"
        );
    });

    {
        let global = get_script_engine();
        let mut global = global.write().expect("script engine global lock");
        global
            .as_mut()
            .expect("script engine should initialize")
            .set_action_handler(previous_handler);
    }
    get_named_object_tracker()
        .unregister_object(OBJECT_ID)
        .expect("named object cleanup");

    assert_eq!(completed, Some(()));
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            format!("hide-superweapon:{OBJECT_ID}"),
            format!("show-superweapon:{OBJECT_ID}"),
        ]
    );
}

#[test]
fn active_team_sequential_actions_keep_cxx_lookup_idle_append_order() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    const TEAM_NAME: &str = "ActiveTeamSequentialActions";
    get_team_factory().lock().unwrap().reset();
    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from(TEAM_NAME),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team(TEAM_NAME)
            .expect("team should be created");
    }

    let completed = with_script_engine_mut(|engine| {
        let mut script_to_run = Script::new();
        script_to_run.script_name = "ActiveTeamSequentialTarget".to_string();
        let mut list = ScriptList::new();
        list.append_script(Box::new(script_to_run));
        engine
            .set_script_list_for_player(0, Some(Box::new(list)))
            .expect("sequential target script list");

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        let mut start = ScriptAction::new(ScriptActionType::TeamExecuteSequentialScript);
        start
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                TEAM_NAME.to_string(),
            ))
            .expect("team parameter");
        start
            .add_parameter(Parameter::with_string(
                ParameterType::Script,
                "ActiveTeamSequentialTarget".to_string(),
            ))
            .expect("target script parameter");
        assert_eq!(
            dispatcher
                .execute_action(&start)
                .expect("start sequential action"),
            ScriptActionResult::Success
        );
        assert!(engine.has_active_sequential_script_for_team(TEAM_NAME));

        let mut stop = ScriptAction::new(ScriptActionType::TeamStopSequentialScript);
        stop.add_parameter(Parameter::with_string(
            ParameterType::Team,
            TEAM_NAME.to_string(),
        ))
        .expect("team parameter");
        assert_eq!(
            dispatcher
                .execute_action(&stop)
                .expect("stop sequential action"),
            ScriptActionResult::Success
        );
        assert!(!engine.has_active_sequential_script_for_team(TEAM_NAME));
    });

    get_team_factory().lock().unwrap().reset();
    assert_eq!(completed, Some(()));
}

#[test]
fn active_script_special_power_and_upgrade_events_are_immediate_and_one_shot_like_cpp() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    player_list().write().unwrap().clear();
    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    player
        .write()
        .unwrap()
        .set_display_name("ActiveExecutorEventPlayer");
    player_list().write().unwrap().add_player(player);

    let completed = with_script_engine_mut(|engine| {
        engine.notify_of_triggered_special_power(0, "ActiveExecutorSpecialPower", INVALID_ID);

        let mut special_power = Condition::new(ConditionType::PlayerTriggeredSpecialPower);
        special_power
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "ActiveExecutorEventPlayer".to_string(),
            ))
            .expect("player parameter");
        special_power
            .add_parameter(Parameter::with_string(
                ParameterType::SpecialPower,
                "ActiveExecutorSpecialPower".to_string(),
            ))
            .expect("special-power parameter");

        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));
        assert_eq!(
            evaluator.evaluate_condition(&mut special_power).unwrap(),
            ScriptConditionResult::True
        );
        assert_eq!(
            evaluator.evaluate_condition(&mut special_power).unwrap(),
            ScriptConditionResult::False,
            "C++ removes the matched special-power event"
        );

        engine.notify_of_completed_upgrade(0, "ActiveExecutorUpgrade", INVALID_ID);
        let mut upgrade = Condition::new(ConditionType::PlayerBuiltUpgrade);
        upgrade
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "ActiveExecutorEventPlayer".to_string(),
            ))
            .expect("player parameter");
        upgrade
            .add_parameter(Parameter::with_string(
                ParameterType::Upgrade,
                "ActiveExecutorUpgrade".to_string(),
            ))
            .expect("upgrade parameter");

        assert_eq!(
            evaluator.evaluate_condition(&mut upgrade).unwrap(),
            ScriptConditionResult::True
        );
        assert_eq!(
            evaluator.evaluate_condition(&mut upgrade).unwrap(),
            ScriptConditionResult::False,
            "C++ PLAYER_BUILT_UPGRADE is an edge-triggered ScriptEngine event"
        );
    });

    assert_eq!(completed, Some(()));
    player_list().write().unwrap().clear();
}

#[test]
fn team_hunt_with_command_button_invokes_hunt_update() {
    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();

    const TEAM_NAME: &str = "HuntWithCommandTeam";
    const BUTTON_NAME: &str = "Command_DummyHuntFireWeapon";
    const COMMAND_SET: &str = "HuntWithCommandTestSet";
    const HUNTER_ID: ObjectID = 8801;

    crate::control_bar::install_test_command_button(
        crate::command_button::CommandButton::new(8801, BUTTON_NAME.to_string(), String::new(), 0)
            .with_command_type(CommandType::DoAttackObject),
        COMMAND_SET,
        0,
    )
    .expect("dummy hunt command button");

    {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from(TEAM_NAME),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team(TEAM_NAME)
            .expect("hunt team should be created");
    }

    let recorded = Arc::new(Mutex::new(Vec::new()));
    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let hunter = crate::object_manager::GameObjectInstance::new(
        HUNTER_ID,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("test hunter instance");

    {
        let __base_arc = hunter.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
        }))));
        base.set_command_set_string_override(&AsciiString::from(COMMAND_SET));
        let data = Arc::new(
                crate::object::update::command_button_hunt_update::CommandButtonHuntUpdateModuleData::default(),
            );
        base.install_update_module(
            "CommandButtonHuntUpdate",
            Box::new(RecordingHuntModule {
                recorded: Arc::clone(&recorded),
                data: Arc::clone(&data),
            }),
            data,
        );
    }

    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(hunter, Coord3D::new(8.0, 4.0, 0.0))
        .unwrap();
    {
        let factory = get_team_factory();
        let mut factory_guard = factory.lock().unwrap();
        factory_guard
            .find_team(TEAM_NAME)
            .unwrap()
            .write()
            .unwrap()
            .add_member(HUNTER_ID);
    }

    let mut action = ScriptAction::new(ScriptActionType::TeamHuntWithCommandButton);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            TEAM_NAME.to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::CommandbuttonAllAbilities,
            BUTTON_NAME.to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let result = dispatcher
        .execute_action(&action)
        .expect("TEAM_HUNT_WITH_COMMAND_BUTTON should succeed");
    assert_eq!(result, ScriptActionResult::Success);
    assert_eq!(
        *recorded.lock().unwrap(),
        vec![BUTTON_NAME.to_string()],
        "C++ calls CommandButtonHuntUpdate::setCommandButton(ability) on each valid team member"
    );

    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();
}

#[test]
fn active_team_build_actions_fail_closed_without_a_prototype_controller() {
    let _test_lock = crate::test_sync::lock();
    initialize_script_engine().expect("script engine should initialize");

    const TEAM_NAME: &str = "UnownedActiveBuildTeam";
    get_team_factory().lock().unwrap().reset();
    get_team_factory().lock().unwrap().init_team(
        AsciiString::from(TEAM_NAME),
        AsciiString::default(),
        false,
        None,
    );

    let completed = with_script_engine_mut(|_| {
        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));

        let mut build = ScriptAction::new(ScriptActionType::BuildTeam);
        build
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                TEAM_NAME.to_string(),
            ))
            .expect("team parameter");
        assert_eq!(
            dispatcher.execute_action(&build).expect("build action"),
            ScriptActionResult::Success
        );

        let mut recruit = ScriptAction::new(ScriptActionType::RecruitTeam);
        recruit
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                TEAM_NAME.to_string(),
            ))
            .expect("team parameter");
        recruit
            .add_parameter(Parameter::with_real(ParameterType::Real, 120.0))
            .expect("recruit radius parameter");
        assert_eq!(
            dispatcher.execute_action(&recruit).expect("recruit action"),
            ScriptActionResult::Success
        );
    });

    get_team_factory().lock().unwrap().reset();
    assert_eq!(completed, Some(()));
}

#[test]
fn build_and_recruit_team_queue_host_when_dual_world_empty() {
    let _test_lock = crate::test_sync::lock();
    crate::object::registry::OBJECT_REGISTRY.clear();
    let _ = take_host_build_team_requests();
    let _ = take_host_recruit_team_requests();
    initialize_script_engine().expect("script engine should initialize");

    const TEAM_NAME: &str = "HostBuildTeam";
    const OWNER: &str = "PlyrAmerica";
    get_team_factory().lock().unwrap().reset();
    get_team_factory().lock().unwrap().init_team(
        AsciiString::from(TEAM_NAME),
        AsciiString::from(OWNER),
        false,
        None,
    );

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let mut build = ScriptAction::new(ScriptActionType::BuildTeam);
    build
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            TEAM_NAME.to_string(),
        ))
        .expect("team parameter");
    assert_eq!(
        dispatcher.execute_action(&build).expect("build action"),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_build_team_requests(),
        vec![(OWNER.to_string(), TEAM_NAME.to_string())]
    );

    let mut recruit = ScriptAction::new(ScriptActionType::RecruitTeam);
    recruit
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            TEAM_NAME.to_string(),
        ))
        .expect("team parameter");
    recruit
        .add_parameter(Parameter::with_real(ParameterType::Real, 250.0))
        .expect("recruit radius parameter");
    assert_eq!(
        dispatcher.execute_action(&recruit).expect("recruit action"),
        ScriptActionResult::Success
    );
    assert_eq!(
        take_host_recruit_team_requests(),
        vec![(OWNER.to_string(), TEAM_NAME.to_string(), 250.0)]
    );

    get_team_factory().lock().unwrap().reset();
}

#[test]
fn team_transfer_to_player_reassigns_team_controller_without_capture() {
    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();
    player_list().write().unwrap().clear();

    const TEAM_NAME: &str = "ExecutorTransferTeam";
    const SRC_PLAYER: &str = "ExecutorTransferSrc";
    const DST_PLAYER: &str = "ExecutorTransferDst";
    const MEMBER_ID: ObjectID = 8701;

    let src_player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    src_player.write().unwrap().set_display_name(SRC_PLAYER);
    let dst_player = Arc::new(RwLock::new(crate::player::Player::new(1)));
    dst_player.write().unwrap().set_display_name(DST_PLAYER);
    {
        let mut list = player_list().write().unwrap();
        list.add_player(src_player);
        list.add_player(dst_player);
    }

    let team = {
        let mut factory = get_team_factory().lock().unwrap();
        factory.init_team(
            AsciiString::from(TEAM_NAME),
            AsciiString::from(SRC_PLAYER),
            false,
            None,
        );
        factory
            .create_team(TEAM_NAME)
            .expect("transfer team should be created")
    };

    let member = crate::object_manager::GameObjectInstance::new(
        MEMBER_ID,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("transfer team member");
    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(member, Coord3D::new(3.0, 4.0, 0.0))
        .unwrap();
    // Registry must be non-empty or Team::set_controlling_player_id no-ops.
    team.write().unwrap().set_controlling_player_id(Some(0));
    team.write().unwrap().add_member(MEMBER_ID);
    assert_eq!(team.read().unwrap().get_controlling_player_id(), Some(0));

    let mut action = ScriptAction::new(ScriptActionType::TeamTransferToPlayer);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            TEAM_NAME.to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            DST_PLAYER.to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let result = dispatcher
        .execute_action(&action)
        .expect("TEAM_TRANSFER_TO_PLAYER should succeed");
    assert_eq!(result, ScriptActionResult::Success);
    assert_eq!(
        team.read().unwrap().get_controlling_player_id(),
        Some(1),
        "C++ Team::setControllingPlayer reassigns the team, not individual captures"
    );
    assert_eq!(
        team.read().unwrap().get_members(),
        &[MEMBER_ID],
        "members stay on the same team after TEAM_TRANSFER_TO_PLAYER"
    );

    get_object_manager().write().unwrap().reset();
    get_team_factory().lock().unwrap().reset();
    player_list().write().unwrap().clear();
}

#[test]
fn player_sell_everything_sells_faction_structures_like_cxx() {
    get_object_manager().write().unwrap().reset();
    player_list().write().unwrap().clear();
    game_engine::common::system::build_assistant::init_build_assistant();

    const PLAYER_NAME: &str = "ExecutorSellPlayer";
    const FACTORY_ID: ObjectID = 8710;
    const CC_ID: ObjectID = 8711;
    const UNIT_ID: ObjectID = 8712;

    let player = Arc::new(RwLock::new(crate::player::Player::new(0)));
    player.write().unwrap().set_display_name(PLAYER_NAME);
    player_list().write().unwrap().add_player(player.clone());

    let mut factory_template =
        crate::common::DefaultThingTemplate::new("ExecutorFactionFactory".to_string());
    factory_template.add_kind_of(crate::common::KindOf::FSWarfactory);
    let mut factory = crate::object::Object::new_test(FACTORY_ID, 100.0);
    factory.set_template_for_test(Arc::new(factory_template));
    TheGameLogic::register_object(Arc::new(RwLock::new(factory)))
        .expect("register faction factory");

    let mut cc_template =
        crate::common::DefaultThingTemplate::new("ExecutorCommandCenter".to_string());
    cc_template.add_kind_of(crate::common::KindOf::CommandCenter);
    let mut command_center = crate::object::Object::new_test(CC_ID, 100.0);
    command_center.set_template_for_test(Arc::new(cc_template));
    TheGameLogic::register_object(Arc::new(RwLock::new(command_center)))
        .expect("register command center");

    let unit = crate::object::Object::new_test(UNIT_ID, 100.0);
    TheGameLogic::register_object(Arc::new(RwLock::new(unit))).expect("register unit");

    {
        let mut player_guard = player.write().unwrap();
        player_guard.add_owned_object(FACTORY_ID);
        player_guard.add_owned_object(CC_ID);
        player_guard.add_owned_object(UNIT_ID);
    }

    let mut action = ScriptAction::new(ScriptActionType::PlayerSellEverything);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Side,
            PLAYER_NAME.to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let result = dispatcher
        .execute_action(&action)
        .expect("PLAYER_SELL_EVERYTHING should succeed");
    assert_eq!(result, ScriptActionResult::Success);

    let assistant = game_engine::common::system::build_assistant::get_build_assistant()
        .expect("build assistant");
    let sold: Vec<ObjectID> = assistant
        .get_sell_list()
        .iter()
        .map(|info| info.id)
        .collect();
    assert!(
        sold.contains(&FACTORY_ID),
        "C++ sellBuildings sells faction structures: {sold:?}"
    );
    assert!(
        sold.contains(&CC_ID),
        "C++ sellBuildings also sells command centers: {sold:?}"
    );
    assert!(
        !sold.contains(&UNIT_ID),
        "non-faction units must not be sold: {sold:?}"
    );

    get_object_manager().write().unwrap().reset();
    player_list().write().unwrap().clear();
}

#[test]
fn damage_members_of_team_applies_unresistable_damage() {
    get_object_manager().write().unwrap().reset();

    const TEAM_NAME: &str = "ExecutorDamageTeam";
    const MEMBER_ID: ObjectID = 8720;

    let team = {
        let mut factory = get_team_factory().lock().unwrap_or_else(|e| e.into_inner());
        factory.reset();
        factory.init_team(
            AsciiString::from(TEAM_NAME),
            AsciiString::default(),
            false,
            None,
        );
        factory
            .create_team(TEAM_NAME)
            .expect("damage team should be created")
    };

    let member = crate::object::Object::new_test(MEMBER_ID, 100.0);
    let member_arc = Arc::new(RwLock::new(member));
    TheGameLogic::register_object(member_arc.clone()).expect("register damage member");
    team.write().unwrap().add_member(MEMBER_ID);

    let mut action = ScriptAction::new(ScriptActionType::DamageMembersOfTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            TEAM_NAME.to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_real(ParameterType::Real, 25.0))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let result = dispatcher
        .execute_action(&action)
        .expect("DAMAGE_MEMBERS_OF_TEAM should succeed");
    assert_eq!(result, ScriptActionResult::Success);
    assert_eq!(
        member_arc.read().unwrap().get_health(),
        75.0,
        "C++ Team::damageTeamMembers applies DAMAGE_UNRESISTABLE of the given amount"
    );

    get_object_manager().write().unwrap().reset();
    get_team_factory()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .reset();
}

#[test]
fn move_named_unit_to_leaves_group_and_dispatches_ai_move() {
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_terrain_logic().write().unwrap().reset();

    let mut map_data = crate::system::map_loader::MapData::new();
    map_data.width = 4;
    map_data.height = 4;
    map_data.heightmap = vec![0; 16];
    map_data
        .waypoints
        .push(crate::system::map_loader::MapWaypoint {
            id: 9101,
            name: "ExecutorMoveWp".to_string(),
            location: crate::system::map_loader::Coord3D::new(50.0, 60.0, 0.0),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    get_terrain_logic().write().unwrap().load_map_data(map_data);
    let waypoint_pos = *get_terrain_logic()
        .read()
        .unwrap()
        .get_waypoint_by_name(&AsciiString::from("ExecutorMoveWp"))
        .expect("loaded MOVE_NAMED_UNIT_TO waypoint")
        .get_location();

    let commands = Arc::new(Mutex::new(Vec::new()));
    let locomotors = Arc::new(Mutex::new(Vec::new()));
    let cleared = Arc::new(Mutex::new(0u32));
    const UNIT_ID: ObjectID = 8730;
    let unit = crate::object_manager::GameObjectInstance::new(
        UNIT_ID,
        None,
        None,
        ObjectCreationFlags::new(),
    )
    .expect("named move unit");
    {
        let __base_arc = unit.base();
        let mut base = __base_arc.write().unwrap();
        base.set_ai_update_interface(Some(Arc::new(Mutex::new(RecordingMoveAi {
            commands: Arc::clone(&commands),
            locomotors: Arc::clone(&locomotors),
            cleared: Arc::clone(&cleared),
            attitudes: Arc::new(Mutex::new(Vec::new())),
        }))));
        base.enter_group(&crate::ai::AIGroup::new(93));
        assert_eq!(base.get_group_id(), Some(93));
    }

    get_object_manager()
        .write()
        .unwrap()
        .register_object_instance(unit, Coord3D::new(4.0, 5.0, 0.0))
        .unwrap();
    get_named_object_tracker()
        .register_named_object("ExecutorMover".to_string(), UNIT_ID)
        .unwrap();

    let mut action = ScriptAction::new(ScriptActionType::MoveNamedUnitTo);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorMover".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Waypoint,
            "ExecutorMoveWp".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    let result = dispatcher
        .execute_action(&action)
        .expect("MOVE_NAMED_UNIT_TO should succeed");
    assert_eq!(result, ScriptActionResult::Success);
    // C++ ScriptActions.cpp:433 doNamedMoveToWaypoint clears the queue first.
    assert_eq!(*cleared.lock().unwrap(), 1);
    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        *commands.lock().unwrap(),
        vec![(
            AiCommandType::MoveToPosition,
            waypoint_pos,
            CommandSourceType::FromScript,
        )]
    );
    assert_eq!(
        get_object_manager()
            .read()
            .unwrap()
            .with_object(UNIT_ID, |o| o
                .base()
                .read()
                .ok()
                .and_then(|b| b.get_group_id()))
            .flatten(),
        None
    );

    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_terrain_logic().write().unwrap().reset();
}

#[test]
fn named_follow_waypoints_leaves_group_and_selects_normal_loco() {
    // C++ ScriptActions.cpp:1621-1623 doNamedFollowWaypoints.
    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_terrain_logic().write().unwrap().reset();

    let mut map_data = crate::system::map_loader::MapData::new();
    map_data.width = 4;
    map_data.height = 4;
    map_data.heightmap = vec![0; 16];
    map_data
        .waypoints
        .push(crate::system::map_loader::MapWaypoint {
            id: 9201,
            name: "FollowWp".to_string(),
            location: crate::system::map_loader::Coord3D::new(80.0, 90.0, 0.0),
            path_label1: "FollowPath".to_string(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
    get_terrain_logic().write().unwrap().load_map_data(map_data);

    const UNIT_ID: ObjectID = 8740;
    let (commands, locomotors, _, _) =
        install_recording_named_unit(UNIT_ID, "FollowUnit", Some(94));

    let mut action = ScriptAction::new(ScriptActionType::NamedFollowWaypoints);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "FollowUnit".to_string(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::WaypointPath,
            "FollowPath".to_string(),
        ))
        .unwrap();

    let mut dispatcher = ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
    dispatcher.do_named_follow_waypoints(&action).unwrap();

    assert_eq!(*locomotors.lock().unwrap(), vec![LocomotorSetType::Normal]);
    assert_eq!(
        commands.lock().unwrap()[0].0,
        AiCommandType::FollowWaypointPath
    );
    assert_eq!(
        get_object_manager()
            .read()
            .unwrap()
            .with_object(UNIT_ID, |o| o
                .base()
                .read()
                .ok()
                .and_then(|b| b.get_group_id()))
            .flatten(),
        None
    );

    get_object_manager().write().unwrap().reset();
    get_named_object_tracker().clear().unwrap();
    get_terrain_logic().write().unwrap().reset();
}
