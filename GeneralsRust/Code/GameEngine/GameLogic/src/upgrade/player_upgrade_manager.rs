//! Player Upgrade Manager
//!
//! Manages upgrades that apply globally to a player (not just individual objects).
//! Matches C++ Player upgrade management functionality.
//!
//! Original C++ reference: Player.cpp, Upgrade.cpp

use game_engine::common::system::{Snapshotable, Xfer};
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

use super::center::with_upgrade_center;
use super::{Upgrade, UpgradeError, UpgradeMask, UpgradeResult, UpgradeStatus, UpgradeTemplate};
use crate::common::*;
use crate::object::registry::OBJECT_REGISTRY;
use crate::scripting::engine::get_script_engine;

/// Player-specific upgrade state
/// Matches C++ Player's upgrade-related fields
#[derive(Debug, Clone)]
pub struct PlayerUpgradeManager {
    /// Player ID this manager belongs to
    player_id: u32,
    /// Active player upgrades (completed)
    active_upgrades: UpgradeMask,
    /// Upgrades in progress
    in_progress_upgrades: HashMap<NameKeyType, Upgrade>,
    /// Completed upgrade instances
    completed_upgrades: HashSet<NameKeyType>,
    /// Money spent on upgrades
    total_upgrade_cost: Int,
}

impl PlayerUpgradeManager {
    /// Create a new player upgrade manager
    /// Matches C++ Player initialization of upgrade state
    pub fn new(player_id: u32) -> Self {
        Self {
            player_id,
            active_upgrades: UpgradeMask::none(),
            in_progress_upgrades: HashMap::new(),
            completed_upgrades: HashSet::new(),
            total_upgrade_cost: 0,
        }
    }

    /// Get active upgrade mask
    /// Matches C++ Player::getUpgradeMask()
    pub fn get_active_upgrades(&self) -> UpgradeMask {
        self.active_upgrades
    }

    /// Check if player has specific upgrade
    /// Matches C++ Player::hasUpgrade(upgrade_key)
    pub fn has_upgrade(&self, upgrade_key: NameKeyType) -> bool {
        self.completed_upgrades.contains(&upgrade_key)
    }

    /// Alias for has_upgrade for consistency with other APIs
    pub fn has_upgrade_by_key(&self, upgrade_key: NameKeyType) -> bool {
        self.has_upgrade(upgrade_key)
    }

    /// Check if player has upgrade by mask
    /// Matches C++ Player::hasUpgradeMask(mask)
    pub fn has_upgrade_mask(&self, mask: UpgradeMask) -> bool {
        self.active_upgrades.test_for_any(mask)
    }

    /// Check if upgrade is in progress
    pub fn is_upgrade_in_progress(&self, upgrade_key: NameKeyType) -> bool {
        self.in_progress_upgrades.contains_key(&upgrade_key)
    }

    /// Begin researching an upgrade
    /// Matches C++ Player::beginUpgradeResearch
    pub fn begin_upgrade(
        &mut self,
        template: Arc<UpgradeTemplate>,
        player: &mut Player,
        current_frame: u32,
    ) -> UpgradeResult<()> {
        let upgrade_key = template.get_name_key();

        // Check if already complete or in progress
        if self.has_upgrade(upgrade_key) {
            return Err(UpgradeError::AlreadyExists(template.get_name().to_string()));
        }

        if self.is_upgrade_in_progress(upgrade_key) {
            return Err(UpgradeError::AlreadyExists(format!(
                "{} (in progress)",
                template.get_name()
            )));
        }

        // Check if can afford
        let cost = template.calc_cost_to_build(player);
        let money = player.get_money_mut();
        if money.get_money() < cost {
            return Err(UpgradeError::CannotAfford(template.get_name().to_string()));
        }

        // Deduct money
        money.add_money(-cost);
        self.total_upgrade_cost += cost;

        // Create upgrade instance and begin production
        let mut upgrade = Upgrade::new(template.clone());
        upgrade.begin_production(current_frame);

        self.in_progress_upgrades.insert(upgrade_key, upgrade);

        log::info!(
            "Player {} began researching upgrade: {}",
            self.player_id,
            template.get_name()
        );

        Ok(())
    }

