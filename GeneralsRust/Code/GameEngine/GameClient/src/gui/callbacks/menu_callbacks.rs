//! Menu Callback Functions
//!
//! This module contains callback functions for all the shell menus,
//! including main menu, single player menu, options menu, etc.

use crate::core::game_client::apply_lod_texture_reduction;
use crate::game_text::GameText;
use crate::gui::callbacks::quit_menu::destroy_quit_menu;
use crate::gui::campaign_manager::get_campaign_manager;
use crate::gui::gadgets::ComboBoxItem;
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::header_template::get_header_template_manager;
use crate::gui::options_host_bridge::{
    publish_host_alternate_mouse, publish_host_draw_rmb_scroll_anchor,
    publish_host_move_rmb_scroll_anchor,
};
use crate::gui::shell::main_menu::{DisplaySettings, get_main_menu};
use crate::gui::{
    AnimationType, GLM_DOUBLE_CLICKED, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowWidget, queue_shell_operation, queue_shell_pop, queue_shell_push,
    queue_shell_reverse_animate_window, queue_shell_shutdown_complete,
    queue_shell_window_animation, show_shell_map_if_available, try_with_shell_mut,
    with_window_manager, write_input_focus_response,
};
use crate::helpers::TheInGameUI;
use crate::map_util::{get_map_cache_manager, populate_map_listbox};
use crate::message_stream::{GameMessageType, get_message_stream};
use game_engine::common::audio::AudioAffect as EngineAudioAffect;
use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager,
};
use game_engine::common::global_data as runtime_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::random_value::init_random_with_seed;
use game_engine::common::user_preferences::UserPreferences;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::helpers::{TheAudio, TheGameLogic, TheScriptEngine};
use gamelogic::system::game_logic::{GAME_SHELL, GAME_SINGLE_PLAYER};
use log::{debug, error, info, warn};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};

#[cfg(feature = "online_ui")]
use crate::gamespy_overlay::{GameSpyOverlayType, close_overlay, is_overlay_open};

#[cfg(feature = "online_ui")]
fn options_overlay_is_open() -> bool {
    is_overlay_open(GameSpyOverlayType::Options)
}

#[cfg(not(feature = "online_ui"))]
fn options_overlay_is_open() -> bool {
    false
}

#[cfg(feature = "online_ui")]
fn close_options_overlay() {
    close_overlay(GameSpyOverlayType::Options);
}

