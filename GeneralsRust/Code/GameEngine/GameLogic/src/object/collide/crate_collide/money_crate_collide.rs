//! Money Crate Collision Module
//!
//! This crate provides money to the player who collects it. The amount can be
//! enhanced by certain upgrades that the player has researched.

use super::super::{CollideModule, CollisionError, Coord3D, GameObject};
use super::crate_collide::{CrateCollide, CrateCollideBehavior, CrateCollideModuleData};
use crate::common::*;
use crate::helpers::{TheAudio, TheGameLogic};
use crate::object::collide::crate_collide::*;
use crate::player::{player_list, PlayerIndex};
use crate::upgrade::center::get_upgrade_center;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Upgrade pair structure for bonus calculations
#[derive(Debug, Clone, PartialEq)]
pub struct UpgradePair {
    /// Type/name of the upgrade
    pub upgrade_type: String,
    /// Bonus amount provided by this upgrade
    pub amount: i32,
}

impl UpgradePair {
    pub fn new(upgrade_type: String, amount: i32) -> Self {
        Self {
            upgrade_type,
            amount,
        }
    }
}

/// Configuration data for MoneyCrateCollide
#[derive(Debug, Clone)]
pub struct MoneyCrateCollideModuleData {
    /// Base crate collision data
    pub base: CrateCollideModuleData,
    /// Base amount of money provided by this crate
    pub money_provided: u32,
    /// List of upgrade pairs that provide bonus money
    pub upgrade_boosts: Vec<UpgradePair>,
}

impl MoneyCrateCollideModuleData {
    pub fn new() -> Self {
        Self {
            base: CrateCollideModuleData::new(),
            money_provided: 0,
            upgrade_boosts: Vec::new(),
        }
    }

    pub fn with_money_amount(mut self, amount: u32) -> Self {
        self.money_provided = amount;
        self
    }

    pub fn with_upgrade_boost(mut self, upgrade_type: String, bonus_amount: i32) -> Self {
        self.upgrade_boosts
            .push(UpgradePair::new(upgrade_type, bonus_amount));
        self
    }

    pub fn with_upgrade_boosts(mut self, boosts: Vec<UpgradePair>) -> Self {
        self.upgrade_boosts = boosts;
        self
    }
}

impl Default for MoneyCrateCollideModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// Money collection statistics
#[derive(Debug, Clone)]
pub struct MoneyCollectionStats {
    /// Base money amount collected
    pub base_amount: u32,
    /// Bonus money from upgrades
    pub bonus_amount: i32,
    /// Total money collected
    pub total_amount: u32,
    /// Upgrades that contributed to the bonus
    pub contributing_upgrades: Vec<String>,
    /// Time when money was collected
    pub collection_time: u64,
}

impl MoneyCollectionStats {
    pub fn new(base_amount: u32, bonus_amount: i32, contributing_upgrades: Vec<String>) -> Self {
        let total_amount = if bonus_amount < 0 {
            base_amount.saturating_sub((-bonus_amount) as u32)
        } else {
            base_amount.saturating_add(bonus_amount as u32)
        };

        Self {
            base_amount,
            bonus_amount,
            total_amount,
            contributing_upgrades,
            collection_time: 0,
        }
    }
}

/// Money collection state
#[derive(Debug)]
struct MoneyCollectionState {
    /// Whether money collection is in progress
    is_collecting: bool,
    /// ID of the collecting player
    collecting_player_id: Option<PlayerId>,
    /// Collection start time
    collection_start_time: u64,
    /// Statistics from the last collection
    last_collection_stats: Option<MoneyCollectionStats>,
}

/// Money Crate Collide implementation
pub struct MoneyCrateCollide {
    /// Base crate collision functionality
    base_crate: CrateCollide,
    /// Module-specific configuration
    module_data: MoneyCrateCollideModuleData,
    /// Thread-safe collection state
    state: Arc<Mutex<MoneyCollectionState>>,
}

impl MoneyCrateCollide {
    pub fn new(object_id: ObjectId, module_data: MoneyCrateCollideModuleData) -> Self {
        Self {
            base_crate: CrateCollide::new(object_id, module_data.base.clone()),
            module_data,
            state: Arc::new(Mutex::new(MoneyCollectionState {
                is_collecting: false,
                collecting_player_id: None,
                collection_start_time: 0,
                last_collection_stats: None,
            })),
        }
    }

    pub fn get_module_data(&self) -> &MoneyCrateCollideModuleData {
        &self.module_data
    }

