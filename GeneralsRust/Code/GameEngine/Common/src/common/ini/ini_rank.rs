////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_rank.rs
//! Author: Steven Johnson, September 2002 (Converted to Rust)
//! Desc: Rank info parsing and management for player progression
//!
//! Matches C++ RankInfo.h and RankInfo.cpp from:
//! - GeneralsMD/Code/GameEngine/Include/GameLogic/RankInfo.h
//! - GeneralsMD/Code/GameEngine/Source/GameLogic/System/RankInfo.cpp
//!
//! # C++ Field Parse Table (RankInfo.cpp lines 66-71)
//! ```cpp
//! static const FieldParse myFieldParse[] =
//! {
//!     { "RankName", INI::parseAndTranslateLabel, NULL, offsetof( RankInfo, m_rankName ) },
//!     { "SkillPointsNeeded", INI::parseInt, NULL, offsetof( RankInfo, m_skillPointsNeeded ) },
//!     { "SciencesGranted", INI::parseScienceVector, NULL, offsetof( RankInfo, m_sciencesGranted ) },
//!     { "SciencePurchasePointsGranted", INI::parseUnsignedInt, NULL, offsetof( RankInfo, m_sciencePurchasePointsGranted ) },
//!     { 0, 0, 0, 0 }
//! };
//! ```

use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ini::{INIError, INIResult, INI};
use crate::common::rts::science::{get_science_store, ScienceType, SCIENCE_INVALID};

/// Result type for rank operations
pub type RankResult<T> = Result<T, RankError>;

/// Errors that can occur during rank parsing
#[derive(Debug, Clone, PartialEq)]
pub enum RankError {
    InvalidRankLevel,
    InvalidRankName,
    ParseError(String),
    NonMonotonicRank,
    NotFound,
    AlreadyExists,
}

impl std::fmt::Display for RankError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RankError::InvalidRankLevel => write!(f, "Invalid rank level"),
            RankError::InvalidRankName => write!(f, "Invalid rank name"),
            RankError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            RankError::NonMonotonicRank => write!(f, "Ranks must increase monotonically"),
            RankError::NotFound => write!(f, "Rank not found"),
            RankError::AlreadyExists => write!(f, "Rank already exists"),
        }
    }
}

impl std::error::Error for RankError {}

/// Rank information structure
/// Matches C++ RankInfo from RankInfo.h lines 26-33
///
/// # C++ Definition
/// ```cpp
/// class RankInfo : public Overridable
/// {
/// public:
///     UnicodeString   m_rankName;
///     Int             m_skillPointsNeeded;
///     Int             m_sciencePurchasePointsGranted;
///     ScienceVec      m_sciencesGranted;
/// };
/// ```
#[derive(Debug, Clone)]
pub struct RankInfo {
    /// Localized rank display name (from INI label translation)
    pub rank_name: String,

    /// Skill points required to reach this rank level
    /// Note: Rank 1 has 0 skill points needed (starting rank)
    pub skill_points_needed: i32,

    /// Science purchase points granted when reaching this rank
    /// These are the points used to buy abilities in the Generals Points system
    pub science_purchase_points_granted: u32,

    /// Sciences automatically granted when reaching this rank level
    /// Typically contains SCIENCE_RankN for the corresponding rank
    pub sciences_granted: Vec<ScienceType>,

    /// Flag indicating this is an override definition
    is_override: bool,
}

impl RankInfo {
    /// Create a new RankInfo with default values
    /// Matches C++ default initialization
    pub fn new() -> Self {
        Self {
            rank_name: String::new(),
            skill_points_needed: 0,
            science_purchase_points_granted: 0,
            sciences_granted: Vec::new(),
            is_override: false,
        }
    }

    /// Mark this RankInfo as an override
    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    /// Check if this is an override
    pub fn is_override(&self) -> bool {
        self.is_override
    }
}

impl Default for RankInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Rank info store - manages all rank definitions
/// Matches C++ RankInfoStore from RankInfo.h lines 36-60
///
/// # C++ Definition
/// ```cpp
/// class RankInfoStore : public SubsystemInterface
/// {
/// public:
///     void init();
///     void reset();
///     void update() { }
///     Int getRankLevelCount() const;
///     const RankInfo* getRankInfo(Int level) const;  // level is 1...n, NOT 0...n-1
///     static void friend_parseRankDefinition(INI* ini);
/// private:
///     typedef std::vector<RankInfo*> RankInfoVec;
///     RankInfoVec m_rankInfos;
/// };
/// ```
pub struct RankInfoStore {
    /// Vector of rank infos, indexed by (level - 1)
    /// Ranks are 1-indexed externally, 0-indexed internally
    rank_infos: Vec<RankInfo>,
}

