//! Behavior suite extracted from the original test module.
use super::*;
use crate::scripting::engine::ScriptOwnerQuery;
use crate::team::TeamID;

#[test]
fn executor_named_attack_named_selects_borrowed_driver() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptNamedCommand};

    struct NamedOwner {
        calls: Vec<(String, String, Option<ObjectID>)>,
        reject: bool,
    }
    impl ScriptExecutionDriver for NamedOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }

        fn named_command(
            &mut self,
            request: ScriptNamedCommand<'_>,
            this_object: Option<ObjectID>,
        ) -> Option<GameLogicResult<()>> {
            let ScriptNamedCommand::ForceAttack { unit, target } = request else {
                panic!("unexpected named command: {request:?}");
            };
            self.calls.push((unit.into(), target.into(), this_object));
            Some(if self.reject {
                Err(GameLogicError::ModuleError(
                    "named owner rejected command".into(),
                ))
            } else {
                Ok(())
            })
        }
    }

    let engine = ScriptEngine::new().expect("script engine");
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = NamedOwner {
        calls: Vec::new(),
        reject: false,
    };
    let mut action = ScriptAction::new(ScriptActionType::NamedAttackNamed);
    for name in ["ExecutorAttacker", "ExecutorVictim"] {
        action
            .add_parameter(Parameter::with_string(ParameterType::Unit, name.into()))
            .unwrap();
    }
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.calls,
        vec![("ExecutorAttacker".into(), "ExecutorVictim".into(), None)]
    );

    // A selected owner's failure is authoritative; the standalone adapter
    // must not turn it into Success or enqueue the command for another world.
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(message)) if message.contains("named owner rejected command")
    ));
    assert_eq!(owner.calls.len(), 2);
}

#[test]
fn executor_team_attack_team_dispatches_attack_team() {
    // This checks explicit dispatcher admission; real C++ effect and peer-world
    // isolation assertions live in Main::owned_runtime_command_tests.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::TeamAttackTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "SourceTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "TargetTeam".into(),
        ))
        .unwrap();
    let mut owner = ExplicitCommandIngress::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.calls,
        vec![(
            "TeamAttackTeam".into(),
            "SourceTeam".into(),
            Some("TargetTeam".into())
        )]
    );
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(_))
    ));
}

#[test]
fn executor_named_attack_area_leaves_group_and_selects_normal_locomotor() {
    #[derive(Debug, Default)]
    struct OwnedAi {
        normal: bool,
        command: Option<AiCommandParams>,
    }
    impl AIUpdateInterface for OwnedAi {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }
        fn is_moving(&self) -> bool {
            false
        }
        fn is_idle(&self) -> bool {
            true
        }
        fn set_movement_target(&mut self, _: &Coord3D) -> Result<(), String> {
            Ok(())
        }
        fn choose_locomotor_set(
            &mut self,
            set: LocomotorSetType,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.normal = set == LocomotorSetType::Normal;
            Ok(())
        }
        fn execute_command(
            &mut self,
            params: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            assert!(self.normal, "CPP selects Normal before issuing AttackArea");
            self.command = Some(params.clone());
            Ok(())
        }
    }
    struct Owner {
        object: crate::object::Object,
        ai: OwnedAi,
        trigger: crate::polygon_trigger::PolygonTrigger,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn named_attack_area(
            &mut self,
            unit: &str,
            area: &str,
            _: Option<&str>,
            _: Option<ObjectID>,
        ) -> Option<GameLogicResult<()>> {
            if unit == "ExecutorAreaAttacker" && area == "ExecutorAttackArea" {
                self.object.leave_group();
                crate::scripting::executor::actions_named::issue_borrowed_named_attack_area(
                    &mut self.ai,
                    Coord3D::new(10.0, 10.0, 0.0),
                    self.trigger.get_id(),
                );
            }
            Some(Ok(()))
        }
    }
    fn make_owner() -> Owner {
        let mut object = crate::object::Object::new_raw(
            Arc::new(crate::common::DefaultThingTemplate::new(
                "AreaAttacker".into(),
            )),
            8471,
            crate::common::ObjectStatusMaskType::NONE,
            None,
        );
        object.enter_group(&crate::ai::AIGroup::new(92));
        Owner {
            object,
            ai: OwnedAi::default(),
            trigger: crate::polygon_trigger::PolygonTrigger::new(
                8470,
                "ExecutorAttackArea".into(),
                vec![
                    crate::common::ICoord3D::new(0, 0, 0),
                    crate::common::ICoord3D::new(20, 0, 0),
                    crate::common::ICoord3D::new(20, 20, 0),
                    crate::common::ICoord3D::new(0, 20, 0),
                ],
            ),
        }
    }
    let mut owner = make_owner();
    let foreign = make_owner();
    let engine = ScriptEngine::new().unwrap();
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &state);
    let mut action = ScriptAction::new(ScriptActionType::NamedAttackArea);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorAreaAttacker".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::TriggerArea,
            "MissingArea".into(),
        ))
        .unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert_eq!(owner.object.get_group_id(), Some(92));
    assert!(owner.ai.command.is_none());
    action.parameters[1] = Some(Parameter::with_string(
        ParameterType::TriggerArea,
        "ExecutorAttackArea".into(),
    ));
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert_eq!(owner.object.get_group_id(), None);
    let command = owner.ai.command.unwrap();
    assert_eq!(command.cmd, AiCommandType::AttackArea);
    assert_eq!(command.cmd_source, CommandSourceType::FromScript);
    assert_eq!(command.polygon, Some(8470));
    assert_eq!(command.pos, Coord3D::new(10.0, 10.0, 0.0));
    assert_eq!(foreign.object.get_group_id(), Some(92));
    assert!(foreign.ai.command.is_none());
}

#[test]
fn executor_named_attack_team_validates_team_and_sets_max_shots() {
    #[derive(Debug, Default)]
    struct OwnedAi {
        commands: Vec<(
            AiCommandType,
            Option<ObjectID>,
            Option<String>,
            i32,
            CommandSourceType,
        )>,
        locomotors: Vec<LocomotorSetType>,
    }
    impl AIUpdateInterface for OwnedAi {
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
            self.locomotors.push(set);
            Ok(())
        }
        fn execute_command(
            &mut self,
            command: &AiCommandParams,
        ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            self.commands.push((
                command.cmd,
                command.obj,
                command.team.clone(),
                command.int_value,
                command.cmd_source,
            ));
            Ok(())
        }
    }
    struct AttackOwner {
        factory: crate::team::TeamFactory,
        object: crate::object::Object,
        ai: OwnedAi,
    }
    impl ScriptExecutionDriver for AttackOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn named_attack_team(
            &mut self,
            unit: &str,
            team: &str,
            _this: Option<ObjectID>,
            _calling: Option<crate::team::TeamID>,
            _condition: Option<crate::team::TeamID>,
        ) -> Option<GameLogicResult<()>> {
            if unit == "ExecutorTeamAttacker" && !self.factory.find_team_instances(team).is_empty()
            {
                self.object.leave_group();
                crate::scripting::executor::actions_named::issue_borrowed_named_attack_team(
                    &mut self.ai,
                    team,
                );
            }
            Some(Ok(()))
        }
    }
    fn make_owner() -> AttackOwner {
        let template = Arc::new(crate::common::DefaultThingTemplate::new(
            "LocalTeamAttacker".into(),
        ));
        let mut object = crate::object::Object::new_raw(
            template,
            8480,
            crate::common::ObjectStatusMaskType::NONE,
            None,
        );
        object.enter_group(&crate::ai::AIGroup::new(93));
        AttackOwner {
            factory: crate::team::TeamFactory::new(),
            object,
            ai: OwnedAi::default(),
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = make_owner();
    let foreign = make_owner();
    let mut action = ScriptAction::new(ScriptActionType::NamedAttackTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorTeamAttacker".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorTargetTeam".into(),
        ))
        .unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert!(owner.ai.commands.is_empty());
    assert_eq!(owner.object.get_group_id(), Some(93));
    owner.factory.init_team(
        "ExecutorTargetTeam".into(),
        AsciiString::default(),
        false,
        None,
    );
    owner.factory.create_team("ExecutorTargetTeam").unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert_eq!(owner.ai.locomotors, vec![LocomotorSetType::Normal]);
    assert_eq!(
        owner.ai.commands,
        vec![(
            AiCommandType::AttackTeam,
            None,
            Some("ExecutorTargetTeam".into()),
            -1,
            CommandSourceType::FromScript
        )]
    );
    assert_eq!(owner.object.get_group_id(), None);
    assert_eq!(foreign.object.get_group_id(), Some(93));
    assert!(foreign.ai.commands.is_empty());
}

