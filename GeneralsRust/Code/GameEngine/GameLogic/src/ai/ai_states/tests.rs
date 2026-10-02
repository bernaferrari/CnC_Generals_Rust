#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_state_machine_creation() {
        let machine = AIStateMachine::new(123, "TestMachine".to_string());
        assert_eq!(machine.owner_id, 123);
        assert_eq!(machine.name, "TestMachine");
        assert!(machine.is_idle()); // Should start in idle state
    }

    #[test]
    fn test_state_transitions() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Test move to state
        machine.set_goal_position(Coord3D::new(100.0, 200.0, 0.0));
        machine.set_state(AIStateType::MoveTo);
        assert_eq!(machine.get_current_state_type(), Some(AIStateType::MoveTo));

        // Test attack state
        machine.set_goal_object(456);
        machine.set_state(AIStateType::AttackObject);
        assert_eq!(
            machine.get_current_state_type(),
            Some(AIStateType::AttackObject)
        );
        assert!(machine.is_in_attack_state());
    }

    #[test]
    fn test_ai_command_interface() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Test move command
        let mut params =
            AiCommandParams::new(AiCommandType::MoveToPosition, CommandSourceType::FromAi);
        params.pos = Coord3D::new(150.0, 250.0, 0.0);

        assert!(machine.ai_do_command(&params).is_ok());
        assert_eq!(machine.get_current_state_type(), Some(AIStateType::MoveTo));

        // Test attack command
        params.cmd = AiCommandType::AttackObject;
        params.obj = Some(789);

        assert!(machine.ai_do_command(&params).is_ok());
        assert_eq!(
            machine.get_current_state_type(),
            Some(AIStateType::AttackObject)
        );
    }

    #[test]
    fn test_temporary_state_without_existing_path_fails_and_clears_override() {
        let mut machine = AIStateMachine::new(0xAD_0001, "TestMachine".to_string());
        assert!(OBJECT_REGISTRY.get_object(0xAD_0001).is_none());
        // C++ AIMoveOutOfTheWayState::onEnter requires an existing AI path.
        // A missing owner cannot supply one; failed entry must not remain active.
        let result = machine.set_temporary_state(AIStateType::MoveOutOfTheWay, 100);
        assert_eq!(result, StateReturnType::Failed);
        assert!(machine.temporary_state.is_none());
        assert!(machine.temporary_state_frame_end.is_none());
    }

    #[test]
    fn test_ai_idle_state() {
        let mut idle_state = AIIdleState::new(true);
        assert!(idle_state.is_idle());
        assert_eq!(idle_state.get_state_type(), AIStateType::Idle);

        let mut context = AIStateMachineContext::default();
        let result = idle_state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Continue);
    }

    #[test]
    fn test_ai_attack_state_rejects_unregistered_owner() {
        let mut attack_state = AIAttackState::new(false, true, false, false);
        assert!(attack_state.is_attack());
        assert_eq!(attack_state.get_state_type(), AIStateType::AttackObject);

        let mut context = AIStateMachineContext::default();
        context.owner_id = 0xAD_0002;
        context.goal_object = Some(456);
        assert!(OBJECT_REGISTRY.get_object(context.owner_id).is_none());

        // No registered source, authored weapon, or target is supplied. C++
        // chooseWeapon requires a real source and usable weapon; this fixture
        // cannot enter a firing state.
        let result = attack_state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Failed);
    }

    #[test]
    fn test_move_and_tighten_state() {
        let mut tighten_state = AIMoveAndTightenState::new();
        assert_eq!(tighten_state.get_state_type(), AIStateType::MoveAndTighten);

        let mut context = AIStateMachineContext::default();
        context.goal_position = Some(Coord3D::new(100.0, 200.0, 0.0));

        let result = tighten_state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Continue);

        // Verify goal position was set
        assert_eq!(tighten_state.goal_position.x, 100.0);
        assert_eq!(tighten_state.goal_position.y, 200.0);
    }

    #[test]
    fn test_move_and_tighten_needs_tightening() {
        let tighten_state = AIMoveAndTightenState::new();

        // Tight formation - should not need tightening
        let tight_positions = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(5.0, 0.0, 0.0),
            Coord3D::new(0.0, 5.0, 0.0),
        ];
        assert!(!tighten_state.needs_tightening(&tight_positions));

        // Spread formation - should need tightening
        let spread_positions = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(100.0, 0.0, 0.0),
            Coord3D::new(0.0, 100.0, 0.0),
        ];
        assert!(tighten_state.needs_tightening(&spread_positions));
    }

    #[test]
    fn test_move_and_tighten_spread_calculation() {
        let tighten_state = AIMoveAndTightenState::new();

        let positions = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(0.0, 10.0, 0.0),
        ];

        let spread = tighten_state.get_group_spread(&positions);

        // Spread should be greater than 0
        assert!(spread > 0.0);
        // Spread should be reasonable for these positions
        assert!(spread < 20.0);
    }

    #[test]
    fn test_state_machine_move_and_tighten() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Set goal position and switch to MoveAndTighten state
        machine.set_goal_position(Coord3D::new(100.0, 200.0, 0.0));
        machine.set_state(AIStateType::MoveAndTighten);

        assert_eq!(
            machine.get_current_state_type(),
            Some(AIStateType::MoveAndTighten)
        );
    }

    #[test]
    fn test_temporary_move_and_tighten() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Set a temporary MoveAndTighten state
        machine.set_goal_position(Coord3D::new(50.0, 50.0, 0.0));
        let result = machine.set_temporary_state(AIStateType::MoveAndTighten, 100);
        assert_eq!(result, StateReturnType::Continue);

        // Check that temporary state is set
        assert!(machine.temporary_state.is_some());
        assert_eq!(
            machine.temporary_state_frame_end,
            Some(TheGameLogic::get_frame().wrapping_add(100))
        );
    }
    #[test]
    fn temporary_state_deadline_uses_logic_frame_and_cpp_one_minute_limit() {
        let _frame = crate::system::game_logic::enter_update_frame(37);
        let mut machine = AIStateMachine::new(123, "TemporaryDeadline".to_string());
        machine.set_goal_position(Coord3D::new(50.0, 50.0, 0.0));
        assert_eq!(
            machine.set_temporary_state(AIStateType::MoveAndTighten, 100),
            StateReturnType::Continue
        );
        assert_eq!(machine.temporary_state_frame_end, Some(137));
        assert_eq!(
            machine.set_temporary_state(AIStateType::MoveAndTighten, 10_000),
            StateReturnType::Continue
        );
        assert_eq!(
            machine.temporary_state_frame_end,
            Some(37 + 60 * LOGICFRAMES_PER_SECOND)
        );
    }

    #[derive(Debug)]
    struct TemporaryExitProbe;

    impl AIState for TemporaryExitProbe {
        fn on_enter(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
            StateReturnType::Continue
        }
        fn update(&mut self, _context: &mut AIStateMachineContext) -> StateReturnType {
            StateReturnType::Continue
        }
        fn on_exit(&mut self, context: &mut AIStateMachineContext, exit: StateExitType) {
            assert_eq!(exit, StateExitType::Reset);
            context.int_value += 1;
            // Entry of the replacement must observe this synchronous effect.
            context.goal_position = None;
        }
        fn get_state_type(&self) -> AIStateType {
            AIStateType::MoveAndTighten
        }
    }

    #[test]
    fn replacement_resets_previous_override_before_entering_and_exits_failed_entry() {
        let mut machine = AIStateMachine::new(0xAD_0003, "TemporaryReplacement".to_string());
        machine.set_goal_position(Coord3D::new(10.0, 20.0, 0.0));
        machine.temporary_state = Some(Box::new(TemporaryExitProbe));
        machine.temporary_state_frame_end = Some(100);
        assert_eq!(
            machine.set_temporary_state(AIStateType::MoveOutOfTheWay, 100),
            StateReturnType::Failed
        );
        assert_eq!(machine.context.int_value, 1);
        assert!(machine.context.goal_position.is_none());
        assert!(machine.temporary_state.is_none());
        assert!(machine.temporary_state_frame_end.is_none());
    }
}
