use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/WOLQMScoreScreen.cpp",
    "crate::gui::callbacks::menus::wol_qm_score_screen",
    "WOL QM Score Screen",
    "Quick-match score callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "WOLQMScoreScreen",
    "WOL QM Score",
    "Quick-match post-game score screen.",
    "WOL",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WolQmScoreScreenPort {
    pub rating_before: i32,
    pub rating_after: i32,
    pub streak: i32,
    pub summary_lines: Vec<String>,
}

impl Default for WolQmScoreScreenPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl WolQmScoreScreenPort {
    pub fn rating_delta(&self) -> i32 {
        self.rating_after - self.rating_before
    }

    pub fn sample() -> Self {
        Self {
            rating_before: 1465,
            rating_after: 1482,
            streak: 3,
            summary_lines: vec![
                "Quick Match result: Victory".to_string(),
                "Faction matchup: USA vs China".to_string(),
            ],
        }
    }
}
