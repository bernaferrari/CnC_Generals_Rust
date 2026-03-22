////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_eva_event.rs
//! Author: John K. McDonald, Jr. (Converted to Rust)
//! Desc: EvaEvent block parser for EVA (Electronic Video Agent) event handling
//!
//! Matches C++ Eva.cpp and Eva.h from:
//! - GeneralsMD/Code/GameEngine/Source/GameClient/Eva.cpp
//! - GeneralsMD/Code/GameEngine/Include/GameClient/Eva.h
//!
//! # C++ Field Parse Tables
//!
//! EvaSideSounds (Eva.cpp lines 61-66):
//! ```cpp
//! const FieldParse EvaSideSounds::s_evaSideSounds[] =
//! {
//!     { "Side",    INI::parseAsciiString, NULL, offsetof(EvaSideSounds, m_side) },
//!     { "Sounds",  INI::parseSoundsList,  NULL, offsetof(EvaSideSounds, m_soundNames) },
//!     { 0, 0, 0, 0 },
//! };
//! ```
//!
//! EvaCheckInfo (Eva.cpp lines 83-89):
//! ```cpp
//! const FieldParse EvaCheckInfo::s_evaEventInfo[] =
//! {
//!     { "Priority",           INI::parseUnsignedInt,          NULL, offsetof(EvaCheckInfo, m_priority) },
//!     { "TimeBetweenChecksMS",INI::parseDurationUnsignedInt,  NULL, offsetof(EvaCheckInfo, m_framesBetweenChecks) },
//!     { "ExpirationTimeMS",   INI::parseDurationUnsignedInt,  NULL, offsetof(EvaCheckInfo, m_framesToExpire) },
//!     { "SideSounds",         parseSideSoundsList,            NULL, offsetof(EvaCheckInfo, m_evaSideSounds) },
//!     { 0, 0, 0, 0 },
//! };
//! ```

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ini::{INIError, INIResult, INI};

/// EVA message enumeration
/// Matches C++ EvaMessage enum from Eva.h lines 29-80
///
/// These are the various EVA announcements that can be played during gameplay.
/// Each message has specific conditions under which it triggers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum EvaMessage {
    Invalid = -1,
    LowPower = 0,
    InsufficientFunds = 1,
    SuperweaponDetectedOwnParticleCannon = 2,
    SuperweaponDetectedOwnNuke = 3,
    SuperweaponDetectedOwnScudStorm = 4,
    SuperweaponDetectedAllyParticleCannon = 5,
    SuperweaponDetectedAllyNuke = 6,
    SuperweaponDetectedAllyScudStorm = 7,
    SuperweaponDetectedEnemyParticleCannon = 8,
    SuperweaponDetectedEnemyNuke = 9,
    SuperweaponDetectedEnemyScudStorm = 10,
    SuperweaponLaunchedOwnParticleCannon = 11,
    SuperweaponLaunchedOwnNuke = 12,
    SuperweaponLaunchedOwnScudStorm = 13,
    SuperweaponLaunchedAllyParticleCannon = 14,
    SuperweaponLaunchedAllyNuke = 15,
    SuperweaponLaunchedAllyScudStorm = 16,
    SuperweaponLaunchedEnemyParticleCannon = 17,
    SuperweaponLaunchedEnemyNuke = 18,
    SuperweaponLaunchedEnemyScudStorm = 19,
    SuperweaponReadyOwnParticleCannon = 20,
    SuperweaponReadyOwnNuke = 21,
    SuperweaponReadyOwnScudStorm = 22,
    SuperweaponReadyAllyParticleCannon = 23,
    SuperweaponReadyAllyNuke = 24,
    SuperweaponReadyAllyScudStorm = 25,
    SuperweaponReadyEnemyParticleCannon = 26,
    SuperweaponReadyEnemyNuke = 27,
    SuperweaponReadyEnemyScudStorm = 28,
    BuildingLost = 29,
    BaseUnderAttack = 30,
    AllyUnderAttack = 31,
    BeaconDetected = 32,
    EnemyBlackLotusDetected = 33,
    EnemyJarmenKellDetected = 34,
    EnemyColonelBurtonDetected = 35,
    OwnBlackLotusDetected = 36,
    OwnJarmenKellDetected = 37,
    OwnColonelBurtonDetected = 38,
    UnitLost = 39,
    GeneralLevelUp = 40,
    VehicleStolen = 41,
    BuildingStolen = 42,
    CashStolen = 43,
    UpgradeComplete = 44,
    BuildingBeingStolen = 45,
    BuildingSabotaged = 46,
    SuperweaponLaunchedOwnGpsScrambler = 47,
    SuperweaponLaunchedAllyGpsScrambler = 48,
    SuperweaponLaunchedEnemyGpsScrambler = 49,
    SuperweaponLaunchedOwnSneakAttack = 50,
    SuperweaponLaunchedAllySneakAttack = 51,
    SuperweaponLaunchedEnemySneakAttack = 52,
    Count = 53,
}

