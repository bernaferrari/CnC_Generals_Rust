//! MessageBox.cpp callbacks borrow metadata only between user callbacks.
use super::*;
use crate::gui::game_window::WindowEventEffects;
#[derive(Clone, Copy)]
pub(super) enum MessageBoxLane {
    Standard,
    Extended,
    Quit,
}
fn base(owner: &mut MessageBoxSystem, lane: MessageBoxLane) -> &mut MessageBoxCallbacks {
    match lane {
        MessageBoxLane::Standard => &mut owner.standard,
        MessageBoxLane::Extended => &mut owner.extended.base,
        MessageBoxLane::Quit => &mut owner.quit.base,
    }
}
pub(super) fn bind_created(
    lane: MessageBoxLane,
    window: &Rc<RefCell<GameWindow>>,
    title: &str,
    message: &str,
    flags: u16,
    effects: &mut WindowEventEffects<'_>,
) {
    let id = window.borrow().get_id();
    effects.with_message_boxes(|owner| {
        let state = base(owner, lane);
        state.generation = state.generation.wrapping_add(1);
        state.window = Some(Rc::downgrade(window));
        state.window_id = Some(id);
        state.title = title.into();
        state.message = message.into();
        state.message_type = if flags & MSG_BOX_YES != 0 {
            if flags & MSG_BOX_CANCEL != 0 {
                MessageBoxType::YesNoCancel
            } else {
                MessageBoxType::YesNo
            }
        } else if flags & MSG_BOX_CANCEL != 0 && flags & MSG_BOX_OK != 0 {
            MessageBoxType::OkCancel
        } else {
            MessageBoxType::Ok
        };
        state.result = None;
        state.visible = true;
    });
}
fn button(control: WindowId, lane: MessageBoxLane) -> Option<MessageBoxResult> {
    let prefix = if matches!(lane, MessageBoxLane::Quit) {
        "QuitMessageBox.wnd"
    } else {
        "MessageBox.wnd"
    };
    for (suffix, result) in [
        ("Ok", MessageBoxResult::Ok),
        ("Yes", MessageBoxResult::Yes),
        ("No", MessageBoxResult::No),
        ("Cancel", MessageBoxResult::Cancel),
    ] {
        if control == NameKeyGenerator::name_to_key(&format!("{prefix}:Button{suffix}")) as WindowId
        {
            return Some(result);
        }
    }
    None
}
fn registered(
    window: &GameWindow,
    effects: &mut WindowEventEffects<'_>,
) -> Option<Rc<RefCell<GameWindow>>> {
    effects.with_manager_ref(|m| {
        m.get_window_by_id_with_selected(window.get_id(), Some(window))
            .filter(|h| std::ptr::eq(h.as_ptr(), std::ptr::from_ref(window)))
    })
}
fn metadata_generation(
    window: &GameWindow,
    lane: MessageBoxLane,
    effects: &mut WindowEventEffects<'_>,
) -> Option<u64> {
    effects.with_message_boxes(|o| {
        let state = base(o, lane);
        state
            .window
            .as_ref()
            .and_then(std::rc::Weak::upgrade)
            .is_some_and(|w| std::ptr::eq(w.as_ref().as_ptr(), std::ptr::from_ref(window)))
            .then_some(state.generation)
    })
}
fn update_current(
    lane: MessageBoxLane,
    generation: Option<u64>,
    effects: &mut WindowEventEffects<'_>,
    operation: impl FnOnce(&mut MessageBoxCallbacks),
) {
    effects.with_message_boxes(|o| {
        let state = base(o, lane);
        if generation == Some(state.generation) {
            operation(state)
        }
    })
}
/// An owned callback phase, never a moved/default replacement of menu state.
/// The window keeps the exact same Rc userdata visible during user callbacks.
pub(crate) struct PendingMessageBox {
    lane: MessageBoxLane,
    window: Rc<RefCell<GameWindow>>,
    generation: Option<u64>,
    result: Option<MessageBoxResult>,
    callbacks: PendingCallbacks,
}
enum PendingCallbacks {
    Standard(Option<Rc<WindowMessageBoxData>>),
    Extended(Option<Rc<WindowExMessageBoxData>>),
}
impl PendingMessageBox {
    pub(crate) fn complete(self, effects: &mut WindowEventEffects<'_>) {
        let Self {
            lane,
            window,
            generation,
            result,
            callbacks,
        } = self;
        let close = match callbacks {
            PendingCallbacks::Standard(callbacks) => {
                if let (Some(callbacks), Some(result)) = (callbacks, result.as_ref()) {
                    match result {
                        MessageBoxResult::Ok => callbacks.call_ok(effects),
                        MessageBoxResult::Yes => callbacks.call_yes(effects),
                        MessageBoxResult::No => callbacks.call_no(effects),
                        MessageBoxResult::Cancel => callbacks.call_cancel(effects),
                        _ => {}
                    }
                }
                update_current(lane, generation, effects, |state| {
                    state.result = result;
                    state.visible = false;
                });
                true
            }
            PendingCallbacks::Extended(callbacks) => {
                // Original Rust-facing extended result was observable before
                // the user function; C++ user function still decides closing.
                if let Some(result) = result.clone() {
                    update_current(lane, generation, effects, |state| {
                        state.result = Some(result)
                    });
                }
                let response = if let (Some(callbacks), Some(result)) = (callbacks, result) {
                    match result {
                        MessageBoxResult::Ok => callbacks.call_ok(effects),
                        MessageBoxResult::Yes => callbacks.call_yes(effects),
                        MessageBoxResult::No => callbacks.call_no(effects),
                        MessageBoxResult::Cancel => callbacks.call_cancel(effects),
                        _ => MessageBoxReturnType::Close,
                    }
                } else {
                    MessageBoxReturnType::Close
                };
                let close = response == MessageBoxReturnType::Close;
                update_current(lane, generation, effects, |state| state.visible = !close);
                close
            }
        };
        if close {
            effects.with_manager(|manager, effects| {
                let _ = manager.destroy_window(window);
                manager.flush_destroy_queue(effects);
            });
        }
    }
}
fn prepare(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
    lane: MessageBoxLane,
) -> WindowMsgHandled {
    if msg == WindowMessage::InputFocus {
        return if data1 != 0 {
            write_input_focus_response(data1, data2, true)
        } else {
            WindowMsgHandled::Ignored
        };
    }
    let generation = metadata_generation(window, lane, effects);
    if msg == WindowMessage::Destroy {
        window.user_data = None;
        update_current(lane, generation, effects, |state| {
            state.visible = false;
            state.window_id = None;
            state.window = None;
        });
        return WindowMsgHandled::Handled;
    }
    if msg != WindowMessage::GadgetSelected {
        return WindowMsgHandled::Ignored;
    }
    let Some(handle) = registered(window, effects) else {
        return WindowMsgHandled::Ignored;
    };
    let result = button(data1 as WindowId, lane);
    if result.is_none() && !matches!(lane, MessageBoxLane::Extended) {
        return WindowMsgHandled::Handled;
    }
    let callbacks = if matches!(lane, MessageBoxLane::Extended) {
        PendingCallbacks::Extended(
            window
                .get_user_data::<Rc<WindowExMessageBoxData>>()
                .cloned(),
        )
    } else {
        PendingCallbacks::Standard(window.get_user_data::<Rc<WindowMessageBoxData>>().cloned())
    };
    effects.queue_window_callback(crate::gui::game_window::PendingWindowCallback::MessageBox(
        PendingMessageBox {
            lane,
            window: handle,
            generation,
            result,
            callbacks,
        },
    ));
    WindowMsgHandled::Handled
}
pub fn message_box_system_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    prepare(window, msg, data1, data2, effects, MessageBoxLane::Standard)
}
pub fn quit_message_box_system_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    prepare(window, msg, data1, data2, effects, MessageBoxLane::Quit)
}
pub fn extended_message_box_system_receiving(
    window: &mut GameWindow,
    msg: WindowMessage,
    data1: WindowMsgData,
    data2: WindowMsgData,
    effects: &mut WindowEventEffects<'_>,
) -> WindowMsgHandled {
    prepare(window, msg, data1, data2, effects, MessageBoxLane::Extended)
}
