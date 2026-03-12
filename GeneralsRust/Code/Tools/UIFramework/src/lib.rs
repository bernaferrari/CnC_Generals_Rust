//! Modern UI Framework for Command & Conquer Game Development Tools
//!
//! This framework provides a unified, modern interface for all game development tools
//! including 3D viewports, asset browsers, property panels, and hot-reload capabilities.

pub mod app;
pub mod asset_browser;
pub mod dialogs;
pub mod hot_reload;
pub mod panels;
pub mod themes;
pub mod utils;
pub mod viewport;
pub mod widgets;

pub use app::*;
pub use panels::*;
pub use themes::*;
pub use viewport::*;
pub use widgets::*;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Core UI framework trait that all tools implement
pub trait GameTool {
    /// Get the tool's unique identifier
    fn id(&self) -> Uuid;

    /// Get the tool's display name
    fn name(&self) -> &str;

    /// Get the tool's version
    fn version(&self) -> &str;

    /// Initialize the tool
    fn initialize(&mut self) -> Result<()>;

    /// Update the tool (called every frame)
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) -> Result<()>;

    /// Handle tool-specific menu items
    fn menu_bar(&mut self, ui: &mut eframe::egui::Ui) -> Result<()>;

    /// Handle tool shutdown
    fn shutdown(&mut self) -> Result<()>;

    /// Get tool configuration
    fn config(&self) -> &ToolConfig;

    /// Set tool configuration
    fn set_config(&mut self, config: ToolConfig) -> Result<()>;
}

/// Configuration for a game development tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    pub name: String,
    pub version: String,
    pub window_size: [f32; 2],
    pub window_position: Option<[f32; 2]>,
    pub theme: ThemeType,
    pub hot_reload_enabled: bool,
    pub recent_files: Vec<String>,
    pub custom_settings: HashMap<String, serde_json::Value>,
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            name: "Unknown Tool".to_string(),
            version: "0.1.0".to_string(),
            window_size: [1200.0, 800.0],
            window_position: None,
            theme: ThemeType::Dark,
            hot_reload_enabled: true,
            recent_files: Vec::new(),
            custom_settings: HashMap::new(),
        }
    }
}

/// Available UI themes
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum ThemeType {
    Dark,
    Light,
    CnCClassic,
    Modern,
}

/// UI Framework error types
#[derive(thiserror::Error, Debug)]
pub enum UIError {
    #[error("Window creation failed: {0}")]
    WindowCreationFailed(String),

    #[error("Rendering error: {0}")]
    RenderingError(String),

    #[error("Asset loading error: {0}")]
    AssetLoadingError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Hot reload error: {0}")]
    HotReloadError(String),
}
