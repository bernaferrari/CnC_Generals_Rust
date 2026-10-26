// FILE: challenge_generals.rs
// Author: Ported from C++ by Claude, Original by Steve Copeland, 6/24/2003
// Desc: Manager for data pertaining to the Generals' Challenge personas and related GUI.
//
// C++ Reference: /GeneralsMD/Code/GameEngine/Include/GameClient/ChallengeGenerals.h
//                /GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ChallengeGenerals.cpp

use game_engine::common::ini::{
    ensure_challenge_generals_loaded, get_challenge_generals as get_ini_challenge_generals,
};
use std::sync::OnceLock;

/// Number of generals in Challenge mode
/// Matches C++ ChallengeGenerals.h line 18
pub const NUM_GENERALS: usize = 12;

fn optional_ini_string(value: &str) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

/// Represents an individual General's persona data
/// Matches C++ GeneralPersona class (ChallengeGenerals.h lines 24-96)
#[derive(Debug, Clone)]
pub struct GeneralPersona {
    // Basic flags
    starts_enabled: bool,

    // Bio information strings
    bio_name: String,
    bio_dob: String,
    bio_birthplace: String,
    bio_strategy: String,
    bio_rank: String,
    bio_branch: String,
    bio_class_number: String,

    // Image paths (stored as strings, actual Image* would be managed elsewhere)
    bio_portrait_small: Option<String>,
    bio_portrait_large: Option<String>,

    // Campaign and template info
    campaign: String,
    player_template_name: String,

    // Portrait movie names
    portrait_movie_left_name: String,
    portrait_movie_right_name: String,

    // Victory/defeat images
    image_defeated: Option<String>,
    image_victorious: Option<String>,

    // Victory/defeat strings
    str_defeated: String,
    str_victorious: String,

    // Audio sound names
    selection_sound: String,
    taunt_sound_1: String,
    taunt_sound_2: String,
    taunt_sound_3: String,
    win_sound: String,
    loss_sound: String,
    preview_sound: String,
    name_sound: String,
}

impl GeneralPersona {
    /// Creates a new GeneralPersona with default values
    /// Matches C++ GeneralPersona constructor (ChallengeGenerals.h lines 58-62)
    pub fn new() -> Self {
        Self {
            starts_enabled: false,
            bio_name: String::new(),
            bio_dob: String::new(),
            bio_birthplace: String::new(),
            bio_strategy: String::new(),
            bio_rank: String::new(),
            bio_branch: String::new(),
            bio_class_number: String::new(),
            bio_portrait_small: None,
            bio_portrait_large: None,
            campaign: String::new(),
            player_template_name: String::new(),
            portrait_movie_left_name: String::new(),
            portrait_movie_right_name: String::new(),
            image_defeated: None,
            image_victorious: None,
            str_defeated: String::new(),
            str_victorious: String::new(),
            selection_sound: String::new(),
            taunt_sound_1: String::new(),
            taunt_sound_2: String::new(),
            taunt_sound_3: String::new(),
            win_sound: String::new(),
            loss_sound: String::new(),
            preview_sound: String::new(),
            name_sound: String::new(),
        }
    }

    // Accessor methods matching C++ interface (ChallengeGenerals.h lines 65-96)

    pub fn is_starting_enabled(&self) -> bool {
        self.starts_enabled
    }

    pub fn set_starts_enabled(&mut self, enabled: bool) {
        self.starts_enabled = enabled;
    }

    pub fn bio_name(&self) -> &str {
        &self.bio_name
    }

    pub fn set_bio_name(&mut self, name: String) {
        self.bio_name = name;
    }

    pub fn bio_dob(&self) -> &str {
        &self.bio_dob
    }

    pub fn set_bio_dob(&mut self, dob: String) {
        self.bio_dob = dob;
    }

    pub fn bio_birthplace(&self) -> &str {
        &self.bio_birthplace
    }

    pub fn set_bio_birthplace(&mut self, birthplace: String) {
        self.bio_birthplace = birthplace;
    }

    pub fn bio_strategy(&self) -> &str {
        &self.bio_strategy
    }

    pub fn set_bio_strategy(&mut self, strategy: String) {
        self.bio_strategy = strategy;
    }

    pub fn bio_rank(&self) -> &str {
        &self.bio_rank
    }

    pub fn set_bio_rank(&mut self, rank: String) {
        self.bio_rank = rank;
    }

    pub fn bio_class_number(&self) -> &str {
        &self.bio_class_number
    }

    pub fn set_bio_class_number(&mut self, class_number: String) {
        self.bio_class_number = class_number;
    }

