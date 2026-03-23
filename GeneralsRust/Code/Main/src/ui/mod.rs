//! Main UI System Module
//!
//! This module provides the complete user interface system for Command & Conquer Generals,
//! including main menu, in-game HUD, faction selection, and all RTS interface elements.
//!
//! The system is designed to match the original game's UI layout and functionality while
//! providing modern responsiveness and interaction.

use log::warn;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

pub mod audio;
pub mod campaign_menu;
pub mod credits_screen;
pub mod events;
pub mod faction_selection;
pub mod hud;
pub mod hud_state;
pub mod layout_manager;
pub mod loading_screen;
pub mod main_menu;
pub mod minimap_panel;
pub mod objectives;
pub mod options_menu;
pub mod pause_menu;
pub mod quit_dialog;
pub mod rts_interface;
pub mod save_load_menu;
pub mod skirmish_menu;
pub mod themes;
pub mod ui_manager;
pub mod victory_screen;
pub mod wgpu_renderer;
pub mod wgpu_ui_system;
pub mod widgets;

// Re-exports for convenience
pub use campaign_menu::{CampaignFaction, CampaignMenu, MedalType, Mission};
pub use credits_screen::CreditsScreen;
pub use events::{InputEvent, KeyEvent, MouseEvent, UIEventHandler};
pub use faction_selection::{Faction, FactionSelectionScreen};
pub use hud::{GameHUD, MiniMap, ResourceDisplay};
pub use hud_state::{
    color_for_player, default_minimap_viewport, normalized_minimap_rect, BuildQueueEntry,
    DiagnosticsOverlayStats, GameUIState, MinimapActionKind, MinimapDot, MinimapInteraction,
    RadarMessageEntry, RadarPing, RadarPingKind, UiColor, UiPos2, UiRect, UiTextureId, UiVec2,
    UnitDisplayInfo, VictoryOverlayAction,
};
pub use loading_screen::LoadingScreen;
pub use main_menu::{MainMenu, MainMenuState};
pub use minimap_panel::{update_minimap_state, BeaconDot, MinimapClickEvent, MinimapUIState};
pub use options_menu::{GraphicsQuality, OptionsMenu, OptionsTab, Resolution};
pub use pause_menu::{PauseMenu, PauseMenuAction};
pub use quit_dialog::QuitDialog;
pub use rts_interface::{BuildingInterface, RTSInterface, UnitCommandPanel};
pub use save_load_menu::{SaveGameEntry, SaveLoadMenu, SaveLoadMode};
pub use skirmish_menu::{GameRules, GameSlot, PlayerColor, PlayerType, SkirmishMenu, MAX_SLOTS};
pub use themes::{Colors, GeneralsTheme, UITheme};
pub use ui_manager::{UIEvent, UIManager, UIState};
pub use victory_screen::{VictoryScreen, VictoryScreenType};
pub use wgpu_ui_system::{UISystemEvent, UISystemState, WgpuUISystem};
pub use widgets::{Button, Panel, ProgressBar, Slider, Text, UIWidget};

/// UI Screen types that can be displayed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Title screen with game logo
    Title,
    /// Main menu with options
    MainMenu,
    /// Faction selection screen
    FactionSelection,
    /// Map selection for skirmish
    MapSelection,
    /// Options menu
    Options,
    /// Load game menu
    LoadGame,
    /// Save game menu
    SaveGame,
    /// Campaign mission select
    Campaign,
    /// Skirmish setup
    Skirmish,
    /// Credits screen
    Credits,
    /// Loading screen
    Loading,
    /// Quit confirmation dialog
    QuitDialog,
    /// In-game HUD during gameplay
    GameHUD,
    /// Pause menu during game
    PauseMenu,
    /// Victory/Defeat screen
    Victory,
    /// Disconnection screen
    Disconnect,
}

impl Screen {
    /// Screen used while the engine is booting before the first menu-ready handoff.
    pub fn startup_entry_screen(quick_start: bool) -> Self {
        // C++ `-quickstart` suppresses the intro/title path and lands directly
        // in the shell menu flow after the startup shell-map checks are applied.
        if quick_start {
            Self::MainMenu
        } else {
            let global = game_engine::global_data::read();
            if !global.writable.play_intro || global.writable.after_intro {
                Self::MainMenu
            } else {
                Self::Title
            }
        }
    }

    /// Screen used once startup has completed and the interactive menu is ready.
    pub const fn first_menu_screen() -> Self {
        Self::MainMenu
    }
}

