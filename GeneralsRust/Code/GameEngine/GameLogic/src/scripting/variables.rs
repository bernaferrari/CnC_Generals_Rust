//! Variable Scope Management
//!
//! Implements hierarchical variable scoping for script variables, matching
//! C++ ScriptEngine variable behavior. Variables can exist at different scope
//! levels with proper shadowing and resolution semantics.
//!
//! # Scope Hierarchy
//!
//! Variables are resolved in the following order (most local to most global):
//! 1. **Script-Local**: Variables defined within a specific script instance
//! 2. **Team**: Variables shared across all scripts for a specific team
//! 3. **Map**: Variables shared across all scripts in the current map
//! 4. **Global**: Variables shared across all maps and scripts
//!
//! # Examples
//!
//! ```rust
//! use gamelogic::scripting::{ScriptValue, VariableScope, VariableScopeManager};
//!
//! let mut manager = VariableScopeManager::new();
//!
//! // Set a global variable
//! manager.set_variable(
//!     VariableScope::Global,
//!     "difficulty",
//!     ScriptValue::String("hard".to_string()),
//!     None,
//! ).unwrap();
//!
//! // Set a script-local variable that shadows the global
//! manager.set_variable(
//!     VariableScope::ScriptLocal,
//!     "difficulty",
//!     ScriptValue::String("easy".to_string()),
//!     Some("my_script"),
//! ).unwrap();
//!
//! // Get variable - script-local shadows global
//! let value = manager.get_variable("difficulty", Some("my_script"), None);
//! assert_eq!(value, Some(&ScriptValue::String("easy".to_string())));
//!
//! // Get from different script - uses global
//! let value = manager.get_variable("difficulty", Some("other_script"), None);
//! assert_eq!(value, Some(&ScriptValue::String("hard".to_string())));
//! ```

use super::ScriptValue;
use crate::{GameLogicError, GameLogicResult};
use std::collections::HashMap;

/// Variable scope levels matching C++ ScriptEngine scoping
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VariableScope {
    /// Global scope - accessible from all scripts across all maps
    /// Persists for entire game session
    Global,

    /// Map scope - accessible within current map only
    /// Cleared when map changes
    Map,

    /// Team scope - accessible within team scripts only
    /// Separate namespace per team
    Team,

    /// Script-local scope - accessible only within specific script instance
    /// Most local scope, shadows all others
    ScriptLocal,
}

/// Scoped variable storage with hierarchical resolution
///
/// Matches C++ ScriptEngine variable management, providing proper scoping
/// semantics for script variables.
pub struct VariableScopeManager {
    /// Global variables - persist across maps
    global_vars: HashMap<String, ScriptValue>,

    /// Map-level variables - cleared on map change
    map_vars: HashMap<String, ScriptValue>,

    /// Team-level variables - keyed by team name
    team_vars: HashMap<String, HashMap<String, ScriptValue>>,

    /// Script-local variables - keyed by script name
    script_vars: HashMap<String, HashMap<String, ScriptValue>>,
}

impl VariableScopeManager {
    /// Create a new variable scope manager
    pub fn new() -> Self {
        Self {
            global_vars: HashMap::new(),
            map_vars: HashMap::new(),
            team_vars: HashMap::new(),
            script_vars: HashMap::new(),
        }
    }

