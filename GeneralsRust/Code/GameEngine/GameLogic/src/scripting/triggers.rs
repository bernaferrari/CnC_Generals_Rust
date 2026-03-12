//! Script Trigger System
//!
//! This module provides the trigger system for map scripting and campaigns.
//! Triggers combine conditions with actions and support one-shot vs repeating,
//! AND/OR logic, sequential script execution, and map editor integration.
//!
//! Matches C++ ScriptEngine trigger evaluation from ScriptEngine.cpp

use super::core::*;
use super::engine::ScriptEngine;
use super::evaluator::ScriptEvaluator;
use crate::{GameLogicError, GameLogicResult};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Trigger execution mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerMode {
    /// Execute once and disable
    OneShot,
    /// Execute repeatedly while condition is true
    Repeating,
    /// Execute once per condition becoming true (edge-triggered)
    OncePerTrue,
}

/// Trigger state for tracking execution
#[derive(Debug, Clone)]
pub struct TriggerState {
    /// Has the trigger been activated at least once
    pub activated: bool,
    /// Number of times the trigger has executed
    pub execution_count: u32,
    /// Frame number of last execution
    pub last_execution_frame: u32,
    /// Was the condition true last frame (for edge detection)
    pub was_condition_true: bool,
    /// Is the trigger currently enabled
    pub enabled: bool,
}

impl TriggerState {
    pub fn new() -> Self {
        Self {
            activated: false,
            execution_count: 0,
            last_execution_frame: 0,
            was_condition_true: false,
            enabled: true,
        }
    }
}

/// Trigger definition combining conditions, actions, and execution rules
/// Matches C++ Script class structure from ScriptEngine.h
#[derive(Debug, Clone)]
pub struct Trigger {
    /// Unique trigger name
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// Execution mode
    pub mode: TriggerMode,
    /// Minimum frames between executions (0 = no limit)
    pub min_delay_frames: u32,
    /// Condition tree (OR of AND chains)
    pub condition: Option<Box<OrCondition>>,
    /// Actions to execute when condition is true
    pub true_actions: Option<Box<ScriptAction>>,
    /// Actions to execute when condition is false (optional)
    pub false_actions: Option<Box<ScriptAction>>,
    /// Difficulty settings
    pub active_on_easy: bool,
    pub active_on_normal: bool,
    pub active_on_hard: bool,
    /// Runtime state
    pub state: TriggerState,
}

impl Trigger {
    pub fn new(name: String) -> Self {
        Self {
            name,
            description: String::new(),
            mode: TriggerMode::Repeating,
            min_delay_frames: 0,
            condition: None,
            true_actions: None,
            false_actions: None,
            active_on_easy: true,
            active_on_normal: true,
            active_on_hard: true,
            state: TriggerState::new(),
        }
    }

    /// Check if trigger should execute this frame
    /// Matches C++ ScriptEngine::EvaluateScripts logic
    pub fn should_evaluate(&self, current_frame: u32, difficulty: GameDifficulty) -> bool {
        // Check if enabled
        if !self.state.enabled {
            return false;
        }

        // Check difficulty setting
        match difficulty {
            GameDifficulty::Easy => {
                if !self.active_on_easy {
                    return false;
                }
            }
            GameDifficulty::Normal => {
                if !self.active_on_normal {
                    return false;
                }
            }
            GameDifficulty::Hard => {
                if !self.active_on_hard {
                    return false;
                }
            }
        }

        // Check one-shot constraint
        if self.mode == TriggerMode::OneShot && self.state.activated {
            return false;
        }

        // Check minimum delay
        if self.min_delay_frames > 0 {
            let frames_since_last = current_frame.saturating_sub(self.state.last_execution_frame);
            if frames_since_last < self.min_delay_frames {
                return false;
            }
        }

        true
    }

    /// Check if trigger is active
    pub fn is_active(&self) -> bool {
        self.state.enabled
    }

    /// Enable the trigger
    pub fn enable(&mut self) {
        self.state.enabled = true;
    }

    /// Disable the trigger
    pub fn disable(&mut self) {
        self.state.enabled = false;
    }

    /// Reset trigger state (for testing or scenario restart)
    pub fn reset(&mut self) {
        self.state = TriggerState::new();
    }
}

/// Game difficulty setting
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameDifficulty {
    Easy,
    Normal,
    Hard,
}

