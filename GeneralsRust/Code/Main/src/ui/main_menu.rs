//! Main Menu System
//!
//! This module implements the main menu screen with options for starting games,
//! loading saves, accessing multiplayer, options, and exiting the game.

use super::{
    animations, layout, sound_files, utils, ClickSpring, Interactive, KeyCode, MouseButton,
    Renderable, Screen, UIEvent, UIRenderContext,
};
use crate::game_logic::GameMode;
use crate::localization;
use log::trace;

/// Main menu state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MainMenuState {
    /// Main menu options
    Main,
    /// Single player sub-menu
    SinglePlayer,
    /// Multiplayer options
    Multiplayer,
    /// Options menu
    Options,
    /// Credits screen
    Credits,
}

/// Main menu button IDs
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonId {
    NewGame,
    LoadGame,
    Multiplayer,
    Options,
    Credits,
    Exit,
    // Single player sub-menu
    Campaign,
    Skirmish,
    Challenges,
    Back,
    // Multiplayer sub-menu
    Internet,
    Network,
    Direct,
    // Options sub-menu
    Audio,
    Video,
    Controls,
    Gameplay,
}

/// Main menu button
struct MenuButton {
    id: ButtonId,
    text: String,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    pressed: bool,
    enabled: bool,
    hover_time: f32,
    click_spring: ClickSpring,
}

impl MenuButton {
    fn new(id: ButtonId, text: String, x: i32, y: i32) -> Self {
        Self {
            id,
            text,
            position: (x, y),
            size: (layout::MENU_BUTTON_WIDTH, layout::MENU_BUTTON_HEIGHT),
            hovered: false,
            pressed: false,
            enabled: true,
            hover_time: 0.0,
            click_spring: ClickSpring::new(),
        }
    }

    fn contains_point(&self, x: i32, y: i32) -> bool {
        utils::point_in_rect(
            (x, y),
            (self.position.0, self.position.1, self.size.0, self.size.1),
        )
    }

    fn set_hovered(&mut self, hovered: bool) {
        if hovered && !self.hovered {
            self.hover_time = 0.0;
        } else if !hovered {
            self.hover_time = 0.0;
        }
        self.hovered = hovered;
    }

    fn get_hover_progress(&self) -> f32 {
        (self.hover_time / animations::BUTTON_HOVER_DURATION.as_secs_f32()).min(1.0)
    }

    fn update(&mut self, delta_time: f32) {
        if self.hovered {
            self.hover_time =
                (self.hover_time + delta_time).min(animations::BUTTON_HOVER_DURATION.as_secs_f32());
        }
        self.click_spring.update(delta_time);
        self.pressed = self.click_spring.is_pressed();
    }

    fn trigger_click(&mut self) {
        self.click_spring.trigger();
        self.pressed = true;
    }

    fn click_scale(&self) -> f32 {
        self.click_spring.scale()
    }
}

/// Main menu implementation
pub struct MainMenu {
    /// Current menu state
    state: MainMenuState,
    /// Menu buttons for current state
    buttons: Vec<MenuButton>,
    /// Currently hovered button
    hovered_button: Option<ButtonId>,
    /// Game version string
    version_text: String,
    /// Background animation time
    background_time: f32,
    /// Menu animation progress
    animation_progress: f32,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Logo animation time
    logo_pulse_time: f32,
    pending_events: Vec<UIEvent>,
    keyboard_focus: Option<usize>,
}

impl Default for MainMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MainMenu {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    /// Create new main menu
    pub fn new() -> Self {
        Self {
            state: MainMenuState::Main,
            buttons: Vec::new(),
            hovered_button: None,
            version_text: localization::localize(
                "menu.version",
                "Command & Conquer Generals Zero Hour v1.04",
            ),
            background_time: 0.0,
            animation_progress: 0.0,
            screen_size: (1024, 768),
            logo_pulse_time: 0.0,
            pending_events: Vec::new(),
            keyboard_focus: None,
        }
    }

    pub fn drain_pending_events(&mut self) -> Vec<UIEvent> {
        std::mem::take(&mut self.pending_events)
    }

