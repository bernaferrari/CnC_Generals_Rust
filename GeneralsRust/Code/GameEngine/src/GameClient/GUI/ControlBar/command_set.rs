// FILE: command_set.rs
// Port of CommandSet class from C++
// Original: ControlBar.h and ControlBar.cpp

use std::sync::Arc;
use super::types::*;
use super::command_button::CommandButton;

/// Command Set Structure
/// Collections of configurable command buttons used in the command context-sensitive window
pub struct CommandSet {
    /// Name of this command set
    name: String,

    /// The set of command buttons that make up this set (max 18 buttons)
    commands: [Option<Arc<CommandButton>>; MAX_COMMANDS_PER_SET],

    /// Next command set in linked list
    next: Option<Box<CommandSet>>,
}

impl CommandSet {
    /// Create a new CommandSet with the given name
    pub fn new(name: String) -> Self {
        Self {
            name,
            commands: Default::default(),
            next: None,
        }
    }

    /// Get the name of this command set
    pub fn get_name(&self) -> &str {
        &self.name
    }

    /// Get a command button by index
    /// Returns None if the index is out of range or no button exists at that index
    pub fn get_command_button(&self, index: usize) -> Option<&Arc<CommandButton>> {
        if index >= MAX_COMMANDS_PER_SET {
            return None;
        }

        // Check for game logic override first
        if let Some(game_logic) = GameLogic::get_instance() {
            if let Some(button) = game_logic.find_control_bar_override(&self.name, index) {
                return Some(button);
            }
        }

        self.commands[index].as_ref()
    }

    /// Set a command button at the given index
    pub fn set_command_button(&mut self, index: usize, button: Option<Arc<CommandButton>>) -> bool {
        if index >= MAX_COMMANDS_PER_SET {
            return false;
        }

        self.commands[index] = button;
        true
    }

    /// Get the next command set in the list
    pub fn get_next(&self) -> Option<&CommandSet> {
        self.next.as_ref().map(|n| &**n)
    }

    /// Get mutable reference to next command set
    pub fn get_next_mut(&mut self) -> Option<&mut CommandSet> {
        self.next.as_mut().map(|n| &mut **n)
    }

    /// Add this command set to a linked list
    pub fn friend_add_to_list(&mut self, list_head: &mut Option<Box<CommandSet>>) {
        self.next = list_head.take();
        *list_head = Some(Box::new(Self {
            name: self.name.clone(),
            commands: self.commands.clone(),
            next: self.next.take(),
        }));
    }

    /// Get field parse table for INI parsing
    pub fn get_field_parse() -> &'static [FieldParse] {
        &COMMAND_SET_FIELD_PARSE_TABLE
    }

    /// Parse a command button from INI file
    pub fn parse_command_button(
        ini: &mut INI,
        instance: &mut Self,
        button_index: usize
    ) -> Result<(), INIError> {
        let token = ini.get_next_token()?;

        // Find the command button from the control bar
        if let Some(control_bar) = ControlBar::get_instance() {
            if let Some(command_button) = control_bar.find_command_button(&token) {
                if button_index >= MAX_COMMANDS_PER_SET {
                    return Err(INIError::InvalidData(
                        format!("Button index {} out of range", button_index)
                    ));
                }

                instance.commands[button_index] = Some(Arc::clone(command_button));
                Ok(())
            } else {
                Err(INIError::InvalidData(
                    format!("Unknown command '{}' found in command set at line {} in file '{}'",
                        token, ini.get_line_num(), ini.get_filename())
                ))
            }
        } else {
            Err(INIError::NotInitialized("ControlBar not initialized".to_string()))
        }
    }
}

impl Clone for CommandSet {
    fn clone(&self) -> Self {
        Self {
            name: self.name.clone(),
            commands: self.commands.clone(),
            next: None, // Don't clone the linked list chain
        }
    }
}

impl Drop for CommandSet {
    fn drop(&mut self) {
        // Rust handles cleanup automatically
    }
}

// Field parse table for INI parsing
const COMMAND_SET_FIELD_PARSE_TABLE: &[FieldParse] = &[
    FieldParse::new("1", FieldParseType::CommandButton(0)),
    FieldParse::new("2", FieldParseType::CommandButton(1)),
    FieldParse::new("3", FieldParseType::CommandButton(2)),
    FieldParse::new("4", FieldParseType::CommandButton(3)),
    FieldParse::new("5", FieldParseType::CommandButton(4)),
    FieldParse::new("6", FieldParseType::CommandButton(5)),
    FieldParse::new("7", FieldParseType::CommandButton(6)),
    FieldParse::new("8", FieldParseType::CommandButton(7)),
    FieldParse::new("9", FieldParseType::CommandButton(8)),
    FieldParse::new("10", FieldParseType::CommandButton(9)),
    FieldParse::new("11", FieldParseType::CommandButton(10)),
    FieldParse::new("12", FieldParseType::CommandButton(11)),
    FieldParse::new("13", FieldParseType::CommandButton(12)),
    FieldParse::new("14", FieldParseType::CommandButton(13)),
    FieldParse::new("15", FieldParseType::CommandButton(14)),
    FieldParse::new("16", FieldParseType::CommandButton(15)),
    FieldParse::new("17", FieldParseType::CommandButton(16)),
    FieldParse::new("18", FieldParseType::CommandButton(17)),
];

// Placeholder types for INI parsing infrastructure

pub struct FieldParse {
    name: &'static str,
    parse_type: FieldParseType,
}

impl FieldParse {
    pub const fn new(name: &'static str, parse_type: FieldParseType) -> Self {
        Self { name, parse_type }
    }
}

pub enum FieldParseType {
    CommandButton(usize),
}

pub struct INI {
    line_num: usize,
    filename: String,
}

impl INI {
    pub fn get_next_token(&mut self) -> Result<String, INIError> {
        // Placeholder implementation
        Ok(String::new())
    }

    pub fn get_line_num(&self) -> usize {
        self.line_num
    }

    pub fn get_filename(&self) -> &str {
        &self.filename
    }
}

#[derive(Debug)]
pub enum INIError {
    InvalidData(String),
    NotInitialized(String),
}

// Placeholder for GameLogic
pub struct GameLogic;

impl GameLogic {
    pub fn get_instance() -> Option<&'static GameLogic> {
        None
    }

    pub fn find_control_bar_override(
        &self,
        _set_name: &str,
        _index: usize
    ) -> Option<&Arc<CommandButton>> {
        None
    }
}

// Re-export from other modules
pub use super::command_button::ControlBar;
