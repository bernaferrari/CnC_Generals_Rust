use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLLadderScreen.cpp",
    "crate::gui::callbacks::menus::wol_ladder_screen",
    "WOL Ladder Screen",
    "WOL ladder callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLLadderScreen",
    "WOL Ladder",
    "Online ladder ranking screen.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LadderStandingPort {
    pub rank: u32,
    pub player_name: String,
    pub points: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolLadderScreenPort {
    pub season_label: String,
    pub local_rank: u32,
    pub standings: Vec<LadderStandingPort>,
}

impl Default for WolLadderScreenPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolLadderScreenPort {
    pub fn sample() -> Self {
        Self {
            season_label: "Season 12".to_string(),
            local_rank: 312,
            standings: vec![
                LadderStandingPort {
                    rank: 1,
                    player_name: "TopGeneral".to_string(),
                    points: 2012,
                },
                LadderStandingPort {
                    rank: 2,
                    player_name: "ChinaNuke".to_string(),
                    points: 1987,
                },
                LadderStandingPort {
                    rank: 3,
                    player_name: "StealthRush".to_string(),
                    points: 1940,
                },
            ],
        }
    }
}
