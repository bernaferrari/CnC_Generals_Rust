use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/GeneralsExpPoints.cpp",
    "crate::gui::callbacks::generals_exp_points",
    "Generals Experience Points",
    "Ports general-points presentation and rank progression callback logic.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "General Points",
    "General points and promotion callback logic.",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GeneralsExpPointsPort {
    pub visible: bool,
    pub current_rank: u8,
    pub earned_points: u8,
    pub spent_points: u8,
    pub progress_to_next_rank_pct: u8,
    pub purchase_science_hidden: bool,
}

impl Default for GeneralsExpPointsPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl GeneralsExpPointsPort {
    pub fn sample() -> Self {
        Self {
            visible: true,
            current_rank: 3,
            earned_points: 5,
            spent_points: 3,
            progress_to_next_rank_pct: 58,
            purchase_science_hidden: false,
        }
    }

    pub fn available_points(&self) -> u8 {
        self.earned_points.saturating_sub(self.spent_points)
    }

    pub fn exit(&mut self) {
        self.visible = false;
        self.purchase_science_hidden = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exit_hides_purchase_science_window() {
        let mut state = GeneralsExpPointsPort::sample();
        state.exit();

        assert!(!state.visible);
        assert!(state.purchase_science_hidden);
    }
}
