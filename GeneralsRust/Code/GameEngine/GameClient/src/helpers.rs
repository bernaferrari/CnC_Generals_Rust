/*
**  Command & Conquer Generals Zero Hour(tm)
**  Minimal helper singletons for the Rust game client port.
**
**  The original C++ client relied heavily on global singletons such as
**  `TheInGameUI`.  To keep the Rust conversion close to the original flow
**  while remaining testable and platform‑agnostic, we provide lightweight
**  facades that simply log the requested operations.  Higher level systems
**  can hook into these calls later to forward them to the real gameplay
**  pipeline.
*/

use crate::game_text::GameText;
use crate::display::view::with_tactical_view_ref;
use crate::gui::{get_shell, with_window_manager, WindowLayout, WindowStatus};
use crate::input::Mouse;
use crate::message_stream::game_message::{Coord3D, ICoord2D};
use gamelogic::helpers::{register_game_pause_hooks, GamePauseHooks, TheGameLogic, TheScriptEngine};
use log::info;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

/// Trait implemented by the real in-game UI layer so the legacy
/// `TheInGameUI::*` entry points can forward requests to the modern
/// Rust subsystem.
pub trait InGameUiHooks: Send + Sync {
    fn select_area(&self, upper_left: ICoord2D, lower_right: ICoord2D);
    fn issue_move_command(&self, position: Coord3D, queue: bool);
    fn issue_force_attack_ground(&self, position: Coord3D);
    fn issue_attack_command(&self, target: u32, queue: bool);
    fn issue_stop_command(&self);
    fn set_hint_text(&self, hint: &str);
    fn get_pending_place_template(&self) -> Option<String>;
    fn get_pending_place_source_object_id(&self) -> u32;
    fn set_pending_place(&self, template_name: Option<String>, source_object_id: Option<u32>);
    fn get_pending_special_power(&self) -> Option<PendingSpecialPower>;
    fn set_pending_special_power(&self, pending: Option<PendingSpecialPower>);
    fn clear_pending_special_power(&self);
    fn get_pending_command(&self) -> Option<PendingCommand>;
    fn set_pending_command(&self, pending: Option<PendingCommand>);
    fn clear_pending_command(&self);
    fn is_placement_anchored(&self) -> bool;
    fn set_placement_start(&self, start: Option<ICoord2D>);
    fn set_placement_end(&self, end: Option<ICoord2D>);
    fn get_placement_points(&self) -> Option<(ICoord2D, ICoord2D)>;
    fn get_placement_angle(&self) -> f32;
    fn set_placement_angle(&self, angle: f32);
    fn set_radius_cursor_active(&self, _radius_cursor_type: Option<String>) {}
    fn set_radius_cursor_none(&self);
    fn display_cant_build_message(&self, message: &str);
    fn message(&self, text: &str);
    fn military_subtitle(&self, label: &str, _duration_ms: i32) {
        self.message(label);
    }
    fn clear_attack_move_to_mode(&self);
    fn is_in_attack_move_to_mode(&self) -> bool;
    fn set_attack_move_to_mode(&self, enabled: bool);
    fn is_in_force_attack_mode(&self) -> bool;
    fn is_in_force_move_to_mode(&self) -> bool;
    fn is_in_prefer_selection_mode(&self) -> bool;
    fn set_force_attack_mode(&self, enabled: bool);
    fn set_force_move_to_mode(&self, enabled: bool);
    fn set_prefer_selection_mode(&self, enabled: bool);
    fn set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
        &self,
        _enabled: bool,
    ) {
    }
    fn get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(&self) -> bool {
        false
    }

    fn play_movie(&self, _movie_name: &str) -> bool {
        // Optional backend-specific playback (e.g., radar/in-game movie windows).
        false
    }

    fn is_movie_playing(&self, _movie_name: &str) -> bool {
        false
    }
}

fn backend_slot() -> &'static Mutex<Option<Arc<dyn InGameUiHooks>>> {
    static BACKEND: OnceLock<Mutex<Option<Arc<dyn InGameUiHooks>>>> = OnceLock::new();
    BACKEND.get_or_init(|| Mutex::new(None))
}

pub fn register_in_game_ui_backend(hooks: Arc<dyn InGameUiHooks>) {
    let mut slot = backend_slot()
        .lock()
        .expect("In-game UI backend lock poisoned");
    *slot = Some(hooks);
}

fn with_backend<F>(f: F) -> bool
where
    F: FnOnce(&dyn InGameUiHooks),
{
    let backend = {
        let slot = backend_slot()
            .lock()
            .expect("In-game UI backend lock poisoned");
        slot.clone()
    };
    if let Some(handler) = backend {
        f(handler.as_ref());
        true
    } else {
        false
    }
}

