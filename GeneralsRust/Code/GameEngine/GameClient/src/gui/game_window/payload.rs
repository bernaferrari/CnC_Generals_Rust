//! Split from `gui/game_window.rs` for module-size parity.
//! Observable window behavior is unchanged.

use std::cell::RefCell;

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

/// Owned payload stored in the thread-local window-message arena.
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

pub(crate) struct WindowMsgPayloadArena {
    slots: Vec<WindowMsgPayloadSlot>,
    free: Vec<usize>,
}

impl WindowMsgPayloadArena {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            free: Vec::new(),
        }
    }

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
        let next = current.wrapping_add(1);
        if next == 0 { 1 } else { next }
    }

    fn push(&mut self, payload: WindowMsgPayload) -> WindowMsgData {
        let index = if let Some(index) = self.free.pop() {
            index
        } else {
            let index = self.slots.len();
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

thread_local! {
    static WINDOW_MSG_PAYLOAD_ARENA: RefCell<WindowMsgPayloadArena> =
        RefCell::new(WindowMsgPayloadArena::new());
}

/// Returns true when `data` is a live typed payload token (not an integer flag).
pub fn is_window_msg_payload(data: WindowMsgData) -> bool {
    WINDOW_MSG_PAYLOAD_ARENA.with(|arena| arena.borrow().get(data).is_some())
}

/// Push an owned payload and return a token suitable for `WindowMsgData`.
pub fn push_payload(payload: WindowMsgPayload) -> WindowMsgData {
    WINDOW_MSG_PAYLOAD_ARENA.with(|arena| arena.borrow_mut().push(payload))
}

/// Clone the payload for `data` if it is a live token.
pub fn payload(data: WindowMsgData) -> Option<WindowMsgPayload> {
    WINDOW_MSG_PAYLOAD_ARENA.with(|arena| arena.borrow().get(data).cloned())
}

/// Take (and free) the payload for `data` if it is a live token.
pub fn pop_payload(data: WindowMsgData) -> Option<WindowMsgPayload> {
    WINDOW_MSG_PAYLOAD_ARENA.with(|arena| arena.borrow_mut().take(data))
}

/// Replace the payload stored at `data`. Returns false if `data` is not a token.
pub fn replace_payload(data: WindowMsgData, payload: WindowMsgPayload) -> bool {
    WINDOW_MSG_PAYLOAD_ARENA.with(|arena| {
        if let Some(slot) = arena.borrow_mut().get_mut(data) {
            *slot = payload;
            true
        } else {
            false
        }
    })
}

/// Push `payload`, invoke `f`, then pop the token (even if `f` panics).
pub fn with_payload<R>(payload: WindowMsgPayload, f: impl FnOnce(WindowMsgData) -> R) -> R {
    let token = push_payload(payload);
    struct PopOnDrop(WindowMsgData);
    impl Drop for PopOnDrop {
        fn drop(&mut self) {
            let _ = pop_payload(self.0);
        }
    }
    let _guard = PopOnDrop(token);
    f(token)
}

/// Push `payload`, invoke `f`, then return both the result and the (possibly updated) payload.
pub fn with_payload_mut<R>(
    payload: WindowMsgPayload,
    f: impl FnOnce(WindowMsgData) -> R,
) -> (R, Option<WindowMsgPayload>) {
    let token = push_payload(payload);
    let result = f(token);
    (result, pop_payload(token))
}

pub(crate) fn write_bool_payload(data: WindowMsgData, value: bool) -> bool {
    replace_payload(data, WindowMsgPayload::Bool(value))
}

pub(crate) fn payload_text(data: WindowMsgData) -> Option<String> {
    match payload(data) {
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
