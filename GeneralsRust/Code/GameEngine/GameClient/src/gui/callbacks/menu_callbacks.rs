//! Menu Callback Functions
//!
//! This module contains callback functions for all the shell menus,
//! including main menu, single player menu, options menu, etc.

use crate::core::game_client::apply_lod_texture_reduction;
use crate::game_text::GameText;
use crate::gui::callbacks::quit_menu::destroy_quit_menu;
use crate::gui::gadgets::ComboBoxItem;
use crate::gui::gadgets::ListBoxItemData;
use crate::gui::game_window::WindowEventEffects;
use crate::gui::header_template::get_header_template_manager;
use crate::gui::options_host_bridge::HostOptionsRequest;
use crate::gui::shell::main_menu::DisplaySettings;
use crate::gui::{
    AnimationType, GLM_DOUBLE_CLICKED, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowWidget, queue_shell_pop, queue_shell_push,
    queue_shell_reverse_animate_window, queue_shell_shutdown_complete,
    queue_shell_window_animation, show_shell_map_if_available, write_input_focus_response,
};

use crate::map_util::{get_map_cache_manager, populate_map_listbox};
use crate::message_stream::{GameMessage, GameMessageType};
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

fn publish_host_option(effects: &mut WindowEventEffects<'_>, request: HostOptionsRequest) {
    if let Some(host) = effects.menu_host.as_deref_mut() {
        host.options.publish(request);
    }
}

/// Menu callback trait
pub trait MenuCallbacks {
    /// Initialize the menu
    fn init(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        _effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.init(layout, user_data, _effects)
    }

    /// Update the menu (called every frame)
    fn update(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    /// Update using the receiving GameClient's borrowed event effects.
    fn update_with_effects(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        _effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.update(layout, user_data, _effects)
    }

    /// Shutdown the menu
    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>>;

    fn shutdown_with_effects(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        _effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.shutdown(layout, user_data, _effects)
    }

    /// Handle system messages
    fn system(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled;

    /// Dispatch using the receiving GameClient's borrowed event effects.
    fn system_with_effects(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        _effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        self.system(window, msg, data1, data2, _effects)
    }

    /// Handle input messages
    fn input(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled;
    fn input_with_effects(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        _effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        self.input(window, msg, data1, data2, _effects)
    }
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Update main menu state
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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
        _window: &mut GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        debug!("Main Menu system message: {:?}", msg);
        WindowMsgHandled::Ignored
    }

    fn input(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        debug!("Main Menu input message: {:?}", msg);
        WindowMsgHandled::Ignored
    }
}

/// Single Player Menu implementation
pub struct SinglePlayerMenu {
    residual_action: ResidualSinglePlayerMenuAction,

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
            residual_action: ResidualSinglePlayerMenuAction::None,

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

    fn initialize_with_effects(
        &mut self,
        layout: &WindowLayout,
        effects: &mut WindowEventEffects<'_>,
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

        {
            show_shell_map_if_available(true, effects);
        }
        layout.hide_receiving(false, effects);

        effects.with_manager(|manager, effects| {
            self.parent = manager.get_window_by_id(self.parent_id);
            // Focus is deferred below: an inline set_focus sends InputFocus to
            // this same window, whose "SinglePlayerMenuSystem" callback would
            // re-enter this init dispatch's menu borrow (RefCell disallows
            // that — it used to re-take a non-re-entrant std RwLock and wedge).
            if let Some(parent) = self.parent.clone() {
                effects.queue_window_operation(move |manager, effects| {
                    let _ = manager.set_focus(Some(&parent), effects);
                });
            }
            if let Some(button_new) = manager.get_window_by_id(self.button_new_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_new,
                        AnimationType::SlideLeft,
                        true,
                        1,
                    )
                });
            }
            if let Some(button_load) = manager.get_window_by_id(self.button_load_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_load,
                        AnimationType::SlideLeft,
                        true,
                        200,
                    )
                });
            }
            if let Some(button_back) = manager.get_window_by_id(self.button_back_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_back,
                        AnimationType::SlideRight,
                        true,
                        1,
                    )
                });
            }
        });

        self.initialized = true;
        Ok(())
    }

    fn shutdown_complete(
        &mut self,
        layout: &WindowLayout,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.is_shutting_down = false;
        layout.hide_receiving(true, effects);
        self.initialized = false;
        self.parent = None;
        queue_shell_shutdown_complete(effects, false);
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.initialize_with_effects(layout, effects)
    }

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.initialize_with_effects(layout, effects)
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_shutting_down && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
        {
            self.shutdown_complete(layout, effects);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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
            self.shutdown_complete(layout, effects);
            return Ok(());
        }
        queue_shell_reverse_animate_window(effects);
        Ok(())
    }

    fn system(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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
                    queue_shell_push(effects, "Menus/MapSelectMenu.wnd", false);
                    self.button_pushed = true;
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_back_id {
                    queue_shell_pop(effects);
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
        _window: &mut GameWindow,
        _msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        panic!("menu input dispatch requires receiving event effects");
    }

    fn input_with_effects(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        if msg != WindowMessage::Char || self.button_pushed || data1 != 0x1B {
            return WindowMsgHandled::Ignored;
        }
        if (data2 & 0x0001) == 0 {
            return WindowMsgHandled::Handled;
        }

        if let Some(parent) = self.parent.clone() {
            effects.queue_system_message(
                Rc::downgrade(&parent),
                WindowMessage::GadgetSelected,
                self.button_back_id as WindowMsgData,
                self.button_back_id as WindowMsgData,
            );
        }

        WindowMsgHandled::Handled
    }
}

/// Immutable observation at native ShowOptions, never a mutable input authority.
#[derive(Clone, Copy, Debug)]
pub struct OptionsCameraInputObservation {
    pub scroll_factor: f32,
    pub default_scroll_factor: f32,
    pub save_camera: bool,
    pub use_camera: bool,
}
impl OptionsCameraInputObservation {
    pub fn observe(
        rules: game_engine::common::ini::ini_game_data::CameraInputRules,
        preferences: &UserPreferences,
    ) -> Self {
        Self {
            scroll_factor: rules.keyboard_scroll_factor(preferences),
            default_scroll_factor: rules.keyboard_default_scroll_factor,
            save_camera: preferences.save_camera_in_replays(),
            use_camera: preferences.use_camera_in_replays(),
        }
    }
}
/// Apply only to windows from the actual admitted receiving layout.
fn set_owned_options_scroll(layout: &WindowLayout, value: f32, range: bool) {
    let id = game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
        "OptionsMenu.wnd:SliderScrollSpeed",
    ) as i32;
    if let Some(window) = layout
        .windows()
        .iter()
        .find(|window| window.borrow().get_id() == id)
    {
        if let Some(slider) = window.borrow_mut().horizontal_slider_mut() {
            if range {
                slider.set_range(0, 100);
            }
            slider.set_value((value * 100.0) as i32);
        }
    }
}
/// Options Menu implementation
pub struct OptionsMenu {
    residual_action: ResidualOptionsMenuAction,
    pending_host_apply: Option<HostOptionsApply>,
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
            residual_action: ResidualOptionsMenuAction::None,
            pending_host_apply: None,
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

    fn find_window(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        (|manager: &mut crate::gui::window_manager::WindowManager| manager.get_window_by_id(id))(
            receiving_manager,
        )
    }

