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

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ScoreScreenInitType {
    SinglePlayer = 0,
    Skirmish,
    Lan,
    Internet,
    Replay,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum ScoreScreenButtonId {
    Ok,
    Continue,
    Buddies,
    SaveReplay,
    Emote,
    AddBuddy { slot: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ScoreScreenAction {
    PopShell,
    PopAndClearCampaign,
    StartNextCampaignGame,
    RetryCampaignMission,
    EndCampaign,
    ToggleBuddyOverlay,
    ShowSaveReplayPopup,
    SendEmoteChat { text: String },
    SendNormalChat { text: String },
    RequestAddBuddy { profile_id: i32 },
    NoAction,
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScoreScreenControlVisibility {
    pub chat_entry: bool,
    pub emote_button: bool,
    pub chat_border: bool,
    pub continue_button: bool,
    pub chat_listbox: bool,
    pub buddies_button: bool,
    pub save_replay_button: bool,
    pub game_saved_text: bool,
    pub academy_listbox: bool,
    pub academy_title: bool,
    pub ok_button: bool,
}

impl Default for ScoreScreenControlVisibility {
    fn default() -> Self {
        Self {
            chat_entry: true,
            emote_button: true,
            chat_border: true,
            continue_button: true,
            chat_listbox: true,
            buddies_button: true,
            save_replay_button: true,
            game_saved_text: false,
            academy_listbox: false,
            academy_title: false,
            ok_button: true,
        }
    }
}

impl ScoreScreenControlVisibility {
    pub fn for_init_type(init_type: ScoreScreenInitType) -> Self {
        match init_type {
            ScoreScreenInitType::Skirmish => Self {
                chat_entry: false,
                emote_button: false,
                chat_border: false,
                continue_button: false,
                chat_listbox: false,
                buddies_button: false,
                save_replay_button: true,
                game_saved_text: false,
                academy_listbox: false,
                academy_title: false,
                ok_button: true,
            },
            ScoreScreenInitType::Lan => Self {
                chat_entry: true,
                emote_button: true,
                chat_border: true,
                continue_button: false,
                chat_listbox: true,
                buddies_button: false,
                save_replay_button: true,
                game_saved_text: false,
                academy_listbox: false,
                academy_title: false,
                ok_button: true,
            },
            ScoreScreenInitType::Internet => Self {
                chat_entry: false,
                emote_button: false,
                chat_border: true,
                continue_button: false,
                chat_listbox: true,
                buddies_button: true,
                save_replay_button: true,
                game_saved_text: false,
                academy_listbox: true,
                academy_title: true,
                ok_button: true,
            },
            ScoreScreenInitType::SinglePlayer => Self {
                chat_entry: false,
                emote_button: false,
                chat_border: false,
                continue_button: true,
                chat_listbox: false,
                buddies_button: false,
                save_replay_button: false,
                game_saved_text: false,
                academy_listbox: false,
                academy_title: false,
                ok_button: true,
            },
            ScoreScreenInitType::Replay => Self {
                chat_entry: false,
                emote_button: false,
                chat_border: false,
                continue_button: false,
                chat_listbox: false,
                buddies_button: false,
                save_replay_button: false,
                game_saved_text: false,
                academy_listbox: false,
                academy_title: false,
                ok_button: true,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SinglePlayerResult {
    Victory,
    Defeat,
    CampaignComplete { campaign_name: CampaignSide },
    ChallengeVictory,
    ChallengeDefeat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CampaignSide {
    Usa,
    China,
    Gla,
    Challenge { index: usize },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BattleHonorEntry {
    pub honor_flag: u32,
    pub label: String,
}

impl BattleHonorEntry {
    pub fn streak() -> Self {
        Self {
            honor_flag: 0x00000002,
            label: "Streak".to_string(),
        }
    }

    pub fn battle_tank() -> Self {
        Self {
            honor_flag: 0x00000080,
            label: "Battle Tank".to_string(),
        }
    }

    pub fn air_wing() -> Self {
        Self {
            honor_flag: 0x00000100,
            label: "Air Wing".to_string(),
        }
    }

    pub fn apocalypse() -> Self {
        Self {
            honor_flag: 0x00020000,
            label: "Apocalypse".to_string(),
        }
    }

    pub fn blitz5() -> Self {
        Self {
            honor_flag: 0x00004000,
            label: "Blitz".to_string(),
        }
    }

    pub fn blitz10() -> Self {
        Self {
            honor_flag: 0x00008000,
            label: "Blitz".to_string(),
        }
    }

    pub fn loyalty(side: &str) -> Self {
        match side {
            "America" => Self {
                honor_flag: 0x00000020,
                label: "Loyalty".to_string(),
            },
            "China" => Self {
                honor_flag: 0x00000040,
                label: "Loyalty".to_string(),
            },
            "GLA" => Self {
                honor_flag: 0x00000200,
                label: "Loyalty".to_string(),
            },
            _ => Self {
                honor_flag: 0,
                label: "Loyalty".to_string(),
            },
        }
    }

    pub fn campaign_usa() -> Self {
        Self {
            honor_flag: 0x00000800,
            label: "Campaign".to_string(),
        }
    }

    pub fn campaign_china() -> Self {
        Self {
            honor_flag: 0x00001000,
            label: "Campaign".to_string(),
        }
    }

    pub fn campaign_gla() -> Self {
        Self {
            honor_flag: 0x00002000,
            label: "Campaign".to_string(),
        }
    }

    pub fn challenge_mode() -> Self {
        Self {
            honor_flag: 0x00100000,
            label: "Challenge".to_string(),
        }
    }

    pub fn global_general() -> Self {
        Self {
            honor_flag: 0x00400000,
            label: "Global General".to_string(),
        }
    }
}

pub const MAX_SLOTS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScoreGather {
    pub total_money_earned: i32,
    pub total_money_spent: i32,
    pub total_units_destroyed: i32,
    pub total_units_built: i32,
    pub total_units_lost: i32,
    pub total_buildings_destroyed: i32,
    pub total_buildings_built: i32,
    pub total_buildings_lost: i32,
}

impl Default for ScoreGather {
    fn default() -> Self {
        Self {
            total_money_earned: 0,
            total_money_spent: 0,
            total_units_destroyed: 0,
            total_units_built: 0,
            total_units_lost: 0,
            total_buildings_destroyed: 0,
            total_buildings_built: 0,
            total_buildings_lost: 0,
        }
    }
}

impl ScoreGather {
    pub fn accumulate(&mut self, other: &ScoreGather) {
        self.total_money_earned += other.total_money_earned;
        self.total_money_spent += other.total_money_spent;
        self.total_units_destroyed += other.total_units_destroyed;
        self.total_units_built += other.total_units_built;
        self.total_units_lost += other.total_units_lost;
        self.total_buildings_destroyed += other.total_buildings_destroyed;
        self.total_buildings_built += other.total_buildings_built;
        self.total_buildings_lost += other.total_buildings_lost;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScoreScreenRelation {
    UsaFriend,
    ChinaFriend,
    GlaFriend,
    UsaEnemy,
    ChinaEnemy,
    GlaEnemy,
}

impl ScoreScreenRelation {
    pub fn side(&self) -> &'static str {
        match self {
            Self::UsaFriend | Self::UsaEnemy => "USA",
            Self::ChinaFriend | Self::ChinaEnemy => "China",
            Self::GlaFriend | Self::GlaEnemy => "GLA",
        }
    }

    pub fn is_friend(&self) -> bool {
        match self {
            Self::UsaFriend | Self::ChinaFriend | Self::GlaFriend => true,
            Self::UsaEnemy | Self::ChinaEnemy | Self::GlaEnemy => false,
        }
    }

    pub fn label_key(&self) -> String {
        let side = self.side();
        if self.is_friend() {
            format!("GUI:{}Allies", side)
        } else {
            format!("GUI:{}Enemies", side)
        }
    }
}

pub const ALL_RELATIONS: [ScoreScreenRelation; 6] = [
    ScoreScreenRelation::UsaFriend,
    ScoreScreenRelation::ChinaFriend,
    ScoreScreenRelation::GlaFriend,
    ScoreScreenRelation::UsaEnemy,
    ScoreScreenRelation::ChinaEnemy,
    ScoreScreenRelation::GlaEnemy,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlayerScoreEntry {
    pub display_name: String,
    pub units_built: i32,
    pub units_lost: i32,
    pub units_destroyed: i32,
    pub buildings_built: i32,
    pub buildings_lost: i32,
    pub buildings_destroyed: i32,
    pub resources: i32,
    pub score: i32,
    pub is_observer: bool,
    pub is_local: bool,
}

#[derive(Clone, Debug)]
pub struct ScoreScreenState {
    pub init_type: ScoreScreenInitType,
    pub button_is_finish_campaign: bool,
    pub can_save_replay: bool,
    pub override_player_display_name: bool,
    pub need_finish_single_player_init: bool,
    pub replay_was_pressed: bool,
    pub last_replay_filename: String,
    pub control_visibility: ScoreScreenControlVisibility,
    pub player_entries: Vec<PlayerScoreEntry>,
    pub single_player_result: Option<SinglePlayerResult>,
    pub battle_honors_earned: Vec<BattleHonorEntry>,
}

impl Default for ScoreScreenState {
    fn default() -> Self {
        Self {
            init_type: ScoreScreenInitType::SinglePlayer,
            button_is_finish_campaign: false,
            can_save_replay: false,
            override_player_display_name: false,
            need_finish_single_player_init: false,
            replay_was_pressed: false,
            last_replay_filename: String::new(),
            control_visibility: ScoreScreenControlVisibility::default(),
            player_entries: Vec::new(),
            single_player_result: None,
            battle_honors_earned: Vec::new(),
        }
    }
}

impl ScoreScreenState {
    pub fn new(init_type: ScoreScreenInitType) -> Self {
        let visibility = ScoreScreenControlVisibility::for_init_type(init_type);
        let override_name = matches!(
            init_type,
            ScoreScreenInitType::SinglePlayer | ScoreScreenInitType::Replay
        );
        Self {
            init_type,
            control_visibility: visibility,
            override_player_display_name: override_name,
            ..Self::default()
        }
    }

    pub fn set_replay_mode(&mut self, recorder_mode: RecorderMode) {
        match recorder_mode {
            RecorderMode::Record => {
                self.can_save_replay = true;
            }
            RecorderMode::None => {
                self.can_save_replay = false;
            }
        }
    }

    pub fn apply_challenge_override(&mut self, is_challenge: bool) {
        if is_challenge {
            self.control_visibility.save_replay_button = false;
        }
    }

    pub fn handle_button(&mut self, button: ScoreScreenButtonId) -> ScoreScreenAction {
        self.replay_was_pressed = false;

        match button {
            ScoreScreenButtonId::Ok => match self.init_type {
                ScoreScreenInitType::SinglePlayer => {
                    if self.button_is_finish_campaign {
                        return ScoreScreenAction::PopShell;
                    }
                    ScoreScreenAction::PopAndClearCampaign
                }
                _ => ScoreScreenAction::PopAndClearCampaign,
            },
            ScoreScreenButtonId::Continue => {
                if !self.button_is_finish_campaign {
                    self.replay_was_pressed = true;
                }
                match self.init_type {
                    ScoreScreenInitType::SinglePlayer => match &self.single_player_result {
                        Some(SinglePlayerResult::CampaignComplete { .. }) => {
                            ScoreScreenAction::EndCampaign
                        }
                        None => {
                            self.replay_was_pressed = false;
                            ScoreScreenAction::PopShell
                        }
                        _ => ScoreScreenAction::StartNextCampaignGame,
                    },
                    _ => ScoreScreenAction::NoAction,
                }
            }
            ScoreScreenButtonId::Buddies => ScoreScreenAction::ToggleBuddyOverlay,
            ScoreScreenButtonId::SaveReplay => ScoreScreenAction::ShowSaveReplayPopup,
            ScoreScreenButtonId::Emote => ScoreScreenAction::NoAction,
            ScoreScreenButtonId::AddBuddy { slot: _ } => ScoreScreenAction::NoAction,
        }
    }

    pub fn enable_controls(&self) -> bool {
        true
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecorderMode {
    None,
    Record,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlotAiState {
    Easy,
    Medium,
    Brutal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnduranceMedalUpdate {
    pub map_name: String,
    pub ai_difficulty: SlotAiState,
    pub opponents: i32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SkirmishBattleHonorUpdate {
    pub victory: bool,
    pub player_side: String,
    pub game_minutes: i32,
    pub vehicles_built: i32,
    pub aircraft_built: i32,
    pub built_nuke: bool,
    pub built_particle_cannon: bool,
    pub built_scud: bool,
    pub win_streak: i32,
    pub games_loyal: i32,
    pub endurance_updates: Vec<EnduranceMedalUpdate>,
    pub challenge_brutals: i32,
}

impl Default for SkirmishBattleHonorUpdate {
    fn default() -> Self {
        Self {
            victory: false,
            player_side: String::new(),
            game_minutes: 0,
            vehicles_built: 0,
            aircraft_built: 0,
            built_nuke: false,
            built_particle_cannon: false,
            built_scud: false,
            win_streak: 0,
            games_loyal: 0,
            endurance_updates: Vec::new(),
            challenge_brutals: 0,
        }
    }
}

impl SkirmishBattleHonorUpdate {
    pub fn compute_honors(&self) -> Vec<BattleHonorEntry> {
        let mut honors = Vec::new();

        if self.win_streak >= 5 {
            honors.push(BattleHonorEntry::streak());
        }

        if self.built_nuke && self.built_particle_cannon && self.built_scud {
            honors.push(BattleHonorEntry::apocalypse());
        }

        if self.vehicles_built >= 50 {
            honors.push(BattleHonorEntry::battle_tank());
        }

        if self.aircraft_built >= 20 {
            honors.push(BattleHonorEntry::air_wing());
        }

        if self.game_minutes < 5 {
            honors.push(BattleHonorEntry::blitz5());
        }

        if self.game_minutes < 10 {
            honors.push(BattleHonorEntry::blitz10());
        }

        if self.games_loyal >= 20 {
            let loyalty = BattleHonorEntry::loyalty(&self.player_side);
            if loyalty.honor_flag != 0 {
                honors.push(loyalty);
            }
        }

        honors
    }

    pub fn compute_challenge_medals(&self) -> u32 {
        if self.challenge_brutals == 0 {
            return 0;
        }
        match self.challenge_brutals {
            1 => 0x0001,
            2 => 0x0003,
            3 => 0x0007,
            4 => 0x000F,
            5 => 0x001F,
            6 => 0x003F,
            7 => 0x007F,
            _ => 0x007F,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiplayerBattleHonorUpdate {
    pub victory: bool,
    pub player_side: String,
    pub game_minutes: i32,
    pub vehicles_built: i32,
    pub aircraft_built: i32,
    pub built_nuke: bool,
    pub built_particle_cannon: bool,
    pub built_scud: bool,
    pub wins_in_a_row: i32,
    pub games_in_row_with_last_general: i32,
    pub global_challenge_wins: i32,
}

impl Default for MultiplayerBattleHonorUpdate {
    fn default() -> Self {
        Self {
            victory: false,
            player_side: String::new(),
            game_minutes: 0,
            vehicles_built: 0,
            aircraft_built: 0,
            built_nuke: false,
            built_particle_cannon: false,
            built_scud: false,
            wins_in_a_row: 0,
            games_in_row_with_last_general: 0,
            global_challenge_wins: 0,
        }
    }
}

impl MultiplayerBattleHonorUpdate {
    pub fn compute_honors(&self) -> Vec<BattleHonorEntry> {
        let mut honors = Vec::new();

        if self.wins_in_a_row >= 5 {
            honors.push(BattleHonorEntry::streak());
        }

        if self.games_in_row_with_last_general >= 20 {
            let loyalty = BattleHonorEntry::loyalty(&self.player_side);
            if loyalty.honor_flag != 0 {
                honors.push(loyalty);
            }
        }

        if self.vehicles_built >= 50 {
            honors.push(BattleHonorEntry::battle_tank());
        }

        if self.aircraft_built >= 20 {
            honors.push(BattleHonorEntry::air_wing());
        }

        if self.built_nuke && self.built_particle_cannon && self.built_scud {
            honors.push(BattleHonorEntry::apocalypse());
        }

        if self.game_minutes < 5 {
            honors.push(BattleHonorEntry::blitz5());
        }

        if self.game_minutes < 10 {
            honors.push(BattleHonorEntry::blitz10());
        }

        honors
    }

    pub fn compute_challenge_medals(&self) -> u32 {
        if self.global_challenge_wins >= 9 {
            0x007F
        } else {
            0
        }
    }
}

pub fn determine_init_type(
    is_replay: bool,
    is_multiplayer_replay: bool,
    is_internet_game: bool,
    is_lan_game: bool,
    is_skirmish_game: bool,
) -> ScoreScreenInitType {
    if is_replay {
        if is_multiplayer_replay {
            ScoreScreenInitType::Replay
        } else {
            ScoreScreenInitType::Replay
        }
    } else if is_internet_game {
        ScoreScreenInitType::Internet
    } else if is_lan_game {
        ScoreScreenInitType::Lan
    } else if is_skirmish_game {
        ScoreScreenInitType::Skirmish
    } else {
        ScoreScreenInitType::SinglePlayer
    }
}

pub fn apply_internet_buddies_visibility(
    visibility: &mut ScoreScreenControlVisibility,
    has_local_profile: bool,
) {
    visibility.buddies_button = has_local_profile;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_type_skirmish() {
        let init_type = determine_init_type(false, false, false, false, true);
        assert_eq!(init_type, ScoreScreenInitType::Skirmish);
    }

    #[test]
    fn test_init_type_single_player() {
        let init_type = determine_init_type(false, false, false, false, false);
        assert_eq!(init_type, ScoreScreenInitType::SinglePlayer);
    }

    #[test]
    fn test_init_type_lan() {
        let init_type = determine_init_type(false, false, false, true, false);
        assert_eq!(init_type, ScoreScreenInitType::Lan);
    }

    #[test]
    fn test_init_type_internet() {
        let init_type = determine_init_type(false, false, true, false, false);
        assert_eq!(init_type, ScoreScreenInitType::Internet);
    }

    #[test]
    fn test_init_type_replay() {
        let init_type = determine_init_type(true, false, false, false, false);
        assert_eq!(init_type, ScoreScreenInitType::Replay);
    }

    #[test]
    fn test_init_type_replay_multiplayer() {
        let init_type = determine_init_type(true, true, false, false, false);
        assert_eq!(init_type, ScoreScreenInitType::Replay);
    }

    #[test]
    fn test_internet_priority_over_lan() {
        let init_type = determine_init_type(false, false, true, true, false);
        assert_eq!(init_type, ScoreScreenInitType::Internet);
    }

    #[test]
    fn test_lan_priority_over_skirmish() {
        let init_type = determine_init_type(false, false, false, true, true);
        assert_eq!(init_type, ScoreScreenInitType::Lan);
    }

    #[test]
    fn test_replay_priority_over_all() {
        let init_type = determine_init_type(true, true, true, true, true);
        assert_eq!(init_type, ScoreScreenInitType::Replay);
    }

    #[test]
    fn test_visibility_skirmish() {
        let vis = ScoreScreenControlVisibility::for_init_type(ScoreScreenInitType::Skirmish);
        assert!(!vis.chat_entry);
        assert!(!vis.emote_button);
        assert!(!vis.chat_border);
        assert!(!vis.continue_button);
        assert!(!vis.chat_listbox);
        assert!(!vis.buddies_button);
        assert!(vis.save_replay_button);
        assert!(!vis.game_saved_text);
        assert!(!vis.academy_listbox);
        assert!(!vis.academy_title);
        assert!(vis.ok_button);
    }

    #[test]
    fn test_visibility_lan() {
        let vis = ScoreScreenControlVisibility::for_init_type(ScoreScreenInitType::Lan);
        assert!(vis.chat_entry);
        assert!(vis.emote_button);
        assert!(vis.chat_border);
        assert!(!vis.continue_button);
        assert!(vis.chat_listbox);
        assert!(!vis.buddies_button);
        assert!(vis.save_replay_button);
        assert!(!vis.academy_listbox);
        assert!(!vis.academy_title);
        assert!(vis.ok_button);
    }

    #[test]
    fn test_visibility_internet() {
        let vis = ScoreScreenControlVisibility::for_init_type(ScoreScreenInitType::Internet);
        assert!(!vis.chat_entry);
        assert!(!vis.emote_button);
        assert!(vis.chat_border);
        assert!(!vis.continue_button);
        assert!(vis.chat_listbox);
        assert!(vis.buddies_button);
        assert!(vis.save_replay_button);
        assert!(vis.academy_listbox);
        assert!(vis.academy_title);
        assert!(vis.ok_button);
    }

    #[test]
    fn test_visibility_single_player() {
        let vis = ScoreScreenControlVisibility::for_init_type(ScoreScreenInitType::SinglePlayer);
        assert!(!vis.chat_entry);
        assert!(!vis.emote_button);
        assert!(!vis.chat_border);
        assert!(vis.continue_button);
        assert!(!vis.chat_listbox);
        assert!(!vis.buddies_button);
        assert!(!vis.save_replay_button);
        assert!(!vis.academy_listbox);
        assert!(!vis.academy_title);
        assert!(vis.ok_button);
    }

    #[test]
    fn test_visibility_replay() {
        let vis = ScoreScreenControlVisibility::for_init_type(ScoreScreenInitType::Replay);
        assert!(!vis.chat_entry);
        assert!(!vis.emote_button);
        assert!(!vis.chat_border);
        assert!(!vis.continue_button);
        assert!(!vis.chat_listbox);
        assert!(!vis.buddies_button);
        assert!(!vis.save_replay_button);
        assert!(!vis.academy_listbox);
        assert!(!vis.academy_title);
        assert!(vis.ok_button);
    }

    #[test]
    fn test_handle_button_ok_pops_campaign() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::Skirmish);
        let action = state.handle_button(ScoreScreenButtonId::Ok);
        assert_eq!(action, ScoreScreenAction::PopAndClearCampaign);
    }

    #[test]
    fn test_handle_button_ok_finish_campaign() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::SinglePlayer);
        state.button_is_finish_campaign = true;
        let action = state.handle_button(ScoreScreenButtonId::Ok);
        assert_eq!(action, ScoreScreenAction::PopShell);
    }

    #[test]
    fn test_handle_button_continue_single_player_no_map() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::SinglePlayer);
        state.single_player_result = None;
        let action = state.handle_button(ScoreScreenButtonId::Continue);
        assert_eq!(action, ScoreScreenAction::PopShell);
        assert!(!state.replay_was_pressed);
    }

    #[test]
    fn test_handle_button_continue_single_player_victory() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::SinglePlayer);
        state.single_player_result = Some(SinglePlayerResult::Victory);
        let action = state.handle_button(ScoreScreenButtonId::Continue);
        assert_eq!(action, ScoreScreenAction::StartNextCampaignGame);
        assert!(state.replay_was_pressed);
    }

    #[test]
    fn test_handle_button_continue_campaign_complete() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::SinglePlayer);
        state.button_is_finish_campaign = true;
        state.single_player_result = Some(SinglePlayerResult::CampaignComplete {
            campaign_name: CampaignSide::Usa,
        });
        let action = state.handle_button(ScoreScreenButtonId::Continue);
        assert_eq!(action, ScoreScreenAction::EndCampaign);
        assert!(!state.replay_was_pressed);
    }

    #[test]
    fn test_handle_button_continue_skirmish_noop() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::Skirmish);
        let action = state.handle_button(ScoreScreenButtonId::Continue);
        assert_eq!(action, ScoreScreenAction::NoAction);
    }

    #[test]
    fn test_handle_button_buddies() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::Internet);
        let action = state.handle_button(ScoreScreenButtonId::Buddies);
        assert_eq!(action, ScoreScreenAction::ToggleBuddyOverlay);
    }

    #[test]
    fn test_handle_button_save_replay() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::Skirmish);
        let action = state.handle_button(ScoreScreenButtonId::SaveReplay);
        assert_eq!(action, ScoreScreenAction::ShowSaveReplayPopup);
    }

    #[test]
    fn test_override_player_display_name_single() {
        let state = ScoreScreenState::new(ScoreScreenInitType::SinglePlayer);
        assert!(state.override_player_display_name);
    }

    #[test]
    fn test_override_player_display_name_replay() {
        let state = ScoreScreenState::new(ScoreScreenInitType::Replay);
        assert!(state.override_player_display_name);
    }

    #[test]
    fn test_override_player_display_name_skirmish() {
        let state = ScoreScreenState::new(ScoreScreenInitType::Skirmish);
        assert!(!state.override_player_display_name);
    }

    #[test]
    fn test_override_player_display_name_lan() {
        let state = ScoreScreenState::new(ScoreScreenInitType::Lan);
        assert!(!state.override_player_display_name);
    }

    #[test]
    fn test_override_player_display_name_internet() {
        let state = ScoreScreenState::new(ScoreScreenInitType::Internet);
        assert!(!state.override_player_display_name);
    }

    #[test]
    fn test_replay_mode_record() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::Skirmish);
        state.set_replay_mode(RecorderMode::Record);
        assert!(state.can_save_replay);
    }

    #[test]
    fn test_replay_mode_none() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::Skirmish);
        state.set_replay_mode(RecorderMode::None);
        assert!(!state.can_save_replay);
    }

    #[test]
    fn test_challenge_override_disables_save_replay() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::Skirmish);
        state.set_replay_mode(RecorderMode::Record);
        state.apply_challenge_override(true);
        assert!(!state.control_visibility.save_replay_button);
    }

    #[test]
    fn test_challenge_override_no_change_when_not_challenge() {
        let mut state = ScoreScreenState::new(ScoreScreenInitType::Skirmish);
        state.set_replay_mode(RecorderMode::Record);
        state.apply_challenge_override(false);
        assert!(state.control_visibility.save_replay_button);
    }

    #[test]
    fn test_internet_buddies_hidden_without_profile() {
        let mut vis = ScoreScreenControlVisibility::for_init_type(ScoreScreenInitType::Internet);
        apply_internet_buddies_visibility(&mut vis, false);
        assert!(!vis.buddies_button);
    }

    #[test]
    fn test_internet_buddies_shown_with_profile() {
        let mut vis = ScoreScreenControlVisibility::for_init_type(ScoreScreenInitType::Internet);
        apply_internet_buddies_visibility(&mut vis, true);
        assert!(vis.buddies_button);
    }

    #[test]
    fn test_skirmish_honor_streak() {
        let update = SkirmishBattleHonorUpdate {
            win_streak: 5,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00000002));
    }

    #[test]
    fn test_skirmish_honor_apocalypse() {
        let update = SkirmishBattleHonorUpdate {
            built_nuke: true,
            built_particle_cannon: true,
            built_scud: true,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00020000));
    }

    #[test]
    fn test_skirmish_honor_battle_tank() {
        let update = SkirmishBattleHonorUpdate {
            vehicles_built: 50,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00000080));
    }

    #[test]
    fn test_skirmish_honor_air_wing() {
        let update = SkirmishBattleHonorUpdate {
            aircraft_built: 20,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00000100));
    }

    #[test]
    fn test_skirmish_honor_blitz5() {
        let update = SkirmishBattleHonorUpdate {
            game_minutes: 4,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00004000));
    }

    #[test]
    fn test_skirmish_honor_blitz10() {
        let update = SkirmishBattleHonorUpdate {
            game_minutes: 9,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00008000));
    }

    #[test]
    fn test_skirmish_honor_loyalty_usa() {
        let update = SkirmishBattleHonorUpdate {
            player_side: "America".to_string(),
            games_loyal: 20,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00000020));
    }

    #[test]
    fn test_skirmish_honor_loyalty_china() {
        let update = SkirmishBattleHonorUpdate {
            player_side: "China".to_string(),
            games_loyal: 20,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00000040));
    }

    #[test]
    fn test_skirmish_honor_loyalty_gla() {
        let update = SkirmishBattleHonorUpdate {
            player_side: "GLA".to_string(),
            games_loyal: 20,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00000200));
    }

    #[test]
    fn test_skirmish_no_loyalty_under_20() {
        let update = SkirmishBattleHonorUpdate {
            player_side: "America".to_string(),
            games_loyal: 19,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(!honors.iter().any(|h| h.honor_flag == 0x00000020));
    }

    #[test]
    fn test_skirmish_challenge_medals() {
        let update = SkirmishBattleHonorUpdate {
            challenge_brutals: 3,
            ..Default::default()
        };
        let medals = update.compute_challenge_medals();
        assert_eq!(medals, 0x0007);
    }

    #[test]
    fn test_skirmish_challenge_medals_max() {
        let update = SkirmishBattleHonorUpdate {
            challenge_brutals: 7,
            ..Default::default()
        };
        let medals = update.compute_challenge_medals();
        assert_eq!(medals, 0x007F);
    }

    #[test]
    fn test_skirmish_challenge_medals_zero() {
        let update = SkirmishBattleHonorUpdate {
            challenge_brutals: 0,
            ..Default::default()
        };
        let medals = update.compute_challenge_medals();
        assert_eq!(medals, 0);
    }

    #[test]
    fn test_mp_honor_streak() {
        let update = MultiplayerBattleHonorUpdate {
            wins_in_a_row: 5,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00000002));
    }

    #[test]
    fn test_mp_honor_apocalypse() {
        let update = MultiplayerBattleHonorUpdate {
            built_nuke: true,
            built_particle_cannon: true,
            built_scud: true,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00020000));
    }

    #[test]
    fn test_mp_no_honor_streak_under_5() {
        let update = MultiplayerBattleHonorUpdate {
            wins_in_a_row: 4,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(!honors.iter().any(|h| h.honor_flag == 0x00000002));
    }

    #[test]
    fn test_mp_honor_loyalty() {
        let update = MultiplayerBattleHonorUpdate {
            player_side: "China".to_string(),
            games_in_row_with_last_general: 20,
            ..Default::default()
        };
        let honors = update.compute_honors();
        assert!(honors.iter().any(|h| h.honor_flag == 0x00000040));
    }

    #[test]
    fn test_relation_side_and_friend() {
        assert_eq!(ScoreScreenRelation::UsaFriend.side(), "USA");
        assert!(ScoreScreenRelation::UsaFriend.is_friend());
        assert_eq!(ScoreScreenRelation::UsaEnemy.side(), "USA");
        assert!(!ScoreScreenRelation::UsaEnemy.is_friend());
        assert_eq!(ScoreScreenRelation::ChinaFriend.side(), "China");
        assert!(ScoreScreenRelation::ChinaFriend.is_friend());
        assert_eq!(ScoreScreenRelation::GlaEnemy.side(), "GLA");
        assert!(!ScoreScreenRelation::GlaEnemy.is_friend());
    }

    #[test]
    fn test_relation_label_key() {
        assert_eq!(ScoreScreenRelation::UsaFriend.label_key(), "GUI:USAAllies");
        assert_eq!(ScoreScreenRelation::UsaEnemy.label_key(), "GUI:USAEnemies");
        assert_eq!(
            ScoreScreenRelation::ChinaFriend.label_key(),
            "GUI:ChinaAllies"
        );
        assert_eq!(ScoreScreenRelation::GlaEnemy.label_key(), "GUI:GLAEnemies");
    }

    #[test]
    fn test_all_relations_count() {
        assert_eq!(ALL_RELATIONS.len(), 6);
    }

    #[test]
    fn test_score_gather_accumulate() {
        let mut total = ScoreGather::default();
        let a = ScoreGather {
            total_units_built: 10,
            total_units_destroyed: 5,
            total_money_earned: 1000,
            ..Default::default()
        };
        let b = ScoreGather {
            total_units_built: 20,
            total_units_destroyed: 15,
            total_money_earned: 2000,
            total_buildings_built: 5,
            ..Default::default()
        };
        total.accumulate(&a);
        total.accumulate(&b);
        assert_eq!(total.total_units_built, 30);
        assert_eq!(total.total_units_destroyed, 20);
        assert_eq!(total.total_money_earned, 3000);
        assert_eq!(total.total_buildings_built, 5);
    }

    #[test]
    fn test_battle_honor_entry_helpers() {
        assert_eq!(BattleHonorEntry::streak().honor_flag, 0x00000002);
        assert_eq!(BattleHonorEntry::apocalypse().honor_flag, 0x00020000);
        assert_eq!(BattleHonorEntry::battle_tank().honor_flag, 0x00000080);
        assert_eq!(BattleHonorEntry::air_wing().honor_flag, 0x00000100);
        assert_eq!(BattleHonorEntry::blitz5().honor_flag, 0x00004000);
        assert_eq!(BattleHonorEntry::blitz10().honor_flag, 0x00008000);
        assert_eq!(BattleHonorEntry::campaign_usa().honor_flag, 0x00000800);
        assert_eq!(BattleHonorEntry::campaign_china().honor_flag, 0x00001000);
        assert_eq!(BattleHonorEntry::campaign_gla().honor_flag, 0x00002000);
        assert_eq!(BattleHonorEntry::challenge_mode().honor_flag, 0x00100000);
        assert_eq!(BattleHonorEntry::global_general().honor_flag, 0x00400000);
    }

    #[test]
    fn test_max_slots() {
        assert_eq!(MAX_SLOTS, 8);
    }
}
