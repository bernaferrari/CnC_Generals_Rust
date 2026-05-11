//! WOLCustomScoreScreen.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Mutex, OnceLock};

use crate::gui::{
    get_shell, with_window_manager, GameWindow, WindowLayout, WindowMessage, WindowMsgData,
    WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;

const KEY_ESC: u32 = 0x1B;
const KEY_STATE_UP: u32 = 0x0001;

#[derive(Default)]
struct WolCustomScoreState {
    parent_id: u32,
    button_disconnect_id: u32,
    button_lobby_id: u32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_disconnect: Option<Rc<RefCell<GameWindow>>>,
    button_lobby: Option<Rc<RefCell<GameWindow>>>,
}

static WOL_CUSTOM_SCORE_STATE: OnceLock<Mutex<WolCustomScoreState>> = OnceLock::new();

fn wol_custom_score_state() -> &'static Mutex<WolCustomScoreState> {
    WOL_CUSTOM_SCORE_STATE.get_or_init(|| Mutex::new(WolCustomScoreState::default()))
}

fn name_to_id(name: &str) -> u32 {
    NameKeyGenerator::name_to_key(name) as u32
}

pub fn wol_custom_score_screen_init(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    let parent_id = name_to_id("WOLCustomScoreScreen.wnd:WOLCustomScoreScreenParent");
    let button_disconnect_id = name_to_id("WOLCustomScoreScreen.wnd:ButtonDisconnect");
    let button_lobby_id = name_to_id("WOLCustomScoreScreen.wnd:ButtonLobby");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let button_disconnect =
        with_window_manager(|manager| manager.get_window_by_id(button_disconnect_id as i32));
    let button_lobby =
        with_window_manager(|manager| manager.get_window_by_id(button_lobby_id as i32));

    layout.hide(false);

    if let Some(parent) = parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    let mut state = wol_custom_score_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    state.parent_id = parent_id;
    state.button_disconnect_id = button_disconnect_id;
    state.button_lobby_id = button_lobby_id;
    state.parent = parent;
    state.button_disconnect = button_disconnect;
    state.button_lobby = button_lobby;
}

pub fn wol_custom_score_screen_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    layout.hide(true);
    get_shell().shutdown_complete(layout);
}

pub fn wol_custom_score_screen_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    // WOL update hooks are handled elsewhere in the Rust port.
}

pub fn wol_custom_score_screen_input(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char || data1 != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }
    if (data2 & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    let state = wol_custom_score_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_disconnect_id,
            state.button_disconnect_id,
        );
    }

    WindowMsgHandled::Handled
}

pub fn wol_custom_score_screen_system(
    _window: &GameWindow,
    msg: WindowMessage,
    _data1: WindowMsgData,
    _data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::InputFocus => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}
