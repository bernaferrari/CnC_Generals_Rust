use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/MessageBox.cpp",
    "crate::gui::callbacks::message_box",
    "Message Box",
    "Ports basic prompt, yes-no, ok-cancel, and close callbacks.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Message Box",
    "Standard prompt and dialog callbacks.",
);