    fn set_checkbox(
        id: i32,
        value: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager) {
            let _ = window
                .borrow_mut()
                .gadget_check_box_set_checked(value, effects);
        }
    }

    fn checkbox_value(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> bool {
        Self::find_window(id, receiving_manager)
            .and_then(|window| {
                let guard = window.borrow();
                match guard.widget() {
                    Some(WindowWidget::CheckBox(check_box)) => Some(check_box.is_checked()),
                    _ => None,
                }
            })
            .unwrap_or(false)
    }

    fn set_slider_value(
        id: i32,
        value: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager) {
            if let Some(slider) = window.borrow_mut().horizontal_slider_mut() {
                slider.set_value(value);
            }
        }
    }

    fn set_slider_range_and_value(
        id: i32,
        min_value: i32,
        max_value: i32,
        value: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager) {
            if let Some(slider) = window.borrow_mut().horizontal_slider_mut() {
                slider.set_range(min_value, max_value);
                slider.set_value(value);
            }
        }
    }

    fn slider_value(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> i32 {
        Self::slider_value_opt(id, receiving_manager).unwrap_or(0)
    }

    /// C++ `GadgetSliderGetPosition` — `None` matches retail `-1` (missing gadget).
    fn slider_value_opt(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Option<i32> {
        Self::find_window(id, receiving_manager).and_then(|window| {
            let guard = window.borrow();
            match guard.widget() {
                Some(WindowWidget::HorizontalSlider(slider)) => Some(slider.value()),
                _ => None,
            }
        })
    }

    fn set_combo_items(
        id: i32,
        items: &[String],
        selected_index: usize,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if items.is_empty() {
            return;
        }
        if let Some(window) = Self::find_window(id, receiving_manager) {
            let mut guard = window.borrow_mut();
            let Some(combo_box) = guard.combo_box_mut() else {
                return;
            };
            combo_box.clear();
            for (index, item) in items.iter().enumerate() {
                combo_box.add_item(ComboBoxItem::new(index as u32, item.clone()));
            }
            guard.set_combo_box_selected(selected_index.min(items.len() - 1), true, effects);
        }
    }

    fn combo_selected_index(
        id: i32,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) -> Option<usize> {
        Self::find_window(id, receiving_manager).and_then(|window| {
            let guard = window.borrow();
            match guard.widget() {
                Some(WindowWidget::ComboBox(combo_box)) => combo_box.selected_index(),
                _ => None,
            }
        })
    }

    fn set_combo_selected(
        id: i32,
        index: usize,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager) {
            window
                .borrow_mut()
                .set_combo_box_selected(index, true, effects);
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

    fn set_window_hidden(
        id: i32,
        hidden: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager) {
            let _ = window.borrow_mut().hide(hidden, receiving_manager);
        }
    }

    fn set_window_enabled(
        id: i32,
        enabled: bool,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        if let Some(window) = Self::find_window(id, receiving_manager) {
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

    fn apply_immediate_checkbox_effect(
        &self,
        control_id: i32,
        checked: bool,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        if control_id == self.check_draw_anchor_id {
            effects.ui_mut().set_draw_rmb_scroll_anchor(checked);
            // Main owns the active RMB camera-drag presentation for an
            // AuthorityOnly match. Preserve the legacy standalone state, then
            // publish the same visual preference only while Main hosts it.
            publish_host_option(
                effects,
                HostOptionsRequest::DrawRmbScrollAnchor { enabled: checked },
            );
            return true;
        }
        if control_id == self.check_move_anchor_id {
            effects.ui_mut().set_move_rmb_scroll_anchor(checked);
            // Main owns live camera input for AuthorityOnly. Preserve this
            // standalone GameClient update, then send the same typed checkbox
            // choice to Main only while its host bridge is installed.
            publish_host_option(
                effects,
                HostOptionsRequest::MoveRmbScrollAnchor { enabled: checked },
            );
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

    fn populate_controls(
        &mut self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.populate_controls_with_camera_input(None, receiving_manager, effects);
    }
    fn populate_controls_with_camera_input(
        &mut self,
        owned: Option<(&WindowLayout, &OptionsCameraInputObservation)>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
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
        let save_camera = owned
            .map(|(_, view)| view.save_camera)
            .unwrap_or_else(|| pref.save_camera_in_replays());
        let use_camera = owned
            .map(|(_, view)| view.use_camera)
            .unwrap_or_else(|| pref.use_camera_in_replays());
        let draw_anchor = game_engine::common::user_preferences::scroll_anchor_pref_enabled(
            pref.get_string("DrawScrollAnchor")
                .map(|value| value.as_str()),
            effects.ui().get_draw_rmb_scroll_anchor(),
        );
        let move_anchor = game_engine::common::user_preferences::scroll_anchor_pref_enabled(
            pref.get_string("MoveScrollAnchor")
                .map(|value| value.as_str()),
            effects.ui().get_move_rmb_scroll_anchor(),
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
        let scroll_speed = if owned.is_none() {
            Some(pref.get_int_or(
                "ScrollFactor",
                (global.keyboard_default_scroll_factor * 100.0) as i32,
            ))
        } else {
            None
        };
        let anti_aliasing = pref
            .get_int_or("AntiAliasing", global.anti_alias_box_value)
            .clamp(0, 2) as usize;
        // First-open LOD applies settings through the same runtime owner.
        // End the observation before that mutation, then observe its results.
        drop(global);
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

        let global = runtime_global_data::read();
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
            receiving_manager,
            effects,
        );
        Self::set_combo_items(
            self.combo_resolution_id,
            &self
                .resolution_modes
                .iter()
                .map(|mode| Self::resolution_label(*mode))
                .collect::<Vec<_>>(),
            resolution_index,
            receiving_manager,
            effects,
        );
        Self::set_combo_items(
            self.combo_detail_id,
            &Self::detail_labels(),
            detail_index,
            receiving_manager,
            effects,
        );

        Self::set_checkbox(
            self.check_alternate_mouse_id,
            alternate_mouse,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_retaliation_id,
            retaliation,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_double_click_attack_move_id,
            double_click_attack_move,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_language_filter_id,
            language_filter,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_send_delay_id,
            send_delay,
            receiving_manager,
            effects,
        );
        if let Some((layout, _)) = owned {
            for (name, checked) in [
                ("OptionsMenu.wnd:CheckBoxSaveCamera", save_camera),
                ("OptionsMenu.wnd:CheckBoxUseCamera", use_camera),
            ] {
                let id =
                    game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(name)
                        as i32;
                if let Some(window) = layout
                    .windows()
                    .iter()
                    .find(|window| window.borrow().get_id() == id)
                {
                    if let Some(checkbox) = window.borrow_mut().check_box_mut() {
                        checkbox.set_checked(checked);
                    }
                }
            }
        } else {
            Self::set_checkbox(
                self.check_save_camera_id,
                save_camera,
                receiving_manager,
                effects,
            );
            Self::set_checkbox(
                self.check_use_camera_id,
                use_camera,
                receiving_manager,
                effects,
            );
        }
        Self::set_checkbox(
            self.check_draw_anchor_id,
            draw_anchor,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_move_anchor_id,
            move_anchor,
            receiving_manager,
            effects,
        );
        // C++ OptionsMenuInit applies these to TheInGameUI on open.
        effects.ui_mut().set_draw_rmb_scroll_anchor(draw_anchor);
        effects.ui_mut().set_move_rmb_scroll_anchor(move_anchor);
        publish_host_option(
            effects,
            HostOptionsRequest::DrawRmbScrollAnchor {
                enabled: draw_anchor,
            },
        );
        publish_host_option(
            effects,
            HostOptionsRequest::MoveRmbScrollAnchor {
                enabled: move_anchor,
            },
        );
        Self::set_checkbox(
            self.check_3d_shadows_id,
            use_shadow_volumes,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_2d_shadows_id,
            use_shadow_decals,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_cloud_shadows_id,
            use_cloud_map,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_ground_lighting_id,
            use_light_map,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_smooth_water_id,
            show_soft_water_edge,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_extra_animations_id,
            extra_animations,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_no_dynamic_lod_id,
            no_dynamic_lod,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_unlock_fps_id,
            unlock_fps,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_heat_effects_id,
            heat_effects,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_building_occlusion_id,
            building_occlusion,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_props_id, show_props, receiving_manager, effects);

        if let Some((layout, observation)) = owned {
            set_owned_options_scroll(layout, observation.scroll_factor, true);
        } else if let Some(scroll_speed) = scroll_speed {
            Self::set_slider_range_and_value(
                self.slider_scroll_speed_id,
                0,
                100,
                scroll_speed,
                receiving_manager,
            );
        }
        Self::set_slider_range_and_value(
            self.slider_music_volume_id,
            0,
            100,
            music_volume,
            receiving_manager,
        );
        Self::set_slider_range_and_value(
            self.slider_sfx_volume_id,
            0,
            100,
            sfx_volume,
            receiving_manager,
        );
        Self::set_slider_range_and_value(
            self.slider_voice_volume_id,
            0,
            100,
            voice_volume,
            receiving_manager,
        );
        Self::set_slider_range_and_value(self.slider_gamma_id, 0, 100, gamma, receiving_manager);
        Self::set_slider_range_and_value(
            self.slider_texture_resolution_id,
            0,
            2,
            texture_resolution,
            receiving_manager,
        );
        Self::set_slider_range_and_value(
            self.slider_particle_cap_id,
            100,
            10000,
            particle_cap,
            receiving_manager,
        );

        Self::set_window_hidden(self.advanced_window_id, true, receiving_manager);
    }

    fn apply_default_controls(
        &mut self,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.apply_default_controls_with_scroll(None, receiving_manager, effects);
    }
    fn apply_default_controls_with_scroll(
        &mut self,
        owned: Option<(Option<Rc<RefCell<GameWindow>>>, f32)>,
        receiving_manager: &mut crate::gui::window_manager::WindowManager,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        Self::set_checkbox(
            self.check_language_filter_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_send_delay_id, false, receiving_manager, effects);
        Self::set_checkbox(
            self.check_alternate_mouse_id,
            false,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_retaliation_id, true, receiving_manager, effects);
        Self::set_checkbox(
            self.check_double_click_attack_move_id,
            false,
            receiving_manager,
            effects,
        );
        // C++ setDefaults never touches replay-camera or RMB-anchor checkboxes.

        if let Some((slider, default)) = owned {
            if let Some(slider) = slider {
                if let Some(slider) = slider.borrow_mut().horizontal_slider_mut() {
                    slider.set_value((default * 100.0) as i32);
                }
            }
        } else {
            let default_scroll =
                (runtime_global_data::read().keyboard_default_scroll_factor * 100.0).round() as i32;
            Self::set_slider_value(
                self.slider_scroll_speed_id,
                default_scroll,
                receiving_manager,
            );
        }
        Self::set_slider_value(self.slider_music_volume_id, 60, receiving_manager);
        Self::set_slider_value(self.slider_sfx_volume_id, 55, receiving_manager);
        Self::set_slider_value(self.slider_voice_volume_id, 70, receiving_manager);
        Self::set_slider_value(self.slider_gamma_id, 50, receiving_manager);
        Self::set_slider_value(self.slider_texture_resolution_id, 2, receiving_manager);
        Self::set_slider_value(self.slider_particle_cap_id, 5000, receiving_manager);

        Self::set_checkbox(self.check_3d_shadows_id, true, receiving_manager, effects);
        Self::set_checkbox(self.check_2d_shadows_id, true, receiving_manager, effects);
        Self::set_checkbox(
            self.check_cloud_shadows_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_ground_lighting_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_smooth_water_id, true, receiving_manager, effects);
        Self::set_checkbox(
            self.check_extra_animations_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(
            self.check_no_dynamic_lod_id,
            false,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_unlock_fps_id, false, receiving_manager, effects);
        Self::set_checkbox(self.check_heat_effects_id, true, receiving_manager, effects);
        Self::set_checkbox(
            self.check_building_occlusion_id,
            true,
            receiving_manager,
            effects,
        );
        Self::set_checkbox(self.check_props_id, true, receiving_manager, effects);
        // C++ setDefaults: comboBoxDetail = findStaticLODLevel()
        let recommended = game_engine::common::game_lod::find_static_lod_level();
        Self::set_combo_selected(
            self.combo_detail_id,
            Self::detail_index_from_name(&recommended),
            receiving_manager,
            effects,
        );

        if Self::should_reset_resolution_on_defaults() {
            Self::set_combo_selected(
                self.combo_resolution_id,
                self.default_resolution_index(),
                receiving_manager,
                effects,
            );
        }
        Self::set_window_hidden(self.advanced_window_id, true, receiving_manager);
    }

    fn apply_options(&mut self, effects: &mut WindowEventEffects<'_>) -> bool {
        let host = self.pending_host_apply.take();
        let mut preferences = UserPreferences::new();
        let _ = preferences.load("Options.ini");
        self.apply_options_with_preferences(host, &mut preferences, None, None, effects)
    }
    fn apply_options_with_preferences(
        &mut self,
        host: Option<HostOptionsApply>,
        pref: &mut UserPreferences,
        owned_scroll: Option<Option<i32>>,
        terrain_lod: Option<&mut dyn FnMut(i32)>,
        effects: &mut WindowEventEffects<'_>,
    ) -> bool {
        let owns_camera_preferences = owned_scroll.is_some();
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
                (!owns_camera_preferences).then(|| (global.keyboard_scroll_factor * 100.0) as i32),
                (global.music_volume_factor * 100.0) as i32,
                (global.sfx_volume_factor * 100.0) as i32,
                (global.voice_volume_factor * 100.0) as i32,
                (global.writable.x_resolution, global.writable.y_resolution),
            )
        };

        let detail_index = host
            .as_ref()
            .map(|h| h.detail_index)
            .or_else(|| {
                effects.with_manager(|receiving_manager, effects| {
                    Self::combo_selected_index(self.combo_detail_id, receiving_manager)
                })
            })
            .unwrap_or(1);
        let anti_aliasing = host
            .as_ref()
            .map(|h| h.anti_aliasing)
            .or_else(|| {
                effects
                    .with_manager(|receiving_manager, effects| {
                        Self::combo_selected_index(self.combo_anti_aliasing_id, receiving_manager)
                    })
                    .map(|i| i as i32)
            })
            .unwrap_or_else(|| pref.get_int_or("AntiAliasing", 0));
        let resolution = host
            .as_ref()
            .map(|h| h.resolution)
            .or_else(|| {
                self.resolution_modes
                    .get(
                        effects
                            .with_manager(|receiving_manager, effects| {
                                Self::combo_selected_index(
                                    self.combo_resolution_id,
                                    receiving_manager,
                                )
                            })
                            .unwrap_or(0),
                    )
                    .copied()
            })
            .unwrap_or_else(|| {
                parse_resolution_pref(pref.get_string("Resolution").map(|s| s.as_str()))
                    .unwrap_or(default_res)
            });

        let alternate_mouse = host.as_ref().map(|h| h.alternate_mouse).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_alternate_mouse_id, receiving_manager)
            })
        });
        let retaliation = host.as_ref().map(|h| h.retaliation).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_retaliation_id, receiving_manager)
            })
        });
        let double_click_attack_move = host
            .as_ref()
            .map(|h| h.double_click_attack_move)
            .unwrap_or_else(|| {
                effects.with_manager(|receiving_manager, effects| {
                    Self::checkbox_value(self.check_double_click_attack_move_id, receiving_manager)
                })
            });
        let language_filter = host.as_ref().map(|h| h.language_filter).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_language_filter_id, receiving_manager)
            })
        });
        let send_delay = effects.with_manager(|receiving_manager, effects| {
            Self::checkbox_value(self.check_send_delay_id, receiving_manager)
        });
        let save_camera = host.as_ref().map(|h| h.save_camera).unwrap_or_else(|| {
            if owns_camera_preferences {
                pref.save_camera_in_replays()
            } else {
                effects.with_manager(|receiving_manager, effects| {
                    Self::checkbox_value(self.check_save_camera_id, receiving_manager)
                })
            }
        });
        let use_camera = host.as_ref().map(|h| h.use_camera).unwrap_or_else(|| {
            if owns_camera_preferences {
                pref.use_camera_in_replays()
            } else {
                effects.with_manager(|receiving_manager, effects| {
                    Self::checkbox_value(self.check_use_camera_id, receiving_manager)
                })
            }
        });
        let draw_anchor = host.as_ref().map(|h| h.draw_anchor).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_draw_anchor_id, receiving_manager)
            })
        });
        let move_anchor = host.as_ref().map(|h| h.move_anchor).unwrap_or_else(|| {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_move_anchor_id, receiving_manager)
            })
        });
        // Host Accept has no WND layout; C++ saveOptions reads checkboxes that
        // OptionsMenuInit populated from prefs/globals. Keep those current values.
        let host_apply = host.is_some();
        let current = runtime_global_data::read();
        let use_shadow_volumes = if host_apply {
            pref.get_bool_or("UseShadowVolumes", current.writable.use_shadow_volumes)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_3d_shadows_id, receiving_manager)
            })
        };
        let use_shadow_decals = if host_apply {
            pref.get_bool_or("UseShadowDecals", current.writable.use_shadow_decals)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_2d_shadows_id, receiving_manager)
            })
        };
        let use_cloud_map = if host_apply {
            pref.get_bool_or("UseCloudMap", current.use_cloud_map)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_cloud_shadows_id, receiving_manager)
            })
        };
        let use_light_map = if host_apply {
            pref.get_bool_or("UseLightMap", current.use_light_map)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_ground_lighting_id, receiving_manager)
            })
        };
        let show_soft_water_edge = if host_apply {
            pref.get_bool_or("ShowSoftWaterEdge", current.show_soft_water_edge)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_smooth_water_id, receiving_manager)
            })
        };
        let extra_animations = if host_apply {
            pref.get_bool("ExtraAnimations")
                .unwrap_or(!current.use_draw_module_lod)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_extra_animations_id, receiving_manager)
            })
        };
        let no_dynamic_lod = if host_apply {
            !pref
                .get_bool("DynamicLOD")
                .unwrap_or(current.writable.enable_dynamic_lod)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_no_dynamic_lod_id, receiving_manager)
            })
        };
        let unlock_fps = if host_apply {
            !pref
                .get_bool("FPSLimit")
                .unwrap_or(current.writable.use_fps_limit)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_unlock_fps_id, receiving_manager)
            })
        };
        let heat_effects = if host_apply {
            pref.get_bool_or("HeatEffects", current.use_heat_effects)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_heat_effects_id, receiving_manager)
            })
        };
        let building_occlusion = if host_apply {
            pref.get_bool_or("BuildingOcclusion", current.enable_behind_building_markers)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_building_occlusion_id, receiving_manager)
            })
        };
        let show_props = if host_apply {
            pref.get_bool_or("ShowTrees", current.use_trees)
        } else {
            effects.with_manager(|receiving_manager, effects| {
                Self::checkbox_value(self.check_props_id, receiving_manager)
            })
        };
        drop(current);

        // C++ OptionsMenu.cpp:1168-1241 `if (val != -1)` — missing slider keeps pref.
        let scroll_speed = match owned_scroll {
            Some(value) => value,
            None => effects
                .with_manager(|receiving_manager, effects| {
                    Self::slider_value_opt(self.slider_scroll_speed_id, receiving_manager)
                })
                .or_else(|| host.as_ref().map(|h| h.scroll_speed))
                .or_else(|| pref.get_int("ScrollFactor"))
                .or(default_scroll)
                .map(|value| value.clamp(0, 100)),
        };
        let music_volume = effects
            .with_manager(|receiving_manager, effects| {
                Self::slider_value_opt(self.slider_music_volume_id, receiving_manager)
            })
            .or_else(|| host.as_ref().map(|h| h.music_volume))
            .unwrap_or_else(|| pref.get_int_or("MusicVolume", default_music))
            .clamp(0, 100);
        let sfx_volume = effects
            .with_manager(|receiving_manager, effects| {
                Self::slider_value_opt(self.slider_sfx_volume_id, receiving_manager)
            })
            .or_else(|| host.as_ref().map(|h| h.sfx_volume))
            .unwrap_or_else(|| pref.get_int_or("SFXVolume", default_sfx))
            .clamp(0, 100);
        let voice_volume = effects
            .with_manager(|receiving_manager, effects| {
                Self::slider_value_opt(self.slider_voice_volume_id, receiving_manager)
            })
            .or_else(|| host.as_ref().map(|h| h.voice_volume))
            .unwrap_or_else(|| pref.get_int_or("VoiceVolume", default_voice))
            .clamp(0, 100);
        let (sfx_2d_volume, sfx_3d_volume, sfx_2d_factor, sfx_3d_factor) =
            Self::split_sfx_volume_for_relative(sfx_volume, Self::current_relative_2d_volume());
        let gamma_slider = effects
            .with_manager(|receiving_manager, effects| {
                Self::slider_value_opt(self.slider_gamma_id, receiving_manager)
            })
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
            effects
                .with_manager(|receiving_manager, effects| {
                    Self::slider_value_opt(self.slider_texture_resolution_id, receiving_manager)
                })
                .unwrap_or(2)
                .clamp(0, 2)
        };
        let particle_cap = if host_apply {
            pref.get_int("MaxParticleCount")
                .unwrap_or(runtime_global_data::read().max_particle_count)
                .max(100)
        } else {
            effects
                .with_manager(|receiving_manager, effects| {
                    Self::slider_value_opt(self.slider_particle_cap_id, receiving_manager)
                })
                .unwrap_or(5000)
                .max(100)
        };
        let texture_reduction = 2 - texture_resolution;
        let detail_name = Self::detail_name_from_index(detail_index);

        pref.set_string("Resolution", format!("{} {}", resolution.0, resolution.1));
        pref.set_int("AntiAliasing", anti_aliasing);
        pref.set_string("StaticGameLOD", detail_name.to_string());
        if let Some(scroll_speed) = scroll_speed {
            pref.set_int("ScrollFactor", scroll_speed);
        }
        pref.set_int("MusicVolume", music_volume);
        pref.set_int("SFXVolume", sfx_2d_volume);
        pref.set_int("SFX3DVolume", sfx_3d_volume);
        pref.set_int("VoiceVolume", voice_volume);
        pref.set_int("Gamma", gamma_slider);
        pref.set_string(
            "LanguageFilter",
            if language_filter { "true" } else { "false" }.to_string(),
        );
        Self::set_yes_no(pref, "SendDelay", send_delay);
        Self::set_yes_no(pref, "UseAlternateMouse", alternate_mouse);
        Self::set_yes_no(pref, "Retaliation", retaliation);
        Self::set_yes_no(pref, "UseDoubleClickAttackMove", double_click_attack_move);
        Self::set_yes_no(pref, "SaveCameraInReplays", save_camera);
        Self::set_yes_no(pref, "UseCameraInReplays", use_camera);
        Self::set_yes_no_title(pref, "DrawScrollAnchor", draw_anchor);
        Self::set_yes_no_title(pref, "MoveScrollAnchor", move_anchor);
        // C++ saveOptions writes advanced LOD only when Detail == CUSTOMDETAIL.
        // FPSLimit is never written (`//Never write this out`).

        const CUSTOMDETAIL: usize = 3;
        if detail_index == CUSTOMDETAIL {
            pref.set_int("TextureReduction", texture_reduction);
            pref.set_int("MaxParticleCount", particle_cap);
            Self::set_yes_no(pref, "UseShadowVolumes", use_shadow_volumes);
            Self::set_yes_no(pref, "UseShadowDecals", use_shadow_decals);
            Self::set_yes_no(pref, "UseCloudMap", use_cloud_map);
            Self::set_yes_no(pref, "UseLightMap", use_light_map);
            Self::set_yes_no(pref, "ShowSoftWaterEdge", show_soft_water_edge);
            Self::set_yes_no(pref, "ExtraAnimations", extra_animations);
            Self::set_yes_no(pref, "DynamicLOD", !no_dynamic_lod);
            Self::set_yes_no(pref, "HeatEffects", heat_effects);
            Self::set_yes_no(pref, "BuildingOcclusion", building_occlusion);
            Self::set_yes_no(pref, "ShowTrees", show_props);
        }
        if let Some(firewall_behavior) = self.firewall_behavior_override {
            pref.set_string("FirewallBehavior", firewall_behavior.to_string());
        }
        let _ = pref.write();
        self.firewall_behavior_override = None;

        effects.ui_mut().set_draw_rmb_scroll_anchor(draw_anchor);
        effects.ui_mut().set_move_rmb_scroll_anchor(move_anchor);
        publish_host_option(
            effects,
            HostOptionsRequest::DrawRmbScrollAnchor {
                enabled: draw_anchor,
            },
        );
        publish_host_option(
            effects,
            HostOptionsRequest::MoveRmbScrollAnchor {
                enabled: move_anchor,
            },
        );

        Self::commit_alternate_mouse_setting(
            alternate_mouse,
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| &mut host.options),
        );
        let display_gamma = Self::slider_to_gamma(gamma_slider);
        {
            let mut global = runtime_global_data::write();
            global.client_retaliation_mode_enabled = retaliation;
            global.double_click_attack_move = double_click_attack_move;
            global.language_filter_pref = language_filter;
            global.firewall_send_delay = send_delay;
            if !owns_camera_preferences {
                global.save_camera_in_replay = save_camera;
                global.use_camera_in_replay = use_camera;
            }
            global.display_gamma = display_gamma;
            global.anti_alias_box_value = anti_aliasing;
            if !owns_camera_preferences {
                if let Some(scroll_speed) = scroll_speed {
                    global.keyboard_scroll_factor = scroll_speed as f32 / 100.0;
                }
            }
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
        let _ = crate::core::script_action_handler::set_script_display_gamma(
            effects.script_display(),
            display_gamma,
            0.0,
            1.0,
        );

        if detail_index == CUSTOMDETAIL {
            if let Some(terrain) = terrain_lod {
                let _ = crate::core::game_client::apply_lod_texture_reduction_to(
                    texture_reduction,
                    terrain,
                );
            } else {
                let _ = apply_lod_texture_reduction(texture_reduction);
            }
        }

        if effects.menu_host.is_some() {
            if let Some(result) =
                game_engine::common::game_lod::set_static_lod_from_string_for_session(detail_name)
            {
                match result {
                    game_engine::common::game_lod::SessionStaticLodResult::Apply {
                        level,
                        rules,
                    } => publish_host_option(
                        effects,
                        HostOptionsRequest::TerrainTrackRules {
                            selected_level: level,
                            rules,
                        },
                    ),
                    game_engine::common::game_lod::SessionStaticLodResult::MarkerOnly { level } => {
                        publish_host_option(
                            effects,
                            HostOptionsRequest::StaticLodMarkerOnly { level },
                        )
                    }
                }
            }
        } else {
            game_engine::common::game_lod::set_static_lod_from_string(detail_name);
        }
        game_engine::common::game_lod::set_ideal_static_lod_from_string(detail_name);

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
            let (display, mouse) = effects.script_display_and_mouse();
            if !crate::core::script_action_handler::apply_script_display_mode(
                display,
                resolution.0.max(0) as u32,
                resolution.1.max(0) as u32,
                32,
                windowed,
                mouse,
            ) {
                return false;
            }
            effects.with_main_menu(|menu, _| {
                menu.set_pending_resolution_change(old_settings, new_settings)
            });
        }

        resolution_changed
    }

    /// C++ has one writable GlobalData object. Rust's live consumers span a
    /// parsed-INI residence and a runtime options residence, so commit this
    /// setting to both before a later LeftHUD callback snapshots it for Main.
    pub fn commit_alternate_mouse_setting(
        alternate_mouse: bool,
        host: Option<&mut crate::gui::options_host_bridge::HostOptionsBridge>,
    ) {
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().use_alternate_mouse = alternate_mouse;
        }
        runtime_global_data::write().use_alternate_mouse = alternate_mouse;
        if let Some(host) = host {
            host.publish(HostOptionsRequest::AlternateMouse {
                enabled: alternate_mouse,
            });
        }
        // Main is the physical-world input authority in an AuthorityOnly
        // match.  Keep the legacy residences above for standalone GameClient,
        // then deliver the same accepted setting through the bounded host
        // bridge when it is installed.
    }

    fn close_menu(&mut self, effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
        let options_overlay_open = options_overlay_is_open();
        if let Some(parent) = self.parent.as_ref() {
            effects.with_manager(|manager, effects| {
                let _ = manager.unset_modal(parent);
            });
        }
        effects.with_manager(|receiving_manager, effects| {
            Self::set_window_hidden(self.advanced_window_id, true, receiving_manager)
        });
        TheScriptEngine::signal_ui_interact("ShellOptionsClosed");
        if options_overlay_open {
            close_options_overlay();
        } else {
            crate::gui::shell::base::queue_shell_operation_with_effects(
                effects,
                |shell, effects| shell.destroy_options_layout(effects),
            );
        }
        self.parent = None;
        self.initialized = false;
    }

    fn accept_options(
        &mut self,
        owned: Option<(&mut UserPreferences, Option<i32>)>,
        terrain_lod: Option<&mut dyn FnMut(i32)>,
        effects: &mut WindowEventEffects<'_>,
    ) {
        let resolution_changed = match owned {
            Some((pref, scroll)) => {
                self.apply_options_with_preferences(None, pref, Some(scroll), terrain_lod, effects)
            }
            None => self.apply_options(effects),
        };
        let options_overlay_open = options_overlay_is_open();
        if !TheGameLogic::is_in_game() || TheGameLogic::get_game_mode() == GAME_SHELL {
            destroy_quit_menu(effects);
        }
        self.close_menu(effects);
        if resolution_changed && !options_overlay_open {
            effects.with_main_menu(|menu, effects| menu.do_resolution_dialog(effects));
        }
    }

    pub(crate) fn init_with_camera_input(
        &mut self,
        layout: &WindowLayout,
        observation: Option<&OptionsCameraInputObservation>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.init_ids();
        self.ignore_selected = true;
        effects.with_manager(|manager, effects| {
            self.parent = manager.get_window_by_id(self.parent_id);
            // Modal bookkeeping dispatches no messages — safe inline. Focus is
            // deferred (see SinglePlayerMenu::init): an inline set_focus would
            // re-enter the menu borrow held by this init dispatch.
            if let Some(parent) = self.parent.as_ref() {
                let _ = manager.set_modal(parent.clone());
            }
            if let Some(parent) = self.parent.clone() {
                effects.queue_window_operation(move |manager, effects| {
                    let _ = manager.set_focus(Some(&parent), effects);
                });
            }
        });
        effects.with_manager(|receiving_manager, effects| {
            self.populate_controls_with_camera_input(
                observation.map(|value| (layout, value)),
                receiving_manager,
                effects,
            )
        });
        let in_game_locked =
            TheGameLogic::is_in_game() && TheGameLogic::get_game_mode() != GAME_SHELL;
        if in_game_locked || options_overlay_is_open() {
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.combo_lan_ip_id, false, receiving_manager)
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.combo_online_ip_id, false, receiving_manager)
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.combo_detail_id, false, receiving_manager)
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.combo_resolution_id, false, receiving_manager)
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.check_send_delay_id, false, receiving_manager)
            });
            effects.with_manager(|receiving_manager, effects| {
                Self::set_window_enabled(self.button_firewall_refresh_id, false, receiving_manager)
            });
        }
        layout.hide_receiving(false, effects);
        self.ignore_selected = false;
        self.initialized = true;
        Ok(())
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
    fn system(
        &mut self,
        _window: &mut GameWindow,
        _msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        panic!("OptionsMenu system callback requires receiving WindowEventEffects")
    }

    fn init(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.init_with_camera_input(layout, None, effects)
    }

    fn update(
        &mut self,
        _layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Options Menu for layout: {}",
            layout.get_filename()
        );
        if let Some(parent) = self.parent.as_ref() {
            effects.with_manager(|manager, effects| {
                let _ = manager.unset_modal(parent);
            });
        }
        effects.with_manager(|receiving_manager, effects| {
            Self::set_window_hidden(self.advanced_window_id, true, receiving_manager)
        });
        layout.hide_receiving(true, effects);
        self.initialized = false;
        Ok(())
    }

    fn system_with_effects(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        match msg {
            WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
            WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
            WindowMessage::GadgetValueChanged => {
                if self.ignore_selected {
                    return WindowMsgHandled::Handled;
                }
                if data1 as i32 == self.combo_detail_id
                    && effects.with_manager(|receiving_manager, effects| {
                        Self::combo_selected_index(self.combo_detail_id, receiving_manager)
                    }) == Some(3)
                {
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_window_hidden(self.advanced_window_id, false, receiving_manager)
                    });
                }
                WindowMsgHandled::Handled
            }
            WindowMessage::GadgetSelected => {
                if self.ignore_selected {
                    return WindowMsgHandled::Handled;
                }
                let control_id = data1 as i32;
                if control_id == self.button_back_id {
                    self.close_menu(effects);
                } else if control_id == self.button_accept_id {
                    self.accept_options(None, None, effects);
                } else if control_id == self.button_defaults_id {
                    effects.with_manager(|receiving_manager, effects| {
                        self.apply_default_controls(receiving_manager, effects)
                    });
                } else if control_id == self.button_firewall_refresh_id {
                    self.firewall_behavior_override = Some(0);
                    {
                        let mut global = runtime_global_data::write();
                        global.firewall_behavior = 0;
                    }
                } else if control_id == self.button_advanced_accept_id {
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_window_hidden(self.advanced_window_id, true, receiving_manager)
                    });
                } else if control_id == self.button_advanced_back_id {
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_combo_selected(
                            self.combo_detail_id,
                            self.initial_detail_index,
                            receiving_manager,
                            effects,
                        )
                    });
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_window_hidden(self.advanced_window_id, true, receiving_manager)
                    });
                } else if control_id == self.button_keyboard_options_id {
                    queue_shell_push(effects, "Menus/KeyboardOptionsMenu.wnd", false);
                } else if control_id == self.combo_detail_id
                    && effects.with_manager(|receiving_manager, effects| {
                        Self::combo_selected_index(self.combo_detail_id, receiving_manager)
                    }) == Some(3)
                {
                    effects.with_manager(|receiving_manager, effects| {
                        Self::set_window_hidden(self.advanced_window_id, false, receiving_manager)
                    });
                } else {
                    self.apply_immediate_checkbox_effect(
                        control_id,
                        effects.with_manager(|receiving_manager, effects| {
                            Self::checkbox_value(control_id, receiving_manager)
                        }),
                        effects,
                    );
                }
                WindowMsgHandled::Handled
            }
            _ => WindowMsgHandled::Ignored,
        }
    }

    fn input(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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
                    effects,
                );
            }
        }

        WindowMsgHandled::Handled
    }
}

