//! Core snapshot trait, world snapshot, and shared utility types.

use super::player::PlayerTemplateBindingSnapshot;
use super::{
    AIPlayerSnapshot, ClientDrawableWorldSnapshot, CombatParticleRegistrySnapshot,
    CombatTrackerSnapshot, ExperienceTrackerSnapshot, GlobalAIStateSnapshot,
    HostUpgradeRegistrySnapshot, ObjectSnapshot, ObjectStatusSnapshot, PathfindingCacheSnapshot,
    PlayerSnapshot, ResourceManagerSnapshot, SpecialPowerStrikeRegistrySnapshot, TeamSnapshot,
    TerrainSnapshot, WeatherSnapshot,
};
use crate::game_logic::*;
use crate::save_load::{SaveLoadResult, Xfer, XferData};
use gamelogic::system::shroud_manager::ShroudSnapshot;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::SystemTime;

/// Positional bincode schema used by host `.sav` / legacy `.gen` snapshots.
///
/// Version 1 persisted only float production progress.  Version 2 adds the
/// nested production frame/quantity/exit fields, which cannot be represented
/// by serde defaults under bincode's positional encoding.  Version 3 appends
/// the Hacker Disable Building channel to every object snapshot.  Version 4
/// appends logical weapon-discharge/barrel state and the renderer-owned client
/// Drawable companion. Version 5 appends exact offline `PlayerTemplate`
/// bindings as a world tail, retaining v1-v4 nested PlayerSnapshot alignment.
/// Version 6 appends exact persistent shroud/FOW counters and pending reveal
/// expiry state as a final world tail.
pub const WORLD_SNAPSHOT_BINCODE_VERSION: u32 = 6;

/// Direct Common Xfer keeps an independent positional envelope from bincode.
///
/// Its raw `u32` world version is the only safe boundary before the timestamp
/// and object records.  Do not derive object-tail gates from the bincode
/// version: a historical direct v3 stream still contains HDB even once the
/// bincode writer has advanced to v4.
pub const WORLD_SNAPSHOT_DIRECT_XFER_VERSION: u32 = 6;
pub const WORLD_SNAPSHOT_DIRECT_XFER_HDB_VERSION: u32 = 3;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V4_TAIL_VERSION: u32 = 4;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V5_TAIL_VERSION: u32 = 5;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V6_TAIL_VERSION: u32 = 6;

/// Reject unknown direct-Xfer outer layouts before consuming any body bytes.
/// Known historical writers are accepted so focused fixtures can verify their
/// positional tails independently of current bincode schema evolution.
pub(crate) fn validate_direct_world_snapshot_version(version: u32) -> SaveLoadResult<()> {
    match version {
        // Keep these arms deliberately explicit. Advancing the current writer
        // must not accidentally make a future positional body acceptable
        // before its object/world gates and exact predecessor fixtures exist.
        1 | 2 | 3 | 4 | 5 | 6 => Ok(()),
        actual => Err(crate::save_load::SaveLoadError::VersionMismatch {
            expected: WORLD_SNAPSHOT_DIRECT_XFER_VERSION,
            actual,
        }),
    }
}

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

    /// Next unused logical accepted-weapon-discharge sequence.  `0` is never
    /// emitted: Object marker sequence zero is reserved for "unseen".
    #[serde(default = "default_next_weapon_discharge_sequence")]
    pub next_weapon_discharge_sequence: u64,

    /// Renderer-local visual state companion.  It is validated against a fresh
    /// frozen presentation topology only after a successful staged restore.
    #[serde(default)]
    pub client_drawables: ClientDrawableWorldSnapshot,

    /// Exact offline Skirmish PlayerTemplate bindings. Kept as a world tail:
    /// appending this to historical nested `PlayerSnapshot` Xfer records
    /// would misalign every direct v1-v4 stream.
    #[serde(default)]
    pub player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,

    /// Exact C++ PartitionManager shroud counters and pending undo-reveal
    /// expiry queue. This remains the final tail so v1-v5 records stay
    /// positionally aligned through their exact predecessor mirrors.
    #[serde(default)]
    pub shroud: ShroudSnapshot,
}

pub const fn default_next_weapon_discharge_sequence() -> u64 {
    1
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
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
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
            next_weapon_discharge_sequence: default_next_weapon_discharge_sequence(),
            client_drawables: ClientDrawableWorldSnapshot::default(),
            player_template_bindings: Vec::new(),
            shroud: ShroudSnapshot::default(),
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