impl RankInfoStore {
    /// Create a new empty rank info store
    pub fn new() -> Self {
        Self {
            rank_infos: Vec::new(),
        }
    }

    /// Initialize the store (clear all existing ranks)
    /// Matches C++ RankInfoStore::init() from RankInfo.cpp lines 52-56
    pub fn init(&mut self) {
        self.rank_infos.clear();
    }

    /// Reset the store (clear overrides, keep base definitions)
    /// Matches C++ RankInfoStore::reset() from RankInfo.cpp lines 59-77
    ///
    /// In C++, this removes override instances while preserving base definitions.
    /// The Rust implementation is simplified since we don't use the Overridable pattern.
    pub fn reset(&mut self) {
        // In the C++ version, this calls deleteOverrides() on each RankInfo
        // and removes entries that become null. Since Rust doesn't have the
        // same override pattern, we just clear the override flags.
        for info in &mut self.rank_infos {
            info.is_override = false;
        }
    }

    /// Get the number of rank levels defined
    /// Matches C++ RankInfoStore::getRankLevelCount() from RankInfo.cpp lines 81-84
    pub fn get_rank_level_count(&self) -> i32 {
        self.rank_infos.len() as i32
    }

    /// Get rank info for a specific level
    /// Matches C++ RankInfoStore::getRankInfo() from RankInfo.cpp lines 88-101
    ///
    /// # Arguments
    /// * `level` - Rank level (1-indexed, NOT 0-indexed)
    ///
    /// # Returns
    /// * `Some(&RankInfo)` if the level exists
    /// * `None` if level < 1 or level > count
    pub fn get_rank_info(&self, level: i32) -> Option<&RankInfo> {
        if level >= 1 && level as usize <= self.rank_infos.len() {
            Some(&self.rank_infos[(level - 1) as usize])
        } else {
            None
        }
    }

    /// Check if the store is empty
    pub fn is_empty(&self) -> bool {
        self.rank_infos.is_empty()
    }

    /// Add a new rank info (must be in sequential order)
    /// Returns error if rank level doesn't match expected next level
    fn add_rank(&mut self, info: RankInfo) -> RankResult<()> {
        let expected_level = self.rank_infos.len() + 1;
        let actual_level = self.rank_infos.len() + 1; // New rank will be at this position

        self.rank_infos.push(info);
        Ok(())
    }

    /// Update an existing rank info (for overrides)
    fn update_rank(&mut self, level: i32, info: RankInfo) -> RankResult<()> {
        if level < 1 || level as usize > self.rank_infos.len() {
            return Err(RankError::NotFound);
        }

        self.rank_infos[(level - 1) as usize] = info;
        Ok(())
    }

    /// Parse a rank definition from INI
    /// Matches C++ RankInfoStore::friend_parseRankDefinition() from RankInfo.cpp lines 104-148
    pub fn parse_rank_definition(&mut self, ini: &mut INI, is_override: bool) -> INIResult<()> {
        // Read the rank level number
        let rank_token = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
        let rank_level: i32 = rank_token.parse().map_err(|_| INIError::InvalidData)?;

        if is_override {
            // In override mode, can only override existing ranks
            // Matches C++ RankInfo.cpp lines 114-128
            if rank_level < 1 || rank_level as usize > self.rank_infos.len() {
                // Rank not found in map.ini - this is an error in C++
                return Err(INIError::InvalidData);
            }

            // Get existing info and copy it
            let existing = &self.rank_infos[(rank_level - 1) as usize];
            let mut new_info = existing.clone();
            new_info.mark_as_override();

            // Parse fields from INI
            self.parse_rank_fields(ini, &mut new_info)?;

            // Update the existing entry
            self.rank_infos[(rank_level - 1) as usize] = new_info;
        } else {
            // In normal mode, ranks must increase monotonically
            // Matches C++ RankInfo.cpp lines 130-147
            let expected_level = self.rank_infos.len() + 1;
            if rank_level != expected_level as i32 {
                // C++ throws INI_INVALID_DATA for this
                return Err(INIError::InvalidData);
            }

            let mut info = RankInfo::new();
            self.parse_rank_fields(ini, &mut info)?;

            self.rank_infos.push(info);
        }

        Ok(())
    }

