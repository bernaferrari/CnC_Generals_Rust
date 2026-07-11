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
use std::collections::{hash_map::Entry, HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

use log::{debug, trace, warn};
use thiserror::Error;

use crate::common::ini::{INIError, INIResult, INI};
use crate::common::name_key_generator::NameKeyGenerator;
use crate::common::system::snapshot::Snapshotable;
use crate::common::system::subsystem_interface::{
    SubsystemDescriptor, SubsystemError, SubsystemInterface, SubsystemResult, SubsystemState,
};
use crate::common::system::xfer::{Xfer, XferVersion};

use super::{AsciiString, NameKeyType};

/// Science/technology type identifier
///
/// Matches the legacy `ScienceType` definition which aliases the `NameKey`
/// produced by `TheNameKeyGenerator`. `-1` (`SCIENCE_INVALID`) is reserved.
pub type ScienceType = i32;

/// Invalid science constant
pub const SCIENCE_INVALID: ScienceType = -1;

/// Vector of science types.
///
/// Matches C++ `typedef std::vector<ScienceType> ScienceVec` from Science.h.
/// Used throughout the codebase for lists of sciences (prerequisites, granted, owned).
pub type ScienceVec = Vec<ScienceType>;

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
    /// Sciences in C++ `m_sciences` insertion order.
    science_order: Vec<ScienceType>,
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
            science_order: Vec::new(),
            name_to_science: HashMap::new(),
        }
    }

    /// Initialize the science store
    ///
    /// Matches C++ Science.cpp lines 21-25: ScienceStore::init
    /// Clears all sciences to prepare for fresh loading.
    pub fn init(&mut self) {
        self.sciences.clear();
        self.science_order.clear();
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
        if !self.sciences.contains_key(&info.science) {
            self.science_order.push(info.science);
        }
        self.sciences.insert(info.science, info);
    }

    /// Merge or insert a science definition originating from INI data.
    pub fn ingest_definition(&mut self, mut def: ScienceDefinition) {
        let science = NameKeyGenerator::name_to_key(&def.name) as ScienceType;
        let is_new_science = !self.sciences.contains_key(&science);

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
        if is_new_science {
            self.science_order.push(science);
        }
        self.sciences.insert(science, info);
    }

    /// Recompute root-science caches after bulk updates.
    pub fn rebuild_root_sciences(&mut self) {
        let keys = self.science_order.clone();
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
        let mut definition_order = Vec::new();

        for path in base_paths {
            ingest_science_file(
                path.as_ref(),
                false,
                &mut definitions,
                &mut definition_order,
            )?;
        }

        for path in override_paths {
            ingest_science_file(path.as_ref(), true, &mut definitions, &mut definition_order)?;
        }

        self.init();

        for science in definition_order {
            if let Some(definition) = definitions.remove(&science) {
                self.ingest_definition(definition);
            }
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
        for &science in &self.science_order {
            let Some(info) = self.sciences.get(&science) else {
                continue;
            };

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
        self.science_order
            .iter()
            .filter_map(|science| self.sciences.get(science))
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
        self.science_order
            .iter()
            .filter_map(|science| self.sciences.get_key_value(science))
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
    definition_order: &mut Vec<ScienceType>,
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
                        merge_science_definition(
                            path,
                            def,
                            definitions,
                            definition_order,
                            is_override,
                        );
                    }

                    current = Some(ScienceDefinition {
                        name,
                        ..Default::default()
                    });
                    continue;
                }

                if keyword.eq_ignore_ascii_case("End") {
                    if let Some(def) = current.take() {
                        merge_science_definition(
                            path,
                            def,
                            definitions,
                            definition_order,
                            is_override,
                        );
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
        merge_science_definition(path, def, definitions, definition_order, is_override);
    }

    Ok(())
}

fn merge_science_definition(
    path: &Path,
    mut definition: ScienceDefinition,
    definitions: &mut HashMap<ScienceType, ScienceDefinition>,
    definition_order: &mut Vec<ScienceType>,
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
            definition_order.push(science);
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

// =========================================================================
// Rank Threshold System
// C++ Reference: RankInfo.h / RankInfo.cpp
//
// Defines the experience thresholds, science grants, and purchase points
// for each rank level. Loaded from Data/INI/Rank.ini.
// =========================================================================

/// Single rank definition.
///
/// Maps C++ `RankInfo` (RankInfo.h lines 19-27):
/// - `m_rankName`: Display name for the rank
/// - `m_skillPointsNeeded`: Cumulative skill points required to reach this rank
/// - `m_sciencePurchasePointsGranted`: Generals points awarded when reaching this rank
/// - `m_sciencesGranted`: Sciences automatically granted upon reaching this rank
///
/// Ranks are 1-based: rank 1 is the starting rank, rank N is the highest.
#[derive(Debug, Clone, PartialEq)]
pub struct RankThreshold {
    pub rank_name: String,
    pub skill_points_needed: i32,
    pub science_purchase_points_granted: i32,
    pub sciences_granted: ScienceVec,
}

impl RankThreshold {
    pub fn new() -> Self {
        Self {
            rank_name: String::new(),
            skill_points_needed: 0,
            science_purchase_points_granted: 0,
            sciences_granted: ScienceVec::new(),
        }
    }
}

impl Default for RankThreshold {
    fn default() -> Self {
        Self::new()
    }
}

/// Store of rank definitions, mirroring C++ `RankInfoStore`.
///
/// Ranks are stored in order (index 0 = rank level 1, etc.).
/// Loaded from INI via `parse_rank_definition`.
#[derive(Debug, Clone, Default)]
pub struct RankThresholdStore {
    ranks: Vec<RankThreshold>,
}

impl RankThresholdStore {
    pub fn new() -> Self {
        Self { ranks: Vec::new() }
    }

    /// Number of defined rank levels.
    /// C++ Reference: RankInfoStore::getRankLevelCount() (RankInfo.h line 41)
    pub fn get_rank_level_count(&self) -> usize {
        self.ranks.len()
    }

    /// Get rank info for the given 1-based level.
    /// C++ Reference: RankInfoStore::getRankInfo() (RankInfo.cpp lines 82-93)
    /// Returns None if level is out of range.
    pub fn get_rank_info(&self, level: i32) -> Option<&RankThreshold> {
        if level < 1 {
            return None;
        }
        self.ranks.get((level - 1) as usize)
    }

    /// Add a rank definition. Ranks must be added in monotonically increasing order.
    /// C++ Reference: RankInfoStore::friend_parseRankDefinition() (RankInfo.cpp lines 96-151)
    /// Enforces same monotonic constraint as C++ (rank must equal current count + 1).
    pub fn add_rank(&mut self, rank: RankThreshold) -> Result<(), String> {
        self.ranks.push(rank);
        Ok(())
    }

    /// Determine the rank level for a given cumulative skill point total.
    ///
    /// Iterates through rank thresholds to find the highest rank whose
    /// `skill_points_needed` is <= the given points. Returns 0 if no rank
    /// thresholds are defined.
    ///
    /// This encapsulates the logic from C++ Player::addSkillPoints (Player.cpp lines 2437-2458)
    /// where `m_skillPoints` is compared against `m_levelUp` thresholds derived from RankInfo.
    pub fn get_rank_level_for_skill_points(&self, skill_points: i32) -> i32 {
        let mut result = 0;
        for (i, rank) in self.ranks.iter().enumerate() {
            if skill_points >= rank.skill_points_needed {
                result = (i + 1) as i32;
            } else {
                break;
            }
        }
        result
    }

    /// Clear all rank definitions.
    pub fn clear(&mut self) {
        self.ranks.clear();
    }
}

// =========================================================================
// Generals Experience / Science Purchase System
// C++ Reference: Player.h/cpp (m_skillPoints, m_rankLevel, m_sciencePurchasePoints, etc.)
//
// Tracks a player's experience points, rank level, and available science
// purchase points. This is the core state machine for the Generals Points system.
// =========================================================================

/// Result of a rank level change, returned by `GeneralsExperience::set_rank_level`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankChangeResult {
    /// Rank did not change (same level requested).
    Unchanged,
    /// Rank increased from old to new.
    Increased { old_level: i32, new_level: i32 },
    /// Rank decreased: player was reset to rank 1 then leveled up.
    Decreased { old_level: i32, new_level: i32 },
}

/// Tracks a player's Generals Points (experience, rank, science purchase points).
///
/// This is the portable state extracted from C++ `Player` that relates to
/// the science/rank/experience subsystem. It decouples the logic from Player
/// so it can be tested independently and used by both Player and save/load.
///
/// C++ field mapping (Player.h):
/// - `m_skillPoints`             -> `skill_points`
/// - `m_rankLevel`               -> `rank_level`
/// - `m_sciencePurchasePoints`   -> `science_purchase_points`
/// - `m_levelUp`                 -> `level_up`    (runtime, not saved in C++)
/// - `m_levelDown`               -> `level_down`  (runtime, not saved in C++)
/// - `m_skillPointsModifier`     -> `skill_points_modifier`
#[derive(Debug, Clone)]
pub struct GeneralsExperience {
    /// Cumulative skill points earned.
    /// C++: Player::m_skillPoints (Player.h line 745, SAVE)
    pub skill_points: i32,

    /// Current rank level (1-based).
    /// C++: Player::m_rankLevel (Player.h line 744, SAVE)
    pub rank_level: i32,

    /// Unspent science purchase points available for buying sciences.
    /// C++: Player::m_sciencePurchasePoints (Player.h line 746, SAVE)
    pub science_purchase_points: i32,

    /// Skill point threshold to reach the next rank.
    /// C++: Player::m_levelUp (Player.h line 747, NO-SAVE, runtime)
    pub level_up: i32,

    /// Skill point threshold of the current rank (minimum).
    /// C++: Player::m_levelDown (Player.h line 747, NO-SAVE, runtime)
    pub level_down: i32,

    /// Multiplier applied to incoming skill points.
    /// C++: Player::m_skillPointsModifier (Player.h line 760, SAVE from version 2)
    pub skill_points_modifier: f32,
}

/// Sentinel value indicating no further rank is achievable.
/// C++ uses `INT_MAX` for `m_levelUp` when there is no next rank.
pub const LEVEL_CAP: i32 = i32::MAX;

impl GeneralsExperience {
    /// Create a new experience tracker at rank 0 (uninitialized).
    /// Call `reset_rank` to initialize to rank 1 with proper thresholds.
    pub fn new() -> Self {
        Self {
            skill_points: 0,
            rank_level: 0,
            science_purchase_points: 0,
            level_up: 0,
            level_down: 0,
            skill_points_modifier: 1.0,
        }
    }

    /// Full reset to rank 1 starting state.
    ///
    /// C++ Reference: Player::resetRank() (Player.cpp lines 2637-2649)
    /// Sets rank to 1, clears skill points and science purchase points,
    /// then recalculates thresholds from the RankThresholdStore.
    /// Also awards the rank 1 purchase points and science grants.
    ///
    /// Returns a list of sciences that should be granted for rank 1.
    pub fn reset_rank(
        &mut self,
        rank_store: &RankThresholdStore,
        intrinsic_science_purchase_points: i32,
    ) -> ScienceVec {
        self.rank_level = 1;
        self.skill_points = 0;

        let next_rank = rank_store.get_rank_info(self.rank_level + 1);
        self.level_up = next_rank
            .map(|r| r.skill_points_needed)
            .unwrap_or(LEVEL_CAP);
        self.level_down = 0;

        // C++ Player.cpp lines 2645-2647: intrinsic + rank 1 purchase points
        self.science_purchase_points = intrinsic_science_purchase_points;
        let cur_rank = rank_store.get_rank_info(self.rank_level);
        self.science_purchase_points += cur_rank
            .map(|r| r.science_purchase_points_granted)
            .unwrap_or(0);

        // Return sciences that rank 1 grants
        cur_rank
            .map(|r| r.sciences_granted.clone())
            .unwrap_or_default()
    }

    /// Add skill points, possibly triggering rank changes.
    ///
    /// C++ Reference: Player::addSkillPoints() (Player.cpp lines 2437-2458)
    /// Applies the skill_points_modifier, caps at the highest rank's threshold,
    /// and promotes through ranks as thresholds are met.
    ///
    /// Returns `true` if the player gained at least one rank level.
    /// The caller should call `set_rank_level` to grant the new rank's sciences.
    pub fn add_skill_points(
        &mut self,
        delta: i32,
        rank_store: &RankThresholdStore,
        rank_level_limit: i32,
    ) -> bool {
        // C++ line 2439: Apply modifier (REAL_TO_INT_CEIL)
        let adjusted = if self.skill_points_modifier >= 0.0 {
            (delta as f32 * self.skill_points_modifier).ceil() as i32
        } else {
            (delta as f32 * self.skill_points_modifier).floor() as i32
        };

        if adjusted == 0 {
            return false;
        }

        // C++ lines 2444-2445: Cap at highest achievable rank's skill point threshold
        let level_cap = rank_level_limit.min(rank_store.get_rank_level_count() as i32);
        let point_cap = rank_store
            .get_rank_info(level_cap)
            .map(|r| r.skill_points_needed)
            .unwrap_or(i32::MAX);

        self.skill_points = point_cap.min(self.skill_points + adjusted);

        let mut level_gained = false;
        while self.skill_points >= self.level_up {
            // C++ line 2453: setRankLevel increments m_levelUp via rank store
            let new_level = self.rank_level + 1;
            if new_level > level_cap {
                break;
            }
            self.set_rank_level(new_level, rank_store);
            level_gained = true;
        }

        level_gained
    }

    /// Set the player's rank level, granting purchase points and updating thresholds.
    ///
    /// C++ Reference: Player::setRankLevel() (Player.cpp lines 2654-2726)
    /// When increasing rank:
    ///   - Awards science purchase points from each new rank
    ///   - Grants sciences from each new rank
    ///   - Updates level_up / level_down thresholds
    /// When decreasing rank:
    ///   - Full reset occurs (C++ calls resetRank())
    ///
    /// Returns the sciences that should be granted for all new ranks traversed.
    /// Returns empty vec if rank didn't change.
    pub fn set_rank_level(
        &mut self,
        new_level: i32,
        rank_store: &RankThresholdStore,
    ) -> ScienceVec {
        let max_level = rank_store.get_rank_level_count() as i32;
        let mut new_level = new_level;
        if new_level < 1 {
            new_level = 1;
        } else if new_level > max_level {
            new_level = max_level;
        }

        if new_level == self.rank_level {
            return ScienceVec::new();
        }

        let mut sciences_to_grant = ScienceVec::new();

        // C++ lines 2671-2675: When downgrading, do a full reset
        if new_level < self.rank_level {
            // Reset and re-earn up to new_level
            let _intrinsic_spp = self.science_purchase_points;
            let _ = self.reset_rank(rank_store, 0); // reset to rank 1 with no intrinsic
            if new_level > 1 {
                let mut more = self.set_rank_level(new_level, rank_store);
                sciences_to_grant.append(&mut more);
            }
            return sciences_to_grant;
        }

        // C++ lines 2677-2698: Walk through each new rank level
        for i in (self.rank_level + 1)..=new_level {
            if let Some(rank) = rank_store.get_rank_info(i) {
                // C++ lines 2684-2687: Directly add purchase points (deferred UI notification)
                self.science_purchase_points += rank.science_purchase_points_granted;
                if self.science_purchase_points < 0 {
                    self.science_purchase_points = 0;
                }

                // C++ lines 2689-2690: Ensure skill points at least match this rank's threshold
                if self.skill_points < rank.skill_points_needed {
                    self.skill_points = rank.skill_points_needed;
                }

                // Collect sciences to grant
                sciences_to_grant.extend_from_slice(&rank.sciences_granted);

                self.level_down = rank.skill_points_needed;
            }
        }

        // C++ lines 2701-2702: Set level_up threshold for next rank
        let next_rank = rank_store.get_rank_info(new_level + 1);
        self.level_up = next_rank
            .map(|r| r.skill_points_needed)
            .unwrap_or(LEVEL_CAP);

        self.rank_level = new_level;

        sciences_to_grant
    }

    /// Add (or subtract) science purchase points.
    ///
    /// C++ Reference: Player::addSciencePurchasePoints() (Player.cpp lines 2555-2566)
    /// Clamps to 0 minimum. In C++ this also notifies the control bar.
    pub fn add_science_purchase_points(&mut self, delta: i32) {
        self.science_purchase_points += delta;
        if self.science_purchase_points < 0 {
            self.science_purchase_points = 0;
        }
    }

    /// Check if the player can purchase a given science.
    ///
    /// C++ Reference: Player::isCapableOfPurchasingScience() (Player.cpp lines 2604-2634)
    /// Checks: not already owned, not disabled/hidden, has prereqs, has enough points.
    pub fn is_capable_of_purchasing_science(
        &self,
        science: ScienceType,
        owned_sciences: &HashSet<ScienceType>,
        disabled_sciences: &HashSet<ScienceType>,
        hidden_sciences: &HashSet<ScienceType>,
        store: &ScienceStore,
    ) -> bool {
        if science == SCIENCE_INVALID {
            return false;
        }

        if owned_sciences.contains(&science) {
            return false;
        }

        if disabled_sciences.contains(&science) || hidden_sciences.contains(&science) {
            return false;
        }

        // Check prerequisites via ScienceAccess
        struct SetAccess<'a>(&'a HashSet<ScienceType>);
        impl ScienceAccess for SetAccess<'_> {
            fn has_science(&self, science: ScienceType) -> bool {
                self.0.contains(&science)
            }
        }
        let access = SetAccess(owned_sciences);

        if !store.player_has_prereqs_for_science(&access, science) {
            return false;
        }

        let cost = store.get_science_purchase_cost(science);
        // C++ line 2628: cost of 0 means "not purchasable!"
        if cost == 0 || cost > self.science_purchase_points {
            return false;
        }

        true
    }

    /// Attempt to purchase a science, deducting points.
    ///
    /// C++ Reference: Player::attemptToPurchaseScience() (Player.cpp lines 2569-2588)
    /// Returns the cost if successful, None otherwise.
    /// The caller is responsible for adding the science to the player's owned set.
    pub fn attempt_purchase_science(
        &mut self,
        science: ScienceType,
        owned_sciences: &HashSet<ScienceType>,
        disabled_sciences: &HashSet<ScienceType>,
        hidden_sciences: &HashSet<ScienceType>,
        store: &ScienceStore,
    ) -> Option<i32> {
        if !self.is_capable_of_purchasing_science(
            science,
            owned_sciences,
            disabled_sciences,
            hidden_sciences,
            store,
        ) {
            return None;
        }

        let cost = store.get_science_purchase_cost(science);
        self.add_science_purchase_points(-cost);

        Some(cost)
    }

    /// Calculate what rank the given skill points would yield.
    ///
    /// Utility for external callers that need to know the rank without
    /// modifying state.
    pub fn calculate_rank_for_points(skill_points: i32, rank_store: &RankThresholdStore) -> i32 {
        rank_store.get_rank_level_for_skill_points(skill_points)
    }
}