    /// Cancel an upgrade in progress
    /// Matches C++ Player::cancelUpgradeResearch
    pub fn cancel_upgrade(
        &mut self,
        upgrade_key: NameKeyType,
        player: &mut Player,
        refund_percentage: Real,
    ) -> UpgradeResult<()> {
        let upgrade = self
            .in_progress_upgrades
            .remove(&upgrade_key)
            .ok_or_else(|| UpgradeError::NotFound("Upgrade not in progress".to_string()))?;

        // Refund money
        let template = upgrade.get_template();
        let cost = template.calc_cost_to_build(player);
        let refund = (cost as Real * refund_percentage) as Int;

        player.get_money_mut().add_money(refund);
        self.total_upgrade_cost -= cost;

        log::info!(
            "Player {} cancelled upgrade: {} (refunded {})",
            self.player_id,
            template.get_name(),
            refund
        );

        Ok(())
    }

    /// Update all upgrades in progress
    /// Matches C++ Player upgrade update logic
    pub fn update(&mut self, current_frame: u32, player: &mut Player) -> Vec<Arc<UpgradeTemplate>> {
        let mut completed = Vec::new();

        // Update all in-progress upgrades
        let mut to_complete = Vec::new();
        for (key, upgrade) in self.in_progress_upgrades.iter_mut() {
            if upgrade.update(current_frame, player) {
                to_complete.push(*key);
            }
        }

        // Complete upgrades
        for key in to_complete {
            if let Some(upgrade) = self.in_progress_upgrades.remove(&key) {
                let template = upgrade.get_template();
                let template_arc = Arc::new(template.clone());

                self.complete_upgrade(template_arc.clone(), player);
                completed.push(template_arc);
            }
        }

        completed
    }

