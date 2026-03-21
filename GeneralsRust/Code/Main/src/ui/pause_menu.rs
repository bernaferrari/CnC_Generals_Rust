//! Pause Menu System
//!
//! This module implements the pause menu that appears when the game is paused,
//! providing options to resume, adjust settings, save/load, or exit to main menu.

use super::{
    layout, sound_files, utils, ClickSpring, Interactive, KeyCode, MouseButton, Renderable, Screen,
    UIEvent, UIRenderContext,
};
use crate::game_logic::GameMode;
use crate::localization;
use log::info;
use std::time::Duration;

/// Actions that can be taken from pause menu
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PauseMenuAction {
    Resume,
    SaveGame,
    LoadGame,
    Options,
    RestartMission,
    ExitToMenu,
    ExitGame,
}

/// Pause menu button
struct PauseButton {
    action: PauseMenuAction,
    text: String,
    position: (i32, i32),
    size: (u32, u32),
    hovered: bool,
    enabled: bool,
    hover_time: f32,
    click_spring: ClickSpring,
}

impl PauseButton {
    fn new(action: PauseMenuAction, text: String, x: i32, y: i32) -> Self {
        Self {
            action,
            text,
            position: (x, y),
            size: (layout::MENU_BUTTON_WIDTH, layout::MENU_BUTTON_HEIGHT),
            hovered: false,
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
        }
        self.hovered = hovered;
    }

    fn update(&mut self, delta_time: f32) {
        if self.hovered {
            self.hover_time += delta_time;
        }
        self.click_spring.update(delta_time);
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn trigger_click(&mut self) {
        self.click_spring.trigger();
    }

    fn click_scale(&self) -> f32 {
        self.click_spring.scale()
    }
}

/// Pause menu implementation
pub struct PauseMenu {
    /// Menu buttons
    buttons: Vec<PauseButton>,
    /// Currently hovered button
    hovered_action: Option<PauseMenuAction>,
    /// Screen dimensions
    screen_size: (u32, u32),
    /// Menu background overlay alpha
    overlay_alpha: f32,
    /// Animation progress
    animation_progress: f32,
    /// Game mode (affects available options)
    game_mode: GameMode,
    /// Current game time when paused
    game_time_when_paused: Duration,
    /// Mission name (if in campaign)
    mission_name: Option<String>,
    pending_events: Vec<UIEvent>,
}

impl Default for PauseMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl PauseMenu {
    fn text(key: &str, fallback: &str) -> String {
        localization::localize(key, fallback)
    }

    fn tooltip(&self, action: PauseMenuAction) -> String {
        match action {
            PauseMenuAction::Resume => {
                Self::text("pause.tooltip.resume", "Return to the battlefield")
            }
            PauseMenuAction::SaveGame => {
                Self::text("pause.tooltip.save", "Save your current progress")
            }
            PauseMenuAction::LoadGame => {
                Self::text("pause.tooltip.load", "Load a previously saved game")
            }
            PauseMenuAction::Options => Self::text("pause.tooltip.options", "Adjust game settings"),
            PauseMenuAction::RestartMission => {
                Self::text("pause.tooltip.restart", "Start the mission over")
            }
            PauseMenuAction::ExitToMenu => {
                Self::text("pause.tooltip.exit_menu", "Return to main menu")
            }
            PauseMenuAction::ExitGame => {
                Self::text("pause.tooltip.exit_game", "Quit Command & Conquer Generals")
            }
        }
    }

    /// Create new pause menu
    pub fn new() -> Self {
        Self {
            buttons: Vec::new(),
            hovered_action: None,
            screen_size: (1024, 768),
            overlay_alpha: 0.0,
            animation_progress: 0.0,
            game_mode: GameMode::Skirmish,
            game_time_when_paused: Duration::from_secs(0),
            mission_name: None,
            pending_events: Vec::new(),
        }
    }

    pub fn drain_pending_events(&mut self) -> Vec<UIEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Initialize pause menu
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.setup_buttons();
        Ok(())
    }

    /// Update pause menu
    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        // Update animation
        if self.animation_progress < 1.0 {
            self.animation_progress += delta_time * 4.0; // Fast animation
            self.animation_progress = self.animation_progress.min(1.0);
        }

        // Update overlay alpha
        let target_alpha = 0.7;
        if self.overlay_alpha < target_alpha {
            self.overlay_alpha += delta_time * 3.0;
            self.overlay_alpha = self.overlay_alpha.min(target_alpha);
        }