/// Map Select Menu implementation
pub struct MapSelectMenu {
    residual_action: ResidualMapSelectMenuAction,

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
            residual_action: ResidualMapSelectMenuAction::None,

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

    fn start_game(&mut self, effects: &mut crate::gui::game_window::WindowEventEffects<'_>) {
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
        queue_shell_reverse_animate_window(effects);
    }

    fn do_game_start(&mut self, effects: &mut WindowEventEffects<'_>) {
        if TheGameLogic::is_in_game() {
            let _ = TheGameLogic::clear_game_data();
        }

        self.start_game = false;
        let mut msg = GameMessage::new(GameMessageType::NewGame);
        msg.append_integer_argument(GAME_SINGLE_PLAYER);
        msg.append_integer_argument(self.difficulty);
        msg.append_integer_argument(0);
        effects.emitted_messages.push(msg);
        init_random_with_seed(0);
        self.is_shutting_down = true;
    }

    fn shutdown_complete(
        &mut self,
        layout: &WindowLayout,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) {
        self.is_shutting_down = false;
        layout.hide_receiving(true, effects);
        self.initialized = false;
        self.parent = None;
        self.listbox_map = None;
        queue_shell_shutdown_complete(effects, false);
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("receiving menu lifecycle effects are required".into())
    }

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
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
        show_shell_map_if_available(true, effects);
        layout.hide_receiving(false, effects);

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

