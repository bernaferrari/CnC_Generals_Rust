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

pub const MAX_SLOTS: usize = 8;
pub const NO_FPS_LIMIT: u8 = 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotStatePort {
    Open,
    Closed,
    EasyAI,
    NormalAI,
    BrutalAI,
    Player,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameTypePort {
    Skirmish,
    SinglePlayer,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SkirmishAction {
    Back,
    Start,
    SelectMap,
    Reset,
    ColorSelect { slot_index: usize },
    FactionSelect { slot_index: usize },
    TeamSelect { slot_index: usize },
    PlayerTypeSelect { slot_index: usize },
    StartPositionSelect { position: usize },
    StartPositionRightClick { position: usize },
    StartingCashSelect,
    SuperweaponToggle,
    GameSpeedSlider { position: u8 },
    PlayerNameEdit { name: String },
    Escape,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StartGameError {
    MapNotFound,
    TooManyPlayers { max_players: u8 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkirmishSlotPort {
    pub player_name: String,
    pub faction: String,
    pub color: String,
    pub team: u8,
    pub state: SlotStatePort,
    pub start_pos: Option<u8>,
}

impl Default for SkirmishSlotPort {
    fn default() -> Self {
        Self {
            player_name: String::new(),
            faction: "Random".to_string(),
            color: "Random".to_string(),
            team: 0,
            state: SlotStatePort::Closed,
            start_pos: None,
        }
    }
}

impl SkirmishSlotPort {
    pub fn is_open_or_ai(&self) -> bool {
        matches!(
            self.state,
            SlotStatePort::Open
                | SlotStatePort::EasyAI
                | SlotStatePort::NormalAI
                | SlotStatePort::BrutalAI
        )
    }

    pub fn is_ai(&self) -> bool {
        matches!(
            self.state,
            SlotStatePort::EasyAI | SlotStatePort::NormalAI | SlotStatePort::BrutalAI
        )
    }

    pub fn is_active(&self) -> bool {
        !matches!(self.state, SlotStatePort::Open | SlotStatePort::Closed)
    }

    pub fn is_participating(&self) -> bool {
        matches!(
            self.state,
            SlotStatePort::Player
                | SlotStatePort::EasyAI
                | SlotStatePort::NormalAI
                | SlotStatePort::BrutalAI
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkirmishGameOptionsMenuPort {
    pub player_name: String,
    pub map_name: String,
    pub map_display_name: String,
    pub game_speed: u8,
    pub superweapons_restricted: bool,
    pub starting_cash: u32,
    pub selected_slot: usize,
    pub slots: Vec<SkirmishSlotPort>,
    pub pending_shell_push: Option<String>,
    pub button_pushed: bool,
    pub just_entered: bool,
    pub initial_gadget_delay: i32,
    pub still_needs_to_set_options: bool,
    pub sandbox_ok: bool,
    pub game_launched: bool,
    pub last_start_error: Option<StartGameError>,
}

impl Default for SkirmishGameOptionsMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl SkirmishGameOptionsMenuPort {
    pub fn sample() -> Self {
        Self {
            player_name: "bernardo".to_string(),
            map_name: "Tournament Desert".to_string(),
            map_display_name: "Tournament Desert".to_string(),
            game_speed: 30,
            superweapons_restricted: false,
            starting_cash: 10_000,
            selected_slot: 0,
            pending_shell_push: None,
            button_pushed: false,
            just_entered: false,
            initial_gadget_delay: 2,
            still_needs_to_set_options: false,
            sandbox_ok: false,
            game_launched: false,
            last_start_error: None,
            slots: vec![
                SkirmishSlotPort {
                    player_name: "bernardo".to_string(),
                    faction: "USA".to_string(),
                    color: "Blue".to_string(),
                    team: 1,
                    state: SlotStatePort::Player,
                    start_pos: Some(1),
                },
                SkirmishSlotPort {
                    player_name: "AI Slot 2".to_string(),
                    faction: "China".to_string(),
                    color: "Red".to_string(),
                    team: 2,
                    state: SlotStatePort::NormalAI,
                    start_pos: Some(2),
                },
                SkirmishSlotPort {
                    player_name: "AI Slot 3".to_string(),
                    faction: "GLA".to_string(),
                    color: "Green".to_string(),
                    team: 3,
                    state: SlotStatePort::EasyAI,
                    start_pos: None,
                },
                SkirmishSlotPort::default(),
                SkirmishSlotPort::default(),
                SkirmishSlotPort::default(),
                SkirmishSlotPort::default(),
                SkirmishSlotPort::default(),
            ],
        }
    }

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

    pub fn set_superweapons(&mut self, restricted: bool) {
        self.superweapons_restricted = restricted;
    }

    pub fn toggle_superweapons(&mut self) {
        self.superweapons_restricted = !self.superweapons_restricted;
    }

    pub fn effective_fps(&self) -> u32 {
        if self.game_speed > NO_FPS_LIMIT {
            1000
        } else {
            (self.game_speed as u32).max(15)
        }
    }

    pub fn num_players(&self) -> usize {
        self.slots.iter().filter(|s| s.is_participating()).count()
    }

    fn ensure_slots(&mut self) {
        while self.slots.len() < MAX_SLOTS {
            self.slots.push(SkirmishSlotPort::default());
        }
    }

    pub fn set_slot(&mut self, index: usize, slot: SkirmishSlotPort) {
        self.ensure_slots();
        if index < MAX_SLOTS {
            self.slots[index] = slot;
        }
    }

    pub fn set_slot_state(&mut self, index: usize, state: SlotStatePort, name: String) -> bool {
        if index == 0 || index >= self.slots.len() {
            return false;
        }
        self.slots[index].state = state;
        if !name.is_empty() {
            self.slots[index].player_name = name;
        }
        true
    }

    pub fn set_slot_color(&mut self, index: usize, color: String) -> bool {
        if index >= self.slots.len() {
            return false;
        }
        if self.slots[index].color == color {
            return true;
        }
        if color != "Random" && color != "-1" {
            for (i, slot) in self.slots.iter().enumerate() {
                if i != index && slot.color == color {
                    return false;
                }
            }
        }
        self.slots[index].color = color;
        true
    }

    pub fn set_slot_faction(&mut self, index: usize, faction: String) -> bool {
        if index >= self.slots.len() {
            return false;
        }
        if self.slots[index].faction == faction {
            return true;
        }
        self.slots[index].faction = faction;
        true
    }

    pub fn set_slot_team(&mut self, index: usize, team: u8) -> bool {
        if index >= self.slots.len() {
            return false;
        }
        if self.slots[index].team == team {
            return true;
        }
        self.slots[index].team = team;
        true
    }

    pub fn set_slot_start_pos(&mut self, index: usize, pos: Option<u8>) -> bool {
        if index >= self.slots.len() {
            return false;
        }
        let current = self.slots[index].start_pos;
        if current == pos {
            return true;
        }
        if let Some(p) = pos {
            for (i, slot) in self.slots.iter().enumerate() {
                if i != index && slot.start_pos == Some(p) {
                    return false;
                }
            }
        }
        self.slots[index].start_pos = pos;
        true
    }

    fn get_next_selectable_player(&self, start: usize) -> Option<usize> {
        for j in start..MAX_SLOTS {
            let slot = &self.slots[j];
            if slot.start_pos.is_none() && (j == 0 || slot.is_ai()) {
                return Some(j);
            }
        }
        None
    }

    fn find_player_at_start_pos(&self, pos: usize) -> Option<usize> {
        for (i, slot) in self.slots.iter().enumerate() {
            if slot.start_pos == Some(pos as u8) {
                return Some(i);
            }
        }
        None
    }

    pub fn handle_start_position_left_click(&mut self, position: usize) {
        if let Some(player_idx) = self.find_player_at_start_pos(position) {
            let slot = &self.slots[player_idx];
            if player_idx == 0 || slot.is_ai() {
                self.set_slot_start_pos(player_idx, None);
                if let Some(next) = self.get_next_selectable_player(player_idx + 1) {
                    self.set_slot_start_pos(next, Some(position as u8));
                }
            }
        } else {
            let next = self.get_next_selectable_player(0).unwrap_or(0);
            self.set_slot_start_pos(next, Some(position as u8));
        }
    }

    pub fn handle_start_position_right_click(&mut self, position: usize) {
        if let Some(player_idx) = self.find_player_at_start_pos(position) {
            let slot = &self.slots[player_idx];
            if player_idx == 0 || slot.is_ai() {
                self.set_slot_start_pos(player_idx, None);
            }
        }
    }

    pub fn validate_start_game(&self, max_map_players: u8) -> Result<GameTypePort, StartGameError> {
        if self.num_players() > max_map_players as usize {
            return Err(StartGameError::TooManyPlayers {
                max_players: max_map_players,
            });
        }
        Ok(GameTypePort::Skirmish)
    }

    pub fn start_pressed(&mut self, max_map_players: u8) -> Option<GameTypePort> {
        self.last_start_error = None;
        match self.validate_start_game(max_map_players) {
            Ok(game_type) => {
                self.button_pushed = true;
                self.game_launched = true;
                Some(game_type)
            }
            Err(e) => {
                self.button_pushed = false;
                self.last_start_error = Some(e);
                None
            }
        }
    }

    pub fn handle_button(&mut self, action: SkirmishAction, max_map_players: u8) -> bool {
        if self.button_pushed && !matches!(action, SkirmishAction::Escape) {
            return false;
        }
        match action {
            SkirmishAction::Back => {
                self.button_pushed = true;
                true
            }
            SkirmishAction::Start => self.start_pressed(max_map_players).is_some(),
            SkirmishAction::SelectMap => {
                self.sandbox_ok = false;
                self.select_map();
                true
            }
            SkirmishAction::Reset => true,
            SkirmishAction::ColorSelect { slot_index: _ } => {
                self.sandbox_ok = false;
                true
            }
            SkirmishAction::FactionSelect { slot_index: _ } => {
                self.sandbox_ok = false;
                true
            }
            SkirmishAction::TeamSelect { slot_index: _ } => {
                self.sandbox_ok = false;
                true
            }
            SkirmishAction::PlayerTypeSelect { slot_index: _ } => {
                self.sandbox_ok = false;
                true
            }
            SkirmishAction::StartPositionSelect { position } => {
                self.handle_start_position_left_click(position);
                self.sandbox_ok = false;
                true
            }
            SkirmishAction::StartPositionRightClick { position } => {
                self.handle_start_position_right_click(position);
                self.sandbox_ok = false;
                true
            }
            SkirmishAction::StartingCashSelect => true,
            SkirmishAction::SuperweaponToggle => {
                self.toggle_superweapons();
                true
            }
            SkirmishAction::GameSpeedSlider { position } => {
                self.set_game_speed(position);
                true
            }
            SkirmishAction::PlayerNameEdit { ref name } => {
                if !self.slots.is_empty() {
                    self.slots[0].player_name = name.clone();
                    self.player_name = name.clone();
                }
                true
            }
            SkirmishAction::Escape => {
                self.button_pushed = true;
                true
            }
        }
    }

    pub fn update_slot_count(&mut self, count: usize) {
        self.ensure_slots();
        for i in 0..MAX_SLOTS {
            if i == 0 {
                self.slots[i].state = SlotStatePort::Player;
            } else if i < count {
                if matches!(
                    self.slots[i].state,
                    SlotStatePort::Open | SlotStatePort::Closed
                ) {
                    self.slots[i].state = SlotStatePort::EasyAI;
                    self.slots[i].player_name = format!("AI Slot {}", i + 1);
                }
            } else {
                self.slots[i].state = SlotStatePort::Closed;
                self.slots[i].start_pos = None;
            }
        }
    }

    pub fn reset_slots(&mut self) {
        for i in 0..MAX_SLOTS {
            if i == 0 {
                self.slots[i].start_pos = None;
            } else {
                self.slots[i] = SkirmishSlotPort::default();
            }
        }
    }

    pub fn fps_display_text(&self) -> String {
        if self.game_speed > NO_FPS_LIMIT {
            "--".to_string()
        } else {
            format!("{:2}", self.game_speed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_menu() -> SkirmishGameOptionsMenuPort {
        SkirmishGameOptionsMenuPort::sample()
    }

    #[test]
    fn selecting_map_pushes_map_select_layout() {
        let mut menu = default_menu();
        menu.select_map();

        assert_eq!(
            menu.pending_shell_push.as_deref(),
            Some("Menus/SkirmishMapSelectMenu.wnd")
        );
        assert!(menu.button_pushed);
    }

    #[test]
    fn toggling_superweapons_flips_rule_state() {
        let mut menu = default_menu();
        menu.toggle_superweapons();
        assert!(menu.superweapons_restricted);
    }

    #[test]
    fn set_slot_color_rejects_duplicate() {
        let mut menu = default_menu();
        menu.slots[0].color = "Blue".to_string();
        assert!(!menu.set_slot_color(1, "Blue".to_string()));
        assert!(menu.set_slot_color(1, "Red".to_string()));
        assert_eq!(menu.slots[1].color, "Red");
    }

    #[test]
    fn set_slot_team_updates_correctly() {
        let mut menu = default_menu();
        assert!(menu.set_slot_team(0, 5));
        assert_eq!(menu.slots[0].team, 5);
    }

    #[test]
    fn set_slot_faction_updates_correctly() {
        let mut menu = default_menu();
        assert!(menu.set_slot_faction(0, "China".to_string()));
        assert_eq!(menu.slots[0].faction, "China");
    }

    #[test]
    fn set_slot_state_rejects_slot_zero() {
        let mut menu = default_menu();
        assert!(!menu.set_slot_state(0, SlotStatePort::EasyAI, "bot".to_string()));
    }

    #[test]
    fn set_slot_state_updates_ai_slot() {
        let mut menu = default_menu();
        assert!(menu.set_slot_state(1, SlotStatePort::BrutalAI, "HardBot".to_string()));
        assert_eq!(menu.slots[1].state, SlotStatePort::BrutalAI);
        assert_eq!(menu.slots[1].player_name, "HardBot");
    }

    #[test]
    fn start_position_left_click_assigns_open_slot() {
        let mut menu = default_menu();
        menu.slots[1].start_pos = None;
        menu.slots[1].state = SlotStatePort::NormalAI;

        menu.handle_start_position_left_click(2);
        assert_eq!(menu.slots[1].start_pos, Some(2));
    }

    #[test]
    fn start_position_left_click_swaps_from_occupied() {
        let mut menu = default_menu();
        menu.slots[0].start_pos = Some(0);
        menu.slots[1].start_pos = None;
        menu.slots[1].state = SlotStatePort::NormalAI;

        menu.handle_start_position_left_click(0);
        assert_eq!(menu.slots[0].start_pos, None);
        assert_eq!(menu.slots[1].start_pos, Some(0));
    }

    #[test]
    fn start_position_right_click_removes_slot() {
        let mut menu = default_menu();
        menu.slots[0].start_pos = Some(3);

        menu.handle_start_position_right_click(3);
        assert_eq!(menu.slots[0].start_pos, None);
    }

    #[test]
    fn start_position_right_click_ignores_non_local() {
        let mut menu = default_menu();
        menu.slots[0].start_pos = None;
        menu.slots[0].state = SlotStatePort::Player;

        menu.handle_start_position_right_click(0);
        assert_eq!(menu.slots[0].start_pos, None);
    }

    #[test]
    fn validate_start_game_rejects_too_many_players() {
        let menu = default_menu();
        let result = menu.validate_start_game(1);
        assert!(matches!(result, Err(StartGameError::TooManyPlayers { .. })));
    }

    #[test]
    fn validate_start_game_allows_correct_count() {
        let menu = default_menu();
        let result = menu.validate_start_game(8);
        assert_eq!(result, Ok(GameTypePort::Skirmish));
    }

    #[test]
    fn start_pressed_sets_game_launched_on_success() {
        let mut menu = default_menu();
        let result = menu.start_pressed(8);
        assert_eq!(result, Some(GameTypePort::Skirmish));
        assert!(menu.game_launched);
        assert!(menu.button_pushed);
    }

    #[test]
    fn start_pressed_sets_error_on_failure() {
        let mut menu = default_menu();
        let result = menu.start_pressed(1);
        assert_eq!(result, None);
        assert!(!menu.game_launched);
        assert!(!menu.button_pushed);
        assert!(menu.last_start_error.is_some());
    }

    #[test]
    fn handle_button_dispatches_start_action() {
        let mut menu = default_menu();
        assert!(menu.handle_button(SkirmishAction::Start, 8));
        assert!(menu.game_launched);
    }

    #[test]
    fn handle_button_dispatches_back_action() {
        let mut menu = default_menu();
        assert!(menu.handle_button(SkirmishAction::Back, 8));
        assert!(menu.button_pushed);
    }

    #[test]
    fn handle_button_dispatches_select_map() {
        let mut menu = default_menu();
        assert!(menu.handle_button(SkirmishAction::SelectMap, 8));
        assert_eq!(
            menu.pending_shell_push.as_deref(),
            Some("Menus/SkirmishMapSelectMenu.wnd")
        );
    }

    #[test]
    fn handle_button_ignores_when_pushed() {
        let mut menu = default_menu();
        menu.button_pushed = true;
        assert!(!menu.handle_button(SkirmishAction::Start, 8));
    }

    #[test]
    fn handle_button_escape_works_when_pushed() {
        let mut menu = default_menu();
        menu.button_pushed = true;
        assert!(menu.handle_button(SkirmishAction::Escape, 8));
    }

    #[test]
    fn handle_button_superweapon_toggle() {
        let mut menu = default_menu();
        menu.superweapons_restricted = false;
        menu.handle_button(SkirmishAction::SuperweaponToggle, 8);
        assert!(menu.superweapons_restricted);
    }

    #[test]
    fn handle_button_game_speed_slider() {
        let mut menu = default_menu();
        menu.handle_button(SkirmishAction::GameSpeedSlider { position: 45 }, 8);
        assert_eq!(menu.game_speed, 45);
    }

    #[test]
    fn handle_button_player_name_edit() {
        let mut menu = default_menu();
        menu.handle_button(
            SkirmishAction::PlayerNameEdit {
                name: "NewName".to_string(),
            },
            8,
        );
        assert_eq!(menu.player_name, "NewName");
        assert_eq!(menu.slots[0].player_name, "NewName");
    }

    #[test]
    fn handle_button_start_position_select() {
        let mut menu = default_menu();
        menu.slots[1].start_pos = None;
        menu.slots[1].state = SlotStatePort::NormalAI;
        menu.handle_button(SkirmishAction::StartPositionSelect { position: 3 }, 8);
        assert_eq!(menu.slots[1].start_pos, Some(3));
    }

    #[test]
    fn handle_button_start_position_right_click() {
        let mut menu = default_menu();
        menu.slots[0].start_pos = Some(3);
        menu.handle_button(SkirmishAction::StartPositionRightClick { position: 3 }, 8);
        assert_eq!(menu.slots[0].start_pos, None);
    }

    #[test]
    fn effective_fps_clamps_minimum() {
        let mut menu = default_menu();
        menu.game_speed = 5;
        assert_eq!(menu.effective_fps(), 15);
    }

    #[test]
    fn effective_fps_no_limit() {
        let mut menu = default_menu();
        menu.game_speed = 70;
        assert_eq!(menu.effective_fps(), 1000);
    }

    #[test]
    fn effective_fps_normal() {
        let mut menu = default_menu();
        menu.game_speed = 30;
        assert_eq!(menu.effective_fps(), 30);
    }

    #[test]
    fn fps_display_text_unlimited() {
        let mut menu = default_menu();
        menu.game_speed = 70;
        assert_eq!(menu.fps_display_text(), "--");
    }

    #[test]
    fn fps_display_text_normal() {
        let mut menu = default_menu();
        menu.game_speed = 30;
        assert_eq!(menu.fps_display_text(), "30");
    }

    #[test]
    fn update_slot_count_opens_and_closes() {
        let mut menu = default_menu();
        menu.update_slot_count(4);

        assert_eq!(menu.slots[0].state, SlotStatePort::Player);
        assert!(menu.slots[1].is_ai());
        assert!(menu.slots[2].is_ai());
        assert!(menu.slots[3].is_ai());
        assert_eq!(menu.slots[4].state, SlotStatePort::Closed);
    }

    #[test]
    fn reset_slots_clears_non_local() {
        let mut menu = default_menu();
        menu.reset_slots();

        assert_eq!(menu.slots[0].start_pos, None);
        assert_eq!(menu.slots[1].state, SlotStatePort::Closed);
        assert_eq!(menu.slots[2].state, SlotStatePort::Closed);
    }

    #[test]
    fn num_players_counts_participating() {
        let menu = default_menu();
        assert_eq!(menu.num_players(), 3);
    }

    #[test]
    fn set_slot_start_pos_rejects_duplicate() {
        let mut menu = default_menu();
        menu.slots[0].start_pos = Some(1);
        assert!(!menu.set_slot_start_pos(1, Some(1)));
        assert!(menu.set_slot_start_pos(1, Some(2)));
    }

    #[test]
    fn set_slot_start_pos_none_clears() {
        let mut menu = default_menu();
        menu.slots[0].start_pos = Some(5);
        assert!(menu.set_slot_start_pos(0, None));
        assert_eq!(menu.slots[0].start_pos, None);
    }

    #[test]
    fn slot_is_participating() {
        let mut slot = SkirmishSlotPort::default();
        assert!(!slot.is_participating());

        slot.state = SlotStatePort::Player;
        assert!(slot.is_participating());

        slot.state = SlotStatePort::EasyAI;
        assert!(slot.is_participating());

        slot.state = SlotStatePort::Closed;
        assert!(!slot.is_participating());
    }
}
