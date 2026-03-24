//! Damage Over Time (DoT) System
//!
//! This module implements all damage-over-time effects from C&C Generals Zero Hour:
//! - Poison damage (from toxin weapons)
//! - Fire/Burn damage (from flame weapons, particle beam)
//! - Radiation damage (from nuclear/radiation sources)
//! - Status effects and debuffs
//!
//! Matches C++ implementation from PoisonedBehavior.cpp and related modules.

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::common::ObjectID;
use crate::weapon::damage_system::DamageType;
use crate::weapon::damage_system::DeathType;
use crate::weapon::{DamageInfo, INVALID_OBJECT_ID};
use crate::{GameLogicError, GameLogicResult};

/// Frame-based timing constant (30 FPS game logic)
pub const FRAMES_PER_SECOND: u32 = 30;

/// Damage over time effect types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DotEffectType {
    /// Poison damage from toxin weapons
    Poison,
    /// Burn damage from fire/flame
    Burning,
    /// Burn damage from particle beam (different death type)
    ParticleBeamBurn,
    /// Radiation poisoning
    Radiation,
    /// Microwave radiation (infantry only)
    Microwave,
    /// Beta toxin (different visual effects)
    PoisonBeta,
    /// Gamma toxin (different visual effects)
    PoisonGamma,
}

impl DotEffectType {
    /// Get the death type associated with this DoT effect
    pub fn death_type(&self) -> DeathType {
        match self {
            DotEffectType::Poison => DeathType::Poisoned,
            DotEffectType::Burning => DeathType::Burned,
            DotEffectType::ParticleBeamBurn => DeathType::Burned,
            DotEffectType::Radiation => DeathType::Poisoned,
            DotEffectType::Microwave => DeathType::Burned,
            DotEffectType::PoisonBeta => DeathType::PoisonedBeta,
            DotEffectType::PoisonGamma => DeathType::PoisonedGamma,
        }
    }

    /// Get the visual FX damage type for this DoT effect
    pub fn fx_damage_type(&self) -> DamageType {
        match self {
            DotEffectType::Poison => DamageType::Poison,
            DotEffectType::Burning => DamageType::Flame,
            DotEffectType::ParticleBeamBurn => DamageType::ParticleBeam,
            DotEffectType::Radiation => DamageType::Radiation,
            DotEffectType::Microwave => DamageType::Microwave,
            DotEffectType::PoisonBeta => DamageType::Poison,
            DotEffectType::PoisonGamma => DamageType::Poison,
        }
    }

    /// Check if this effect can stack with another
    pub fn can_stack_with(&self, other: &DotEffectType) -> bool {
        // Same type stacks by refreshing duration
        if self == other {
            return true;
        }

        // Different poison types don't stack
        if matches!(
            self,
            DotEffectType::Poison | DotEffectType::PoisonBeta | DotEffectType::PoisonGamma
        ) && matches!(
            other,
            DotEffectType::Poison | DotEffectType::PoisonBeta | DotEffectType::PoisonGamma
        ) {
            return false;
        }

        // Different burn types don't stack
        if matches!(
            self,
            DotEffectType::Burning | DotEffectType::ParticleBeamBurn
        ) && matches!(
            other,
            DotEffectType::Burning | DotEffectType::ParticleBeamBurn
        ) {
            return false;
        }

        // Radiation types don't stack
        if matches!(self, DotEffectType::Radiation | DotEffectType::Microwave)
            && matches!(other, DotEffectType::Radiation | DotEffectType::Microwave)
        {
            return false;
        }

        // Otherwise, different types can coexist
        true
    }
}

/// Active damage-over-time effect on an object
#[derive(Debug, Clone)]
pub struct DotEffect {
    /// Type of DoT effect
    pub effect_type: DotEffectType,
    /// Source object that applied this effect
    pub source_id: ObjectID,
    /// Damage amount per tick
    pub damage_per_tick: f32,
    /// Game frame when next damage tick occurs
    pub next_damage_frame: u32,
    /// Game frame when effect expires
    pub expiration_frame: u32,
    /// Interval between damage ticks (in frames)
    pub damage_interval_frames: u32,
    /// Original damage that caused this effect (for re-application)
    pub original_damage: f32,
    /// Whether this effect was applied by the object itself
    pub self_inflicted: bool,
}