    /// Execute the money collection process
    pub fn execute_money_collection(
        &mut self,
        other: &dyn GameObject,
    ) -> Result<bool, CollisionError> {
        let player_id = other.get_controlling_player();

        // Start collection state tracking
        {
            let mut state = self.state.lock().map_err(|e| {
                CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
            })?;
            state.is_collecting = true;
            state.collecting_player_id = Some(player_id);
            state.collection_start_time = self.get_current_time()?;
        }

        // Calculate total money amount including upgrades
        let base_money = self.module_data.money_provided;
        let (upgrade_bonus, contributing_upgrades) = self.get_upgraded_supply_boost(other)?;

        let total_money = if upgrade_bonus < 0 {
            base_money.saturating_sub((-upgrade_bonus) as u32)
        } else {
            base_money.saturating_add(upgrade_bonus as u32)
        };

        // Deposit money to player's account
        self.deposit_money_to_player(player_id, total_money)?;

        // Add to player's score
        self.add_money_earned_to_score(player_id, total_money)?;

        // Play money collection audio
        self.play_money_audio(other)?;

        // Create collection statistics
        let collection_stats =
            MoneyCollectionStats::new(base_money, upgrade_bonus, contributing_upgrades);

        // Store collection statistics
        {
            let mut state = self.state.lock().map_err(|e| {
                CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
            })?;
            state.is_collecting = false;
            state.last_collection_stats = Some(collection_stats);
        }

        Ok(true)
    }

    /// Calculate upgrade-based money boost
    fn get_upgraded_supply_boost(
        &self,
        other: &dyn GameObject,
    ) -> Result<(i32, Vec<String>), CollisionError> {
        let player_id = other.get_controlling_player();
        let mut total_boost = 0i32;
        let mut contributing_upgrades = Vec::new();

        // Loop through upgrade pairs and check if player has them
        for upgrade_pair in &self.module_data.upgrade_boosts {
            if self.player_has_upgrade(player_id, &upgrade_pair.upgrade_type)? {
                total_boost += upgrade_pair.amount;
                contributing_upgrades.push(upgrade_pair.upgrade_type.clone());
            }
        }

        Ok((total_boost, contributing_upgrades))
    }

    /// Get the last collection statistics
    pub fn get_last_collection_stats(
        &self,
    ) -> Result<Option<MoneyCollectionStats>, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.last_collection_stats.clone())
    }

    /// Check if currently collecting money
    pub fn is_collecting(&self) -> Result<bool, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.is_collecting)
    }

    /// Get the player currently collecting (if any)
    pub fn get_collecting_player(&self) -> Result<Option<PlayerId>, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.collecting_player_id)
    }

    /// Calculate the total money this crate would provide to a specific player
    pub fn calculate_total_money_for_player(
        &self,
        player_id: PlayerId,
    ) -> Result<u32, CollisionError> {
        let base_money = self.module_data.money_provided;
        let mut total_boost = 0i32;

        // Calculate upgrade bonuses
        for upgrade_pair in &self.module_data.upgrade_boosts {
            if self.player_has_upgrade(player_id, &upgrade_pair.upgrade_type)? {
                total_boost += upgrade_pair.amount;
            }
        }

        let total_money = if total_boost < 0 {
            base_money.saturating_sub((-total_boost) as u32)
        } else {
            base_money.saturating_add(total_boost as u32)
        };

        Ok(total_money)
    }

    // Helper methods that would interface with the game engine
    fn get_current_time(&self) -> Result<u64, CollisionError> {
        Ok(TheGameLogic::get_frame() as u64)
    }

    fn player_has_upgrade(
        &self,
        _player_id: PlayerId,
        _upgrade_type: &str,
    ) -> Result<bool, CollisionError> {
        let upgrade = get_upgrade_center()
            .read()
            .map_err(|_| CollisionError::InvalidObject("UpgradeCenter lock poisoned".to_string()))?
            .find_upgrade(_upgrade_type);

        let Some(upgrade) = upgrade else {
            return Ok(false);
        };

        let player_index = _player_id.value() as PlayerIndex;
        let player = player_list()
            .read()
            .map_err(|_| CollisionError::InvalidObject("PlayerList lock poisoned".to_string()))?
            .get_player(player_index)
            .cloned();

        let Some(player) = player else {
            return Ok(false);
        };

        let guard = player
            .read()
            .map_err(|_| CollisionError::InvalidObject("Player lock poisoned".to_string()))?;
        Ok(guard.has_upgrade_complete(&upgrade))
    }

    fn deposit_money_to_player(
        &self,
        _player_id: PlayerId,
        _amount: u32,
    ) -> Result<(), CollisionError> {
        let player_index = _player_id.value() as PlayerIndex;
        let player = player_list()
            .read()
            .map_err(|_| CollisionError::InvalidObject("PlayerList lock poisoned".to_string()))?
            .get_player(player_index)
            .cloned()
            .ok_or_else(|| CollisionError::InvalidObject("Player not found".to_string()))?;

        let mut guard = player
            .write()
            .map_err(|_| CollisionError::InvalidObject("Player lock poisoned".to_string()))?;

        guard
            .get_money_mut()
            .deposit(_amount)
            .map_err(|err| CollisionError::InvalidObject(format!("Deposit failed: {err}")))
    }

    fn add_money_earned_to_score(
        &self,
        _player_id: PlayerId,
        _amount: u32,
    ) -> Result<(), CollisionError> {
        let player_index = _player_id.value() as PlayerIndex;
        let player = player_list()
            .read()
            .map_err(|_| CollisionError::InvalidObject("PlayerList lock poisoned".to_string()))?
            .get_player(player_index)
            .cloned()
            .ok_or_else(|| CollisionError::InvalidObject("Player not found".to_string()))?;

        let mut guard = player
            .write()
            .map_err(|_| CollisionError::InvalidObject("Player lock poisoned".to_string()))?;
        guard.get_score_keeper_mut().add_money_earned(_amount);
        Ok(())
    }

    fn play_money_audio(&self, other: &dyn GameObject) -> Result<(), CollisionError> {
        let Some(audio) = TheAudio::get() else {
            return Ok(());
        };

        let event = TheAudio::get_misc_audio().crate_money.clone();
        let mut audio_event = crate::common::audio::AudioEventRts::new(event.sound_type);
        let position = other.get_position();
        audio_event.set_position(&(position.x, position.y, position.z));
        audio.add_audio_event(&audio_event);
        Ok(())
    }
}

