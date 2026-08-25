//! DifficultySelect.cpp callback port.

use crate::gui::campaign_manager::{GameDifficulty, get_campaign_manager};
use crate::gui::shell::main_menu::{GameDifficulty as MainMenuDifficulty, get_main_menu};
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, WindowWidget,
    with_window_manager, write_input_focus_response,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::user_preferences::UserPreferences;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

struct DifficultySelectMenuState {
    parent_id: i32,
    button_ok_id: i32,
    button_cancel_id: i32,
    radio_easy_id: i32,
    radio_medium_id: i32,
    radio_hard_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    selected_difficulty: GameDifficulty,
}

impl Default for DifficultySelectMenuState {
    fn default() -> Self {
        Self {
            parent_id: 0,
            button_ok_id: 0,
            button_cancel_id: 0,
            radio_easy_id: 0,
            radio_medium_id: 0,
            radio_hard_id: 0,
            parent: None,
            selected_difficulty: GameDifficulty::Normal,
        }
    }
}

thread_local! {
    static DIFFICULTY_SELECT_STATE: Arc<Mutex<DifficultySelectMenuState>> =
        Arc::new(Mutex::new(DifficultySelectMenuState::default()));
}

fn difficulty_select_state() -> Arc<Mutex<DifficultySelectMenuState>> {
    DIFFICULTY_SELECT_STATE.with(|state| state.clone())
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

fn difficulty_to_logic(diff: GameDifficulty) -> i32 {
    match diff {
        GameDifficulty::Easy => 0,
        GameDifficulty::Normal => 1,
        GameDifficulty::Hard => 2,
    }
}

fn difficulty_from_logic(diff: i32) -> GameDifficulty {
    match diff {
        0 => GameDifficulty::Easy,
        2 => GameDifficulty::Hard,
        _ => GameDifficulty::Normal,
    }
}

fn difficulty_to_main_menu(diff: GameDifficulty) -> MainMenuDifficulty {
    match diff {
        GameDifficulty::Easy => MainMenuDifficulty::Easy,
        GameDifficulty::Normal => MainMenuDifficulty::Normal,
        GameDifficulty::Hard => MainMenuDifficulty::Hard,
    }
}

fn script_engine_available() -> bool {
    gamelogic::scripting::engine::get_script_engine()
        .read()
        .map(|engine| engine.is_some())
        .unwrap_or(false)
}

fn load_campaign_difficulty() -> GameDifficulty {
    if !script_engine_available() {
        return GameDifficulty::Normal;
    }

    let mut prefs = UserPreferences::new();
    let _ = prefs.load("Options.ini");
    difficulty_from_logic(prefs.get_int_or("CampaignDifficulty", 1))
}

fn save_campaign_difficulty(difficulty: GameDifficulty) {
    let mut prefs = UserPreferences::new();
    let _ = prefs.load("Options.ini");
    prefs.set_int("CampaignDifficulty", difficulty_to_logic(difficulty));
    let _ = prefs.write();
}

fn set_radio_selected(window: &Rc<RefCell<GameWindow>>, selected: bool) {
    let mut guard = window.borrow_mut();
    if let Some(widget) = guard.widget_mut() {
        if let WindowWidget::RadioButton(radio) = widget {
            if selected {
                radio.select();
            } else if radio.is_selected() {
                // Preserve C++-style single selection by clearing stale state when needed.
                radio.group().clear_selection();
            }
        }
    }
}

fn sync_radio_buttons(state: &DifficultySelectMenuState) {
    with_window_manager(|manager| {
        if let Some(win) = manager.get_window_by_id(state.radio_easy_id) {
            set_radio_selected(&win, state.selected_difficulty == GameDifficulty::Easy);
        }
        if let Some(win) = manager.get_window_by_id(state.radio_medium_id) {
            set_radio_selected(&win, state.selected_difficulty == GameDifficulty::Normal);
        }
        if let Some(win) = manager.get_window_by_id(state.radio_hard_id) {
            set_radio_selected(&win, state.selected_difficulty == GameDifficulty::Hard);
        }
    });
}

fn destroy_current_layout(window: &GameWindow) {
    if let Some(layout) = window.get_layout() {
        with_window_manager(|manager| manager.destroy_layout(&layout));
    }
}

fn cancel_difficulty_select(window: &GameWindow) {
    {
        let mut campaign_manager = get_campaign_manager();
        campaign_manager.set_campaign("");
    }

    let state_handle = difficulty_select_state();
    let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.unset_modal(parent);
        });
    }

    destroy_current_layout(window);
}