fn with_backend_result<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&dyn InGameUiHooks) -> R,
{
    let backend = {
        let slot = backend_slot()
            .lock()
            .expect("In-game UI backend lock poisoned");
        slot.clone()
    };
    backend.map(|handler| f(handler.as_ref()))
}

fn mouse_backend_slot() -> &'static Mutex<Option<Arc<Mutex<Mouse>>>> {
    static BACKEND: OnceLock<Mutex<Option<Arc<Mutex<Mouse>>>>> = OnceLock::new();
    BACKEND.get_or_init(|| Mutex::new(None))
}

static MOUSE_CURSOR_VISIBLE: AtomicBool = AtomicBool::new(true);

pub fn register_mouse_backend(mouse: Arc<Mutex<Mouse>>) {
    let visible = MOUSE_CURSOR_VISIBLE.load(Ordering::Relaxed);
    let mut slot = mouse_backend_slot()
        .lock()
        .expect("Mouse backend lock poisoned");
    *slot = Some(mouse);

    if let Some(mouse) = slot.as_ref() {
        if let Ok(mut mouse) = mouse.lock() {
            mouse.set_cursor_visible(visible);
        }
    }
}

pub fn set_mouse_cursor_visibility(visible: bool) {
    MOUSE_CURSOR_VISIBLE.store(visible, Ordering::Relaxed);
    let backend = {
        let slot = mouse_backend_slot()
            .lock()
            .expect("Mouse backend lock poisoned");
        slot.clone()
    };

    if let Some(mouse) = backend {
        if let Ok(mut mouse) = mouse.lock() {
            mouse.set_cursor_visible(visible);
        }
    }
}

pub fn is_mouse_cursor_visible() -> bool {
    let backend = {
        let slot = mouse_backend_slot()
            .lock()
            .expect("Mouse backend lock poisoned");
        slot.clone()
    };

    if let Some(mouse) = backend {
        if let Ok(mouse) = mouse.lock() {
            return mouse.state().is_cursor_visible();
        }
    }

    MOUSE_CURSOR_VISIBLE.load(Ordering::Relaxed)
}

/// Trait implemented by the control bar layer so legacy control bar calls can
/// forward into the Rust UI stack.
pub trait ControlBarHooks: Send + Sync {
    fn hide_purchase_science(&self);
    fn process_context_sensitive_button_click(&self, control_id: u32, msg: u32);
    fn process_context_sensitive_button_transition(&self, control_id: u32, entering: bool);
    fn toggle_purchase_science(&self);
    fn show_special_power_shortcut(&self);
    fn hide_special_power_shortcut(&self);
    fn animate_special_power_shortcut(&self, enabled: bool);
    fn toggle_control_bar_stage(&self);
    fn get_observer_look_at_player_index(&self) -> Option<i32>;
}

fn control_bar_backend_slot() -> &'static Mutex<Option<Arc<dyn ControlBarHooks>>> {
    static BACKEND: OnceLock<Mutex<Option<Arc<dyn ControlBarHooks>>>> = OnceLock::new();
    BACKEND.get_or_init(|| Mutex::new(None))
}

pub fn register_control_bar_backend(hooks: Arc<dyn ControlBarHooks>) {
    let mut slot = control_bar_backend_slot()
        .lock()
        .expect("Control bar backend lock poisoned");
    *slot = Some(hooks);
}

fn with_control_bar_backend<F>(f: F) -> bool
where
    F: FnOnce(&dyn ControlBarHooks),
{
    let backend = {
        let slot = control_bar_backend_slot()
            .lock()
            .expect("Control bar backend lock poisoned");
        slot.clone()
    };
    if let Some(handler) = backend {
        f(handler.as_ref());
        true
    } else {
        false
    }
}

fn with_control_bar_backend_result<R, F>(f: F) -> Option<R>
where
    F: FnOnce(&dyn ControlBarHooks) -> R,
{
    let backend = {
        let slot = control_bar_backend_slot()
            .lock()
            .expect("Control bar backend lock poisoned");
        slot.clone()
    };
    backend.map(|handler| f(handler.as_ref()))
}

struct GameClientPrepareNewGameHooks;

impl gamelogic::helpers::PrepareNewGameHooks for GameClientPrepareNewGameHooks {
    fn ensure_background_window(&self) {
        let layout_slot = background_layout_slot();
        let existing = layout_slot
            .lock()
            .expect("Background layout lock poisoned")
            .clone();
        if let Some(layout) = existing {
            layout.borrow_mut().hide(false);
            layout.borrow_mut().bring_forward();
            if let Some(window) = layout.borrow().get_first_window() {
                window.borrow_mut().clear_status(WindowStatus::IMAGE);
            }
            return;
        }
        let new_layout = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("Menus/BlankWindow.wnd")
                .ok()
                .map(|(layout, _)| layout)
        });
        if let Some(layout) = new_layout {
            layout.borrow_mut().hide(false);
            layout.borrow_mut().bring_forward();
            if let Some(window) = layout.borrow().get_first_window() {
                window.borrow_mut().clear_status(WindowStatus::IMAGE);
            }
            let mut slot = layout_slot.lock().expect("Background layout lock poisoned");
            *slot = Some(layout);
        }
    }

    fn hide_shell(&self) {
        let _ = get_shell().hide_shell();
    }
}