    /// Set variable in specific scope
    ///
    /// # Arguments
    /// * `scope` - The scope level to set the variable in
    /// * `name` - Variable name
    /// * `value` - Variable value
    /// * `context_name` - Team or script name for Team/ScriptLocal scopes
    ///
    /// # Errors
    /// Returns error if context_name is required but not provided
    ///
    /// # Example
    /// ```rust
    /// # use gamelogic::scripting::{ScriptValue, VariableScope, VariableScopeManager};
    /// # let mut manager = VariableScopeManager::new();
    /// // Global variable
    /// manager.set_variable(
    ///     VariableScope::Global,
    ///     "player_score",
    ///     ScriptValue::Int(1000),
    ///     None,
    /// ).unwrap();
    ///
    /// // Team variable
    /// manager.set_variable(
    ///     VariableScope::Team,
    ///     "team_kills",
    ///     ScriptValue::Int(5),
    ///     Some("teamPlayer"),
    /// ).unwrap();
    /// ```
    pub fn set_variable(
        &mut self,
        scope: VariableScope,
        name: &str,
        value: ScriptValue,
        context_name: Option<&str>,
    ) -> GameLogicResult<()> {
        match scope {
            VariableScope::Global => {
                log::trace!("Setting global variable '{}' = {:?}", name, value);
                self.global_vars.insert(name.to_string(), value);
            }
            VariableScope::Map => {
                log::trace!("Setting map variable '{}' = {:?}", name, value);
                self.map_vars.insert(name.to_string(), value);
            }
            VariableScope::Team => {
                let team_name = context_name.ok_or_else(|| {
                    GameLogicError::Configuration(
                        "Team name required for team-scoped variable".to_string(),
                    )
                })?;
                log::trace!(
                    "Setting team variable '{}' = {:?} for team '{}'",
                    name,
                    value,
                    team_name
                );
                self.team_vars
                    .entry(team_name.to_string())
                    .or_insert_with(HashMap::new)
                    .insert(name.to_string(), value);
            }
            VariableScope::ScriptLocal => {
                let script_name = context_name.ok_or_else(|| {
                    GameLogicError::Configuration(
                        "Script name required for script-local variable".to_string(),
                    )
                })?;
                log::trace!(
                    "Setting script-local variable '{}' = {:?} for script '{}'",
                    name,
                    value,
                    script_name
                );
                self.script_vars
                    .entry(script_name.to_string())
                    .or_insert_with(HashMap::new)
                    .insert(name.to_string(), value);
            }
        }
        Ok(())
    }

    /// Get variable with scope resolution
    ///
    /// Searches scopes in order: ScriptLocal -> Team -> Map -> Global
    /// Returns the first match found (most local scope wins).
    ///
    /// # Arguments
    /// * `name` - Variable name to look up
    /// * `script_name` - Current script name (for script-local lookup)
    /// * `team_name` - Current team name (for team lookup)
    ///
    /// # Returns
    /// Reference to variable value, or None if not found in any scope
    ///
    /// # Example
    /// ```rust
    /// # use gamelogic::scripting::{ScriptValue, VariableScope, VariableScopeManager};
    /// # let mut manager = VariableScopeManager::new();
    /// # manager.set_variable(VariableScope::Global, "var", ScriptValue::Int(1), None).unwrap();
    /// # manager.set_variable(VariableScope::Map, "var", ScriptValue::Int(2), None).unwrap();
    /// # manager.set_variable(VariableScope::Team, "var", ScriptValue::Int(3), Some("team1")).unwrap();
    /// # manager.set_variable(VariableScope::ScriptLocal, "var", ScriptValue::Int(4), Some("script1")).unwrap();
    /// // Most local scope (script-local) wins
    /// let value = manager.get_variable("var", Some("script1"), Some("team1"));
    /// assert_eq!(value, Some(&ScriptValue::Int(4)));
    ///
    /// // Without script context, team scope wins
    /// let value = manager.get_variable("var", None, Some("team1"));
    /// assert_eq!(value, Some(&ScriptValue::Int(3)));
    ///
    /// // Without team context, map scope wins
    /// let value = manager.get_variable("var", None, None);
    /// assert_eq!(value, Some(&ScriptValue::Int(2)));
    /// ```
    pub fn get_variable(
        &self,
        name: &str,
        script_name: Option<&str>,
        team_name: Option<&str>,
    ) -> Option<&ScriptValue> {
        // Search order: ScriptLocal -> Team -> Map -> Global

        // 1. Check script-local scope (most local)
        if let Some(script) = script_name {
            if let Some(vars) = self.script_vars.get(script) {
                if let Some(value) = vars.get(name) {
                    log::trace!(
                        "Variable '{}' found in script-local scope (script '{}')",
                        name,
                        script
                    );
                    return Some(value);
                }
            }
        }

        // 2. Check team scope
        if let Some(team) = team_name {
            if let Some(vars) = self.team_vars.get(team) {
                if let Some(value) = vars.get(name) {
                    log::trace!("Variable '{}' found in team scope (team '{}')", name, team);
                    return Some(value);
                }
            }
        }

        // 3. Check map scope
        if let Some(value) = self.map_vars.get(name) {
            log::trace!("Variable '{}' found in map scope", name);
            return Some(value);
        }

        // 4. Check global scope (most global)
        if let Some(value) = self.global_vars.get(name) {
            log::trace!("Variable '{}' found in global scope", name);
            return Some(value);
        }

        log::trace!("Variable '{}' not found in any scope", name);
        None
    }