/// UI colors matching the original Generals theme
pub mod colors {
    pub const BLUE_DARK: (u8, u8, u8) = (20, 40, 80);
    pub const BLUE_LIGHT: (u8, u8, u8) = (60, 120, 200);
    pub const ORANGE: (u8, u8, u8) = (255, 140, 0);
    pub const GREEN: (u8, u8, u8) = (0, 200, 0);
    pub const RED: (u8, u8, u8) = (200, 0, 0);
    pub const YELLOW: (u8, u8, u8) = (255, 255, 0);
    pub const WHITE: (u8, u8, u8) = (255, 255, 255);
    pub const GRAY: (u8, u8, u8) = (128, 128, 128);
    pub const BLACK: (u8, u8, u8) = (0, 0, 0);
}

/// Font definitions for UI elements
pub mod fonts {
    pub const TITLE_SIZE: f32 = 36.0;
    pub const MENU_SIZE: f32 = 24.0;
    pub const BUTTON_SIZE: f32 = 18.0;
    pub const HUD_SIZE: f32 = 16.0;
    pub const TOOLTIP_SIZE: f32 = 14.0;
}

/// Animation configuration
pub mod animations {
    use std::time::Duration;

    pub const FADE_DURATION: Duration = Duration::from_millis(300);
    pub const SLIDE_DURATION: Duration = Duration::from_millis(250);
    pub const BUTTON_HOVER_DURATION: Duration = Duration::from_millis(100);
    pub const MENU_TRANSITION_DURATION: Duration = Duration::from_millis(200);
    pub const CLICK_SPRING_DURATION: f32 = 0.35;
    pub const CLICK_SPRING_DAMPING: f32 = 12.0;
    pub const CLICK_SPRING_FREQUENCY: f32 = 24.0;
    pub const CLICK_SPRING_AMPLITUDE: f32 = 0.06;
    pub const CLICK_PRESSED_SCALE: f32 = 0.96;
    pub const CLICK_PRESSED_DURATION: f32 = 0.08;
}

/// Spring-style click animation used by legacy UI screens.
#[derive(Debug, Clone)]
pub struct ClickSpring {
    elapsed: Option<f32>,
    pressed_time: f32,
}

impl ClickSpring {
    pub fn new() -> Self {
        Self {
            elapsed: None,
            pressed_time: 0.0,
        }
    }

    pub fn trigger(&mut self) {
        self.elapsed = Some(0.0);
        self.pressed_time = animations::CLICK_PRESSED_DURATION;
    }

    pub fn update(&mut self, delta_time: f32) {
        if let Some(elapsed) = self.elapsed {
            let next = elapsed + delta_time;
            if next >= animations::CLICK_SPRING_DURATION {
                self.elapsed = None;
            } else {
                self.elapsed = Some(next);
            }
        }

        if self.pressed_time > 0.0 {
            self.pressed_time = (self.pressed_time - delta_time).max(0.0);
        }
    }

    pub fn is_pressed(&self) -> bool {
        self.pressed_time > 0.0
    }

    pub fn scale(&self) -> f32 {
        let mut scale = 1.0;
        if self.is_pressed() {
            scale *= animations::CLICK_PRESSED_SCALE;
        }

        if let Some(elapsed) = self.elapsed {
            let spring = (-animations::CLICK_SPRING_DAMPING * elapsed).exp()
                * (animations::CLICK_SPRING_FREQUENCY * elapsed).sin();
            scale *= 1.0 + animations::CLICK_SPRING_AMPLITUDE * spring;
        }

        scale
    }
}

/// Layout constants for UI positioning
pub mod layout {
    pub const MENU_BUTTON_WIDTH: u32 = 200;
    pub const MENU_BUTTON_HEIGHT: u32 = 40;
    pub const MENU_SPACING: u32 = 10;
    pub const HUD_PANEL_HEIGHT: u32 = 120;
    pub const MINIMAP_SIZE: u32 = 200;
    pub const RESOURCE_PANEL_WIDTH: u32 = 300;
}

/// Sound effects for UI interactions
pub mod sounds {
    pub const BUTTON_HOVER: &str = "GUI_ButtonMouseOver";
    pub const BUTTON_CLICK: &str = "GUI_ButtonClick";
    pub const MENU_OPEN: &str = "GUI_MenuOpen";
    pub const MENU_CLOSE: &str = "GUI_MenuClose";
    pub const ERROR: &str = "GUI_Error";
    pub const VICTORY: &str = "GUI_Victory";
    pub const DEFEAT: &str = "GUI_Defeat";
}

