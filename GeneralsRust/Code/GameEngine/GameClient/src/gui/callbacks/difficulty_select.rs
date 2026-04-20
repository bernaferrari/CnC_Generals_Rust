//! DifficultySelect.cpp callback port.

use crate::gui::campaign_manager::{get_campaign_manager, GameDifficulty};
use crate::gui::shell::main_menu::{get_main_menu, GameDifficulty as MainMenuDifficulty};
use crate::gui::{
    with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
    WindowWidget,
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
    let state = state_handle
        .lock().unwrap_or_else(|e| e.into_inner());
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
    let state = state_handle
        .lock().unwrap_or_else(|e| e.into_inner());
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
    let mut state = state_handle
        .lock().unwrap_or_else(|e| e.into_inner());

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
    let mut state = state_handle
        .lock().unwrap_or_else(|e| e.into_inner());

    match msg {
        WindowMessage::Create | WindowMessage::Destroy | WindowMessage::InputFocus => {
            WindowMsgHandled::Handled
        }
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