    /// Get mutable variable reference with scope resolution
    ///
    /// Same as get_variable but returns a mutable reference.
    pub fn get_variable_mut(
        &mut self,
        name: &str,
        script_name: Option<&str>,
        team_name: Option<&str>,
    ) -> Option<&mut ScriptValue> {
        // Search order: ScriptLocal -> Team -> Map -> Global

        // 1. Check script-local scope
        if let Some(script) = script_name {
            if let Some(vars) = self.script_vars.get_mut(script) {
                if let Some(value) = vars.get_mut(name) {
                    return Some(value);
                }
            }
        }

        // 2. Check team scope
        if let Some(team) = team_name {
            if let Some(vars) = self.team_vars.get_mut(team) {
                if let Some(value) = vars.get_mut(name) {
                    return Some(value);
                }
            }
        }

        // 3. Check map scope
        if let Some(value) = self.map_vars.get_mut(name) {
            return Some(value);
        }

        // 4. Check global scope
        self.global_vars.get_mut(name)
    }

    /// Clear all variables in a specific scope
    ///
    /// # Arguments
    /// * `scope` - Scope to clear
    /// * `context_name` - Team or script name for Team/ScriptLocal scopes
    pub fn clear_scope(&mut self, scope: VariableScope, context_name: Option<&str>) {
        match scope {
            VariableScope::Global => {
                log::debug!("Clearing global scope ({} vars)", self.global_vars.len());
                self.global_vars.clear();
            }
            VariableScope::Map => {
                log::debug!("Clearing map scope ({} vars)", self.map_vars.len());
                self.map_vars.clear();
            }
            VariableScope::Team => {
                if let Some(team) = context_name {
                    if let Some(vars) = self.team_vars.remove(team) {
                        log::debug!("Clearing team scope for '{}' ({} vars)", team, vars.len());
                    }
                }
            }
            VariableScope::ScriptLocal => {
                if let Some(script) = context_name {
                    if let Some(vars) = self.script_vars.remove(script) {
                        log::debug!(
                            "Clearing script-local scope for '{}' ({} vars)",
                            script,
                            vars.len()
                        );
                    }
                }
            }
        }
    }

    /// Clear all variables in all scopes
    ///
    /// Use when resetting the entire scripting system (e.g., on map change).
    pub fn clear_all(&mut self) {
        log::debug!("Clearing all variable scopes");
        self.global_vars.clear();
        self.map_vars.clear();
        self.team_vars.clear();
        self.script_vars.clear();
    }

    /// Get count of variables in a specific scope
    pub fn count_scope(&self, scope: VariableScope, context_name: Option<&str>) -> usize {
        match scope {
            VariableScope::Global => self.global_vars.len(),
            VariableScope::Map => self.map_vars.len(),
            VariableScope::Team => context_name
                .and_then(|team| self.team_vars.get(team))
                .map(|vars| vars.len())
                .unwrap_or(0),
            VariableScope::ScriptLocal => context_name
                .and_then(|script| self.script_vars.get(script))
                .map(|vars| vars.len())
                .unwrap_or(0),
        }
    }

    /// Get total variable count across all scopes
    pub fn total_count(&self) -> usize {
        let team_count: usize = self.team_vars.values().map(|vars| vars.len()).sum();
        let script_count: usize = self.script_vars.values().map(|vars| vars.len()).sum();

        self.global_vars.len() + self.map_vars.len() + team_count + script_count
    }

