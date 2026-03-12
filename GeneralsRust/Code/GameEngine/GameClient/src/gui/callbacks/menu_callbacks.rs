//! Menu Callback Functions
//!
//! This module contains callback functions for all the shell menus,
//! including main menu, single player menu, options menu, etc.

use crate::game_text::GameText;
use crate::gui::campaign_manager::get_campaign_manager;
use crate::gui::gadgets::ComboBoxItem;
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::header_template::get_header_template_manager;
use crate::gui::{
    get_shell, with_window_manager, AnimationType, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled, WindowWidget,
};
use crate::map_util::{get_map_cache_manager, populate_map_listbox};
use game_engine::common::audio::AudioAffect as EngineAudioAffect;
use game_engine::common::global_data as runtime_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::user_preferences::UserPreferences;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::{TheAudio, TheGameLogic};
use gamelogic::system::game_logic::GAME_SINGLE_PLAYER;
use log::{debug, error, info, warn};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

/// Menu callback trait
pub trait MenuCallbacks {
    /// Initialize the menu
    fn init(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Update the menu (called every frame)
    fn update(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Shutdown the menu
    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Handle system messages
    fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled;

    /// Handle input messages
    fn input(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled;
}

/// Main Menu implementation
pub struct MainMenu {
    initialized: bool,
}

impl MainMenu {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for MainMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for MainMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Main Menu for layout: {}",
            layout.get_filename()
        );
        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        _layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Update main menu state
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Main Menu for layout: {}",
            layout.get_filename()
        );
        self.initialized = false;
        Ok(())
    }

    fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!("Main Menu system message: {:?}", msg);
        WindowMsgHandled::Ignored
    }

    fn input(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!("Main Menu input message: {:?}", msg);
        WindowMsgHandled::Ignored
    }
}

/// Single Player Menu implementation
pub struct SinglePlayerMenu {
    initialized: bool,
    parent_id: i32,
    button_new_id: i32,
    button_load_id: i32,
    button_back_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    is_shutting_down: bool,
    button_pushed: bool,
}

impl SinglePlayerMenu {
    pub fn new() -> Self {
        Self {
            initialized: false,
            parent_id: 0,
            button_new_id: 0,
            button_load_id: 0,
            button_back_id: 0,
            parent: None,
            is_shutting_down: false,
            button_pushed: false,
        }
    }

    fn shutdown_complete(&mut self, layout: &WindowLayout) {
        self.is_shutting_down = false;
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
    }
}

