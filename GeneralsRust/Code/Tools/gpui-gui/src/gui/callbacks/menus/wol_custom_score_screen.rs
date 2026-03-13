use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLCustomScoreScreen.cpp",
    "crate::gui::callbacks::menus::wol_custom_score_screen",
    "WOL Custom Score Screen",
    "Custom online score callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLCustomScoreScreen",
    "WOL Custom Score",
    "Custom online match score screen.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CustomScoreLinePort {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolCustomScoreScreenPort {
    pub player_name: String,
    pub opponent_name: String,
    pub result: String,
    pub score_lines: Vec<CustomScoreLinePort>,
}

impl Default for WolCustomScoreScreenPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolCustomScoreScreenPort {
    pub fn sample() -> Self {
        Self {
            player_name: "bernardo".to_string(),
            opponent_name: "CommanderFox".to_string(),
            result: "Victory".to_string(),
            score_lines: vec![
                CustomScoreLinePort {
                    label: "Units Destroyed".to_string(),
                    value: "88".to_string(),
                },
                CustomScoreLinePort {
                    label: "Structures Lost".to_string(),
                    value: "4".to_string(),
                },
                CustomScoreLinePort {
                    label: "Credits Floating".to_string(),
                    value: "$1,200".to_string(),
                },
            ],
        }
    }
}