#[test]
fn executor_team_attack_named_ignores_stale_target_tracker_id() {
    struct AttackOwner {
        factory: crate::team::TeamFactory,
        named: HashMap<String, ObjectID>,
        objects: HashMap<ObjectID, crate::object::Object>,
        commands: Vec<AiCommandParams>,
    }
    impl ScriptExecutionDriver for AttackOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_attack_named(
            &mut self,
            team: &str,
            target: &str,
            _this: Option<ObjectID>,
            _calling: Option<crate::team::TeamID>,
            _condition: Option<crate::team::TeamID>,
        ) -> Option<GameLogicResult<()>> {
            let Some(_team) = self.factory.find_team_instances(team).into_iter().next() else {
                return Some(Ok(()));
            };
            let Some(target) = self.named.get(target).copied() else {
                return Some(Ok(()));
            };
            // A stale local named tracker is not a live target. Admission
            // cannot issue a group command or select a foreign same-ID object.
            if !self.objects.contains_key(&target) {
                return Some(Ok(()));
            }
            let mut command =
                AiCommandParams::new(AiCommandType::AttackObject, CommandSourceType::FromScript);
            command.obj = Some(target);
            command.int_value = -1;
            self.commands.push(command);
            Some(Ok(()))
        }
    }
    let mut owner = AttackOwner {
        factory: crate::team::TeamFactory::new(),
        named: HashMap::from([("MissingExecutorVictim".into(), 8491)]),
        objects: HashMap::new(),
        commands: Vec::new(),
    };
    owner.factory.init_team(
        "ExecutorSourceTeam".into(),
        AsciiString::default(),
        false,
        None,
    );
    owner.factory.create_team("ExecutorSourceTeam").unwrap();
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::TeamAttackNamed);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorSourceTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MissingExecutorVictim".into(),
        ))
        .unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert!(owner.commands.is_empty());
}

struct NamedHuntOwner {
    calls: Vec<(String, Option<ObjectID>)>,
    reject: bool,
}

impl crate::scripting::engine::ScriptExecutionDriver for NamedHuntOwner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }

    fn named_command(
        &mut self,
        request: crate::scripting::engine::ScriptNamedCommand<'_>,
        this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        let crate::scripting::engine::ScriptNamedCommand::Hunt { unit } = request else {
            panic!("unexpected named command: {request:?}");
        };
        self.calls.push((unit.into(), this_object));
        Some(if self.reject {
            Err(GameLogicError::ModuleError(
                "hunt owner rejected command".into(),
            ))
        } else {
            Ok(())
        })
    }
}

#[test]
fn executor_named_hunt_selects_normal_locomotor_before_hunt() {
    // The driver owns NORMAL -> Hunt and preservation of group membership;
    // Main's real object tests verify those effects. Core verifies that public
    // dispatch selects that owner, including its missing-object no-op.
    let engine = ScriptEngine::new().expect("script engine");
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = NamedHuntOwner {
        calls: Vec::new(),
        reject: false,
    };
    let mut action = ScriptAction::new(ScriptActionType::NamedHunt);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorHunter".into(),
        ))
        .unwrap();

    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.calls, vec![("ExecutorHunter".into(), None)]);

    // An authoritative error cannot fall back to a global registry or queue.
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(message)) if message.contains("hunt owner rejected command")
    ));
    assert_eq!(owner.calls.len(), 2);
}

#[test]
fn named_hunt_requires_unit_parameter_before_owner_dispatch() {
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = NamedHuntOwner {
        calls: Vec::new(),
        reject: false,
    };
    let action = ScriptAction::new(ScriptActionType::NamedHunt);
    assert!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .is_err()
    );
    assert!(owner.calls.is_empty());
}

#[test]
fn default_named_hunt_driver_leaves_standalone_adapter_available() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptNamedCommand};
    struct UnavailableOwner;
    impl ScriptExecutionDriver for UnavailableOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
    }
    assert!(
        UnavailableOwner
            .named_command(ScriptNamedCommand::Hunt { unit: "Hunter" }, None)
            .is_none()
    );
}

#[test]
fn executor_named_stop_dispatches_direct_ai_without_player_owner() {
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = BorrowedIdleOwner::new();
    let peer = BorrowedIdleOwner::new();
    let mut action = ScriptAction::new(ScriptActionType::NamedStop);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "OwnedIdleUnit".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    owner.assert_script_idle();
    assert_eq!(
        owner.object.get_group_id(),
        Some(92),
        "Idle retains the AI group"
    );
    assert!(peer.ai.command.is_none());
}

/// Command contract for the same bounded helper used by the standalone adapter.
/// Actual movement/target behavior is asserted by Main's script_idle_owner_tests.
#[derive(Debug, Default)]
struct BorrowedIdleAi {
    command: Option<AiCommandParams>,
    locomotor_calls: usize,
}
impl AIUpdateInterface for BorrowedIdleAi {
    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
    fn is_moving(&self) -> bool {
        false
    }
    fn is_idle(&self) -> bool {
        true
    }
    fn set_movement_target(&mut self, _: &Coord3D) -> Result<(), String> {
        Ok(())
    }
    fn choose_locomotor_set(
        &mut self,
        _: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.locomotor_calls += 1;
        Ok(())
    }
    fn execute_command(
        &mut self,
        command: &AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.command = Some(command.clone());
        Ok(())
    }
}
struct BorrowedIdleOwner {
    object: crate::object::Object,
    ai: BorrowedIdleAi,
}
impl BorrowedIdleOwner {
    fn new() -> Self {
        let mut object = crate::object::Object::new_raw(
            Arc::new(crate::common::DefaultThingTemplate::new("IdleUnit".into())),
            8496,
            crate::common::ObjectStatusMaskType::NONE,
            None,
        );
        object.enter_group(&crate::ai::AIGroup::new(92));
        Self {
            object,
            ai: BorrowedIdleAi::default(),
        }
    }
    fn assert_script_idle(&self) {
        let command = self
            .ai
            .command
            .as_ref()
            .expect("actual bounded Idle command");
        assert_eq!(command.cmd, AiCommandType::Idle);
        assert_eq!(command.cmd_source, CommandSourceType::FromScript);
        assert_eq!(self.ai.locomotor_calls, 0);
    }
}
impl ScriptExecutionDriver for BorrowedIdleOwner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn named_command(
        &mut self,
        command: crate::scripting::engine::ScriptNamedCommand<'_>,
        _: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        let crate::scripting::engine::ScriptNamedCommand::Stop { unit } = command else {
            return None;
        };
        if unit == "OwnedIdleUnit" {
            crate::scripting::executor::actions_victory_team::issue_borrowed_script_idle(
                &mut self.ai,
            );
        }
        Some(Ok(()))
    }
    fn team_idle(
        &mut self,
        team: &str,
        _: Option<crate::team::TeamID>,
        _: Option<crate::team::TeamID>,
    ) -> Option<GameLogicResult<()>> {
        if team == "OwnedIdleTeam" {
            crate::scripting::executor::actions_victory_team::issue_borrowed_script_idle(
                &mut self.ai,
            );
        }
        Some(Ok(()))
    }
}

struct NamedGuardOwner {
    calls: Vec<(String, Option<ObjectID>)>,
    reject: bool,
}

