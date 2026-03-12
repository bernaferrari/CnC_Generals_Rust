//! Control Bar Commands - handles command processing

use super::{CommandButton, CommandOption, CommandSourceType};

/// Command processor for control bar
pub struct ControlBarCommandProcessor;

impl ControlBarCommandProcessor {
    pub fn new() -> Self {
        Self
    }

    pub fn process_command(
        &self,
        button: &CommandButton,
        source: CommandSourceType,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        log::info!(
            "Processing command: {} from {:?}",
            button.command_name,
            source
        );
        Ok(true)
    }
}

impl Default for ControlBarCommandProcessor {
    fn default() -> Self {
        Self::new()
    }
}
