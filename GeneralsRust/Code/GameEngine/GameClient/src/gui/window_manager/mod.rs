//! WindowManager Implementation
//!
//! This module provides the `WindowManager` struct, which serves as the central coordinator
//! for the entire windowing system. It manages window creation, destruction, event routing,
//! focus handling, modal dialogs, and drawing operations.

#![allow(unused_imports)]

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use crate::gui::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    RadioButtonGroup, StaticText, TabControl, TextEntry, VerticalSlider,
};

use crate::gui::game_window::*;

use crate::gui::game_window_transitions::GameWindowTransitionsHandler;
use crate::gui::w3d_gadget_draw::{
    w3d_cameo_movie_draw, w3d_clock_draw, w3d_command_bar_background_draw,
    w3d_command_bar_foreground_draw, w3d_command_bar_gen_exp_draw, w3d_command_bar_grid_draw,
    w3d_command_bar_help_popup_draw, w3d_command_bar_top_draw, w3d_credits_menu_draw,
    w3d_draw_map_preview, w3d_gadget_check_box_draw, w3d_gadget_check_box_image_draw,
    w3d_gadget_combo_box_draw, w3d_gadget_combo_box_image_draw, w3d_gadget_horizontal_slider_draw,
    w3d_gadget_horizontal_slider_image_draw, w3d_gadget_horizontal_slider_image_draw_a,
    w3d_gadget_horizontal_slider_image_draw_b, w3d_gadget_list_box_draw,
    w3d_gadget_list_box_image_draw, w3d_gadget_progress_bar_draw,
    w3d_gadget_progress_bar_image_draw, w3d_gadget_progress_bar_image_draw_a,
    w3d_gadget_push_button_draw, w3d_gadget_push_button_image_draw, w3d_gadget_radio_button_draw,
    w3d_gadget_radio_button_image_draw, w3d_gadget_static_text_draw,
    w3d_gadget_static_text_image_draw, w3d_gadget_tab_control_draw,
    w3d_gadget_tab_control_image_draw, w3d_gadget_text_entry_draw,
    w3d_gadget_text_entry_image_draw, w3d_gadget_vertical_slider_draw,
    w3d_gadget_vertical_slider_image_draw, w3d_left_hud_draw,
    w3d_main_menu_button_drop_shadow_draw, w3d_main_menu_draw, w3d_main_menu_four_draw,
    w3d_main_menu_map_border, w3d_main_menu_random_text_draw, w3d_metal_bar_menu_draw, w3d_no_draw,
    w3d_power_draw, w3d_power_draw_a, w3d_right_hud_draw, w3d_shell_menu_scheme_draw,
    w3d_thin_border_draw,
};

use crate::gui::window_script::{
    TabControlData as ScriptTabControlData, WindowDefinition, WindowLayoutDefinition,
    parse_window_script,
};