impl Default for EvaMessage {
    fn default() -> Self {
        EvaMessage::Invalid
    }
}

impl EvaMessage {
    /// Convert message name to enum value
    /// Matches C++ Eva::nameToMessage() from Eva.cpp lines 147-161
    pub fn from_name(name: &str) -> Self {
        // Must match TheEvaMessageNames array from Eva.cpp lines 16-71
        match name.to_ascii_uppercase().as_str() {
            "LOWPOWER" => EvaMessage::LowPower,
            "INSUFFICIENTFUNDS" => EvaMessage::InsufficientFunds,
            "SUPERWEAPONDETECTED_OWN_PARTICLECANNON" => {
                EvaMessage::SuperweaponDetectedOwnParticleCannon
            }
            "SUPERWEAPONDETECTED_OWN_NUKE" => EvaMessage::SuperweaponDetectedOwnNuke,
            "SUPERWEAPONDETECTED_OWN_SCUDSTORM" => EvaMessage::SuperweaponDetectedOwnScudStorm,
            "SUPERWEAPONDETECTED_ALLY_PARTICLECANNON" => {
                EvaMessage::SuperweaponDetectedAllyParticleCannon
            }
            "SUPERWEAPONDETECTED_ALLY_NUKE" => EvaMessage::SuperweaponDetectedAllyNuke,
            "SUPERWEAPONDETECTED_ALLY_SCUDSTORM" => EvaMessage::SuperweaponDetectedAllyScudStorm,
            "SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON" => {
                EvaMessage::SuperweaponDetectedEnemyParticleCannon
            }
            "SUPERWEAPONDETECTED_ENEMY_NUKE" => EvaMessage::SuperweaponDetectedEnemyNuke,
            "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM" => EvaMessage::SuperweaponDetectedEnemyScudStorm,
            "SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON" => {
                EvaMessage::SuperweaponLaunchedOwnParticleCannon
            }
            "SUPERWEAPONLAUNCHED_OWN_NUKE" => EvaMessage::SuperweaponLaunchedOwnNuke,
            "SUPERWEAPONLAUNCHED_OWN_SCUDSTORM" => EvaMessage::SuperweaponLaunchedOwnScudStorm,
            "SUPERWEAPONLAUNCHED_ALLY_PARTICLECANNON" => {
                EvaMessage::SuperweaponLaunchedAllyParticleCannon
            }
            "SUPERWEAPONLAUNCHED_ALLY_NUKE" => EvaMessage::SuperweaponLaunchedAllyNuke,
            "SUPERWEAPONLAUNCHED_ALLY_SCUDSTORM" => EvaMessage::SuperweaponLaunchedAllyScudStorm,
            "SUPERWEAPONLAUNCHED_ENEMY_PARTICLECANNON" => {
                EvaMessage::SuperweaponLaunchedEnemyParticleCannon
            }
            "SUPERWEAPONLAUNCHED_ENEMY_NUKE" => EvaMessage::SuperweaponLaunchedEnemyNuke,
            "SUPERWEAPONLAUNCHED_ENEMY_SCUDSTORM" => EvaMessage::SuperweaponLaunchedEnemyScudStorm,
            "SUPERWEAPONREADY_OWN_PARTICLECANNON" => EvaMessage::SuperweaponReadyOwnParticleCannon,
            "SUPERWEAPONREADY_OWN_NUKE" => EvaMessage::SuperweaponReadyOwnNuke,
            "SUPERWEAPONREADY_OWN_SCUDSTORM" => EvaMessage::SuperweaponReadyOwnScudStorm,
            "SUPERWEAPONREADY_ALLY_PARTICLECANNON" => {
                EvaMessage::SuperweaponReadyAllyParticleCannon
            }
            "SUPERWEAPONREADY_ALLY_NUKE" => EvaMessage::SuperweaponReadyAllyNuke,
            "SUPERWEAPONREADY_ALLY_SCUDSTORM" => EvaMessage::SuperweaponReadyAllyScudStorm,
            "SUPERWEAPONREADY_ENEMY_PARTICLECANNON" => {
                EvaMessage::SuperweaponReadyEnemyParticleCannon
            }
            "SUPERWEAPONREADY_ENEMY_NUKE" => EvaMessage::SuperweaponReadyEnemyNuke,
            "SUPERWEAPONREADY_ENEMY_SCUDSTORM" => EvaMessage::SuperweaponReadyEnemyScudStorm,
            "BUILDINGLOST" => EvaMessage::BuildingLost,
            "BASEUNDERATTACK" => EvaMessage::BaseUnderAttack,
            "ALLYUNDERATTACK" => EvaMessage::AllyUnderAttack,
            "BEACONDETECTED" => EvaMessage::BeaconDetected,
            "ENEMYBLACKLOTUSDETECTED" => EvaMessage::EnemyBlackLotusDetected,
            "ENEMYJARMENKELLDETECTED" => EvaMessage::EnemyJarmenKellDetected,
            "ENEMYCOLONELBURTONDETECTED" => EvaMessage::EnemyColonelBurtonDetected,
            "OWNBLACKLOTUSDETECTED" => EvaMessage::OwnBlackLotusDetected,
            "OWNJARMENKELLDETECTED" => EvaMessage::OwnJarmenKellDetected,
            "OWNCOLONELBURTONDETECTED" => EvaMessage::OwnColonelBurtonDetected,
            "UNITLOST" => EvaMessage::UnitLost,
            "GENERALLEVELUP" => EvaMessage::GeneralLevelUp,
            "VEHICLESTOLEN" => EvaMessage::VehicleStolen,
            "BUILDINGSTOLEN" => EvaMessage::BuildingStolen,
            "CASHSTOLEN" => EvaMessage::CashStolen,
            "UPGRADECOMPLETE" => EvaMessage::UpgradeComplete,
            "BUILDINGBEINGSTOLEN" => EvaMessage::BuildingBeingStolen,
            "BUILDINGSABOTAGED" => EvaMessage::BuildingSabotaged,
            "SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER" => {
                EvaMessage::SuperweaponLaunchedOwnGpsScrambler
            }
            "SUPERWEAPONLAUNCHED_ALLY_GPS_SCRAMBLER" => {
                EvaMessage::SuperweaponLaunchedAllyGpsScrambler
            }
            "SUPERWEAPONLAUNCHED_ENEMY_GPS_SCRAMBLER" => {
                EvaMessage::SuperweaponLaunchedEnemyGpsScrambler
            }
            "SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK" => EvaMessage::SuperweaponLaunchedOwnSneakAttack,
            "SUPERWEAPONLAUNCHED_ALLY_SNEAK_ATTACK" => {
                EvaMessage::SuperweaponLaunchedAllySneakAttack
            }
            "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK" => {
                EvaMessage::SuperweaponLaunchedEnemySneakAttack
            }
            "EVA_INVALID" => EvaMessage::Invalid,
            _ => EvaMessage::Invalid,
        }
    }