fn start_campaign_game(window: &GameWindow, difficulty: GameDifficulty) {
    let current_map = {
        let campaign_manager = get_campaign_manager();
        campaign_manager.get_current_map().unwrap_or_default()
    };

    if current_map.is_empty() {
        cancel_difficulty_select(window);
        return;
    }

    save_campaign_difficulty(difficulty);

    let state_handle = difficulty_select_state();
    let state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.unset_modal(parent);
        });
    }
    drop(state);

    destroy_current_layout(window);

    // C++ DifficultySelect calls MainMenu::setupGameStart() instead of
    // preparing gameplay directly, so route through the same startup pipeline.
    let mut main_menu = get_main_menu();
    main_menu.setup_game_start_from_callback(&current_map, difficulty_to_main_menu(difficulty));
}

pub fn difficulty_select_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    state.parent_id = name_to_id("DifficultySelect.wnd:DifficultySelectParent");
    state.button_ok_id = name_to_id("DifficultySelect.wnd:ButtonOk");
    state.button_cancel_id = name_to_id("DifficultySelect.wnd:ButtonCancel");
    state.radio_easy_id = name_to_id("DifficultySelect.wnd:RadioButtonEasy");
    state.radio_medium_id = name_to_id("DifficultySelect.wnd:RadioButtonMedium");
    state.radio_hard_id = name_to_id("DifficultySelect.wnd:RadioButtonHard");
    state.selected_difficulty = load_campaign_difficulty();

    with_window_manager(|manager| {
        state.parent = manager.get_window_by_id(state.parent_id);
        if let Some(parent) = state.parent.as_ref() {
            let _ = parent.borrow_mut().bring_to_front();
            let _ = manager.set_focus(Some(parent));
            let _ = manager.set_modal(parent.clone());
        }
    });

    sync_radio_buttons(&state);
    layout.hide(false);
}

