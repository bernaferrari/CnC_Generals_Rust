//! Leftover Rhai/name ActionRegistry (hq-8ta4n).
//!
//! C++ `executeAction` lives in `scripting/executor/dispatch.rs`.
//! This directory is leftover-only and must not run as a second action brain.

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

pub(crate) use helpers::is_money_resource;
pub use helpers::{
    get_float_param, get_float_param_optional, get_int_param, get_int_param_optional,
    get_string_param,
};
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

/// Concatenated live sources for residual `include_str!` scans.
pub const ACTIONS_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("helpers.rs"),
    include_str!("registry.rs"),
    include_str!("unit_actions.rs"),
    include_str!("object_actions.rs"),
    include_str!("team_command.rs"),
    include_str!("named_unit.rs"),
    include_str!("player_command.rs"),
    include_str!("player_economy.rs"),
    include_str!("camera_ui.rs"),
    include_str!("music_audio.rs"),
    include_str!("weather_radar.rs"),
    include_str!("science_special.rs"),
    include_str!("building.rs"),
    include_str!("leftover.rs"),
);