use crate::game_text::GameText;
use crate::gui::callbacks::menu_callbacks::MenuCallbacks;
use crate::gui::callbacks::{
    beacon_window_input, challenge_menu_init, challenge_menu_input, challenge_menu_shutdown,
    challenge_menu_system, challenge_menu_update, difficulty_select_init, difficulty_select_input,
    difficulty_select_system, download_menu_init, download_menu_input, download_menu_shutdown,
    download_menu_system, download_menu_update, game_info_window_init, game_info_window_system,
    generals_exp_points_input, generals_exp_points_system, get_control_bar_system,
    get_diplomacy_system, get_ingame_ui_system, get_menu_manager, get_message_box_system,
    ime_candidate_main_draw, ime_candidate_text_area_draw, ime_candidate_window_input,
    ime_candidate_window_system, in_game_popup_message_init, in_game_popup_message_input,
    in_game_popup_message_system, keyboard_options_menu_init, keyboard_options_menu_input,
    keyboard_options_menu_shutdown, keyboard_options_menu_system, keyboard_options_menu_update,
    lan_game_options_menu_init, lan_game_options_menu_input, lan_game_options_menu_shutdown,
    lan_game_options_menu_system, lan_game_options_menu_update, lan_map_select_menu_init,
    lan_map_select_menu_input, lan_map_select_menu_shutdown, lan_map_select_menu_system,
    lan_map_select_menu_update, network_direct_connect_init, network_direct_connect_input,
    network_direct_connect_shutdown, network_direct_connect_system, network_direct_connect_update,
    popup_buddy_notification_system, popup_communicator_init, popup_communicator_input,
    popup_communicator_shutdown, popup_communicator_system, popup_communicator_update,
    popup_host_game_init, popup_host_game_input, popup_host_game_system, popup_host_game_update,
    popup_join_game_init, popup_join_game_input, popup_join_game_system, popup_ladder_select_init,
    popup_ladder_select_input, popup_ladder_select_shutdown, popup_ladder_select_system,
    popup_ladder_select_update, popup_player_info_init, popup_player_info_input,
    popup_player_info_shutdown, popup_player_info_system, popup_player_info_update,
    popup_replay_init, popup_replay_input, popup_replay_shutdown, popup_replay_system,
    popup_replay_update, quit_menu_system, rc_game_details_menu_init, rc_game_details_menu_system,
    replay_menu_init, replay_menu_input, replay_menu_shutdown, replay_menu_system,
    replay_menu_update, save_load_menu_full_screen_init, save_load_menu_init, save_load_menu_input,
    save_load_menu_shutdown, save_load_menu_system, save_load_menu_update, score_screen_init,
    score_screen_input, score_screen_shutdown, score_screen_system, score_screen_update,
    skirmish_game_options_menu_init, skirmish_game_options_menu_input,
    skirmish_game_options_menu_shutdown, skirmish_game_options_menu_system,
    skirmish_game_options_menu_update, skirmish_map_select_menu_init,
    skirmish_map_select_menu_input, skirmish_map_select_menu_shutdown,
    skirmish_map_select_menu_system, skirmish_map_select_menu_update, wol_buddy_overlay_init,
    wol_buddy_overlay_input, wol_buddy_overlay_rc_menu_init, wol_buddy_overlay_rc_menu_system,
    wol_buddy_overlay_shutdown, wol_buddy_overlay_system, wol_buddy_overlay_update,
    wol_custom_score_screen_init, wol_custom_score_screen_input, wol_custom_score_screen_shutdown,
    wol_custom_score_screen_system, wol_custom_score_screen_update, wol_game_setup_menu_init,
    wol_game_setup_menu_input, wol_game_setup_menu_shutdown, wol_game_setup_menu_system,
    wol_game_setup_menu_update, wol_ladder_screen_init, wol_ladder_screen_input,
    wol_ladder_screen_shutdown, wol_ladder_screen_system, wol_ladder_screen_update,
    wol_lobby_menu_init, wol_lobby_menu_input, wol_lobby_menu_shutdown, wol_lobby_menu_system,
    wol_lobby_menu_update, wol_locale_select_init, wol_locale_select_input,
    wol_locale_select_shutdown, wol_locale_select_system, wol_locale_select_update,
    wol_login_menu_init, wol_login_menu_input, wol_login_menu_shutdown, wol_login_menu_system,
    wol_login_menu_update, wol_map_select_menu_init, wol_map_select_menu_input,
    wol_map_select_menu_shutdown, wol_map_select_menu_system, wol_map_select_menu_update,
    wol_message_window_init, wol_message_window_input, wol_message_window_shutdown,
    wol_message_window_system, wol_message_window_update, wol_qm_score_screen_init,
    wol_qm_score_screen_input, wol_qm_score_screen_shutdown, wol_qm_score_screen_system,
    wol_qm_score_screen_update, wol_quick_match_menu_init, wol_quick_match_menu_input,
    wol_quick_match_menu_shutdown, wol_quick_match_menu_system, wol_quick_match_menu_update,
    wol_status_menu_init, wol_status_menu_input, wol_status_menu_shutdown, wol_status_menu_system,
    wol_status_menu_update, wol_welcome_menu_init, wol_welcome_menu_input,
    wol_welcome_menu_shutdown, wol_welcome_menu_system, wol_welcome_menu_update,
};
use crate::gui::{MAX_DRAW_DATA, MAX_WINDOWS};

use crate::gui::header_template::get_header_template_manager;
use crate::gui::shell::main_menu::get_main_menu;
use crate::gui::{get_disconnect_menu, get_establish_connections_menu};
use crate::input::with_mouse;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{file::FileAccess, file_system::get_file_system};
use log::warn;

mod create_destroy;
mod draw;
mod focus_tab;
mod gadgets;
mod hierarchy;
mod input;
mod layout;
mod layout_load;
mod messages;
mod modal;
mod reentry;
mod script_callbacks;
mod types;
mod wnd_parse;

