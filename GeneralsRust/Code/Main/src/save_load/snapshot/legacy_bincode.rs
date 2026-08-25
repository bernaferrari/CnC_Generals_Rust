//! Exact bincode layout used before production frame/exit state was persisted.
//!
//! `bincode` encodes structs positionally.  In particular, serde's
//! `#[serde(default)]` does not make an old three-field production entry safe
//! to read as the newer seven-field record: the first appended field consumes
//! bytes belonging to the rest of the enclosing snapshot.  Keep this private
//! mirror of the pre-production-frame layout so old `.sav` / `.gen` payloads
//! can be decoded, then explicitly migrate them into the current snapshot.

use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult};
use bincode::Options;
use gamelogic::system::shroud_manager::ShroudSnapshot;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::collections::HashMap;
use std::time::SystemTime;

/// `WorldSnapshot::version` was the first positional bincode field before and
/// after this migration.  Read it with the same fixed-integer bincode options
/// used by `bincode::serialize`, rather than manually interpreting bytes.
///
/// Every known schema is decoded only through its exact historical record:
/// v1 has float-only production, v2 predates the HDB channel, v3 has HDB but
/// predates the v4 barrel/discharge/client tails, v4 predates the v5
/// PlayerTemplate tail, v5 predates the v6 shroud tail, v6 predates the v7
/// normal-Weapon suspend-FX tail, and v7 predates the v8 temporary-Weapon
/// behavior tail. Version 9 predates the v10 Player rank/skill/science tail.
/// Version 10 predates the v11 Object instance-name/guard tail.
/// Version 11 predates the v12 OverchargeBehavior m_overchargeActive tail.
/// Version 12 predates the v13 CIA/builder/sell world tail.
/// Version 13 predates the v14 object persist / drawable visual tail.
/// Version 14 predates the v15 Energy sabotage-frame tail.
/// Version 15 predates the v16 Object trigger-area tail.
/// Version 16 predates the v17 GameLogic persist tail (scoring, restriction,
/// CaveSystem, TunnelTracker, airfield stalls). Version 17 predates the v18
/// persist_v18 / experience-tracker tail. Version 18 predates the v19
/// Object command-set override tail. Version 19 predates the v20
/// StealthUpdate disguise identity/transition tail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BincodeWorldSnapshotDecodePath {
    Current,
    LegacyPreV20V19,
    LegacyPreV19V18,
    LegacyPreV18V17,
    LegacyPreV17V16,
    LegacyPreV16V15,
    LegacyPreV15V14,
    LegacyPreV14V13,
    LegacyPreV13V12,
    LegacyPreV12V11,
    LegacyPreV11V10,
    LegacyPreV10V9,
    LegacyPreV9V8,
    LegacyPreV8V7,
    LegacyPreV7V6,
    LegacyPreV6V5,
    LegacyPreV5V4,
    LegacyPreV4V3,
    LegacyPreHackerDisableV2,
    LegacyProductionV1,
}

pub(crate) fn decode_bincode_world_snapshot(
    payload: &[u8],
) -> SaveLoadResult<(WorldSnapshot, BincodeWorldSnapshotDecodePath)> {
    let version = bincode_prefix::<u32>(payload)
        .map_err(|error| SaveLoadError::Serialization(error.to_string()))?;

    match version {
        WORLD_SNAPSHOT_BINCODE_VERSION => bincode_exact::<WorldSnapshot>(payload)
            .map(|snapshot| (snapshot, BincodeWorldSnapshotDecodePath::Current))
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        19 => bincode_exact::<PreV20WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV20V19,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        18 => bincode_exact::<PreV19WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV19V18,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        17 => bincode_exact::<PreV18WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV18V17,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        16 => bincode_exact::<PreV17WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV17V16,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        15 => bincode_exact::<PreV16WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV16V15,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        14 => bincode_exact::<PreV15WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV15V14,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        13 => bincode_exact::<PreV14WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV14V13,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        12 => bincode_exact::<PreV13WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV13V12,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        11 => bincode_exact::<PreV12WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV12V11,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        10 => bincode_exact::<PreV11WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV11V10,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        9 => bincode_exact::<PreV10WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV10V9,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        8 => bincode_exact::<PreV9WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV9V8,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        7 => bincode_exact::<PreV8WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV8V7,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        6 => bincode_exact::<PreV7WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV7V6,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        5 => bincode_exact::<PreV6WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV6V5,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        4 => bincode_exact::<PreV5WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV5V4,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        3 => bincode_exact::<PreV4WorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreV4V3,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        2 => bincode_exact::<PreHackerDisableWorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyPreHackerDisableV2,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        1 => bincode_exact::<LegacyWorldSnapshot>(payload)
            .map(|snapshot| {
                (
                    snapshot.into(),
                    BincodeWorldSnapshotDecodePath::LegacyProductionV1,
                )
            })
            .map_err(|error| SaveLoadError::Serialization(error.to_string())),
        actual => Err(SaveLoadError::VersionMismatch {
            expected: WORLD_SNAPSHOT_BINCODE_VERSION,
            actual,
        }),
    }
}

