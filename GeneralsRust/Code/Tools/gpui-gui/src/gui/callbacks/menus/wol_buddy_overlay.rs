use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLBuddyOverlay.cpp",
    "crate::gui::callbacks::menus::wol_buddy_overlay",
    "WOL Buddy Overlay",
    "Buddy overlay callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLBuddyOverlay",
    "Buddy Overlay",
    "Online buddy list overlay.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuddyEntryPort {
    pub name: String,
    pub status: String,
    pub unread_messages: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolBuddyOverlayPort {
    pub buddies: Vec<BuddyEntryPort>,
    pub selected_index: usize,
}

impl Default for WolBuddyOverlayPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolBuddyOverlayPort {
    pub fn sample() -> Self {
        Self {
            buddies: vec![
                BuddyEntryPort {
                    name: "CommanderFox".to_string(),
                    status: "In lobby".to_string(),
                    unread_messages: 0,
                },
                BuddyEntryPort {
                    name: "TankMaster".to_string(),
                    status: "Quick Match queue".to_string(),
                    unread_messages: 2,
                },
                BuddyEntryPort {
                    name: "DemoTrap".to_string(),
                    status: "Offline".to_string(),
                    unread_messages: 0,
                },
            ],
            selected_index: 1,
        }
    }
}
