//! FILE: ini_challenge_generals.rs
//! Ported from: ChallengeGenerals.cpp (INI parsing section)
//! Original Author: Steve Copeland
//! Rust port: 2025
//!
//! Purpose: INI parsing for ChallengeGenerals and GeneralPersona definitions
//! Used for the Generals' Challenge mode personas and related GUI data.

use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

// Constants matching C++ definitions
pub const NUM_GENERALS: usize = 12;

/// GeneralPersona data structure matching C++ GeneralPersona class
///
/// Contains data for each General's Challenge persona including
/// bio information, portraits, campaign links, and audio cues.
#[derive(Debug, Clone, Default)]
pub struct GeneralPersona {
    /// Whether this general is enabled at game start
    pub starts_enabled: bool,
    /// General's display name string key
    pub bio_name: String,
    /// Date of birth string key
    pub bio_dob: String,
    /// Birthplace string key
    pub bio_birthplace: String,
    /// Strategy description string key
    pub bio_strategy: String,
    /// Rank string key
    pub bio_rank: String,
    /// Branch string key
    pub bio_branch: String,
    /// Class number string key
    pub bio_class_number: String,
    /// Small portrait image name
    pub bio_portrait_small: String,
    /// Large portrait image name
    pub bio_portrait_large: String,
    /// Associated campaign name
    pub campaign: String,
    /// Player template name for this general
    pub player_template_name: String,
    /// Left portrait movie filename
    pub portrait_movie_left_name: String,
    /// Right portrait movie filename
    pub portrait_movie_right_name: String,
    /// Defeated state image name
    pub image_defeated: String,
    /// Victorious state image name
    pub image_victorious: String,
    /// Defeated string key
    pub string_defeated: String,
    /// Victorious string key
    pub string_victorious: String,
    /// Selection sound name
    pub selection_sound: String,
    /// Taunt sound 1 name
    pub taunt_sound1: String,
    /// Taunt sound 2 name
    pub taunt_sound2: String,
    /// Taunt sound 3 name
    pub taunt_sound3: String,
    /// Win sound name
    pub win_sound: String,
    /// Loss sound name
    pub loss_sound: String,
    /// Preview sound name
    pub preview_sound: String,
    /// Name announcement sound
    pub name_sound: String,
}

impl GeneralPersona {
    /// Create a new GeneralPersona with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if this general starts enabled
    pub fn is_starting_enabled(&self) -> bool {
        self.starts_enabled
    }

    /// Get bio name string key
    pub fn get_bio_name(&self) -> &str {
        &self.bio_name
    }

    /// Get date of birth string key
    pub fn get_bio_dob(&self) -> &str {
        &self.bio_dob
    }

    /// Get birthplace string key
    pub fn get_bio_birthplace(&self) -> &str {
        &self.bio_birthplace
    }

    /// Get strategy string key
    pub fn get_bio_strategy(&self) -> &str {
        &self.bio_strategy
    }

    /// Get rank string key
    pub fn get_bio_rank(&self) -> &str {
        &self.bio_rank
    }

    /// Get branch string key
    pub fn get_bio_branch(&self) -> &str {
        &self.bio_branch
    }

    /// Get class number string key
    pub fn get_bio_class_number(&self) -> &str {
        &self.bio_class_number
    }

    /// Get small portrait image name
    pub fn get_bio_portrait_small(&self) -> &str {
        &self.bio_portrait_small
    }

    /// Get large portrait image name
    pub fn get_bio_portrait_large(&self) -> &str {
        &self.bio_portrait_large
    }

    /// Get campaign name
    pub fn get_campaign(&self) -> &str {
        &self.campaign
    }

    /// Get player template name
    pub fn get_player_template_name(&self) -> &str {
        &self.player_template_name
    }

    /// Get left portrait movie name
    pub fn get_portrait_movie_left_name(&self) -> &str {
        &self.portrait_movie_left_name
    }

    /// Get right portrait movie name
    pub fn get_portrait_movie_right_name(&self) -> &str {
        &self.portrait_movie_right_name
    }

    /// Get defeated image name
    pub fn get_image_defeated(&self) -> &str {
        &self.image_defeated
    }

    /// Get victorious image name
    pub fn get_image_victorious(&self) -> &str {
        &self.image_victorious
    }

    /// Get defeated string key
    pub fn get_string_defeated(&self) -> &str {
        &self.string_defeated
    }

    /// Get victorious string key
    pub fn get_string_victorious(&self) -> &str {
        &self.string_victorious
    }

    /// Get selection sound name
    pub fn get_selection_sound(&self) -> &str {
        &self.selection_sound
    }

