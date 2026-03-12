//! Stealth Special Power Manager System
//!
//! Manages temporary stealth grants, area-effect stealth effects, and spy vision effects.
//! Provides the special power system with methods to:
//! - Grant temporary stealth to individual units
//! - Grant permanent stealth to units
//! - Create expanding area-effect stealth zones
//! - Grant spy vision capabilities with optional KindOf filtering
//! - Track and expire stealth grants over time
//!
//! Faithful to C++ implementation (GrantStealthBehavior and SpyVisionSpecialPower)

use crate::common::{ObjectID, UnsignedInt};
use log::{debug, trace, warn};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::sync::OnceLock;

/// Permanent stealth marker (matches C++ convention of -1)
pub const PERMANENT_STEALTH: i32 = -1;

/// Represents a stealth grant to a specific unit
#[derive(Debug, Clone)]
pub struct StealthGrant {
    /// ObjectID of the unit being granted stealth
    pub granted_to_id: ObjectID,

    /// ObjectID of the unit/object granting the stealth
    pub granted_by_id: ObjectID,

    /// Remaining frames of stealth (-1 = permanent)
    pub frames_remaining: i32,

    /// Frame number when stealth was granted
    pub start_frame: u32,

    /// Whether this grant is an area-effect stealth
    pub is_area_effect: bool,
}

impl StealthGrant {
    /// Create a new temporary stealth grant
    pub fn new_temporary(
        granted_to_id: ObjectID,
        granted_by_id: ObjectID,
        duration_frames: i32,
        start_frame: u32,
    ) -> Self {
        Self {
            granted_to_id,
            granted_by_id,
            frames_remaining: duration_frames,
            start_frame,
            is_area_effect: false,
        }
    }

    /// Create a new permanent stealth grant
    pub fn new_permanent(
        granted_to_id: ObjectID,
        granted_by_id: ObjectID,
        start_frame: u32,
    ) -> Self {
        Self {
            granted_to_id,
            granted_by_id,
            frames_remaining: PERMANENT_STEALTH,
            start_frame,
            is_area_effect: false,
        }
    }

    /// Check if this grant is still active
    pub fn is_active(&self) -> bool {
        self.frames_remaining != 0
    }

    /// Check if this grant is permanent
    pub fn is_permanent(&self) -> bool {
        self.frames_remaining == PERMANENT_STEALTH
    }
}

/// Represents an expanding area-effect stealth zone
#[derive(Debug, Clone)]
pub struct AreaStealthEffect {
    /// Unique identifier for this area effect
    pub area_id: u32,

    /// Center position of the stealth effect
    pub center_position: Coord3D,

    /// Current radius of the effect
    pub current_radius: f32,

    /// Final/target radius of the effect
    pub final_radius: f32,

    /// Number of frames over which the radius grows to final_radius
    pub radius_grow_frames: u32,

    /// Current elapsed frames during radius growth
    pub elapsed_grow_frames: u32,

    /// ObjectIDs of units currently affected by this area effect
    pub affected_units: HashSet<ObjectID>,

    /// Optional particle system to visualize the effect
    pub particle_system: String,

    /// KindOf mask for unit filtering (what types can be affected)
    pub kindof_mask: u32,

    /// ObjectID of the unit that created this area effect
    pub created_by_id: ObjectID,
}

impl AreaStealthEffect {
    /// Create a new area stealth effect
    pub fn new(
        area_id: u32,
        center_position: Coord3D,
        start_radius: f32,
        final_radius: f32,
        radius_grow_frames: u32,
        kindof_mask: u32,
        created_by_id: ObjectID,
    ) -> Self {
        Self {
            area_id,
            center_position,
            current_radius: start_radius,
            final_radius,
            radius_grow_frames,
            elapsed_grow_frames: 0,
            affected_units: HashSet::new(),
            particle_system: String::new(),
            kindof_mask,
            created_by_id,
        }
    }

    /// Set the particle system for this effect
    pub fn set_particle_system(&mut self, particle_system: String) {
        self.particle_system = particle_system;
    }