impl DotEffect {
    /// Create new DoT effect
    pub fn new(
        effect_type: DotEffectType,
        source_id: ObjectID,
        damage_per_tick: f32,
        current_frame: u32,
        damage_interval_frames: u32,
        duration_frames: u32,
    ) -> Self {
        Self {
            effect_type,
            source_id,
            damage_per_tick,
            next_damage_frame: current_frame + damage_interval_frames,
            expiration_frame: current_frame + duration_frames,
            damage_interval_frames,
            original_damage: damage_per_tick,
            self_inflicted: false,
        }
    }

    /// Check if effect has expired
    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expiration_frame
    }

    /// Check if it's time to apply damage
    pub fn should_tick(&self, current_frame: u32) -> bool {
        current_frame >= self.next_damage_frame && !self.is_expired(current_frame)
    }

    /// Advance to next damage tick
    pub fn advance_tick(&mut self, current_frame: u32) {
        self.next_damage_frame = current_frame + self.damage_interval_frames;
    }

    /// Refresh effect duration (when reapplied)
    pub fn refresh(&mut self, current_frame: u32, duration_frames: u32, new_damage: f32) {
        // Extend duration
        self.expiration_frame = current_frame + duration_frames;

        // Update damage if higher
        if new_damage > self.damage_per_tick {
            self.damage_per_tick = new_damage;
            self.original_damage = new_damage;
        }

        // Reset next tick if not already scheduled soon
        if self.next_damage_frame > current_frame + self.damage_interval_frames {
            self.next_damage_frame = current_frame + self.damage_interval_frames;
        }
    }

    /// Get frames until next tick
    pub fn frames_until_next_tick(&self, current_frame: u32) -> u32 {
        if current_frame >= self.next_damage_frame {
            0
        } else {
            self.next_damage_frame - current_frame
        }
    }

    /// Get frames until expiration
    pub fn frames_until_expiration(&self, current_frame: u32) -> u32 {
        if current_frame >= self.expiration_frame {
            0
        } else {
            self.expiration_frame - current_frame
        }
    }

    /// Create damage info for this tick
    pub fn create_damage_info(&self) -> DamageInfo {
        let mut damage_info = DamageInfo::new();
        damage_info.input.source_id = self.source_id;
        damage_info.input.damage_type = self.effect_type.fx_damage_type(); // C++: DoT uses actual damage type (Poison/Flame), goes through armor
        damage_info.input.death_type = self.effect_type.death_type();
        damage_info.input.amount = self.damage_per_tick;
        damage_info.sync_from_input();
        damage_info
    }
}

/// Configuration for DoT effects
#[derive(Debug, Clone)]
pub struct DotConfig {
    /// Damage interval for poison (frames)
    pub poison_damage_interval: u32,
    /// Duration of poison effect (frames)
    pub poison_duration: u32,
    /// Damage interval for fire (frames)
    pub fire_damage_interval: u32,
    /// Duration of fire effect (frames)
    pub fire_duration: u32,
    /// Damage interval for radiation (frames)
    pub radiation_damage_interval: u32,
    /// Duration of radiation effect (frames)
    pub radiation_duration: u32,
    /// Damage multiplier for DoT ticks (fraction of original damage)
    pub damage_tick_multiplier: f32,
}

impl DotConfig {
    /// Create default DoT configuration matching C++ values
    pub fn new() -> Self {
        Self {
            // Poison: 1 second intervals, 10 second duration (30 FPS)
            poison_damage_interval: 30, // 1 second
            poison_duration: 300,       // 10 seconds
            // Fire: 0.5 second intervals, 5 second duration
            fire_damage_interval: 15, // 0.5 seconds
            fire_duration: 150,       // 5 seconds
            // Radiation: 2 second intervals, 15 second duration
            radiation_damage_interval: 60, // 2 seconds
            radiation_duration: 450,       // 15 seconds
            // DoT damage is typically 10-20% of original per tick
            damage_tick_multiplier: 0.15,
        }
    }

    /// Get interval for effect type
    pub fn get_interval(&self, effect_type: DotEffectType) -> u32 {
        match effect_type {
            DotEffectType::Poison | DotEffectType::PoisonBeta | DotEffectType::PoisonGamma => {
                self.poison_damage_interval
            }
            DotEffectType::Burning | DotEffectType::ParticleBeamBurn => self.fire_damage_interval,
            DotEffectType::Radiation | DotEffectType::Microwave => self.radiation_damage_interval,
        }
    }