/// Complete v4 world record before exact offline PlayerTemplate bindings and
/// the v5 collector runtime tail were appended. It must stay a separate
/// positional mirror: serde defaults cannot safely decode an omitted bincode
/// struct field.
#[derive(Debug, Deserialize, Serialize)]
struct PreV5WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, PreV5ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
}

/// Complete v5 world record before the v6 persistent shroud tail was
/// appended. Keep this exact positional mirror so an old v5 payload cannot
/// consume the first shroud byte as part of a current record.
#[derive(Debug, Deserialize, Serialize)]
struct PreV6WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, PreV5ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
}

/// Complete v6 world record before the v7 per-object Weapon suspend-FX tail.
/// This is intentionally a separate mirror: the v6 object record already
/// includes collector runtime and the v6 world tail includes persistent
/// shroud state.
#[derive(Debug, Deserialize, Serialize)]
struct PreV7WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, PreV7ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
}

/// Exact v6 ObjectSnapshot positional record.  The v7 parallel suspend-FX
/// vector is the only omitted field.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PreV7ObjectSnapshot {
    id: ObjectId,
    template_name: String,
    team: Team,
    player_id: u32,
    geometry: GeometryInfo,
    status: ObjectStatusSnapshot,
    health: Health,
    movement: Movement,
    experience: Experience,
    weapons: Vec<Weapon>,
    contained_objects: Vec<ObjectId>,
    container_object: Option<ObjectId>,
    modules: HashMap<String, ModuleSnapshot>,
    object_type: ObjectTypeSnapshot,
    hacker_disable_channel: Option<HackerDisableChannelState>,
    weapon_barrel_states: [WeaponBarrelStateSnapshot; 3],
    last_weapon_discharge_sequence: u64,
    last_weapon_discharge_slot: u8,
    last_weapon_discharge_barrel: u8,
    last_weapon_discharge_frame: u32,
    collector_runtime: Option<CollectorRuntimeSnapshot>,
}

#[derive(Debug, Deserialize, Serialize)]
struct PreV9WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
}

/// Complete v9 world record before the v10 Player rank/skill/science tail.
/// Nested `PlayerSnapshot` remains the historical positional layout.
#[derive(Debug, Deserialize, Serialize)]
struct PreV10WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
}

/// Complete v10 world record before the v11 Object instance-name / guard tail.
/// Nested `ObjectSnapshot` remains the historical positional layout.
#[derive(Debug, Deserialize, Serialize)]
struct PreV11WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
}

/// Complete v11 world record before the v12 OverchargeBehavior active tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV12WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
}

/// Complete v12 world record before the v13 CIA / builder / sell tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV13WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
    overcharge_active: Vec<ObjectOverchargeSnapshot>,
}

/// Complete v13 world record before the v14 object persist / drawable visual tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV14WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
    overcharge_active: Vec<ObjectOverchargeSnapshot>,
    cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    vision_spied: Vec<ObjectVisionSpiedSnapshot>,
    builder_tasks: Vec<ObjectBuilderTaskSnapshot>,
    sell_list: Vec<SellListEntrySnapshot>,
}

/// Complete v14 world record before the v15 Energy sabotage-frame tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV15WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
    overcharge_active: Vec<ObjectOverchargeSnapshot>,
    cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    vision_spied: Vec<ObjectVisionSpiedSnapshot>,
    builder_tasks: Vec<ObjectBuilderTaskSnapshot>,
    sell_list: Vec<SellListEntrySnapshot>,
    object_persist: Vec<ObjectPersistTailSnapshot>,
    client_drawable_visuals: Vec<ClientDrawableVisualSnapshot>,
}

/// Complete v15 world record before the v16 Object trigger-area tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV16WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
    overcharge_active: Vec<ObjectOverchargeSnapshot>,
    cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    vision_spied: Vec<ObjectVisionSpiedSnapshot>,
    builder_tasks: Vec<ObjectBuilderTaskSnapshot>,
    sell_list: Vec<SellListEntrySnapshot>,
    object_persist: Vec<ObjectPersistTailSnapshot>,
    client_drawable_visuals: Vec<ClientDrawableVisualSnapshot>,
    player_energy: Vec<PlayerEnergySnapshot>,
}

/// Complete v16 world record before the v17 GameLogic persist tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV17WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
    overcharge_active: Vec<ObjectOverchargeSnapshot>,
    cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    vision_spied: Vec<ObjectVisionSpiedSnapshot>,
    builder_tasks: Vec<ObjectBuilderTaskSnapshot>,
    sell_list: Vec<SellListEntrySnapshot>,
    object_persist: Vec<ObjectPersistTailSnapshot>,
    client_drawable_visuals: Vec<ClientDrawableVisualSnapshot>,
    player_energy: Vec<PlayerEnergySnapshot>,
    object_triggers: Vec<ObjectTriggerPersistSnapshot>,
}

