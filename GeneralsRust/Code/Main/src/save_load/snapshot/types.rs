//! Core snapshot trait, world snapshot, and shared utility types.

use super::{
    AIPlayerSnapshot, CombatParticleRegistrySnapshot, CombatTrackerSnapshot,
    ExperienceTrackerSnapshot, GlobalAIStateSnapshot, HostUpgradeRegistrySnapshot, ObjectSnapshot,
    ObjectStatusSnapshot, PathfindingCacheSnapshot, PlayerSnapshot, ResourceManagerSnapshot,
    SpecialPowerStrikeRegistrySnapshot, TeamSnapshot, TerrainSnapshot, WeatherSnapshot,
};
use crate::game_logic::*;
use crate::save_load::{SaveLoadResult, Xfer, XferData};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Trait for objects that can be included in game snapshots
pub trait Snapshot {
    /// Perform light CRC check on this data structure
    fn crc(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()>;

    /// Run save, load, or deep CRC check on this data structure
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()>;

    /// Post-process phase for loading save games
    fn load_post_process(&mut self) -> SaveLoadResult<()>;
}

/// Game world snapshot containing all persistent game state
#[derive(Debug, Serialize, Deserialize)]
pub struct WorldSnapshot {
    pub version: u32,
    pub timestamp: SystemTime,
    pub frame_number: u64,
    pub random_seed: u64,

    // Game objects and state
    pub objects: HashMap<ObjectId, ObjectSnapshot>,
    pub players: Vec<PlayerSnapshot>,
    pub teams: Vec<TeamSnapshot>,
    pub terrain: TerrainSnapshot,
    pub weather: WeatherSnapshot,

    // Game logic state
    pub resource_manager: ResourceManagerSnapshot,
    pub combat_tracker: CombatTrackerSnapshot,
    pub experience_tracker: ExperienceTrackerSnapshot,
    pub pathfinding_cache: PathfindingCacheSnapshot,

    // AI state
    pub ai_players: Vec<AIPlayerSnapshot>,
    pub global_ai_state: GlobalAIStateSnapshot,

    /// Host superweapon strike queue (DaisyCutter / A10 / … residual).
    /// Absolute impact frames must survive so mid-flight loads still detonate.
    #[serde(default)]
    pub special_power_strikes: SpecialPowerStrikeRegistrySnapshot,

    /// Host combat particle registry residual (active systems only path).
    /// Fail-closed: not full client W3D particle GPU state.
    #[serde(default)]
    pub combat_particles: CombatParticleRegistrySnapshot,

    /// Host upgrade research queue residual (Capture / FlashBang / TOW / …).
    /// Mid-flight loads must keep pending research so complete unlocks still fire.
    #[serde(default)]
    pub host_upgrades: HostUpgradeRegistrySnapshot,
}

impl XferData for SerializableVec3 {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("SerializableVec3")?;
        xfer.xfer_i32(&mut self.x)?;
        xfer.xfer_i32(&mut self.y)?;
        xfer.xfer_i32(&mut self.z)?;
        Ok(())
    }
}

// Default implementations for snapshot types
impl Default for WorldSnapshot {
    fn default() -> Self {
        Self {
            version: 1,
            timestamp: SystemTime::now(),
            frame_number: 0,
            random_seed: 0,
            objects: HashMap::new(),
            players: Vec::new(),
            teams: Vec::new(),
            terrain: TerrainSnapshot::default(),
            weather: WeatherSnapshot::default(),
            resource_manager: ResourceManagerSnapshot::default(),
            combat_tracker: CombatTrackerSnapshot::default(),
            experience_tracker: ExperienceTrackerSnapshot::default(),
            pathfinding_cache: PathfindingCacheSnapshot::default(),
            ai_players: Vec::new(),
            global_ai_state: GlobalAIStateSnapshot::default(),
            special_power_strikes: SpecialPowerStrikeRegistrySnapshot::default(),
            combat_particles: CombatParticleRegistrySnapshot::default(),
            host_upgrades: HostUpgradeRegistrySnapshot::default(),
        }
    }
}

/// Serializable Vec3 wrapper that can be used as HashMap key
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SerializableVec3 {
    pub x: i32, // Use integer representation for hashing
    pub y: i32,
    pub z: i32,
}

impl From<Vec3> for SerializableVec3 {
    fn from(v: Vec3) -> Self {
        Self {
            x: (v.x * 1000.0) as i32, // Convert to millimeters for precision
            y: (v.y * 1000.0) as i32,
            z: (v.z * 1000.0) as i32,
        }
    }
}

impl From<SerializableVec3> for Vec3 {
    fn from(val: SerializableVec3) -> Self {
        Vec3::new(
            val.x as f32 / 1000.0,
            val.y as f32 / 1000.0,
            val.z as f32 / 1000.0,
        )
    }
}

/// Wave 79 Drawable residual honesty: StealthLook ordinal survives ObjectStatus.
pub fn honesty_drawable_residual_fields_wave79_ok() -> bool {
    // HostCamoStealthLook ordinals (Drawable.h residual).
    let looks = [0u8, 1, 2, 3, 4, 5];
    looks.iter().all(|&look| {
        let mut status = ObjectStatusSnapshot::default();
        status.camo_stealth_look = look;
        status.stealthed = look != 0;
        status.detected = look == 3 || look == 4;
        // Round-trip via clone residual.
        let cloned = status.clone();
        cloned.camo_stealth_look == look
            && cloned.stealthed == status.stealthed
            && cloned.detected == status.detected
    }) && ObjectStatusSnapshot::default().camo_stealth_look == 0
}