    /// Complete an upgrade (apply effects)
    /// Matches C++ Player::completeUpgrade
    fn complete_upgrade(&mut self, template: Arc<UpgradeTemplate>, player: &mut Player) {
        let upgrade_key = template.get_name_key();
        let upgrade_mask = template.get_mask();

        // Add to completed set
        self.completed_upgrades.insert(upgrade_key);

        // Add to active mask
        self.active_upgrades |= upgrade_mask;

        log::info!(
            "Player {} completed upgrade: {}",
            self.player_id,
            template.get_name()
        );

        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.notify_of_completed_upgrade(
                    self.player_id as usize,
                    template.get_name().as_str(),
                    INVALID_ID,
                );
            }
        }

        // Play completion sound if available
        // Matches C++ UpgradeTemplate::playResearchSound
        let sound = template.get_research_sound();
        // Check if sound is valid (not invalid/empty)
        // In C++, this checks if (m_researchSound != NULL)
        if sound.is_valid() {
            // Audio playback would be handled by the audio subsystem
            // In C++, this calls TheAudio->playAudioEvent(sound)
            // For now, we log the intent until audio system is fully integrated
            log::debug!(
                "Would play upgrade completion sound for {}",
                template.get_name()
            );
        }

        // Apply upgrade effects to all existing objects
        if template.affects_existing_objects() {
            self.apply_upgrade_to_existing_objects(template.as_ref(), player);
        }
    }

    /// Apply upgrade to all existing player objects
    /// Matches C++ Player::applyUpgradeToExistingObjects
    fn apply_upgrade_to_existing_objects(&self, template: &UpgradeTemplate, player: &Player) {
        // Get all objects owned by this player
        let objects = player.get_all_objects();

        for object_id in objects {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                if object_guard.is_destroyed() {
                    return;
                }
                if object_guard.get_controlling_player_id() != Some(self.player_id) {
                    return;
                }
                object_guard.give_upgrade(template);
            });
        }
    }

    /// Remove an upgrade (for conflicting upgrades)
    /// Matches C++ Player::removeUpgrade
    pub fn remove_upgrade(&mut self, upgrade_key: NameKeyType, player: &mut Player) {
        if self.completed_upgrades.remove(&upgrade_key) {
            // Find template to get mask
            if let Some(template) =
                with_upgrade_center(|center| center.find_upgrade_by_key(upgrade_key))
            {
                let upgrade_mask = template.get_mask();
                self.active_upgrades &= !upgrade_mask;

                log::info!(
                    "Player {} removed upgrade: {}",
                    self.player_id,
                    template.get_name()
                );

                // Remove from existing objects
                self.remove_upgrade_from_existing_objects(upgrade_mask, player);
            }
        }
    }

    /// Remove upgrade from all existing player objects
    fn remove_upgrade_from_existing_objects(&self, upgrade_mask: UpgradeMask, player: &Player) {
        let objects = player.get_all_objects();

        // Remove upgrade from each object via object manager
        // Matches C++ Player::removeUpgradeFromExistingObjects
        // C++ calls obj->loseUpgrade(upgradeMask) for each object

        // Convert UpgradeMask to UpgradeMaskType for object methods
        let mask_bits = UpgradeMaskType::from_bits_retain(upgrade_mask.to_bits());

        for object_id in objects {
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |object_guard| {
                if object_guard.is_destroyed() {
                    return;
                }
                if object_guard.get_controlling_player_id() != Some(self.player_id) {
                    return;
                }
                object_guard.remove_upgrade_mask(mask_bits);
            });
        }
    }

    /// Grant an upgrade immediately (for cheats/scripting)
    /// Matches C++ Player::grantUpgrade
    pub fn grant_upgrade(&mut self, template: Arc<UpgradeTemplate>, player: &mut Player) {
        self.complete_upgrade(template, player);
    }

    /// Add a completed upgrade directly (flags + mask, no object iteration).
    /// Used by the top-level `complete_upgrade()` function which handles
    /// object iteration separately to avoid borrow conflicts.
    pub fn add_completed_upgrade(&mut self, upgrade_key: NameKeyType, upgrade_mask: UpgradeMask) {
        self.completed_upgrades.insert(upgrade_key);
        self.active_upgrades |= upgrade_mask;
    }

    /// Get all in-progress upgrades
    pub fn get_in_progress_upgrades(&self) -> &HashMap<NameKeyType, Upgrade> {
        &self.in_progress_upgrades
    }

    /// Get total money spent on upgrades
    pub fn get_total_upgrade_cost(&self) -> Int {
        self.total_upgrade_cost
    }

    /// Reset all upgrades (for new game)
    pub fn reset(&mut self) {
        self.active_upgrades = UpgradeMask::none();
        self.in_progress_upgrades.clear();
        self.completed_upgrades.clear();
        self.total_upgrade_cost = 0;
    }
}

/// Serialization support
impl Snapshotable for PlayerUpgradeManager {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        // CRC active upgrades
        let mut mask_bits = self.active_upgrades.to_bits();
        xfer.xfer_u128(&mut mask_bits).map_err(|e| e.to_string())?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version = 1u8;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        // Serialize player ID
        xfer.xfer_u32(&mut self.player_id)
            .map_err(|e| e.to_string())?;

        // Serialize active upgrades mask
        let mut mask_bits = self.active_upgrades.to_bits();
        xfer.xfer_u128(&mut mask_bits).map_err(|e| e.to_string())?;
        if xfer.is_reading() {
            self.active_upgrades = UpgradeMask::from_bits_value(mask_bits);
        }

        // Serialize total cost
        xfer.xfer_i32(&mut self.total_upgrade_cost)
            .map_err(|e| e.to_string())?;

        // Serialize completed upgrades set
        // Matches C++ Player upgrade serialization
        if xfer.is_writing() {
            let mut count = self.completed_upgrades.len() as u32;
            xfer.xfer_u32(&mut count).map_err(|e| e.to_string())?;

            for upgrade_key in &self.completed_upgrades {
                let mut key = *upgrade_key;
                xfer.xfer_u32(&mut key).map_err(|e| e.to_string())?;
            }
        } else {
            // Reading
            let mut count = 0u32;
            xfer.xfer_u32(&mut count).map_err(|e| e.to_string())?;

            self.completed_upgrades.clear();
            for _ in 0..count {
                let mut key = 0u32;
                xfer.xfer_u32(&mut key).map_err(|e| e.to_string())?;
                self.completed_upgrades.insert(key);
            }
        }

