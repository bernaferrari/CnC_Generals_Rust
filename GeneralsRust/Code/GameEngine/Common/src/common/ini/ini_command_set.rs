//! INI Command Set parsing module
//! Author: Colin Day, March 2002
//! Desc: Command sets are a configurable set of CommandButtons, we will use the sets as
//!       part of the context sensitive user interface

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use super::ini::{FieldParse, INIError, INIResult, INI};
use super::ini_command_button::get_control_bar;
use super::ini_command_button::CommandButton;

/// Maximum number of command buttons in a set
pub const MAX_COMMAND_BUTTONS_PER_SET: usize = 18; // Typically 3x6 grid

/// Command set structure containing a collection of command buttons
#[derive(Debug, Clone)]
pub struct CommandSet {
    pub name: String,
    pub buttons: Vec<Option<String>>, // Button names, None for empty slots
    pub tooltip_text: String,
    pub is_override: bool,
}

impl Default for CommandSet {
    fn default() -> Self {
        Self {
            name: String::new(),
            buttons: vec![None; MAX_COMMAND_BUTTONS_PER_SET],
            tooltip_text: String::new(),
            is_override: false,
        }
    }
}

impl CommandSet {
    /// Create a new command set with the given name
    pub fn new(name: String) -> Self {
        Self {
            name,
            buttons: vec![None; MAX_COMMAND_BUTTONS_PER_SET],
            tooltip_text: String::new(),
            is_override: false,
        }
    }

    /// Get the name of this command set
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Mark this command set as an override
    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    /// Check if this command set is an override
    pub fn is_override(&self) -> bool {
        self.is_override
    }

    /// Set a button at the specified position
    pub fn set_button_at_position(
        &mut self,
        position: usize,
        button_name: String,
    ) -> Result<(), &'static str> {
        if position >= MAX_COMMAND_BUTTONS_PER_SET {
            return Err("Position exceeds maximum command buttons per set");
        }
        self.buttons[position] = Some(button_name);
        Ok(())
    }

    /// Get the button name at the specified position
    pub fn get_button_at_position(&self, position: usize) -> Option<&String> {
        if position >= MAX_COMMAND_BUTTONS_PER_SET {
            return None;
        }
        self.buttons[position].as_ref()
    }

    /// Remove a button at the specified position
    pub fn remove_button_at_position(&mut self, position: usize) -> Option<String> {
        if position >= MAX_COMMAND_BUTTONS_PER_SET {
            return None;
        }
        self.buttons[position].take()
    }

    /// Add a button to the first available position
    pub fn add_button(&mut self, button_name: String) -> Result<usize, &'static str> {
        for (index, slot) in self.buttons.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(button_name);
                return Ok(index);
            }
        }
        Err("No available slots in command set")
    }

    /// Get all non-empty button names
    pub fn get_all_buttons(&self) -> Vec<&String> {
        self.buttons.iter().filter_map(|b| b.as_ref()).collect()
    }

    /// Get the number of buttons in this set
    pub fn button_count(&self) -> usize {
        self.buttons.iter().filter(|b| b.is_some()).count()
    }

    /// Check if the command set is empty
    pub fn is_empty(&self) -> bool {
        self.buttons.iter().all(|b| b.is_none())
    }

    /// Clear all buttons from the set
    pub fn clear_buttons(&mut self) {
        self.buttons.fill(None);
    }

    /// Find the position of a button by name
    pub fn find_button_position(&self, button_name: &str) -> Option<usize> {
        self.buttons
            .iter()
            .position(|b| b.as_ref().map(|name| name == button_name).unwrap_or(false))
    }

    /// Check if a button exists in this command set
    pub fn contains_button(&self, button_name: &str) -> bool {
        self.find_button_position(button_name).is_some()
    }

    /// Parse command set from INI
    pub fn parse_from_ini(ini: &mut INI, name: String) -> INIResult<Self> {
        let mut command_set = Self::new(name);
        command_set.parse_command_set_fields(ini)?;
        Ok(command_set)
    }

    /// Parse command set fields
    fn parse_command_set_fields(&mut self, ini: &mut INI) -> INIResult<()> {
        ini.init_from_ini_with_fields(self, CommandSet::get_field_parse())
    }

    /// Get the field parsing table for command sets
    pub fn get_field_parse() -> &'static [super::ini::FieldParse<Self>] {
        COMMAND_SET_FIELDS
    }

    /// Validate the command set configuration
    pub fn validate(&self) -> INIResult<()> {
        // C++ accepts repeated command buttons in different slots. Retail transport
        // command sets intentionally repeat exit commands for passenger slots.

        Ok(())
    }
}