impl CrateCollideBehavior for MoneyCrateCollide {
    fn execute_crate_behavior(&mut self, other: &dyn GameObject) -> Result<bool, CollisionError> {
        self.execute_money_collection(other)
    }

    fn is_valid_to_execute(&self, other: &dyn GameObject) -> bool {
        // Use base validation - money collection doesn't require additional restrictions
        self.base_crate.is_valid_to_execute(other)
    }
}

impl CollideModule for MoneyCrateCollide {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        let Some(other_obj) = other else {
            return Ok(());
        };

        if !self.base_crate.is_valid_to_execute(other_obj) {
            return Ok(());
        }

        if self.execute_crate_behavior(other_obj)? {
            self.base_crate.finalize_collection(other_obj)?;
        }

        Ok(())
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        CrateCollideBehavior::is_valid_to_execute(self, other)
    }
}

/// Factory for creating MoneyCrateCollide modules
pub struct MoneyCrateCollideFactory;

impl MoneyCrateCollideFactory {
    pub fn create(object_id: ObjectId, money_amount: u32) -> MoneyCrateCollide {
        let data = MoneyCrateCollideModuleData::new().with_money_amount(money_amount);
        MoneyCrateCollide::new(object_id, data)
    }

    pub fn create_with_config(
        object_id: ObjectId,
        config: MoneyCrateCollideModuleData,
    ) -> MoneyCrateCollide {
        MoneyCrateCollide::new(object_id, config)
    }

    pub fn create_with_upgrades(
        object_id: ObjectId,
        base_amount: u32,
        upgrades: Vec<UpgradePair>,
    ) -> MoneyCrateCollide {
        let data = MoneyCrateCollideModuleData::new()
            .with_money_amount(base_amount)
            .with_upgrade_boosts(upgrades);
        MoneyCrateCollide::new(object_id, data)
    }

    pub fn create_small_money_crate(object_id: ObjectId) -> MoneyCrateCollide {
        Self::create(object_id, 100)
    }

    pub fn create_medium_money_crate(object_id: ObjectId) -> MoneyCrateCollide {
        Self::create(object_id, 500)
    }

    pub fn create_large_money_crate(object_id: ObjectId) -> MoneyCrateCollide {
        Self::create(object_id, 1000)
    }

    pub fn create_salvage_money_crate(object_id: ObjectId) -> MoneyCrateCollide {
        let upgrades = vec![
            UpgradePair::new("Salvage".to_string(), 25),
            UpgradePair::new("CashBounty".to_string(), 50),
        ];
        Self::create_with_upgrades(object_id, 200, upgrades)
    }
}

// Mock-based tests removed to avoid mocks in fidelity-critical code.

impl game_engine::common::system::Snapshotable for MoneyCrateCollide {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base_crate.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // C++ parity: versioned xfer entry point (current version 1).
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())?;
        self.base_crate.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base_crate.load_post_process()
    }
}
