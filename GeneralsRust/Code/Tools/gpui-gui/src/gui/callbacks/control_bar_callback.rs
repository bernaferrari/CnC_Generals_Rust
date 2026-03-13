use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/ControlBarCallback.cpp",
    "crate::gui::callbacks::control_bar_callback",
    "Control Bar Callback",
    "Routes gadget and command-bar messages into gameplay-facing control bar handlers.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Control Bar Callback",
    "Owner callback entry point for command-bar messages.",
);