        // Update buttons
        for button in &mut self.buttons {
            button.update(delta_time);
        }

        Ok(())
    }

    /// Handle mouse clicks
    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> Option<UIEvent> {
        if button != MouseButton::Left {
            return None;
        }

        // Find clicked button
        let mut clicked_action = None;
        for menu_button in &mut self.buttons {
            if menu_button.contains_point(x, y) && menu_button.enabled {
                menu_button.trigger_click();
                clicked_action = Some(menu_button.action);
                break;
            }
        }

        if let Some(action) = clicked_action {
            self.pending_events.push(UIEvent::PlaySoundEffectPath(
                sound_files::BUTTON_CLICK.to_string(),
            ));
            return self.handle_button_action(action);
        }

        None
    }

    /// Handle button actions
    fn handle_button_action(&mut self, action: PauseMenuAction) -> Option<UIEvent> {
        match action {
            PauseMenuAction::Resume => Some(UIEvent::TogglePause),
            PauseMenuAction::SaveGame => Some(UIEvent::ChangeScreen(Screen::SaveGame)),
            PauseMenuAction::LoadGame => Some(UIEvent::ChangeScreen(Screen::LoadGame)),
            PauseMenuAction::Options => Some(UIEvent::ShowOptions),
            PauseMenuAction::RestartMission => Some(UIEvent::RestartMission),
            PauseMenuAction::ExitToMenu => Some(UIEvent::ExitToMenu),
            PauseMenuAction::ExitGame => Some(UIEvent::ExitGame),
        }
    }

    /// Set game context for pause menu
    pub fn set_game_context(
        &mut self,
        mode: GameMode,
        mission_name: Option<String>,
        game_time: Duration,
    ) {
        self.game_mode = mode;
        self.mission_name = mission_name;
        self.game_time_when_paused = game_time;
        self.setup_buttons(); // Refresh buttons based on context
    }

    /// Resize menu for new screen dimensions
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);
        self.setup_buttons();
    }

    /// Reset menu animation
    pub fn reset_animation(&mut self) {
        self.animation_progress = 0.0;
        self.overlay_alpha = 0.0;
    }

    // Private methods

    fn setup_buttons(&mut self) {
        self.buttons.clear();

        let center_x = (self.screen_size.0 as i32 / 2) - (layout::MENU_BUTTON_WIDTH as i32 / 2);
        let start_y = (self.screen_size.1 as i32 / 2) - 150;
        let spacing = (layout::MENU_BUTTON_HEIGHT + layout::MENU_SPACING) as i32;

        // Always available options
        self.buttons.push(PauseButton::new(
            PauseMenuAction::Resume,
            Self::text("pause.resume", "Resume Game"),
            center_x,
            start_y,
        ));

        // Save/Load available in single player modes
        if matches!(self.game_mode, GameMode::SinglePlayer | GameMode::Skirmish) {
            self.buttons.push(PauseButton::new(
                PauseMenuAction::SaveGame,
                Self::text("pause.save", "Save Game"),
                center_x,
                start_y + spacing,
            ));

            self.buttons.push(PauseButton::new(
                PauseMenuAction::LoadGame,
                Self::text("pause.load", "Load Game"),
                center_x,
                start_y + spacing * 2,
            ));
        }

        // Options always available
        let options_y = if matches!(self.game_mode, GameMode::SinglePlayer | GameMode::Skirmish) {
            start_y + spacing * 3
        } else {
            start_y + spacing
        };

        self.buttons.push(PauseButton::new(
            PauseMenuAction::Options,
            Self::text("pause.options", "Options"),
            center_x,
            options_y,
        ));

        // Restart mission in campaign mode
        if self.game_mode == GameMode::SinglePlayer && self.mission_name.is_some() {
            self.buttons.push(PauseButton::new(
                PauseMenuAction::RestartMission,
                Self::text("pause.restart", "Restart Mission"),
                center_x,
                options_y + spacing,
            ));
        }

        // Exit options
        let exit_menu_y = options_y + spacing * 2;
        let exit_game_y = exit_menu_y + spacing;

        self.buttons.push(PauseButton::new(
            PauseMenuAction::ExitToMenu,
            Self::text("pause.exit_menu", "Exit to Menu"),
            center_x,
            exit_menu_y,
        ));

        self.buttons.push(PauseButton::new(
            PauseMenuAction::ExitGame,
            Self::text("pause.exit_game", "Exit Game"),
            center_x,
            exit_game_y,
        ));
    }

    fn format_game_time(&self) -> String {
        let total_seconds = self.game_time_when_paused.as_secs();
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if hours > 0 {
            format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
        } else {
            format!("{:02}:{:02}", minutes, seconds)
        }
    }

    fn get_pause_title(&self) -> String {
        match self.game_mode {
            GameMode::SinglePlayer => Self::text("pause.title_campaign", "Campaign Paused"),
            GameMode::Skirmish => Self::text("pause.title_skirmish", "Skirmish Paused"),
            GameMode::Lan => Self::text("pause.title_lan", "LAN Game Paused"),
            GameMode::Internet => Self::text("pause.title_online", "Online Game Paused"),
            _ => Self::text("pause.title_default", "Game Paused"),
        }
    }
}

