//! WOLMessageWindow.cpp callback port.

use std::cell::RefCell;
use std::rc::Rc;

use crate::gui::callbacks::online_callback_support::dispatch_esc_gadget_selected;
use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled, get_shell,
    with_window_manager, write_input_focus_response,
};
use game_engine::common::name_key_generator::NameKeyGenerator;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Default)]
struct WolMessageWindowState {
    parent_id: i32,
    button_cancel_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_cancel: Option<Rc<RefCell<GameWindow>>>,
}

thread_local! {
    static WOL_MESSAGE_WINDOW_STATE: Rc<RefCell<WolMessageWindowState>> = Rc::new(RefCell::new(WolMessageWindowState::default()));
}

fn wol_message_window_state() -> Rc<RefCell<WolMessageWindowState>> {
    WOL_MESSAGE_WINDOW_STATE.with(Rc::clone)
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

pub fn wol_message_window_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let parent_id = name_to_id("WOLMessageWindow.wnd:WOLMessageWindowParent");
    let button_cancel_id = name_to_id("WOLMessageWindow.wnd:ButtonCancel");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let button_cancel =
        with_window_manager(|manager| manager.get_window_by_id(button_cancel_id as i32));

    layout.hide(false);

    if let Some(parent) = parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    let state_slot = wol_message_window_state();
    let mut state = state_slot.borrow_mut();
    state.parent_id = parent_id;
    state.button_cancel_id = button_cancel_id;
    state.parent = parent;
    state.button_cancel = button_cancel;
}

pub fn wol_message_window_shutdown(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    layout.hide(true);
    let _ = get_shell().shutdown_complete(None, false);
}

pub fn wol_message_window_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
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

    // C++ WOLMessageWindow.cpp:106 — winSendSystemMsg GBM_SELECTED.
    let (parent, button_id) = {
        let slot = wol_message_window_state();
        let state = slot.borrow();
        (state.parent.clone(), state.button_cancel_id)
    };
    dispatch_esc_gadget_selected(parent, button_id);

    WindowMsgHandled::Handled
}

pub fn wol_message_window_system(
    _window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => WindowMsgHandled::Handled,
        WindowMessage::GadgetEditDone => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}
