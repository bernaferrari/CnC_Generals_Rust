use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/ExtendedMessageBox.cpp",
    "crate::gui::callbacks::extended_message_box",
    "Extended Message Box",
    "Ports richer prompt and confirmation flows beyond the basic message box.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Extended Message Box",
    "Extended confirmation and prompt callback layer.",
);