impl Default for SinglePlayerMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for SinglePlayerMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Single Player Menu for layout: {}",
            layout.get_filename()
        );
        self.parent_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
        self.button_new_id = NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
        self.button_load_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
        self.button_back_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
        self.button_pushed = false;
        self.is_shutting_down = false;

        get_shell().show_shell_map(true);
        layout.hide(false);

        with_window_manager(|manager| {
            self.parent = manager.get_window_by_id(self.parent_id);
            if let Some(parent) = self.parent.as_ref() {
                let _ = manager.set_focus(Some(parent));
            }
            if let Some(button_new) = manager.get_window_by_id(self.button_new_id) {
                get_shell().register_with_animate_manager(
                    button_new,
                    AnimationType::SlideLeft,
                    true,
                    1,
                );
            }
            if let Some(button_load) = manager.get_window_by_id(self.button_load_id) {
                get_shell().register_with_animate_manager(
                    button_load,
                    AnimationType::SlideLeft,
                    true,
                    200,
                );
            }
            if let Some(button_back) = manager.get_window_by_id(self.button_back_id) {
                get_shell().register_with_animate_manager(
                    button_back,
                    AnimationType::SlideRight,
                    true,
                    1,
                );
            }
        });

        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_shutting_down && get_shell().is_anim_finished() {
            self.shutdown_complete(layout);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Single Player Menu for layout: {}",
            layout.get_filename()
        );
        self.is_shutting_down = true;
        get_shell().reverse_animate_window();
        self.initialized = false;
        Ok(())
    }

    fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
            WindowMessage::InputFocus => WindowMsgHandled::Handled,
            WindowMessage::GadgetSelected => {
                if self.button_pushed {
                    return WindowMsgHandled::Handled;
                }

                let control_id = data1 as i32;
                if control_id == self.button_new_id {
                    let _ = get_shell().push("Menus/MapSelectMenu.wnd", false);
                    self.button_pushed = true;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_back_id {
                    let _ = get_shell().pop();
                    self.button_pushed = true;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_load_id {
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Ignored
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Char || self.button_pushed || data1 != 0x1B {
            return WindowMsgHandled::Ignored;
        }
        if (data2 & 0x0001) == 0 {
            return WindowMsgHandled::Handled;
        }

        if let Some(parent) = self.parent.as_ref() {
            let _ = parent.borrow_mut().send_system_message(
                WindowMessage::GadgetSelected,
                self.button_back_id as u32,
                self.button_back_id as u32,
            );
        }

        WindowMsgHandled::Handled
    }
}

/// Options Menu implementation
pub struct OptionsMenu {
    initialized: bool,
    ignore_selected: bool,
    parent_id: i32,
    button_back_id: i32,
    button_defaults_id: i32,
    button_accept_id: i32,
    button_keyboard_options_id: i32,
    button_advanced_accept_id: i32,
    button_advanced_back_id: i32,
    combo_anti_aliasing_id: i32,
    combo_resolution_id: i32,
    combo_detail_id: i32,
    check_alternate_mouse_id: i32,
    check_retaliation_id: i32,
    check_double_click_attack_move_id: i32,
    check_language_filter_id: i32,
    check_send_delay_id: i32,
    check_use_camera_id: i32,
    check_save_camera_id: i32,
    check_draw_anchor_id: i32,
    check_move_anchor_id: i32,
    advanced_window_id: i32,
    check_3d_shadows_id: i32,
    check_2d_shadows_id: i32,
    check_cloud_shadows_id: i32,
    check_ground_lighting_id: i32,
    check_smooth_water_id: i32,
    check_building_occlusion_id: i32,
    check_props_id: i32,
    check_extra_animations_id: i32,
    check_no_dynamic_lod_id: i32,
    check_unlock_fps_id: i32,
    check_heat_effects_id: i32,
    slider_scroll_speed_id: i32,
    slider_music_volume_id: i32,
    slider_sfx_volume_id: i32,
    slider_voice_volume_id: i32,
    slider_gamma_id: i32,
    slider_texture_resolution_id: i32,
    slider_particle_cap_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    resolution_modes: Vec<(i32, i32)>,
    initial_detail_index: usize,
}

impl OptionsMenu {
    pub fn new() -> Self {
        Self {
            initialized: false,
            ignore_selected: false,
            parent_id: 0,
            button_back_id: 0,
            button_defaults_id: 0,
            button_accept_id: 0,
            button_keyboard_options_id: 0,
            button_advanced_accept_id: 0,
            button_advanced_back_id: 0,
            combo_anti_aliasing_id: 0,
            combo_resolution_id: 0,
            combo_detail_id: 0,
            check_alternate_mouse_id: 0,
            check_retaliation_id: 0,
            check_double_click_attack_move_id: 0,
            check_language_filter_id: 0,
            check_send_delay_id: 0,
            check_use_camera_id: 0,
            check_save_camera_id: 0,
            check_draw_anchor_id: 0,
            check_move_anchor_id: 0,
            advanced_window_id: 0,
            check_3d_shadows_id: 0,
            check_2d_shadows_id: 0,
            check_cloud_shadows_id: 0,
            check_ground_lighting_id: 0,
            check_smooth_water_id: 0,
            check_building_occlusion_id: 0,
            check_props_id: 0,
            check_extra_animations_id: 0,
            check_no_dynamic_lod_id: 0,
            check_unlock_fps_id: 0,
            check_heat_effects_id: 0,
            slider_scroll_speed_id: 0,
            slider_music_volume_id: 0,
            slider_sfx_volume_id: 0,
            slider_voice_volume_id: 0,
            slider_gamma_id: 0,
            slider_texture_resolution_id: 0,
            slider_particle_cap_id: 0,
            parent: None,
            resolution_modes: Vec::new(),
            initial_detail_index: 1,
        }
    }

    fn name_to_id(name: &str) -> i32 {
        NameKeyGenerator::name_to_key(name) as i32
    }

    fn find_window(id: i32) -> Option<Rc<RefCell<GameWindow>>> {
        with_window_manager(|manager| manager.get_window_by_id(id))
    }

    fn set_checkbox(id: i32, value: bool) {
        if let Some(window) = Self::find_window(id) {
            if let Some(check_box) = window.borrow_mut().check_box_mut() {
                check_box.set_checked(value);
            }
        }
    }

    fn checkbox_value(id: i32) -> bool {
        Self::find_window(id)
            .and_then(|window| {
                let guard = window.borrow();
                match guard.widget() {
                    Some(WindowWidget::CheckBox(check_box)) => Some(check_box.is_checked()),
                    _ => None,
                }
            })
            .unwrap_or(false)
    }

    fn set_slider_value(id: i32, value: i32) {
        if let Some(window) = Self::find_window(id) {
            if let Some(slider) = window.borrow_mut().horizontal_slider_mut() {
                slider.set_value(value);
            }
        }
    }

    fn set_slider_range_and_value(id: i32, min_value: i32, max_value: i32, value: i32) {
        if let Some(window) = Self::find_window(id) {
            if let Some(slider) = window.borrow_mut().horizontal_slider_mut() {
                slider.set_range(min_value, max_value);
                slider.set_value(value);
            }
        }
    }

    fn slider_value(id: i32) -> i32 {
        Self::find_window(id)
            .and_then(|window| {
                let guard = window.borrow();
                match guard.widget() {
                    Some(WindowWidget::HorizontalSlider(slider)) => Some(slider.value()),
                    _ => None,
                }
            })
            .unwrap_or(0)
    }

    fn set_combo_items(id: i32, items: &[String], selected_index: usize) {
        if items.is_empty() {
            return;
        }
        if let Some(window) = Self::find_window(id) {
            let mut guard = window.borrow_mut();
            let Some(combo_box) = guard.combo_box_mut() else {
                return;
            };
            combo_box.clear();
            for (index, item) in items.iter().enumerate() {
                combo_box.add_item(ComboBoxItem::new(index as u32, item.clone()));
            }
            guard.set_combo_box_selected(selected_index.min(items.len() - 1), true);
        }
    }

    fn combo_selected_index(id: i32) -> Option<usize> {
        Self::find_window(id).and_then(|window| {
            let guard = window.borrow();
            match guard.widget() {
                Some(WindowWidget::ComboBox(combo_box)) => combo_box.selected_index(),
                _ => None,
            }
        })
    }

    fn set_combo_selected(id: i32, index: usize) {
        if let Some(window) = Self::find_window(id) {
            window.borrow_mut().set_combo_box_selected(index, true);
        }
    }

    fn set_window_hidden(id: i32, hidden: bool) {
        if let Some(window) = Self::find_window(id) {
            let _ = window.borrow_mut().hide(hidden);
        }
    }

    fn set_window_enabled(id: i32, enabled: bool) {
        if let Some(window) = Self::find_window(id) {
            let _ = window.borrow_mut().enable(enabled);
        }
    }

    fn detail_index_from_name(value: &str) -> usize {
        match value.trim().to_ascii_lowercase().as_str() {
            "high" => 0,
            "medium" => 1,
            "low" => 2,
            "custom" => 3,
            _ => 1,
        }
    }

    fn detail_name_from_index(index: usize) -> &'static str {
        match index {
            0 => "High",
            1 => "Medium",
            2 => "Low",
            3 => "Custom",
            _ => "Medium",
        }
    }

    fn detail_labels() -> Vec<String> {
        vec![
            GameText::fetch("GUI:High"),
            GameText::fetch("GUI:Medium"),
            GameText::fetch("GUI:Low"),
            GameText::fetch("GUI:Custom"),
        ]
    }

    fn anti_alias_labels() -> Vec<String> {
        (0..3)
            .map(|index| GameText::fetch(&format!("GUI:AntiAliasing{index}")))
            .collect()
    }

    fn resolution_label(mode: (i32, i32)) -> String {
        format!("{} x {}", mode.0, mode.1)
    }

    fn slider_to_gamma(slider_value: i32) -> f32 {
        if slider_value < 50 {
            if slider_value <= 0 {
                0.6
            } else {
                1.0 - (0.4 * (50 - slider_value) as f32 / 50.0)
            }
        } else if slider_value > 50 {
            1.0 + (1.0 * (slider_value - 50) as f32 / 50.0)
        } else {
            1.0
        }
    }

    fn set_yes_no(pref: &mut UserPreferences, key: &str, value: bool) {
        pref.set_string(key, if value { "yes" } else { "no" }.to_string());
    }

    fn set_yes_no_title(pref: &mut UserPreferences, key: &str, value: bool) {
        pref.set_string(key, if value { "Yes" } else { "No" }.to_string());
    }

    fn load_resolution_modes(&mut self) {
        self.resolution_modes = vec![
            (800, 600),
            (1024, 768),
            (1152, 864),
            (1280, 720),
            (1280, 768),
            (1280, 800),
            (1280, 960),
            (1280, 1024),
            (1360, 768),
            (1366, 768),
            (1440, 900),
            (1600, 900),
            (1600, 1200),
            (1680, 1050),
            (1920, 1080),
            (1920, 1200),
        ];
        if let Ok(global) = runtime_global_data::read_safe() {
            let current = (global.writable.x_resolution, global.writable.y_resolution);
            if current.0 > 0 && current.1 > 0 && !self.resolution_modes.contains(&current) {
                self.resolution_modes.push(current);
                self.resolution_modes.sort_unstable();
            }
        }
    }

    fn populate_controls(&mut self) {
        self.load_resolution_modes();

        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");

        let global = runtime_global_data::read();

        let alternate_mouse = pref.get_bool_or("UseAlternateMouse", global.use_alternate_mouse);
        let retaliation = pref.get_bool_or("Retaliation", global.client_retaliation_mode_enabled);
        let double_click_attack_move =
            pref.get_bool_or("UseDoubleClickAttackMove", global.double_click_attack_move);
        let language_filter = pref.get_bool_or("LanguageFilter", global.language_filter_pref);
        let send_delay = pref.get_bool_or("SendDelay", global.firewall_send_delay);
        let save_camera = pref.get_bool_or("SaveCameraInReplays", true);
        let use_camera = pref.get_bool_or("UseCameraInReplays", true);
        let draw_anchor = pref
            .get_string("DrawScrollAnchor")
            .map(|value| value.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);
        let move_anchor = pref
            .get_string("MoveScrollAnchor")
            .map(|value| value.eq_ignore_ascii_case("yes"))
            .unwrap_or(false);

        let music_volume = pref
            .get_int("MusicVolume")
            .unwrap_or((global.music_volume_factor * 100.0) as i32);
        let sfx_volume = pref.get_int("SFXVolume").unwrap_or_else(|| {
            ((global.sfx_volume_factor.max(global.voice_volume_factor)) * 100.0) as i32
        });
        let voice_volume = pref
            .get_int("VoiceVolume")
            .unwrap_or((global.voice_volume_factor * 100.0) as i32);
        let gamma = pref.get_int_or("Gamma", 50);
        let scroll_speed = pref.get_int_or(
            "ScrollFactor",
            (global.keyboard_scroll_factor * 100.0) as i32,
        );
        let anti_aliasing = pref
            .get_int_or("AntiAliasing", global.anti_alias_box_value)
            .clamp(0, 2) as usize;
        let detail_name = pref.get_string_or(
            "StaticGameLOD",
            &game_engine::common::game_lod::get_static_lod(),
        );
        let detail_index = Self::detail_index_from_name(&detail_name);
        self.initial_detail_index = detail_index;

        let texture_reduction = pref
            .get_int("TextureReduction")
            .unwrap_or(global.texture_reduction_factor)
            .clamp(0, 2);
        let texture_resolution = 2 - texture_reduction;
        let particle_cap = pref
            .get_int("MaxParticleCount")
            .unwrap_or(global.max_particle_count)
            .max(100);

        let use_shadow_volumes =
            pref.get_bool_or("UseShadowVolumes", global.writable.use_shadow_volumes);
        let use_shadow_decals =
            pref.get_bool_or("UseShadowDecals", global.writable.use_shadow_decals);
        let use_cloud_map = pref.get_bool_or("UseCloudMap", global.use_cloud_map);
        let use_light_map = pref.get_bool_or("UseLightMap", global.use_light_map);
        let show_soft_water_edge =
            pref.get_bool_or("ShowSoftWaterEdge", global.show_soft_water_edge);
        let extra_animations = pref
            .get_bool("ExtraAnimations")
            .unwrap_or(!global.use_draw_module_lod);
        let no_dynamic_lod = !pref
            .get_bool("DynamicLOD")
            .unwrap_or(global.writable.enable_dynamic_lod);
        let unlock_fps = !pref
            .get_bool("FPSLimit")
            .unwrap_or(global.writable.use_fps_limit);
        let heat_effects = pref.get_bool_or("HeatEffects", global.use_heat_effects);
        let building_occlusion =
            pref.get_bool_or("BuildingOcclusion", global.enable_behind_building_markers);
        let show_props = pref.get_bool_or("ShowTrees", global.use_trees);

        drop(global);

        let resolution_pref = pref.get_string_or("Resolution", "");
        let resolution = {
            let mut parts = resolution_pref.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(width), Some(height)) => {
                    match (width.parse::<i32>(), height.parse::<i32>()) {
                        (Ok(width), Ok(height)) => (width, height),
                        _ => {
                            let global = runtime_global_data::read();
                            (global.writable.x_resolution, global.writable.y_resolution)
                        }
                    }
                }
                _ => {
                    let global = runtime_global_data::read();
                    (global.writable.x_resolution, global.writable.y_resolution)
                }
            }
        };
        let resolution_index = self
            .resolution_modes
            .iter()
            .position(|mode| *mode == resolution)
            .unwrap_or(0);

        Self::set_combo_items(
            self.combo_anti_aliasing_id,
            &Self::anti_alias_labels(),
            anti_aliasing,
        );
        Self::set_combo_items(
            self.combo_resolution_id,
            &self
                .resolution_modes
                .iter()
                .map(|mode| Self::resolution_label(*mode))
                .collect::<Vec<_>>(),
            resolution_index,
        );
        Self::set_combo_items(self.combo_detail_id, &Self::detail_labels(), detail_index);

        Self::set_checkbox(self.check_alternate_mouse_id, alternate_mouse);
        Self::set_checkbox(self.check_retaliation_id, retaliation);
        Self::set_checkbox(
            self.check_double_click_attack_move_id,
            double_click_attack_move,
        );
        Self::set_checkbox(self.check_language_filter_id, language_filter);
        Self::set_checkbox(self.check_send_delay_id, send_delay);
        Self::set_checkbox(self.check_save_camera_id, save_camera);
        Self::set_checkbox(self.check_use_camera_id, use_camera);
        Self::set_checkbox(self.check_draw_anchor_id, draw_anchor);
        Self::set_checkbox(self.check_move_anchor_id, move_anchor);
        Self::set_checkbox(self.check_3d_shadows_id, use_shadow_volumes);
        Self::set_checkbox(self.check_2d_shadows_id, use_shadow_decals);
        Self::set_checkbox(self.check_cloud_shadows_id, use_cloud_map);
        Self::set_checkbox(self.check_ground_lighting_id, use_light_map);
        Self::set_checkbox(self.check_smooth_water_id, show_soft_water_edge);
        Self::set_checkbox(self.check_extra_animations_id, extra_animations);
        Self::set_checkbox(self.check_no_dynamic_lod_id, no_dynamic_lod);
        Self::set_checkbox(self.check_unlock_fps_id, unlock_fps);
        Self::set_checkbox(self.check_heat_effects_id, heat_effects);
        Self::set_checkbox(self.check_building_occlusion_id, building_occlusion);
        Self::set_checkbox(self.check_props_id, show_props);

        Self::set_slider_range_and_value(self.slider_scroll_speed_id, 0, 100, scroll_speed);
        Self::set_slider_range_and_value(self.slider_music_volume_id, 0, 100, music_volume);
        Self::set_slider_range_and_value(self.slider_sfx_volume_id, 0, 100, sfx_volume);
        Self::set_slider_range_and_value(self.slider_voice_volume_id, 0, 100, voice_volume);
        Self::set_slider_range_and_value(self.slider_gamma_id, 0, 100, gamma);
        Self::set_slider_range_and_value(
            self.slider_texture_resolution_id,
            0,
            2,
            texture_resolution,
        );
        Self::set_slider_range_and_value(self.slider_particle_cap_id, 100, 10000, particle_cap);

        Self::set_window_hidden(self.advanced_window_id, true);
    }

    fn apply_default_controls(&mut self) {
        Self::set_checkbox(self.check_language_filter_id, true);
        Self::set_checkbox(self.check_send_delay_id, false);
        Self::set_checkbox(self.check_alternate_mouse_id, false);
        Self::set_checkbox(self.check_retaliation_id, true);
        Self::set_checkbox(self.check_double_click_attack_move_id, false);
        Self::set_checkbox(self.check_save_camera_id, true);
        Self::set_checkbox(self.check_use_camera_id, true);
        Self::set_checkbox(self.check_draw_anchor_id, false);
        Self::set_checkbox(self.check_move_anchor_id, false);

        Self::set_slider_value(self.slider_scroll_speed_id, 50);
        Self::set_slider_value(self.slider_music_volume_id, 60);
        Self::set_slider_value(self.slider_sfx_volume_id, 55);
        Self::set_slider_value(self.slider_voice_volume_id, 70);
        Self::set_slider_value(self.slider_gamma_id, 50);
        Self::set_slider_value(self.slider_texture_resolution_id, 2);
        Self::set_slider_value(self.slider_particle_cap_id, 5000);

        Self::set_checkbox(self.check_3d_shadows_id, true);
        Self::set_checkbox(self.check_2d_shadows_id, true);
        Self::set_checkbox(self.check_cloud_shadows_id, true);
        Self::set_checkbox(self.check_ground_lighting_id, true);
        Self::set_checkbox(self.check_smooth_water_id, true);
        Self::set_checkbox(self.check_extra_animations_id, true);
        Self::set_checkbox(self.check_no_dynamic_lod_id, false);
        Self::set_checkbox(self.check_unlock_fps_id, false);
        Self::set_checkbox(self.check_heat_effects_id, true);
        Self::set_checkbox(self.check_building_occlusion_id, true);
        Self::set_checkbox(self.check_props_id, true);
        Self::set_combo_selected(self.combo_detail_id, self.initial_detail_index);
        Self::set_window_hidden(self.advanced_window_id, true);
    }

    fn apply_options(&mut self) {
        let detail_index = Self::combo_selected_index(self.combo_detail_id).unwrap_or(1);
        let anti_aliasing =
            Self::combo_selected_index(self.combo_anti_aliasing_id).unwrap_or(0) as i32;
        let resolution = self
            .resolution_modes
            .get(Self::combo_selected_index(self.combo_resolution_id).unwrap_or(0))
            .copied()
            .unwrap_or((1024, 768));

        let alternate_mouse = Self::checkbox_value(self.check_alternate_mouse_id);
        let retaliation = Self::checkbox_value(self.check_retaliation_id);
        let double_click_attack_move = Self::checkbox_value(self.check_double_click_attack_move_id);
        let language_filter = Self::checkbox_value(self.check_language_filter_id);
        let send_delay = Self::checkbox_value(self.check_send_delay_id);
        let save_camera = Self::checkbox_value(self.check_save_camera_id);
        let use_camera = Self::checkbox_value(self.check_use_camera_id);
        let draw_anchor = Self::checkbox_value(self.check_draw_anchor_id);
        let move_anchor = Self::checkbox_value(self.check_move_anchor_id);
        let use_shadow_volumes = Self::checkbox_value(self.check_3d_shadows_id);
        let use_shadow_decals = Self::checkbox_value(self.check_2d_shadows_id);
        let use_cloud_map = Self::checkbox_value(self.check_cloud_shadows_id);
        let use_light_map = Self::checkbox_value(self.check_ground_lighting_id);
        let show_soft_water_edge = Self::checkbox_value(self.check_smooth_water_id);
        let extra_animations = Self::checkbox_value(self.check_extra_animations_id);
        let no_dynamic_lod = Self::checkbox_value(self.check_no_dynamic_lod_id);
        let unlock_fps = Self::checkbox_value(self.check_unlock_fps_id);
        let heat_effects = Self::checkbox_value(self.check_heat_effects_id);
        let building_occlusion = Self::checkbox_value(self.check_building_occlusion_id);
        let show_props = Self::checkbox_value(self.check_props_id);

        let scroll_speed = Self::slider_value(self.slider_scroll_speed_id).clamp(0, 100);
        let music_volume = Self::slider_value(self.slider_music_volume_id).clamp(0, 100);
        let sfx_volume = Self::slider_value(self.slider_sfx_volume_id).clamp(0, 100);
        let voice_volume = Self::slider_value(self.slider_voice_volume_id).clamp(0, 100);
        let gamma_slider = Self::slider_value(self.slider_gamma_id).clamp(0, 100);
        let texture_resolution = Self::slider_value(self.slider_texture_resolution_id).clamp(0, 2);
        let particle_cap = Self::slider_value(self.slider_particle_cap_id).max(100);
        let texture_reduction = 2 - texture_resolution;
        let detail_name = Self::detail_name_from_index(detail_index);

        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");
        pref.set_string("Resolution", format!("{} {}", resolution.0, resolution.1));
        pref.set_int("AntiAliasing", anti_aliasing);
        pref.set_string("StaticGameLOD", detail_name.to_string());
        pref.set_int("TextureReduction", texture_reduction);
        pref.set_int("MaxParticleCount", particle_cap);
        pref.set_int("ScrollFactor", scroll_speed);
        pref.set_int("MusicVolume", music_volume);
        pref.set_int("SFXVolume", sfx_volume);
        pref.set_int("SFX3DVolume", sfx_volume);
        pref.set_int("VoiceVolume", voice_volume);
        pref.set_int("Gamma", gamma_slider);
        pref.set_string(
            "LanguageFilter",
            if language_filter { "true" } else { "false" }.to_string(),
        );
        Self::set_yes_no(&mut pref, "SendDelay", send_delay);
        Self::set_yes_no(&mut pref, "UseAlternateMouse", alternate_mouse);
        Self::set_yes_no(&mut pref, "Retaliation", retaliation);
        Self::set_yes_no(
            &mut pref,
            "UseDoubleClickAttackMove",
            double_click_attack_move,
        );
        Self::set_yes_no(&mut pref, "SaveCameraInReplays", save_camera);
        Self::set_yes_no(&mut pref, "UseCameraInReplays", use_camera);
        Self::set_yes_no_title(&mut pref, "DrawScrollAnchor", draw_anchor);
        Self::set_yes_no_title(&mut pref, "MoveScrollAnchor", move_anchor);
        Self::set_yes_no(&mut pref, "UseShadowVolumes", use_shadow_volumes);
        Self::set_yes_no(&mut pref, "UseShadowDecals", use_shadow_decals);
        Self::set_yes_no(&mut pref, "UseCloudMap", use_cloud_map);
        Self::set_yes_no(&mut pref, "UseLightMap", use_light_map);
        Self::set_yes_no(&mut pref, "ShowSoftWaterEdge", show_soft_water_edge);
        Self::set_yes_no(&mut pref, "ExtraAnimations", extra_animations);
        Self::set_yes_no(&mut pref, "DynamicLOD", !no_dynamic_lod);
        Self::set_yes_no(&mut pref, "FPSLimit", !unlock_fps);
        Self::set_yes_no(&mut pref, "HeatEffects", heat_effects);
        Self::set_yes_no(&mut pref, "BuildingOcclusion", building_occlusion);
        Self::set_yes_no(&mut pref, "ShowTrees", show_props);
        let _ = pref.write();

        {
            let mut global = runtime_global_data::write();
            global.use_alternate_mouse = alternate_mouse;
            global.client_retaliation_mode_enabled = retaliation;
            global.double_click_attack_move = double_click_attack_move;
            global.language_filter_pref = language_filter;
            global.firewall_send_delay = send_delay;
            global.save_camera_in_replay = save_camera;
            global.use_camera_in_replay = use_camera;
            global.use_cloud_map = use_cloud_map;
            global.use_light_map = use_light_map;
            global.show_soft_water_edge = show_soft_water_edge;
            global.use_draw_module_lod = !extra_animations;
            global.use_heat_effects = heat_effects;
            global.enable_behind_building_markers = building_occlusion;
            global.use_trees = show_props;
            global.texture_reduction_factor = texture_reduction;
            global.max_particle_count = particle_cap;
            global.display_gamma = Self::slider_to_gamma(gamma_slider);
            global.anti_alias_box_value = anti_aliasing;
            global.keyboard_scroll_factor = scroll_speed as f32 / 100.0;
            global.music_volume_factor = music_volume as f32 / 100.0;
            global.sfx_volume_factor = sfx_volume as f32 / 100.0;
            global.voice_volume_factor = voice_volume as f32 / 100.0;
            global.writable.x_resolution = resolution.0;
            global.writable.y_resolution = resolution.1;
            global.writable.use_shadow_volumes = use_shadow_volumes;
            global.writable.use_shadow_decals = use_shadow_decals;
            global.writable.enable_dynamic_lod = !no_dynamic_lod;
            global.writable.use_fps_limit = !unlock_fps;
        }

        game_engine::common::game_lod::set_static_lod_from_string(detail_name);
        game_engine::common::game_lod::set_ideal_static_lod_from_string(detail_name);

        let audio = TheAudio;
        audio.set_volume(music_volume as f32 / 100.0, EngineAudioAffect::Music);
        audio.set_volume(sfx_volume as f32 / 100.0, EngineAudioAffect::Sound);
        audio.set_volume(sfx_volume as f32 / 100.0, EngineAudioAffect::Sound3D);
        audio.set_volume(voice_volume as f32 / 100.0, EngineAudioAffect::Speech);
        get_header_template_manager().header_notify_resolution_change();
    }

    fn close_menu(&mut self) {
        Self::set_window_hidden(self.advanced_window_id, true);
        let _ = get_shell().pop();
    }

    fn init_ids(&mut self) {
        self.parent_id = Self::name_to_id("OptionsMenu.wnd:OptionsMenuParent");
        self.button_back_id = Self::name_to_id("OptionsMenu.wnd:ButtonBack");
        self.button_defaults_id = Self::name_to_id("OptionsMenu.wnd:ButtonDefaults");
        self.button_accept_id = Self::name_to_id("OptionsMenu.wnd:ButtonAccept");
        self.button_keyboard_options_id = Self::name_to_id("OptionsMenu.wnd:ButtonKeyboardOptions");
        self.button_advanced_accept_id = Self::name_to_id("OptionsMenu.wnd:ButtonAdvanceAccept");
        self.button_advanced_back_id = Self::name_to_id("OptionsMenu.wnd:ButtonAdvanceBack");
        self.combo_anti_aliasing_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxAntiAliasing");
        self.combo_resolution_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxResolution");
        self.combo_detail_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxDetail");
        self.check_alternate_mouse_id = Self::name_to_id("OptionsMenu.wnd:CheckAlternateMouse");
        self.check_retaliation_id = Self::name_to_id("OptionsMenu.wnd:Retaliation");
        self.check_double_click_attack_move_id =
            Self::name_to_id("OptionsMenu.wnd:CheckDoubleClickAttackMove");
        self.check_language_filter_id = Self::name_to_id("OptionsMenu.wnd:CheckLanguageFilter");
        self.check_send_delay_id = Self::name_to_id("OptionsMenu.wnd:CheckSendDelay");
        self.check_use_camera_id = Self::name_to_id("OptionsMenu.wnd:CheckBoxUseCamera");
        self.check_save_camera_id = Self::name_to_id("OptionsMenu.wnd:CheckBoxSaveCamera");
        self.check_draw_anchor_id = Self::name_to_id("OptionsMenu.wnd:CheckBoxDrawAnchor");
        self.check_move_anchor_id = Self::name_to_id("OptionsMenu.wnd:CheckBoxMoveAnchor");
        self.advanced_window_id = Self::name_to_id("OptionsMenu.wnd:WinAdvancedDisplayOptions");
        self.check_3d_shadows_id = Self::name_to_id("OptionsMenu.wnd:Check3DShadows");
        self.check_2d_shadows_id = Self::name_to_id("OptionsMenu.wnd:Check2DShadows");
        self.check_cloud_shadows_id = Self::name_to_id("OptionsMenu.wnd:CheckCloudShadows");
        self.check_ground_lighting_id = Self::name_to_id("OptionsMenu.wnd:CheckGroundLighting");
        self.check_smooth_water_id = Self::name_to_id("OptionsMenu.wnd:CheckSmoothWater");
        self.check_building_occlusion_id = Self::name_to_id("OptionsMenu.wnd:CheckBehindBuilding");
        self.check_props_id = Self::name_to_id("OptionsMenu.wnd:CheckShowProps");
        self.check_extra_animations_id = Self::name_to_id("OptionsMenu.wnd:CheckExtraAnimations");
        self.check_no_dynamic_lod_id = Self::name_to_id("OptionsMenu.wnd:CheckNoDynamicLOD");
        self.check_unlock_fps_id = Self::name_to_id("OptionsMenu.wnd:CheckUnlockFPS");
        self.check_heat_effects_id = Self::name_to_id("OptionsMenu.wnd:CheckHeatEffects");
        self.slider_scroll_speed_id = Self::name_to_id("OptionsMenu.wnd:SliderScrollSpeed");
        self.slider_music_volume_id = Self::name_to_id("OptionsMenu.wnd:SliderMusicVolume");
        self.slider_sfx_volume_id = Self::name_to_id("OptionsMenu.wnd:SliderSFXVolume");
        self.slider_voice_volume_id = Self::name_to_id("OptionsMenu.wnd:SliderVoiceVolume");
        self.slider_gamma_id = Self::name_to_id("OptionsMenu.wnd:SliderGamma");
        self.slider_texture_resolution_id = Self::name_to_id("OptionsMenu.wnd:LowResSlider");
        self.slider_particle_cap_id = Self::name_to_id("OptionsMenu.wnd:ParticleCapSlider");
    }
}

