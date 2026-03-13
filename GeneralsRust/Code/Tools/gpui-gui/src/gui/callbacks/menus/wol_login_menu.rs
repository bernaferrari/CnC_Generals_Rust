use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLLoginMenu.cpp",
    "crate::gui::callbacks::menus::wol_login_menu",
    "WOL Login Menu",
    "WOL login callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLLoginMenu",
    "WOL Login",
    "Online account sign-in flow.",
    "WOL",
);
