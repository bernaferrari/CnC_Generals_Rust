use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

use super::skirmish_game_options_menu::SkirmishGameOptionsMenuPort;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolGameSetupMenuPort {
    pub setup: SkirmishGameOptionsMenuPort,
    pub ladder_game: bool,
    pub stats_reporting: bool,
}

impl Default for WolGameSetupMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolGameSetupMenuPort {
    pub fn sample() -> Self {
        let mut setup = SkirmishGameOptionsMenuPort::sample();
        setup.player_name = "wol-host".to_string();

        Self {
            setup,
            ladder_game: true,
            stats_reporting: true,
        }
    }
}
