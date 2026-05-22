//! UI Manager - Central coordinator for all UI systems
//!
//! This module manages the overall UI state, screen transitions, and coordinates
//! between different UI components like menus, HUD, and dialogs.

use super::{
    animations, sound_files, FactionSelectionScreen, FontManager, GameHUD, Interactive, KeyCode,
    MainMenu, MouseButton, PauseMenu, Renderable, SaveLoadMenu, SaveLoadMode, Screen, SkirmishMenu,
    TextureManager, UIRenderContext, VictoryScreen,
};
use crate::{
    game_logic::{victory::VictorySummary, GameMode},
    localization,
    subsystem_manager::initialize_shell_ui_schemes,
};
use log::{debug, info, trace};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Current UI state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UIState {
    /// Loading initial assets
    Loading,
    /// Showing title screen
    Title,
    /// Main menu navigation
    MainMenu,
    /// Selecting faction for game
    FactionSelection,
    /// Setting up game options
    GameSetup,
    /// In-game with HUD active
    InGame,
    /// Game is paused
    Paused,
    /// Victory/defeat screen
    GameEnd,
    /// Disconnected from network game
    Disconnected,
}

/// UI events that can be triggered
#[derive(Debug, Clone)]
pub enum UIEvent {
    /// Screen transition requested
    ChangeScreen(Screen),
    /// Start new game with specified parameters
    StartGame {
        mode: GameMode,
        faction: String,
        map: String,
    },
    /// Load existing game
    LoadGame(String),
    /// Save the current game into a slot (filename stem).
    SaveGame { slot: String, display_name: String },
    /// Restart the current mission / map from scratch.
    RestartMission,
    /// Return to main menu
    ExitToMenu,
    /// Exit the application
    ExitGame,
    /// Pause/unpause game
    TogglePause,
    /// Show options menu
    ShowOptions,
    /// Audio/visual settings changed
    SettingsChanged,
    /// Request to play a UI sound effect from a concrete archive path (e.g. "Audio/Interface/ButtonClick.wav").
    PlaySoundEffectPath(String),
    /// Focus the tactical camera to a world position
    FocusCamera(glam::Vec3),
}

/// Main UI manager that coordinates all UI systems
pub struct UIManager {
    /// Current UI state
    current_state: UIState,
    /// Current screen being displayed
    current_screen: Option<Screen>,
    /// Screen transition in progress
    transitioning: bool,
    /// Transition elapsed time in seconds
    transition_elapsed: f32,
    /// Transition duration
    transition_duration: Duration,

    /// Main menu system
    main_menu: MainMenu,
    /// Faction selection screen
    faction_selection: FactionSelectionScreen,
    /// In-game HUD
    game_hud: GameHUD,
    /// Pause menu
    pause_menu: PauseMenu,
    /// Victory screen
    victory_screen: VictoryScreen,
    /// Skirmish setup screen
    skirmish_menu: SkirmishMenu,
    /// Save/load menu
    save_load_menu: SaveLoadMenu,

    /// Font manager
    font_manager: Arc<Mutex<FontManager>>,
    /// Texture manager
    texture_manager: Arc<Mutex<TextureManager>>,

    /// Event queue
    event_queue: Vec<UIEvent>,
    /// Input state
    mouse_position: (i32, i32),
    /// Keys currently pressed
    keys_pressed: HashMap<KeyCode, bool>,

    /// Screen size
    screen_size: (u32, u32),
    /// UI scale factor
    ui_scale: f32,
    /// Debug mode
    debug_mode: bool,
    /// Quick-start preserves the legacy startup flow while suppressing shell-map animation.
    quick_start_enabled: bool,

    /// Most recent delta time passed to `update` (used by render-time animations).
    last_delta_time: f32,
}