thread_local! {
    static BACKGROUND_LAYOUT_SLOT: Arc<Mutex<Option<Rc<RefCell<WindowLayout>>>>> =
        Arc::new(Mutex::new(None));
}

fn background_layout_slot() -> Arc<Mutex<Option<Rc<RefCell<WindowLayout>>>>> {
    BACKGROUND_LAYOUT_SLOT.with(|slot| slot.clone())
}

pub fn register_prepare_new_game_hooks() {
    let _ = gamelogic::helpers::register_prepare_new_game_hooks(Arc::new(
        GameClientPrepareNewGameHooks,
    ));
    let _ = register_game_pause_hooks(Arc::new(GameClientPauseHooks));
}

struct GameClientObserverAudioLocalityHooks;

impl gamelogic::helpers::ObserverAudioLocalityHooks for GameClientObserverAudioLocalityHooks {
    fn get_observer_look_at_player_index(&self) -> Option<i32> {
        TheControlBar::get_observer_look_at_player_index()
    }
}

pub fn register_observer_audio_locality_hooks() {
    let _ = gamelogic::helpers::register_observer_audio_locality_hooks(Arc::new(
        GameClientObserverAudioLocalityHooks,
    ));
}

struct GameClientObserverAudioViewHooks;

impl gamelogic::helpers::ObserverAudioViewHooks for GameClientObserverAudioViewHooks {
    fn get_tactical_view_position(&self) -> Option<(f32, f32, f32)> {
        Some(with_tactical_view_ref(|view| {
            let pos = view.position();
            (pos.x, pos.y, pos.z)
        }))
    }

    fn get_tactical_view_angle(&self) -> Option<f32> {
        Some(with_tactical_view_ref(|view| view.angle()))
    }

    fn get_3d_camera_position(&self) -> Option<(f32, f32, f32)> {
        Some(with_tactical_view_ref(|view| {
            let camera = view.get_3d_camera_position();
            (camera.x, camera.y, camera.z)
        }))
    }
}

pub fn register_observer_audio_view_hooks() {
    let _ =
        gamelogic::helpers::register_observer_audio_view_hooks(Arc::new(GameClientObserverAudioViewHooks));
}

#[derive(Debug, Clone, Copy)]
struct PauseTransitionState {
    input_enabled_memory: bool,
    mouse_visible_memory: bool,
}

impl Default for PauseTransitionState {
    fn default() -> Self {
        Self {
            input_enabled_memory: true,
            mouse_visible_memory: true,
        }
    }
}

fn pause_transition_state() -> &'static Mutex<PauseTransitionState> {
    static STATE: OnceLock<Mutex<PauseTransitionState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(PauseTransitionState::default()))
}

struct GameClientPauseHooks;

impl GamePauseHooks for GameClientPauseHooks {
    fn on_game_pause_state_changed(&self, paused: bool) {
        let (input_enabled_memory, mouse_visible_memory) = {
            let mut state = pause_transition_state()
                .lock()
                .expect("Pause transition state lock poisoned");
            if paused {
                state.input_enabled_memory = TheInGameUI::get_input_enabled();
                state.mouse_visible_memory = is_mouse_cursor_visible();
            }
            (state.input_enabled_memory, state.mouse_visible_memory)
        };

        if paused {
            set_mouse_cursor_visibility(true);
            TheInGameUI::set_cursor_arrow();
            if input_enabled_memory {
                TheInGameUI::set_input_enabled(false);
            }
        } else {
            set_mouse_cursor_visibility(mouse_visible_memory);
            if input_enabled_memory {
                TheInGameUI::set_input_enabled(true);
            }
        }
    }
}

#[derive(Debug, Clone)]
struct InGameUIPlacementState {
    pending_template: Option<String>,
    pending_source_object_id: u32,
    placement_start: Option<ICoord2D>,
    placement_end: Option<ICoord2D>,
    placement_angle: f32,
    radius_cursor_active: bool,
    radius_cursor_type: String,
    attack_move_to_mode: bool,
    force_attack_mode: bool,
    force_move_to_mode: bool,
    prefer_selection_mode: bool,
    pending_special_power: Option<PendingSpecialPower>,
    pending_command: Option<PendingCommand>,
}