        effects.with_manager(|manager, effects| {
            self.parent = manager.get_window_by_id(self.parent_id);
            self.listbox_map = manager.get_window_by_id(self.listbox_map_id);
            // Focus is deferred (see SinglePlayerMenu::init): an inline
            // set_focus would re-enter the menu borrow held by this init
            // dispatch via the window's system callback.
            if let Some(parent) = self.parent.clone() {
                effects.queue_window_operation(move |manager, effects| {
                    let _ = manager.set_focus(Some(&parent), effects);
                });
            }
            if let Some(button_back) = manager.get_window_by_id(self.button_back_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_back,
                        AnimationType::SlideRight,
                        true,
                        0,
                    )
                });
            }
            if let Some(button_ok) = manager.get_window_by_id(self.button_ok_id) {
                effects.with_receiving_manager(manager, |effects| {
                    queue_shell_window_animation(
                        effects,
                        button_ok,
                        AnimationType::SlideLeft,
                        true,
                        0,
                    )
                });
            }
        });

        let mut pref = UserPreferences::new();
        let _ = pref.load("Options.ini");
        self.use_system_maps = pref.get_bool_or("UseSystemMapDir", true);

        let difficulty = gamelogic::helpers::TheScriptEngine::get_global_difficulty();
        self.difficulty = difficulty;

        Self::set_radio_selected(
            &effects.with_manager(|manager, effects| manager.get_window_by_id(self.radio_easy_id)),
            difficulty == 0,
        );
        Self::set_radio_selected(
            &effects
                .with_manager(|manager, effects| manager.get_window_by_id(self.radio_medium_id)),
            difficulty == 1,
        );
        Self::set_radio_selected(
            &effects.with_manager(|manager, effects| manager.get_window_by_id(self.radio_hard_id)),
            difficulty == 2,
        );
        Self::set_radio_selected(
            &effects.with_manager(|manager, effects| {
                manager.get_window_by_id(self.radio_system_maps_id)
            }),
            self.use_system_maps,
        );
        Self::set_radio_selected(
            &effects
                .with_manager(|manager, effects| manager.get_window_by_id(self.radio_user_maps_id)),
            !self.use_system_maps,
        );

        self.refresh_map_list();
        Ok(())
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.is_shutting_down && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
        {
            self.shutdown_complete(layout, effects);
        }
        Ok(())
    }

    fn update_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if self.start_game && crate::gui::shell::base::shell_anim_finished_for_layout(effects) {
            self.do_game_start(effects);
        }
        if self.is_shutting_down && crate::gui::shell::base::shell_anim_finished_for_layout(effects)
        {
            self.shutdown_complete(layout, effects);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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
            self.shutdown_complete(layout, effects);
            return Ok(());
        }
        if !self.start_game {
            self.is_shutting_down = true;
            queue_shell_reverse_animate_window(effects);
        }
        Ok(())
    }

    fn system(
        &mut self,
        _window: &mut GameWindow,
        _msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        panic!("MapSelectMenu system dispatch requires receiving event effects");
    }

    fn system_with_effects(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
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
                        if let Some(manager) = effects.campaign_manager.as_deref_mut() {
                            manager.set_campaign("");
                        } else {
                            return WindowMsgHandled::Handled;
                        }
                        self.start_game(effects);
                        return WindowMsgHandled::Handled;
                    }
                    return WindowMsgHandled::Handled;
                }
                if control_id == self.button_back_id {
                    self.button_pushed = true;
                    queue_shell_pop(effects);
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
                            effects.queue_system_message(
                                Rc::downgrade(parent),
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
        _window: &mut GameWindow,
        _msg: WindowMessage,
        _data1: WindowMsgData,
        _data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        panic!("menu input dispatch requires receiving event effects");
    }

    fn input_with_effects(
        &mut self,
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut WindowEventEffects<'_>,
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
        if let Some(parent) = self.parent.clone() {
            effects.queue_system_message(
                Rc::downgrade(&parent),
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
    residual_action: ResidualCreditsMenuAction,
    residual_active: bool,
    initialized: bool,
    parent_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    credits: Option<crate::credits::CreditsManager>,
}

impl CreditsMenu {
    pub fn new() -> Self {
        Self {
            residual_action: ResidualCreditsMenuAction::None,
            residual_active: false,
            initialized: false,
            parent_id: 0,
            parent: None,
            credits: None,
        }
    }

    pub fn draw(
        &mut self,
        draw_context: &mut crate::gui::window_draw_context::WindowDrawContext<'_>,
    ) {
        if let Some(credits) = self.credits.as_mut() {
            credits.draw(draw_context);
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("receiving menu lifecycle effects are required".into())
    }

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Initializing Credits Menu for layout: {}",
            layout.get_filename()
        );

        show_shell_map_if_available(false, effects);

        let mut credits = crate::credits::CreditsManager::new();
        if let Err(err) = credits.load_from_path("Data/INI/Credits.ini") {
            warn!("Failed to load credits data: {}", err);
        }
        credits.init();
        self.credits = Some(credits);

        self.parent_id =
            NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
        layout.hide_receiving(false, effects);
        effects.with_manager(|manager, effects| {
            self.parent = manager.get_window_by_id(self.parent_id);
            // Focus is deferred (see SinglePlayerMenu::init): an inline
            // set_focus sends InputFocus to this same window, whose
            // "CreditsMenuSystem" callback would re-enter this init dispatch's
            // menu borrow (RefCell forbids that; the std RwLock it replaced
            // was not re-entrant and this was the sampled CreditsMenu wedge).
            if let Some(parent) = self.parent.clone() {
                effects.queue_window_operation(move |manager, effects| {
                    let _ = manager.set_focus(Some(&parent), effects);
                });
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // Deferred for the same reason as in init: this update runs inside the
        // menu borrow taken by the CreditsMenuUpdate dispatch, and an inline
        // set_focus re-enters it via the InputFocus system callback.
        if let Some(parent) = self.parent.clone() {
            effects.queue_window_operation(move |manager, effects| {
                let _ = manager.set_focus(Some(&parent), effects);
            });
        }

        if let Some(credits) = self.credits.as_mut() {
            credits.update(
                _layout
                    .windows()
                    .first()
                    .map(|window| window.borrow().get_size())
                    .unwrap_or((0, 0)),
            );
            if credits.is_finished() {
                queue_shell_pop(effects);
            }
        } else {
            queue_shell_pop(effects);
        }
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("receiving menu lifecycle effects are required".into())
    }

    fn shutdown_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!(
            "Shutting down Credits Menu for layout: {}",
            layout.get_filename()
        );

        if let Some(credits) = self.credits.as_mut() {
            credits.reset();
        }

        show_shell_map_if_available(true, effects);
        layout.hide_receiving(true, effects);
        queue_shell_shutdown_complete(effects, false);

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
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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
        _window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        if msg == WindowMessage::Char && data1 == 0x1B {
            if (data2 & 0x0001) != 0 {
                queue_shell_pop(effects);
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
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("receiving LAN menu effects are required".into())
    }

    fn init_with_effects(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_init(layout, effects);
        self.initialized = true;
        Ok(())
    }

    fn update(
        &mut self,
        layout: &WindowLayout,
        _user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_update(layout);
        Ok(())
    }

    fn shutdown(
        &mut self,
        layout: &WindowLayout,
        user_data: Option<&mut dyn std::any::Any>,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_system(window, msg, data1, data2)
    }

    fn input(
        &mut self,
        window: &mut GameWindow,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
        effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
    ) -> WindowMsgHandled {
        crate::gui::callbacks::lan_lobby_menu::lan_lobby_input(window, msg, data1, data2, effects)
    }
}

/// Menu state belongs to one receiving GameClient; construction is inert.
pub struct MenuManager {
    challenge: crate::gui::callbacks::challenge_menu::ChallengeMenuState,
    challenge_generals: crate::gui::challenge_generals::ChallengeGenerals,
    control_bar_callbacks: crate::gui::callbacks::control_bar_callbacks::ControlBarCallbacks,
    message_boxes: crate::gui::callbacks::message_box::MessageBoxSystem,
    diplomacy: crate::gui::callbacks::diplomacy::DiplomacyCallbacks,
    ingame_ui: crate::gui::callbacks::ingame_callbacks::InGameUISystem,
    difficulty_select: crate::gui::callbacks::difficulty_select::DifficultySelectMenuState,
    save_load: crate::gui::callbacks::popup_save_load::SaveLoadMenuState,
    save_load_host: crate::gui::callbacks::popup_save_load::HostPopupSaveLoadBridge,
    popup_replay: crate::gui::callbacks::popup_replay::PopupReplayState,
    flags: crate::gui::menu_flags::MenuFlags,
    score: crate::gui::callbacks::score_screen::ScoreScreenState,
    quit: crate::gui::callbacks::quit_menu::QuitMenuState,
    establish_connections: crate::gui::menus::establish_connections_menu::EstablishConnectionsMenu,
    disconnect: crate::gui::menus::disconnect_menu::DisconnectMenu,
    main_menu: MainMenu,
    single_player_menu: SinglePlayerMenu,
    options_menu: OptionsMenu,
    map_select_menu: MapSelectMenu,
    credits_menu: CreditsMenu,
    lan_lobby_menu: LanLobbyMenu,
    popup: crate::gui::callbacks::in_game_popup_message::PopupMessageState,
}

impl MenuManager {
    pub fn ingame_ui(&self) -> &crate::gui::callbacks::ingame_callbacks::InGameUISystem {
        &self.ingame_ui
    }
    pub fn ingame_ui_mut(
        &mut self,
    ) -> &mut crate::gui::callbacks::ingame_callbacks::InGameUISystem {
        &mut self.ingame_ui
    }
    pub(crate) fn save_load_host_installed(&self) -> bool {
        self.save_load_host.is_installed()
    }
    pub(crate) fn publish_save_load_entries(
        &mut self,
        entries: Vec<crate::gui::callbacks::popup_save_load::PopupSaveLoadEntry>,
    ) {
        self.save_load_host.publish_entries(entries);
    }
    pub(crate) fn take_save_load_requests(
        &mut self,
    ) -> Vec<crate::gui::callbacks::popup_save_load::HostPopupSaveLoadPublishedRequest> {
        self.save_load_host.take_requests()
    }

    pub fn new() -> Self {
        Self {
            challenge: Default::default(),
            challenge_generals: crate::gui::challenge_generals::ChallengeGenerals::new(),
            control_bar_callbacks: Default::default(),
            establish_connections:
                crate::gui::menus::establish_connections_menu::EstablishConnectionsMenu::new(),
            disconnect: crate::gui::menus::disconnect_menu::DisconnectMenu::new(),
            main_menu: MainMenu::new(),
            single_player_menu: SinglePlayerMenu::new(),
            options_menu: OptionsMenu::new(),
            map_select_menu: MapSelectMenu::new(),
            credits_menu: CreditsMenu::new(),
            lan_lobby_menu: LanLobbyMenu::new(),
            popup: Default::default(),
            score: Default::default(),
            popup_replay: Default::default(),
            save_load: Default::default(),
            difficulty_select: Default::default(),
            save_load_host: Default::default(),
            flags: Default::default(),
            quit: Default::default(),
            ingame_ui: Default::default(),
            diplomacy: Default::default(),
            message_boxes: Default::default(),
        }
    }

    pub(crate) fn quit_menu_visible(&self) -> bool {
        self.quit.is_visible()
    }

    pub fn main_menu_mut(&mut self) -> &mut MainMenu {
        &mut self.main_menu
    }

    pub fn single_player_menu_mut(&mut self) -> &mut SinglePlayerMenu {
        &mut self.single_player_menu
    }

    pub fn options_menu_mut(&mut self) -> &mut OptionsMenu {
        &mut self.options_menu
    }

    pub fn map_select_menu_mut(&mut self) -> &mut MapSelectMenu {
        &mut self.map_select_menu
    }

    pub fn credits_menu_mut(&mut self) -> &mut CreditsMenu {
        &mut self.credits_menu
    }

    pub fn lan_lobby_menu_mut(&mut self) -> &mut LanLobbyMenu {
        &mut self.lan_lobby_menu
    }
}

impl Default for MenuManager {
    fn default() -> Self {
        Self::new()
    }
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

pub fn residual_options_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualOptionsMenuAction {
    effects.with_options_menu(|menu, _effects| menu.residual_action)
}

pub fn residual_options_menu_is_bound(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_options_menu(|menu, _effects| menu.initialized)
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

fn parse_resolution_pref(raw: Option<&str>) -> Option<(i32, i32)> {
    let raw = raw?;
    let mut parts = raw.split(|c: char| c.is_ascii_whitespace() || c == 'x' || c == 'X');
    let w = parts.next()?.parse::<i32>().ok()?;
    let h = parts.next()?.parse::<i32>().ok()?;
    if w > 0 && h > 0 { Some((w, h)) } else { None }
}

/// C++ Accept: `TheDisplay->setGamma` + flat Options.ini + live volumes.
pub fn apply_options_menu_like_cpp(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Accept;
        let _ = menu.apply_options(effects);
        true
    })
}

/// In-game stub / host Accept: same `apply_options` path with explicit values.
pub fn apply_options_from_host(
    apply: HostOptionsApply,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, _effects| {
        menu.pending_host_apply = Some(apply);
    });
    apply_options_menu_like_cpp(effects)
}

/// Actual host Options Apply; the same kernel mutates this application's store.
pub fn apply_options_from_host_with_preferences(
    apply: HostOptionsApply,
    preferences: &mut UserPreferences,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    apply_options_from_host_with_preferences_and_terrain(apply, preferences, None, effects)
}

/// Receiving fallback UI operation; applies synchronously through the same kernel.
pub fn apply_options_from_host_with_preferences_and_terrain(
    apply: HostOptionsApply,
    preferences: &mut UserPreferences,
    terrain_lod: Option<&mut dyn FnMut(i32)>,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    let scroll = apply.scroll_speed;
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Accept;
        let _ = menu.apply_options_with_preferences(
            Some(apply),
            preferences,
            Some(Some(scroll)),
            terrain_lod,
            effects,
        );
        true
    })
}
/// Complete the single native Accept after sender and receiving WM loans end.
pub(crate) fn complete_options_accept_with_preferences(
    preferences: &mut UserPreferences,
    scroll: Option<i32>,
    terrain_lod: Option<&mut dyn FnMut(i32)>,
    effects: &mut WindowEventEffects<'_>,
) {
    effects.with_options_menu(|menu, effects| {
        if !menu.ignore_selected {
            menu.accept_options(Some((preferences, scroll)), terrain_lod, effects);
        }
    });
}

pub(crate) fn complete_options_defaults_with_preferences(
    slider: Option<Rc<RefCell<GameWindow>>>,
    default: f32,
    effects: &mut WindowEventEffects<'_>,
) {
    effects.with_options_menu(|menu, effects| {
        if !menu.ignore_selected {
            effects.with_manager(|manager, _effects| {
                menu.apply_default_controls_with_scroll(Some((slider, default)), manager, _effects)
            });
        }
    });
}

/// Residual: bind OptionsMenu control IDs (no layout load / populate required).
pub fn simulate_options_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_options_menu(|menu, effects| {
        menu.init_ids();
        menu.ignore_selected = false;
        menu.initialized = true;
        true
    })
}

/// Residual: fire ButtonAccept without apply_options / close_menu side effects.
pub fn simulate_options_menu_accept_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Accept;
        true
    })
}

