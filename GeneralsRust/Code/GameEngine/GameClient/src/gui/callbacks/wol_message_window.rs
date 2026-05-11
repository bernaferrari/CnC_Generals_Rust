//! WOLMessageWindow.cpp callback port.

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
struct WolMessageWindowState {
    parent_id: u32,
    button_cancel_id: u32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_cancel: Option<Rc<RefCell<GameWindow>>>,
}

static WOL_MESSAGE_WINDOW_STATE: OnceLock<Mutex<WolMessageWindowState>> = OnceLock::new();

fn wol_message_window_state() -> &'static Mutex<WolMessageWindowState> {
    WOL_MESSAGE_WINDOW_STATE.get_or_init(|| Mutex::new(WolMessageWindowState::default()))
}

fn name_to_id(name: &str) -> u32 {
    NameKeyGenerator::name_to_key(name) as u32
}

pub fn wol_message_window_init(layout: &WindowLayout, _user_data: Option<&mut dyn std::any::Any>) {
    let parent_id = name_to_id("WOLMessageWindow.wnd:WOLMessageWindowParent");
    let button_cancel_id = name_to_id("WOLMessageWindow.wnd:ButtonCancel");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let button_cancel =
        with_window_manager(|manager| manager.get_window_by_id(button_cancel_id as i32));

    layout.hide(false);

    if let Some(parent) = parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    let mut state = wol_message_window_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    state.parent_id = parent_id;
    state.button_cancel_id = button_cancel_id;
    state.parent = parent;
    state.button_cancel = button_cancel;
}

pub fn wol_message_window_shutdown(
    layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    layout.hide(true);
    get_shell().shutdown_complete(layout);
}

pub fn wol_message_window_update(
    _layout: &WindowLayout,
    _user_data: Option<&mut dyn std::any::Any>,
) {
    // WOL update hooks are handled elsewhere in the Rust port.
}

pub fn wol_message_window_input(
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

    let state = wol_message_window_state()
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    if let Some(parent) = state.parent.as_ref() {
        let _ = parent.borrow_mut().send_system_message(
            WindowMessage::GadgetSelected,
            state.button_cancel_id,
            state.button_cancel_id,
        );
    }

    WindowMsgHandled::Handled
}

pub fn wol_message_window_system(
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
