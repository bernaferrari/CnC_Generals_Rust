use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

use super::map_select_menu::MapSelectMenuPort;
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/LanMapSelectMenu.cpp",
    "crate::gui::callbacks::menus::lan_map_select_menu",
    "LAN Map Select Menu",
    "LAN map-selection callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "LanMapSelectMenu",
    "LAN Maps",
    "Choose the map for a LAN match.",
    "LAN",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanMapSelectMenuPort {
    pub map_select: MapSelectMenuPort,
    pub direct_connect_hint: String,
}

impl Default for LanMapSelectMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl LanMapSelectMenuPort {
    pub fn sample() -> Self {
        Self {
            map_select: MapSelectMenuPort::sample(),
            direct_connect_hint: "LAN maps include both official and shared custom maps."
                .to_string(),
        }
    }
}
