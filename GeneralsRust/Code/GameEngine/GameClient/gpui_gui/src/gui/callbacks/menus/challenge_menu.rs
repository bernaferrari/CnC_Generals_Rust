use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/ChallengeMenu.cpp",
    "crate::gui::callbacks::menus::challenge_menu",
    "Challenge Menu",
    "General's Challenge callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "ChallengeMenu",
    "Challenge Menu",
    "General selection and challenge progression.",
    "Shell",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChallengeGeneralPort {
    pub name: String,
    pub enabled: bool,
    pub campaign: String,
    pub bio_name: String,
    pub bio_rank: String,
    pub bio_branch: String,
    pub bio_strategy: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChallengeMenuPort {
    pub selected_general: usize,
    pub teletype_position: usize,
    pub intro_sequence_step: usize,
    pub just_entered: bool,
    pub can_play: bool,
    pub generals: Vec<ChallengeGeneralPort>,
}

impl Default for ChallengeMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ChallengeMenuPort {
    pub fn select_general(&mut self, index: usize) -> bool {
        if index >= self.generals.len() || !self.generals[index].enabled {
            return false;
        }
        self.selected_general = index;
        self.teletype_position = 0;
        self.can_play = true;
        true
    }

    pub fn update_bio(&mut self, frames: usize, skip: usize) {
        let total = self.current_bio_text().chars().count();
        self.teletype_position = (self.teletype_position + frames * skip).min(total);
    }

    pub fn current_bio_lines(&self) -> [String; 4] {
        let general = &self.generals[self.selected_general];
        [
            general.bio_name.clone(),
            general.bio_rank.clone(),
            general.bio_branch.clone(),
            general.bio_strategy.clone(),
        ]
    }

    pub fn current_bio_text(&self) -> String {
        self.current_bio_lines().join("")
    }

    pub fn current_readout(&self) -> String {
        self.current_bio_text()
            .chars()
            .take(self.teletype_position)
            .collect()
    }

    pub fn sample() -> Self {
        Self {
            selected_general: 0,
            teletype_position: 0,
            intro_sequence_step: 0,
            just_entered: true,
            can_play: true,
            generals: vec![
                ChallengeGeneralPort {
                    name: "General Alexander".to_string(),
                    enabled: true,
                    campaign: "Challenge_Alexander".to_string(),
                    bio_name: "Name: Alexander".to_string(),
                    bio_rank: "Rank: General".to_string(),
                    bio_branch: "Branch: USA".to_string(),
                    bio_strategy: "Strategy: Superweapons and defensive fortification.".to_string(),
                },
                ChallengeGeneralPort {
                    name: "General Leang".to_string(),
                    enabled: false,
                    campaign: "Challenge_Leang".to_string(),
                    bio_name: "Name: Leang".to_string(),
                    bio_rank: "Rank: General".to_string(),
                    bio_branch: "Branch: Boss".to_string(),
                    bio_strategy: "Strategy: Unknown".to_string(),
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_enabled_general_resets_teletype() {
        let mut menu = ChallengeMenuPort::sample();
        menu.teletype_position = 20;

        assert!(menu.select_general(0));
        assert_eq!(menu.teletype_position, 0);
    }

    #[test]
    fn teletype_update_reveals_bio_incrementally() {
        let mut menu = ChallengeMenuPort::sample();
        menu.update_bio(2, 2);

        assert_eq!(menu.current_readout().chars().count(), 4);
    }
}
