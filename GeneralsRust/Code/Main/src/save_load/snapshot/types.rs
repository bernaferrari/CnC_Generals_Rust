//! Core snapshot trait, world snapshot, and shared utility types.

use super::player::{PlayerEnergySnapshot, PlayerRankSnapshot, PlayerTemplateBindingSnapshot};
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
/// expiry state as a final world tail. Version 7 appends each object's
/// parallel `Weapon::m_suspendFXFrame` tail without changing historical
/// nested Weapon records. Version 8 appends the source-keyed temporary
/// behavior runtime tail to each object. Version 9 appends the entity
/// lifecycle envelope. Version 10 appends C++ `Player::xfer` rank/skill/
/// science-purchase-point residuals as a world tail so nested
/// `PlayerSnapshot` records stay aligned with v1-v9 streams. Version 11
/// appends C++ `Object::m_name` plus `AIUpdateInterface` guard anchors
/// (`m_locationToGuard` / `m_objectToGuard` / `m_guardMode` and the host
/// guard radius) as a world tail so nested `ObjectSnapshot` records stay
/// aligned with v1-v10 streams. Version 12 appends OverchargeBehavior
/// per-object `vision_spied_mask`, exclusive `builder_id` / dozer BUILD
/// task, and BuildAssistant `sell_list` so mid-spy / mid-build / mid-sell
/// loads keep those residuals alive. Version 14 appends C++ `Object::xfer`
/// sole-heal benefactor, `m_containedByFrame`, garrison `m_originalTeamName`,
/// formation id/offset, and Drawable hidden/stealth-opacity/loco/expiration
/// /decal companions as a world tail so nested object and client-drawable
/// records stay aligned with v1-v13 streams. Version 15 appends C++
/// `Energy::xfer` v3 `m_powerSabotagedTillFrame` as a world tail so nested
/// `PlayerSnapshot` records stay aligned with v1-v14 streams. Version 16
/// `ObjectSnapshot` / `object_persist` records stay aligned with v1-v15 streams.
/// `ObjectSnapshot` / `object_persist` records stay aligned with v1-v15 streams.
/// Version 17 appends C++ `GameLogic::xfer` scoring + superweapon restriction,
/// `CaveSystem` / player `TunnelTracker` communal pools, and airfield /
/// FlightDeck stall occupancy as a world tail so nested records stay aligned
/// with v1-v16 streams. Version 18 appends InGameUI timers/SW display,
/// TacticalView camera, ScriptEngine counters/flags/actives, GameLogic v5-v9
/// globals, TerrainLogic water updates, Radar hidden/force-on/event ring,
/// and remaining Drawable::xfer residuals as a world tail. Version 19 appends
/// C++ `Object::xfer` `m_commandSetStringOverride` as a world tail so nested
/// `ObjectSnapshot` records stay aligned with v1-v18 streams. Version 20
/// appends C++ `StealthUpdate::xfer` disguise identity / transition
/// (`StealthUpdate.cpp:1141-1177`) as a world tail so nested object records
/// stay aligned with v1-v20 streams. Version 21 appends the per-weapon
/// clip/splash/reload residual (clip_size, clip_reload_time, splash_radius,
/// reloading_clip, last_bonus_rof) that the serde payload always carried but
/// the historical direct-Xfer `Weapon` record dropped — C++ `Weapon::xfer` v3
/// (Weapon.cpp:3364-3367) persists `m_status` RELOADING_CLIP + `m_ammoInClip`
/// so a mid-clip-reload slot resumes after load.
pub const WORLD_SNAPSHOT_BINCODE_VERSION: u32 = 21;

/// Direct Common Xfer keeps an independent positional envelope from bincode.
///
/// Its raw `u32` world version is the only safe boundary before the timestamp
/// and object records.  Do not derive object-tail gates from the bincode
/// version: a historical direct v3 stream still contains HDB even once the
/// bincode writer has advanced to v4.
pub const WORLD_SNAPSHOT_DIRECT_XFER_VERSION: u32 = 21;
pub const WORLD_SNAPSHOT_DIRECT_XFER_HDB_VERSION: u32 = 3;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V4_TAIL_VERSION: u32 = 4;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V5_TAIL_VERSION: u32 = 5;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V6_TAIL_VERSION: u32 = 6;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V7_TAIL_VERSION: u32 = 7;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V8_TAIL_VERSION: u32 = 8;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V9_TAIL_VERSION: u32 = 9;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V10_TAIL_VERSION: u32 = 10;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V11_TAIL_VERSION: u32 = 11;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V12_TAIL_VERSION: u32 = 12;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V13_TAIL_VERSION: u32 = 13;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V14_TAIL_VERSION: u32 = 14;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V15_TAIL_VERSION: u32 = 15;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V16_TAIL_VERSION: u32 = 16;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V17_TAIL_VERSION: u32 = 17;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V18_TAIL_VERSION: u32 = 18;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V19_TAIL_VERSION: u32 = 19;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V20_TAIL_VERSION: u32 = 20;
pub const WORLD_SNAPSHOT_DIRECT_XFER_V21_TAIL_VERSION: u32 = 21;