    /// Check if this area effect is still growing
    pub fn is_growing(&self) -> bool {
        self.elapsed_grow_frames < self.radius_grow_frames
            && self.current_radius < self.final_radius
    }

    /// Update radius growth for this frame
    pub fn update_radius(&mut self) {
        if self.is_growing() {
            self.elapsed_grow_frames += 1;
            let progress = self.elapsed_grow_frames as f32 / self.radius_grow_frames.max(1) as f32;
            self.current_radius = self
                .current_radius
                .max(self.final_radius * progress.min(1.0));
        }
    }
}

/// Represents a spy vision grant that shares vision of specific units with a player
#[derive(Debug, Clone)]
pub struct SpyVisionGrant {
    /// Player ID receiving the spy vision
    pub granted_to_player: u32,

    /// Remaining frames (-1 = permanent)
    pub frames_remaining: i32,

    /// KindOf mask for which unit types to spy on
    pub spy_on_kindof: u32,

    /// Player IDs from which vision is being shared
    pub shared_from_players: Vec<u32>,

    /// Frame when the grant was created
    pub start_frame: u32,
}

impl SpyVisionGrant {
    /// Create a new spy vision grant
    pub fn new(
        granted_to_player: u32,
        duration_frames: i32,
        spy_on_kindof: u32,
        start_frame: u32,
    ) -> Self {
        Self {
            granted_to_player,
            frames_remaining: duration_frames,
            spy_on_kindof,
            shared_from_players: Vec::new(),
            start_frame,
        }
    }

    /// Check if grant is still active
    pub fn is_active(&self) -> bool {
        self.frames_remaining != 0
    }

    /// Check if grant is permanent
    pub fn is_permanent(&self) -> bool {
        self.frames_remaining == PERMANENT_STEALTH
    }
}