impl Default for OptionsMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for OptionsMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Options Menu for layout: {}",
            layout.get_filename()
        );
        self.init_ids();
        self.ignore_selected = true;
        with_window_manager(|manager| {
            self.parent = manager.get_window_by_id(self.parent_id);
            if let Some(parent) = self.parent.as_ref() {
                let _ = manager.set_focus(Some(parent));
                let _ = manager.set_modal(parent.clone());
            }
        });
        self.populate_controls();
        if TheGameLogic::is_in_game() {
            Self::set_window_enabled(self.combo_detail_id, false);
            Self::set_window_enabled(self.combo_resolution_id, false);
            Self::set_window_enabled(self.check_send_delay_id, false);
        }
        layout.hide(false);
        self.ignore_selected = false;
        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        _layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Options Menu for layout: {}",
            layout.get_filename()
        );
        if let Some(parent) = self.parent.as_ref() {
            with_window_manager(|manager| {
                let _ = manager.unset_modal(parent);
            });
        }
        Self::set_window_hidden(self.advanced_window_id, true);
        layout.hide(true);
        self.initialized = false;
        Ok(())
    }

    fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy | WindowMessage::InputFocus => {
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetValueChanged => {
                if self.ignore_selected {
                    return WindowMsgHandled::Handled;
                }
                if data1 as i32 == self.combo_detail_id
                    && Self::combo_selected_index(self.combo_detail_id) == Some(3)
                {
                    Self::set_window_hidden(self.advanced_window_id, false);
                }
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetSelected => {
                if self.ignore_selected {
                    return WindowMsgHandled::Handled;
                }
                let control_id = data1 as i32;
                if control_id == self.button_back_id {
                    self.close_menu();
                } else if control_id == self.button_accept_id {
                    self.apply_options();
                    self.close_menu();
                } else if control_id == self.button_defaults_id {
                    self.apply_default_controls();
                } else if control_id == self.button_advanced_accept_id {
                    Self::set_window_hidden(self.advanced_window_id, true);
                } else if control_id == self.button_advanced_back_id {
                    Self::set_combo_selected(self.combo_detail_id, self.initial_detail_index);
                    Self::set_window_hidden(self.advanced_window_id, true);
                } else if control_id == self.button_keyboard_options_id {
                    let _ = get_shell().push("Menus/KeyboardOptionsMenu.wnd", false);
                } else if control_id == self.combo_detail_id
                    && Self::combo_selected_index(self.combo_detail_id) == Some(3)
                {
                    Self::set_window_hidden(self.advanced_window_id, false);
                }
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg == WindowMessage::Char && data1 == 0x1B && (data2 & 0x0001) != 0 {
            self.close_menu();
            return WindowMsgHandled::Handled;
        }
        WindowMsgHandled::Ignored
    }
}

/// Map Select Menu implementation
pub struct MapSelectMenu {
    initialized: bool,
    parent_id: i32,
    listbox_map_id: i32,
    button_ok_id: i32,
    button_back_id: i32,
    button_single_player_id: i32,
    button_multiplayer_id: i32,
    radio_easy_id: i32,
    radio_medium_id: i32,
    radio_hard_id: i32,
    radio_system_maps_id: i32,
    radio_user_maps_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    listbox_map: Option<Rc<RefCell<GameWindow>>>,
    show_solo_maps: bool,
    use_system_maps: bool,
    selected_map: Option<String>,
    button_pushed: bool,
    is_shutting_down: bool,
    start_game: bool,
    difficulty: i32,
}

impl MapSelectMenu {
    pub fn new() -> Self {
        Self {
            initialized: false,
            parent_id: 0,
            listbox_map_id: 0,
            button_ok_id: 0,
            button_back_id: 0,
            button_single_player_id: 0,
            button_multiplayer_id: 0,
            radio_easy_id: 0,
            radio_medium_id: 0,
            radio_hard_id: 0,
            radio_system_maps_id: 0,
            radio_user_maps_id: 0,
            parent: None,
            listbox_map: None,
            show_solo_maps: true,
            use_system_maps: true,
            selected_map: None,
            button_pushed: false,
            is_shutting_down: false,
            start_game: false,
            difficulty: gamelogic::helpers::TheScriptEngine::get_global_difficulty(),
        }
    }

    fn name_to_id(name: &str) -> i32 {
        NameKeyGenerator::name_to_key(name) as i32
    }

    fn populate_map_list(&mut self) {
        let Some(listbox) = self.listbox_map.as_ref() else {
            return;
        };
        let mut listbox_guard = listbox.borrow_mut();
        let Some(widget) = listbox_guard.list_box_mut() else {
            return;
        };
        let map_to_select = self.selected_map.as_deref();
        populate_map_listbox(
            widget,
            self.use_system_maps,
            !self.show_solo_maps,
            map_to_select,
        );
        self.selected_map = widget
            .selected_item()
            .and_then(|item| match item.data.as_ref() {
                Some(ListBoxItemData::Text(path)) => Some(path.clone()),
                _ => None,
            });
    }

    fn set_radio_selected(window: &Option<Rc<RefCell<GameWindow>>>, selected: bool) {
        let Some(window) = window.as_ref() else {
            return;
        };
        let mut guard = window.borrow_mut();
        if let Some(widget) = guard.widget_mut() {
            if let WindowWidget::RadioButton(radio) = widget {
                if selected {
                    radio.select();
                }
            }
        }
    }

    fn update_selected_map(&mut self) {
        let Some(listbox) = self.listbox_map.as_ref() else {
            return;
        };
        let listbox_guard = listbox.borrow();
        let Some(widget) = listbox_guard.widget().and_then(|widget| match widget {
            WindowWidget::ListBox(listbox) => Some(listbox),
            _ => None,
        }) else {
            return;
        };
        self.selected_map = widget
            .selected_item()
            .and_then(|item| match item.data.as_ref() {
                Some(ListBoxItemData::Text(path)) => Some(path.clone()),
                _ => None,
            });
    }

    fn start_game(&mut self) {
        let Some(map_name) = self.selected_map.clone() else {
            return;
        };
        self.start_game = true;
        if let Some(data) = game_engine::common::ini::get_global_data() {
            let mut data = data.write();
            data.pending_file = map_name;
        }
        get_shell().reverse_animate_window();
    }

    fn do_game_start(&mut self) {
        self.button_pushed = true;

        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }

        let mut shell = get_shell();
        let _ = shell.pop();
        let _ = shell.hide_shell();
        drop(shell);

        self.start_game = false;
        TheGameLogic::prepare_new_game(GAME_SINGLE_PLAYER, self.difficulty, 0);
    }

    fn shutdown_complete(&mut self, layout: &WindowLayout) {
        self.is_shutting_down = false;
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);
    }

    fn write_use_system_maps_preference(&self) {
        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");
        pref.set_bool("UseSystemMapDir", self.use_system_maps);
        let _ = pref.write();
    }

    fn update_map_cache(&self) {
        if let Ok(mut cache) = get_map_cache_manager().lock() {
            cache.update_cache();
        }
    }

    fn refresh_map_list(&mut self) {
        self.update_map_cache();
        self.populate_map_list();
        self.update_selected_map();
    }

    fn current_map_selection(&self) -> Option<String> {
        let Some(map_name) = self.selected_map.clone() else {
            return None;
        };
        Some(map_name)
    }
}