impl Default for InGameUIPlacementState {
    fn default() -> Self {
        Self {
            pending_template: None,
            pending_source_object_id: 0,
            placement_start: None,
            placement_end: None,
            placement_angle: 0.0,
            radius_cursor_active: false,
            radius_cursor_type: String::new(),
            attack_move_to_mode: false,
            force_attack_mode: false,
            force_move_to_mode: false,
            prefer_selection_mode: false,
            pending_special_power: None,
            pending_command: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PendingSpecialPower {
    pub power_id: u32,
    pub options: u32,
    pub source_object_id: u32,
}

#[derive(Debug, Clone)]
pub struct PendingCommand {
    pub command_type: gamelogic::commands::command::CommandType,
    pub options: u32,
    pub source_object_id: u32,
    pub cursor_name: String,
    pub invalid_cursor_name: String,
    pub radius_cursor_type: String,
}

#[derive(Debug, Clone)]
pub struct PopupMessageData {
    pub message: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub text_color: u32,
    pub pause: bool,
    pub pause_music: bool,
    pub layout: Option<Rc<RefCell<WindowLayout>>>,
}

#[derive(Default)]
struct PopupMessageState {
    data: Option<PopupMessageData>,
}

thread_local! {
    static POPUP_MESSAGE_STATE: Arc<Mutex<PopupMessageState>> =
        Arc::new(Mutex::new(PopupMessageState::default()));
}

fn popup_message_state() -> Arc<Mutex<PopupMessageState>> {
    POPUP_MESSAGE_STATE.with(|state| state.clone())
}

#[derive(Debug, Clone, Copy)]
enum CursorType {
    Arrow,
    Cross,
    Selecting,
    MoveTo,
    AttackMoveTo,
    Waypoint,
    AttackObject,
    OutRange,
    ForceAttackObject,
    ForceAttackGround,
    GetRepaired,
    Dock,
    GetHealed,
    DoRepair,
    ResumeConstruction,
    EnterFriendly,
    EnterAggressively,
    Defector,
    CaptureBuilding,
    Hack,
    GenericInvalid,
    SetRallyPoint,
    ParticleUplinkCannon,
}

#[derive(Debug)]
struct InGameUIStatusState {
    quit_menu_visible: bool,
    input_enabled: bool,
    client_quiet: bool,
    selecting: bool,
    scrolling: bool,
    scroll_amount_x: f32,
    scroll_amount_y: f32,
    cursor: CursorType,
    prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click: bool,
}

impl Default for InGameUIStatusState {
    fn default() -> Self {
        Self {
            quit_menu_visible: false,
            input_enabled: true,
            client_quiet: false,
            selecting: false,
            scrolling: false,
            scroll_amount_x: 0.0,
            scroll_amount_y: 0.0,
            cursor: CursorType::Arrow,
            prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click: false,
        }
    }
}

fn in_game_ui_status_state() -> &'static Mutex<InGameUIStatusState> {
    static STATE: OnceLock<Mutex<InGameUIStatusState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(InGameUIStatusState::default()))
}

fn fallback_placement_state() -> &'static Mutex<InGameUIPlacementState> {
    static STATE: OnceLock<Mutex<InGameUIPlacementState>> = OnceLock::new();
    STATE.get_or_init(|| Mutex::new(InGameUIPlacementState::default()))
}

fn map_cant_build_message_key(message: &str) -> String {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return "GUI:CantBuildThere".to_string();
    }
    if trimmed.starts_with("GUI:") {
        return trimmed.to_string();
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.contains("flat") {
        "GUI:CantBuildNotFlatEnough".to_string()
    } else if lower.contains("object") {
        "GUI:CantBuildObjectsInTheWay".to_string()
    } else if lower.contains("supply") {
        "GUI:CantBuildTooCloseToSupplies".to_string()
    } else if lower.contains("path") {
        "GUI:CantBuildNoClearPath".to_string()
    } else if lower.contains("shroud") || lower.contains("visible") {
        "GUI:CantBuildShroud".to_string()
    } else if lower.contains("terrain")
        || lower.contains("cliff")
        || lower.contains("underwater")
        || lower.contains("bridge")
    {
        "GUI:CantBuildRestrictedTerrain".to_string()
    } else {
        "GUI:CantBuildThere".to_string()
    }
}

/// Minimal stand‑in for the classic `TheInGameUI` singleton.
///
/// The real client would translate these calls into message stream operations
/// and UI updates.  For now we simply emit trace information so that the
/// message translators remain close to their C++ counterparts.
pub struct TheInGameUI;

impl TheInGameUI {
    fn set_cursor(cursor: CursorType) {
        let mut guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.cursor = cursor;
    }

