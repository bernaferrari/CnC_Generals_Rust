//! Script Actions System
//!
//! This module provides all the action types that scripts can execute,
//! including unit creation, resource manipulation, UI updates, and game state changes.
//!
//! Split from `scripting/actions.rs` for module-size parity.
//! Observable script behavior is unchanged.

mod building;
mod camera_ui;
mod helpers;
mod leftover;
mod music_audio;
mod named_unit;
mod object_actions;
mod player_command;
mod player_economy;
mod registry;
mod science_special;
mod team_command;
mod unit_actions;
mod weather_radar;

#[cfg(test)]
mod tests;

pub use helpers::{
    get_float_param, get_float_param_optional, get_int_param, get_int_param_optional,
    get_string_param,
};
pub(crate) use helpers::is_money_resource;
pub use registry::ActionRegistry;

use async_trait::async_trait;
use std::collections::HashMap;

use super::{ScriptContext, ScriptResult, ScriptValue};
use crate::{GameLogicError, GameLogicResult};

/// Script action trait
#[async_trait]
pub trait ScriptAction: Send + Sync {
    /// Execute the action
    async fn execute(
        &self,
        parameters: &HashMap<String, ScriptValue>,
        context: &ScriptContext,
    ) -> GameLogicResult<ScriptResult>;

    /// Get action name
    fn name(&self) -> &str;

    /// Get action description
    fn description(&self) -> &str;

    /// Get required parameters
    fn required_parameters(&self) -> Vec<String>;

    /// Get optional parameters
    fn optional_parameters(&self) -> Vec<String>;
}
