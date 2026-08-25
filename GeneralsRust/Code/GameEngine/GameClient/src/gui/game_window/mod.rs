//! GameWindow Implementation
//!
//! This module provides the `GameWindow` struct, which represents individual UI windows
//! and controls in the game's windowing system. It handles window properties, hierarchy,
//! event callbacks, and drawing.
//!
//! Split into focused submodules. Public API is identical to the former
//! `gui/game_window.rs` god-file.

mod callbacks;
mod combo;
mod font;
mod listbox;
mod messages;
mod other_gadgets;
mod payload;
mod window_impl_core;
mod window_impl_draw;
mod window_impl_input;
mod window_impl_widgets;
mod window_struct;

#[cfg(test)]
mod tests;

/// Shared imports for split impl / gadget helper files.
mod prelude {
    #![allow(unused_imports)]

    pub(super) use super::callbacks::*;
    pub(super) use super::combo::*;
    pub(super) use super::font::*;
    pub(super) use super::listbox::*;
    pub(super) use super::messages::*;
    pub(super) use super::other_gadgets::*;
    pub(super) use super::payload::*;
    pub(super) use super::window_struct::*;

    pub(super) use std::cell::{Cell, RefCell};
    pub(super) use std::fmt;
    pub(super) use std::rc::{Rc, Weak};
    pub(super) use std::sync::Arc;
    pub(super) use std::sync::OnceLock;

    pub(super) use crate::display::image::{
        ensure_client_mapped_image, get_mapped_image_collection,
    };
    pub(super) use crate::game_text::GameText;
    pub(super) use crate::gui::window_manager::{
        TabDirection, queue_window_manager_op, queue_window_manager_op_deferred,
        with_window_manager,
    };
    pub(super) use crate::video_buffer::{VideoBufferHandle, VideoBufferType};

    pub(super) use super::super::gadgets::{
        CheckBox, Color as GadgetColor, ComboBox, ComboBoxItem, Gadget, GadgetMessage, GadgetState,
        GadgetValue, HorizontalSlider, InputEvent, KeyCode, KeyModifiers, ListBox, ListBoxAddEntry,
        ListBoxItemData, ListBoxSelection, ListBoxTextAndColor, MouseButton, ProgressBar,
        PushButton, RadioButton, SelectionMode, StaticText, TabControl, TabControlData, TextEntry,
        ValidationMode, VerticalSlider,
    };
    pub(super) use super::super::{
        MAX_DRAW_DATA, TOOLTIP_DELAY, TOOLTIP_MAX_LEN, UIRect, WindowLayout,
        display_string::DisplayStringHandle,
        font::{FontDesc, get_font_library},
        get_display_string_manager, with_ui_renderer_mut, with_window_manager_ref,
    };
}

pub use callbacks::*;
pub use combo::*;
pub use font::*;
pub use listbox::*;
pub use messages::*;
pub use payload::*;
pub use window_struct::*;

/// Concatenated live sources for residual `include_str!` scans.
pub const GAME_WINDOW_SRC: &str = concat!(
    include_str!("mod.rs"),
    include_str!("payload.rs"),
    include_str!("messages.rs"),
    include_str!("font.rs"),
    include_str!("listbox.rs"),
    include_str!("combo.rs"),
    include_str!("other_gadgets.rs"),
    include_str!("window_struct.rs"),
    include_str!("window_impl_core.rs"),
    include_str!("window_impl_widgets.rs"),
    include_str!("window_impl_input.rs"),
    include_str!("window_impl_draw.rs"),
    include_str!("callbacks.rs"),
);