/// Concrete on-disk sound paths used by the archive system.
///
/// These are used by the Rust UI layer to request audio playback without needing to resolve
/// `sounds::*` symbolic names (which normally come from INI tables in the original game).
pub mod sound_files {
    // Sound files live under `Data/Audio/Sounds` in the original archives.
    // These align with `SoundEffects.ini` entries like `GUIBoarderFadeIn` and `GUITransitionFade`.
    pub const BUTTON_HOVER: &str = "Data/Audio/Sounds/uboarder.wav";
    pub const BUTTON_CLICK: &str = "Data/Audio/Sounds/ubutton2.wav";
}

/// Common UI helper functions
pub mod utils {

    /// Convert screen coordinates to UI coordinates
    pub fn screen_to_ui(screen_pos: (i32, i32), screen_size: (u32, u32)) -> (f32, f32) {
        let (x, y) = screen_pos;
        let (width, height) = screen_size;
        (x as f32 / width as f32, y as f32 / height as f32)
    }

    /// Convert UI coordinates to screen coordinates
    pub fn ui_to_screen(ui_pos: (f32, f32), screen_size: (u32, u32)) -> (i32, i32) {
        let (x, y) = ui_pos;
        let (width, height) = screen_size;
        ((x * width as f32) as i32, (y * height as f32) as i32)
    }

    /// Check if point is inside rectangle
    pub fn point_in_rect(point: (i32, i32), rect: (i32, i32, u32, u32)) -> bool {
        let (px, py) = point;
        let (rx, ry, rw, rh) = rect;
        px >= rx && py >= ry && px < rx + rw as i32 && py < ry + rh as i32
    }

    /// Interpolate between two colors
    pub fn lerp_color(color1: (u8, u8, u8), color2: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
        let t = t.clamp(0.0, 1.0);
        (
            (color1.0 as f32 * (1.0 - t) + color2.0 as f32 * t) as u8,
            (color1.1 as f32 * (1.0 - t) + color2.1 as f32 * t) as u8,
            (color1.2 as f32 * (1.0 - t) + color2.2 as f32 * t) as u8,
        )
    }

    /// Scale a rectangle from its center.
    pub fn scale_rect_center(rect: (i32, i32, u32, u32), scale: f32) -> (f32, f32, f32, f32) {
        let (x, y, w, h) = rect;
        let center_x = x as f32 + w as f32 * 0.5;
        let center_y = y as f32 + h as f32 * 0.5;
        let width = w as f32 * scale;
        let height = h as f32 * scale;
        (
            center_x - width * 0.5,
            center_y - height * 0.5,
            width,
            height,
        )
    }
}

/// Trait for UI components that can be rendered
pub trait Renderable {
    fn render(&self, context: &mut UIRenderContext);
    fn get_bounds(&self) -> (i32, i32, u32, u32);
    fn is_visible(&self) -> bool;
}

/// Trait for UI components that can handle input
pub trait Interactive {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool;
    fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool;
    fn handle_key_press(&mut self, key: KeyCode) -> bool;
    fn handle_text_input(&mut self, text: &str) -> bool;
}

/// Mouse button types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

/// Keyboard key codes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Escape,
    Enter,
    Space,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Key0,
    Key1,
    Key2,
    Key3,
    Key4,
    Key5,
    Key6,
    Key7,
    Key8,
    Key9,
    LShift,
    RShift,
    LControl,
    RControl,
    LAlt,
    RAlt,
    Other(u32),
}

/// UI rendering context for drawing operations
pub struct UIRenderContext {
    pub screen_size: (u32, u32),
    pub delta_time: f32,
    pub mouse_position: (i32, i32),
    pub font_manager: Arc<Mutex<FontManager>>,
    pub texture_manager: Arc<Mutex<TextureManager>>,
}

impl Default for UIRenderContext {
    fn default() -> Self {
        Self::new()
    }
}

impl UIRenderContext {
    pub fn new() -> Self {
        Self {
            screen_size: (1024, 768),
            delta_time: 0.016,
            mouse_position: (0, 0),
            font_manager: Arc::new(Mutex::new(FontManager::new())),
            texture_manager: Arc::new(Mutex::new(TextureManager::new())),
        }
    }
}

/// Font management for UI text rendering
pub struct FontManager {
    fonts: HashMap<String, FontData>,
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

impl FontManager {
    pub fn new() -> Self {
        Self {
            fonts: HashMap::new(),
        }
    }

    pub fn load_font(
        &mut self,
        name: &str,
        path: &str,
        size: f32,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Keep UI initialization resilient when packaged font assets are absent.
        if !std::path::Path::new(path).exists() {
            warn!("UI font asset missing: {}", path);
        }
        self.fonts.insert(
            name.to_string(),
            FontData {
                name: name.to_string(),
                size,
                line_height: size * 1.2,
            },
        );
        Ok(())
    }