/// Complete v17 world record before the v18 persist tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV18WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
    overcharge_active: Vec<ObjectOverchargeSnapshot>,
    cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    vision_spied: Vec<ObjectVisionSpiedSnapshot>,
    builder_tasks: Vec<ObjectBuilderTaskSnapshot>,
    sell_list: Vec<SellListEntrySnapshot>,
    object_persist: Vec<ObjectPersistTailSnapshot>,
    client_drawable_visuals: Vec<ClientDrawableVisualSnapshot>,
    player_energy: Vec<PlayerEnergySnapshot>,
    object_triggers: Vec<ObjectTriggerPersistSnapshot>,
    is_scoring_enabled: bool,
    limit_superweapons: bool,
    cave_system: crate::game_logic::HostCaveSystem,
    tunnel_network: crate::game_logic::HostTunnelNetworkRegistry,
    airfield_parking: AirfieldParkingWorldSnapshot,
}

impl From<PreV18WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV18WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
            object_persist: snapshot.object_persist,
            client_drawable_visuals: snapshot.client_drawable_visuals,
            player_energy: snapshot.player_energy,
            object_triggers: snapshot.object_triggers,
            is_scoring_enabled: snapshot.is_scoring_enabled,
            limit_superweapons: snapshot.limit_superweapons,
            cave_system: snapshot.cave_system,
            tunnel_network: snapshot.tunnel_network,
            airfield_parking: snapshot.airfield_parking,
            persist_v18: super::persist_v18::WorldPersistV18::default(),
            object_experience_trackers: Vec::new(),
            object_command_sets: Vec::new(),
            object_disguises: Vec::new(),
        }
    }
}

/// Complete v18 world record before the v19 command-set override tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV19WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
    overcharge_active: Vec<ObjectOverchargeSnapshot>,
    cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    vision_spied: Vec<ObjectVisionSpiedSnapshot>,
    builder_tasks: Vec<ObjectBuilderTaskSnapshot>,
    sell_list: Vec<SellListEntrySnapshot>,
    object_persist: Vec<ObjectPersistTailSnapshot>,
    client_drawable_visuals: Vec<ClientDrawableVisualSnapshot>,
    player_energy: Vec<PlayerEnergySnapshot>,
    object_triggers: Vec<ObjectTriggerPersistSnapshot>,
    is_scoring_enabled: bool,
    limit_superweapons: bool,
    cave_system: crate::game_logic::HostCaveSystem,
    tunnel_network: crate::game_logic::HostTunnelNetworkRegistry,
    airfield_parking: AirfieldParkingWorldSnapshot,
    persist_v18: super::persist_v18::WorldPersistV18,
    object_experience_trackers: Vec<ObjectExperienceTrackerSnapshot>,
}

impl From<PreV19WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV19WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
            object_persist: snapshot.object_persist,
            client_drawable_visuals: snapshot.client_drawable_visuals,
            player_energy: snapshot.player_energy,
            object_triggers: snapshot.object_triggers,
            is_scoring_enabled: snapshot.is_scoring_enabled,
            limit_superweapons: snapshot.limit_superweapons,
            cave_system: snapshot.cave_system,
            tunnel_network: snapshot.tunnel_network,
            airfield_parking: snapshot.airfield_parking,
            persist_v18: snapshot.persist_v18,
            object_experience_trackers: snapshot.object_experience_trackers,
            object_command_sets: Vec::new(),
            object_disguises: Vec::new(),
        }
    }
}

/// Complete v19 world record before the v20 StealthUpdate disguise tail.
#[derive(Debug, Deserialize, Serialize)]
struct PreV20WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
    lifecycle_tail: Vec<u8>,
    player_ranks: Vec<PlayerRankSnapshot>,
    object_instance_guards: Vec<ObjectInstanceGuardSnapshot>,
    overcharge_active: Vec<ObjectOverchargeSnapshot>,
    cia_intelligence: crate::game_logic::host_cia_intelligence::HostCiaIntelligenceRegistry,
    vision_spied: Vec<ObjectVisionSpiedSnapshot>,
    builder_tasks: Vec<ObjectBuilderTaskSnapshot>,
    sell_list: Vec<SellListEntrySnapshot>,
    object_persist: Vec<ObjectPersistTailSnapshot>,
    client_drawable_visuals: Vec<ClientDrawableVisualSnapshot>,
    player_energy: Vec<PlayerEnergySnapshot>,
    object_triggers: Vec<ObjectTriggerPersistSnapshot>,
    is_scoring_enabled: bool,
    limit_superweapons: bool,
    cave_system: crate::game_logic::HostCaveSystem,
    tunnel_network: crate::game_logic::HostTunnelNetworkRegistry,
    airfield_parking: AirfieldParkingWorldSnapshot,
    persist_v18: super::persist_v18::WorldPersistV18,
    object_experience_trackers: Vec<ObjectExperienceTrackerSnapshot>,
    object_command_sets: Vec<ObjectCommandSetSnapshot>,
}

impl From<PreV20WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV20WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
            object_persist: snapshot.object_persist,
            client_drawable_visuals: snapshot.client_drawable_visuals,
            player_energy: snapshot.player_energy,
            object_triggers: snapshot.object_triggers,
            is_scoring_enabled: snapshot.is_scoring_enabled,
            limit_superweapons: snapshot.limit_superweapons,
            cave_system: snapshot.cave_system,
            tunnel_network: snapshot.tunnel_network,
            airfield_parking: snapshot.airfield_parking,
            persist_v18: snapshot.persist_v18,
            object_experience_trackers: snapshot.object_experience_trackers,
            object_command_sets: snapshot.object_command_sets,
            object_disguises: Vec::new(),
        }
    }
}

