//! PopupJoinGame.cpp callback port.

use crate::gamespy_overlay::{
    close_overlay, current_staging_room_id, find_staging_room_by_id, queue_join_request,
    set_lobby_attempt_host_join, GameSpyOverlayType,
};
use crate::gui::{
    with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;

#[derive(Default)]
struct PopupJoinState {
    parent_id: Option<u32>,
    text_entry_id: Option<u32>,
    button_cancel_id: Option<u32>,
    parent: Option<Rc<RefCell<GameWindow>>>,
    text_entry: Option<Rc<RefCell<GameWindow>>>,
}

static POPUP_JOIN_STATE: OnceLock<Mutex<PopupJoinState>> = OnceLock::new();

fn popup_join_state() -> &'static Mutex<PopupJoinState> {
    POPUP_JOIN_STATE.get_or_init(|| Mutex::new(PopupJoinState::default()))
}

pub fn popup_join_game_init(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let parent_id = NameKeyGenerator::name_to_key("PopupJoinGame.wnd:ParentJoinPopUp");
    let text_entry_id = NameKeyGenerator::name_to_key("PopupJoinGame.wnd:TextEntryGamePassword");
    let button_cancel_id = NameKeyGenerator::name_to_key("PopupJoinGame.wnd:ButtonCancel");
    let static_text_game_name_id =
        NameKeyGenerator::name_to_key("PopupJoinGame.wnd:StaticTextGameName");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let text_entry = parent
        .as_ref()
        .and_then(|parent| parent.borrow().find_child_by_id(text_entry_id as i32));
    let static_text_game_name = parent.as_ref().and_then(|parent| {
        parent
            .borrow()
            .find_child_by_id(static_text_game_name_id as i32)
    });

    if let Some(text_entry) = text_entry.as_ref() {
        if let Some(widget) = text_entry.borrow_mut().text_entry_mut() {
            widget.set_text("");
        }
    }

    if let (Some(static_text), Some(room_id)) =
        (static_text_game_name.as_ref(), current_staging_room_id())
    {
        if let Some(room) = find_staging_room_by_id(room_id) {
            if let Some(widget) = static_text.borrow_mut().static_text_mut() {
                widget.set_text(room.game_name);
            }
        }
    }

    if let Some(parent) = parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
            let _ = manager.set_modal(parent.clone());
        });
    }

    let mut state = popup_join_state()
        .lock()
        .expect("popup join state lock poisoned");
    state.parent_id = Some(parent_id);
    state.text_entry_id = Some(text_entry_id);
    state.button_cancel_id = Some(button_cancel_id);
    state.parent = parent;
    state.text_entry = text_entry;
}

pub fn popup_join_game_input(
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

    close_overlay(GameSpyOverlayType::GamePassword);
    set_lobby_attempt_host_join(false);
    let mut state = popup_join_state()
        .lock()
        .expect("popup join state lock poisoned");
    state.parent = None;
    state.text_entry = None;

    WindowMsgHandled::Handled
}

pub fn popup_join_game_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let mut state = popup_join_state()
                .lock()
                .expect("popup join state lock poisoned");
            if control_id == state.button_cancel_id.unwrap_or(0) {
                close_overlay(GameSpyOverlayType::GamePassword);
                set_lobby_attempt_host_join(false);
                state.parent = None;
                state.text_entry = None;
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => {
            let control_id = data1 as u32;
            let mut state = popup_join_state()
                .lock()
                .expect("popup join state lock poisoned");
            if control_id == state.text_entry_id.unwrap_or(0) {
                if let Some(text_entry) = state.text_entry.as_ref() {
                    let mut text_entry = text_entry.borrow_mut();
                    if let Some(widget) = text_entry.text_entry_mut() {
                        let input = widget.text().trim().to_string();
                        widget.set_text("");
                        if !input.is_empty() {
                            join_game(input);
                        }
                    }
                }
            }
            WindowMsgHandled::Handled
        }
        _ => WindowMsgHandled::Ignored,
    }
}

fn join_game(password: String) {
    let Some(room_id) = current_staging_room_id() else {
        close_overlay(GameSpyOverlayType::GamePassword);
        set_lobby_attempt_host_join(false);
        let mut state = popup_join_state()
            .lock()
            .expect("popup join state lock poisoned");
        state.parent = None;
        state.text_entry = None;
        return;
    };

    if find_staging_room_by_id(room_id).is_none() {
        close_overlay(GameSpyOverlayType::GamePassword);
        set_lobby_attempt_host_join(false);
        let mut state = popup_join_state()
            .lock()
            .expect("popup join state lock poisoned");
        state.parent = None;
        state.text_entry = None;
        return;
    }

    queue_join_request(room_id, password);
    close_overlay(GameSpyOverlayType::GamePassword);
    let mut state = popup_join_state()
        .lock()
        .expect("popup join state lock poisoned");
    state.parent = None;
    state.text_entry = None;
}
