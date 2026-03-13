use crate::gui::source_catalog::{GuiPortRecord, MenuScreenPort};

pub const RECORD: GuiPortRecord = GuiPortRecord::new(
    "GUICallbacks/Menus/SkirmishGameOptionsMenu.cpp",
    "crate::gui::callbacks::menus::skirmish_game_options_menu",
    "Skirmish Game Options Menu",
    "Skirmish setup callbacks.",
);
pub const SCREEN: MenuScreenPort = MenuScreenPort::new(
    &RECORD,
    "SkirmishGameOptionsMenu",
    "Skirmish Setup",
    "Configure players, AI, and match rules for skirmish.",
    "Shell",
);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkirmishSlotPort {
    pub player_name: String,
    pub faction: String,
    pub color: String,
    pub team: u8,
    pub is_ai: bool,
    pub start_pos: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkirmishGameOptionsMenuPort {
    pub player_name: String,
    pub map_name: String,
    pub game_speed: u8,
    pub superweapons_restricted: bool,
    pub starting_cash: u32,
    pub selected_slot: usize,
    pub slots: Vec<SkirmishSlotPort>,
    pub pending_shell_push: Option<String>,
    pub button_pushed: bool,
}

impl Default for SkirmishGameOptionsMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl SkirmishGameOptionsMenuPort {
    pub fn select_slot(&mut self, index: usize) -> bool {
        if index >= self.slots.len() {
            return false;
        }
        self.selected_slot = index;
        true
    }

    pub fn select_map(&mut self) {
        self.pending_shell_push = Some("Menus/SkirmishMapSelectMenu.wnd".to_string());
        self.button_pushed = true;
    }

    pub fn set_game_speed(&mut self, value: u8) {
        self.game_speed = value.min(100);
    }

    pub fn set_starting_cash(&mut self, cash: u32) {
        self.starting_cash = cash;
    }

    pub fn toggle_superweapons(&mut self) {
        self.superweapons_restricted = !self.superweapons_restricted;
    }

    pub fn sample() -> Self {
        Self {
            player_name: "bernardo".to_string(),
            map_name: "Tournament Desert".to_string(),
            game_speed: 50,
            superweapons_restricted: false,
            starting_cash: 10_000,
            selected_slot: 0,
            pending_shell_push: None,
            button_pushed: false,
            slots: vec![
                SkirmishSlotPort {
                    player_name: "bernardo".to_string(),
                    faction: "USA".to_string(),
                    color: "Blue".to_string(),
                    team: 1,
                    is_ai: false,
                    start_pos: Some(1),
                },
                SkirmishSlotPort {
                    player_name: "AI Slot 2".to_string(),
                    faction: "China".to_string(),
                    color: "Red".to_string(),
                    team: 2,
                    is_ai: true,
                    start_pos: Some(2),
                },
                SkirmishSlotPort {
                    player_name: "AI Slot 3".to_string(),
                    faction: "GLA".to_string(),
                    color: "Green".to_string(),
                    team: 3,
                    is_ai: true,
                    start_pos: None,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_map_pushes_map_select_layout() {
        let mut menu = SkirmishGameOptionsMenuPort::sample();
        menu.select_map();

        assert_eq!(
            menu.pending_shell_push.as_deref(),
            Some("Menus/SkirmishMapSelectMenu.wnd")
        );
        assert!(menu.button_pushed);
    }

    #[test]
    fn toggling_superweapons_flips_rule_state() {
        let mut menu = SkirmishGameOptionsMenuPort::sample();
        menu.toggle_superweapons();
        assert!(menu.superweapons_restricted);
    }
}
