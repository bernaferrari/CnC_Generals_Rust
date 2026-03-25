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

pub const MAX_SLOTS: usize = 8;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DiplomacyButtonId {
    Hide,
    RadioButtonInGame,
    RadioButtonBuddies,
    Mute(usize),
    UnMute(usize),
}

impl DiplomacyButtonId {
    pub fn from_control_name(name: &str) -> Option<Self> {
        if name == "Diplomacy.wnd:ButtonHide" {
            return Some(Self::Hide);
        }
        if name == "Diplomacy.wnd:RadioButtonInGame" {
            return Some(Self::RadioButtonInGame);
        }
        if name == "Diplomacy.wnd:RadioButtonBuddies" {
            return Some(Self::RadioButtonBuddies);
        }
        if let Some(rest) = name.strip_prefix("Diplomacy.wnd:ButtonMute") {
            if let Ok(slot) = rest.parse::<usize>() {
                if slot < MAX_SLOTS {
                    return Some(Self::Mute(slot));
                }
            }
        }
        if let Some(rest) = name.strip_prefix("Diplomacy.wnd:ButtonUnMute") {
            if let Ok(slot) = rest.parse::<usize>() {
                if slot < MAX_SLOTS {
                    return Some(Self::UnMute(slot));
                }
            }
        }
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiplomacyPlayerPort {
    pub name: String,
    pub side: String,
    pub team: i8,
    pub relation: DiplomacyRelationPort,
    pub muted: bool,
    pub alive: bool,
    pub observer: bool,
    pub in_game: bool,
}

impl Default for DiplomacyPlayerPort {
    fn default() -> Self {
        Self {
            name: String::new(),
            side: String::new(),
            team: -1,
            relation: DiplomacyRelationPort::Neutral,
            muted: false,
            alive: true,
            observer: false,
            in_game: false,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiplomacyPort {
    pub visible: bool,
    pub active_tab: DiplomacyTabPort,
    pub players: Vec<DiplomacyPlayerPort>,
    pub solo_briefing_lines: Vec<String>,
    pub selected_player: Option<usize>,
    slot_to_row: [i32; MAX_SLOTS],
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
        self.release_window_pointers();
    }

    pub fn set_tab(&mut self, tab: DiplomacyTabPort) {
        self.active_tab = tab;
    }

    pub fn grab_window_pointers(&mut self) {
        let mut row: usize = 0;
        for slot in 0..MAX_SLOTS {
            self.slot_to_row[slot] = -1;
        }
        for (slot, _player) in self.players.iter().enumerate() {
            if row >= MAX_SLOTS {
                break;
            }
            self.slot_to_row[slot] = row as i32;
            row += 1;
        }
    }

    pub fn release_window_pointers(&mut self) {
        for slot in 0..MAX_SLOTS {
            self.slot_to_row[slot] = -1;
        }
    }

    pub fn handle_button(&mut self, button: DiplomacyButtonId) {
        match button {
            DiplomacyButtonId::Hide => {
                self.hide();
            }
            DiplomacyButtonId::RadioButtonInGame => {
                self.active_tab = DiplomacyTabPort::InGame;
            }
            DiplomacyButtonId::RadioButtonBuddies => {
                self.active_tab = DiplomacyTabPort::Buddies;
            }
            DiplomacyButtonId::Mute(slot) => {
                if self.slot_to_row[slot] >= 0 && slot < self.players.len() {
                    self.set_alliance_mute(slot, true);
                }
            }
            DiplomacyButtonId::UnMute(slot) => {
                if self.slot_to_row[slot] >= 0 && slot < self.players.len() {
                    self.set_alliance_mute(slot, false);
                }
            }
        }
    }

    pub fn set_alliance_state(&mut self, slot: usize, relation: DiplomacyRelationPort) -> bool {
        let Some(player) = self.players.get_mut(slot) else {
            return false;
        };
        player.relation = relation;
        self.selected_player = Some(slot);
        true
    }

    pub fn set_team_assignment(&mut self, slot: usize, team: i8) -> bool {
        let Some(player) = self.players.get_mut(slot) else {
            return false;
        };
        player.team = team;
        self.selected_player = Some(slot);
        true
    }

    pub fn set_alliance_mute(&mut self, slot: usize, muted: bool) -> bool {
        let Some(player) = self.players.get_mut(slot) else {
            return false;
        };
        player.muted = muted;
        self.selected_player = Some(slot);
        true
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

    pub fn get_slot_for_row(&self, row: usize) -> Option<usize> {
        for slot in 0..MAX_SLOTS {
            if self.slot_to_row[slot] == row as i32 {
                return Some(slot);
            }
        }
        None
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
                    alive: true,
                    observer: false,
                    in_game: true,
                },
                DiplomacyPlayerPort {
                    name: "China".to_string(),
                    side: "China".to_string(),
                    team: 2,
                    relation: DiplomacyRelationPort::Neutral,
                    muted: false,
                    alive: true,
                    observer: false,
                    in_game: true,
                },
                DiplomacyPlayerPort {
                    name: "GLA".to_string(),
                    side: "GLA".to_string(),
                    team: 3,
                    relation: DiplomacyRelationPort::Enemy,
                    muted: true,
                    alive: false,
                    observer: false,
                    in_game: false,
                },
            ],
            solo_briefing_lines: vec![
                "Secure the objective.".to_string(),
                "Destroy all enemy structures.".to_string(),
            ],
            selected_player: Some(0),
            slot_to_row: [-1; MAX_SLOTS],
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

    #[test]
    fn grab_window_pointers_maps_slots_to_rows() {
        let mut diplomacy = DiplomacyPort::sample();
        assert_eq!(diplomacy.slot_to_row, [-1; MAX_SLOTS]);

        diplomacy.grab_window_pointers();

        assert_eq!(diplomacy.slot_to_row[0], 0);
        assert_eq!(diplomacy.slot_to_row[1], 1);
        assert_eq!(diplomacy.slot_to_row[2], 2);
        for slot in 3..MAX_SLOTS {
            assert_eq!(diplomacy.slot_to_row[slot], -1);
        }
    }

    #[test]
    fn release_window_pointers_clears_all() {
        let mut diplomacy = DiplomacyPort::sample();
        diplomacy.grab_window_pointers();
        assert_eq!(diplomacy.slot_to_row[0], 0);

        diplomacy.release_window_pointers();
        assert_eq!(diplomacy.slot_to_row, [-1; MAX_SLOTS]);
    }

    #[test]
    fn handle_button_hide_hides_diplomacy() {
        let mut diplomacy = DiplomacyPort::sample();
        assert!(diplomacy.visible);

        diplomacy.handle_button(DiplomacyButtonId::Hide);
        assert!(!diplomacy.visible);
    }

    #[test]
    fn handle_button_radio_switches_tab() {
        let mut diplomacy = DiplomacyPort::sample();
        assert_eq!(diplomacy.active_tab, DiplomacyTabPort::InGame);

        diplomacy.handle_button(DiplomacyButtonId::RadioButtonBuddies);
        assert_eq!(diplomacy.active_tab, DiplomacyTabPort::Buddies);

        diplomacy.handle_button(DiplomacyButtonId::RadioButtonInGame);
        assert_eq!(diplomacy.active_tab, DiplomacyTabPort::InGame);
    }

    #[test]
    fn handle_button_mute_ignores_unmapped_slot() {
        let mut diplomacy = DiplomacyPort::sample();
        assert_eq!(diplomacy.slot_to_row, [-1; MAX_SLOTS]);

        diplomacy.handle_button(DiplomacyButtonId::Mute(0));
        assert!(!diplomacy.players[0].muted);
    }

    #[test]
    fn handle_button_mute_works_after_grab() {
        let mut diplomacy = DiplomacyPort::sample();
        diplomacy.grab_window_pointers();

        diplomacy.handle_button(DiplomacyButtonId::Mute(1));
        assert!(diplomacy.players[1].muted);
        assert_eq!(diplomacy.selected_player, Some(1));

        diplomacy.handle_button(DiplomacyButtonId::UnMute(1));
        assert!(!diplomacy.players[1].muted);
    }

    #[test]
    fn set_alliance_state_changes_relation() {
        let mut diplomacy = DiplomacyPort::sample();

        assert!(diplomacy.set_alliance_state(0, DiplomacyRelationPort::Enemy));
        assert_eq!(diplomacy.players[0].relation, DiplomacyRelationPort::Enemy);
        assert_eq!(diplomacy.selected_player, Some(0));

        assert!(!diplomacy.set_alliance_state(99, DiplomacyRelationPort::Allied));
    }

    #[test]
    fn set_team_assignment_changes_team() {
        let mut diplomacy = DiplomacyPort::sample();

        assert!(diplomacy.set_team_assignment(1, 5));
        assert_eq!(diplomacy.players[1].team, 5);
        assert_eq!(diplomacy.selected_player, Some(1));

        assert!(!diplomacy.set_team_assignment(99, 1));
    }

    #[test]
    fn get_slot_for_row_round_trips() {
        let mut diplomacy = DiplomacyPort::sample();
        diplomacy.grab_window_pointers();

        assert_eq!(diplomacy.get_slot_for_row(0), Some(0));
        assert_eq!(diplomacy.get_slot_for_row(1), Some(1));
        assert_eq!(diplomacy.get_slot_for_row(2), Some(2));
        assert_eq!(diplomacy.get_slot_for_row(3), None);
    }

    #[test]
    fn button_id_parsing() {
        assert_eq!(
            DiplomacyButtonId::from_control_name("Diplomacy.wnd:ButtonHide"),
            Some(DiplomacyButtonId::Hide)
        );
        assert_eq!(
            DiplomacyButtonId::from_control_name("Diplomacy.wnd:RadioButtonInGame"),
            Some(DiplomacyButtonId::RadioButtonInGame)
        );
        assert_eq!(
            DiplomacyButtonId::from_control_name("Diplomacy.wnd:RadioButtonBuddies"),
            Some(DiplomacyButtonId::RadioButtonBuddies)
        );
        assert_eq!(
            DiplomacyButtonId::from_control_name("Diplomacy.wnd:ButtonMute3"),
            Some(DiplomacyButtonId::Mute(3))
        );
        assert_eq!(
            DiplomacyButtonId::from_control_name("Diplomacy.wnd:ButtonUnMute7"),
            Some(DiplomacyButtonId::UnMute(7))
        );
        assert_eq!(
            DiplomacyButtonId::from_control_name("Diplomacy.wnd:ButtonMute8"),
            None
        );
        assert_eq!(
            DiplomacyButtonId::from_control_name("SomeOther.wnd:ButtonMute0"),
            None
        );
    }

    #[test]
    fn hide_releases_window_pointers() {
        let mut diplomacy = DiplomacyPort::sample();
        diplomacy.grab_window_pointers();
        assert_eq!(diplomacy.slot_to_row[0], 0);

        diplomacy.hide();
        assert_eq!(diplomacy.slot_to_row, [-1; MAX_SLOTS]);
    }
}