/// Coordinate system for 3D positions
#[derive(Debug, Clone, Copy)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    /// Create new coordinate
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    /// Calculate distance to another coordinate
    pub fn distance_to(&self, other: &Coord3D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Stealth Special Power Manager singleton
///
/// Manages all temporary stealth grants, area-effect stealth zones, and spy vision effects.
/// Thread-safe access via mutex.
pub struct StealthSpecialPowerManager {
    /// Active stealth grants per unit
    active_grants: HashMap<ObjectID, StealthGrant>,

    /// Area-effect stealth zones
    area_effects: Vec<AreaStealthEffect>,

    /// Next available area effect ID
    next_area_id: u32,

    /// Spy vision grants per player
    spy_vision_grants: HashMap<u32, SpyVisionGrant>,

    /// Last frame the manager was updated
    last_update_frame: u32,
}

impl StealthSpecialPowerManager {
    /// Create new StealthSpecialPowerManager
    pub fn new() -> Self {
        Self {
            active_grants: HashMap::new(),
            area_effects: Vec::new(),
            next_area_id: 1,
            spy_vision_grants: HashMap::new(),
            last_update_frame: 0,
        }
    }

    /// Grant temporary stealth to a unit
    pub fn grant_stealth(
        &mut self,
        to_id: ObjectID,
        by_id: ObjectID,
        duration_frames: i32,
        current_frame: u32,
    ) -> Result<(), String> {
        if duration_frames <= 0 {
            return Err("Duration must be positive".to_string());
        }

        let grant = StealthGrant::new_temporary(to_id, by_id, duration_frames, current_frame);
        self.active_grants.insert(to_id, grant);
        trace!(
            "Granted stealth to unit {} for {} frames",
            to_id,
            duration_frames
        );
        Ok(())
    }

    /// Grant permanent stealth to a unit
    pub fn grant_stealth_permanent(
        &mut self,
        to_id: ObjectID,
        by_id: ObjectID,
        current_frame: u32,
    ) -> Result<(), String> {
        let grant = StealthGrant::new_permanent(to_id, by_id, current_frame);
        self.active_grants.insert(to_id, grant);
        trace!("Granted permanent stealth to unit {}", to_id);
        Ok(())
    }

    /// Create an area-effect stealth zone
    pub fn create_area_stealth(
        &mut self,
        center: Coord3D,
        start_radius: f32,
        final_radius: f32,
        radius_grow_frames: u32,
        kindof_mask: u32,
        created_by_id: ObjectID,
    ) -> Result<u32, String> {
        if final_radius < 0.0 {
            return Err("Final radius cannot be negative".to_string());
        }

        let area_id = self.next_area_id;
        self.next_area_id += 1;

        let area = AreaStealthEffect::new(
            area_id,
            center,
            start_radius,
            final_radius,
            radius_grow_frames,
            kindof_mask,
            created_by_id,
        );

        self.area_effects.push(area);
        trace!(
            "Created area stealth effect {} at radius {}",
            area_id,
            final_radius
        );
        Ok(area_id)
    }

    /// Get a specific area stealth effect
    pub fn get_area_stealth(&self, area_id: u32) -> Result<AreaStealthEffect, String> {
        self.area_effects
            .iter()
            .find(|a| a.area_id == area_id)
            .cloned()
            .ok_or_else(|| format!("Area stealth effect {} not found", area_id))
    }

    /// Get mutable reference to area stealth effect
    pub fn get_area_stealth_mut(&mut self, area_id: u32) -> Result<&mut AreaStealthEffect, String> {
        self.area_effects
            .iter_mut()
            .find(|a| a.area_id == area_id)
            .ok_or_else(|| format!("Area stealth effect {} not found", area_id))
    }

    /// Add a unit to an area stealth effect
    pub fn add_unit_to_area(&mut self, area_id: u32, unit_id: ObjectID) -> Result<(), String> {
        let area = self.get_area_stealth_mut(area_id)?;
        area.affected_units.insert(unit_id);
        trace!("Added unit {} to area stealth effect {}", unit_id, area_id);
        Ok(())
    }

    /// Remove a unit from an area stealth effect
    pub fn remove_unit_from_area(&mut self, area_id: u32, unit_id: ObjectID) -> Result<(), String> {
        let area = self.get_area_stealth_mut(area_id)?;
        area.affected_units.remove(&unit_id);
        trace!(
            "Removed unit {} from area stealth effect {}",
            unit_id,
            area_id
        );
        Ok(())
    }

    /// Grant spy vision capability to a player
    pub fn grant_spy_vision(
        &mut self,
        to_player: u32,
        duration_frames: i32,
        spy_on_kindof: u32,
        current_frame: u32,
    ) -> Result<(), String> {
        if duration_frames <= 0 && duration_frames != PERMANENT_STEALTH {
            return Err("Duration must be positive or permanent".to_string());
        }

        let grant = SpyVisionGrant::new(to_player, duration_frames, spy_on_kindof, current_frame);
        self.spy_vision_grants.insert(to_player, grant);
        trace!(
            "Granted spy vision to player {} for {} frames",
            to_player,
            duration_frames
        );
        Ok(())
    }

    /// Add a player as a source of shared vision
    pub fn add_spy_vision_source(
        &mut self,
        to_player: u32,
        from_player: u32,
    ) -> Result<(), String> {
        let grant = self
            .spy_vision_grants
            .get_mut(&to_player)
            .ok_or_else(|| format!("No spy vision grant for player {}", to_player))?;

        if !grant.shared_from_players.contains(&from_player) {
            grant.shared_from_players.push(from_player);
            trace!(
                "Added player {} as spy vision source for player {}",
                from_player,
                to_player
            );
        }
        Ok(())
    }

    /// Revoke stealth grant from a unit
    pub fn revoke_stealth_grant(&mut self, to_id: ObjectID) -> Result<(), String> {
        if self.active_grants.remove(&to_id).is_some() {
            trace!("Revoked stealth grant for unit {}", to_id);
            Ok(())
        } else {
            Err(format!("No stealth grant found for unit {}", to_id))
        }
    }

    /// Check if a unit has stealth granted
    pub fn is_granted_stealth(&self, object_id: ObjectID) -> Result<bool, String> {
        Ok(self.active_grants.contains_key(&object_id))
    }

    /// Get remaining frames for a stealth grant
    pub fn get_remaining_frames(&self, object_id: ObjectID) -> Result<i32, String> {
        self.active_grants
            .get(&object_id)
            .map(|g| g.frames_remaining)
            .ok_or_else(|| format!("No stealth grant for object {}", object_id))
    }

    /// Remove expired area effects
    fn remove_expired_areas(&mut self) {
        self.area_effects
            .retain(|area| !area.affected_units.is_empty() || area.is_growing());
        trace!("Cleaned up expired area stealth effects");
    }

    /// Update all stealth grants (decrement timers and expire old grants)
    pub fn update_grants(&mut self, current_frame: u32) -> Result<(), String> {
        let delta_frames = current_frame.saturating_sub(self.last_update_frame);
        self.last_update_frame = current_frame;
        if delta_frames == 0 {
            return Ok(());
        }

        // Update and expire stealth grants
        let mut expired_units = Vec::new();
        for (unit_id, grant) in self.active_grants.iter_mut() {
            if !grant.is_permanent() {
                grant.frames_remaining -= delta_frames as i32;
                if grant.frames_remaining <= 0 {
                    expired_units.push(*unit_id);
                }
            }
        }

        for unit_id in expired_units {
            self.active_grants.remove(&unit_id);
            debug!("Stealth grant expired for unit {}", unit_id);
        }

        // Update and expire spy vision grants
        let mut expired_players = Vec::new();
        for (player_id, grant) in self.spy_vision_grants.iter_mut() {
            if !grant.is_permanent() {
                grant.frames_remaining -= delta_frames as i32;
                if grant.frames_remaining <= 0 {
                    expired_players.push(*player_id);
                }
            }
        }

        for player_id in expired_players {
            self.spy_vision_grants.remove(&player_id);
            debug!("Spy vision grant expired for player {}", player_id);
        }

        trace!("Updated stealth grants at frame {}", current_frame);
        Ok(())
    }

    /// Update all area effects (grow radius and track affected units)
    pub fn update_area_effects(&mut self, current_frame: u32) -> Result<(), String> {
        for area in self.area_effects.iter_mut() {
            area.update_radius();
        }

        self.remove_expired_areas();
        trace!("Updated area stealth effects at frame {}", current_frame);
        Ok(())
    }

    /// Get all active area effects
    pub fn get_all_area_effects(&self) -> Vec<AreaStealthEffect> {
        self.area_effects.clone()
    }

    /// Get all active grants
    pub fn get_all_active_grants(&self) -> Vec<StealthGrant> {
        self.active_grants.values().cloned().collect()
    }

    /// Get spy vision grant for a player
    pub fn get_spy_vision_grant(&self, player_id: u32) -> Option<SpyVisionGrant> {
        self.spy_vision_grants.get(&player_id).cloned()
    }

    /// Check if a player has active spy vision
    pub fn has_spy_vision(&self, player_id: u32) -> bool {
        self.spy_vision_grants
            .get(&player_id)
            .map(|g| g.is_active())
            .unwrap_or(false)
    }
}

impl Default for StealthSpecialPowerManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global singleton accessor for StealthSpecialPowerManager
static STEALTH_SPECIAL_POWER_MANAGER: OnceLock<Mutex<StealthSpecialPowerManager>> = OnceLock::new();

/// Get the global StealthSpecialPowerManager singleton
pub fn get_stealth_special_power_manager() -> &'static Mutex<StealthSpecialPowerManager> {
    STEALTH_SPECIAL_POWER_MANAGER.get_or_init(|| Mutex::new(StealthSpecialPowerManager::new()))
}

