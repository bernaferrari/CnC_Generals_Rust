//! Executor unit tests.

    use super::*;
    use crate::common::LocomotorSetType;
    use crate::modules::AIUpdateInterface;
    use crate::object_manager::ObjectCreationFlags;
    use crate::scripting::engine::{ScriptEngine, SequentialScript};
    use std::sync::Mutex;

    #[derive(Debug)]
    struct RecordingAi {
        commands: Arc<
            Mutex<
                Vec<(
                    AiCommandType,
                    Option<ObjectID>,
                    Option<String>,
                    i32,
                    CommandSourceType,
                )>,
            >,
        >,
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
                command.team.clone(),
                command.int_value,
                command.cmd_source,
            ));
            Ok(())
        }
    }

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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        dispatcher.do_team_guard_object(&action).unwrap();

        assert!(commands.lock().unwrap().is_empty());
        assert!(locomotors.lock().unwrap().is_empty());
    }

    #[derive(Debug)]
    struct RecruitableRecordingAi {
        commands: Arc<
            Mutex<
                Vec<(
                    AiCommandType,
                    Option<ObjectID>,
                    Option<String>,
                    i32,
                    CommandSourceType,
                )>,
            >,
        >,
        recruitable: Arc<Mutex<Vec<bool>>>,
    }

    impl AIUpdateInterface for RecruitableRecordingAi {
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
            self.commands.lock().unwrap().push((
                command.cmd,
                command.obj,
                command.team.clone(),
                command.int_value,
                command.cmd_source,
            ));
            Ok(())
        }

        fn set_is_recruitable(&mut self, recruitable: bool) {
            self.recruitable.lock().unwrap().push(recruitable);
        }
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        assert!(!script_engine_lock
            .read()
            .unwrap()
            .as_ref()
            .unwrap()
            .has_active_sequential_script_for_team("MissingSequentialTeam"));
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

        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::False,
            "C++ resolves the side parameters, ignores N, and returns FALSE because the helper is unimplemented"
        );
    }

    #[test]
    fn condition_mission_attempts_ignores_parameters_like_cxx_stub() {
        let mut condition = Condition::new(ConditionType::MissionAttempts);
        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::False,
            "C++ evaluateMissionAttempts does not read parameters and always returns false"
        );
    }

    #[test]
    fn condition_player_has_credits_compares_threshold_to_player_money_like_cxx() {
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
            .add_parameter(Parameter::with_string(
                ParameterType::Side,
                "CreditsExecutorPlayer".to_string(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Comparison, 0))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 500))
            .unwrap();

        let mut evaluator =
            ScriptConditionEvaluator::new(Arc::new(RwLock::new(ScriptContext::new())));

        assert_eq!(
            evaluator.evaluate_condition(&mut condition).unwrap(),
            ScriptConditionResult::True,
            "C++ evaluates threshold < player's credits, not player's credits < threshold"
        );
    }

    /// Records CommandButtonHuntUpdate::setCommandButton calls without running the full hunt update.
    struct RecordingHuntModule {
        recorded: Arc<Mutex<Vec<String>>>,
        data: Arc<crate::object::update::command_button_hunt_update::CommandButtonHuntUpdateModuleData>,
    }

    impl game_engine::common::system::Snapshotable for RecordingHuntModule {
        fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
            Ok(())
        }
        fn xfer(&mut self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
            Ok(())
        }
        fn load_post_process(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    impl game_engine::common::thing::module::Module for RecordingHuntModule {
        fn get_module_data(&self) -> &dyn game_engine::common::thing::module::ModuleData {
            self.data.as_ref()
        }

        fn get_command_button_hunt_control_interface(
            &mut self,
        ) -> Option<&mut dyn game_engine::common::thing::module::CommandButtonHuntControlInterface>
        {
            Some(self)
        }
    }

    impl game_engine::common::thing::module::CommandButtonHuntControlInterface
        for RecordingHuntModule
    {
        fn set_command_button(&mut self, button_name: String) {
            self.recorded.lock().unwrap().push(button_name);
        }
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
            crate::command_button::CommandButton::new(
                8801,
                BUTTON_NAME.to_string(),
                String::new(),
                0,
            )
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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
    fn team_transfer_to_player_reassigns_team_controller_without_capture() {
        get_object_manager().write().unwrap().reset();
        get_team_factory().lock().unwrap().reset();
        player_list().write().unwrap().clear();

        const TEAM_NAME: &str = "ExecutorTransferTeam";
        const SRC_PLAYER: &str = "ExecutorTransferSrc";
        const DST_PLAYER: &str = "ExecutorTransferDst";
        const MEMBER_ID: ObjectID = 8701;

        let src_player = Arc::new(RwLock::new(crate::player::Player::new(0)));
        src_player
            .write()
            .unwrap()
            .set_display_name(SRC_PLAYER);
        let dst_player = Arc::new(RwLock::new(crate::player::Player::new(1)));
        dst_player
            .write()
            .unwrap()
            .set_display_name(DST_PLAYER);
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
        team.write()
            .unwrap()
            .set_controlling_player_id(Some(0));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        let result = dispatcher
            .execute_action(&action)
            .expect("PLAYER_SELL_EVERYTHING should succeed");
        assert_eq!(result, ScriptActionResult::Success);

        let assistant = game_engine::common::system::build_assistant::get_build_assistant()
            .expect("build assistant");
        let sold: Vec<ObjectID> = assistant.get_sell_list().iter().map(|info| info.id).collect();
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
            let mut factory = get_team_factory()
                .lock()
                .unwrap_or_else(|e| e.into_inner());
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
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

    #[derive(Debug)]
    struct RecordingMoveAi {
        commands: Arc<Mutex<Vec<(AiCommandType, Coord3D, CommandSourceType)>>>,
        locomotors: Arc<Mutex<Vec<LocomotorSetType>>>,
    }

    impl AIUpdateInterface for RecordingMoveAi {
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
                command.cmd_source,
            ));
            Ok(())
        }
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
        map_data.waypoints.push(crate::system::map_loader::MapWaypoint {
            id: 9101,
            name: "ExecutorMoveWp".to_string(),
            location: crate::system::map_loader::Coord3D::new(50.0, 60.0, 0.0),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });
        get_terrain_logic()
            .write()
            .unwrap()
            .load_map_data(map_data);
        let waypoint_pos = *get_terrain_logic()
            .read()
            .unwrap()
            .get_waypoint_by_name(&AsciiString::from("ExecutorMoveWp"))
            .expect("loaded MOVE_NAMED_UNIT_TO waypoint")
            .get_location();

        let commands = Arc::new(Mutex::new(Vec::new()));
        let locomotors = Arc::new(Mutex::new(Vec::new()));
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

        let mut dispatcher =
            ScriptActionDispatcher::new(Arc::new(RwLock::new(ScriptContext::new())));
        let result = dispatcher
            .execute_action(&action)
            .expect("MOVE_NAMED_UNIT_TO should succeed");
        assert_eq!(result, ScriptActionResult::Success);
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
