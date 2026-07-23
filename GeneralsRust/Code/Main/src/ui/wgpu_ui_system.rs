use super::layout_manager::UILayoutManager;
use super::wgpu_renderer::{Color, WgpuUIRenderer};
use crate::{
    game_logic::{GameLogic, GameMode},
    localization,
};
use game_engine::common::ini::ini_webpage_url::get_registry_language;
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::get_file_system;
use glam::Vec2;
use glam::Vec4;
use log::{debug, info, warn};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MainMenuPanel {
    Main,
    SinglePlayer,
    Multiplayer,
    LoadReplay,
    Difficulty,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CampaignSelection {
    USA,
    GLA,
    China,
    Challenge,
}

#[derive(Debug, Clone)]
struct MappedImageDef {
    texture: String,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
}

#[derive(Debug, Clone, Copy, Default)]
struct ButtonSliceSet {
    left: Option<u32>,
    middle: Option<u32>,
    right: Option<u32>,
}

impl ButtonSliceSet {
    fn has_any(self) -> bool {
        self.left.is_some() || self.middle.is_some() || self.right.is_some()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ButtonVisualSet {
    normal: ButtonSliceSet,
    hover: ButtonSliceSet,
    pressed: ButtonSliceSet,
    disabled: ButtonSliceSet,
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
    main_menu_panel: MainMenuPanel,
    pending_campaign: Option<CampaignSelection>,
    previous_main_menu_panel: MainMenuPanel,
    loading_progress: f32,
    loading_phase: String,

    // Game integration
    game_logic: Option<Arc<Mutex<GameLogic>>>,

    // Resource loading
    ui_textures: HashMap<String, u32>,
    button_visuals: HashMap<u32, ButtonVisualSet>,
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
            main_menu_panel: MainMenuPanel::Main,
            pending_campaign: None,
            previous_main_menu_panel: MainMenuPanel::Main,
            loading_progress: 0.0,
            loading_phase: "Loading assets...".to_string(),
            game_logic: None,
            ui_textures: HashMap::new(),
            button_visuals: HashMap::new(),
            ui_fonts: HashMap::new(),
        };

        // Initialize UI layouts
        ui_system.initialize_main_menu();
        ui_system.initialize_control_bar();
        ui_system.initialize_loading_background();
        ui_system.set_loading_progress(0.0, Some("Loading assets..."));

        Ok(ui_system)
    }

    fn initialize_loading_background(&mut self) {
        let candidates = [
            "Data/English/Art/Textures/loadpageuserinterface.tga",
            "Data/English/Art/Textures/Skirmish_Loaduserinterface.tga",
            "Art/Textures/mp_loaduserinterface_00b.tga",
            "Art/Textures/loadpageuserinterface.tga",
            "Art/Textures/Skirmish_Loaduserinterface.tga",
        ];

        for candidate in candidates {
            let Some(texture_id) = self.load_texture_from_virtual_path(candidate) else {
                continue;
            };

            if let Some(overlay_id) = self.loading_elements.get("LoadingOverlay").copied() {
                if let Some(overlay) = self.layout_manager.get_element_mut(overlay_id) {
                    overlay.texture_id = Some(texture_id);
                    overlay.background_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
                }
            }

            info!("Loaded loading background image from '{}'", candidate);
            return;
        }

        warn!("No loading background image found; using fallback solid color");
    }

    pub fn set_loading_progress(&mut self, progress: f32, phase: Option<&str>) {
        self.loading_progress = progress.clamp(0.0, 1.0);
        if let Some(phase) = phase.filter(|p| !p.trim().is_empty()) {
            self.loading_phase = phase.trim().to_string();
        }

        let label = format!(
            "{} {:.0}%",
            self.loading_phase,
            self.loading_progress * 100.0
        );
        if let Some(text_id) = self.loading_elements.get("LoadingText").copied() {
            if let Some(text) = self.layout_manager.get_element_mut(text_id) {
                text.text = label;
            }
        }

        let track_rect = self
            .loading_elements
            .get("LoadingProgressTrack")
            .and_then(|id| self.layout_manager.get_element(*id))
            .map(|track| track.rect);
        let fill_id = self.loading_elements.get("LoadingProgressFill").copied();
        if let (Some(track_rect), Some(fill_id)) = (track_rect, fill_id) {
            if let Some(fill) = self.layout_manager.get_element_mut(fill_id) {
                let inner_width = (track_rect.width - 4.0).max(0.0);
                fill.rect.x = track_rect.x + 2.0;
                fill.rect.y = track_rect.y + 2.0;
                fill.rect.height = (track_rect.height - 4.0).max(0.0);
                fill.rect.width = (inner_width * self.loading_progress).max(0.0);
            }
        }
    }

    /// Initialize main menu layout - matches C++ MainMenu.cpp
    fn initialize_main_menu(&mut self) {
        info!(
            "{}",
            localization::localize("wgpu_ui.log.init_main_menu", "Initializing Main Menu UI...")
        );
        self.main_menu_elements = self.layout_manager.create_main_menu_layout();
        self.apply_main_menu_shell_theme();
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
        // Drop transient pointer state across screen transitions so stale
        // press/hover data from a previous state cannot leak into the new UI.
        self.hover_element = None;
        self.active_element = None;
        self.mouse_pressed = false;
        self.click_times.clear();

        if new_state == UISystemState::MainMenu {
            self.main_menu_panel = MainMenuPanel::Main;
            self.pending_campaign = None;
            self.previous_main_menu_panel = MainMenuPanel::Main;
        }

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
        self.apply_main_menu_panel_visibility();
    }

    fn apply_main_menu_shell_theme(&mut self) {
        let normal_middle = self.load_shell_mapped_texture("Buttons-Middle");
        let normal_left = self.load_shell_mapped_texture("Buttons-Left");
        let normal_right = self.load_shell_mapped_texture("Buttons-Right");
        let hover_middle = self.load_shell_mapped_texture("Buttons-HiLite-Middle");
        let hover_left = self.load_shell_mapped_texture("Buttons-HiLite-Left");
        let hover_right = self.load_shell_mapped_texture("Buttons-HiLite-Right");
        let pressed_middle = self.load_shell_mapped_texture("Buttons-Pushed-Middle");
        let pressed_left = self.load_shell_mapped_texture("Buttons-Pushed-Left");
        let pressed_right = self.load_shell_mapped_texture("Buttons-Pushed-Right");
        let disabled_middle = self.load_shell_mapped_texture("Buttons-Disabled-Middle");
        let disabled_left = self.load_shell_mapped_texture("Buttons-Disabled-Left");
        let disabled_right = self.load_shell_mapped_texture("Buttons-Disabled-Right");
        info!(
            "MainMenu shell button textures: normal(L/M/R)={:?}/{:?}/{:?} hover(L/M/R)={:?}/{:?}/{:?} pressed(L/M/R)={:?}/{:?}/{:?} disabled(L/M/R)={:?}/{:?}/{:?}",
            normal_left,
            normal_middle,
            normal_right,
            hover_left,
            hover_middle,
            hover_right,
            pressed_left,
            pressed_middle,
            pressed_right,
            disabled_left,
            disabled_middle,
            disabled_right
        );
        let normal_slices = ButtonSliceSet {
            left: normal_left,
            middle: normal_middle,
            right: normal_right,
        };
        let hover_slices = ButtonSliceSet {
            left: hover_left,
            middle: hover_middle,
            right: hover_right,
        };
        let pressed_slices = ButtonSliceSet {
            left: pressed_left,
            middle: pressed_middle,
            right: pressed_right,
        };
        let disabled_slices = ButtonSliceSet {
            left: disabled_left,
            middle: disabled_middle,
            right: disabled_right,
        };

        let button_names = [
            "ButtonSinglePlayer",
            "ButtonMultiplayer",
            "ButtonLoadReplay",
            "ButtonOptions",
            "ButtonCredits",
            "ButtonExit",
            "ButtonUSA",
            "ButtonGLA",
            "ButtonChina",
            "ButtonChallenge",
            "ButtonSkirmish",
            "ButtonSingleBack",
            "ButtonOnline",
            "ButtonNetwork",
            "ButtonMultiBack",
            "ButtonLoadGame",
            "ButtonReplay",
            "ButtonLoadReplayBack",
            "ButtonEasy",
            "ButtonMedium",
            "ButtonHard",
            "ButtonDiffBack",
        ];

        for button_name in button_names {
            let Some(id) = self.main_menu_elements.get(button_name).copied() else {
                continue;
            };
            let Some(button) = self.layout_manager.get_element_mut(id) else {
                continue;
            };
            self.button_visuals.insert(
                id,
                ButtonVisualSet {
                    normal: normal_slices,
                    hover: hover_slices,
                    pressed: pressed_slices,
                    disabled: disabled_slices,
                },
            );
            button.texture_id = normal_middle;
            button.hover_texture_id = hover_middle;
            button.pressed_texture_id = pressed_middle;
            button.disabled_texture_id = disabled_middle;
            button.background_color = Vec4::new(0.0, 0.0, 0.0, 0.0);
            button.border_color = Vec4::new(0.0, 0.0, 0.0, 0.0);
            button.text_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
        }

        let backdrop_texture = self
            .load_shell_mapped_texture("MainMenuBackdrop")
            .or_else(|| {
                self.load_texture_from_candidates(&[
                    "Data/English/Art/Textures/TitleScreenuserinterface.tga",
                    "Art/Textures/TitleScreenuserinterface.tga",
                    "Data/English/Art/Textures/loadpageuserinterface.tga",
                    "Art/Textures/loadpageuserinterface.tga",
                ])
            });
        info!("MainMenu backdrop texture id: {:?}", backdrop_texture);
        if let Some(background_id) = self.main_menu_elements.get("MainMenuBackground").copied() {
            if let Some(background) = self.layout_manager.get_element_mut(background_id) {
                background.texture_id = backdrop_texture;
                background.background_color = if backdrop_texture.is_some() {
                    Vec4::new(1.0, 1.0, 1.0, 1.0)
                } else {
                    Vec4::new(0.0, 0.0, 0.0, 0.25)
                };
            }
        }

        let logo_texture = self.load_shell_mapped_texture("GeneralsLogo");
        info!("MainMenu logo texture id: {:?}", logo_texture);
        if let Some(title_id) = self.main_menu_elements.get("MainMenuTitle").copied() {
            if let Some(title) = self.layout_manager.get_element_mut(title_id) {
                title.texture_id = logo_texture;
                title.text.clear();
                title.background_color = Vec4::new(1.0, 1.0, 1.0, 1.0);
            }
        }

        let ruler_texture = self.load_texture_from_candidates(&[
            "Data/English/Art/Textures/mainmenuruleruserinterface.tga",
            "Art/Textures/mainmenuruleruserinterface.tga",
        ]);
        info!("MainMenu ruler texture id: {:?}", ruler_texture);
        if let Some(ruler_id) = self.main_menu_elements.get("MainMenuRuler").copied() {
            if let Some(ruler) = self.layout_manager.get_element_mut(ruler_id) {
                ruler.texture_id = ruler_texture;
                ruler.background_color = Vec4::new(1.0, 1.0, 1.0, 0.9);
            }
        }
    }

    fn mapped_image_ini_candidates() -> &'static [&'static str] {
        &[
            "Data/INI/MappedImages/TextureSize_512/SCSmShellUserInterface512.INI",
            "Data/INI/MappedImages/TextureSize_512/HandCreatedMappedImages.INI",
            "Data/INI/MappedImages/HandCreated/HandCreatedMappedImages.INI",
        ]
    }

    fn load_texture_from_virtual_path(&mut self, virtual_path: &str) -> Option<u32> {
        let data = Self::read_virtual_file_bytes(virtual_path)?;
        let decoded = Self::decode_image_from_bytes(virtual_path, &data)?;
        let rgba = decoded.to_rgba8();
        let (w, h) = rgba.dimensions();
        Some(self.renderer.load_texture(w, h, rgba.as_raw()))
    }

    fn read_virtual_file_bytes(path: &str) -> Option<Vec<u8>> {
        let fs = get_file_system();
        let mut guard = fs.lock().ok()?;
        let mut file = guard.open_file(path, FileAccess::READ.combine(FileAccess::BINARY))?;
        file.read_entire_and_close().ok()
    }

    fn read_virtual_file_string(path: &str) -> Option<String> {
        let bytes = Self::read_virtual_file_bytes(path)?;
        String::from_utf8(bytes).ok()
    }

    fn decode_image_from_bytes(path: &str, bytes: &[u8]) -> Option<image::DynamicImage> {
        let extension = std::path::Path::new(path)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());

        match extension.as_deref() {
            Some("tga") => image::load_from_memory_with_format(bytes, image::ImageFormat::Tga).ok(),
            Some("dds") => image::load_from_memory_with_format(bytes, image::ImageFormat::Dds).ok(),
            Some("png") => image::load_from_memory_with_format(bytes, image::ImageFormat::Png).ok(),
            Some("jpg") | Some("jpeg") => {
                image::load_from_memory_with_format(bytes, image::ImageFormat::Jpeg).ok()
            }
            Some("bmp") => image::load_from_memory_with_format(bytes, image::ImageFormat::Bmp).ok(),
            _ => image::load_from_memory(bytes).ok(),
        }
    }

    fn load_texture_from_candidates(&mut self, candidates: &[&str]) -> Option<u32> {
        for path in candidates {
            if let Some(texture_id) = self.load_texture_from_virtual_path(path) {
                return Some(texture_id);
            }
        }
        None
    }

    fn load_shell_mapped_texture(&mut self, mapped_name: &str) -> Option<u32> {
        if let Some(texture_id) = self.ui_textures.get(mapped_name).copied() {
            return Some(texture_id);
        }

        for ini_path in Self::mapped_image_ini_candidates() {
            let Some(def) = Self::parse_mapped_image(ini_path, mapped_name) else {
                continue;
            };

            let language = get_registry_language().as_str().to_string();
            let mut texture_paths = vec![
                format!("Data/{language}/Art/Textures/{}", def.texture),
                format!("Art/Textures/{}", def.texture),
            ];

            if !def.texture.to_ascii_lowercase().ends_with(".tga") {
                // Most mapped images are TGA; if extension is missing, try tga fallback.
                texture_paths.push(format!("Data/{language}/Art/Textures/{}.tga", def.texture));
                texture_paths.push(format!("Art/Textures/{}.tga", def.texture));
            }

            for texture_path in texture_paths {
                let Some(bytes) = Self::read_virtual_file_bytes(&texture_path) else {
                    continue;
                };
                let Some(decoded) = Self::decode_image_from_bytes(&texture_path, &bytes) else {
                    continue;
                };
                let rgba = decoded.to_rgba8();
                let width = def.right.saturating_sub(def.left).max(1);
                let height = def.bottom.saturating_sub(def.top).max(1);
                if def.left >= rgba.width() || def.top >= rgba.height() {
                    continue;
                }
                let crop_width = width.min(rgba.width().saturating_sub(def.left));
                let crop_height = height.min(rgba.height().saturating_sub(def.top));
                if crop_width == 0 || crop_height == 0 {
                    continue;
                }
                let cropped =
                    image::imageops::crop_imm(&rgba, def.left, def.top, crop_width, crop_height)
                        .to_image();
                let texture_id =
                    self.renderer
                        .load_texture(crop_width, crop_height, cropped.as_raw());
                self.ui_textures.insert(mapped_name.to_string(), texture_id);
                return Some(texture_id);
            }
        }

        if mapped_name.eq_ignore_ascii_case("MainMenuBackdrop") {
            if let Some(texture_id) = self.load_texture_from_candidates(&[
                "Data/English/Art/Textures/TitleScreenuserinterface.tga",
                "Art/Textures/TitleScreenuserinterface.tga",
            ]) {
                self.ui_textures.insert(mapped_name.to_string(), texture_id);
                return Some(texture_id);
            }
        }

        warn!("Missing mapped UI texture: {}", mapped_name);
        None
    }

    fn parse_mapped_image(path: &str, mapped_name: &str) -> Option<MappedImageDef> {
        let content = Self::read_virtual_file_string(path)?;
        let mut in_target = false;
        let mut texture: Option<String> = None;
        let mut left: Option<u32> = None;
        let mut top: Option<u32> = None;
        let mut right: Option<u32> = None;
        let mut bottom: Option<u32> = None;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }

            if let Some(name) = line.strip_prefix("MappedImage ") {
                in_target = name.trim().eq_ignore_ascii_case(mapped_name);
                texture = None;
                left = None;
                top = None;
                right = None;
                bottom = None;
                continue;
            }

            if !in_target {
                continue;
            }

            if let Some(value) = line.strip_prefix("Texture =") {
                texture = Some(value.trim().to_string());
                continue;
            }

            if let Some(value) = line.strip_prefix("Coords =") {
                let numbers: Vec<u32> = value
                    .split(|c: char| !c.is_ascii_digit())
                    .filter(|s| !s.is_empty())
                    .filter_map(|s| s.parse::<u32>().ok())
                    .collect();
                if numbers.len() >= 4 {
                    left = Some(numbers[0]);
                    top = Some(numbers[1]);
                    right = Some(numbers[2]);
                    bottom = Some(numbers[3]);
                }
                continue;
            }

            if line == "End" {
                if let (Some(texture), Some(left), Some(top), Some(right), Some(bottom)) =
                    (texture.clone(), left, top, right, bottom)
                {
                    return Some(MappedImageDef {
                        texture,
                        left,
                        top,
                        right,
                        bottom,
                    });
                }
                in_target = false;
            }
        }

        None
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

        let new_hover = self.find_interactive_element_at_position(x, y);
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
            if let Some(element_id) = self.find_interactive_element_at_position(x, y) {
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
    fn on_element_clicked(&mut self, element_id: u32) -> UISystemEvent {
        if let Some(element_name) = self
            .layout_manager
            .get_element(element_id)
            .map(|element| element.name.clone())
        {
            let element_id_str = element_id.to_string();
            info!(
                "{}",
                localization::localize_with_args(
                    "wgpu_ui.log.element_clicked",
                    "Element clicked: {name} (ID: {id})",
                    &[
                        ("name", element_name.as_str()),
                        ("id", element_id_str.as_str())
                    ],
                )
            );

            match self.current_state {
                UISystemState::MainMenu => self.handle_main_menu_click(&element_name),
                UISystemState::FactionSelection => self.handle_faction_click(&element_name),
                UISystemState::PauseMenu => self.handle_pause_click(&element_name),
                UISystemState::Victory => self.handle_victory_click(&element_name),
                UISystemState::InGame => self.handle_control_bar_click(&element_name),
                _ => UISystemEvent::None,
            }
        } else {
            UISystemEvent::None
        }
    }

    /// Handle main menu button clicks - matches C++ MainMenu callbacks.
    fn handle_main_menu_click(&mut self, element_name: &str) -> UISystemEvent {
        match element_name {
            "ButtonSinglePlayer" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.single_player", "Single Player clicked")
                );
                self.set_main_menu_panel(MainMenuPanel::SinglePlayer);
                UISystemEvent::None
            }
            "ButtonMultiplayer" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.network", "Multiplayer clicked")
                );
                self.set_main_menu_panel(MainMenuPanel::Multiplayer);
                UISystemEvent::None
            }
            "ButtonLoadReplay" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.load_replay", "Load/Replay clicked")
                );
                self.set_main_menu_panel(MainMenuPanel::LoadReplay);
                UISystemEvent::None
            }
            "ButtonOptions" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.options", "Options clicked")
                );
                UISystemEvent::ShowOptions
            }
            "ButtonCredits" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.credits", "Credits clicked")
                );
                UISystemEvent::ShowOptions
            }
            "ButtonExit" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.exit", "Exit clicked")
                );
                UISystemEvent::ExitGame
            }

            "ButtonSingleBack" => {
                self.pending_campaign = None;
                self.set_main_menu_panel(MainMenuPanel::Main);
                UISystemEvent::None
            }
            "ButtonSkirmish" => {
                info!(
                    "{}",
                    localization::localize("wgpu_ui.log.skirmish", "Skirmish clicked")
                );
                UISystemEvent::StartGame {
                    mode: GameMode::Skirmish,
                    faction: "USA".to_string(),
                }
            }
            "ButtonUSA" => self.open_difficulty_menu(CampaignSelection::USA),
            "ButtonGLA" => self.open_difficulty_menu(CampaignSelection::GLA),
            "ButtonChina" => self.open_difficulty_menu(CampaignSelection::China),
            "ButtonChallenge" => self.open_difficulty_menu(CampaignSelection::Challenge),

            "ButtonMultiBack" => {
                self.set_main_menu_panel(MainMenuPanel::Main);
                UISystemEvent::None
            }
            "ButtonOnline" | "ButtonNetwork" => UISystemEvent::StartGame {
                mode: GameMode::Multiplayer,
                faction: "USA".to_string(),
            },

            "ButtonLoadReplayBack" => {
                self.set_main_menu_panel(MainMenuPanel::Main);
                UISystemEvent::None
            }
            "ButtonLoadGame" | "ButtonReplay" => UISystemEvent::LoadGame,

            "ButtonDiffBack" => {
                self.set_main_menu_panel(self.previous_main_menu_panel);
                UISystemEvent::None
            }
            "ButtonEasy" | "ButtonMedium" | "ButtonHard" => {
                let faction = match self.pending_campaign.unwrap_or(CampaignSelection::USA) {
                    CampaignSelection::USA => "USA",
                    CampaignSelection::GLA => "GLA",
                    CampaignSelection::China => "China",
                    CampaignSelection::Challenge => "USA",
                };
                UISystemEvent::StartGame {
                    mode: GameMode::SinglePlayer,
                    faction: faction.to_string(),
                }
            }

            _ => UISystemEvent::None,
        }
    }

    /// Handle control bar clicks - matches C++ ControlBar callbacks
    fn handle_control_bar_click(&self, element_name: &str) -> UISystemEvent {
        // CommandButton* grid slots OR named Command_* residual buttons.
        if element_name.starts_with("CommandButton")
            || element_name.starts_with("Command_")
            || element_name.starts_with("command_")
        {
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

        // Selection centroid residual (guard / rally fallback).
        let mut center = glam::Vec3::ZERO;
        let mut count = 0.0f32;
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

        // 1) Named Command_* residual via shared mapper (Upgrade/Cancel/Stop/…).
        let mut command_type = crate::command_system::command_type_from_button_name(element_name);

        // 2) Legacy CommandButton grid slots → core RTS residual subset.
        if command_type.is_none() {
            command_type = match element_name {
                "CommandButton0_0" => Some(crate::command_system::CommandType::Stop),
                "CommandButton0_1" => Some(crate::command_system::CommandType::Guard {
                    target: crate::command_system::GuardTarget::Position(center),
                    mode: crate::game_logic::GuardMode::Normal,
                }),
                "CommandButton0_2" => Some(crate::command_system::CommandType::Scatter),
                "CommandButton0_3" => Some(crate::command_system::CommandType::Sell {
                    object_id: units[0],
                }),
                _ => None,
            };
        }

        let Some(mut command_type) = command_type else {
            return;
        };

        // Fill placeholders residual from selection / cursor world.
        match &mut command_type {
            crate::command_system::CommandType::DozerCancelConstruct { object_id }
            | crate::command_system::CommandType::Sell { object_id } => {
                *object_id = units[0];
            }
            crate::command_system::CommandType::Guard {
                target,
                mode: _mode,
            } => {
                if matches!(
                    target,
                    crate::command_system::GuardTarget::Position(p) if *p == glam::Vec3::ZERO
                ) {
                    *target = crate::command_system::GuardTarget::Position(center);
                }
            }
            crate::command_system::CommandType::SetRallyPoint { location } => {
                if *location == glam::Vec3::ZERO {
                    // Natural rally residual: forward of primary selection.
                    if let Some(obj) = game_logic.get_object(units[0]) {
                        let f = obj.thing.get_direction_vector();
                        *location = obj.get_position() + f * obj.selection_radius.max(10.0);
                    } else {
                        *location = center;
                    }
                }
            }
            crate::command_system::CommandType::AttackMoveTo { destination } => {
                if *destination == glam::Vec3::ZERO {
                    // Cursor world residual when available; else forward push.
                    if let Some(obj) = game_logic.get_object(units[0]) {
                        let f = obj.thing.get_direction_vector();
                        *destination = obj.get_position() + f * 50.0;
                    } else {
                        *destination = center;
                    }
                }
            }
            _ => {}
        }

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
            debug!(
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
            debug!(
                "{}",
                localization::localize_with_args(
                    "wgpu_ui.log.mouse_leave",
                    "Mouse left: {name}",
                    &[("name", element.name.as_str())],
                )
            );
        }
    }

    pub fn element_name_at_position(&self, x: f32, y: f32) -> Option<&str> {
        let id = self.layout_manager.find_element_at_position(x, y)?;
        self.layout_manager.get_element(id).map(|e| e.name.as_str())
    }

    pub fn interactive_element_name_at_position(&self, x: f32, y: f32) -> Option<&str> {
        let id = self.find_interactive_element_at_position(x, y)?;
        self.layout_manager.get_element(id).map(|e| e.name.as_str())
    }

    /// Render the UI - generates draw commands and renders them
    pub fn render(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let mut draw_commands = Vec::new();
        let now = Instant::now();
        let mut expired_clicks = Vec::new();

        let visible_elements = self.layout_manager.get_all_visible_elements();

        // Generate draw commands for all visible elements
        for element_id in visible_elements {
            if let Some(element) = self.layout_manager.get_element(element_id).cloned() {
                let mut rect = element.get_absolute_rect(&self.layout_manager);
                let scale = self.element_click_scale(element_id, now);
                if (scale - 1.0).abs() > 0.001 {
                    rect = scale_rect_from_center(rect, scale);
                }
                let mut bg_color = vec4_to_color(element.background_color);
                let mut texture_id = element.texture_id;
                let button_slices = self.current_button_slices(element_id, &element);
                let mut text_color = vec4_to_color(element.text_color);

                if !element.enabled {
                    texture_id = element.disabled_texture_id.or(texture_id);
                    text_color = Color {
                        r: 62.0 / 255.0,
                        g: 64.0 / 255.0,
                        b: 92.0 / 255.0,
                        a: 1.0,
                    };
                } else if Some(element_id) == self.active_element && self.mouse_pressed {
                    texture_id = element
                        .pressed_texture_id
                        .or(element.hover_texture_id)
                        .or(texture_id);
                    if texture_id.is_none() {
                        bg_color = Color::UI_BUTTON_PRESSED;
                    }
                } else if Some(element_id) == self.hover_element {
                    texture_id = element.hover_texture_id.or(texture_id);
                    text_color = Color {
                        r: 186.0 / 255.0,
                        g: 255.0 / 255.0,
                        b: 12.0 / 255.0,
                        a: 1.0,
                    };
                    if texture_id.is_none() {
                        bg_color = Color::UI_BUTTON_HOVER;
                    }
                }

                let bg_command =
                    self.renderer
                        .create_rect(rect.x, rect.y, rect.width, rect.height, bg_color);
                draw_commands.push(bg_command);

                if let Some(slices) = button_slices {
                    self.append_button_slice_draws(&mut draw_commands, rect, slices);
                } else if let Some(tex_id) = texture_id {
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
                    let default_x = rect.x + element.padding.x;
                    let default_y = rect.y + element.padding.y + element.font_size;
                    let (text_x, text_y) = match element.alignment {
                        super::layout_manager::Alignment::Center => {
                            let approx_text_width =
                                Self::estimate_text_width(&element.text, element.font_size);
                            (
                                rect.x + (rect.width - approx_text_width).max(0.0) * 0.5,
                                rect.y + (rect.height + element.font_size * 0.42) * 0.5,
                            )
                        }
                        super::layout_manager::Alignment::CenterLeft => (
                            rect.x + element.padding.x,
                            rect.y + (rect.height + element.font_size * 0.42) * 0.5,
                        ),
                        _ => (default_x, default_y),
                    };
                    let shadow_cmd = self.renderer.create_text(
                        &element.text,
                        text_x + 1.0,
                        text_y + 1.0,
                        element.font_size,
                        Color {
                            r: 0.0,
                            g: 0.0,
                            b: 0.0,
                            a: 0.7,
                        },
                    );
                    draw_commands.push(shadow_cmd);
                    let text_cmd = self.renderer.create_text(
                        &element.text,
                        text_x,
                        text_y,
                        element.font_size,
                        text_color,
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
        // Runtime state transitions are owned by CnCGameEngine (matching C++ top-level flow).
        // Keep this hook for per-frame HUD data refresh without implicit state flips.
    }

    pub fn get_current_state(&self) -> UISystemState {
        self.current_state
    }

    fn estimate_text_width(text: &str, font_size: f32) -> f32 {
        let pixel = (font_size.max(8.0) / 8.0).max(1.0);
        let advance = pixel * 8.0 + pixel;
        text.chars().take_while(|&ch| ch != '\n').count() as f32 * advance
    }

    fn current_button_slices(
        &self,
        element_id: u32,
        element: &super::layout_manager::UIElement,
    ) -> Option<ButtonSliceSet> {
        let visuals = self.button_visuals.get(&element_id)?;
        let slices = if !element.enabled {
            visuals.disabled
        } else if Some(element_id) == self.active_element && self.mouse_pressed {
            if visuals.pressed.has_any() {
                visuals.pressed
            } else {
                visuals.hover
            }
        } else if Some(element_id) == self.hover_element {
            visuals.hover
        } else {
            visuals.normal
        };
        slices.has_any().then_some(slices)
    }

    fn append_button_slice_draws(
        &self,
        draw_commands: &mut Vec<super::wgpu_renderer::UIDrawCommand>,
        rect: super::layout_manager::Rect,
        slices: ButtonSliceSet,
    ) {
        if !slices.has_any() || rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }

        let left_w = slices
            .left
            .and_then(|id| self.renderer.texture_size(id).map(|(w, _)| w as f32))
            .unwrap_or(0.0)
            .min(rect.width * 0.5);
        let right_w = slices
            .right
            .and_then(|id| self.renderer.texture_size(id).map(|(w, _)| w as f32))
            .unwrap_or(0.0)
            .min((rect.width - left_w).max(0.0));
        let mid_w = (rect.width - left_w - right_w).max(0.0);

        if let Some(left_id) = slices.left {
            draw_commands.push(self.renderer.create_textured_rect(
                rect.x,
                rect.y,
                left_w.max(1.0),
                rect.height,
                left_id,
            ));
        }

        if let Some(mid_id) = slices.middle {
            draw_commands.push(self.renderer.create_textured_rect(
                rect.x + left_w,
                rect.y,
                mid_w.max(1.0),
                rect.height,
                mid_id,
            ));
        } else if left_w <= 0.0 && right_w <= 0.0 {
            if let Some(left_or_right) = slices.left.or(slices.right) {
                draw_commands.push(self.renderer.create_textured_rect(
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                    left_or_right,
                ));
            }
        }

        if let Some(right_id) = slices.right {
            draw_commands.push(self.renderer.create_textured_rect(
                rect.x + rect.width - right_w,
                rect.y,
                right_w.max(1.0),
                rect.height,
                right_id,
            ));
        }
    }

    fn element_click_scale(&self, element_id: u32, now: Instant) -> f32 {
        if self.current_state == UISystemState::MainMenu {
            return 1.0;
        }

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

    fn element_is_interactive_in_current_state(&self, name: &str) -> bool {
        match self.current_state {
            UISystemState::MainMenu => matches!(
                name,
                "ButtonSinglePlayer"
                    | "ButtonMultiplayer"
                    | "ButtonLoadReplay"
                    | "ButtonOptions"
                    | "ButtonCredits"
                    | "ButtonExit"
                    | "ButtonSingleBack"
                    | "ButtonSkirmish"
                    | "ButtonUSA"
                    | "ButtonGLA"
                    | "ButtonChina"
                    | "ButtonChallenge"
                    | "ButtonMultiBack"
                    | "ButtonOnline"
                    | "ButtonNetwork"
                    | "ButtonLoadReplayBack"
                    | "ButtonLoadGame"
                    | "ButtonReplay"
                    | "ButtonDiffBack"
                    | "ButtonEasy"
                    | "ButtonMedium"
                    | "ButtonHard"
            ),
            UISystemState::FactionSelection => {
                name.starts_with("Faction") && name != "FactionBackground"
            }
            UISystemState::InGame => name == "Minimap" || name.starts_with("CommandButton"),
            UISystemState::PauseMenu => {
                matches!(name, "PauseResume" | "PauseOptions" | "PauseQuitToMenu")
            }
            UISystemState::Victory => name == "VictoryExit",
            UISystemState::Loading => false,
        }
    }

    fn find_interactive_element_at_position(&self, x: f32, y: f32) -> Option<u32> {
        let visible = self.layout_manager.get_all_visible_elements();
        for id in visible.into_iter().rev() {
            let Some(element) = self.layout_manager.get_element(id) else {
                continue;
            };
            if !element.enabled || !self.element_is_interactive_in_current_state(&element.name) {
                continue;
            }
            let rect = element.get_absolute_rect(&self.layout_manager);
            if rect.contains_point(x, y) {
                return Some(id);
            }
        }
        None
    }

    fn open_difficulty_menu(&mut self, selection: CampaignSelection) -> UISystemEvent {
        self.pending_campaign = Some(selection);
        self.previous_main_menu_panel = MainMenuPanel::SinglePlayer;
        self.set_main_menu_panel(MainMenuPanel::Difficulty);
        UISystemEvent::None
    }

    fn set_main_menu_panel(&mut self, panel: MainMenuPanel) {
        self.main_menu_panel = panel;
        self.apply_main_menu_panel_visibility();
    }

    fn apply_main_menu_panel_visibility(&mut self) {
        for &id in self.main_menu_elements.values() {
            self.layout_manager.set_element_visible(id, false);
        }

        let always_visible = ["MainMenuBackground", "MainMenuRuler", "MainMenuTitle"];
        for name in always_visible {
            if let Some(id) = self.main_menu_elements.get(name).copied() {
                self.layout_manager.set_element_visible(id, true);
            }
        }

        let active_names: &[&str] = match self.main_menu_panel {
            MainMenuPanel::Main => &[
                "MapBorder2",
                "ButtonSinglePlayer",
                "ButtonMultiplayer",
                "ButtonLoadReplay",
                "ButtonOptions",
                "ButtonCredits",
                "ButtonExit",
            ],
            MainMenuPanel::SinglePlayer => &[
                "MapBorder",
                "ButtonUSA",
                "ButtonGLA",
                "ButtonChina",
                "ButtonChallenge",
                "ButtonSkirmish",
                "ButtonSingleBack",
            ],
            MainMenuPanel::Multiplayer => &[
                "MapBorder1",
                "ButtonOnline",
                "ButtonNetwork",
                "ButtonMultiBack",
            ],
            MainMenuPanel::LoadReplay => &[
                "MapBorder3",
                "ButtonLoadGame",
                "ButtonReplay",
                "ButtonLoadReplayBack",
            ],
            MainMenuPanel::Difficulty => &[
                "MapBorder4",
                "StaticTextSelectDifficulty",
                "ButtonEasy",
                "ButtonMedium",
                "ButtonHard",
                "ButtonDiffBack",
            ],
        };

        for name in active_names {
            if let Some(id) = self.main_menu_elements.get(*name).copied() {
                self.layout_manager.set_element_visible(id, true);
            }
        }
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
