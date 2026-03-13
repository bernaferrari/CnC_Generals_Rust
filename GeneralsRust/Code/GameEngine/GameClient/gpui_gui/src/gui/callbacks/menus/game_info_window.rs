use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/GameInfoWindow.cpp",
    "crate::gui::callbacks::menus::game_info_window",
    "Game Info Window",
    "Game info popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "GameInfoWindow",
    "Game Info",
    "Read-only game details popup.",
    "Popup",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GameInfoWindowPort {
    pub game_name: String,
    pub map_name: String,
    pub host_name: String,
    pub player_counts: (u8, u8),
    pub rule_lines: Vec<String>,
    pub download_required: bool,
}

impl Default for GameInfoWindowPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl GameInfoWindowPort {
    pub fn sample() -> Self {
        Self {
            game_name: "2v2 Tournament Desert".to_string(),
            map_name: "Tournament Desert".to_string(),
            host_name: "HostPlayer".to_string(),
            player_counts: (3, 4),
            rule_lines: vec![
                "Starting cash: $10,000".to_string(),
                "Superweapons: enabled".to_string(),
                "Observers: allowed".to_string(),
            ],
            download_required: false,
        }
    }
}