fn parse_command_button_at(
    index: usize,
    _ini: &mut INI,
    data: &mut CommandSet,
    tokens: &[&str],
) -> INIResult<()> {
    let Some(name) = tokens.first() else {
        return Err(INIError::InvalidData);
    };

    if let Some(control_bar) = get_control_bar() {
        if control_bar.find_command_button_resolved(name).is_none() {
            return Err(INIError::InvalidData);
        }
    }

    data.set_button_at_position(index, (*name).to_string())
        .map_err(|_| INIError::InvalidData)
}

macro_rules! command_set_button_field {
    ($fn_name:ident, $index:expr) => {
        fn $fn_name(ini: &mut INI, data: &mut CommandSet, tokens: &[&str]) -> INIResult<()> {
            parse_command_button_at($index, ini, data, tokens)
        }
    };
}

command_set_button_field!(parse_button_1, 0);
command_set_button_field!(parse_button_2, 1);
command_set_button_field!(parse_button_3, 2);
command_set_button_field!(parse_button_4, 3);
command_set_button_field!(parse_button_5, 4);
command_set_button_field!(parse_button_6, 5);
command_set_button_field!(parse_button_7, 6);
command_set_button_field!(parse_button_8, 7);
command_set_button_field!(parse_button_9, 8);
command_set_button_field!(parse_button_10, 9);
command_set_button_field!(parse_button_11, 10);
command_set_button_field!(parse_button_12, 11);
command_set_button_field!(parse_button_13, 12);
command_set_button_field!(parse_button_14, 13);
command_set_button_field!(parse_button_15, 14);
command_set_button_field!(parse_button_16, 15);
command_set_button_field!(parse_button_17, 16);
command_set_button_field!(parse_button_18, 17);

const COMMAND_SET_FIELDS: &[FieldParse<CommandSet>] = &[
    FieldParse {
        token: "1",
        parse: parse_button_1,
    },
    FieldParse {
        token: "2",
        parse: parse_button_2,
    },
    FieldParse {
        token: "3",
        parse: parse_button_3,
    },
    FieldParse {
        token: "4",
        parse: parse_button_4,
    },
    FieldParse {
        token: "5",
        parse: parse_button_5,
    },
    FieldParse {
        token: "6",
        parse: parse_button_6,
    },
    FieldParse {
        token: "7",
        parse: parse_button_7,
    },
    FieldParse {
        token: "8",
        parse: parse_button_8,
    },
    FieldParse {
        token: "9",
        parse: parse_button_9,
    },
    FieldParse {
        token: "10",
        parse: parse_button_10,
    },
    FieldParse {
        token: "11",
        parse: parse_button_11,
    },
    FieldParse {
        token: "12",
        parse: parse_button_12,
    },
    FieldParse {
        token: "13",
        parse: parse_button_13,
    },
    FieldParse {
        token: "14",
        parse: parse_button_14,
    },
    FieldParse {
        token: "15",
        parse: parse_button_15,
    },
    FieldParse {
        token: "16",
        parse: parse_button_16,
    },
    FieldParse {
        token: "17",
        parse: parse_button_17,
    },
    FieldParse {
        token: "18",
        parse: parse_button_18,
    },
];

/// Command set manager for handling collections of command sets
#[derive(Debug)]
pub struct CommandSetManager {
    command_sets: HashMap<String, CommandSet>,
    command_set_order: Vec<String>,
    set_overrides: HashMap<String, Vec<CommandSet>>,
}

impl CommandSetManager {
    /// Create a new command set manager
    pub fn new() -> Self {
        Self {
            command_sets: HashMap::new(),
            command_set_order: Vec::new(),
            set_overrides: HashMap::new(),
        }
    }

    /// Find a command set by name
    pub fn find_command_set(&self, name: &str) -> Option<&CommandSet> {
        self.command_sets.get(name)
    }