    /// Get a random taunt sound name
    /// Matches C++ getRandomTauntSound behavior
    pub fn get_random_taunt_sound(&self) -> &str {
        // Use simple deterministic rotation instead of rand()
        // The C++ implementation uses rand()%3 which is meant to be simple
        // In practice, callers should handle actual randomization
        let index = (std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as usize)
            .unwrap_or(0)) % 3;

        match index {
            0 => &self.taunt_sound1,
            1 => &self.taunt_sound2,
            _ => &self.taunt_sound3,
        }
    }

    /// Get taunt sound 1
    pub fn get_taunt_sound1(&self) -> &str {
        &self.taunt_sound1
    }

    /// Get taunt sound 2
    pub fn get_taunt_sound2(&self) -> &str {
        &self.taunt_sound2
    }

    /// Get taunt sound 3
    pub fn get_taunt_sound3(&self) -> &str {
        &self.taunt_sound3
    }

    /// Get win sound name
    pub fn get_win_sound(&self) -> &str {
        &self.win_sound
    }

    /// Get loss sound name
    pub fn get_loss_sound(&self) -> &str {
        &self.loss_sound
    }

    /// Get preview sound name
    pub fn get_preview_sound(&self) -> &str {
        &self.preview_sound
    }

    /// Get name sound
    pub fn get_name_sound(&self) -> &str {
        &self.name_sound
    }
}

// ============================================================================
// GeneralPersona field parsing functions
// ============================================================================

fn parse_starts_enabled(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.starts_enabled = INI::parse_bool(token)?;
    Ok(())
}

