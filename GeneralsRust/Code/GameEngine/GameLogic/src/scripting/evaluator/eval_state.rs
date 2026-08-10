// Counter, flag, and timer condition evaluators
//
// Split from `scripting/evaluator.rs` for module-size parity.
// Observable behavior is unchanged.

impl ScriptEvaluator {
    fn evaluate_counter_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let counter_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing counter parameter".to_string())
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration(
                "Counter condition missing comparison parameter".to_string(),
            )
        })?;
        let value_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing value parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let comparison = comparison_param.get_int() as u32;
        let target_value = value_param.get_int();

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(counter) = engine.get_counter(counter_name) {
            let current_value = counter.value;
            match comparison {
                0 => Ok(current_value < target_value),  // LessThan
                1 => Ok(current_value <= target_value), // LessEqual
                2 => Ok(current_value == target_value), // Equal
                3 => Ok(current_value >= target_value), // GreaterEqual
                4 => Ok(current_value > target_value),  // Greater
                5 => Ok(current_value != target_value), // NotEqual
                _ => Err(GameLogicError::Configuration(format!(
                    "Invalid comparison type: {}",
                    comparison
                ))),
            }
        } else {
            Ok(false) // Counter doesn't exist
        }
    }

    /// Evaluate flag condition
    fn evaluate_flag_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let flag_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Flag condition missing flag parameter".to_string())
        })?;
        let value_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("Flag condition missing value parameter".to_string())
        })?;

        let flag_name = flag_param.get_string();
        let target_value = value_param.get_int() != 0;

        // Prefer re-entrant global helper so nested CALL_SUBROUTINE paths don't
        // deadlock on std::sync::RwLock when the engine write lock is held.
        if let Some(result) = super::engine::with_script_engine_ref(|engine| {
            if let Some(flag) = engine.get_flag(flag_name) {
                flag.value == target_value
            } else {
                false
            }
        }) {
            return Ok(result);
        }

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(flag) = engine.get_flag(flag_name) {
            Ok(flag.value == target_value)
        } else {
            Ok(false) // Flag doesn't exist
        }
    }

    /// Evaluate timer expired condition
    fn evaluate_timer_expired_condition(&self, condition: &Condition) -> GameLogicResult<bool> {
        let counter_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Timer condition missing counter parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();

        if let Some(result) = super::engine::with_script_engine_ref(|engine| {
            if let Some(counter) = engine.get_counter(counter_name) {
                counter.is_countdown_timer && counter.value < 1
            } else {
                false
            }
        }) {
            return Ok(result);
        }

        let engine = self.engine.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire engine lock: {}", e))
        })?;
        let engine = engine.as_ref().ok_or_else(|| {
            GameLogicError::Configuration("Script engine not initialized".to_string())
        })?;

        if let Some(counter) = engine.get_counter(counter_name) {
            if !counter.is_countdown_timer {
                return Ok(false);
            }
            Ok(counter.value < 1)
        } else {
            Ok(false) // Timer doesn't exist
        }
    }
}
