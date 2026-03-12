//! DifficultySelect.cpp callback port.

use crate::gui::campaign_manager::{get_campaign_manager, GameDifficulty};
use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled, WindowWidget,
};
use game_engine::common::ini::get_global_data;
use game_engine::common::name_key_generator::NameKeyGenerator;
use gamelogic::helpers::{TheGameLogic, TheScriptEngine};
use gamelogic::system::game_logic::GAME_SINGLE_PLAYER;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;

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
        .lock()
        .expect("difficulty select state lock poisoned");
    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.unset_modal(parent);
        });
    }

    destroy_current_layout(window);
}

fn start_campaign_game(window: &GameWindow, difficulty: GameDifficulty) {
    let (current_map, rank_points) = {
        let mut campaign_manager = get_campaign_manager();
        campaign_manager.set_game_difficulty(difficulty);
        (
            campaign_manager.get_current_map().unwrap_or_default(),
            campaign_manager.get_rank_points(),
        )
    };

    if current_map.is_empty() {
        cancel_difficulty_select(window);
        return;
    }

    if let Some(data) = get_global_data() {
        data.write().pending_file = current_map;
    }
    TheScriptEngine::set_global_difficulty(difficulty_to_logic(difficulty));

    let state_handle = difficulty_select_state();
    let state = state_handle
        .lock()
        .expect("difficulty select state lock poisoned");
    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.unset_modal(parent);
        });
    }
    drop(state);

    destroy_current_layout(window);

    let _ = get_shell().hide_shell();
    TheGameLogic::prepare_new_game(
        GAME_SINGLE_PLAYER,
        difficulty_to_logic(difficulty),
        rank_points,
    );
}

pub fn difficulty_select_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let state_handle = difficulty_select_state();
    let mut state = state_handle
        .lock()
        .expect("difficulty select state lock poisoned");

    state.parent_id = name_to_id("DifficultySelect.wnd:DifficultySelectParent");
    state.button_ok_id = name_to_id("DifficultySelect.wnd:ButtonOk");
    state.button_cancel_id = name_to_id("DifficultySelect.wnd:ButtonCancel");
    state.radio_easy_id = name_to_id("DifficultySelect.wnd:RadioButtonEasy");
    state.radio_medium_id = name_to_id("DifficultySelect.wnd:RadioButtonMedium");
    state.radio_hard_id = name_to_id("DifficultySelect.wnd:RadioButtonHard");
    state.selected_difficulty = get_campaign_manager().get_game_difficulty();

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
        .lock()
        .expect("difficulty select state lock poisoned");

    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
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
        WindowMessage::GadgetValueChanged => {
            let control_id = data1 as i32;
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
            WindowMsgHandled::Ignored
        }
        WindowMessage::User(0x8000) => {
            let control_id = data1 as i32;
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
            WindowMsgHandled::Ignored
        }
        _ => WindowMsgHandled::Ignored,
    }
}

pub fn difficulty_select_input(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg == WindowMessage::Char && data1 == KEY_ESC && (data2 & KEY_STATE_UP) != 0 {
        cancel_difficulty_select(window);
        return WindowMsgHandled::Handled;
    }

    WindowMsgHandled::Ignored
}