    fn move_keyboard_focus(&mut self, delta: i32) -> bool {
        if self.buttons.is_empty() {
            return false;
        }

        let enabled_indices: Vec<usize> = self
            .buttons
            .iter()
            .enumerate()
            .filter_map(|(idx, b)| if b.enabled { Some(idx) } else { None })
            .collect();
        if enabled_indices.is_empty() {
            return false;
        }

        let current = self.keyboard_focus.unwrap_or(enabled_indices[0]);
        let cur_pos = enabled_indices
            .iter()
            .position(|&idx| idx == current)
            .unwrap_or(0) as i32;
        let len = enabled_indices.len() as i32;
        let next_pos = (cur_pos + delta).rem_euclid(len);
        let next_idx = enabled_indices[next_pos as usize];

        self.keyboard_focus = Some(next_idx);

        for (idx, button) in self.buttons.iter_mut().enumerate() {
            let is_hovered = idx == next_idx;
            if is_hovered != button.hovered {
                button.set_hovered(is_hovered);
                if is_hovered {
                    self.pending_events.push(UIEvent::PlaySoundEffectPath(
                        sound_files::BUTTON_HOVER.to_string(),
                    ));
                }
            }
            if is_hovered {
                self.hovered_button = Some(button.id);
            }
        }

        true
    }

