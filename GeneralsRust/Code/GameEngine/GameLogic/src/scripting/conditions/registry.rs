//! Condition registry for script evaluation.

use super::ScriptCondition;
use super::skirmish_conditions;
use std::collections::HashMap;

/// Condition registry
pub struct ConditionRegistry {
    conditions: HashMap<String, Box<dyn ScriptCondition>>,
}

impl ConditionRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            conditions: HashMap::new(),
        };

        // Register built-in conditions
        registry.register_builtin_conditions();

        // Register skirmish AI conditions
        skirmish_conditions::register_skirmish_conditions(&mut registry);

        registry
    }

    /// Register built-in conditions
    fn register_builtin_conditions(&mut self) {
        super::player::register_player_conditions(self);
        super::object::register_object_conditions(self);
        super::area::register_area_conditions(self);
        super::combat::register_combat_conditions(self);
        super::named::register_named_conditions(self);
        super::team::register_team_conditions(self);
        super::multiplayer::register_multiplayer_conditions(self);
        super::logic::register_logic_conditions(self);
        super::leftover::register_leftover_conditions(self);
    }

    /// Register a condition
    pub fn register_condition(&mut self, condition: Box<dyn ScriptCondition>) {
        self.conditions
            .insert(condition.name().to_string(), condition);
    }

    /// Get condition by name
    pub fn get_condition(&self, name: &str) -> Option<&dyn ScriptCondition> {
        self.conditions
            .get(name)
            .map(|condition| condition.as_ref())
    }

    /// List all available conditions
    pub fn list_conditions(&self) -> Vec<String> {
        self.conditions.keys().cloned().collect()
    }
}