#[cfg(not(feature = "online_ui"))]
fn close_options_overlay() {}

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
        self.initialized = false;
        self.parent = None;
        queue_shell_shutdown_complete(false);
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

        show_shell_map_if_available(true);
        layout.hide(false);

        with_window_manager(|manager| {
            self.parent = manager.get_window_by_id(self.parent_id);
            if let Some(parent) = self.parent.as_ref() {
                let _ = manager.set_focus(Some(parent));
            }
            if let Some(button_new) = manager.get_window_by_id(self.button_new_id) {
                queue_shell_window_animation(button_new, AnimationType::SlideLeft, true, 1);
            }
            if let Some(button_load) = manager.get_window_by_id(self.button_load_id) {
                queue_shell_window_animation(button_load, AnimationType::SlideLeft, true, 200);
            }
            if let Some(button_back) = manager.get_window_by_id(self.button_back_id) {
                queue_shell_window_animation(button_back, AnimationType::SlideRight, true, 1);
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
        if self.is_shutting_down
            && try_with_shell_mut(|shell| shell.is_anim_finished()).unwrap_or(false)
        {
            self.shutdown_complete(layout);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Single Player Menu for layout: {}",
            layout.get_filename()
        );
        let pop_immediate = user_data
            .and_then(|data| data.downcast_ref::<bool>())
            .copied()
            .unwrap_or(false);
        self.is_shutting_down = true;
        if pop_immediate {
            self.shutdown_complete(layout);
            return Ok(());
        }
        queue_shell_reverse_animate_window();
        Ok(())
    }

    fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
            WindowMessage::GadgetSelected => {
                if self.button_pushed {
                    return WindowMsgHandled::Handled;
                }

                let control_id = data1 as i32;
                if control_id == self.button_new_id {
                    queue_shell_push("Menus/MapSelectMenu.wnd", false);
                    self.button_pushed = true;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_back_id {
                    queue_shell_pop();
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
                self.button_back_id as WindowMsgData,
                self.button_back_id as WindowMsgData,
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
    combo_lan_ip_id: i32,
    combo_online_ip_id: i32,
    button_firewall_refresh_id: i32,
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
    firewall_behavior_override: Option<u32>,
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
            combo_lan_ip_id: 0,
            combo_online_ip_id: 0,
            button_firewall_refresh_id: 0,
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
            firewall_behavior_override: None,
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
            let _ = window.borrow_mut().gadget_check_box_set_checked(value);
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
        Self::slider_value_opt(id).unwrap_or(0)
    }

    /// C++ `GadgetSliderGetPosition` — `None` matches retail `-1` (missing gadget).
    fn slider_value_opt(id: i32) -> Option<i32> {
        Self::find_window(id).and_then(|window| {
            let guard = window.borrow();
            match guard.widget() {
                Some(WindowWidget::HorizontalSlider(slider)) => Some(slider.value()),
                _ => None,
            }
        })
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

    fn default_resolution_index(&self) -> usize {
        self.resolution_modes
            .iter()
            .position(|mode| *mode == (800, 600))
            .unwrap_or(0)
    }

    fn should_reset_resolution_on_defaults() -> bool {
        !TheGameLogic::is_in_game() || TheGameLogic::get_game_mode() == GAME_SHELL
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

    fn apply_immediate_checkbox_effect(&self, control_id: i32, checked: bool) -> bool {
        if control_id == self.check_draw_anchor_id {
            TheInGameUI::set_draw_rmb_scroll_anchor(checked);
            // Main owns the active RMB camera-drag presentation for an
            // AuthorityOnly match. Preserve the legacy standalone state, then
            // publish the same visual preference only while Main hosts it.
            let _ = publish_host_draw_rmb_scroll_anchor(checked);
            return true;
        }
        if control_id == self.check_move_anchor_id {
            TheInGameUI::set_move_rmb_scroll_anchor(checked);
            // Main owns live camera input for AuthorityOnly. Preserve this
            // standalone GameClient update, then send the same typed checkbox
            // choice to Main only while its host bridge is installed.
            let _ = publish_host_move_rmb_scroll_anchor(checked);
            return true;
        }
        if control_id == self.check_save_camera_id {
            runtime_global_data::write().save_camera_in_replay = checked;
            return true;
        }
        if control_id == self.check_use_camera_id {
            runtime_global_data::write().use_camera_in_replay = checked;
            return true;
        }
        false
    }

    fn current_relative_2d_volume() -> f32 {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        let Ok(guard) = manager.lock() else {
            return 0.0;
        };
        guard.get_audio_settings().relative_2d_volume
    }

    fn split_sfx_volume_for_relative(
        sfx_volume: i32,
        relative_2d_volume: f32,
    ) -> (i32, i32, f32, f32) {
        let base_volume = sfx_volume.clamp(0, 100) as f32 / 100.0;
        let relative_2d_volume = relative_2d_volume.clamp(-1.0, 1.0);
        let mut sound_2d_volume = base_volume;
        let mut sound_3d_volume = base_volume;
        if relative_2d_volume < 0.0 {
            sound_2d_volume *= 1.0 + relative_2d_volume;
        } else {
            sound_3d_volume *= 1.0 - relative_2d_volume;
        }

        (
            (sound_2d_volume * 100.0) as i32,
            (sound_3d_volume * 100.0) as i32,
            sound_2d_volume,
            sound_3d_volume,
        )
    }

    fn load_resolution_modes(&mut self) {
        // C++ OptionsMenu enumerates TheDisplay->getDisplayModeCount/Description
        // (4:3, >=800x600, >=24-bit).
        self.resolution_modes.clear();
        let count = crate::display::display_fx::display_mode_count();
        for i in 0..count {
            if let Some((w, h, _)) = crate::display::display_fx::display_mode_description(i) {
                self.resolution_modes.push((w as i32, h as i32));
            }
        }
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
        let draw_anchor = game_engine::common::user_preferences::scroll_anchor_pref_enabled(
            pref.get_string("DrawScrollAnchor")
                .map(|value| value.as_str()),
            TheInGameUI::get_draw_rmb_scroll_anchor(),
        );
        let move_anchor = game_engine::common::user_preferences::scroll_anchor_pref_enabled(
            pref.get_string("MoveScrollAnchor")
                .map(|value| value.as_str()),
            TheInGameUI::get_move_rmb_scroll_anchor(),
        );

        let music_volume = pref
            .get_int("MusicVolume")
            .unwrap_or((global.music_volume_factor * 100.0) as i32);
        let sfx_2d_volume = pref
            .get_int("SFXVolume")
            .unwrap_or((global.sfx_volume_factor * 100.0) as i32);
        let sfx_3d_volume = pref.get_int_or("SFX3DVolume", sfx_2d_volume);
        let sfx_volume = sfx_2d_volume.max(sfx_3d_volume);
        let voice_volume = pref
            .get_int("VoiceVolume")
            .unwrap_or((global.voice_volume_factor * 100.0) as i32);
        let gamma = pref.get_int_or("Gamma", 50);
        let scroll_speed = pref.get_int_or(
            "ScrollFactor",
            (global.keyboard_default_scroll_factor * 100.0) as i32,
        );
        let anti_aliasing = pref
            .get_int_or("AntiAliasing", global.anti_alias_box_value)
            .clamp(0, 2) as usize;
        // C++ OptionsMenu first-open: if static LOD is UNKNOWN, find+set.
        game_engine::common::game_lod::ensure_static_lod_applied();
        let detail_name = pref.get_string_or(
            "StaticGameLOD",
            &game_engine::common::game_lod::get_static_lod(),
        );
        let detail_name = if detail_name.eq_ignore_ascii_case("Unknown") {
            game_engine::common::game_lod::find_static_lod_level()
        } else {
            detail_name
        };
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
        // C++ OptionsMenuInit applies these to TheInGameUI on open.
        TheInGameUI::set_draw_rmb_scroll_anchor(draw_anchor);
        TheInGameUI::set_move_rmb_scroll_anchor(move_anchor);
        let _ = publish_host_draw_rmb_scroll_anchor(draw_anchor);
        let _ = publish_host_move_rmb_scroll_anchor(move_anchor);
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
        // C++ setDefaults never touches replay-camera or RMB-anchor checkboxes.

        let default_scroll =
            (runtime_global_data::read().keyboard_default_scroll_factor * 100.0).round() as i32;
        Self::set_slider_value(self.slider_scroll_speed_id, default_scroll);
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
        // C++ setDefaults: comboBoxDetail = findStaticLODLevel()
        let recommended = game_engine::common::game_lod::find_static_lod_level();
        Self::set_combo_selected(
            self.combo_detail_id,
            Self::detail_index_from_name(&recommended),
        );

        if Self::should_reset_resolution_on_defaults() {
            Self::set_combo_selected(self.combo_resolution_id, self.default_resolution_index());
        }
        Self::set_window_hidden(self.advanced_window_id, true);
    }

    fn apply_options(&mut self) -> bool {
        crate::display::display_fx::install_default_display_gamma_hook();
        let host = take_host_options_apply();
        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");
        let (
            old_resolution,
            windowed,
            default_scroll,
            default_music,
            default_sfx,
            default_voice,
            default_res,
        ) = {
            let global = runtime_global_data::read();
            (
                (global.writable.x_resolution, global.writable.y_resolution),
                global.writable.windowed,
                (global.keyboard_scroll_factor * 100.0) as i32,
                (global.music_volume_factor * 100.0) as i32,
                (global.sfx_volume_factor * 100.0) as i32,
                (global.voice_volume_factor * 100.0) as i32,
                (global.writable.x_resolution, global.writable.y_resolution),
            )
        };

        let detail_index = host
            .as_ref()
            .map(|h| h.detail_index)
            .or_else(|| Self::combo_selected_index(self.combo_detail_id))
            .unwrap_or(1);
        let anti_aliasing = host
            .as_ref()
            .map(|h| h.anti_aliasing)
            .or_else(|| Self::combo_selected_index(self.combo_anti_aliasing_id).map(|i| i as i32))
            .unwrap_or_else(|| pref.get_int_or("AntiAliasing", 0));
        let resolution = host
            .as_ref()
            .map(|h| h.resolution)
            .or_else(|| {
                self.resolution_modes
                    .get(Self::combo_selected_index(self.combo_resolution_id).unwrap_or(0))
                    .copied()
            })
            .unwrap_or_else(|| {
                parse_resolution_pref(pref.get_string("Resolution").map(|s| s.as_str()))
                    .unwrap_or(default_res)
            });

        let alternate_mouse = host
            .as_ref()
            .map(|h| h.alternate_mouse)
            .unwrap_or_else(|| Self::checkbox_value(self.check_alternate_mouse_id));
        let retaliation = host
            .as_ref()
            .map(|h| h.retaliation)
            .unwrap_or_else(|| Self::checkbox_value(self.check_retaliation_id));
        let double_click_attack_move = host
            .as_ref()
            .map(|h| h.double_click_attack_move)
            .unwrap_or_else(|| Self::checkbox_value(self.check_double_click_attack_move_id));
        let language_filter = host
            .as_ref()
            .map(|h| h.language_filter)
            .unwrap_or_else(|| Self::checkbox_value(self.check_language_filter_id));
        let send_delay = Self::checkbox_value(self.check_send_delay_id);
        let save_camera = host
            .as_ref()
            .map(|h| h.save_camera)
            .unwrap_or_else(|| Self::checkbox_value(self.check_save_camera_id));
        let use_camera = host
            .as_ref()
            .map(|h| h.use_camera)
            .unwrap_or_else(|| Self::checkbox_value(self.check_use_camera_id));
        let draw_anchor = host
            .as_ref()
            .map(|h| h.draw_anchor)
            .unwrap_or_else(|| Self::checkbox_value(self.check_draw_anchor_id));
        let move_anchor = host
            .as_ref()
            .map(|h| h.move_anchor)
            .unwrap_or_else(|| Self::checkbox_value(self.check_move_anchor_id));
        // Host Accept has no WND layout; C++ saveOptions reads checkboxes that
        // OptionsMenuInit populated from prefs/globals. Keep those current values.
        let host_apply = host.is_some();
        let current = runtime_global_data::read();
        let use_shadow_volumes = if host_apply {
            pref.get_bool_or("UseShadowVolumes", current.writable.use_shadow_volumes)
        } else {
            Self::checkbox_value(self.check_3d_shadows_id)
        };
        let use_shadow_decals = if host_apply {
            pref.get_bool_or("UseShadowDecals", current.writable.use_shadow_decals)
        } else {
            Self::checkbox_value(self.check_2d_shadows_id)
        };
        let use_cloud_map = if host_apply {
            pref.get_bool_or("UseCloudMap", current.use_cloud_map)
        } else {
            Self::checkbox_value(self.check_cloud_shadows_id)
        };
        let use_light_map = if host_apply {
            pref.get_bool_or("UseLightMap", current.use_light_map)
        } else {
            Self::checkbox_value(self.check_ground_lighting_id)
        };
        let show_soft_water_edge = if host_apply {
            pref.get_bool_or("ShowSoftWaterEdge", current.show_soft_water_edge)
        } else {
            Self::checkbox_value(self.check_smooth_water_id)
        };
        let extra_animations = if host_apply {
            pref.get_bool("ExtraAnimations")
                .unwrap_or(!current.use_draw_module_lod)
        } else {
            Self::checkbox_value(self.check_extra_animations_id)
        };
        let no_dynamic_lod = if host_apply {
            !pref
                .get_bool("DynamicLOD")
                .unwrap_or(current.writable.enable_dynamic_lod)
        } else {
            Self::checkbox_value(self.check_no_dynamic_lod_id)
        };
        let unlock_fps = if host_apply {
            !pref
                .get_bool("FPSLimit")
                .unwrap_or(current.writable.use_fps_limit)
        } else {
            Self::checkbox_value(self.check_unlock_fps_id)
        };
        let heat_effects = if host_apply {
            pref.get_bool_or("HeatEffects", current.use_heat_effects)
        } else {
            Self::checkbox_value(self.check_heat_effects_id)
        };
        let building_occlusion = if host_apply {
            pref.get_bool_or("BuildingOcclusion", current.enable_behind_building_markers)
        } else {
            Self::checkbox_value(self.check_building_occlusion_id)
        };
        let show_props = if host_apply {
            pref.get_bool_or("ShowTrees", current.use_trees)
        } else {
            Self::checkbox_value(self.check_props_id)
        };
        drop(current);

        // C++ OptionsMenu.cpp:1168-1241 `if (val != -1)` — missing slider keeps pref.
        let scroll_speed = Self::slider_value_opt(self.slider_scroll_speed_id)
            .or_else(|| host.as_ref().map(|h| h.scroll_speed))
            .unwrap_or_else(|| pref.get_int_or("ScrollFactor", default_scroll))
            .clamp(0, 100);
        let music_volume = Self::slider_value_opt(self.slider_music_volume_id)
            .or_else(|| host.as_ref().map(|h| h.music_volume))
            .unwrap_or_else(|| pref.get_int_or("MusicVolume", default_music))
            .clamp(0, 100);
        let sfx_volume = Self::slider_value_opt(self.slider_sfx_volume_id)
            .or_else(|| host.as_ref().map(|h| h.sfx_volume))
            .unwrap_or_else(|| pref.get_int_or("SFXVolume", default_sfx))
            .clamp(0, 100);
        let voice_volume = Self::slider_value_opt(self.slider_voice_volume_id)
            .or_else(|| host.as_ref().map(|h| h.voice_volume))
            .unwrap_or_else(|| pref.get_int_or("VoiceVolume", default_voice))
            .clamp(0, 100);
        let (sfx_2d_volume, sfx_3d_volume, sfx_2d_factor, sfx_3d_factor) =
            Self::split_sfx_volume_for_relative(sfx_volume, Self::current_relative_2d_volume());
        let gamma_slider = Self::slider_value_opt(self.slider_gamma_id)
            .or_else(|| host.as_ref().map(|h| h.gamma_slider))
            .unwrap_or_else(|| pref.get_int_or("Gamma", 50))
            .clamp(0, 100);
        let texture_resolution = if host_apply {
            let reduction = pref
                .get_int("TextureReduction")
                .unwrap_or(runtime_global_data::read().texture_reduction_factor)
                .clamp(0, 2);
            2 - reduction
        } else {
            Self::slider_value_opt(self.slider_texture_resolution_id)
                .unwrap_or(2)
                .clamp(0, 2)
        };
        let particle_cap = if host_apply {
            pref.get_int("MaxParticleCount")
                .unwrap_or(runtime_global_data::read().max_particle_count)
                .max(100)
        } else {
            Self::slider_value_opt(self.slider_particle_cap_id)
                .unwrap_or(5000)
                .max(100)
        };
        let texture_reduction = 2 - texture_resolution;
        let detail_name = Self::detail_name_from_index(detail_index);

        pref.set_string("Resolution", format!("{} {}", resolution.0, resolution.1));
        pref.set_int("AntiAliasing", anti_aliasing);
        pref.set_string("StaticGameLOD", detail_name.to_string());
        pref.set_int("ScrollFactor", scroll_speed);
        pref.set_int("MusicVolume", music_volume);
        pref.set_int("SFXVolume", sfx_2d_volume);
        pref.set_int("SFX3DVolume", sfx_3d_volume);
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
        // C++ saveOptions writes advanced LOD only when Detail == CUSTOMDETAIL.
        // FPSLimit is never written (`//Never write this out`).
        const CUSTOMDETAIL: usize = 3;
        if detail_index == CUSTOMDETAIL {
            pref.set_int("TextureReduction", texture_reduction);
            pref.set_int("MaxParticleCount", particle_cap);
            Self::set_yes_no(&mut pref, "UseShadowVolumes", use_shadow_volumes);
            Self::set_yes_no(&mut pref, "UseShadowDecals", use_shadow_decals);
            Self::set_yes_no(&mut pref, "UseCloudMap", use_cloud_map);
            Self::set_yes_no(&mut pref, "UseLightMap", use_light_map);
            Self::set_yes_no(&mut pref, "ShowSoftWaterEdge", show_soft_water_edge);
            Self::set_yes_no(&mut pref, "ExtraAnimations", extra_animations);
            Self::set_yes_no(&mut pref, "DynamicLOD", !no_dynamic_lod);
            Self::set_yes_no(&mut pref, "HeatEffects", heat_effects);
            Self::set_yes_no(&mut pref, "BuildingOcclusion", building_occlusion);
            Self::set_yes_no(&mut pref, "ShowTrees", show_props);
        }
        if let Some(firewall_behavior) = self.firewall_behavior_override {
            pref.set_string("FirewallBehavior", firewall_behavior.to_string());
        }
        let _ = pref.write();
        self.firewall_behavior_override = None;

        TheInGameUI::set_draw_rmb_scroll_anchor(draw_anchor);
        TheInGameUI::set_move_rmb_scroll_anchor(move_anchor);
        let _ = publish_host_draw_rmb_scroll_anchor(draw_anchor);
        let _ = publish_host_move_rmb_scroll_anchor(move_anchor);

        Self::commit_alternate_mouse_setting(alternate_mouse);
        let display_gamma = Self::slider_to_gamma(gamma_slider);
        {
            let mut global = runtime_global_data::write();
            global.client_retaliation_mode_enabled = retaliation;
            global.double_click_attack_move = double_click_attack_move;
            global.language_filter_pref = language_filter;
            global.firewall_send_delay = send_delay;
            global.save_camera_in_replay = save_camera;
            global.use_camera_in_replay = use_camera;
            global.display_gamma = display_gamma;
            global.anti_alias_box_value = anti_aliasing;
            global.keyboard_scroll_factor = scroll_speed as f32 / 100.0;
            global.music_volume_factor = music_volume as f32 / 100.0;
            global.sfx_volume_factor = sfx_volume as f32 / 100.0;
            global.voice_volume_factor = voice_volume as f32 / 100.0;
            global.writable.x_resolution = resolution.0;
            global.writable.y_resolution = resolution.1;
            if detail_index == CUSTOMDETAIL {
                global.use_cloud_map = use_cloud_map;
                global.use_light_map = use_light_map;
                global.show_soft_water_edge = show_soft_water_edge;
                global.use_draw_module_lod = !extra_animations;
                global.use_heat_effects = heat_effects;
                global.enable_behind_building_markers = building_occlusion;
                global.use_trees = show_props;
                global.max_particle_count = particle_cap;
                global.writable.use_shadow_volumes = use_shadow_volumes;
                global.writable.use_shadow_decals = use_shadow_decals;
                global.writable.enable_dynamic_lod = !no_dynamic_lod;
            }
        }
        crate::display::display_fx::set_gamma_state(display_gamma, 0.0, 1.0);
        let _ =
            crate::core::script_action_handler::set_script_display_gamma(display_gamma, 0.0, 1.0);

        if detail_index == CUSTOMDETAIL {
            let _ = apply_lod_texture_reduction(texture_reduction);
        }

        game_engine::common::game_lod::set_static_lod_from_string(detail_name);
        game_engine::common::game_lod::set_ideal_static_lod_from_string(detail_name);
        crate::gui::options_host_bridge::apply_display_gamma(display_gamma, 0.0, 1.0, false);

        let audio = TheAudio;
        // C++ OptionsMenu.cpp:1188/1214/1215/1235 AudioAffect_*|SystemSetting.
        audio.set_volume(
            music_volume as f32 / 100.0,
            EngineAudioAffect::MusicSystemSetting,
        );
        audio.set_volume(sfx_2d_factor, EngineAudioAffect::SoundSystemSetting);
        audio.set_volume(sfx_3d_factor, EngineAudioAffect::Sound3DSystemSetting);
        audio.set_volume(
            voice_volume as f32 / 100.0,
            EngineAudioAffect::SpeechSystemSetting,
        );
        get_header_template_manager().header_notify_resolution_change();

        let resolution_changed = resolution != old_resolution;
        if resolution_changed {
            let old_settings = DisplaySettings {
                x_res: old_resolution.0,
                y_res: old_resolution.1,
                bit_depth: 32,
                windowed,
            };
            let new_settings = DisplaySettings {
                x_res: resolution.0,
                y_res: resolution.1,
                bit_depth: 32,
                windowed,
            };
            // C++ OptionsMenu.cpp:1062 `if (TheDisplay->setDisplayMode(...))`.
            if !crate::core::script_action_handler::apply_script_display_mode(
                resolution.0.max(0) as u32,
                resolution.1.max(0) as u32,
                32,
                windowed,
            ) {
                return false;
            }
            get_main_menu().set_pending_resolution_change(old_settings, new_settings);
        }

        resolution_changed
    }

    /// C++ has one writable GlobalData object. Rust's live consumers span a
    /// parsed-INI residence and a runtime options residence, so commit this
    /// setting to both before a later LeftHUD callback snapshots it for Main.
    fn commit_alternate_mouse_setting(alternate_mouse: bool) {
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().use_alternate_mouse = alternate_mouse;
        }
        runtime_global_data::write().use_alternate_mouse = alternate_mouse;
        // Main is the physical-world input authority in an AuthorityOnly
        // match.  Keep the legacy residences above for standalone GameClient,
        // then deliver the same accepted setting through the bounded host
        // bridge when it is installed.
        let _ = publish_host_alternate_mouse(alternate_mouse);
    }

    fn close_menu(&mut self) {
        let options_overlay_open = options_overlay_is_open();
        if let Some(parent) = self.parent.as_ref() {
            with_window_manager(|manager| {
                let _ = manager.unset_modal(parent);
            });
        }
        Self::set_window_hidden(self.advanced_window_id, true);
        TheScriptEngine::signal_ui_interact("ShellOptionsClosed");
        if options_overlay_open {
            close_options_overlay();
        } else {
            queue_shell_operation(|shell| shell.destroy_options_layout());
        }
        self.parent = None;
        self.initialized = false;
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
        self.combo_lan_ip_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxIP");
        self.combo_online_ip_id = Self::name_to_id("OptionsMenu.wnd:ComboBoxOnlineIP");
        self.button_firewall_refresh_id = Self::name_to_id("OptionsMenu.wnd:ButtonFirewallRefresh");
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
        let in_game_locked =
            TheGameLogic::is_in_game() && TheGameLogic::get_game_mode() != GAME_SHELL;
        if in_game_locked || options_overlay_is_open() {
            Self::set_window_enabled(self.combo_lan_ip_id, false);
            Self::set_window_enabled(self.combo_online_ip_id, false);
            Self::set_window_enabled(self.combo_detail_id, false);
            Self::set_window_enabled(self.combo_resolution_id, false);
            Self::set_window_enabled(self.check_send_delay_id, false);
            Self::set_window_enabled(self.button_firewall_refresh_id, false);
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
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
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
                    let resolution_changed = self.apply_options();
                    let options_overlay_open = options_overlay_is_open();
                    if !TheGameLogic::is_in_game() || TheGameLogic::get_game_mode() == GAME_SHELL {
                        destroy_quit_menu();
                    }
                    self.close_menu();
                    if resolution_changed && !options_overlay_open {
                        get_main_menu().do_resolution_dialog();
                    }
                } else if control_id == self.button_defaults_id {
                    self.apply_default_controls();
                } else if control_id == self.button_firewall_refresh_id {
                    self.firewall_behavior_override = Some(0);
                    {
                        let mut global = runtime_global_data::write();
                        global.firewall_behavior = 0;
                    }
                } else if control_id == self.button_advanced_accept_id {
                    Self::set_window_hidden(self.advanced_window_id, true);
                } else if control_id == self.button_advanced_back_id {
                    Self::set_combo_selected(self.combo_detail_id, self.initial_detail_index);
                    Self::set_window_hidden(self.advanced_window_id, true);
                } else if control_id == self.button_keyboard_options_id {
                    queue_shell_push("Menus/KeyboardOptionsMenu.wnd", false);
                } else if control_id == self.combo_detail_id
                    && Self::combo_selected_index(self.combo_detail_id) == Some(3)
                {
                    Self::set_window_hidden(self.advanced_window_id, false);
                } else {
                    self.apply_immediate_checkbox_effect(
                        control_id,
                        Self::checkbox_value(control_id),
                    );
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
        if msg != WindowMessage::Char || data1 != 0x1B {
            return WindowMsgHandled::Ignored;
        }

        if (data2 & 0x0001) != 0 {
            if let Some(parent) = self.parent.as_ref() {
                let _ = parent.borrow_mut().send_system_message(
                    WindowMessage::GadgetSelected,
                    self.button_back_id as WindowMsgData,
                    self.button_back_id as WindowMsgData,
                );
            }
        }

        WindowMsgHandled::Handled
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

    fn set_map_selection_from_cpp_row(&mut self, row: i32) {
        let Some(listbox) = self.listbox_map.as_ref() else {
            return;
        };
        let mut listbox_guard = listbox.borrow_mut();
        let Some(widget) = listbox_guard.widget_mut() else {
            return;
        };
        let WindowWidget::ListBox(listbox) = widget else {
            return;
        };
        if row < 0 {
            listbox.set_selected_indices(&[]);
        } else {
            let _ = listbox.select_index(row as usize, crate::gui::gadgets::KeyModifiers::none());
        }
    }

    fn start_game(&mut self) {
        let Some(map_name) = self.selected_map.clone() else {
            return;
        };
        self.start_game = true;
        // Menus retain the authored INI/GameData Arc, while Main's AuthorityOnly
        // MSG_NEW_GAME drain reads the runtime GlobalData singleton.  Keep the
        // chosen map in both residences before the reverse animation can emit
        // the NewGame request (same ownership bridge as SkirmishGameOptions).
        {
            let pending = map_name;
            if let Some(data) = game_engine::common::ini::get_global_data() {
                let mut data = data.write();
                data.pending_file = pending.clone();
            }
            runtime_global_data::write().pending_file = pending;
        }
        queue_shell_reverse_animate_window();
    }

    fn do_game_start(&mut self) {
        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }

        self.start_game = false;
        let message_stream = get_message_stream();
        let mut stream = message_stream.write().unwrap_or_else(|e| e.into_inner());
        let msg = stream.append_message(GameMessageType::NewGame);
        msg.append_integer_argument(GAME_SINGLE_PLAYER);
        msg.append_integer_argument(self.difficulty);
        msg.append_integer_argument(0);
        init_random_with_seed(0);
        self.is_shutting_down = true;
    }

    fn shutdown_complete(&mut self, layout: &WindowLayout) {
        self.is_shutting_down = false;
        layout.hide(true);
        self.initialized = false;
        self.parent = None;
        self.listbox_map = None;
        queue_shell_shutdown_complete(false);
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
        show_shell_map_if_available(true);
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
                queue_shell_window_animation(button_back, AnimationType::SlideRight, true, 0);
            }
            if let Some(button_ok) = manager.get_window_by_id(self.button_ok_id) {
                queue_shell_window_animation(button_ok, AnimationType::SlideLeft, true, 0);
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
        if self.start_game && try_with_shell_mut(|shell| shell.is_anim_finished()).unwrap_or(false)
        {
            self.do_game_start();
        }
        if self.is_shutting_down
            && try_with_shell_mut(|shell| shell.is_anim_finished()).unwrap_or(false)
        {
            self.shutdown_complete(layout);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Map Select Menu for layout: {}",
            layout.get_filename()
        );
        let pop_immediate = user_data
            .and_then(|data| data.downcast_ref::<bool>())
            .copied()
            .unwrap_or(false);
        if pop_immediate {
            self.shutdown_complete(layout);
            return Ok(());
        }
        if !self.start_game {
            self.is_shutting_down = true;
            queue_shell_reverse_animate_window();
        }
        Ok(())
    }

    fn system(
        &mut self,
        _window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
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
                    queue_shell_pop();
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
            WindowMessage::User(code) if code == GLM_DOUBLE_CLICKED => {
                if self.button_pushed {
                    return WindowMsgHandled::Handled;
                }
                let control_id = data1 as i32;
                if control_id == self.listbox_map_id {
                    self.set_map_selection_from_cpp_row(data2 as i32);
                    self.update_selected_map();
                    if self.current_map_selection().is_some() {
                        if let Some(parent) = self.parent.as_ref() {
                            let _ = parent.borrow_mut().send_system_message(
                                WindowMessage::GadgetSelected,
                                self.button_ok_id as WindowMsgData,
                                self.button_ok_id as WindowMsgData,
                            );
                        }
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
                self.button_back_id as WindowMsgData,
                self.button_back_id as WindowMsgData,
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

        show_shell_map_if_available(false);

        let mut credits = crate::credits::CreditsManager::new();
        if let Err(err) = credits.load_from_path("Data/INI/Credits.ini") {
            warn!("Failed to load credits data: {}", err);
        }
        credits.init();
        self.credits = Some(credits);

        self.parent_id =
            NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
        layout.hide(false);
        with_window_manager(|manager| {
            self.parent = manager.get_window_by_id(self.parent_id);
            if let Some(parent) = self.parent.as_ref() {
                let _ = manager.set_focus(Some(parent));
            }
        });

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
                queue_shell_pop();
            }
        } else {
            queue_shell_pop();
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

        show_shell_map_if_available(true);
        layout.hide(true);
        queue_shell_shutdown_complete(false);

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
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy | WindowMessage::GadgetSelected => {
                WindowMsgHandled::Handled
            }
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
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
                queue_shell_pop();
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
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_init(layout);
        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_update(layout);
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let immediate = user_data
            .and_then(|data| data.downcast_ref::<bool>())
            .copied()
            .unwrap_or(false);
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_shutdown(layout, immediate);
        self.initialized = false;
        Ok(())
    }

    fn system(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_system(window, msg, data1, data2)
    }

    fn input(
        &mut self,
        window: &GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_input(window, msg, data1, data2)
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

/// Residual: last OptionsMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualOptionsMenuAction {
    None = 0,
    Accept = 1,
    Back = 2,
    Defaults = 3,
    Keyboard = 4,
    AdvancedAccept = 5,
    AdvancedBack = 6,
    FirewallRefresh = 7,
}

static RESIDUAL_OPTIONS_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_OPTIONS_BOUND: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_options_action_store(action: ResidualOptionsMenuAction) {
    RESIDUAL_OPTIONS_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last OptionsMenu residual action.
pub fn residual_options_menu_last_action() -> ResidualOptionsMenuAction {
    match RESIDUAL_OPTIONS_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualOptionsMenuAction::Accept,
        2 => ResidualOptionsMenuAction::Back,
        3 => ResidualOptionsMenuAction::Defaults,
        4 => ResidualOptionsMenuAction::Keyboard,
        5 => ResidualOptionsMenuAction::AdvancedAccept,
        6 => ResidualOptionsMenuAction::AdvancedBack,
        7 => ResidualOptionsMenuAction::FirewallRefresh,
        _ => ResidualOptionsMenuAction::None,
    }
}

/// Residual: whether OptionsMenu control IDs were bound.
pub fn residual_options_menu_is_bound() -> bool {
    RESIDUAL_OPTIONS_BOUND.load(std::sync::atomic::Ordering::Relaxed)
}

/// In-game / host snapshot applied through C++ `saveOptions` (OptionsMenu.cpp:1047-1264).
#[derive(Debug, Clone)]
pub struct HostOptionsApply {
    pub resolution: (i32, i32),
    pub music_volume: i32,
    pub sfx_volume: i32,
    pub voice_volume: i32,
    pub gamma_slider: i32,
    pub scroll_speed: i32,
    pub alternate_mouse: bool,
    pub retaliation: bool,
    pub double_click_attack_move: bool,
    pub language_filter: bool,
    pub save_camera: bool,
    pub use_camera: bool,
    pub draw_anchor: bool,
    pub move_anchor: bool,
    pub anti_aliasing: i32,
    pub detail_index: usize,
}

impl Default for HostOptionsApply {
    fn default() -> Self {
        Self {
            resolution: (1024, 768),
            music_volume: 60,
            sfx_volume: 55,
            voice_volume: 70,
            gamma_slider: 50,
            scroll_speed: 50,
            alternate_mouse: false,
            retaliation: true,
            double_click_attack_move: false,
            language_filter: true,
            save_camera: true,
            use_camera: true,
            draw_anchor: true,
            move_anchor: true,
            anti_aliasing: 0,
            detail_index: 1,
        }
    }
}

thread_local! {
    static HOST_OPTIONS_APPLY: std::cell::RefCell<Option<HostOptionsApply>> =
        const { std::cell::RefCell::new(None) };
}

fn take_host_options_apply() -> Option<HostOptionsApply> {
    HOST_OPTIONS_APPLY.with(|slot| slot.borrow_mut().take())
}

fn parse_resolution_pref(raw: Option<&str>) -> Option<(i32, i32)> {
    let raw = raw?;
    let mut parts = raw.split(|c: char| c.is_ascii_whitespace() || c == 'x' || c == 'X');
    let w = parts.next()?.parse::<i32>().ok()?;
    let h = parts.next()?.parse::<i32>().ok()?;
    if w > 0 && h > 0 { Some((w, h)) } else { None }
}

/// C++ Accept: `TheDisplay->setGamma` + flat Options.ini + live volumes.
pub fn apply_options_menu_like_cpp() -> bool {
    crate::display::display_fx::install_default_display_gamma_hook();
    with_options_menu_mut(|menu| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
            RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        residual_options_action_store(ResidualOptionsMenuAction::Accept);
        let _ = menu.apply_options();
        true
    })
}

/// In-game stub / host Accept: same `apply_options` path with explicit values.
pub fn apply_options_from_host(apply: HostOptionsApply) -> bool {
    crate::display::display_fx::install_default_display_gamma_hook();
    HOST_OPTIONS_APPLY.with(|slot| {
        *slot.borrow_mut() = Some(apply);
    });
    apply_options_menu_like_cpp()
}

fn with_options_menu_mut<R>(f: impl FnOnce(&mut OptionsMenu) -> R) -> R {
    let menu = {
        let manager = get_menu_manager();
        let guard = manager.read().unwrap_or_else(|e| e.into_inner());
        guard.get_options_menu()
    };
    let mut menu = menu.write().unwrap_or_else(|e| e.into_inner());
    f(&mut menu)
}

/// Residual: bind OptionsMenu control IDs (no layout load / populate required).
pub fn simulate_options_menu_bind_controls() -> bool {
    with_options_menu_mut(|menu| {
        menu.init_ids();
        menu.ignore_selected = false;
        menu.initialized = true;
        RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        true
    })
}

/// Residual: fire ButtonAccept without apply_options / close_menu side effects.
pub fn simulate_options_menu_accept_button_gadget_selected() -> bool {
    with_options_menu_mut(|menu| {
        if !residual_options_menu_is_bound() {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
            RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        residual_options_action_store(ResidualOptionsMenuAction::Accept);
        true
    })
}

/// Residual: fire ButtonBack without destroy_options_layout.
pub fn simulate_options_menu_back_button_gadget_selected() -> bool {
    with_options_menu_mut(|menu| {
        if !residual_options_menu_is_bound() {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
            RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        residual_options_action_store(ResidualOptionsMenuAction::Back);
        true
    })
}

/// Residual: fire ButtonDefaults without apply_default_controls widget writes.
pub fn simulate_options_menu_defaults_button_gadget_selected() -> bool {
    with_options_menu_mut(|menu| {
        if !residual_options_menu_is_bound() {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
            RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        residual_options_action_store(ResidualOptionsMenuAction::Defaults);
        true
    })
}

/// Residual: fire ButtonKeyboardOptions without shell push.
pub fn simulate_options_menu_keyboard_button_gadget_selected() -> bool {
    with_options_menu_mut(|menu| {
        if !residual_options_menu_is_bound() {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
            RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        residual_options_action_store(ResidualOptionsMenuAction::Keyboard);
        true
    })
}

/// Residual: fire ButtonAdvanceAccept without advanced panel hide.
pub fn simulate_options_menu_advanced_accept_button_gadget_selected() -> bool {
    with_options_menu_mut(|menu| {
        if !residual_options_menu_is_bound() {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
            RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        residual_options_action_store(ResidualOptionsMenuAction::AdvancedAccept);
        true
    })
}

/// Residual: fire ButtonAdvanceBack without detail combo restore.
pub fn simulate_options_menu_advanced_back_button_gadget_selected() -> bool {
    with_options_menu_mut(|menu| {
        if !residual_options_menu_is_bound() {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
            RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        residual_options_action_store(ResidualOptionsMenuAction::AdvancedBack);
        true
    })
}

/// Residual: fire ButtonFirewallRefresh without global write.
pub fn simulate_options_menu_firewall_refresh_button_gadget_selected() -> bool {
    with_options_menu_mut(|menu| {
        if !residual_options_menu_is_bound() {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
            RESIDUAL_OPTIONS_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // Mirror C++ residual intent: mark override without applying prefs.
        menu.firewall_behavior_override = Some(0);
        residual_options_action_store(ResidualOptionsMenuAction::FirewallRefresh);
        true
    })
}

/// Residual: bind + Accept composite (save settings honesty).
pub fn simulate_options_menu_prepare_accept() -> bool {
    if !simulate_options_menu_bind_controls() {
        return false;
    }
    simulate_options_menu_accept_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on a retail `OptionsMenu.wnd:*` gadget
/// (C++ WindowXlat hit → GBM_SELECTED). Not `simulate_*` first.
fn drive_os_wnd_options_named(name: &str, latch: impl FnOnce() -> bool) -> bool {
    if !crate::gui::dispatch_os_click_named_window(name) {
        return false;
    }
    latch()
}

pub fn drive_os_wnd_options_menu_accept_like_cpp() -> bool {
    drive_os_wnd_options_named("OptionsMenu.wnd:ButtonAccept", apply_options_menu_like_cpp)
}

pub fn drive_os_wnd_options_menu_back_like_cpp() -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonBack",
        simulate_options_menu_back_button_gadget_selected,
    )
}

pub fn drive_os_wnd_options_menu_defaults_like_cpp() -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonDefaults",
        simulate_options_menu_defaults_button_gadget_selected,
    )
}

pub fn drive_os_wnd_options_menu_keyboard_like_cpp() -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonKeyboardOptions",
        simulate_options_menu_keyboard_button_gadget_selected,
    )
}

pub fn drive_os_wnd_options_menu_advanced_accept_like_cpp() -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonAdvanceAccept",
        simulate_options_menu_advanced_accept_button_gadget_selected,
    )
}

pub fn drive_os_wnd_options_menu_advanced_back_like_cpp() -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonAdvanceBack",
        simulate_options_menu_advanced_back_button_gadget_selected,
    )
}

pub fn drive_os_wnd_options_menu_firewall_like_cpp() -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonFirewallRefresh",
        simulate_options_menu_firewall_refresh_button_gadget_selected,
    )
}

#[cfg(test)]
mod options_os_wnd_tests {
    use super::*;
    use crate::gui::with_window_manager;

    fn install_named_button(name: &str, x: i32, y: i32) {
        with_window_manager(|manager| {
            let button = manager.create_window(None, x, y, 80, 24).expect(name);
            button.borrow_mut().set_name(name);
            let _ = button.borrow_mut().hide(false);
        });
    }

    #[test]
    fn os_wnd_options_menu_accept_hits_button_then_latches() {
        install_named_button("OptionsMenu.wnd:ButtonAccept", 10, 10);
        assert!(
            drive_os_wnd_options_menu_accept_like_cpp(),
            "OS WND click on ButtonAccept must latch Accept residual"
        );
        assert_eq!(
            residual_options_menu_last_action(),
            ResidualOptionsMenuAction::Accept
        );
        assert!(!drive_os_wnd_options_menu_back_like_cpp());
    }

    #[test]
    fn apply_options_from_host_sets_gamma_and_music_volume_like_cpp() {
        // C++ OptionsMenu.cpp:1180-1262 MusicVolume 0-100 + TheDisplay->setGamma.
        crate::display::display_fx::set_gamma_state(1.0, 0.0, 1.0);
        let mut apply = HostOptionsApply::default();
        apply.music_volume = 80;
        apply.gamma_slider = 100;
        apply.resolution = (1024, 768);
        assert!(apply_options_from_host(apply));
        let gamma = crate::display::display_fx::gamma_state();
        assert!((gamma.gamma - 2.0).abs() < 0.01);
        assert!((runtime_global_data::read().music_volume_factor - 0.8).abs() < 0.01);
        crate::display::display_fx::set_gamma_state(1.0, 0.0, 1.0);
    }

    #[test]
    fn apply_options_from_host_custom_preserves_advanced_detail() {
        let original = {
            let g = runtime_global_data::read();
            (
                g.writable.use_shadow_volumes,
                g.writable.use_shadow_decals,
                g.use_cloud_map,
                g.use_light_map,
                g.show_soft_water_edge,
                g.use_heat_effects,
                g.enable_behind_building_markers,
                g.use_trees,
                g.max_particle_count,
            )
        };
        {
            let mut g = runtime_global_data::write();
            g.writable.use_shadow_volumes = true;
            g.writable.use_shadow_decals = true;
            g.use_cloud_map = true;
            g.use_light_map = true;
            g.show_soft_water_edge = true;
            g.use_heat_effects = true;
            g.enable_behind_building_markers = true;
            g.use_trees = true;
            g.max_particle_count = 3000;
        }
        let mut apply = HostOptionsApply::default();
        apply.detail_index = 3;
        apply.resolution = (1024, 768);
        assert!(apply_options_from_host(apply));
        {
            let g = runtime_global_data::read();
            assert!(g.writable.use_shadow_volumes);
            assert!(g.writable.use_shadow_decals);
            assert!(g.use_cloud_map);
            assert!(g.use_light_map);
            assert!(g.show_soft_water_edge);
            assert!(g.use_heat_effects);
            assert!(g.enable_behind_building_markers);
            assert!(g.use_trees);
            assert_eq!(g.max_particle_count, 3000);
        }
        let mut g = runtime_global_data::write();
        g.writable.use_shadow_volumes = original.0;
        g.writable.use_shadow_decals = original.1;
        g.use_cloud_map = original.2;
        g.use_light_map = original.3;
        g.show_soft_water_edge = original.4;
        g.use_heat_effects = original.5;
        g.enable_behind_building_markers = original.6;
        g.use_trees = original.7;
        g.max_particle_count = original.8;
    }

    #[test]
    fn apply_options_from_host_publishes_scroll_anchors() {
        use crate::gui::options_host_bridge::{
            HostOptionsRequest, acquire_host_options_bridge_test_guard,
            set_host_options_bridge_enabled, take_host_options_requests,
        };

        let _bridge_guard = acquire_host_options_bridge_test_guard();
        set_host_options_bridge_enabled(true);
        let _ = take_host_options_requests();
        let original_draw = TheInGameUI::get_draw_rmb_scroll_anchor();
        let original_move = TheInGameUI::get_move_rmb_scroll_anchor();

        let mut apply = HostOptionsApply::default();
        apply.draw_anchor = true;
        apply.move_anchor = true;
        apply.resolution = (1024, 768);
        assert!(apply_options_from_host(apply));
        let reqs = take_host_options_requests();
        assert!(
            reqs.contains(&HostOptionsRequest::DrawRmbScrollAnchor { enabled: true }),
            "{reqs:?}"
        );
        assert!(
            reqs.contains(&HostOptionsRequest::MoveRmbScrollAnchor { enabled: true }),
            "{reqs:?}"
        );
        assert!(TheInGameUI::get_draw_rmb_scroll_anchor());
        assert!(TheInGameUI::get_move_rmb_scroll_anchor());

        TheInGameUI::set_draw_rmb_scroll_anchor(original_draw);
        TheInGameUI::set_move_rmb_scroll_anchor(original_move);
    }

    #[test]
    fn os_wnd_options_menu_keyboard_and_defaults_hit_named_gadgets() {
        install_named_button("OptionsMenu.wnd:ButtonKeyboardOptions", 10, 40);
        install_named_button("OptionsMenu.wnd:ButtonDefaults", 10, 70);
        assert!(drive_os_wnd_options_menu_keyboard_like_cpp());
        assert_eq!(
            residual_options_menu_last_action(),
            ResidualOptionsMenuAction::Keyboard
        );
        assert!(drive_os_wnd_options_menu_defaults_like_cpp());
        assert_eq!(
            residual_options_menu_last_action(),
            ResidualOptionsMenuAction::Defaults
        );
    }
}

/// Residual: last CreditsMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualCreditsMenuAction {
    None = 0,
    Bind = 1,
    Skip = 2,
    Finished = 3,
    Shutdown = 4,
}

static RESIDUAL_CREDITS_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_CREDITS_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_credits_action_store(action: ResidualCreditsMenuAction) {
    RESIDUAL_CREDITS_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last CreditsMenu residual action.
pub fn residual_credits_menu_last_action() -> ResidualCreditsMenuAction {
    match RESIDUAL_CREDITS_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualCreditsMenuAction::Bind,
        2 => ResidualCreditsMenuAction::Skip,
        3 => ResidualCreditsMenuAction::Finished,
        4 => ResidualCreditsMenuAction::Shutdown,
        _ => ResidualCreditsMenuAction::None,
    }
}

/// Residual: CreditsMenu active latch (independent of live layout/audio).
pub fn residual_credits_menu_is_active() -> bool {
    RESIDUAL_CREDITS_ACTIVE.load(std::sync::atomic::Ordering::Relaxed)
}

fn with_credits_menu_mut<R>(f: impl FnOnce(&mut CreditsMenu) -> R) -> R {
    let menu = {
        let manager = get_menu_manager();
        let guard = manager.read().unwrap_or_else(|e| e.into_inner());
        guard.get_credits_menu()
    };
    let mut menu = menu.write().unwrap_or_else(|e| e.into_inner());
    f(&mut menu)
}

/// Residual: bind CreditsMenu parent control ID (no INI/audio/layout load).
pub fn simulate_credits_menu_bind_controls() -> bool {
    with_credits_menu_mut(|menu| {
        menu.parent_id =
            NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
        menu.initialized = true;
        RESIDUAL_CREDITS_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
        residual_credits_action_store(ResidualCreditsMenuAction::Bind);
        true
    })
}

/// Residual: ESC skip path without shell pop.
pub fn simulate_credits_menu_skip() -> bool {
    with_credits_menu_mut(|menu| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
            menu.initialized = true;
        }
        RESIDUAL_CREDITS_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
        residual_credits_action_store(ResidualCreditsMenuAction::Skip);
        true
    })
}