/// Complete v7 world record before the v8 source-keyed temporary-Weapon
/// behavior tail was appended to each object.  This mirror intentionally
/// includes the v7 normal-Weapon suspend-FX vector.
#[derive(Debug, Deserialize, Serialize)]
struct PreV8WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, PreV8ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
    next_weapon_discharge_sequence: u64,
    client_drawables: ClientDrawableWorldSnapshot,
    player_template_bindings: Vec<PlayerTemplateBindingSnapshot>,
    shroud: ShroudSnapshot,
}

/// Exact v7 ObjectSnapshot positional record. The v8 temporary behavior tail
/// is the only omitted field.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PreV8ObjectSnapshot {
    id: ObjectId,
    template_name: String,
    team: Team,
    player_id: u32,
    geometry: GeometryInfo,
    status: ObjectStatusSnapshot,
    health: Health,
    movement: Movement,
    experience: Experience,
    weapons: Vec<Weapon>,
    contained_objects: Vec<ObjectId>,
    container_object: Option<ObjectId>,
    modules: HashMap<String, ModuleSnapshot>,
    object_type: ObjectTypeSnapshot,
    hacker_disable_channel: Option<HackerDisableChannelState>,
    weapon_barrel_states: [WeaponBarrelStateSnapshot; 3],
    last_weapon_discharge_sequence: u64,
    last_weapon_discharge_slot: u8,
    last_weapon_discharge_barrel: u8,
    last_weapon_discharge_frame: u32,
    collector_runtime: Option<CollectorRuntimeSnapshot>,
    weapon_suspend_fx_frames: Vec<u32>,
}

/// Exact v4 `ObjectSnapshot` positional record. Version 4 already carried the
/// hacker-disable, weapon-barrel, and accepted-discharge tails, but predates
/// the v5 collector runtime tail. Reusing the live `ObjectSnapshot` here would
/// make the decoder consume the first byte of the next object/world field as
/// `collector_runtime` and reject or misalign real v4 saves.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PreV5ObjectSnapshot {
    id: ObjectId,
    template_name: String,
    team: Team,
    player_id: u32,
    geometry: GeometryInfo,
    status: ObjectStatusSnapshot,
    health: Health,
    movement: Movement,
    experience: Experience,
    weapons: Vec<Weapon>,
    contained_objects: Vec<ObjectId>,
    container_object: Option<ObjectId>,
    modules: HashMap<String, ModuleSnapshot>,
    object_type: ObjectTypeSnapshot,
    hacker_disable_channel: Option<HackerDisableChannelState>,
    weapon_barrel_states: [WeaponBarrelStateSnapshot; 3],
    last_weapon_discharge_sequence: u64,
    last_weapon_discharge_slot: u8,
    last_weapon_discharge_barrel: u8,
    last_weapon_discharge_frame: u32,
}

fn bincode_prefix<T: DeserializeOwned>(payload: &[u8]) -> bincode::Result<T> {
    // These options are exactly the ones behind bincode 1.3's public
    // `deserialize` helper: fixed-width, little-endian primitives and a
    // trailing-byte-tolerant prefix read.
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .allow_trailing_bytes()
        .deserialize(payload)
}

fn bincode_exact<T: DeserializeOwned>(payload: &[u8]) -> bincode::Result<T> {
    // Do not let a positional record with an incompatible inner field layout
    // succeed by silently ignoring the remainder of the payload.
    bincode::DefaultOptions::new()
        .with_fixint_encoding()
        .reject_trailing_bytes()
        .deserialize(payload)
}

/// Full predecessor shape of `WorldSnapshot`.  At production schema v1 the
/// outer record already contained the three residual registries below; only
/// `objects -> modules -> Production` used the old nested layout.
#[derive(Debug, Deserialize, Serialize)]
struct LegacyWorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, LegacyObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
}

/// Full predecessor shape of `ObjectSnapshot`; every field except `modules`
/// is deliberately the live type because it was byte-identical at v1.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct LegacyObjectSnapshot {
    id: ObjectId,
    template_name: String,
    team: Team,
    player_id: u32,
    geometry: GeometryInfo,
    status: ObjectStatusSnapshot,
    health: Health,
    movement: Movement,
    experience: Experience,
    weapons: Vec<Weapon>,
    contained_objects: Vec<ObjectId>,
    container_object: Option<ObjectId>,
    modules: HashMap<String, LegacyModuleSnapshot>,
    object_type: ObjectTypeSnapshot,
}

/// Preserve the historical enum discriminants.  Only the second (Production)
/// variant changed its positional payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
enum LegacyModuleSnapshot {
    AIUpdate(AIUpdateModuleSnapshot),
    Production(LegacyProductionModuleSnapshot),
    Weapon(WeaponModuleSnapshot),
    Body(BodyModuleSnapshot),
    Locomotor(LocomotorModuleSnapshot),
    Physics(PhysicsModuleSnapshot),
    Contain(ContainModuleSnapshot),
    Upgrade(UpgradeModuleSnapshot),
}