    /// Convert enum value to message name
    /// Matches C++ Eva::messageToName() from Eva.cpp lines 165-175
    pub fn to_name(&self) -> &'static str {
        match self {
            EvaMessage::Invalid => "EVA_INVALID",
            EvaMessage::LowPower => "LOWPOWER",
            EvaMessage::InsufficientFunds => "INSUFFICIENTFUNDS",
            EvaMessage::SuperweaponDetectedOwnParticleCannon => {
                "SUPERWEAPONDETECTED_OWN_PARTICLECANNON"
            }
            EvaMessage::SuperweaponDetectedOwnNuke => "SUPERWEAPONDETECTED_OWN_NUKE",
            EvaMessage::SuperweaponDetectedOwnScudStorm => "SUPERWEAPONDETECTED_OWN_SCUDSTORM",
            EvaMessage::SuperweaponDetectedAllyParticleCannon => {
                "SUPERWEAPONDETECTED_ALLY_PARTICLECANNON"
            }
            EvaMessage::SuperweaponDetectedAllyNuke => "SUPERWEAPONDETECTED_ALLY_NUKE",
            EvaMessage::SuperweaponDetectedAllyScudStorm => "SUPERWEAPONDETECTED_ALLY_SCUDSTORM",
            EvaMessage::SuperweaponDetectedEnemyParticleCannon => {
                "SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON"
            }
            EvaMessage::SuperweaponDetectedEnemyNuke => "SUPERWEAPONDETECTED_ENEMY_NUKE",
            EvaMessage::SuperweaponDetectedEnemyScudStorm => "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM",
            EvaMessage::SuperweaponLaunchedOwnParticleCannon => {
                "SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON"
            }
            EvaMessage::SuperweaponLaunchedOwnNuke => "SUPERWEAPONLAUNCHED_OWN_NUKE",
            EvaMessage::SuperweaponLaunchedOwnScudStorm => "SUPERWEAPONLAUNCHED_OWN_SCUDSTORM",
            EvaMessage::SuperweaponLaunchedAllyParticleCannon => {
                "SUPERWEAPONLAUNCHED_ALLY_PARTICLECANNON"
            }
            EvaMessage::SuperweaponLaunchedAllyNuke => "SUPERWEAPONLAUNCHED_ALLY_NUKE",
            EvaMessage::SuperweaponLaunchedAllyScudStorm => "SUPERWEAPONLAUNCHED_ALLY_SCUDSTORM",
            EvaMessage::SuperweaponLaunchedEnemyParticleCannon => {
                "SUPERWEAPONLAUNCHED_ENEMY_PARTICLECANNON"
            }
            EvaMessage::SuperweaponLaunchedEnemyNuke => "SUPERWEAPONLAUNCHED_ENEMY_NUKE",
            EvaMessage::SuperweaponLaunchedEnemyScudStorm => "SUPERWEAPONLAUNCHED_ENEMY_SCUDSTORM",
            EvaMessage::SuperweaponReadyOwnParticleCannon => "SUPERWEAPONREADY_OWN_PARTICLECANNON",
            EvaMessage::SuperweaponReadyOwnNuke => "SUPERWEAPONREADY_OWN_NUKE",
            EvaMessage::SuperweaponReadyOwnScudStorm => "SUPERWEAPONREADY_OWN_SCUDSTORM",
            EvaMessage::SuperweaponReadyAllyParticleCannon => {
                "SUPERWEAPONREADY_ALLY_PARTICLECANNON"
            }
            EvaMessage::SuperweaponReadyAllyNuke => "SUPERWEAPONREADY_ALLY_NUKE",
            EvaMessage::SuperweaponReadyAllyScudStorm => "SUPERWEAPONREADY_ALLY_SCUDSTORM",
            EvaMessage::SuperweaponReadyEnemyParticleCannon => {
                "SUPERWEAPONREADY_ENEMY_PARTICLECANNON"
            }
            EvaMessage::SuperweaponReadyEnemyNuke => "SUPERWEAPONREADY_ENEMY_NUKE",
            EvaMessage::SuperweaponReadyEnemyScudStorm => "SUPERWEAPONREADY_ENEMY_SCUDSTORM",
            EvaMessage::BuildingLost => "BUILDINGLOST",
            EvaMessage::BaseUnderAttack => "BASEUNDERATTACK",
            EvaMessage::AllyUnderAttack => "ALLYUNDERATTACK",
            EvaMessage::BeaconDetected => "BEACONDETECTED",
            EvaMessage::EnemyBlackLotusDetected => "ENEMYBLACKLOTUSDETECTED",
            EvaMessage::EnemyJarmenKellDetected => "ENEMYJARMENKELLDETECTED",
            EvaMessage::EnemyColonelBurtonDetected => "ENEMYCOLONELBURTONDETECTED",
            EvaMessage::OwnBlackLotusDetected => "OWNBLACKLOTUSDETECTED",
            EvaMessage::OwnJarmenKellDetected => "OWNJARMENKELLDETECTED",
            EvaMessage::OwnColonelBurtonDetected => "OWNCOLONELBURTONDETECTED",
            EvaMessage::UnitLost => "UNITLOST",
            EvaMessage::GeneralLevelUp => "GENERALLEVELUP",
            EvaMessage::VehicleStolen => "VEHICLESTOLEN",
            EvaMessage::BuildingStolen => "BUILDINGSTOLEN",
            EvaMessage::CashStolen => "CASHSTOLEN",
            EvaMessage::UpgradeComplete => "UPGRADECOMPLETE",
            EvaMessage::BuildingBeingStolen => "BUILDINGBEINGSTOLEN",
            EvaMessage::BuildingSabotaged => "BUILDINGSABOTAGED",
            EvaMessage::SuperweaponLaunchedOwnGpsScrambler => {
                "SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER"
            }
            EvaMessage::SuperweaponLaunchedAllyGpsScrambler => {
                "SUPERWEAPONLAUNCHED_ALLY_GPS_SCRAMBLER"
            }
            EvaMessage::SuperweaponLaunchedEnemyGpsScrambler => {
                "SUPERWEAPONLAUNCHED_ENEMY_GPS_SCRAMBLER"
            }
            EvaMessage::SuperweaponLaunchedOwnSneakAttack => "SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK",
            EvaMessage::SuperweaponLaunchedAllySneakAttack => {
                "SUPERWEAPONLAUNCHED_ALLY_SNEAK_ATTACK"
            }
            EvaMessage::SuperweaponLaunchedEnemySneakAttack => {
                "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK"
            }
            EvaMessage::Count => "EVA_INVALID",
        }
    }
}