    pub fn bio_branch(&self) -> &str {
        &self.bio_branch
    }

    pub fn set_bio_branch(&mut self, branch: String) {
        self.bio_branch = branch;
    }

    pub fn bio_portrait_small(&self) -> Option<&str> {
        self.bio_portrait_small.as_deref()
    }

    pub fn set_bio_portrait_small(&mut self, portrait: Option<String>) {
        self.bio_portrait_small = portrait;
    }

    pub fn bio_portrait_large(&self) -> Option<&str> {
        self.bio_portrait_large.as_deref()
    }

    pub fn set_bio_portrait_large(&mut self, portrait: Option<String>) {
        self.bio_portrait_large = portrait;
    }

    pub fn portrait_movie_left_name(&self) -> &str {
        &self.portrait_movie_left_name
    }

    pub fn set_portrait_movie_left_name(&mut self, name: String) {
        self.portrait_movie_left_name = name;
    }

    pub fn portrait_movie_right_name(&self) -> &str {
        &self.portrait_movie_right_name
    }

    pub fn set_portrait_movie_right_name(&mut self, name: String) {
        self.portrait_movie_right_name = name;
    }

    pub fn campaign(&self) -> &str {
        &self.campaign
    }

    pub fn set_campaign(&mut self, campaign: String) {
        self.campaign = campaign;
    }

    pub fn player_template_name(&self) -> &str {
        &self.player_template_name
    }

    pub fn set_player_template_name(&mut self, name: String) {
        self.player_template_name = name;
    }

    pub fn image_defeated(&self) -> Option<&str> {
        self.image_defeated.as_deref()
    }

    pub fn set_image_defeated(&mut self, image: Option<String>) {
        self.image_defeated = image;
    }

    pub fn image_victorious(&self) -> Option<&str> {
        self.image_victorious.as_deref()
    }

    pub fn set_image_victorious(&mut self, image: Option<String>) {
        self.image_victorious = image;
    }

    pub fn string_defeated(&self) -> &str {
        &self.str_defeated
    }

    pub fn set_string_defeated(&mut self, s: String) {
        self.str_defeated = s;
    }

    pub fn string_victorious(&self) -> &str {
        &self.str_victorious
    }

    pub fn set_string_victorious(&mut self, s: String) {
        self.str_victorious = s;
    }

    pub fn selection_sound(&self) -> &str {
        &self.selection_sound
    }

    pub fn set_selection_sound(&mut self, sound: String) {
        self.selection_sound = sound;
    }

    /// Returns a random taunt sound from the three available
    /// Matches C++ getRandomTauntSound (ChallengeGenerals.h lines 84-91)
    pub fn random_taunt_sound(&self) -> &str {
        // C++: rand()%3; client stream inclusive [0, 2]
        match crate::GameClientRandomValue!(0, 2) {
            0 => &self.taunt_sound_1,
            1 => &self.taunt_sound_2,
            _ => &self.taunt_sound_3,
        }
    }

    pub fn taunt_sound_1(&self) -> &str {
        &self.taunt_sound_1
    }

    pub fn set_taunt_sound_1(&mut self, sound: String) {
        self.taunt_sound_1 = sound;
    }

    pub fn taunt_sound_2(&self) -> &str {
        &self.taunt_sound_2
    }

    pub fn set_taunt_sound_2(&mut self, sound: String) {
        self.taunt_sound_2 = sound;
    }

    pub fn taunt_sound_3(&self) -> &str {
        &self.taunt_sound_3
    }

    pub fn set_taunt_sound_3(&mut self, sound: String) {
        self.taunt_sound_3 = sound;
    }

    pub fn win_sound(&self) -> &str {
        &self.win_sound
    }

    pub fn set_win_sound(&mut self, sound: String) {
        self.win_sound = sound;
    }

    pub fn loss_sound(&self) -> &str {
        &self.loss_sound
    }

    pub fn set_loss_sound(&mut self, sound: String) {
        self.loss_sound = sound;
    }

    pub fn preview_sound(&self) -> &str {
        &self.preview_sound
    }

    pub fn set_preview_sound(&mut self, sound: String) {
        self.preview_sound = sound;
    }

    pub fn name_sound(&self) -> &str {
        &self.name_sound
    }

    pub fn set_name_sound(&mut self, sound: String) {
        self.name_sound = sound;
    }
}

impl Default for GeneralPersona {
    fn default() -> Self {
        Self::new()
    }
}