/// Production module as serialized before QueueProductionExitUpdate state was
/// added to the bincode snapshot.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct LegacyProductionModuleSnapshot {
    production_queue: Vec<LegacyProductionQueueEntry>,
    is_producing: bool,
    production_progress: f32,
    rally_point: Option<glam::Vec3>,
}

/// Production queue entry as serialized before the authoritative frame count,
/// batch state, and upgrade discriminator were persisted.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct LegacyProductionQueueEntry {
    template_name: String,
    progress: f32,
    cost: u32,
}

/// Schema v2 was the first tagged production-frame layout.  Its object record
/// predates the appended Hacker Disable Building channel, so it needs a second
/// exact mirror once current writes advance to v3.
///
/// Keep it separate from [`LegacyWorldSnapshot`]: v1 changes the nested
/// Production variant as well, while v2 uses the current Production variant
/// byte-for-byte and differs only at the trailing object field added by HDB.
#[derive(Debug, Deserialize, Serialize)]
struct PreHackerDisableWorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, PreHackerDisableObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
}

/// Exact `ObjectSnapshot` v2 positional record.  Do not replace this with the
/// live type after HDB adds its trailing channel: bincode would otherwise read
/// the following world fields as that new option discriminator/payload.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PreHackerDisableObjectSnapshot {
    id: ObjectId,
    template_name: String,
    team: Team,
    player_id: u32,
    geometry: GeometryInfo,
    status: ObjectStatusSnapshot,
    health: Health,
    movement: Movement,
    experience: Experience,
    weapons: Vec<Weapon>,
    contained_objects: Vec<ObjectId>,
    container_object: Option<ObjectId>,
    modules: HashMap<String, ModuleSnapshot>,
    object_type: ObjectTypeSnapshot,
}

/// Exact v3 world record.  Version 3 already carried the HDB Object tail,
/// but its positional outer record ended at `host_upgrades` and its objects
/// predated the v4 logical barrel/discharge tail.
///
/// This must remain a complete mirror instead of using serde defaults on the
/// live v4 types: bincode would treat the first v4 tail byte as part of the
/// next enclosing field and silently shift the rest of the record.
#[derive(Debug, Deserialize, Serialize)]
struct PreV4WorldSnapshot {
    version: u32,
    timestamp: SystemTime,
    frame_number: u64,
    random_seed: u64,
    objects: HashMap<ObjectId, PreV4ObjectSnapshot>,
    players: Vec<PlayerSnapshot>,
    teams: Vec<TeamSnapshot>,
    terrain: TerrainSnapshot,
    weather: WeatherSnapshot,
    resource_manager: ResourceManagerSnapshot,
    combat_tracker: CombatTrackerSnapshot,
    experience_tracker: ExperienceTrackerSnapshot,
    pathfinding_cache: PathfindingCacheSnapshot,
    ai_players: Vec<AIPlayerSnapshot>,
    global_ai_state: GlobalAIStateSnapshot,
    special_power_strikes: SpecialPowerStrikeRegistrySnapshot,
    combat_particles: CombatParticleRegistrySnapshot,
    host_upgrades: HostUpgradeRegistrySnapshot,
}

/// Exact v3 object record: the HDB channel was the final positional field
/// before v4 appended weapon barrel and accepted-discharge state.
#[derive(Debug, Clone, Deserialize, Serialize)]
struct PreV4ObjectSnapshot {
    id: ObjectId,
    template_name: String,
    team: Team,
    player_id: u32,
    geometry: GeometryInfo,
    status: ObjectStatusSnapshot,
    health: Health,
    movement: Movement,
    experience: Experience,
    weapons: Vec<Weapon>,
    contained_objects: Vec<ObjectId>,
    container_object: Option<ObjectId>,
    modules: HashMap<String, ModuleSnapshot>,
    object_type: ObjectTypeSnapshot,
    hacker_disable_channel: Option<HackerDisableChannelState>,
}