/// Residual: credits finished path without shell pop.
pub fn simulate_credits_menu_finished() -> bool {
    with_credits_menu_mut(|menu| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
            menu.initialized = true;
        }
        RESIDUAL_CREDITS_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
        residual_credits_action_store(ResidualCreditsMenuAction::Finished);
        true
    })
}

/// Residual: shutdown latch without audio/layout teardown.
pub fn simulate_credits_menu_shutdown() -> bool {
    with_credits_menu_mut(|menu| {
        menu.initialized = false;
        menu.parent = None;
        menu.parent_id = 0;
        menu.credits = None;
        RESIDUAL_CREDITS_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
        residual_credits_action_store(ResidualCreditsMenuAction::Shutdown);
        true
    })
}

/// Residual: bind + skip composite (MainMenu ButtonCredits exit honesty).
pub fn simulate_credits_menu_prepare_skip() -> bool {
    if !simulate_credits_menu_bind_controls() {
        return false;
    }
    simulate_credits_menu_skip()
}

/// Human click-through: OS LeftDown/Up on `CreditsMenu.wnd:ParentCreditsWindow`
/// (C++ named layout hit; ESC skip latches via residual). Not `simulate_*` first.
pub fn drive_os_wnd_credits_menu_skip_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("CreditsMenu.wnd:ParentCreditsWindow");
    if !clicked {
        return false;
    }
    simulate_credits_menu_skip()
}

