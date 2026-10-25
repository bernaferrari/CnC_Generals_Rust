//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use super::WindowEventEffects;

use crate::gui::gadgets::{ListBoxAddEntry, ListBoxItemData, ListBoxTextAndColor};

/// Window ID type for uniquely identifying windows
pub type WindowId = i32;

pub(crate) const KEY_STATE_UP: WindowMsgData = 0x0001;
pub(crate) const KEY_STATE_DOWN: WindowMsgData = 0x0002;
pub(crate) const KEY_STATE_LCONTROL: WindowMsgData = 0x0004;
pub(crate) const KEY_STATE_RCONTROL: WindowMsgData = 0x0008;
pub(crate) const KEY_STATE_LSHIFT: WindowMsgData = 0x0010;
pub(crate) const KEY_STATE_RSHIFT: WindowMsgData = 0x0020;
pub(crate) const KEY_STATE_LALT: WindowMsgData = 0x0040;
pub(crate) const KEY_STATE_RALT: WindowMsgData = 0x0080;
pub(crate) const GADGET_SIZE: i32 = 16;

/// Window message data type
///
/// C++ integer flags, gadget ids, and key states stay as plain `usize` values.
/// Typed out-params and owned strings are passed as arena tokens produced by
/// [`push_payload`] / [`with_payload`]. Tokens are never raw pointers.
pub type WindowMsgData = usize;

/// Owned payload stored for one receiving synchronous window operation.
///
/// Tokens are indices with a generation tag and a high bit set so random small
/// integers (`KEY_STATE_*`, gadget ids, `-1`) are never treated as payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowMsgPayload {
    None,
    Int(i32),
    UInt(usize),
    Bool(bool),
    Text(String),
    IntList(Vec<i32>),
    AddEntry(ListBoxAddEntry),
    CellPosition(ListBoxCellPosition),
    TextAndColor(ListBoxTextAndColor),
    ItemData(ListBoxItemData),
    ItemDataOpt(Option<ListBoxItemData>),
    RightClick(RightClickStruct),
}

pub(crate) const WINDOW_MSG_TOKEN_TAG: usize = 1usize << (usize::BITS - 1);
pub(crate) const WINDOW_MSG_INDEX_BITS: u32 = 24;
pub(crate) const WINDOW_MSG_INDEX_MASK: usize = (1usize << WINDOW_MSG_INDEX_BITS) - 1;
pub(crate) const WINDOW_MSG_GEN_SHIFT: u32 = WINDOW_MSG_INDEX_BITS;
pub(crate) const WINDOW_MSG_GEN_MASK: usize =
    (1usize << (usize::BITS - 1 - WINDOW_MSG_INDEX_BITS)) - 1;

pub(crate) struct WindowMsgPayloadSlot {
    generation: u32,
    value: Option<WindowMsgPayload>,
}

#[derive(Default)]
pub(crate) struct WindowMsgPayloadArena {
    slots: Vec<WindowMsgPayloadSlot>,
    free: Vec<usize>,
}

impl WindowMsgPayloadArena {
    fn encode_token(index: usize, generation: u32) -> WindowMsgData {
        WINDOW_MSG_TOKEN_TAG
            | (((generation as usize) & WINDOW_MSG_GEN_MASK) << WINDOW_MSG_GEN_SHIFT)
            | (index & WINDOW_MSG_INDEX_MASK)
    }

    fn decode_token(data: WindowMsgData) -> Option<(usize, u32)> {
        if data & WINDOW_MSG_TOKEN_TAG == 0 {
            return None;
        }
        let index = data & WINDOW_MSG_INDEX_MASK;
        let generation = ((data >> WINDOW_MSG_GEN_SHIFT) & WINDOW_MSG_GEN_MASK) as u32;
        if generation == 0 {
            return None;
        }
        Some((index, generation))
    }

    fn bump_generation(current: u32) -> u32 {
        let next = (current.wrapping_add(1) as usize & WINDOW_MSG_GEN_MASK) as u32;
        if next == 0 { 1 } else { next }
    }

    fn push(&mut self, payload: WindowMsgPayload) -> WindowMsgData {
        let index = if let Some(index) = self.free.pop() {
            index
        } else {
            let index = self.slots.len();
            assert!(
                index <= WINDOW_MSG_INDEX_MASK,
                "window message payload capacity exceeded"
            );
            self.slots.push(WindowMsgPayloadSlot {
                generation: 0,
                value: None,
            });
            index
        };
        let slot = &mut self.slots[index];
        slot.generation = Self::bump_generation(slot.generation);
        slot.value = Some(payload);
        Self::encode_token(index, slot.generation)
    }

