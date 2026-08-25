//! Shim for ExtendedMessageBox.cpp.

pub use crate::gui::callbacks::message_box::{
    ExtendedMessageBoxFunc, MessageBoxReturnType, ex_message_box_cancel, ex_message_box_ok,
    ex_message_box_ok_cancel, ex_message_box_yes_no, ex_message_box_yes_no_cancel,
};