pub fn drive_os_wnd_credits_menu_finished_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("CreditsMenu.wnd:ParentCreditsWindow");
    if !clicked {
        return false;
    }
    simulate_credits_menu_finished()
}

pub fn drive_os_wnd_credits_menu_prepare_skip_like_cpp() -> bool {
    let clicked = drive_os_wnd_credits_menu_skip_like_cpp();
    if !clicked {
        return false;
    }
    residual_credits_menu_last_action() == ResidualCreditsMenuAction::Skip
}

#[cfg(test)]
mod credits_os_wnd_tests {
    use super::*;
    use crate::gui::with_window_manager;

    #[test]
    fn os_wnd_credits_menu_skip_hits_parent_then_latches() {
        with_window_manager(|manager| {
            let parent = manager
                .create_window(None, 10, 10, 200, 120)
                .expect("ParentCreditsWindow");
            parent
                .borrow_mut()
                .set_name("CreditsMenu.wnd:ParentCreditsWindow");
            let _ = parent.borrow_mut().hide(false);
        });
        assert!(
            drive_os_wnd_credits_menu_skip_like_cpp(),
            "OS WND click on ParentCreditsWindow must latch skip residual"
        );
        assert_eq!(
            residual_credits_menu_last_action(),
            ResidualCreditsMenuAction::Skip
        );
        assert!(!residual_credits_menu_is_active());
    }