fn parse_bio_name(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_dob(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_dob = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_birthplace(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_birthplace = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_strategy(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_strategy = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_rank(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_rank = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_branch(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_branch = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_class_number(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_class_number = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_portrait_small(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    // C++ uses INI::parseMappedImage which returns Image*, we store the name
    persona.bio_portrait_small = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_portrait_large(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_portrait_large = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_campaign(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.campaign = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_player_template(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.player_template_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_portrait_movie_left(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.portrait_movie_left_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_portrait_movie_right(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.portrait_movie_right_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_defeated_image(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.image_defeated = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_victorious_image(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.image_victorious = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_defeated_string(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.string_defeated = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_victorious_string(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.string_victorious = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_selection_sound(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.selection_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_taunt_sound1(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.taunt_sound1 = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_taunt_sound2(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.taunt_sound2 = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_taunt_sound3(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.taunt_sound3 = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_win_sound(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.win_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_loss_sound(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.loss_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_preview_sound(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.preview_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_name_sound(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.name_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

/// GeneralPersona field parse table matching C++ ChallengeGenerals::parseGeneralPersona
const GENERAL_PERSONA_FIELD_PARSE_TABLE: &[FieldParse<GeneralPersona>] = &[
    FieldParse { token: "StartsEnabled", parse: parse_starts_enabled },
    FieldParse { token: "BioNameString", parse: parse_bio_name },
    FieldParse { token: "BioDOBString", parse: parse_bio_dob },
    FieldParse { token: "BioBirthplaceString", parse: parse_bio_birthplace },
    FieldParse { token: "BioStrategyString", parse: parse_bio_strategy },
    FieldParse { token: "BioRankString", parse: parse_bio_rank },
    FieldParse { token: "BioBranchString", parse: parse_bio_branch },
    FieldParse { token: "BioClassNumberString", parse: parse_bio_class_number },
    FieldParse { token: "BioPortraitSmall", parse: parse_bio_portrait_small },
    FieldParse { token: "BioPortraitLarge", parse: parse_bio_portrait_large },
    FieldParse { token: "Campaign", parse: parse_campaign },
    FieldParse { token: "PlayerTemplate", parse: parse_player_template },
    FieldParse { token: "PortraitMovieLeftName", parse: parse_portrait_movie_left },
    FieldParse { token: "PortraitMovieRightName", parse: parse_portrait_movie_right },
    FieldParse { token: "DefeatedImage", parse: parse_defeated_image },
    FieldParse { token: "VictoriousImage", parse: parse_victorious_image },
    FieldParse { token: "DefeatedString", parse: parse_defeated_string },
    FieldParse { token: "VictoriousString", parse: parse_victorious_string },
    FieldParse { token: "SelectionSound", parse: parse_selection_sound },
    FieldParse { token: "TauntSound1", parse: parse_taunt_sound1 },
    FieldParse { token: "TauntSound2", parse: parse_taunt_sound2 },
    FieldParse { token: "TauntSound3", parse: parse_taunt_sound3 },
    FieldParse { token: "WinSound", parse: parse_win_sound },
    FieldParse { token: "LossSound", parse: parse_loss_sound },
    FieldParse { token: "PreviewSound", parse: parse_preview_sound },
    FieldParse { token: "NameSound", parse: parse_name_sound },
];

/// ChallengeGenerals data structure matching C++ ChallengeGenerals class
///
/// Manages all General persona data for the Challenge mode.
/// Contains an array of NUM_GENERALS (12) GeneralPersona entries.
#[derive(Debug, Clone)]
pub struct ChallengeGenerals {
    /// Array of general personas indexed by position (0-11)
    pub positions: [GeneralPersona; NUM_GENERALS],
    /// Current player template number (for UI state)
    pub player_template_num: i32,
    /// Current game difficulty selection
    pub current_difficulty: i32,
}

impl Default for ChallengeGenerals {
    fn default() -> Self {
        Self::new()
    }
}

impl ChallengeGenerals {
    /// Create a new ChallengeGenerals instance
    pub fn new() -> Self {
        Self {
            positions: Default::default(),
            player_template_num: 0,
            current_difficulty: 0, // EASY
        }
    }

    /// Get the array of general personas
    pub fn get_challenge_generals(&self) -> &[GeneralPersona] {
        &self.positions
    }

    /// Get a specific general by index
    pub fn get_general(&self, index: usize) -> Option<&GeneralPersona> {
        self.positions.get(index)
    }

    /// Get a mutable general by index
    pub fn get_general_mut(&mut self, index: usize) -> Option<&mut GeneralPersona> {
        self.positions.get_mut(index)
    }

    /// Find a general by campaign name (case-insensitive)
    /// Matches C++ getPlayerGeneralByCampaignName
    pub fn get_player_general_by_campaign_name(&self, name: &str) -> Option<&GeneralPersona> {
        let name_lower = name.to_lowercase();
        self.positions.iter().find(|p| p.campaign.to_lowercase() == name_lower)
    }

    /// Find a general by bio name (case-insensitive)
    /// Matches C++ getGeneralByGeneralName
    pub fn get_general_by_general_name(&self, name: &str) -> Option<&GeneralPersona> {
        let name_lower = name.to_lowercase();
        self.positions.iter().find(|p| p.bio_name.to_lowercase() == name_lower)
    }

    /// Find a general by player template name (case-insensitive)
    /// Matches C++ getGeneralByTemplateName
    pub fn get_general_by_template_name(&self, name: &str) -> Option<&GeneralPersona> {
        let name_lower = name.to_lowercase();
        self.positions.iter().find(|p| p.player_template_name.to_lowercase() == name_lower)
    }

    /// Set the current player template number
    pub fn set_current_player_template_num(&mut self, num: i32) {
        self.player_template_num = num;
    }

    /// Get the current player template number
    pub fn get_current_player_template_num(&self) -> i32 {
        self.player_template_num
    }

    /// Set the current difficulty
    pub fn set_current_difficulty(&mut self, difficulty: i32) {
        self.current_difficulty = difficulty;
    }

    /// Get the current difficulty
    pub fn get_current_difficulty(&self) -> i32 {
        self.current_difficulty
    }
}

// ============================================================================
// ChallengeGenerals field parsing functions
// ============================================================================

/// Helper to parse a GeneralPersona at a specific index
fn parse_general_persona_at(index: usize, ini: &mut INI, _tokens: &[&str]) -> INIResult<()> {
    let mut store = get_challenge_generals_mut();
    let persona = &mut store.positions[index];
    ini.init_from_ini_with_fields(persona, GENERAL_PERSONA_FIELD_PARSE_TABLE)?;
    Ok(())
}

fn parse_general_persona0(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[0], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona1(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[1], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona2(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[2], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona3(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[3], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona4(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[4], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona5(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[5], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona6(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[6], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona7(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[7], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona8(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[8], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona9(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[9], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona10(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[10], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona11(ini: &mut INI, store: &mut ChallengeGenerals, _tokens: &[&str]) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[11], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

/// ChallengeGenerals field parse table matching C++ ChallengeGenerals::s_fieldParseTable
const CHALLENGE_GENERALS_FIELD_PARSE_TABLE: &[FieldParse<ChallengeGenerals>] = &[
    FieldParse { token: "GeneralPersona0", parse: parse_general_persona0 },
    FieldParse { token: "GeneralPersona1", parse: parse_general_persona1 },
    FieldParse { token: "GeneralPersona2", parse: parse_general_persona2 },
    FieldParse { token: "GeneralPersona3", parse: parse_general_persona3 },
    FieldParse { token: "GeneralPersona4", parse: parse_general_persona4 },
    FieldParse { token: "GeneralPersona5", parse: parse_general_persona5 },
    FieldParse { token: "GeneralPersona6", parse: parse_general_persona6 },
    FieldParse { token: "GeneralPersona7", parse: parse_general_persona7 },
    FieldParse { token: "GeneralPersona8", parse: parse_general_persona8 },
    FieldParse { token: "GeneralPersona9", parse: parse_general_persona9 },
    FieldParse { token: "GeneralPersona10", parse: parse_general_persona10 },
    FieldParse { token: "GeneralPersona11", parse: parse_general_persona11 },
];

// ============================================================================
// Global ChallengeGenerals Store
// ============================================================================

static CHALLENGE_GENERALS: OnceCell<RwLock<ChallengeGenerals>> = OnceCell::new();

/// Get read access to the global ChallengeGenerals
pub fn get_challenge_generals() -> RwLockReadGuard<'static, ChallengeGenerals> {
    CHALLENGE_GENERALS
        .get_or_init(|| RwLock::new(ChallengeGenerals::new()))
        .read()
        .unwrap()
}

/// Get write access to the global ChallengeGenerals
pub fn get_challenge_generals_mut() -> RwLockWriteGuard<'static, ChallengeGenerals> {
    CHALLENGE_GENERALS
        .get_or_init(|| RwLock::new(ChallengeGenerals::new()))
        .write()
        .unwrap()
}

/// Initialize the global ChallengeGenerals store
pub fn init_challenge_generals() {
    let _unused = get_challenge_generals();
}

// ============================================================================
// INI Block Parser
// ============================================================================

/// Parse a ChallengeGenerals block from INI
/// Matches C++ INI::parseChallengeModeDefinition
///
/// The ChallengeGenerals block contains GeneralPersona0 through GeneralPersona11
/// sub-blocks that define each general's persona data.
pub fn parse_challenge_generals_definition(ini: &mut INI) -> INIResult<()> {
    // The ChallengeGenerals block doesn't have a name parameter like other blocks
    // It directly contains GeneralPersona0..11 sub-blocks

    let mut store = get_challenge_generals_mut();
    ini.init_from_ini_with_fields(&mut *store, CHALLENGE_GENERALS_FIELD_PARSE_TABLE)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_general_persona_creation() {
        let persona = GeneralPersona::new();
        assert!(!persona.starts_enabled);
        assert!(persona.bio_name.is_empty());
        assert!(persona.campaign.is_empty());
    }

    #[test]
    fn test_challenge_generals_creation() {
        let generals = ChallengeGenerals::new();
        assert_eq!(generals.positions.len(), NUM_GENERALS);
        assert_eq!(generals.player_template_num, 0);
    }

    #[test]
    fn test_challenge_generals_get_by_campaign() {
        let mut generals = ChallengeGenerals::new();
        generals.positions[0].campaign = "GLAChallenge".to_string();
        generals.positions[0].bio_name = "Dr. Thrax".to_string();

        let found = generals.get_player_general_by_campaign_name("GLAChallenge");
        assert!(found.is_some());
        assert_eq!(found.unwrap().bio_name, "Dr. Thrax");

        // Case insensitive
        let found_lower = generals.get_player_general_by_campaign_name("glachallenge");
        assert!(found_lower.is_some());
    }

    #[test]
    fn test_challenge_generals_get_by_template_name() {
        let mut generals = ChallengeGenerals::new();
        generals.positions[1].player_template_name = "FactionChinaTank".to_string();
        generals.positions[1].bio_name = "Tank General".to_string();

        let found = generals.get_general_by_template_name("FactionChinaTank");
        assert!(found.is_some());
        assert_eq!(found.unwrap().bio_name, "Tank General");

        let not_found = generals.get_general_by_template_name("NonExistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_challenge_generals_get_by_general_name() {
        let mut generals = ChallengeGenerals::new();
        generals.positions[2].bio_name = "Superweapon General".to_string();

        let found = generals.get_general_by_general_name("Superweapon General");
        assert!(found.is_some());

        // Case insensitive
        let found_lower = generals.get_general_by_general_name("superweapon general");
        assert!(found_lower.is_some());
    }

    #[test]
    fn test_global_challenge_generals() {
        init_challenge_generals();
        let generals = get_challenge_generals();
        assert_eq!(generals.positions.len(), NUM_GENERALS);
    }
}