/// Side-specific sound definitions for EVA messages
/// Matches C++ EvaSideSounds from Eva.h lines 84-90
///
/// # C++ Definition
/// ```cpp
/// struct EvaSideSounds
/// {
///     AsciiString m_side;
///     std::vector<AsciiString> m_soundNames;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct EvaSideSounds {
    /// Side/faction name (e.g., "America", "China", "GLA")
    pub side: String,

    /// List of sound event names for this side
    /// One will be randomly selected when playing
    pub sound_names: Vec<String>,
}

impl EvaSideSounds {
    /// Create new empty side sounds
    pub fn new() -> Self {
        Self {
            side: String::new(),
            sound_names: Vec::new(),
        }
    }
}

impl Default for EvaSideSounds {
    fn default() -> Self {
        Self::new()
    }
}

/// EVA check information - defines how an EVA message behaves
/// Matches C++ EvaCheckInfo from Eva.h lines 93-106
///
/// # C++ Definition
/// ```cpp
/// class EvaCheckInfo : public MemoryPoolObject
/// {
/// public:
///     EvaMessage m_message;
///     UnsignedInt m_framesBetweenChecks;
///     UnsignedInt m_framesToExpire;
///     UnsignedInt m_priority;
///     std::vector<EvaSideSounds> m_evaSideSounds;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct EvaCheckInfo {
    /// The EVA message this info is for
    pub message: EvaMessage,

    /// Number of frames between check attempts
    /// Parsed from TimeBetweenChecksMS (converted from ms to frames)
    pub frames_between_checks: u32,

    /// Number of frames before an unplayed message expires
    /// Parsed from ExpirationTimeMS (converted from ms to frames)
    pub frames_to_expire: u32,

    /// Priority for playing (higher = more important)
    /// When multiple messages want to play, highest priority plays first
    pub priority: u32,

    /// Side-specific sound definitions
    pub eva_side_sounds: Vec<EvaSideSounds>,
}