impl crate::scripting::engine::ScriptExecutionDriver for NamedGuardOwner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }

    fn named_command(
        &mut self,
        request: crate::scripting::engine::ScriptNamedCommand<'_>,
        this_object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        let crate::scripting::engine::ScriptNamedCommand::Guard { unit } = request else {
            panic!("unexpected named command: {request:?}");
        };
        self.calls.push((unit.into(), this_object));
        Some(if self.reject {
            Err(GameLogicError::ModuleError(
                "guard owner rejected command".into(),
            ))
        } else {
            Ok(())
        })
    }
}

#[test]
fn executor_named_guard_leaves_group_selects_locomotor_and_sets_guard_mode() {
    // Main's real object tests own leaveGroup -> NORMAL -> position guard,
    // NORMAL mode and SCRIPT source. Core verifies actual dispatcher selection
    // of that owner, including its authoritative missing-object no-op.
    let engine = ScriptEngine::new().expect("script engine");
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = NamedGuardOwner {
        calls: Vec::new(),
        reject: false,
    };
    let mut action = ScriptAction::new(ScriptActionType::NamedGuard);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "ExecutorGuard".into(),
        ))
        .unwrap();

    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.calls, vec![("ExecutorGuard".into(), None)]);

    // An authoritative error cannot fall back to a global registry or queue.
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(message)) if message.contains("guard owner rejected command")
    ));
    assert_eq!(owner.calls.len(), 2);
}

#[test]
fn named_guard_requires_unit_parameter_before_owner_dispatch() {
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = NamedGuardOwner {
        calls: Vec::new(),
        reject: false,
    };
    let action = ScriptAction::new(ScriptActionType::NamedGuard);
    assert!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .is_err()
    );
    assert!(owner.calls.is_empty());
}

#[test]
fn default_named_guard_driver_leaves_standalone_adapter_available() {
    use crate::scripting::engine::{ScriptExecutionDriver, ScriptNamedCommand};
    struct UnavailableOwner;
    impl ScriptExecutionDriver for UnavailableOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
    }
    assert!(
        UnavailableOwner
            .named_command(ScriptNamedCommand::Guard { unit: "Guard" }, None)
            .is_none()
    );
}

#[test]
fn executor_team_guard_for_framecount_dispatches_idle_like_cxx_switch() {
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    for kind in [
        ScriptActionType::TeamGuardForFramecount,
        ScriptActionType::TeamIdleForFramecount,
    ] {
        let mut owner = BorrowedIdleOwner::new();
        let mut action = ScriptAction::new(kind);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "OwnedIdleTeam".into(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Int, 7))
            .unwrap();
        assert_eq!(
            dispatcher
                .execute_action_with_driver(&action, &mut owner)
                .unwrap(),
            ScriptActionResult::Pending(7.0)
        );
        owner.assert_script_idle();
        assert_eq!(owner.object.get_group_id(), Some(92));
        action.parameters[1] = Some(Parameter::with_int(ParameterType::Int, 0));
        assert_eq!(
            dispatcher
                .execute_action_with_driver(&action, &mut owner)
                .unwrap(),
            ScriptActionResult::Success
        );
    }
}

#[test]
fn executor_team_guard_object_ignores_stale_target_tracker_id() {
    // The dispatcher selects a borrowed owner even for authoritative absence.
    // Main accepts the actual guard/health effects in owned_runtime_command_tests.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::TeamGuardObject);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "OwnedTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "MissingTarget".into(),
        ))
        .unwrap();
    let mut owner = ExplicitCommandIngress::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.calls.len(), 1);
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(_))
    ));
}

#[test]
fn executor_team_stop_and_disband_missing_local_team_cannot_select_same_id_peer() {
    // Full stop/recruit/merge identity assertions live on Main's real owner:
    // team_stop_disband_stops_then_recruits_and_merges_actual_owned_rosters.
    // Core covers authoritative absence without globally constructing an Object.
    struct Owner {
        factory: crate::team::TeamFactory,
        object: crate::object::Object,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_stop(
            &mut self,
            team: &str,
            _calling: Option<crate::team::TeamID>,
            _condition: Option<crate::team::TeamID>,
            _disband: bool,
        ) -> Option<GameLogicResult<()>> {
            assert!(
                self.factory.find_team_instances(team).is_empty(),
                "only this owner is searched"
            );
            Some(Ok(()))
        }
    }
    fn owner() -> Owner {
        Owner {
            factory: crate::team::TeamFactory::new(),
            object: crate::object::Object::new_raw(
                Arc::new(crate::common::DefaultThingTemplate::new(
                    "StopDisbandUnit".into(),
                )),
                8530,
                crate::common::ObjectStatusMaskType::NONE,
                None,
            ),
        }
    }
    let mut local = owner();
    let mut peer = owner();
    peer.factory.init_team(
        "ExecutorDisbandSource".into(),
        AsciiString::default(),
        false,
        None,
    );
    let source = peer.factory.create_team("ExecutorDisbandSource").unwrap();
    source.write().unwrap().add_member(peer.object.get_id());
    assert_eq!(local.object.get_id(), peer.object.get_id());
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::TeamStopAndDisband);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "ExecutorDisbandSource".into(),
        ))
        .unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut local)
        .unwrap();
    assert!(source.read().unwrap().has_member(peer.object.get_id()));
    assert!(
        local
            .factory
            .find_team_instances("ExecutorDisbandSource")
            .is_empty()
    );
}

#[test]
fn executor_team_execute_sequential_script_requires_script_before_idle() {
    struct Owner;
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_sequential(
            &mut self,
            _: &str,
            _: Option<crate::team::TeamID>,
            _: Option<crate::team::TeamID>,
            _: Option<(crate::scripting::core::Script, i32)>,
            _: &ScriptEngine,
        ) -> Option<GameLogicResult<()>> {
            panic!("CPP missing-script lookup returns before any Idle owner callback")
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::TeamExecuteSequentialScript);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "OwnedIdleTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Script,
            "MissingSequentialScript".into(),
        ))
        .unwrap();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut Owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert!(!engine.has_active_sequential_script_for_team(1));
}

#[test]
fn executor_team_stop_sequential_script_requires_live_team() {
    struct SequentialOwner {
        factory: crate::team::TeamFactory,
    }
    impl ScriptExecutionDriver for SequentialOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_sequential(
            &mut self,
            name: &str,
            _calling: Option<crate::team::TeamID>,
            _condition: Option<crate::team::TeamID>,
            script: Option<(Script, i32)>,
            engine: &ScriptEngine,
        ) -> Option<GameLogicResult<()>> {
            assert!(
                script.is_none(),
                "this action stops rather than starts a script"
            );
            let team = self.factory.find_team_instances(name).into_iter().next();
            if let Some(team) = team {
                engine.remove_all_sequential_scripts_for_team(team.read().unwrap().get_id());
            }
            Some(Ok(()))
        }
    }
    let engine = ScriptEngine::new().expect("script engine");
    let foreign_engine = ScriptEngine::new().expect("foreign script engine");
    for owner in [&engine, &foreign_engine] {
        let mut sequential = SequentialScript::new();
        sequential.team_to_exec_on = Some(1);
        owner.append_sequential_script(sequential);
    }
    let mut owner = SequentialOwner {
        factory: crate::team::TeamFactory::new(),
    };
    let mut action = ScriptAction::new(ScriptActionType::TeamStopSequentialScript);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "MissingSequentialTeam".into(),
        ))
        .unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert!(
        engine.has_active_sequential_script_for_team(1),
        "missing actual team returns before removal"
    );
    owner.factory.init_team(
        "MissingSequentialTeam".into(),
        AsciiString::default(),
        false,
        None,
    );
    owner.factory.create_team("MissingSequentialTeam").unwrap();
    dispatcher
        .execute_action_with_driver(&action, &mut owner)
        .unwrap();
    assert!(!engine.has_active_sequential_script_for_team(1));
    assert!(foreign_engine.has_active_sequential_script_for_team(1));
}

#[test]
fn condition_player_destroyed_n_buildings_player_matches_cxx_todo_false() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

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

    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);

    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "C++ resolves the side parameters, ignores N, and returns FALSE because the helper is unimplemented"
    );
}