/// Residual: fire ButtonBack without destroy_options_layout.
pub fn simulate_options_menu_back_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Back;
        true
    })
}

/// Residual: fire ButtonDefaults without apply_default_controls widget writes.
pub fn simulate_options_menu_defaults_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Defaults;
        true
    })
}

/// Residual: fire ButtonKeyboardOptions without shell push.
pub fn simulate_options_menu_keyboard_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::Keyboard;
        true
    })
}

/// Residual: fire ButtonAdvanceAccept without advanced panel hide.
pub fn simulate_options_menu_advanced_accept_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::AdvancedAccept;
        true
    })
}

/// Residual: fire ButtonAdvanceBack without detail combo restore.
pub fn simulate_options_menu_advanced_back_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        menu.residual_action = ResidualOptionsMenuAction::AdvancedBack;
        true
    })
}

/// Residual: fire ButtonFirewallRefresh without global write.
pub fn simulate_options_menu_firewall_refresh_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_options_menu(|menu, effects| {
        if !menu.initialized {
            menu.init_ids();
            menu.ignore_selected = false;
            menu.initialized = true;
        }
        // Mirror C++ residual intent: mark override without applying prefs.
        menu.firewall_behavior_override = Some(0);
        menu.residual_action = ResidualOptionsMenuAction::FirewallRefresh;
        true
    })
}