impl From<LegacyWorldSnapshot> for WorldSnapshot {
    fn from(snapshot: LegacyWorldSnapshot) -> Self {
        Self {
            // The returned in-memory record is now in the current shape.  A
            // later save is emitted with the tagged current bincode schema.
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
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

impl From<LegacyObjectSnapshot> for ObjectSnapshot {
    fn from(snapshot: LegacyObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot
                .modules
                .into_iter()
                .map(|(name, module)| (name, module.into()))
                .collect(),
            object_type: snapshot.object_type,
            hacker_disable_channel: None,
            weapon_barrel_states: default_weapon_barrel_state_snapshots(),
            last_weapon_discharge_sequence: 0,
            last_weapon_discharge_slot: 0,
            last_weapon_discharge_barrel: 0,
            last_weapon_discharge_frame: 0,
            collector_runtime: None,
            weapon_suspend_fx_frames: Vec::new(),
            temporary_weapon_runtime: None,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_frenzy_until_frame: 0,
        }
    }
}

impl From<LegacyModuleSnapshot> for ModuleSnapshot {
    fn from(snapshot: LegacyModuleSnapshot) -> Self {
        match snapshot {
            LegacyModuleSnapshot::AIUpdate(data) => Self::AIUpdate(data),
            LegacyModuleSnapshot::Production(data) => Self::Production(data.into()),
            LegacyModuleSnapshot::Weapon(data) => Self::Weapon(data),
            LegacyModuleSnapshot::Body(data) => Self::Body(data),
            LegacyModuleSnapshot::Locomotor(data) => Self::Locomotor(data),
            LegacyModuleSnapshot::Physics(data) => Self::Physics(data),
            LegacyModuleSnapshot::Contain(data) => Self::Contain(data),
            LegacyModuleSnapshot::Upgrade(data) => Self::Upgrade(data),
        }
    }
}

impl From<LegacyProductionModuleSnapshot> for ProductionModuleSnapshot {
    fn from(snapshot: LegacyProductionModuleSnapshot) -> Self {
        Self {
            production_queue: snapshot
                .production_queue
                .into_iter()
                .map(ProductionQueueEntry::from)
                .collect(),
            is_producing: snapshot.is_producing,
            production_progress: snapshot.production_progress,
            rally_point: snapshot.rally_point,
            exit_delay_remaining: 0.0,
            exit_delay_remaining_frames: 0,
            exit_burst_remaining: 0,
            queue_exit_state_initialized: false,
        }
    }
}

impl From<LegacyProductionQueueEntry> for ProductionQueueEntry {
    fn from(entry: LegacyProductionQueueEntry) -> Self {
        Self {
            template_name: entry.template_name,
            progress: entry.progress,
            cost: entry.cost,
            // `ProductionItem` rebuilds this once from float progress on its
            // first logic update, using the live template and power factor.
            construction_frames: 0,
            quantity_total: 1,
            quantity_produced: 0,
            is_upgrade: false,
        }
    }
}

impl From<PreHackerDisableWorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreHackerDisableWorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
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

impl From<PreHackerDisableObjectSnapshot> for ObjectSnapshot {
    fn from(snapshot: PreHackerDisableObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: None,
            weapon_barrel_states: default_weapon_barrel_state_snapshots(),
            last_weapon_discharge_sequence: 0,
            last_weapon_discharge_slot: 0,
            last_weapon_discharge_barrel: 0,
            last_weapon_discharge_frame: 0,
            collector_runtime: None,
            weapon_suspend_fx_frames: Vec::new(),
            temporary_weapon_runtime: None,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_frenzy_until_frame: 0,
        }
    }
}

impl From<PreV4WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV4WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
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

impl From<PreV6WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV6WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
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

impl From<PreV8WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV8WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
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

impl From<PreV9WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV9WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
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

impl From<PreV10WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV10WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
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

impl From<PreV11WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV11WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
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

impl From<PreV12WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV12WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
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

impl From<PreV13WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV13WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
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

impl From<PreV14WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV14WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
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

impl From<PreV15WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV15WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
            object_persist: snapshot.object_persist,
            client_drawable_visuals: snapshot.client_drawable_visuals,
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

impl From<PreV16WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV16WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
            object_persist: snapshot.object_persist,
            client_drawable_visuals: snapshot.client_drawable_visuals,
            player_energy: snapshot.player_energy,
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

impl From<PreV17WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV17WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
            object_persist: snapshot.object_persist,
            client_drawable_visuals: snapshot.client_drawable_visuals,
            player_energy: snapshot.player_energy,
            object_triggers: snapshot.object_triggers,
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

impl From<PreV8ObjectSnapshot> for ObjectSnapshot {
    fn from(snapshot: PreV8ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: snapshot.hacker_disable_channel,
            weapon_barrel_states: snapshot.weapon_barrel_states,
            last_weapon_discharge_sequence: snapshot.last_weapon_discharge_sequence,
            last_weapon_discharge_slot: snapshot.last_weapon_discharge_slot,
            last_weapon_discharge_barrel: snapshot.last_weapon_discharge_barrel,
            last_weapon_discharge_frame: snapshot.last_weapon_discharge_frame,
            collector_runtime: snapshot.collector_runtime,
            weapon_suspend_fx_frames: snapshot.weapon_suspend_fx_frames,
            temporary_weapon_runtime: None,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_frenzy_until_frame: 0,
        }
    }
}

impl From<PreV7WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV7WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
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

impl From<PreV7ObjectSnapshot> for ObjectSnapshot {
    fn from(snapshot: PreV7ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: snapshot.hacker_disable_channel,
            weapon_barrel_states: snapshot.weapon_barrel_states,
            last_weapon_discharge_sequence: snapshot.last_weapon_discharge_sequence,
            last_weapon_discharge_slot: snapshot.last_weapon_discharge_slot,
            last_weapon_discharge_barrel: snapshot.last_weapon_discharge_barrel,
            last_weapon_discharge_frame: snapshot.last_weapon_discharge_frame,
            collector_runtime: snapshot.collector_runtime,
            weapon_suspend_fx_frames: Vec::new(),
            temporary_weapon_runtime: None,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_frenzy_until_frame: 0,
        }
    }
}