    /// Get duration for effect type
    pub fn get_duration(&self, effect_type: DotEffectType) -> u32 {
        match effect_type {
            DotEffectType::Poison | DotEffectType::PoisonBeta | DotEffectType::PoisonGamma => {
                self.poison_duration
            }
            DotEffectType::Burning | DotEffectType::ParticleBeamBurn => self.fire_duration,
            DotEffectType::Radiation | DotEffectType::Microwave => self.radiation_duration,
        }
    }
}

impl Default for DotConfig {
    fn default() -> Self {
        Self::new()
    }
}

/// Manager for all damage-over-time effects in the game
#[derive(Debug)]
pub struct DotManager {
    /// Active effects by object ID
    effects: RwLock<HashMap<ObjectID, Vec<DotEffect>>>,
    /// Configuration
    config: DotConfig,
    /// Current game frame
    current_frame: RwLock<u32>,
}

impl DotManager {
    /// Create new DoT manager
    pub fn new(config: DotConfig) -> Self {
        Self {
            effects: RwLock::new(HashMap::new()),
            config,
            current_frame: RwLock::new(0),
        }
    }

    /// Create with default configuration
    pub fn with_defaults() -> Self {
        Self::new(DotConfig::new())
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

    /// Apply DoT effect to an object
    pub fn apply_effect(
        &self,
        object_id: ObjectID,
        effect_type: DotEffectType,
        source_id: ObjectID,
        base_damage: f32,
    ) -> GameLogicResult<()> {
        let current_frame = self.get_current_frame()?;
        let damage_per_tick = base_damage * self.config.damage_tick_multiplier;
        let interval = self.config.get_interval(effect_type);
        let duration = self.config.get_duration(effect_type);

        let mut effects = self.effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        let object_effects = effects.entry(object_id).or_insert_with(Vec::new);

        // Check if effect already exists
        if let Some(existing) = object_effects
            .iter_mut()
            .find(|e| e.effect_type == effect_type)
        {
            // Refresh existing effect
            existing.refresh(current_frame, duration, damage_per_tick);
        } else {
            // Check for conflicting effects
            object_effects.retain(|e| e.effect_type.can_stack_with(&effect_type));

            // Add new effect
            let effect = DotEffect::new(
                effect_type,
                source_id,
                damage_per_tick,
                current_frame,
                interval,
                duration,
            );
            object_effects.push(effect);
        }

        Ok(())
    }

    /// Remove all effects from an object
    pub fn clear_effects(&self, object_id: ObjectID) -> GameLogicResult<()> {
        let mut effects = self.effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        effects.remove(&object_id);
        Ok(())
    }

    /// Remove specific effect type from an object
    pub fn clear_effect_type(
        &self,
        object_id: ObjectID,
        effect_type: DotEffectType,
    ) -> GameLogicResult<()> {
        let mut effects = self.effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        if let Some(object_effects) = effects.get_mut(&object_id) {
            object_effects.retain(|e| e.effect_type != effect_type);
            if object_effects.is_empty() {
                effects.remove(&object_id);
            }
        }

        Ok(())
    }

    /// Update all effects and return damage to apply
    pub fn update_effects(
        &self,
        current_frame: u32,
    ) -> GameLogicResult<Vec<(ObjectID, DamageInfo)>> {
        self.set_current_frame(current_frame)?;

        let mut effects = self.effects.write().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        let mut damage_to_apply = Vec::new();

        // Iterate through all objects with effects
        for (object_id, object_effects) in effects.iter_mut() {
            // Remove expired effects
            object_effects.retain(|e| !e.is_expired(current_frame));

            // Apply ticking effects
            for effect in object_effects.iter_mut() {
                if effect.should_tick(current_frame) {
                    damage_to_apply.push((*object_id, effect.create_damage_info()));
                    effect.advance_tick(current_frame);
                }
            }
        }

        // Clean up objects with no effects
        effects.retain(|_, effects| !effects.is_empty());

        Ok(damage_to_apply)
    }

    /// Get active effects for an object
    pub fn get_effects(&self, object_id: ObjectID) -> GameLogicResult<Vec<DotEffect>> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        Ok(effects.get(&object_id).cloned().unwrap_or_default())
    }