impl EvaCheckInfo {
    /// Create new EVA check info with default values
    /// Matches C++ EvaCheckInfo::EvaCheckInfo() from Eva.cpp lines 73-79
    pub fn new() -> Self {
        Self {
            message: EvaMessage::Invalid,
            priority: 1,                // Lowest priority
            frames_between_checks: 900, // 30 seconds at 30 FPS
            frames_to_expire: 150,      // 5 seconds at 30 FPS
            eva_side_sounds: Vec::new(),
        }
    }
}

impl Default for EvaCheckInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// EVA event store - manages all EVA check info definitions
pub struct EvaEventStore {
    /// Map of message name to check info
    check_infos: HashMap<EvaMessage, EvaCheckInfo>,
}

impl EvaEventStore {
    /// Create a new empty store
    pub fn new() -> Self {
        Self {
            check_infos: HashMap::new(),
        }
    }

    /// Initialize the store (clear all existing info)
    pub fn init(&mut self) {
        self.check_infos.clear();
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.check_infos.is_empty()
    }

    /// Get count of registered check infos
    pub fn len(&self) -> usize {
        self.check_infos.len()
    }

    /// Get an EVA check info by message
    pub fn get_check_info(&self, message: EvaMessage) -> Option<&EvaCheckInfo> {
        self.check_infos.get(&message)
    }