/// Game difficulty levels
/// Matches C++ GameDifficulty enum (GameCommon.h lines 109-116)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
#[derive(Default)]
pub enum GameDifficulty {
    Easy = 0,
    #[default]
    Normal = 1,
    Hard = 2,
}

/// Manager for Challenge Generals data and operations
/// Matches C++ ChallengeGenerals class (ChallengeGenerals.h lines 99-128)
#[derive(Clone)]
pub struct ChallengeGenerals {
    /// Array of 12 general personas
    /// Matches C++ m_position (ChallengeGenerals.h line 103)
    positions: [GeneralPersona; NUM_GENERALS],

    /// Common could not provide a valid authored persona table.  Keep this
    /// explicit so the legacy GameClient projection cannot mistake blank
    /// positions for an all-unlocked table.
    persona_data_failed: bool,
}

impl ChallengeGenerals {
    fn sync_from_ini_store(&mut self) {
        let store = get_ini_challenge_generals();
        for index in 0..NUM_GENERALS {
            let src = &store.positions[index];
            let dst = &mut self.positions[index];

            dst.starts_enabled = src.starts_enabled;
            dst.bio_name = src.bio_name.clone();
            dst.bio_dob = src.bio_dob.clone();
            dst.bio_birthplace = src.bio_birthplace.clone();
            dst.bio_strategy = src.bio_strategy.clone();
            dst.bio_rank = src.bio_rank.clone();
            dst.bio_branch = src.bio_branch.clone();
            dst.bio_class_number = src.bio_class_number.clone();
            dst.bio_portrait_small = optional_ini_string(&src.bio_portrait_small);
            dst.bio_portrait_large = optional_ini_string(&src.bio_portrait_large);
            dst.campaign = src.campaign.clone();
            dst.player_template_name = src.player_template_name.clone();
            dst.portrait_movie_left_name = src.portrait_movie_left_name.clone();
            dst.portrait_movie_right_name = src.portrait_movie_right_name.clone();
            dst.image_defeated = optional_ini_string(&src.image_defeated);
            dst.image_victorious = optional_ini_string(&src.image_victorious);
            dst.str_defeated = src.string_defeated.clone();
            dst.str_victorious = src.string_victorious.clone();
            dst.selection_sound = src.selection_sound.clone();
            dst.taunt_sound_1 = src.taunt_sound1.clone();
            dst.taunt_sound_2 = src.taunt_sound2.clone();
            dst.taunt_sound_3 = src.taunt_sound3.clone();
            dst.win_sound = src.win_sound.clone();
            dst.loss_sound = src.loss_sound.clone();
            dst.preview_sound = src.preview_sound.clone();
            dst.name_sound = src.name_sound.clone();
        }
    }

    /// Creates a new ChallengeGenerals manager
    /// Matches C++ ChallengeGenerals constructor (ChallengeGenerals.cpp lines 24-27)
    pub fn new() -> Self {
        Self {
            positions: [
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
                GeneralPersona::new(),
            ],
            persona_data_failed: false,
        }
    }

    /// Initializes the UI projection from the Common-owned authored table.
    ///
    /// C++ has one `TheChallengeGenerals` singleton loaded through
    /// `Data\\INI\\ChallengeMode.ini`.  Main's headless authority and this
    /// UI projection must consume the same source, so GameClient never parses
    /// a second potentially divergent table here.
    pub fn init(&mut self) {
        if let Err(error) = ensure_challenge_generals_loaded() {
            // The Common store records this failure and Main's session
            // authority rejects all template selection while it remains
            // unresolved.  Keep GameClient alive enough to surface its normal
            // UI diagnostics instead of manufacturing unlocked persona data.
            log::warn!("ChallengeGenerals::init: {error}");
            self.positions = std::array::from_fn(|_| GeneralPersona::new());
            self.persona_data_failed = true;
            return;
        }

        self.persona_data_failed = false;
        self.sync_from_ini_store();
    }

    /// Returns a reference to all challenge generals
    /// Matches C++ getChallengeGenerals() (ChallengeGenerals.h line 114)
    pub fn challenge_generals(&self) -> &[GeneralPersona; NUM_GENERALS] {
        &self.positions
    }

    /// Returns a mutable reference to all challenge generals
    /// Matches C++ getChallengeGenerals() (ChallengeGenerals.h line 114)
    pub fn challenge_generals_mut(&mut self) -> &mut [GeneralPersona; NUM_GENERALS] {
        &mut self.positions
    }