impl UIManager {
    /// Create new UI manager
    pub fn new(screen_width: u32, screen_height: u32) -> Self {
        let font_manager = Arc::new(Mutex::new(FontManager::new()));
        let texture_manager = Arc::new(Mutex::new(TextureManager::new()));

        Self {
            current_state: UIState::Loading,
            current_screen: None,
            transitioning: false,
            transition_elapsed: 0.0,
            transition_duration: animations::MENU_TRANSITION_DURATION,

            main_menu: MainMenu::new(),
            faction_selection: FactionSelectionScreen::new(),
            game_hud: GameHUD::new(),
            pause_menu: PauseMenu::new(),
            victory_screen: VictoryScreen::new(),
            skirmish_menu: SkirmishMenu::new(),
            save_load_menu: SaveLoadMenu::new(SaveLoadMode::Load),

            font_manager: font_manager.clone(),
            texture_manager: texture_manager.clone(),

            event_queue: Vec::new(),
            mouse_position: (0, 0),
            keys_pressed: HashMap::new(),

            screen_size: (screen_width, screen_height),
            ui_scale: 1.0,
            debug_mode: false,
            quick_start_enabled: false,
            // C++ gameplay logic advances at 30 logic frames/second.
            last_delta_time: 1.0 / 30.0,
        }
    }

    /// Enable quick-start behavior (skip title/menu screens).
    pub fn enable_quick_start(&mut self) {
        self.quick_start_enabled = true;
    }

    /// Initialize the UI system
    pub fn initialize(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        initialize_shell_ui_schemes();

        #[cfg(not(feature = "game_client"))]
        {
            let mut font_manager = self.font_manager.lock().unwrap_or_else(|e| e.into_inner());
            font_manager.load_font("title", "fonts/generals_title.ttf", 36.0)?;
            font_manager.load_font("menu", "fonts/generals_menu.ttf", 24.0)?;
            font_manager.load_font("button", "fonts/generals_button.ttf", 18.0)?;
            font_manager.load_font("hud", "fonts/generals_hud.ttf", 16.0)?;
        }

        // C++ parity: startup/menu visuals come from the shell window system
        // (WindowZH + mapped images + draw callbacks), not from a second Rust-local
        // texture pack. Keep the temporary UI layer textureless when the shell
        // path is compiled in so startup does not depend on fake asset names.
        #[cfg(not(feature = "game_client"))]
        {
            let mut texture_manager = self
                .texture_manager
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            texture_manager.load_texture("background", "textures/menu_background.tga")?;
            texture_manager.load_texture("button_normal", "textures/button_normal.tga")?;
            texture_manager.load_texture("button_hover", "textures/button_hover.tga")?;
            texture_manager.load_texture("button_pressed", "textures/button_pressed.tga")?;
            texture_manager.load_texture("logo", "textures/generals_logo.tga")?;
        }

        // Initialize individual UI components
        self.main_menu.initialize()?;
        self.faction_selection.initialize()?;
        self.game_hud.initialize()?;
        self.pause_menu.initialize()?;
        self.victory_screen.initialize()?;
        self.skirmish_menu.initialize()?;
        self.save_load_menu.initialize()?;

        if self.quick_start_enabled {
            info!(
                "{}",
                localization::localize(
                    "ui_manager.log.quick_start_skip",
                    "UI: QuickStart enabled, keeping title/menu startup flow"
                )
            );
        }
        self.transition_to_screen(Screen::startup_entry_screen(self.quick_start_enabled));

        Ok(())
    }