    /// Check if object has any DoT effects
    pub fn has_effects(&self, object_id: ObjectID) -> GameLogicResult<bool> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        Ok(effects.contains_key(&object_id))
    }

    /// Check if object has specific effect type
    pub fn has_effect_type(
        &self,
        object_id: ObjectID,
        effect_type: DotEffectType,
    ) -> GameLogicResult<bool> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        if let Some(object_effects) = effects.get(&object_id) {
            Ok(object_effects.iter().any(|e| e.effect_type == effect_type))
        } else {
            Ok(false)
        }
    }

    /// Get total DoT damage per second for an object
    pub fn get_dps(&self, object_id: ObjectID) -> GameLogicResult<f32> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        let mut total_dps = 0.0;
        if let Some(object_effects) = effects.get(&object_id) {
            let current_frame = *self.current_frame.read().map_err(|e| {
                GameLogicError::Threading(format!("Failed to acquire frame lock: {}", e))
            })?;

            for effect in object_effects.iter() {
                if !effect.is_expired(current_frame) {
                    let ticks_per_second =
                        FRAMES_PER_SECOND as f32 / effect.damage_interval_frames as f32;
                    total_dps += effect.damage_per_tick * ticks_per_second;
                }
            }
        }

        Ok(total_dps)
    }

    /// Get statistics about active effects
    pub fn get_stats(&self) -> GameLogicResult<DotStats> {
        let effects = self.effects.read().map_err(|e| {
            GameLogicError::Threading(format!("Failed to acquire effects lock: {}", e))
        })?;

        let current_frame = self.get_current_frame()?;

        let mut stats = DotStats::default();
        stats.total_affected_objects = effects.len();

        for object_effects in effects.values() {
            for effect in object_effects {
                if !effect.is_expired(current_frame) {
                    stats.total_active_effects += 1;
                    match effect.effect_type {
                        DotEffectType::Poison
                        | DotEffectType::PoisonBeta
                        | DotEffectType::PoisonGamma => stats.poison_effects += 1,
                        DotEffectType::Burning | DotEffectType::ParticleBeamBurn => {
                            stats.fire_effects += 1
                        }
                        DotEffectType::Radiation | DotEffectType::Microwave => {
                            stats.radiation_effects += 1
                        }
                    }
                }
            }
        }

        Ok(stats)
    }
}

impl Default for DotManager {
    fn default() -> Self {
        Self::with_defaults()
    }
}

pub static DOT_MANAGER: Lazy<DotManager> = Lazy::new(DotManager::with_defaults);

pub fn update_dot_effects(current_frame: u32) -> GameLogicResult<()> {
    let damage_to_apply = DOT_MANAGER.update_effects(current_frame)?;

    for (object_id, damage_info) in damage_to_apply {
        let Some(obj_arc) = crate::helpers::TheGameLogic::find_object_by_id(object_id) else {
            continue;
        };
        let mut obj_guard = obj_arc
            .write()
            .map_err(|_| GameLogicError::Threading("DoT target lock failed".to_string()))?;

        let mut engine_info = crate::damage::DamageInfo::new();
        engine_info.input.source_id = damage_info.input.source_id;
        engine_info.input.source_player_mask =
            crate::damage::PlayerMaskType::from_bits_truncate(damage_info.input.source_player_mask);
        engine_info.input.damage_type =
            crate::damage::DamageType::from_u32(damage_info.input.damage_type as u32);
        engine_info.input.damage_fx_override =
            crate::damage::DamageType::from_u32(damage_info.input.damage_fx_override as u32);
        engine_info.input.damage_status_type = crate::common::ObjectStatusTypes::None;
        engine_info.input.death_type =
            crate::damage::DeathType::from_u32(damage_info.input.death_type as u32);
        engine_info.input.amount = damage_info.input.amount;
        engine_info.input.kill = damage_info.input.kill;
        engine_info.input.shock_wave_vector = damage_info.input.shock_wave_vector;
        engine_info.input.shock_wave_amount = damage_info.input.shock_wave_amount;
        engine_info.input.shock_wave_radius = damage_info.input.shock_wave_radius;
        engine_info.input.shock_wave_taper_off = damage_info.input.shock_wave_taper_off;
        engine_info.sync_from_input();

        let _ = obj_guard.attempt_damage(&mut engine_info);
    }

    Ok(())
}

