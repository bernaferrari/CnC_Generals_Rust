//! PopupHostGame.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::gamespy_overlay::{
    close_overlay, queue_host_request, set_lobby_attempt_host_join, GameSpyHostRequest,
    GameSpyOverlayType,
};
use crate::gui::{
    with_window_manager, CustomMatchPreferencesStore, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::gamespy::peer_defs::get_gamespy_info;

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;

#[derive(Default)]
struct PopupHostState {
    parent_id: i32,
    text_entry_game_name_id: i32,
    text_entry_game_description_id: i32,
    text_entry_game_password_id: i32,
    button_create_game_id: i32,
    button_cancel_id: i32,
    check_box_allow_observers_id: i32,
    check_box_limit_armies_id: i32,
    check_box_use_stats_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    text_entry_game_name: Option<Rc<RefCell<GameWindow>>>,
    text_entry_game_description: Option<Rc<RefCell<GameWindow>>>,
    text_entry_game_password: Option<Rc<RefCell<GameWindow>>>,
    check_box_allow_observers: Option<Rc<RefCell<GameWindow>>>,
    check_box_limit_armies: Option<Rc<RefCell<GameWindow>>>,
    check_box_use_stats: Option<Rc<RefCell<GameWindow>>>,
}

static POPUP_HOST_STATE: OnceLock<Mutex<PopupHostState>> = OnceLock::new();

fn popup_host_state() -> &'static Mutex<PopupHostState> {
    POPUP_HOST_STATE.get_or_init(|| Mutex::new(PopupHostState::default()))
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

pub fn custom_match_hide_host_popup(hide: bool) {
    let mut state = popup_host_state()
        .lock()
        .expect("PopupHostGame state lock poisoned");
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().hide(hide);
        return;
    }
    if state.parent_id == 0 {
        state.parent_id = name_to_id("PopupHostGame.wnd:ParentHostPopUp");
    }
    state.parent = with_window_manager(|manager| manager.get_window_by_id(state.parent_id as u32));
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().hide(hide);
    }
}

fn set_text_entry(window: &Option<Rc<RefCell<GameWindow>>>, value: &str) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if let Some(entry) = guard.text_entry_mut() {
        entry.set_text(value);
    }
}

fn get_text_entry(window: &Option<Rc<RefCell<GameWindow>>>) -> String {
    let Some(window) = window.as_ref() else {
        return String::new();
    };
    let guard = window.borrow();
    if let Some(entry) = guard.widget().and_then(|widget| match widget {
        crate::gui::WindowWidget::TextEntry(entry) => Some(entry),
        _ => None,
    }) {
        return entry.text().to_string();
    }
    String::new()
}

fn set_checkbox(window: &Option<Rc<RefCell<GameWindow>>>, checked: bool) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    if let Some(check) = guard.check_box_mut() {
        check.set_checked(checked);
    }
}

fn checkbox_checked(window: &Option<Rc<RefCell<GameWindow>>>) -> bool {
    let Some(window) = window.as_ref() else {
        return false;
    };
    let guard = window.borrow();
    guard
        .widget()
        .and_then(|widget| match widget {
            crate::gui::WindowWidget::CheckBox(check) => Some(check),
            _ => None,
        })
        .map(|check| check.is_checked())
        .unwrap_or(false)
}

fn enable_window(window: &Option<Rc<RefCell<GameWindow>>>, enabled: bool) {
    let Some(window) = window.as_ref() else {
        return;
    };
    let mut guard = window.borrow_mut();
    let _ = guard.enable(enabled);
}

fn sync_limit_armies_state(state: &PopupHostState) {
    let use_stats = checkbox_checked(&state.check_box_use_stats);
    if use_stats {
        set_checkbox(&state.check_box_limit_armies, false);
        enable_window(&state.check_box_limit_armies, false);
    } else {
        enable_window(&state.check_box_limit_armies, true);
    }
}

fn clear_popup_host_refs(state: &mut PopupHostState) {
    state.parent = None;
    state.text_entry_game_name = None;
    state.text_entry_game_description = None;
    state.text_entry_game_password = None;
    state.check_box_allow_observers = None;
    state.check_box_limit_armies = None;
    state.check_box_use_stats = None;
}