    /// Update UI system
    pub fn update(&mut self, delta_time: f32) -> Result<(), Box<dyn std::error::Error>> {
        self.last_delta_time = delta_time;
        // Process events
        self.process_events();

        // Update transitions
        if self.transitioning {
            self.transition_elapsed += delta_time;
            if self.transition_elapsed >= self.transition_duration.as_secs_f32() {
                self.transitioning = false;
            }
        }

        // Update current screen
        match self.current_screen {
            Some(Screen::Title) => {
                // Title screen auto-transitions to main menu after delay
                if !self.transitioning {
                    // Use non-blocking approach for async runtime
                    self.transition_to_screen(Screen::MainMenu);
                }
            }
            Some(Screen::MainMenu) => {
                self.main_menu.update(delta_time)?;
            }
            Some(Screen::FactionSelection) => {
                self.faction_selection.update(delta_time)?;
            }
            Some(Screen::GameHUD) => {
                self.game_hud.update(delta_time)?;
            }
            Some(Screen::PauseMenu) => {
                self.pause_menu.update(delta_time)?;
            }
            Some(Screen::Victory) => {
                self.victory_screen.update(delta_time)?;
            }
            Some(Screen::Skirmish) => {
                self.skirmish_menu.update(delta_time)?;
            }
            Some(Screen::LoadGame) | Some(Screen::SaveGame) => {
                self.save_load_menu.update(delta_time)?;
            }
            _ => {}
        }

        self.drain_screen_pending_events();
        Ok(())
    }

    /// Render UI system
    pub fn render(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut context = UIRenderContext {
            screen_size: self.screen_size,
            delta_time: self.last_delta_time,
            mouse_position: self.mouse_position,
            font_manager: self.font_manager.clone(),
            texture_manager: self.texture_manager.clone(),
            draw_commands: Vec::new(),
        };

        // Render current screen
        match self.current_screen {
            Some(Screen::Title) => {
                self.render_title_screen(&mut context)?;
            }
            Some(Screen::MainMenu) => {
                self.main_menu.render(&mut context);
            }
            Some(Screen::FactionSelection) => {
                self.faction_selection.render(&mut context);
            }
            Some(Screen::GameHUD) => {
                self.game_hud.render(&mut context);
            }
            Some(Screen::PauseMenu) => {
                // Also render game HUD in background (dimmed)
                self.render_dimmed_background(&mut context)?;
                self.pause_menu.render(&mut context);
            }
            Some(Screen::Victory) => {
                self.victory_screen.render(&mut context);
            }
            Some(Screen::Skirmish) => {
                self.skirmish_menu.render(&mut context);
            }
            Some(Screen::LoadGame) | Some(Screen::SaveGame) => {
                self.save_load_menu.render(&mut context);
            }
            _ => {}
        }

        // Render transition effects
        if self.transitioning {
            self.render_transition_effect(&mut context)?;
        }

        // Render debug info if enabled
        if self.debug_mode {
            self.render_debug_info(&mut context)?;
        }

        Ok(())
    }

    /// Handle mouse movement
    pub fn handle_mouse_move(&mut self, x: i32, y: i32) {
        self.mouse_position = (x, y);

        match self.current_screen {
            Some(Screen::MainMenu) => {
                self.main_menu.handle_mouse_move(x, y);
            }
            Some(Screen::FactionSelection) => {
                self.faction_selection.handle_mouse_move(x, y);
            }
            Some(Screen::GameHUD) => {
                self.game_hud.handle_mouse_move(x, y);
            }
            Some(Screen::PauseMenu) => {
                self.pause_menu.handle_mouse_move(x, y);
            }
            Some(Screen::Victory) => {
                self.victory_screen.handle_mouse_move(x, y);
            }
            Some(Screen::Skirmish) => {
                self.skirmish_menu.handle_mouse_move(x, y);
            }
            Some(Screen::LoadGame) | Some(Screen::SaveGame) => {
                self.save_load_menu.handle_mouse_move(x, y);
            }
            _ => {}
        }

        self.drain_screen_pending_events();
    }

