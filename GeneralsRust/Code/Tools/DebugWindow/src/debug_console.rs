/*!
 * Debug console functionality
 */

use chrono::{DateTime, Utc};

pub struct DebugConsole {
    history: Vec<ConsoleEntry>,
}

pub struct ConsoleEntry {
    pub text: String,
    pub timestamp: DateTime<Utc>,
}

impl DebugConsole {
    pub fn new() -> Self {
        Self {
            history: Vec::new(),
        }
    }

    pub fn execute_command(&mut self, command: &str) {
        self.history.push(ConsoleEntry {
            text: format!("> {}", command),
            timestamp: Utc::now(),
        });
        
        // Process command
        let response = match command.trim() {
            "help" => "Available commands: help, clear, status",
            "clear" => {
                self.history.clear();
                "Console cleared"
            }
            "status" => "Debug console is running",
            _ => "Unknown command. Type 'help' for available commands.",
        };

        self.history.push(ConsoleEntry {
            text: response.to_string(),
            timestamp: Utc::now(),
        });
    }

    pub fn get_history(&self) -> &[ConsoleEntry] {
        &self.history
    }
}