pub fn popup_host_game_init(_layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let mut state = popup_host_state()
        .lock()
        .expect("PopupHostGame state lock poisoned");

    state.parent_id = name_to_id("PopupHostGame.wnd:ParentHostPopUp");
    state.text_entry_game_name_id = name_to_id("PopupHostGame.wnd:TextEntryGameName");
    state.text_entry_game_description_id = name_to_id("PopupHostGame.wnd:TextEntryGameDescription");
    state.text_entry_game_password_id = name_to_id("PopupHostGame.wnd:TextEntryGamePassword");
    state.button_create_game_id = name_to_id("PopupHostGame.wnd:ButtonCreateGame");
    state.button_cancel_id = name_to_id("PopupHostGame.wnd:ButtonCancel");
    state.check_box_allow_observers_id = name_to_id("PopupHostGame.wnd:CheckBoxAllowObservers");
    state.check_box_limit_armies_id = name_to_id("PopupHostGame.wnd:CheckBoxLimitArmies");
    state.check_box_use_stats_id = name_to_id("PopupHostGame.wnd:CheckBoxUseStats");

    state.parent = with_window_manager(|manager| manager.get_window_by_id(state.parent_id));

    if let Some(parent) = state.parent.as_ref() {
        state.text_entry_game_name = parent
            .borrow()
            .find_child_by_id(state.text_entry_game_name_id as u32);
        state.text_entry_game_description = parent
            .borrow()
            .find_child_by_id(state.text_entry_game_description_id as u32);
        state.text_entry_game_password = parent
            .borrow()
            .find_child_by_id(state.text_entry_game_password_id as u32);
        state.check_box_allow_observers = parent
            .borrow()
            .find_child_by_id(state.check_box_allow_observers_id as u32);
        state.check_box_limit_armies = parent
            .borrow()
            .find_child_by_id(state.check_box_limit_armies_id as u32);
        state.check_box_use_stats = parent
            .borrow()
            .find_child_by_id(state.check_box_use_stats_id as u32);
    }

    let mut prefs = CustomMatchPreferencesStore::new();
    let local_name = get_gamespy_info()
        .and_then(|info| {
            info.lock()
                .ok()
                .map(|guard| guard.get_local_name().to_string())
        })
        .unwrap_or_else(|| "My Game".to_string());
    set_text_entry(&state.text_entry_game_name, &local_name);
    set_text_entry(&state.text_entry_game_description, "");
    set_text_entry(&state.text_entry_game_password, "");
    set_checkbox(
        &state.check_box_allow_observers,
        prefs.prefs().allows_observers(),
    );
    set_checkbox(&state.check_box_use_stats, prefs.prefs().get_use_stats());
    set_checkbox(
        &state.check_box_limit_armies,
        prefs.prefs().get_factions_limited(),
    );

    sync_limit_armies_state(&state);

    if let Some(parent) = state.parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
            let _ = manager.set_modal(parent.clone());
        });
    }
}

pub fn popup_host_game_update(_layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let state = popup_host_state()
        .lock()
        .expect("PopupHostGame state lock poisoned");
    sync_limit_armies_state(&state);
}

pub fn popup_host_game_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char {
        return WindowMsgHandled::Ignored;
    }
    if data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    let mut state = popup_host_state()
        .lock()
        .expect("PopupHostGame state lock poisoned");
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_cancel_id as u32,
            state.button_cancel_id as u32,
        );
    }

    WindowMsgHandled::Handled
}

pub fn popup_host_game_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let mut state = popup_host_state()
                .lock()
                .expect("PopupHostGame state lock poisoned");
            let control_id = data1 as i32;
            if control_id == state.button_cancel_id {
                // Clear modal before closing - matches C++ GWM_DESTROY handling
                if let Some(parent) = state.parent.as_ref() {
                    with_window_manager(|manager| {
                        let _ = manager.unset_modal(parent);
                    });
                }
                close_overlay(GameSpyOverlayType::GameOptions);
                set_lobby_attempt_host_join(false);
                clear_popup_host_refs(&mut state);
                return WindowMsgHandled::Handled;
            }
            if control_id == state.button_create_game_id {
                let mut prefs = CustomMatchPreferencesStore::new();
                let mut game_name = get_text_entry(&state.text_entry_game_name);
                game_name = game_name.trim().to_string();
                if game_name.is_empty() {
                    game_name = get_gamespy_info()
                        .and_then(|info| {
                            info.lock()
                                .ok()
                                .map(|guard| guard.get_local_name().to_string())
                        })
                        .unwrap_or_else(|| "My Game".to_string());
                    set_text_entry(&state.text_entry_game_name, &game_name);
                }

                let description = get_text_entry(&state.text_entry_game_description);
                let password = get_text_entry(&state.text_entry_game_password);
                let allow_observers = checkbox_checked(&state.check_box_allow_observers);
                let use_stats = checkbox_checked(&state.check_box_use_stats);
                let limit_armies = checkbox_checked(&state.check_box_limit_armies);

                prefs.prefs_mut().set_allows_observers(allow_observers);
                prefs.prefs_mut().set_use_stats(use_stats);
                prefs.prefs_mut().set_factions_limited(limit_armies);
                prefs.write();

                queue_host_request(GameSpyHostRequest {
                    game_name,
                    game_description: description,
                    game_password: password,
                    allow_observers,
                    use_stats,
                    limit_armies,
                });
                // Clear modal before closing
                if let Some(parent) = state.parent.as_ref() {
                    with_window_manager(|manager| {
                        let _ = manager.unset_modal(parent);
                    });
                }
                close_overlay(GameSpyOverlayType::GameOptions);
                set_lobby_attempt_host_join(false);
                clear_popup_host_refs(&mut state);
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            let mut state = popup_host_state()
                .lock()
                .expect("PopupHostGame state lock poisoned");
            if data1 as i32 == state.text_entry_game_name_id {
                let text = get_text_entry(&state.text_entry_game_name);
                let trimmed = text.trim().to_string();
                set_text_entry(&state.text_entry_game_name, &trimmed);
                return WindowMsgHandled::Handled;
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}