impl From<PreV5WorldSnapshot> for WorldSnapshot {
    fn from(snapshot: PreV5WorldSnapshot) -> Self {
        Self {
            version: WORLD_SNAPSHOT_BINCODE_VERSION,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
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

impl From<PreV5ObjectSnapshot> for ObjectSnapshot {
    fn from(snapshot: PreV5ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: snapshot.hacker_disable_channel,
            weapon_barrel_states: snapshot.weapon_barrel_states,
            last_weapon_discharge_sequence: snapshot.last_weapon_discharge_sequence,
            last_weapon_discharge_slot: snapshot.last_weapon_discharge_slot,
            last_weapon_discharge_barrel: snapshot.last_weapon_discharge_barrel,
            last_weapon_discharge_frame: snapshot.last_weapon_discharge_frame,
            collector_runtime: None,
            weapon_suspend_fx_frames: Vec::new(),
            temporary_weapon_runtime: None,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_frenzy_until_frame: 0,
        }
    }
}

impl From<PreV4ObjectSnapshot> for ObjectSnapshot {
    fn from(snapshot: PreV4ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: snapshot.hacker_disable_channel,
            weapon_barrel_states: default_weapon_barrel_state_snapshots(),
            last_weapon_discharge_sequence: 0,
            last_weapon_discharge_slot: 0,
            last_weapon_discharge_barrel: 0,
            last_weapon_discharge_frame: 0,
            collector_runtime: None,
            weapon_suspend_fx_frames: Vec::new(),
            temporary_weapon_runtime: None,
            weapon_bonus_frenzy: false,
            weapon_bonus_frenzy_level: 0,
            weapon_bonus_frenzy_until_frame: 0,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_legacy_production_v1_fixture(
    snapshot: WorldSnapshot,
) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&LegacyWorldSnapshot::from(snapshot))
}

#[cfg(test)]
impl From<WorldSnapshot> for LegacyWorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 1,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
        }
    }
}

#[cfg(test)]
impl From<ObjectSnapshot> for LegacyObjectSnapshot {
    fn from(snapshot: ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot
                .modules
                .into_iter()
                .map(|(name, module)| (name, module.into()))
                .collect(),
            object_type: snapshot.object_type,
        }
    }
}

#[cfg(test)]
impl From<ModuleSnapshot> for LegacyModuleSnapshot {
    fn from(snapshot: ModuleSnapshot) -> Self {
        match snapshot {
            ModuleSnapshot::AIUpdate(data) => Self::AIUpdate(data),
            ModuleSnapshot::Production(data) => Self::Production(data.into()),
            ModuleSnapshot::Weapon(data) => Self::Weapon(data),
            ModuleSnapshot::Body(data) => Self::Body(data),
            ModuleSnapshot::Locomotor(data) => Self::Locomotor(data),
            ModuleSnapshot::Physics(data) => Self::Physics(data),
            ModuleSnapshot::Contain(data) => Self::Contain(data),
            ModuleSnapshot::Upgrade(data) => Self::Upgrade(data),
        }
    }
}

#[cfg(test)]
impl From<ProductionModuleSnapshot> for LegacyProductionModuleSnapshot {
    fn from(snapshot: ProductionModuleSnapshot) -> Self {
        Self {
            production_queue: snapshot
                .production_queue
                .into_iter()
                .map(LegacyProductionQueueEntry::from)
                .collect(),
            is_producing: snapshot.is_producing,
            production_progress: snapshot.production_progress,
            rally_point: snapshot.rally_point,
        }
    }
}

#[cfg(test)]
impl From<ProductionQueueEntry> for LegacyProductionQueueEntry {
    fn from(entry: ProductionQueueEntry) -> Self {
        Self {
            template_name: entry.template_name,
            progress: entry.progress,
            cost: entry.cost,
        }
    }
}

#[cfg(test)]
impl From<WorldSnapshot> for PreHackerDisableWorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 2,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
        }
    }
}

#[cfg(test)]
impl From<ObjectSnapshot> for PreHackerDisableObjectSnapshot {
    fn from(snapshot: ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
        }
    }
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV4WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 3,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
        }
    }
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV5WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 4,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
        }
    }
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV6WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 5,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
        }
    }
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV8WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 7,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
        }
    }
}

#[cfg(test)]
impl From<ObjectSnapshot> for PreV8ObjectSnapshot {
    fn from(snapshot: ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: snapshot.hacker_disable_channel,
            weapon_barrel_states: snapshot.weapon_barrel_states,
            last_weapon_discharge_sequence: snapshot.last_weapon_discharge_sequence,
            last_weapon_discharge_slot: snapshot.last_weapon_discharge_slot,
            last_weapon_discharge_barrel: snapshot.last_weapon_discharge_barrel,
            last_weapon_discharge_frame: snapshot.last_weapon_discharge_frame,
            collector_runtime: snapshot.collector_runtime,
            weapon_suspend_fx_frames: snapshot.weapon_suspend_fx_frames,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_pre_v8_v7_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV8WorldSnapshot::from(snapshot))
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV7WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 6,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot
                .objects
                .into_iter()
                .map(|(id, object)| (id, object.into()))
                .collect(),
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
        }
    }
}