    fn cursor_from_name(name: &str) -> CursorType {
        match name {
            "ARROW" => CursorType::Arrow,
            "CROSS" => CursorType::Cross,
            "SELECTING" => CursorType::Selecting,
            "MOVETO" => CursorType::MoveTo,
            "ATTACKMOVETO" => CursorType::AttackMoveTo,
            "WAYPOINT" => CursorType::Waypoint,
            "ATTACK_OBJECT" => CursorType::AttackObject,
            "OUTRANGE" => CursorType::OutRange,
            "FORCE_ATTACK_OBJECT" => CursorType::ForceAttackObject,
            "FORCE_ATTACK_GROUND" => CursorType::ForceAttackGround,
            "GET_REPAIRED" => CursorType::GetRepaired,
            "DOCK" => CursorType::Dock,
            "GET_HEALED" => CursorType::GetHealed,
            "DO_REPAIR" => CursorType::DoRepair,
            "RESUME_CONSTRUCTION" => CursorType::ResumeConstruction,
            "ENTER_FRIENDLY" => CursorType::EnterFriendly,
            "ENTER_AGGRESSIVELY" => CursorType::EnterAggressively,
            "DEFECTOR" => CursorType::Defector,
            "CAPTUREBUILDING" => CursorType::CaptureBuilding,
            "HACK" => CursorType::Hack,
            "GENERIC_INVALID" => CursorType::GenericInvalid,
            "SET_RALLY_POINT" => CursorType::SetRallyPoint,
            "PARTICLE_UPLINK_CANNON" => CursorType::ParticleUplinkCannon,
            _ => CursorType::Arrow,
        }
    }