    #[test]
    fn os_wnd_credits_menu_finished_hits_parent_then_latches() {
        with_window_manager(|manager| {
            let parent = manager
                .create_window(None, 10, 140, 200, 120)
                .expect("ParentCreditsWindow finished");
            parent
                .borrow_mut()
                .set_name("CreditsMenu.wnd:ParentCreditsWindow");
            let _ = parent.borrow_mut().hide(false);
        });
        assert!(drive_os_wnd_credits_menu_finished_like_cpp());
        assert_eq!(
            residual_credits_menu_last_action(),
            ResidualCreditsMenuAction::Finished
        );
    }
}

/// Residual: last SinglePlayerMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualSinglePlayerMenuAction {
    None = 0,
    New = 1,
    Load = 2,
    Back = 3,
}

static RESIDUAL_SP_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_SP_BOUND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static RESIDUAL_SP_BUTTON_PUSHED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_sp_action_store(action: ResidualSinglePlayerMenuAction) {
    RESIDUAL_SP_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last SinglePlayerMenu residual action.
pub fn residual_single_player_menu_last_action() -> ResidualSinglePlayerMenuAction {
    match RESIDUAL_SP_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualSinglePlayerMenuAction::New,
        2 => ResidualSinglePlayerMenuAction::Load,
        3 => ResidualSinglePlayerMenuAction::Back,
        _ => ResidualSinglePlayerMenuAction::None,
    }
}

