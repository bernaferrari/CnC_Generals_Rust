use crate::gui::callbacks::menus::main_menu::GameDifficultyPort;
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

const DEFAULT_GENERAL: usize = 0;
const TELETYPE_SKIP: usize = 2;
const NUM_GENERALS: usize = 12;
const BUTTON_ID_PLAY: i32 = -100;
const BUTTON_ID_BACK: i32 = -101;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChallengeGeneralPort {
    pub name: String,
    pub enabled: bool,
    pub starts_enabled: bool,
    pub campaign: String,
    pub current_map: String,
    pub player_template_name: String,
    pub bio_name: String,
    pub bio_rank: String,
    pub bio_branch: String,
    pub bio_strategy: String,
    pub completed: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChallengeMenuButton {
    GeneralPosition(usize),
    Play,
    Back,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChallengeMenuAction {
    None,
    SelectGeneral(usize),
    PreviewGeneral(usize),
    LaunchChallenge {
        campaign: String,
        player_template: String,
        map: String,
        difficulty: GameDifficultyPort,
        rank_points: i32,
    },
    Back,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChallengeMenuPort {
    pub selected_general: Option<usize>,
    pub last_hilited_index: Option<usize>,
    pub is_auto_selecting: bool,
    pub teletype_position: usize,
    pub intro_sequence_step: usize,
    pub intro_audio_counter: i32,
    pub has_played_intro_audio: bool,
    pub just_entered: bool,
    pub initial_gadget_delay: i32,
    pub is_shutting_down: bool,
    pub can_play: bool,
    pub difficulty: GameDifficultyPort,
    pub rank_points: i32,
    pub generals: Vec<ChallengeGeneralPort>,
    pub completed_campaigns: Vec<String>,
}

impl Default for ChallengeMenuPort {
    fn default() -> Self {
        Self::sample()
    }
}

impl ChallengeMenuPort {
    pub fn find_position_button(&self, button_id: i32) -> Option<usize> {
        if button_id >= 0 && (button_id as usize) < self.generals.len() {
            Some(button_id as usize)
        } else {
            None
        }
    }

    pub fn classify_button(&self, button_id: i32) -> ChallengeMenuButton {
        if button_id == BUTTON_ID_PLAY {
            return ChallengeMenuButton::Play;
        }
        if button_id == BUTTON_ID_BACK {
            return ChallengeMenuButton::Back;
        }
        if let Some(index) = self.find_position_button(button_id) {
            return ChallengeMenuButton::GeneralPosition(index);
        }
        ChallengeMenuButton::Back
    }

    pub fn select_general(&mut self, index: usize) -> bool {
        if index >= self.generals.len() || !self.generals[index].enabled {
            return false;
        }
        let prev = self.selected_general;
        self.selected_general = Some(index);
        self.teletype_position = 0;
        self.can_play = true;
        if let Some(prev) = prev {
            if prev != index {
                self.is_auto_selecting = true;
            }
        }
        true
    }

    pub fn set_general_campaign(&mut self, button_index: usize) {
        if button_index >= self.generals.len() {
            return;
        }
        let general = &self.generals[button_index];
        let _ = &general.campaign;
        let _ = &general.current_map;
        let _ = &general.player_template_name;
    }

    pub fn set_general_bio(&mut self, button_index: usize) {
        if button_index >= self.generals.len() {
            return;
        }
        self.teletype_position = 0;
    }

    pub fn update_bio(&mut self, frames: usize, skip: usize) -> bool {
        let total = self.current_bio_text().chars().count();
        let prev = self.teletype_position;
        self.teletype_position = (self.teletype_position + frames * skip).min(total);
        self.teletype_position != prev
    }

    pub fn current_bio_lines(&self) -> [String; 4] {
        let idx = self.selected_general.unwrap_or(DEFAULT_GENERAL);
        if idx >= self.generals.len() {
            return [String::new(), String::new(), String::new(), String::new()];
        }
        let general = &self.generals[idx];
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

    pub fn mark_general_completed(&mut self, campaign_name: &str) {
        for general in &mut self.generals {
            if general.campaign.eq_ignore_ascii_case(campaign_name) {
                general.completed = true;
            }
        }
        if !self
            .completed_campaigns
            .iter()
            .any(|c| c.eq_ignore_ascii_case(campaign_name))
        {
            self.completed_campaigns.push(campaign_name.to_string());
        }
        self.update_enabled_generals();
    }

    fn update_enabled_generals(&mut self) {
        let all_prior_completed = self.completed_campaigns.len() > 0;
        for general in &mut self.generals {
            if all_prior_completed {
                general.enabled = general.starts_enabled || general.completed;
            }
        }
    }

    pub fn set_difficulty(&mut self, difficulty: GameDifficultyPort) {
        self.difficulty = difficulty;
    }

    pub fn set_rank_points(&mut self, points: i32) {
        self.rank_points = points;
    }

    pub fn init(&mut self) {
        self.selected_general = None;
        self.last_hilited_index = None;
        self.is_auto_selecting = false;
        self.teletype_position = 0;
        self.intro_sequence_step = 0;
        self.intro_audio_counter = 0;
        self.has_played_intro_audio = false;
        self.just_entered = true;
        self.initial_gadget_delay = 2;
        self.is_shutting_down = false;
        self.can_play = false;
        self.update_enabled_generals();
    }

    pub fn update(&mut self, transitions_finished: bool) -> ChallengeMenuAction {
        if self.just_entered {
            if self.initial_gadget_delay == 1 {
                self.initial_gadget_delay = 2;
                self.just_entered = false;
            } else {
                self.initial_gadget_delay -= 1;
            }
        }

        if !self.has_played_intro_audio && transitions_finished {
            self.intro_audio_counter += 1;
            if self.intro_audio_counter == 10 {
                self.has_played_intro_audio = true;
                return ChallengeMenuAction::PreviewGeneral(0);
            }
        }

        self.update_bio(1, TELETYPE_SKIP);

        ChallengeMenuAction::None
    }

    pub fn handle_button(&mut self, button_id: i32) -> ChallengeMenuAction {
        if self.is_auto_selecting {
            self.is_auto_selecting = false;
            return ChallengeMenuAction::None;
        }

        match self.classify_button(button_id) {
            ChallengeMenuButton::GeneralPosition(index) => {
                if !self.generals[index].enabled {
                    return ChallengeMenuAction::None;
                }
                let prev = self.selected_general;
                if prev.is_some() && prev != Some(index) {
                    self.is_auto_selecting = true;
                }
                self.selected_general = Some(index);
                self.set_general_bio(index);
                self.can_play = true;
                ChallengeMenuAction::SelectGeneral(index)
            }
            ChallengeMenuButton::Play => {
                let Some(button_index) = self.selected_general else {
                    return ChallengeMenuAction::None;
                };
                if button_index >= self.generals.len() {
                    return ChallengeMenuAction::None;
                }

                self.set_general_campaign(button_index);

                let general = &self.generals[button_index];
                let campaign = general.campaign.clone();
                let player_template = general.player_template_name.clone();
                let map = general.current_map.clone();
                let difficulty = self.difficulty;
                let rank_points = self.rank_points;

                self.is_auto_selecting = true;
                self.selected_general = None;
                self.intro_sequence_step = 0;

                ChallengeMenuAction::LaunchChallenge {
                    campaign,
                    player_template,
                    map,
                    difficulty,
                    rank_points,
                }
            }
            ChallengeMenuButton::Back => ChallengeMenuAction::Back,
        }
    }

    pub fn handle_mouse_entering(&mut self, button_id: i32) -> ChallengeMenuAction {
        if let Some(index) = self.find_position_button(button_id) {
            if self.selected_general != Some(index) {
                self.set_general_bio(index);
                self.last_hilited_index = Some(index);
                return ChallengeMenuAction::PreviewGeneral(index);
            }
        }
        ChallengeMenuAction::None
    }

    pub fn handle_mouse_leaving(&mut self, button_id: i32) -> ChallengeMenuAction {
        if let Some(index) = self.find_position_button(button_id) {
            if self.selected_general != Some(index) {
                if let Some(selected) = self.selected_general {
                    self.set_general_bio(selected);
                }
            }
        }
        ChallengeMenuAction::None
    }

    pub fn shutdown(&mut self, immediate: bool) -> bool {
        self.selected_general = None;
        self.intro_sequence_step = 0;
        if immediate {
            return true;
        }
        self.is_shutting_down = true;
        self.intro_audio_counter = 0;
        false
    }

    pub fn sample() -> Self {
        Self {
            selected_general: None,
            last_hilited_index: None,
            is_auto_selecting: false,
            teletype_position: 0,
            intro_sequence_step: 0,
            intro_audio_counter: 0,
            has_played_intro_audio: false,
            just_entered: true,
            initial_gadget_delay: 2,
            is_shutting_down: false,
            can_play: false,
            difficulty: GameDifficultyPort::Normal,
            rank_points: 0,
            generals: vec![
                ChallengeGeneralPort {
                    name: "General Alexander".to_string(),
                    enabled: true,
                    starts_enabled: true,
                    campaign: "BossGeneral".to_string(),
                    current_map: "Maps/Challenge/BossGeneral/BossGeneral.map".to_string(),
                    player_template_name: "FactionAmericaSuperWeaponGeneral".to_string(),
                    bio_name: "Name: Alexander".to_string(),
                    bio_rank: "Rank: 4 Star General".to_string(),
                    bio_branch: "Branch: USA Superweapon".to_string(),
                    bio_strategy: "Strategy: Superweapons and defensive fortification.".to_string(),
                    completed: false,
                },
                ChallengeGeneralPort {
                    name: "General Kwai".to_string(),
                    enabled: true,
                    starts_enabled: true,
                    campaign: "TankGeneral".to_string(),
                    current_map: "Maps/Challenge/TankGeneral/TankGeneral.map".to_string(),
                    player_template_name: "FactionChinaTankGeneral".to_string(),
                    bio_name: "Name: Kwai".to_string(),
                    bio_rank: "Rank: General".to_string(),
                    bio_branch: "Branch: China Tank".to_string(),
                    bio_strategy: "Strategy: Armored assault and overwhelming firepower."
                        .to_string(),
                    completed: false,
                },
                ChallengeGeneralPort {
                    name: "General Leang".to_string(),
                    enabled: false,
                    starts_enabled: false,
                    campaign: "Challenge_Leang".to_string(),
                    current_map: "Maps/Challenge/Challenge_Leang/Challenge_Leang.map".to_string(),
                    player_template_name: "FactionBossGeneral".to_string(),
                    bio_name: "Name: Leang".to_string(),
                    bio_rank: "Rank: General".to_string(),
                    bio_branch: "Branch: Boss".to_string(),
                    bio_strategy: "Strategy: Unknown".to_string(),
                    completed: false,
                },
            ],
            completed_campaigns: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selecting_enabled_general_resets_teletype() {
        let mut menu = ChallengeMenuPort::sample();
        menu.select_general(0);
        menu.teletype_position = 20;

        assert!(menu.select_general(0));
        assert_eq!(menu.teletype_position, 0);
    }

    #[test]
    fn selecting_disabled_general_fails() {
        let mut menu = ChallengeMenuPort::sample();
        assert!(!menu.select_general(2));
    }

    #[test]
    fn teletype_update_reveals_bio_incrementally() {
        let mut menu = ChallengeMenuPort::sample();
        menu.select_general(0);
        menu.update_bio(2, 2);

        assert_eq!(menu.current_readout().chars().count(), 4);
    }

    #[test]
    fn handle_button_selects_general() {
        let mut menu = ChallengeMenuPort::sample();
        let action = menu.handle_button(0);
        assert_eq!(action, ChallengeMenuAction::SelectGeneral(0));
        assert_eq!(menu.selected_general, Some(0));
        assert!(menu.can_play);
    }

    #[test]
    fn handle_button_launches_challenge_with_campaign_current_map() {
        let mut menu = ChallengeMenuPort::sample();
        menu.generals[0].current_map = "Maps/Challenge/Resolved/Resolved.map".to_string();
        menu.handle_button(0);

        let action = menu.handle_button(BUTTON_ID_PLAY);
        match action {
            ChallengeMenuAction::LaunchChallenge {
                campaign,
                player_template,
                map,
                difficulty,
                rank_points,
            } => {
                assert_eq!(campaign, "BossGeneral");
                assert_eq!(player_template, "FactionAmericaSuperWeaponGeneral");
                assert_eq!(map, "Maps/Challenge/Resolved/Resolved.map");
                assert_eq!(difficulty, GameDifficultyPort::Normal);
                assert_eq!(rank_points, 0);
            }
            _ => panic!("expected LaunchChallenge action, got {:?}", action),
        }
        assert_eq!(menu.selected_general, None);
    }

    #[test]
    fn handle_button_back_returns_back_action() {
        let mut menu = ChallengeMenuPort::sample();
        let action = menu.handle_button(BUTTON_ID_BACK);
        assert_eq!(action, ChallengeMenuAction::Back);
    }

    #[test]
    fn handle_button_play_without_selection_returns_none() {
        let mut menu = ChallengeMenuPort::sample();
        let action = menu.handle_button(BUTTON_ID_PLAY);
        assert_eq!(action, ChallengeMenuAction::None);
    }

    #[test]
    fn auto_selecting_suppresses_button() {
        let mut menu = ChallengeMenuPort::sample();
        menu.handle_button(0);
        menu.is_auto_selecting = true;
        let action = menu.handle_button(1);
        assert_eq!(action, ChallengeMenuAction::None);
        assert!(!menu.is_auto_selecting);
    }

    #[test]
    fn handle_mouse_entering_previews_general() {
        let mut menu = ChallengeMenuPort::sample();
        let action = menu.handle_mouse_entering(0);
        assert_eq!(action, ChallengeMenuAction::PreviewGeneral(0));
        assert_eq!(menu.last_hilited_index, Some(0));
    }

    #[test]
    fn handle_mouse_leaving_restores_selected_bio() {
        let mut menu = ChallengeMenuPort::sample();
        menu.handle_button(0);
        menu.handle_mouse_entering(1);
        let action = menu.handle_mouse_leaving(1);
        assert_eq!(action, ChallengeMenuAction::None);
    }

    #[test]
    fn mark_general_completed_tracks_completion() {
        let mut menu = ChallengeMenuPort::sample();
        menu.mark_general_completed("BossGeneral");
        assert!(menu.generals[0].completed);
        assert!(menu
            .completed_campaigns
            .contains(&"BossGeneral".to_string()));
    }

    #[test]
    fn init_resets_all_state() {
        let mut menu = ChallengeMenuPort::sample();
        menu.handle_button(0);
        menu.init();
        assert_eq!(menu.selected_general, None);
        assert!(menu.just_entered);
        assert!(!menu.can_play);
        assert_eq!(menu.intro_audio_counter, 0);
    }

    #[test]
    fn shutdown_immediate_returns_true() {
        let mut menu = ChallengeMenuPort::sample();
        assert!(menu.shutdown(true));
        assert_eq!(menu.selected_general, None);
    }

    #[test]
    fn shutdown_animated_sets_shutting_down() {
        let mut menu = ChallengeMenuPort::sample();
        menu.handle_button(0);
        assert!(!menu.shutdown(false));
        assert!(menu.is_shutting_down);
        assert_eq!(menu.selected_general, None);
        assert_eq!(menu.intro_sequence_step, 0);
    }

    #[test]
    fn set_difficulty_changes_stored_difficulty() {
        let mut menu = ChallengeMenuPort::sample();
        menu.set_difficulty(GameDifficultyPort::Hard);
        assert_eq!(menu.difficulty, GameDifficultyPort::Hard);
    }

    #[test]
    fn launch_challenge_uses_current_difficulty() {
        let mut menu = ChallengeMenuPort::sample();
        menu.set_difficulty(GameDifficultyPort::Hard);
        menu.set_rank_points(500);
        menu.handle_button(0);

        let action = menu.handle_button(BUTTON_ID_PLAY);
        match action {
            ChallengeMenuAction::LaunchChallenge {
                difficulty,
                rank_points,
                ..
            } => {
                assert_eq!(difficulty, GameDifficultyPort::Hard);
                assert_eq!(rank_points, 500);
            }
            _ => panic!("expected LaunchChallenge"),
        }
    }

    #[test]
    fn update_returns_preview_on_intro_audio() {
        let mut menu = ChallengeMenuPort::sample();
        menu.just_entered = false;
        for _ in 0..9 {
            assert!(matches!(menu.update(true), ChallengeMenuAction::None));
        }
        assert!(matches!(
            menu.update(true),
            ChallengeMenuAction::PreviewGeneral(0)
        ));
        assert!(menu.has_played_intro_audio);
    }

    #[test]
    fn find_position_button_validates_range() {
        let menu = ChallengeMenuPort::sample();
        assert_eq!(menu.find_position_button(0), Some(0));
        assert_eq!(menu.find_position_button(2), Some(2));
        assert_eq!(menu.find_position_button(3), None);
        assert_eq!(menu.find_position_button(-1), None);
    }
}
