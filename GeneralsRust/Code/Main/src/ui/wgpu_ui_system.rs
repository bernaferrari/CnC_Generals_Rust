use super::layout_manager::UILayoutManager;
use super::wgpu_renderer::{Color, WgpuUIRenderer};
use crate::{
    game_logic::{GameLogic, GameMode},
    localization,
};
use glam::Vec2;
use glam::Vec4;
use log::info;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use ww3d_engine::FrameTiming;

fn vec4_to_color(v: Vec4) -> super::wgpu_renderer::Color {
    super::wgpu_renderer::Color {
        r: v.x,
        g: v.y,
        b: v.z,
        a: v.w,
    }
}

/// UI System State - matches C++ UI states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UISystemState {
    MainMenu,
    FactionSelection,
    InGame,
    PauseMenu,
    Victory,
    Loading,
}

/// UI Event types matching C++ GUI callbacks
#[derive(Debug, Clone)]
pub enum UISystemEvent {
    ButtonClicked {
        element_id: u32,
        button_name: String,
    },
    StartGame {
        mode: GameMode,
        faction: String,
    },
    ExitGame,
    ShowOptions,
    LoadGame,
    BackToMainMenu,
    PauseToggle,
    None,
}

/// Main WGPU UI System - replaces C++ GameWindowManager + UI rendering
pub struct WgpuUISystem {
    renderer: WgpuUIRenderer,
    layout_manager: UILayoutManager,
    current_state: UISystemState,
    mouse_pos: Vec2,
    mouse_pressed: bool,
    hover_element: Option<u32>,
    active_element: Option<u32>,
    last_mouse_button: u32,
    click_times: HashMap<u32, Instant>,

    // UI State Management
    main_menu_elements: HashMap<String, u32>,
    control_bar_elements: HashMap<String, u32>,
    faction_selection_elements: HashMap<String, u32>,
    pause_menu_elements: HashMap<String, u32>,
    victory_elements: HashMap<String, u32>,
    loading_elements: HashMap<String, u32>,

    // Game integration
    game_logic: Option<Arc<Mutex<GameLogic>>>,

    // Resource loading
    ui_textures: HashMap<String, u32>,
    ui_fonts: HashMap<String, u32>,
}

impl WgpuUISystem {
    pub async fn new(
        window: &winit::window::Window,
    ) -> Result<Self, Box<dyn std::error::Error + '_>> {
        let renderer = WgpuUIRenderer::new(window).await?;
        let window_size = window.inner_size();
        let layout_manager =
            UILayoutManager::new(window_size.width as f32, window_size.height as f32);

        let mut ui_system = Self {
            renderer,
            layout_manager,
            current_state: UISystemState::MainMenu,
            mouse_pos: Vec2::ZERO,
            mouse_pressed: false,
            hover_element: None,
            active_element: None,
            last_mouse_button: 0,
            click_times: HashMap::new(),
            main_menu_elements: HashMap::new(),
            control_bar_elements: HashMap::new(),
            faction_selection_elements: HashMap::new(),
            pause_menu_elements: HashMap::new(),
            victory_elements: HashMap::new(),
            loading_elements: HashMap::new(),
            game_logic: None,
            ui_textures: HashMap::new(),
            ui_fonts: HashMap::new(),
        };

        // Initialize UI layouts
        ui_system.initialize_main_menu();
        ui_system.initialize_control_bar();