pub use layout::{WindowLayout, WindowLayoutInfo};
pub use reentry::{
    ReentryFallback, dispatch_os_key_to_window_manager, dispatch_os_mouse_to_window_manager,
    hide_window_rc, queue_create_layout, queue_set_focus, queue_window_manager_op,
    queue_window_manager_op_deferred, window_manager_try_borrow_free, with_window_manager,
    with_window_manager_ref,
};
pub use types::{CaptureFlags, ModalWindow, TabDirection};

pub(crate) use reentry::apply_w3d_main_menu_runtime_draw_overrides;
pub(crate) use wnd_parse::{
    apply_window_status_to_widget, apply_window_text, apply_window_tooltip,
    apply_window_widget_data, create_widget_for_style, is_none_callback_name,
    map_window_message_to_main_menu, resolve_window_script_path, style_for_window_type,
};

/// Atomic counter for generating unique window IDs
static NEXT_WINDOW_ID: AtomicI32 = AtomicI32::new(1);

/// Generate a unique window ID
pub(crate) fn generate_window_id() -> WindowId {
    NEXT_WINDOW_ID.fetch_add(1, Ordering::SeqCst)
}

pub(crate) fn with_arc_write<T, R>(
    lock: &Arc<std::sync::RwLock<T>>,
    f: impl FnOnce(&mut T) -> R,
) -> R {
    let mut guard = lock.write().unwrap_or_else(|e| e.into_inner());
    f(&mut *guard)
}

pub(crate) fn as_any_ref(user_data: Option<&mut dyn std::any::Any>) -> Option<&dyn std::any::Any> {
    user_data.map(|data| data as &dyn std::any::Any)
}

/// Main WindowManager struct
pub struct WindowManager {
    // Window lists
    root_windows: Vec<Rc<RefCell<GameWindow>>>,
    window_by_id: HashMap<WindowId, Weak<RefCell<GameWindow>>>,
    destroy_queue: VecDeque<Rc<RefCell<GameWindow>>>,

    // Focus and input handling
    keyboard_focus: Option<Weak<RefCell<GameWindow>>>,
    pending_focus: Option<WindowId>,
    mouse_capture: Option<Weak<RefCell<GameWindow>>>,
    current_mouse_region: Option<Weak<RefCell<GameWindow>>>,
    grab_window: Option<Weak<RefCell<GameWindow>>>,
    lone_window: Option<Weak<RefCell<GameWindow>>>,

    // Modal windows
    modal_stack: Option<Box<ModalWindow>>,

    // Tab handling
    tab_list: Vec<Weak<RefCell<GameWindow>>>,

    // Capture state
    capture_flags: CaptureFlags,

    // Layouts
    layouts: Vec<Rc<RefCell<WindowLayout>>>,

    // Statistics
    window_count: usize,

    // Screen size for layout scaling (defaults to 800x600)
    screen_size: (i32, i32),

    // Radio button groups keyed by .wnd group id
    radio_groups: HashMap<u32, RadioButtonGroup>,

    // Window transition handler (WindowTransitions.ini)
    transitions: GameWindowTransitionsHandler,

    // Timing for per-frame updates
    last_update: Instant,
}

impl WindowManager {
    /// Create a new WindowManager
    pub fn new() -> Self {
        Self {
            root_windows: Vec::new(),
            window_by_id: HashMap::new(),
            destroy_queue: VecDeque::new(),
            keyboard_focus: None,
            pending_focus: None,
            mouse_capture: None,
            current_mouse_region: None,
            grab_window: None,
            lone_window: None,
            modal_stack: None,
            tab_list: Vec::new(),
            capture_flags: CaptureFlags::empty(),
            layouts: Vec::new(),
            window_count: 0,
            screen_size: (800, 600),
            radio_groups: HashMap::new(),
            transitions: GameWindowTransitionsHandler::new(),
            last_update: Instant::now(),
        }
    }

    fn materialize_window_transitions_ini() -> Option<PathBuf> {
        let output = PathBuf::from("Data/INI/WindowTransitions.ini");
        if output.exists() {
            return Some(output);
        }

        let file_system = get_file_system();
        let mut fs_guard = file_system.lock().ok()?;

        for candidate in ["Data/INI/WindowTransitions.ini", "WindowTransitions.ini"] {
            let Some(mut file) =
                fs_guard.open_file(candidate, FileAccess::READ.combine(FileAccess::BINARY))
            else {
                continue;
            };
            let Ok(bytes) = file.read_entire_and_close() else {
                continue;
            };
            if let Some(parent) = output.parent() {
                let _ = fs::create_dir_all(parent);
            }
            if fs::write(&output, &bytes).is_ok() {
                return Some(output);
            }
        }

        None
    }