/// Residual: whether SinglePlayerMenu control IDs were bound.
pub fn residual_single_player_menu_is_bound() -> bool {
    RESIDUAL_SP_BOUND.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: button_pushed latch (C++ double-click guard).
pub fn residual_single_player_menu_button_pushed() -> bool {
    RESIDUAL_SP_BUTTON_PUSHED.load(std::sync::atomic::Ordering::Relaxed)
}

fn with_single_player_menu_mut<R>(f: impl FnOnce(&mut SinglePlayerMenu) -> R) -> R {
    let menu = {
        let manager = get_menu_manager();
        let guard = manager.read().unwrap_or_else(|e| e.into_inner());
        guard.get_single_player_menu()
    };
    let mut menu = menu.write().unwrap_or_else(|e| e.into_inner());
    f(&mut menu)
}

/// Residual: bind SinglePlayerMenu control IDs (no layout load).
pub fn simulate_single_player_menu_bind_controls() -> bool {
    with_single_player_menu_mut(|menu| {
        menu.parent_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
        menu.button_new_id = NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
        menu.button_load_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
        menu.button_back_id =
            NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
        menu.initialized = true;
        menu.is_shutting_down = false;
        menu.button_pushed = false;
        RESIDUAL_SP_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        RESIDUAL_SP_BUTTON_PUSHED.store(false, std::sync::atomic::Ordering::Relaxed);
        true
    })
}

/// Residual: fire ButtonNew without shell push MapSelectMenu.
pub fn simulate_single_player_menu_new_button_gadget_selected() -> bool {
    with_single_player_menu_mut(|menu| {
        if !residual_single_player_menu_is_bound() {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
            RESIDUAL_SP_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        RESIDUAL_SP_BUTTON_PUSHED.store(true, std::sync::atomic::Ordering::Relaxed);
        residual_sp_action_store(ResidualSinglePlayerMenuAction::New);
        true
    })
}

/// Residual: fire ButtonLoad without save/load menu open.
pub fn simulate_single_player_menu_load_button_gadget_selected() -> bool {
    with_single_player_menu_mut(|menu| {
        if !residual_single_player_menu_is_bound() {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
            RESIDUAL_SP_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if menu.button_pushed {
            return false;
        }
        // C++ Load path returns Handled without always setting button_pushed in all builds;
        // residual latches Load without shell navigation.
        residual_sp_action_store(ResidualSinglePlayerMenuAction::Load);
        true
    })
}

/// Residual: fire ButtonBack without shell pop.
pub fn simulate_single_player_menu_back_button_gadget_selected() -> bool {
    with_single_player_menu_mut(|menu| {
        if !residual_single_player_menu_is_bound() {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
            RESIDUAL_SP_BOUND.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        RESIDUAL_SP_BUTTON_PUSHED.store(true, std::sync::atomic::Ordering::Relaxed);
        residual_sp_action_store(ResidualSinglePlayerMenuAction::Back);
        true
    })
}

/// Residual: clear button_pushed latch (re-enter honesty).
pub fn simulate_single_player_menu_clear_button_pushed() -> bool {
    with_single_player_menu_mut(|menu| {
        menu.button_pushed = false;
        RESIDUAL_SP_BUTTON_PUSHED.store(false, std::sync::atomic::Ordering::Relaxed);
        true
    })
}

/// Residual: bind + New composite (campaign map select entry honesty).
pub fn simulate_single_player_menu_prepare_new() -> bool {
    if !simulate_single_player_menu_bind_controls() {
        return false;
    }
    simulate_single_player_menu_new_button_gadget_selected()
}

/// Residual: last MapSelectMenu action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualMapSelectMenuAction {
    None = 0,
    SelectMap = 1,
    Ok = 2,
    Back = 3,
    SoloMaps = 4,
    MultiplayerMaps = 5,
    SystemMaps = 6,
    UserMaps = 7,
    DifficultyEasy = 8,
    DifficultyMedium = 9,
    DifficultyHard = 10,
}

static RESIDUAL_MAP_SELECT_ACTION: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_MAP_SELECT_DIFFICULTY: std::sync::atomic::AtomicI32 =
    std::sync::atomic::AtomicI32::new(1);
static RESIDUAL_MAP_SELECT_MAP: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

fn residual_map_select_action_store(action: ResidualMapSelectMenuAction) {
    RESIDUAL_MAP_SELECT_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last MapSelectMenu residual action.
pub fn residual_map_select_menu_last_action() -> ResidualMapSelectMenuAction {
    match RESIDUAL_MAP_SELECT_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualMapSelectMenuAction::SelectMap,
        2 => ResidualMapSelectMenuAction::Ok,
        3 => ResidualMapSelectMenuAction::Back,
        4 => ResidualMapSelectMenuAction::SoloMaps,
        5 => ResidualMapSelectMenuAction::MultiplayerMaps,
        6 => ResidualMapSelectMenuAction::SystemMaps,
        7 => ResidualMapSelectMenuAction::UserMaps,
        8 => ResidualMapSelectMenuAction::DifficultyEasy,
        9 => ResidualMapSelectMenuAction::DifficultyMedium,
        10 => ResidualMapSelectMenuAction::DifficultyHard,
        _ => ResidualMapSelectMenuAction::None,
    }
}

/// Residual: last difficulty (0 easy / 1 medium / 2 hard).
pub fn residual_map_select_menu_difficulty() -> i32 {
    RESIDUAL_MAP_SELECT_DIFFICULTY.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: last selected map path/name.
pub fn residual_map_select_menu_selected_map() -> Option<String> {
    let name = RESIDUAL_MAP_SELECT_MAP
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clone();
    if name.is_empty() { None } else { Some(name) }
}

fn with_map_select_menu_mut<R>(f: impl FnOnce(&mut MapSelectMenu) -> R) -> R {
    let menu = {
        let manager = get_menu_manager();
        let guard = manager.read().unwrap_or_else(|e| e.into_inner());
        guard.get_map_select_menu()
    };
    let mut menu = menu.write().unwrap_or_else(|e| e.into_inner());
    f(&mut menu)
}

fn ensure_map_select_control_ids(menu: &mut MapSelectMenu) {
    if menu.parent_id == 0 {
        menu.parent_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:MapSelectMenuParent") as i32;
    }
    if menu.listbox_map_id == 0 {
        menu.listbox_map_id = NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ListboxMap") as i32;
    }
    if menu.button_ok_id == 0 {
        menu.button_ok_id = NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ButtonOK") as i32;
    }
    if menu.button_back_id == 0 {
        menu.button_back_id = NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ButtonBack") as i32;
    }
    if menu.button_single_player_id == 0 {
        menu.button_single_player_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ButtonSinglePlayer") as i32;
    }
    if menu.button_multiplayer_id == 0 {
        menu.button_multiplayer_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:ButtonMultiplayer") as i32;
    }
    if menu.radio_easy_id == 0 {
        menu.radio_easy_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonEasyAI") as i32;
    }
    if menu.radio_medium_id == 0 {
        menu.radio_medium_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonMediumAI") as i32;
    }
    if menu.radio_hard_id == 0 {
        menu.radio_hard_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonHardAI") as i32;
    }
    if menu.radio_system_maps_id == 0 {
        menu.radio_system_maps_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonSystemMaps") as i32;
    }
    if menu.radio_user_maps_id == 0 {
        menu.radio_user_maps_id =
            NameKeyGenerator::name_to_key("MapSelectMenu.wnd:RadioButtonUserMaps") as i32;
    }
}

/// Residual: bind MapSelectMenu control IDs (no layout/list populate).
pub fn simulate_map_select_menu_bind_controls() -> bool {
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        menu.initialized = true;
        menu.button_pushed = false;
        menu.is_shutting_down = false;
        menu.start_game = false;
        true
    })
}

/// Residual: select a map path without live listbox.
pub fn simulate_map_select_menu_select_map(map_path: &str) -> bool {
    if map_path.is_empty() {
        return false;
    }
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        menu.selected_map = Some(map_path.to_string());
        *RESIDUAL_MAP_SELECT_MAP
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = map_path.to_string();
        residual_map_select_action_store(ResidualMapSelectMenuAction::SelectMap);
        residual_map_select_menu_selected_map().as_deref() == Some(map_path)
    })
}

