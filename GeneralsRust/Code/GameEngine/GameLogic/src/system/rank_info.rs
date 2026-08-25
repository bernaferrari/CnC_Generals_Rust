//! Direct translation of the legacy `RankInfo` subsystem.
//!
//! The original C++ implementation relied on the engine's override-aware
//! memory pools and INI parsing helpers.  Those systems are not yet available
//! in the Rust port, so this module focuses on faithfully modelling the data
//! structures and public API while providing lightweight override handling.

use crate::common::UnicodeString;
use game_engine::common::rts::ScienceType;
use game_engine::common::system::subsystem_interface::{
    SubsystemError, SubsystemInterface, SubsystemResult, SubsystemState,
};
use once_cell::sync::OnceCell;
use std::any::Any;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::time::Duration;

/// Global singleton mirroring `TheRankInfoStore`.
static RANK_INFO_STORE: OnceCell<RwLock<RankInfoStore>> = OnceCell::new();

pub fn the_rank_info_store() -> Option<RwLockReadGuard<'static, RankInfoStore>> {
    sync_rank_store_from_ini();
    RANK_INFO_STORE
        .get()
        .map(|store| store.read().expect("RankInfoStore poisoned"))
}

pub fn the_rank_info_store_mut() -> Option<RwLockWriteGuard<'static, RankInfoStore>> {
    sync_rank_store_from_ini();
    RANK_INFO_STORE
        .get()
        .map(|store| store.write().expect("RankInfoStore poisoned"))
}

pub fn init_global_rank_info_store() {
    let _ = RANK_INFO_STORE.set(RwLock::new(RankInfoStore::default()));
    sync_rank_store_from_ini();
}

fn sync_rank_store_from_ini() {
    init_global_rank_info_store_if_needed();
    let common = game_engine::common::ini::get_rank_info_store();
    if common.is_empty() {
        return;
    }
    let Some(store) = RANK_INFO_STORE.get() else {
        return;
    };
    let Ok(mut dest) = store.write() else {
        return;
    };
    dest.clear();
    let count = common.get_rank_level_count();
    for level in 1..=count {
        let Some(src) = common.get_rank_info(level) else {
            continue;
        };
        let definition = RankDefinition {
            rank: level as usize,
            rank_name: src.rank_name.clone(),
            skill_points_needed: src.skill_points_needed,
            science_purchase_points_granted: src.science_purchase_points_granted as i32,
            sciences_granted: src.sciences_granted.clone(),
            mode: RankDefinitionMode::Create,
        };
        let _ = dest.apply_rank_definition(definition);
    }
}

fn init_global_rank_info_store_if_needed() {
    let _ = RANK_INFO_STORE.set(RwLock::new(RankInfoStore::default()));
}

/// Rank descriptor exported by the subsystem.
#[derive(Debug, Clone, PartialEq)]
pub struct RankInfo {
    pub rank_name: UnicodeString,
    pub skill_points_needed: i32,
    pub science_purchase_points_granted: i32,
    pub sciences_granted: Vec<ScienceType>,
    is_override: bool,
    next_override: Option<Box<RankInfo>>,
}

impl RankInfo {
    /// Construct a rank descriptor using the legacy defaults.
    pub fn new() -> Self {
        Self {
            rank_name: UnicodeString::new(),
            skill_points_needed: 0,
            science_purchase_points_granted: 0,
            sciences_granted: Vec::new(),
            is_override: false,
            next_override: None,
        }
    }

    pub fn mark_as_override(&mut self) {
        self.is_override = true;
    }

    /// Whether this descriptor overrides a base rank definition.
    pub fn is_override(&self) -> bool {
        self.is_override
    }

    pub fn get_final_override(&self) -> &RankInfo {
        match &self.next_override {
            Some(next) => next.get_final_override(),
            None => self,
        }
    }

    fn get_final_override_mut(&mut self) -> &mut RankInfo {
        if self.next_override.is_some() {
            self.next_override
                .as_mut()
                .unwrap()
                .get_final_override_mut()
        } else {
            self
        }
    }

    pub fn delete_overrides(&mut self) {
        self.next_override = None;
    }
}

impl Default for RankInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Load-mode flag indicating whether the incoming definition is a new entry or an override.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankDefinitionMode {
    /// Create a brand new rank (load type `INI_LOAD_NORMAL` in the original code).
    Create,
    /// Override an existing rank definition (`INI_LOAD_CREATE_OVERRIDES`).
    Override,
}

/// Data produced by INI parsing for a single rank definition.
#[derive(Debug, Clone, PartialEq)]
pub struct RankDefinition {
    pub rank: usize,
    pub rank_name: UnicodeString,
    pub skill_points_needed: i32,
    pub science_purchase_points_granted: i32,
    pub sciences_granted: Vec<ScienceType>,
    pub mode: RankDefinitionMode,
}

impl RankDefinition {
    /// Convenience constructor mirroring the C++ parsing workflow.
    pub fn new(rank: usize, mode: RankDefinitionMode) -> Self {
        Self {
            rank,
            rank_name: UnicodeString::new(),
            skill_points_needed: 0,
            science_purchase_points_granted: 0,
            sciences_granted: Vec::new(),
            mode,
        }
    }
}

/// Rank subsystem responsible for storing all defined ranks.
#[derive(Debug)]
pub struct RankInfoStore {
    rank_infos: Vec<RankInfo>,
    state: SubsystemState,
}

impl RankInfoStore {
    /// Create a fresh store with no ranks defined.
    pub fn new() -> Self {
        Self {
            rank_infos: Vec::new(),
            state: SubsystemState::Uninitialized,
        }
    }

