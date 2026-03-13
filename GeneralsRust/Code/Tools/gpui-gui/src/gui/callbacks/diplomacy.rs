use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Diplomacy.cpp",
    "crate::gui::callbacks::diplomacy",
    "Diplomacy Callback",
    "Ports diplomacy overlay interactions and alliance-state UI callbacks.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Diplomacy",
    "Diplomacy overlay and alliance callbacks.",
);
