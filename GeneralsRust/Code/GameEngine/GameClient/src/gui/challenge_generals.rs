// FILE: challenge_generals.rs
// Author: Ported from C++ by Claude, Original by Steve Copeland, 6/24/2003
// Desc: Manager for data pertaining to the Generals' Challenge personas and related GUI.
//
// C++ Reference: /GeneralsMD/Code/GameEngine/Include/GameClient/ChallengeGenerals.h
//                /GeneralsMD/Code/GameEngine/Source/GameClient/GUI/ChallengeGenerals.cpp

use game_engine::common::ini::{
    get_challenge_generals as get_ini_challenge_generals,
    get_challenge_generals_mut as get_ini_challenge_generals_mut,
    init_challenge_generals as init_ini_challenge_generals,
    ChallengeGenerals as IniChallengeGenerals, INILoadType, INI,
};
use std::collections::{BTreeSet, HashSet};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

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

fn push_challenge_ini_file(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if path.is_file() {
        let key = fs::canonicalize(&path).unwrap_or(path.clone());
        if seen.insert(key) {
            files.push(path);
        }
    }
}

fn discover_challenge_mode_ini_files() -> Vec<PathBuf> {
    let mut roots = BTreeSet::new();

    if let Ok(cwd) = env::current_dir() {
        for ancestor in cwd.ancestors() {
            roots.insert(ancestor.to_path_buf());
        }
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            for ancestor in parent.ancestors() {
                roots.insert(ancestor.to_path_buf());
            }
        }
    }

    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for root in roots {
        push_challenge_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/ChallengeMode.ini"),
        );
        push_challenge_ini_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/Default/ChallengeMode.ini"),
        );

        for extracted in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            push_challenge_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/ChallengeMode.ini"),
            );
            push_challenge_ini_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/Default/ChallengeMode.ini"),
            );
        }
    }

    files
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
        use rand::Rng;
        let mut rng = rand::thread_rng();
        match rng.gen_range(0..3) {
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
pub struct ChallengeGenerals {
    /// Array of 12 general personas
    /// Matches C++ m_position (ChallengeGenerals.h line 103)
    positions: [GeneralPersona; NUM_GENERALS],

    /// Currently selected player template number
    /// Matches C++ m_PlayerTemplateNum (ChallengeGenerals.h line 104)
    player_template_num: i32,

    /// Current difficulty setting for challenge mode
    /// Matches C++ m_currentDifficulty (ChallengeGenerals.h line 105)
    current_difficulty: GameDifficulty,
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
            player_template_num: 0,
            current_difficulty: GameDifficulty::Normal,
        }
    }

    /// Initializes the challenge generals from INI file
    /// Matches C++ init() (ChallengeGenerals.cpp lines 36-40)
    ///
    /// In the C++ version, this loads from Data\\INI\\ChallengeMode.ini
    pub fn init(&mut self) {
        init_ini_challenge_generals();
        {
            let mut store = get_ini_challenge_generals_mut();
            *store = IniChallengeGenerals::new();
        }

        let sources = discover_challenge_mode_ini_files();
        if sources.is_empty() {
            log::warn!("ChallengeGenerals::init: no ChallengeMode.ini sources found");
            return;
        }

        let mut ini = INI::new();
        for (idx, source) in sources.iter().enumerate() {
            let load_type = if idx == 0 {
                INILoadType::Overwrite
            } else {
                INILoadType::MultiFile
            };
            if let Err(err) = ini.load(source, load_type) {
                log::warn!(
                    "ChallengeGenerals::init: failed to load '{}': {}",
                    source.display(),
                    err
                );
            }
        }

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

    /// Sets the current player template number
    /// Matches C++ setCurrentPlayerTemplateNum (ChallengeGenerals.h line 120)
    pub fn set_current_player_template_num(&mut self, template_num: i32) {
        self.player_template_num = template_num;
    }

    /// Gets the current player template number
    /// Matches C++ getCurrentPlayerTemplateNum (ChallengeGenerals.h line 121)
    pub fn current_player_template_num(&self) -> i32 {
        self.player_template_num
    }

    /// Sets the current difficulty
    /// Matches C++ setCurrentDifficulty (ChallengeGenerals.h line 123)
    pub fn set_current_difficulty(&mut self, difficulty: GameDifficulty) {
        self.current_difficulty = difficulty;
    }

    /// Gets the current difficulty
    /// Matches C++ getCurrentDifficulty (ChallengeGenerals.h line 124)
    pub fn current_difficulty(&self) -> GameDifficulty {
        self.current_difficulty
    }
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

/// Global challenge generals instance (mimics C++ singleton pattern)
static THE_CHALLENGE_GENERALS: OnceLock<Mutex<ChallengeGenerals>> = OnceLock::new();

/// Initialize the global challenge generals instance
/// Matches C++ TheChallengeGenerals initialization
pub fn init_challenge_generals() {
    let generals = THE_CHALLENGE_GENERALS.get_or_init(|| Mutex::new(ChallengeGenerals::new()));
    generals.lock().unwrap_or_else(|e| e.into_inner()).init();
}

/// Get immutable reference to global challenge generals
/// Matches C++ TheChallengeGenerals access
pub fn get_challenge_generals() -> Option<&'static Mutex<ChallengeGenerals>> {
    THE_CHALLENGE_GENERALS.get()
}

/// Get mutable reference to global challenge generals
/// Matches C++ TheChallengeGenerals access
pub fn get_challenge_generals_mut() -> Option<std::sync::MutexGuard<'static, ChallengeGenerals>> {
    THE_CHALLENGE_GENERALS.get().and_then(|m| m.lock().ok())
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
        assert_eq!(generals.current_difficulty(), GameDifficulty::Normal);
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
        let mut generals = ChallengeGenerals::new();
        assert_eq!(generals.current_difficulty(), GameDifficulty::Normal);

        generals.set_current_difficulty(GameDifficulty::Hard);
        assert_eq!(generals.current_difficulty(), GameDifficulty::Hard);

        generals.set_current_difficulty(GameDifficulty::Easy);
        assert_eq!(generals.current_difficulty(), GameDifficulty::Easy);
    }
}