/// Residual: set AI difficulty radio (0/1/2) without widgets.
pub fn simulate_map_select_menu_set_difficulty(difficulty: i32) -> bool {
    if !(0..=2).contains(&difficulty) {
        return false;
    }
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        menu.difficulty = difficulty;
        RESIDUAL_MAP_SELECT_DIFFICULTY.store(difficulty, std::sync::atomic::Ordering::Relaxed);
        residual_map_select_action_store(match difficulty {
            0 => ResidualMapSelectMenuAction::DifficultyEasy,
            2 => ResidualMapSelectMenuAction::DifficultyHard,
            _ => ResidualMapSelectMenuAction::DifficultyMedium,
        });
        residual_map_select_menu_difficulty() == difficulty
    })
}

/// Residual: fire ButtonOK without start_game engine transfer.
pub fn simulate_map_select_menu_ok_button_gadget_selected() -> bool {
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        if menu.button_pushed {
            return false;
        }
        if menu.selected_map.is_none() && residual_map_select_menu_selected_map().is_none() {
            // C++ ignores OK with no selection.
            return false;
        }
        if menu.selected_map.is_none() {
            menu.selected_map = residual_map_select_menu_selected_map();
        }
        menu.button_pushed = true;
        // Do not call start_game(); residual latch only.
        residual_map_select_action_store(ResidualMapSelectMenuAction::Ok);
        true
    })
}

/// Residual: fire ButtonBack without shell pop.
pub fn simulate_map_select_menu_back_button_gadget_selected() -> bool {
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        residual_map_select_action_store(ResidualMapSelectMenuAction::Back);
        true
    })
}

/// Residual: ButtonSinglePlayer map filter residual.
pub fn simulate_map_select_menu_solo_maps_button_gadget_selected() -> bool {
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        menu.show_solo_maps = true;
        residual_map_select_action_store(ResidualMapSelectMenuAction::SoloMaps);
        true
    })
}

/// Residual: ButtonMultiplayer map filter residual.
pub fn simulate_map_select_menu_multiplayer_maps_button_gadget_selected() -> bool {
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        menu.show_solo_maps = false;
        residual_map_select_action_store(ResidualMapSelectMenuAction::MultiplayerMaps);
        true
    })
}

/// Residual: RadioButtonSystemMaps residual.
pub fn simulate_map_select_menu_system_maps_radio_selected() -> bool {
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        menu.use_system_maps = true;
        residual_map_select_action_store(ResidualMapSelectMenuAction::SystemMaps);
        true
    })
}

/// Residual: RadioButtonUserMaps residual.
pub fn simulate_map_select_menu_user_maps_radio_selected() -> bool {
    with_map_select_menu_mut(|menu| {
        ensure_map_select_control_ids(menu);
        menu.use_system_maps = false;
        residual_map_select_action_store(ResidualMapSelectMenuAction::UserMaps);
        true
    })
}

/// Residual: clear button_pushed latch.
pub fn simulate_map_select_menu_clear_button_pushed() -> bool {
    with_map_select_menu_mut(|menu| {
        menu.button_pushed = false;
        menu.start_game = false;
        true
    })
}