/// Trigger system manager
/// Coordinates trigger evaluation and execution
pub struct TriggerSystem {
    /// All registered triggers
    triggers: HashMap<String, Trigger>,
    /// Current game frame
    current_frame: u32,
    /// Current difficulty
    difficulty: GameDifficulty,
    /// Script engine reference
    engine: Arc<RwLock<Option<ScriptEngine>>>,
}

impl TriggerSystem {
    pub fn new(engine: Arc<RwLock<Option<ScriptEngine>>>) -> Self {
        Self {
            triggers: HashMap::new(),
            current_frame: 0,
            difficulty: GameDifficulty::Normal,
            engine,
        }
    }

    /// Register a trigger
    pub fn register_trigger(&mut self, trigger: Trigger) -> GameLogicResult<()> {
        let name = trigger.name.clone();
        if self.triggers.contains_key(&name) {
            return Err(GameLogicError::Configuration(format!(
                "Trigger '{}' already registered",
                name
            )));
        }
        self.triggers.insert(name, trigger);
        Ok(())
    }

    /// Get trigger by name
    pub fn get_trigger(&self, name: &str) -> Option<&Trigger> {
        self.triggers.get(name)
    }

    /// Get mutable trigger by name
    pub fn get_trigger_mut(&mut self, name: &str) -> Option<&mut Trigger> {
        self.triggers.get_mut(name)
    }

    /// Enable a trigger
    pub fn enable_trigger(&mut self, name: &str) -> GameLogicResult<()> {
        let trigger = self.triggers.get_mut(name).ok_or_else(|| {
            GameLogicError::Configuration(format!("Trigger '{}' not found", name))
        })?;
        trigger.enable();
        log::info!("Enabled trigger: {}", name);
        Ok(())
    }

    /// Disable a trigger
    pub fn disable_trigger(&mut self, name: &str) -> GameLogicResult<()> {
        let trigger = self.triggers.get_mut(name).ok_or_else(|| {
            GameLogicError::Configuration(format!("Trigger '{}' not found", name))
        })?;
        trigger.disable();
        log::info!("Disabled trigger: {}", name);
        Ok(())
    }

    /// Set game difficulty
    pub fn set_difficulty(&mut self, difficulty: GameDifficulty) {
        self.difficulty = difficulty;
    }

    /// Update trigger system (called each frame)
    /// Matches C++ ScriptEngine::Update -> EvaluateScripts
    pub fn update(&mut self) -> GameLogicResult<()> {
        self.current_frame += 1;

        // Collect triggers that need evaluation
        let mut triggers_to_evaluate = Vec::new();
        for (name, trigger) in &self.triggers {
            if trigger.should_evaluate(self.current_frame, self.difficulty) {
                triggers_to_evaluate.push(name.clone());
            }
        }

        // Evaluate each trigger
        for trigger_name in triggers_to_evaluate {
            self.evaluate_trigger(&trigger_name)?;
        }

        Ok(())
    }

    /// Evaluate a single trigger
    /// Matches C++ ScriptEngine::EvaluateScripts single script evaluation
    pub fn evaluate_trigger(&mut self, name: &str) -> GameLogicResult<bool> {
        // Take ownership of the trigger to avoid borrow conflicts while executing actions.
        let mut trigger = self.triggers.remove(name).ok_or_else(|| {
            GameLogicError::Configuration(format!("Trigger '{}' not found", name))
        })?;

        log::debug!("Evaluating trigger: {}", name);

        // Evaluate conditions
        let condition_result = if let Some(or_condition) = trigger.condition.as_deref_mut() {
            self.evaluate_or_condition(or_condition)?
        } else {
            true // No condition means always true
        };

        // Check for edge-triggered mode
        let should_execute = match trigger.mode {
            TriggerMode::OneShot => condition_result && !trigger.state.activated,
            TriggerMode::Repeating => condition_result,
            TriggerMode::OncePerTrue => condition_result && !trigger.state.was_condition_true,
        };

        // Update state
        trigger.state.was_condition_true = condition_result;

        if should_execute {
            log::info!("Trigger '{}' condition is TRUE, executing actions", name);

            // Execute true actions
            if let Some(true_actions) = &trigger.true_actions {
                self.execute_action_sequence(true_actions)?;
            }

            // Update execution state
            trigger.state.activated = true;
            trigger.state.execution_count += 1;
            trigger.state.last_execution_frame = self.current_frame;

            // Disable if one-shot
            if trigger.mode == TriggerMode::OneShot {
                trigger.state.enabled = false;
                log::info!("One-shot trigger '{}' has been disabled", name);
            }
        } else if !condition_result {
            // Execute false actions if provided
            if let Some(false_actions) = &trigger.false_actions {
                log::debug!(
                    "Trigger '{}' condition is FALSE, executing false actions",
                    name
                );
                self.execute_action_sequence(false_actions)?;
            }
        }

        // Re-insert the trigger with its updated state
        self.triggers.insert(trigger.name.clone(), trigger);

        Ok(should_execute)
    }