    /// Initialize the main menu
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.setup_main_menu_buttons();
        Ok(())
    }

    /// Update main menu
    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        self.background_time += delta_time;
        self.logo_pulse_time += delta_time * 2.0;

        // Update animation progress
        if self.animation_progress < 1.0 {
            self.animation_progress += delta_time * 2.0;
            self.animation_progress = self.animation_progress.min(1.0);
        }

        for button in &mut self.buttons {
            button.update(delta_time);
        }

        Ok(())
    }

    /// Handle mouse clicks and return UI events
    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> Option<UIEvent> {
        if button != MouseButton::Left {
            return None;
        }

        // Find clicked button
        let mut clicked_button_id = None;
        self.keyboard_focus = None;
        for menu_button in &mut self.buttons {
            if menu_button.contains_point(x, y) && menu_button.enabled {
                menu_button.trigger_click();
                clicked_button_id = Some(menu_button.id);
                break;
            }
        }

        // Handle button click if any
        if let Some(button_id) = clicked_button_id {
            self.pending_events.push(UIEvent::PlaySoundEffectPath(
                sound_files::BUTTON_CLICK.to_string(),
            ));
            return self.handle_button_click(button_id);
        }

        None
    }

    /// Handle button click and return appropriate event
    fn handle_button_click(&mut self, button_id: ButtonId) -> Option<UIEvent> {
        match button_id {
            ButtonId::NewGame => {
                self.state = MainMenuState::SinglePlayer;
                self.setup_single_player_buttons();
                None
            }
            ButtonId::LoadGame => Some(UIEvent::ChangeScreen(Screen::LoadGame)),
            ButtonId::Multiplayer => {
                self.state = MainMenuState::Multiplayer;
                self.setup_multiplayer_buttons();
                None
            }
            ButtonId::Options => Some(UIEvent::ChangeScreen(Screen::Options)),
            ButtonId::Credits => Some(UIEvent::ChangeScreen(Screen::Credits)),
            ButtonId::Exit => Some(UIEvent::ExitGame),

            // Single player buttons
            ButtonId::Campaign => Some(UIEvent::ChangeScreen(Screen::Campaign)),
            ButtonId::Skirmish => Some(UIEvent::ChangeScreen(Screen::Skirmish)),
            ButtonId::Challenges => Some(UIEvent::ChangeScreen(Screen::Campaign)),

            // Multiplayer buttons
            ButtonId::Internet => Some(UIEvent::StartGame {
                mode: GameMode::Internet,
                faction: "USA".to_string(),
                map: "multiplayer_01".to_string(),
            }),
            ButtonId::Network => Some(UIEvent::StartGame {
                mode: GameMode::Lan,
                faction: "USA".to_string(),
                map: "multiplayer_01".to_string(),
            }),
            ButtonId::Direct => Some(UIEvent::StartGame {
                mode: GameMode::Lan,
                faction: "USA".to_string(),
                map: "multiplayer_01".to_string(),
            }),

            // Options buttons - handled by options menu itself
            ButtonId::Audio | ButtonId::Video | ButtonId::Controls | ButtonId::Gameplay => {
                // These are handled within the options menu
                None
            }

            ButtonId::Back => {
                self.state = MainMenuState::Main;
                self.setup_main_menu_buttons();
                None
            }
        }
    }

    /// Resize menu for new screen dimensions
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);

        // Recalculate button positions
        match self.state {
            MainMenuState::Main => self.setup_main_menu_buttons(),
            MainMenuState::SinglePlayer => self.setup_single_player_buttons(),
            MainMenuState::Multiplayer => self.setup_multiplayer_buttons(),
            MainMenuState::Options => self.setup_options_buttons(),
            MainMenuState::Credits => {}
        }
    }

    // Private methods for button setup

    fn setup_main_menu_buttons(&mut self) {
        self.buttons.clear();

        let center_x = (self.screen_size.0 as i32 / 2) - (layout::MENU_BUTTON_WIDTH as i32 / 2);
        let start_y = (self.screen_size.1 as i32 / 2) - 100;
        let spacing = (layout::MENU_BUTTON_HEIGHT + layout::MENU_SPACING) as i32;

        self.buttons.push(MenuButton::new(
            ButtonId::NewGame,
            Self::text("menu.new_game", "New Game"),
            center_x,
            start_y,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::LoadGame,
            Self::text("menu.load_game", "Load Game"),
            center_x,
            start_y + spacing,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Multiplayer,
            Self::text("menu.multiplayer", "Multiplayer"),
            center_x,
            start_y + spacing * 2,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Options,
            Self::text("menu.options", "Options"),
            center_x,
            start_y + spacing * 3,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Credits,
            Self::text("menu.credits", "Credits"),
            center_x,
            start_y + spacing * 4,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Exit,
            Self::text("menu.exit", "Exit"),
            center_x,
            start_y + spacing * 5,
        ));
    }

    fn setup_single_player_buttons(&mut self) {
        self.buttons.clear();

        let center_x = (self.screen_size.0 as i32 / 2) - (layout::MENU_BUTTON_WIDTH as i32 / 2);
        let start_y = (self.screen_size.1 as i32 / 2) - 50;
        let spacing = (layout::MENU_BUTTON_HEIGHT + layout::MENU_SPACING) as i32;

        self.buttons.push(MenuButton::new(
            ButtonId::Campaign,
            Self::text("menu.campaign", "Campaign"),
            center_x,
            start_y,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Skirmish,
            Self::text("menu.skirmish", "Skirmish"),
            center_x,
            start_y + spacing,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Challenges,
            Self::text("menu.challenges", "Challenges"),
            center_x,
            start_y + spacing * 2,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Back,
            Self::text("menu.back", "Back"),
            center_x,
            start_y + spacing * 4,
        ));
    }

    fn setup_multiplayer_buttons(&mut self) {
        self.buttons.clear();

        let center_x = (self.screen_size.0 as i32 / 2) - (layout::MENU_BUTTON_WIDTH as i32 / 2);
        let start_y = (self.screen_size.1 as i32 / 2) - 50;
        let spacing = (layout::MENU_BUTTON_HEIGHT + layout::MENU_SPACING) as i32;

        self.buttons.push(MenuButton::new(
            ButtonId::Internet,
            Self::text("menu.internet_game", "Internet Game"),
            center_x,
            start_y,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Network,
            Self::text("menu.network_game", "Network Game"),
            center_x,
            start_y + spacing,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Direct,
            Self::text("menu.direct_connect", "Direct Connect"),
            center_x,
            start_y + spacing * 2,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Back,
            Self::text("menu.back", "Back"),
            center_x,
            start_y + spacing * 4,
        ));
    }

    fn setup_options_buttons(&mut self) {
        self.buttons.clear();

        let center_x = (self.screen_size.0 as i32 / 2) - (layout::MENU_BUTTON_WIDTH as i32 / 2);
        let start_y = (self.screen_size.1 as i32 / 2) - 75;
        let spacing = (layout::MENU_BUTTON_HEIGHT + layout::MENU_SPACING) as i32;

        self.buttons.push(MenuButton::new(
            ButtonId::Audio,
            Self::text("menu.audio_options", "Audio Options"),
            center_x,
            start_y,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Video,
            Self::text("menu.video_options", "Video Options"),
            center_x,
            start_y + spacing,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Controls,
            Self::text("menu.control_options", "Control Options"),
            center_x,
            start_y + spacing * 2,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Gameplay,
            Self::text("menu.gameplay_options", "Gameplay Options"),
            center_x,
            start_y + spacing * 3,
        ));
        self.buttons.push(MenuButton::new(
            ButtonId::Back,
            Self::text("menu.back", "Back"),
            center_x,
            start_y + spacing * 5,
        ));
    }

    /// Get menu title for current state
    fn get_menu_title(&self) -> String {
        match self.state {
            MainMenuState::Main => Self::text("menu.title.main", "Main Menu"),
            MainMenuState::SinglePlayer => Self::text("menu.title.single_player", "Single Player"),
            MainMenuState::Multiplayer => Self::text("menu.title.multiplayer", "Multiplayer"),
            MainMenuState::Options => Self::text("menu.title.options", "Options"),
            MainMenuState::Credits => Self::text("menu.title.credits", "Credits"),
        }
    }
}