    pub fn get_font(&self, name: &str) -> Option<&FontData> {
        self.fonts.get(name)
    }
}

/// Font data structure
pub struct FontData {
    pub name: String,
    pub size: f32,
    pub line_height: f32,
}

/// Texture management for UI graphics
pub struct TextureManager {
    textures: HashMap<String, TextureData>,
}

impl Default for TextureManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TextureManager {
    pub fn new() -> Self {
        Self {
            textures: HashMap::new(),
        }
    }

    pub fn load_texture(
        &mut self,
        name: &str,
        path: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let texture = match image::open(path) {
            Ok(image) => {
                let rgba = image.to_rgba8();
                TextureData {
                    name: name.to_string(),
                    width: rgba.width(),
                    height: rgba.height(),
                    data: rgba.into_raw(),
                }
            }
            Err(err) => {
                // Fallback placeholder to keep UI operational when assets are missing.
                warn!("UI texture asset missing or invalid ({}): {}", path, err);
                TextureData {
                    name: name.to_string(),
                    width: 1,
                    height: 1,
                    data: vec![255, 255, 255, 255],
                }
            }
        };

        self.textures.insert(name.to_string(), texture);
        Ok(())
    }

    pub fn get_texture(&self, name: &str) -> Option<&TextureData> {
        self.textures.get(name)
    }
}

/// Texture data structure
pub struct TextureData {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

#[cfg(test)]
mod tests {
    use super::{utils, Screen, *};
    use game_engine::global_data;
    use std::sync::Mutex;

    static GLOBAL_DATA_TEST_LOCK: Mutex<()> = Mutex::new(());

    fn with_global_data_snapshot_restored<F: FnOnce()>(f: F) {
        let _guard = GLOBAL_DATA_TEST_LOCK.lock().unwrap();
        let snapshot = global_data::read().clone();
        f();
        *global_data::write() = snapshot;
    }

    #[test]
    fn test_coordinate_conversion() {
        let screen_size = (1024, 768);
        let screen_pos = (512, 384);
        let ui_pos = utils::screen_to_ui(screen_pos, screen_size);
        assert_eq!(ui_pos, (0.5, 0.5));

        let back_to_screen = utils::ui_to_screen(ui_pos, screen_size);
        assert_eq!(back_to_screen, screen_pos);
    }

    #[test]
    fn test_point_in_rect() {
        let rect = (10, 20, 100, 50);
        assert!(utils::point_in_rect((50, 40), rect));
        assert!(!utils::point_in_rect((5, 40), rect));
        assert!(!utils::point_in_rect((50, 15), rect));
    }

    #[test]
    fn test_color_lerp() {
        let color1 = (0, 0, 0);
        let color2 = (255, 255, 255);
        let mid = utils::lerp_color(color1, color2, 0.5);
        assert_eq!(mid, (127, 127, 127));
    }

    #[test]
    fn quick_start_enters_main_menu_immediately() {
        with_global_data_snapshot_restored(|| {
            {
                let mut global = global_data::write();
                global.writable.play_intro = true;
                global.writable.after_intro = false;
            }

            assert_eq!(Screen::startup_entry_screen(true), Screen::MainMenu);
            assert_eq!(Screen::startup_entry_screen(false), Screen::Title);
        });
    }

    #[test]
    fn intro_disabled_startup_enters_main_menu_immediately() {
        with_global_data_snapshot_restored(|| {
            {
                let mut global = global_data::write();
                global.writable.play_intro = false;
                global.writable.after_intro = true;
            }

            assert_eq!(Screen::startup_entry_screen(false), Screen::MainMenu);
        });
    }
}

// Additional types needed for selection renderer
/// UI Vertex for rendering
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Vertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub tex_coords: [f32; 2],
}

/// UI Color type
pub type UIColor = [f32; 4];

/// Blend modes for UI rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    None,
    Alpha,
    Additive,
    Multiply,
}

/// Primitive types for UI rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimitiveType {
    Points,
    Lines,
    LineStrip,
    Triangles,
    TriangleStrip,
}

/// UI render command
#[derive(Debug, Clone)]
pub struct UIRenderCommand {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u16>,
    pub texture_id: Option<u32>,
    pub blend_mode: BlendMode,
    pub primitive_type: PrimitiveType,
    pub clip_rect: Option<(i32, i32, u32, u32)>,
}

impl Default for UIRenderCommand {
    fn default() -> Self {
        Self::new()
    }
}

impl UIRenderCommand {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            texture_id: None,
            blend_mode: BlendMode::Alpha,
            primitive_type: PrimitiveType::Triangles,
            clip_rect: None,
        }
    }
}