/// Statistics about active DoT effects
#[derive(Debug, Clone, Default)]
pub struct DotStats {
    /// Total objects with DoT effects
    pub total_affected_objects: usize,
    /// Total active effects (may be multiple per object)
    pub total_active_effects: usize,
    /// Number of poison effects
    pub poison_effects: usize,
    /// Number of fire/burn effects
    pub fire_effects: usize,
    /// Number of radiation effects
    pub radiation_effects: usize,
}

/// Helper functions for common DoT operations
impl DotManager {
    /// Apply poison from damage
    pub fn apply_poison_damage(
        &self,
        object_id: ObjectID,
        source_id: ObjectID,
        damage: f32,
    ) -> GameLogicResult<()> {
        self.apply_effect(object_id, DotEffectType::Poison, source_id, damage)
    }

    /// Apply fire damage
    pub fn apply_fire_damage(
        &self,
        object_id: ObjectID,
        source_id: ObjectID,
        damage: f32,
    ) -> GameLogicResult<()> {
        self.apply_effect(object_id, DotEffectType::Burning, source_id, damage)
    }

    /// Apply radiation damage
    pub fn apply_radiation_damage(
        &self,
        object_id: ObjectID,
        source_id: ObjectID,
        damage: f32,
    ) -> GameLogicResult<()> {
        self.apply_effect(object_id, DotEffectType::Radiation, source_id, damage)
    }

    /// Cure all DoT effects (healing removes DoT)
    pub fn cure_all_effects(&self, object_id: ObjectID) -> GameLogicResult<()> {
        self.clear_effects(object_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dot_effect_creation() {
        let effect = DotEffect::new(DotEffectType::Poison, 123, 10.0, 0, 30, 300);

        assert_eq!(effect.effect_type, DotEffectType::Poison);
        assert_eq!(effect.source_id, 123);
        assert_eq!(effect.damage_per_tick, 10.0);
        assert_eq!(effect.next_damage_frame, 30);
        assert_eq!(effect.expiration_frame, 300);
    }

    #[test]
    fn test_dot_effect_ticking() {
        let mut effect = DotEffect::new(DotEffectType::Poison, 123, 10.0, 0, 30, 300);

        assert!(!effect.should_tick(0));
        assert!(!effect.should_tick(29));
        assert!(effect.should_tick(30));
        assert!(effect.should_tick(31));

        effect.advance_tick(30);
        assert_eq!(effect.next_damage_frame, 60);
    }

    #[test]
    fn test_dot_effect_expiration() {
        let effect = DotEffect::new(DotEffectType::Poison, 123, 10.0, 0, 30, 300);

        assert!(!effect.is_expired(0));
        assert!(!effect.is_expired(299));
        assert!(effect.is_expired(300));
        assert!(effect.is_expired(301));
    }

    #[test]
    fn test_dot_effect_refresh() {
        let mut effect = DotEffect::new(DotEffectType::Poison, 123, 10.0, 0, 30, 300);

        effect.refresh(100, 300, 15.0);

        assert_eq!(effect.expiration_frame, 400); // 100 + 300
        assert_eq!(effect.damage_per_tick, 15.0); // Upgraded
    }

    #[test]
    fn test_dot_effect_stacking() {
        assert!(DotEffectType::Poison.can_stack_with(&DotEffectType::Poison));
        assert!(DotEffectType::Poison.can_stack_with(&DotEffectType::Burning));
        assert!(!DotEffectType::Poison.can_stack_with(&DotEffectType::PoisonBeta));
        assert!(!DotEffectType::Burning.can_stack_with(&DotEffectType::ParticleBeamBurn));
    }

    #[test]
    fn test_dot_manager_apply_effect() {
        let manager = DotManager::with_defaults();
        manager.set_current_frame(0).unwrap();

        manager.apply_poison_damage(100, 200, 50.0).unwrap();

        assert!(manager.has_effects(100).unwrap());
        assert!(manager.has_effect_type(100, DotEffectType::Poison).unwrap());
        assert!(!manager
            .has_effect_type(100, DotEffectType::Burning)
            .unwrap());
    }

    #[test]
    fn test_dot_manager_update() {
        let manager = DotManager::with_defaults();
        manager.set_current_frame(0).unwrap();

        manager.apply_poison_damage(100, 200, 50.0).unwrap();

        // No damage at frame 0
        let damage = manager.update_effects(0).unwrap();
        assert_eq!(damage.len(), 0);

        // No damage at frame 29
        let damage = manager.update_effects(29).unwrap();
        assert_eq!(damage.len(), 0);

        // Damage at frame 30 (first tick)
        let damage = manager.update_effects(30).unwrap();
        assert_eq!(damage.len(), 1);
        assert_eq!(damage[0].0, 100);

        // No damage at frame 31
        let damage = manager.update_effects(31).unwrap();
        assert_eq!(damage.len(), 0);

        // Damage at frame 60 (second tick)
        let damage = manager.update_effects(60).unwrap();
        assert_eq!(damage.len(), 1);
    }

    #[test]
    fn test_dot_manager_expiration() {
        let manager = DotManager::with_defaults();
        manager.set_current_frame(0).unwrap();

        manager.apply_poison_damage(100, 200, 50.0).unwrap();

        // Effect active at frame 299
        let damage = manager.update_effects(299).unwrap();
        assert!(manager.has_effects(100).unwrap());

        // Effect expired at frame 300
        let damage = manager.update_effects(300).unwrap();
        assert!(!manager.has_effects(100).unwrap());
    }

    #[test]
    fn test_dot_manager_multiple_effects() {
        let manager = DotManager::with_defaults();
        manager.set_current_frame(0).unwrap();

        // Apply poison and fire
        manager.apply_poison_damage(100, 200, 50.0).unwrap();
        manager.apply_fire_damage(100, 200, 30.0).unwrap();

        let effects = manager.get_effects(100).unwrap();
        assert_eq!(effects.len(), 2);
    }

    #[test]
    fn test_dot_manager_refresh() {
        let manager = DotManager::with_defaults();
        manager.set_current_frame(0).unwrap();

        manager.apply_poison_damage(100, 200, 50.0).unwrap();
        manager.set_current_frame(150).unwrap();
        manager.apply_poison_damage(100, 200, 75.0).unwrap();

        let effects = manager.get_effects(100).unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].damage_per_tick, 75.0 * 0.15); // Should be upgraded
    }