    /// Finds a general by campaign name
    /// Matches C++ getPlayerGeneralByCampaignName (ChallengeGenerals.cpp lines 109-119)
    pub fn player_general_by_campaign_name(&self, name: &str) -> Option<&GeneralPersona> {
        for i in 0..NUM_GENERALS {
            if self.positions[i].campaign().eq_ignore_ascii_case(name) {
                return Some(&self.positions[i]);
            }
        }
        None
    }

    /// Finds a general by general name (bio name)
    /// Matches C++ getGeneralByGeneralName (ChallengeGenerals.cpp lines 121-131)
    pub fn general_by_general_name(&self, name: &str) -> Option<&GeneralPersona> {
        for i in 0..NUM_GENERALS {
            if self.positions[i].bio_name().eq_ignore_ascii_case(name) {
                return Some(&self.positions[i]);
            }
        }
        None
    }

    /// Finds a general by template name
    /// Matches C++ getGeneralByTemplateName (ChallengeGenerals.cpp lines 132-142)
    pub fn general_by_template_name(&self, name: &str) -> Option<&GeneralPersona> {
        if self.persona_data_failed {
            return Some(fail_closed_general_persona());
        }
        for i in 0..NUM_GENERALS {
            if self.positions[i]
                .player_template_name()
                .eq_ignore_ascii_case(name)
            {
                return Some(&self.positions[i]);
            }
        }
        None
    }
}

fn fail_closed_general_persona() -> &'static GeneralPersona {
    static FAIL_CLOSED_PERSONA: OnceLock<GeneralPersona> = OnceLock::new();
    FAIL_CLOSED_PERSONA.get_or_init(GeneralPersona::new)
}

impl Default for ChallengeGenerals {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton instance (matching C++ TheChallengeGenerals pattern)
/// In C++: extern ChallengeGenerals *TheChallengeGenerals (ChallengeGenerals.h line 133)
pub fn create_challenge_generals() -> ChallengeGenerals {
    // Matches C++ createChallengeGenerals (ChallengeGenerals.cpp lines 18-21)
    ChallengeGenerals::new()
}

/// Residual: last ChallengeGenerals action requested by residual peels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualChallengeGeneralsAction {
    None = 0,
    Init = 1,
    SetStartsEnabled = 2,
    SetBioName = 3,
    SetDifficulty = 4,
    SelectTemplate = 5,
}