    /// Handle mouse clicks
    pub fn handle_mouse_click(&mut self, x: i32, y: i32, button: MouseButton) -> bool {
        let handled = match self.current_screen {
            Some(Screen::Title) => {
                // Any click on title screen goes to main menu
                self.transition_to_screen(Screen::MainMenu);
                true
            }
            Some(Screen::MainMenu) => {
                if let Some(event) = self.main_menu.handle_mouse_click(x, y, button) {
                    self.queue_event(event);
                    true
                } else {
                    false
                }
            }
            Some(Screen::FactionSelection) => {
                if let Some(event) = self.faction_selection.handle_mouse_click(x, y, button) {
                    self.queue_event(event);
                    true
                } else {
                    false
                }
            }
            Some(Screen::GameHUD) => {
                if let Some(event) = self.game_hud.handle_mouse_click(x, y, button) {
                    self.queue_event(event);
                    true
                } else {
                    false
                }
            }
            Some(Screen::PauseMenu) => {
                if let Some(event) = self.pause_menu.handle_mouse_click(x, y, button) {
                    self.queue_event(event);
                    true
                } else {
                    false
                }
            }
            Some(Screen::Victory) => {
                if let Some(event) = self.victory_screen.handle_mouse_click(x, y, button) {
                    self.queue_event(event);
                    true
                } else {
                    false
                }
            }
            Some(Screen::Skirmish) => {
                if let Some(event) = self.skirmish_menu.handle_mouse_click(x, y, button) {
                    self.queue_event(event);
                    true
                } else {
                    false
                }
            }
            Some(Screen::LoadGame) | Some(Screen::SaveGame) => {
                if let Some(event) = self.save_load_menu.handle_mouse_click(x, y, button) {
                    self.queue_event(event);
                    true
                } else {
                    false
                }
            }
            _ => false,
        };

        self.drain_screen_pending_events();
        handled
    }

    /// Handle key presses
    pub fn handle_key_press(&mut self, key: KeyCode) -> bool {
        self.keys_pressed.insert(key, true);

        // Global key handlers
        match key {
            KeyCode::Escape => match self.current_state {
                UIState::InGame => {
                    self.transition_to_screen(Screen::PauseMenu);
                    return true;
                }
                UIState::Paused => {
                    self.transition_to_screen(Screen::GameHUD);
                    return true;
                }
                _ => {}
            },
            KeyCode::F1 => {
                self.debug_mode = !self.debug_mode;
                return true;
            }
            _ => {}
        }

        // Screen-specific key handlers
        let handled = match self.current_screen {
            Some(Screen::MainMenu) => self.main_menu.handle_key_press(key),
            Some(Screen::FactionSelection) => self.faction_selection.handle_key_press(key),
            Some(Screen::GameHUD) => self.game_hud.handle_key_press(key),
            Some(Screen::PauseMenu) => self.pause_menu.handle_key_press(key),
            Some(Screen::Skirmish) => self.skirmish_menu.handle_key_press(key),
            Some(Screen::LoadGame) | Some(Screen::SaveGame) => {
                self.save_load_menu.handle_key_press(key)
            }
            _ => false,
        };

        self.drain_screen_pending_events();
        handled
    }

    /// Handle key releases
    pub fn handle_key_release(&mut self, key: KeyCode) {
        self.keys_pressed.insert(key, false);
    }

    /// Queue an event for processing
    pub fn queue_event(&mut self, event: UIEvent) {
        self.event_queue.push(event);
    }

    /// Pop the next pending UI event in FIFO order for external systems to handle.
    pub fn pop_event(&mut self) -> Option<UIEvent> {
        if self.event_queue.is_empty() {
            None
        } else {
            Some(self.event_queue.remove(0))
        }
    }

    /// Get current UI state
    pub fn get_state(&self) -> UIState {
        self.current_state
    }

    /// Get the current screen being displayed
    pub fn current_screen(&self) -> Option<Screen> {
        self.current_screen
    }

    /// Set game state (called from game logic)
    pub fn set_game_paused(&mut self, paused: bool) {
        if paused {
            self.transition_to_screen(Screen::PauseMenu);
        } else if self.current_state == UIState::Paused {
            self.transition_to_screen(Screen::GameHUD);
        }
    }