impl Default for MapSelectMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for MapSelectMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Map Select Menu for layout: {}",
            layout.get_filename()
        );
        self.initialized = true;
        self.button_pushed = false;
        self.show_solo_maps = true;
        self.is_shutting_down = false;
        self.start_game = false;
        get_shell().show_shell_map(true);
        layout.hide(false);

        self.parent_id = Self::name_to_id("MapSelectMenu.wnd:MapSelectMenuParent");
        self.listbox_map_id = Self::name_to_id("MapSelectMenu.wnd:ListboxMap");
        self.button_ok_id = Self::name_to_id("MapSelectMenu.wnd:ButtonOK");
        self.button_back_id = Self::name_to_id("MapSelectMenu.wnd:ButtonBack");
        self.button_single_player_id = Self::name_to_id("MapSelectMenu.wnd:ButtonSinglePlayer");
        self.button_multiplayer_id = Self::name_to_id("MapSelectMenu.wnd:ButtonMultiplayer");
        self.radio_easy_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonEasyAI");
        self.radio_medium_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonMediumAI");
        self.radio_hard_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonHardAI");
        self.radio_user_maps_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonUserMaps");
        self.radio_system_maps_id = Self::name_to_id("MapSelectMenu.wnd:RadioButtonSystemMaps");

        with_window_manager(|manager| {
            self.parent = manager.get_window_by_id(self.parent_id);
            self.listbox_map = manager.get_window_by_id(self.listbox_map_id);
            if let Some(parent) = self.parent.as_ref() {
                let _ = manager.set_focus(Some(parent));
            }
            if let Some(button_back) = manager.get_window_by_id(self.button_back_id) {
                get_shell().register_with_animate_manager(
                    button_back,
                    AnimationType::SlideRight,
                    true,
                    0,
                );
            }
            if let Some(button_ok) = manager.get_window_by_id(self.button_ok_id) {
                get_shell().register_with_animate_manager(
                    button_ok,
                    AnimationType::SlideLeft,
                    true,
                    0,
                );
            }
        });

        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");
        self.use_system_maps = pref.get_bool_or("UseSystemMapDir", true);

        let difficulty = gamelogic::helpers::TheScriptEngine::get_global_difficulty();
        self.difficulty = difficulty;

        Self::set_radio_selected(
            &with_window_manager(|manager| manager.get_window_by_id(self.radio_easy_id)),
            difficulty == 0,
        );
        Self::set_radio_selected(
            &with_window_manager(|manager| manager.get_window_by_id(self.radio_medium_id)),
            difficulty == 1,
        );
        Self::set_radio_selected(
            &with_window_manager(|manager| manager.get_window_by_id(self.radio_hard_id)),
            difficulty == 2,
        );
        Self::set_radio_selected(
            &with_window_manager(|manager| manager.get_window_by_id(self.radio_system_maps_id)),
            self.use_system_maps,
        );
        Self::set_radio_selected(
            &with_window_manager(|manager| manager.get_window_by_id(self.radio_user_maps_id)),
            !self.use_system_maps,
        );

        self.refresh_map_list();
        Ok(())
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.start_game && get_shell().is_anim_finished() {
            self.do_game_start();
        } else if self.is_shutting_down && get_shell().is_anim_finished() {
            self.shutdown_complete(layout);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Map Select Menu for layout: {}",
            layout.get_filename()
        );
        if !self.start_game {
            self.is_shutting_down = true;
            get_shell().reverse_animate_window();
        }
        self.initialized = false;
        self.parent = None;
        self.listbox_map = None;
        Ok(())
    }

    fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
            WindowMessage::InputFocus => WindowMsgHandled::Handled,
            WindowMessage::GadgetSelected => {
                if self.button_pushed {
                    return WindowMsgHandled::Handled;
                }

                let control_id = data1 as i32;
                if control_id == self.button_ok_id {
                    self.update_selected_map();
                    if self.current_map_selection().is_some() {
                        self.button_pushed = true;
                        get_campaign_manager().set_campaign("");
                        self.start_game();
                        return WindowMsgHandled::Handled;
                    }
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_back_id {
                    self.button_pushed = true;
                    let _ = get_shell().pop();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_single_player_id {
                    self.show_solo_maps = true;
                    self.refresh_map_list();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_multiplayer_id {
                    self.show_solo_maps = false;
                    self.refresh_map_list();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_system_maps_id {
                    self.use_system_maps = true;
                    self.write_use_system_maps_preference();
                    self.refresh_map_list();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_user_maps_id {
                    self.use_system_maps = false;
                    self.write_use_system_maps_preference();
                    self.refresh_map_list();
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_easy_id {
                    self.difficulty = 0;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_medium_id {
                    self.difficulty = 1;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.radio_hard_id {
                    self.difficulty = 2;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.listbox_map_id {
                    self.update_selected_map();
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Ignored
            }
            WindowMessage::GadgetValueChanged => {
                let control_id = data1 as i32;
                if control_id == self.listbox_map_id {
                    self.update_selected_map();
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Ignored
            }
            WindowMessage::User(0x8000) => {
                if self.button_pushed {
                    return WindowMsgHandled::Handled;
                }
                let control_id = data1 as i32;
                if control_id == self.listbox_map_id {
                    self.update_selected_map();
                    if self.current_map_selection().is_some() {
                        self.button_pushed = true;
                        get_campaign_manager().set_campaign("");
                        self.start_game();
                    }
                    return WindowMsgHandled::Handled;
                }
                WindowMsgHandled::Ignored
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Char || self.button_pushed {
            return WindowMsgHandled::Ignored;
        }
        let key = data1 as u32;
        let state = data2 as u32;
        if key != 0x1B {
            return WindowMsgHandled::Ignored;
        }
        if (state & 0x0001) == 0 {
            return WindowMsgHandled::Handled;
        }
        if let Some(parent) = self.parent.as_ref() {
            let _ = parent.borrow_mut().send_system_message(
                WindowMessage::GadgetSelected,
                self.button_back_id as u32,
                self.button_back_id as u32,
            );
        }
        WindowMsgHandled::Handled
    }
}

/// Credits Menu implementation  
pub struct CreditsMenu {
    initialized: bool,
    parent_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    credits: Option<crate::credits::CreditsManager>,
}

impl CreditsMenu {
    pub fn new() -> Self {
        Self {
            initialized: false,
            parent_id: 0,
            parent: None,
            credits: None,
        }
    }

    pub fn draw(&mut self) {
        if let Some(credits) = self.credits.as_mut() {
            credits.draw();
        }
    }
}

impl Default for CreditsMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for CreditsMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Credits Menu for layout: {}",
            layout.get_filename()
        );

        get_shell().show_shell_map(false);

        self.parent_id =
            NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
        with_window_manager(|manager| {
            self.parent = manager.get_window_by_id(self.parent_id);
            if let Some(parent) = self.parent.as_ref() {
                let _ = manager.set_focus(Some(parent));
            }
        });

        layout.hide(false);

        let mut credits = crate::credits::CreditsManager::new();
        if let Err(err) = credits.load_from_path("Data/INI/Credits.ini") {
            warn!("Failed to load credits data: {}", err);
        }
        credits.init();
        self.credits = Some(credits);

        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(0xFFFF_FFF1);
            let mut event = AudioEventRts::new("Credits");
            event.set_should_fade(true);
            let _ = audio.add_audio_event(&event);
        }

        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        _layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(parent) = self.parent.as_ref() {
            with_window_manager(|manager| {
                let _ = manager.set_focus(Some(parent));
            });
        }

        if let Some(credits) = self.credits.as_mut() {
            credits.update();
            if credits.is_finished() {
                let _ = get_shell().pop();
            }
        } else {
            let _ = get_shell().pop();
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Credits Menu for layout: {}",
            layout.get_filename()
        );

        if let Some(credits) = self.credits.as_mut() {
            credits.reset();
        }

        get_shell().show_shell_map(true);
        layout.hide(true);
        let _ = get_shell().shutdown_complete(None, false);

        if let Some(audio) = TheAudio::get() {
            audio.remove_audio_event(0xFFFF_FFF1);
        }

        self.initialized = false;
        self.credits = None;
        self.parent = None;
        self.parent_id = 0;
        Ok(())
    }

    fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create
            | WindowMessage::Destroy
            | WindowMessage::InputFocus
            | WindowMessage::GadgetSelected => WindowMsgHandled::Handled,
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        if msg == WindowMessage::Char && data1 == 0x1B {
            if (data2 & 0x0001) != 0 {
                let _ = get_shell().pop();
            }
            return WindowMsgHandled::Handled;
        }

        WindowMsgHandled::Ignored
    }
}

/// LAN Lobby Menu implementation
pub struct LanLobbyMenu {
    initialized: bool,
}

impl LanLobbyMenu {
    pub fn new() -> Self {
        Self { initialized: false }
    }
}

impl Default for LanLobbyMenu {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuCallbacks for LanLobbyMenu {
    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing LAN Lobby Menu for layout: {}",
            layout.get_filename()
        );
        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        _layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Update LAN lobby menu state
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down LAN Lobby Menu for layout: {}",
            layout.get_filename()
        );
        self.initialized = false;
        Ok(())
    }

    fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!("LAN Lobby Menu system message: {:?}", msg);
        WindowMsgHandled::Ignored
    }

    fn input(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
    ) -> WindowMsgHandled {
        debug!("LAN Lobby Menu input message: {:?}", msg);
        WindowMsgHandled::Ignored
    }
}

/// Menu manager to handle all menu instances
pub struct MenuManager {
    main_menu: Arc<RwLock<MainMenu>>,
    single_player_menu: Arc<RwLock<SinglePlayerMenu>>,
    options_menu: Arc<RwLock<OptionsMenu>>,
    map_select_menu: Arc<RwLock<MapSelectMenu>>,
    credits_menu: Arc<RwLock<CreditsMenu>>,
    lan_lobby_menu: Arc<RwLock<LanLobbyMenu>>,
}

impl MenuManager {
    pub fn new() -> Self {
        Self {
            main_menu: Arc::new(RwLock::new(MainMenu::new())),
            single_player_menu: Arc::new(RwLock::new(SinglePlayerMenu::new())),
            options_menu: Arc::new(RwLock::new(OptionsMenu::new())),
            map_select_menu: Arc::new(RwLock::new(MapSelectMenu::new())),
            credits_menu: Arc::new(RwLock::new(CreditsMenu::new())),
            lan_lobby_menu: Arc::new(RwLock::new(LanLobbyMenu::new())),
        }
    }

    pub fn get_main_menu(&self) -> Arc<RwLock<MainMenu>> {
        self.main_menu.clone()
    }

    pub fn get_single_player_menu(&self) -> Arc<RwLock<SinglePlayerMenu>> {
        self.single_player_menu.clone()
    }

    pub fn get_options_menu(&self) -> Arc<RwLock<OptionsMenu>> {
        self.options_menu.clone()
    }

    pub fn get_map_select_menu(&self) -> Arc<RwLock<MapSelectMenu>> {
        self.map_select_menu.clone()
    }

    pub fn get_credits_menu(&self) -> Arc<RwLock<CreditsMenu>> {
        self.credits_menu.clone()
    }

    pub fn get_lan_lobby_menu(&self) -> Arc<RwLock<LanLobbyMenu>> {
        self.lan_lobby_menu.clone()
    }
}

impl Default for MenuManager {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    static THE_MENU_MANAGER: Arc<RwLock<MenuManager>> =
        Arc::new(RwLock::new(MenuManager::new()));
}

/// Helper function to get the global menu manager
pub fn get_menu_manager() -> Arc<RwLock<MenuManager>> {
    THE_MENU_MANAGER.with(|manager| manager.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_main_menu_lifecycle() {
        let mut main_menu = MainMenu::new();
        let layout = WindowLayout::new("TestLayout".to_string());

        // Test initialization
        assert!(main_menu.init(&layout, None).is_ok());
        assert!(main_menu.initialized);

        // Test update
        assert!(main_menu.update(&layout, None).is_ok());

        // Test shutdown
        assert!(main_menu.shutdown(&layout, None).is_ok());
        assert!(!main_menu.initialized);
    }

    #[test]
    fn test_menu_manager() {
        let manager = MenuManager::new();

        // Test that all menus are accessible
        assert!(manager.get_main_menu().read().is_ok());
        assert!(manager.get_single_player_menu().read().is_ok());
        assert!(manager.get_options_menu().read().is_ok());
        assert!(manager.get_map_select_menu().read().is_ok());
        assert!(manager.get_credits_menu().read().is_ok());
        assert!(manager.get_lan_lobby_menu().read().is_ok());
    }

    #[test]
    fn test_global_menu_manager() {
        let manager1 = get_menu_manager();
        let manager2 = get_menu_manager();

        // Both should point to the same instance
        assert!(Arc::ptr_eq(&manager1, &manager2));
    }
}