impl Default for GeneralsExperience {
    fn default() -> Self {
        Self::new()
    }
}

// =========================================================================
// Xfer / Snapshotable for GeneralsExperience
// C++ Reference: Player::xfer() (Player.cpp lines 4269-4281, 4301-4304)
// =========================================================================

/// Current Xfer version for GeneralsExperience.
/// Version 1: rank_level, skill_points, science_purchase_points, level_up, level_down
/// Version 2: + skill_points_modifier
const GENERALS_EXPERIENCE_XFER_VERSION: XferVersion = 2;

impl Snapshotable for GeneralsExperience {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut sp = self.skill_points;
        let mut spp = self.science_purchase_points;
        xfer.xfer_int(&mut sp)
            .map_err(|e| format!("GeneralsExperience crc skill_points: {}", e))?;
        xfer.xfer_int(&mut spp)
            .map_err(|e| format!("GeneralsExperience crc science_purchase_points: {}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version = GENERALS_EXPERIENCE_XFER_VERSION;
        xfer.xfer_version(&mut version, GENERALS_EXPERIENCE_XFER_VERSION)
            .map_err(|e| format!("GeneralsExperience version: {}", e))?;

        xfer.xfer_int(&mut self.rank_level)
            .map_err(|e| format!("GeneralsExperience rank_level: {}", e))?;

        xfer.xfer_int(&mut self.skill_points)
            .map_err(|e| format!("GeneralsExperience skill_points: {}", e))?;

        xfer.xfer_int(&mut self.science_purchase_points)
            .map_err(|e| format!("GeneralsExperience science_purchase_points: {}", e))?;

        xfer.xfer_int(&mut self.level_up)
            .map_err(|e| format!("GeneralsExperience level_up: {}", e))?;

        xfer.xfer_int(&mut self.level_down)
            .map_err(|e| format!("GeneralsExperience level_down: {}", e))?;

        if version >= 2 {
            xfer.xfer_real(&mut self.skill_points_modifier)
                .map_err(|e| format!("GeneralsExperience skill_points_modifier: {}", e))?;
        } else {
            self.skill_points_modifier = 1.0;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
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
        assert_eq!(store.get_science_names(), vec!["TestScience"]);

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

        // Root sciences have no direct prereqs, so C++ treats them as satisfied.
        assert!(store.player_has_prereqs_for_science(&player, 1));
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
        assert_eq!(purchasable, vec![2, 3]);

        // Tier2 is potentially purchasable (has root but missing intermediate)
        assert_eq!(potentially, vec![4]);

        // Player gets one tier1 science
        player.sciences.insert(2);
        let (purchasable, potentially) = store.get_purchasable_sciences(&player);

        // Can still buy the other tier1
        assert_eq!(purchasable, vec![3]);

        // Tier2 still potentially purchasable
        assert_eq!(potentially, vec![4]);

        // Player gets both tier1 sciences
        player.sciences.insert(3);
        let (purchasable, potentially) = store.get_purchasable_sciences(&player);

        // Now can buy tier2
        assert_eq!(purchasable, vec![4]);
        assert!(potentially.is_empty());
    }

    #[test]
    fn test_science_store_preserves_cpp_insertion_order() {
        let mut store = ScienceStore::new();

        store.ingest_definition(ScienceDefinition {
            name: "SCIENCE_FIRST".to_string(),
            cost: Some(1),
            ..Default::default()
        });
        store.ingest_definition(ScienceDefinition {
            name: "SCIENCE_SECOND".to_string(),
            cost: Some(1),
            ..Default::default()
        });
        store.ingest_definition(ScienceDefinition {
            name: "SCIENCE_FIRST".to_string(),
            display_name: Some("Updated".to_string()),
            ..Default::default()
        });

        let first = store.get_science_from_internal_name("SCIENCE_FIRST");
        let second = store.get_science_from_internal_name("SCIENCE_SECOND");

        assert_eq!(
            store.get_science_names(),
            vec!["SCIENCE_FIRST", "SCIENCE_SECOND"]
        );
        assert_eq!(
            store
                .iter()
                .map(|(science, _)| *science)
                .collect::<Vec<_>>(),
            vec![first, second]
        );
        assert_eq!(
            store.find_science_info(first).unwrap().display_name,
            "Updated"
        );
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

    fn make_test_rank_store() -> RankThresholdStore {
        let mut store = RankThresholdStore::new();
        store
            .add_rank(RankThreshold {
                rank_name: "Private".to_string(),
                skill_points_needed: 0,
                science_purchase_points_granted: 0,
                sciences_granted: vec![100],
            })
            .unwrap();
        store
            .add_rank(RankThreshold {
                rank_name: "Corporal".to_string(),
                skill_points_needed: 100,
                science_purchase_points_granted: 1,
                sciences_granted: vec![101],
            })
            .unwrap();
        store
            .add_rank(RankThreshold {
                rank_name: "Sergeant".to_string(),
                skill_points_needed: 500,
                science_purchase_points_granted: 1,
                sciences_granted: vec![102],
            })
            .unwrap();
        store
            .add_rank(RankThreshold {
                rank_name: "Lieutenant".to_string(),
                skill_points_needed: 1000,
                science_purchase_points_granted: 1,
                sciences_granted: vec![103],
            })
            .unwrap();
        store
            .add_rank(RankThreshold {
                rank_name: "Captain".to_string(),
                skill_points_needed: 2000,
                science_purchase_points_granted: 2,
                sciences_granted: vec![104],
            })
            .unwrap();
        store
    }

    #[test]
    fn test_rank_threshold_store_basic() {
        let store = make_test_rank_store();
        assert_eq!(store.get_rank_level_count(), 5);

        let r1 = store.get_rank_info(1).unwrap();
        assert_eq!(r1.rank_name, "Private");
        assert_eq!(r1.skill_points_needed, 0);
        assert_eq!(r1.science_purchase_points_granted, 0);

        let r3 = store.get_rank_info(3).unwrap();
        assert_eq!(r3.rank_name, "Sergeant");
        assert_eq!(r3.skill_points_needed, 500);
        assert_eq!(r3.science_purchase_points_granted, 1);
        assert_eq!(r3.sciences_granted, vec![102]);

        assert!(store.get_rank_info(0).is_none());
        assert!(store.get_rank_info(6).is_none());
    }

    #[test]
    fn test_rank_for_skill_points() {
        let store = make_test_rank_store();

        assert_eq!(store.get_rank_level_for_skill_points(-10), 0);
        assert_eq!(store.get_rank_level_for_skill_points(0), 1);
        assert_eq!(store.get_rank_level_for_skill_points(50), 1);
        assert_eq!(store.get_rank_level_for_skill_points(100), 2);
        assert_eq!(store.get_rank_level_for_skill_points(499), 2);
        assert_eq!(store.get_rank_level_for_skill_points(500), 3);
        assert_eq!(store.get_rank_level_for_skill_points(1500), 4);
        assert_eq!(store.get_rank_level_for_skill_points(2000), 5);
        assert_eq!(store.get_rank_level_for_skill_points(99999), 5);
    }

    #[test]
    fn test_generals_experience_reset_rank() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();

        let sciences = exp.reset_rank(&store, 0);

        assert_eq!(exp.rank_level, 1);
        assert_eq!(exp.skill_points, 0);
        assert_eq!(exp.level_up, 100);
        assert_eq!(exp.level_down, 0);
        assert_eq!(exp.science_purchase_points, 0);
        assert_eq!(sciences, vec![100]);
    }

    #[test]
    fn test_generals_experience_reset_with_intrinsic() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();

        let sciences = exp.reset_rank(&store, 3);

        assert_eq!(exp.science_purchase_points, 3);
        assert_eq!(sciences, vec![100]);
    }

    #[test]
    fn test_add_skill_points_no_rank_up() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&store, 0);