    #[test]
    fn test_dot_manager_cure() {
        let manager = DotManager::with_defaults();
        manager.set_current_frame(0).unwrap();

        manager.apply_poison_damage(100, 200, 50.0).unwrap();
        assert!(manager.has_effects(100).unwrap());

        manager.cure_all_effects(100).unwrap();
        assert!(!manager.has_effects(100).unwrap());
    }

    #[test]
    fn test_dot_manager_dps_calculation() {
        let manager = DotManager::with_defaults();
        manager.set_current_frame(0).unwrap();

        // Poison: 7.5 damage every 30 frames = 7.5 DPS
        manager.apply_poison_damage(100, 200, 50.0).unwrap();

        let dps = manager.get_dps(100).unwrap();
        assert!((dps - 7.5).abs() < 0.01);
    }

    #[test]
    fn test_dot_manager_stats() {
        let manager = DotManager::with_defaults();
        manager.set_current_frame(0).unwrap();

        manager.apply_poison_damage(100, 200, 50.0).unwrap();
        manager.apply_fire_damage(101, 200, 30.0).unwrap();
        manager.apply_radiation_damage(102, 200, 40.0).unwrap();

        let stats = manager.get_stats().unwrap();
        assert_eq!(stats.total_affected_objects, 3);
        assert_eq!(stats.total_active_effects, 3);
        assert_eq!(stats.poison_effects, 1);
        assert_eq!(stats.fire_effects, 1);
        assert_eq!(stats.radiation_effects, 1);
    }

    #[test]
    fn test_death_types() {
        assert_eq!(DotEffectType::Poison.death_type(), DeathType::Poisoned);
        assert_eq!(DotEffectType::Burning.death_type(), DeathType::Burned);
        assert_eq!(
            DotEffectType::PoisonBeta.death_type(),
            DeathType::PoisonedBeta
        );
        assert_eq!(
            DotEffectType::PoisonGamma.death_type(),
            DeathType::PoisonedGamma
        );
    }

    #[test]
    fn test_fx_damage_types() {
        assert_eq!(DotEffectType::Poison.fx_damage_type(), DamageType::Poison);
        assert_eq!(DotEffectType::Burning.fx_damage_type(), DamageType::Flame);
        assert_eq!(
            DotEffectType::ParticleBeamBurn.fx_damage_type(),
            DamageType::ParticleBeam
        );
    }
}