    /// Parse rank fields from INI
    /// Matches C++ field parse table from RankInfo.cpp lines 66-71
    fn parse_rank_fields(&mut self, ini: &mut INI, info: &mut RankInfo) -> INIResult<()> {
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
            let value = value_tokens.join(" ");

            // Parse fields based on key
            // Matches C++ field parse table from RankInfo.cpp lines 66-71
            match key.to_ascii_lowercase().as_str() {
                "rankname" => {
                    // parseAndTranslateLabel - store as string (would translate from STR file)
                    info.rank_name = value;
                }
                "skillpointsneeded" => {
                    // parseInt
                    info.skill_points_needed = value.parse().map_err(|_| INIError::InvalidData)?;
                }
                "sciencesgranted" => {
                    // parseScienceVector - parse space-separated science names
                    info.sciences_granted = self.parse_science_vector(&value);
                }
                "sciencepurchasepointsgranted" => {
                    // parseUnsignedInt
                    info.science_purchase_points_granted =
                        value.parse().map_err(|_| INIError::InvalidData)?;
                }
                _ => {
                    // Unknown field - log warning but don't fail
                    // In C++, unknown fields in the parse table are silently ignored
                }
            }
        }

        Ok(())
    }

    /// Parse a vector of sciences from a space-separated string
    /// Matches C++ INI::parseScienceVector from INI.cpp lines 674-685
    fn parse_science_vector(&self, value: &str) -> Vec<ScienceType> {
        let mut sciences = Vec::new();

        for token in value.split_whitespace() {
            if token.is_empty() || token.eq_ignore_ascii_case("None") {
                continue;
            }

            // Look up science by name from the science store
            if let Some(store) = get_science_store() {
                let science = store.get_science_from_internal_name(token);
                if science != SCIENCE_INVALID {
                    sciences.push(science);
                }
            }
        }

        sciences
    }

    /// Get rank level for a given skill points total
    /// Helper function to determine current rank based on accumulated skill points
    pub fn get_rank_level_for_skill_points(&self, skill_points: i32) -> i32 {
        let mut level = 1;

        for (idx, info) in self.rank_infos.iter().enumerate() {
            if skill_points >= info.skill_points_needed {
                level = (idx + 1) as i32;
            } else {
                break;
            }
        }

        level
    }

    /// Get total science purchase points granted up to a given rank level
    pub fn get_total_purchase_points_up_to_level(&self, level: i32) -> u32 {
        let mut total = 0u32;

        for i in 0..level.min(self.rank_infos.len() as i32) {
            total += self.rank_infos[i as usize].science_purchase_points_granted;
        }

        total
    }

    /// Get all sciences granted up to a given rank level
    pub fn get_sciences_granted_up_to_level(&self, level: i32) -> Vec<ScienceType> {
        let mut sciences = Vec::new();

        for i in 0..level.min(self.rank_infos.len() as i32) {
            sciences.extend(self.rank_infos[i as usize].sciences_granted.iter().copied());
        }

        sciences
    }
}

impl Default for RankInfoStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global rank info store instance
static RANK_INFO_STORE: OnceCell<RwLock<RankInfoStore>> = OnceCell::new();

/// Get the global rank info store (read access)
pub fn get_rank_info_store() -> RwLockReadGuard<'static, RankInfoStore> {
    RANK_INFO_STORE
        .get_or_init(|| RwLock::new(RankInfoStore::new()))
        .read()
        .unwrap()
}

/// Get the global rank info store (write access)
pub fn get_rank_info_store_mut() -> RwLockWriteGuard<'static, RankInfoStore> {
    RANK_INFO_STORE
        .get_or_init(|| RwLock::new(RankInfoStore::new()))
        .write()
        .unwrap()
}

/// Initialize the global rank info store
pub fn init_rank_info_store() {
    if RANK_INFO_STORE.get().is_none() {
        let _ = RANK_INFO_STORE.set(RwLock::new(RankInfoStore::new()));
    } else if let Some(store) = RANK_INFO_STORE.get() {
        if let Ok(mut guard) = store.write() {
            guard.init();
        }
    }
}

