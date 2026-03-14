use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLWelcomeMenu.cpp",
    "crate::gui::callbacks::menus::wol_welcome_menu",
    "WOL Welcome Menu",
    "WOL welcome callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLWelcomeMenu",
    "WOL Welcome",
    "Online welcome and navigation screen.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolWelcomeMenuPort {
    pub server_name: String,
    pub players_online: u32,
    pub ladder_wins: u32,
    pub ladder_losses: u32,
    pub ladder_points: u32,
    pub ladder_rank: u32,
    pub disconnects: u32,
    pub info_items: Vec<String>,
}

impl Default for WolWelcomeMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolWelcomeMenuPort {
    pub fn sample() -> Self {
        Self {
            server_name: "World Online".to_string(),
            players_online: 12_842,
            ladder_wins: 42,
            ladder_losses: 18,
            ladder_points: 1465,
            ladder_rank: 312,
            disconnects: 1,
            info_items: vec![
                "Quick Match season is live.".to_string(),
                "Custom lobbies available.".to_string(),
                "Buddy list synchronized.".to_string(),
            ],
        }
    }
}
