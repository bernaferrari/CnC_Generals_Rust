use crate::gui::source_catalog::{CallbackPort, GuiPortRecord};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Diplomacy.cpp",
    "crate::gui::callbacks::diplomacy",
    "Diplomacy Callback",
    "Ports diplomacy overlay interactions and alliance-state UI callbacks.",
);

pub const PORT: CallbackPort = CallbackPort::new(
    &RECORD,
    "Diplomacy",
    "Diplomacy overlay and alliance callbacks.",
);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiplomacyTabPort {
    InGame,
    Buddies,
    Solo,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiplomacyRelationPort {
    Allied,
    Neutral,
    Enemy,
}

impl DiplomacyRelationPort {
    pub fn label(self) -> &'static str {
        match self {
            Self::Allied => "Allied",
            Self::Neutral => "Neutral",
            Self::Enemy => "Enemy",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiplomacyPlayerPort {
    pub name: String,
    pub side: String,
    pub team: u8,
    pub relation: DiplomacyRelationPort,
    pub muted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiplomacyPort {
    pub visible: bool,
    pub active_tab: DiplomacyTabPort,
    pub players: Vec<DiplomacyPlayerPort>,
    pub solo_briefing_lines: Vec<String>,
    pub selected_player: Option<usize>,
}

impl Default for DiplomacyPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl DiplomacyPort {
    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn set_tab(&mut self, tab: DiplomacyTabPort) {
        self.active_tab = tab;
    }

    pub fn toggle_mute(&mut self, index: usize) -> bool {
        let Some(player) = self.players.get_mut(index) else {
            return false;
        };
        player.muted = !player.muted;
        self.selected_player = Some(index);
        true
    }

    pub fn update_briefing_text(&mut self, new_text: impl Into<String>, clear: bool) {
        if clear {
            self.solo_briefing_lines.clear();
        }
        self.solo_briefing_lines.push(new_text.into());
    }

    pub fn sample() -> Self {
        Self {
            visible: true,
            active_tab: DiplomacyTabPort::InGame,
            players: vec![
                DiplomacyPlayerPort {
                    name: "USA".to_string(),
                    side: "USA".to_string(),
                    team: 1,
                    relation: DiplomacyRelationPort::Allied,
                    muted: false,
                },
                DiplomacyPlayerPort {
                    name: "China".to_string(),
                    side: "China".to_string(),
                    team: 2,
                    relation: DiplomacyRelationPort::Neutral,
                    muted: false,
                },
                DiplomacyPlayerPort {
                    name: "GLA".to_string(),
                    side: "GLA".to_string(),
                    team: 3,
                    relation: DiplomacyRelationPort::Enemy,
                    muted: true,
                },
            ],
            solo_briefing_lines: vec![
                "Secure the objective.".to_string(),
                "Destroy all enemy structures.".to_string(),
            ],
            selected_player: Some(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggling_mute_updates_selected_player() {
        let mut diplomacy = DiplomacyPort::sample();
        assert!(diplomacy.toggle_mute(1));
        assert!(diplomacy.players[1].muted);
        assert_eq!(diplomacy.selected_player, Some(1));
    }

    #[test]
    fn briefing_text_can_be_cleared_and_replaced() {
        let mut diplomacy = DiplomacyPort::sample();
        diplomacy.update_briefing_text("Capture the airfield.", true);

        assert_eq!(diplomacy.solo_briefing_lines, vec!["Capture the airfield."]);
    }
}