    /// Evaluate OR condition tree
    /// Matches C++ ScriptEngine::EvaluateConditions
    fn evaluate_or_condition(&self, or_condition: &mut OrCondition) -> GameLogicResult<bool> {
        let evaluator = ScriptEvaluator::new(self.engine.clone());
        evaluator.evaluate_or_condition(or_condition)
    }

    /// Evaluate AND condition chain
    fn evaluate_and_condition(&self, and_condition: &mut Condition) -> GameLogicResult<bool> {
        let evaluator = ScriptEvaluator::new(self.engine.clone());
        evaluator.evaluate_and_condition(and_condition)
    }

    /// Evaluate a single condition
    /// Matches C++ ScriptConditions::Evaluate
    fn evaluate_single_condition(&self, condition: &mut Condition) -> GameLogicResult<bool> {
        let evaluator = ScriptEvaluator::new(self.engine.clone());
        evaluator.evaluate_condition(condition)
    }

    /// Evaluate counter condition
    /// Matches C++ ScriptConditions::EvaluateCounter
    fn evaluate_counter_condition(
        engine: &ScriptEngine,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let counter_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing counter parameter".to_string())
        })?;
        let comparison_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing comparison".to_string())
        })?;
        let value_param = condition.get_parameter(2).ok_or_else(|| {
            GameLogicError::Configuration("Counter condition missing value".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let comparison_type = comparison_param.get_int() as u32;
        let target_value = value_param.get_int();

        if let Some(counter) = engine.get_counter(counter_name) {
            let current_value = counter.value;
            match comparison_type {
                0 => Ok(current_value < target_value),  // LessThan
                1 => Ok(current_value <= target_value), // LessEqual
                2 => Ok(current_value == target_value), // Equal
                3 => Ok(current_value >= target_value), // GreaterEqual
                4 => Ok(current_value > target_value),  // Greater
                5 => Ok(current_value != target_value), // NotEqual
                _ => Err(GameLogicError::Configuration(format!(
                    "Invalid comparison type: {}",
                    comparison_type
                ))),
            }
        } else {
            // Counter doesn't exist, treat as 0
            Ok(false)
        }
    }

    /// Evaluate flag condition
    /// Matches C++ ScriptConditions::EvaluateFlag
    fn evaluate_flag_condition(
        engine: &ScriptEngine,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let flag_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Flag condition missing flag parameter".to_string())
        })?;
        let expected_param = condition.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("Flag condition missing expected value".to_string())
        })?;

        let flag_name = flag_param.get_string();
        let expected_value = expected_param.get_int() != 0;

        if let Some(flag) = engine.get_flag(flag_name) {
            Ok(flag.value == expected_value)
        } else {
            // Flag doesn't exist, treat as false
            Ok(!expected_value)
        }
    }

    /// Evaluate timer expired condition
    /// Matches C++ ScriptConditions::EvaluateTimerExpired
    fn evaluate_timer_expired(
        engine: &ScriptEngine,
        condition: &Condition,
    ) -> GameLogicResult<bool> {
        let timer_param = condition.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("Timer condition missing timer parameter".to_string())
        })?;

        let timer_name = timer_param.get_string();

        if let Some(counter) = engine.get_counter(timer_name) {
            // Timer expired if it's a countdown timer and value is <= 0
            Ok(counter.is_countdown_timer && counter.value <= 0)
        } else {
            Ok(false)
        }
    }

    /// Execute action sequence
    /// Matches C++ ScriptActions::Execute
    fn execute_action_sequence(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let evaluator = ScriptEvaluator::new(self.engine.clone());
        evaluator.execute_action_sequence(action)
    }

    /// Execute a single action (delegates to ScriptEvaluator for full parity).
    fn execute_single_action(&self, action: &ScriptAction) -> GameLogicResult<()> {
        let evaluator = ScriptEvaluator::new(self.engine.clone());
        evaluator.execute_action(action)
    }

    /// Execute set flag action
    /// Matches C++ ScriptActions::doSetFlag
    fn execute_set_flag(engine: &mut ScriptEngine, action: &ScriptAction) -> GameLogicResult<()> {
        let flag_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetFlag action missing flag parameter".to_string())
        })?;
        let value_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetFlag action missing value parameter".to_string())
        })?;

        let flag_name = flag_param.get_string();
        let value = value_param.get_int() != 0;

        engine.set_flag(flag_name, value)?;
        log::debug!("Set flag '{}' to {}", flag_name, value);
        Ok(())
    }

    /// Execute set counter action
    /// Matches C++ ScriptActions::doSetCounter
    fn execute_set_counter(
        engine: &mut ScriptEngine,
        action: &ScriptAction,
    ) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetCounter action missing counter parameter".to_string())
        })?;
        let value_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetCounter action missing value parameter".to_string())
        })?;

        let counter_name = counter_param.get_string();
        let value = value_param.get_int();

        engine.set_counter(counter_name, value)?;
        log::debug!("Set counter '{}' to {}", counter_name, value);
        Ok(())
    }

    /// Execute set timer action
    /// Matches C++ ScriptActions::doSetTimer
    fn execute_set_timer(engine: &mut ScriptEngine, action: &ScriptAction) -> GameLogicResult<()> {
        let timer_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration("SetTimer action missing timer parameter".to_string())
        })?;
        let seconds_param = action.get_parameter(1).ok_or_else(|| {
            GameLogicError::Configuration("SetTimer action missing seconds parameter".to_string())
        })?;

        let timer_name = timer_param.get_string();
        let seconds = seconds_param.get_real();

        engine.set_timer_seconds(timer_name, seconds)?;
        log::debug!("Set timer '{}' to {} seconds", timer_name, seconds);
        Ok(())
    }

    /// Execute increment counter action
    /// Matches C++ ScriptActions::doIncrementCounter
    fn execute_increment_counter(
        engine: &mut ScriptEngine,
        action: &ScriptAction,
    ) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "IncrementCounter action missing counter parameter".to_string(),
            )
        })?;

        let counter_name = counter_param.get_string();
        engine.increment_counter(counter_name)?;
        log::debug!("Incremented counter '{}'", counter_name);
        Ok(())
    }

    /// Execute decrement counter action
    /// Matches C++ ScriptActions::doDecrementCounter
    fn execute_decrement_counter(
        engine: &mut ScriptEngine,
        action: &ScriptAction,
    ) -> GameLogicResult<()> {
        let counter_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "DecrementCounter action missing counter parameter".to_string(),
            )
        })?;

        let counter_name = counter_param.get_string();
        engine.decrement_counter(counter_name)?;
        log::debug!("Decremented counter '{}'", counter_name);
        Ok(())
    }

    /// Execute enable script action
    fn execute_enable_script(action: &ScriptAction) -> GameLogicResult<()> {
        let script_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "EnableScript action missing script parameter".to_string(),
            )
        })?;

        let script_name = script_param.get_string();
        log::info!("Enable script '{}'", script_name);
        // In real implementation, enable the named script/trigger
        Ok(())
    }

    /// Execute disable script action
    fn execute_disable_script(action: &ScriptAction) -> GameLogicResult<()> {
        let script_param = action.get_parameter(0).ok_or_else(|| {
            GameLogicError::Configuration(
                "DisableScript action missing script parameter".to_string(),
            )
        })?;

        let script_name = script_param.get_string();
        log::info!("Disable script '{}'", script_name);
        // In real implementation, disable the named script/trigger
        Ok(())
    }

    /// List all registered triggers
    pub fn list_triggers(&self) -> Vec<String> {
        self.triggers.keys().cloned().collect()
    }

    /// Get current frame number
    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Reset all triggers
    pub fn reset_all(&mut self) {
        for trigger in self.triggers.values_mut() {
            trigger.reset();
        }
        self.current_frame = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_engine() -> Arc<RwLock<Option<ScriptEngine>>> {
        let engine = ScriptEngine::new().unwrap();
        Arc::new(RwLock::new(Some(engine)))
    }

    #[test]
    fn test_trigger_creation() {
        let trigger = Trigger::new("test_trigger".to_string());
        assert_eq!(trigger.name, "test_trigger");
        assert!(trigger.is_active());
        assert_eq!(trigger.mode, TriggerMode::Repeating);
    }

    #[test]
    fn test_trigger_enable_disable() {
        let mut trigger = Trigger::new("test".to_string());
        assert!(trigger.is_active());

        trigger.disable();
        assert!(!trigger.is_active());

        trigger.enable();
        assert!(trigger.is_active());
    }

    #[test]
    fn test_one_shot_trigger() {
        let mut trigger = Trigger::new("one_shot".to_string());
        trigger.mode = TriggerMode::OneShot;

        // First evaluation
        assert!(trigger.should_evaluate(1, GameDifficulty::Normal));

        // Mark as activated
        trigger.state.activated = true;

        // Second evaluation should return false
        assert!(!trigger.should_evaluate(2, GameDifficulty::Normal));
    }

    #[test]
    fn test_trigger_system_registration() {
        let engine = create_test_engine();
        let mut system = TriggerSystem::new(engine);

        let trigger = Trigger::new("test".to_string());
        system.register_trigger(trigger).unwrap();

        assert!(system.get_trigger("test").is_some());
        assert_eq!(system.list_triggers().len(), 1);
    }

    #[test]
    fn test_counter_condition_evaluation() {
        let engine = create_test_engine();
        {
            let mut eng = engine.write().unwrap();
            let eng = eng.as_mut().unwrap();
            eng.set_counter("test_counter", 42).unwrap();
        }

        let mut condition = Condition::new(ConditionType::Counter);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "test_counter".to_string(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Comparison, 2)) // Equal
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Int, 42))
            .unwrap();

        let eng = engine.read().unwrap();
        let eng = eng.as_ref().unwrap();
        let result = TriggerSystem::evaluate_counter_condition(eng, &condition).unwrap();
        assert!(result);
    }

    #[test]
    fn test_flag_condition_evaluation() {
        let engine = create_test_engine();
        {
            let mut eng = engine.write().unwrap();
            let eng = eng.as_mut().unwrap();
            eng.set_flag("test_flag", true).unwrap();
        }

        let mut condition = Condition::new(ConditionType::Flag);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Flag,
                "test_flag".to_string(),
            ))
            .unwrap();
        condition
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1)) // true
            .unwrap();

        let eng = engine.read().unwrap();
        let eng = eng.as_ref().unwrap();
        let result = TriggerSystem::evaluate_flag_condition(eng, &condition).unwrap();
        assert!(result);
    }

    #[test]
    fn test_timer_expired_condition() {
        let engine = create_test_engine();
        {
            let mut eng = engine.write().unwrap();
            let eng = eng.as_mut().unwrap();
            eng.set_timer("test_timer", 0).unwrap(); // Expired timer
        }

        let mut condition = Condition::new(ConditionType::TimerExpired);
        condition
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "test_timer".to_string(),
            ))
            .unwrap();

        let eng = engine.read().unwrap();
        let eng = eng.as_ref().unwrap();
        let result = TriggerSystem::evaluate_timer_expired(eng, &condition).unwrap();
        assert!(result);
    }

    #[test]
    fn test_set_flag_action() {
        let engine = create_test_engine();

        let mut action = ScriptAction::new(ScriptActionType::SetFlag);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Flag,
                "my_flag".to_string(),
            ))
            .unwrap();
        action
            .add_parameter(Parameter::with_int(ParameterType::Boolean, 1))
            .unwrap();

        {
            let mut eng = engine.write().unwrap();
            let eng = eng.as_mut().unwrap();
            TriggerSystem::execute_set_flag(eng, &action).unwrap();
        }

        let eng = engine.read().unwrap();
        let eng = eng.as_ref().unwrap();
        let flag = eng.get_flag("my_flag").unwrap();
        assert!(flag.value);
    }

    #[test]
    fn test_increment_counter_action() {
        let engine = create_test_engine();

        let mut action = ScriptAction::new(ScriptActionType::IncrementCounter);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Counter,
                "my_counter".to_string(),
            ))
            .unwrap();

        {
            let mut eng = engine.write().unwrap();
            let eng = eng.as_mut().unwrap();
            eng.set_counter("my_counter", 10).unwrap();
            TriggerSystem::execute_increment_counter(eng, &action).unwrap();
        }

        let eng = engine.read().unwrap();
        let eng = eng.as_ref().unwrap();
        let counter = eng.get_counter("my_counter").unwrap();
        assert_eq!(counter.value, 11);
    }
}