    /// Reset the store, removing any overrides and definitions.
    pub fn clear(&mut self) {
        self.rank_infos.clear();
    }

    /// Number of rank levels (1-based for APIs that mirror the C++ version).
    pub fn get_rank_level_count(&self) -> usize {
        self.rank_infos.len()
    }

    pub fn get_rank_info(&self, level: usize) -> Option<&RankInfo> {
        if level == 0 {
            return None;
        }
        self.rank_infos
            .get(level - 1)
            .map(RankInfo::get_final_override)
    }

    /// Apply a rank definition generated from INI parsing.
    ///
    /// This mirrors `RankInfoStore::friend_parseRankDefinition` from the C++ sources,
    /// enforcing the same monotonic creation rule and update semantics.
    pub fn apply_rank_definition(&mut self, definition: RankDefinition) -> Result<(), String> {
        match definition.mode {
            RankDefinitionMode::Create => self.apply_new_rank(definition),
            RankDefinitionMode::Override => self.apply_override(definition),
        }
    }

    fn apply_new_rank(&mut self, definition: RankDefinition) -> Result<(), String> {
        if definition.rank != self.rank_infos.len() + 1 {
            return Err(format!(
                "Ranks must increase monotonically: expected next rank {}, received {}",
                self.rank_infos.len() + 1,
                definition.rank
            ));
        }

        let mut info = RankInfo {
            rank_name: definition.rank_name,
            skill_points_needed: definition.skill_points_needed,
            science_purchase_points_granted: definition.science_purchase_points_granted,
            sciences_granted: definition.sciences_granted,
            is_override: false,
            next_override: None,
        };
        info.is_override = false;
        self.rank_infos.push(info);
        Ok(())
    }

    fn apply_override(&mut self, definition: RankDefinition) -> Result<(), String> {
        if definition.rank == 0 || definition.rank > self.rank_infos.len() {
            return Err(format!(
                "Rank {} not found when applying override",
                definition.rank
            ));
        }

        let index = definition.rank - 1;
        let mut stacked = self.rank_infos[index].get_final_override().clone();
        stacked.next_override = None;
        stacked.rank_name = definition.rank_name;
        stacked.skill_points_needed = definition.skill_points_needed;
        stacked.science_purchase_points_granted = definition.science_purchase_points_granted;
        stacked.sciences_granted = definition.sciences_granted;
        stacked.mark_as_override();
        self.rank_infos[index]
            .get_final_override_mut()
            .next_override = Some(Box::new(stacked));
        Ok(())
    }
}

impl Default for RankInfoStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for RankInfoStore {
    fn name(&self) -> &str {
        "RankInfoStore"
    }

    fn init(&mut self) -> SubsystemResult<()> {
        self.clear();
        self.state = SubsystemState::Running;
        Ok(())
    }

    fn update(&mut self, _delta_time: Duration) -> SubsystemResult<()> {
        Ok(())
    }

    fn shutdown(&mut self) -> SubsystemResult<()> {
        self.clear();
        self.state = SubsystemState::Shutdown;
        Ok(())
    }

    fn state(&self) -> SubsystemState {
        self.state
    }

    fn reset(&mut self) -> SubsystemResult<()> {
        for info in &mut self.rank_infos {
            info.delete_overrides();
        }
        Ok(())
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

    #[test]
    fn create_rank_sequence() {
        let mut store = RankInfoStore::new();
        store
            .apply_rank_definition(RankDefinition {
                rank: 1,
                rank_name: UnicodeString::from("Rank 1"),
                skill_points_needed: 100,
                science_purchase_points_granted: 1,
                sciences_granted: vec![1],
                mode: RankDefinitionMode::Create,
            })
            .expect("rank 1");
        store
            .apply_rank_definition(RankDefinition {
                rank: 2,
                rank_name: UnicodeString::from("Rank 2"),
                skill_points_needed: 200,
                science_purchase_points_granted: 2,
                sciences_granted: vec![1, 2],
                mode: RankDefinitionMode::Create,
            })
            .expect("rank 2");

        assert_eq!(store.get_rank_level_count(), 2);
        let rank2 = store.get_rank_info(2).unwrap();
        assert_eq!(rank2.skill_points_needed, 200);
        assert_eq!(rank2.sciences_granted, vec![1, 2]);
    }

    #[test]
    fn override_rank_definition() {
        let mut store = RankInfoStore::new();
        store
            .apply_rank_definition(RankDefinition {
                rank: 1,
                rank_name: UnicodeString::from("Rank 1"),
                skill_points_needed: 100,
                science_purchase_points_granted: 1,
                sciences_granted: vec![1],
                mode: RankDefinitionMode::Create,
            })
            .expect("rank 1");

        store
            .apply_rank_definition(RankDefinition {
                rank: 1,
                rank_name: UnicodeString::from("Rank 1 Override"),
                skill_points_needed: 150,
                science_purchase_points_granted: 3,
                sciences_granted: vec![2, 3],
                mode: RankDefinitionMode::Override,
            })
            .expect("override rank 1");

        let rank1 = store.get_rank_info(1).unwrap();
        assert!(rank1.is_override());
        assert_eq!(rank1.skill_points_needed, 150);
        assert_eq!(rank1.sciences_granted, vec![2, 3]);
    }

    #[test]
    fn reject_non_monotonic_creation() {
        let mut store = RankInfoStore::new();
        let result = store.apply_rank_definition(RankDefinition {
            rank: 2,
            rank_name: UnicodeString::from("Rank 2"),
            skill_points_needed: 200,
            science_purchase_points_granted: 2,
            sciences_granted: Vec::new(),
            mode: RankDefinitionMode::Create,
        });

        assert!(result.is_err());
    }
}