    fn get(&self, data: WindowMsgData) -> Option<&WindowMsgPayload> {
        let (index, generation) = Self::decode_token(data)?;
        let slot = self.slots.get(index)?;
        if slot.generation != generation {
            return None;
        }
        slot.value.as_ref()
    }

    fn get_mut(&mut self, data: WindowMsgData) -> Option<&mut WindowMsgPayload> {
        let (index, generation) = Self::decode_token(data)?;
        let slot = self.slots.get_mut(index)?;
        if slot.generation != generation {
            return None;
        }
        slot.value.as_mut()
    }

    fn take(&mut self, data: WindowMsgData) -> Option<WindowMsgPayload> {
        let (index, generation) = Self::decode_token(data)?;
        let slot = self.slots.get_mut(index)?;
        if slot.generation != generation {
            return None;
        }
        let value = slot.value.take();
        slot.generation = Self::bump_generation(slot.generation);
        self.free.push(index);
        value
    }
}

/// Returns true when `data` is live in this receiving operation.
pub fn is_window_msg_payload(effects: &WindowEventEffects<'_>, data: WindowMsgData) -> bool {
    effects.payloads.get(data).is_some()
}

/// Push an owned payload for this synchronous operation.
pub fn push_payload(
    effects: &mut WindowEventEffects<'_>,
    payload: WindowMsgPayload,
) -> WindowMsgData {
    effects.payloads.push(payload)
}

pub fn payload(effects: &WindowEventEffects<'_>, data: WindowMsgData) -> Option<WindowMsgPayload> {
    effects.payloads.get(data).cloned()
}

pub fn pop_payload(
    effects: &mut WindowEventEffects<'_>,
    data: WindowMsgData,
) -> Option<WindowMsgPayload> {
    effects.payloads.take(data)
}

pub fn replace_payload(
    effects: &mut WindowEventEffects<'_>,
    data: WindowMsgData,
    payload: WindowMsgPayload,
) -> bool {
    if let Some(slot) = effects.payloads.get_mut(data) {
        *slot = payload;
        true
    } else {
        false
    }
}