impl Interactive for PauseMenu {
    fn handle_mouse_move(&mut self, x: i32, y: i32) -> bool {
        let mut handled = false;
        self.hovered_action = None;

        for button in &mut self.buttons {
            let was_hovered = button.hovered;
            let is_hovered = button.contains_point(x, y) && button.enabled;

            if is_hovered {
                self.hovered_action = Some(button.action);
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
                // Resume game on Escape
                true // This will be handled by UI manager
            }
            KeyCode::Enter => {
                // Activate hovered button
                if let Some(action) = self.hovered_action {
                    self.pending_events.push(UIEvent::PlaySoundEffectPath(
                        sound_files::BUTTON_CLICK.to_string(),
                    ));
                    if let Some(event) = self.handle_button_action(action) {
                        self.pending_events.push(event);
                    }
                    return true;
                }
                false
            }
            KeyCode::F5 => {
                // Quick save
                if matches!(self.game_mode, GameMode::SinglePlayer | GameMode::Skirmish) {
                    info!(
                        "{}",
                        Self::text("pause.log.quick_save", "Quick save triggered")
                    );
                    self.pending_events.push(UIEvent::PlaySoundEffectPath(
                        sound_files::BUTTON_CLICK.to_string(),
                    ));
                    self.pending_events.push(UIEvent::SaveGame {
                        slot: "quicksave".to_string(),
                        display_name: Self::text("pause.quick_save_name", "Quick Save"),
                    });
                    return true;
                }
                info!(
                    "{}",
                    Self::text(
                        "pause.log.quick_save_unavailable",
                        "Quick save is only available in single-player or skirmish"
                    )
                );
                false
            }
            KeyCode::F9 => {
                // Quick load
                if matches!(self.game_mode, GameMode::SinglePlayer | GameMode::Skirmish) {
                    info!(
                        "{}",
                        Self::text("pause.log.quick_load", "Quick load triggered")
                    );
                    self.pending_events.push(UIEvent::PlaySoundEffectPath(
                        sound_files::BUTTON_CLICK.to_string(),
                    ));
                    self.pending_events
                        .push(UIEvent::LoadGame("quicksave".to_string()));
                    return true;
                }
                info!(
                    "{}",
                    Self::text(
                        "pause.log.quick_load_unavailable",
                        "Quick load is only available in single-player or skirmish"
                    )
                );
                false
            }
            _ => false,
        }
    }

    fn handle_text_input(&mut self, _text: &str) -> bool {
        false
    }
}

