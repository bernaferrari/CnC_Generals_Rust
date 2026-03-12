//! Science/Technology System
//!
//! Manages the research and technology tree for the game,
//! including prerequisites, costs, and player progression.
//!
//! # Overview
//!
//! This module implements the science (technology) system from C&C Generals Zero Hour.
//! Sciences represent unlockable abilities, units, and upgrades that players can acquire
//! through:
//! - Intrinsic faction bonuses (e.g., SCIENCE_AMERICA, SCIENCE_CHINA, SCIENCE_GLA)
//! - Rank progression (SCIENCE_Rank1 through SCIENCE_Rank8)
//! - Purchase with Generals Points earned during gameplay
//!
//! # C++ Reference
//!
//! This is a faithful port of:
//! - `/GeneralsMD/Code/GameEngine/Include/Common/Science.h`
//! - `/GeneralsMD/Code/GameEngine/Source/Common/RTS/Science.cpp`
//!
//! # Architecture
//!
//! ## ScienceInfo
//! Represents a single science definition with:
//! - Prerequisites: Other sciences required before this can be acquired
//! - Root sciences: Ultimate base requirements (calculated at runtime)
//! - Purchase cost: Points needed to buy (0 = not purchasable)
//! - Grantable flag: Whether science can be granted vs purchased
//! - Display info: Localized name and description for UI
//!
//! ## ScienceStore
//! Global registry of all available sciences. Provides:
//! - Science lookup by name or ID
//! - Prerequisite validation
//! - Purchasable science queries (what player can buy now vs later)
//! - INI file loading
//!
//! ## Player Integration
//! The `Player` struct (in `player.rs`) tracks:
//! - Sciences owned: Set of acquired sciences
//! - Sciences disabled: Temporarily unusable sciences
//! - Sciences hidden: UI-gated sciences
//! - Purchase points: Currency for buying sciences
//!
//! # Generals Points System Integration
//!
//! **Note:** The Generals Points (experience/rank) system is NOT implemented in this module.
//! It would be implemented in a separate `experience.rs` or `generals_points.rs` module.
//!
//! ## How Generals Points Work (C++ reference)
//!
//! 1. **Earning Points**: Players earn points by:
//!    - Destroying enemy units/buildings
//!    - Completing mission objectives
//!    - Winning multiplayer matches
//!
//! 2. **Rank Progression**: Points unlock ranks:
//!    - Each rank threshold grants corresponding SCIENCE_RankN
//!    - Ranks are cumulative (Rank3 implies Rank1 and Rank2)
//!
//! 3. **Science Purchase**: Points can be spent to buy sciences:
//!    - Player must have enough unspent points
//!    - Player must have all prerequisites
//!    - Cost is deducted from available points
//!    - Purchase is permanent (no refunds)
//!
//! ## Integration Points for Future Implementation
//!
//! When implementing the Generals Points system, it should:
//!
//! ```rust,ignore
//! // Example integration (shown only as a usage sketch)
//! struct GeneralsExperience {
//!     total_points: i32,
//!     available_points: i32,  // total - spent
//!     current_rank: i32,
//! }
//!
//! impl GeneralsExperience {
//!     fn award_points(&mut self, player: &mut Player, amount: i32) {
//!         self.total_points += amount;
//!         self.available_points += amount;
//!
//!         // Check for rank-ups
//!         let new_rank = self.calculate_rank(self.total_points);
//!         if new_rank > self.current_rank {
//!             player.grant_science(get_science_for_rank(new_rank));
//!             self.current_rank = new_rank;
//!         }
//!     }
//!
//!     fn purchase_science(&mut self, player: &mut Player,
//!                        store: &ScienceStore, science: ScienceType) -> Result<()> {
//!         let cost = store.get_science_purchase_cost(science);
//!         if cost == 0 { return Err("Not purchasable"); }
//!         if self.available_points < cost { return Err("Insufficient points"); }
//!         if !store.player_has_prereqs_for_science(player, science) {
//!             return Err("Missing prerequisites");
//!         }
//!
//!         self.available_points -= cost;
//!         player.grant_science(science);
//!         Ok(())
//!     }
//! }
//! ```
//!
//! # Serialization
//!
//! **Note:** Serialization is NOT currently implemented but follows this pattern:
//!
//! The C++ implementation uses the Xfer system for save/load:
//!
//! ```cpp
//! // C++ example (not in this port yet)
//! void Player::xfer(Xfer* xfer) {
//!     xfer->xferSnapshot(m_sciences);           // Set of owned sciences
//!     xfer->xferSnapshot(m_sciencesDisabled);   // Set of disabled sciences
//!     xfer->xferSnapshot(m_sciencesHidden);     // Set of hidden sciences
//!     xfer->xferInt(&m_sciencePurchasePoints);  // Available points
//! }
//! ```
//!
//! ## Rust Serialization Strategy
//!
//! For the Rust port, we have several options:
//!
//! ### Option 1: Serde (recommended for simplicity)
//! ```rust,ignore
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct PlayerScienceState {
//!     sciences: HashSet<ScienceType>,
//!     sciences_disabled: HashSet<ScienceType>,
//!     sciences_hidden: HashSet<ScienceType>,
//!     purchase_points: i32,
//! }
//! ```
//!
//! ### Option 2: Custom Xfer Integration (C++ compatible)
//! If we need exact binary compatibility with C++ saves:
//! ```rust,ignore
//! impl XferSave for Player {
//!     fn xfer_save(&self, xfer: &mut impl XferWriter) -> Result<()> {
//!         xfer.xfer_set(&self.sciences)?;
//!         xfer.xfer_set(&self.sciences_disabled)?;
//!         xfer.xfer_set(&self.sciences_hidden)?;
//!         xfer.xfer_i32(self.science_purchase_points)?;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ### What Needs to be Serialized
//!
//! For **runtime state** (save games):
//! - Player::sciences (owned sciences)
//! - Player::sciences_disabled
//! - Player::sciences_hidden
//! - Player::science_purchase_points
//! - GeneralsExperience::total_points
//! - GeneralsExperience::available_points
//! - GeneralsExperience::current_rank
//!
//! For **configuration** (mods, scenarios):
//! - ScienceStore does NOT need save/load (loaded from INI at startup)
//! - However, mod overrides may need special handling
//!
//! # Example Usage
//!
//! ```rust
//! use game_engine::common::rts::*;
//!
//! // Initialize the global science store
//! init_science_store();
//! let mut store = get_science_store_mut().unwrap();
//!
//! // Load sciences from INI files
//! store.load_from_paths(
//!     &["Data/INI/Default/Science.ini"],
//!     &["Data/INI/Science.ini"]
//! ).expect("Failed to load sciences");
//!
//! // Create a player
//! let mut player = Player::new(0);
//! player.grant_science(
//!     store.get_science_from_internal_name("SCIENCE_AMERICA")
//! );
//!
//! // Check what sciences player can purchase
//! let (purchasable, potential) = store.get_purchasable_sciences(&player);
//! ```

use once_cell::sync::OnceCell;
use std::any::Any;
use std::collections::{hash_map::Entry, HashMap};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

use log::{debug, trace, warn};
use thiserror::Error;

use crate::common::ini::{INIError, INIResult, INI};
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::system::subsystem_interface::{
    SubsystemDescriptor, SubsystemError, SubsystemInterface, SubsystemResult, SubsystemState,
};

use super::{AsciiString, NameKeyType};

/// Science/technology type identifier
///
/// Matches the legacy `ScienceType` definition which aliases the `NameKey`
/// produced by `TheNameKeyGenerator`. `-1` (`SCIENCE_INVALID`) is reserved.
pub type ScienceType = i32;

/// Invalid science constant
pub const SCIENCE_INVALID: ScienceType = -1;

/// Science information structure
#[derive(Debug, Clone)]
pub struct ScienceInfo {
    /// The science type identifier
    pub science: ScienceType,
    /// Internal name of the science
    pub name: String,
    /// Display name for UI
    pub display_name: String,
    /// Description text
    pub description: String,
    /// Prerequisite sciences required
    pub prereq_sciences: Vec<ScienceType>,
    /// Root sciences (ultimate prerequisites)
    pub root_sciences: Vec<ScienceType>,
    /// Cost in science purchase points
    pub science_purchase_point_cost: i32,
    /// Whether this science can be granted (vs purchased)
    pub grantable: bool,
}

impl ScienceInfo {
    /// Create a new ScienceInfo with default values
    ///
    /// Matches C++ Science.h lines 43-47 default constructor:
    /// - m_sciencePurchasePointCost: 0 (means "cannot be purchased")
    /// - m_grantable: true (can be granted by game events)
    pub fn new<S: Into<String>>(science: ScienceType, name: S) -> Self {
        Self {
            science,
            name: name.into(),
            display_name: String::new(),
            description: String::new(),
            prereq_sciences: Vec::new(),
            root_sciences: Vec::new(),
            science_purchase_point_cost: 0, // C++ default: 0 means "not purchasable"
            grantable: true,                // C++ default: true
        }
    }

    /// Add root sciences to the provided vector
    ///
    /// If this science has no prerequisites, it's a root science.
    /// Otherwise, add the root sciences of all prerequisites.
    ///
    /// Matches C++ Science.cpp lines 102-120: ScienceInfo::addRootSciences
    /// Recursively traverses prerequisite tree to find base sciences.
    pub fn add_root_sciences(&self, roots: &mut Vec<ScienceType>, science_store: &ScienceStore) {
        // C++ Science.cpp:104-108 - If no prereqs, we're a root
        if self.prereq_sciences.is_empty() {
            // This is a root science
            if !roots.contains(&self.science) {
                roots.push(self.science);
            }
        } else {
            // C++ Science.cpp:110-119 - Otherwise, add roots of all prereqs
            // Add roots of all prerequisites
            for &prereq in &self.prereq_sciences {
                if let Some(prereq_info) = science_store.find_science_info(prereq) {
                    prereq_info.add_root_sciences(roots, science_store);
                }
            }
        }
    }
}

/// Science store - manages all available sciences
#[derive(Debug)]
pub struct ScienceStore {
    /// Map of science type to science information
    sciences: HashMap<ScienceType, ScienceInfo>,
    /// Internal-name key to science type mapping for quick lookups
    name_to_science: HashMap<NameKeyType, ScienceType>,
}

/// Minimal interface required for science prerequisite checks.
pub trait ScienceAccess {
    fn has_science(&self, science: ScienceType) -> bool;
}

/// Intermediate representation parsed from Science INI files.
#[derive(Debug, Clone, Default)]
pub struct ScienceDefinition {
    pub name: AsciiString,
    pub display_name: Option<AsciiString>,
    pub description: Option<AsciiString>,
    pub prereq_names: Option<Vec<AsciiString>>,
    pub cost: Option<i32>,
    pub grantable: Option<bool>,
}

/// Errors that can occur while loading science data.
#[derive(Debug, Error)]
pub enum ScienceLoadError {
    #[error("I/O error while reading '{path:?}': {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("Parse error in '{path:?}' at line {line}: {message}")]
    Parse {
        path: PathBuf,
        line: usize,
        message: String,
    },
}

impl From<ScienceLoadError> for SubsystemError {
    fn from(value: ScienceLoadError) -> Self {
        match value {
            ScienceLoadError::Io { path, source } => SubsystemError::ResourceError(format!(
                "Failed to read science file {:?}: {}",
                path, source
            )),
            ScienceLoadError::Parse {
                path,
                line,
                message,
            } => SubsystemError::InitializationFailed(format!(
                "Error parsing {:?} at line {}: {}",
                path, line, message
            )),
        }
    }
}

impl ScienceStore {
    pub fn new() -> Self {
        Self {
            sciences: HashMap::new(),
            name_to_science: HashMap::new(),
        }
    }

    /// Initialize the science store
    ///
    /// Matches C++ Science.cpp lines 21-25: ScienceStore::init
    /// Clears all sciences to prepare for fresh loading.
    pub fn init(&mut self) {
        self.sciences.clear();
        self.name_to_science.clear();
    }

    /// Reset the science store (clear overrides)
    ///
    /// Matches C++ Science.cpp lines 45-64: ScienceStore::reset
    /// In C++, this deletes override instances while preserving base definitions.
    /// In Rust, we use a simpler approach without the override system.
    pub fn reset(&mut self) {
        // In the C++ version, this handles override cleanup via Overridable::deleteOverrides()
        // The Rust port doesn't use the Overridable pattern, so this is simplified.
        // If we need mod support later, we can extend this to clear mod-loaded sciences.
    }

    /// Get science type from internal name
    ///
    /// Matches C++ Science.cpp lines 67-74: getScienceFromInternalName
    /// Converts a string name (like "SCIENCE_PaladinTank") to a ScienceType key.
    pub fn get_science_from_internal_name(&self, name: &str) -> ScienceType {
        if name.is_empty() {
            return SCIENCE_INVALID;
        }

        let key = NameKeyGenerator::name_to_key(name);
        self.name_to_science
            .get(&key)
            .copied()
            .unwrap_or(SCIENCE_INVALID)
    }

    /// Get internal name for science type
    ///
    /// Matches C++ Science.cpp lines 77-83: getInternalNameForScience
    /// Converts a ScienceType back to its string name.
    pub fn get_internal_name_for_science(&self, science: ScienceType) -> AsciiString {
        self.sciences
            .get(&science)
            .map(|info| info.name.clone())
            .unwrap_or_default()
    }

    /// Find science information by type
    ///
    /// Matches C++ Science.cpp lines 124-135: findScienceInfo
    /// Internal helper to locate ScienceInfo by ScienceType.
    pub fn find_science_info(&self, science: ScienceType) -> Option<&ScienceInfo> {
        self.sciences.get(&science)
    }

    /// Add a science to the store
    pub fn add_science(&mut self, mut info: ScienceInfo) {
        // Ensure internal identifier matches canonical hash if one was not provided
        if info.science == SCIENCE_INVALID {
            info.science = NameKeyGenerator::name_to_key(&info.name) as ScienceType;
        }

        // Calculate root sciences
        let mut roots = Vec::new();
        info.add_root_sciences(&mut roots, self);
        info.root_sciences = roots;

        // Add to mappings
        let key = NameKeyGenerator::name_to_key(&info.name);
        self.name_to_science.insert(key, info.science);
        self.sciences.insert(info.science, info);
    }

    /// Merge or insert a science definition originating from INI data.
    pub fn ingest_definition(&mut self, mut def: ScienceDefinition) {
        let science = NameKeyGenerator::name_to_key(&def.name) as ScienceType;

        let mut info = self
            .sciences
            .remove(&science)
            .unwrap_or_else(|| ScienceInfo::new(science, def.name.clone()));

        if let Some(display_name) = def.display_name.take() {
            info.display_name = display_name;
        }

        if let Some(description) = def.description.take() {
            info.description = description;
        }

        if let Some(prereq_names) = def.prereq_names.take() {
            info.prereq_sciences = prereq_names
                .into_iter()
                .map(|name| NameKeyGenerator::name_to_key(&name) as ScienceType)
                .collect();
        }

        if let Some(cost) = def.cost {
            info.science_purchase_point_cost = cost;
        }

        if let Some(grantable) = def.grantable {
            info.grantable = grantable;
        }

        self.name_to_science
            .insert(NameKeyGenerator::name_to_key(&info.name), science);
        self.sciences.insert(science, info);
    }

    /// Recompute root-science caches after bulk updates.
    pub fn rebuild_root_sciences(&mut self) {
        let keys: Vec<ScienceType> = self.sciences.keys().copied().collect();
        for science in keys {
            let mut roots = Vec::new();

            if let Some(info) = self.sciences.get(&science) {
                info.add_root_sciences(&mut roots, self);
            }

            roots.sort();
            roots.dedup();

            if let Some(info) = self.sciences.get_mut(&science) {
                info.root_sciences = roots;
            }
        }
    }

    /// Load science definitions from the provided INI paths.
    pub fn load_from_paths<P>(
        &mut self,
        base_paths: &[P],
        override_paths: &[P],
    ) -> Result<(), ScienceLoadError>
    where
        P: AsRef<Path>,
    {
        let mut definitions: HashMap<ScienceType, ScienceDefinition> = HashMap::new();

        for path in base_paths {
            ingest_science_file(path.as_ref(), false, &mut definitions)?;
        }

        for path in override_paths {
            ingest_science_file(path.as_ref(), true, &mut definitions)?;
        }

        self.init();

        for definition in definitions.into_values() {
            self.ingest_definition(definition);
        }

        self.rebuild_root_sciences();
        Ok(())
    }

    /// Get science purchase cost
    ///
    /// Matches C++ Science.cpp lines 213-224: getSciencePurchaseCost
    /// Returns 0 if science is invalid or not purchasable.
    pub fn get_science_purchase_cost(&self, science: ScienceType) -> i32 {
        self.sciences
            .get(&science)
            .map(|info| info.science_purchase_point_cost)
            .unwrap_or(0)
    }

    /// Check if science is grantable
    ///
    /// Matches C++ Science.cpp lines 227-238: isScienceGrantable
    /// Returns whether this science can be granted to a player.
    pub fn is_science_grantable(&self, science: ScienceType) -> bool {
        self.sciences
            .get(&science)
            .map(|info| info.grantable)
            .unwrap_or(false)
    }

    /// Get name and description for a science
    ///
    /// Matches C++ Science.cpp lines 241-254: getNameAndDescription
    /// Returns display name and description for UI display.
    pub fn get_name_and_description(&self, science: ScienceType) -> Option<(String, String)> {
        self.sciences
            .get(&science)
            .map(|info| (info.display_name.clone(), info.description.clone()))
    }

    /// Check if player has prerequisites for a science
    ///
    /// Matches C++ Science.cpp lines 257-275: playerHasPrereqsForScience
    /// Verifies player owns ALL direct prerequisites for this science.
    pub fn player_has_prereqs_for_science<A>(&self, player: &A, science: ScienceType) -> bool
    where
        A: ScienceAccess,
    {
        if let Some(info) = self.sciences.get(&science) {
            // C++ Science.cpp:262-268 - Check each prereq
            for &prereq in &info.prereq_sciences {
                if !player.has_science(prereq) {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// Check if player has root prerequisites for a science
    ///
    /// Matches C++ Science.cpp lines 278-296: playerHasRootPrereqsForScience
    /// Verifies player owns ALL ultimate root prerequisites.
    /// This is used to determine if science is "potentially purchasable" vs "never accessible".
    pub fn player_has_root_prereqs_for_science(
        &self,
        player: &impl ScienceAccess,
        science: ScienceType,
    ) -> bool {
        if let Some(info) = self.sciences.get(&science) {
            // C++ Science.cpp:283-289 - Check each root prereq
            for &root in &info.root_sciences {
                if !player.has_science(root) {
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    /// Get purchasable sciences for a player
    ///
    /// Matches C++ Science.cpp lines 299-329: getPurchasableSciences
    /// Returns two lists:
    /// - purchasable: Sciences player can buy RIGHT NOW (has prereqs)
    /// - potentially_purchasable: Sciences player MIGHT buy later (has root prereqs but missing intermediate)
    pub fn get_purchasable_sciences(
        &self,
        player: &impl ScienceAccess,
    ) -> (Vec<ScienceType>, Vec<ScienceType>) {
        let mut purchasable = Vec::new();
        let mut potentially_purchasable = Vec::new();

        // C++ Science.cpp:305-328 - Iterate all sciences
        for (&science, info) in &self.sciences {
            // C++ Science.cpp:309-313 - Skip if not purchasable (cost == 0)
            if info.science_purchase_point_cost == 0 {
                continue;
            }

            // C++ Science.cpp:315-318 - Skip if already owned
            if player.has_science(science) {
                continue;
            }

            // C++ Science.cpp:320-327 - Categorize by prereqs
            if self.player_has_prereqs_for_science(player, science) {
                purchasable.push(science);
            } else if self.player_has_root_prereqs_for_science(player, science) {
                potentially_purchasable.push(science);
            }
        }

        (purchasable, potentially_purchasable)
    }

    /// Check if a science type is valid
    ///
    /// Matches C++ Science.cpp lines 348-352: isValidScience
    /// Returns true if this science is defined in the store.
    pub fn is_valid_science(&self, science: ScienceType) -> bool {
        self.sciences.contains_key(&science)
    }

    /// Get all science names (for WorldBuilder)
    ///
    /// Matches C++ Science.cpp lines 89-99: friend_getScienceNames
    /// Returns list of all known science names. Used by WorldBuilder for editing.
    /// NOTE: Don't use this in RTS runtime code!
    pub fn get_science_names(&self) -> Vec<String> {
        self.sciences
            .values()
            .map(|info| info.name.clone())
            .collect()
    }

    /// Get number of sciences
    pub fn get_science_count(&self) -> usize {
        self.sciences.len()
    }

    /// Returns true when no sciences have been registered yet.
    pub fn is_empty(&self) -> bool {
        self.sciences.is_empty()
    }

    /// Iterates over the science definitions currently resident in the store.
    pub fn iter(&self) -> impl Iterator<Item = (&ScienceType, &ScienceInfo)> {
        self.sciences.iter()
    }

    /// Parse science definition from INI file
    pub fn parse_science_definition(&mut self, ini: &mut INI) -> INIResult<()> {
        let tokens = ini.get_line_tokens();
        let name = tokens
            .iter()
            .skip(1)
            .find(|token| **token != "=")
            .ok_or(INIError::InvalidData)?
            .to_string();

        let mut definition = ScienceDefinition {
            name,
            ..Default::default()
        };

        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }

            let line_tokens = ini.get_line_tokens();
            if line_tokens.is_empty() {
                continue;
            }

            let key = line_tokens[0];
            if key.eq_ignore_ascii_case("End") {
                break;
            }

            let mut value_tokens: Vec<&str> = line_tokens.iter().skip(1).copied().collect();
            value_tokens.retain(|token| *token != "=");

            let value = value_tokens.join(" ");
            if value.is_empty() {
                continue;
            }

            match key.to_ascii_lowercase().as_str() {
                "prerequisitesciences" => {
                    let prereqs = value
                        .split_whitespace()
                        .filter(|token| !token.is_empty())
                        .map(|token| token.to_string())
                        .collect::<Vec<_>>();
                    definition.prereq_names = Some(prereqs);
                }
                "sciencepurchasepointcost" => {
                    definition.cost =
                        Some(value.parse::<i32>().map_err(|_| INIError::InvalidData)?);
                }
                "isgrantable" => {
                    definition.grantable = Some(parse_bool_token(&value));
                }
                "displayname" => {
                    definition.display_name = Some(value);
                }
                "description" => {
                    definition.description = Some(value);
                }
                _ => {
                    debug!("ScienceStore: unhandled science token '{}' in INI", key);
                }
            }
        }

        self.ingest_definition(definition);
        self.rebuild_root_sciences();
        Ok(())
    }
}

/// INI block entry point for Science definitions.
pub fn parse_science_definition_block(ini: &mut INI) -> INIResult<()> {
    if get_science_store().is_none() {
        init_science_store();
    }

    let mut store = get_science_store_mut().ok_or(INIError::UnknownError)?;
    store.parse_science_definition(ini)
}

fn ingest_science_file(
    path: &Path,
    is_override: bool,
    definitions: &mut HashMap<ScienceType, ScienceDefinition>,
) -> Result<(), ScienceLoadError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            warn!(
                "ScienceStore: optional science INI '{:?}' not found; skipping",
                path
            );
            return Ok(());
        }
        Err(err) => {
            return Err(ScienceLoadError::Io {
                path: path.to_path_buf(),
                source: err,
            })
        }
    };

    trace!("ScienceStore: loading sciences from {:?}", path);

    let mut current: Option<ScienceDefinition> = None;
    let mut reader = BufReader::new(file);
    let mut line = String::new();
    let mut line_no = 0usize;

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                line_no += 1;
                let trimmed = strip_comments(&line);
                if trimmed.is_empty() {
                    continue;
                }

                let mut parts = trimmed.split_whitespace();
                let keyword = parts.next().unwrap();

                if keyword.eq_ignore_ascii_case("Science") {
                    let name = parts.collect::<Vec<_>>().join(" ").trim().to_string();
                    if name.is_empty() {
                        return Err(ScienceLoadError::Parse {
                            path: path.to_path_buf(),
                            line: line_no,
                            message: "Science block missing name".to_string(),
                        });
                    }

                    if let Some(def) = current.take() {
                        merge_science_definition(path, def, definitions, is_override);
                    }

                    current = Some(ScienceDefinition {
                        name,
                        ..Default::default()
                    });
                    continue;
                }

                if keyword.eq_ignore_ascii_case("End") {
                    if let Some(def) = current.take() {
                        merge_science_definition(path, def, definitions, is_override);
                    } else {
                        warn!(
                            "ScienceStore: stray 'End' found in {:?} at line {}",
                            path, line_no
                        );
                    }
                    continue;
                }

                let (key, value) = if let Some((key, value)) = trimmed.split_once('=') {
                    (key.trim(), value.trim())
                } else {
                    warn!(
                        "ScienceStore: ignoring malformed line in {:?} at {}: {}",
                        path, line_no, trimmed
                    );
                    continue;
                };

                if let Some(def) = current.as_mut() {
                    match key.to_lowercase().as_str() {
                        "prerequisitesciences" => {
                            let prereqs = value
                                .split_whitespace()
                                .filter(|token| !token.is_empty())
                                .map(|token| token.to_string())
                                .collect::<Vec<_>>();
                            def.prereq_names = Some(prereqs);
                        }
                        "sciencepurchasepointcost" => match value.parse::<i32>() {
                            Ok(cost) => def.cost = Some(cost),
                            Err(_) => {
                                return Err(ScienceLoadError::Parse {
                                    path: path.to_path_buf(),
                                    line: line_no,
                                    message: format!(
                                        "Invalid SciencePurchasePointCost value '{}'",
                                        value
                                    ),
                                });
                            }
                        },
                        "isgrantable" => {
                            def.grantable = Some(parse_bool_token(value));
                        }
                        "displayname" => {
                            def.display_name = Some(value.to_string());
                        }
                        "description" => {
                            def.description = Some(value.to_string());
                        }
                        _ => {
                            debug!(
                                "ScienceStore: unhandled science token '{}' in {:?} at line {}",
                                key, path, line_no
                            );
                        }
                    }
                } else {
                    warn!(
                        "ScienceStore: encountered '{}' outside of a Science block in {:?} at line {}",
                        key, path, line_no
                    );
                }
            }
            Err(err) => {
                return Err(ScienceLoadError::Io {
                    path: path.to_path_buf(),
                    source: err,
                });
            }
        }
    }

    if let Some(def) = current.take() {
        merge_science_definition(path, def, definitions, is_override);
    }

    Ok(())
}

fn merge_science_definition(
    path: &Path,
    mut definition: ScienceDefinition,
    definitions: &mut HashMap<ScienceType, ScienceDefinition>,
    is_override: bool,
) {
    // Ensure base definitions provide defaults when fields are omitted.
    if !is_override {
        definition.display_name.get_or_insert_with(Default::default);
        definition.description.get_or_insert_with(Default::default);
        definition.cost.get_or_insert(0);
        definition.grantable.get_or_insert(true);
        definition.prereq_names.get_or_insert_with(Vec::new);
    }

    let science_name = definition.name.clone();
    let science = NameKeyGenerator::name_to_key(&definition.name) as ScienceType;

    match definitions.entry(science) {
        Entry::Occupied(mut entry) => {
            let existing = entry.get_mut();

            if let Some(display_name) = definition.display_name.take() {
                existing.display_name = Some(display_name);
            }

            if let Some(description) = definition.description.take() {
                existing.description = Some(description);
            }

            if let Some(prereqs) = definition.prereq_names.take() {
                existing.prereq_names = Some(prereqs);
            }

            if let Some(cost) = definition.cost.take() {
                existing.cost = Some(cost);
            }

            if let Some(grantable) = definition.grantable.take() {
                existing.grantable = Some(grantable);
            }
        }
        Entry::Vacant(entry) => {
            entry.insert(definition);
        }
    }

    trace!(
        "ScienceStore: merged science '{}' from {:?}",
        science_name,
        path
    );
}

fn strip_comments(line: &str) -> &str {
    let mut end = line.len();

    if let Some(idx) = line.find("//") {
        end = end.min(idx);
    }

    if let Some(idx) = line.find(';') {
        end = end.min(idx);
    }

    line[..end].trim()
}

fn parse_bool_token(token: &str) -> bool {
    match token.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "y" => true,
        "0" | "false" | "no" | "n" => false,
        _ => false,
    }
}