        // Note: in_progress_upgrades are not serialized as they are transient
        // and will be reconstructed from production queue state

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_upgrade_manager_creation() {
        let manager = PlayerUpgradeManager::new(1);
        assert_eq!(manager.get_active_upgrades(), UpgradeMask::none());
        assert_eq!(manager.get_total_upgrade_cost(), 0);
    }

    #[test]
    fn test_has_upgrade() {
        let mut manager = PlayerUpgradeManager::new(1);
        let upgrade_key = 123;

        assert!(!manager.has_upgrade(upgrade_key));

        manager.completed_upgrades.insert(upgrade_key);
        assert!(manager.has_upgrade(upgrade_key));
    }

    #[test]
    fn test_begin_upgrade() {
        let mut manager = PlayerUpgradeManager::new(1);
        let mut player = Player::new(1);

        let mut template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));
        template.set_cost(500);
        template.set_build_time(10.0);
        let template = Arc::new(template);

        // Give player enough money
        player.get_money_mut().add_money(1000);

        let result = manager.begin_upgrade(template.clone(), &mut player, 0);
        assert!(result.is_ok());

        assert!(manager.is_upgrade_in_progress(template.get_name_key()));
        assert_eq!(player.get_money().get_money(), 500); // 1000 - 500
    }

    #[test]
    fn test_cancel_upgrade() {
        let mut manager = PlayerUpgradeManager::new(1);
        let mut player = Player::new(1);

        let mut template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));
        template.set_cost(500);
        template.set_build_time(10.0);
        let template = Arc::new(template);

        player.get_money_mut().add_money(1000);
        let _ = manager.begin_upgrade(template.clone(), &mut player, 0);

        // Cancel with 50% refund
        let result = manager.cancel_upgrade(template.get_name_key(), &mut player, 0.5);
        assert!(result.is_ok());

        assert!(!manager.is_upgrade_in_progress(template.get_name_key()));
        assert_eq!(player.get_money().get_money(), 750); // 500 + 250 refund
    }

    #[test]
    fn test_update_completes_upgrade() {
        let mut manager = PlayerUpgradeManager::new(1);
        let mut player = Player::new(1);

        let mut template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));
        template.set_cost(500);
        template.set_build_time(10.0); // 300 frames
        let template = Arc::new(template);

        player.get_money_mut().add_money(1000);
        let _ = manager.begin_upgrade(template.clone(), &mut player, 0);

        // Update at frame 300 (should complete)
        let completed = manager.update(300, &mut player);
        assert_eq!(completed.len(), 1);
        assert!(manager.has_upgrade(template.get_name_key()));
        assert!(!manager.is_upgrade_in_progress(template.get_name_key()));
    }

    #[test]
    fn test_grant_upgrade() {
        let mut manager = PlayerUpgradeManager::new(1);
        let mut player = Player::new(1);

        let template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));
        let template = Arc::new(template);

        manager.grant_upgrade(template.clone(), &mut player);

        assert!(manager.has_upgrade(template.get_name_key()));
        assert!(manager.has_upgrade_mask(template.get_mask()));
    }

    #[test]
    fn test_remove_upgrade() {
        let mut manager = PlayerUpgradeManager::new(1);
        let mut player = Player::new(1);

        let template = UpgradeTemplate::new(AsciiString::from("TestUpgrade"));
        let template = Arc::new(template);

        manager.grant_upgrade(template.clone(), &mut player);
        assert!(manager.has_upgrade(template.get_name_key()));

        manager.remove_upgrade(template.get_name_key(), &mut player);
        assert!(!manager.has_upgrade(template.get_name_key()));
    }
}
