use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

use super::map_select_menu::MapSelectMenuPort;
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/SkirmishMapSelectMenu.cpp",
    "crate::gui::callbacks::menus::skirmish_map_select_menu",
    "Skirmish Map Select Menu",
    "Skirmish map selection callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SkirmishMapSelectMenu",
    "Skirmish Maps",
    "Select a skirmish battleground.",
    "Shell",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkirmishMapSelectMenuPort {
    pub map_select: MapSelectMenuPort,
    pub official_maps_only: bool,
}

impl Default for SkirmishMapSelectMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl SkirmishMapSelectMenuPort {
    pub fn sample() -> Self {
        let mut map_select = MapSelectMenuPort::sample();
        map_select.uses_system_map_dir = true;

        Self {
            map_select,
            official_maps_only: true,
        }
    }
}
