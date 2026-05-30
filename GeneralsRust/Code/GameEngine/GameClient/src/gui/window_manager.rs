//! WindowManager Implementation
//!
//! This module provides the `WindowManager` struct, which serves as the central coordinator
//! for the entire windowing system. It manages window creation, destruction, event routing,
//! focus handling, modal dialogs, and drawing operations.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use std::thread_local;
use std::time::Instant;

use super::gadgets::{
    CheckBox, ComboBox, HorizontalSlider, ListBox, ProgressBar, PushButton, RadioButton,
    RadioButtonGroup, StaticText, TabControl, TextEntry, VerticalSlider,
};
use super::game_window::*;
use super::game_window_transitions::GameWindowTransitionsHandler;
use super::w3d_gadget_draw::{
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
use super::window_script::{parse_window_script, WindowDefinition, WindowLayoutDefinition};
use super::{MAX_DRAW_DATA, MAX_WINDOWS};
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
use crate::gui::header_template::get_header_template_manager;
use crate::gui::shell::main_menu::get_main_menu;
use crate::gui::{get_disconnect_menu, get_establish_connections_menu};
use crate::input::with_mouse;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{file::FileAccess, file_system::get_file_system};
use log::warn;

thread_local! {
    static THE_WINDOW_MANAGER: RefCell<WindowManager> = RefCell::new(WindowManager::new());
}

pub fn with_window_manager<R>(f: impl FnOnce(&mut WindowManager) -> R) -> R {
    THE_WINDOW_MANAGER.with(|manager| {
        if let Ok(mut borrow) = manager.try_borrow_mut() {
            return f(&mut borrow);
        }

        // C++ parity: shell/input callbacks can re-enter TheWindowManager while outer
        // event dispatch already holds the singleton mutably. The original engine uses
        // a re-entrant global singleton here; Rust's RefCell would panic instead.
        let ptr = manager.as_ptr();
        // SAFETY: this mirrors the legacy single-threaded singleton access pattern used
        // by the shell/window system. It is constrained to the UI thread.
        unsafe { f(&mut *ptr) }
    })
}

pub fn with_window_manager_ref<R>(f: impl FnOnce(&WindowManager) -> R) -> R {
    THE_WINDOW_MANAGER.with(|manager| {
        if let Ok(borrow) = manager.try_borrow() {
            return f(&borrow);
        }

        // C++ parity: shell/window draw callbacks read TheWindowManager while the outer
        // frame traversal is already mutably iterating it. Rust's RefCell would panic
        // here, but the legacy engine treats this as a re-entrant singleton read.
        let ptr = manager.as_ptr();
        // SAFETY: this path only exposes `&WindowManager`, never `&mut WindowManager`.
        // It is used to mirror legacy singleton read access during draw traversal.
        unsafe { f(&*ptr) }
    })
}

fn apply_draw_callback_override(window_name: &str, draw: fn(&GameWindow, &WindowInstanceData)) {
    with_window_manager(|manager| {
        if let Some(window) = manager.find_window_by_name(window_name) {
            window.borrow_mut().set_draw_callback(draw);
        }
    });
}

fn apply_w3d_main_menu_runtime_draw_overrides() {
    for window_name in [
        "MainMenu.wnd:ButtonSkirmish",
        "MainMenu.wnd:ButtonOnline",
        "MainMenu.wnd:ButtonNetwork",
        "MainMenu.wnd:ButtonUSA",
        "MainMenu.wnd:ButtonGLA",
        "MainMenu.wnd:ButtonChina",
        "MainMenu.wnd:ButtonMultiBack",
        "MainMenu.wnd:ButtonSingleBack",
        "MainMenu.wnd:ButtonExit",
        "MainMenu.wnd:ButtonOptions",
        "MainMenu.wnd:ButtonMultiplayer",
        "MainMenu.wnd:ButtonSinglePlayer",
        "MainMenu.wnd:ButtonReplay",
        "MainMenu.wnd:ButtonLoadGame",
        "MainMenu.wnd:ButtonLoadReplay",
        "MainMenu.wnd:ButtonLoadReplayBack",
        "MainMenu.wnd:ButtonTRAINING",
        "MainMenu.wnd:ButtonCredits",
    ] {
        apply_draw_callback_override(window_name, w3d_main_menu_button_drop_shadow_draw);
    }

    for window_name in [
        "MainMenu.wnd:StaticTextRandom1",
        "MainMenu.wnd:StaticTextRandom2",
    ] {
        apply_draw_callback_override(window_name, w3d_main_menu_random_text_draw);
    }
}

/// Atomic counter for generating unique window IDs
static NEXT_WINDOW_ID: AtomicI32 = AtomicI32::new(1);

/// Generate a unique window ID
fn generate_window_id() -> WindowId {
    NEXT_WINDOW_ID.fetch_add(1, Ordering::SeqCst)
}

fn with_arc_write<T, R>(lock: &Arc<std::sync::RwLock<T>>, f: impl FnOnce(&mut T) -> R) -> R {
    let mut guard = lock.write().unwrap_or_else(|e| e.into_inner());
    f(&mut *guard)
}

fn as_any_ref(user_data: Option<&mut dyn std::any::Any>) -> Option<&dyn std::any::Any> {
    user_data.map(|data| data as &dyn std::any::Any)
}

/// Tab navigation direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabDirection {
    Next,
    Previous,
}

/// Capture flags for input capture
bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CaptureFlags: u32 {
        const MOUSE = 0x00000001;
        const KEYBOARD = 0x00000002;
        const ALL = 0xFFFFFFFF;
    }
}

/// Modal window wrapper
#[derive(Debug)]
pub struct ModalWindow {
    pub window: Rc<RefCell<GameWindow>>,
    pub next: Option<Box<ModalWindow>>,
}

impl ModalWindow {
    pub fn new(window: Rc<RefCell<GameWindow>>) -> Self {
        Self { window, next: None }
    }
}

impl Clone for ModalWindow {
    fn clone(&self) -> Self {
        Self {
            window: Rc::clone(&self.window),
            next: self.next.as_ref().map(|next| Box::new((**next).clone())),
        }
    }
}

/// Window layout for grouping related windows
pub struct WindowLayout {
    filename: String,
    windows: Vec<Rc<RefCell<GameWindow>>>,
    hidden: Cell<bool>,
    default_text_color: Option<Color>,
    default_font: Option<GameFont>,
    // Layout callbacks would be function pointers in the original
    init_callback: Option<Box<dyn Fn(&WindowLayout, Option<&dyn std::any::Any>)>>,
    update_callback: Option<Box<dyn Fn(&WindowLayout, Option<&dyn std::any::Any>)>>,
    shutdown_callback: Option<Box<dyn Fn(&WindowLayout, Option<&mut dyn std::any::Any>)>>,
}

impl std::fmt::Debug for WindowLayout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WindowLayout")
            .field("filename", &self.filename)
            .field("window_count", &self.windows.len())
            .field("hidden", &self.hidden.get())
            .finish()
    }
}

impl WindowLayout {
    pub fn new(filename: String) -> Self {
        Self {
            filename,
            windows: Vec::new(),
            hidden: Cell::new(false),
            default_text_color: None,
            default_font: None,
            init_callback: None,
            update_callback: None,
            shutdown_callback: None,
        }
    }

    /// Get the filename associated with this layout
    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    /// Check if layout is hidden
    pub fn is_hidden(&self) -> bool {
        self.hidden.get()
    }

    /// Hide or show all windows in this layout
    pub fn hide(&self, hide: bool) {
        for window_rc in &self.windows {
            if window_rc.borrow().get_parent().is_none() {
                let mut window = window_rc.borrow_mut();
                let _ = window.hide(hide);
            }
        }
        self.hidden.set(hide);
    }

    /// Add window to this layout
    pub fn add_window(&mut self, window: Rc<RefCell<GameWindow>>) {
        // Check if window is already in layout
        let window_ptr = window.as_ptr();
        if !self.windows.iter().any(|w| w.as_ptr() == window_ptr) {
            self.windows.push(window);
        }
    }

    /// Access windows owned by this layout.
    pub fn windows(&self) -> &[Rc<RefCell<GameWindow>>] {
        &self.windows
    }

    /// Remove window from this layout
    pub fn remove_window(&mut self, window: &Rc<RefCell<GameWindow>>) {
        let window_ptr = window.as_ptr();
        self.windows.retain(|w| w.as_ptr() != window_ptr);
    }

    /// Get first window in layout
    pub fn get_first_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.windows.first().cloned()
    }

    /// Bring all windows in this layout to front
    pub fn bring_forward(&mut self) {
        with_window_manager(|manager| manager.bring_layout_forward(self));
    }

    /// Run initialization callback
    pub fn run_init(&self, user_data: Option<&dyn std::any::Any>) {
        if let Some(ref callback) = self.init_callback {
            callback(self, user_data);
        }
    }

    /// Run update callback
    pub fn run_update(&self, user_data: Option<&dyn std::any::Any>) {
        if let Some(ref callback) = self.update_callback {
            callback(self, user_data);
        }
    }

    /// Run shutdown callback
    pub fn run_shutdown(&self, user_data: Option<&mut dyn std::any::Any>) {
        if let Some(ref callback) = self.shutdown_callback {
            callback(self, user_data);
        }
    }

    /// Destroy all windows in this layout
    pub fn destroy_windows(&mut self) {
        let windows = self.windows.iter().cloned().rev().collect::<Vec<_>>();

        with_window_manager(|manager| {
            for window in windows {
                let _ = manager.destroy_window(window);
            }
            manager.flush_destroy_queue();
        });

        self.windows.clear();
    }
}

/// Layout information returned from script loading
#[derive(Debug, Default)]
pub struct WindowLayoutInfo {
    pub version: u32,
    pub init_callback_name: String,
    pub update_callback_name: String,
    pub shutdown_callback_name: String,
    pub windows: Vec<Rc<RefCell<GameWindow>>>,
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