impl Interactive for MainMenu {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let mut handled = false;
        self.hovered_button = None;
        self.keyboard_focus = None;

        for button in &mut self.buttons {
            let was_hovered = button.hovered;
            let is_hovered = button.contains_point(x, y) && button.enabled;

            if is_hovered {
                self.hovered_button = Some(button.id);
                handled = true;
            }

            if is_hovered != was_hovered {
                button.set_hovered(is_hovered);
                if is_hovered {
                    self.pending_events.push(UIEvent::PlaySoundEffectPath(
                        sound_files::BUTTON_HOVER.to_string(),
                    ));
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
            KeyCode::Escape => {
                if self.state != MainMenuState::Main {
                    self.state = MainMenuState::Main;
                    self.setup_main_menu_buttons();
                    self.keyboard_focus = None;
                    true
                } else {
                    false
                }
            }
            KeyCode::Enter => {
                // Activate hovered button
                if let Some(button_id) = self.hovered_button {
                    self.pending_events.push(UIEvent::PlaySoundEffectPath(
                        sound_files::BUTTON_CLICK.to_string(),
                    ));
                    if let Some(event) = self.handle_button_click(button_id) {
                        self.pending_events.push(event);
                    }
                    return true;
                }
                false
            }
            KeyCode::Up => self.move_keyboard_focus(-1),
            KeyCode::Down => self.move_keyboard_focus(1),
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false // Main menu doesn't handle text input
    }
}

impl Renderable for MainMenu {
    fn render(&self, context: &mut UIRenderContext) {
        // Menu drawing currently routes through the shared UI render context logging path.

        // For now, just print what would be rendered
        let title = self.get_menu_title();
        let header = localization::localize_with_args(
            "menu.log.header",
            "=== {title} ===",
            &[("title", title.as_str())],
        );
        trace!("{header}");

        // Render background
        self.render_background(context);

        // Render logo
        self.render_logo(context);

        // Render menu buttons
        for button in &self.buttons {
            self.render_button(button, context);
        }

        // Render version text
        self.render_version_text(context);

        // Render credits if in credits mode
        if self.state == MainMenuState::Credits {
            self.render_credits(context);
        }
    }

    fn get_bounds(&self) -> (i32, i32, u32, u32) {
        (0, 0, self.screen_size.0, self.screen_size.1)
    }

    fn is_visible(&self) -> bool {
        true
    }
}

impl MainMenu {
    fn render_background(&self, _context: &mut UIRenderContext) {
        // Keep deterministic backdrop timing until particle background pass is wired.
        let time_str = format!("{:.2}", self.background_time);
        let background_log = localization::localize_with_args(
            "menu.log.render_background",
            "Rendering menu background (time: {time})",
            &[("time", time_str.as_str())],
        );
        trace!("{background_log}");
    }

    fn render_logo(&self, _context: &mut UIRenderContext) {
        let pulse_scale = 1.0 + (self.logo_pulse_time.sin() * 0.05);
        let scale_str = format!("{:.2}", pulse_scale);
        let logo_log = localization::localize_with_args(
            "menu.log.render_logo",
            "Rendering logo (pulse scale: {scale})",
            &[("scale", scale_str.as_str())],
        );
        trace!("{logo_log}");
    }

    fn render_button(&self, button: &MenuButton, _context: &mut UIRenderContext) {
        let pressed_label = localization::localize("menu.log.button_state_pressed", "pressed");
        let hovered_label = localization::localize("menu.log.button_state_hovered", "hovered");
        let normal_label = localization::localize("menu.log.button_state_normal", "normal");
        let disabled_label = localization::localize("menu.log.button_state_disabled", "disabled");

        let state = if button.pressed {
            pressed_label.as_str()
        } else if button.hovered {
            hovered_label.as_str()
        } else if button.enabled {
            normal_label.as_str()
        } else {
            disabled_label.as_str()
        };

        let scale = button.click_scale();
        let (x, y, _, _) = utils::scale_rect_center(
            (
                button.position.0,
                button.position.1,
                button.size.0,
                button.size.1,
            ),
            scale,
        );
        let x_str = format!("{:.1}", x);
        let y_str = format!("{:.1}", y);
        let button_log = localization::localize_with_args(
            "menu.log.button_render",
            "Button '{text}' at ({x}, {y}) - state: {state}",
            &[
                ("text", button.text.as_str()),
                ("x", x_str.as_str()),
                ("y", y_str.as_str()),
                ("state", state),
            ],
        );
        trace!("{button_log}");

        if button.hovered {
            let hover_progress = button.get_hover_progress();
            let hover_text = localization::localize_with_args(
                "menu.log.hover_animation",
                "  Hover animation: {progress}",
                &[("progress", format!("{:.2}", hover_progress).as_str())],
            );
            trace!("{hover_text}");
        }
    }

    fn render_version_text(&self, _context: &mut UIRenderContext) {
        let version_x = 10;
        let version_y = self.screen_size.1 as i32 - 30;
        let version_log = localization::localize_with_args(
            "menu.log.version_text",
            "Version text '{text}' at ({x}, {y})",
            &[
                ("text", self.version_text.as_str()),
                ("x", version_x.to_string().as_str()),
                ("y", version_y.to_string().as_str()),
            ],
        );
        trace!("{version_log}");
    }

    fn render_credits(&self, _context: &mut UIRenderContext) {
        trace!(
            "{}",
            localization::localize("menu.credits.header", "=== CREDITS ===")
        );
        trace!(
            "{}",
            localization::localize(
                "menu.credits.original",
                "Command & Conquer Generals Zero Hour"
            )
        );
        trace!(
            "{}",
            localization::localize("menu.credits.developer", "Originally by EA Los Angeles")
        );
        trace!(
            "{}",
            localization::localize("menu.credits.rust_port", "Rust conversion by the community")
        );
        trace!(
            "{}",
            localization::localize("menu.credits.footer", "===============")
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_menu_creation() {
        let menu = MainMenu::new();
        assert_eq!(menu.state, MainMenuState::Main);
        assert_eq!(menu.buttons.len(), 0); // Buttons created during initialize
    }

    #[test]
    fn test_button_creation() {
        let button = MenuButton::new(ButtonId::NewGame, "New Game".to_string(), 100, 200);
        assert_eq!(button.text, "New Game");
        assert_eq!(button.position, (100, 200));
        assert!(button.enabled);
        assert!(!button.hovered);
    }

    #[test]
    fn test_button_collision() {
        let button = MenuButton::new(ButtonId::NewGame, "Test".to_string(), 100, 200);
        assert!(button.contains_point(150, 220));
        assert!(!button.contains_point(50, 220));
        assert!(!button.contains_point(150, 150));
    }

    #[test]
    fn test_menu_navigation() {
        let mut menu = MainMenu::new();
        menu.initialize().unwrap();

        // Should start in main menu
        assert_eq!(menu.state, MainMenuState::Main);

        // Simulate clicking "New Game" button
        if let Some(_) = menu.handle_button_click(ButtonId::NewGame) {
            // Button click should change state
        }
        assert_eq!(menu.state, MainMenuState::SinglePlayer);

        // Back button should return to main
        menu.handle_button_click(ButtonId::Back);
        assert_eq!(menu.state, MainMenuState::Main);
    }
}
