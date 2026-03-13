use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/ScoreScreen.cpp",
    "crate::gui::callbacks::menus::score_screen",
    "Score Screen",
    "Post-match score callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "ScoreScreen",
    "Score Screen",
    "Post-match summary and performance breakdown.",
    "HUD",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreMetricPort {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ScoreScreenPort {
    pub player_name: String,
    pub result: String,
    pub rating: f32,
    pub metrics: Vec<ScoreMetricPort>,
}

impl Default for ScoreScreenPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ScoreScreenPort {
    pub fn sample() -> Self {
        Self {
            player_name: "bernardo".to_string(),
            result: "Victory".to_string(),
            rating: 0.74,
            metrics: vec![
                ScoreMetricPort {
                    label: "Units Lost".to_string(),
                    value: "54".to_string(),
                },
                ScoreMetricPort {
                    label: "Units Destroyed".to_string(),
                    value: "88".to_string(),
                },
                ScoreMetricPort {
                    label: "Structures".to_string(),
                    value: "12".to_string(),
                },
                ScoreMetricPort {
                    label: "Cash Float".to_string(),
                    value: "$3,412".to_string(),
                },
            ],
        }
    }
}
