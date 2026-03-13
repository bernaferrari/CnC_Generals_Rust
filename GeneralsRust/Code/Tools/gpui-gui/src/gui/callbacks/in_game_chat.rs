use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/InGameChat.cpp",
    "crate::gui::callbacks::in_game_chat",
    "In-Game Chat",
    "Ports chat overlay, entry, and history behavior for active gameplay.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "In-Game Chat",
    "Chat overlay and message-entry callbacks.",
);