    /// Find a command set by name, resolving overrides to the final entry.
    pub fn find_command_set_resolved(&self, name: &str) -> Option<&CommandSet> {
        if let Some(overrides) = self.set_overrides.get(name) {
            if let Some(last) = overrides.last() {
                return Some(last);
            }
        }
        self.find_command_set(name)
    }

    /// Find a mutable command set by name
    pub fn find_command_set_mut(&mut self, name: &str) -> Option<&mut CommandSet> {
        self.command_sets.get_mut(name)
    }

    /// Create a new command set
    pub fn new_command_set(&mut self, name: String) -> &mut CommandSet {
        let command_set = CommandSet::new(name.clone());
        if !self.command_sets.contains_key(&name) {
            self.command_set_order.insert(0, name.clone());
        }
        self.command_sets.insert(name.clone(), command_set);
        self.command_sets.get_mut(&name).unwrap()
    }

    /// Create a new command set override
    pub fn new_command_set_override(&mut self, base_set: &CommandSet) -> &mut CommandSet {
        let mut override_set = base_set.clone();
        override_set.mark_as_override();

        let name = base_set.name.clone();
        let overrides = self
            .set_overrides
            .entry(name.clone())
            .or_insert_with(Vec::new);
        overrides.push(override_set);

        overrides.last_mut().unwrap()
    }

    /// Remove a command set
    pub fn remove_command_set(&mut self, name: &str) -> Option<CommandSet> {
        let removed = self.command_sets.remove(name);
        if removed.is_some() {
            self.command_set_order.retain(|set_name| set_name != name);
            self.set_overrides.remove(name);
        }
        removed
    }

    /// Get all command set names
    pub fn get_command_set_names(&self) -> Vec<&String> {
        self.command_set_order.iter().collect()
    }

    /// Iterate over resolved command sets, returning overrides when present.
    pub fn iter_resolved_sets(&self) -> Vec<(&String, &CommandSet)> {
        self.command_set_order
            .iter()
            .filter_map(|name| {
                let base = self.command_sets.get(name)?;
                let resolved = self.find_command_set_resolved(name).unwrap_or(base);
                Some((name, resolved))
            })
            .collect()
    }

    /// Get the number of command sets
    pub fn count(&self) -> usize {
        self.command_sets.len()
    }

    /// Clear all command sets
    pub fn clear(&mut self) {
        self.command_sets.clear();
        self.command_set_order.clear();
        self.set_overrides.clear();
    }

    /// Parse command set definition from INI
    pub fn parse_command_set_definition(ini: &mut INI) -> INIResult<()> {
        // Read the command set name
        let name = match ini.get_next_value_token().or_else(|| ini.get_first_token()) {
            Some(token) => token,
            None => return Err(INIError::InvalidData),
        };

        initialize_command_set_manager();
        let mut manager =
            get_command_set_manager_mut().expect("Command set manager not initialized");

        // Check if command set already exists
        if let Some(existing_set) = manager.find_command_set(&name) {
            if ini.get_load_type() != super::ini::INILoadType::CreateOverrides {
                eprintln!(
                    "Duplicate command set {} found at line {} in '{}'",
                    name,
                    ini.get_line_num(),
                    ini.get_filename()
                );
                return Err(INIError::InvalidData);
            } else {
                // Create override
                let base_set = existing_set.clone();
                let override_set = manager.new_command_set_override(&base_set);
                override_set.parse_command_set_fields(ini)?;
                override_set.validate()?;
            }
        } else {
            // Create new command set
            let command_set = manager.new_command_set(name.to_string());
            if ini.get_load_type() == super::ini::INILoadType::CreateOverrides {
                command_set.mark_as_override();
            }

            command_set.parse_command_set_fields(ini)?;
            command_set.validate()?;
        }

        Ok(())
    }
}

impl Default for CommandSetManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global command set manager instance
static COMMAND_SET_MANAGER: OnceCell<RwLock<CommandSetManager>> = OnceCell::new();

/// Initialize the global command set manager
pub fn initialize_command_set_manager() {
    if COMMAND_SET_MANAGER.get().is_none() {
        let _ = COMMAND_SET_MANAGER.set(RwLock::new(CommandSetManager::new()));
    }
}

