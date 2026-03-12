//! Healing and Repair System
//!
//! This module implements healing and repair mechanics:
//! - Unit healing (medics, ambulances, hospitals)
//! - Vehicle repair (repair depots, dozer repairs)
//! - Structure repair (worker repairs, self-repair)
//! - Regeneration (veteran units, special abilities)
//! - Healing over time effects

use std::collections::HashMap;
use std::sync::RwLock;

use crate::common::ObjectID;
use crate::weapon::damage_system::{DamageType, DeathType};
use crate::weapon::{DamageInfo, INVALID_OBJECT_ID};
use crate::{GameLogicError, GameLogicResult};

/// Healing types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HealingType {
    /// Medical healing for infantry
    Medical,
    /// Mechanical repair for vehicles
    VehicleRepair,
    /// Construction repair for buildings
    StructureRepair,
    /// Natural regeneration
    Regeneration,
    /// Instant heal (from special powers)
    InstantHeal,
    /// Healing from supply crates/pickups
    SupplyHeal,
}

/// Healing source configuration
#[derive(Debug, Clone)]
pub struct HealingSource {
    /// Type of healing provided
    pub healing_type: HealingType,
    /// Healing amount per second
    pub heal_per_second: f32,
    /// Maximum range for healing (0.0 = contact only)
    pub heal_range: f32,
    /// Whether healing requires line of sight
    pub requires_los: bool,
    /// Can heal self
    pub can_self_heal: bool,
    /// Can heal allies
    pub can_heal_allies: bool,
    /// Can heal enemies
    pub can_heal_enemies: bool,
    /// Maximum targets that can be healed simultaneously
    pub max_targets: u32,
    /// Whether healing removes DoT effects
    pub removes_dot_effects: bool,
    /// Veterancy bonus to healing rate
    pub veterancy_heal_bonus: [f32; 4],
}

impl HealingSource {
    /// Create medic healing configuration
    pub fn medic() -> Self {
        Self {
            healing_type: HealingType::Medical,
            heal_per_second: 10.0,
            heal_range: 50.0,
            requires_los: true,
            can_self_heal: false,
            can_heal_allies: true,
            can_heal_enemies: false,
            max_targets: 1,
            removes_dot_effects: true,
            veterancy_heal_bonus: [1.0, 1.1, 1.25, 1.5],
        }
    }

    /// Create ambulance healing configuration
    pub fn ambulance() -> Self {
        Self {
            healing_type: HealingType::Medical,
            heal_per_second: 15.0,
            heal_range: 0.0, // Must load into ambulance
            requires_los: false,
            can_self_heal: false,
            can_heal_allies: true,
            can_heal_enemies: false,
            max_targets: 8, // Can heal multiple loaded units
            removes_dot_effects: true,
            veterancy_heal_bonus: [1.0, 1.15, 1.3, 1.5],
        }
    }

    /// Create repair depot configuration
    pub fn repair_depot() -> Self {
        Self {
            healing_type: HealingType::VehicleRepair,
            heal_per_second: 25.0,
            heal_range: 100.0,
            requires_los: false,
            can_self_heal: false,
            can_heal_allies: true,
            can_heal_enemies: false,
            max_targets: 1,
            removes_dot_effects: false,
            veterancy_heal_bonus: [1.0, 1.0, 1.0, 1.0],
        }
    }

    /// Create dozer repair configuration
    pub fn dozer_repair() -> Self {
        Self {
            healing_type: HealingType::StructureRepair,
            heal_per_second: 20.0,
            heal_range: 10.0, // Must be adjacent
            requires_los: true,
            can_self_heal: false,
            can_heal_allies: true,
            can_heal_enemies: false,
            max_targets: 1,
            removes_dot_effects: false,
            veterancy_heal_bonus: [1.0, 1.2, 1.4, 1.6],
        }
    }

    /// Create regeneration configuration (veteran units)
    pub fn regeneration() -> Self {
        Self {
            healing_type: HealingType::Regeneration,
            heal_per_second: 2.0,
            heal_range: 0.0,
            requires_los: false,
            can_self_heal: true,
            can_heal_allies: false,
            can_heal_enemies: false,
            max_targets: 1,
            removes_dot_effects: false,
            veterancy_heal_bonus: [0.0, 0.5, 1.0, 2.0], // Only at veteran+
        }
    }