/// Residual: select map + medium AI + OK composite (campaign start honesty).
pub fn simulate_map_select_menu_prepare_ok(map_path: &str) -> bool {
    if !simulate_map_select_menu_bind_controls() {
        return false;
    }
    let _ = simulate_map_select_menu_clear_button_pushed();
    if !simulate_map_select_menu_select_map(map_path) {
        return false;
    }
    if !simulate_map_select_menu_set_difficulty(1) {
        return false;
    }
    simulate_map_select_menu_ok_button_gadget_selected()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_global_data_residences_restored(f: impl FnOnce()) {
        runtime_global_data::with_global_data_restored(|| {
            let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
            let ini_snapshot = ini_global.read().clone();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
            *ini_global.write() = ini_snapshot;
            if let Err(payload) = result {
                std::panic::resume_unwind(payload);
            }
        });
    }

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

    #[test]
    fn options_menu_esc_char_is_consumed_before_key_up_like_cpp() {
        let mut menu = OptionsMenu::new();
        let window = GameWindow::new();

        assert_eq!(
            menu.input(&window, WindowMessage::Char, 0x1B, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            menu.input(&window, WindowMessage::Char, 0x1B, 0x0001),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            menu.input(&window, WindowMessage::Char, b'A' as WindowMsgData, 0),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn options_sfx_slider_splits_2d_and_3d_volume_like_cpp() {
        fn assert_split(relative: f32, expected_2d: i32, expected_3d: i32) {
            let (volume_2d, volume_3d, factor_2d, factor_3d) =
                OptionsMenu::split_sfx_volume_for_relative(80, relative);
            assert_eq!((volume_2d, volume_3d), (expected_2d, expected_3d));
            assert!((factor_2d - expected_2d as f32 / 100.0).abs() < 0.001);
            assert!((factor_3d - expected_3d as f32 / 100.0).abs() < 0.001);
        }

        assert_split(-0.25, 60, 80);
        assert_split(0.0, 80, 80);
        assert_split(0.25, 80, 60);
        assert_split(2.0, 80, 0);
    }

    #[test]
    fn options_defaults_resolution_index_prefers_800x600_like_cpp() {
        let mut menu = OptionsMenu::new();
        menu.resolution_modes = vec![(1024, 768), (800, 600), (1920, 1080)];
        assert_eq!(menu.default_resolution_index(), 1);

        menu.resolution_modes = vec![(1024, 768), (1280, 720)];
        assert_eq!(menu.default_resolution_index(), 0);
    }

    #[test]
    fn options_checkbox_immediate_effects_match_cpp() {
        let original_draw_anchor = TheInGameUI::get_draw_rmb_scroll_anchor();
        let original_move_anchor = TheInGameUI::get_move_rmb_scroll_anchor();
        let (original_save_camera, original_use_camera) = {
            let global = runtime_global_data::read();
            (global.save_camera_in_replay, global.use_camera_in_replay)
        };

        let mut menu = OptionsMenu::new();
        menu.check_draw_anchor_id = 11;
        menu.check_move_anchor_id = 12;
        menu.check_save_camera_id = 13;
        menu.check_use_camera_id = 14;

        assert!(menu.apply_immediate_checkbox_effect(menu.check_draw_anchor_id, true));
        assert!(TheInGameUI::get_draw_rmb_scroll_anchor());

        assert!(menu.apply_immediate_checkbox_effect(menu.check_move_anchor_id, true));
        assert!(TheInGameUI::get_move_rmb_scroll_anchor());

        assert!(menu.apply_immediate_checkbox_effect(menu.check_save_camera_id, false));
        assert!(!runtime_global_data::read().save_camera_in_replay);

        assert!(menu.apply_immediate_checkbox_effect(menu.check_use_camera_id, false));
        assert!(!runtime_global_data::read().use_camera_in_replay);

        assert!(!menu.apply_immediate_checkbox_effect(99, true));

        TheInGameUI::set_draw_rmb_scroll_anchor(original_draw_anchor);
        TheInGameUI::set_move_rmb_scroll_anchor(original_move_anchor);
        let mut global = runtime_global_data::write();
        global.save_camera_in_replay = original_save_camera;
        global.use_camera_in_replay = original_use_camera;
    }

    #[test]
    fn options_move_rmb_scroll_anchor_publishes_only_to_an_enabled_host() {
        use crate::gui::options_host_bridge::{
            HostOptionsRequest, acquire_host_options_bridge_test_guard,
            set_host_options_bridge_enabled, take_host_options_requests,
        };

        let _bridge_guard = acquire_host_options_bridge_test_guard();
        let original_move_anchor = TheInGameUI::get_move_rmb_scroll_anchor();
        let mut menu = OptionsMenu::new();
        menu.check_move_anchor_id = 12;

        set_host_options_bridge_enabled(true);
        assert!(menu.apply_immediate_checkbox_effect(menu.check_move_anchor_id, true));
        assert_eq!(
            take_host_options_requests(),
            vec![HostOptionsRequest::MoveRmbScrollAnchor { enabled: true }]
        );
        assert!(TheInGameUI::get_move_rmb_scroll_anchor());

        set_host_options_bridge_enabled(false);
        assert!(menu.apply_immediate_checkbox_effect(menu.check_move_anchor_id, false));
        assert!(take_host_options_requests().is_empty());
        assert!(
            !TheInGameUI::get_move_rmb_scroll_anchor(),
            "disabled host delivery must retain the standalone legacy callback"
        );

        TheInGameUI::set_move_rmb_scroll_anchor(original_move_anchor);
    }

    #[test]
    fn options_draw_rmb_scroll_anchor_publishes_only_to_an_enabled_host() {
        use crate::gui::options_host_bridge::{
            HostOptionsRequest, acquire_host_options_bridge_test_guard,
            set_host_options_bridge_enabled, take_host_options_requests,
        };

        let _bridge_guard = acquire_host_options_bridge_test_guard();
        let original_draw_anchor = TheInGameUI::get_draw_rmb_scroll_anchor();
        let mut menu = OptionsMenu::new();
        menu.check_draw_anchor_id = 11;

        set_host_options_bridge_enabled(true);
        assert!(menu.apply_immediate_checkbox_effect(menu.check_draw_anchor_id, true));
        assert_eq!(
            take_host_options_requests(),
            vec![HostOptionsRequest::DrawRmbScrollAnchor { enabled: true }]
        );
        assert!(TheInGameUI::get_draw_rmb_scroll_anchor());

        set_host_options_bridge_enabled(false);
        assert!(menu.apply_immediate_checkbox_effect(menu.check_draw_anchor_id, false));
        assert!(take_host_options_requests().is_empty());
        assert!(
            !TheInGameUI::get_draw_rmb_scroll_anchor(),
            "disabled host delivery must retain the standalone legacy callback"
        );

        TheInGameUI::set_draw_rmb_scroll_anchor(original_draw_anchor);
    }

    #[test]
    fn options_accept_converges_alternate_mouse_residences_and_publishes_to_main() {
        use crate::gui::options_host_bridge::{
            HostOptionsRequest, acquire_host_options_bridge_test_guard,
            set_host_options_bridge_enabled, take_host_options_requests,
        };

        let _bridge_guard = acquire_host_options_bridge_test_guard();
        set_host_options_bridge_enabled(true);
        with_global_data_residences_restored(|| {
            let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
            ini_global.write().use_alternate_mouse = false;
            runtime_global_data::write().use_alternate_mouse = true;

            OptionsMenu::commit_alternate_mouse_setting(true);
            assert!(ini_global.read().use_alternate_mouse);
            assert!(runtime_global_data::read().use_alternate_mouse);
            assert_eq!(
                take_host_options_requests(),
                vec![HostOptionsRequest::AlternateMouse { enabled: true }]
            );

            OptionsMenu::commit_alternate_mouse_setting(false);
            assert!(!ini_global.read().use_alternate_mouse);
            assert!(!runtime_global_data::read().use_alternate_mouse);
            assert_eq!(
                take_host_options_requests(),
                vec![HostOptionsRequest::AlternateMouse { enabled: false }]
            );
        });
    }

    #[test]
    fn map_select_double_click_uses_event_row_selection_like_cpp() {
        let mut menu = MapSelectMenu::new();
        menu.listbox_map_id = 42;
        menu.button_ok_id = 77;

        let mut listbox = crate::gui::gadgets::ListBox::new(42, 0, 0, 200, 80);
        listbox.add_item_with_data(
            0,
            "first",
            Some(ListBoxItemData::Text("Maps\\First\\First.map".to_string())),
        );
        listbox.add_item_with_data(
            1,
            "second",
            Some(ListBoxItemData::Text(
                "Maps\\Second\\Second.map".to_string(),
            )),
        );
        listbox.set_selected_indices(&[1]);

        let listbox_window = Rc::new(RefCell::new(GameWindow::new()));
        listbox_window
            .borrow_mut()
            .set_widget(WindowWidget::ListBox(listbox));
        menu.listbox_map = Some(listbox_window.clone());

        let window = GameWindow::new();

        assert_eq!(
            menu.system(&window, WindowMessage::User(GLM_DOUBLE_CLICKED), 42, 0),
            WindowMsgHandled::Handled
        );

        assert_eq!(menu.selected_map.as_deref(), Some("Maps\\First\\First.map"));
        let selected = listbox_window
            .borrow()
            .widget()
            .and_then(|widget| match widget {
                WindowWidget::ListBox(listbox) => listbox.selected_indices().first().copied(),
                _ => None,
            });
        assert_eq!(selected, Some(0));
    }

    #[test]
    fn map_select_start_game_mirrors_selected_map_to_both_global_data_residences() {
        with_global_data_residences_restored(|| {
            let selected_map = "Maps\\Official\\MapSelectExact.map".to_string();
            let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
            ini_global.write().pending_file = "Maps\\Legacy\\Ini.map".to_string();
            runtime_global_data::write().pending_file = "Maps\\Legacy\\Runtime.map".to_string();

            let mut menu = MapSelectMenu::new();
            menu.selected_map = Some(selected_map.clone());
            menu.start_game();

            assert!(menu.start_game);
            assert_eq!(ini_global.read().pending_file, selected_map);
            assert_eq!(runtime_global_data::read().pending_file, selected_map);
        });
    }

    #[test]
    fn map_select_start_game_without_selection_preserves_both_pending_maps() {
        with_global_data_residences_restored(|| {
            let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
            ini_global.write().pending_file = "Maps\\Legacy\\Ini.map".to_string();
            runtime_global_data::write().pending_file = "Maps\\Legacy\\Runtime.map".to_string();

            let mut menu = MapSelectMenu::new();
            menu.start_game();

            assert!(!menu.start_game);
            assert_eq!(ini_global.read().pending_file, "Maps\\Legacy\\Ini.map");
            assert_eq!(
                runtime_global_data::read().pending_file,
                "Maps\\Legacy\\Runtime.map"
            );
        });
    }
}

#[cfg(test)]
mod menu_callbacks_shell_borrow_residual_tests {
    #[test]
    fn single_player_menu_avoids_nested_get_shell_on_shutdown() {
        let src = include_str!("menu_callbacks.rs");
        assert!(
            src.contains("queue_shell_shutdown_complete(false);"),
            "SinglePlayerMenu::shutdown_complete must queue the lifecycle completion"
        );
        assert!(
            src.contains("show_shell_map_if_available(true)"),
            "SinglePlayerMenu init must use show_shell_map_if_available"
        );
        assert!(
            src.contains("queue_shell_reverse_animate_window();"),
            "reverse_animate_window must queue at the shell lifecycle boundary"
        );
        assert!(
            !src.contains("get_shell().pop()")
                && !src.contains("get_shell().push(")
                && !src.contains("get_shell().reverse_animate_window()"),
            "menu_callbacks must not call get_shell() for push/pop/reverse during nested shell"
        );
    }
}