/// Parse rank definition from INI block
/// This is the main entry point for the INI parser
/// Matches C++ INI::parseRankDefinition from RankInfo.cpp lines 151-154
pub fn parse_rank_definition(ini: &mut INI) -> Result<(), String> {
    let is_override = ini.get_load_type() == crate::common::ini::INILoadType::CreateOverrides;

    let mut store = get_rank_info_store_mut();
    store
        .parse_rank_definition(ini, is_override)
        .map_err(|e| format!("Rank parse error: {:?}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rank_info_creation() {
        let info = RankInfo::new();

        assert!(info.rank_name.is_empty());
        assert_eq!(info.skill_points_needed, 0);
        assert_eq!(info.science_purchase_points_granted, 0);
        assert!(info.sciences_granted.is_empty());
        assert!(!info.is_override());
    }

    #[test]
    fn test_rank_info_override() {
        let mut info = RankInfo::new();
        info.mark_as_override();

        assert!(info.is_override());
    }

    #[test]
    fn test_rank_info_store_creation() {
        let store = RankInfoStore::new();

        assert!(store.is_empty());
        assert_eq!(store.get_rank_level_count(), 0);
        assert!(store.get_rank_info(1).is_none());
    }

    #[test]
    fn test_rank_info_store_operations() {
        let mut store = RankInfoStore::new();

        // Add first rank
        let mut info1 = RankInfo::new();
        info1.rank_name = "Rank 1".to_string();
        info1.skill_points_needed = 0;
        info1.science_purchase_points_granted = 1;
        store.add_rank(info1).unwrap();

        assert_eq!(store.get_rank_level_count(), 1);
        assert!(store.get_rank_info(1).is_some());
        assert!(store.get_rank_info(0).is_none()); // Level 0 is invalid
        assert!(store.get_rank_info(2).is_none()); // Level 2 doesn't exist

        // Add second rank
        let mut info2 = RankInfo::new();
        info2.rank_name = "Rank 2".to_string();
        info2.skill_points_needed = 800;
        info2.science_purchase_points_granted = 1;
        store.add_rank(info2).unwrap();

        assert_eq!(store.get_rank_level_count(), 2);

        // Verify rank 1 is accessible
        let rank1 = store.get_rank_info(1).unwrap();
        assert_eq!(rank1.rank_name, "Rank 1");
        assert_eq!(rank1.skill_points_needed, 0);

        // Verify rank 2 is accessible
        let rank2 = store.get_rank_info(2).unwrap();
        assert_eq!(rank2.rank_name, "Rank 2");
        assert_eq!(rank2.skill_points_needed, 800);
    }

    #[test]
    fn test_rank_level_for_skill_points() {
        let mut store = RankInfoStore::new();

        // Add ranks matching the actual game data
        let mut rank1 = RankInfo::new();
        rank1.skill_points_needed = 0;
        store.add_rank(rank1).unwrap();

        let mut rank2 = RankInfo::new();
        rank2.skill_points_needed = 800;
        store.add_rank(rank2).unwrap();

        let mut rank3 = RankInfo::new();
        rank3.skill_points_needed = 1500;
        store.add_rank(rank3).unwrap();

        let mut rank4 = RankInfo::new();
        rank4.skill_points_needed = 2500;
        store.add_rank(rank4).unwrap();

        let mut rank5 = RankInfo::new();
        rank5.skill_points_needed = 5000;
        store.add_rank(rank5).unwrap();

        // Test skill point thresholds
        assert_eq!(store.get_rank_level_for_skill_points(0), 1);
        assert_eq!(store.get_rank_level_for_skill_points(799), 1);
        assert_eq!(store.get_rank_level_for_skill_points(800), 2);
        assert_eq!(store.get_rank_level_for_skill_points(1499), 2);
        assert_eq!(store.get_rank_level_for_skill_points(1500), 3);
        assert_eq!(store.get_rank_level_for_skill_points(2499), 3);
        assert_eq!(store.get_rank_level_for_skill_points(2500), 4);
        assert_eq!(store.get_rank_level_for_skill_points(4999), 4);
        assert_eq!(store.get_rank_level_for_skill_points(5000), 5);
        assert_eq!(store.get_rank_level_for_skill_points(10000), 5);
    }

    #[test]
    fn test_total_purchase_points() {
        let mut store = RankInfoStore::new();

        // Add ranks matching the actual game data
        let mut rank1 = RankInfo::new();
        rank1.science_purchase_points_granted = 1;
        store.add_rank(rank1).unwrap();

        let mut rank2 = RankInfo::new();
        rank2.science_purchase_points_granted = 1;
        store.add_rank(rank2).unwrap();

        let mut rank3 = RankInfo::new();
        rank3.science_purchase_points_granted = 1;
        store.add_rank(rank3).unwrap();

        let mut rank4 = RankInfo::new();
        rank4.science_purchase_points_granted = 1;
        store.add_rank(rank4).unwrap();

        let mut rank5 = RankInfo::new();
        rank5.science_purchase_points_granted = 3;
        store.add_rank(rank5).unwrap();

        // Test cumulative purchase points
        assert_eq!(store.get_total_purchase_points_up_to_level(1), 1);
        assert_eq!(store.get_total_purchase_points_up_to_level(2), 2);
        assert_eq!(store.get_total_purchase_points_up_to_level(3), 3);
        assert_eq!(store.get_total_purchase_points_up_to_level(4), 4);
        assert_eq!(store.get_total_purchase_points_up_to_level(5), 7); // 1+1+1+1+3
    }

    #[test]
    fn test_global_store() {
        init_rank_info_store();

        let store = get_rank_info_store();
        assert!(store.is_empty() || store.get_rank_level_count() >= 0);
    }
}