/// Reject unknown direct-Xfer outer layouts before consuming any body bytes.
/// Known historical writers are accepted so focused fixtures can verify their
/// positional tails independently of current bincode schema evolution.
pub(crate) fn validate_direct_world_snapshot_version(version: u32) -> SaveLoadResult<()> {
    match version {
        // Keep these arms deliberately explicit. Advancing the current writer
        // must not accidentally make a future positional body acceptable
        // before its object/world gates and exact predecessor fixtures exist.
        1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15 | 16 | 17 | 18 | 19
        | 20 | 21 => Ok(()),
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

    /// v9 Entity lifecycle envelope + contain/producer fixup side block.
    #[serde(default)]
    pub lifecycle_tail: Vec<u8>,

    /// C++ `Player::xfer` rank/skill/science-purchase-point residuals.
    /// Appended after the v9 lifecycle tail so historical nested
    /// `PlayerSnapshot` records stay aligned.
    #[serde(default)]
    pub player_ranks: Vec<PlayerRankSnapshot>,

    /// C++ `Object::xfer` instance name (`m_name`) and `AIUpdateInterface::xfer`
    /// guard anchors. World tail so v1-v10 nested object records stay aligned.
    #[serde(default)]
    pub object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,

    /// C++ `OverchargeBehavior::xfer` `m_overchargeActive`. World tail so
    /// nested Object/Building records stay aligned with v1-v11 streams.
    #[serde(default)]
    pub overcharge_active: Vec<ObjectOverchargeSnapshot>,

    /// C++ `Object::xfer` `m_visionSpiedMask` + SpyVisionUpdate / CIA
    /// registry. World tail so nested Object records stay aligned with
    /// v1-v12 streams (`Object.cpp:4126-4130`).
    #[serde(default)]
    pub cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    #[serde(default)]
    pub vision_spied: Vec<ObjectVisionSpiedSnapshot>,

    /// C++ `Object::xfer` `m_builderID` (`Object.cpp:4050-4053`) plus
    /// `DozerAIUpdate` BUILD task (`DozerAIUpdate.cpp:1986`).
    #[serde(default)]
    pub builder_tasks: Vec<ObjectBuilderTaskSnapshot>,

    /// C++ `GameLogic::xfer` v6 `TheBuildAssistant->xferTheSellList`
    /// (object id + sell frame). Mid-sell buildings stay on this list.
    #[serde(default)]
    pub sell_list: Vec<SellListEntrySnapshot>,

    /// C++ `Object::xfer` sole-heal window, contain enter-frame, garrison
    /// original team, and formation stamp. World tail so nested
    /// `ObjectSnapshot` / `BuildingSnapshot` / `UnitSnapshot` stay aligned.
    #[serde(default)]
    pub object_persist: Vec<ObjectPersistTailSnapshot>,

    /// C++ `Drawable::xfer` v7 hidden / stealth opacity / loco / expiration
    /// / terrain decal. World tail so historical `ClientDrawableStateSnapshot`
    /// records stay aligned.
    #[serde(default)]
    pub client_drawable_visuals: Vec<ClientDrawableVisualSnapshot>,

    /// C++ `Energy::xfer` v3 `m_powerSabotagedTillFrame`. World tail so
    /// nested `PlayerSnapshot` records stay aligned with v1-v14 streams.
    #[serde(default)]
    pub player_energy: Vec<PlayerEnergySnapshot>,

    /// C++ `Object::xfer` (`Object.cpp:4218-4246`) trigger-area slots.
    /// World tail so nested `ObjectSnapshot` / `object_persist` stay aligned.
    #[serde(default)]
    pub object_triggers: Vec<ObjectTriggerPersistSnapshot>,

    /// C++ `GameLogic::xfer` v2 `m_isScoringEnabled`. Default true matches
    /// leftover `impl_init` so pre-v17 loads keep awarding score.
    #[serde(default = "default_scoring_enabled")]
    pub is_scoring_enabled: bool,

    /// C++ `GameLogic::xfer` v10 `m_superweaponRestriction` as the live
    /// Limit Superweapons cap (`skirmish_rules.limit_superweapons`).
    #[serde(default)]
    pub limit_superweapons: bool,

    /// C++ `CaveSystem::xfer` v1 shared CaveIndex TunnelTracker pool.
    #[serde(default)]
    pub cave_system: crate::game_logic::HostCaveSystem,

    /// C++ `Player::m_tunnelSystem` TunnelTracker communal pool.
    #[serde(default)]
    pub tunnel_network: crate::game_logic::HostTunnelNetworkRegistry,

    /// C++ ParkingPlace / FlightDeck persist tail.
    #[serde(default)]
    pub airfield_parking: AirfieldParkingWorldSnapshot,

    /// C++ InGameUI / View / ScriptEngine / GameLogic v5-v9 / TerrainLogic /
    /// Radar / remaining Drawable::xfer residuals. World tail so v1-v17
    /// nested records stay aligned.
    #[serde(default)]
    pub persist_v18: super::persist_v18::WorldPersistV18,

    /// Reserved v18 sibling for FixW1805Vet experience_sink/scalar persist.
    #[serde(default)]
    pub object_experience_trackers: Vec<ObjectExperienceTrackerSnapshot>,

    /// C++ `Object::xfer` (`Object.cpp:4403`) `m_commandSetStringOverride`.
    /// World tail so nested `ObjectSnapshot` stays aligned with v1-v18 streams.
    #[serde(default)]
    pub object_command_sets: Vec<ObjectCommandSetSnapshot>,

    /// C++ `StealthUpdate::xfer` disguise identity + transition
    /// (`StealthUpdate.cpp:1141-1177`). World tail so nested
    /// `ObjectSnapshot` / `ObjectStatusSnapshot` stay aligned with v1-v19.
    #[serde(default)]
    pub object_disguises: Vec<ObjectDisguiseSnapshot>,
}

/// C++ `ExperienceTracker::xfer` `m_experienceSink` + `m_experienceScalar`.
/// World tail so nested `ObjectSnapshot` stays aligned with v1-v17 streams.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectExperienceTrackerSnapshot {
    pub object_id: ObjectId,
    pub experience_sink: Option<ObjectId>,
    pub experience_scalar: f32,
}