#[cfg(test)]
mod stealth_special_power_tests {
    use super::*;

    #[test]
    fn test_grant_stealth_temporary() {
        let mut manager = StealthSpecialPowerManager::new();
        assert!(manager.grant_stealth(1, 2, 100, 0).is_ok());
        assert!(manager.is_granted_stealth(1).is_ok());
        assert_eq!(manager.get_remaining_frames(1).unwrap(), 100);
    }

    #[test]
    fn test_grant_stealth_permanent() {
        let mut manager = StealthSpecialPowerManager::new();
        assert!(manager.grant_stealth_permanent(1, 2, 0).is_ok());
        assert!(manager.is_granted_stealth(1).is_ok());
        assert_eq!(manager.get_remaining_frames(1).unwrap(), PERMANENT_STEALTH);
    }

    #[test]
    fn test_stealth_grant_expiration() {
        let mut manager = StealthSpecialPowerManager::new();
        manager.grant_stealth(1, 2, 5, 0).unwrap();

        // Update multiple frames
        for frame in 1..=5 {
            manager.update_grants(frame).unwrap();
            if frame < 5 {
                assert!(manager.is_granted_stealth(1).is_ok());
            }
        }

        // After 5 frames, grant should expire
        assert!(!manager.is_granted_stealth(1).unwrap());
    }