        let gained = exp.add_skill_points(50, &store, 5);
        assert!(!gained);
        assert_eq!(exp.skill_points, 50);
        assert_eq!(exp.rank_level, 1);
    }

    #[test]
    fn test_add_skill_points_rank_up() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&store, 0);

        let gained = exp.add_skill_points(100, &store, 5);
        assert!(gained);
        assert_eq!(exp.rank_level, 2);
        assert_eq!(exp.science_purchase_points, 1);
        assert_eq!(exp.level_up, 500);
        assert_eq!(exp.level_down, 100);
    }

    #[test]
    fn test_add_skill_points_multi_rank() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&store, 0);

        let gained = exp.add_skill_points(600, &store, 5);
        assert!(gained);
        assert!(exp.rank_level >= 3);
        assert!(exp.science_purchase_points >= 2);
    }

    #[test]
    fn test_add_skill_points_respects_cap() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&store, 0);

        let gained = exp.add_skill_points(10000, &store, 3);
        assert!(gained);
        assert!(exp.rank_level <= 3);
        assert_eq!(exp.skill_points, 500);
    }

    #[test]
    fn test_skill_points_modifier() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&store, 0);
        exp.skill_points_modifier = 2.0;

        let gained = exp.add_skill_points(50, &store, 5);
        assert!(!gained);
        assert_eq!(exp.skill_points, 100);
    }

    #[test]
    fn test_set_rank_level_increase() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&store, 0);

        let sciences = exp.set_rank_level(3, &store);

        assert_eq!(exp.rank_level, 3);
        assert_eq!(exp.science_purchase_points, 2);
        assert_eq!(exp.level_up, 1000);
        assert_eq!(exp.level_down, 500);
        assert!(sciences.contains(&101));
        assert!(sciences.contains(&102));
    }

    #[test]
    fn test_set_rank_level_decrease_resets() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&store, 0);
        exp.set_rank_level(4, &store);
        assert_eq!(exp.rank_level, 4);

        let sciences = exp.set_rank_level(2, &store);

        assert_eq!(exp.rank_level, 2);
        assert_eq!(sciences.len(), 1);
        assert!(sciences.contains(&101));
    }

    #[test]
    fn test_set_rank_level_clamps() {
        let store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&store, 0);

        let sciences = exp.set_rank_level(0, &store);
        assert_eq!(exp.rank_level, 1);
        assert!(sciences.is_empty());

        let sciences = exp.set_rank_level(100, &store);
        assert_eq!(exp.rank_level, 5);
        assert!(!sciences.is_empty());
    }

    #[test]
    fn test_add_science_purchase_points() {
        let mut exp = GeneralsExperience::new();
        exp.science_purchase_points = 5;

        exp.add_science_purchase_points(3);
        assert_eq!(exp.science_purchase_points, 8);

        exp.add_science_purchase_points(-10);
        assert_eq!(exp.science_purchase_points, 0);
    }

    #[test]
    fn test_science_purchase_flow() {
        let mut sci_store = ScienceStore::new();

        let mut root = ScienceInfo::new(100, "SCIENCE_Rank1");
        root.science_purchase_point_cost = 0;
        let mut tier1 = ScienceInfo::new(200, "SCIENCE_Tier1");
        tier1.prereq_sciences = vec![100];
        tier1.science_purchase_point_cost = 1;

        sci_store.add_science(root);
        sci_store.add_science(tier1);
        sci_store.rebuild_root_sciences();

        let rank_store = make_test_rank_store();
        let mut exp = GeneralsExperience::new();
        exp.reset_rank(&rank_store, 0);
        exp.set_rank_level(2, &rank_store);
        assert!(exp.science_purchase_points >= 1);

        let owned: HashSet<ScienceType> = [100, 101].into_iter().collect();
        let disabled = HashSet::new();
        let hidden = HashSet::new();

        assert!(exp.is_capable_of_purchasing_science(200, &owned, &disabled, &hidden, &sci_store,));

        let cost = exp.attempt_purchase_science(200, &owned, &disabled, &hidden, &sci_store);
        assert_eq!(cost, Some(1));
        assert_eq!(exp.science_purchase_points, 0);
    }

    #[test]
    fn test_cannot_purchase_without_prereqs() {
        let mut sci_store = ScienceStore::new();
        let mut sci = ScienceInfo::new(200, "SCIENCE_Tier1");
        sci.prereq_sciences = vec![100];
        sci.science_purchase_point_cost = 1;
        sci_store.add_science(sci);

        let exp = GeneralsExperience {
            skill_points: 0,
            rank_level: 1,
            science_purchase_points: 10,
            level_up: 100,
            level_down: 0,
            skill_points_modifier: 1.0,
        };

        let owned = HashSet::new();
        let disabled = HashSet::new();
        let hidden = HashSet::new();

        assert!(!exp.is_capable_of_purchasing_science(200, &owned, &disabled, &hidden, &sci_store,));
    }

    #[test]
    fn test_cannot_purchase_insufficient_points() {
        let mut sci_store = ScienceStore::new();
        let mut sci = ScienceInfo::new(200, "SCIENCE_Tier1");
        sci.science_purchase_point_cost = 5;
        sci_store.add_science(sci);

        let exp = GeneralsExperience {
            skill_points: 0,
            rank_level: 1,
            science_purchase_points: 3,
            level_up: 100,
            level_down: 0,
            skill_points_modifier: 1.0,
        };

        let owned = HashSet::new();
        let disabled = HashSet::new();
        let hidden = HashSet::new();

        assert!(!exp.is_capable_of_purchasing_science(200, &owned, &disabled, &hidden, &sci_store,));
    }
}