impl Renderable for PauseMenu {
    fn render(&self, _context: &mut UIRenderContext) {
        let title = self.get_pause_title();
        let header = localization::localize_with_args(
            "pause.log.header",
            "=== {title} ===",
            &[("title", title.as_str())],
        );
        println!("{header}");

        // Render background overlay
        let overlay_alpha = format!("{:.2}", self.overlay_alpha);
        let overlay_text = localization::localize_with_args(
            "pause.log.overlay_alpha",
            "Background overlay (alpha: {alpha})",
            &[("alpha", overlay_alpha.as_str())],
        );
        println!("{overlay_text}");

        // Render menu title and info
        if let Some(mission) = &self.mission_name {
            println!(
                "{} {}",
                Self::text("pause.mission_label", "Mission:"),
                mission
            );
        }
        println!(
            "{} {}",
            Self::text("pause.game_time", "Game Time:"),
            self.format_game_time()
        );

        // Render menu buttons with animation
        let slide_offset = ((1.0 - self.animation_progress) * 100.0) as i32;
        let disabled_label =
            localization::localize("hud.panel.button_state_disabled", "[DISABLED]");
        let hovered_label = localization::localize("hud.panel.button_state_hovered", "[HOVERED]");

        for button in &self.buttons {
            let animated_x = button.position.0 + slide_offset;
            let state = if !button.enabled {
                disabled_label.as_str()
            } else if button.hovered {
                hovered_label.as_str()
            } else {
                ""
            };

            let scale = button.click_scale();
            let (x, y, _, _) = utils::scale_rect_center(
                (animated_x, button.position.1, button.size.0, button.size.1),
                scale,
            );
            let x_str = format!("{:.1}", x);
            let y_str = format!("{:.1}", y);
            let button_line = localization::localize_with_args(
                "pause.log.button_line",
                "Button: '{text}' at ({x}, {y}) {state}",
                &[
                    ("text", button.text.as_str()),
                    ("x", x_str.as_str()),
                    ("y", y_str.as_str()),
                    ("state", state),
                ],
            );
            println!("{button_line}");

            if button.hovered {
                let hover_seconds = format!("{:.2}", button.hover_time);
                let hover_text = localization::localize_with_args(
                    "pause.log.hover_effect",
                    "  Hover effect: {seconds}s",
                    &[("seconds", hover_seconds.as_str())],
                );
                println!("{hover_text}");
                println!("  {}", self.tooltip(button.action));
            }
        }

        // Render animation progress
        if self.animation_progress < 1.0 {
            let percent = format!("{:.1}", self.animation_progress * 100.0);
            let animation_text = localization::localize_with_args(
                "pause.log.menu_animation",
                "Menu animation: {percent}%",
                &[("percent", percent.as_str())],
            );
            println!("{animation_text}");
        }

        // Render helpful tips
        println!("\n{}", Self::text("pause.controls_title", "Controls:"));
        println!(
            "{}",
            Self::text("pause.controls_resume", "ESC - Resume game")
        );
        if matches!(self.game_mode, GameMode::SinglePlayer | GameMode::Skirmish) {
            println!(
                "{}",
                Self::text("pause.controls_quick_save", "F5 - Quick save")
            );
            println!(
                "{}",
                Self::text("pause.controls_quick_load", "F9 - Quick load")
            );
        }

        // Show game mode specific info
        match self.game_mode {
            GameMode::Internet | GameMode::Lan => {
                println!(
                    "\n{}",
                    Self::text(
                        "pause.warning_multiplayer",
                        "⚠️  Game will continue for other players while paused"
                    )
                );
            }
            _ => {}
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
    fn test_pause_menu_creation() {
        let menu = PauseMenu::new();
        assert_eq!(menu.hovered_action, None);
        assert_eq!(menu.animation_progress, 0.0);
        assert_eq!(menu.overlay_alpha, 0.0);
    }

    #[test]
    fn test_button_creation() {
        let button = PauseButton::new(PauseMenuAction::Resume, "Resume".to_string(), 100, 200);
        assert_eq!(button.text, "Resume");
        assert_eq!(button.position, (100, 200));
        assert!(button.enabled);
        assert!(!button.hovered);
    }

    #[test]
    fn test_button_collision() {
        let button = PauseButton::new(PauseMenuAction::Resume, "Test".to_string(), 100, 200);
        assert!(button.contains_point(150, 220));
        assert!(!button.contains_point(50, 220));
    }

    #[test]
    fn test_game_context() {
        let mut menu = PauseMenu::new();
        menu.set_game_context(
            GameMode::SinglePlayer,
            Some("GLA Mission 01".to_string()),
            Duration::from_secs(300),
        );

        assert_eq!(menu.game_mode, GameMode::SinglePlayer);
        assert_eq!(menu.mission_name, Some("GLA Mission 01".to_string()));
        assert_eq!(menu.game_time_when_paused, Duration::from_secs(300));
    }

    #[test]
    fn test_time_formatting() {
        let mut menu = PauseMenu::new();

        // Test minutes and seconds
        menu.game_time_when_paused = Duration::from_secs(125);
        assert_eq!(menu.format_game_time(), "02:05");

        // Test hours, minutes, and seconds
        menu.game_time_when_paused = Duration::from_secs(3661);
        assert_eq!(menu.format_game_time(), "01:01:01");
    }

    #[test]
    fn test_button_actions() {
        let mut menu = PauseMenu::new();

        // Test resume action
        let event = menu.handle_button_action(PauseMenuAction::Resume);
        assert!(matches!(event, Some(UIEvent::TogglePause)));

        // Test exit to menu action
        let event = menu.handle_button_action(PauseMenuAction::ExitToMenu);
        assert!(matches!(event, Some(UIEvent::ExitToMenu)));
    }
}