impl Default for ScienceStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global science store instance
///
/// In the original C++, this was a global pointer.
/// In Rust, we'd typically use a different pattern, but
/// this maintains API compatibility.
static SCIENCE_STORE: OnceCell<RwLock<ScienceStore>> = OnceCell::new();

/// Get the global science store
pub fn get_science_store() -> Option<RwLockReadGuard<'static, ScienceStore>> {
    SCIENCE_STORE
        .get()
        .map(|store| store.read().expect("ScienceStore poisoned"))
}

/// Initialize the global science store
pub fn init_science_store() {
    if SCIENCE_STORE.get().is_none() {
        let _ = SCIENCE_STORE.set(RwLock::new(ScienceStore::new()));
    } else if let Some(store) = SCIENCE_STORE.get() {
        if let Ok(mut guard) = store.write() {
            *guard = ScienceStore::new();
        }
    }
}

/// Get a mutable guard to the global science store.
pub fn get_science_store_mut() -> Option<RwLockWriteGuard<'static, ScienceStore>> {
    SCIENCE_STORE
        .get()
        .map(|store| store.write().expect("ScienceStore poisoned"))
}

/// Subsystem responsible for loading and maintaining the global science store.
pub struct ScienceSubsystem {
    state: SubsystemState,
    base_paths: Vec<PathBuf>,
    override_paths: Vec<PathBuf>,
}

