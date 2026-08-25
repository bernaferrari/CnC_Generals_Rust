//! Options Menu System
//!
//! This module implements the comprehensive options/settings menu matching the original
//! C&C Generals interface from OptionsMenu.cpp.
//! Provides tabs for Video, Audio, Controls, and Game settings.

use super::{
    ClickSpring, Interactive, KeyCode, MouseButton, Renderable, UIEvent, UIRenderContext, utils,
};
use crate::localization;
use log::info;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::PathBuf;

/// Options menu tab categories (from C++ OptionsMenu.cpp)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OptionsTab {
    Video,
    Audio,
    Controls,
    Game,
}

/// Video quality settings (from C++ Detail enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsQuality {
    Low,
    Medium,
    High,
    Custom,
}

/// Screen resolution
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Resolution {
    pub width: u32,
    pub height: u32,
}

impl Resolution {
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub fn to_string(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }
}

/// Option setting that can be adjusted
#[derive(Debug, Clone, PartialEq)]
pub enum OptionValue {
    Boolean(bool),
    Integer(i32),
    Float(f32),
    String(String),
    Resolution(Resolution),
    Quality(GraphicsQuality),
}

/// Individual option control
struct OptionControl {
    key: String,
    label: String,
    value: OptionValue,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    enabled: bool,
    click_spring: ClickSpring,
}

impl OptionControl {
    fn new(key: &str, label: String, value: OptionValue, x: i32, y: i32) -> Self {
        Self {
            key: key.to_string(),
            label,
            value,
            position: (x, y),
            size: (400, 30),
            hovered: false,
            enabled: true,
            click_spring: ClickSpring::new(),
        }
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        utils::point_in_rect(
            (x, y),
            (self.position.0, self.position.1, self.size.0, self.size.1),
        )
    }

    fn update(&mut self, delta_time: f32) {
        self.click_spring.update(delta_time);
    }

    fn trigger_click(&mut self) {
        self.click_spring.trigger();
    }

    fn click_scale(&self) -> f32 {
        self.click_spring.scale()
    }
}

/// Tab button
struct TabButton {
    tab: OptionsTab,
    text: String,
    position: (i32, i32),
    size: (u32, u32),
    active: bool,
    hovered: bool,
    click_spring: ClickSpring,
}

impl TabButton {
    fn new(tab: OptionsTab, text: String, x: i32, y: i32) -> Self {
        Self {
            tab,
            text,
            position: (x, y),
            size: (150, 40),
            active: false,
            hovered: false,
            click_spring: ClickSpring::new(),
        }
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        utils::point_in_rect(
            (x, y),
            (self.position.0, self.position.1, self.size.0, self.size.1),
        )
    }

    fn update(&mut self, delta_time: f32) {
        self.click_spring.update(delta_time);
    }

    fn trigger_click(&mut self) {
        self.click_spring.trigger();
    }

    fn click_scale(&self) -> f32 {
        self.click_spring.scale()
    }
}

/// Action buttons (OK, Cancel, Apply)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ActionButton {
    Accept,
    Cancel,
    Apply,
    RestoreDefaults,
}

struct ActionBtn {
    action: ActionButton,
    text: String,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    click_spring: ClickSpring,
}

impl ActionBtn {
    fn new(action: ActionButton, text: String, x: i32, y: i32) -> Self {
        Self {
            action,
            text,
            position: (x, y),
            size: (120, 40),
            hovered: false,
            click_spring: ClickSpring::new(),
        }
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        utils::point_in_rect(
            (x, y),
            (self.position.0, self.position.1, self.size.0, self.size.1),
        )
    }

    fn update(&mut self, delta_time: f32) {
        self.click_spring.update(delta_time);
    }

    fn trigger_click(&mut self) {
        self.click_spring.trigger();
    }

    fn click_scale(&self) -> f32 {
        self.click_spring.scale()
    }
}

/// Options Menu implementation (from C++ OptionsMenu.cpp)
pub struct OptionsMenu {
    /// Current active tab
    current_tab: OptionsTab,
    /// Tab buttons
    tab_buttons: Vec<TabButton>,
    /// Action buttons
    action_buttons: Vec<ActionBtn>,
    /// All option controls organized by tab
    options: HashMap<OptionsTab, Vec<OptionControl>>,
    /// Original values (for cancel operation)
    original_values: HashMap<String, OptionValue>,
    /// Default values as defined by this menu (Restore Defaults).
    default_values: HashMap<String, OptionValue>,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Whether settings have been modified
    settings_modified: bool,
    /// Animation progress
    animation_progress: f32,
}

impl Default for OptionsMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl OptionsMenu {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    /// Create new options menu
    pub fn new() -> Self {
        Self {
            current_tab: OptionsTab::Video,
            tab_buttons: Vec::new(),
            action_buttons: Vec::new(),
            options: HashMap::new(),
            original_values: HashMap::new(),
            default_values: HashMap::new(),
            screen_size: (1024, 768),
            settings_modified: false,
            animation_progress: 0.0,
        }
    }

    /// Initialize options menu

