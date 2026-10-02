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

// THREAD: main thread only — the state owns `Rc` window handles (already
// `!Send`); the cell replaces the lock-wrapped `Arc` around it.
thread_local! {
    static POPUP_COMMUNICATOR_STATE: RefCell<PopupCommunicatorState> =
        RefCell::new(PopupCommunicatorState::default());
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

    POPUP_COMMUNICATOR_STATE.with_borrow_mut(|state| {
        state.parent_id = Some(parent_id);
        state.button_ok_id = Some(button_ok_id);
        state.parent = parent;
        state.button_ok = button_ok;
    });
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

    let button_ok_id =
        POPUP_COMMUNICATOR_STATE.with_borrow(|guard| guard.button_ok_id.unwrap_or(0));

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
            POPUP_COMMUNICATOR_STATE.with_borrow_mut(|guard| {
                let button_ok_id = guard.button_ok_id.unwrap_or(0);

                if control_id == button_ok_id {
                    if let Some(parent) = guard.parent.as_ref() {
                        let parent = parent.clone();
                        with_window_manager(|manager| {
                            let _ = manager.unset_modal(&parent);
                        });
                    }
                    let layout = window.get_layout();
                    guard.parent = None;
                    guard.button_ok = None;
                    if let Some(layout) = layout {
                        with_window_manager(|manager| manager.destroy_layout(&layout));
                    }
                }
            });
            WindowMsgHandled::Handled
        }
        WindowMessage::GadgetEditDone => WindowMsgHandled::Handled,
        _ => WindowMsgHandled::Ignored,
    }
}

/// Residual: last PopupCommunicator action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualPopupCommunicatorAction {
    None = 0,
    Bind = 1,
    Ok = 2,
    Esc = 3,
}

static RESIDUAL_POPCOM_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_POPCOM_VISIBLE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

fn residual_popcom_action_store(action: ResidualPopupCommunicatorAction) {
    RESIDUAL_POPCOM_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last PopupCommunicator residual action.
pub fn residual_popup_communicator_last_action() -> ResidualPopupCommunicatorAction {
    match RESIDUAL_POPCOM_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualPopupCommunicatorAction::Bind,
        2 => ResidualPopupCommunicatorAction::Ok,
        3 => ResidualPopupCommunicatorAction::Esc,
        _ => ResidualPopupCommunicatorAction::None,
    }
}

/// Residual: PopupCommunicator visibility latch.
pub fn residual_popup_communicator_is_visible() -> bool {
    RESIDUAL_POPCOM_VISIBLE.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: bind PopupCommunicator control IDs (no layout/modal).
pub fn simulate_popup_communicator_bind_controls() -> bool {
    POPUP_COMMUNICATOR_STATE.with_borrow_mut(|state| {
        if state.parent_id.is_none() {
            state.parent_id = Some(NameKeyGenerator::name_to_key(
                "PopupCommunicator.wnd:PopupCommunicator",
            ));
        }
        if state.button_ok_id.is_none() {
            state.button_ok_id = Some(NameKeyGenerator::name_to_key(
                "PopupCommunicator.wnd:ButtonOk",
            ));
        }
        residual_popcom_action_store(ResidualPopupCommunicatorAction::Bind);
        true
    })
}

/// Residual: show residual without modal/layout create.
pub fn simulate_popup_communicator_show() -> bool {
    let _ = simulate_popup_communicator_bind_controls();
    RESIDUAL_POPCOM_VISIBLE.store(true, std::sync::atomic::Ordering::Relaxed);
    residual_popup_communicator_is_visible()
}

/// Residual: fire ButtonOk without destroy_layout/unset_modal.
pub fn simulate_popup_communicator_ok_button_gadget_selected() -> bool {
    let _ = simulate_popup_communicator_bind_controls();
    RESIDUAL_POPCOM_VISIBLE.store(false, std::sync::atomic::Ordering::Relaxed);
    residual_popcom_action_store(ResidualPopupCommunicatorAction::Ok);
    !residual_popup_communicator_is_visible()
}

/// Residual: ESC maps to Ok residual.
pub fn simulate_popup_communicator_esc() -> bool {
    residual_popcom_action_store(ResidualPopupCommunicatorAction::Esc);
    simulate_popup_communicator_ok_button_gadget_selected();
    residual_popcom_action_store(ResidualPopupCommunicatorAction::Esc);
    !residual_popup_communicator_is_visible()
}

/// Residual: show + Ok composite.
pub fn simulate_popup_communicator_prepare_ok() -> bool {
    if !simulate_popup_communicator_show() {
        return false;
    }
    simulate_popup_communicator_ok_button_gadget_selected()
}
