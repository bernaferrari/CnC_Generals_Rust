use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLWelcomeMenu.cpp",
    "crate::gui::callbacks::menus::wol_welcome_menu",
    "WOL Welcome Menu",
    "WOL welcome callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLWelcomeMenu",
    "WOL Welcome",
    "Online welcome and navigation screen.",
    "WOL",
);