#[test]
fn condition_mission_attempts_ignores_parameters_like_cxx_stub() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    let mut condition = Condition::new(ConditionType::MissionAttempts);
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);

    assert_eq!(
        evaluator.evaluate_condition(&mut condition).unwrap(),
        ScriptConditionResult::False,
        "C++ evaluateMissionAttempts does not read parameters and always returns false"
    );
}

struct PlayerCensusOwner {
    literal_templates: Vec<String>,
    rows: HashMap<String, (i32, crate::scripting::HostScriptPlayerCensus)>,
}
impl ScriptExecutionDriver for PlayerCensusOwner {
    fn literal_template_exists(&self, name: &str) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Present(
            self.literal_templates
                .iter()
                .any(|template| template == name),
        )
    }
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn player_census(
        &self,
        name: &str,
        _current: Option<&str>,
    ) -> ScriptOwnerQuery<(i32, crate::scripting::HostScriptPlayerCensus)> {
        self.rows
            .get(name)
            .cloned()
            .map(ScriptOwnerQuery::Present)
            .unwrap_or(ScriptOwnerQuery::Missing)
    }
    fn player_has_any_objects(&self, name: &str, _current: Option<&str>) -> ScriptOwnerQuery<bool> {
        self.rows
            .get(name)
            .map(|(_, census)| ScriptOwnerQuery::Present(census.has_any_objects))
            .unwrap_or(ScriptOwnerQuery::Missing)
    }
}

#[test]
fn condition_player_has_credits_compares_threshold_to_player_money_like_cxx() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // C++ ScriptEngine.cpp:4414-4416 template [INT, COMPARISON, SIDE]
    // ScriptConditions.cpp:952-966 compares credits param to countMoney().
    let mut player = crate::player::Player::new(0);
    player.get_money_mut().set_money(1000);
    let mut owner = PlayerCensusOwner {
        literal_templates: vec!["AmericaWarFactory".into()],
        rows: HashMap::from([(
            "CreditsExecutorPlayer".into(),
            (
                0,
                crate::scripting::HostScriptPlayerCensus {
                    money: player.get_money().get_money(),
                    ..Default::default()
                },
            ),
        )]),
    };

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

    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);

    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut missing, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "C++ evaluatePlayerHasCredits returns false when playerFromParam fails"
    );
}

#[test]
fn player_conditions_use_host_census_instead_of_stale_leftover_player() {
    let dispatch_engine = ScriptEngine::new().unwrap();
    // The contradictory peer is never published, so only the driving census
    // can answer these conditions even when player identities coincide.
    let peer = crate::player::Player::new(0);
    assert_eq!(peer.get_money().get_money(), 0);
    let mut snap = crate::scripting::HostScriptQuerySnapshot::default();
    let mut template_counts = std::collections::HashMap::new();
    template_counts.insert("americawarfactory".into(), 1);
    template_counts.insert("americainfantryranger".into(), 2);
    template_counts.insert("americacommandcenter".into(), 1);
    template_counts.insert("americatankcrusader".into(), 1);
    let template_counts_ignore_dead = template_counts.clone();
    snap.player_census.insert(
        "PlyrAmerica".into(),
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
    let mut owner = PlayerCensusOwner {
        literal_templates: vec!["AmericaWarFactory".into()],
        rows: snap
            .player_census
            .into_iter()
            .map(|(name, census)| (name, (0, census)))
            .collect(),
    };

    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);

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
        evaluator
            .evaluate_condition_with_driver(&mut credits, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut power, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut no_power, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut destroyed, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut facilities, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut few, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut enough, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut built, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut has_rangers, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut few_cc, &mut owner)
            .unwrap(),
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
        evaluator
            .evaluate_condition_with_driver(&mut lost, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "LOST first census sample stores ignoreDead count and is not yet lost"
    );
    {
        let mut snap = crate::scripting::HostScriptQuerySnapshot::default();
        let mut template_counts = std::collections::HashMap::new();
        template_counts.insert("americatankcrusader".into(), 1);
        snap.player_census.insert(
            "PlyrAmerica".into(),
            crate::scripting::HostScriptPlayerCensus {
                template_counts,
                template_counts_ignore_dead: std::collections::HashMap::new(),
                ..Default::default()
            },
        );
        owner.rows = snap
            .player_census
            .into_iter()
            .map(|(name, census)| (name, (0, census)))
            .collect();
    }
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut lost, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "LOST fires when ignoreDead census drops vs ScriptEngine cache"
    );

    assert_eq!(peer.get_money().get_money(), 0);
}

// Owner ingress contracts; Main named_lifecycle_owner_tests exercises actual
// object death/removal, name history and save continuation.
struct NamedLifetimeOwner {
    name: &'static str,
    state: crate::scripting::engine::ScriptNamedLifecycle,
}
impl ScriptExecutionDriver for NamedLifetimeOwner {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn named_lifecycle(
        &self,
        name: &str,
        _this_object: Option<u32>,
    ) -> ScriptOwnerQuery<crate::scripting::engine::ScriptNamedLifecycle> {
        if name == self.name {
            ScriptOwnerQuery::Present(self.state)
        } else {
            ScriptOwnerQuery::Missing
        }
    }
}

#[test]
fn named_destroyed_false_if_name_never_existed_like_cxx() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // C++ ScriptConditions::evaluateNamedUnitDestroyed (ScriptConditions.cpp:274-286)
    let mut owner = NamedLifetimeOwner {
        name: "NeverSpawnedHero",
        state: crate::scripting::engine::ScriptNamedLifecycle {
            exists: false,
            effectively_dead: false,
            ever_existed: false,
        },
    };

    let mut condition = Condition::new(ConditionType::NamedDestroyed);
    condition
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NeverSpawnedHero".into(),
        ))
        .unwrap();
    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "C++ evaluateNamedUnitDestroyed returns false when the name never existed"
    );
}

#[test]
fn named_destroyed_and_dying_use_effectively_dead_while_object_exists_like_cxx() {
    let dispatch_engine = crate::scripting::engine::ScriptEngine::new().expect("script engine");

    // C++ ScriptConditions.cpp:274-318 — existing unit uses isEffectivelyDead();
    // NAMED_DYING is false once the object is gone.
    let mut owner = NamedLifetimeOwner {
        name: "DyingHero",
        state: crate::scripting::engine::ScriptNamedLifecycle {
            exists: true,
            effectively_dead: true,
            ever_existed: true,
        },
    };
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

    let evaluator_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&dispatch_engine, &evaluator_state);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut destroyed, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "C++ evaluateNamedUnitDestroyed uses isEffectivelyDead while the object exists"
    );
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut dying, &mut owner)
            .unwrap(),
        ScriptConditionResult::True,
        "C++ evaluateNamedUnitDying is isEffectivelyDead while the object exists"
    );

    owner.state.exists = false;
    owner.state.effectively_dead = false;

    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut dying, &mut owner)
            .unwrap(),
        ScriptConditionResult::False,
        "C++ evaluateNamedUnitDying is false once the object is gone"
    );
}

struct VictoryOwner<'a> {
    engine: &'a ScriptEngine,
    input_enabled: bool,
    observer: bool,
    input_callbacks: usize,
}
impl crate::scripting::engine::ScriptExecutionDriver for VictoryOwner<'_> {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn local_player_observer(&self) -> ScriptOwnerQuery<bool> {
        ScriptOwnerQuery::Present(self.observer)
    }
    fn world(
        &mut self,
        request: crate::scripting::engine::ScriptWorldRequest<'_>,
    ) -> Option<GameLogicResult<()>> {
        let crate::scripting::engine::ScriptWorldRequest::Input { enabled } = request else {
            panic!("unexpected victory request: {request:?}");
        };
        self.input_enabled = enabled;
        self.input_callbacks += 1;
        // Input admission may immediately reenter this same interpreter.
        assert_eq!(self.engine.get_counter("VictoryReentry").unwrap().value, 3);
        assert!(
            !self.engine.is_game_ending(),
            "input is disabled before the end-game timer starts"
        );
        Some(Ok(()))
    }
}

