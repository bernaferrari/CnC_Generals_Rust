use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/InGamePopupMessage.cpp",
    "crate::gui::callbacks::in_game_popup_message",
    "In-Game Popup Message",
    "Ports transient popup messaging shown during gameplay events.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Popup Message",
    "Transient in-game popup messaging callbacks.",
);