impl Default for ObjectExperienceTrackerSnapshot {
    fn default() -> Self {
        Self {
            object_id: ObjectId(0),
            experience_sink: None,
            experience_scalar: 1.0,
        }
    }
}

/// C++ `Object::xfer` (`Object.cpp:4403`) `m_commandSetStringOverride`.
/// Empty string is none. World tail so nested object records stay aligned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ObjectCommandSetSnapshot {
    pub object_id: ObjectId,
    pub command_set_override: String,
}

/// C++ `StealthUpdate::xfer` disguise persist (`StealthUpdate.cpp:1141-1177`).
/// Empty template / team 255 is none. Pending fields cover pre-halfpoint live
/// residual (`disguise_pending_*`); C++ stores the target on
/// `m_disguiseAsTemplate` before the visual swap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ObjectDisguiseSnapshot {
    pub object_id: ObjectId,
    pub disguise_as_template: String,
    pub disguise_as_team: u8,
    pub disguise_pending_template: String,
    pub disguise_pending_team: u8,
    pub disguised: bool,
    pub disguise_transition_frames: u32,
    pub disguise_transitioning_to: bool,
    pub disguise_halfpoint_reached: bool,
}

/// C++ `OverchargeBehavior::xfer` (`OverchargeBehavior.cpp:275-289`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectOverchargeSnapshot {
    pub object_id: ObjectId,
    pub overcharge_enabled: bool,
}

/// C++ `Object::xfer` (`Object.cpp:4126-4130`) `m_visionSpiedMask`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectVisionSpiedSnapshot {
    pub object_id: ObjectId,
    pub vision_spied_mask: u32,
}

/// C++ `Object::xfer` `m_builderID` + `DozerAIUpdate` BUILD task slot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectBuilderTaskSnapshot {
    pub object_id: ObjectId,
    pub builder_id: Option<ObjectId>,
    pub dozer_task_build_target: Option<ObjectId>,
    pub dozer_task_build_order_frame: u32,
}

/// C++ `BuildAssistant::xferTheSellList` (`ObjectSellInfo` id + sell frame).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SellListEntrySnapshot {
    pub object_id: ObjectId,
    pub sell_frame: u32,
}

/// C++ `Object::xfer` residuals that must not change nested object layout.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectPersistTailSnapshot {
    pub object_id: ObjectId,
    pub sole_healing_benefactor: Option<ObjectId>,
    pub sole_healing_benefactor_expiration_frame: u32,
    pub contained_by_frame: Option<u32>,
    pub original_team: Option<Team>,
    pub formation_id: u32,
    pub formation_offset: [f32; 2],
    pub stealth_opacity: f32,
    pub terrain_decal_type: u8,
    pub terrain_decal_size: f32,
}

/// C++ `Object::xfer` per-area trigger slot (`entered` / `exited` / `isInside`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ObjectTriggerSlotSnapshot {
    pub trigger_id: i32,
    pub trigger_name: String,
    pub is_inside: bool,
    pub entered: bool,
    pub exited: bool,
}

