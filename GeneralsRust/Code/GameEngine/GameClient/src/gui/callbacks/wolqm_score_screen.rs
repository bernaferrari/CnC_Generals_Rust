//! WOLQMScoreScreen.cpp callback port.

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
struct WolQmScoreState {
    parent_id: i32,
    button_disconnect_id: i32,
    button_quickmatch_id: i32,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_disconnect: Option<Rc<RefCell<GameWindow>>>,
    button_quickmatch: Option<Rc<RefCell<GameWindow>>>,
}

thread_local! {
    static WOL_QM_SCORE_STATE: Rc<RefCell<WolQmScoreState>> = Rc::new(RefCell::new(WolQmScoreState::default()));
}

fn wol_qm_score_state() -> Rc<RefCell<WolQmScoreState>> {
    WOL_QM_SCORE_STATE.with(Rc::clone)
}

fn name_to_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

pub fn wol_qm_score_screen_init(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let parent_id = name_to_id("WOLQMScoreScreen.wnd:WOLQMScoreScreenParent");
    let button_disconnect_id = name_to_id("WOLQMScoreScreen.wnd:ButtonDisconnect");
    let button_quickmatch_id = name_to_id("WOLQMScoreScreen.wnd:ButtonQuickMatch");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    let button_disconnect =
        with_window_manager(|manager| manager.get_window_by_id(button_disconnect_id as i32));
    let button_quickmatch =
        with_window_manager(|manager| manager.get_window_by_id(button_quickmatch_id as i32));

    layout.hide(false);

    if let Some(parent) = parent.as_ref() {
        let _ = with_window_manager(|manager| manager.set_focus(Some(parent)));
    }

    let state_slot = wol_qm_score_state();
    let mut state = state_slot.borrow_mut();
    state.parent_id = parent_id;
    state.button_disconnect_id = button_disconnect_id;
    state.button_quickmatch_id = button_quickmatch_id;
    state.parent = parent;
    state.button_disconnect = button_disconnect;
    state.button_quickmatch = button_quickmatch;
}

pub fn wol_qm_score_screen_shutdown(layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    layout.hide(true);
    let _ = get_shell().shutdown_complete(None, false);
}

pub fn wol_qm_score_screen_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    // WOL update hooks are handled elsewhere in the Rust port.
}

pub fn wol_qm_score_screen_input(
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

    // C++ WOLQMScoreScreen.cpp:122 — winSendSystemMsg GBM_SELECTED.
    let (parent, button_id) = {
        let slot = wol_qm_score_state();
        let state = slot.borrow();
        (state.parent.clone(), state.button_disconnect_id)
    };
    dispatch_esc_gadget_selected(parent, button_id);

    WindowMsgHandled::Handled
}

pub fn wol_qm_score_screen_system(
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
