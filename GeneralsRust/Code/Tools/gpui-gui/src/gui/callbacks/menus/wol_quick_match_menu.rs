use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLQuickMatchMenu.cpp",
    "crate::gui::callbacks::menus::wol_quick_match_menu",
    "WOL Quick Match Menu",
    "Quick-match callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLQuickMatchMenu",
    "Quick Match",
    "Quick-match setup and queueing.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolQuickMatchMenuPort {
    pub preferred_faction: String,
    pub map_pool: Vec<String>,
    pub queue_state: String,
    pub estimated_wait_seconds: u32,
}

impl Default for WolQuickMatchMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolQuickMatchMenuPort {
    pub fn sample() -> Self {
        Self {
            preferred_faction: "Random".to_string(),
            map_pool: vec![
                "Tournament Desert".to_string(),
                "Forgotten Forest".to_string(),
                "Defcon 6".to_string(),
            ],
            queue_state: "Searching".to_string(),
            estimated_wait_seconds: 27,
        }
    }
}