    /// Calculate effective healing rate with veterancy
    pub fn get_effective_heal_rate(&self, veterancy_level: usize) -> f32 {
        let bonus = self
            .veterancy_heal_bonus
            .get(veterancy_level)
            .copied()
            .unwrap_or(1.0);
        self.heal_per_second * bonus
    }
}

/// Active healing effect on an object
#[derive(Debug, Clone)]
pub struct HealingEffect {
    /// Source providing healing
    pub source_id: ObjectID,
    /// Type of healing
    pub healing_type: HealingType,
    /// Healing amount per tick (frame-based)
    pub heal_per_tick: f32,
    /// Game frame when next heal tick occurs
    pub next_heal_frame: u32,
    /// Interval between heal ticks (in frames)
    pub heal_interval_frames: u32,
    /// Game frame when effect expires (0 = infinite)
    pub expiration_frame: u32,
    /// Total healing provided so far
    pub total_healing_done: f32,
}

impl HealingEffect {
    /// Create new healing effect
    pub fn new(
        source_id: ObjectID,
        healing_type: HealingType,
        heal_per_second: f32,
        current_frame: u32,
        duration_frames: u32,
    ) -> Self {
        // Convert per-second to per-tick (30 FPS)
        let frames_per_second = 30;
        let heal_interval = frames_per_second / 2; // 2 ticks per second
        let heal_per_tick = heal_per_second / 2.0;

        Self {
            source_id,
            healing_type,
            heal_per_tick,
            next_heal_frame: current_frame + heal_interval,
            heal_interval_frames: heal_interval,
            expiration_frame: if duration_frames > 0 {
                current_frame + duration_frames
            } else {
                0 // Infinite
            },
            total_healing_done: 0.0,
        }
    }

    /// Check if effect has expired
    pub fn is_expired(&self, current_frame: u32) -> bool {
        self.expiration_frame > 0 && current_frame >= self.expiration_frame
    }

    /// Check if it's time to apply healing
    pub fn should_tick(&self, current_frame: u32) -> bool {
        current_frame >= self.next_heal_frame && !self.is_expired(current_frame)
    }

    /// Advance to next heal tick
    pub fn advance_tick(&mut self, current_frame: u32) {
        self.next_heal_frame = current_frame + self.heal_interval_frames;
    }

    /// Record healing done
    pub fn record_healing(&mut self, amount: f32) {
        self.total_healing_done += amount;
    }

    /// Create damage info for this heal (healing uses negative damage)
    pub fn create_healing_info(&self) -> DamageInfo {
        let mut damage_info = DamageInfo::new();
        damage_info.input.source_id = self.source_id;
        damage_info.input.damage_type = DamageType::Healing;
        damage_info.input.death_type = DeathType::None;
        damage_info.input.amount = self.heal_per_tick;
        damage_info.sync_from_input();
        damage_info
    }
}

/// Healing manager for tracking all healing effects
#[derive(Debug)]
pub struct HealingManager {
    /// Active healing effects by object ID
    effects: RwLock<HashMap<ObjectID, Vec<HealingEffect>>>,
    /// Current game frame
    current_frame: RwLock<u32>,
}

impl HealingManager {
    /// Create new healing manager
    pub fn new() -> Self {
        Self {
            effects: RwLock::new(HashMap::new()),
            current_frame: RwLock::new(0),
        }
    }