pub fn residual_challenge_generals_starts_enabled(generals: &ChallengeGenerals) -> bool {
    generals.challenge_generals()[0].is_starting_enabled()
}
pub fn residual_challenge_generals_bio_name_len(generals: &ChallengeGenerals) -> usize {
    generals.challenge_generals()[0].bio_name().len()
}
pub fn residual_challenge_generals_difficulty(
    campaign: &crate::gui::campaign_manager::CampaignManager,
) -> u8 {
    match campaign.get_game_difficulty() {
        crate::gui::campaign_manager::GameDifficulty::Easy => 0,
        crate::gui::campaign_manager::GameDifficulty::Normal => 1,
        crate::gui::campaign_manager::GameDifficulty::Hard => 2,
    }
}
pub fn residual_challenge_generals_template_num(
    campaign: &crate::gui::campaign_manager::CampaignManager,
) -> i32 {
    campaign.get_xfer_challenge_generals_player_template_num()
}
pub fn simulate_challenge_generals_init(generals: &mut ChallengeGenerals) -> bool {
    generals.init();
    !generals.persona_data_failed
}
pub fn simulate_challenge_generals_set_starts_enabled(
    generals: &mut ChallengeGenerals,
    index: usize,
    enabled: bool,
) -> bool {
    let Some(persona) = generals.positions.get_mut(index) else {
        return false;
    };
    persona.set_starts_enabled(enabled);
    persona.is_starting_enabled() == enabled
}
pub fn simulate_challenge_generals_set_bio_name(
    generals: &mut ChallengeGenerals,
    index: usize,
    name: &str,
) -> bool {
    if name.is_empty() {
        return false;
    }
    let Some(persona) = generals.positions.get_mut(index) else {
        return false;
    };
    persona.set_bio_name(name.to_owned());
    persona.bio_name() == name
}
pub fn simulate_challenge_generals_set_difficulty(
    campaign: &mut crate::gui::campaign_manager::CampaignManager,
    difficulty: u8,
) -> bool {
    use crate::gui::campaign_manager::GameDifficulty;
    let difficulty = match difficulty {
        0 => GameDifficulty::Easy,
        2 => GameDifficulty::Hard,
        _ => GameDifficulty::Normal,
    };
    campaign.set_game_difficulty(difficulty);
    campaign.get_game_difficulty() == difficulty
}
pub fn simulate_challenge_generals_set_template_num(
    campaign: &mut crate::gui::campaign_manager::CampaignManager,
    template: i32,
) -> bool {
    campaign.set_xfer_challenge_generals_player_template_num(template);
    campaign.get_xfer_challenge_generals_player_template_num() == template
}
pub fn simulate_challenge_generals_prepare_default(
    generals: &mut ChallengeGenerals,
    campaign: &mut crate::gui::campaign_manager::CampaignManager,
) -> bool {
    simulate_challenge_generals_init(generals)
        && simulate_challenge_generals_set_starts_enabled(generals, 0, true)
        && simulate_challenge_generals_set_bio_name(generals, 0, "General Alexander")
        && simulate_challenge_generals_set_difficulty(campaign, 2)
        && simulate_challenge_generals_set_template_num(campaign, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general_persona_creation() {
        let persona = GeneralPersona::new();
        assert!(!persona.is_starting_enabled());
        assert_eq!(persona.bio_name(), "");
        assert_eq!(persona.campaign(), "");
    }

    #[test]
    fn test_general_persona_setters() {
        let mut persona = GeneralPersona::new();
        persona.set_bio_name("General Tao".to_string());
        persona.set_campaign("GLA_01".to_string());

        assert_eq!(persona.bio_name(), "General Tao");
        assert_eq!(persona.campaign(), "GLA_01");
    }

    #[test]
    fn test_challenge_generals_creation() {
        let generals = ChallengeGenerals::new();
        assert_eq!(generals.positions.len(), NUM_GENERALS);
        assert_eq!(
            crate::gui::campaign_manager::CampaignManager::new().get_game_difficulty(),
            crate::gui::campaign_manager::GameDifficulty::Normal
        );
    }

    #[test]
    fn test_find_general_by_campaign() {
        let mut generals = ChallengeGenerals::new();
        generals.positions[0].set_campaign("TEST_CAMPAIGN".to_string());

        let found = generals.player_general_by_campaign_name("test_campaign");
        assert!(found.is_some());
    }

    #[test]
    fn test_find_general_by_name() {
        let mut generals = ChallengeGenerals::new();
        generals.positions[0].set_bio_name("General Test".to_string());

        let found = generals.general_by_general_name("GENERAL TEST");
        assert!(found.is_some());
    }

    #[test]
    fn test_find_general_by_template() {
        let mut generals = ChallengeGenerals::new();
        generals.positions[0].set_player_template_name("Template1".to_string());

        let found = generals.general_by_template_name("template1");
        assert!(found.is_some());
    }

    #[test]
    fn test_difficulty_setting() {
        let mut campaign = crate::gui::campaign_manager::CampaignManager::new();
        assert_eq!(residual_challenge_generals_difficulty(&campaign), 1);
        assert!(simulate_challenge_generals_set_difficulty(&mut campaign, 2));
        assert_eq!(residual_challenge_generals_difficulty(&campaign), 2);
        assert!(simulate_challenge_generals_set_difficulty(&mut campaign, 0));
        assert_eq!(residual_challenge_generals_difficulty(&campaign), 0);
    }

    #[test]
    fn game_client_projection_uses_the_common_locked_general_table() {
        let mut generals = ChallengeGenerals::new();
        generals.init();

        assert!(
            generals
                .general_by_template_name("FactionAmericaAirForceGeneral")
                .is_some_and(|persona| persona.is_starting_enabled())
        );
        assert!(
            generals
                .general_by_template_name("FactionBossGeneral")
                .is_some_and(|persona| !persona.is_starting_enabled())
        );

        let common = game_engine::common::ini::get_challenge_generals();
        assert!(
            common
                .get_general_by_template_name("FactionAmericaAirForceGeneral")
                .is_some_and(|persona| persona.is_starting_enabled())
        );
        assert!(
            common
                .get_general_by_template_name("FactionBossGeneral")
                .is_some_and(|persona| !persona.is_starting_enabled())
        );
    }

    #[test]
    fn failed_common_persona_projection_rejects_every_template() {
        let mut generals = ChallengeGenerals::new();
        generals.persona_data_failed = true;

        assert!(
            generals
                .general_by_template_name("FactionBossGeneral")
                .is_some_and(|persona| !persona.is_starting_enabled())
        );
        assert!(
            generals
                .general_by_template_name("FactionAmericaAirForceGeneral")
                .is_some_and(|persona| !persona.is_starting_enabled())
        );
    }
}
