use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};
pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/DifficultySelect.cpp",
    "crate::gui::callbacks::menus::difficulty_select",
    "Difficulty Select",
    "Difficulty popup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "DifficultySelect",
    "Difficulty Select",
    "Difficulty-selection popup.",
    "Popup",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DifficultyChoicePort {
    Easy,
    Medium,
    Hard,
}

impl DifficultyChoicePort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Easy => "Easy",
            Self::Medium => "Medium",
            Self::Hard => "Hard",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DifficultySelectPort {
    pub selected: DifficultyChoicePort,
    pub last_confirmed: DifficultyChoicePort,
    pub solo_campaign: bool,
    pub description: Vec<String>,
}

impl Default for DifficultySelectPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl DifficultySelectPort {
    pub fn choose(&mut self, choice: DifficultyChoicePort) {
        self.selected = choice;
    }

    pub fn confirm(&mut self) {
        self.last_confirmed = self.selected;
    }

    pub fn sample() -> Self {
        Self {
            selected: DifficultyChoicePort::Hard,
            last_confirmed: DifficultyChoicePort::Medium,
            solo_campaign: true,
            description: vec![
                "Enemy AI reacts faster and scouts more aggressively.".to_string(),
                "Resource pressure matches the original shell difficulty popup.".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirming_updates_last_confirmed_choice() {
        let mut popup = DifficultySelectPort::sample();
        popup.choose(DifficultyChoicePort::Easy);
        popup.confirm();

        assert_eq!(popup.last_confirmed, DifficultyChoicePort::Easy);
    }
}