/// Deliver a scoped payload, reclaiming it even when a nested callback unwinds.
pub fn with_payload<R>(
    effects: &mut WindowEventEffects<'_>,
    payload: WindowMsgPayload,
    f: impl FnOnce(&mut WindowEventEffects<'_>, WindowMsgData) -> R,
) -> R {
    with_payload_mut(effects, payload, f).0
}

pub fn with_payload_mut<R>(
    effects: &mut WindowEventEffects<'_>,
    payload: WindowMsgPayload,
    f: impl FnOnce(&mut WindowEventEffects<'_>, WindowMsgData) -> R,
) -> (R, Option<WindowMsgPayload>) {
    let token = push_payload(effects, payload);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(effects, token)));
    let payload = pop_payload(effects, token);
    match result {
        Ok(value) => (value, payload),
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

pub(crate) fn write_bool_payload(
    effects: &mut WindowEventEffects<'_>,
    data: WindowMsgData,
    value: bool,
) -> bool {
    replace_payload(effects, data, WindowMsgPayload::Bool(value))
}

pub(crate) fn payload_text(
    effects: &WindowEventEffects<'_>,
    data: WindowMsgData,
) -> Option<String> {
    match payload(effects, data) {
        Some(WindowMsgPayload::Text(text)) => Some(text),
        _ => None,
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ListBoxCellPosition {
    pub x: i32,
    pub y: i32,
}

/// C++ `RightClickStruct` sent as `GLM_RIGHT_CLICKED` data2.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RightClickStruct {
    pub pos: i32,
    pub mouse_x: i32,
    pub mouse_y: i32,
}

#[cfg(test)]
mod ownership_tests {
    use super::*;
    use crate::gui::game_window::{
        GameWindow, WindowMessage, WindowMsgHandled, write_input_focus_response,
    };
    use crate::gui::window_manager::WindowManager;

    #[test]
    fn repeated_payload_reuse_keeps_generation_encodable() {
        let mut messages = Vec::new();
        let mut effects = WindowEventEffects::new(&mut messages);
        for value in 0..300 {
            let token = push_payload(&mut effects, WindowMsgPayload::Int(value));
            assert_eq!(payload(&effects, token), Some(WindowMsgPayload::Int(value)));
            assert_eq!(
                pop_payload(&mut effects, token),
                Some(WindowMsgPayload::Int(value))
            );
            assert_eq!(payload(&effects, token), None);
            assert_eq!(pop_payload(&mut effects, token), None);
        }
        assert_eq!(
            WindowMsgPayloadArena::bump_generation(WINDOW_MSG_GEN_MASK as u32),
            1
        );
    }

    #[test]
    fn receiving_operations_with_identical_tokens_do_not_share_payloads() {
        let mut messages_a = Vec::new();
        let mut messages_b = Vec::new();
        let mut a = WindowEventEffects::new(&mut messages_a);
        let mut b = WindowEventEffects::new(&mut messages_b);
        let token_a = push_payload(&mut a, WindowMsgPayload::Text("A".into()));
        let token_b = push_payload(&mut b, WindowMsgPayload::Text("B".into()));
        assert_eq!(token_a, token_b);
        assert!(replace_payload(&mut b, token_b, WindowMsgPayload::Int(7)));
        assert_eq!(pop_payload(&mut b, token_b), Some(WindowMsgPayload::Int(7)));
        drop(b);
        assert_eq!(
            payload(&a, token_a),
            Some(WindowMsgPayload::Text("A".into()))
        );
        assert_eq!(
            pop_payload(&mut a, token_a),
            Some(WindowMsgPayload::Text("A".into()))
        );
    }

    #[test]
    fn nested_manager_borrow_keeps_outer_payload_and_cleans_up_on_panic() {
        let mut messages = Vec::new();
        let mut effects = WindowEventEffects::new(&mut messages);
        let mut manager = WindowManager::new();
        let outer = push_payload(&mut effects, WindowMsgPayload::Int(1));
        let mut inner = 0;
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            effects.with_receiving_manager(&mut manager, |effects| {
                let nested = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    effects.with_manager(|_, effects| {
                        with_payload(effects, WindowMsgPayload::Int(2), |effects, token| {
                            inner = token;
                            assert_eq!(payload(effects, outer), Some(WindowMsgPayload::Int(1)));
                            assert!(replace_payload(effects, outer, WindowMsgPayload::Int(3)));
                            panic!("nested callback");
                        });
                    });
                }));
                assert!(
                    effects.manager.is_some(),
                    "caught callback must retain its receiving manager"
                );
                assert!(!is_window_msg_payload(effects, inner));
                effects.with_manager(|_, effects| {
                    assert_eq!(payload(effects, outer), Some(WindowMsgPayload::Int(3)));
                });
                std::panic::resume_unwind(nested.unwrap_err());
            });
        }));
        assert!(unwind.is_err());
        assert!(!is_window_msg_payload(&effects, inner));
        assert_eq!(
            pop_payload(&mut effects, outer),
            Some(WindowMsgPayload::Int(3))
        );
    }

    #[test]
    fn nested_focus_response_is_visible_before_the_caller_continues() {
        let mut messages = Vec::new();
        let mut effects = WindowEventEffects::new(&mut messages);
        let mut parent = GameWindow::new();
        parent.set_system_callback_with_message_sink(|_, msg, _, outer, effects| {
            assert_eq!(msg, WindowMessage::InputFocus);
            let mut child = GameWindow::new();
            child.set_system_callback(|_, _, data1, data2| {
                write_input_focus_response(data1, data2, true)
            });
            let (_, child_value) =
                with_payload_mut(effects, WindowMsgPayload::Bool(false), |effects, inner| {
                    assert_eq!(
                        child.send_system_message(WindowMessage::InputFocus, 1, inner, effects),
                        WindowMsgHandled::Handled
                    );
                    assert_eq!(payload(effects, inner), Some(WindowMsgPayload::Bool(true)));
                    assert_eq!(payload(effects, outer), Some(WindowMsgPayload::Bool(false)));
                });
            assert_eq!(child_value, Some(WindowMsgPayload::Bool(true)));
            write_input_focus_response(1, outer, true)
        });
        let (handled, value) = with_payload_mut(
            &mut effects,
            WindowMsgPayload::Bool(false),
            |effects, token| {
                parent.send_system_message(WindowMessage::InputFocus, 1, token, effects)
            },
        );
        assert_eq!(handled, WindowMsgHandled::Handled);
        assert_eq!(value, Some(WindowMsgPayload::Bool(true)));
        // A raw C++ integer is never dereferenced or admitted as a typed payload.
        let mut raw = GameWindow::new();
        raw.set_system_callback(|_, _, d1, d2| write_input_focus_response(d1, d2, true));
        assert_eq!(
            raw.send_system_message(WindowMessage::InputFocus, 1, 17, &mut effects),
            WindowMsgHandled::Handled
        );
        assert!(!is_window_msg_payload(&effects, 17));
    }
}