impl ScienceSubsystem {
    /// Create a subsystem with the default Generals science INI paths.
    pub fn new() -> Self {
        Self::with_paths(
            vec![PathBuf::from("Data/INI/Default/Science.ini")],
            vec![PathBuf::from("Data/INI/Science.ini")],
        )
    }

    /// Create a subsystem with explicit base and override paths.
    pub fn with_paths(base_paths: Vec<PathBuf>, override_paths: Vec<PathBuf>) -> Self {
        Self {
            state: SubsystemState::Uninitialized,
            base_paths,
            override_paths,
        }
    }

    /// Convenience helper for registering with a subsystem manager.
    pub fn descriptor() -> SubsystemDescriptor {
        SubsystemDescriptor::new(Box::new(Self::new()))
    }
}

impl Default for ScienceSubsystem {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for ScienceSubsystem {
    fn name(&self) -> &str {
        "ScienceStore"
    }

    fn init(&mut self) -> SubsystemResult<()> {
        self.state = SubsystemState::Initializing;
        init_science_store();

        let mut store = get_science_store_mut().ok_or_else(|| {
            SubsystemError::OperationFailed(
                "ScienceStore global instance was not initialised".to_string(),
            )
        })?;

        store.load_from_paths(&self.base_paths, &self.override_paths)?;
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        if let Some(mut store) = get_science_store_mut() {
            store.reset();
        }
        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn state(&self) -> SubsystemState {
        self.state
    }

    fn as_any(&self) -> &(dyn Any + Send + Sync) {
        self
    }

    fn as_any_mut(&mut self) -> &mut (dyn Any + Send + Sync) {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn test_science_store_creation() {
        let store = ScienceStore::new();
        assert_eq!(store.get_science_count(), 0);
        assert!(!store.is_valid_science(123));
    }

    #[test]
    fn test_science_info_creation() {
        let info = ScienceInfo::new(100, "TestScience".to_string());

        assert_eq!(info.science, 100);
        assert_eq!(info.name, "TestScience");
        assert_eq!(info.science_purchase_point_cost, 0);
        // Fixed: C++ default is grantable = true (Science.h:46)
        assert!(info.grantable);
        assert!(info.prereq_sciences.is_empty());
    }

    #[test]
    fn test_science_store_operations() {
        let mut store = ScienceStore::new();

        let info = ScienceInfo::new(100, "TestScience".to_string());
        store.add_science(info);

        assert_eq!(store.get_science_count(), 1);
        assert!(store.is_valid_science(100));
        assert!(!store.is_valid_science(200));

        assert_eq!(store.get_science_from_internal_name("TestScience"), 100);
        assert_eq!(store.get_internal_name_for_science(100), "TestScience");

        let science_info = store.find_science_info(100);
        assert!(science_info.is_some());
        assert_eq!(science_info.unwrap().name, "TestScience");
    }

    #[test]
    fn test_science_costs_and_properties() {
        let mut store = ScienceStore::new();

        let mut info = ScienceInfo::new(100, "ExpensiveScience".to_string());
        info.science_purchase_point_cost = 50;
        info.grantable = true;

        store.add_science(info);

        assert_eq!(store.get_science_purchase_cost(100), 50);
        assert!(store.is_science_grantable(100));

        // Test invalid science
        assert_eq!(store.get_science_purchase_cost(999), 0);
        assert!(!store.is_science_grantable(999));
    }

    /// Test prerequisite chain validation
    /// Matches C++ behavior from Science.cpp:257-275
    #[test]
    fn test_prerequisite_chains() {
        let mut store = ScienceStore::new();

        // Create a prerequisite chain: Base -> Mid -> Advanced
        let base = ScienceInfo::new(1, "SCIENCE_BASE");
        let mut mid = ScienceInfo::new(2, "SCIENCE_MID");
        mid.prereq_sciences = vec![1];
        let mut advanced = ScienceInfo::new(3, "SCIENCE_ADVANCED");
        advanced.prereq_sciences = vec![2];

        store.add_science(base);
        store.add_science(mid);
        store.add_science(advanced);
        store.rebuild_root_sciences();

        // Create a mock player with base science
        struct MockPlayer {
            sciences: HashSet<ScienceType>,
        }
        impl ScienceAccess for MockPlayer {
            fn has_science(&self, science: ScienceType) -> bool {
                self.sciences.contains(&science)
            }
        }

        let mut player = MockPlayer {
            sciences: HashSet::new(),
        };

        // Player has no sciences - can't get any
        assert!(!store.player_has_prereqs_for_science(&player, 1));
        assert!(!store.player_has_prereqs_for_science(&player, 2));
        assert!(!store.player_has_prereqs_for_science(&player, 3));

        // Player gets base - can now get mid (base has no prereqs)
        player.sciences.insert(1);
        assert!(store.player_has_prereqs_for_science(&player, 2));
        assert!(!store.player_has_prereqs_for_science(&player, 3));

        // Player gets mid - can now get advanced
        player.sciences.insert(2);
        assert!(store.player_has_prereqs_for_science(&player, 3));
    }

    /// Test root science calculation
    /// Matches C++ behavior from Science.cpp:102-120
    #[test]
    fn test_root_sciences() {
        let mut store = ScienceStore::new();

        // Create complex tree:
        //   Root1    Root2
        //     |        |
        //   Mid1     Mid2
        //     |        |
        //     +---+----+
        //         |
        //      Advanced

        let root1 = ScienceInfo::new(1, "SCIENCE_ROOT1");
        let root2 = ScienceInfo::new(2, "SCIENCE_ROOT2");

        let mut mid1 = ScienceInfo::new(3, "SCIENCE_MID1");
        mid1.prereq_sciences = vec![1];

        let mut mid2 = ScienceInfo::new(4, "SCIENCE_MID2");
        mid2.prereq_sciences = vec![2];

        let mut advanced = ScienceInfo::new(5, "SCIENCE_ADVANCED");
        advanced.prereq_sciences = vec![3, 4];

        store.add_science(root1);
        store.add_science(root2);
        store.add_science(mid1);
        store.add_science(mid2);
        store.add_science(advanced);
        store.rebuild_root_sciences();

        // Verify root sciences were calculated correctly
        let adv_info = store.find_science_info(5).unwrap();
        assert_eq!(adv_info.root_sciences.len(), 2);
        assert!(adv_info.root_sciences.contains(&1));
        assert!(adv_info.root_sciences.contains(&2));

        // Mid sciences should have single root
        let mid1_info = store.find_science_info(3).unwrap();
        assert_eq!(mid1_info.root_sciences.len(), 1);
        assert!(mid1_info.root_sciences.contains(&1));

        let mid2_info = store.find_science_info(4).unwrap();
        assert_eq!(mid2_info.root_sciences.len(), 1);
        assert!(mid2_info.root_sciences.contains(&2));
    }

    /// Test purchasable science categorization
    /// Matches C++ behavior from Science.cpp:299-329
    #[test]
    fn test_get_purchasable_sciences() {
        let mut store = ScienceStore::new();

        // Create science tree with costs
        let mut root = ScienceInfo::new(1, "SCIENCE_ROOT");
        root.science_purchase_point_cost = 0; // Not purchasable

        let mut tier1a = ScienceInfo::new(2, "SCIENCE_T1A");
        tier1a.prereq_sciences = vec![1];
        tier1a.science_purchase_point_cost = 1;

        let mut tier1b = ScienceInfo::new(3, "SCIENCE_T1B");
        tier1b.prereq_sciences = vec![1];
        tier1b.science_purchase_point_cost = 1;

        let mut tier2 = ScienceInfo::new(4, "SCIENCE_T2");
        tier2.prereq_sciences = vec![2, 3];
        tier2.science_purchase_point_cost = 2;

        store.add_science(root);
        store.add_science(tier1a);
        store.add_science(tier1b);
        store.add_science(tier2);
        store.rebuild_root_sciences();

        struct MockPlayer {
            sciences: HashSet<ScienceType>,
        }
        impl ScienceAccess for MockPlayer {
            fn has_science(&self, science: ScienceType) -> bool {
                self.sciences.contains(&science)
            }
        }

        // Player starts with root only
        let mut player = MockPlayer {
            sciences: HashSet::from([1]),
        };

        let (purchasable, potentially) = store.get_purchasable_sciences(&player);

        // Can purchase tier1 sciences immediately
        assert_eq!(purchasable.len(), 2);
        assert!(purchasable.contains(&2));
        assert!(purchasable.contains(&3));

        // Tier2 is potentially purchasable (has root but missing intermediate)
        assert_eq!(potentially.len(), 1);
        assert!(potentially.contains(&4));

        // Player gets one tier1 science
        player.sciences.insert(2);
        let (purchasable, potentially) = store.get_purchasable_sciences(&player);

        // Can still buy the other tier1
        assert_eq!(purchasable.len(), 1);
        assert!(purchasable.contains(&3));

        // Tier2 still potentially purchasable
        assert_eq!(potentially.len(), 1);

        // Player gets both tier1 sciences
        player.sciences.insert(3);
        let (purchasable, potentially) = store.get_purchasable_sciences(&player);

        // Now can buy tier2
        assert_eq!(purchasable.len(), 1);
        assert!(purchasable.contains(&4));
        assert_eq!(potentially.len(), 0);
    }

    /// Test science name lookups
    /// Matches C++ behavior from Science.cpp:67-83
    #[test]
    fn test_science_name_lookups() {
        let mut store = ScienceStore::new();

        let info = ScienceInfo::new(SCIENCE_INVALID, "SCIENCE_PaladinTank");
        store.add_science(info);

        // Test name -> type lookup
        let science = store.get_science_from_internal_name("SCIENCE_PaladinTank");
        assert_ne!(science, SCIENCE_INVALID);

        // Test type -> name lookup
        let name = store.get_internal_name_for_science(science);
        assert_eq!(name, "SCIENCE_PaladinTank");

        // Test empty/invalid cases
        assert_eq!(store.get_science_from_internal_name(""), SCIENCE_INVALID);
        assert_eq!(
            store.get_science_from_internal_name("NONEXISTENT"),
            SCIENCE_INVALID
        );
        assert_eq!(store.get_internal_name_for_science(SCIENCE_INVALID), "");
    }

    /// Test display names and descriptions
    /// Matches C++ behavior from Science.cpp:241-254
    #[test]
    fn test_science_display_info() {
        let mut store = ScienceStore::new();

        let mut info = ScienceInfo::new(100, "SCIENCE_TEST");
        info.display_name = "Paladin Tank".to_string();
        info.description = "Heavy armor support vehicle".to_string();

        store.add_science(info);

        let (name, desc) = store.get_name_and_description(100).unwrap();
        assert_eq!(name, "Paladin Tank");
        assert_eq!(desc, "Heavy armor support vehicle");

        // Test invalid science
        assert!(store.get_name_and_description(999).is_none());
    }

    /// Test INI parsing with multiple prereqs
    /// Verifies the INI file format from Science.ini
    #[test]
    fn test_science_definition_parsing() {
        let mut store = ScienceStore::new();

        // Simulate parsing Science.ini format:
        // Science SCIENCE_Paradrop2
        //   PrerequisiteSciences = SCIENCE_Paradrop1 SCIENCE_Rank3
        //   SciencePurchasePointCost = 1
        //   IsGrantable = Yes

        let mut def = ScienceDefinition {
            name: "SCIENCE_Paradrop2".to_string(),
            display_name: Some("Paradrop Level 2".to_string()),
            description: Some("Enhanced paradrop capability".to_string()),
            prereq_names: Some(vec![
                "SCIENCE_Paradrop1".to_string(),
                "SCIENCE_Rank3".to_string(),
            ]),
            cost: Some(1),
            grantable: Some(true),
        };

        store.ingest_definition(def);

        let science = store.get_science_from_internal_name("SCIENCE_Paradrop2");
        assert_ne!(science, SCIENCE_INVALID);

        let info = store.find_science_info(science).unwrap();
        assert_eq!(info.science_purchase_point_cost, 1);
        assert!(info.grantable);
        assert_eq!(info.prereq_sciences.len(), 2);
    }
}
