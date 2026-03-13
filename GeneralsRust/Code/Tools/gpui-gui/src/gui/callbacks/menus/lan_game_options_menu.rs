use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

use super::skirmish_game_options_menu::SkirmishGameOptionsMenuPort;
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/LanGameOptionsMenu.cpp",
    "crate::gui::callbacks::menus::lan_game_options_menu",
    "LAN Game Options Menu",
    "LAN game-options callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "LanGameOptionsMenu",
    "LAN Game Options",
    "Configure a LAN match before launch.",
    "LAN",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanGameOptionsMenuPort {
    pub setup: SkirmishGameOptionsMenuPort,
    pub local_address: String,
    pub broadcast_visible: bool,
}

impl Default for LanGameOptionsMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl LanGameOptionsMenuPort {
    pub fn sample() -> Self {
        let mut setup = SkirmishGameOptionsMenuPort::sample();
        setup.player_name = "lan-host".to_string();

        Self {
            setup,
            local_address: "192.168.1.44".to_string(),
            broadcast_visible: true,
        }
    }
}
