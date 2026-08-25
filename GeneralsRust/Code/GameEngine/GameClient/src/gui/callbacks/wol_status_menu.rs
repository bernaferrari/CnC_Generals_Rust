//! WOLStatusMenu.cpp callback port.

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
struct WolStatusState {
    parent_id: i32,
    button_disconnect_id: i32,
    progress_text_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_disconnect: Option<Rc<RefCell<GameWindow>>>,
    progress_text: Option<Rc<RefCell<GameWindow>>>,
}

thread_local! {
    static WOL_STATUS_STATE: Rc<RefCell<WolStatusState>> = Rc::new(RefCell::new(WolStatusState::default()));
}

fn wol_status_state() -> Rc<RefCell<WolStatusState>> {
    WOL_STATUS_STATE.with(Rc::clone)
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

pub fn wol_status_menu_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let parent_id = name_to_id("WOLStatusMenu.wnd:WOLStatusMenuParent");
    let button_disconnect_id = name_to_id("WOLStatusMenu.wnd:ButtonDisconnect");
    let progress_text_id = name_to_id("WOLStatusMenu.wnd:ListboxStatus");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let button_disconnect =
        with_window_manager(|manager| manager.get_window_by_id(button_disconnect_id as i32));
    let progress_text =
        with_window_manager(|manager| manager.get_window_by_id(progress_text_id as i32));

    layout.hide(false);

    if let Some(parent) = parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    let state_slot = wol_status_state();
    let mut state = state_slot.borrow_mut();
    state.parent_id = parent_id;
    state.button_disconnect_id = button_disconnect_id;
    state.progress_text_id = progress_text_id;
    state.parent = parent;
    state.button_disconnect = button_disconnect;
    state.progress_text = progress_text;
}

pub fn wol_status_menu_shutdown(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    layout.hide(true);
    let _ = get_shell().shutdown_complete(None, false);
}

pub fn wol_status_menu_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    // WOL update hooks are handled elsewhere in the Rust port.
}

pub fn wol_status_menu_input(
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

    // C++ WOLStatusMenu.cpp:112-113 sends GBM_SELECTED then returns HANDLED.
    // Drop the RefCell borrow before dispatch (C++ file-statics are re-entrant).
    let (parent, button_id) = {
        let slot = wol_status_state();
        let state = slot.borrow();
        (state.parent.clone(), state.button_disconnect_id)
    };
    dispatch_esc_gadget_selected(parent, button_id);

    WindowMsgHandled::Handled
}

pub fn wol_status_menu_system(
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_system_handles_create_and_ignores_unknown_when_dispatched() {
        let window = GameWindow::new();
        assert_eq!(
            wol_status_menu_system(&window, WindowMessage::Create, 0, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            wol_status_menu_system(&window, WindowMessage::User(1), 0, 0),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            wol_status_menu_system(&window, WindowMessage::GadgetEditDone, 0, 0),
            WindowMsgHandled::Handled
        );
    }

    #[test]
    fn status_input_ignores_non_escape_when_char_is_other_key() {
        let window = GameWindow::new();
        assert_eq!(
            wol_status_menu_input(&window, WindowMessage::Char, 0x41, KEY_STATE_UP),
            WindowMsgHandled::Ignored
        );
        assert_eq!(
            wol_status_menu_input(&window, WindowMessage::LeftDown, KEY_ESC, KEY_STATE_UP),
            WindowMsgHandled::Ignored
        );
    }

    #[test]
    fn status_input_esc_does_not_panic_when_parent_missing() {
        let window = GameWindow::new();
        assert_eq!(
            wol_status_menu_input(&window, WindowMessage::Char, KEY_ESC, 0),
            WindowMsgHandled::Handled
        );
        assert_eq!(
            wol_status_menu_input(&window, WindowMessage::Char, KEY_ESC, KEY_STATE_UP),
            WindowMsgHandled::Handled
        );
    }
}