#[cfg(test)]
impl From<ObjectSnapshot> for PreV7ObjectSnapshot {
    fn from(snapshot: ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: snapshot.hacker_disable_channel,
            weapon_barrel_states: snapshot.weapon_barrel_states,
            last_weapon_discharge_sequence: snapshot.last_weapon_discharge_sequence,
            last_weapon_discharge_slot: snapshot.last_weapon_discharge_slot,
            last_weapon_discharge_barrel: snapshot.last_weapon_discharge_barrel,
            last_weapon_discharge_frame: snapshot.last_weapon_discharge_frame,
            collector_runtime: snapshot.collector_runtime,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_pre_v7_v6_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV7WorldSnapshot::from(snapshot))
}

#[cfg(test)]
impl From<ObjectSnapshot> for PreV5ObjectSnapshot {
    fn from(snapshot: ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: snapshot.hacker_disable_channel,
            weapon_barrel_states: snapshot.weapon_barrel_states,
            last_weapon_discharge_sequence: snapshot.last_weapon_discharge_sequence,
            last_weapon_discharge_slot: snapshot.last_weapon_discharge_slot,
            last_weapon_discharge_barrel: snapshot.last_weapon_discharge_barrel,
            last_weapon_discharge_frame: snapshot.last_weapon_discharge_frame,
        }
    }
}

#[cfg(test)]
impl From<ObjectSnapshot> for PreV4ObjectSnapshot {
    fn from(snapshot: ObjectSnapshot) -> Self {
        Self {
            id: snapshot.id,
            template_name: snapshot.template_name,
            team: snapshot.team,
            player_id: snapshot.player_id,
            geometry: snapshot.geometry,
            status: snapshot.status,
            health: snapshot.health,
            movement: snapshot.movement,
            experience: snapshot.experience,
            weapons: snapshot.weapons,
            contained_objects: snapshot.contained_objects,
            container_object: snapshot.container_object,
            modules: snapshot.modules,
            object_type: snapshot.object_type,
            hacker_disable_channel: snapshot.hacker_disable_channel,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_pre_v4_v3_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV4WorldSnapshot::from(snapshot))
}

#[cfg(test)]
pub(crate) fn serialize_pre_v5_v4_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV5WorldSnapshot::from(snapshot))
}

#[cfg(test)]
pub(crate) fn serialize_pre_v6_v5_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV6WorldSnapshot::from(snapshot))
}

#[cfg(test)]
pub(crate) fn serialize_pre_hacker_disable_v2_fixture(
    snapshot: WorldSnapshot,
) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreHackerDisableWorldSnapshot::from(snapshot))
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV10WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 9,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_pre_v10_v9_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV10WorldSnapshot::from(snapshot))
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV11WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 10,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_pre_v11_v10_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV11WorldSnapshot::from(snapshot))
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV13WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 12,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_pre_v13_v12_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV13WorldSnapshot::from(snapshot))
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV14WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 13,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_pre_v14_v13_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV14WorldSnapshot::from(snapshot))
}

#[cfg(test)]
impl From<WorldSnapshot> for PreV15WorldSnapshot {
    fn from(snapshot: WorldSnapshot) -> Self {
        Self {
            version: 14,
            timestamp: snapshot.timestamp,
            frame_number: snapshot.frame_number,
            random_seed: snapshot.random_seed,
            objects: snapshot.objects,
            players: snapshot.players,
            teams: snapshot.teams,
            terrain: snapshot.terrain,
            weather: snapshot.weather,
            resource_manager: snapshot.resource_manager,
            combat_tracker: snapshot.combat_tracker,
            experience_tracker: snapshot.experience_tracker,
            pathfinding_cache: snapshot.pathfinding_cache,
            ai_players: snapshot.ai_players,
            global_ai_state: snapshot.global_ai_state,
            special_power_strikes: snapshot.special_power_strikes,
            combat_particles: snapshot.combat_particles,
            host_upgrades: snapshot.host_upgrades,
            next_weapon_discharge_sequence: snapshot.next_weapon_discharge_sequence,
            client_drawables: snapshot.client_drawables,
            player_template_bindings: snapshot.player_template_bindings,
            shroud: snapshot.shroud,
            lifecycle_tail: snapshot.lifecycle_tail,
            player_ranks: snapshot.player_ranks,
            object_instance_guards: snapshot.object_instance_guards,
            overcharge_active: snapshot.overcharge_active,
            cia_intelligence: snapshot.cia_intelligence,
            vision_spied: snapshot.vision_spied,
            builder_tasks: snapshot.builder_tasks,
            sell_list: snapshot.sell_list,
            object_persist: snapshot.object_persist,
            client_drawable_visuals: snapshot.client_drawable_visuals,
        }
    }
}

#[cfg(test)]
pub(crate) fn serialize_pre_v15_v14_fixture(snapshot: WorldSnapshot) -> bincode::Result<Vec<u8>> {
    bincode::serialize(&PreV15WorldSnapshot::from(snapshot))
}