fn run_owned_victory_action(kind: ScriptActionType, observer: bool, already_defeated: bool) {
    let engine = ScriptEngine::new().unwrap();
    let peer = ScriptEngine::new().unwrap();
    engine.set_counter("VictoryReentry", 3).unwrap();
    peer.set_counter("VictoryReentry", 9).unwrap();
    engine.set_campaign_victorious(true);
    engine.set_shown_mp_local_defeat_window(already_defeated);
    engine.close_windows(false);
    let state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &state);
    let mut counter = Condition::new(ConditionType::Counter);
    counter
        .add_parameter(Parameter::with_string(
            ParameterType::Counter,
            "VictoryReentry".into(),
        ))
        .unwrap();
    counter
        .add_parameter(Parameter::with_int(
            ParameterType::Comparison,
            ComparisonType::Equal as i32,
        ))
        .unwrap();
    counter
        .add_parameter(Parameter::with_int(ParameterType::Int, 3))
        .unwrap();
    assert_eq!(
        evaluator.evaluate_condition(&mut counter).unwrap(),
        ScriptConditionResult::True
    );
    let mut owner = VictoryOwner {
        engine: &engine,
        input_enabled: true,
        observer,
        input_callbacks: 0,
    };
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &state);
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&ScriptAction::new(kind), &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    let local = kind == ScriptActionType::Localdefeat;
    assert_eq!(owner.input_enabled, local);
    assert_eq!(owner.input_callbacks, usize::from(!local));
    assert_eq!(
        engine.is_campaign_victorious(),
        kind == ScriptActionType::Victory
    );
    let expected = if local {
        if observer {
            None
        } else {
            Some("Menus/LocalDefeat.wnd")
        }
    } else if observer || already_defeated {
        Some("Menus/ObserverQuit.wnd")
    } else if kind == ScriptActionType::Victory {
        Some("Menus/Victorious.wnd")
    } else {
        Some("Menus/Defeat.wnd")
    };
    assert_eq!(engine.current_win_lose_window().as_deref(), expected);
    assert_eq!(engine.is_game_ending(), !local);
    assert_eq!(
        engine.has_shown_mp_local_defeat_window(),
        local || already_defeated
    );
    assert_eq!(peer.get_counter("VictoryReentry").unwrap().value, 9);
    assert!(!peer.is_game_ending());
    assert_eq!(peer.current_win_lose_window(), None);
}

#[test]
fn active_script_counter_and_victory_reenter_the_borrowed_engine() {
    run_owned_victory_action(ScriptActionType::Victory, false, false);
}
#[test]
fn do_defeat_clears_campaign_victorious_flag() {
    run_owned_victory_action(ScriptActionType::Defeat, true, false);
}
#[test]
fn do_victory_creates_victorious_window_layout() {
    run_owned_victory_action(ScriptActionType::Victory, false, false);
    run_owned_victory_action(ScriptActionType::Victory, false, true);
}
#[test]
fn do_defeat_creates_defeat_window_layout() {
    run_owned_victory_action(ScriptActionType::Defeat, false, false);
}
#[test]
fn do_local_defeat_creates_local_defeat_window_layout() {
    run_owned_victory_action(ScriptActionType::Localdefeat, false, false);
    run_owned_victory_action(ScriptActionType::Localdefeat, true, false);
}

#[test]
fn active_world_actions_clone_the_handler_before_host_callback_reentry() {
    let engine = ScriptEngine::new().unwrap();
    let peer = ScriptEngine::new().unwrap();
    peer.set_counter("WorldHandlerImmediateReentry", 9).unwrap();
    let mut owner = BorrowedWorldOwner {
        engine: &engine,
        calls: Vec::new(),
    };

    engine
        .set_counter("WorldHandlerImmediateReentry", 0)
        .expect("reentry counter");
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &dispatcher_state);

    let mut fullscreen_movie = ScriptAction::new(ScriptActionType::MoviePlayFullscreen);
    fullscreen_movie
        .add_parameter(Parameter::with_string(
            ParameterType::Movie,
            "campaign_intro.bik".to_string(),
        ))
        .expect("movie parameter");
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&fullscreen_movie, &mut owner)
            .expect("movie action"),
        ScriptActionResult::Success
    );

    assert_eq!(
        dispatcher
            .execute_action_with_driver(
                &ScriptAction::new(ScriptActionType::RadarForceEnable),
                &mut owner
            )
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
        dispatcher
            .execute_action_with_driver(&timer, &mut owner)
            .expect("timer action"),
        ScriptActionResult::Success
    );

    assert_eq!(
        dispatcher
            .execute_action_with_driver(
                &ScriptAction::new(ScriptActionType::FreezeTime),
                &mut owner
            )
            .expect("freeze action"),
        ScriptActionResult::Success
    );
    assert_eq!(
        dispatcher
            .execute_action_with_driver(
                &ScriptAction::new(ScriptActionType::UnfreezeTime),
                &mut owner
            )
            .expect("unfreeze action"),
        ScriptActionResult::Success
    );

    let mut visual_speed = ScriptAction::new(ScriptActionType::SetVisualSpeedMultiplier);
    visual_speed
        .add_parameter(Parameter::with_int(ParameterType::Int, 2))
        .expect("visual speed parameter");
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&visual_speed, &mut owner)
            .expect("visual speed action"),
        ScriptActionResult::Success
    );

    let mut weather = ScriptAction::new(ScriptActionType::ShowWeather);
    weather
        .add_parameter(Parameter::with_int(ParameterType::Boolean, 0))
        .expect("weather parameter");
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&weather, &mut owner)
            .expect("weather action"),
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
    assert_eq!(
        peer.get_counter("WorldHandlerImmediateReentry")
            .unwrap()
            .value,
        9
    );

    assert_eq!(
        owner.calls,
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
    let engine = ScriptEngine::new().unwrap();
    let peer = ScriptEngine::new().unwrap();
    peer.set_counter("WorldHandlerImmediateReentry", 9).unwrap();
    let mut owner = BorrowedWorldOwner {
        engine: &engine,
        calls: Vec::new(),
    };

    engine
        .set_counter("WorldHandlerImmediateReentry", 0)
        .expect("reentry counter");
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &dispatcher_state);

    let mut display = ScriptAction::new(ScriptActionType::DisplayText);
    display
        .add_parameter(Parameter::with_string(
            ParameterType::LocalizedText,
            "GUI:CampaignMessage".to_string(),
        ))
        .expect("display text parameter");
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&display, &mut owner)
            .expect("display action"),
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
        dispatcher
            .execute_action_with_driver(&music, &mut owner)
            .expect("music action"),
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
    assert_eq!(
        peer.get_counter("WorldHandlerImmediateReentry")
            .unwrap()
            .value,
        9
    );

    assert_eq!(
        owner.calls,
        vec![
            "text:GUI:CampaignMessage",
            "music:CampaignCombat:true:false",
        ]
    );
}

#[test]
fn active_camera_actions_snapshot_handler_and_mutate_fade_without_relocking() {
    use crate::scripting::engine::ScriptCameraRequest;
    struct Owner<'a> {
        engine: &'a ScriptEngine,
        calls: Vec<String>,
    }
    impl ScriptExecutionDriver for Owner<'_> {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn camera(&mut self, request: ScriptCameraRequest<'_>) -> Option<GameLogicResult<()>> {
            let ScriptCameraRequest::Zoom {
                zoom,
                seconds,
                ease_in_seconds,
                ease_out_seconds,
            } = request
            else {
                panic!("unexpected camera request")
            };
            self.calls.push(format!(
                "zoom:{zoom}:{seconds}:{ease_in_seconds}:{ease_out_seconds}"
            ));
            Some(self.engine.increment_counter("ImmediateCameraReentry", 1))
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let peer = ScriptEngine::new().unwrap();
    engine.set_counter("ImmediateCameraReentry", 0).unwrap();
    peer.set_counter("ImmediateCameraReentry", 9).unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = Owner {
        engine: &engine,
        calls: Vec::new(),
    };
    let mut zoom = ScriptAction::new(ScriptActionType::ZoomCamera);
    for value in [25.0, 1.5, 0.25, 0.5] {
        zoom.add_parameter(Parameter::with_real(ParameterType::Real, value))
            .unwrap();
    }
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&zoom, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        engine.get_counter("ImmediateCameraReentry").unwrap().value,
        1
    );
    let mut fade = ScriptAction::new(ScriptActionType::CameraFadeAdd);
    for value in [0.2, 0.8] {
        fade.add_parameter(Parameter::with_real(ParameterType::Real, value))
            .unwrap();
    }
    for value in [0, 3, 2] {
        fade.add_parameter(Parameter::with_int(ParameterType::Int, value))
            .unwrap();
    }
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&fade, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(engine.get_fade(), TFade::Add);
    assert!((engine.get_fade_value() - 0.8).abs() < f32::EPSILON);
    assert_eq!(owner.calls, vec!["zoom:25:1.5:0.25:0.5"]);
    assert_eq!(peer.get_counter("ImmediateCameraReentry").unwrap().value, 9);
}