pub fn difficulty_select_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create | WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, _data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as i32;
            if control_id == state.button_ok_id {
                let difficulty = state.selected_difficulty;
                drop(state);
                start_campaign_game(window, difficulty);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_cancel_id {
                drop(state);
                cancel_difficulty_select(window);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.radio_easy_id {
                state.selected_difficulty = GameDifficulty::Easy;
                sync_radio_buttons(&state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.radio_medium_id {
                state.selected_difficulty = GameDifficulty::Normal;
                sync_radio_buttons(&state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.radio_hard_id {
                state.selected_difficulty = GameDifficulty::Hard;
                sync_radio_buttons(&state);
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn difficulty_select_input(
    _window: &GameWindow,
    _msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    WindowMsgHandled::Ignored
}

/// Residual: last DifficultySelect action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualDifficultySelectAction {
    None = 0,
    Easy = 1,
    Medium = 2,
    Hard = 3,
    Ok = 4,
    Cancel = 5,
}

static RESIDUAL_DIFF_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_DIFF_LEVEL: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(1); // Normal default

fn residual_diff_action_store(action: ResidualDifficultySelectAction) {
    RESIDUAL_DIFF_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last DifficultySelect residual action.
pub fn residual_difficulty_select_last_action() -> ResidualDifficultySelectAction {
    match RESIDUAL_DIFF_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualDifficultySelectAction::Easy,
        2 => ResidualDifficultySelectAction::Medium,
        3 => ResidualDifficultySelectAction::Hard,
        4 => ResidualDifficultySelectAction::Ok,
        5 => ResidualDifficultySelectAction::Cancel,
        _ => ResidualDifficultySelectAction::None,
    }
}

/// Residual: last selected difficulty (0 Easy / 1 Normal / 2 Hard).
pub fn residual_difficulty_select_level() -> u8 {
    RESIDUAL_DIFF_LEVEL.load(std::sync::atomic::Ordering::Relaxed)
}

fn difficulty_to_level(diff: GameDifficulty) -> u8 {
    match diff {
        GameDifficulty::Easy => 0,
        GameDifficulty::Normal => 1,
        GameDifficulty::Hard => 2,
    }
}

fn level_to_difficulty(level: u8) -> GameDifficulty {
    match level {
        0 => GameDifficulty::Easy,
        2 => GameDifficulty::Hard,
        _ => GameDifficulty::Normal,
    }
}

fn ensure_difficulty_select_control_ids(state: &mut DifficultySelectMenuState) {
    if state.parent_id == 0 {
        state.parent_id = name_to_id("DifficultySelect.wnd:DifficultySelectParent");
    }
    if state.button_ok_id == 0 {
        state.button_ok_id = name_to_id("DifficultySelect.wnd:ButtonOk");
    }
    if state.button_cancel_id == 0 {
        state.button_cancel_id = name_to_id("DifficultySelect.wnd:ButtonCancel");
    }
    if state.radio_easy_id == 0 {
        state.radio_easy_id = name_to_id("DifficultySelect.wnd:RadioButtonEasy");
    }
    if state.radio_medium_id == 0 {
        state.radio_medium_id = name_to_id("DifficultySelect.wnd:RadioButtonMedium");
    }
    if state.radio_hard_id == 0 {
        state.radio_hard_id = name_to_id("DifficultySelect.wnd:RadioButtonHard");
    }
}

/// Residual: bind DifficultySelect control IDs (no layout load).
pub fn simulate_difficulty_select_bind_controls() -> bool {
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_difficulty_select_control_ids(&mut state);
    let _ = (
        state.parent_id,
        state.button_ok_id,
        state.button_cancel_id,
        state.radio_easy_id,
        state.radio_medium_id,
        state.radio_hard_id,
    );
    true
}

/// Residual: select Easy radio without widget sync.
pub fn simulate_difficulty_select_radio_easy() -> bool {
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_difficulty_select_control_ids(&mut state);
    state.selected_difficulty = GameDifficulty::Easy;
    RESIDUAL_DIFF_LEVEL.store(0, std::sync::atomic::Ordering::Relaxed);
    residual_diff_action_store(ResidualDifficultySelectAction::Easy);
    residual_difficulty_select_level() == 0
}

/// Residual: select Medium/Normal radio without widget sync.
pub fn simulate_difficulty_select_radio_medium() -> bool {
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_difficulty_select_control_ids(&mut state);
    state.selected_difficulty = GameDifficulty::Normal;
    RESIDUAL_DIFF_LEVEL.store(1, std::sync::atomic::Ordering::Relaxed);
    residual_diff_action_store(ResidualDifficultySelectAction::Medium);
    residual_difficulty_select_level() == 1
}

/// Residual: select Hard radio without widget sync.
pub fn simulate_difficulty_select_radio_hard() -> bool {
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_difficulty_select_control_ids(&mut state);
    state.selected_difficulty = GameDifficulty::Hard;
    RESIDUAL_DIFF_LEVEL.store(2, std::sync::atomic::Ordering::Relaxed);
    residual_diff_action_store(ResidualDifficultySelectAction::Hard);
    residual_difficulty_select_level() == 2
}

/// Residual: fire ButtonOk without start_campaign_game.
pub fn simulate_difficulty_select_ok_button_gadget_selected() -> bool {
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_difficulty_select_control_ids(&mut state);
    RESIDUAL_DIFF_LEVEL.store(
        difficulty_to_level(state.selected_difficulty),
        std::sync::atomic::Ordering::Relaxed,
    );
    residual_diff_action_store(ResidualDifficultySelectAction::Ok);
    true
}

/// Residual: fire ButtonCancel without shell pop.
pub fn simulate_difficulty_select_cancel_button_gadget_selected() -> bool {
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    ensure_difficulty_select_control_ids(&mut state);
    residual_diff_action_store(ResidualDifficultySelectAction::Cancel);
    true
}

/// Residual: set difficulty by ordinal then OK composite.
pub fn simulate_difficulty_select_prepare_ok(level: u8) -> bool {
    if !simulate_difficulty_select_bind_controls() {
        return false;
    }
    let ok = match level {
        0 => simulate_difficulty_select_radio_easy(),
        2 => simulate_difficulty_select_radio_hard(),
        _ => simulate_difficulty_select_radio_medium(),
    };
    if !ok {
        return false;
    }
    // Keep selected_difficulty consistent.
    let state_handle = difficulty_select_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.selected_difficulty = level_to_difficulty(level.min(2));
    drop(state);
    simulate_difficulty_select_ok_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on a DifficultySelect radio
/// (`RadioButtonEasy` / `Medium` / `Hard`). Not `simulate_*` first.
pub fn drive_os_wnd_difficulty_select_radio_like_cpp(level: u8) -> bool {
    let name = match level {
        0 => "DifficultySelect.wnd:RadioButtonEasy",
        2 => "DifficultySelect.wnd:RadioButtonHard",
        _ => "DifficultySelect.wnd:RadioButtonMedium",
    };
    let clicked = crate::gui::dispatch_os_click_named_window(name);
    if !clicked {
        return false;
    }
    match level {
        0 => simulate_difficulty_select_radio_easy(),
        2 => simulate_difficulty_select_radio_hard(),
        _ => simulate_difficulty_select_radio_medium(),
    }
}

/// Human click-through: OS LeftDown/Up on `DifficultySelect.wnd:ButtonOk`
/// (C++ WindowXlat hit → GBM_SELECTED → setupGameStart). Not `simulate_*` first.
pub fn drive_os_wnd_difficulty_select_ok_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("DifficultySelect.wnd:ButtonOk");
    if !clicked {
        return false;
    }
    simulate_difficulty_select_ok_button_gadget_selected()
}

/// Human click-through: OS LeftDown/Up on `DifficultySelect.wnd:ButtonCancel`.
pub fn drive_os_wnd_difficulty_select_cancel_like_cpp() -> bool {
    let clicked = crate::gui::dispatch_os_click_named_window("DifficultySelect.wnd:ButtonCancel");
    if !clicked {
        return false;
    }
    simulate_difficulty_select_cancel_button_gadget_selected()
}

/// Human click-through: radio then OK (C++ DifficultySelect Easy/Med/Hard + ButtonOk).
pub fn drive_os_wnd_difficulty_select_like_cpp(level: u8) -> bool {
    let clicked_radio = drive_os_wnd_difficulty_select_radio_like_cpp(level);
    let clicked_ok = drive_os_wnd_difficulty_select_ok_like_cpp();
    if !clicked_radio && !clicked_ok {
        return false;
    }
    if clicked_ok {
        return residual_difficulty_select_last_action() == ResidualDifficultySelectAction::Ok;
    }
    simulate_difficulty_select_ok_button_gadget_selected()
}

#[cfg(test)]
mod os_wnd_tests {
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
    fn os_wnd_difficulty_select_hits_radio_and_ok_then_latches() {
        install_named_button("DifficultySelect.wnd:RadioButtonHard", 10, 10);
        install_named_button("DifficultySelect.wnd:ButtonOk", 10, 40);
        assert!(
            drive_os_wnd_difficulty_select_like_cpp(2),
            "OS WND clicks on Hard + Ok must latch difficulty residual"
        );
        assert_eq!(residual_difficulty_select_level(), 2);
        assert_eq!(
            residual_difficulty_select_last_action(),
            ResidualDifficultySelectAction::Ok
        );
        assert!(!drive_os_wnd_difficulty_select_cancel_like_cpp());
    }

    #[test]
    fn os_wnd_difficulty_select_cancel_hits_named_gadget() {
        install_named_button("DifficultySelect.wnd:ButtonCancel", 10, 70);
        assert!(drive_os_wnd_difficulty_select_cancel_like_cpp());
        assert_eq!(
            residual_difficulty_select_last_action(),
            ResidualDifficultySelectAction::Cancel
        );
    }
}