    /// Update current frame
    pub fn set_current_frame(&self, frame: u32) -> GameLogicResult<()> {
        let mut current = self.current_frame.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire frame lock: {}", e))
        })?;
        *current = frame;
        Ok(())
    }

    /// Get current frame
    pub fn get_current_frame(&self) -> GameLogicResult<u32> {
        let current = self.current_frame.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire frame lock: {}", e))
        })?;
        Ok(*current)
    }

    /// Start healing effect on an object
    pub fn start_healing(
        &self,
        object_id: ObjectID,
        source_id: ObjectID,
        healing_type: HealingType,
        heal_per_second: f32,
        duration_frames: u32,
    ) -> GameLogicResult<()> {
        let current_frame = self.get_current_frame()?;

        let mut effects = self.effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        let object_effects = effects.entry(object_id).or_insert_with(Vec::new);

        // Check if healing from this source already exists
        if let Some(existing) = object_effects.iter_mut().find(|e| e.source_id == source_id) {
            // Refresh existing effect
            existing.expiration_frame = if duration_frames > 0 {
                current_frame + duration_frames
            } else {
                0
            };
        } else {
            // Add new healing effect
            let effect = HealingEffect::new(
                source_id,
                healing_type,
                heal_per_second,
                current_frame,
                duration_frames,
            );
            object_effects.push(effect);
        }

        Ok(())
    }

    /// Stop all healing on an object
    pub fn stop_healing(&self, object_id: ObjectID) -> GameLogicResult<()> {
        let mut effects = self.effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        effects.remove(&object_id);
        Ok(())
    }

    /// Stop healing from specific source
    pub fn stop_healing_from_source(
        &self,
        object_id: ObjectID,
        source_id: ObjectID,
    ) -> GameLogicResult<()> {
        let mut effects = self.effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        if let Some(object_effects) = effects.get_mut(&object_id) {
            object_effects.retain(|e| e.source_id != source_id);
            if object_effects.is_empty() {
                effects.remove(&object_id);
            }
        }

        Ok(())
    }

    /// Update all healing effects and return healing to apply
    pub fn update_healing(
        &self,
        current_frame: u32,
    ) -> GameLogicResult<Vec<(ObjectID, DamageInfo, f32)>> {
        self.set_current_frame(current_frame)?;

        let mut effects = self.effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        let mut healing_to_apply = Vec::new();

        // Iterate through all objects with healing
        for (object_id, object_effects) in effects.iter_mut() {
            // Remove expired effects
            object_effects.retain(|e| !e.is_expired(current_frame));

            // Apply ticking effects
            for effect in object_effects.iter_mut() {
                if effect.should_tick(current_frame) {
                    let heal_amount = effect.heal_per_tick;
                    healing_to_apply.push((*object_id, effect.create_healing_info(), heal_amount));
                    effect.record_healing(heal_amount);
                    effect.advance_tick(current_frame);
                }
            }
        }

        // Clean up objects with no effects
        effects.retain(|_, effects| !effects.is_empty());

        Ok(healing_to_apply)
    }

    /// Get active healing effects for an object
    pub fn get_healing_effects(&self, object_id: ObjectID) -> GameLogicResult<Vec<HealingEffect>> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        Ok(effects.get(&object_id).cloned().unwrap_or_default())
    }

    /// Check if object is being healed
    pub fn is_being_healed(&self, object_id: ObjectID) -> GameLogicResult<bool> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        Ok(effects.contains_key(&object_id))
    }

    /// Get total healing per second for an object
    pub fn get_hps(&self, object_id: ObjectID) -> GameLogicResult<f32> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        let mut total_hps = 0.0;
        if let Some(object_effects) = effects.get(&object_id) {
            let current_frame = *self.current_frame.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire frame lock: {}", e))
            })?;

            for effect in object_effects.iter() {
                if !effect.is_expired(current_frame) {
                    // Each effect ticks twice per second (heal_interval_frames = 15)
                    total_hps += effect.heal_per_tick * 2.0;
                }
            }
        }

        Ok(total_hps)
    }

    /// Get statistics
    pub fn get_stats(&self) -> GameLogicResult<HealingStats> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        let current_frame = self.get_current_frame()?;

        let mut stats = HealingStats::default();
        stats.total_objects_healing = effects.len();

        for object_effects in effects.values() {
            for effect in object_effects {
                if !effect.is_expired(current_frame) {
                    stats.total_active_effects += 1;
                    stats.total_healing_done += effect.total_healing_done;

                    match effect.healing_type {
                        HealingType::Medical => stats.medical_healing += 1,
                        HealingType::VehicleRepair => stats.repair_effects += 1,
                        HealingType::StructureRepair => stats.repair_effects += 1,
                        HealingType::Regeneration => stats.regeneration_effects += 1,
                        HealingType::InstantHeal => {}
                        HealingType::SupplyHeal => {}
                    }
                }
            }
        }

        Ok(stats)
    }
}

impl Default for HealingManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Healing statistics
#[derive(Debug, Clone, Default)]
pub struct HealingStats {
    /// Total objects being healed
    pub total_objects_healing: usize,
    /// Total active healing effects
    pub total_active_effects: usize,
    /// Total healing done (all time)
    pub total_healing_done: f32,
    /// Number of medical healing effects
    pub medical_healing: usize,
    /// Number of repair effects
    pub repair_effects: usize,
    /// Number of regeneration effects
    pub regeneration_effects: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_healing_source_configs() {
        let medic = HealingSource::medic();
        assert_eq!(medic.healing_type, HealingType::Medical);
        assert_eq!(medic.heal_per_second, 10.0);
        assert!(medic.removes_dot_effects);

        let ambulance = HealingSource::ambulance();
        assert_eq!(ambulance.max_targets, 8);

        let depot = HealingSource::repair_depot();
        assert_eq!(depot.healing_type, HealingType::VehicleRepair);
    }