    /// Add or update an EVA check info
    /// Returns true if added (new), false if updated (existing)
    /// Matches C++ Eva::newEvaCheckInfo() from Eva.cpp lines 179-196
    pub fn add_check_info(&mut self, info: EvaCheckInfo) -> bool {
        let is_new = !self.check_infos.contains_key(&info.message);
        self.check_infos.insert(info.message, info);
        is_new
    }

    /// Parse an EvaEvent definition from INI
    /// Matches C++ INI::parseEvaEvent() from Eva.cpp lines 43-57
    pub fn parse_eva_event_definition(&mut self, ini: &mut INI) -> INIResult<()> {
        // Read the message name
        let name_token = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let message = EvaMessage::from_name(&name_token);

        if message == EvaMessage::Invalid {
            // Invalid message name - C++ throws ERROR_BAD_INI
            return Err(INIError::InvalidData);
        }

        // Check if already exists (C++ returns NULL for existing)
        if self.check_infos.contains_key(&message) {
            // Skip parsing - already exists
            // Consume the block without parsing
            self.skip_block(ini)?;
            return Ok(());
        }

        // Create new check info
        let mut info = EvaCheckInfo::new();
        info.message = message;

        // Parse fields
        self.parse_eva_event_fields(ini, &mut info)?;

        // Add to store
        self.check_infos.insert(message, info);

        Ok(())
    }

    /// Skip an INI block without parsing
    fn skip_block(&self, ini: &mut INI) -> INIResult<()> {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }

            let tokens = ini.get_line_tokens();
            if tokens.is_empty() {
                continue;
            }