#[test]
fn active_attack_priority_and_object_list_actions_mutate_the_live_engine() {
    // The real dispatcher mutates this engine's priority/list/flag stores.
    // No process-wide ScriptEngine admission or test serialization is needed.
    let engine = ScriptEngine::new().expect("script engine");
    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &dispatcher_state);

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
}

#[test]
fn active_skirmish_prerequisite_condition_reads_the_live_object_type_list() {
    use crate::scripting::engine::ScriptOwnerQuery;

    struct PrerequisiteOwner {
        candidates: RefCell<Vec<Vec<String>>>,
    }
    impl ScriptExecutionDriver for PrerequisiteOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn skirmish_player_can_build_any(
            &self,
            player: &str,
            candidates: &[String],
            _: Option<&str>,
        ) -> ScriptOwnerQuery<bool> {
            assert_eq!(player, "ActiveSkirmishPrerequisitePlayer");
            self.candidates.borrow_mut().push(candidates.to_vec());
            ScriptOwnerQuery::Present(
                candidates
                    .iter()
                    .any(|candidate| candidate == "OwnedBuildableCandidate"),
            )
        }
    }

    let engine = ScriptEngine::new().expect("owned script engine");
    let list_name = "ActiveSkirmishEmptyPrerequisiteList";
    engine.set_object_types(
        list_name.into(),
        ObjectTypes::with_list_name(AsciiString::from(list_name)),
    );
    let mut condition = Condition::new(ConditionType::SkirmishPlayerHasPrerequisiteToBuild);
    for (parameter_type, value) in [
        (ParameterType::Side, "ActiveSkirmishPrerequisitePlayer"),
        (ParameterType::ObjectType, list_name),
    ] {
        condition
            .add_parameter(Parameter::with_string(parameter_type, value.into()))
            .unwrap();
    }
    let evaluator_state = RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &evaluator_state);
    let mut driver = PrerequisiteOwner {
        candidates: RefCell::new(Vec::new()),
    };
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut driver)
            .unwrap(),
        ScriptConditionResult::False,
        "a registered empty C++ ObjectTypes list remains empty"
    );
    let mut types = ObjectTypes::with_list_name(AsciiString::from(list_name));
    types.add_object_type(AsciiString::from("OwnedBuildableCandidate"));
    engine.set_object_types(list_name.into(), types);
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut driver)
            .unwrap(),
        ScriptConditionResult::True,
        "the next evaluation observes the current borrowed engine list"
    );
    engine.set_object_types(list_name.into(), ObjectTypes::new());
    assert_eq!(
        evaluator
            .evaluate_condition_with_driver(&mut condition, &mut driver)
            .unwrap(),
        ScriptConditionResult::False
    );
    assert_eq!(
        *driver.candidates.borrow(),
        vec![
            Vec::<String>::new(),
            vec!["OwnedBuildableCandidate".to_owned()],
            Vec::<String>::new()
        ]
    );
}

#[test]
fn active_named_actions_do_not_relock_the_engine_or_hold_host_callbacks() {
    struct Owner<'a> {
        engine: &'a ScriptEngine,
        object: crate::object::Object,
        calls: Vec<String>,
    }
    impl ScriptExecutionDriver for Owner<'_> {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn named_object_id(&self, name: &str, _: Option<ObjectID>) -> ScriptOwnerQuery<ObjectID> {
            if name == self.object.get_name().as_str() {
                ScriptOwnerQuery::Present(self.object.get_id())
            } else {
                ScriptOwnerQuery::Missing
            }
        }
        fn named_superweapon_display(
            &mut self,
            name: &str,
            show: bool,
            _: Option<ObjectID>,
        ) -> Option<GameLogicResult<()>> {
            assert_eq!(name, self.object.get_name().as_str());
            self.calls.push(format!(
                "{}-superweapon:{}",
                if show { "show" } else { "hide" },
                self.object.get_id()
            ));
            Some(
                self.engine
                    .increment_counter("WorldHandlerImmediateReentry", 1),
            )
        }
        fn named_topple_direction(
            &mut self,
            name: &str,
            direction: Coord3D,
        ) -> Option<GameLogicResult<()>> {
            self.engine.set_topple_direction(name, Some(direction));
            Some(Ok(()))
        }
    }
    const OBJECT_ID: ObjectID = 86220;
    const OBJECT_NAME: &str = "OwnedNamedActionObject";
    let engine = ScriptEngine::new().unwrap();
    let peer = ScriptEngine::new().unwrap();
    let mut named_object = crate::object::Object::new_raw(
        Arc::new(crate::common::DefaultThingTemplate::new(
            "OwnedNamedAction".into(),
        )),
        OBJECT_ID,
        crate::common::ObjectStatusMaskType::NONE,
        None,
    );
    named_object.set_name(AsciiString::from(OBJECT_NAME));
    let mut owner = Owner {
        engine: &engine,
        object: named_object,
        calls: Vec::new(),
    };

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

    let dispatcher_state = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &dispatcher_state);

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
                .execute_action_with_driver(&action, &mut owner)
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
        dispatcher
            .execute_action_with_driver(&topple, &mut owner)
            .expect("topple action"),
        ScriptActionResult::Success
    );
    let mut adjusted = Coord3D::new(0.0, 0.0, 0.0);
    engine.adjust_topple_direction(&owner.object, &mut adjusted);
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
            .execute_action_with_driver(&start, &mut owner)
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
            .execute_action_with_driver(&stop, &mut owner)
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
    assert!(!peer.has_active_sequential_script_for_object(OBJECT_ID));
    assert!(peer.topple_direction_for_name(OBJECT_NAME).is_none());

    assert_eq!(
        owner.calls,
        vec![
            format!("hide-superweapon:{OBJECT_ID}"),
            format!("show-superweapon:{OBJECT_ID}"),
        ]
    );
}

#[test]
fn active_team_sequential_actions_keep_cxx_lookup_idle_append_order() {
    struct Owner {
        factory: crate::team::TeamFactory,
        ai: BorrowedIdleAi,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn team_sequential(
            &mut self,
            name: &str,
            _: Option<TeamID>,
            _: Option<TeamID>,
            script: Option<(Script, i32)>,
            engine: &ScriptEngine,
        ) -> Option<GameLogicResult<()>> {
            let team = self
                .factory
                .find_team_instances(name)
                .into_iter()
                .next()
                .unwrap();
            let id = team.read().unwrap().get_id();
            if let Some((script, loops)) = script {
                assert!(!engine.has_active_sequential_script_for_team(id));
                crate::scripting::executor::actions_victory_team::issue_borrowed_script_idle(
                    &mut self.ai,
                );
                assert_eq!(self.ai.command.as_ref().unwrap().cmd, AiCommandType::Idle);
                let mut sequential = SequentialScript::new();
                sequential.team_to_exec_on = Some(id);
                sequential.script_to_execute_sequentially = Some(Box::new(script));
                sequential.times_to_loop = loops;
                engine.append_sequential_script(sequential);
            } else {
                engine.remove_all_sequential_scripts_for_team(id);
            }
            Some(Ok(()))
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let peer = ScriptEngine::new().unwrap();
    let mut sequential = SequentialScript::new();
    sequential.team_to_exec_on = Some(1);
    peer.append_sequential_script(sequential);
    let mut owner = Owner {
        factory: crate::team::TeamFactory::new(),
        ai: BorrowedIdleAi::default(),
    };
    owner.factory.init_team(
        "OwnedSequentialTeam".into(),
        AsciiString::default(),
        false,
        None,
    );
    owner.factory.create_team("OwnedSequentialTeam").unwrap();
    let mut script = Script::new();
    script.script_name = "OwnedSequentialTarget".into();
    let mut list = ScriptList::new();
    list.append_script(Box::new(script));
    engine
        .set_script_list_for_player(0, Some(Box::new(list)))
        .unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    for kind in [
        ScriptActionType::TeamExecuteSequentialScript,
        ScriptActionType::TeamStopSequentialScript,
    ] {
        let mut action = ScriptAction::new(kind);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "OwnedSequentialTeam".into(),
            ))
            .unwrap();
        if kind == ScriptActionType::TeamExecuteSequentialScript {
            action
                .add_parameter(Parameter::with_string(
                    ParameterType::Script,
                    "OwnedSequentialTarget".into(),
                ))
                .unwrap();
        }
        assert_eq!(
            dispatcher
                .execute_action_with_driver(&action, &mut owner)
                .unwrap(),
            ScriptActionResult::Success
        );
        assert_eq!(
            engine.has_active_sequential_script_for_team(1),
            kind == ScriptActionType::TeamExecuteSequentialScript
        );
        assert!(peer.has_active_sequential_script_for_team(1));
    }
}

