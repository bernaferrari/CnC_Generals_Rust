use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/ChallengeMenu.cpp",
    "crate::gui::callbacks::menus::challenge_menu",
    "Challenge Menu",
    "General's Challenge callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "ChallengeMenu",
    "Challenge Menu",
    "General selection and challenge progression.",
    "Shell",
);