    /// Set the current screen size for layout scaling.
    pub fn set_screen_size(&mut self, width: i32, height: i32) {
        if width > 0 && height > 0 {
            self.screen_size = (width, height);
        }
    }

    /// Get the current screen size used for layout scaling.
    pub fn screen_size(&self) -> (i32, i32) {
        self.screen_size
    }

    /// Initialize the window manager
    pub fn init(&mut self) {
        self.transitions.init();
        let default_path = "Data/INI/WindowTransitions.ini";
        if Path::new(default_path).exists() {
            self.transitions.load(default_path);
            return;
        }

        let fallback_paths = [
            "windows_game/extracted_big_files_v2/INIZH/Data/INI/WindowTransitions.ini",
            "windows_game/extracted_big_files/INIZH/Data/INI/WindowTransitions.ini",
            "../windows_game/extracted_big_files_v2/INIZH/Data/INI/WindowTransitions.ini",
            "../windows_game/extracted_big_files/INIZH/Data/INI/WindowTransitions.ini",
        ];
        for path in fallback_paths {
            if Path::new(path).exists() {
                log::info!(
                    "WindowTransitions.ini not found at {}; using fallback {}",
                    default_path,
                    path
                );
                self.transitions.load(path);
                return;
            }

            if let Ok(cwd) = std::env::current_dir() {
                for ancestor in cwd.ancestors() {
                    let candidate = ancestor.join(path);
                    if candidate.exists() {
                        log::info!(
                            "WindowTransitions.ini not found at {}; using fallback {}",
                            default_path,
                            candidate.display()
                        );
                        self.transitions.load(candidate.to_string_lossy().as_ref());
                        return;
                    }
                }
            }
        }

        if let Some(materialized) = Self::materialize_window_transitions_ini() {
            log::info!(
                "WindowTransitions.ini not found at {}; materialized from mounted archives to {}",
                default_path,
                materialized.display()
            );
            self.transitions
                .load(materialized.to_string_lossy().as_ref());
            return;
        }

        log::warn!(
            "WindowTransitions.ini not found (searched {}, fallback paths unavailable)",
            default_path
        );
    }

    /// Reset the window manager (destroy all windows)
    pub fn reset(&mut self) {
        self.destroy_all_windows();
        self.layouts.clear();
        self.tab_list.clear();
        self.modal_stack = None;
        self.keyboard_focus = None;
        self.mouse_capture = None;
        self.current_mouse_region = None;
        self.grab_window = None;
        self.lone_window = None;
        self.capture_flags = CaptureFlags::empty();
        self.transitions.reset();
    }

    /// Update the window manager (process destroy queue, etc.)
    pub fn update(&mut self) {
        self.process_destroy_queue();
        if let Some(id) = self.pending_focus.take() {
            if let Some(window) = self.get_window_by_id(id) {
                let _ = self.set_focus(Some(&window));
            }
        }
        self.transitions.update();
        let now = Instant::now();
        let delta_time = now.duration_since(self.last_update).as_secs_f32();
        self.last_update = now;
        self.update_press_animations(delta_time);
    }

    fn update_press_animations(&mut self, delta_time: f32) {
        for window in &self.root_windows {
            Self::update_press_animation_recursive(window, delta_time);
        }
    }

    fn update_press_animation_recursive(window: &Rc<RefCell<GameWindow>>, delta_time: f32) {
        {
            let mut win = window.borrow_mut();
            win.update_press_animation(delta_time);
        }
        let children = window.borrow().children().to_vec();
        for child in children {
            Self::update_press_animation_recursive(&child, delta_time);
        }
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;

/// Concatenated live sources for residual `include_str!` scans.
pub const WINDOW_MANAGER_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("create_destroy.rs"),
    include_str!("draw.rs"),
    include_str!("focus_tab.rs"),
    include_str!("gadgets.rs"),
    include_str!("hierarchy.rs"),
    include_str!("input.rs"),
    include_str!("layout.rs"),
    include_str!("layout_load.rs"),
    include_str!("messages.rs"),
    include_str!("modal.rs"),
    include_str!("reentry.rs"),
    include_str!("script_callbacks.rs"),
    include_str!("types.rs"),
    include_str!("wnd_parse.rs"),
);