#[test]
fn active_script_special_power_and_upgrade_events_are_immediate_and_one_shot_like_cpp() {
    use crate::scripting::engine::{ScriptOwnerQuery, ScriptPlayerEventSource};

    struct EventOwner;
    impl ScriptExecutionDriver for EventOwner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn player_event_source(
            &self,
            player: &str,
            source: Option<&str>,
            _: Option<&str>,
            _: Option<ObjectID>,
        ) -> ScriptOwnerQuery<ScriptPlayerEventSource> {
            assert_eq!(player, "ActiveExecutorEventPlayer");
            assert_eq!(source, None);
            ScriptOwnerQuery::Present(ScriptPlayerEventSource {
                player_index: 0,
                source_object: INVALID_ID,
            })
        }
    }

    let engine = ScriptEngine::new().expect("owned script engine");
    let evaluator_state = RefCell::new(ScriptContext::at_frame(0));
    let mut evaluator = ScriptConditionEvaluator::new(&engine, &evaluator_state);
    let mut driver = EventOwner;
    for (kind, parameter_type, event) in [
        (
            ConditionType::PlayerTriggeredSpecialPower,
            ParameterType::SpecialPower,
            "ActiveExecutorSpecialPower",
        ),
        (
            ConditionType::PlayerBuiltUpgrade,
            ParameterType::Upgrade,
            "ActiveExecutorUpgrade",
        ),
    ] {
        if kind == ConditionType::PlayerBuiltUpgrade {
            engine.notify_of_completed_upgrade(0, event, INVALID_ID);
        } else {
            engine.notify_of_triggered_special_power(0, event, INVALID_ID);
        }
        let mut condition = Condition::new(kind);
        for (parameter_type, value) in [
            (ParameterType::Side, "ActiveExecutorEventPlayer"),
            (parameter_type, event),
        ] {
            condition
                .add_parameter(Parameter::with_string(parameter_type, value.into()))
                .unwrap();
        }
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut driver)
                .unwrap(),
            ScriptConditionResult::True,
            "notification is immediately visible on its borrowed engine"
        );
        assert_eq!(
            evaluator
                .evaluate_condition_with_driver(&mut condition, &mut driver)
                .unwrap(),
            ScriptConditionResult::False,
            "C++ consumes exactly the matched event, rather than testing permanent upgrade state"
        );
    }
}

#[test]
fn team_hunt_with_command_button_invokes_hunt_update() {
    // This checks explicit dispatcher admission; real C++ effect and peer-world
    // isolation assertions live in Main::owned_runtime_command_tests.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::TeamHuntWithCommandButton);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "HuntTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "Command_OwnedFireWeapon".into(),
        ))
        .unwrap();
    let mut owner = ExplicitCommandIngress::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.calls,
        vec![(
            "TeamHuntWithCommandButton".into(),
            "HuntTeam".into(),
            Some("Command_OwnedFireWeapon".into())
        )]
    );
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(_))
    ));
}

#[test]
fn active_team_build_actions_fail_closed_without_a_prototype_controller() {
    use crate::scripting::engine::ScriptAiPlayerRequest;
    struct Owner {
        factory: crate::team::TeamFactory,
    }
    impl ScriptExecutionDriver for Owner {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn ai_player(&mut self, request: ScriptAiPlayerRequest<'_>) -> Option<GameLogicResult<()>> {
            let name = match request {
                ScriptAiPlayerRequest::BuildTeam { team }
                | ScriptAiPlayerRequest::RecruitTeam { team, .. } => team,
                _ => panic!("unexpected AI action"),
            };
            let prototype = self.factory.find_team_prototype(name).unwrap();
            assert!(prototype.get_owner_name().is_empty());
            // An actual ownerless prototype cannot choose a player or acquire
            // another session's AI manager. Main tests accept positive AI effects.
            Some(Ok(()))
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut owner = Owner {
        factory: crate::team::TeamFactory::new(),
    };
    owner.factory.init_team(
        "OwnerlessPrototype".into(),
        AsciiString::default(),
        false,
        None,
    );
    for kind in [ScriptActionType::BuildTeam, ScriptActionType::RecruitTeam] {
        let mut action = ScriptAction::new(kind);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "OwnerlessPrototype".into(),
            ))
            .unwrap();
        if kind == ScriptActionType::RecruitTeam {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 120.0))
                .unwrap();
        }
        assert_eq!(
            dispatcher
                .execute_action_with_driver(&action, &mut owner)
                .unwrap(),
            ScriptActionResult::Success
        );
    }
}

#[test]
fn build_and_recruit_team_select_borrowed_driver_with_exact_payloads() {
    use crate::scripting::engine::{ScriptAiPlayerRequest, ScriptExecutionDriver};
    #[derive(Default)]
    struct Admission {
        calls: Vec<(String, Option<f32>)>,
    }
    impl ScriptExecutionDriver for Admission {
        fn after_action(&mut self) -> GameLogicResult<()> {
            Ok(())
        }
        fn ai_player(&mut self, request: ScriptAiPlayerRequest<'_>) -> Option<GameLogicResult<()>> {
            match request {
                ScriptAiPlayerRequest::BuildTeam { team } => self.calls.push((team.into(), None)),
                ScriptAiPlayerRequest::RecruitTeam { team, radius } => {
                    self.calls.push((team.into(), Some(radius)))
                }
                _ => panic!("unexpected request: {request:?}"),
            }
            Some(Ok(()))
        }
    }
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(0));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut admission = Admission::default();
    for kind in [ScriptActionType::BuildTeam, ScriptActionType::RecruitTeam] {
        let mut action = ScriptAction::new(kind);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Team,
                "HostBuildTeam".into(),
            ))
            .unwrap();
        if kind == ScriptActionType::RecruitTeam {
            action
                .add_parameter(Parameter::with_real(ParameterType::Real, 250.0))
                .unwrap();
        }
        assert_eq!(
            dispatcher
                .execute_action_with_driver(&action, &mut admission)
                .unwrap(),
            ScriptActionResult::Success
        );
    }
    // This is an ingress contract; real AI queue/recruit/build effects are
    // exercised by Main's script_ai_build_owner_tests through its actual driver.
    assert_eq!(
        admission.calls,
        [
            ("HostBuildTeam".into(), None),
            ("HostBuildTeam".into(), Some(250.0))
        ]
    );
}

#[test]
fn team_transfer_to_player_reassigns_team_controller_without_capture() {
    // This checks explicit dispatcher admission; real C++ effect and peer-world
    // isolation assertions live in Main::owned_runtime_command_tests.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::TeamTransferToPlayer);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "TransferTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "TransferPlayer".into(),
        ))
        .unwrap();
    let mut owner = ExplicitCommandIngress::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.calls,
        vec![(
            "TeamTransferToPlayer".into(),
            "TransferTeam".into(),
            Some("TransferPlayer".into())
        )]
    );
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(_))
    ));
}