    fn cursor_name(cursor: CursorType) -> &'static str {
        match cursor {
            CursorType::Arrow => "ARROW",
            CursorType::Cross => "CROSS",
            CursorType::Selecting => "SELECTING",
            CursorType::MoveTo => "MOVETO",
            CursorType::AttackMoveTo => "ATTACKMOVETO",
            CursorType::Waypoint => "WAYPOINT",
            CursorType::AttackObject => "ATTACK_OBJECT",
            CursorType::OutRange => "OUTRANGE",
            CursorType::ForceAttackObject => "FORCE_ATTACK_OBJECT",
            CursorType::ForceAttackGround => "FORCE_ATTACK_GROUND",
            CursorType::GetRepaired => "GET_REPAIRED",
            CursorType::Dock => "DOCK",
            CursorType::GetHealed => "GET_HEALED",
            CursorType::DoRepair => "DO_REPAIR",
            CursorType::ResumeConstruction => "RESUME_CONSTRUCTION",
            CursorType::EnterFriendly => "ENTER_FRIENDLY",
            CursorType::EnterAggressively => "ENTER_AGGRESSIVELY",
            CursorType::Defector => "DEFECTOR",
            CursorType::CaptureBuilding => "CAPTUREBUILDING",
            CursorType::Hack => "HACK",
            CursorType::GenericInvalid => "GENERIC_INVALID",
            CursorType::SetRallyPoint => "SET_RALLY_POINT",
            CursorType::ParticleUplinkCannon => "PARTICLE_UPLINK_CANNON",
        }
    }

    pub fn select_area(upper_left: ICoord2D, lower_right: ICoord2D) {
        if !with_backend(|backend| backend.select_area(upper_left.clone(), lower_right.clone())) {
            info!(
                "Selecting area from ({}, {}) to ({}, {})",
                upper_left.x, upper_left.y, lower_right.x, lower_right.y
            );
        }
    }

    pub fn issue_move_command(position: Coord3D, queue: bool) {
        if !with_backend(|backend| backend.issue_move_command(position.clone(), queue)) {
            info!(
                "Issuing move command to ({:.1}, {:.1}, {:.1}) queued={}",
                position.x, position.y, position.z, queue
            );
        }
    }

    pub fn issue_force_attack_ground(position: Coord3D) {
        if !with_backend(|backend| backend.issue_force_attack_ground(position.clone())) {
            info!(
                "Issuing force attack ground at ({:.1}, {:.1}, {:.1})",
                position.x, position.y, position.z
            );
        }
    }

    pub fn issue_attack_command(target: u32, queue: bool) {
        if !with_backend(|backend| backend.issue_attack_command(target, queue)) {
            info!(
                "Issuing attack command on target {} queued={}",
                target, queue
            );
        }
    }

    pub fn issue_stop_command() {
        if !with_backend(|backend| backend.issue_stop_command()) {
            info!("Issuing stop command");
        }
    }

    pub fn set_hint_text(hint: &str) {
        if !with_backend(|backend| backend.set_hint_text(hint)) {
            info!("Hint: {}", hint);
        }
    }

    pub fn set_quit_menu_visible(visible: bool) {
        let mut guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.quit_menu_visible = visible;
    }

    pub fn is_quit_menu_visible() -> bool {
        let guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.quit_menu_visible
    }

    pub fn get_input_enabled() -> bool {
        let guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.input_enabled && gamelogic::helpers::TheGameLogic::is_input_enabled()
    }

    pub fn set_input_enabled(enabled: bool) {
        let mut guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.input_enabled = enabled;
        if !enabled {
            guard.scrolling = false;
            guard.scroll_amount_x = 0.0;
            guard.scroll_amount_y = 0.0;
        }
        gamelogic::helpers::TheGameLogic::set_input_enabled(enabled);
    }

    pub fn is_selecting() -> bool {
        let guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.selecting
    }

    pub fn set_selecting(selecting: bool) {
        let mut guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.selecting = selecting;
    }

    pub fn set_scrolling(scrolling: bool) {
        let mut guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.scrolling = scrolling;
        if !scrolling {
            guard.scroll_amount_x = 0.0;
            guard.scroll_amount_y = 0.0;
        }
    }

    pub fn is_scrolling() -> bool {
        let guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.scrolling
    }

    pub fn set_scroll_amount(x: f32, y: f32) {
        let mut guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.scroll_amount_x = x;
        guard.scroll_amount_y = y;
    }

    pub fn get_scroll_amount() -> (f32, f32) {
        let guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        (guard.scroll_amount_x, guard.scroll_amount_y)
    }

    pub fn set_client_quiet(quiet: bool) {
        let mut guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.client_quiet = quiet;
    }

    pub fn is_client_quiet() -> bool {
        let guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.client_quiet
    }

    pub fn set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
        enabled: bool,
    ) {
        if with_backend(|backend| {
            backend.set_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click(
                enabled,
            )
        }) {
            return;
        }
        let mut guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click = enabled;
    }

    pub fn get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click() -> bool {
        if let Some(value) = with_backend_result(|backend| {
            backend.get_prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click()
        }) {
            return value;
        }
        let guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        guard.prevent_left_click_deselection_in_alternate_mouse_mode_for_one_click
    }

    pub fn set_cursor_arrow() {
        Self::set_cursor(CursorType::Arrow);
    }

    pub fn set_cursor_by_name(cursor: &str) {
        Self::set_cursor(Self::cursor_from_name(cursor));
    }

    pub fn get_cursor_name() -> &'static str {
        let guard = in_game_ui_status_state()
            .lock()
            .expect("In-game UI status lock poisoned");
        Self::cursor_name(guard.cursor)
    }

    pub fn set_radius_cursor_active() {
        if with_backend(|backend| backend.set_radius_cursor_active(None)) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.radius_cursor_active = true;
        guard.radius_cursor_type.clear();
    }

    pub fn set_radius_cursor_active_with_type(radius_cursor_type: &str) {
        if with_backend(|backend| {
            backend.set_radius_cursor_active(Some(radius_cursor_type.to_string()))
        }) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        let radius_type = radius_cursor_type.trim();
        guard.radius_cursor_active =
            !radius_type.is_empty() && !radius_type.eq_ignore_ascii_case("NONE");
        guard.radius_cursor_type = radius_type.to_string();
    }

    pub fn get_pending_place_template() -> Option<String> {
        if let Some(value) = with_backend_result(|backend| backend.get_pending_place_template()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_template.clone()
    }

    pub fn get_pending_place_source_object_id() -> u32 {
        if let Some(value) =
            with_backend_result(|backend| backend.get_pending_place_source_object_id())
        {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_source_object_id
    }

    pub fn place_build_available(template_name: Option<String>, source_object_id: Option<u32>) {
        if with_backend(|backend| {
            backend.set_pending_place(template_name.clone(), source_object_id)
        }) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_template = template_name;
        guard.pending_source_object_id = source_object_id.unwrap_or(0);
        guard.placement_start = None;
        guard.placement_end = None;
        guard.placement_angle = 0.0;
    }

    pub fn get_pending_special_power() -> Option<PendingSpecialPower> {
        if let Some(value) = with_backend_result(|backend| backend.get_pending_special_power()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_special_power.clone()
    }

    pub fn set_pending_special_power(power_id: u32, options: u32, source_object_id: u32) {
        let pending = PendingSpecialPower {
            power_id,
            options,
            source_object_id,
        };
        if with_backend(|backend| backend.set_pending_special_power(Some(pending.clone()))) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_special_power = Some(pending);
    }

    pub fn clear_pending_special_power() {
        if with_backend(|backend| backend.clear_pending_special_power()) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_special_power = None;
    }

    pub fn get_pending_command() -> Option<PendingCommand> {
        if let Some(value) = with_backend_result(|backend| backend.get_pending_command()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_command.clone()
    }

    pub fn set_pending_command(
        command_type: gamelogic::commands::command::CommandType,
        options: u32,
        source_object_id: u32,
    ) {
        let pending = PendingCommand {
            command_type,
            options,
            source_object_id,
            cursor_name: String::new(),
            invalid_cursor_name: String::new(),
            radius_cursor_type: String::new(),
        };
        if with_backend(|backend| backend.set_pending_command(Some(pending.clone()))) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_command = Some(pending);
    }

    pub fn set_pending_command_with_visual(
        command_type: gamelogic::commands::command::CommandType,
        options: u32,
        source_object_id: u32,
        cursor_name: String,
        invalid_cursor_name: String,
        radius_cursor_type: String,
    ) {
        let pending = PendingCommand {
            command_type,
            options,
            source_object_id,
            cursor_name,
            invalid_cursor_name,
            radius_cursor_type,
        };
        if with_backend(|backend| backend.set_pending_command(Some(pending.clone()))) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_command = Some(pending);
    }

    pub fn clear_pending_command() {
        if with_backend(|backend| backend.clear_pending_command()) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.pending_command = None;
    }

    pub fn is_placement_anchored() -> bool {
        if let Some(value) = with_backend_result(|backend| backend.is_placement_anchored()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.placement_start.is_some()
    }

    pub fn set_placement_start(start: Option<ICoord2D>) {
        if with_backend(|backend| backend.set_placement_start(start.clone())) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.placement_start = start.clone();
        if start.is_none() {
            guard.placement_end = None;
        } else if guard.placement_end.is_none() {
            guard.placement_end = start;
        }
    }

    pub fn set_placement_end(end: Option<ICoord2D>) {
        if with_backend(|backend| backend.set_placement_end(end.clone())) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.placement_end = end;
    }

    pub fn get_placement_points() -> Option<(ICoord2D, ICoord2D)> {
        if let Some(value) = with_backend_result(|backend| backend.get_placement_points()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        let start = guard.placement_start.clone()?;
        let end = guard.placement_end.clone().unwrap_or_else(|| start.clone());
        Some((start, end))
    }

    pub fn get_placement_angle() -> f32 {
        if let Some(value) = with_backend_result(|backend| backend.get_placement_angle()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.placement_angle
    }

    pub fn set_placement_angle(angle: f32) {
        if with_backend(|backend| backend.set_placement_angle(angle)) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.placement_angle = angle;
    }

    pub fn set_radius_cursor_none() {
        if with_backend(|backend| backend.set_radius_cursor_none()) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.radius_cursor_active = false;
        guard.radius_cursor_type.clear();
    }

    pub fn display_cant_build_message(message: &str) {
        if !with_backend(|backend| backend.display_cant_build_message(message)) {
            let key = map_cant_build_message_key(message);
            info!("Can't build: {}", GameText::fetch(&key));
        }
    }

    pub fn message(text: &str) {
        if !with_backend(|backend| backend.message(text)) {
            info!("UI message: {}", GameText::fetch(text));
        }
    }

    pub fn play_movie(movie_name: &str) -> bool {
        if let Some(started) = with_backend_result(|backend| backend.play_movie(movie_name)) {
            if !started {
                info!("Play movie request failed: {movie_name}");
            }
            return started;
        }

        info!("Play movie request without active backend: {movie_name}");
        false
    }

    pub fn is_movie_playing(movie_name: &str) -> bool {
        with_backend_result(|backend| backend.is_movie_playing(movie_name)).unwrap_or(false)
    }

    pub fn military_subtitle(label: &str, duration_ms: i32) {
        if !with_backend(|backend| backend.military_subtitle(label, duration_ms)) {
            info!(
                "Military subtitle ({}ms): {}",
                duration_ms.max(0),
                GameText::fetch(label)
            );
        }
    }

    pub fn clear_attack_move_to_mode() {
        if with_backend(|backend| backend.clear_attack_move_to_mode()) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.attack_move_to_mode = false;
    }

    pub fn is_in_attack_move_to_mode() -> bool {
        if let Some(value) = with_backend_result(|backend| backend.is_in_attack_move_to_mode()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.attack_move_to_mode
    }

    pub fn set_attack_move_to_mode(enabled: bool) {
        if with_backend(|backend| backend.set_attack_move_to_mode(enabled)) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.attack_move_to_mode = enabled;
    }

    pub fn toggle_attack_move_to_mode() -> bool {
        let enabled = !Self::is_in_attack_move_to_mode();
        Self::set_attack_move_to_mode(enabled);
        enabled
    }

    pub fn is_in_force_attack_mode() -> bool {
        if let Some(value) = with_backend_result(|backend| backend.is_in_force_attack_mode()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.force_attack_mode
    }

    pub fn is_in_force_move_to_mode() -> bool {
        if let Some(value) = with_backend_result(|backend| backend.is_in_force_move_to_mode()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.force_move_to_mode
    }

    pub fn is_in_prefer_selection_mode() -> bool {
        if let Some(value) = with_backend_result(|backend| backend.is_in_prefer_selection_mode()) {
            return value;
        }
        let guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.prefer_selection_mode
    }

    pub fn set_force_attack_mode(enabled: bool) {
        if with_backend(|backend| backend.set_force_attack_mode(enabled)) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.force_attack_mode = enabled;
    }

    pub fn set_force_move_to_mode(enabled: bool) {
        if with_backend(|backend| backend.set_force_move_to_mode(enabled)) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.force_move_to_mode = enabled;
    }

    pub fn set_prefer_selection_mode(enabled: bool) {
        if with_backend(|backend| backend.set_prefer_selection_mode(enabled)) {
            return;
        }
        let mut guard = fallback_placement_state()
            .lock()
            .expect("In-game UI placement state lock poisoned");
        guard.prefer_selection_mode = enabled;
    }

    pub fn popup_message(
        identifier: &str,
        x: i32,
        y: i32,
        width: i32,
        pause: bool,
        pause_music: bool,
    ) {
        Self::popup_message_with_color(identifier, x, y, width, 0xFFFFFFFF, pause, pause_music);
    }

    pub fn popup_message_with_color(
        identifier: &str,
        x: i32,
        y: i32,
        width: i32,
        text_color: u32,
        pause: bool,
        pause_music: bool,
    ) {
        Self::clear_popup_message_data();

        let message = GameText::fetch(identifier);
        let x_percent = x.clamp(0, 100);
        let y_percent = y.clamp(0, 100);

        let (screen_w, screen_h) = with_window_manager(|manager| manager.screen_size());
        let x_screen = (screen_w as f32 * (x_percent as f32 / 100.0)) as i32;
        let y_screen = (screen_h as f32 * (y_percent as f32 / 100.0)) as i32;
        let width = width.max(50);

        if pause {
            TheGameLogic::set_game_paused(true, pause_music);
        }

        let layout = with_window_manager(|manager| {
            manager
                .create_layout_with_windows("InGamePopupMessage.wnd")
                .ok()
                .map(|(layout, _)| layout)
        });

        let data = PopupMessageData {
            message,
            x: x_screen,
            y: y_screen,
            width,
            text_color,
            pause,
            pause_music,
            layout: layout.clone(),
        };

        {
            let state_handle = popup_message_state();
            let mut state = state_handle
                .lock()
                .expect("popup message state lock poisoned");
            state.data = Some(data);
        }

        if let Some(layout) = layout {
            layout.borrow().run_init(None);
        }
    }

    pub fn get_popup_message_data() -> Option<PopupMessageData> {
        let state_handle = popup_message_state();
        state_handle
            .lock()
            .ok()
            .and_then(|state| state.data.clone())
    }

    pub fn clear_popup_message_data() {
        let data = {
            let state_handle = popup_message_state();
            let mut state = state_handle
                .lock()
                .expect("popup message state lock poisoned");
            state.data.take()
        };

        let Some(data) = data else {
            return;
        };

        if let Some(layout) = data.layout {
            with_window_manager(|manager| {
                manager.destroy_layout(&layout);
            });
        }

        if data.pause {
            TheGameLogic::set_game_paused(false, data.pause_music);
        }
    }
}

/// Minimal stand-in for classic `TheControlBar` singleton.
pub struct TheControlBar;

impl TheControlBar {
    pub fn hide_purchase_science() {
        if !with_control_bar_backend(|backend| backend.hide_purchase_science()) {
            info!("Request to hide purchase science UI");
        }
    }

    pub fn process_context_sensitive_button_click(control_id: u32, msg: u32) {
        if !with_control_bar_backend(|backend| {
            backend.process_context_sensitive_button_click(control_id, msg)
        }) {
            info!("Process context-sensitive control bar click on {control_id} msg={msg}");
        }
    }

    pub fn process_context_sensitive_button_transition(control_id: u32, entering: bool) {
        if !with_control_bar_backend(|backend| {
            backend.process_context_sensitive_button_transition(control_id, entering)
        }) {
            if entering {
                info!("Control bar button {control_id} mouse enter");
            } else {
                info!("Control bar button {control_id} mouse leave");
            }
        }
    }

    pub fn toggle_purchase_science() {
        if !with_control_bar_backend(|backend| backend.toggle_purchase_science()) {
            info!("Request to toggle purchase science UI");
        }
    }

    pub fn show_special_power_shortcut() {
        if !with_control_bar_backend(|backend| backend.show_special_power_shortcut()) {
            info!("Request to show special power shortcut UI");
        }
    }

    pub fn hide_special_power_shortcut() {
        if !with_control_bar_backend(|backend| backend.hide_special_power_shortcut()) {
            info!("Request to hide special power shortcut UI");
        }
    }

    pub fn animate_special_power_shortcut(enabled: bool) {
        if !with_control_bar_backend(|backend| backend.animate_special_power_shortcut(enabled)) {
            info!("Request to animate special power shortcut UI: {enabled}");
        }
    }

    pub fn toggle_control_bar_stage() {
        if !with_control_bar_backend(|backend| backend.toggle_control_bar_stage()) {
            info!("Request to toggle control bar stage");
        }
    }

    pub fn get_observer_look_at_player_index() -> Option<i32> {
        with_control_bar_backend_result(|backend| backend.get_observer_look_at_player_index())
            .flatten()
    }
}