/// Residual: bind + Accept composite (save settings honesty).
pub fn simulate_options_menu_prepare_accept(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_options_menu_bind_controls(effects) {
        return false;
    }
    simulate_options_menu_accept_button_gadget_selected(effects)
}

/// Human click-through: OS LeftDown/Up on a retail `OptionsMenu.wnd:*` gadget
/// (C++ WindowXlat hit → GBM_SELECTED). Not `simulate_*` first.
fn drive_os_wnd_options_named(
    name: &str,
    latch: impl FnOnce(&mut WindowEventEffects<'_>) -> bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !crate::gui::dispatch_os_click_named_window(name, effects) {
        return false;
    }
    latch(effects)
}

pub fn drive_os_wnd_options_menu_accept_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonAccept",
        apply_options_menu_like_cpp,
        effects,
    )
}

pub fn drive_os_wnd_options_menu_back_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonBack",
        |effects| simulate_options_menu_back_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_defaults_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonDefaults",
        |effects| simulate_options_menu_defaults_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_keyboard_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonKeyboardOptions",
        |effects| simulate_options_menu_keyboard_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_advanced_accept_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonAdvanceAccept",
        |effects| simulate_options_menu_advanced_accept_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_advanced_back_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonAdvanceBack",
        |effects| simulate_options_menu_advanced_back_button_gadget_selected(effects),
        effects,
    )
}

pub fn drive_os_wnd_options_menu_firewall_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    drive_os_wnd_options_named(
        "OptionsMenu.wnd:ButtonFirewallRefresh",
        |effects| simulate_options_menu_firewall_refresh_button_gadget_selected(effects),
        effects,
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
        match crate::test_process::run_bounded(
            module_path!(),
            "os_wnd_options_menu_accept_hits_button_then_latches",
        ) {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
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
        match crate::test_process::run_bounded(
            module_path!(),
            "apply_options_from_host_sets_gamma_and_music_volume_like_cpp",
        ) {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
        // C++ OptionsMenu.cpp:1180-1262 MusicVolume 0-100 + TheDisplay->setGamma.
        let mut apply = HostOptionsApply::default();
        apply.music_volume = 80;
        apply.gamma_slider = 100;
        apply.resolution = (1024, 768);
        assert!(apply_options_from_host(apply, effects));
        let gamma = crate::display::display_fx::gamma_state();
        assert!((gamma.gamma - 2.0).abs() < 0.01);
        assert!((runtime_global_data::read().music_volume_factor - 0.8).abs() < 0.01);
    }

    #[test]
    fn apply_options_from_host_custom_preserves_advanced_detail() {
        match crate::test_process::run_bounded(
            module_path!(),
            "apply_options_from_host_custom_preserves_advanced_detail",
        ) {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
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
        assert!(apply_options_from_host(apply, effects));
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
        match crate::test_process::run_bounded(
            module_path!(),
            "apply_options_from_host_publishes_scroll_anchors",
        ) {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
        use crate::gui::options_host_bridge::HostOptionsRequest;

        let mut messages = Vec::new();
        let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menu_host = Some(&mut mailbox);

        let _ = effects
            .menu_host
            .as_deref_mut()
            .map(|host| host.options.take_requests())
            .unwrap_or_default();
        let original_draw = TheInGameUI::get_draw_rmb_scroll_anchor();
        let original_move = TheInGameUI::get_move_rmb_scroll_anchor();

        let mut apply = HostOptionsApply::default();
        apply.draw_anchor = true;
        apply.move_anchor = true;
        apply.resolution = (1024, 768);
        assert!(apply_options_from_host(apply, &mut effects));
        let reqs = effects
            .menu_host
            .as_deref_mut()
            .map(|host| host.options.take_requests())
            .unwrap_or_default();
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
        match crate::test_process::run_bounded(
            module_path!(),
            "os_wnd_options_menu_keyboard_and_defaults_hit_named_gadgets",
        ) {
            crate::test_process::TestProcess::ParentVerified => return,
            crate::test_process::TestProcess::Child => {}
        }
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

pub fn residual_credits_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualCreditsMenuAction {
    effects.with_credits_menu(|menu, _effects| menu.residual_action)
}

pub fn residual_credits_menu_is_active(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, _effects| menu.residual_active)
}

/// Residual: bind CreditsMenu parent control ID (no INI/audio/layout load).
pub fn simulate_credits_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, effects| {
        menu.parent_id =
            NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
        menu.initialized = true;
        menu.residual_active = true;
        menu.residual_action = ResidualCreditsMenuAction::Bind;
        true
    })
}

/// Residual: ESC skip path without shell pop.
pub fn simulate_credits_menu_skip(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
            menu.initialized = true;
        }
        menu.residual_active = false;
        menu.residual_action = ResidualCreditsMenuAction::Skip;
        true
    })
}

