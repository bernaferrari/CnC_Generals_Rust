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
        machine.set_goal_position([100.0, 200.0, 0.0]);
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
        params.pos = [150.0, 250.0, 0.0];

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
    fn test_temporary_states() {
        let mut machine = AIStateMachine::new(123, "TestMachine".to_string());

        // Set a temporary state
        let result = machine.set_temporary_state(AIStateType::MoveOutOfTheWay, 100);
        assert_eq!(result, StateReturnType::Continue);

        // Check that temporary state is set
        assert!(machine.temporary_state.is_some());
        assert_eq!(machine.temporary_state_frame_end, Some(100));
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
    fn test_ai_attack_state() {
        let mut attack_state = AIAttackState::new(false, true, false, false);
        assert!(attack_state.is_attack());
        assert_eq!(attack_state.get_state_type(), AIStateType::AttackObject);

        let mut context = AIStateMachineContext::default();
        context.goal_object = Some(456);

        let result = attack_state.on_enter(&mut context);
        assert_eq!(result, StateReturnType::Continue);
    }

    #[test]
    fn test_move_and_tighten_state() {
        let mut tighten_state = AIMoveAndTightenState::new();
        assert_eq!(tighten_state.get_state_type(), AIStateType::MoveAndTighten);

        let mut context = AIStateMachineContext::default();
        context.goal_position = Some([100.0, 200.0, 0.0]);

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
        let tight_positions = vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0], [0.0, 5.0, 0.0]];
        assert!(!tighten_state.needs_tightening(&tight_positions));

        // Spread formation - should need tightening
        let spread_positions = vec![[0.0, 0.0, 0.0], [100.0, 0.0, 0.0], [0.0, 100.0, 0.0]];
        assert!(tighten_state.needs_tightening(&spread_positions));
    }

    #[test]
    fn test_move_and_tighten_spread_calculation() {
        let tighten_state = AIMoveAndTightenState::new();

        let positions = vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];

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
        machine.set_goal_position([100.0, 200.0, 0.0]);
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
        machine.set_goal_position([50.0, 50.0, 0.0]);
        let result = machine.set_temporary_state(AIStateType::MoveAndTighten, 100);
        assert_eq!(result, StateReturnType::Continue);

        // Check that temporary state is set
        assert!(machine.temporary_state.is_some());
        assert_eq!(machine.temporary_state_frame_end, Some(100));
    }
}