    /// Set victory state
    pub fn set_victory(&mut self, player_id: u32) {
        self.set_victory_with_summary(player_id, None);
    }

    pub fn set_victory_with_summary(&mut self, player_id: u32, summary: Option<VictorySummary>) {
        self.victory_screen.set_victory(player_id);
        self.victory_screen.set_summary(summary);
        self.transition_to_screen(Screen::Victory);
    }

    /// Set defeat state  
    pub fn set_defeat(&mut self) {
        self.set_defeat_with_summary(None);
    }

    pub fn set_defeat_with_summary(&mut self, summary: Option<VictorySummary>) {
        self.victory_screen.set_defeat();
        self.victory_screen.set_summary(summary);
        self.transition_to_screen(Screen::Victory);
    }

    pub fn set_draw(&mut self) {
        self.set_draw_with_summary(None);
    }

    pub fn set_draw_with_summary(&mut self, summary: Option<VictorySummary>) {
        self.victory_screen.set_draw();
        self.victory_screen.set_summary(summary);
        self.transition_to_screen(Screen::Victory);
    }

    pub fn clear_victory_screen(&mut self) {
        self.victory_screen.clear();
    }

    /// Resize UI for new screen dimensions
    pub fn resize(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);

        // Update UI scale based on screen size
        let base_width = 1024.0;
        self.ui_scale = width as f32 / base_width;

