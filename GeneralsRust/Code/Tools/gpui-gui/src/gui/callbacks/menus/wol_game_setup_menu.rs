use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLGameSetupMenu.cpp",
    "crate::gui::callbacks::menus::wol_game_setup_menu",
    "WOL Game Setup Menu",
    "Online game-setup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLGameSetupMenu",
    "WOL Game Setup",
    "Configure hosted online matches.",
    "WOL",
);