            if tokens[0].eq_ignore_ascii_case("End") {
                break;
            }
        }
        Ok(())
    }

    /// Parse EVA event fields from INI
    /// Matches C++ field parse tables from Eva.cpp
    fn parse_eva_event_fields(&self, ini: &mut INI, info: &mut EvaCheckInfo) -> INIResult<()> {
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }

            let tokens = ini.get_line_tokens();
            if tokens.is_empty() {
                continue;
            }

            let key = tokens[0];
            if key.eq_ignore_ascii_case("End") {
                break;
            }

            // Get the value tokens (skip key and any '=' signs)
            let mut value_tokens: Vec<&str> = tokens.iter().skip(1).copied().collect();
            value_tokens.retain(|t| *t != "=");

            // Parse fields based on key
            // Matches C++ field parse table from Eva.cpp lines 83-89
            match key.to_ascii_lowercase().as_str() {
                "priority" => {
                    // parseUnsignedInt
                    info.priority = value_tokens
                        .first()
                        .ok_or(INIError::InvalidData)?
                        .parse()
                        .map_err(|_| INIError::InvalidData)?;
                }
                "timebetweenchecksms" => {
                    // parseDurationUnsignedInt - convert ms to frames
                    info.frames_between_checks = Self::parse_duration_to_frames(
                        value_tokens.first().ok_or(INIError::InvalidData)?,
                    )?;
                }
                "expirationtimems" => {
                    // parseDurationUnsignedInt - convert ms to frames
                    info.frames_to_expire = Self::parse_duration_to_frames(
                        value_tokens.first().ok_or(INIError::InvalidData)?,
                    )?;
                }
                "sidesounds" => {
                    // parseSideSoundsList - nested block
                    let side_sounds = self.parse_side_sounds_block(ini)?;
                    info.eva_side_sounds.push(side_sounds);
                }
                _ => {
                    // Unknown field - log warning but don't fail
                }
            }
        }

        Ok(())
    }

    /// Parse a SideSounds nested block
    /// Matches C++ parseSideSoundsList() from Eva.cpp lines 61-70
    fn parse_side_sounds_block(&self, ini: &mut INI) -> INIResult<EvaSideSounds> {
        let mut side_sounds = EvaSideSounds::new();

        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }

            let tokens = ini.get_line_tokens();
            if tokens.is_empty() {
                continue;
            }

            let key = tokens[0];
            if key.eq_ignore_ascii_case("End") {
                break;
            }

            // Get the value tokens (skip key and any '=' signs)
            let mut value_tokens: Vec<&str> = tokens.iter().skip(1).copied().collect();
            value_tokens.retain(|t| *t != "=");

            // Parse fields based on key
            // Matches C++ field parse table from Eva.cpp lines 61-66
            match key.to_ascii_lowercase().as_str() {
                "side" => {
                    // parseAsciiString
                    side_sounds.side = value_tokens
                        .first()
                        .ok_or(INIError::InvalidData)?
                        .to_string();
                }
                "sounds" => {
                    // parseSoundsList - space-separated sound names
                    side_sounds.sound_names = value_tokens.iter().map(|s| s.to_string()).collect();
                }
                _ => {
                    // Unknown field - ignore
                }
            }
        }

        Ok(side_sounds)
    }

    /// Parse duration string to frames (assuming 30 FPS)
    /// Matches C++ INI::parseDurationUnsignedInt
    fn parse_duration_to_frames(token: &str) -> INIResult<u32> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(INIError::InvalidData);
        }

        let lower = trimmed.to_ascii_lowercase();
        let (value_str, multiplier) = if let Some(stripped) = lower.strip_suffix("ms") {
            (stripped, 1.0)
        } else if let Some(stripped) = lower.strip_suffix('s') {
            (stripped, 1000.0)
        } else {
            (lower.as_str(), 1.0)
        };

        let value: f32 = value_str.parse().map_err(|_| INIError::InvalidData)?;
        if value.is_sign_negative() {
            return Err(INIError::InvalidData);
        }

        let msecs = value * multiplier;
        // Convert ms to frames at 30 FPS
        let frames = (msecs / (1000.0 / 30.0)).round() as u32;
        Ok(frames)
    }
}

impl Default for EvaEventStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global EVA event store instance
static EVA_EVENT_STORE: OnceCell<RwLock<EvaEventStore>> = OnceCell::new();

/// Get the global EVA event store (read access)
pub fn get_eva_event_store() -> RwLockReadGuard<'static, EvaEventStore> {
    EVA_EVENT_STORE
        .get_or_init(|| RwLock::new(EvaEventStore::new()))
        .read()
        .unwrap()
}

/// Get the global EVA event store (write access)
pub fn get_eva_event_store_mut() -> RwLockWriteGuard<'static, EvaEventStore> {
    EVA_EVENT_STORE
        .get_or_init(|| RwLock::new(EvaEventStore::new()))
        .write()
        .unwrap()
}

/// Initialize the global EVA event store
pub fn init_eva_event_store() {
    if EVA_EVENT_STORE.get().is_none() {
        let _ = EVA_EVENT_STORE.set(RwLock::new(EvaEventStore::new()));
    } else if let Some(store) = EVA_EVENT_STORE.get() {
        if let Ok(mut guard) = store.write() {
            guard.init();
        }
    }
}