/// Residual: credits finished path without shell pop.
pub fn simulate_credits_menu_finished(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("CreditsMenu.wnd:ParentCreditsWindow") as i32;
            menu.initialized = true;
        }
        menu.residual_active = false;
        menu.residual_action = ResidualCreditsMenuAction::Finished;
        true
    })
}

/// Residual: shutdown latch without audio/layout teardown.
pub fn simulate_credits_menu_shutdown(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_credits_menu(|menu, effects| {
        menu.initialized = false;
        menu.parent = None;
        menu.parent_id = 0;
        menu.credits = None;
        menu.residual_active = false;
        menu.residual_action = ResidualCreditsMenuAction::Shutdown;
        true
    })
}

/// Residual: bind + skip composite (MainMenu ButtonCredits exit honesty).
pub fn simulate_credits_menu_prepare_skip(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_credits_menu_bind_controls(effects) {
        return false;
    }
    simulate_credits_menu_skip(effects)
}

/// Human click-through: OS LeftDown/Up on `CreditsMenu.wnd:ParentCreditsWindow`
/// (C++ named layout hit; ESC skip latches via residual). Not `simulate_*` first.
pub fn drive_os_wnd_credits_menu_skip_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("CreditsMenu.wnd:ParentCreditsWindow", effects);
    if !clicked {
        return false;
    }
    simulate_credits_menu_skip(effects)
}

pub fn drive_os_wnd_credits_menu_finished_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked =
        crate::gui::dispatch_os_click_named_window("CreditsMenu.wnd:ParentCreditsWindow", effects);
    if !clicked {
        return false;
    }
    simulate_credits_menu_finished(effects)
}

pub fn drive_os_wnd_credits_menu_prepare_skip_like_cpp(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    let clicked = drive_os_wnd_credits_menu_skip_like_cpp(effects);
    if !clicked {
        return false;
    }
    residual_credits_menu_last_action(effects) == ResidualCreditsMenuAction::Skip
}

#[cfg(test)]
mod credits_os_wnd_tests {
    use super::*;
    use crate::gui::with_window_manager;

    #[test]
    fn os_wnd_credits_menu_skip_hits_parent_then_latches() {
        let owner = crate::gui::window_manager::WindowManagerOwner::new();
        owner.with_scope(|| {
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
        });
    }

    #[test]
    fn os_wnd_credits_menu_finished_hits_parent_then_latches() {
        let owner = crate::gui::window_manager::WindowManagerOwner::new();
        owner.with_scope(|| {
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
        });
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

pub fn residual_single_player_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualSinglePlayerMenuAction {
    effects.with_single_player_menu(|menu, _effects| menu.residual_action)
}

pub fn residual_single_player_menu_is_bound(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_single_player_menu(|menu, _effects| menu.initialized)
}

pub fn residual_single_player_menu_button_pushed(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_single_player_menu(|menu, _effects| menu.button_pushed)
}

/// Residual: bind SinglePlayerMenu control IDs (no layout load).
pub fn simulate_single_player_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_single_player_menu(|menu, effects| {
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
        true
    })
}

/// Residual: fire ButtonNew without shell push MapSelectMenu.
pub fn simulate_single_player_menu_new_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
        }
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        menu.residual_action = ResidualSinglePlayerMenuAction::New;
        true
    })
}

/// Residual: fire ButtonLoad without save/load menu open.
pub fn simulate_single_player_menu_load_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
        }
        if menu.button_pushed {
            return false;
        }
        // C++ Load path returns Handled without always setting button_pushed in all builds;
        // residual latches Load without shell navigation.
        menu.residual_action = ResidualSinglePlayerMenuAction::Load;
        true
    })
}

/// Residual: fire ButtonBack without shell pop.
pub fn simulate_single_player_menu_back_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        if !menu.initialized {
            menu.parent_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:SinglePlayerMenuParent") as i32;
            menu.button_new_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonNew") as i32;
            menu.button_load_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonLoad") as i32;
            menu.button_back_id =
                NameKeyGenerator::name_to_key("SinglePlayerMenu.wnd:ButtonBack") as i32;
            menu.initialized = true;
        }
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        menu.residual_action = ResidualSinglePlayerMenuAction::Back;
        true
    })
}

/// Residual: clear button_pushed latch (re-enter honesty).
pub fn simulate_single_player_menu_clear_button_pushed(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_single_player_menu(|menu, effects| {
        menu.button_pushed = false;
        true
    })
}

/// Residual: bind + New composite (campaign map select entry honesty).
pub fn simulate_single_player_menu_prepare_new(effects: &mut WindowEventEffects<'_>) -> bool {
    if !simulate_single_player_menu_bind_controls(effects) {
        return false;
    }
    simulate_single_player_menu_new_button_gadget_selected(effects)
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

pub fn residual_map_select_menu_last_action(
    effects: &mut WindowEventEffects<'_>,
) -> ResidualMapSelectMenuAction {
    effects.with_map_select_menu(|menu, _effects| menu.residual_action)
}

pub fn residual_map_select_menu_difficulty(effects: &mut WindowEventEffects<'_>) -> i32 {
    effects.with_map_select_menu(|menu, _effects| menu.difficulty)
}

pub fn residual_map_select_menu_selected_map(
    effects: &mut WindowEventEffects<'_>,
) -> Option<String> {
    effects.with_map_select_menu(|menu, _effects| menu.selected_map.clone())
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
pub fn simulate_map_select_menu_bind_controls(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.initialized = true;
        menu.button_pushed = false;
        menu.is_shutting_down = false;
        menu.start_game = false;
        true
    })
}

/// Residual: select a map path without live listbox.
pub fn simulate_map_select_menu_select_map(
    map_path: &str,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    if map_path.is_empty() {
        return false;
    }
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.selected_map = Some(map_path.to_string());

        menu.residual_action = ResidualMapSelectMenuAction::SelectMap;
        menu.selected_map.clone().as_deref() == Some(map_path)
    })
}

/// Residual: set AI difficulty radio (0/1/2) without widgets.
pub fn simulate_map_select_menu_set_difficulty(
    difficulty: i32,
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    if !(0..=2).contains(&difficulty) {
        return false;
    }
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.difficulty = difficulty;

        menu.residual_action = match difficulty {
            0 => ResidualMapSelectMenuAction::DifficultyEasy,
            2 => ResidualMapSelectMenuAction::DifficultyHard,
            _ => ResidualMapSelectMenuAction::DifficultyMedium,
        };
        menu.difficulty == difficulty
    })
}

/// Residual: fire ButtonOK without start_game engine transfer.
pub fn simulate_map_select_menu_ok_button_gadget_selected(
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        if menu.button_pushed {
            return false;
        }
        if menu.selected_map.is_none() {
            // C++ ignores OK with no selection.
            return false;
        }
        menu.button_pushed = true;
        // Do not call start_game(); residual latch only.
        menu.residual_action = ResidualMapSelectMenuAction::Ok;
        true
    })
}

/// Residual: fire ButtonBack without shell pop.
pub fn simulate_map_select_menu_back_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        if menu.button_pushed {
            return false;
        }
        menu.button_pushed = true;
        menu.residual_action = ResidualMapSelectMenuAction::Back;
        true
    })
}

/// Residual: ButtonSinglePlayer map filter residual.
pub fn simulate_map_select_menu_solo_maps_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.show_solo_maps = true;
        menu.residual_action = ResidualMapSelectMenuAction::SoloMaps;
        true
    })
}

/// Residual: ButtonMultiplayer map filter residual.
pub fn simulate_map_select_menu_multiplayer_maps_button_gadget_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.show_solo_maps = false;
        menu.residual_action = ResidualMapSelectMenuAction::MultiplayerMaps;
        true
    })
}

/// Residual: RadioButtonSystemMaps residual.
pub fn simulate_map_select_menu_system_maps_radio_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.use_system_maps = true;
        menu.residual_action = ResidualMapSelectMenuAction::SystemMaps;
        true
    })
}

/// Residual: RadioButtonUserMaps residual.
pub fn simulate_map_select_menu_user_maps_radio_selected(
    effects: &mut WindowEventEffects<'_>,
) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        ensure_map_select_control_ids(menu);
        menu.use_system_maps = false;
        menu.residual_action = ResidualMapSelectMenuAction::UserMaps;
        true
    })
}

/// Residual: clear button_pushed latch.
pub fn simulate_map_select_menu_clear_button_pushed(effects: &mut WindowEventEffects<'_>) -> bool {
    effects.with_map_select_menu(|menu, effects| {
        menu.button_pushed = false;
        menu.start_game = false;
        true
    })
}