    #[test]
    fn test_healing_effect_creation() {
        let effect = HealingEffect::new(123, HealingType::Medical, 10.0, 0, 300);

        assert_eq!(effect.source_id, 123);
        assert_eq!(effect.healing_type, HealingType::Medical);
        assert_eq!(effect.heal_per_tick, 5.0); // 10.0 / 2
        assert_eq!(effect.heal_interval_frames, 15);
    }

    #[test]
    fn test_healing_effect_ticking() {
        let mut effect = HealingEffect::new(123, HealingType::Medical, 10.0, 0, 300);

        assert!(!effect.should_tick(0));
        assert!(!effect.should_tick(14));
        assert!(effect.should_tick(15));

        effect.advance_tick(15);
        assert_eq!(effect.next_heal_frame, 30);
    }

    #[test]
    fn test_healing_manager_start_stop() {
        let manager = HealingManager::new();
        manager.set_current_frame(0).unwrap();

        manager
            .start_healing(100, 200, HealingType::Medical, 10.0, 300)
            .unwrap();

        assert!(manager.is_being_healed(100).unwrap());

        manager.stop_healing(100).unwrap();

        assert!(!manager.is_being_healed(100).unwrap());
    }

    #[test]
    fn test_healing_manager_update() {
        let manager = HealingManager::new();
        manager.set_current_frame(0).unwrap();

        manager
            .start_healing(100, 200, HealingType::Medical, 10.0, 300)
            .unwrap();

        // No healing at frame 0
        let healing = manager.update_healing(0).unwrap();
        assert_eq!(healing.len(), 0);

        // Healing at frame 15
        let healing = manager.update_healing(15).unwrap();
        assert_eq!(healing.len(), 1);
        assert_eq!(healing[0].0, 100);
        assert_eq!(healing[0].2, 5.0); // 10.0 / 2 per tick

        // No healing at frame 16
        let healing = manager.update_healing(16).unwrap();
        assert_eq!(healing.len(), 0);

        // Healing at frame 30
        let healing = manager.update_healing(30).unwrap();
        assert_eq!(healing.len(), 1);
    }

    #[test]
    fn test_healing_manager_hps() {
        let manager = HealingManager::new();
        manager.set_current_frame(0).unwrap();

        manager
            .start_healing(100, 200, HealingType::Medical, 10.0, 300)
            .unwrap();

        let hps = manager.get_hps(100).unwrap();
        assert_eq!(hps, 10.0); // Should match heal_per_second
    }

    #[test]
    fn test_healing_manager_multiple_sources() {
        let manager = HealingManager::new();
        manager.set_current_frame(0).unwrap();

        manager
            .start_healing(100, 200, HealingType::Medical, 10.0, 300)
            .unwrap();
        manager
            .start_healing(100, 201, HealingType::Regeneration, 2.0, 0)
            .unwrap();

        let effects = manager.get_healing_effects(100).unwrap();
        assert_eq!(effects.len(), 2);

        let hps = manager.get_hps(100).unwrap();
        assert_eq!(hps, 12.0); // 10 + 2
    }

    #[test]
    fn test_veterancy_healing_bonus() {
        let medic = HealingSource::medic();

        assert_eq!(medic.get_effective_heal_rate(0), 10.0);
        assert_eq!(medic.get_effective_heal_rate(1), 11.0);
        assert_eq!(medic.get_effective_heal_rate(2), 12.5);
        assert_eq!(medic.get_effective_heal_rate(3), 15.0);
    }

    #[test]
    fn test_healing_stats() {
        let manager = HealingManager::new();
        manager.set_current_frame(0).unwrap();

        manager
            .start_healing(100, 200, HealingType::Medical, 10.0, 300)
            .unwrap();
        manager
            .start_healing(101, 201, HealingType::VehicleRepair, 25.0, 300)
            .unwrap();

        let stats = manager.get_stats().unwrap();
        assert_eq!(stats.total_objects_healing, 2);
        assert_eq!(stats.total_active_effects, 2);
        assert_eq!(stats.medical_healing, 1);
        assert_eq!(stats.repair_effects, 1);
    }
}
