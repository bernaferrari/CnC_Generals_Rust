use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/DownloadMenu.cpp",
    "crate::gui::callbacks::menus::download_menu",
    "Download Menu",
    "Patch/download screen callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "DownloadMenu",
    "Download Menu",
    "Patch and download workflow.",
    "Shell",
);
