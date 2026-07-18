//! Options Menu System
//!
//! This module implements the comprehensive options/settings menu matching the original
//! C&C Generals interface from OptionsMenu.cpp.
//! Provides tabs for Video, Audio, Controls, and Game settings.

use super::{
    layout, utils, ClickSpring, Interactive, KeyCode, MouseButton, Renderable, UIEvent,
    UIRenderContext,
};
use crate::config::{IniParser, LoadMode};
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
#[derive(Debug, Clone)]
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

        let Some(path) = Self::options_ini_path() else {
            return;
        };
        if !path.exists() {
            return;
        }

        let mut parser = IniParser::new();
        if parser.load_file(&path, LoadMode::Overwrite).is_err() {
            return;
        }

        for control in self.iter_controls_mut() {
            let Some((section, key)) = control.key.split_once('.') else {
                continue;
            };

            match &mut control.value {
                OptionValue::Boolean(val) => {
                    *val = parser.get_bool(section, key, *val);
                }
                OptionValue::Integer(val) => {
                    *val = parser.get_int(section, key, *val);
                }
                OptionValue::Float(val) => {
                    *val = parser.get_float(section, key, *val);
                }
                OptionValue::String(val) => {
                    *val = parser.get_string(section, key, Some(val.as_str()));
                }
                OptionValue::Resolution(res) => {
                    let raw = parser.get_string(section, key, Some(&res.to_string()));
                    if let Some((w, h)) = raw.split_once('x') {
                        if let (Ok(w), Ok(h)) = (w.trim().parse::<u32>(), h.trim().parse::<u32>()) {
                            *res = Resolution {
                                width: w,
                                height: h,
                            };
                        }
                    }
                }
                OptionValue::Quality(q) => {
                    let raw = parser.get_string(section, key, Some(&format!("{q:?}")));
                    *q = match raw.to_ascii_lowercase().as_str() {
                        "low" => GraphicsQuality::Low,
                        "medium" => GraphicsQuality::Medium,
                        "high" => GraphicsQuality::High,
                        "ultra" => GraphicsQuality::High,
                        "custom" => GraphicsQuality::Custom,
                        _ => *q,
                    };
                }
            }
        }
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
                if control.key == key {
                    if let OptionValue::Boolean(val) = &mut control.value {
                        *val = !*val;
                        self.settings_modified = true;
                    }
                    info!(
                        "{}",
                        localization::localize_with_args(
                            "options.log.toggle_option",
                            "Toggled option: {key}",
                            &[("key", key)],
                        )
                    );
                }
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

        let Some(path) = Self::options_ini_path() else {
            self.settings_modified = false;
            return;
        };

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let mut sections: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for control in self.iter_controls() {
            let Some((section, key)) = control.key.split_once('.') else {
                continue;
            };
            let value = match &control.value {
                OptionValue::Boolean(v) => v.to_string(),
                OptionValue::Integer(v) => v.to_string(),
                OptionValue::Float(v) => format!("{v:.3}"),
                OptionValue::String(v) => v.clone(),
                OptionValue::Resolution(r) => r.to_string(),
                OptionValue::Quality(q) => format!("{q:?}"),
            };
            sections
                .entry(section.to_string())
                .or_default()
                .insert(key.to_string(), value);
        }

        let mut out = String::new();
        out.push_str("; Auto-generated options file (Rust port)\n");
        out.push_str("; Mirrors the C++ Options.ini behaviour at a basic level\n\n");
        for (section, kv) in sections {
            out.push_str(&format!("[{section}]\n"));
            for (key, value) in kv {
                out.push_str(&format!("{key} = {value}\n"));
            }
            out.push('\n');
        }
        let _ = std::fs::write(&path, out);

        self.original_values = self.snapshot_current_values();
        self.settings_modified = false;
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
        let defaults = self.default_values.clone();
        for control in self.iter_controls_mut() {
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
    fn render(&self, _context: &mut UIRenderContext) {
        println!(
            "{}",
            Self::text("options.log.header", "=== OPTIONS MENU ===")
        );

        // Render tab buttons
        println!("{}", Self::text("options.log.tabs_header", "Tabs:"));
        for tab_btn in &self.tab_buttons {
            let state = if tab_btn.active {
                "[ACTIVE]"
            } else if tab_btn.hovered {
                "[HOVERED]"
            } else {
                ""
            };
            println!("  {} {}", tab_btn.text, state);
        }

        // Render current tab options
        println!(
            "\n{} {:?}",
            Self::text("options.log.current_tab", "Current Tab:"),
            self.current_tab
        );

        if let Some(controls) = self.options.get(&self.current_tab) {
            for control in controls {
                let value_str = match &control.value {
                    OptionValue::Boolean(b) => if *b { "ON" } else { "OFF" }.to_string(),
                    OptionValue::Integer(i) => i.to_string(),
                    OptionValue::Float(f) => format!("{:.1}", f * 100.0),
                    OptionValue::String(s) => s.clone(),
                    OptionValue::Resolution(r) => r.to_string(),
                    OptionValue::Quality(q) => format!("{:?}", q),
                };

                let state = if !control.enabled {
                    "[DISABLED]"
                } else if control.hovered {
                    "[HOVERED]"
                } else {
                    ""
                };

                println!("  {}: {} {}", control.label, value_str, state);
            }
        }

        // Render action buttons
        println!("\n{}", Self::text("options.log.actions_header", "Actions:"));
        for action_btn in &self.action_buttons {
            let state = if action_btn.hovered { "[HOVERED]" } else { "" };
            println!("  {} {}", action_btn.text, state);
        }

        if self.settings_modified {
            println!(
                "\n{}",
                Self::text("options.log.unsaved_changes", "* Unsaved changes")
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
}