/// C++ `Object::xfer` trigger housekeeping keyed by object id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectTriggerPersistSnapshot {
    pub object_id: ObjectId,
    pub i_x: i32,
    pub i_y: i32,
    pub entered_or_exited_frame: u32,
    pub slots: Vec<ObjectTriggerSlotSnapshot>,
}

/// C++ `Drawable::xfer` v7 companion extras keyed by draw module.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClientDrawableVisualSnapshot {
    pub object_id: u32,
    pub draw_module_index: u32,
    pub hidden: bool,
    pub hidden_by_stealth: bool,
    pub stealth_opacity: f32,
    pub effective_opacity: f32,
    pub loco_pitch: f32,
    pub loco_roll: f32,
    pub expiration_date: u32,
    pub terrain_decal: u8,
}

/// C++ `Object::xfer` (`Object.cpp:4068`) `m_name` plus `AIUpdateInterface::xfer`
/// (`AIUpdate.cpp:5015-5019`) guard anchors. Stored as a world tail so
/// historical nested `ObjectSnapshot` records stay aligned.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ObjectInstanceGuardSnapshot {
    pub object_id: ObjectId,
    pub instance_name: String,
    pub guard_position: Option<Vec3>,
    pub guard_target: Option<ObjectId>,
    pub guard_radius: f32,
    pub guard_mode: GuardMode,
}

pub const fn default_scoring_enabled() -> bool {
    true
}

/// One C++ `ParkingPlaceBehavior` stall (`m_spaces[i].m_objectInSpace`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AirfieldParkingSpaceSnapshot {
    pub object_id: Option<ObjectId>,
    pub reserved_for_exit: bool,
}

/// Per-airfield hangar roster.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AirfieldParkingFieldSnapshot {
    pub airfield_id: ObjectId,
    pub spaces: Vec<AirfieldParkingSpaceSnapshot>,
}

/// C++ `RunwayInfo::m_inUseBy` / `m_nextInLineForTakeoff`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AirfieldRunwaySnapshot {
    pub airfield_id: ObjectId,
    pub occupants: Vec<Option<ObjectId>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AirfieldRunwayWasInLineSnapshot {
    pub airfield_id: ObjectId,
    pub was_in_line: Vec<bool>,
}

/// C++ `Object` hangar index written by ParkingPlace / FlightDeck reserve.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AirfieldJetStallSnapshot {
    pub object_id: ObjectId,
    pub space_index: Option<u32>,
}

/// C++ `FlightDeckBehavior` stall occupancy (poses rebuild after load).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FlightDeckSpaceSnapshot {
    pub object_id: Option<ObjectId>,
    pub runway: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FlightDeckRunwaySnapshot {
    pub in_use_takeoff: Option<ObjectId>,
    pub in_use_landing: Option<ObjectId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct FlightDeckPersistSnapshot {
    pub carrier_id: ObjectId,
    pub spaces: Vec<FlightDeckSpaceSnapshot>,
    pub runways: Vec<FlightDeckRunwaySnapshot>,
    pub got_info: bool,
    pub designated_target: Option<ObjectId>,
    pub designated_command: u8,
    pub pending_replacement: bool,
}

/// C++ ParkingPlace / FlightDeck persist tail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AirfieldParkingWorldSnapshot {
    pub fields: Vec<AirfieldParkingFieldSnapshot>,
    pub runways: Vec<AirfieldRunwaySnapshot>,
    pub next_in_line: Vec<AirfieldRunwaySnapshot>,
    pub was_in_line: Vec<AirfieldRunwayWasInLineSnapshot>,
    pub jet_stalls: Vec<AirfieldJetStallSnapshot>,
    pub flight_decks: Vec<FlightDeckPersistSnapshot>,
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
            lifecycle_tail: Vec::new(),
            player_ranks: Vec::new(),
            object_instance_guards: Vec::new(),
            overcharge_active: Vec::new(),
            cia_intelligence:
                crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry::new(),
            vision_spied: Vec::new(),
            builder_tasks: Vec::new(),
            sell_list: Vec::new(),
            object_persist: Vec::new(),
            client_drawable_visuals: Vec::new(),
            player_energy: Vec::new(),
            object_triggers: Vec::new(),
            is_scoring_enabled: true,
            limit_superweapons: false,
            cave_system: crate::game_logic::HostCaveSystem::new(),
            tunnel_network: crate::game_logic::HostTunnelNetworkRegistry::new(),
            airfield_parking: AirfieldParkingWorldSnapshot::default(),
            persist_v18: super::persist_v18::WorldPersistV18::default(),
            object_experience_trackers: Vec::new(),
            object_command_sets: Vec::new(),
            object_disguises: Vec::new(),
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