    #[test]
    fn test_area_stealth_creation() {
        let mut manager = StealthSpecialPowerManager::new();
        let center = Coord3D::new(100.0, 200.0, 0.0);
        let area_id = manager
            .create_area_stealth(center, 0.0, 200.0, 30, 0xFFFFFFFF, 5)
            .unwrap();

        assert_eq!(area_id, 1);
        let area = manager.get_area_stealth(area_id).unwrap();
        assert_eq!(area.final_radius, 200.0);
    }

    #[test]
    fn test_area_radius_growth() {
        let mut manager = StealthSpecialPowerManager::new();
        let center = Coord3D::new(100.0, 200.0, 0.0);
        let area_id = manager
            .create_area_stealth(center, 0.0, 100.0, 100, 0xFFFFFFFF, 5)
            .unwrap();

        // Update a few frames
        manager.update_area_effects(0).unwrap();
        manager.update_area_effects(1).unwrap();

        let area = manager.get_area_stealth(area_id).unwrap();
        assert!(area.current_radius > 0.0);
        assert!(area.current_radius < 100.0);
    }

    #[test]
    fn test_area_stealth_affected_units() {
        let mut manager = StealthSpecialPowerManager::new();
        let center = Coord3D::new(100.0, 200.0, 0.0);
        let area_id = manager
            .create_area_stealth(center, 0.0, 200.0, 30, 0xFFFFFFFF, 5)
            .unwrap();

        // Add units
        manager.add_unit_to_area(area_id, 10).unwrap();
        manager.add_unit_to_area(area_id, 11).unwrap();

        let area = manager.get_area_stealth(area_id).unwrap();
        assert_eq!(area.affected_units.len(), 2);
        assert!(area.affected_units.contains(&10));
        assert!(area.affected_units.contains(&11));

        // Remove unit
        manager.remove_unit_from_area(area_id, 10).unwrap();
        let area = manager.get_area_stealth(area_id).unwrap();
        assert_eq!(area.affected_units.len(), 1);
    }

    #[test]
    fn test_spy_vision_grant() {
        let mut manager = StealthSpecialPowerManager::new();
        assert!(manager.grant_spy_vision(0, 100, 0xFFFFFFFF, 0).is_ok());

        assert!(manager.has_spy_vision(0));
        let grant = manager.get_spy_vision_grant(0).unwrap();
        assert_eq!(grant.frames_remaining, 100);
    }

    #[test]
    fn test_spy_vision_expiration() {
        let mut manager = StealthSpecialPowerManager::new();
        manager.grant_spy_vision(0, 3, 0xFFFFFFFF, 0).unwrap();

        // Update frames
        for frame in 1..=3 {
            manager.update_grants(frame).unwrap();
            if frame < 3 {
                assert!(manager.has_spy_vision(0));
            }
        }

        assert!(!manager.has_spy_vision(0));
    }

