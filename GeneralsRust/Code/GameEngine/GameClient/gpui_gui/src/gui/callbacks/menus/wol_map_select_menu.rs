use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

use super::map_select_menu::MapSelectMenuPort;
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLMapSelectMenu.cpp",
    "crate::gui::callbacks::menus::wol_map_select_menu",
    "WOL Map Select Menu",
    "Online map-selection callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLMapSelectMenu",
    "WOL Maps",
    "Select maps for online sessions.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolMapSelectMenuPort {
    pub map_select: MapSelectMenuPort,
    pub rotation_name: String,
}

impl Default for WolMapSelectMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolMapSelectMenuPort {
    pub fn sample() -> Self {
        Self {
            map_select: MapSelectMenuPort::sample(),
            rotation_name: "Ranked 1v1 Rotation".to_string(),
        }
    }
}