#[test]
fn player_sell_everything_sells_faction_structures_like_cxx() {
    // This checks explicit dispatcher admission; real C++ effect and peer-world
    // isolation assertions live in Main::owned_runtime_command_tests.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::PlayerSellEverything);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "SellPlayer".into(),
        ))
        .unwrap();
    let mut owner = ExplicitCommandIngress::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.calls,
        vec![("PlayerSellEverything".into(), "SellPlayer".into(), None)]
    );
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(_))
    ));
}

#[test]
fn damage_members_of_team_applies_unresistable_damage() {
    // The dispatcher selects a borrowed owner even for authoritative absence.
    // Main accepts the actual guard/health effects in owned_runtime_command_tests.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::DamageMembersOfTeam);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Team,
            "OwnedTeam".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_real(ParameterType::Real, 25.0))
        .unwrap();
    let mut owner = ExplicitCommandIngress::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(owner.calls.len(), 1);
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(_))
    ));
}

#[test]
fn move_named_unit_to_leaves_group_and_dispatches_ai_move() {
    // This checks explicit dispatcher admission; real C++ effect and peer-world
    // isolation assertions live in Main::owned_runtime_command_tests.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::MoveNamedUnitTo);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedMover".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "OwnedWaypoint".into(),
        ))
        .unwrap();
    let mut owner = ExplicitCommandIngress::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.calls,
        vec![(
            "MoveNamedUnitTo".into(),
            "NamedMover".into(),
            Some("OwnedWaypoint".into())
        )]
    );
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(_))
    ));
}

#[test]
fn named_follow_waypoints_leaves_group_and_selects_normal_loco() {
    // This checks explicit dispatcher admission; real C++ effect and peer-world
    // isolation assertions live in Main::owned_runtime_command_tests.
    let engine = ScriptEngine::new().unwrap();
    let context = std::cell::RefCell::new(ScriptContext::at_frame(47));
    let mut dispatcher = ScriptActionDispatcher::new(&engine, &context);
    let mut action = ScriptAction::new(ScriptActionType::NamedFollowWaypoints);
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "NamedFollower".into(),
        ))
        .unwrap();
    action
        .add_parameter(Parameter::with_string(
            ParameterType::Unit,
            "OwnedPath".into(),
        ))
        .unwrap();
    let mut owner = ExplicitCommandIngress::default();
    assert_eq!(
        dispatcher
            .execute_action_with_driver(&action, &mut owner)
            .unwrap(),
        ScriptActionResult::Success
    );
    assert_eq!(
        owner.calls,
        vec![(
            "NamedFollowWaypoints".into(),
            "NamedFollower".into(),
            Some("OwnedPath".into())
        )]
    );
    owner.reject = true;
    assert!(matches!(
        dispatcher.execute_action_with_driver(&action, &mut owner),
        Err(ScriptError::ExecutionFailed(_))
    ));
}

/// Payload-only contract owner. Gameplay effects are accepted against Main's
/// real owned objects, teams, terrain and sell list, not this recorder.
#[derive(Default)]
struct ExplicitCommandIngress {
    calls: Vec<(String, String, Option<String>)>,
    reject: bool,
}
impl ExplicitCommandIngress {
    fn admit(
        &mut self,
        kind: &str,
        first: &str,
        second: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        self.calls
            .push((kind.into(), first.into(), second.map(str::to_owned)));
        Some(if self.reject {
            Err(GameLogicError::ModuleError(
                "explicit command rejected".into(),
            ))
        } else {
            Ok(())
        })
    }
}
impl crate::scripting::engine::ScriptExecutionDriver for ExplicitCommandIngress {
    fn team_guard_object(
        &mut self,
        team: &str,
        target: &str,
        object: Option<ObjectID>,
        calling: Option<TeamID>,
        condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        assert_eq!((object, calling, condition), (None, None, None));
        self.admit("TeamGuardObject", team, Some(target))
    }
    fn team_damage(
        &mut self,
        team: &str,
        amount: f32,
        calling: Option<TeamID>,
        condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        assert_eq!((calling, condition), (None, None));
        assert_eq!(amount, 25.0);
        self.admit("DamageMembersOfTeam", team, None)
    }

    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn team_attack_team(
        &mut self,
        source: &str,
        target: &str,
        calling: Option<TeamID>,
        condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        assert_eq!((calling, condition), (None, None));
        self.admit("TeamAttackTeam", source, Some(target))
    }
    fn named_waypoint(
        &mut self,
        unit: &str,
        waypoint: &str,
        follow: Option<bool>,
        object: Option<ObjectID>,
    ) -> Option<GameLogicResult<()>> {
        assert_eq!(object, None);
        assert_ne!(follow, Some(true));
        self.admit(
            if follow.is_some() {
                "NamedFollowWaypoints"
            } else {
                "MoveNamedUnitTo"
            },
            unit,
            Some(waypoint),
        )
    }
    fn team_hunt_button(
        &mut self,
        team: &str,
        button: &str,
        calling: Option<TeamID>,
        condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        assert_eq!((calling, condition), (None, None));
        self.admit("TeamHuntWithCommandButton", team, Some(button))
    }
    fn team_transfer(
        &mut self,
        team: &str,
        player: &str,
        current: Option<&str>,
        calling: Option<TeamID>,
        condition: Option<TeamID>,
    ) -> Option<GameLogicResult<()>> {
        assert_eq!(current, None);
        assert_eq!((calling, condition), (None, None));
        self.admit("TeamTransferToPlayer", team, Some(player))
    }
    fn sell_everything(
        &mut self,
        player: &str,
        current: Option<&str>,
    ) -> Option<GameLogicResult<()>> {
        assert_eq!(current, None);
        self.admit("PlayerSellEverything", player, None)
    }
}

struct BorrowedWorldOwner<'a> {
    engine: &'a ScriptEngine,
    calls: Vec<String>,
}
impl BorrowedWorldOwner<'_> {
    fn record(&mut self, call: String) -> Option<GameLogicResult<()>> {
        self.calls.push(call);
        Some(
            self.engine
                .increment_counter("WorldHandlerImmediateReentry", 1),
        )
    }
}
impl ScriptExecutionDriver for BorrowedWorldOwner<'_> {
    fn after_action(&mut self) -> GameLogicResult<()> {
        Ok(())
    }
    fn world(
        &mut self,
        request: crate::scripting::engine::ScriptWorldRequest<'_>,
    ) -> Option<GameLogicResult<()>> {
        use crate::scripting::engine::ScriptWorldRequest;
        let call = match request {
            ScriptWorldRequest::MovieFullscreen(name) => format!("movie:{name}"),
            ScriptWorldRequest::RadarForced(forced) => format!("radar:{forced}"),
            ScriptWorldRequest::RadarRefreshTerrain => "radar_refresh_terrain".into(),
            ScriptWorldRequest::Timer {
                name,
                text,
                countdown,
            } => format!("timer:{name}:{text}:{countdown}"),
            ScriptWorldRequest::Freeze(true) => "freeze".into(),
            ScriptWorldRequest::Freeze(false) => "unfreeze".into(),
            ScriptWorldRequest::VisualSpeed(multiplier) => format!("speed:{multiplier}"),
            ScriptWorldRequest::Weather(visible) => format!("weather:{visible}"),
            ScriptWorldRequest::Music {
                track,
                fade_out,
                fade_in,
            } => format!("music:{track}:{fade_out}:{fade_in}"),
            ScriptWorldRequest::Input { .. } => panic!("unexpected input effect"),
        };
        self.record(call)
    }
    fn display(
        &mut self,
        request: crate::scripting::engine::ScriptDisplayRequest<'_>,
    ) -> Option<GameLogicResult<()>> {
        let crate::scripting::engine::ScriptDisplayRequest::Text(text) = request else {
            panic!("unexpected display effect")
        };
        self.record(format!("text:{text}"))
    }
}