        Ok(ui_system)
    }

    /// Initialize main menu layout - matches C++ MainMenu.cpp
    fn initialize_main_menu(&mut self) {
        info!(
            "{}",
            localization::localize("wgpu_ui.log.init_main_menu", "Initializing Main Menu UI...")
        );
        self.main_menu_elements = self.layout_manager.create_main_menu_layout();
        self.set_state(UISystemState::MainMenu);

        // Pre-create layouts so state switches are instant.
        self.faction_selection_elements = self.layout_manager.create_faction_selection_layout();
        self.pause_menu_elements = self.layout_manager.create_pause_menu_layout();
        self.victory_elements = self.layout_manager.create_victory_layout();
        self.loading_elements = self.layout_manager.create_loading_layout();

        // Hide non-active overlays by default.
        for map in [
            &self.faction_selection_elements,
            &self.pause_menu_elements,
            &self.victory_elements,
            &self.loading_elements,
        ] {
            for &id in map.values() {
                self.layout_manager.set_element_visible(id, false);
            }
        }
    }

    /// Initialize control bar (in-game HUD) - matches C++ ControlBar.cpp
    fn initialize_control_bar(&mut self) {
        info!(
            "{}",
            localization::localize(
                "wgpu_ui.log.init_control_bar",
                "Initializing Control Bar UI..."
            )
        );
        self.control_bar_elements = self.layout_manager.create_control_bar_layout();
    }

    /// Set the current UI state and show/hide appropriate elements
    pub fn set_state(&mut self, new_state: UISystemState) {
        if self.current_state == new_state {
            return;
        }

        let old_state = format!("{:?}", self.current_state);
        let new_state_str = format!("{:?}", new_state);
        info!(
            "{}",
            localization::localize_with_args(
                "wgpu_ui.log.state_change",
                "UI State changing from {old} to {new}",
                &[("old", old_state.as_str()), ("new", new_state_str.as_str())],
            )
        );
        self.current_state = new_state;

        // Hide all elements first
        self.hide_all_elements();

        // Show elements for current state
        match new_state {
            UISystemState::MainMenu => self.show_main_menu(),
            UISystemState::InGame => self.show_in_game_hud(),
            UISystemState::FactionSelection => self.show_faction_selection(),
            UISystemState::PauseMenu => self.show_pause_menu(),
            UISystemState::Victory => self.show_victory_screen(),
            UISystemState::Loading => self.show_loading_screen(),
        }
    }

    fn hide_all_elements(&mut self) {
        let all_visible = self.layout_manager.get_all_visible_elements();
        for element_id in all_visible {
            self.layout_manager.set_element_visible(element_id, false);
        }
    }

    fn show_main_menu(&mut self) {
        for &element_id in self.main_menu_elements.values() {
            self.layout_manager.set_element_visible(element_id, true);
        }
    }

    fn show_in_game_hud(&mut self) {
        for &element_id in self.control_bar_elements.values() {
            self.layout_manager.set_element_visible(element_id, true);
        }
    }

    fn show_faction_selection(&mut self) {
        for &id in self.faction_selection_elements.values() {
            self.layout_manager.set_element_visible(id, true);
        }
    }

    fn show_pause_menu(&mut self) {
        // Show pause menu over in-game HUD
        self.show_in_game_hud();
        for &id in self.pause_menu_elements.values() {
            self.layout_manager.set_element_visible(id, true);
        }
    }

    fn show_victory_screen(&mut self) {
        for &id in self.victory_elements.values() {
            self.layout_manager.set_element_visible(id, true);
        }
    }

    fn show_loading_screen(&mut self) {
        for &id in self.loading_elements.values() {
            self.layout_manager.set_element_visible(id, true);
        }
    }

    /// Handle mouse input - matches C++ mouse handling
    pub fn handle_mouse_move(&mut self, x: f32, y: f32) -> bool {
        self.mouse_pos = Vec2::new(x, y);

        let new_hover = self.layout_manager.find_element_at_position(x, y);
        if new_hover != self.hover_element {
            // Mouse enter/leave events
            if let Some(old_hover) = self.hover_element {
                self.on_mouse_leave(old_hover);
            }
            if let Some(new_hover) = new_hover {
                self.on_mouse_enter(new_hover);
            }
            self.hover_element = new_hover;
        }

        new_hover.is_some()
    }

    pub fn handle_mouse_click(
        &mut self,
        x: f32,
        y: f32,
        button: u32,
        pressed: bool,
    ) -> UISystemEvent {
        self.mouse_pressed = pressed;
        self.last_mouse_button = button;

        if pressed {
            if let Some(element_id) = self.layout_manager.find_element_at_position(x, y) {
                self.active_element = Some(element_id);
                self.click_times.insert(element_id, Instant::now());
                return self.on_element_clicked(element_id);
            }
        } else {
            self.active_element = None;
        }

        UISystemEvent::None
    }

    /// Handle element click - matches C++ button callbacks
    fn on_element_clicked(&self, element_id: u32) -> UISystemEvent {
        if let Some(element) = self.layout_manager.get_element(element_id) {
            let element_id_str = element_id.to_string();
            info!(
                "{}",
                localization::localize_with_args(
                    "wgpu_ui.log.element_clicked",
                    "Element clicked: {name} (ID: {id})",
                    &[
                        ("name", element.name.as_str()),
                        ("id", element_id_str.as_str()),
                    ],
                )
            );

            match self.current_state {
                UISystemState::MainMenu => self.handle_main_menu_click(&element.name),
                UISystemState::FactionSelection => self.handle_faction_click(&element.name),
                UISystemState::PauseMenu => self.handle_pause_click(&element.name),
                UISystemState::Victory => self.handle_victory_click(&element.name),
                UISystemState::InGame => self.handle_control_bar_click(&element.name),
                _ => UISystemEvent::None,
            }
        } else {
            UISystemEvent::None
        }
    }

    /// Handle main menu button clicks - matches C++ MainMenu callbacks
    fn handle_main_menu_click(&self, element_name: &str) -> UISystemEvent {
        match element_name {
            "SinglePlayer" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.single_player", "Single Player clicked")
                );
                UISystemEvent::StartGame {
                    mode: GameMode::SinglePlayer,
                    faction: "USA".to_string(),
                }
            }
            "Skirmish" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.skirmish", "Skirmish clicked")
                );
                UISystemEvent::StartGame {
                    mode: GameMode::Skirmish,
                    faction: "USA".to_string(),
                }
            }
            "Network" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.network", "Network clicked")
                );
                UISystemEvent::StartGame {
                    mode: GameMode::Multiplayer,
                    faction: "USA".to_string(),
                }
            }
            "Options" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.options", "Options clicked")
                );
                UISystemEvent::ShowOptions
            }
            "LoadReplay" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.load_replay", "Load Replay clicked")
                );
                UISystemEvent::LoadGame
            }
            "Credits" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.credits", "Credits clicked")
                );
                UISystemEvent::ShowOptions
            }
            "Exit" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.exit", "Exit clicked")
                );
                UISystemEvent::ExitGame
            }
            _ => UISystemEvent::None,
        }
    }

    /// Handle control bar clicks - matches C++ ControlBar callbacks
    fn handle_control_bar_click(&self, element_name: &str) -> UISystemEvent {
        if element_name.starts_with("CommandButton") {
            info!(
                "{}",
                localization::localize_with_args(
                    "wgpu_ui.log.command_button",
                    "Command button clicked: {name}",
                    &[("name", element_name)],
                )
            );
            self.handle_command_button(element_name);
            if let Some(element) = self.layout_manager.get_element_by_name(element_name) {
                return UISystemEvent::ButtonClicked {
                    element_id: element.id,
                    button_name: element_name.to_string(),
                };
            }
        } else if element_name == "Minimap" {
            info!(
                "{}",
                localization::localize("wgpu_ui.log.minimap_clicked", "Minimap clicked")
            );
            self.handle_minimap_click();
        }

        UISystemEvent::None
    }

    fn local_player_id(game_logic: &GameLogic) -> Option<u32> {
        if game_logic.get_player(0).is_some() {
            return Some(0);
        }
        game_logic.get_players().keys().copied().min()
    }

    fn selected_units(game_logic: &GameLogic) -> Vec<crate::game_logic::ObjectId> {
        let Some(pid) = Self::local_player_id(game_logic) else {
            return Vec::new();
        };
        game_logic
            .get_player(pid)
            .map(|p| p.selected_objects.clone())
            .unwrap_or_default()
    }

    fn handle_command_button(&self, element_name: &str) {
        let Some(game_logic_arc) = self.game_logic.as_ref() else {
            return;
        };
        let Ok(mut game_logic) = game_logic_arc.lock() else {
            return;
        };

        let units = Self::selected_units(&game_logic);
        if units.is_empty() {
            return;
        }

        let player_id = Self::local_player_id(&game_logic).unwrap_or(0);
        let now = std::time::SystemTime::now();
        let modifier_keys = crate::command_system::ModifierKeys::default();

        // Map the first row of 3x4 command buttons to core RTS actions.
        // (C++ ControlBar is context-sensitive; this is a faithful "minimal core" subset.)
        let command_type = match element_name {
            "CommandButton0_0" => crate::command_system::CommandType::Stop,
            "CommandButton0_1" => {
                let mut center = glam::Vec3::ZERO;
                let mut count = 0.0;
                for id in &units {
                    if let Some(obj) = game_logic.get_object(*id) {
                        center += obj.get_position();
                        count += 1.0;
                    }
                }
                let center = if count > 0.0 {
                    center / count
                } else {
                    glam::Vec3::ZERO
                };
                crate::command_system::CommandType::Guard {
                    target: crate::command_system::GuardTarget::Position(center),
                }
            }
            "CommandButton0_2" => crate::command_system::CommandType::Scatter,
            "CommandButton0_3" => crate::command_system::CommandType::Sell {
                object_id: units[0],
            },
            _ => return,
        };

        game_logic.queue_command(crate::command_system::GameCommand {
            command_type,
            player_id,
            command_id: 0,
            timestamp: now,
            selected_units: units,
            modifier_keys,
        });
    }

    fn handle_minimap_click(&self) {
        let Some(minimap) = self.layout_manager.get_element_by_name("Minimap") else {
            return;
        };
        let rect = minimap.get_absolute_rect(&self.layout_manager);
        if rect.width <= 1.0 || rect.height <= 1.0 {
            return;
        }

        let u = ((self.mouse_pos.x - rect.x) / rect.width).clamp(0.0, 1.0);
        let v = ((self.mouse_pos.y - rect.y) / rect.height).clamp(0.0, 1.0);

        let Some(game_logic_arc) = self.game_logic.as_ref() else {
            return;
        };
        let Ok(mut game_logic) = game_logic_arc.lock() else {
            return;
        };

        let (min, max) = game_logic.world_bounds();
        let world_pos = glam::Vec3::new(
            min.x + (max.x - min.x) * u,
            0.0,
            min.z + (max.z - min.z) * v,
        );

        // Right click issues move for current selection; left click pans camera.
        if self.last_mouse_button == 1 {
            let units = Self::selected_units(&game_logic);
            let player_id = Self::local_player_id(&game_logic).unwrap_or(0);
            game_logic.queue_command(crate::command_system::GameCommand {
                command_type: crate::command_system::CommandType::Move {
                    destination: world_pos,
                },
                player_id,
                command_id: 0,
                timestamp: std::time::SystemTime::now(),
                selected_units: units,
                modifier_keys: crate::command_system::ModifierKeys::default(),
            });
        } else {
            game_logic.request_camera_focus(world_pos);
        }
    }

    /// Handle faction selection clicks.
    fn handle_faction_click(&self, element_name: &str) -> UISystemEvent {
        if element_name.starts_with("Faction") && element_name.len() > "Faction".len() {
            let faction = element_name.trim_start_matches("Faction").to_string();
            return UISystemEvent::StartGame {
                mode: GameMode::Skirmish,
                faction,
            };
        }
        if element_name == "FactionStart" {
            return UISystemEvent::StartGame {
                mode: GameMode::Skirmish,
                faction: "USA".to_string(),
            };
        }
        UISystemEvent::None
    }

    /// Handle pause menu clicks.
    fn handle_pause_click(&self, element_name: &str) -> UISystemEvent {
        match element_name {
            "PauseResume" => UISystemEvent::PauseToggle,
            "PauseOptions" => UISystemEvent::ShowOptions,
            "PauseQuitToMenu" => UISystemEvent::BackToMainMenu,
            _ => UISystemEvent::None,
        }
    }

    /// Handle victory screen clicks.
    fn handle_victory_click(&self, element_name: &str) -> UISystemEvent {
        if element_name == "VictoryExit" {
            UISystemEvent::BackToMainMenu
        } else {
            UISystemEvent::None
        }
    }

    fn on_mouse_enter(&self, element_id: u32) {
        if let Some(element) = self.layout_manager.get_element(element_id) {
            info!(
                "{}",
                localization::localize_with_args(
                    "wgpu_ui.log.mouse_enter",
                    "Mouse entered: {name}",
                    &[("name", element.name.as_str())],
                )
            );
        }
    }

    fn on_mouse_leave(&self, element_id: u32) {
        if let Some(element) = self.layout_manager.get_element(element_id) {
            info!(
                "{}",
                localization::localize_with_args(
                    "wgpu_ui.log.mouse_leave",
                    "Mouse left: {name}",
                    &[("name", element.name.as_str())],
                )
            );
        }
    }

    /// Render the UI - generates draw commands and renders them
    pub fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut draw_commands = Vec::new();
        let now = Instant::now();
        let mut expired_clicks = Vec::new();

        let visible_elements = self.layout_manager.get_all_visible_elements();

        // Generate draw commands for all visible elements
        for element_id in visible_elements {
            if let Some(element) = self.layout_manager.get_element(element_id) {
                let mut rect = element.get_absolute_rect(&self.layout_manager);
                let scale = self.element_click_scale(element_id, now);
                if (scale - 1.0).abs() > 0.001 {
                    rect = scale_rect_from_center(rect, scale);
                }
                let mut bg_color = vec4_to_color(element.background_color);

                if Some(element_id) == self.hover_element {
                    bg_color = Color::UI_BUTTON_HOVER;
                }
                if Some(element_id) == self.active_element && self.mouse_pressed {
                    bg_color = Color::UI_BUTTON_PRESSED;
                }

                let bg_command =
                    self.renderer
                        .create_rect(rect.x, rect.y, rect.width, rect.height, bg_color);
                draw_commands.push(bg_command);

                if let Some(tex_id) = element.texture_id {
                    let tex_cmd = self.renderer.create_textured_rect(
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        tex_id,
                    );
                    draw_commands.push(tex_cmd);
                }

                if !element.text.is_empty() {
                    let text_cmd = self.renderer.create_text(
                        &element.text,
                        rect.x + element.padding.x,
                        rect.y + element.padding.y + element.font_size,
                        element.font_size,
                        vec4_to_color(element.text_color),
                    );
                    draw_commands.push(text_cmd);
                }

                if self
                    .click_times
                    .get(&element_id)
                    .is_some_and(|t| now.duration_since(*t).as_secs_f32() > 0.35)
                {
                    expired_clicks.push(element_id);
                }
            }
        }

        for id in expired_clicks {
            self.click_times.remove(&id);
        }

        // Render all draw commands
        self.renderer.render(&draw_commands)?;

        Ok(())
    }

    /// Handle window resize
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.renderer.resize(new_size);
        self.layout_manager
            .resize(new_size.width as f32, new_size.height as f32);
    }

    /// Connect to game logic for data integration
    pub fn connect_game_logic(&mut self, game_logic: Arc<Mutex<GameLogic>>) {
        self.game_logic = Some(game_logic);
    }

    /// Update UI state based on WW3D frame timing
    pub fn update_with_timing(&mut self, _timing: &FrameTiming) {
        self.update_internal();
    }

    /// Legacy update hook (defaults to zero timing).
    pub fn update(&mut self) {
        self.update_internal();
    }

    fn update_internal(&mut self) {
        // Update resource display, unit health bars, etc.
        // This matches the C++ UI update loops
        let should_change_state = if let Some(ref game_logic) = self.game_logic {
            if let Ok(logic) = game_logic.try_lock() {
                let is_in_game = logic.isInGame();
                let current_state = self.current_state;

                // Determine state change without borrowing self
                if is_in_game && current_state == UISystemState::MainMenu {
                    Some(UISystemState::InGame)
                } else if !is_in_game && current_state == UISystemState::InGame {
                    Some(UISystemState::MainMenu)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        // Apply state change after releasing all borrows
        if let Some(new_state) = should_change_state {
            self.set_state(new_state);
        }
    }

    pub fn get_current_state(&self) -> UISystemState {
        self.current_state
    }

    fn element_click_scale(&self, element_id: u32, now: Instant) -> f32 {
        let mut scale = 1.0;
        if Some(element_id) == self.active_element && self.mouse_pressed {
            scale *= 0.96;
        }

        if let Some(click_time) = self.click_times.get(&element_id) {
            let elapsed = now.duration_since(*click_time).as_secs_f32();
            if elapsed <= 0.35 {
                let damping = 12.0;
                let frequency = 24.0;
                let amplitude = 0.06;
                let spring = (-damping * elapsed).exp() * (frequency * elapsed).sin();
                scale *= 1.0 + amplitude * spring;
            }
        }

        scale
    }
}

fn scale_rect_from_center(
    rect: super::layout_manager::Rect,
    scale: f32,
) -> super::layout_manager::Rect {
    let center = rect.center();
    let width = rect.width * scale;
    let height = rect.height * scale;
    super::layout_manager::Rect::new(
        center.x - width * 0.5,
        center.y - height * 0.5,
        width,
        height,
    )
}