/// Residual: select map + medium AI + OK composite (campaign start honesty).
pub fn simulate_map_select_menu_prepare_ok(
    map_path: &str,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> bool {
    if !simulate_map_select_menu_bind_controls(effects) {
        return false;
    }
    let _ = simulate_map_select_menu_clear_button_pushed(effects);
    if !simulate_map_select_menu_select_map(map_path, effects) {
        return false;
    }
    if !simulate_map_select_menu_set_difficulty(1, effects) {
        return false;
    }
    simulate_map_select_menu_ok_button_gadget_selected(effects)
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
        let mut manager = MenuManager::new();

        // Test that all menus are accessible as owned fields
        let _ = manager.main_menu_mut();
        let _ = manager.single_player_menu_mut();
        let _ = manager.options_menu_mut();
        let _ = manager.map_select_menu_mut();
        let _ = manager.credits_menu_mut();
        let _ = manager.lan_lobby_menu_mut();
    }

    #[test]
    fn menu_recipients_borrow_the_same_owned_fields_and_leave_peers_independent() {
        let mut owner = MenuManager::new();
        let mut peer = MenuManager::new();
        let expected = std::ptr::from_mut(owner.options_menu_mut());
        {
            let mut recipients =
                owner.receiving_menus(crate::core::game_client::ClientInputAuthority::NativeHosted);
            assert_eq!(
                std::ptr::from_mut(recipients.options.as_deref_mut().unwrap()),
                expected
            );
            let mut child = recipients.reborrow();
            child.options.as_deref_mut().unwrap().initialized = true;
            assert!(!peer.options_menu_mut().initialized);
        }
        assert!(owner.options_menu_mut().initialized);
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
        let mut messages = Vec::new();
        let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menu_host = Some(&mut mailbox);
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

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_draw_anchor_id,
            true,
            &mut effects
        ));
        assert!(TheInGameUI::get_draw_rmb_scroll_anchor());

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_move_anchor_id,
            true,
            &mut effects
        ));
        assert!(TheInGameUI::get_move_rmb_scroll_anchor());

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_save_camera_id,
            false,
            &mut effects
        ));
        assert!(!runtime_global_data::read().save_camera_in_replay);

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_use_camera_id,
            false,
            &mut effects
        ));
        assert!(!runtime_global_data::read().use_camera_in_replay);

        assert!(!menu.apply_immediate_checkbox_effect(99, true, &mut effects));

        TheInGameUI::set_draw_rmb_scroll_anchor(original_draw_anchor);
        TheInGameUI::set_move_rmb_scroll_anchor(original_move_anchor);
        let mut global = runtime_global_data::write();
        global.save_camera_in_replay = original_save_camera;
        global.use_camera_in_replay = original_use_camera;
    }

    #[test]
    fn options_move_rmb_scroll_anchor_publishes_only_to_an_enabled_host() {
        use crate::gui::options_host_bridge::HostOptionsRequest;

        let mut messages = Vec::new();
        let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menu_host = Some(&mut mailbox);
        let original_move_anchor = TheInGameUI::get_move_rmb_scroll_anchor();
        let mut menu = OptionsMenu::new();
        menu.check_move_anchor_id = 12;

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_move_anchor_id,
            true,
            &mut effects
        ));
        assert_eq!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default(),
            vec![HostOptionsRequest::MoveRmbScrollAnchor { enabled: true }]
        );
        assert!(TheInGameUI::get_move_rmb_scroll_anchor());

        effects.menu_host = None;
        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_move_anchor_id,
            false,
            &mut effects
        ));
        assert!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default()
                .is_empty()
        );
        assert!(
            !TheInGameUI::get_move_rmb_scroll_anchor(),
            "disabled host delivery must retain the standalone legacy callback"
        );

        TheInGameUI::set_move_rmb_scroll_anchor(original_move_anchor);
    }

    #[test]
    fn options_draw_rmb_scroll_anchor_publishes_only_to_an_enabled_host() {
        use crate::gui::options_host_bridge::HostOptionsRequest;

        let mut messages = Vec::new();
        let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menu_host = Some(&mut mailbox);
        let original_draw_anchor = TheInGameUI::get_draw_rmb_scroll_anchor();
        let mut menu = OptionsMenu::new();
        menu.check_draw_anchor_id = 11;

        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_draw_anchor_id,
            true,
            &mut effects
        ));
        assert_eq!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default(),
            vec![HostOptionsRequest::DrawRmbScrollAnchor { enabled: true }]
        );
        assert!(TheInGameUI::get_draw_rmb_scroll_anchor());

        effects.menu_host = None;
        assert!(menu.apply_immediate_checkbox_effect(
            menu.check_draw_anchor_id,
            false,
            &mut effects
        ));
        assert!(
            effects
                .menu_host
                .as_deref_mut()
                .map(|host| host.options.take_requests())
                .unwrap_or_default()
                .is_empty()
        );
        assert!(
            !TheInGameUI::get_draw_rmb_scroll_anchor(),
            "disabled host delivery must retain the standalone legacy callback"
        );

        TheInGameUI::set_draw_rmb_scroll_anchor(original_draw_anchor);
    }

    #[test]
    fn options_accept_converges_alternate_mouse_residences_and_publishes_to_main() {
        use crate::gui::options_host_bridge::HostOptionsRequest;

        let mut messages = Vec::new();
        let mut mailbox = crate::gui::campaign_launch_host_bridge::MenuHostMailbox::default();
        let mut effects = WindowEventEffects::new(&mut messages);
        effects.menu_host = Some(&mut mailbox);

        with_global_data_residences_restored(|| {
            let ini_global = game_engine::common::ini::ini_game_data::ensure_global_data();
            ini_global.write().use_alternate_mouse = false;
            runtime_global_data::write().use_alternate_mouse = true;

            OptionsMenu::commit_alternate_mouse_setting(
                true,
                effects
                    .menu_host
                    .as_deref_mut()
                    .map(|host| &mut host.options),
            );
            assert!(ini_global.read().use_alternate_mouse);
            assert!(runtime_global_data::read().use_alternate_mouse);
            assert_eq!(
                effects
                    .menu_host
                    .as_deref_mut()
                    .map(|host| host.options.take_requests())
                    .unwrap_or_default(),
                vec![HostOptionsRequest::AlternateMouse { enabled: true }]
            );

            OptionsMenu::commit_alternate_mouse_setting(
                false,
                effects
                    .menu_host
                    .as_deref_mut()
                    .map(|host| &mut host.options),
            );
            assert!(!ini_global.read().use_alternate_mouse);
            assert!(!runtime_global_data::read().use_alternate_mouse);
            assert_eq!(
                effects
                    .menu_host
                    .as_deref_mut()
                    .map(|host| host.options.take_requests())
                    .unwrap_or_default(),
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
mod menu_callbacks_shell_borrow_residual_tests {}

/// Disjoint operation-scoped loans of the canonical client menu fields.
/// Taking a loan never moves, replaces, or publishes the menu object.
pub struct MenuCallbackRecipients<'a> {
    pub(crate) challenge: &'a mut crate::gui::callbacks::challenge_menu::ChallengeMenuState,
    pub(crate) challenge_generals: &'a crate::gui::challenge_generals::ChallengeGenerals,
    pub(crate) control_bar_callbacks:
        Option<&'a mut crate::gui::callbacks::control_bar_callbacks::ControlBarCallbacks>,
    pub(crate) message_boxes: &'a mut crate::gui::callbacks::message_box::MessageBoxSystem,
    pub(crate) diplomacy: &'a mut crate::gui::callbacks::diplomacy::DiplomacyCallbacks,
    pub(crate) input_authority: crate::core::game_client::ClientInputAuthority,
    pub(crate) ingame_ui: &'a mut crate::gui::callbacks::ingame_callbacks::InGameUISystem,
    pub(crate) difficulty_select:
        &'a mut crate::gui::callbacks::difficulty_select::DifficultySelectMenuState,
    pub(crate) save_load: &'a mut crate::gui::callbacks::popup_save_load::SaveLoadMenuState,
    pub(crate) save_load_host:
        &'a mut crate::gui::callbacks::popup_save_load::HostPopupSaveLoadBridge,
    pub(crate) popup_replay: Option<&'a mut crate::gui::callbacks::popup_replay::PopupReplayState>,
    pub(crate) flags: Option<&'a mut crate::gui::menu_flags::MenuFlags>,
    pub(crate) score: Option<&'a mut crate::gui::callbacks::score_screen::ScoreScreenState>,
    pub(crate) quit: Option<&'a mut crate::gui::callbacks::quit_menu::QuitMenuState>,
    pub(crate) popup:
        Option<&'a mut crate::gui::callbacks::in_game_popup_message::PopupMessageState>,
    pub(crate) establish_connections:
        Option<&'a mut crate::gui::menus::establish_connections_menu::EstablishConnectionsMenu>,
    pub(crate) disconnect: Option<&'a mut crate::gui::menus::disconnect_menu::DisconnectMenu>,
    pub(crate) basic_main: Option<&'a mut MainMenu>,
    pub(crate) single_player: Option<&'a mut SinglePlayerMenu>,
    pub(crate) options: Option<&'a mut OptionsMenu>,
    pub(crate) map_select: Option<&'a mut MapSelectMenu>,
    pub(crate) credits: Option<&'a mut CreditsMenu>,
    pub(crate) lan_lobby: Option<&'a mut LanLobbyMenu>,
}
impl MenuManager {
    pub(crate) fn receiving_menus(
        &mut self,
        input_authority: crate::core::game_client::ClientInputAuthority,
    ) -> MenuCallbackRecipients<'_> {
        MenuCallbackRecipients {
            challenge: &mut self.challenge,
            challenge_generals: &self.challenge_generals,
            control_bar_callbacks: Some(&mut self.control_bar_callbacks),
            input_authority,
            ingame_ui: &mut self.ingame_ui,
            diplomacy: &mut self.diplomacy,
            difficulty_select: &mut self.difficulty_select,
            save_load: &mut self.save_load,
            save_load_host: &mut self.save_load_host,
            message_boxes: &mut self.message_boxes,
            quit: Some(&mut self.quit),
            flags: Some(&mut self.flags),
            popup_replay: Some(&mut self.popup_replay),
            score: Some(&mut self.score),
            popup: Some(&mut self.popup),
            establish_connections: Some(&mut self.establish_connections),
            disconnect: Some(&mut self.disconnect),
            basic_main: Some(&mut self.main_menu),
            single_player: Some(&mut self.single_player_menu),
            options: Some(&mut self.options_menu),
            map_select: Some(&mut self.map_select_menu),
            credits: Some(&mut self.credits_menu),
            lan_lobby: Some(&mut self.lan_lobby_menu),
        }
    }
}
impl MenuCallbackRecipients<'_> {
    pub fn reborrow(&mut self) -> MenuCallbackRecipients<'_> {
        MenuCallbackRecipients {
            challenge: &mut *self.challenge,
            challenge_generals: self.challenge_generals,
            control_bar_callbacks: self.control_bar_callbacks.as_deref_mut(),
            input_authority: self.input_authority,
            ingame_ui: &mut *self.ingame_ui,
            diplomacy: &mut *self.diplomacy,
            difficulty_select: &mut *self.difficulty_select,
            save_load: &mut *self.save_load,
            save_load_host: &mut *self.save_load_host,
            message_boxes: &mut *self.message_boxes,
            quit: self.quit.as_deref_mut(),
            flags: self.flags.as_deref_mut(),
            popup_replay: self.popup_replay.as_deref_mut(),
            score: self.score.as_deref_mut(),
            popup: self.popup.as_deref_mut(),
            establish_connections: self.establish_connections.as_deref_mut(),
            disconnect: self.disconnect.as_deref_mut(),
            basic_main: self.basic_main.as_deref_mut(),
            single_player: self.single_player.as_deref_mut(),
            options: self.options.as_deref_mut(),
            map_select: self.map_select.as_deref_mut(),
            credits: self.credits.as_deref_mut(),
            lan_lobby: self.lan_lobby.as_deref_mut(),
        }
    }
}

impl MenuManager {
    pub fn diplomacy(&self) -> &crate::gui::callbacks::diplomacy::DiplomacyCallbacks {
        &self.diplomacy
    }
    pub fn diplomacy_mut(&mut self) -> &mut crate::gui::callbacks::diplomacy::DiplomacyCallbacks {
        &mut self.diplomacy
    }
}

impl MenuManager {
    pub fn message_boxes(&self) -> &crate::gui::callbacks::message_box::MessageBoxSystem {
        &self.message_boxes
    }
}

impl MenuManager {
    pub(crate) fn init_challenge_generals(&mut self) {
        self.challenge_generals.init();
    }
    pub fn challenge_generals(&self) -> &crate::gui::challenge_generals::ChallengeGenerals {
        &self.challenge_generals
    }
    /// Admit effective definitions between callbacks; readers borrow one
    /// immutable catalog for the bounded operation.
    pub fn admit_challenge_generals(
        &mut self,
        catalog: crate::gui::challenge_generals::ChallengeGenerals,
    ) {
        self.challenge_generals = catalog;
    }
}