    /// List all variables in a scope
    pub fn list_scope_variables(
        &self,
        scope: VariableScope,
        context_name: Option<&str>,
    ) -> Vec<(String, ScriptValue)> {
        match scope {
            VariableScope::Global => self
                .global_vars
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            VariableScope::Map => self
                .map_vars
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            VariableScope::Team => context_name
                .and_then(|team| self.team_vars.get(team))
                .map(|vars| vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
            VariableScope::ScriptLocal => context_name
                .and_then(|script| self.script_vars.get(script))
                .map(|vars| vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
                .unwrap_or_default(),
        }
    }
}

impl Default for VariableScopeManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_global_variables() {
        let mut manager = VariableScopeManager::new();

        manager
            .set_variable(
                VariableScope::Global,
                "test_var",
                ScriptValue::Int(42),
                None,
            )
            .unwrap();

        let value = manager.get_variable("test_var", None, None).unwrap();
        assert_eq!(*value, ScriptValue::Int(42));
    }

    #[test]
    fn test_map_variables() {
        let mut manager = VariableScopeManager::new();

        manager
            .set_variable(VariableScope::Map, "map_var", ScriptValue::Int(100), None)
            .unwrap();

        let value = manager.get_variable("map_var", None, None).unwrap();
        assert_eq!(*value, ScriptValue::Int(100));
    }

    #[test]
    fn test_team_variables() {
        let mut manager = VariableScopeManager::new();

        manager
            .set_variable(
                VariableScope::Team,
                "team_var",
                ScriptValue::Int(50),
                Some("team1"),
            )
            .unwrap();

        // Should find it with team context
        let value = manager
            .get_variable("team_var", None, Some("team1"))
            .unwrap();
        assert_eq!(*value, ScriptValue::Int(50));

        // Should not find it without team context
        assert!(manager.get_variable("team_var", None, None).is_none());
    }

    #[test]
    fn test_script_local_variables() {
        let mut manager = VariableScopeManager::new();

        manager
            .set_variable(
                VariableScope::ScriptLocal,
                "local_var",
                ScriptValue::Int(25),
                Some("script1"),
            )
            .unwrap();

        // Should find it with script context
        let value = manager
            .get_variable("local_var", Some("script1"), None)
            .unwrap();
        assert_eq!(*value, ScriptValue::Int(25));

        // Should not find it without script context
        assert!(manager.get_variable("local_var", None, None).is_none());
    }

    #[test]
    fn test_scope_resolution_order() {
        let mut manager = VariableScopeManager::new();

        // Set same variable at different scopes
        manager
            .set_variable(VariableScope::Global, "var", ScriptValue::Int(1), None)
            .unwrap();
        manager
            .set_variable(VariableScope::Map, "var", ScriptValue::Int(2), None)
            .unwrap();
        manager
            .set_variable(
                VariableScope::Team,
                "var",
                ScriptValue::Int(3),
                Some("team1"),
            )
            .unwrap();
        manager
            .set_variable(
                VariableScope::ScriptLocal,
                "var",
                ScriptValue::Int(4),
                Some("script1"),
            )
            .unwrap();

        // Should resolve to most local scope (script-local = 4)
        let value = manager
            .get_variable("var", Some("script1"), Some("team1"))
            .unwrap();
        assert_eq!(*value, ScriptValue::Int(4));

        // Without script context, should resolve to team (3)
        let value = manager.get_variable("var", None, Some("team1")).unwrap();
        assert_eq!(*value, ScriptValue::Int(3));

        // Without team context, should resolve to map (2)
        let value = manager.get_variable("var", None, None).unwrap();
        assert_eq!(*value, ScriptValue::Int(2));
    }

    #[test]
    fn test_clear_scope() {
        let mut manager = VariableScopeManager::new();

        manager
            .set_variable(VariableScope::Global, "global", ScriptValue::Int(1), None)
            .unwrap();
        manager
            .set_variable(VariableScope::Map, "map", ScriptValue::Int(2), None)
            .unwrap();

        manager.clear_scope(VariableScope::Map, None);

        assert!(manager.get_variable("global", None, None).is_some());
        assert!(manager.get_variable("map", None, None).is_none());
    }

    #[test]
    fn test_clear_all() {
        let mut manager = VariableScopeManager::new();

        manager
            .set_variable(VariableScope::Global, "g", ScriptValue::Int(1), None)
            .unwrap();
        manager
            .set_variable(VariableScope::Map, "m", ScriptValue::Int(2), None)
            .unwrap();
        manager
            .set_variable(VariableScope::Team, "t", ScriptValue::Int(3), Some("team1"))
            .unwrap();

        assert_eq!(manager.total_count(), 3);

        manager.clear_all();

        assert_eq!(manager.total_count(), 0);
    }

    #[test]
    fn test_count_scope() {
        let mut manager = VariableScopeManager::new();

        manager
            .set_variable(VariableScope::Global, "g1", ScriptValue::Int(1), None)
            .unwrap();
        manager
            .set_variable(VariableScope::Global, "g2", ScriptValue::Int(2), None)
            .unwrap();
        manager
            .set_variable(VariableScope::Map, "m1", ScriptValue::Int(3), None)
            .unwrap();

        assert_eq!(manager.count_scope(VariableScope::Global, None), 2);
        assert_eq!(manager.count_scope(VariableScope::Map, None), 1);
        assert_eq!(manager.total_count(), 3);
    }

    #[test]
    fn test_list_scope_variables() {
        let mut manager = VariableScopeManager::new();

        manager
            .set_variable(VariableScope::Global, "a", ScriptValue::Int(1), None)
            .unwrap();
        manager
            .set_variable(VariableScope::Global, "b", ScriptValue::Int(2), None)
            .unwrap();

        let vars = manager.list_scope_variables(VariableScope::Global, None);
        assert_eq!(vars.len(), 2);
    }
}