    /// Load a window layout file and return the first window instance.
    pub fn load_window(&mut self, filename: &str) -> WindowResult<Rc<RefCell<GameWindow>>> {
        let layout_info = self.create_windows_from_script(filename)?;
        layout_info
            .windows
            .first()
            .cloned()
            .ok_or(WindowError::InvalidParameter)
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

    /// Create a new window
    pub fn create_window(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        let window_id = generate_window_id();
        self.create_window_with_id(parent, x, y, width, height, window_id)
    }

    /// Create a new window with an explicit ID.
    pub fn create_window_with_id(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        window_id: WindowId,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        self.create_window_with_id_internal(parent, x, y, width, height, window_id, true)
    }

    fn create_window_with_id_internal(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        window_id: WindowId,
        send_create: bool,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        if self.window_count >= MAX_WINDOWS {
            return Err(WindowError::OutOfWindows);
        }

        let window = Rc::new(RefCell::new(GameWindow::new()));

        // Set up window properties
        {
            let mut window_mut = window.borrow_mut();
            window_mut.set_id(window_id);
            window_mut.set_position(x, y)?;
            window_mut.set_size(width, height)?;
            window_mut.enable(true)?;
        }

        // Add to parent or root list
        if let Some(parent_rc) = parent {
            window.borrow_mut().set_parent(Some(parent_rc));
            parent_rc.borrow_mut().add_child(window.clone());
        } else {
            self.add_root_window(window.clone());
        }

        // Register in lookup table
        self.window_by_id.insert(window_id, Rc::downgrade(&window));
        self.window_count += 1;

        // Send create message
        if send_create {
            let _msg_result = window
                .borrow_mut()
                .send_system_message(WindowMessage::Create, 0, 0);
        }

        Ok(window)
    }

    /// Destroy a window
    pub fn destroy_window(&mut self, window: Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        // Add to destroy queue for safe deletion
        self.destroy_queue.push_back(window);
        Ok(())
    }

    /// Destroy all windows
    pub fn destroy_all_windows(&mut self) {
        // Add all root windows to destroy queue
        for window in self.root_windows.drain(..) {
            self.destroy_queue.push_back(window);
        }

        // Process destroy queue
        self.process_destroy_queue();
    }

    /// Process any queued window destruction immediately.
    pub fn flush_destroy_queue(&mut self) {
        self.process_destroy_queue();
    }

    /// Get window by ID
    pub fn get_window_by_id(&self, id: WindowId) -> Option<Rc<RefCell<GameWindow>>> {
        self.window_by_id.get(&id)?.upgrade()
    }

    /// Get the window list (root windows)
    pub fn get_window_list(&self) -> &[Rc<RefCell<GameWindow>>] {
        &self.root_windows
    }

    /// Get the total number of windows managed by this WindowManager.
    /// C++ parity: mirrors `TheWindowManager->winGetWindowList() != NULL` check.
    pub fn window_count(&self) -> usize {
        self.window_count
    }

    pub fn root_window_count(&self) -> usize {
        self.root_windows.len()
    }

    pub fn debug_collect_window_texts_by_prefix(
        &self,
        prefix: &str,
    ) -> Vec<(String, String, String, bool, Option<String>)> {
        fn collect(
            out: &mut Vec<(String, String, String, bool, Option<String>)>,
            prefix: &str,
            window: &Rc<RefCell<GameWindow>>,
        ) {
            let guard = window.borrow();
            if guard.get_name().starts_with(prefix) {
                let parent_name = guard
                    .get_parent()
                    .map(|parent| parent.borrow().get_name().to_string());
                out.push((
                    guard.get_name().to_string(),
                    guard.get_text().to_string(),
                    guard.get_text_label().to_string(),
                    guard.is_hidden(),
                    parent_name,
                ));
            }
            let children = guard.children().to_vec();
            drop(guard);
            for child in &children {
                collect(out, prefix, child);
            }
        }

        let mut out = Vec::new();
        for root in &self.root_windows {
            collect(&mut out, prefix, root);
        }
        out
    }

    pub fn debug_collect_window_draws_by_prefix(
        &self,
        prefix: &str,
    ) -> Vec<(
        String,
        bool,
        (i32, i32),
        (i32, i32),
        Option<String>,
        Option<String>,
    )> {
        fn collect(
            out: &mut Vec<(
                String,
                bool,
                (i32, i32),
                (i32, i32),
                Option<String>,
                Option<String>,
            )>,
            prefix: &str,
            window: &Rc<RefCell<GameWindow>>,
        ) {
            let guard = window.borrow();
            if guard.get_name().starts_with(prefix) {
                let parent_name = guard
                    .get_parent()
                    .map(|parent| parent.borrow().get_name().to_string());
                let image = guard
                    .get_enabled_draw_data(0)
                    .and_then(|entry| entry.image)
                    .map(|image| image.name);
                out.push((
                    guard.get_name().to_string(),
                    guard.is_hidden(),
                    guard.get_screen_position(),
                    guard.get_size(),
                    parent_name,
                    image,
                ));
            }
            let children = guard.children().to_vec();
            drop(guard);
            for child in &children {
                collect(out, prefix, child);
            }
        }

        let mut out = Vec::new();
        for root in &self.root_windows {
            collect(&mut out, prefix, root);
        }
        out
    }

    pub fn find_window_by_name(&self, name: &str) -> Option<Rc<RefCell<GameWindow>>> {
        fn find_recursive(
            name: &str,
            window: &Rc<RefCell<GameWindow>>,
        ) -> Option<Rc<RefCell<GameWindow>>> {
            let guard = window.borrow();
            if guard.get_name().eq_ignore_ascii_case(name) {
                return Some(window.clone());
            }
            let children = guard.children().to_vec();
            drop(guard);
            for child in &children {
                if let Some(found) = find_recursive(name, child) {
                    return Some(found);
                }
            }
            None
        }

        for root in &self.root_windows {
            if let Some(found) = find_recursive(name, root) {
                return Some(found);
            }
        }
        None
    }

    pub fn find_window_from_id(
        &self,
        base_window: &Rc<RefCell<GameWindow>>,
        id: WindowId,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        fn find_recursive(
            window: &Rc<RefCell<GameWindow>>,
            id: WindowId,
        ) -> Option<Rc<RefCell<GameWindow>>> {
            let Ok(guard) = window.try_borrow() else {
                return None;
            };
            if guard.get_id() == id {
                return Some(window.clone());
            }
            let children = guard.children().to_vec();
            drop(guard);
            for child in &children {
                if let Some(found) = find_recursive(child, id) {
                    return Some(found);
                }
            }
            None
        }

        find_recursive(base_window, id)
    }

    pub fn bring_layout_forward(&mut self, layout: &WindowLayout) {
        for window in &layout.windows {
            self.root_windows.retain(|root| !Rc::ptr_eq(root, window));
        }
        for window in layout.windows.iter().rev() {
            if window.borrow().get_parent().is_none() {
                self.root_windows.push(window.clone());
            }
        }
    }

    pub fn bring_window_forward(&mut self, window: &Rc<RefCell<GameWindow>>) {
        if let Some(parent) = window.borrow().get_parent() {
            let mut parent = parent.borrow_mut();
            let children = parent.children_mut();
            if let Some(index) = children.iter().position(|child| Rc::ptr_eq(child, window)) {
                let child = children.remove(index);
                children.push(child);
            }
        } else if let Some(index) = self
            .root_windows
            .iter()
            .position(|root| Rc::ptr_eq(root, window))
        {
            let root = self.root_windows.remove(index);
            self.root_windows.push(root);
        }
    }

    /// Set keyboard focus to a window
    pub fn set_focus(&mut self, window: Option<&Rc<RefCell<GameWindow>>>) -> WindowResult<()> {
        if let Some(new_focus) = window {
            if new_focus
                .borrow()
                .get_status()
                .contains(WindowStatus::NO_FOCUS)
            {
                return Ok(());
            }
        }

        // Clear old focus
        if let Some(old_focus_weak) = &self.keyboard_focus {
            if let Some(old_focus) = old_focus_weak.upgrade() {
                let changing_focus = window
                    .map(|new_focus| !Rc::ptr_eq(&old_focus, new_focus))
                    .unwrap_or(true);
                if changing_focus {
                    old_focus
                        .borrow_mut()
                        .send_system_message(WindowMessage::InputFocus, 0, 0);
                }
            }
        }

        // Set new focus
        if let Some(new_focus) = window {
            self.keyboard_focus = Some(Rc::downgrade(new_focus));

            let mut wants_focus = false;
            let mut focus_probe = Some(new_focus.clone());
            while let Some(window) = focus_probe {
                let result =
                    window
                        .borrow_mut()
                        .send_system_message(WindowMessage::InputFocus, 1, 0);
                if result == WindowMsgHandled::Handled {
                    wants_focus = true;
                    break;
                }
                focus_probe = window.borrow().get_parent();
            }

            if !wants_focus {
                self.keyboard_focus = None;
            }
        } else {
            self.keyboard_focus = None;
        }

        Ok(())
    }

    pub fn request_focus(&mut self, id: WindowId) {
        self.pending_focus = Some(id);
    }

    /// Get window that has keyboard focus
    pub fn get_focus(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.keyboard_focus.as_ref()?.upgrade()
    }

    /// Capture mouse input to a window
    pub fn capture_mouse(&mut self, window: &Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        if self.mouse_capture.is_some() {
            return Err(WindowError::MouseCaptured);
        }

        self.mouse_capture = Some(Rc::downgrade(window));
        self.capture_flags |= CaptureFlags::MOUSE;
        Ok(())
    }

    /// Release mouse capture
    pub fn release_capture(&mut self, window: &Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        if let Some(capture_weak) = &self.mouse_capture {
            if let Some(capture_window) = capture_weak.upgrade() {
                if Rc::ptr_eq(&capture_window, window) {
                    self.mouse_capture = None;
                    self.capture_flags &= !CaptureFlags::MOUSE;
                }
            }
        }
        Ok(())
    }

    /// Get window that has mouse capture
    pub fn get_capture(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.mouse_capture.as_ref()?.upgrade()
    }

    /// Set modal window
    pub fn set_modal(&mut self, window: Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        if window.borrow().get_parent().is_some() {
            return Err(WindowError::InvalidParameter);
        }

        let modal_window = Box::new(ModalWindow::new(window));

        // Push onto modal stack
        if let Some(old_head) = self.modal_stack.take() {
            let mut new_modal = modal_window;
            new_modal.next = Some(old_head);
            self.modal_stack = Some(new_modal);
        } else {
            self.modal_stack = Some(modal_window);
        }

        Ok(())
    }

    /// Remove modal window
    pub fn unset_modal(&mut self, window: &Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        if let Some(modal_head) = &self.modal_stack {
            if Rc::ptr_eq(&modal_head.window, window) {
                self.modal_stack = modal_head.next.as_ref().map(|n| n.clone());
                return Ok(());
            }
        }
        Err(WindowError::InvalidWindow)
    }

    /// Set grab window (for drag operations)
    pub fn set_grab_window(&mut self, window: Option<&Rc<RefCell<GameWindow>>>) {
        self.grab_window = window.map(|w| Rc::downgrade(w));
    }

    /// Get grab window
    pub fn get_grab_window(&self) -> Option<Rc<RefCell<GameWindow>>> {
        self.grab_window.as_ref()?.upgrade()
    }

    /// Set lone window (for exclusive operations like combo boxes)
    pub fn set_lone_window(&mut self, window: Option<&Rc<RefCell<GameWindow>>>) {
        const GGM_LEFT_DRAG: u32 = 16384;
        const GGM_CLOSE: u32 = GGM_LEFT_DRAG + 5;
        if let Some(old) = self.lone_window.as_ref().and_then(|w| w.upgrade()) {
            let same = window.map(|w| Rc::ptr_eq(&old, w)).unwrap_or(false);
            if !same {
                let _ = old
                    .borrow_mut()
                    .send_system_message(WindowMessage::User(GGM_CLOSE), 0, 0);
            }
        }
        self.lone_window = window.map(|w| Rc::downgrade(w));
    }

    /// Process mouse event
    pub fn process_mouse_event(
        &mut self,
        msg: WindowMessage,
        x: i32,
        y: i32,
        data: WindowMsgData,
    ) -> WindowInputReturnCode {
        const GGM_LEFT_DRAG: u32 = 16384;
        const GGM_CLOSE: u32 = GGM_LEFT_DRAG + 5;
        let old_lone = self.lone_window.as_ref().and_then(|w| w.upgrade());
        self.update_cursor_tooltip_for_mouse_event(x, y);
        // Find window under cursor or use capture
        let target_window = if let Some(capture) = self.get_capture() {
            Some(capture)
        } else {
            self.get_window_under_cursor(x, y, false)
        };

        // Match classic UI behavior: capture mouse on press so release restores state
        if msg == WindowMessage::LeftDown {
            if self.get_capture().is_none() {
                if let Some(window) = target_window.as_ref() {
                    let _ = self.capture_mouse(window);
                }
            }
        }

        if msg == WindowMessage::MousePos {
            let previous = self.current_mouse_region.as_ref().and_then(|w| w.upgrade());
            let same = match (&previous, &target_window) {
                (Some(prev), Some(cur)) => Rc::ptr_eq(prev, cur),
                (None, None) => true,
                _ => false,
            };
            if !same {
                if let Some(prev) = previous {
                    let _ = prev
                        .borrow_mut()
                        .send_input_message(WindowMessage::MouseLeaving, 0, 0);
                }
                if let Some(ref new_window) = target_window {
                    let _ = new_window.borrow_mut().send_input_message(
                        WindowMessage::MouseEntering,
                        0,
                        0,
                    );
                    self.current_mouse_region = Some(Rc::downgrade(new_window));
                } else {
                    self.current_mouse_region = None;
                }
            }
        }

        let focus_window = self.get_focus();
        if let Some(window) = target_window {
            let (wx, wy) = window.borrow().get_screen_position();
            let _ = window.borrow_mut().set_cursor_position(x - wx, y - wy);
            // Send message to window
            let result = window.borrow_mut().send_input_message(msg, data, 0);

            if msg == WindowMessage::MousePos {
                if let Some(focus_window) = focus_window {
                    if !Rc::ptr_eq(&focus_window, &window) {
                        let (fx, fy) = focus_window.borrow().get_screen_position();
                        let _ = focus_window
                            .borrow_mut()
                            .set_cursor_position(x - fx, y - fy);
                        let _ = focus_window.borrow_mut().send_input_message(msg, data, 0);
                    }
                }
            }

            if matches!(
                msg,
                WindowMessage::LeftUp | WindowMessage::MiddleUp | WindowMessage::RightUp
            ) {
                if let Some(lone) = old_lone {
                    let inside = lone.borrow().contains_descendant(&window.borrow());
                    if !inside {
                        let _ = lone.borrow_mut().send_system_message(
                            WindowMessage::User(GGM_CLOSE),
                            0,
                            0,
                        );
                        self.lone_window = None;
                    }
                }
            }
            // Release capture after mouse-up so press animations unwind correctly.
            if msg == WindowMessage::LeftUp {
                if let Some(capture) = self.get_capture() {
                    let _ = self.release_capture(&capture);
                }
            }

            match result {
                WindowMsgHandled::Handled => WindowInputReturnCode::Used,
                WindowMsgHandled::Ignored => WindowInputReturnCode::NotUsed,
            }
        } else {
            if msg == WindowMessage::MousePos {
                if let Some(focus_window) = focus_window {
                    let (fx, fy) = focus_window.borrow().get_screen_position();
                    let _ = focus_window
                        .borrow_mut()
                        .set_cursor_position(x - fx, y - fy);
                    let _ = focus_window.borrow_mut().send_input_message(msg, data, 0);
                }
            }
            if msg == WindowMessage::LeftUp {
                if let Some(capture) = self.get_capture() {
                    let _ = self.release_capture(&capture);
                }
            }
            if matches!(
                msg,
                WindowMessage::LeftUp | WindowMessage::MiddleUp | WindowMessage::RightUp
            ) {
                if let Some(lone) = old_lone {
                    let _ =
                        lone.borrow_mut()
                            .send_system_message(WindowMessage::User(GGM_CLOSE), 0, 0);
                    self.lone_window = None;
                }
            }
            WindowInputReturnCode::NotUsed
        }
    }

    fn update_cursor_tooltip_for_mouse_event(&self, x: i32, y: i32) {
        with_mouse(|mouse| mouse.set_cursor_tooltip(String::new(), None, None, None));

        if self.get_capture().is_some() || self.get_grab_window().is_some() {
            return;
        }

        let tooltip = self.get_window_under_cursor(x, y, true).and_then(|window| {
            let window = window.borrow();
            let tooltip = window.get_tooltip();
            if tooltip.is_empty() {
                None
            } else {
                Some((tooltip.to_string(), window.get_tooltip_delay()))
            }
        });

        if let Some((tooltip, delay)) = tooltip {
            with_mouse(|mouse| mouse.set_cursor_tooltip(tooltip, Some(delay), None, None));
        }
    }

    /// Process key event
    pub fn process_key_event(&mut self, key: u8, state: u8) -> WindowInputReturnCode {
        if key == 0 {
            return WindowInputReturnCode::NotUsed;
        }

        if let Some(mut window) = self.get_focus() {
            loop {
                let result = window.borrow_mut().send_input_message(
                    WindowMessage::Char,
                    key as u32,
                    state as u32,
                );
                if result == WindowMsgHandled::Handled {
                    return WindowInputReturnCode::Used;
                }

                let parent = window.borrow().get_parent();
                if let Some(parent) = parent {
                    window = parent;
                } else {
                    return WindowInputReturnCode::NotUsed;
                }
            }
        } else {
            WindowInputReturnCode::NotUsed
        }
    }

    /// Get window under cursor coordinates
    pub fn get_window_under_cursor(
        &self,
        x: i32,
        y: i32,
        ignore_enabled: bool,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        // Check modal windows first
        if let Some(modal) = &self.modal_stack {
            if let Some(window) = self.find_window_at_point(&modal.window, x, y, ignore_enabled) {
                return Some(window);
            }
        }

        // Match C++ getWindowUnderCursor: root windows are tested head-first in
        // ABOVE, normal, then BELOW passes so input priority mirrors status.
        let passes: [fn(WindowStatus) -> bool; 3] = [
            |status| status.contains(WindowStatus::ABOVE),
            |status| !status.intersects(WindowStatus::ABOVE | WindowStatus::BELOW),
            |status| status.contains(WindowStatus::BELOW),
        ];

        for matches_pass in passes {
            for window in &self.root_windows {
                if !matches_pass(window.borrow().get_status()) {
                    continue;
                }
                if let Some(found) = self.find_window_at_point(window, x, y, ignore_enabled) {
                    return Some(found);
                }
            }
        }

        None
    }

    /// Navigate to next/previous tab
    pub fn navigate_tab(&mut self, direction: TabDirection) {
        if self.tab_list.is_empty() || self.modal_stack.is_some() {
            return;
        }

        let current_focus = self.get_focus();
        let mut next_window = None;

        // Clean up dead references
        self.tab_list.retain(|w| w.upgrade().is_some());
        if self.tab_list.is_empty() {
            return;
        }

        if let Some(current) = current_focus {
            // Find current window in tab list
            let current_ptr = current.as_ptr();
            let current_index = self
                .tab_list
                .iter()
                .position(|w| w.upgrade().map(|rc| rc.as_ptr()) == Some(current_ptr));

            if let Some(index) = current_index {
                let next_index = match direction {
                    TabDirection::Next => (index + 1) % self.tab_list.len(),
                    TabDirection::Previous => {
                        if index == 0 {
                            self.tab_list.len() - 1
                        } else {
                            index - 1
                        }
                    }
                };

                next_window = self.tab_list[next_index].upgrade();
            }
        }

        // If no current focus or not in tab list, use first tab window
        if next_window.is_none() {
            if let Some(first) = self.tab_list.first() {
                next_window = first.upgrade();
            }
        }

        if let Some(window) = next_window {
            let _ = self.set_focus(Some(&window));
            self.set_lone_window(None);
        }
    }

    /// Register tab list
    pub fn register_tab_list(&mut self, windows: Vec<Rc<RefCell<GameWindow>>>) {
        self.tab_list = windows.into_iter().map(|w| Rc::downgrade(&w)).collect();
    }

    /// Clear tab list
    pub fn clear_tab_list(&mut self) {
        self.tab_list.clear();
    }

    /// Send system message to window
    pub fn send_system_message(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        window.borrow_mut().send_system_message(msg, data1, data2)
    }

    /// Send input message to window
    pub fn send_input_message(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        msg: WindowMessage,
        data1: WindowMsgData,
        data2: WindowMsgData,
    ) -> WindowMsgHandled {
        window.borrow_mut().send_input_message(msg, data1, data2)
    }

    /// Hide windows in ID range
    pub fn hide_windows_in_range(
        &mut self,
        base_window: &Rc<RefCell<GameWindow>>,
        first: WindowId,
        last: WindowId,
        hide: bool,
    ) {
        self.apply_to_window_range(base_window, first, last, |window| {
            let _ = window.borrow_mut().hide(hide);
        });
    }

    /// Enable windows in ID range
    pub fn enable_windows_in_range(
        &mut self,
        base_window: &Rc<RefCell<GameWindow>>,
        first: WindowId,
        last: WindowId,
        enable: bool,
    ) {
        self.apply_to_window_range(base_window, first, last, |window| {
            let _ = window.borrow_mut().enable(enable);
        });
    }

    /// Draw all windows
    pub fn draw_all(&mut self) {
        // Match C++ WinRepaint ordering: top-level windows are stored head-first,
        // but repaint walks from tail to head in BELOW / normal / ABOVE passes.
        for window in self.root_windows.iter().rev() {
            let status = window.borrow().get_status();
            if status.contains(WindowStatus::BELOW) {
                self.draw_window_hierarchy(window);
            }
        }

        for window in self.root_windows.iter().rev() {
            let status = window.borrow().get_status();
            if !status.intersects(WindowStatus::ABOVE | WindowStatus::BELOW) {
                self.draw_window_hierarchy(window);
            }
        }

        for window in self.root_windows.iter().rev() {
            let status = window.borrow().get_status();
            if status.contains(WindowStatus::ABOVE) {
                self.draw_window_hierarchy(window);
            }
        }

        // Draw modal windows on top
        if let Some(modal) = &self.modal_stack {
            self.draw_window_hierarchy(&modal.window);
        }
        self.transitions.draw();
    }

    /// Activate a transition group.
    pub fn transition_set_group(&mut self, group_name: &str, immediate: bool) {
        let window_lookup = self.window_by_id.clone();
        self.transitions
            .set_group(group_name, immediate, &window_lookup);
    }

    /// Reverse a transition group.
    pub fn transition_reverse(&mut self, group_name: &str) {
        let window_lookup = self.window_by_id.clone();
        self.transitions.reverse(group_name, &window_lookup);
    }

    /// Remove a transition group.
    pub fn transition_remove(&mut self, group_name: &str, skip_pending: bool) {
        self.transitions.remove(group_name, skip_pending);
    }

    /// Check if the current transition group has finished.
    pub fn transitions_finished(&self) -> bool {
        self.transitions.is_finished()
    }

    /// Create a window layout
    pub fn create_layout(&mut self, filename: String) -> Rc<RefCell<WindowLayout>> {
        let layout = Rc::new(RefCell::new(WindowLayout::new(filename)));
        self.layouts.push(layout.clone());
        layout
    }

    /// Create a window layout, populate it from script, and return the layout with info.
    pub fn create_layout_with_windows(
        &mut self,
        filename: &str,
    ) -> WindowResult<(Rc<RefCell<WindowLayout>>, WindowLayoutInfo)> {
        let path = resolve_window_script_path(filename)?;
        let layout_def = parse_window_script(&path).map_err(|err| {
            log::error!(
                "Failed to parse window script '{}': {:?}",
                path.display(),
                err
            );
            WindowError::GeneralFailure
        })?;

        let layout = self.create_layout(filename.to_string());
        {
            let mut layout_mut = layout.borrow_mut();
            layout_mut.default_text_color = layout_def.default_text_color;
            layout_mut.default_font = layout_def.default_font.clone();
            self.bind_layout_callbacks(&mut layout_mut, &layout_def);
        }

        let mut info = WindowLayoutInfo {
            version: layout_def.version,
            init_callback_name: layout_def.init_callback.clone(),
            update_callback_name: layout_def.update_callback.clone(),
            shutdown_callback_name: layout_def.shutdown_callback.clone(),
            windows: Vec::new(),
        };

        for window_def in &layout_def.windows {
            self.create_window_from_definition(&window_def, None, &layout, &layout_def, &mut info)?;
        }

        Ok((layout, info))
    }

    /// Remove a layout after destroying its windows.
    pub fn destroy_layout(&mut self, layout: &Rc<RefCell<WindowLayout>>) {
        layout.borrow_mut().destroy_windows();
        self.layouts.retain(|entry| !Rc::ptr_eq(entry, layout));
        self.flush_destroy_queue();
    }

    /// Load windows from script and create window instances.
    pub fn create_windows_from_script(&mut self, filename: &str) -> WindowResult<WindowLayoutInfo> {
        let path = resolve_window_script_path(filename)?;
        let layout_def = parse_window_script(&path).map_err(|err| WindowError::GeneralFailure)?;

        let layout = self.create_layout(filename.to_string());
        {
            let mut layout_mut = layout.borrow_mut();
            layout_mut.default_text_color = layout_def.default_text_color;
            layout_mut.default_font = layout_def.default_font.clone();
            self.bind_layout_callbacks(&mut layout_mut, &layout_def);
        }
        let mut info = WindowLayoutInfo {
            version: layout_def.version,
            init_callback_name: layout_def.init_callback.clone(),
            update_callback_name: layout_def.update_callback.clone(),
            shutdown_callback_name: layout_def.shutdown_callback.clone(),
            windows: Vec::new(),
        };

        for window_def in &layout_def.windows {
            self.create_window_from_definition(&window_def, None, &layout, &layout_def, &mut info)?;
        }

        Ok(info)
    }

    fn bind_layout_callbacks(
        &self,
        layout: &mut WindowLayout,
        layout_def: &WindowLayoutDefinition,
    ) {
        if !layout_def.init_callback.is_empty() {
            layout.init_callback = match layout_def.init_callback.as_str() {
                "W3DMainMenuInit" | "MainMenuInit" => Some(Box::new(|layout, _| {
                    apply_w3d_main_menu_runtime_draw_overrides();
                    let mut menu = get_main_menu();
                    if let Err(err) = menu.init(layout, None) {
                        warn!("Main menu init failed: {}", err);
                    }
                })),
                "SinglePlayerMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_single_player_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("Single player menu init failed: {}", err);
                    }
                })),
                "OptionsMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_options_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("Options menu init failed: {}", err);
                    }
                })),
                "MapSelectMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_map_select_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("Map select menu init failed: {}", err);
                    }
                })),
                "CreditsMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_credits_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("Credits menu init failed: {}", err);
                    }
                })),
                "LanLobbyMenuInit" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_lan_lobby_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.init(layout, None)) {
                        warn!("LAN lobby menu init failed: {}", err);
                    }
                })),
                "InGamePopupMessageInit" => Some(Box::new(|layout, _| {
                    in_game_popup_message_init(layout, None);
                })),
                "PopupCommunicatorInit" => Some(Box::new(|layout, _| {
                    popup_communicator_init(layout, None);
                })),
                "PopupJoinGameInit" => Some(Box::new(|layout, _| {
                    popup_join_game_init(layout, None);
                })),
                "SaveLoadMenuInit" => Some(Box::new(|layout, data| {
                    save_load_menu_init(layout, data);
                })),
                "SaveLoadMenuFullScreenInit" => Some(Box::new(|layout, data| {
                    save_load_menu_full_screen_init(layout, data);
                })),
                "PopupReplayInit" => Some(Box::new(|layout, data| {
                    popup_replay_init(layout, data);
                })),
                "ReplayMenuInit" => Some(Box::new(|layout, data| {
                    replay_menu_init(layout, data);
                })),
                "ChallengeMenuInit" => Some(Box::new(|layout, data| {
                    challenge_menu_init(layout, data);
                })),
                "DifficultySelectInit" => Some(Box::new(|layout, data| {
                    difficulty_select_init(layout, data);
                })),
                "KeyboardOptionsMenuInit" => Some(Box::new(|layout, data| {
                    keyboard_options_menu_init(layout, data);
                })),
                "GameSpyPlayerInfoOverlayInit" => Some(Box::new(|layout, data| {
                    popup_player_info_init(layout, data);
                })),
                "ScoreScreenInit" => Some(Box::new(|layout, data| {
                    score_screen_init(layout, None);
                })),
                "SkirmishMapSelectMenuInit" => Some(Box::new(|layout, data| {
                    skirmish_map_select_menu_init(layout, None);
                })),
                "SkirmishGameOptionsMenuInit" => Some(Box::new(|layout, data| {
                    skirmish_game_options_menu_init(layout, None);
                })),
                "LanMapSelectMenuInit" => Some(Box::new(|layout, data| {
                    lan_map_select_menu_init(layout, None);
                })),
                "LanGameOptionsMenuInit" => Some(Box::new(|layout, data| {
                    lan_game_options_menu_init(layout, None);
                })),
                "PopupHostGameInit" => Some(Box::new(|layout, data| {
                    popup_host_game_init(layout, data);
                })),
                "PopupLadderSelectInit" => Some(Box::new(|layout, data| {
                    popup_ladder_select_init(layout, data);
                })),
                "RCGameDetailsMenuInit" => Some(Box::new(|layout, data| {
                    rc_game_details_menu_init(layout, data);
                })),
                "DownloadMenuInit" => Some(Box::new(|layout, data| {
                    download_menu_init(layout, None);
                })),
                "GameInfoWindowInit" => Some(Box::new(|layout, data| {
                    game_info_window_init(layout, None);
                })),
                "NetworkDirectConnectInit" => Some(Box::new(|layout, data| {
                    network_direct_connect_init(layout, data);
                })),
                "WOLLoginMenuInit" => Some(Box::new(|layout, data| {
                    wol_login_menu_init(layout, data);
                })),
                "WOLLocaleSelectInit" => Some(Box::new(|layout, data| {
                    wol_locale_select_init(layout, data);
                })),
                "WOLMessageWindowInit" => Some(Box::new(|layout, data| {
                    wol_message_window_init(layout, data);
                })),
                "WOLBuddyOverlayInit" => Some(Box::new(|layout, data| {
                    wol_buddy_overlay_init(layout, data);
                })),
                "WOLBuddyOverlayRCMenuInit" => Some(Box::new(|layout, data| {
                    wol_buddy_overlay_rc_menu_init(layout, data);
                })),
                "WOLStatusMenuInit" => Some(Box::new(|layout, data| {
                    wol_status_menu_init(layout, data);
                })),
                "WOLWelcomeMenuInit" => Some(Box::new(|layout, data| {
                    wol_welcome_menu_init(layout, data);
                })),
                "WOLLobbyMenuInit" => Some(Box::new(|layout, data| {
                    wol_lobby_menu_init(layout, data);
                })),
                "WOLLadderScreenInit" => Some(Box::new(|layout, data| {
                    wol_ladder_screen_init(layout, data);
                })),
                "WOLMapSelectMenuInit" => Some(Box::new(|layout, data| {
                    wol_map_select_menu_init(layout, data);
                })),
                "WOLGameSetupMenuInit" => Some(Box::new(|layout, data| {
                    wol_game_setup_menu_init(layout, data);
                })),
                "WOLQuickMatchMenuInit" => Some(Box::new(|layout, data| {
                    wol_quick_match_menu_init(layout, data);
                })),
                "WOLQMScoreScreenInit" => Some(Box::new(|layout, data| {
                    wol_qm_score_screen_init(layout, data);
                })),
                "WOLCustomScoreScreenInit" => Some(Box::new(|layout, data| {
                    wol_custom_score_screen_init(layout, data);
                })),
                "MarketingScreenInit" => Some(Box::new(|_, _| {})),
                other => {
                    warn!("Unknown layout init callback: {}", other);
                    None
                }
            };
        }

        if !layout_def.update_callback.is_empty() {
            layout.update_callback = match layout_def.update_callback.as_str() {
                "MainMenuUpdate" => Some(Box::new(|layout, _| {
                    let mut menu = get_main_menu();
                    if let Err(err) = menu.update(layout, None) {
                        warn!("Main menu update failed: {}", err);
                    }
                })),
                "SinglePlayerMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_single_player_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("Single player menu update failed: {}", err);
                    }
                })),
                "OptionsMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_options_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("Options menu update failed: {}", err);
                    }
                })),
                "MapSelectMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_map_select_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("Map select menu update failed: {}", err);
                    }
                })),
                "CreditsMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_credits_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("Credits menu update failed: {}", err);
                    }
                })),
                "LanLobbyMenuUpdate" => Some(Box::new(|layout, _| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_lan_lobby_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.update(layout, None)) {
                        warn!("LAN lobby menu update failed: {}", err);
                    }
                })),
                "PopupCommunicatorUpdate" => Some(Box::new(|layout, _| {
                    popup_communicator_update(layout, None);
                })),
                "SaveLoadMenuUpdate" => Some(Box::new(|layout, data| {
                    save_load_menu_update(layout, data);
                })),
                "PopupReplayUpdate" => Some(Box::new(|layout, data| {
                    popup_replay_update(layout, data);
                })),
                "ReplayMenuUpdate" => Some(Box::new(|layout, data| {
                    replay_menu_update(layout, data);
                })),
                "ChallengeMenuUpdate" => Some(Box::new(|layout, data| {
                    challenge_menu_update(layout, data);
                })),
                "KeyboardOptionsMenuUpdate" => Some(Box::new(|layout, data| {
                    keyboard_options_menu_update(layout, data);
                })),
                "GameSpyPlayerInfoOverlayUpdate" => Some(Box::new(|layout, data| {
                    popup_player_info_update(layout, data);
                })),
                "PopupHostGameUpdate" => Some(Box::new(|layout, data| {
                    popup_host_game_update(layout, data);
                })),
                "PopupLadderSelectUpdate" => Some(Box::new(|layout, data| {
                    popup_ladder_select_update(layout, data);
                })),
                "DownloadMenuUpdate" => Some(Box::new(|layout, data| {
                    download_menu_update(layout, None);
                })),
                "ScoreScreenUpdate" => Some(Box::new(|layout, data| {
                    score_screen_update(layout, None);
                })),
                "SkirmishMapSelectMenuUpdate" => Some(Box::new(|layout, data| {
                    skirmish_map_select_menu_update(layout, None);
                })),
                "SkirmishGameOptionsMenuUpdate" => Some(Box::new(|layout, data| {
                    skirmish_game_options_menu_update(layout, None);
                })),
                "LanMapSelectMenuUpdate" => Some(Box::new(|layout, data| {
                    lan_map_select_menu_update(layout, None);
                })),
                "LanGameOptionsMenuUpdate" => Some(Box::new(|layout, data| {
                    lan_game_options_menu_update(layout, None);
                })),
                "NetworkDirectConnectUpdate" => Some(Box::new(|layout, data| {
                    network_direct_connect_update(layout, data);
                })),
                "WOLLoginMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_login_menu_update(layout, data);
                })),
                "WOLLocaleSelectUpdate" => Some(Box::new(|layout, data| {
                    wol_locale_select_update(layout, data);
                })),
                "WOLMessageWindowUpdate" => Some(Box::new(|layout, data| {
                    wol_message_window_update(layout, data);
                })),
                "WOLBuddyOverlayUpdate" => Some(Box::new(|layout, data| {
                    wol_buddy_overlay_update(layout, data);
                })),
                "WOLStatusMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_status_menu_update(layout, data);
                })),
                "WOLWelcomeMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_welcome_menu_update(layout, data);
                })),
                "WOLLobbyMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_lobby_menu_update(layout, data);
                })),
                "WOLLadderScreenUpdate" => Some(Box::new(|layout, data| {
                    wol_ladder_screen_update(layout, data);
                })),
                "WOLMapSelectMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_map_select_menu_update(layout, data);
                })),
                "WOLGameSetupMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_game_setup_menu_update(layout, data);
                })),
                "WOLQuickMatchMenuUpdate" => Some(Box::new(|layout, data| {
                    wol_quick_match_menu_update(layout, data);
                })),
                "WOLQMScoreScreenUpdate" => Some(Box::new(|layout, data| {
                    wol_qm_score_screen_update(layout, data);
                })),
                "WOLCustomScoreScreenUpdate" => Some(Box::new(|layout, data| {
                    wol_custom_score_screen_update(layout, data);
                })),
                "MarketingScreenUpdate" => Some(Box::new(|_, _| {})),
                other => {
                    warn!("Unknown layout update callback: {}", other);
                    None
                }
            };
        }

        if !layout_def.shutdown_callback.is_empty() {
            layout.shutdown_callback = match layout_def.shutdown_callback.as_str() {
                "MainMenuShutdown" => Some(Box::new(|layout, data| {
                    let mut menu = get_main_menu();
                    if let Err(err) = menu.shutdown(layout, as_any_ref(data)) {
                        warn!("Main menu shutdown failed: {}", err);
                    }
                })),
                "SinglePlayerMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_single_player_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("Single player menu shutdown failed: {}", err);
                    }
                })),
                "OptionsMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_options_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("Options menu shutdown failed: {}", err);
                    }
                })),
                "MapSelectMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_map_select_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("Map select menu shutdown failed: {}", err);
                    }
                })),
                "CreditsMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_credits_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("Credits menu shutdown failed: {}", err);
                    }
                })),
                "LanLobbyMenuShutdown" => Some(Box::new(|layout, data| {
                    let manager = get_menu_manager();
                    let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                    let menu = manager.get_lan_lobby_menu();
                    if let Err(err) = with_arc_write(&menu, |menu| menu.shutdown(layout, None)) {
                        warn!("LAN lobby menu shutdown failed: {}", err);
                    }
                })),
                "PopupCommunicatorShutdown" => Some(Box::new(|layout, data| {
                    popup_communicator_shutdown(layout, as_any_ref(data));
                })),
                "SaveLoadMenuShutdown" => Some(Box::new(|layout, data| {
                    save_load_menu_shutdown(layout, as_any_ref(data));
                })),
                "PopupReplayShutdown" => Some(Box::new(|layout, data| {
                    popup_replay_shutdown(layout, as_any_ref(data));
                })),
                "ReplayMenuShutdown" => Some(Box::new(|layout, data| {
                    replay_menu_shutdown(layout, as_any_ref(data));
                })),
                "ChallengeMenuShutdown" => Some(Box::new(|layout, data| {
                    challenge_menu_shutdown(layout, as_any_ref(data));
                })),
                "KeyboardOptionsMenuShutdown" => Some(Box::new(|layout, data| {
                    keyboard_options_menu_shutdown(layout, as_any_ref(data));
                })),
                "PopupLadderSelectShutdown" => Some(Box::new(|layout, data| {
                    popup_ladder_select_shutdown(layout, as_any_ref(data));
                })),
                "GameSpyPlayerInfoOverlayShutdown" => Some(Box::new(|layout, data| {
                    popup_player_info_shutdown(layout, as_any_ref(data));
                })),
                "DownloadMenuShutdown" => Some(Box::new(|layout, data| {
                    download_menu_shutdown(layout, data);
                })),
                "ScoreScreenShutdown" => Some(Box::new(|layout, data| {
                    score_screen_shutdown(layout, data);
                })),
                "SkirmishMapSelectMenuShutdown" => Some(Box::new(|layout, data| {
                    skirmish_map_select_menu_shutdown(layout, data);
                })),
                "SkirmishGameOptionsMenuShutdown" => Some(Box::new(|layout, data| {
                    skirmish_game_options_menu_shutdown(layout, data);
                })),
                "LanMapSelectMenuShutdown" => Some(Box::new(|layout, data| {
                    lan_map_select_menu_shutdown(layout, data);
                })),
                "LanGameOptionsMenuShutdown" => Some(Box::new(|layout, data| {
                    lan_game_options_menu_shutdown(layout, data);
                })),
                "NetworkDirectConnectShutdown" => Some(Box::new(|layout, data| {
                    network_direct_connect_shutdown(layout, as_any_ref(data));
                })),
                "WOLLoginMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_login_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLLocaleSelectShutdown" => Some(Box::new(|layout, data| {
                    wol_locale_select_shutdown(layout, as_any_ref(data));
                })),
                "WOLMessageWindowShutdown" => Some(Box::new(|layout, data| {
                    wol_message_window_shutdown(layout, as_any_ref(data));
                })),
                "WOLBuddyOverlayShutdown" => Some(Box::new(|layout, data| {
                    wol_buddy_overlay_shutdown(layout, as_any_ref(data));
                })),
                "WOLStatusMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_status_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLWelcomeMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_welcome_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLLobbyMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_lobby_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLLadderScreenShutdown" => Some(Box::new(|layout, data| {
                    wol_ladder_screen_shutdown(layout, as_any_ref(data));
                })),
                "WOLMapSelectMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_map_select_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLGameSetupMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_game_setup_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLQuickMatchMenuShutdown" => Some(Box::new(|layout, data| {
                    wol_quick_match_menu_shutdown(layout, as_any_ref(data));
                })),
                "WOLQMScoreScreenShutdown" => Some(Box::new(|layout, data| {
                    wol_qm_score_screen_shutdown(layout, as_any_ref(data));
                })),
                "WOLCustomScoreScreenShutdown" => Some(Box::new(|layout, data| {
                    wol_custom_score_screen_shutdown(layout, as_any_ref(data));
                })),
                "ChallengeLoadScreenShutdown"
                | "MarketingScreenShutdown"
                | "SinglePlayerLoadScreenShutdown" => Some(Box::new(|_, _| {})),
                other => {
                    warn!("Unknown layout shutdown callback: {}", other);
                    None
                }
            };
        }
    }

    fn bind_window_callbacks(&self, window: &mut GameWindow, window_def: &WindowDefinition) {
        if !window_def.system_callback.is_empty() {
            let name = window_def.system_callback.as_str();
            match name {
                "GameWinDefaultSystem" => {
                    window.set_system_callback(default_system_callback);
                }
                "GadgetCheckBoxSystem"
                | "GadgetComboBoxSystem"
                | "GadgetHorizontalSliderSystem"
                | "GadgetListBoxSystem"
                | "GadgetProgressBarSystem"
                | "GadgetPushButtonSystem"
                | "GadgetRadioButtonSystem"
                | "GadgetStaticTextSystem"
                | "GadgetTabControlSystem"
                | "GadgetTextEntrySystem"
                | "GadgetVerticalSliderSystem"
                | "MOTDSystem" => {
                    window.set_system_callback(default_system_callback);
                }
                "ControlBarSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_control_bar_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_callbacks();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.system(window, msg, data1, data2)
                        })
                    });
                }
                "ControlBarObserverSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_control_bar_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_observer();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.system(window, msg, data1, data2)
                        })
                    });
                }
                "DiplomacySystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_diplomacy_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_callbacks();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.system(window, msg, data1, data2)
                        })
                    });
                }
                "InGameChatSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let chat = system.get_chat();
                        with_arc_write(&chat, |chat| chat.system(window, msg, data1, data2))
                    });
                }
                "ReplayControlSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let replay = system.get_replay();
                        with_arc_write(&replay, |replay| replay.system(window, msg, data1, data2))
                    });
                }
                "IdleWorkerSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let idle_worker = system.get_idle_worker();
                        with_arc_write(&idle_worker, |idle_worker| {
                            idle_worker.system(window, msg, data1, data2)
                        })
                    });
                }
                "MessageBoxSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_message_box_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let standard = system.get_standard();
                        with_arc_write(&standard, |standard| {
                            standard.system(window, msg, data1, data2)
                        })
                    });
                }
                "ExtendedMessageBoxSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_message_box_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let extended = system.get_extended();
                        with_arc_write(&extended, |extended| {
                            extended.system(window, msg, data1, data2)
                        })
                    });
                }
                "EstablishConnectionsControlSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let menu = get_establish_connections_menu();
                        let mut menu = menu.write().unwrap_or_else(|e| e.into_inner());
                        menu.system(window, msg, data1, data2)
                    });
                }
                "DisconnectControlSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let menu = get_disconnect_menu();
                        let mut menu = menu.write().unwrap_or_else(|e| e.into_inner());
                        menu.system(window, msg, data1, data2)
                    });
                }
                "MainMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let mut menu = get_main_menu();
                        let raw_msg = map_window_message_to_main_menu(msg);
                        if raw_msg == 0 {
                            return WindowMsgHandled::Ignored;
                        }
                        if menu.system(window.get_id() as u32, raw_msg, data1, data2) {
                            WindowMsgHandled::Handled
                        } else {
                            WindowMsgHandled::Ignored
                        }
                    });
                }
                "SinglePlayerMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_single_player_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "OptionsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_options_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "MapSelectMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_map_select_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "CreditsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_credits_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "LanLobbyMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_lan_lobby_menu();
                        with_arc_write(&menu, |menu| menu.system(window, msg, data1, data2))
                    });
                }
                "QuitMessageBoxSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        let system = get_message_box_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let quit = system.get_quit();
                        with_arc_write(&quit, |quit| quit.system(window, msg, data1, data2))
                    });
                }
                "GeneralsExpPointsSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        generals_exp_points_system(window, msg, data1, data2)
                    });
                }
                "IMECandidateWindowSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        ime_candidate_window_system(window, msg, data1, data2)
                    });
                }
                "InGamePopupMessageSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        in_game_popup_message_system(window, msg, data1, data2)
                    });
                }
                "PopupCommunicatorSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_communicator_system(window, msg, data1, data2)
                    });
                }
                "PopupJoinGameSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_join_game_system(window, msg, data1, data2)
                    });
                }
                "PopupHostGameSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_host_game_system(window, msg, data1, data2)
                    });
                }
                "PopupLadderSelectSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_ladder_select_system(window, msg, data1, data2)
                    });
                }
                "RCGameDetailsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        rc_game_details_menu_system(window, msg, data1, data2)
                    });
                }
                "DownloadMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        download_menu_system(window, msg, data1, data2)
                    });
                }
                "QuitMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        quit_menu_system(window, msg, data1, data2)
                    });
                }
                "SaveLoadMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        save_load_menu_system(window, msg, data1, data2)
                    });
                }
                "PopupReplaySystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_replay_system(window, msg, data1, data2)
                    });
                }
                "ReplayMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        replay_menu_system(window, msg, data1, data2)
                    });
                }
                "ChallengeMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        challenge_menu_system(window, msg, data1, data2)
                    });
                }
                "DifficultySelectSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        difficulty_select_system(window, msg, data1, data2)
                    });
                }
                "KeyboardOptionsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        keyboard_options_menu_system(window, msg, data1, data2)
                    });
                }
                "GameSpyPlayerInfoOverlaySystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_player_info_system(window, msg, data1, data2)
                    });
                }
                "ScoreScreenSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        score_screen_system(window, msg, data1, data2)
                    });
                }
                "SkirmishMapSelectMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        skirmish_map_select_menu_system(window, msg, data1, data2)
                    });
                }
                "SkirmishGameOptionsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        skirmish_game_options_menu_system(window, msg, data1, data2)
                    });
                }
                "LanMapSelectMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        lan_map_select_menu_system(window, msg, data1, data2)
                    });
                }
                "LanGameOptionsMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        lan_game_options_menu_system(window, msg, data1, data2)
                    });
                }
                "GameInfoWindowSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        game_info_window_system(window, msg, data1, data2)
                    });
                }
                "NetworkDirectConnectSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        network_direct_connect_system(window, msg, data1, data2)
                    });
                }
                "WOLLoginMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_login_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLLocaleSelectSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_locale_select_system(window, msg, data1, data2)
                    });
                }
                "WOLMessageWindowSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_message_window_system(window, msg, data1, data2)
                    });
                }
                "WOLBuddyOverlaySystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_buddy_overlay_system(window, msg, data1, data2)
                    });
                }
                "WOLBuddyOverlayRCMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_buddy_overlay_rc_menu_system(window, msg, data1, data2)
                    });
                }
                "PopupBuddyNotificationSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        popup_buddy_notification_system(window, msg, data1, data2)
                    });
                }
                "WOLStatusMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_status_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLWelcomeMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_welcome_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLLobbyMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_lobby_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLLadderScreenSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_ladder_screen_system(window, msg, data1, data2)
                    });
                }
                "WOLMapSelectMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_map_select_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLGameSetupMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_game_setup_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLQuickMatchMenuSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_quick_match_menu_system(window, msg, data1, data2)
                    });
                }
                "WOLQMScoreScreenSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_qm_score_screen_system(window, msg, data1, data2)
                    });
                }
                "WOLCustomScoreScreenSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        wol_custom_score_screen_system(window, msg, data1, data2)
                    });
                }
                "PassMessagesToParentSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        if msg == WindowMessage::Create
                            || msg == WindowMessage::Destroy
                            || msg == WindowMessage::ScriptCreate
                        {
                            return WindowMsgHandled::Ignored;
                        }

                        if let Some(parent) = window.get_parent() {
                            if let Ok(mut parent_ref) = parent.try_borrow_mut() {
                                parent_ref.send_system_message(msg, data1, data2)
                            } else {
                                let ptr = parent.as_ptr();
                                // SAFETY: mirrors legacy re-entrant parent dispatch in the
                                // single-threaded UI message pump.
                                let parent_ref = unsafe { &mut *ptr };
                                parent_ref.send_system_message(msg, data1, data2)
                            }
                        } else {
                            WindowMsgHandled::Ignored
                        }
                    });
                }
                "PassSelectedButtonsToParentSystem" => {
                    window.set_system_callback(|window, msg, data1, data2| {
                        if msg != WindowMessage::GadgetSelected
                            && msg != WindowMessage::GadgetRightClick
                            && msg != WindowMessage::GadgetMouseEntering
                            && msg != WindowMessage::GadgetMouseLeaving
                            && msg != WindowMessage::GadgetEditDone
                        {
                            return WindowMsgHandled::Ignored;
                        }

                        if let Some(parent) = window.get_parent() {
                            if let Ok(mut parent_ref) = parent.try_borrow_mut() {
                                parent_ref.send_system_message(msg, data1, data2)
                            } else {
                                let ptr = parent.as_ptr();
                                // SAFETY: mirrors legacy re-entrant parent dispatch in the
                                // single-threaded UI message pump.
                                let parent_ref = unsafe { &mut *ptr };
                                parent_ref.send_system_message(msg, data1, data2)
                            }
                        } else {
                            WindowMsgHandled::Ignored
                        }
                    });
                }
                other => {
                    warn!("Unimplemented system callback '{}', using default.", other);
                    window.set_system_callback(default_system_callback);
                }
            }
        }

        if !window_def.input_callback.is_empty() {
            let name = window_def.input_callback.as_str();
            match name {
                "GameWinDefaultInput" => {
                    window.set_input_callback(default_input_callback);
                }
                "BeaconWindowInput" => {
                    window.set_input_callback(beacon_window_input);
                }
                "DisconnectControlInput"
                | "EstablishConnectionsControlInput"
                | "GadgetCheckBoxInput"
                | "GadgetComboBoxInput"
                | "GadgetHorizontalSliderInput"
                | "GadgetListBoxInput"
                | "GadgetListBoxMultiInput"
                | "GadgetPushButtonInput"
                | "GadgetRadioButtonInput"
                | "GadgetStaticTextInput"
                | "GadgetTabControlInput"
                | "GadgetTextEntryInput"
                | "GadgetVerticalSliderInput" => {
                    window.set_input_callback(default_input_callback);
                }
                "GameWinBlockInput" => {
                    window.set_input_callback(|_window, msg, _data1, _data2| match msg {
                        WindowMessage::Char | WindowMessage::MousePos => WindowMsgHandled::Ignored,
                        _ => WindowMsgHandled::Handled,
                    });
                }
                "ControlBarInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_control_bar_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_callbacks();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.system(window, msg, data1, data2)
                        })
                    });
                }
                "LeftHUDInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_control_bar_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_left_hud();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.input(window, msg, data1, data2)
                        })
                    });
                }
                "DiplomacyInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_diplomacy_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let callbacks = system.get_callbacks();
                        with_arc_write(&callbacks, |callbacks| {
                            callbacks.input(window, msg, data1, data2)
                        })
                    });
                }
                "InGameChatInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let chat = system.get_chat();
                        with_arc_write(&chat, |chat| chat.input(window, msg, data1, data2))
                    });
                }
                "ReplayControlInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let system = get_ingame_ui_system();
                        let system = system.read().unwrap_or_else(|e| e.into_inner());
                        let replay = system.get_replay();
                        with_arc_write(&replay, |replay| replay.input(window, msg, data1, data2))
                    });
                }
                "MainMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let mut menu = get_main_menu();
                        let raw_msg = map_window_message_to_main_menu(msg);
                        if raw_msg == 0 {
                            return WindowMsgHandled::Ignored;
                        }
                        if menu.input(window.get_id() as u32, raw_msg, data1, data2) {
                            WindowMsgHandled::Handled
                        } else {
                            WindowMsgHandled::Ignored
                        }
                    });
                }
                "SinglePlayerMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_single_player_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "OptionsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_options_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "MapSelectMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_map_select_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "CreditsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_credits_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "LanLobbyMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        let manager = get_menu_manager();
                        let manager = manager.read().unwrap_or_else(|e| e.into_inner());
                        let menu = manager.get_lan_lobby_menu();
                        with_arc_write(&menu, |menu| menu.input(window, msg, data1, data2))
                    });
                }
                "GeneralsExpPointsInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        generals_exp_points_input(window, msg, data1, data2)
                    });
                }
                "IMECandidateWindowInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        ime_candidate_window_input(window, msg, data1, data2)
                    });
                }
                "InGamePopupMessageInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        in_game_popup_message_input(window, msg, data1, data2)
                    });
                }
                "PopupCommunicatorInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_communicator_input(window, msg, data1, data2)
                    });
                }
                "PopupJoinGameInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_join_game_input(window, msg, data1, data2)
                    });
                }
                "PopupHostGameInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_host_game_input(window, msg, data1, data2)
                    });
                }
                "PopupLadderSelectInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_ladder_select_input(window, msg, data1, data2)
                    });
                }
                "DownloadMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        download_menu_input(window, msg, data1, data2)
                    });
                }
                "SaveLoadMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        save_load_menu_input(window, msg, data1, data2)
                    });
                }
                "PopupReplayInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_replay_input(window, msg, data1, data2)
                    });
                }
                "ReplayMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        replay_menu_input(window, msg, data1, data2)
                    });
                }
                "ChallengeMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        challenge_menu_input(window, msg, data1, data2)
                    });
                }
                "DifficultySelectInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        difficulty_select_input(window, msg, data1, data2)
                    });
                }
                "KeyboardOptionsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        keyboard_options_menu_input(window, msg, data1, data2)
                    });
                }
                "GameSpyPlayerInfoOverlayInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        popup_player_info_input(window, msg, data1, data2)
                    });
                }
                "ScoreScreenInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        score_screen_input(window, msg, data1, data2)
                    });
                }
                "SkirmishMapSelectMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        skirmish_map_select_menu_input(window, msg, data1, data2)
                    });
                }
                "SkirmishGameOptionsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        skirmish_game_options_menu_input(window, msg, data1, data2)
                    });
                }
                "LanMapSelectMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        lan_map_select_menu_input(window, msg, data1, data2)
                    });
                }
                "LanGameOptionsMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        lan_game_options_menu_input(window, msg, data1, data2)
                    });
                }
                "NetworkDirectConnectInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        network_direct_connect_input(window, msg, data1, data2)
                    });
                }
                "WOLLoginMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_login_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLLocaleSelectInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_locale_select_input(window, msg, data1, data2)
                    });
                }
                "WOLMessageWindowInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_message_window_input(window, msg, data1, data2)
                    });
                }
                "WOLBuddyOverlayInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_buddy_overlay_input(window, msg, data1, data2)
                    });
                }
                "WOLStatusMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_status_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLWelcomeMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_welcome_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLLobbyMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_lobby_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLLadderScreenInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_ladder_screen_input(window, msg, data1, data2)
                    });
                }
                "WOLMapSelectMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_map_select_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLGameSetupMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_game_setup_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLQuickMatchMenuInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_quick_match_menu_input(window, msg, data1, data2)
                    });
                }
                "WOLQMScoreScreenInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_qm_score_screen_input(window, msg, data1, data2)
                    });
                }
                "WOLCustomScoreScreenInput" => {
                    window.set_input_callback(|window, msg, data1, data2| {
                        wol_custom_score_screen_input(window, msg, data1, data2)
                    });
                }
                other => {
                    warn!("Unimplemented input callback '{}', using default.", other);
                    window.set_input_callback(default_input_callback);
                }
            }
        }

        if !window_def.tooltip_callback.is_empty() {
            let name = window_def.tooltip_callback.as_str();
            match name {
                "GameWinDefaultTooltip" => {
                    window.set_tooltip_callback(default_tooltip_callback);
                }
                other => {
                    warn!("Unimplemented tooltip callback '{}', using default.", other);
                    window.set_tooltip_callback(default_tooltip_callback);
                }
            }
        }

        if !window_def.draw_callback.is_empty() {
            let name = window_def.draw_callback.as_str();
            match name {
                "GameWinDefaultDraw" => {
                    window.set_draw_callback(default_draw_callback);
                }
                "W3DGameWinDefaultDraw" => {
                    window.set_draw_callback(default_draw_callback);
                }
                "W3DGadgetPushButtonDraw" => {
                    window.set_draw_callback(w3d_gadget_push_button_draw);
                }
                "W3DGadgetPushButtonImageDraw" => {
                    window.set_draw_callback(w3d_gadget_push_button_image_draw);
                }
                "W3DGadgetStaticTextDraw" => {
                    window.set_draw_callback(w3d_gadget_static_text_draw);
                }
                "W3DGadgetStaticTextImageDraw" => {
                    window.set_draw_callback(w3d_gadget_static_text_image_draw);
                }
                "W3DGadgetProgressBarDraw" => {
                    window.set_draw_callback(w3d_gadget_progress_bar_draw);
                }
                "W3DGadgetProgressBarImageDraw" => {
                    window.set_draw_callback(w3d_gadget_progress_bar_image_draw);
                }
                "W3DGadgetProgressBarImageDrawA" => {
                    window.set_draw_callback(w3d_gadget_progress_bar_image_draw_a);
                }
                "W3DGadgetCheckBoxDraw" => {
                    window.set_draw_callback(w3d_gadget_check_box_draw);
                }
                "W3DGadgetCheckBoxImageDraw" => {
                    window.set_draw_callback(w3d_gadget_check_box_image_draw);
                }
                "W3DGadgetRadioButtonDraw" => {
                    window.set_draw_callback(w3d_gadget_radio_button_draw);
                }
                "W3DGadgetRadioButtonImageDraw" => {
                    window.set_draw_callback(w3d_gadget_radio_button_image_draw);
                }
                "W3DGadgetHorizontalSliderDraw" => {
                    window.set_draw_callback(w3d_gadget_horizontal_slider_draw);
                }
                "W3DGadgetHorizontalSliderImageDraw" => {
                    window.set_draw_callback(w3d_gadget_horizontal_slider_image_draw);
                }
                "W3DGadgetHorizontalSliderImageDrawA" => {
                    window.set_draw_callback(w3d_gadget_horizontal_slider_image_draw_a);
                }
                "W3DGadgetHorizontalSliderImageDrawB" => {
                    window.set_draw_callback(w3d_gadget_horizontal_slider_image_draw_b);
                }
                "W3DGadgetVerticalSliderDraw" => {
                    window.set_draw_callback(w3d_gadget_vertical_slider_draw);
                }
                "W3DGadgetVerticalSliderImageDraw" => {
                    window.set_draw_callback(w3d_gadget_vertical_slider_image_draw);
                }
                "W3DGadgetTextEntryDraw" => {
                    window.set_draw_callback(w3d_gadget_text_entry_draw);
                }
                "W3DGadgetTextEntryImageDraw" => {
                    window.set_draw_callback(w3d_gadget_text_entry_image_draw);
                }
                "W3DGadgetListBoxDraw" => {
                    window.set_draw_callback(w3d_gadget_list_box_draw);
                }
                "W3DGadgetListBoxImageDraw" => {
                    window.set_draw_callback(w3d_gadget_list_box_image_draw);
                }
                "W3DGadgetTabControlDraw" => {
                    window.set_draw_callback(w3d_gadget_tab_control_draw);
                }
                "W3DGadgetTabControlImageDraw" => {
                    window.set_draw_callback(w3d_gadget_tab_control_image_draw);
                }
                "W3DGadgetComboBoxDraw" => {
                    window.set_draw_callback(w3d_gadget_combo_box_draw);
                }
                "W3DGadgetComboBoxImageDraw" => {
                    window.set_draw_callback(w3d_gadget_combo_box_image_draw);
                }
                "W3DMainMenuDraw" => {
                    window.set_draw_callback(w3d_main_menu_draw);
                }
                "W3DMainMenuFourDraw" => {
                    window.set_draw_callback(w3d_main_menu_four_draw);
                }
                "W3DMetalBarMenuDraw" => {
                    window.set_draw_callback(w3d_metal_bar_menu_draw);
                }
                "W3DCreditsMenuDraw" => {
                    window.set_draw_callback(w3d_credits_menu_draw);
                }
                "W3DShellMenuSchemeDraw" => {
                    window.set_draw_callback(w3d_shell_menu_scheme_draw);
                }
                "W3DClockDraw" => {
                    window.set_draw_callback(w3d_clock_draw);
                }
                "W3DMainMenuMapBorder" => {
                    window.set_draw_callback(w3d_main_menu_map_border);
                }
                "W3DMainMenuButtonDropShadowDraw" => {
                    window.set_draw_callback(w3d_main_menu_button_drop_shadow_draw);
                }
                "W3DMainMenuRandomTextDraw" => {
                    window.set_draw_callback(w3d_main_menu_random_text_draw);
                }
                "W3DThinBorderDraw" => {
                    window.set_draw_callback(w3d_thin_border_draw);
                }
                "W3DCameoMovieDraw" => {
                    window.set_draw_callback(w3d_cameo_movie_draw);
                }
                "W3DLeftHUDDraw" => {
                    window.set_draw_callback(w3d_left_hud_draw);
                }
                "W3DRightHUDDraw" => {
                    window.set_draw_callback(w3d_right_hud_draw);
                }
                "W3DPowerDraw" => {
                    window.set_draw_callback(w3d_power_draw);
                }
                "W3DPowerDrawA" => {
                    window.set_draw_callback(w3d_power_draw_a);
                }
                "W3DCommandBarTopDraw" => {
                    window.set_draw_callback(w3d_command_bar_top_draw);
                }
                "W3DCommandBarBackgroundDraw" => {
                    window.set_draw_callback(w3d_command_bar_background_draw);
                }
                "W3DCommandBarForegroundDraw" => {
                    window.set_draw_callback(w3d_command_bar_foreground_draw);
                }
                "W3DCommandBarGridDraw" => {
                    window.set_draw_callback(w3d_command_bar_grid_draw);
                }
                "W3DCommandBarGenExpDraw" => {
                    window.set_draw_callback(w3d_command_bar_gen_exp_draw);
                }
                "W3DCommandBarHelpPopupDraw" => {
                    window.set_draw_callback(w3d_command_bar_help_popup_draw);
                }
                "W3DNoDraw" => {
                    window.set_draw_callback(w3d_no_draw);
                }
                "W3DDrawMapPreview" => {
                    window.set_draw_callback(w3d_draw_map_preview);
                }
                "IMECandidateMainDraw" => {
                    window.set_draw_callback(|window, inst| ime_candidate_main_draw(window, inst));
                }
                "IMECandidateTextAreaDraw" => {
                    window.set_draw_callback(|window, inst| {
                        ime_candidate_text_area_draw(window, inst)
                    });
                }
                other => {
                    warn!("Unimplemented draw callback '{}', using default.", other);
                    window.set_draw_callback(default_draw_callback);
                }
            }
        }
    }

    fn create_window_from_definition(
        &mut self,
        window_def: &WindowDefinition,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        layout: &Rc<RefCell<WindowLayout>>,
        layout_def: &WindowLayoutDefinition,
        info: &mut WindowLayoutInfo,
    ) -> WindowResult<Rc<RefCell<GameWindow>>> {
        let (x, y, width, height) = self.resolve_window_rect(window_def, parent);
        log::debug!(
            "Creating window '{}' type={:?} rect=({}, {}, {}, {}) parent={}",
            window_def.name,
            window_def.window_type,
            x,
            y,
            width,
            height,
            parent
                .map(|p| p.borrow().get_name().to_string())
                .unwrap_or_else(|| "<root>".to_string())
        );
        let window_id = if window_def.name.is_empty() {
            generate_window_id()
        } else {
            NameKeyGenerator::name_to_key(&window_def.name) as WindowId
        };
        let window = self
            .create_window_with_id_internal(parent, x, y, width, height, window_id, false)
            .map_err(|err| {
                log::error!(
                    "Failed to create window '{}' type={:?} rect=({}, {}, {}, {}): {:?}",
                    window_def.name,
                    window_def.window_type,
                    x,
                    y,
                    width,
                    height,
                    err
                );
                err
            })?;
        let has_tab_pane_child = window_def.children.iter().any(|child| {
            let style = child.style | style_for_window_type(&child.window_type);
            (style & GWS_TAB_PANE) != 0
        });
        {
            let mut window_mut = window.borrow_mut();
            window_mut.set_layout(Some(layout));
            let data = window_mut.instance_data_mut();
            data.style = window_def.style | style_for_window_type(&window_def.window_type);
            data.decorated_name = window_def.name.clone();
            data.text_label = window_def.text_label.clone();
            data.header_template = window_def.header_template.clone();
            data.tooltip_delay = window_def.tooltip_delay;
            data.text = window_def.text.clone();
            data.tooltip = window_def.tooltip.clone();
            data.enabled_text = window_def.enabled_text.clone();
            data.disabled_text = window_def.disabled_text.clone();
            data.hilite_text = window_def.hilite_text.clone();
            if data.enabled_text.color == 0
                && data.disabled_text.color == 0
                && data.hilite_text.color == 0
            {
                if let Some(default_color) = layout.borrow().default_text_color {
                    data.enabled_text.color = default_color;
                    data.enabled_text.border_color = default_color;
                    data.disabled_text.color = default_color;
                    data.disabled_text.border_color = default_color;
                    data.hilite_text.color = default_color;
                    data.hilite_text.border_color = default_color;
                }
            }
            if let Some(font) = window_def.font.clone() {
                data.font = Some(font);
            } else if let Some(default_font) = layout.borrow().default_font.clone() {
                data.font = Some(default_font);
            }
            if !data.header_template.is_empty() {
                if let Some(font) =
                    get_header_template_manager().get_font_from_template(&data.header_template)
                {
                    data.font = Some(font);
                }
            }
            for (idx, draw) in window_def.enabled_draw_data.iter().enumerate() {
                if idx < data.enabled_draw_data.len() {
                    data.enabled_draw_data[idx] = draw.clone();
                }
            }
            for (idx, draw) in window_def.disabled_draw_data.iter().enumerate() {
                if idx < data.disabled_draw_data.len() {
                    data.disabled_draw_data[idx] = draw.clone();
                }
            }
            for (idx, draw) in window_def.hilite_draw_data.iter().enumerate() {
                if idx < data.hilite_draw_data.len() {
                    data.hilite_draw_data[idx] = draw.clone();
                }
            }
            if let Some(parent_window) = parent {
                data.owner = Some(Rc::downgrade(parent_window));
            }
            if let Some(widget) = create_widget_for_style(
                &mut self.radio_groups,
                window_def,
                window_mut.get_id(),
                x,
                y,
                width,
                height,
            ) {
                window_mut.set_widget(widget);
            }
            apply_window_text(&mut window_mut, window_def);
            apply_window_tooltip(&mut window_mut, window_def);
            window_mut.set_status_exact(window_def.status);
            apply_window_status_to_widget(&mut window_mut);
            apply_window_widget_data(&mut window_mut, window_def);
            self.bind_window_callbacks(&mut window_mut, window_def);
            if window_def.draw_callback.is_empty()
                || window_def.draw_callback.eq_ignore_ascii_case("[none]")
            {
                self.apply_default_draw_callback(&mut window_mut);
            }
            let _ = window_mut.send_system_message(WindowMessage::Create, 0, 0);
            let _ = window_mut.send_system_message(WindowMessage::ScriptCreate, 0, 0);
        }

        layout.borrow_mut().add_window(window.clone());
        info.windows.push(window.clone());
        if window_def.status.contains(WindowStatus::TAB_STOP)
            || (window_def.style | style_for_window_type(&window_def.window_type)) & GWS_TAB_STOP
                != 0
        {
            self.tab_list.push(Rc::downgrade(&window));
        }

        for child_def in &window_def.children {
            self.create_window_from_definition(child_def, Some(&window), layout, layout_def, info)
                .map_err(|err| {
                    log::error!(
                        "Failed while creating child '{}' under '{}': {:?}",
                        child_def.name,
                        window_def.name,
                        err
                    );
                    err
                })?;
        }

        if (window.borrow().get_style() & GWS_TAB_CONTROL) != 0 {
            if !has_tab_pane_child {
                self.create_default_tab_panes(&window).map_err(|err| {
                    log::error!(
                        "Failed creating default tab panes for '{}': {:?}",
                        window_def.name,
                        err
                    );
                    err
                })?;
            }
            self.resize_tab_panes(&window);
            let active_index =
                if let Some(WindowWidget::TabControl(tab_control)) = window.borrow().widget() {
                    tab_control.active_tab_index()
                } else {
                    0
                };
            window.borrow_mut().show_tab_pane(active_index);
        }

        if (window.borrow().get_style() & GWS_ALL_SLIDER) != 0 {
            self.create_slider_thumb_child(&window, layout_def)
                .map_err(|err| {
                    log::error!(
                        "Failed creating slider thumb for '{}': {:?}",
                        window_def.name,
                        err
                    );
                    err
                })?;
        }

        if (window.borrow().get_style() & GWS_COMBO_BOX) != 0 {
            self.create_combo_box_children(&window, layout_def, window_def)
                .map_err(|err| {
                    log::error!(
                        "Failed creating combo-box children for '{}': {:?}",
                        window_def.name,
                        err
                    );
                    err
                })?;
        }

        if (window.borrow().get_style() & GWS_SCROLL_LISTBOX) != 0 {
            if let Some(listbox_data) = window_def.listbox_data.as_ref() {
                if listbox_data.scrollbar {
                    self.create_listbox_scrollbar_children(&window, layout_def)
                        .map_err(|err| {
                            log::error!(
                                "Failed creating listbox scrollbar children for '{}': {:?}",
                                window_def.name,
                                err
                            );
                            err
                        })?;
                }
            }
        }

        Ok(window)
    }

    fn resolve_window_rect(
        &self,
        window_def: &WindowDefinition,
        parent: Option<&Rc<RefCell<GameWindow>>>,
    ) -> (i32, i32, i32, i32) {
        if let Some((x1, y1, x2, y2)) = window_def.raw_screen_rect {
            let (screen_w, screen_h) = self.screen_size;
            let (create_w, create_h) = window_def
                .creation_resolution
                .unwrap_or((screen_w.max(1), screen_h.max(1)));
            let x_scale = screen_w as f32 / create_w.max(1) as f32;
            let y_scale = screen_h as f32 / create_h.max(1) as f32;
            let scaled_x1 = (x1 as f32 * x_scale).round() as i32;
            let scaled_y1 = (y1 as f32 * y_scale).round() as i32;
            let scaled_x2 = (x2 as f32 * x_scale).round() as i32;
            let scaled_y2 = (y2 as f32 * y_scale).round() as i32;
            let (mut rel_x, mut rel_y) = (scaled_x1, scaled_y1);
            if let Some(parent_window) = parent {
                let (parent_x, parent_y) = parent_window.borrow().get_screen_position();
                rel_x -= parent_x;
                rel_y -= parent_y;
            }
            let width = scaled_x2 - scaled_x1;
            let height = scaled_y2 - scaled_y1;
            return (rel_x, rel_y, width, height);
        }

        let (x, y) = window_def.position;
        let (width, height) = window_def.size;
        (x, y, width, height)
    }

    /// Check if window and all parents are enabled
    pub fn is_window_enabled(&self, window: &Rc<RefCell<GameWindow>>) -> bool {
        let mut current = Some(window.clone());
        while let Some(win) = current {
            let win_borrow = win.borrow();
            if !win_borrow.is_enabled() {
                return false;
            }
            current = win_borrow.get_parent();
        }
        true
    }

    fn create_default_tab_panes(&mut self, window: &Rc<RefCell<GameWindow>>) -> WindowResult<()> {
        let (pane_x, pane_y, pane_width, pane_height) = self.compute_tab_pane_rect(window);

        for pane_index in 0..super::gadgets::tabcontrol::NUM_TAB_PANES {
            let pane_id = generate_window_id();
            let pane = self.create_window_with_id_internal(
                Some(window),
                pane_x,
                pane_y,
                pane_width,
                pane_height,
                pane_id,
                false,
            )?;
            {
                let mut pane_mut = pane.borrow_mut();
                if let Some(layout) = window.borrow().get_layout() {
                    pane_mut.set_layout(Some(&layout));
                }
                let data = pane_mut.instance_data_mut();
                data.style |= GWS_TAB_PANE;
                data.decorated_name = format!("Pane {}", pane_index);
                pane_mut.set_widget(WindowWidget::TabPane);
                pane_mut.enable(window.borrow().is_enabled())?;
            }
        }

        Ok(())
    }

    fn resize_tab_panes(&self, window: &Rc<RefCell<GameWindow>>) {
        let (pane_x, pane_y, pane_width, pane_height) = self.compute_tab_pane_rect(window);
        let panes: Vec<Rc<RefCell<GameWindow>>> = window
            .borrow()
            .children()
            .iter()
            .filter(|child| {
                let child = child.borrow();
                (child.get_style() & GWS_TAB_PANE) != 0
            })
            .cloned()
            .collect();

        for pane in panes {
            let mut pane_mut = pane.borrow_mut();
            let _ = pane_mut.set_size(pane_width, pane_height);
            let _ = pane_mut.set_position(pane_x, pane_y);
        }
    }

    fn compute_tab_pane_rect(&self, window: &Rc<RefCell<GameWindow>>) -> (i32, i32, i32, i32) {
        let window_ref = window.borrow();
        let (win_width, win_height) = window_ref.get_size();
        let (win_width, win_height) = (win_width as i32, win_height as i32);
        let mut tab_edge = super::gadgets::tabcontrol::TP_TOP_SIDE;
        let mut tab_width = 0;
        let mut tab_height = 0;
        let mut pane_border = 0;

        if let Some(WindowWidget::TabControl(tab_control)) = window_ref.widget() {
            tab_edge = tab_control.tab_edge();
            tab_width = tab_control.tab_width_px();
            tab_height = tab_control.tab_height_px();
            pane_border = tab_control.pane_border();
        }

        let mut width = win_width - (2 * pane_border);
        let mut height = win_height - (2 * pane_border);

        if tab_edge == super::gadgets::tabcontrol::TP_TOP_SIDE
            || tab_edge == super::gadgets::tabcontrol::TP_BOTTOM_SIDE
        {
            height -= tab_height;
        }
        if tab_edge == super::gadgets::tabcontrol::TP_LEFT_SIDE
            || tab_edge == super::gadgets::tabcontrol::TP_RIGHT_SIDE
        {
            width -= tab_width;
        }

        let mut x = pane_border;
        let mut y = pane_border;
        if tab_edge == super::gadgets::tabcontrol::TP_LEFT_SIDE {
            x += tab_width;
        }
        if tab_edge == super::gadgets::tabcontrol::TP_TOP_SIDE {
            y += tab_height;
        }

        (x, y, width.max(0), height.max(0))
    }

    fn create_combo_box_children(
        &mut self,
        window: &Rc<RefCell<GameWindow>>,
        layout: &WindowLayoutDefinition,
        window_def: &WindowDefinition,
    ) -> WindowResult<()> {
        let (width, height) = window.borrow().get_size();
        let mut status = window.borrow().get_status();
        status.remove(WindowStatus::BORDER);
        status.remove(WindowStatus::HIDDEN);
        let is_editable = window_def
            .combo_box_data
            .as_ref()
            .map(|data| data.is_editable)
            .unwrap_or(false);

        let button_width = 21;
        let button_height = height as i32;

        let drop_down_id = generate_window_id();
        let drop_down = self.create_window_with_id_internal(
            Some(window),
            (width as i32 - button_width).max(0),
            0,
            button_width,
            button_height,
            drop_down_id,
            false,
        )?;
        {
            let mut drop_mut = drop_down.borrow_mut();
            drop_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
            drop_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                drop_down_id as u32,
                0,
                0,
                button_width as u32,
                height.max(0) as u32,
            )));
            drop_mut.set_status_exact(status | WindowStatus::ACTIVE | WindowStatus::ENABLED);
            if let Some(font) = window.borrow().get_font().cloned() {
                drop_mut.set_font(font);
            }
            let _ = drop_mut.set_tooltip(window.borrow().get_tooltip());
            drop_mut.instance_data_mut().tooltip_delay = window.borrow().get_tooltip_delay();
            self.apply_draw_data_set(
                &mut drop_mut,
                &layout.combo_dropdown_enabled_draw_data,
                &layout.combo_dropdown_disabled_draw_data,
                &layout.combo_dropdown_hilite_draw_data,
            );
            self.apply_default_draw_callback(&mut drop_mut);
        }

        let edit_id = generate_window_id();
        let edit_width = (width as i32 - button_width).max(0);
        let edit = self.create_window_with_id_internal(
            Some(window),
            0,
            0,
            edit_width,
            height as i32,
            edit_id,
            false,
        )?;
        {
            let mut edit_mut = edit.borrow_mut();
            edit_mut.instance_data_mut().style |= GWS_ENTRY_FIELD;
            edit_mut.set_widget(WindowWidget::TextEntry(TextEntry::new(
                edit_id as u32,
                0,
                0,
                edit_width as u32,
                height.max(0) as u32,
            )));
            let mut edit_status = status;
            if !is_editable {
                edit_status |= WindowStatus::NO_INPUT;
            }
            edit_mut.set_status_exact(edit_status);
            if let Some(font) = window.borrow().get_font().cloned() {
                edit_mut.set_font(font);
            }
            let _ = edit_mut.set_tooltip(window.borrow().get_tooltip());
            edit_mut.instance_data_mut().tooltip_delay = window.borrow().get_tooltip_delay();
            if let Some(data) = window_def.combo_box_data.as_ref() {
                if let Some(WindowWidget::TextEntry(entry)) = edit_mut.widget_mut() {
                    let validation = if data.ascii_only {
                        super::gadgets::ValidationMode::AsciiOnly
                    } else if data.letters_and_numbers {
                        super::gadgets::ValidationMode::AlphanumericOnly
                    } else {
                        super::gadgets::ValidationMode::None
                    };
                    entry.set_validation(validation);
                    if data.max_chars > 0 {
                        entry.set_max_length(data.max_chars);
                    }
                }
            }
            self.apply_draw_data_set(
                &mut edit_mut,
                &layout.combo_edit_enabled_draw_data,
                &layout.combo_edit_disabled_draw_data,
                &layout.combo_edit_hilite_draw_data,
            );
            self.apply_default_draw_callback(&mut edit_mut);
        }

        let list_id = generate_window_id();
        let list = self.create_window_with_id_internal(
            Some(window),
            0,
            height as i32,
            width as i32,
            height as i32,
            list_id,
            false,
        )?;
        {
            let mut list_mut = list.borrow_mut();
            list_mut.instance_data_mut().style |= GWS_SCROLL_LISTBOX;
            list_mut.set_widget(WindowWidget::ListBox(ListBox::new(
                list_id as u32,
                0,
                height as i32,
                width.max(0) as u32,
                height.max(0) as u32,
            )));
            let mut list_status = status;
            list_status.remove(WindowStatus::IMAGE);
            list_mut.set_status_exact(list_status | WindowStatus::ABOVE | WindowStatus::ONE_LINE);
            list_mut.hide(true)?;
            if let Some(font) = window.borrow().get_font().cloned() {
                list_mut.set_font(font);
            }
            let _ = list_mut.set_tooltip(window.borrow().get_tooltip());
            list_mut.instance_data_mut().tooltip_delay = window.borrow().get_tooltip_delay();
            if let Some(WindowWidget::ListBox(listbox)) = list_mut.widget_mut() {
                listbox.set_max_length(10);
                listbox.set_auto_purge(false);
                listbox.set_auto_scroll(false);
                listbox.set_scroll_if_at_end(false);
                listbox.set_force_select(true);
                listbox.set_selection_mode(super::gadgets::SelectionMode::Single);
                listbox.set_columns(1);
                listbox.set_audio_feedback(true);
            }
            self.apply_draw_data_set(
                &mut list_mut,
                &layout.combo_list_enabled_draw_data,
                &layout.combo_list_disabled_draw_data,
                &layout.combo_list_hilite_draw_data,
            );
            self.apply_default_draw_callback(&mut list_mut);
        }

        self.create_listbox_scrollbar_children(&list, layout)?;

        window
            .borrow_mut()
            .set_combobox_links(super::game_window::ComboBoxLinks {
                drop_down: drop_down_id,
                edit_box: edit_id,
                list_box: list_id,
            });

        Ok(())
    }

    fn apply_draw_data_set(
        &self,
        window: &mut GameWindow,
        enabled: &[WindowDrawData],
        disabled: &[WindowDrawData],
        hilite: &[WindowDrawData],
    ) {
        for idx in 0..MAX_DRAW_DATA {
            if let Some(draw) = enabled.get(idx) {
                window.instance_data_mut().enabled_draw_data[idx] = draw.clone();
            }
            if let Some(draw) = disabled.get(idx) {
                window.instance_data_mut().disabled_draw_data[idx] = draw.clone();
            }
            if let Some(draw) = hilite.get(idx) {
                window.instance_data_mut().hilite_draw_data[idx] = draw.clone();
            }
        }
    }

    fn apply_default_draw_callback(&self, window: &mut GameWindow) {
        let has_image = window
            .instance_data()
            .enabled_draw_data
            .iter()
            .chain(window.instance_data().disabled_draw_data.iter())
            .chain(window.instance_data().hilite_draw_data.iter())
            .any(|draw| draw.image.is_some());

        let draw = match (window.widget(), has_image) {
            (Some(WindowWidget::PushButton(_)), true) => w3d_gadget_push_button_image_draw,
            (Some(WindowWidget::PushButton(_)), false) => w3d_gadget_push_button_draw,
            (Some(WindowWidget::TextEntry(_)), true) => w3d_gadget_text_entry_image_draw,
            (Some(WindowWidget::TextEntry(_)), false) => w3d_gadget_text_entry_draw,
            (Some(WindowWidget::ListBox(_)), true) => w3d_gadget_list_box_image_draw,
            (Some(WindowWidget::ListBox(_)), false) => w3d_gadget_list_box_draw,
            (Some(WindowWidget::StaticText(_)), true) => w3d_gadget_static_text_image_draw,
            (Some(WindowWidget::StaticText(_)), false) => w3d_gadget_static_text_draw,
            (Some(WindowWidget::ProgressBar(_)), true) => w3d_gadget_progress_bar_image_draw,
            (Some(WindowWidget::ProgressBar(_)), false) => w3d_gadget_progress_bar_draw,
            (Some(WindowWidget::CheckBox(_)), true) => w3d_gadget_check_box_image_draw,
            (Some(WindowWidget::CheckBox(_)), false) => w3d_gadget_check_box_draw,
            (Some(WindowWidget::RadioButton(_)), true) => w3d_gadget_radio_button_image_draw,
            (Some(WindowWidget::RadioButton(_)), false) => w3d_gadget_radio_button_draw,
            (Some(WindowWidget::VerticalSlider(_)), true) => w3d_gadget_vertical_slider_image_draw,
            (Some(WindowWidget::VerticalSlider(_)), false) => w3d_gadget_vertical_slider_draw,
            (Some(WindowWidget::HorizontalSlider(_)), true) => {
                w3d_gadget_horizontal_slider_image_draw
            }
            (Some(WindowWidget::HorizontalSlider(_)), false) => w3d_gadget_horizontal_slider_draw,
            (Some(WindowWidget::TabControl(_)), true) => w3d_gadget_tab_control_image_draw,
            (Some(WindowWidget::TabControl(_)), false) => w3d_gadget_tab_control_draw,
            (Some(WindowWidget::ComboBox(_)), true) => w3d_gadget_combo_box_image_draw,
            (Some(WindowWidget::ComboBox(_)), false) => w3d_gadget_combo_box_draw,
            // C++ W3DGameWindowManager::getDefaultDraw() returns W3DGameWinDefaultDraw,
            // so USER/[None] windows still render image/color draw data in the W3D path.
            _ => default_draw_callback,
        };
        window.set_draw_callback(draw);
    }

    fn apply_slider_draw_callback(&self, window: &mut GameWindow) {
        let has_image = window
            .instance_data()
            .enabled_draw_data
            .iter()
            .chain(window.instance_data().disabled_draw_data.iter())
            .chain(window.instance_data().hilite_draw_data.iter())
            .any(|draw| draw.image.is_some());

        let draw = if has_image {
            w3d_gadget_vertical_slider_image_draw
        } else {
            w3d_gadget_vertical_slider_draw
        };
        window.set_draw_callback(draw);
    }

    fn create_slider_thumb_child(
        &mut self,
        slider: &Rc<RefCell<GameWindow>>,
        layout: &WindowLayoutDefinition,
    ) -> WindowResult<()> {
        if layout.slider_thumb_enabled_draw_data.is_empty()
            && layout.slider_thumb_disabled_draw_data.is_empty()
            && layout.slider_thumb_hilite_draw_data.is_empty()
        {
            return Ok(());
        }

        let (width, _height) = slider.borrow().get_size();
        let is_horizontal = (slider.borrow().get_style() & GWS_HORZ_SLIDER) != 0;
        let (thumb_w, thumb_h) = if is_horizontal { (13, 16) } else { (width, 16) };
        let thumb_y = if is_horizontal { 10 } else { 0 };

        let mut status = slider.borrow().get_status();
        status.remove(WindowStatus::BORDER | WindowStatus::HIDDEN);
        status.insert(WindowStatus::ACTIVE | WindowStatus::ENABLED | WindowStatus::NO_INPUT);

        let thumb_id = generate_window_id();
        let thumb = self.create_window_with_id_internal(
            Some(slider),
            0,
            thumb_y,
            thumb_w,
            thumb_h,
            thumb_id,
            false,
        )?;
        {
            let mut thumb_mut = thumb.borrow_mut();
            thumb_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
            thumb_mut.set_status_exact(status);
            thumb_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                thumb_id as u32,
                0,
                0,
                thumb_w as u32,
                thumb_h as u32,
            )));
            self.apply_draw_data_set(
                &mut thumb_mut,
                &layout.slider_thumb_enabled_draw_data,
                &layout.slider_thumb_disabled_draw_data,
                &layout.slider_thumb_hilite_draw_data,
            );
            self.apply_default_draw_callback(&mut thumb_mut);
        }

        slider.borrow_mut().set_slider_thumb(thumb_id);
        slider.borrow_mut().update_slider_thumb();

        Ok(())
    }

    fn create_listbox_scrollbar_children(
        &mut self,
        listbox: &Rc<RefCell<GameWindow>>,
        layout: &WindowLayoutDefinition,
    ) -> WindowResult<()> {
        let (width, height) = listbox.borrow().get_size();
        let button_width = 21;
        let button_height = 22;
        let has_title = !listbox.borrow().get_text().is_empty();
        let font_height = if has_title {
            with_window_manager_ref(|manager| {
                listbox
                    .borrow()
                    .get_font()
                    .map(|font| manager.win_font_height(font))
                    .unwrap_or(12)
            })
        } else {
            0
        };
        let top = if has_title { font_height + 1 } else { 0 };
        let bottom = if has_title {
            height - (font_height + 1)
        } else {
            height
        };

        let mut status = listbox.borrow().get_status();
        status.remove(WindowStatus::BORDER | WindowStatus::HIDDEN | WindowStatus::NO_INPUT);
        status.insert(WindowStatus::ACTIVE | WindowStatus::ENABLED);

        let up_id = generate_window_id();
        let up_button = self.create_window_with_id_internal(
            Some(listbox),
            width - button_width - 2,
            top + 2,
            button_width,
            button_height,
            up_id,
            false,
        )?;
        {
            let mut up_mut = up_button.borrow_mut();
            up_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
            up_mut.set_status_exact(status);
            let mut button = PushButton::new(
                up_id as u32,
                0,
                0,
                button_width as u32,
                button_height as u32,
            );
            button.set_triggers_on_mouse_down(true);
            up_mut.set_widget(WindowWidget::PushButton(button));
            self.apply_draw_data_set(
                &mut up_mut,
                &layout.listbox_enabled_up_button_draw_data,
                &layout.listbox_disabled_up_button_draw_data,
                &layout.listbox_hilite_up_button_draw_data,
            );
            self.apply_default_draw_callback(&mut up_mut);
        }

        let down_id = generate_window_id();
        let down_button = self.create_window_with_id_internal(
            Some(listbox),
            width - button_width - 2,
            top + bottom - button_height - 2,
            button_width,
            button_height,
            down_id,
            false,
        )?;
        {
            let mut down_mut = down_button.borrow_mut();
            down_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
            down_mut.set_status_exact(status);
            let mut button = PushButton::new(
                down_id as u32,
                0,
                0,
                button_width as u32,
                button_height as u32,
            );
            button.set_triggers_on_mouse_down(true);
            down_mut.set_widget(WindowWidget::PushButton(button));
            self.apply_draw_data_set(
                &mut down_mut,
                &layout.listbox_enabled_down_button_draw_data,
                &layout.listbox_disabled_down_button_draw_data,
                &layout.listbox_hilite_down_button_draw_data,
            );
            self.apply_default_draw_callback(&mut down_mut);
        }

        let slider_id = generate_window_id();
        let slider_height = (bottom - (2 * button_height) - 6).max(0);
        let slider = self.create_window_with_id_internal(
            Some(listbox),
            width - button_width - 2,
            top + button_height + 3,
            button_width,
            slider_height,
            slider_id,
            false,
        )?;
        {
            let mut slider_mut = slider.borrow_mut();
            slider_mut.instance_data_mut().style |= GWS_VERT_SLIDER;
            slider_mut.set_status_exact(status);
            slider_mut.set_widget(WindowWidget::VerticalSlider(VerticalSlider::new(
                slider_id as u32,
                0,
                0,
                button_width as u32,
                slider_height as u32,
            )));
            self.apply_draw_data_set(
                &mut slider_mut,
                &layout.listbox_enabled_slider_draw_data,
                &layout.listbox_disabled_slider_draw_data,
                &layout.listbox_hilite_slider_draw_data,
            );
            self.apply_slider_draw_callback(&mut slider_mut);
        }

        let mut thumb_id = None;
        if !layout.slider_thumb_enabled_draw_data.is_empty()
            || !layout.slider_thumb_disabled_draw_data.is_empty()
            || !layout.slider_thumb_hilite_draw_data.is_empty()
        {
            let thumb_window_id = generate_window_id();
            let thumb = self.create_window_with_id_internal(
                Some(&slider),
                0,
                0,
                button_width,
                16,
                thumb_window_id,
                false,
            )?;
            {
                let mut thumb_mut = thumb.borrow_mut();
                thumb_mut.instance_data_mut().style |= GWS_PUSH_BUTTON;
                thumb_mut.set_status_exact(status);
                thumb_mut.set_widget(WindowWidget::PushButton(PushButton::new(
                    thumb_window_id as u32,
                    0,
                    0,
                    button_width as u32,
                    16,
                )));
                self.apply_draw_data_set(
                    &mut thumb_mut,
                    &layout.slider_thumb_enabled_draw_data,
                    &layout.slider_thumb_disabled_draw_data,
                    &layout.slider_thumb_hilite_draw_data,
                );
                self.apply_default_draw_callback(&mut thumb_mut);
            }
            thumb_id = Some(thumb_window_id);
        }

        listbox
            .borrow_mut()
            .set_listbox_links(super::game_window::ListBoxLinks {
                up_button: up_id,
                down_button: down_id,
                slider: slider_id,
                thumb: thumb_id,
            });
        listbox.borrow_mut().update_listbox_scrollbar();

        Ok(())
    }

    /// Check if window or any parent is hidden
    pub fn is_window_hidden(&self, window: &Rc<RefCell<GameWindow>>) -> bool {
        let mut current = Some(window.clone());
        while let Some(win) = current {
            let win_borrow = win.borrow();
            if win_borrow.is_hidden() {
                return true;
            }
            current = win_borrow.get_parent();
        }
        false
    }

    // Private helper methods

    /// Add window to root window list
    fn add_root_window(&mut self, window: Rc<RefCell<GameWindow>>) {
        // Insert at beginning for proper z-order
        self.root_windows.insert(0, window);
    }

    /// Process the destroy queue
    fn process_destroy_queue(&mut self) {
        while let Some(window) = self.destroy_queue.pop_front() {
            self.destroy_window_immediate(window);
        }
    }

    /// Immediately destroy a window
    fn destroy_window_immediate(&mut self, window: Rc<RefCell<GameWindow>>) {
        if window
            .borrow()
            .get_status()
            .contains(WindowStatus::DESTROYED)
        {
            return;
        }

        let window_id = window.borrow().get_id();
        let status = window.borrow().get_status() | WindowStatus::DESTROYED;
        window.borrow_mut().set_status_exact(status);

        // Remove from various manager references
        self.clear_references_to_destroyed_window(&window);

        let children = window.borrow().children().to_vec();
        for child in children {
            self.destroy_window_immediate(child);
        }

        // Remove from parent's children or root list
        let parent = window.borrow().get_parent();
        if let Some(parent) = parent {
            parent.borrow_mut().remove_child(&window);
        } else {
            self.root_windows.retain(|w| !Rc::ptr_eq(w, &window));
        }

        // Remove from lookup table
        self.window_by_id.remove(&window_id);

        // Send destroy message
        window
            .borrow_mut()
            .send_system_message(WindowMessage::Destroy, 0, 0);

        self.window_count = self.window_count.saturating_sub(1);
    }

    fn clear_references_to_destroyed_window(&mut self, window: &Rc<RefCell<GameWindow>>) {
        if self
            .keyboard_focus
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|focus| Rc::ptr_eq(&focus, window))
        {
            self.keyboard_focus = None;
        }

        if self
            .mouse_capture
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|capture| Rc::ptr_eq(&capture, window))
        {
            self.mouse_capture = None;
            self.capture_flags &= !CaptureFlags::MOUSE;
        }

        if self
            .modal_stack
            .as_ref()
            .is_some_and(|modal| Rc::ptr_eq(&modal.window, window))
        {
            if let Some(modal) = self.modal_stack.take() {
                self.modal_stack = modal.next;
            }
        }

        if self
            .current_mouse_region
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|region| Rc::ptr_eq(&region, window))
        {
            self.current_mouse_region = None;
        }

        if self
            .grab_window
            .as_ref()
            .and_then(Weak::upgrade)
            .is_some_and(|grab| Rc::ptr_eq(&grab, window))
        {
            self.grab_window = None;
        }
    }

    /// Find window at specific point (recursive)
    fn find_window_at_point(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        x: i32,
        y: i32,
        ignore_enabled: bool,
    ) -> Option<Rc<RefCell<GameWindow>>> {
        let window_borrow = window.borrow();

        // Skip if hidden or no-input
        if window_borrow.is_hidden() || window_borrow.get_status().contains(WindowStatus::NO_INPUT)
        {
            return None;
        }

        // Skip if disabled (unless ignoring enabled state)
        if !ignore_enabled && !window_borrow.is_enabled() {
            return None;
        }

        // Check if point is in this window
        if window_borrow.point_in_window(x, y) {
            // Check children first (they're on top)
            for child in window_borrow.children() {
                if let Some(found) = self.find_window_at_point(child, x, y, ignore_enabled) {
                    return Some(found);
                }
            }

            // Return this window if no child found
            return Some(window.clone());
        }

        None
    }

    /// Draw window and its children recursively
    fn draw_window_hierarchy(&self, window: &Rc<RefCell<GameWindow>>) {
        self.draw_window_hierarchy_internal(window, false);
    }

    fn draw_window_hierarchy_internal(
        &self,
        window: &Rc<RefCell<GameWindow>>,
        ancestor_hidden: bool,
    ) {
        let window_borrow = window.borrow();
        let name = window_borrow.get_name().to_string();
        let status = window_borrow.get_status();
        let see_thru = status.contains(WindowStatus::SEE_THRU);
        let effectively_hidden = ancestor_hidden || window_borrow.is_hidden();

        // Match C++ hierarchy semantics: a hidden parent suppresses its entire subtree.
        if effectively_hidden {
            return;
        }

        let border = status.contains(WindowStatus::BORDER) && !see_thru;
        let is_listbox = (window_borrow.get_style() & GWS_SCROLL_LISTBOX) != 0;

        if !see_thru {
            window_borrow.draw();
        }

        if is_listbox && border {
            window_borrow.draw_border_w3d();
        }

        // C++ drawWindow(): child = m_child; while(child->m_next) child = child->m_next;
        // for(; child; child = child->m_prev) drawWindow(child);
        // Our Vec is stored head-first, so reverse iteration matches tail-to-head repaint.
        for child in window_borrow.children().iter().rev() {
            self.draw_window_hierarchy_internal(child, effectively_hidden);
        }

        if !is_listbox && border {
            window_borrow.draw_border_w3d();
        }
    }

    /// Apply function to windows in ID range
    fn apply_to_window_range<F>(
        &self,
        base_window: &Rc<RefCell<GameWindow>>,
        first: WindowId,
        last: WindowId,
        mut func: F,
    ) where
        F: FnMut(&Rc<RefCell<GameWindow>>),
    {
        self.apply_to_window_hierarchy(base_window, &mut |window| {
            let window_id = window.borrow().get_id();
            if window_id >= first && window_id <= last {
                func(window);
            }
        });
    }

    /// Apply function to window hierarchy recursively
    fn apply_to_window_hierarchy<F>(&self, window: &Rc<RefCell<GameWindow>>, func: &mut F)
    where
        F: FnMut(&Rc<RefCell<GameWindow>>),
    {
        func(window);

        // Apply to children
        let children = window
            .borrow()
            .children()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        for child in &children {
            self.apply_to_window_hierarchy(child, func);
        }
    }
    // -----------------------------------------------------------------------
    // Gadget factory methods
    // -----------------------------------------------------------------------

    /// PARITY_NOTE: C++ uses `TheWindowManager->getMessageBox()` with explicit
    /// yes/no button callbacks. This Rust version creates the window directly
    /// and wires up the callbacks via user data, matching the observable behavior.
    pub fn gogo_message_box(
        &mut self,
        title: &str,
        body: &str,
        yes_cb: Option<Box<dyn Fn()>>,
        no_cb: Option<Box<dyn Fn()>>,
    ) -> Option<WindowId> {
        let (screen_w, screen_h) = self.screen_size;
        let box_w = (screen_w as f32 * 0.4) as i32;
        let box_h = (screen_h as f32 * 0.25) as i32;
        let box_x = (screen_w - box_w) / 2;
        let box_y = (screen_h - box_h) / 2;

        let window = self.create_window(None, box_x, box_y, box_w, box_h).ok()?;
        let window_id = window.borrow().get_id();

        {
            let mut wm = window.borrow_mut();
            wm.set_name("MessageBox");
            let _ = wm.set_text(body);
            wm.instance_data_mut().text_label = title.to_string();
            wm.set_status_exact(
                WindowStatus::ACTIVE
                    | WindowStatus::ENABLED
                    | WindowStatus::ABOVE
                    | WindowStatus::NO_FOCUS,
            );
            if let Some(cb) = yes_cb {
                wm.set_user_data(cb);
            }
            wm.set_system_callback(default_system_callback);
            wm.set_draw_callback(default_draw_callback);
        }

        let _ = self.set_modal(window);
        Some(window_id)
    }

    pub fn gogo_gadget_push_button(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_PUSH_BUTTON;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::PushButton(PushButton::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_checkbox(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_CHECK_BOX;
            let gadget_id = window_id as u32;
            let box_size = size.0.min(size.1).max(0) as u32;
            wm.set_widget(WindowWidget::CheckBox(CheckBox::new(
                gadget_id, pos.0, pos.1, box_size,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_radio_button(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_RADIO_BUTTON;
            let gadget_id = window_id as u32;
            let group = RadioButtonGroup::new(gadget_id);
            let btn_size = size.0.min(size.1).max(0) as u32;
            wm.set_widget(WindowWidget::RadioButton(RadioButton::new(
                gadget_id, pos.0, pos.1, btn_size, group,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_tab_control(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_TAB_CONTROL;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::TabControl(TabControl::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_list_box(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_SCROLL_LISTBOX;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::ListBox(ListBox::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_slider(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_HORZ_SLIDER;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::HorizontalSlider(HorizontalSlider::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_progress_bar(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_PROGRESS_BAR;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::ProgressBar(ProgressBar::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_static_text(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_STATIC_TEXT;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::StaticText(StaticText::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_text_entry(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_ENTRY_FIELD;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::TextEntry(TextEntry::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }

    pub fn gogo_gadget_combo_box(
        &mut self,
        parent: Option<&Rc<RefCell<GameWindow>>>,
        pos: (i32, i32),
        size: (i32, i32),
    ) -> Option<WindowId> {
        let window = self
            .create_window(parent, pos.0, pos.1, size.0, size.1)
            .ok()?;
        let window_id = window.borrow().get_id();
        {
            let mut wm = window.borrow_mut();
            wm.instance_data_mut().style = GWS_COMBO_BOX;
            let gadget_id = window_id as u32;
            wm.set_widget(WindowWidget::ComboBox(ComboBox::new(
                gadget_id,
                pos.0,
                pos.1,
                size.0.max(0) as u32,
                size.1.max(0) as u32,
            )));
            wm.set_system_callback(default_system_callback);
            wm.set_input_callback(default_input_callback);
        }
        {
            let mut wm = window.borrow_mut();
            self.apply_default_draw_callback(&mut wm);
        }
        Some(window_id)
    }
}

fn resolve_window_script_path(filename: &str) -> WindowResult<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(current_dir) = std::env::current_dir() {
        for base in current_dir.ancestors() {
            candidates.push(
                base.join("windows_game/extracted_big_files_v2/WindowZH/Window")
                    .join(filename),
            );
            candidates.push(
                base.join("windows_game/extracted_big_files_v2/WindowZH/Window/Menus")
                    .join(filename),
            );
            candidates.push(
                base.join("windows_game/extracted_big_files/WindowZH/Window")
                    .join(filename),
            );
            candidates.push(
                base.join("windows_game/extracted_big_files/WindowZH/Window/Menus")
                    .join(filename),
            );
        }
    }
    candidates
        .push(Path::new("windows_game/extracted_big_files_v2/WindowZH/Window").join(filename));
    candidates.push(
        Path::new("windows_game/extracted_big_files_v2/WindowZH/Window/Menus").join(filename),
    );
    candidates.push(Path::new("windows_game/extracted_big_files/WindowZH/Window").join(filename));
    candidates
        .push(Path::new("windows_game/extracted_big_files/WindowZH/Window/Menus").join(filename));
    candidates.push(Path::new(filename).to_path_buf());
    for path in candidates {
        if path.exists() {
            return Ok(path);
        }
    }
    Err(WindowError::InvalidParameter)
}

fn style_for_window_type(window_type: &str) -> u32 {
    match window_type.trim().to_ascii_uppercase().as_str() {
        "PUSHBUTTON" => GWS_PUSH_BUTTON,
        "RADIOBUTTON" => GWS_RADIO_BUTTON,
        "CHECKBOX" => GWS_CHECK_BOX,
        "VERTSLIDER" => GWS_VERT_SLIDER,
        "HORZSLIDER" => GWS_HORZ_SLIDER,
        "SCROLLLISTBOX" => GWS_SCROLL_LISTBOX,
        "ENTRYFIELD" => GWS_ENTRY_FIELD,
        "STATICTEXT" => GWS_STATIC_TEXT,
        "PROGRESSBAR" => GWS_PROGRESS_BAR,
        "USER" => GWS_USER_WINDOW,
        "TABCONTROL" => GWS_TAB_CONTROL,
        "TABPANE" => GWS_TAB_PANE,
        "COMBOBOX" => GWS_COMBO_BOX,
        _ => 0,
    }
}

fn create_widget_for_style(
    radio_groups: &mut HashMap<u32, RadioButtonGroup>,
    window_def: &WindowDefinition,
    window_id: WindowId,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) -> Option<WindowWidget> {
    let gadget_id = if window_id > 0 { window_id as u32 } else { 0 };
    let width_u = width.max(0) as u32;
    let height_u = height.max(0) as u32;
    let size = width.min(height).max(0) as u32;
    let text = if !window_def.text.is_empty() {
        window_def.text.clone()
    } else {
        window_def.text_label.clone()
    };

    let style = window_def.style | style_for_window_type(&window_def.window_type);
    if style & GWS_PUSH_BUTTON != 0 {
        let mut button = PushButton::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            button.set_text(text);
        }
        return Some(WindowWidget::PushButton(button));
    }
    if style & GWS_RADIO_BUTTON != 0 {
        let group_id = window_def
            .radio_button_data
            .as_ref()
            .map(|data| data.group)
            .unwrap_or(gadget_id);
        let group = radio_groups
            .entry(group_id)
            .or_insert_with(|| RadioButtonGroup::new(group_id))
            .clone();
        let mut radio = RadioButton::new(gadget_id, x, y, size, group);
        if !text.is_empty() {
            radio.set_label(text);
        }
        return Some(WindowWidget::RadioButton(radio));
    }
    if style & GWS_CHECK_BOX != 0 {
        let checkbox = super::gadgets::CheckBox::new(gadget_id, x, y, size);
        return Some(WindowWidget::CheckBox(checkbox));
    }
    if style & GWS_VERT_SLIDER != 0 {
        return Some(WindowWidget::VerticalSlider(VerticalSlider::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_HORZ_SLIDER != 0 {
        return Some(WindowWidget::HorizontalSlider(HorizontalSlider::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_SCROLL_LISTBOX != 0 {
        return Some(WindowWidget::ListBox(ListBox::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_ENTRY_FIELD != 0 {
        let mut entry = TextEntry::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            entry.set_text(text);
        }
        return Some(WindowWidget::TextEntry(entry));
    }
    if style & GWS_STATIC_TEXT != 0 {
        let mut label = StaticText::new(gadget_id, x, y, width_u, height_u);
        if !text.is_empty() {
            label.set_text(text);
        }
        return Some(WindowWidget::StaticText(label));
    }
    if style & GWS_PROGRESS_BAR != 0 {
        return Some(WindowWidget::ProgressBar(ProgressBar::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_USER_WINDOW != 0 {
        return Some(WindowWidget::User);
    }
    if style & GWS_MOUSE_TRACK != 0 {
        return Some(WindowWidget::MouseTrack);
    }
    if style & GWS_ANIMATED != 0 {
        return Some(WindowWidget::Animated);
    }
    if style & GWS_TAB_CONTROL != 0 {
        return Some(WindowWidget::TabControl(TabControl::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }
    if style & GWS_TAB_PANE != 0 {
        return Some(WindowWidget::TabPane);
    }
    if style & GWS_COMBO_BOX != 0 {
        return Some(WindowWidget::ComboBox(ComboBox::new(
            gadget_id, x, y, width_u, height_u,
        )));
    }

    None
}

fn apply_window_text(window: &mut GameWindow, window_def: &WindowDefinition) {
    let text = if !window_def.text_label.is_empty() {
        GameText::fetch(&window_def.text_label)
    } else if !window_def.text.is_empty() {
        if window_def.text.contains(':') && !window_def.text.contains(' ') {
            GameText::fetch(&window_def.text)
        } else {
            window_def.text.clone()
        }
    } else {
        return;
    };

    let _ = window.set_text(&text);
}

fn apply_window_tooltip(window: &mut GameWindow, window_def: &WindowDefinition) {
    if window_def.tooltip.is_empty() {
        return;
    }
    let tooltip = GameText::fetch(&window_def.tooltip);
    window.set_tooltip(&tooltip);
    if let Some(widget) = window.widget_mut() {
        if let WindowWidget::ListBox(listbox) = widget {
            listbox.set_tooltip(tooltip);
        }
    }
}

fn map_window_message_to_main_menu(msg: WindowMessage) -> u32 {
    const GGM_LEFT_DRAG: u32 = 16384;
    const GBM_MOUSE_ENTERING: u32 = GGM_LEFT_DRAG + 6;
    const GBM_MOUSE_LEAVING: u32 = GGM_LEFT_DRAG + 7;
    const GBM_SELECTED: u32 = GGM_LEFT_DRAG + 8;
    const GBM_SELECTED_RIGHT: u32 = GGM_LEFT_DRAG + 9;

    match msg {
        WindowMessage::Create => 1,
        WindowMessage::Destroy => 2,
        WindowMessage::Char => 21,
        WindowMessage::InputFocus => 23,
        WindowMessage::MousePos => 24,
        WindowMessage::GadgetMouseEntering => GBM_MOUSE_ENTERING,
        WindowMessage::GadgetMouseLeaving => GBM_MOUSE_LEAVING,
        WindowMessage::GadgetSelected => GBM_SELECTED,
        WindowMessage::GadgetRightClick => GBM_SELECTED_RIGHT,
        _ => 0,
    }
}

fn apply_window_status_to_widget(window: &mut GameWindow) {
    let status = window.get_status();
    if let Some(widget) = window.widget_mut() {
        match widget {
            WindowWidget::PushButton(button) => {
                if status.contains(WindowStatus::CHECK_LIKE) {
                    button.set_checkbox(true, false);
                }
                if status.contains(WindowStatus::ON_MOUSE_DOWN) {
                    button.set_triggers_on_mouse_down(true);
                }
            }
            _ => {}
        }
    }
}

fn apply_window_widget_data(window: &mut GameWindow, window_def: &WindowDefinition) {
    if let Some(widget) = window.widget_mut() {
        match widget {
            WindowWidget::ListBox(listbox) => {
                if let Some(data) = window_def.listbox_data.as_ref() {
                    if data.length > 0 {
                        listbox.set_max_length(data.length);
                    }
                    listbox.set_auto_purge(data.autopurge);
                    listbox.set_auto_scroll(data.autoscroll);
                    listbox.set_scroll_if_at_end(data.scroll_if_at_end);
                    listbox.set_force_select(data.force_select);
                    listbox.set_columns(data.columns);
                    if !data.column_widths.is_empty() {
                        listbox.set_column_width_percentages(data.column_widths.clone());
                    }
                    if data.multiselect {
                        listbox.set_selection_mode(super::gadgets::SelectionMode::Multiple);
                    }
                }
            }
            WindowWidget::TextEntry(entry) => {
                if let Some(data) = window_def.text_entry_data.as_ref() {
                    if data.max_len > 0 {
                        entry.set_max_length(data.max_len);
                    }
                    entry.set_password(data.secret_text);
                    let validation = if data.numerical_only {
                        super::gadgets::ValidationMode::NumericOnly
                    } else if data.alphanumerical_only {
                        super::gadgets::ValidationMode::AlphanumericOnly
                    } else if data.ascii_only {
                        super::gadgets::ValidationMode::AsciiOnly
                    } else {
                        super::gadgets::ValidationMode::None
                    };
                    entry.set_validation(validation);
                }
            }
            WindowWidget::StaticText(label) => {
                if let Some(data) = window_def.static_text_data.as_ref() {
                    if data.centered {
                        label.set_alignment(
                            super::gadgets::TextAlignment::Center,
                            super::gadgets::VerticalAlignment::Center,
                        );
                    }
                }
            }
            WindowWidget::HorizontalSlider(slider) => {
                if let Some(data) = window_def.slider_data.as_ref() {
                    slider.set_range(data.min_value, data.max_value);
                    window.update_slider_thumb();
                }
            }
            WindowWidget::VerticalSlider(slider) => {
                if let Some(data) = window_def.slider_data.as_ref() {
                    slider.set_range(data.min_value, data.max_value);
                    window.update_slider_thumb();
                }
            }
            WindowWidget::ComboBox(combo) => {
                if let Some(data) = window_def.combo_box_data.as_ref() {
                    combo.set_editable(data.is_editable);
                    if data.max_chars > 0 {
                        combo.set_max_chars(data.max_chars);
                    }
                    combo.set_ascii_only(data.ascii_only);
                    combo.set_letters_and_numbers(data.letters_and_numbers);
                    if data.max_display > 0 {
                        combo.set_max_display(data.max_display);
                    }
                }
            }
            WindowWidget::TabControl(tab_control) => {
                if let Some(data) = window_def.tab_control_data.as_ref() {
                    tab_control.set_tab_data(super::gadgets::TabControlData {
                        tab_orientation: data.tab_orientation,
                        tab_edge: data.tab_edge,
                        tab_width: data.tab_width,
                        tab_height: data.tab_height,
                        tab_count: data.tab_count,
                        pane_border: data.pane_border,
                        sub_pane_disabled: data.sub_pane_disabled,
                    });
                }
            }
            _ => {}
        }
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    static TEST_MOUSE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn lock_test_mouse() -> MutexGuard<'static, ()> {
        TEST_MOUSE_LOCK
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("test mouse lock poisoned")
    }

    #[test]
    fn test_window_manager_creation() {
        let manager = WindowManager::new();
        assert_eq!(manager.window_count, 0);
        assert!(manager.root_windows.is_empty());
        assert!(manager.get_focus().is_none());
    }

    #[test]
    fn test_window_creation() {
        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

        assert_eq!(manager.window_count, 1);
        assert_eq!(manager.root_windows.len(), 1);

        let window_id = window.borrow().get_id();
        assert!(window_id > 0);

        let found_window = manager.get_window_by_id(window_id).unwrap();
        assert!(Rc::ptr_eq(&window, &found_window));
    }

    #[test]
    fn test_window_hierarchy() {
        let mut manager = WindowManager::new();

        let parent = manager.create_window(None, 0, 0, 200, 200).unwrap();
        let child = manager
            .create_window(Some(&parent), 10, 10, 50, 50)
            .unwrap();

        assert_eq!(manager.window_count, 2);
        assert_eq!(manager.root_windows.len(), 1); // Only parent is root

        let parent_borrow = parent.borrow();
        assert!(parent_borrow.is_child(&*child.borrow()));

        let child_borrow = child.borrow();
        let child_parent = child_borrow.get_parent().unwrap();
        assert!(Rc::ptr_eq(&parent, &child_parent));
    }

    #[test]
    fn test_focus_management() {
        let mut manager = WindowManager::new();
        let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let window2 = manager.create_window(None, 100, 100, 100, 100).unwrap();
        window1
            .borrow_mut()
            .set_system_callback(|_, msg, _, _| match msg {
                WindowMessage::InputFocus => WindowMsgHandled::Handled,
                _ => WindowMsgHandled::Ignored,
            });
        window2
            .borrow_mut()
            .set_system_callback(|_, msg, _, _| match msg {
                WindowMessage::InputFocus => WindowMsgHandled::Handled,
                _ => WindowMsgHandled::Ignored,
            });

        assert!(manager.get_focus().is_none());

        manager.set_focus(Some(&window1)).unwrap();
        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window1, &focused));

        manager.set_focus(Some(&window2)).unwrap();
        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window2, &focused));

        manager.set_focus(None).unwrap();
        assert!(manager.get_focus().is_none());
    }

    #[test]
    fn test_focus_requires_input_focus_acceptance() {
        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

        manager.set_focus(Some(&window)).unwrap();

        assert!(manager.get_focus().is_none());
    }

    #[test]
    fn set_focus_does_not_send_lost_when_refocusing_same_window_like_cpp() {
        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let focus_messages = Rc::new(RefCell::new(Vec::new()));

        {
            let focus_messages = Rc::clone(&focus_messages);
            window
                .borrow_mut()
                .set_system_callback(move |_, msg, data1, _| {
                    if msg == WindowMessage::InputFocus {
                        focus_messages.borrow_mut().push(data1);
                        return if data1 != 0 {
                            WindowMsgHandled::Handled
                        } else {
                            WindowMsgHandled::Ignored
                        };
                    }
                    WindowMsgHandled::Ignored
                });
        }

        manager.set_focus(Some(&window)).unwrap();
        manager.set_focus(Some(&window)).unwrap();

        assert_eq!(focus_messages.borrow().as_slice(), &[1, 1]);
        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window, &focused));
    }

    #[test]
    fn set_focus_no_focus_window_preserves_existing_focus_like_cpp() {
        let mut manager = WindowManager::new();
        let focused = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let no_focus = manager.create_window(None, 100, 0, 100, 100).unwrap();
        let focus_messages = Rc::new(RefCell::new(Vec::new()));

        {
            let focus_messages = Rc::clone(&focus_messages);
            focused
                .borrow_mut()
                .set_system_callback(move |_, msg, data1, _| {
                    if msg == WindowMessage::InputFocus {
                        focus_messages.borrow_mut().push(data1);
                        return if data1 != 0 {
                            WindowMsgHandled::Handled
                        } else {
                            WindowMsgHandled::Ignored
                        };
                    }
                    WindowMsgHandled::Ignored
                });
        }
        no_focus
            .borrow_mut()
            .set_status_exact(WindowStatus::ENABLED | WindowStatus::NO_FOCUS);

        manager.set_focus(Some(&focused)).unwrap();
        manager.set_focus(Some(&no_focus)).unwrap();

        assert_eq!(focus_messages.borrow().as_slice(), &[1]);
        let current = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&focused, &current));
    }

    #[test]
    fn test_focus_acceptance_can_come_from_parent() {
        let mut manager = WindowManager::new();
        let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let child = manager
            .create_window(Some(&parent), 10, 10, 20, 20)
            .unwrap();
        parent
            .borrow_mut()
            .set_system_callback(|_, msg, data1, _| match msg {
                WindowMessage::InputFocus if data1 != 0 => WindowMsgHandled::Handled,
                _ => WindowMsgHandled::Ignored,
            });

        manager.set_focus(Some(&child)).unwrap();

        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&child, &focused));
    }

    #[test]
    fn process_key_event_passes_key_and_state_to_parent_until_handled() {
        let mut manager = WindowManager::new();
        let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let child = manager
            .create_window(Some(&parent), 10, 10, 20, 20)
            .unwrap();
        let seen = Rc::new(RefCell::new(Vec::new()));

        parent
            .borrow_mut()
            .set_system_callback(|_, msg, data1, _| match msg {
                WindowMessage::InputFocus if data1 != 0 => WindowMsgHandled::Handled,
                _ => WindowMsgHandled::Ignored,
            });

        child
            .borrow_mut()
            .set_input_callback(|_, msg, data1, data2| {
                if msg == WindowMessage::Char {
                    assert_eq!((data1, data2), (13, 0x02));
                }
                WindowMsgHandled::Ignored
            });

        {
            let seen = Rc::clone(&seen);
            parent
                .borrow_mut()
                .set_input_callback(move |_, msg, data1, data2| {
                    seen.borrow_mut().push((msg, data1, data2));
                    WindowMsgHandled::Handled
                });
        }

        manager.set_focus(Some(&child)).unwrap();

        assert_eq!(
            manager.process_key_event(13, 0x02),
            WindowInputReturnCode::Used
        );
        assert_eq!(seen.borrow().as_slice(), &[(WindowMessage::Char, 13, 0x02)]);
        assert_eq!(
            manager.process_key_event(0, 0x02),
            WindowInputReturnCode::NotUsed
        );
    }

    #[test]
    fn test_mouse_capture() {
        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

        assert!(manager.get_capture().is_none());

        manager.capture_mouse(&window).unwrap();
        let captured = manager.get_capture().unwrap();
        assert!(Rc::ptr_eq(&window, &captured));

        manager.release_capture(&window).unwrap();
        assert!(manager.get_capture().is_none());
    }

    #[test]
    fn release_capture_is_idempotent_like_cpp() {
        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let other = manager.create_window(None, 100, 0, 100, 100).unwrap();

        assert_eq!(manager.release_capture(&window), Ok(()));

        manager.capture_mouse(&window).unwrap();
        assert_eq!(manager.release_capture(&other), Ok(()));
        let captured = manager.get_capture().unwrap();
        assert!(Rc::ptr_eq(&window, &captured));

        assert_eq!(manager.release_capture(&window), Ok(()));
        assert!(manager.get_capture().is_none());
    }

    #[test]
    fn process_mouse_event_sets_window_tooltip_like_cpp() {
        let _mouse_guard = lock_test_mouse();
        with_mouse(|mouse| mouse.set_cursor_tooltip("Stale".to_string(), Some(0), None, None));

        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
        {
            let mut window = window.borrow_mut();
            window.set_tooltip("Window tip");
            window.instance_data_mut().tooltip_delay = 123;
        }

        let result = manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0);

        assert_eq!(result, WindowInputReturnCode::NotUsed);
        with_mouse(|mouse| {
            let state = mouse.cursor_tooltip_state();
            assert_eq!(state.tooltip_text, "Window tip");
            assert_eq!(state.tooltip_delay_override_ms, 123);
            assert!(!state.is_tooltip_empty);
        });
    }

    #[test]
    fn process_mouse_event_clears_stale_tooltip_without_tooltip_window_like_cpp() {
        let _mouse_guard = lock_test_mouse();
        with_mouse(|mouse| mouse.set_cursor_tooltip("Stale".to_string(), Some(0), None, None));

        let mut manager = WindowManager::new();

        let result = manager.process_mouse_event(WindowMessage::MousePos, 500, 500, 0);

        assert_eq!(result, WindowInputReturnCode::NotUsed);
        with_mouse(|mouse| {
            let state = mouse.cursor_tooltip_state();
            assert_eq!(state.tooltip_text, "");
            assert!(state.is_tooltip_empty);
        });
    }

    #[test]
    fn process_mouse_event_reads_disabled_window_tooltip_like_cpp() {
        let _mouse_guard = lock_test_mouse();
        with_mouse(|mouse| mouse.set_cursor_tooltip(String::new(), None, None, None));

        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
        {
            let mut window = window.borrow_mut();
            window.set_tooltip("Disabled tip");
            window.enable(false).unwrap();
        }

        let result = manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0);

        assert_eq!(result, WindowInputReturnCode::NotUsed);
        with_mouse(|mouse| {
            let state = mouse.cursor_tooltip_state();
            assert_eq!(state.tooltip_text, "Disabled tip");
            assert!(!state.is_tooltip_empty);
        });
    }

    #[test]
    fn process_mouse_event_only_clears_tooltip_during_capture_like_cpp() {
        let _mouse_guard = lock_test_mouse();
        with_mouse(|mouse| mouse.set_cursor_tooltip("Stale".to_string(), Some(0), None, None));

        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
        window.borrow_mut().set_tooltip("Captured tip");
        manager.capture_mouse(&window).unwrap();

        let result = manager.process_mouse_event(WindowMessage::MousePos, 10, 10, 0);

        assert_eq!(result, WindowInputReturnCode::NotUsed);
        with_mouse(|mouse| {
            let state = mouse.cursor_tooltip_state();
            assert_eq!(state.tooltip_text, "");
            assert!(state.is_tooltip_empty);
        });
    }

    #[test]
    fn test_modal_windows() {
        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();

        manager.set_modal(window.clone()).unwrap();
        // Modal stack would be tested here, but the current implementation
        // doesn't provide easy access to check the modal stack state

        manager.unset_modal(&window).unwrap();
    }

    #[test]
    fn set_modal_rejects_child_windows_like_cpp() {
        let mut manager = WindowManager::new();
        let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let child = manager
            .create_window(Some(&parent), 10, 10, 20, 20)
            .unwrap();

        assert_eq!(manager.set_modal(child), Err(WindowError::InvalidParameter));
        assert!(manager.modal_stack.is_none());
    }

    #[test]
    fn get_window_under_cursor_prioritizes_above_normal_below_like_cpp() {
        let mut manager = WindowManager::new();
        let below = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let normal = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let above = manager.create_window(None, 0, 0, 100, 100).unwrap();

        below
            .borrow_mut()
            .set_status_exact(WindowStatus::ENABLED | WindowStatus::BELOW);
        normal.borrow_mut().set_status_exact(WindowStatus::ENABLED);
        above
            .borrow_mut()
            .set_status_exact(WindowStatus::ENABLED | WindowStatus::ABOVE);

        let found = manager.get_window_under_cursor(10, 10, false).unwrap();
        assert!(Rc::ptr_eq(&above, &found));

        above
            .borrow_mut()
            .set_status_exact(WindowStatus::ENABLED | WindowStatus::ABOVE | WindowStatus::HIDDEN);

        let found = manager.get_window_under_cursor(10, 10, false).unwrap();
        assert!(Rc::ptr_eq(&normal, &found));
    }

    #[test]
    fn test_window_destruction() {
        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let window_id = window.borrow().get_id();

        assert_eq!(manager.window_count, 1);
        assert!(manager.get_window_by_id(window_id).is_some());

        manager.destroy_window(window).unwrap();
        manager.update(); // Process destroy queue

        assert_eq!(manager.window_count, 0);
        assert!(manager.get_window_by_id(window_id).is_none());
    }

    #[test]
    fn destroy_window_recursively_destroys_children_like_cpp() {
        let mut manager = WindowManager::new();
        let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let child = manager
            .create_window(Some(&parent), 10, 10, 20, 20)
            .unwrap();
        let parent_id = parent.borrow().get_id();
        let child_id = child.borrow().get_id();

        manager.destroy_window(parent.clone()).unwrap();
        manager.update();

        assert_eq!(manager.window_count, 0);
        assert!(manager.get_window_by_id(parent_id).is_none());
        assert!(manager.get_window_by_id(child_id).is_none());
        assert!(parent
            .borrow()
            .get_status()
            .contains(WindowStatus::DESTROYED));
        assert!(child
            .borrow()
            .get_status()
            .contains(WindowStatus::DESTROYED));
    }

    #[test]
    fn destroy_window_clears_runtime_references_like_cpp() {
        let mut manager = WindowManager::new();
        let window = manager.create_window(None, 0, 0, 100, 100).unwrap();
        window
            .borrow_mut()
            .set_system_callback(|_, msg, data1, _| match msg {
                WindowMessage::InputFocus if data1 != 0 => WindowMsgHandled::Handled,
                _ => WindowMsgHandled::Ignored,
            });

        manager.set_focus(Some(&window)).unwrap();
        manager.capture_mouse(&window).unwrap();
        manager.set_modal(window.clone()).unwrap();
        manager.current_mouse_region = Some(Rc::downgrade(&window));
        manager.set_grab_window(Some(&window));

        manager.destroy_window(window).unwrap();
        manager.update();

        assert!(manager.get_focus().is_none());
        assert!(manager.get_capture().is_none());
        assert!(manager.modal_stack.is_none());
        assert!(manager.current_mouse_region.is_none());
        assert!(manager.get_grab_window().is_none());
        assert!(!manager.capture_flags.contains(CaptureFlags::MOUSE));
    }

    #[test]
    fn test_layout_hide_only_toggles_root_windows() {
        let mut manager = WindowManager::new();
        let layout = manager.create_layout("test_layout.wnd".to_string());

        let parent = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let child = manager.create_window(Some(&parent), 5, 5, 20, 20).unwrap();
        child.borrow_mut().hide(true).unwrap();

        {
            let mut layout_mut = layout.borrow_mut();
            layout_mut.add_window(parent.clone());
            layout_mut.add_window(child.clone());
        }

        layout.borrow().hide(false);

        assert!(!parent.borrow().is_hidden());
        assert!(child.borrow().is_hidden());
    }

    #[test]
    fn test_tab_navigation() {
        let mut manager = WindowManager::new();
        let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let window2 = manager.create_window(None, 100, 0, 100, 100).unwrap();
        let window3 = manager.create_window(None, 200, 0, 100, 100).unwrap();
        for window in [&window1, &window2, &window3] {
            window
                .borrow_mut()
                .set_system_callback(|_, msg, _, _| match msg {
                    WindowMessage::InputFocus => WindowMsgHandled::Handled,
                    _ => WindowMsgHandled::Ignored,
                });
        }

        manager.register_tab_list(vec![window1.clone(), window2.clone(), window3.clone()]);

        // Set initial focus
        manager.set_focus(Some(&window1)).unwrap();

        // Navigate forward
        manager.navigate_tab(TabDirection::Next);
        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window2, &focused));

        manager.navigate_tab(TabDirection::Next);
        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window3, &focused));

        // Should wrap around
        manager.navigate_tab(TabDirection::Next);
        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window1, &focused));

        // Navigate backward
        manager.navigate_tab(TabDirection::Previous);
        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window3, &focused));
    }

    #[test]
    fn tab_navigation_is_blocked_by_modal_like_cpp() {
        let mut manager = WindowManager::new();
        let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let window2 = manager.create_window(None, 100, 0, 100, 100).unwrap();
        let modal = manager.create_window(None, 0, 100, 100, 100).unwrap();
        for window in [&window1, &window2] {
            window
                .borrow_mut()
                .set_system_callback(|_, msg, _, _| match msg {
                    WindowMessage::InputFocus => WindowMsgHandled::Handled,
                    _ => WindowMsgHandled::Ignored,
                });
        }

        manager.register_tab_list(vec![window1.clone(), window2.clone()]);
        manager.set_focus(Some(&window1)).unwrap();
        manager.set_modal(modal).unwrap();

        manager.navigate_tab(TabDirection::Next);

        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window1, &focused));
    }

    #[test]
    fn tab_navigation_clears_lone_window_like_cpp() {
        let mut manager = WindowManager::new();
        let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let window2 = manager.create_window(None, 100, 0, 100, 100).unwrap();
        let lone = manager.create_window(None, 200, 0, 100, 100).unwrap();
        for window in [&window1, &window2] {
            window
                .borrow_mut()
                .set_system_callback(|_, msg, _, _| match msg {
                    WindowMessage::InputFocus => WindowMsgHandled::Handled,
                    _ => WindowMsgHandled::Ignored,
                });
        }
        let close_count = Rc::new(RefCell::new(0));
        {
            let close_count = Rc::clone(&close_count);
            lone.borrow_mut().set_system_callback(move |_, msg, _, _| {
                if msg == WindowMessage::User(16389) {
                    *close_count.borrow_mut() += 1;
                    WindowMsgHandled::Handled
                } else {
                    WindowMsgHandled::Ignored
                }
            });
        }

        manager.register_tab_list(vec![window1.clone(), window2.clone()]);
        manager.set_focus(Some(&window1)).unwrap();
        manager.set_lone_window(Some(&lone));

        manager.navigate_tab(TabDirection::Next);

        let focused = manager.get_focus().unwrap();
        assert!(Rc::ptr_eq(&window2, &focused));
        assert!(manager.lone_window.is_none());
        assert_eq!(*close_count.borrow(), 1);
    }

    #[test]
    fn test_window_layout() {
        let mut manager = WindowManager::new();
        let layout = manager.create_layout("test.wnd".to_string());

        let window1 = manager.create_window(None, 0, 0, 100, 100).unwrap();
        let window2 = manager.create_window(None, 100, 100, 100, 100).unwrap();

        layout.borrow_mut().add_window(window1.clone());
        layout.borrow_mut().add_window(window2.clone());

        assert_eq!(layout.borrow().windows.len(), 2);
        assert_eq!(layout.borrow().get_filename(), "test.wnd");

        // Test hiding layout
        layout.borrow_mut().hide(true);
        assert!(layout.borrow().is_hidden());
        assert!(window1.borrow().is_hidden());
        assert!(window2.borrow().is_hidden());
    }
}
