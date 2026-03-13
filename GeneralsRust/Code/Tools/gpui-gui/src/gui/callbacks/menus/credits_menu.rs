use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/CreditsMenu.cpp",
    "crate::gui::callbacks::menus::credits_menu",
    "Credits Menu",
    "Credits screen callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "CreditsMenu",
    "Credits",
    "Scrolling credits and acknowledgements.",
    "Shell",
);