/// Get a reference to the global command set manager
pub fn get_command_set_manager() -> Option<RwLockReadGuard<'static, CommandSetManager>> {
    COMMAND_SET_MANAGER
        .get()
        .map(|manager| manager.read().expect("CommandSetManager poisoned"))
}

/// Get a mutable reference to the global command set manager
pub fn get_command_set_manager_mut() -> Option<RwLockWriteGuard<'static, CommandSetManager>> {
    COMMAND_SET_MANAGER
        .get()
        .map(|manager| manager.write().expect("CommandSetManager poisoned"))
}

/// Parse command set definition from INI file
/// This is the main entry point called by the INI parser
pub fn parse_command_set_definition(ini: &mut INI) -> INIResult<()> {
    CommandSetManager::parse_command_set_definition(ini)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_set_creation() {
        let command_set = CommandSet::new("TestSet".to_string());
        assert_eq!(command_set.get_name(), "TestSet");
        assert_eq!(command_set.button_count(), 0);
        assert!(command_set.is_empty());
        assert!(!command_set.is_override());
    }

    #[test]
    fn test_command_set_button_management() {
        let mut command_set = CommandSet::new("TestSet".to_string());

        // Add buttons
        let pos1 = command_set.add_button("Button1".to_string()).unwrap();
        let pos2 = command_set.add_button("Button2".to_string()).unwrap();

        assert_eq!(command_set.button_count(), 2);
        assert!(!command_set.is_empty());

        // Check positions
        assert_eq!(pos1, 0);
        assert_eq!(pos2, 1);

        // Get buttons
        assert_eq!(
            command_set.get_button_at_position(0),
            Some(&"Button1".to_string())
        );
        assert_eq!(
            command_set.get_button_at_position(1),
            Some(&"Button2".to_string())
        );

        // Find button positions
        assert_eq!(command_set.find_button_position("Button1"), Some(0));
        assert_eq!(command_set.find_button_position("Button2"), Some(1));
        assert_eq!(command_set.find_button_position("NonExistent"), None);

        // Check contains
        assert!(command_set.contains_button("Button1"));
        assert!(command_set.contains_button("Button2"));
        assert!(!command_set.contains_button("NonExistent"));
    }

    #[test]
    fn test_command_set_button_removal() {
        let mut command_set = CommandSet::new("TestSet".to_string());

        // Add buttons
        command_set.add_button("Button1".to_string()).unwrap();
        command_set.add_button("Button2".to_string()).unwrap();

        // Remove button
        let removed = command_set.remove_button_at_position(0);
        assert_eq!(removed, Some("Button1".to_string()));
        assert_eq!(command_set.button_count(), 1);

        // Clear all buttons
        command_set.clear_buttons();
        assert!(command_set.is_empty());
        assert_eq!(command_set.button_count(), 0);
    }

    #[test]
    fn test_command_set_set_button_at_position() {
        let mut command_set = CommandSet::new("TestSet".to_string());

        // Set button at specific position
        assert!(command_set
            .set_button_at_position(5, "Button5".to_string())
            .is_ok());
        assert_eq!(
            command_set.get_button_at_position(5),
            Some(&"Button5".to_string())
        );

        // Try to set button beyond max position
        assert!(command_set
            .set_button_at_position(MAX_COMMAND_BUTTONS_PER_SET, "InvalidButton".to_string())
            .is_err());
    }

    #[test]
    fn test_command_set_get_all_buttons() {
        let mut command_set = CommandSet::new("TestSet".to_string());

        command_set
            .set_button_at_position(0, "Button1".to_string())
            .unwrap();
        command_set
            .set_button_at_position(2, "Button3".to_string())
            .unwrap();
        command_set
            .set_button_at_position(5, "Button6".to_string())
            .unwrap();

        let all_buttons = command_set.get_all_buttons();
        assert_eq!(all_buttons.len(), 3);

        let button_names: Vec<String> = all_buttons.iter().map(|&s| s.clone()).collect();
        assert!(button_names.contains(&"Button1".to_string()));
        assert!(button_names.contains(&"Button3".to_string()));
        assert!(button_names.contains(&"Button6".to_string()));
    }

    #[test]
    fn test_command_set_manager() {
        let mut manager = CommandSetManager::new();
        assert_eq!(manager.count(), 0);

        // Add a command set
        let command_set = manager.new_command_set("TestSet".to_string());
        command_set.add_button("TestButton".to_string()).unwrap();

        assert_eq!(manager.count(), 1);

        // Find the command set
        let found = manager.find_command_set("TestSet");
        assert!(found.is_some());
        assert_eq!(found.unwrap().button_count(), 1);

        // Try to find non-existent command set
        let not_found = manager.find_command_set("NonExistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn command_set_manager_enumerates_cpp_list_order() {
        let mut manager = CommandSetManager::new();

        manager.new_command_set("FirstSet".to_string());
        manager.new_command_set("SecondSet".to_string());
        manager.new_command_set("ThirdSet".to_string());

        let names: Vec<&str> = manager
            .get_command_set_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names, vec!["ThirdSet", "SecondSet", "FirstSet"]);

        let first_set = manager.find_command_set("FirstSet").unwrap().clone();
        let first_override = manager.new_command_set_override(&first_set);
        first_override
            .set_button_at_position(0, "OverrideButton".to_string())
            .unwrap();

        let resolved_names: Vec<&str> = manager
            .iter_resolved_sets()
            .into_iter()
            .map(|(name, _)| name.as_str())
            .collect();
        assert_eq!(resolved_names, vec!["ThirdSet", "SecondSet", "FirstSet"]);
        assert_eq!(
            manager
                .find_command_set_resolved("FirstSet")
                .and_then(|set| set.get_button_at_position(0))
                .map(String::as_str),
            Some("OverrideButton")
        );

        assert!(manager.remove_command_set("SecondSet").is_some());
        let names_after_remove: Vec<&str> = manager
            .get_command_set_names()
            .into_iter()
            .map(String::as_str)
            .collect();
        assert_eq!(names_after_remove, vec!["ThirdSet", "FirstSet"]);
    }

    #[test]
    fn test_command_set_override() {
        let mut manager = CommandSetManager::new();

        // Create base command set
        let base_set = manager.new_command_set("TestSet".to_string());
        base_set.add_button("Button1".to_string()).unwrap();
        let base_set_copy = base_set.clone();

        // Create override
        let override_set = manager.new_command_set_override(&base_set_copy);
        assert!(override_set.is_override());
        assert_eq!(override_set.button_count(), 1); // Inherited from base
    }

    #[test]
    fn test_command_set_validation() {
        let mut command_set = CommandSet::new("TestSet".to_string());

        // Valid set
        command_set.add_button("Button1".to_string()).unwrap();
        command_set.add_button("Button2".to_string()).unwrap();
        assert!(command_set.validate().is_ok());

        // C++ CommandSet::parseCommandButton stores per-slot pointers and allows
        // duplicates. Retail transport command sets rely on this for exit slots.
        command_set
            .set_button_at_position(10, "Button1".to_string())
            .unwrap();
        assert!(command_set.validate().is_ok());
        assert_eq!(command_set.button_count(), 3);
    }

    #[test]
    fn parse_fields_accept_duplicate_button_slots() {
        crate::common::ini::ini_command_button::initialize_control_bar();
        {
            let mut control_bar =
                crate::common::ini::ini_command_button::get_control_bar_mut().unwrap();
            control_bar.clear();
            control_bar.new_command_button("Command_TransportExit".to_string());
        }

        let mut command_set = CommandSet::new("AmericaTransportCommandSet".to_string());
        let mut ini = INI::new();

        parse_button_1(&mut ini, &mut command_set, &["Command_TransportExit"]).unwrap();
        parse_button_2(&mut ini, &mut command_set, &["Command_TransportExit"]).unwrap();
        parse_button_3(&mut ini, &mut command_set, &["Command_TransportExit"]).unwrap();

        assert_eq!(
            command_set.get_button_at_position(0).map(String::as_str),
            Some("Command_TransportExit")
        );
        assert_eq!(
            command_set.get_button_at_position(1).map(String::as_str),
            Some("Command_TransportExit")
        );
        assert_eq!(
            command_set.get_button_at_position(2).map(String::as_str),
            Some("Command_TransportExit")
        );
        assert!(command_set.validate().is_ok());

        crate::common::ini::ini_command_button::get_control_bar_mut()
            .unwrap()
            .clear();
    }
}