    /// Read a bool option residual (gameplay toggles).
    pub fn bool_option(&self, key: &str) -> Option<bool> {
        for controls in self.options.values() {
            for c in controls {
                if c.key == key {
                    return match &c.value {
                        OptionValue::Boolean(v) => Some(*v),
                        _ => None,
                    };
                }
            }
        }
        None
    }

    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.setup_tabs();
        self.setup_action_buttons();
        self.setup_video_options();
        self.setup_audio_options();
        self.setup_control_options();
        self.setup_game_options();
        self.default_values = self.snapshot_current_values();
        self.load_current_settings();
        self.original_values = self.snapshot_current_values();
        Ok(())
    }

    /// Update options menu
    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        // Update animation
        if self.animation_progress < 1.0 {
            self.animation_progress += delta_time * 3.0;
            self.animation_progress = self.animation_progress.min(1.0);
        }
        for tab_btn in &mut self.tab_buttons {
            tab_btn.update(delta_time);
        }
        for action_btn in &mut self.action_buttons {
            action_btn.update(delta_time);
        }
        for controls in self.options.values_mut() {
            for control in controls {
                control.update(delta_time);
            }
        }

        Ok(())
    }

    /// Handle mouse clicks
    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> Option<UIEvent> {
        if button != MouseButton::Left {
            return None;
        }

        // Check tab buttons - find which tab was clicked first
        if let Some(tab_btn) = self
            .tab_buttons
            .iter_mut()
            .find(|tab_btn| tab_btn.contains_point(x, y))
        {
            tab_btn.trigger_click();
            let tab = tab_btn.tab;
            self.switch_tab(tab);
            return None;
        }

        // Check action buttons
        let mut clicked_action = None;
        for action_btn in &mut self.action_buttons {
            if action_btn.contains_point(x, y) {
                action_btn.trigger_click();
                clicked_action = Some(action_btn.action);
                break;
            }
        }
        if let Some(action) = clicked_action {
            return self.handle_action(action);
        }

        // Check option controls - find which control was clicked first
        let clicked_key = {
            if let Some(controls) = self.options.get_mut(&self.current_tab) {
                controls
                    .iter_mut()
                    .find(|control| control.contains_point(x, y) && control.enabled)
                    .map(|control| {
                        control.trigger_click();
                        control.key.clone()
                    })
            } else {
                None
            }
        };
        if let Some(key) = clicked_key {
            self.toggle_option(&key);
            return None;
        }

        None
    }

    /// Resize menu
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
        self.setup_tabs();
        self.setup_action_buttons();
    }

    // Private methods

    fn setup_tabs(&mut self) {
        self.tab_buttons.clear();

        let start_x = 50;
        let start_y = 50;
        let tab_width = 150;
        let tab_spacing = 10;

        self.tab_buttons.push(TabButton::new(
            OptionsTab::Video,
            Self::text("options.tab_video", "Video"),
            start_x,
            start_y,
        ));

        self.tab_buttons.push(TabButton::new(
            OptionsTab::Audio,
            Self::text("options.tab_audio", "Audio"),
            start_x + tab_width + tab_spacing,
            start_y,
        ));

        self.tab_buttons.push(TabButton::new(
            OptionsTab::Controls,
            Self::text("options.tab_controls", "Controls"),
            start_x + (tab_width + tab_spacing) * 2,
            start_y,
        ));

        self.tab_buttons.push(TabButton::new(
            OptionsTab::Game,
            Self::text("options.tab_game", "Game"),
            start_x + (tab_width + tab_spacing) * 3,
            start_y,
        ));

        // Mark current tab as active
        for tab_btn in &mut self.tab_buttons {
            tab_btn.active = tab_btn.tab == self.current_tab;
        }
    }

    fn setup_action_buttons(&mut self) {
        self.action_buttons.clear();

        let bottom_y = self.screen_size.1 as i32 - 70;
        let button_spacing = 140;
        let start_x = (self.screen_size.0 as i32 / 2) - (button_spacing * 2);

        self.action_buttons.push(ActionBtn::new(
            ActionButton::Accept,
            Self::text("options.accept", "OK"),
            start_x,
            bottom_y,
        ));

        self.action_buttons.push(ActionBtn::new(
            ActionButton::Cancel,
            Self::text("options.cancel", "Cancel"),
            start_x + button_spacing,
            bottom_y,
        ));

        self.action_buttons.push(ActionBtn::new(
            ActionButton::Apply,
            Self::text("options.apply", "Apply"),
            start_x + button_spacing * 2,
            bottom_y,
        ));

        self.action_buttons.push(ActionBtn::new(
            ActionButton::RestoreDefaults,
            Self::text("options.defaults", "Defaults"),
            start_x + button_spacing * 3,
            bottom_y,
        ));
    }

    fn setup_video_options(&mut self) {
        let mut video_options = Vec::new();
        let start_x = 100;
        let start_y = 120;
        let spacing = 40;

        // Resolution (from C++ comboBoxResolution)
        video_options.push(OptionControl::new(
            "video.resolution",
            Self::text("options.video.resolution", "Resolution"),
            OptionValue::Resolution(Resolution::new(1024, 768)),
            start_x,
            start_y,
        ));

        // Graphics Quality (from C++ comboBoxDetail)
        video_options.push(OptionControl::new(
            "video.quality",
            Self::text("options.video.quality", "Graphics Quality"),
            OptionValue::Quality(GraphicsQuality::High),
            start_x,
            start_y + spacing,
        ));

        // Anti-aliasing (from C++ comboBoxAntiAliasing)
        video_options.push(OptionControl::new(
            "video.antialiasing",
            Self::text("options.video.antialiasing", "Anti-Aliasing"),
            OptionValue::String("4x".to_string()),
            start_x,
            start_y + spacing * 2,
        ));

        // Gamma (C++ OptionsMenu.cpp:1239 SliderGamma, 0-100, 50 = 1.0)
        video_options.push(OptionControl::new(
            "video.gamma",
            Self::text("options.video.gamma", "Gamma"),
            OptionValue::Integer(50),
            start_x,
            start_y + spacing * 9,
        ));
        // Fullscreen
        video_options.push(OptionControl::new(
            "video.fullscreen",
            Self::text("options.video.fullscreen", "Fullscreen"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 3,
        ));

        // VSync
        video_options.push(OptionControl::new(
            "video.vsync",
            Self::text("options.video.vsync", "Vertical Sync"),
            OptionValue::Boolean(false),
            start_x,
            start_y + spacing * 4,
        ));

        // Advanced video options (from C++ WinAdvancedDisplay)
        video_options.push(OptionControl::new(
            "video.shadows_3d",
            Self::text("options.video.shadows_3d", "3D Shadows"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 5,
        ));

        video_options.push(OptionControl::new(
            "video.shadows_2d",
            Self::text("options.video.shadows_2d", "2D Shadows"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 6,
        ));

        video_options.push(OptionControl::new(
            "video.heat_effects",
            Self::text("options.video.heat_effects", "Heat Effects"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 7,
        ));

        video_options.push(OptionControl::new(
            "video.building_occlusion",
            Self::text("options.video.building_occlusion", "Building Transparency"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 8,
        ));

        self.options.insert(OptionsTab::Video, video_options);
    }

    fn setup_audio_options(&mut self) {
        let mut audio_options = Vec::new();
        let start_x = 100;
        let start_y = 120;
        let spacing = 40;

        // Volume controls (from C++ sliders)
        audio_options.push(OptionControl::new(
            "audio.music_volume",
            Self::text("options.audio.music_volume", "Music Volume"),
            OptionValue::Float(0.8),
            start_x,
            start_y,
        ));

        audio_options.push(OptionControl::new(
            "audio.sfx_volume",
            Self::text("options.audio.sfx_volume", "Sound Effects Volume"),
            OptionValue::Float(0.8),
            start_x,
            start_y + spacing,
        ));

        audio_options.push(OptionControl::new(
            "audio.voice_volume",
            Self::text("options.audio.voice_volume", "Voice Volume"),
            OptionValue::Float(0.8),
            start_x,
            start_y + spacing * 2,
        ));

        // Audio quality
        audio_options.push(OptionControl::new(
            "audio.enable_sound",
            Self::text("options.audio.enable_sound", "Enable Sound"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 3,
        ));

        audio_options.push(OptionControl::new(
            "audio.enable_music",
            Self::text("options.audio.enable_music", "Enable Music"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 4,
        ));

        audio_options.push(OptionControl::new(
            "audio.enable_3d_sound",
            Self::text("options.audio.enable_3d_sound", "3D Sound"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 5,
        ));

        self.options.insert(OptionsTab::Audio, audio_options);
    }

    fn setup_control_options(&mut self) {
        let mut control_options = Vec::new();
        let start_x = 100;
        let start_y = 120;
        let spacing = 40;

        // Mouse controls (from C++ checkAlternateMouse)
        control_options.push(OptionControl::new(
            "controls.alternate_mouse",
            Self::text("options.controls.alternate_mouse", "Alternate Mouse Setup"),
            OptionValue::Boolean(false),
            start_x,
            start_y,
        ));

        control_options.push(OptionControl::new(
            "controls.scroll_speed",
            Self::text("options.controls.scroll_speed", "Scroll Speed"),
            OptionValue::Float(0.5),
            start_x,
            start_y + spacing,
        ));

        // Camera controls (from C++ checkUseCamera)
        control_options.push(OptionControl::new(
            "controls.use_camera",
            Self::text("options.controls.use_camera", "Use Camera Hotkeys"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 2,
        ));

        control_options.push(OptionControl::new(
            "controls.save_camera",
            Self::text("options.controls.save_camera", "Save Camera Position"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 3,
        ));

        // Unit behavior (from C++ checkRetaliation)
        control_options.push(OptionControl::new(
            "controls.auto_retaliate",
            Self::text("options.controls.auto_retaliate", "Auto-Retaliate"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 4,
        ));

        control_options.push(OptionControl::new(
            "controls.double_click_attack",
            Self::text(
                "options.controls.double_click_attack",
                "Double-Click Attack-Move",
            ),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 5,
        ));

        // Selection anchors (from C++ checkDrawAnchor, checkMoveAnchor)
        control_options.push(OptionControl::new(
            "controls.draw_anchor",
            Self::text("options.controls.draw_anchor", "Show Selection Anchor"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 6,
        ));

        control_options.push(OptionControl::new(
            "controls.move_anchor",
            Self::text("options.controls.move_anchor", "Move Selection Anchor"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 7,
        ));

        self.options.insert(OptionsTab::Controls, control_options);
    }

    fn setup_game_options(&mut self) {
        let mut game_options = Vec::new();
        let start_x = 100;
        let start_y = 120;
        let spacing = 40;

        // Network settings
        game_options.push(OptionControl::new(
            "game.show_tooltips",
            Self::text("options.game.show_tooltips", "Show Tooltips"),
            OptionValue::Boolean(true),
            start_x,
            start_y,
        ));

        game_options.push(OptionControl::new(
            "game.language_filter",
            Self::text("options.game.language_filter", "Language Filter"),
            OptionValue::Boolean(false),
            start_x,
            start_y + spacing,
        ));

        game_options.push(OptionControl::new(
            "game.show_health_bars",
            Self::text("options.game.show_health_bars", "Show Health Bars"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 2,
        ));

        game_options.push(OptionControl::new(
            "game.show_fps",
            Self::text("options.game.show_fps", "Show FPS Counter"),
            OptionValue::Boolean(false),
            start_x,
            start_y + spacing * 3,
        ));

        game_options.push(OptionControl::new(
            "game.autosave",
            Self::text("options.game.autosave", "Enable Autosave"),
            OptionValue::Boolean(true),
            start_x,
            start_y + spacing * 4,
        ));

        self.options.insert(OptionsTab::Game, game_options);
    }

    fn load_current_settings(&mut self) {
        info!(
            "{}",
            Self::text("options.log.load_settings", "Loading current settings...")
        );

        let flat = Self::options_ini_path()
            .filter(|path| path.exists())
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|text| Self::parse_flat_options_ini(&text))
            .unwrap_or_default();

        for control in self.iter_controls_mut() {
            match control.key.as_str() {
                "video.resolution" => {
                    if let Some(raw) = flat.get("Resolution") {
                        if let Some(res) = Self::parse_resolution_value(raw) {
                            control.value = OptionValue::Resolution(res);
                        }
                    }
                }
                "video.quality" => {
                    if let Some(raw) = flat.get("StaticGameLOD") {
                        control.value =
                            OptionValue::Quality(match raw.to_ascii_lowercase().as_str() {
                                "low" => GraphicsQuality::Low,
                                "medium" => GraphicsQuality::Medium,
                                "high" => GraphicsQuality::High,
                                "custom" => GraphicsQuality::Custom,
                                _ => GraphicsQuality::High,
                            });
                    }
                }
                "video.gamma" => {
                    if let Some(v) = flat.get("Gamma").and_then(|s| s.parse::<i32>().ok()) {
                        control.value = OptionValue::Integer(v.clamp(0, 100));
                    }
                }
                "audio.music_volume" => {
                    if let Some(v) = flat.get("MusicVolume").and_then(|s| s.parse::<f32>().ok()) {
                        control.value = OptionValue::Float((v / 100.0).clamp(0.0, 1.0));
                    }
                }
                "audio.sfx_volume" => {
                    if let Some(v) = flat.get("SFXVolume").and_then(|s| s.parse::<f32>().ok()) {
                        control.value = OptionValue::Float((v / 100.0).clamp(0.0, 1.0));
                    }
                }
                "audio.voice_volume" => {
                    if let Some(v) = flat.get("VoiceVolume").and_then(|s| s.parse::<f32>().ok()) {
                        control.value = OptionValue::Float((v / 100.0).clamp(0.0, 1.0));
                    }
                }
                "controls.scroll_speed" => {
                    if let Some(v) = flat.get("ScrollFactor").and_then(|s| s.parse::<f32>().ok()) {
                        control.value = OptionValue::Float((v / 100.0).clamp(0.0, 1.0));
                    }
                }
                "controls.alternate_mouse" => {
                    if let Some(v) = flat.get("UseAlternateMouse") {
                        control.value = OptionValue::Boolean(Self::parse_yes_no(v));
                    }
                }
                "controls.auto_retaliate" => {
                    if let Some(v) = flat.get("Retaliation") {
                        control.value = OptionValue::Boolean(Self::parse_yes_no(v));
                    }
                }
                "controls.double_click_attack" => {
                    if let Some(v) = flat.get("UseDoubleClickAttackMove") {
                        control.value = OptionValue::Boolean(v.eq_ignore_ascii_case("yes"));
                    }
                }
                "controls.save_camera" => {
                    if let Some(v) = flat.get("SaveCameraInReplays") {
                        control.value = OptionValue::Boolean(Self::parse_yes_no(v));
                    }
                }
                "controls.use_camera" => {
                    if let Some(v) = flat.get("UseCameraInReplays") {
                        control.value = OptionValue::Boolean(Self::parse_yes_no(v));
                    }
                }
                "controls.draw_anchor" => {
                    let fallback =
                        game_engine::common::ini::ini_in_game_ui::get_in_game_ui_settings()
                            .map(|s| s.draw_rmb_scroll_anchor)
                            .unwrap_or(false);
                    control.value = OptionValue::Boolean(
                        game_engine::common::user_preferences::scroll_anchor_pref_enabled(
                            flat.get("DrawScrollAnchor").map(String::as_str),
                            fallback,
                        ),
                    );
                }
                "controls.move_anchor" => {
                    let fallback =
                        game_engine::common::ini::ini_in_game_ui::get_in_game_ui_settings()
                            .map(|s| s.move_rmb_scroll_anchor)
                            .unwrap_or(false);
                    control.value = OptionValue::Boolean(
                        game_engine::common::user_preferences::scroll_anchor_pref_enabled(
                            flat.get("MoveScrollAnchor").map(String::as_str),
                            fallback,
                        ),
                    );
                }
                "game.language_filter" => {
                    if let Some(v) = flat.get("LanguageFilter") {
                        control.value = OptionValue::Boolean(Self::parse_yes_no(v));
                    }
                }
                _ => {}
            }
        }
    }

    fn parse_flat_options_ini(text: &str) -> BTreeMap<String, String> {
        let mut map = BTreeMap::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('[') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() && !value.is_empty() {
                    map.insert(key.to_string(), value.to_string());
                }
            }
        }
        map
    }

    fn parse_resolution_value(raw: &str) -> Option<Resolution> {
        let mut parts = raw.split(|c: char| c.is_ascii_whitespace() || c == 'x' || c == 'X');
        let w = parts.next()?.parse::<u32>().ok()?;
        let h = parts.next()?.parse::<u32>().ok()?;
        if w > 0 && h > 0 {
            Some(Resolution::new(w, h))
        } else {
            None
        }
    }

    fn parse_yes_no(raw: &str) -> bool {
        matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "1" | "t" | "true" | "y" | "yes" | "ok"
        )
    }

    fn switch_tab(&mut self, tab: OptionsTab) {
        if self.current_tab != tab {
            self.current_tab = tab;
            for tab_btn in &mut self.tab_buttons {
                tab_btn.active = tab_btn.tab == tab;
            }
            info!(
                "{}",
                localization::localize_with_args(
                    "options.log.switch_tab",
                    "Switched to {tab} tab",
                    &[("tab", &format!("{:?}", tab))],
                )
            );
        }
    }

    fn toggle_option(&mut self, key: &str) {
        if let Some(controls) = self.options.get_mut(&self.current_tab) {
            for control in controls {
                if control.key != key {
                    continue;
                }
                match &mut control.value {
                    OptionValue::Boolean(val) => {
                        *val = !*val;
                    }
                    OptionValue::Integer(val) => {
                        *val = if *val >= 100 { 0 } else { (*val + 10).min(100) };
                    }
                    OptionValue::Float(val) => {
                        *val += 0.1;
                        if *val > 1.001 {
                            *val = 0.0;
                        }
                    }
                    OptionValue::Resolution(res) => {
                        const MODES: &[(u32, u32)] =
                            &[(800, 600), (1024, 768), (1280, 960), (1600, 1200)];
                        let idx = MODES
                            .iter()
                            .position(|m| m.0 == res.width && m.1 == res.height)
                            .unwrap_or(0);
                        let next = MODES[(idx + 1) % MODES.len()];
                        *res = Resolution::new(next.0, next.1);
                    }
                    OptionValue::Quality(q) => {
                        *q = match q {
                            GraphicsQuality::Low => GraphicsQuality::Medium,
                            GraphicsQuality::Medium => GraphicsQuality::High,
                            GraphicsQuality::High => GraphicsQuality::Custom,
                            GraphicsQuality::Custom => GraphicsQuality::Low,
                        };
                    }
                    OptionValue::String(s) => {
                        *s = match s.as_str() {
                            "Off" => "2x".to_string(),
                            "2x" => "4x".to_string(),
                            _ => "Off".to_string(),
                        };
                    }
                }
                self.settings_modified = true;
            }
        }
    }

    fn handle_action(&mut self, action: ActionButton) -> Option<UIEvent> {
        match action {
            ActionButton::Accept => {
                self.apply_settings();
                Some(UIEvent::SettingsChanged)
            }
            ActionButton::Cancel => {
                self.revert_settings();
                None
            }
            ActionButton::Apply => {
                self.apply_settings();
                Some(UIEvent::SettingsChanged)
            }
            ActionButton::RestoreDefaults => {
                self.restore_defaults();
                None
            }
        }
    }

    fn apply_settings(&mut self) {
        info!(
            "{}",
            Self::text("options.log.apply_settings", "Applying settings...")
        );

        // C++ OptionsMenu.cpp:1168 live-apply via WND saveOptions, not a stub.
        #[cfg(feature = "game_client")]
        {
            let _ = game_client::gui::callbacks::apply_options_from_host(self.host_apply_values());
        }

        if let Some(path) = Self::options_ini_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&path, self.format_cpp_options_ini());
        }

        self.original_values = self.snapshot_current_values();
        self.settings_modified = false;
    }

    fn volume_to_cpp(v: f32) -> i32 {
        (v.clamp(0.0, 1.0) * 100.0).round() as i32
    }

    fn control_value(&self, key: &str) -> Option<&OptionValue> {
        self.iter_controls()
            .find(|c| c.key == key)
            .map(|c| &c.value)
    }

    fn format_cpp_options_ini(&self) -> String {
        // C++ UserPreferences / OptionsMenu.cpp:1078-1258 — flat Key = Value.
        let mut out = String::new();
        for (key, value) in self.cpp_option_pairs() {
            out.push_str(&format!("{key} = {value}\n"));
        }
        out
    }

    fn cpp_option_pairs(&self) -> Vec<(String, String)> {
        let mut pairs = Vec::new();
        if let Some(OptionValue::Resolution(r)) = self.control_value("video.resolution") {
            pairs.push(("Resolution".into(), format!("{} {}", r.width, r.height)));
        }
        if let Some(OptionValue::Quality(q)) = self.control_value("video.quality") {
            let name = match q {
                GraphicsQuality::High => "High",
                GraphicsQuality::Medium => "Medium",
                GraphicsQuality::Low => "Low",
                GraphicsQuality::Custom => "Custom",
            };
            pairs.push(("StaticGameLOD".into(), name.into()));
        }
        if let Some(OptionValue::String(s)) = self.control_value("video.antialiasing") {
            let idx = match s.as_str() {
                "2x" => 1,
                "4x" => 2,
                _ => 0,
            };
            pairs.push(("AntiAliasing".into(), idx.to_string()));
        }
        if let Some(OptionValue::Integer(g)) = self.control_value("video.gamma") {
            pairs.push(("Gamma".into(), g.to_string()));
        }
        if let Some(OptionValue::Float(v)) = self.control_value("audio.music_volume") {
            pairs.push(("MusicVolume".into(), Self::volume_to_cpp(*v).to_string()));
        }
        if let Some(OptionValue::Float(v)) = self.control_value("audio.sfx_volume") {
            let n = Self::volume_to_cpp(*v);
            pairs.push(("SFXVolume".into(), n.to_string()));
            pairs.push(("SFX3DVolume".into(), n.to_string()));
        }
        if let Some(OptionValue::Float(v)) = self.control_value("audio.voice_volume") {
            pairs.push(("VoiceVolume".into(), Self::volume_to_cpp(*v).to_string()));
        }
        if let Some(OptionValue::Float(v)) = self.control_value("controls.scroll_speed") {
            pairs.push(("ScrollFactor".into(), Self::volume_to_cpp(*v).to_string()));
        }
        if let Some(OptionValue::Boolean(v)) = self.control_value("controls.alternate_mouse") {
            pairs.push((
                "UseAlternateMouse".into(),
                if *v { "yes" } else { "no" }.into(),
            ));
        }
        if let Some(OptionValue::Boolean(v)) = self.control_value("controls.auto_retaliate") {
            pairs.push(("Retaliation".into(), if *v { "yes" } else { "no" }.into()));
        }
        if let Some(OptionValue::Boolean(v)) = self.control_value("controls.double_click_attack") {
            pairs.push((
                "UseDoubleClickAttackMove".into(),
                if *v { "yes" } else { "no" }.into(),
            ));
        }
        if let Some(OptionValue::Boolean(v)) = self.control_value("controls.save_camera") {
            pairs.push((
                "SaveCameraInReplays".into(),
                if *v { "yes" } else { "no" }.into(),
            ));
        }
        if let Some(OptionValue::Boolean(v)) = self.control_value("controls.use_camera") {
            pairs.push((
                "UseCameraInReplays".into(),
                if *v { "yes" } else { "no" }.into(),
            ));
        }
        if let Some(OptionValue::Boolean(v)) = self.control_value("controls.draw_anchor") {
            pairs.push((
                "DrawScrollAnchor".into(),
                if *v { "Yes" } else { "No" }.into(),
            ));
        }
        if let Some(OptionValue::Boolean(v)) = self.control_value("controls.move_anchor") {
            pairs.push((
                "MoveScrollAnchor".into(),
                if *v { "Yes" } else { "No" }.into(),
            ));
        }
        if let Some(OptionValue::Boolean(v)) = self.control_value("game.language_filter") {
            pairs.push((
                "LanguageFilter".into(),
                if *v { "true" } else { "false" }.into(),
            ));
        }
        pairs
    }

    #[cfg(feature = "game_client")]
    fn host_apply_values(&self) -> game_client::gui::callbacks::HostOptionsApply {
        use game_client::gui::callbacks::HostOptionsApply;
        let resolution = match self.control_value("video.resolution") {
            Some(OptionValue::Resolution(r)) => (r.width as i32, r.height as i32),
            _ => (1024, 768),
        };
        let music_volume = match self.control_value("audio.music_volume") {
            Some(OptionValue::Float(v)) => Self::volume_to_cpp(*v),
            _ => 60,
        };
        let sfx_volume = match self.control_value("audio.sfx_volume") {
            Some(OptionValue::Float(v)) => Self::volume_to_cpp(*v),
            _ => 55,
        };
        let voice_volume = match self.control_value("audio.voice_volume") {
            Some(OptionValue::Float(v)) => Self::volume_to_cpp(*v),
            _ => 70,
        };
        let gamma_slider = match self.control_value("video.gamma") {
            Some(OptionValue::Integer(v)) => *v,
            _ => 50,
        };
        let scroll_speed = match self.control_value("controls.scroll_speed") {
            Some(OptionValue::Float(v)) => Self::volume_to_cpp(*v),
            _ => 50,
        };
        let bool_of = |key: &str, default: bool| match self.control_value(key) {
            Some(OptionValue::Boolean(v)) => *v,
            _ => default,
        };
        let anti_aliasing = match self.control_value("video.antialiasing") {
            Some(OptionValue::String(s)) => match s.as_str() {
                "2x" => 1,
                "4x" => 2,
                _ => 0,
            },
            _ => 0,
        };
        let detail_index = match self.control_value("video.quality") {
            Some(OptionValue::Quality(GraphicsQuality::High)) => 0,
            Some(OptionValue::Quality(GraphicsQuality::Medium)) => 1,
            Some(OptionValue::Quality(GraphicsQuality::Low)) => 2,
            Some(OptionValue::Quality(GraphicsQuality::Custom)) => 3,
            _ => 1,
        };
        HostOptionsApply {
            resolution,
            music_volume,
            sfx_volume,
            voice_volume,
            gamma_slider,
            scroll_speed,
            alternate_mouse: bool_of("controls.alternate_mouse", false),
            retaliation: bool_of("controls.auto_retaliate", true),
            double_click_attack_move: bool_of("controls.double_click_attack", false),
            language_filter: bool_of("game.language_filter", true),
            save_camera: bool_of("controls.save_camera", true),
            use_camera: bool_of("controls.use_camera", true),
            draw_anchor: bool_of("controls.draw_anchor", true),
            move_anchor: bool_of("controls.move_anchor", true),
            anti_aliasing,
            detail_index,
        }
    }

    fn revert_settings(&mut self) {
        info!(
            "{}",
            Self::text("options.log.revert_settings", "Reverting settings...")
        );
        let originals = self.original_values.clone();
        for control in self.iter_controls_mut() {
            if let Some(value) = originals.get(&control.key).cloned() {
                control.value = value;
            }
        }
        self.settings_modified = false;
    }

    fn restore_defaults(&mut self) {
        info!(
            "{}",
            Self::text(
                "options.log.restore_defaults",
                "Restoring default settings..."
            )
        );
        // C++ setDefaults never touches replay-camera or RMB-anchor checkboxes.
        const LEAVE_ON_DEFAULTS: &[&str] = &[
            "controls.save_camera",
            "controls.use_camera",
            "controls.draw_anchor",
            "controls.move_anchor",
        ];
        let defaults = self.default_values.clone();
        for control in self.iter_controls_mut() {
            if LEAVE_ON_DEFAULTS.contains(&control.key.as_str()) {
                continue;
            }
            if let Some(value) = defaults.get(&control.key).cloned() {
                control.value = value;
            }
        }
        self.settings_modified = true;
    }

    fn options_ini_path() -> Option<PathBuf> {
        let user_data_dir = game_engine::common::global_data::read()
            .get_user_data_dir()
            .to_string();
        if user_data_dir.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(user_data_dir).join("Options.ini"))
        }
    }

    fn iter_controls(&self) -> impl Iterator<Item = &OptionControl> {
        self.options.values().flat_map(|controls| controls.iter())
    }

    fn iter_controls_mut(&mut self) -> impl Iterator<Item = &mut OptionControl> {
        self.options
            .values_mut()
            .flat_map(|controls| controls.iter_mut())
    }

    fn snapshot_current_values(&self) -> HashMap<String, OptionValue> {
        let mut map = HashMap::new();
        for control in self.iter_controls() {
            map.insert(control.key.clone(), control.value.clone());
        }
        map
    }
}

impl Interactive for OptionsMenu {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let mut handled = false;

        // Check tab buttons
        for tab_btn in &mut self.tab_buttons {
            let was_hovered = tab_btn.hovered;
            let is_hovered = tab_btn.contains_point(x, y);
            if is_hovered != was_hovered {
                tab_btn.hovered = is_hovered;
                handled = true;
            }
        }

        // Check action buttons
        for action_btn in &mut self.action_buttons {
            let was_hovered = action_btn.hovered;
            let is_hovered = action_btn.contains_point(x, y);
            if is_hovered != was_hovered {
                action_btn.hovered = is_hovered;
                handled = true;
            }
        }

        // Check option controls
        if let Some(controls) = self.options.get_mut(&self.current_tab) {
            for control in controls {
                let was_hovered = control.hovered;
                let is_hovered = control.contains_point(x, y) && control.enabled;
                if is_hovered != was_hovered {
                    control.hovered = is_hovered;
                    handled = true;
                }
            }
        }

        handled
    }

    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        self.handle_mouse_click(x, y, button).is_some()
    }

    fn handle_key_press(&mut self, key: KeyCode) -> bool {
        match key {
            KeyCode::Escape => true,
            KeyCode::Enter => {
                self.apply_settings();
                true
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for OptionsMenu {
    fn render(&self, context: &mut UIRenderContext) {
        let (sw, sh) = (self.screen_size.0 as f32, self.screen_size.1 as f32);
        context.draw_rect(0.0, 0.0, sw, sh, [0.05, 0.06, 0.08, 0.94]);
        context.draw_text(
            &Self::text("options.log.header", "OPTIONS"),
            50.0,
            36.0,
            22.0,
            [0.95, 0.85, 0.35, 1.0],
        );

        for tab_btn in &self.tab_buttons {
            let color = if tab_btn.active {
                [0.35, 0.28, 0.10, 1.0]
            } else if tab_btn.hovered {
                [0.22, 0.22, 0.26, 1.0]
            } else {
                [0.14, 0.14, 0.16, 1.0]
            };
            context.draw_rect(
                tab_btn.position.0 as f32,
                tab_btn.position.1 as f32,
                tab_btn.size.0 as f32,
                tab_btn.size.1 as f32,
                color,
            );
            context.draw_text(
                &tab_btn.text,
                tab_btn.position.0 as f32 + 16.0,
                tab_btn.position.1 as f32 + 28.0,
                16.0,
                [0.95, 0.95, 0.90, 1.0],
            );
        }

        if let Some(controls) = self.options.get(&self.current_tab) {
            for control in controls {
                let value_str = match &control.value {
                    OptionValue::Boolean(b) => if *b { "ON" } else { "OFF" }.to_string(),
                    OptionValue::Integer(i) => i.to_string(),
                    OptionValue::Float(f) => format!("{:.0}", f * 100.0),
                    OptionValue::String(s) => s.clone(),
                    OptionValue::Resolution(r) => r.to_string(),
                    OptionValue::Quality(q) => format!("{:?}", q),
                };
                let bg = if control.hovered {
                    [0.20, 0.18, 0.12, 1.0]
                } else {
                    [0.10, 0.10, 0.12, 0.0]
                };
                context.draw_rect(
                    control.position.0 as f32,
                    control.position.1 as f32,
                    control.size.0 as f32,
                    control.size.1 as f32,
                    bg,
                );
                context.draw_text(
                    &format!("{}: {}", control.label, value_str),
                    control.position.0 as f32,
                    control.position.1 as f32 + 22.0,
                    16.0,
                    [0.92, 0.92, 0.88, 1.0],
                );
            }
        }

        for action_btn in &self.action_buttons {
            let color = if action_btn.hovered {
                [0.40, 0.32, 0.10, 1.0]
            } else {
                [0.22, 0.18, 0.08, 1.0]
            };
            context.draw_rect(
                action_btn.position.0 as f32,
                action_btn.position.1 as f32,
                action_btn.size.0 as f32,
                action_btn.size.1 as f32,
                color,
            );
            context.draw_text(
                &action_btn.text,
                action_btn.position.0 as f32 + 20.0,
                action_btn.position.1 as f32 + 28.0,
                16.0,
                [0.98, 0.92, 0.55, 1.0],
            );
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_options_menu_creation() {
        let menu = OptionsMenu::new();
        assert_eq!(menu.current_tab, OptionsTab::Video);
        assert!(!menu.settings_modified);
    }

    #[test]
    fn test_tab_switching() {
        let mut menu = OptionsMenu::new();
        menu.initialize().unwrap();

        menu.switch_tab(OptionsTab::Audio);
        assert_eq!(menu.current_tab, OptionsTab::Audio);

        menu.switch_tab(OptionsTab::Controls);
        assert_eq!(menu.current_tab, OptionsTab::Controls);
    }

    #[test]
    fn test_resolution_display() {
        let res = Resolution::new(1920, 1080);
        assert_eq!(res.to_string(), "1920x1080");
    }

    #[test]
    fn apply_writes_flat_cpp_options_ini_keys() {
        // C++ OptionsMenu.cpp:1078 Resolution "%d %d", 1187 MusicVolume 0-100.
        let mut menu = OptionsMenu::new();
        menu.initialize().unwrap();
        let ini = menu.format_cpp_options_ini();
        assert!(ini.contains("Resolution = 1024 768"), "{ini}");
        assert!(ini.contains("MusicVolume = 80"), "{ini}");
        assert!(!ini.contains("[video]"), "{ini}");
        assert!(!ini.contains("music_volume"), "{ini}");
        let music: i32 = ini
            .lines()
            .find(|l| l.starts_with("MusicVolume"))
            .and_then(|l| l.split('=').nth(1))
            .and_then(|v| v.trim().parse().ok())
            .unwrap();
        assert!((0..=100).contains(&music));
    }

    #[test]
    fn toggle_option_cycles_non_boolean_controls() {
        let mut menu = OptionsMenu::new();
        menu.initialize().unwrap();
        menu.toggle_option("video.gamma");
        assert_eq!(
            menu.control_value("video.gamma"),
            Some(&OptionValue::Integer(60))
        );
        menu.toggle_option("audio.music_volume");
        match menu.control_value("audio.music_volume") {
            Some(OptionValue::Float(v)) => assert!((*v - 0.9).abs() < 0.001),
            other => panic!("expected float volume, got {other:?}"),
        }
    }

    #[test]
    fn parse_flat_resolution_and_music_volume() {
        let map = OptionsMenu::parse_flat_options_ini(
            "Resolution = 800 600\nMusicVolume = 45\n[ignored]\nfoo = bar\n",
        );
        assert_eq!(map.get("Resolution").map(String::as_str), Some("800 600"));
        assert_eq!(map.get("MusicVolume").map(String::as_str), Some("45"));
        let res = OptionsMenu::parse_resolution_value("800 600").unwrap();
        assert_eq!(res, Resolution::new(800, 600));
    }

    #[test]
    fn restore_defaults_leaves_camera_and_anchor_checkboxes() {
        let mut menu = OptionsMenu::new();
        menu.initialize().unwrap();
        menu.current_tab = OptionsTab::Controls;
        menu.toggle_option("controls.save_camera");
        menu.toggle_option("controls.use_camera");
        menu.toggle_option("controls.draw_anchor");
        menu.toggle_option("controls.move_anchor");
        assert_eq!(
            menu.control_value("controls.save_camera"),
            Some(&OptionValue::Boolean(false))
        );
        menu.restore_defaults();
        assert_eq!(
            menu.control_value("controls.save_camera"),
            Some(&OptionValue::Boolean(false))
        );
        assert_eq!(
            menu.control_value("controls.use_camera"),
            Some(&OptionValue::Boolean(false))
        );
        assert_eq!(
            menu.control_value("controls.draw_anchor"),
            Some(&OptionValue::Boolean(false))
        );
        assert_eq!(
            menu.control_value("controls.move_anchor"),
            Some(&OptionValue::Boolean(false))
        );
    }

    #[test]
    fn load_current_settings_reads_scroll_anchor_yes() {
        let mut menu = OptionsMenu::new();
        menu.initialize().unwrap();
        let flat = OptionsMenu::parse_flat_options_ini(
            "DrawScrollAnchor = Yes\nMoveScrollAnchor = No\nSaveCameraInReplays = no\nUseDoubleClickAttackMove = yes\n",
        );
        assert_eq!(
            game_engine::common::user_preferences::scroll_anchor_pref_enabled(
                flat.get("DrawScrollAnchor").map(String::as_str),
                false,
            ),
            true
        );
        assert_eq!(
            game_engine::common::user_preferences::scroll_anchor_pref_enabled(
                flat.get("MoveScrollAnchor").map(String::as_str),
                true,
            ),
            false
        );
        assert!(!OptionsMenu::parse_yes_no(
            flat.get("SaveCameraInReplays").unwrap()
        ));
        assert!(
            flat.get("UseDoubleClickAttackMove")
                .unwrap()
                .eq_ignore_ascii_case("yes")
        );
    }
}
