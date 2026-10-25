//! FILE: ini_challenge_generals.rs
//! Ported from: ChallengeGenerals.cpp (INI parsing section)
//! Original Author: Steve Copeland
//! Rust port: 2025
//!
//! Purpose: INI parsing for ChallengeGenerals and GeneralPersona definitions
//! Used for the Generals' Challenge mode personas and related GUI data.

use crate::common::ini::ini::{FieldParse, INI, INIError, INILoadType, INIResult};
use std::collections::HashSet;
use std::env;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

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
            .unwrap_or(0))
            % 3;

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

fn parse_starts_enabled(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
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

fn parse_bio_birthplace(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_birthplace = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_strategy(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_strategy = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_rank(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_rank = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_branch(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_branch = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_class_number(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_class_number = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_portrait_small(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    // C++ uses INI::parseMappedImage which returns Image*, we store the name
    persona.bio_portrait_small = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_bio_portrait_large(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.bio_portrait_large = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_campaign(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.campaign = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_player_template(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.player_template_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_portrait_movie_left(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.portrait_movie_left_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_portrait_movie_right(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.portrait_movie_right_name = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_defeated_image(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.image_defeated = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_victorious_image(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.image_victorious = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_defeated_string(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.string_defeated = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_victorious_string(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.string_victorious = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_selection_sound(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.selection_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_taunt_sound1(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.taunt_sound1 = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_taunt_sound2(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.taunt_sound2 = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_taunt_sound3(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.taunt_sound3 = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_win_sound(_ini: &mut INI, persona: &mut GeneralPersona, tokens: &[&str]) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.win_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_loss_sound(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.loss_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_preview_sound(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.preview_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

fn parse_name_sound(
    _ini: &mut INI,
    persona: &mut GeneralPersona,
    tokens: &[&str],
) -> INIResult<()> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    persona.name_sound = INI::parse_ascii_string(token)?;
    Ok(())
}

/// GeneralPersona field parse table matching C++ ChallengeGenerals::parseGeneralPersona
const GENERAL_PERSONA_FIELD_PARSE_TABLE: &[FieldParse<GeneralPersona>] = &[
    FieldParse {
        token: "StartsEnabled",
        parse: parse_starts_enabled,
    },
    FieldParse {
        token: "BioNameString",
        parse: parse_bio_name,
    },
    FieldParse {
        token: "BioDOBString",
        parse: parse_bio_dob,
    },
    FieldParse {
        token: "BioBirthplaceString",
        parse: parse_bio_birthplace,
    },
    FieldParse {
        token: "BioStrategyString",
        parse: parse_bio_strategy,
    },
    FieldParse {
        token: "BioRankString",
        parse: parse_bio_rank,
    },
    FieldParse {
        token: "BioBranchString",
        parse: parse_bio_branch,
    },
    FieldParse {
        token: "BioClassNumberString",
        parse: parse_bio_class_number,
    },
    FieldParse {
        token: "BioPortraitSmall",
        parse: parse_bio_portrait_small,
    },
    FieldParse {
        token: "BioPortraitLarge",
        parse: parse_bio_portrait_large,
    },
    FieldParse {
        token: "Campaign",
        parse: parse_campaign,
    },
    FieldParse {
        token: "PlayerTemplate",
        parse: parse_player_template,
    },
    FieldParse {
        token: "PortraitMovieLeftName",
        parse: parse_portrait_movie_left,
    },
    FieldParse {
        token: "PortraitMovieRightName",
        parse: parse_portrait_movie_right,
    },
    FieldParse {
        token: "DefeatedImage",
        parse: parse_defeated_image,
    },
    FieldParse {
        token: "VictoriousImage",
        parse: parse_victorious_image,
    },
    FieldParse {
        token: "DefeatedString",
        parse: parse_defeated_string,
    },
    FieldParse {
        token: "VictoriousString",
        parse: parse_victorious_string,
    },
    FieldParse {
        token: "SelectionSound",
        parse: parse_selection_sound,
    },
    FieldParse {
        token: "TauntSound1",
        parse: parse_taunt_sound1,
    },
    FieldParse {
        token: "TauntSound2",
        parse: parse_taunt_sound2,
    },
    FieldParse {
        token: "TauntSound3",
        parse: parse_taunt_sound3,
    },
    FieldParse {
        token: "WinSound",
        parse: parse_win_sound,
    },
    FieldParse {
        token: "LossSound",
        parse: parse_loss_sound,
    },
    FieldParse {
        token: "PreviewSound",
        parse: parse_preview_sound,
    },
    FieldParse {
        token: "NameSound",
        parse: parse_name_sound,
    },
];

/// ChallengeGenerals data structure matching C++ ChallengeGenerals class
///
/// Manages all General persona data for the Challenge mode.
/// Contains an array of NUM_GENERALS (12) GeneralPersona entries.
#[derive(Debug, Clone)]
pub struct ChallengeGenerals {
    /// Array of general personas indexed by position (0-11)
    pub positions: [GeneralPersona; NUM_GENERALS],
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
        self.positions
            .iter()
            .find(|p| p.campaign.to_lowercase() == name_lower)
    }

    /// Find a general by bio name (case-insensitive)
    /// Matches C++ getGeneralByGeneralName
    pub fn get_general_by_general_name(&self, name: &str) -> Option<&GeneralPersona> {
        let name_lower = name.to_lowercase();
        self.positions
            .iter()
            .find(|p| p.bio_name.to_lowercase() == name_lower)
    }

    /// Find a general by player template name (case-insensitive)
    /// Matches C++ getGeneralByTemplateName
    /// Find an authored persona by template name without consulting process
    /// load state. This remains a pure lookup for local parser/test fixtures.
    pub fn get_general_by_template_name(&self, name: &str) -> Option<&GeneralPersona> {
        self.positions
            .iter()
            .find(|p| p.player_template_name.eq_ignore_ascii_case(name))
    }
}

// ============================================================================
// ChallengeGenerals field parsing functions
// ============================================================================

fn parse_general_persona0(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[0], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona1(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[1], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona2(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[2], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona3(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[3], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona4(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[4], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona5(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[5], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona6(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[6], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona7(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[7], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona8(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[8], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona9(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[9], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona10(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[10], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

fn parse_general_persona11(
    ini: &mut INI,
    store: &mut ChallengeGenerals,
    _tokens: &[&str],
) -> INIResult<()> {
    ini.init_from_ini_with_fields(&mut store.positions[11], GENERAL_PERSONA_FIELD_PARSE_TABLE)
}

/// ChallengeGenerals field parse table matching C++ ChallengeGenerals::s_fieldParseTable
const CHALLENGE_GENERALS_FIELD_PARSE_TABLE: &[FieldParse<ChallengeGenerals>] = &[
    FieldParse {
        token: "GeneralPersona0",
        parse: parse_general_persona0,
    },
    FieldParse {
        token: "GeneralPersona1",
        parse: parse_general_persona1,
    },
    FieldParse {
        token: "GeneralPersona2",
        parse: parse_general_persona2,
    },
    FieldParse {
        token: "GeneralPersona3",
        parse: parse_general_persona3,
    },
    FieldParse {
        token: "GeneralPersona4",
        parse: parse_general_persona4,
    },
    FieldParse {
        token: "GeneralPersona5",
        parse: parse_general_persona5,
    },
    FieldParse {
        token: "GeneralPersona6",
        parse: parse_general_persona6,
    },
    FieldParse {
        token: "GeneralPersona7",
        parse: parse_general_persona7,
    },
    FieldParse {
        token: "GeneralPersona8",
        parse: parse_general_persona8,
    },
    FieldParse {
        token: "GeneralPersona9",
        parse: parse_general_persona9,
    },
    FieldParse {
        token: "GeneralPersona10",
        parse: parse_general_persona10,
    },
    FieldParse {
        token: "GeneralPersona11",
        parse: parse_general_persona11,
    },
];

// ============================================================================
// Immutable authored ChallengeGenerals definitions
// ============================================================================

/// The only authored ChallengeMode source the retail client asks the C++
/// `FileSystem` to resolve.  In particular, C++ does not separately load a
/// `Default/ChallengeMode.ini` layer.
const CHALLENGE_MODE_INI_PATH: &str = "Data/INI/ChallengeMode.ini";

/// Result of loading the authoritative ChallengeMode persona table.
///
/// `GameLogic::populateRandomSideAndColor` assumes `TheChallengeGenerals` was
/// initialized during GameClient startup. Rust also has headless/offline entry
/// points, so preserve that invariant explicitly instead of treating an empty
/// lazy store as authored data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChallengeGeneralsLoadError {
    SourceNotFound,
    ParseFailed { source: String, error: INIError },
    InvalidData { source: String, detail: String },
}

impl fmt::Display for ChallengeGeneralsLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceNotFound => write!(f, "could not resolve {CHALLENGE_MODE_INI_PATH}"),
            Self::ParseFailed { source, error } => {
                write!(
                    f,
                    "failed to parse ChallengeMode source '{source}': {error}"
                )
            }
            Self::InvalidData { source, detail } => {
                write!(f, "invalid ChallengeMode source '{source}': {detail}")
            }
        }
    }
}

impl std::error::Error for ChallengeGeneralsLoadError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChallengeGeneralsLoadStatus {
    Uninitialized,
    Loaded { source: String },
    Failed(ChallengeGeneralsLoadError),
}

struct LoadedChallengeGenerals {
    source: String,
    definitions: ChallengeGenerals,
}

static CHALLENGE_GENERALS: OnceLock<Result<LoadedChallengeGenerals, ChallengeGeneralsLoadError>> =
    OnceLock::new();

fn validate_challenge_generals_data(
    store: &ChallengeGenerals,
    source: &str,
) -> Result<(), ChallengeGeneralsLoadError> {
    let mut template_names = HashSet::new();

    // The retail file defines all twelve UI positions. Positions 0..=9 are
    // actual General records; positions 10/11 are disabled placeholders.
    for (index, persona) in store.positions.iter().enumerate() {
        if persona.bio_name.trim().is_empty() {
            return Err(ChallengeGeneralsLoadError::InvalidData {
                source: source.to_string(),
                detail: format!("GeneralPersona{index} is missing BioNameString"),
            });
        }

        let template_name = persona.player_template_name.trim();
        if index < 10 && template_name.is_empty() {
            return Err(ChallengeGeneralsLoadError::InvalidData {
                source: source.to_string(),
                detail: format!("GeneralPersona{index} is missing PlayerTemplate"),
            });
        }
        if persona.starts_enabled && template_name.is_empty() {
            return Err(ChallengeGeneralsLoadError::InvalidData {
                source: source.to_string(),
                detail: format!(
                    "enabled GeneralPersona{index} has no PlayerTemplate and cannot be selected"
                ),
            });
        }
        if !template_name.is_empty() && !template_names.insert(template_name.to_ascii_lowercase()) {
            return Err(ChallengeGeneralsLoadError::InvalidData {
                source: source.to_string(),
                detail: format!("duplicate PlayerTemplate '{template_name}'"),
            });
        }
    }

    Ok(())
}

fn push_unique_root(roots: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, root: PathBuf) {
    let key = fs::canonicalize(&root).unwrap_or_else(|_| root.clone());
    if seen.insert(key) {
        roots.push(root);
    }
}

fn add_root_and_ancestors(roots: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, root: PathBuf) {
    for ancestor in root.ancestors() {
        push_unique_root(roots, seen, ancestor.to_path_buf());
    }
}

/// A fallback only for hosts whose Common FileSystem has not been initialized
/// yet (notably focused/headless tests).  The normal path below is the C++
/// virtual `FileSystem` lookup, which keeps loose-file-before-archive
/// precedence intact.
fn discover_challenge_mode_ini_file() -> Option<PathBuf> {
    let mut roots = Vec::new();
    let mut seen_roots = HashSet::new();

    // A selected mod is the explicit local override.  The C++ loader still
    // asks for the same canonical virtual filename.
    let mod_dir = {
        let global_data = crate::common::global_data::read();
        global_data.writable.mod_dir.clone()
    };
    if !mod_dir.trim().is_empty() {
        add_root_and_ancestors(&mut roots, &mut seen_roots, PathBuf::from(mod_dir.trim()));
    }

    if let Ok(cwd) = env::current_dir() {
        add_root_and_ancestors(&mut roots, &mut seen_roots, cwd);
    }
    if let Ok(exe) = env::current_exe() {
        if let Some(parent) = exe.parent() {
            add_root_and_ancestors(&mut roots, &mut seen_roots, parent.to_path_buf());
        }
    }
    for extracted in crate::common::system::install_layout::extracted_asset_roots() {
        push_unique_root(&mut roots, &mut seen_roots, extracted);
    }

    let mut seen_files = HashSet::new();
    for root in roots {
        let candidates = [
            root.join(CHALLENGE_MODE_INI_PATH),
            root.join("windows_game/extracted_big_files/INIZH")
                .join(CHALLENGE_MODE_INI_PATH),
            root.join("windows_game/extracted_big_files_v2/INIZH")
                .join(CHALLENGE_MODE_INI_PATH),
        ];
        for path in candidates {
            if !path.is_file() {
                continue;
            }
            let key = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
            if seen_files.insert(key) {
                return Some(path);
            }
        }
    }
    None
}

fn load_challenge_generals_once() -> Result<LoadedChallengeGenerals, ChallengeGeneralsLoadError> {
    // Preserve C++ FileSystem precedence: the canonical virtual filename is
    // attempted before a physical fallback is discovered for headless hosts.
    let mut ini = INI::new();
    ini.set_challenge_generals_target(ChallengeGenerals::new());
    let source = match ini.load(CHALLENGE_MODE_INI_PATH, INILoadType::Overwrite) {
        Ok(()) => CHALLENGE_MODE_INI_PATH.to_string(),
        Err(INIError::CantOpenFile) => {
            let source = discover_challenge_mode_ini_file()
                .ok_or(ChallengeGeneralsLoadError::SourceNotFound)?;
            let mut fallback_ini = INI::new();
            fallback_ini.set_challenge_generals_target(ChallengeGenerals::new());
            fallback_ini
                .load(&source, INILoadType::Overwrite)
                .map_err(|error| ChallengeGeneralsLoadError::ParseFailed {
                    source: source.display().to_string(),
                    error,
                })?;
            let source = source.display().to_string();
            let definitions = fallback_ini
                .take_challenge_generals_target()
                .expect("ChallengeMode parser target was installed");
            validate_challenge_generals_data(&definitions, &source)?;
            return Ok(LoadedChallengeGenerals {
                source,
                definitions,
            });
        }
        Err(error) => {
            return Err(ChallengeGeneralsLoadError::ParseFailed {
                source: CHALLENGE_MODE_INI_PATH.to_string(),
                error,
            });
        }
    };

    let definitions = ini
        .take_challenge_generals_target()
        .expect("ChallengeMode parser target was installed");
    validate_challenge_generals_data(&definitions, &source)?;
    Ok(LoadedChallengeGenerals {
        source,
        definitions,
    })
}

pub fn ensure_challenge_generals_loaded() -> Result<(), ChallengeGeneralsLoadError> {
    CHALLENGE_GENERALS
        .get_or_init(load_challenge_generals_once)
        .as_ref()
        .map(|_| ())
        .map_err(Clone::clone)
}

pub fn challenge_generals_load_status() -> ChallengeGeneralsLoadStatus {
    match CHALLENGE_GENERALS.get() {
        None => ChallengeGeneralsLoadStatus::Uninitialized,
        Some(Ok(loaded)) => ChallengeGeneralsLoadStatus::Loaded {
            source: loaded.source.clone(),
        },
        Some(Err(error)) => ChallengeGeneralsLoadStatus::Failed(error.clone()),
    }
}

fn challenge_generals_load_failed() -> bool {
    matches!(CHALLENGE_GENERALS.get(), Some(Err(_)))
}

fn fail_closed_general_persona() -> &'static GeneralPersona {
    static FAIL_CLOSED_PERSONA: OnceLock<GeneralPersona> = OnceLock::new();
    FAIL_CLOSED_PERSONA.get_or_init(GeneralPersona::new)
}

/// Get the immutable authored persona definitions. The returned reference is
/// stable for the process lifetime; selection and difficulty belong to the
/// receiving campaign/UI state.
pub fn get_challenge_generals() -> &'static ChallengeGenerals {
    let _ = ensure_challenge_generals_loaded();
    match CHALLENGE_GENERALS.get().expect("loader initialized") {
        Ok(loaded) => &loaded.definitions,
        Err(_) => {
            static EMPTY: OnceLock<ChallengeGenerals> = OnceLock::new();
            EMPTY.get_or_init(ChallengeGenerals::new)
        }
    }
}

/// Resolve a template against the authoritative authored table, preserving
/// C++'s fail-closed handling for Challenge personas when the INI is missing.
pub fn get_authored_general_by_template_name(name: &str) -> Option<&'static GeneralPersona> {
    let definitions = get_challenge_generals();
    if challenge_generals_load_failed() {
        return name
            .to_ascii_lowercase()
            .contains("general")
            .then(fail_closed_general_persona);
    }
    definitions.get_general_by_template_name(name)
}

/// Initialize the immutable authored definitions for existing startup callers.
pub fn init_challenge_generals() {
    if let Err(error) = ensure_challenge_generals_loaded() {
        log::warn!(
            "ChallengeMode persona initialization failed; template eligibility is fail-closed: {error}"
        );
    }
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

    let mut store = ini
        .take_challenge_generals_target()
        .unwrap_or_else(ChallengeGenerals::new);
    let result = ini.init_from_ini_with_fields(&mut store, CHALLENGE_GENERALS_FIELD_PARSE_TABLE);
    ini.set_challenge_generals_target(store);
    result
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
        ensure_challenge_generals_loaded().expect("retail ChallengeMode.ini must load");
        assert!(matches!(
            challenge_generals_load_status(),
            ChallengeGeneralsLoadStatus::Loaded { .. }
        ));
        let generals = get_challenge_generals();
        assert_eq!(generals.positions.len(), NUM_GENERALS);

        // `ChallengeMode.ini` deliberately includes the playable Boss
        // PlayerTemplate but marks it locked.  This is the exact record that
        // GameLogic/GUIUtil exclude from Random and direct selection.
        assert!(
            generals
                .get_general_by_template_name("FactionAmericaAirForceGeneral")
                .is_some_and(|persona| persona.is_starting_enabled())
        );
        assert!(
            generals
                .get_general_by_template_name("FactionBossGeneral")
                .is_some_and(|persona| !persona.is_starting_enabled())
        );
    }

    #[test]
    fn malformed_challenge_mode_without_the_locked_boss_record_is_rejected() {
        let mut generals = ChallengeGenerals::new();
        for (index, persona) in generals.positions.iter_mut().enumerate() {
            persona.bio_name = format!("GUI:BioNameEntry_Pos{index}");
            if index < 9 {
                persona.player_template_name = format!("FactionGeneral{index}");
            }
        }

        let error = validate_challenge_generals_data(&generals, "test ChallengeMode.ini")
            .expect_err("a missing GeneralPersona9 PlayerTemplate must fail closed");
        assert!(matches!(
            error,
            ChallengeGeneralsLoadError::InvalidData { detail, .. }
                if detail.contains("GeneralPersona9") && detail.contains("PlayerTemplate")
        ));
    }

    #[test]
    fn test_challenge_mode_equals_separator_forms() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let path = std::env::temp_dir().join(format!("challenge_mode_equals_{unique}.ini"));
        std::fs::write(
            &path,
            r#"
ChallengeGenerals
  GeneralPersona0
    PlayerTemplate=FactionAmericaAirForceGeneral
    StartsEnabled=yes
    TauntSound1 =Taunts_Grainger061
    Campaign = CHALLENGE_0
  END
END
"#,
        )
        .unwrap();

        let mut ini = INI::new();
        ini.set_challenge_generals_target(ChallengeGenerals::new());
        let result = ini.load(&path, crate::common::ini::ini::INILoadType::Overwrite);
        let _ = std::fs::remove_file(&path);
        result.unwrap();
        let generals = ini.take_challenge_generals_target().expect("parser target");
        let persona = &generals.positions[0];
        assert_eq!(
            persona.player_template_name,
            "FactionAmericaAirForceGeneral"
        );
        assert!(persona.starts_enabled);
        assert_eq!(persona.taunt_sound1, "Taunts_Grainger061");
        assert_eq!(persona.campaign, "CHALLENGE_0");
    }
}
