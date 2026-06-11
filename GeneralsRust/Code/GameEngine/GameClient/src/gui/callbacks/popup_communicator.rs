//! PopupCommunicator.cpp callback port.

use crate::gui::{
    with_window_manager, write_input_focus_response, GameWindow, WindowLayout, WindowMessage,
    WindowMsgData, WindowMsgHandled,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Default)]
struct PopupCommunicatorState {
    parent_id: Option<u32>,
    button_ok_id: Option<u32>,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_ok: Option<Rc<RefCell<GameWindow>>>,
}

thread_local! {
    static POPUP_COMMUNICATOR_STATE: Arc<Mutex<PopupCommunicatorState>> =
        Arc::new(Mutex::new(PopupCommunicatorState::default()));
}

fn popup_communicator_state() -> Arc<Mutex<PopupCommunicatorState>> {
    POPUP_COMMUNICATOR_STATE.with(|state| state.clone())
}

pub fn popup_communicator_init(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
    let parent_id = NameKeyGenerator::name_to_key("PopupCommunicator.wnd:PopupCommunicator");
    let button_ok_id = NameKeyGenerator::name_to_key("PopupCommunicator.wnd:ButtonOk");

    let parent = with_window_manager(|manager| manager.get_window_by_id(parent_id as i32));
    if let Some(parent) = parent.as_ref() {
        with_window_manager(|manager| {
            let _ = manager.set_focus(Some(parent));
            let _ = manager.set_modal(parent.clone());
        });
    }

    let button_ok = parent
        .as_ref()
        .and_then(|parent| parent.borrow().find_child_by_id(button_ok_id as i32));

    let state_handle = popup_communicator_state();
    let mut state = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    state.parent_id = Some(parent_id);
    state.button_ok_id = Some(button_ok_id);
    state.parent = parent;
    state.button_ok = button_ok;
}

pub fn popup_communicator_shutdown(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {
}

pub fn popup_communicator_update(_layout: &WindowLayout, _user_data: Option<&dyn std::any::Any>) {}

pub fn popup_communicator_input(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    if msg != WindowMessage::Char {
        return WindowMsgHandled::Ignored;
    }

    let key = data1;
    let state = data2;
    if key != KEY_ESC {
        return WindowMsgHandled::Ignored;
    }

    if (state & KEY_STATE_UP) == 0 {
        return WindowMsgHandled::Handled;
    }

    let state_handle = popup_communicator_state();
    let guard = state_handle.lock().unwrap_or_else(|e| e.into_inner());
    let button_ok_id = guard.button_ok_id.unwrap_or(0) as u32;

    with_window_manager(|manager| {
        if let Some(handle) = manager.get_window_by_id(window.get_id()) {
            manager.send_system_message(
                &handle,
                WindowMessage::GadgetSelected,
                button_ok_id as WindowMsgData,
                button_ok_id as WindowMsgData,
            );
        }
    });

    WindowMsgHandled::Handled
}

pub fn popup_communicator_system(
    window: &GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            let state_handle = popup_communicator_state();
            let mut guard = state_handle.lock().unwrap_or_else(|e| e.into_inner());
            let button_ok_id = guard.button_ok_id.unwrap_or(0);

            if control_id == button_ok_id {
                if let Some(parent) = guard.parent.as_ref() {
                    with_window_manager(|manager| {
                        let _ = manager.unset_modal(parent);
                    });
                }
                let layout = window.get_layout();
                guard.parent = None;
                guard.button_ok = None;
                if let Some(layout) = layout {
                    with_window_manager(|manager| manager.destroy_layout(&layout));
                }
            }
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}
