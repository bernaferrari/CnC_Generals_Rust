    use super::super::engine::initialize_script_engine;
    use super::*;

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
    fn test_victory_action() {
        initialize_script_engine().unwrap();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());

        let action = ScriptAction::new(ScriptActionType::Victory);
        evaluator.execute_action(&action).unwrap();

        // Check that end game timer was started
        let engine_guard = engine.read().unwrap();
        let engine = engine_guard.as_ref().unwrap();
        assert!(engine.is_game_ending());
    }
