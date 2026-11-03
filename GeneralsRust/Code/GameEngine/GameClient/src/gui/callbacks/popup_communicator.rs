//! PopupCommunicator.cpp callback port.

use crate::gui::{
    GameWindow, WindowLayout, WindowMessage, WindowMsgData, WindowMsgHandled,
    write_input_focus_response,
};
use game_engine::common::name_key_generator::NameKeyGenerator;
use std::cell::RefCell;
use std::rc::Rc;

const KEY_ESC: usize = 0x1B;
const KEY_STATE_UP: usize = 0x0001;

#[derive(Default)]
pub(crate) struct PopupCommunicatorState {
    parent_id: Option<u32>,
    button_ok_id: Option<u32>,
    parent: Option<Rc<RefCell<GameWindow>>>,
    button_ok: Option<Rc<RefCell<GameWindow>>>,
    residual_action: u8,
    residual_visible: bool,
}

#[cfg(test)]
thread_local! {
    static TEST_POPUP_COMMUNICATOR: RefCell<PopupCommunicatorState> =
        RefCell::new(PopupCommunicatorState::default());
}

fn with_popup_communicator_mut<R>(
    owner: Option<&crate::gui::window_manager::WindowManagerOwner>,
    f: impl FnOnce(&mut PopupCommunicatorState) -> R,
) -> R {
    let mut f = Some(f);
    if let Some(owner) = owner {
        return owner.with_popup_communicator_mut(|state| f.take().expect("popcom")(state));
    }
    if let Some(result) = crate::core::game_client::with_live_game_client_mut(|client| {
        client
            .window_manager_owner_ref()
            .with_popup_communicator_mut(|state| f.take().expect("popcom")(state))
    }) {
        return result;
    }
    #[cfg(test)]
    {
        return TEST_POPUP_COMMUNICATOR.with(|slot| f.take().expect("popcom")(&mut slot.borrow_mut()));
    }
    #[cfg(not(test))]
    {
        f.take().expect("popcom")(&mut PopupCommunicatorState::default())
    }
}

fn with_popup_communicator_ref<R>(
    owner: Option<&crate::gui::window_manager::WindowManagerOwner>,
    f: impl FnOnce(&PopupCommunicatorState) -> R,
) -> R {
    with_popup_communicator_mut(owner, |state| f(state))
}

pub fn popup_communicator_init(
    _layout: &WindowLayout,
    _user_data: Option<&dyn std::any::Any>,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    let parent_id = NameKeyGenerator::name_to_key("PopupCommunicator.wnd:PopupCommunicator");
    let button_ok_id = NameKeyGenerator::name_to_key("PopupCommunicator.wnd:ButtonOk");

    let parent = (|manager: &mut crate::gui::window_manager::WindowManager| {
        manager.get_window_by_id(parent_id as i32)
    })(receiving_manager);
    if let Some(parent) = parent.as_ref() {
        (|manager: &mut crate::gui::window_manager::WindowManager| {
            let _ = manager.set_focus(Some(parent), effects);
            let _ = manager.set_modal(parent.clone());
        })(receiving_manager);
    }

    let button_ok = parent
        .as_ref()
        .and_then(|parent| parent.borrow().find_child_by_id(button_ok_id as i32));

    with_popup_communicator_mut(effects.window_manager_owner, |state| {
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
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
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

    let button_ok_id = with_popup_communicator_ref(effects.window_manager_owner, |guard| {
        guard.button_ok_id.unwrap_or(0)
    });

    (|manager: &mut crate::gui::window_manager::WindowManager| {
        if let Some(handle) = manager.get_window_by_id(window.get_id()) {
            manager.send_system_message(
                &handle,
                WindowMessage::GadgetSelected,
                button_ok_id as WindowMsgData,
                button_ok_id as WindowMsgData,
                effects,
            );
        }
    })(receiving_manager);

    WindowMsgHandled::Handled
}

pub fn popup_communicator_system(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    receiving_manager: &mut crate::gui::window_manager::WindowManager,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) -> WindowMsgHandled {
    match msg {
        WindowMessage::Create => WindowMsgHandled::Handled,
        WindowMessage::Destroy => WindowMsgHandled::Handled,
        WindowMessage::InputFocus => write_input_focus_response(data1, data2, true),
        WindowMessage::GadgetSelected => {
            let control_id = data1 as u32;
            with_popup_communicator_mut(effects.window_manager_owner, |guard| {
                let button_ok_id = guard.button_ok_id.unwrap_or(0);

                if control_id == button_ok_id {
                    if let Some(parent) = guard.parent.as_ref() {
                        let parent = parent.clone();
                        (|manager: &mut crate::gui::window_manager::WindowManager| {
                            let _ = manager.unset_modal(&parent);
                        })(receiving_manager);
                    }
                    let layout = window.get_layout();
                    guard.parent = None;
                    guard.button_ok = None;
                    if let Some(layout) = layout {
                        (|manager: &mut crate::gui::window_manager::WindowManager| {
                            manager.destroy_layout(&layout, effects)
                        })(receiving_manager);
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

fn residual_popcom_action_store(state: &mut PopupCommunicatorState, action: ResidualPopupCommunicatorAction) {
    state.residual_action = action as u8;
}

/// Residual: last PopupCommunicator residual action.
pub fn residual_popup_communicator_last_action() -> ResidualPopupCommunicatorAction {
    with_popup_communicator_ref(None, |state| match state.residual_action {
        1 => ResidualPopupCommunicatorAction::Bind,
        2 => ResidualPopupCommunicatorAction::Ok,
        3 => ResidualPopupCommunicatorAction::Esc,
        _ => ResidualPopupCommunicatorAction::None,
    })
}

/// Residual: PopupCommunicator visibility latch.
pub fn residual_popup_communicator_is_visible() -> bool {
    with_popup_communicator_ref(None, |state| state.residual_visible)
}

/// Residual: bind PopupCommunicator control IDs (no layout/modal).
pub fn simulate_popup_communicator_bind_controls() -> bool {
    with_popup_communicator_mut(None, |state| {
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
        residual_popcom_action_store(state, ResidualPopupCommunicatorAction::Bind);
        true
    })
}

/// Residual: show residual without modal/layout create.
pub fn simulate_popup_communicator_show() -> bool {
    let _ = simulate_popup_communicator_bind_controls();
    with_popup_communicator_mut(None, |state| {
        state.residual_visible = true;
    });
    residual_popup_communicator_is_visible()
}

/// Residual: fire ButtonOk without destroy_layout/unset_modal.
pub fn simulate_popup_communicator_ok_button_gadget_selected() -> bool {
    let _ = simulate_popup_communicator_bind_controls();
    with_popup_communicator_mut(None, |state| {
        state.residual_visible = false;
        residual_popcom_action_store(state, ResidualPopupCommunicatorAction::Ok);
    });
    !residual_popup_communicator_is_visible()
}

/// Residual: ESC maps to Ok residual.
pub fn simulate_popup_communicator_esc() -> bool {
    with_popup_communicator_mut(None, |state| {
        residual_popcom_action_store(state, ResidualPopupCommunicatorAction::Esc);
    });
    simulate_popup_communicator_ok_button_gadget_selected();
    with_popup_communicator_mut(None, |state| {
        residual_popcom_action_store(state, ResidualPopupCommunicatorAction::Esc);
    });
    !residual_popup_communicator_is_visible()
}

/// Residual: show + Ok composite.
pub fn simulate_popup_communicator_prepare_ok() -> bool {
    if !simulate_popup_communicator_show() {
        return false;
    }
    simulate_popup_communicator_ok_button_gadget_selected()
}