    #[test]
    fn test_multiple_grants() {
        let mut manager = StealthSpecialPowerManager::new();

        // Grant stealth to multiple units
        manager.grant_stealth(1, 0, 50, 0).unwrap();
        manager.grant_stealth(2, 0, 100, 0).unwrap();
        manager.grant_stealth_permanent(3, 0, 0).unwrap();

        assert!(manager.is_granted_stealth(1).unwrap());
        assert!(manager.is_granted_stealth(2).unwrap());
        assert!(manager.is_granted_stealth(3).unwrap());

        // Expire unit 1 (50 frames)
        for frame in 1..=50 {
            manager.update_grants(frame).unwrap();
        }

        assert!(!manager.is_granted_stealth(1).unwrap());
        assert!(manager.is_granted_stealth(2).unwrap());
        assert!(manager.is_granted_stealth(3).unwrap());
    }

    #[test]
    fn test_revoke_grant() {
        let mut manager = StealthSpecialPowerManager::new();
        manager.grant_stealth(1, 2, 100, 0).unwrap();
        assert!(manager.is_granted_stealth(1).unwrap());

        assert!(manager.revoke_stealth_grant(1).is_ok());
        assert!(!manager.is_granted_stealth(1).unwrap());

        // Revoking non-existent grant should fail
        assert!(manager.revoke_stealth_grant(1).is_err());
    }

    #[test]
    fn test_update_frame_progression() {
        let mut manager = StealthSpecialPowerManager::new();
        manager.grant_stealth(1, 2, 10, 0).unwrap();

        assert_eq!(manager.get_remaining_frames(1).unwrap(), 10);

        for frame in 1..=10 {
            manager.update_grants(frame).unwrap();
            let remaining = manager.get_remaining_frames(1);
            if frame < 10 {
                assert!(remaining.is_ok());
                assert_eq!(remaining.unwrap(), 10 - frame as i32);
            } else {
                assert!(remaining.is_err());
            }
        }
    }

    #[test]
    fn test_kindof_filtering() {
        let mut manager = StealthSpecialPowerManager::new();
        let center = Coord3D::new(0.0, 0.0, 0.0);

        // Create area that only affects infantry (kindof mask)
        let kindof_mask = 0x00000001; // Infantry flag
        let area_id = manager
            .create_area_stealth(center, 0.0, 100.0, 30, kindof_mask, 5)
            .unwrap();

        let area = manager.get_area_stealth(area_id).unwrap();
        assert_eq!(area.kindof_mask, 0x00000001);
    }

    #[test]
    fn test_spy_vision_multiple_sources() {
        let mut manager = StealthSpecialPowerManager::new();
        manager.grant_spy_vision(0, 100, 0xFFFFFFFF, 0).unwrap();

        manager.add_spy_vision_source(0, 1).unwrap();
        manager.add_spy_vision_source(0, 2).unwrap();

        let grant = manager.get_spy_vision_grant(0).unwrap();
        assert_eq!(grant.shared_from_players.len(), 2);
        assert!(grant.shared_from_players.contains(&1));
        assert!(grant.shared_from_players.contains(&2));
    }

    #[test]
    fn test_coord3d_distance() {
        let c1 = Coord3D::new(0.0, 0.0, 0.0);
        let c2 = Coord3D::new(3.0, 4.0, 0.0);

        let distance = c1.distance_to(&c2);
        assert!((distance - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_area_effect_is_growing() {
        let mut area = AreaStealthEffect::new(
            1,
            Coord3D::new(0.0, 0.0, 0.0),
            0.0,
            100.0,
            50,
            0xFFFFFFFF,
            5,
        );

        assert!(area.is_growing());

        // Simulate growth
        for _ in 0..50 {
            area.update_radius();
        }

        assert!(!area.is_growing());
    }

    #[test]
    fn test_permanent_stealth_no_expiration() {
        let mut manager = StealthSpecialPowerManager::new();
        manager.grant_stealth_permanent(1, 2, 0).unwrap();

        // Update many frames
        for frame in 1..=1000 {
            manager.update_grants(frame).unwrap();
        }

        // Should still be active
        assert!(manager.is_granted_stealth(1).unwrap());
        assert_eq!(manager.get_remaining_frames(1).unwrap(), PERMANENT_STEALTH);
    }
}