        // Notify components of resize
        self.main_menu.resize(width, height);
        self.faction_selection.resize(width, height);
        self.game_hud.resize(width, height);
        self.pause_menu.resize(width, height);
        self.victory_screen.resize(width, height);
        self.skirmish_menu.resize(width, height);
        self.save_load_menu.resize(width, height);
    }

    // Private methods

    fn drain_screen_pending_events(&mut self) {
        let pending = match self.current_screen {
            Some(Screen::MainMenu) => self.main_menu.drain_pending_events(),
            Some(Screen::FactionSelection) => self.faction_selection.drain_pending_events(),
            Some(Screen::PauseMenu) => self.pause_menu.drain_pending_events(),
            Some(Screen::Skirmish) => self.skirmish_menu.drain_pending_events(),
            Some(Screen::LoadGame) | Some(Screen::SaveGame) => {
                self.save_load_menu.drain_pending_events()
            }
            _ => Vec::new(),
        };

        for event in pending {
            self.queue_event(event);
        }
    }

    /// Transition to a new screen
    pub(crate) fn transition_to_screen(&mut self, screen: Screen) {
        if Some(screen) != self.current_screen {
            let previous_screen = self.current_screen;
            self.current_screen = Some(screen);
            self.transitioning = true;
            self.transition_elapsed = 0.0;

            // Update UI state based on screen
            self.current_state = match screen {
                Screen::Title => UIState::Title,
                Screen::MainMenu => UIState::MainMenu,
                Screen::FactionSelection => UIState::FactionSelection,
                Screen::GameHUD => UIState::InGame,
                Screen::PauseMenu => UIState::Paused,
                Screen::Victory => UIState::GameEnd,
                Screen::Skirmish => UIState::GameSetup,
                Screen::LoadGame | Screen::SaveGame => UIState::GameSetup,
                Screen::Loading => UIState::Loading,
                _ => self.current_state,
            };

            match screen {
                Screen::LoadGame => {
                    self.save_load_menu.set_mode(SaveLoadMode::Load);
                    if let Some(prev) = previous_screen {
                        self.save_load_menu.set_return_screen(prev);
                    }
                    let _ = self.save_load_menu.initialize();
                }
                Screen::SaveGame => {
                    self.save_load_menu.set_mode(SaveLoadMode::Save);
                    if let Some(prev) = previous_screen {
                        self.save_load_menu.set_return_screen(prev);
                    }
                    let _ = self.save_load_menu.initialize();
                }
                _ => {}
            }

            // Play transition sound
            self.event_queue.push(UIEvent::PlaySoundEffectPath(
                sound_files::BUTTON_CLICK.to_string(),
            ));
            let screen_name = format!("{:?}", screen);
            info!(
                "{}",
                localization::localize_with_args(
                    "ui_manager.log.transition",
                    "UI: Transitioning to {screen}",
                    &[("screen", screen_name.as_str())],
                )
            );
        }
    }

    pub(crate) fn suspend_for_shell_overlay(&mut self) {
        self.current_screen = None;
        self.current_state = UIState::Loading;
        self.transitioning = false;
        self.transition_elapsed = 0.0;
        self.event_queue.clear();
        self.keys_pressed.clear();
    }

    /// Process queued events
    fn process_events(&mut self) {
        let events: Vec<UIEvent> = self.event_queue.drain(..).collect();

        for event in events {
            match event {
                UIEvent::ChangeScreen(screen) => {
                    self.transition_to_screen(screen);
                }
                UIEvent::StartGame { mode, faction, map } => {
                    let mode_label = format!("{:?}", mode);
                    info!(
                        "{}",
                        localization::localize_with_args(
                            "ui_manager.log.start_game",
                            "Starting game: mode={mode}, faction={faction}, map={map}",
                            &[
                                ("mode", mode_label.as_str()),
                                ("faction", faction.as_str()),
                                ("map", map.as_str()),
                            ],
                        )
                    );
                    // Instruct game logic to start the appropriate mode/map when available
                    self.event_queue
                        .push(UIEvent::StartGame { mode, faction, map });
                }
                UIEvent::LoadGame(save_path) => {
                    info!(
                        "{}",
                        localization::localize_with_args(
                            "ui_manager.log.load_game",
                            "Loading game: {path}",
                            &[("path", save_path.as_str())],
                        )
                    );
                    self.event_queue.push(UIEvent::LoadGame(save_path));
                }
                UIEvent::SaveGame { slot, display_name } => {
                    info!(
                        "{}",
                        localization::localize_with_args(
                            "ui_manager.log.save_game",
                            "Saving game: {name} ({slot})",
                            &[("name", display_name.as_str()), ("slot", slot.as_str())],
                        )
                    );
                    self.event_queue
                        .push(UIEvent::SaveGame { slot, display_name });
                }
                UIEvent::RestartMission => {
                    info!(
                        "{}",
                        localization::localize(
                            "ui_manager.log.restart_mission",
                            "Restarting mission"
                        )
                    );
                    self.event_queue.push(UIEvent::RestartMission);
                }
                UIEvent::ExitToMenu => {
                    self.transition_to_screen(Screen::MainMenu);
                }
                UIEvent::ExitGame => {
                    info!(
                        "{}",
                        localization::localize("ui_manager.log.exit_game", "Exiting game")
                    );
                    self.event_queue.push(UIEvent::ExitGame);
                }
                UIEvent::TogglePause => match self.current_state {
                    UIState::InGame => self.transition_to_screen(Screen::PauseMenu),
                    UIState::Paused => self.transition_to_screen(Screen::GameHUD),
                    _ => {}
                },
                UIEvent::ShowOptions => {
                    self.transition_to_screen(Screen::Options);
                }
                UIEvent::SettingsChanged => {
                    info!(
                        "{}",
                        localization::localize(
                            "ui_manager.log.settings_changed",
                            "Settings changed"
                        )
                    );
                    // Forward to host systems (graphics/audio/input) to apply runtime changes.
                    self.event_queue.push(UIEvent::SettingsChanged);
                }
                UIEvent::PlaySoundEffectPath(path) => {
                    // Audio playback is handled by the main game engine loop.
                    self.event_queue.push(UIEvent::PlaySoundEffectPath(path));
                }
                UIEvent::FocusCamera(position) => {
                    // Camera movement is handled by the main game engine loop.
                    self.event_queue.push(UIEvent::FocusCamera(position));
                }
            }
        }
    }

    /// Render title screen
    fn render_title_screen(
        &self,
        context: &mut UIRenderContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Title screen uses menu backdrop/logo rendering path to match classic flow.
        self.main_menu.render(context);
        info!(
            "{}",
            localization::localize("ui_manager.log.render_title", "Rendering title screen")
        );
        Ok(())
    }

    /// Render dimmed background (for pause menu)
    fn render_dimmed_background(
        &self,
        context: &mut UIRenderContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Render the active HUD beneath pause/victory overlays.
        self.game_hud.render(context);
        trace!(
            "{}",
            localization::localize(
                "ui_manager.log.render_dimmed_background",
                "Rendering dimmed gameplay background"
            )
        );
        Ok(())
    }

    /// Render transition effects
    fn render_transition_effect(
        &self,
        _context: &mut UIRenderContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.transitioning {
            let progress =
                (self.transition_elapsed / self.transition_duration.as_secs_f32()).min(1.0);

            // Until full shader-driven UI transitions are wired, keep deterministic timing and
            // expose alpha for render diagnostics.
            let alpha = 1.0 - progress;
            debug!(
                "{}",
                localization::localize_with_args(
                    "ui_manager.log.transition_progress_with_alpha",
                    "Transition progress: {percent} alpha: {alpha}",
                    &[
                        ("percent", format!("{:.2}", progress).as_str()),
                        ("alpha", format!("{:.2}", alpha).as_str()),
                    ],
                )
            );
        }
        Ok(())
    }

    /// Render debug information
    fn render_debug_info(
        &self,
        context: &mut UIRenderContext,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let fps = if context.delta_time > f32::EPSILON {
            1.0 / context.delta_time
        } else {
            0.0
        };
        let state_label = format!("{:?}", self.current_state);
        let screen_label = format!("{:?}", self.current_screen);
        let mouse_label = format!("{:?}", self.mouse_position);
        debug!(
            "{}",
            localization::localize_with_args(
                "ui_manager.log.debug_state_with_fps",
                "Debug: State={state}, Screen={screen}, Mouse={mouse}, FPS={fps}",
                &[
                    ("state", state_label.as_str()),
                    ("screen", screen_label.as_str()),
                    ("mouse", mouse_label.as_str()),
                    ("fps", format!("{fps:.1}").as_str()),
                ],
            )
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ui_manager_creation() {
        let manager = UIManager::new(1024, 768);
        assert_eq!(manager.get_state(), UIState::Loading);
        assert_eq!(manager.screen_size, (1024, 768));
    }

    #[test]
    fn test_screen_transitions() {
        let mut manager = UIManager::new(1024, 768);
        manager.transition_to_screen(Screen::MainMenu);
        assert_eq!(manager.current_screen, Some(Screen::MainMenu));
        assert_eq!(manager.get_state(), UIState::MainMenu);
    }

    #[test]
    fn test_event_queuing() {
        let mut manager = UIManager::new(1024, 768);
        manager.queue_event(UIEvent::ChangeScreen(Screen::FactionSelection));
        assert_eq!(manager.event_queue.len(), 1);
    }

    #[test]
    fn game_load_events_do_not_activate_custom_loading_screen() {
        let mut manager = UIManager::new(1024, 768);
        manager.suspend_for_shell_overlay();

        manager.queue_event(UIEvent::StartGame {
            mode: GameMode::Skirmish,
            faction: "America".to_string(),
            map: "Maps/Test/Test.map".to_string(),
        });
        manager.update(1.0 / 30.0).unwrap();
        assert_eq!(manager.current_screen, None);

        manager.queue_event(UIEvent::LoadGame("quicksave".to_string()));
        manager.update(1.0 / 30.0).unwrap();
        assert_eq!(manager.current_screen, None);

        manager.queue_event(UIEvent::RestartMission);
        manager.update(1.0 / 30.0).unwrap();
        assert_eq!(manager.current_screen, None);
    }
}