/// Parse EvaEvent definition from INI block
/// This is the main entry point for the INI parser
/// Matches C++ INI::parseEvaEvent from Eva.cpp lines 43-57
pub fn parse_eva_event_definition(ini: &mut INI) -> Result<(), String> {
    let mut store = get_eva_event_store_mut();

    store
        .parse_eva_event_definition(ini)
        .map_err(|e| format!("EvaEvent parse error: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_eva_message_from_name() {
        assert_eq!(EvaMessage::from_name("LOWPOWER"), EvaMessage::LowPower);
        assert_eq!(EvaMessage::from_name("lowpower"), EvaMessage::LowPower);
        assert_eq!(
            EvaMessage::from_name("INSUFFICIENTFUNDS"),
            EvaMessage::InsufficientFunds
        );
        assert_eq!(
            EvaMessage::from_name("BUILDINGLOST"),
            EvaMessage::BuildingLost
        );
        assert_eq!(
            EvaMessage::from_name("BASEUNDERATTACK"),
            EvaMessage::BaseUnderAttack
        );
        assert_eq!(EvaMessage::from_name("UNKNOWN"), EvaMessage::Invalid);
    }

    #[test]
    fn test_eva_message_to_name() {
        assert_eq!(EvaMessage::LowPower.to_name(), "LOWPOWER");
        assert_eq!(EvaMessage::InsufficientFunds.to_name(), "INSUFFICIENTFUNDS");
        assert_eq!(EvaMessage::BuildingLost.to_name(), "BUILDINGLOST");
        assert_eq!(EvaMessage::Invalid.to_name(), "EVA_INVALID");
    }

    #[test]
    fn test_eva_check_info_defaults() {
        let info = EvaCheckInfo::new();

        assert_eq!(info.message, EvaMessage::Invalid);
        assert_eq!(info.priority, 1);
        assert_eq!(info.frames_between_checks, 900); // 30 seconds at 30 FPS
        assert_eq!(info.frames_to_expire, 150); // 5 seconds at 30 FPS
        assert!(info.eva_side_sounds.is_empty());
    }

    #[test]
    fn test_eva_side_sounds() {
        let mut sounds = EvaSideSounds::new();
        sounds.side = "America".to_string();
        sounds.sound_names = vec!["Eva_LowPower".to_string(), "Eva_LowPower2".to_string()];

        assert_eq!(sounds.side, "America");
        assert_eq!(sounds.sound_names.len(), 2);
    }

    #[test]
    fn test_parse_duration_to_frames() {
        // Milliseconds
        assert_eq!(EvaEventStore::parse_duration_to_frames("900").unwrap(), 27); // 900ms ~ 27 frames

        // With ms suffix
        assert_eq!(
            EvaEventStore::parse_duration_to_frames("1000ms").unwrap(),
            30
        );

        // With s suffix
        assert_eq!(EvaEventStore::parse_duration_to_frames("1s").unwrap(), 30);
        assert_eq!(EvaEventStore::parse_duration_to_frames("2s").unwrap(), 60);
    }

    #[test]
    fn test_eva_event_store() {
        let mut store = EvaEventStore::new();

        assert!(store.is_empty());

        // Add a check info
        let mut info = EvaCheckInfo::new();
        info.message = EvaMessage::LowPower;
        info.priority = 5;

        let added = store.add_check_info(info);
        assert!(added); // New entry
        assert_eq!(store.len(), 1);

        // Get it back
        let retrieved = store.get_check_info(EvaMessage::LowPower);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().priority, 5);

        // Add duplicate (should update, not add new)
        let mut info2 = EvaCheckInfo::new();
        info2.message = EvaMessage::LowPower;
        info2.priority = 10;

        let added2 = store.add_check_info(info2);
        assert!(!added2); // Update, not new
        assert_eq!(store.len(), 1);

        // Verify update
        let retrieved2 = store.get_check_info(EvaMessage::LowPower);
        assert_eq!(retrieved2.unwrap().priority, 10);
    }

    #[test]
    fn test_global_store() {
        init_eva_event_store();

        let store = get_eva_event_store();
        assert!(store.len() >= 0);
    }
}
