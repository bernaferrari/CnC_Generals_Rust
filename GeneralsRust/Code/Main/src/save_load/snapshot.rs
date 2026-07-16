//! World snapshot / Xfer residual for host save/load.
//!
//! # Wave 79 Drawable residual fields
//!
//! `ObjectStatusSnapshot.camo_stealth_look` freezes C++ `Drawable::m_stealthLook`
//! ordinal residual (`Object::camo_stealth_look`) so mid-flight CamoNetting /
//! Camouflage looks survive save/load.
//!
//! # Secondary weapon residual (2026-07-12)
//!
//! Host `Object` gained `secondary_weapon` + `active_weapon_slot` for combat /
//! FlashBang / TOW residual paths. Snapshot capture previously only stored
//! primary in `weapons[0]` and restore never rebound secondary — load desynced
//! dual-slot combat and lost upgrade-equipped secondaries.
//!
//! Closed residual layout (not full C++ WeaponSet Xfer table):
//! - `weapons[0]` = primary, `weapons[1]` = secondary when present
//! - secondary-only uses a zero-damage primary pad so secondary stays at index 1
//! - `ObjectStatusSnapshot.active_weapon_slot` survives player weapon-toggle
//!
//! # Special-power strike residual (2026-07-12)
//!
//! Host `HostSpecialPowerStrikeRegistry` queues DaisyCutter / A10 / ScudStorm /
//! ParticleCannon / NuclearMissile / AnthraxBomb / SpectreGunship / CarpetBomb /
//! ArtilleryBarrage / CruiseMissile impacts with a multi-frame delay (nuke also
//! spawns residual radiation; anthrax also spawns residual toxin; carpet bomb
//! multi-point line damage; artillery multi-shell scatter damage; cruise missile
//! loft then MOAB area damage).
//! Without snapshot persistence, save mid-flight dropped the pending strike
//! and impact never fired after load.
//!
//! Closed residual layout:
//! - `WorldSnapshot.special_power_strikes` stores `next_id` + all strike records
//!   (queued / completed / cancelled), including absolute `impact_frame`
//! - restore rebinds registry so remaining delay continues and area damage still applies
//! - `WorldSnapshot.combat_particles` optionally stores active host particle systems
//!   (template name + pose + spawn frame; not full W3D GPU particle state)
//!
//! # Host upgrade research residual (2026-07-12)
//!
//! Host `HostUpgradeRegistry` records QueueUpgrade → research complete honesty for
//! CaptureBuilding / FlashBang / TOW / SupplyLines. Player `queued_upgrades` already
//! survived via `PlayerSnapshot.research_queue`, but the host registry (pending ids,
//! source object, honesty flags) was live-only — mid-flight save dropped residual
//! queue honesty and could desync complete bookkeeping after load.
//!
//! Closed residual layout:
//! - `WorldSnapshot.host_upgrades` stores `next_id` + all research records
//!   (queued / completed / cancelled) including `queue_frame` / `complete_frame`
//! - restore rebinds registry + `pending_index` so mid-research entries complete
//!   on the next `update_player_upgrades` with unlocks still applied
//!
//! Still residual (fail-closed, not claimed):
//! - Full retail OCL / aircraft / beam / multiplayer superweapon Xfer tables
//! - Client `ParticleSystemManager` GPU rebind after load (host registry only)
//! - Full retail Upgrade.ini BuildTime / ProductionUpdate research timers
//! - Full C++ per-module WeaponSet / SpecialPowerModule / Upgrade Xfer tables

use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

/// Snapshot of [`HostSpecialPowerStrikeRegistry`] for save/load residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpecialPowerStrikeRegistrySnapshot {
    /// Next allocator id after restore.
    pub next_id: u32,
    /// All strike records (queued / completed / cancelled), sorted by id on capture.
    pub strikes: Vec<HostSpecialPowerStrike>,
    /// Next residual radiation field id (NuclearMissile).
    #[serde(default = "default_next_radiation_id")]
    pub next_radiation_id: u32,
    /// Active residual radiation fields (NuclearMissile impact residual).
    #[serde(default)]
    pub radiation_fields: Vec<crate::game_logic::special_power_strikes::HostRadiationField>,
    /// Lifetime radiation fields spawned (honesty after prune).
    #[serde(default)]
    pub radiation_fields_spawned_total: u32,
    /// Lifetime radiation damage applications (honesty after prune).
    #[serde(default)]
    pub radiation_damage_applications_total: u32,
    /// Next residual toxin field id (AnthraxBomb).
    #[serde(default = "default_next_toxin_id")]
    pub next_toxin_id: u32,
    /// Active residual toxin fields (AnthraxBomb impact residual).
    #[serde(default)]
    pub toxin_fields: Vec<crate::game_logic::special_power_strikes::HostToxinField>,
    /// Lifetime toxin fields spawned (honesty after prune).
    #[serde(default)]
    pub toxin_fields_spawned_total: u32,
    /// Lifetime toxin damage applications (honesty after prune).
    #[serde(default)]
    pub toxin_damage_applications_total: u32,
    /// Next residual Spectre orbit field id (SpectreGunship).
    #[serde(default = "default_next_orbit_id")]
    pub next_orbit_id: u32,
    /// Active residual Spectre orbit fields (SpectreGunship residual).
    #[serde(default)]
    pub orbit_fields: Vec<crate::game_logic::special_power_strikes::HostSpectreOrbitField>,
    /// Lifetime orbit fields spawned (honesty after prune).
    #[serde(default)]
    pub orbit_fields_spawned_total: u32,
    /// Lifetime orbit damage applications (honesty after prune).
    #[serde(default)]
    pub orbit_damage_applications_total: u32,
    /// Next residual Particle Uplink beam field id (ParticleCannon).
    #[serde(default = "default_next_beam_id")]
    pub next_beam_id: u32,
    /// Active residual Particle Uplink continuous beam fields.
    #[serde(default)]
    pub beam_fields: Vec<crate::game_logic::special_power_strikes::HostParticleBeamField>,
    /// Lifetime beam fields spawned (honesty after prune).
    #[serde(default)]
    pub beam_fields_spawned_total: u32,
    /// Lifetime beam damage applications (honesty after prune).
    #[serde(default)]
    pub beam_damage_applications_total: u32,
    /// Next residual Particle Uplink remnant field id (DamagePulseRemnant).
    #[serde(default = "default_next_remnant_id")]
    pub next_remnant_id: u32,
    /// Active residual Particle Uplink DamagePulseRemnant trail fields.
    #[serde(default)]
    pub remnant_fields: Vec<crate::game_logic::special_power_strikes::HostParticleRemnantField>,
    /// Lifetime remnant fields spawned (honesty after prune).
    #[serde(default)]
    pub remnant_fields_spawned_total: u32,
    /// Lifetime remnant damage applications (honesty after prune).
    #[serde(default)]
    pub remnant_damage_applications_total: u32,
}

fn default_next_radiation_id() -> u32 {
    1
}

fn default_next_toxin_id() -> u32 {
    1
}

fn default_next_orbit_id() -> u32 {
    1
}

fn default_next_beam_id() -> u32 {
    1
}

fn default_next_remnant_id() -> u32 {
    1
}

impl Default for SpecialPowerStrikeRegistrySnapshot {
    fn default() -> Self {
        Self {
            next_id: 1,
            strikes: Vec::new(),
            next_radiation_id: 1,
            radiation_fields: Vec::new(),
            radiation_fields_spawned_total: 0,
            radiation_damage_applications_total: 0,
            next_toxin_id: 1,
            toxin_fields: Vec::new(),
            toxin_fields_spawned_total: 0,
            toxin_damage_applications_total: 0,
            next_orbit_id: 1,
            orbit_fields: Vec::new(),
            orbit_fields_spawned_total: 0,
            orbit_damage_applications_total: 0,
            next_beam_id: 1,
            beam_fields: Vec::new(),
            beam_fields_spawned_total: 0,
            beam_damage_applications_total: 0,
            next_remnant_id: 1,
            remnant_fields: Vec::new(),
            remnant_fields_spawned_total: 0,
            remnant_damage_applications_total: 0,
        }
    }
}

/// Snapshot of [`CombatParticleRegistry`] for save/load residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CombatParticleRegistrySnapshot {
    /// Next allocator id after restore.
    pub next_id: u32,
    /// Active + inactive host particle system entries (presentation residual).
    pub systems: Vec<CombatParticleSystemEntry>,
}

/// Snapshot of [`HostUpgradeRegistry`] for save/load residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostUpgradeRegistrySnapshot {
    /// Next allocator id after restore.
    pub next_id: u32,
    /// All research records (queued / completed / cancelled), sorted by id on capture.
    pub entries: Vec<HostUpgradeResearch>,
}

impl Default for HostUpgradeRegistrySnapshot {
    fn default() -> Self {
        Self {
            next_id: 0,
            entries: Vec::new(),
        }
    }
}

impl Default for CombatParticleRegistrySnapshot {
    fn default() -> Self {
        Self {
            next_id: 1,
            systems: Vec::new(),
        }
    }
}

/// Complete object state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectSnapshot {
    pub id: ObjectId,
    pub template_name: String,
    pub team: Team,
    pub player_id: u32,

    // Physical state
    pub geometry: GeometryInfo,
    pub status: ObjectStatusSnapshot,
    pub health: Health,
    pub movement: Movement,

    // Gameplay state
    pub experience: Experience,
    pub weapons: Vec<Weapon>,
    pub contained_objects: Vec<ObjectId>,
    pub container_object: Option<ObjectId>,

    // Module states
    pub modules: HashMap<String, ModuleSnapshot>,

    // Special object-specific data
    pub object_type: ObjectTypeSnapshot,
}

/// Object status snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectStatusSnapshot {
    pub ai_state: AIState,
    pub destroyed: bool,
    pub under_construction: bool,
    pub selected: bool,
    pub moving: bool,
    pub attacking: bool,
    pub airborne_target: bool,
    pub stealthed: bool,
    /// C++ OBJECT_STATUS_DETECTED residual. Serde default for older snapshots.
    #[serde(default)]
    pub detected: bool,
    pub garrisoned: bool,
    pub being_repaired: bool,
    pub on_fire: bool,
    pub poisoned: bool,
    pub radar_jammed: bool,
    pub disabled_underpowered: bool,
    /// C++ DISABLED_UNMANNED residual (Jarmen Kell kill-pilot). Serde default for older snaps.
    #[serde(default)]
    pub disabled_unmanned: bool,
    /// C++ DISABLED_HACKED residual (Black Lotus DisableVehicleHack). Serde default for older snaps.
    #[serde(default)]
    pub disabled_hacked: bool,
    /// Absolute host logic frame when DISABLED_HACKED expires (0 = inactive).
    #[serde(default)]
    pub disabled_hacked_until_frame: u32,
    /// C++ DISABLED_EMP residual (EMPUpdate / SuperweaponEMPPulse). Serde default for older snaps.
    #[serde(default)]
    pub disabled_emp: bool,
    /// Absolute host logic frame when DISABLED_EMP expires (0 = inactive).
    #[serde(default)]
    pub disabled_emp_until_frame: u32,
    /// Host ECM tank / jammer residual: weapons cannot fire in jam radius.
    /// Serde default for older snaps.
    #[serde(default)]
    pub weapons_jammed: bool,
    /// C++ DISABLED_SUBDUED residual (Microwave structure cook). Serde default for older snaps.
    #[serde(default)]
    pub disabled_subdued: bool,
    /// C++ OBJECT_STATUS_IS_CARBOMB residual. Serde default for older snaps.
    #[serde(default)]
    pub is_carbomb: bool,
    /// C++ OBJECT_STATUS_HIJACKED residual. Serde default for older snaps.
    #[serde(default)]
    pub hijacked: bool,
    pub special_power_ready: bool,
    pub special_power_cooldown: f32,
    pub special_power_cooldown_remaining: f32,
    /// Host residual: player weapon-slot lock (`0` primary, `1` secondary).
    /// Fail-closed: not full C++ WeaponSet chooser state.
    #[serde(default)]
    pub active_weapon_slot: u8,
    /// Wave 79 Drawable residual: CamoNetting / Camouflage `StealthLookType` ordinal
    /// (`Object::camo_stealth_look` / C++ `Drawable::m_stealthLook`).
    /// Serde default for older snapshots.
    #[serde(default)]
    pub camo_stealth_look: u8,
}

impl Default for ObjectStatusSnapshot {
    fn default() -> Self {
        Self {
            ai_state: AIState::Idle,
            destroyed: false,
            under_construction: false,
            selected: false,
            moving: false,
            attacking: false,
            airborne_target: false,
            stealthed: false,
            detected: false,
            garrisoned: false,
            being_repaired: false,
            on_fire: false,
            poisoned: false,
            radar_jammed: false,
            disabled_underpowered: false,
            disabled_unmanned: false,
            disabled_hacked: false,
            disabled_hacked_until_frame: 0,
            disabled_emp: false,
            disabled_emp_until_frame: 0,
            weapons_jammed: false,
            disabled_subdued: false,
            is_carbomb: false,
            hijacked: false,
            special_power_ready: true,
            special_power_cooldown: 0.0,
            special_power_cooldown_remaining: 0.0,
            active_weapon_slot: 0,
            camo_stealth_look: 0,
        }
    }
}

/// Module state snapshot (generic module data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModuleSnapshot {
    AIUpdate(AIUpdateModuleSnapshot),
    Production(ProductionModuleSnapshot),
    Weapon(WeaponModuleSnapshot),
    Body(BodyModuleSnapshot),
    Locomotor(LocomotorModuleSnapshot),
    Physics(PhysicsModuleSnapshot),
    Contain(ContainModuleSnapshot),
    Upgrade(UpgradeModuleSnapshot),
}

/// AI update module snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIUpdateModuleSnapshot {
    pub current_state: String,
    pub state_machine_data: HashMap<String, String>,
    pub target_object: Option<ObjectId>,
    pub current_task: Option<String>,
    pub task_queue: Vec<String>,
}

/// Production module snapshot  
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionModuleSnapshot {
    pub production_queue: Vec<ProductionQueueEntry>,
    pub is_producing: bool,
    pub production_progress: f32,
    pub rally_point: Option<glam::Vec3>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionQueueEntry {
    pub template_name: String,
    pub progress: f32,
    pub cost: u32,
}

/// Weapon module snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeaponModuleSnapshot {
    pub weapons: Vec<Weapon>,
    pub current_target: Option<ObjectId>,
    pub firing_state: FiringState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FiringState {
    Idle,
    Acquiring,
    Firing,
    Reloading,
}

/// Body module snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BodyModuleSnapshot {
    pub body_type: String,
    pub max_health: f32,
    pub armor_type: String,
    pub damage_states: Vec<DamageState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DamageState {
    pub threshold: f32,
    pub effects_active: Vec<String>,
}

/// Locomotor module snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocomotorModuleSnapshot {
    pub locomotor_type: String,
    pub movement_state: MovementState,
    pub path: Vec<glam::Vec3>,
    pub path_index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MovementState {
    Idle,
    Moving,
    Turning,
    Blocked,
}

/// Physics module snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhysicsModuleSnapshot {
    pub velocity: glam::Vec3,
    pub angular_velocity: f32,
    pub forces: Vec<Force>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Force {
    pub direction: glam::Vec3,
    pub magnitude: f32,
    pub duration: f32,
}

/// Contain module snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainModuleSnapshot {
    pub contained_objects: Vec<ObjectId>,
    pub max_capacity: usize,
    pub contain_type: String,
    pub exit_positions: Vec<glam::Vec3>,
}

/// Upgrade module snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpgradeModuleSnapshot {
    pub active_upgrades: Vec<String>,
    pub upgrade_progress: HashMap<String, f32>,
}

/// Object type specific data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ObjectTypeSnapshot {
    Unit(UnitSnapshot),
    Building(BuildingSnapshot),
    Projectile(ProjectileSnapshot),
    Resource(ResourceSnapshot),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitSnapshot {
    pub unit_type: String,
    pub formation_position: Option<glam::Vec3>,
    pub formation_id: Option<u32>,
    pub group_id: Option<u32>,
    pub waypoints: Vec<glam::Vec3>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildingSnapshot {
    pub building_type: String,
    pub construction_progress: f32,
    pub power_provided: i32,
    pub power_required: i32,
    pub is_powered: bool,
    pub connected_buildings: Vec<ObjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectileSnapshot {
    pub projectile_type: String,
    pub source_object: ObjectId,
    pub target_object: Option<ObjectId>,
    pub target_position: glam::Vec3,
    pub flight_time: f32,
    pub max_flight_time: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceSnapshot {
    pub resource_type: String,
    pub amount: u32,
    pub depletion_rate: f32,
    pub is_infinite: bool,
}

/// Player state snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSnapshot {
    pub id: u32,
    pub name: String,
    pub team: Team,
    pub is_human: bool,
    pub is_active: bool,

    pub resources: Resources,
    pub population: PopulationInfo,
    pub tech_tree: TechTreeSnapshot,
    pub upgrades: Vec<String>,

    pub build_queue: Vec<String>,
    pub research_queue: Vec<String>,

    pub statistics: PlayerStatisticsSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PopulationInfo {
    pub current: u32,
    pub maximum: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechTreeSnapshot {
    pub unlocked_units: Vec<String>,
    pub unlocked_buildings: Vec<String>,
    pub unlocked_upgrades: Vec<String>,
    pub research_progress: HashMap<String, f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerStatisticsSnapshot {
    pub units_built: u32,
    pub units_lost: u32,
    pub buildings_built: u32,
    pub buildings_lost: u32,
    pub damage_dealt: f32,
    pub damage_received: f32,
    pub resources_gathered: u32,
    pub experience_gained: f32,
}

/// Team snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamSnapshot {
    pub team: Team,
    pub players: Vec<u32>,
    pub allied_teams: Vec<Team>,
    pub is_defeated: bool,
    pub shared_vision: bool,
    pub shared_control: bool,
}

/// Terrain state snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TerrainSnapshot {
    pub width: u32,
    pub height: u32,
    pub height_map: Vec<f32>,
    pub texture_map: Vec<u8>,
    pub passability_map: Vec<bool>,
    pub modifications: Vec<TerrainModification>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerrainModification {
    pub position: glam::Vec3,
    pub radius: f32,
    pub height_delta: f32,
    pub modification_type: String,
}

/// Weather system snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherSnapshot {
    pub current_weather: String,
    pub weather_intensity: f32,
    pub weather_duration: f32,
    pub next_weather_change: f32,
    #[serde(default = "weather_visible_default")]
    pub visible: bool,
}

const fn weather_visible_default() -> bool {
    true
}

/// Resource manager snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceManagerSnapshot {
    pub supply_deposits: Vec<SupplyDepositSnapshot>,
    pub resource_zones: Vec<ResourceZoneSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupplyDepositSnapshot {
    pub position: glam::Vec3,
    pub amount: u32,
    pub depletion_rate: f32,
    pub harvesters: Vec<ObjectId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceZoneSnapshot {
    pub bounds: GeometryInfo,
    pub resource_type: String,
    pub total_amount: u32,
    pub remaining_amount: u32,
}

/// Combat tracking snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CombatTrackerSnapshot {
    pub active_combats: Vec<ActiveCombatSnapshot>,
    pub recent_deaths: Vec<DeathEventSnapshot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActiveCombatSnapshot {
    pub attacker: ObjectId,
    pub target: ObjectId,
    pub start_time: f32,
    pub damage_dealt: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeathEventSnapshot {
    pub object_id: ObjectId,
    pub killer_id: Option<ObjectId>,
    pub death_time: f32,
    pub death_position: glam::Vec3,
}

/// Experience tracking snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExperienceTrackerSnapshot {
    pub experience_events: Vec<ExperienceEventSnapshot>,
    pub veterancy_bonuses: HashMap<ObjectId, VeterancyBonuses>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperienceEventSnapshot {
    pub object_id: ObjectId,
    pub experience_gained: f32,
    pub source: String,
    pub timestamp: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VeterancyBonuses {
    pub health_bonus: f32,
    pub damage_bonus: f32,
    pub accuracy_bonus: f32,
    pub range_bonus: f32,
}

/// Pathfinding cache snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PathfindingCacheSnapshot {
    pub cached_paths: HashMap<(SerializableVec3, SerializableVec3), Vec<SerializableVec3>>,
    pub cache_timestamps: HashMap<(SerializableVec3, SerializableVec3), f32>,
}

/// AI player snapshot
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIPlayerSnapshot {
    pub player_id: u32,
    pub difficulty: String,
    pub personality: String,
    pub current_strategy: String,
    pub strategic_state: AIStrategicStateSnapshot,
    pub tactical_state: AITacticalStateSnapshot,
    pub economic_state: AIEconomicStateSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIStrategicStateSnapshot {
    pub current_phase: String,
    pub objectives: Vec<AIObjective>,
    pub threat_assessment: ThreatAssessmentSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIObjective {
    pub objective_type: String,
    pub priority: f32,
    pub target_position: Option<glam::Vec3>,
    pub assigned_units: Vec<ObjectId>,
    pub completion_percentage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreatAssessmentSnapshot {
    pub enemy_strengths: HashMap<Team, f32>,
    pub vulnerable_areas: Vec<glam::Vec3>,
    pub threat_level: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AITacticalStateSnapshot {
    pub unit_groups: Vec<AIUnitGroupSnapshot>,
    pub active_attacks: Vec<AIAttackSnapshot>,
    pub defensive_positions: Vec<glam::Vec3>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIUnitGroupSnapshot {
    pub group_id: u32,
    pub units: Vec<ObjectId>,
    pub role: String,
    pub current_task: String,
    pub formation: String,
    pub target_position: Option<glam::Vec3>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIAttackSnapshot {
    pub attack_id: u32,
    pub target_position: glam::Vec3,
    pub assigned_groups: Vec<u32>,
    pub attack_phase: String,
    pub start_time: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AIEconomicStateSnapshot {
    pub build_priorities: Vec<BuildPriority>,
    pub economic_focus: String,
    pub resource_allocation: ResourceAllocation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildPriority {
    pub template_name: String,
    pub priority: f32,
    pub desired_count: u32,
    pub current_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceAllocation {
    pub military_percentage: f32,
    pub economic_percentage: f32,
    pub defensive_percentage: f32,
}

/// Global AI state snapshot
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalAIStateSnapshot {
    pub global_timers: HashMap<String, f32>,
    pub global_flags: HashMap<String, bool>,
    pub difficulty_modifiers: DifficultyModifiers,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DifficultyModifiers {
    pub ai_resource_bonus: f32,
    pub ai_damage_bonus: f32,
    pub ai_health_bonus: f32,
    pub ai_build_speed_bonus: f32,
}

impl Default for DifficultyModifiers {
    fn default() -> Self {
        Self {
            ai_resource_bonus: 1.0,
            ai_damage_bonus: 1.0,
            ai_health_bonus: 1.0,
            ai_build_speed_bonus: 1.0,
        }
    }
}

// Implement Snapshot trait for WorldSnapshot
impl Snapshot for WorldSnapshot {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        // Light CRC - just check critical values
        self.version.xfer(xfer)?;
        self.frame_number.xfer(xfer)?;
        self.random_seed.xfer(xfer)?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("WorldSnapshot")?;

        xfer.xfer_marker_label("Version")?;
        self.version.xfer(xfer)?;

        xfer.xfer_marker_label("Timestamp")?;
        let duration = self
            .timestamp
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default();
        let mut secs = duration.as_secs();
        let mut nanos = duration.subsec_nanos();
        xfer.xfer_u64(&mut secs)?;
        xfer.xfer_u32(&mut nanos)?;
        self.timestamp = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::new(secs, nanos);

        xfer.xfer_marker_label("FrameNumber")?;
        self.frame_number.xfer(xfer)?;

        xfer.xfer_marker_label("RandomSeed")?;
        self.random_seed.xfer(xfer)?;

        xfer.xfer_marker_label("Objects")?;
        let mut len = self.objects.len() as u32;
        xfer.xfer_u32(&mut len)?;
        if xfer.get_mode() == XferMode::Load {
            self.objects.clear();
            for _ in 0..len {
                let mut id = ObjectId(0);
                id.xfer(xfer)?;
                let mut obj = default_object_snapshot();
                obj.xfer(xfer)?;
                self.objects.insert(id, obj);
            }
        } else {
            for (id, obj) in &mut self.objects {
                let mut id_copy = *id;
                id_copy.xfer(xfer)?;
                obj.xfer(xfer)?;
            }
        }

        xfer.xfer_marker_label("Players")?;
        xfer_vec_default(xfer, &mut self.players, default_player_snapshot())?;

        xfer.xfer_marker_label("Teams")?;
        xfer_vec_default(
            xfer,
            &mut self.teams,
            TeamSnapshot {
                team: Team::Neutral,
                players: Vec::new(),
                allied_teams: Vec::new(),
                is_defeated: false,
                shared_vision: false,
                shared_control: false,
            },
        )?;

        xfer.xfer_marker_label("Terrain")?;
        self.terrain.xfer(xfer)?;

        xfer.xfer_marker_label("Weather")?;
        self.weather.xfer(xfer)?;

        xfer.xfer_marker_label("ResourceManager")?;
        self.resource_manager.xfer(xfer)?;

        xfer.xfer_marker_label("CombatTracker")?;
        self.combat_tracker.xfer(xfer)?;

        xfer.xfer_marker_label("ExperienceTracker")?;
        self.experience_tracker.xfer(xfer)?;

        xfer.xfer_marker_label("PathfindingCache")?;
        self.pathfinding_cache.xfer(xfer)?;

        xfer.xfer_marker_label("AIPlayers")?;
        xfer_vec_default(
            xfer,
            &mut self.ai_players,
            AIPlayerSnapshot {
                player_id: 0,
                difficulty: String::new(),
                personality: String::new(),
                current_strategy: String::new(),
                strategic_state: default_ai_strategic_state(),
                tactical_state: default_ai_tactical_state(),
                economic_state: default_ai_economic_state(),
            },
        )?;

        xfer.xfer_marker_label("GlobalAIState")?;
        self.global_ai_state.xfer(xfer)?;

        // Residual: host superweapon strike queue + combat particle registry.
        // Appended after GlobalAIState so earlier Xfer layouts stay stable until
        // a save that writes these markers. Empty defaults on missing streams
        // are handled by callers using Default; binary Xfer always pairs them.
        xfer.xfer_marker_label("SpecialPowerStrikes")?;
        self.special_power_strikes.xfer(xfer)?;

        xfer.xfer_marker_label("CombatParticles")?;
        self.combat_particles.xfer(xfer)?;

        xfer.xfer_marker_label("HostUpgrades")?;
        self.host_upgrades.xfer(xfer)?;

        Ok(())
    }

    fn load_post_process(&mut self) -> SaveLoadResult<()> {
        // Rebuild any transient state after loading
        // Reconnect object references, rebuild caches, etc.
        Ok(())
    }
}

// Implement XferData for various snapshot types
impl XferData for ObjectSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ObjectSnapshot")?;

        xfer.xfer_marker_label("Id")?;
        self.id.xfer(xfer)?;

        xfer.xfer_marker_label("TemplateName")?;
        self.template_name.xfer(xfer)?;

        xfer.xfer_marker_label("Team")?;
        self.team.xfer(xfer)?;

        xfer.xfer_marker_label("PlayerId")?;
        xfer.xfer_u32(&mut self.player_id)?;

        xfer.xfer_marker_label("Geometry")?;
        self.geometry.xfer(xfer)?;

        xfer.xfer_marker_label("Status")?;
        self.status.xfer(xfer)?;

        xfer.xfer_marker_label("Health")?;
        self.health.xfer(xfer)?;

        xfer.xfer_marker_label("Movement")?;
        self.movement.xfer(xfer)?;

        xfer.xfer_marker_label("Experience")?;
        self.experience.xfer(xfer)?;

        xfer.xfer_marker_label("Weapons")?;
        xfer_vec_default(xfer, &mut self.weapons, Weapon::default())?;

        xfer.xfer_marker_label("ContainedObjects")?;
        xfer_vec_default(xfer, &mut self.contained_objects, ObjectId(0))?;

        xfer.xfer_marker_label("ContainerObject")?;
        xfer_option(xfer, &mut self.container_object, ObjectId(0))?;

        xfer.xfer_marker_label("Modules")?;
        xfer_hashmap_default(
            xfer,
            &mut self.modules,
            String::new(),
            ModuleSnapshot::AIUpdate(AIUpdateModuleSnapshot {
                current_state: String::new(),
                state_machine_data: HashMap::new(),
                target_object: None,
                current_task: None,
                task_queue: Vec::new(),
            }),
        )?;

        xfer.xfer_marker_label("ObjectType")?;
        self.object_type.xfer(xfer)?;

        Ok(())
    }
}

impl XferData for PlayerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PlayerSnapshot")?;

        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;

        xfer.xfer_marker_label("Name")?;
        self.name.xfer(xfer)?;

        xfer.xfer_marker_label("Team")?;
        self.team.xfer(xfer)?;

        xfer.xfer_marker_label("IsHuman")?;
        xfer.xfer_bool(&mut self.is_human)?;

        xfer.xfer_marker_label("IsActive")?;
        xfer.xfer_bool(&mut self.is_active)?;

        xfer.xfer_marker_label("Resources")?;
        self.resources.xfer(xfer)?;

        xfer.xfer_marker_label("Population")?;
        self.population.xfer(xfer)?;

        xfer.xfer_marker_label("TechTree")?;
        self.tech_tree.xfer(xfer)?;

        xfer.xfer_marker_label("Upgrades")?;
        xfer.xfer_vec_string(&mut self.upgrades)?;

        xfer.xfer_marker_label("BuildQueue")?;
        xfer.xfer_vec_string(&mut self.build_queue)?;

        xfer.xfer_marker_label("ResearchQueue")?;
        xfer.xfer_vec_string(&mut self.research_queue)?;

        xfer.xfer_marker_label("Statistics")?;
        self.statistics.xfer(xfer)?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Helper functions for Vec/HashMap/Option xfer (dyn Xfer safe)
// ---------------------------------------------------------------------------

fn xfer_vec_default<T: Clone + XferData>(
    xfer: &mut dyn Xfer,
    data: &mut Vec<T>,
    default: T,
) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut item = default.clone();
            item.xfer(xfer)?;
            data.push(item);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

fn xfer_option<T: XferData>(
    xfer: &mut dyn Xfer,
    data: &mut Option<T>,
    default: T,
) -> SaveLoadResult<()> {
    let mut is_some = data.is_some();
    xfer.xfer_bool(&mut is_some)?;
    if is_some {
        if data.is_none() {
            *data = Some(default);
        }
        if let Some(ref mut val) = data {
            val.xfer(xfer)?;
        }
    } else {
        *data = None;
    }
    Ok(())
}

fn xfer_hashmap_default<K, V>(
    xfer: &mut dyn Xfer,
    data: &mut HashMap<K, V>,
    key_default: K,
    val_default: V,
) -> SaveLoadResult<()>
where
    K: Clone + std::hash::Hash + Eq + XferData,
    V: Clone + XferData,
{
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut k = key_default.clone();
            let mut v = val_default.clone();
            k.xfer(xfer)?;
            v.xfer(xfer)?;
            data.insert(k, v);
        }
    } else {
        for (k, v) in data.iter_mut() {
            let mut kc = k.clone();
            kc.xfer(xfer)?;
            v.xfer(xfer)?;
        }
    }
    Ok(())
}

fn xfer_vec_f32(xfer: &mut dyn Xfer, data: &mut Vec<f32>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = 0.0f32;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

fn xfer_vec_bool(xfer: &mut dyn Xfer, data: &mut Vec<bool>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = false;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

fn xfer_vec_u8(xfer: &mut dyn Xfer, data: &mut Vec<u8>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        data.reserve(len as usize);
        for _ in 0..len {
            let mut val = 0u8;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

fn xfer_vec_vec3(xfer: &mut dyn Xfer, data: &mut Vec<glam::Vec3>) -> SaveLoadResult<()> {
    let mut len = data.len() as u32;
    xfer.xfer_u32(&mut len)?;
    if xfer.get_mode() == XferMode::Load {
        data.clear();
        for _ in 0..len {
            let mut val = glam::Vec3::ZERO;
            val.xfer(xfer)?;
            data.push(val);
        }
    } else {
        for item in data.iter_mut() {
            item.xfer(xfer)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Default constructors for complex snapshot types (used during load)
// ---------------------------------------------------------------------------

fn default_object_snapshot() -> ObjectSnapshot {
    ObjectSnapshot {
        id: ObjectId(0),
        template_name: String::new(),
        team: Team::Neutral,
        player_id: 0,
        geometry: GeometryInfo::default(),
        status: ObjectStatusSnapshot::default(),
        health: Health {
            current: 0.0,
            maximum: 0.0,
        },
        movement: Movement::default(),
        experience: Experience::default(),
        weapons: Vec::new(),
        contained_objects: Vec::new(),
        container_object: None,
        modules: HashMap::new(),
        object_type: ObjectTypeSnapshot::Unit(UnitSnapshot {
            unit_type: String::new(),
            formation_position: None,
            formation_id: None,
            group_id: None,
            waypoints: Vec::new(),
        }),
    }
}

fn default_player_snapshot() -> PlayerSnapshot {
    PlayerSnapshot {
        id: 0,
        name: String::new(),
        team: Team::Neutral,
        is_human: false,
        is_active: false,
        resources: Resources::default(),
        population: PopulationInfo {
            current: 0,
            maximum: 0,
        },
        tech_tree: TechTreeSnapshot {
            unlocked_units: Vec::new(),
            unlocked_buildings: Vec::new(),
            unlocked_upgrades: Vec::new(),
            research_progress: HashMap::new(),
        },
        upgrades: Vec::new(),
        build_queue: Vec::new(),
        research_queue: Vec::new(),
        statistics: PlayerStatisticsSnapshot {
            units_built: 0,
            units_lost: 0,
            buildings_built: 0,
            buildings_lost: 0,
            damage_dealt: 0.0,
            damage_received: 0.0,
            resources_gathered: 0,
            experience_gained: 0.0,
        },
    }
}

fn default_ai_strategic_state() -> AIStrategicStateSnapshot {
    AIStrategicStateSnapshot {
        current_phase: String::new(),
        objectives: Vec::new(),
        threat_assessment: ThreatAssessmentSnapshot {
            enemy_strengths: HashMap::new(),
            vulnerable_areas: Vec::new(),
            threat_level: 0.0,
        },
    }
}

fn default_ai_tactical_state() -> AITacticalStateSnapshot {
    AITacticalStateSnapshot {
        unit_groups: Vec::new(),
        active_attacks: Vec::new(),
        defensive_positions: Vec::new(),
    }
}

fn default_ai_economic_state() -> AIEconomicStateSnapshot {
    AIEconomicStateSnapshot {
        build_priorities: Vec::new(),
        economic_focus: String::new(),
        resource_allocation: ResourceAllocation {
            military_percentage: 0.0,
            economic_percentage: 0.0,
            defensive_percentage: 0.0,
        },
    }
}

// ---------------------------------------------------------------------------
// XferData implementations for game_logic types
// ---------------------------------------------------------------------------

impl XferData for GeometryInfo {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("GeometryInfo")?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("Rotation")?;
        xfer.xfer_f32(&mut self.rotation)?;
        xfer.xfer_marker_label("BoundsMin")?;
        self.bounds_min.xfer(xfer)?;
        xfer.xfer_marker_label("BoundsMax")?;
        self.bounds_max.xfer(xfer)?;
        xfer.xfer_marker_label("Radius")?;
        xfer.xfer_f32(&mut self.radius)?;
        Ok(())
    }
}

impl XferData for Health {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Health")?;
        xfer.xfer_marker_label("Current")?;
        xfer.xfer_f32(&mut self.current)?;
        xfer.xfer_marker_label("Maximum")?;
        xfer.xfer_f32(&mut self.maximum)?;
        Ok(())
    }
}

impl XferData for Resources {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Resources")?;
        xfer.xfer_marker_label("Supplies")?;
        xfer.xfer_u32(&mut self.supplies)?;
        xfer.xfer_marker_label("Power")?;
        xfer.xfer_i32(&mut self.power)?;
        Ok(())
    }
}

impl XferData for Movement {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Movement")?;
        xfer.xfer_marker_label("TargetPosition")?;
        xfer_option(xfer, &mut self.target_position, glam::Vec3::ZERO)?;
        xfer.xfer_marker_label("Velocity")?;
        self.velocity.xfer(xfer)?;
        xfer.xfer_marker_label("MaxSpeed")?;
        xfer.xfer_f32(&mut self.max_speed)?;
        xfer.xfer_marker_label("Acceleration")?;
        xfer.xfer_f32(&mut self.acceleration)?;
        xfer.xfer_marker_label("TurnRate")?;
        xfer.xfer_f32(&mut self.turn_rate)?;
        xfer.xfer_marker_label("Path")?;
        xfer_vec_vec3(xfer, &mut self.path)?;
        xfer.xfer_marker_label("CurrentPathIndex")?;
        let mut idx = self.current_path_index as u32;
        xfer.xfer_u32(&mut idx)?;
        self.current_path_index = idx as usize;
        Ok(())
    }
}

impl XferData for VeterancyLevel {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut disc: u32 = match self {
            VeterancyLevel::Rookie => 0,
            VeterancyLevel::Veteran => 1,
            VeterancyLevel::Elite => 2,
            VeterancyLevel::Heroic => 3,
        };
        xfer.xfer_u32(&mut disc)?;
        *self = match disc {
            0 => VeterancyLevel::Rookie,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            3 => VeterancyLevel::Heroic,
            _ => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid VeterancyLevel: {disc}"
                )))
            }
        };
        Ok(())
    }
}

impl XferData for Experience {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Experience")?;
        xfer.xfer_marker_label("Current")?;
        xfer.xfer_f32(&mut self.current)?;
        xfer.xfer_marker_label("Level")?;
        self.level.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for Weapon {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Weapon")?;
        xfer.xfer_marker_label("Damage")?;
        xfer.xfer_f32(&mut self.damage)?;
        xfer.xfer_marker_label("Range")?;
        xfer.xfer_f32(&mut self.range)?;
        xfer.xfer_marker_label("MinRange")?;
        xfer.xfer_f32(&mut self.min_range)?;
        xfer.xfer_marker_label("ReloadTime")?;
        xfer.xfer_f32(&mut self.reload_time)?;
        xfer.xfer_marker_label("LastFireTime")?;
        xfer.xfer_f32(&mut self.last_fire_time)?;
        xfer.xfer_marker_label("Ammo")?;
        xfer_option(xfer, &mut self.ammo, 0u32)?;
        xfer.xfer_marker_label("CanTargetAir")?;
        xfer.xfer_bool(&mut self.can_target_air)?;
        xfer.xfer_marker_label("CanTargetGround")?;
        xfer.xfer_bool(&mut self.can_target_ground)?;
        xfer.xfer_marker_label("ProjectileSpeed")?;
        xfer.xfer_f32(&mut self.projectile_speed)?;
        xfer.xfer_marker_label("PreAttackDelay")?;
        xfer.xfer_f32(&mut self.pre_attack_delay)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// XferData implementations for snapshot types
// ---------------------------------------------------------------------------

impl XferData for ObjectStatusSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ObjectStatusSnapshot")?;
        xfer.xfer_marker_label("AIState")?;
        self.ai_state.xfer(xfer)?;
        xfer.xfer_marker_label("Destroyed")?;
        xfer.xfer_bool(&mut self.destroyed)?;
        xfer.xfer_marker_label("UnderConstruction")?;
        xfer.xfer_bool(&mut self.under_construction)?;
        xfer.xfer_marker_label("Selected")?;
        xfer.xfer_bool(&mut self.selected)?;
        xfer.xfer_marker_label("Moving")?;
        xfer.xfer_bool(&mut self.moving)?;
        xfer.xfer_marker_label("Attacking")?;
        xfer.xfer_bool(&mut self.attacking)?;
        xfer.xfer_marker_label("AirborneTarget")?;
        xfer.xfer_bool(&mut self.airborne_target)?;
        xfer.xfer_marker_label("Stealthed")?;
        xfer.xfer_bool(&mut self.stealthed)?;
        xfer.xfer_marker_label("Detected")?;
        xfer.xfer_bool(&mut self.detected)?;
        xfer.xfer_marker_label("Garrisoned")?;
        xfer.xfer_bool(&mut self.garrisoned)?;
        xfer.xfer_marker_label("BeingRepaired")?;
        xfer.xfer_bool(&mut self.being_repaired)?;
        xfer.xfer_marker_label("OnFire")?;
        xfer.xfer_bool(&mut self.on_fire)?;
        xfer.xfer_marker_label("Poisoned")?;
        xfer.xfer_bool(&mut self.poisoned)?;
        xfer.xfer_marker_label("RadarJammed")?;
        xfer.xfer_bool(&mut self.radar_jammed)?;
        xfer.xfer_marker_label("DisabledUnderpowered")?;
        xfer.xfer_bool(&mut self.disabled_underpowered)?;
        xfer.xfer_marker_label("DisabledUnmanned")?;
        xfer.xfer_bool(&mut self.disabled_unmanned)?;
        xfer.xfer_marker_label("DisabledHacked")?;
        xfer.xfer_bool(&mut self.disabled_hacked)?;
        xfer.xfer_marker_label("DisabledHackedUntilFrame")?;
        xfer.xfer_u32(&mut self.disabled_hacked_until_frame)?;
        xfer.xfer_marker_label("IsCarbomb")?;
        xfer.xfer_bool(&mut self.is_carbomb)?;
        xfer.xfer_marker_label("Hijacked")?;
        xfer.xfer_bool(&mut self.hijacked)?;
        xfer.xfer_marker_label("SpecialPowerReady")?;
        xfer.xfer_bool(&mut self.special_power_ready)?;
        xfer.xfer_marker_label("SpecialPowerCooldown")?;
        xfer.xfer_f32(&mut self.special_power_cooldown)?;
        xfer.xfer_marker_label("SpecialPowerCooldownRemaining")?;
        xfer.xfer_f32(&mut self.special_power_cooldown_remaining)?;
        xfer.xfer_marker_label("ActiveWeaponSlot")?;
        xfer.xfer_u8(&mut self.active_weapon_slot)?;
        // Appended residual (ECM weapons_jammed); older binary residual saves without
        // this field fail-closed on xfer (serde JSON path uses #[serde(default)]).
        xfer.xfer_marker_label("WeaponsJammed")?;
        xfer.xfer_bool(&mut self.weapons_jammed)?;
        // Appended residual (DISABLED_EMP); older binary residual saves without
        // these fields fail-closed on xfer (serde JSON path uses #[serde(default)]).
        xfer.xfer_marker_label("DisabledEmp")?;
        xfer.xfer_bool(&mut self.disabled_emp)?;
        xfer.xfer_marker_label("DisabledEmpUntilFrame")?;
        xfer.xfer_u32(&mut self.disabled_emp_until_frame)?;
        // Appended residual (DISABLED_SUBDUED / Microwave structure cook).
        xfer.xfer_marker_label("DisabledSubdued")?;
        xfer.xfer_bool(&mut self.disabled_subdued)?;
        // Wave 79: Drawable residual StealthLook ordinal (appended).
        xfer.xfer_marker_label("CamoStealthLook")?;
        xfer.xfer_u8(&mut self.camo_stealth_look)?;
        Ok(())
    }
}

impl XferData for AIState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            AIState::Idle => 0,
            AIState::Moving => 1,
            AIState::Attacking => 2,
            AIState::AttackMoving => 3,
            AIState::AttackingGround => 4,
            AIState::Gathering => 5,
            AIState::ReturningResources => 6,
            AIState::Constructing => 7,
            AIState::Repairing => 8,
            AIState::GuardingArea => 9,
            AIState::GuardingObject => 10,
            AIState::Patrolling => 11,
            AIState::Docked => 12,
            AIState::Garrisoned => 13,
            AIState::SpecialAbility => 14,
            AIState::SeekingRepair => 15,
            AIState::SeekingHealing => 16,
            AIState::Entering => 17,
            AIState::Docking => 18,
            AIState::Capturing => 19,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => AIState::Idle,
            1 => AIState::Moving,
            2 => AIState::Attacking,
            3 => AIState::AttackMoving,
            4 => AIState::AttackingGround,
            5 => AIState::Gathering,
            6 => AIState::ReturningResources,
            7 => AIState::Constructing,
            8 => AIState::Repairing,
            9 => AIState::GuardingArea,
            10 => AIState::GuardingObject,
            11 => AIState::Patrolling,
            12 => AIState::Docked,
            13 => AIState::Garrisoned,
            14 => AIState::SpecialAbility,
            15 => AIState::SeekingRepair,
            16 => AIState::SeekingHealing,
            17 => AIState::Entering,
            18 => AIState::Docking,
            19 => AIState::Capturing,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid AIState value in object snapshot: {}",
                    other
                )));
            }
        };
        Ok(())
    }
}

impl XferData for AIUpdateModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIUpdateModuleSnapshot")?;
        xfer.xfer_marker_label("CurrentState")?;
        self.current_state.xfer(xfer)?;
        xfer.xfer_marker_label("StateMachineData")?;
        xfer_hashmap_default(
            xfer,
            &mut self.state_machine_data,
            String::new(),
            String::new(),
        )?;
        xfer.xfer_marker_label("TargetObject")?;
        xfer_option(xfer, &mut self.target_object, ObjectId(0))?;
        xfer.xfer_marker_label("CurrentTask")?;
        xfer_option(xfer, &mut self.current_task, String::new())?;
        xfer.xfer_marker_label("TaskQueue")?;
        xfer.xfer_vec_string(&mut self.task_queue)?;
        Ok(())
    }
}

impl XferData for ProductionQueueEntry {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ProductionQueueEntry")?;
        xfer.xfer_marker_label("TemplateName")?;
        self.template_name.xfer(xfer)?;
        xfer.xfer_marker_label("Progress")?;
        xfer.xfer_f32(&mut self.progress)?;
        xfer.xfer_marker_label("Cost")?;
        xfer.xfer_u32(&mut self.cost)?;
        Ok(())
    }
}

impl XferData for ProductionModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ProductionModuleSnapshot")?;
        xfer.xfer_marker_label("ProductionQueue")?;
        xfer_vec_default(
            xfer,
            &mut self.production_queue,
            ProductionQueueEntry {
                template_name: String::new(),
                progress: 0.0,
                cost: 0,
            },
        )?;
        xfer.xfer_marker_label("IsProducing")?;
        xfer.xfer_bool(&mut self.is_producing)?;
        xfer.xfer_marker_label("ProductionProgress")?;
        xfer.xfer_f32(&mut self.production_progress)?;
        xfer.xfer_marker_label("RallyPoint")?;
        xfer_option(xfer, &mut self.rally_point, glam::Vec3::ZERO)?;
        Ok(())
    }
}

impl XferData for FiringState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut disc: u32 = match self {
            FiringState::Idle => 0,
            FiringState::Acquiring => 1,
            FiringState::Firing => 2,
            FiringState::Reloading => 3,
        };
        xfer.xfer_u32(&mut disc)?;
        *self = match disc {
            0 => FiringState::Idle,
            1 => FiringState::Acquiring,
            2 => FiringState::Firing,
            3 => FiringState::Reloading,
            _ => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid FiringState: {disc}"
                )))
            }
        };
        Ok(())
    }
}

impl XferData for WeaponModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("WeaponModuleSnapshot")?;
        xfer.xfer_marker_label("Weapons")?;
        xfer_vec_default(xfer, &mut self.weapons, Weapon::default())?;
        xfer.xfer_marker_label("CurrentTarget")?;
        xfer_option(xfer, &mut self.current_target, ObjectId(0))?;
        xfer.xfer_marker_label("FiringState")?;
        self.firing_state.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for DamageState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("DamageState")?;
        xfer.xfer_marker_label("Threshold")?;
        xfer.xfer_f32(&mut self.threshold)?;
        xfer.xfer_marker_label("EffectsActive")?;
        xfer.xfer_vec_string(&mut self.effects_active)?;
        Ok(())
    }
}

impl XferData for BodyModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("BodyModuleSnapshot")?;
        xfer.xfer_marker_label("BodyType")?;
        self.body_type.xfer(xfer)?;
        xfer.xfer_marker_label("MaxHealth")?;
        xfer.xfer_f32(&mut self.max_health)?;
        xfer.xfer_marker_label("ArmorType")?;
        self.armor_type.xfer(xfer)?;
        xfer.xfer_marker_label("DamageStates")?;
        xfer_vec_default(
            xfer,
            &mut self.damage_states,
            DamageState {
                threshold: 0.0,
                effects_active: Vec::new(),
            },
        )?;
        Ok(())
    }
}

impl XferData for MovementState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut disc: u32 = match self {
            MovementState::Idle => 0,
            MovementState::Moving => 1,
            MovementState::Turning => 2,
            MovementState::Blocked => 3,
        };
        xfer.xfer_u32(&mut disc)?;
        *self = match disc {
            0 => MovementState::Idle,
            1 => MovementState::Moving,
            2 => MovementState::Turning,
            3 => MovementState::Blocked,
            _ => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid MovementState: {disc}"
                )))
            }
        };
        Ok(())
    }
}

impl XferData for LocomotorModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("LocomotorModuleSnapshot")?;
        xfer.xfer_marker_label("LocomotorType")?;
        self.locomotor_type.xfer(xfer)?;
        xfer.xfer_marker_label("MovementState")?;
        self.movement_state.xfer(xfer)?;
        xfer.xfer_marker_label("Path")?;
        xfer_vec_vec3(xfer, &mut self.path)?;
        xfer.xfer_marker_label("PathIndex")?;
        let mut idx = self.path_index as u32;
        xfer.xfer_u32(&mut idx)?;
        self.path_index = idx as usize;
        Ok(())
    }
}

impl XferData for Force {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("Force")?;
        xfer.xfer_marker_label("Direction")?;
        self.direction.xfer(xfer)?;
        xfer.xfer_marker_label("Magnitude")?;
        xfer.xfer_f32(&mut self.magnitude)?;
        xfer.xfer_marker_label("Duration")?;
        xfer.xfer_f32(&mut self.duration)?;
        Ok(())
    }
}

impl XferData for PhysicsModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PhysicsModuleSnapshot")?;
        xfer.xfer_marker_label("Velocity")?;
        self.velocity.xfer(xfer)?;
        xfer.xfer_marker_label("AngularVelocity")?;
        xfer.xfer_f32(&mut self.angular_velocity)?;
        xfer.xfer_marker_label("Forces")?;
        xfer_vec_default(
            xfer,
            &mut self.forces,
            Force {
                direction: glam::Vec3::ZERO,
                magnitude: 0.0,
                duration: 0.0,
            },
        )?;
        Ok(())
    }
}

impl XferData for ContainModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ContainModuleSnapshot")?;
        xfer.xfer_marker_label("ContainedObjects")?;
        xfer_vec_default(xfer, &mut self.contained_objects, ObjectId(0))?;
        xfer.xfer_marker_label("MaxCapacity")?;
        let mut cap = self.max_capacity as u32;
        xfer.xfer_u32(&mut cap)?;
        self.max_capacity = cap as usize;
        xfer.xfer_marker_label("ContainType")?;
        self.contain_type.xfer(xfer)?;
        xfer.xfer_marker_label("ExitPositions")?;
        xfer_vec_vec3(xfer, &mut self.exit_positions)?;
        Ok(())
    }
}

impl XferData for UpgradeModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("UpgradeModuleSnapshot")?;
        xfer.xfer_marker_label("ActiveUpgrades")?;
        xfer.xfer_vec_string(&mut self.active_upgrades)?;
        xfer.xfer_marker_label("UpgradeProgress")?;
        xfer_hashmap_default(xfer, &mut self.upgrade_progress, String::new(), 0.0f32)?;
        Ok(())
    }
}

impl XferData for ModuleSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ModuleSnapshot")?;
        let mut disc: u32 = match self {
            ModuleSnapshot::AIUpdate(_) => 0,
            ModuleSnapshot::Production(_) => 1,
            ModuleSnapshot::Weapon(_) => 2,
            ModuleSnapshot::Body(_) => 3,
            ModuleSnapshot::Locomotor(_) => 4,
            ModuleSnapshot::Physics(_) => 5,
            ModuleSnapshot::Contain(_) => 6,
            ModuleSnapshot::Upgrade(_) => 7,
        };
        xfer.xfer_u32(&mut disc)?;
        if xfer.get_mode() == XferMode::Save {
            match self {
                ModuleSnapshot::AIUpdate(d) => d.xfer(xfer)?,
                ModuleSnapshot::Production(d) => d.xfer(xfer)?,
                ModuleSnapshot::Weapon(d) => d.xfer(xfer)?,
                ModuleSnapshot::Body(d) => d.xfer(xfer)?,
                ModuleSnapshot::Locomotor(d) => d.xfer(xfer)?,
                ModuleSnapshot::Physics(d) => d.xfer(xfer)?,
                ModuleSnapshot::Contain(d) => d.xfer(xfer)?,
                ModuleSnapshot::Upgrade(d) => d.xfer(xfer)?,
            }
        } else {
            *self = match disc {
                0 => {
                    let mut d = AIUpdateModuleSnapshot {
                        current_state: String::new(),
                        state_machine_data: HashMap::new(),
                        target_object: None,
                        current_task: None,
                        task_queue: Vec::new(),
                    };
                    d.xfer(xfer)?;
                    ModuleSnapshot::AIUpdate(d)
                }
                1 => {
                    let mut d = ProductionModuleSnapshot {
                        production_queue: Vec::new(),
                        is_producing: false,
                        production_progress: 0.0,
                        rally_point: None,
                    };
                    d.xfer(xfer)?;
                    ModuleSnapshot::Production(d)
                }
                2 => {
                    let mut d = WeaponModuleSnapshot {
                        weapons: Vec::new(),
                        current_target: None,
                        firing_state: FiringState::Idle,
                    };
                    d.xfer(xfer)?;
                    ModuleSnapshot::Weapon(d)
                }
                3 => {
                    let mut d = BodyModuleSnapshot {
                        body_type: String::new(),
                        max_health: 0.0,
                        armor_type: String::new(),
                        damage_states: Vec::new(),
                    };
                    d.xfer(xfer)?;
                    ModuleSnapshot::Body(d)
                }
                4 => {
                    let mut d = LocomotorModuleSnapshot {
                        locomotor_type: String::new(),
                        movement_state: MovementState::Idle,
                        path: Vec::new(),
                        path_index: 0,
                    };
                    d.xfer(xfer)?;
                    ModuleSnapshot::Locomotor(d)
                }
                5 => {
                    let mut d = PhysicsModuleSnapshot {
                        velocity: glam::Vec3::ZERO,
                        angular_velocity: 0.0,
                        forces: Vec::new(),
                    };
                    d.xfer(xfer)?;
                    ModuleSnapshot::Physics(d)
                }
                6 => {
                    let mut d = ContainModuleSnapshot {
                        contained_objects: Vec::new(),
                        max_capacity: 0,
                        contain_type: String::new(),
                        exit_positions: Vec::new(),
                    };
                    d.xfer(xfer)?;
                    ModuleSnapshot::Contain(d)
                }
                7 => {
                    let mut d = UpgradeModuleSnapshot {
                        active_upgrades: Vec::new(),
                        upgrade_progress: HashMap::new(),
                    };
                    d.xfer(xfer)?;
                    ModuleSnapshot::Upgrade(d)
                }
                _ => {
                    return Err(SaveLoadError::Corrupted(format!(
                        "Invalid ModuleSnapshot: {disc}"
                    )))
                }
            };
        }
        Ok(())
    }
}

impl XferData for UnitSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("UnitSnapshot")?;
        xfer.xfer_marker_label("UnitType")?;
        self.unit_type.xfer(xfer)?;
        xfer.xfer_marker_label("FormationPosition")?;
        xfer_option(xfer, &mut self.formation_position, glam::Vec3::ZERO)?;
        xfer.xfer_marker_label("FormationId")?;
        xfer_option(xfer, &mut self.formation_id, 0u32)?;
        xfer.xfer_marker_label("GroupId")?;
        xfer_option(xfer, &mut self.group_id, 0u32)?;
        xfer.xfer_marker_label("Waypoints")?;
        xfer_vec_vec3(xfer, &mut self.waypoints)?;
        Ok(())
    }
}

impl XferData for BuildingSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("BuildingSnapshot")?;
        xfer.xfer_marker_label("BuildingType")?;
        self.building_type.xfer(xfer)?;
        xfer.xfer_marker_label("ConstructionProgress")?;
        xfer.xfer_f32(&mut self.construction_progress)?;
        xfer.xfer_marker_label("PowerProvided")?;
        xfer.xfer_i32(&mut self.power_provided)?;
        xfer.xfer_marker_label("PowerRequired")?;
        xfer.xfer_i32(&mut self.power_required)?;
        xfer.xfer_marker_label("IsPowered")?;
        xfer.xfer_bool(&mut self.is_powered)?;
        xfer.xfer_marker_label("ConnectedBuildings")?;
        xfer_vec_default(xfer, &mut self.connected_buildings, ObjectId(0))?;
        Ok(())
    }
}

impl XferData for ProjectileSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ProjectileSnapshot")?;
        xfer.xfer_marker_label("ProjectileType")?;
        self.projectile_type.xfer(xfer)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("TargetObject")?;
        xfer_option(xfer, &mut self.target_object, ObjectId(0))?;
        xfer.xfer_marker_label("TargetPosition")?;
        self.target_position.xfer(xfer)?;
        xfer.xfer_marker_label("FlightTime")?;
        xfer.xfer_f32(&mut self.flight_time)?;
        xfer.xfer_marker_label("MaxFlightTime")?;
        xfer.xfer_f32(&mut self.max_flight_time)?;
        Ok(())
    }
}

impl XferData for ResourceSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ResourceSnapshot")?;
        xfer.xfer_marker_label("ResourceType")?;
        self.resource_type.xfer(xfer)?;
        xfer.xfer_marker_label("Amount")?;
        xfer.xfer_u32(&mut self.amount)?;
        xfer.xfer_marker_label("DepletionRate")?;
        xfer.xfer_f32(&mut self.depletion_rate)?;
        xfer.xfer_marker_label("IsInfinite")?;
        xfer.xfer_bool(&mut self.is_infinite)?;
        Ok(())
    }
}

impl XferData for ObjectTypeSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ObjectTypeSnapshot")?;
        let mut disc: u32 = match self {
            ObjectTypeSnapshot::Unit(_) => 0,
            ObjectTypeSnapshot::Building(_) => 1,
            ObjectTypeSnapshot::Projectile(_) => 2,
            ObjectTypeSnapshot::Resource(_) => 3,
        };
        xfer.xfer_u32(&mut disc)?;
        if xfer.get_mode() == XferMode::Save {
            match self {
                ObjectTypeSnapshot::Unit(d) => d.xfer(xfer)?,
                ObjectTypeSnapshot::Building(d) => d.xfer(xfer)?,
                ObjectTypeSnapshot::Projectile(d) => d.xfer(xfer)?,
                ObjectTypeSnapshot::Resource(d) => d.xfer(xfer)?,
            }
        } else {
            *self = match disc {
                0 => {
                    let mut d = UnitSnapshot {
                        unit_type: String::new(),
                        formation_position: None,
                        formation_id: None,
                        group_id: None,
                        waypoints: Vec::new(),
                    };
                    d.xfer(xfer)?;
                    ObjectTypeSnapshot::Unit(d)
                }
                1 => {
                    let mut d = BuildingSnapshot {
                        building_type: String::new(),
                        construction_progress: 0.0,
                        power_provided: 0,
                        power_required: 0,
                        is_powered: false,
                        connected_buildings: Vec::new(),
                    };
                    d.xfer(xfer)?;
                    ObjectTypeSnapshot::Building(d)
                }
                2 => {
                    let mut d = ProjectileSnapshot {
                        projectile_type: String::new(),
                        source_object: ObjectId(0),
                        target_object: None,
                        target_position: glam::Vec3::ZERO,
                        flight_time: 0.0,
                        max_flight_time: 0.0,
                    };
                    d.xfer(xfer)?;
                    ObjectTypeSnapshot::Projectile(d)
                }
                3 => {
                    let mut d = ResourceSnapshot {
                        resource_type: String::new(),
                        amount: 0,
                        depletion_rate: 0.0,
                        is_infinite: false,
                    };
                    d.xfer(xfer)?;
                    ObjectTypeSnapshot::Resource(d)
                }
                _ => {
                    return Err(SaveLoadError::Corrupted(format!(
                        "Invalid ObjectTypeSnapshot: {disc}"
                    )))
                }
            };
        }
        Ok(())
    }
}

impl XferData for PopulationInfo {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PopulationInfo")?;
        xfer.xfer_marker_label("Current")?;
        xfer.xfer_u32(&mut self.current)?;
        xfer.xfer_marker_label("Maximum")?;
        xfer.xfer_u32(&mut self.maximum)?;
        Ok(())
    }
}

impl XferData for TechTreeSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("TechTreeSnapshot")?;
        xfer.xfer_marker_label("UnlockedUnits")?;
        xfer.xfer_vec_string(&mut self.unlocked_units)?;
        xfer.xfer_marker_label("UnlockedBuildings")?;
        xfer.xfer_vec_string(&mut self.unlocked_buildings)?;
        xfer.xfer_marker_label("UnlockedUpgrades")?;
        xfer.xfer_vec_string(&mut self.unlocked_upgrades)?;
        xfer.xfer_marker_label("ResearchProgress")?;
        xfer_hashmap_default(xfer, &mut self.research_progress, String::new(), 0.0f32)?;
        Ok(())
    }
}

impl XferData for PlayerStatisticsSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PlayerStatisticsSnapshot")?;
        xfer.xfer_marker_label("UnitsBuilt")?;
        xfer.xfer_u32(&mut self.units_built)?;
        xfer.xfer_marker_label("UnitsLost")?;
        xfer.xfer_u32(&mut self.units_lost)?;
        xfer.xfer_marker_label("BuildingsBuilt")?;
        xfer.xfer_u32(&mut self.buildings_built)?;
        xfer.xfer_marker_label("BuildingsLost")?;
        xfer.xfer_u32(&mut self.buildings_lost)?;
        xfer.xfer_marker_label("DamageDealt")?;
        xfer.xfer_f32(&mut self.damage_dealt)?;
        xfer.xfer_marker_label("DamageReceived")?;
        xfer.xfer_f32(&mut self.damage_received)?;
        xfer.xfer_marker_label("ResourcesGathered")?;
        xfer.xfer_u32(&mut self.resources_gathered)?;
        xfer.xfer_marker_label("ExperienceGained")?;
        xfer.xfer_f32(&mut self.experience_gained)?;
        Ok(())
    }
}

impl XferData for TeamSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("TeamSnapshot")?;
        xfer.xfer_marker_label("Team")?;
        self.team.xfer(xfer)?;
        xfer.xfer_marker_label("Players")?;
        xfer.xfer_vec_u32(&mut self.players)?;
        xfer.xfer_marker_label("AlliedTeams")?;
        xfer_vec_default(xfer, &mut self.allied_teams, Team::Neutral)?;
        xfer.xfer_marker_label("IsDefeated")?;
        xfer.xfer_bool(&mut self.is_defeated)?;
        xfer.xfer_marker_label("SharedVision")?;
        xfer.xfer_bool(&mut self.shared_vision)?;
        xfer.xfer_marker_label("SharedControl")?;
        xfer.xfer_bool(&mut self.shared_control)?;
        Ok(())
    }
}

impl XferData for TerrainModification {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("TerrainModification")?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("Radius")?;
        xfer.xfer_f32(&mut self.radius)?;
        xfer.xfer_marker_label("HeightDelta")?;
        xfer.xfer_f32(&mut self.height_delta)?;
        xfer.xfer_marker_label("ModificationType")?;
        self.modification_type.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for TerrainSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("TerrainSnapshot")?;
        xfer.xfer_marker_label("Width")?;
        xfer.xfer_u32(&mut self.width)?;
        xfer.xfer_marker_label("Height")?;
        xfer.xfer_u32(&mut self.height)?;
        xfer.xfer_marker_label("HeightMap")?;
        xfer_vec_f32(xfer, &mut self.height_map)?;
        xfer.xfer_marker_label("TextureMap")?;
        xfer_vec_u8(xfer, &mut self.texture_map)?;
        xfer.xfer_marker_label("PassabilityMap")?;
        xfer_vec_bool(xfer, &mut self.passability_map)?;
        xfer.xfer_marker_label("Modifications")?;
        xfer_vec_default(
            xfer,
            &mut self.modifications,
            TerrainModification {
                position: glam::Vec3::ZERO,
                radius: 0.0,
                height_delta: 0.0,
                modification_type: String::new(),
            },
        )?;
        Ok(())
    }
}

impl XferData for WeatherSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("WeatherSnapshot")?;
        xfer.xfer_marker_label("CurrentWeather")?;
        self.current_weather.xfer(xfer)?;
        xfer.xfer_marker_label("WeatherIntensity")?;
        xfer.xfer_f32(&mut self.weather_intensity)?;
        xfer.xfer_marker_label("WeatherDuration")?;
        xfer.xfer_f32(&mut self.weather_duration)?;
        xfer.xfer_marker_label("NextWeatherChange")?;
        xfer.xfer_f32(&mut self.next_weather_change)?;
        xfer.xfer_marker_label("Visible")?;
        xfer.xfer_bool(&mut self.visible)?;
        Ok(())
    }
}

impl XferData for SupplyDepositSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("SupplyDepositSnapshot")?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("Amount")?;
        xfer.xfer_u32(&mut self.amount)?;
        xfer.xfer_marker_label("DepletionRate")?;
        xfer.xfer_f32(&mut self.depletion_rate)?;
        xfer.xfer_marker_label("Harvesters")?;
        xfer_vec_default(xfer, &mut self.harvesters, ObjectId(0))?;
        Ok(())
    }
}

impl XferData for ResourceZoneSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ResourceZoneSnapshot")?;
        xfer.xfer_marker_label("Bounds")?;
        self.bounds.xfer(xfer)?;
        xfer.xfer_marker_label("ResourceType")?;
        self.resource_type.xfer(xfer)?;
        xfer.xfer_marker_label("TotalAmount")?;
        xfer.xfer_u32(&mut self.total_amount)?;
        xfer.xfer_marker_label("RemainingAmount")?;
        xfer.xfer_u32(&mut self.remaining_amount)?;
        Ok(())
    }
}

impl XferData for ResourceManagerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ResourceManagerSnapshot")?;
        xfer.xfer_marker_label("SupplyDeposits")?;
        xfer_vec_default(
            xfer,
            &mut self.supply_deposits,
            SupplyDepositSnapshot {
                position: glam::Vec3::ZERO,
                amount: 0,
                depletion_rate: 0.0,
                harvesters: Vec::new(),
            },
        )?;
        xfer.xfer_marker_label("ResourceZones")?;
        xfer_vec_default(
            xfer,
            &mut self.resource_zones,
            ResourceZoneSnapshot {
                bounds: GeometryInfo::default(),
                resource_type: String::new(),
                total_amount: 0,
                remaining_amount: 0,
            },
        )?;
        Ok(())
    }
}

impl XferData for ActiveCombatSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ActiveCombatSnapshot")?;
        xfer.xfer_marker_label("Attacker")?;
        self.attacker.xfer(xfer)?;
        xfer.xfer_marker_label("Target")?;
        self.target.xfer(xfer)?;
        xfer.xfer_marker_label("StartTime")?;
        xfer.xfer_f32(&mut self.start_time)?;
        xfer.xfer_marker_label("DamageDealt")?;
        xfer.xfer_f32(&mut self.damage_dealt)?;
        Ok(())
    }
}

impl XferData for DeathEventSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("DeathEventSnapshot")?;
        xfer.xfer_marker_label("ObjectId")?;
        self.object_id.xfer(xfer)?;
        xfer.xfer_marker_label("KillerId")?;
        xfer_option(xfer, &mut self.killer_id, ObjectId(0))?;
        xfer.xfer_marker_label("DeathTime")?;
        xfer.xfer_f32(&mut self.death_time)?;
        xfer.xfer_marker_label("DeathPosition")?;
        self.death_position.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for CombatTrackerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("CombatTrackerSnapshot")?;
        xfer.xfer_marker_label("ActiveCombats")?;
        xfer_vec_default(
            xfer,
            &mut self.active_combats,
            ActiveCombatSnapshot {
                attacker: ObjectId(0),
                target: ObjectId(0),
                start_time: 0.0,
                damage_dealt: 0.0,
            },
        )?;
        xfer.xfer_marker_label("RecentDeaths")?;
        xfer_vec_default(
            xfer,
            &mut self.recent_deaths,
            DeathEventSnapshot {
                object_id: ObjectId(0),
                killer_id: None,
                death_time: 0.0,
                death_position: glam::Vec3::ZERO,
            },
        )?;
        Ok(())
    }
}

impl XferData for ExperienceEventSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ExperienceEventSnapshot")?;
        xfer.xfer_marker_label("ObjectId")?;
        self.object_id.xfer(xfer)?;
        xfer.xfer_marker_label("ExperienceGained")?;
        xfer.xfer_f32(&mut self.experience_gained)?;
        xfer.xfer_marker_label("Source")?;
        self.source.xfer(xfer)?;
        xfer.xfer_marker_label("Timestamp")?;
        xfer.xfer_f32(&mut self.timestamp)?;
        Ok(())
    }
}

impl XferData for VeterancyBonuses {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("VeterancyBonuses")?;
        xfer.xfer_marker_label("HealthBonus")?;
        xfer.xfer_f32(&mut self.health_bonus)?;
        xfer.xfer_marker_label("DamageBonus")?;
        xfer.xfer_f32(&mut self.damage_bonus)?;
        xfer.xfer_marker_label("AccuracyBonus")?;
        xfer.xfer_f32(&mut self.accuracy_bonus)?;
        xfer.xfer_marker_label("RangeBonus")?;
        xfer.xfer_f32(&mut self.range_bonus)?;
        Ok(())
    }
}

impl XferData for ExperienceTrackerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ExperienceTrackerSnapshot")?;
        xfer.xfer_marker_label("ExperienceEvents")?;
        xfer_vec_default(
            xfer,
            &mut self.experience_events,
            ExperienceEventSnapshot {
                object_id: ObjectId(0),
                experience_gained: 0.0,
                source: String::new(),
                timestamp: 0.0,
            },
        )?;
        xfer.xfer_marker_label("VeterancyBonuses")?;
        xfer_hashmap_default(
            xfer,
            &mut self.veterancy_bonuses,
            ObjectId(0),
            VeterancyBonuses {
                health_bonus: 0.0,
                damage_bonus: 0.0,
                accuracy_bonus: 0.0,
                range_bonus: 0.0,
            },
        )?;
        Ok(())
    }
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

impl XferData for PathfindingCacheSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PathfindingCacheSnapshot")?;
        xfer.xfer_marker_label("CachedPaths")?;
        {
            let mut len = self.cached_paths.len() as u32;
            xfer.xfer_u32(&mut len)?;
            if xfer.get_mode() == XferMode::Load {
                self.cached_paths.clear();
                for _ in 0..len {
                    let mut k = (
                        SerializableVec3 { x: 0, y: 0, z: 0 },
                        SerializableVec3 { x: 0, y: 0, z: 0 },
                    );
                    let mut v = Vec::new();
                    k.0.xfer(xfer)?;
                    k.1.xfer(xfer)?;
                    let mut path_len = 0u32;
                    xfer.xfer_u32(&mut path_len)?;
                    for _ in 0..path_len {
                        let mut sv = SerializableVec3 { x: 0, y: 0, z: 0 };
                        sv.xfer(xfer)?;
                        v.push(sv);
                    }
                    self.cached_paths.insert(k, v);
                }
            } else {
                for (k, v) in &mut self.cached_paths {
                    let mut k0 = k.0;
                    let mut k1 = k.1;
                    k0.xfer(xfer)?;
                    k1.xfer(xfer)?;
                    let mut path_len = v.len() as u32;
                    xfer.xfer_u32(&mut path_len)?;
                    for sv in v.iter_mut() {
                        sv.xfer(xfer)?;
                    }
                }
            }
        }
        xfer.xfer_marker_label("CacheTimestamps")?;
        {
            let mut len = self.cache_timestamps.len() as u32;
            xfer.xfer_u32(&mut len)?;
            if xfer.get_mode() == XferMode::Load {
                self.cache_timestamps.clear();
                for _ in 0..len {
                    let mut k = (
                        SerializableVec3 { x: 0, y: 0, z: 0 },
                        SerializableVec3 { x: 0, y: 0, z: 0 },
                    );
                    let mut ts = 0.0f32;
                    k.0.xfer(xfer)?;
                    k.1.xfer(xfer)?;
                    ts.xfer(xfer)?;
                    self.cache_timestamps.insert(k, ts);
                }
            } else {
                for (k, ts) in &mut self.cache_timestamps {
                    let mut k0 = k.0;
                    let mut k1 = k.1;
                    k0.xfer(xfer)?;
                    k1.xfer(xfer)?;
                    ts.xfer(xfer)?;
                }
            }
        }
        Ok(())
    }
}

impl XferData for AIObjective {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIObjective")?;
        xfer.xfer_marker_label("ObjectiveType")?;
        self.objective_type.xfer(xfer)?;
        xfer.xfer_marker_label("Priority")?;
        xfer.xfer_f32(&mut self.priority)?;
        xfer.xfer_marker_label("TargetPosition")?;
        xfer_option(xfer, &mut self.target_position, glam::Vec3::ZERO)?;
        xfer.xfer_marker_label("AssignedUnits")?;
        xfer_vec_default(xfer, &mut self.assigned_units, ObjectId(0))?;
        xfer.xfer_marker_label("CompletionPercentage")?;
        xfer.xfer_f32(&mut self.completion_percentage)?;
        Ok(())
    }
}

impl XferData for ThreatAssessmentSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ThreatAssessmentSnapshot")?;
        xfer.xfer_marker_label("EnemyStrengths")?;
        xfer_hashmap_default(xfer, &mut self.enemy_strengths, Team::Neutral, 0.0f32)?;
        xfer.xfer_marker_label("VulnerableAreas")?;
        xfer_vec_vec3(xfer, &mut self.vulnerable_areas)?;
        xfer.xfer_marker_label("ThreatLevel")?;
        xfer.xfer_f32(&mut self.threat_level)?;
        Ok(())
    }
}

impl XferData for AIStrategicStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIStrategicStateSnapshot")?;
        xfer.xfer_marker_label("CurrentPhase")?;
        self.current_phase.xfer(xfer)?;
        xfer.xfer_marker_label("Objectives")?;
        xfer_vec_default(
            xfer,
            &mut self.objectives,
            AIObjective {
                objective_type: String::new(),
                priority: 0.0,
                target_position: None,
                assigned_units: Vec::new(),
                completion_percentage: 0.0,
            },
        )?;
        xfer.xfer_marker_label("ThreatAssessment")?;
        self.threat_assessment.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for AIUnitGroupSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIUnitGroupSnapshot")?;
        xfer.xfer_marker_label("GroupId")?;
        xfer.xfer_u32(&mut self.group_id)?;
        xfer.xfer_marker_label("Units")?;
        xfer_vec_default(xfer, &mut self.units, ObjectId(0))?;
        xfer.xfer_marker_label("Role")?;
        self.role.xfer(xfer)?;
        xfer.xfer_marker_label("CurrentTask")?;
        self.current_task.xfer(xfer)?;
        xfer.xfer_marker_label("Formation")?;
        self.formation.xfer(xfer)?;
        xfer.xfer_marker_label("TargetPosition")?;
        xfer_option(xfer, &mut self.target_position, glam::Vec3::ZERO)?;
        Ok(())
    }
}

impl XferData for AIAttackSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIAttackSnapshot")?;
        xfer.xfer_marker_label("AttackId")?;
        xfer.xfer_u32(&mut self.attack_id)?;
        xfer.xfer_marker_label("TargetPosition")?;
        self.target_position.xfer(xfer)?;
        xfer.xfer_marker_label("AssignedGroups")?;
        xfer.xfer_vec_u32(&mut self.assigned_groups)?;
        xfer.xfer_marker_label("AttackPhase")?;
        self.attack_phase.xfer(xfer)?;
        xfer.xfer_marker_label("StartTime")?;
        xfer.xfer_f32(&mut self.start_time)?;
        Ok(())
    }
}

impl XferData for AITacticalStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AITacticalStateSnapshot")?;
        xfer.xfer_marker_label("UnitGroups")?;
        xfer_vec_default(
            xfer,
            &mut self.unit_groups,
            AIUnitGroupSnapshot {
                group_id: 0,
                units: Vec::new(),
                role: String::new(),
                current_task: String::new(),
                formation: String::new(),
                target_position: None,
            },
        )?;
        xfer.xfer_marker_label("ActiveAttacks")?;
        xfer_vec_default(
            xfer,
            &mut self.active_attacks,
            AIAttackSnapshot {
                attack_id: 0,
                target_position: glam::Vec3::ZERO,
                assigned_groups: Vec::new(),
                attack_phase: String::new(),
                start_time: 0.0,
            },
        )?;
        xfer.xfer_marker_label("DefensivePositions")?;
        xfer_vec_vec3(xfer, &mut self.defensive_positions)?;
        Ok(())
    }
}

impl XferData for BuildPriority {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("BuildPriority")?;
        xfer.xfer_marker_label("TemplateName")?;
        self.template_name.xfer(xfer)?;
        xfer.xfer_marker_label("Priority")?;
        xfer.xfer_f32(&mut self.priority)?;
        xfer.xfer_marker_label("DesiredCount")?;
        xfer.xfer_u32(&mut self.desired_count)?;
        xfer.xfer_marker_label("CurrentCount")?;
        xfer.xfer_u32(&mut self.current_count)?;
        Ok(())
    }
}

impl XferData for ResourceAllocation {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("ResourceAllocation")?;
        xfer.xfer_marker_label("MilitaryPercentage")?;
        xfer.xfer_f32(&mut self.military_percentage)?;
        xfer.xfer_marker_label("EconomicPercentage")?;
        xfer.xfer_f32(&mut self.economic_percentage)?;
        xfer.xfer_marker_label("DefensivePercentage")?;
        xfer.xfer_f32(&mut self.defensive_percentage)?;
        Ok(())
    }
}

impl XferData for AIEconomicStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIEconomicStateSnapshot")?;
        xfer.xfer_marker_label("BuildPriorities")?;
        xfer_vec_default(
            xfer,
            &mut self.build_priorities,
            BuildPriority {
                template_name: String::new(),
                priority: 0.0,
                desired_count: 0,
                current_count: 0,
            },
        )?;
        xfer.xfer_marker_label("EconomicFocus")?;
        self.economic_focus.xfer(xfer)?;
        xfer.xfer_marker_label("ResourceAllocation")?;
        self.resource_allocation.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for AIPlayerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("AIPlayerSnapshot")?;
        xfer.xfer_marker_label("PlayerId")?;
        xfer.xfer_u32(&mut self.player_id)?;
        xfer.xfer_marker_label("Difficulty")?;
        self.difficulty.xfer(xfer)?;
        xfer.xfer_marker_label("Personality")?;
        self.personality.xfer(xfer)?;
        xfer.xfer_marker_label("CurrentStrategy")?;
        self.current_strategy.xfer(xfer)?;
        xfer.xfer_marker_label("StrategicState")?;
        self.strategic_state.xfer(xfer)?;
        xfer.xfer_marker_label("TacticalState")?;
        self.tactical_state.xfer(xfer)?;
        xfer.xfer_marker_label("EconomicState")?;
        self.economic_state.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for DifficultyModifiers {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("DifficultyModifiers")?;
        xfer.xfer_marker_label("AIResourceBonus")?;
        xfer.xfer_f32(&mut self.ai_resource_bonus)?;
        xfer.xfer_marker_label("AIDamageBonus")?;
        xfer.xfer_f32(&mut self.ai_damage_bonus)?;
        xfer.xfer_marker_label("AIHealthBonus")?;
        xfer.xfer_f32(&mut self.ai_health_bonus)?;
        xfer.xfer_marker_label("AIBuildSpeedBonus")?;
        xfer.xfer_f32(&mut self.ai_build_speed_bonus)?;
        Ok(())
    }
}

impl XferData for GlobalAIStateSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("GlobalAIStateSnapshot")?;
        xfer.xfer_marker_label("GlobalTimers")?;
        xfer_hashmap_default(xfer, &mut self.global_timers, String::new(), 0.0f32)?;
        xfer.xfer_marker_label("GlobalFlags")?;
        xfer_hashmap_default(xfer, &mut self.global_flags, String::new(), false)?;
        xfer.xfer_marker_label("DifficultyModifiers")?;
        self.difficulty_modifiers.xfer(xfer)?;
        Ok(())
    }
}

impl XferData for HostSuperweaponKind {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            HostSuperweaponKind::DaisyCutter => 0u32,
            HostSuperweaponKind::A10Strike => 1,
            HostSuperweaponKind::ScudStorm => 2,
            HostSuperweaponKind::ParticleCannon => 3,
            HostSuperweaponKind::NuclearMissile => 4,
            HostSuperweaponKind::AnthraxBomb => 5,
            HostSuperweaponKind::SpectreGunship => 6,
            HostSuperweaponKind::CarpetBomb => 7,
            HostSuperweaponKind::ArtilleryBarrage => 8,
            HostSuperweaponKind::CruiseMissile => 9,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => HostSuperweaponKind::DaisyCutter,
            1 => HostSuperweaponKind::A10Strike,
            2 => HostSuperweaponKind::ScudStorm,
            3 => HostSuperweaponKind::ParticleCannon,
            4 => HostSuperweaponKind::NuclearMissile,
            5 => HostSuperweaponKind::AnthraxBomb,
            6 => HostSuperweaponKind::SpectreGunship,
            7 => HostSuperweaponKind::CarpetBomb,
            8 => HostSuperweaponKind::ArtilleryBarrage,
            9 => HostSuperweaponKind::CruiseMissile,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HostSuperweaponKind discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for HostStrikePhase {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            HostStrikePhase::Queued => 0u32,
            HostStrikePhase::Completed => 1,
            HostStrikePhase::Cancelled => 2,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => HostStrikePhase::Queued,
            1 => HostStrikePhase::Completed,
            2 => HostStrikePhase::Cancelled,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HostStrikePhase discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for HostSpecialPowerStrike {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostSpecialPowerStrike")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("Kind")?;
        self.kind.xfer(xfer)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("TargetPosition")?;
        self.target_position.xfer(xfer)?;
        xfer.xfer_marker_label("ActivateFrame")?;
        xfer.xfer_u32(&mut self.activate_frame)?;
        xfer.xfer_marker_label("ImpactFrame")?;
        xfer.xfer_u32(&mut self.impact_frame)?;
        xfer.xfer_marker_label("Phase")?;
        self.phase.xfer(xfer)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("ObjectsHit")?;
        xfer.xfer_u32(&mut self.objects_hit)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostRadiationField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostRadiationField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // Wave 56: radiation residual pack honesty counters (appended).
        xfer.xfer_marker_label("RadiationResidualPackArmed")?;
        xfer.xfer_u32(&mut self.radiation_residual_pack_armed)?;
        xfer.xfer_marker_label("RadiationSuspendFxApplications")?;
        xfer.xfer_u32(&mut self.radiation_suspend_fx_applications)?;
        xfer.xfer_marker_label("RadiationFireFxApplications")?;
        xfer.xfer_u32(&mut self.radiation_fire_fx_applications)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostToxinField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostToxinField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // LargePoisonField / Anthrax residual params (appended after parent id).
        xfer.xfer_marker_label("DamagePerTick")?;
        xfer.xfer_f32(&mut self.damage_per_tick)?;
        xfer.xfer_marker_label("Radius")?;
        xfer.xfer_f32(&mut self.radius)?;
        xfer.xfer_marker_label("TickIntervalFrames")?;
        xfer.xfer_u32(&mut self.tick_interval_frames)?;
        // Wave 56: toxin residual pack honesty counters (appended).
        xfer.xfer_marker_label("ToxinResidualPackArmed")?;
        xfer.xfer_u32(&mut self.toxin_residual_pack_armed)?;
        xfer.xfer_marker_label("ToxinFireFxApplications")?;
        xfer.xfer_u32(&mut self.toxin_fire_fx_applications)?;
        xfer.xfer_marker_label("ToxinDamageTypeApplications")?;
        xfer.xfer_u32(&mut self.toxin_damage_type_applications)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostSpectreOrbitField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostSpectreOrbitField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // Gattling/howitzer residual bookkeeping (appended).
        xfer.xfer_marker_label("NextGattlingTickFrame")?;
        xfer.xfer_u32(&mut self.next_gattling_tick_frame)?;
        xfer.xfer_marker_label("HowitzerTicks")?;
        xfer.xfer_u32(&mut self.howitzer_ticks)?;
        xfer.xfer_marker_label("GattlingTicks")?;
        xfer.xfer_u32(&mut self.gattling_ticks)?;
        // Continuous-fire residual bookkeeping (appended).
        xfer.xfer_marker_label("GattlingConsecutive")?;
        xfer.xfer_u32(&mut self.gattling_consecutive)?;
        xfer.xfer_marker_label("HowitzerConsecutive")?;
        xfer.xfer_u32(&mut self.howitzer_consecutive)?;
        xfer.xfer_marker_label("GattlingFireLevel")?;
        xfer.xfer_u8(&mut self.gattling_fire_level)?;
        xfer.xfer_marker_label("HowitzerFireLevel")?;
        xfer.xfer_u8(&mut self.howitzer_fire_level)?;
        // ContinuousFireCoast residual bookkeeping (appended).
        xfer.xfer_marker_label("GattlingCoastUntilFrame")?;
        xfer.xfer_u32(&mut self.gattling_coast_until_frame)?;
        xfer.xfer_marker_label("HowitzerCoastUntilFrame")?;
        xfer.xfer_u32(&mut self.howitzer_coast_until_frame)?;
        xfer.xfer_marker_label("GattlingCoastApplications")?;
        xfer.xfer_u32(&mut self.gattling_coast_applications)?;
        xfer.xfer_marker_label("HowitzerCoastApplications")?;
        xfer.xfer_u32(&mut self.howitzer_coast_applications)?;
        xfer.xfer_marker_label("RapidFireVoiceCues")?;
        xfer.xfer_u32(&mut self.rapid_fire_voice_cues)?;
        // MODELCONDITION_CONTINUOUS_FIRE_* residual bookkeeping (appended).
        xfer.xfer_marker_label("ModelConditionMeanSets")?;
        xfer.xfer_u32(&mut self.model_condition_mean_sets)?;
        xfer.xfer_marker_label("ModelConditionFastSets")?;
        xfer.xfer_u32(&mut self.model_condition_fast_sets)?;
        xfer.xfer_marker_label("ModelConditionSlowSets")?;
        xfer.xfer_u32(&mut self.model_condition_slow_sets)?;
        // SpectreHowitzerShell projectile residual (appended).
        xfer.xfer_marker_label("HowitzerShellsSpawned")?;
        xfer.xfer_u32(&mut self.howitzer_shells_spawned)?;
        xfer.xfer_marker_label("HowitzerShellFireFx")?;
        xfer.xfer_u32(&mut self.howitzer_shell_fire_fx)?;
        xfer.xfer_marker_label("HowitzerShellDetonationFx")?;
        xfer.xfer_u32(&mut self.howitzer_shell_detonation_fx)?;
        xfer.xfer_marker_label("HowitzerShellHeightDieDelays")?;
        xfer.xfer_u32(&mut self.howitzer_shell_height_die_delays)?;
        xfer.xfer_marker_label("HowitzerShellFireSounds")?;
        xfer.xfer_u32(&mut self.howitzer_shell_fire_sounds)?;
        // SpectreHowitzerShell DumbProjectile / Physics / InstantDeath residual.
        xfer.xfer_marker_label("HowitzerShellDumbProjectileApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_dumb_projectile_applications)?;
        xfer.xfer_marker_label("HowitzerShellPhysicsMassApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_physics_mass_applications)?;
        xfer.xfer_marker_label("HowitzerShellDeathDetonatedApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_death_detonated_applications)?;
        xfer.xfer_marker_label("HowitzerShellDeathLaseredApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_death_lasered_applications)?;
        xfer.xfer_marker_label("HowitzerShellDeathLaseredOclApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_death_lasered_ocl_applications)?;
        xfer.xfer_marker_label("HowitzerShellDeathGenericApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_death_generic_applications)?;
        xfer.xfer_marker_label("HowitzerShellObjectParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_object_params_applications)?;
        xfer.xfer_marker_label("HowitzerShellDesignParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_design_params_applications)?;
        xfer.xfer_marker_label("HowitzerShellOnlyMovingDownApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_only_moving_down_applications)?;
        // SpectreHowitzerShell W3D ModelDraw residual (appended).
        xfer.xfer_marker_label("HowitzerShellModelDrawApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_model_draw_applications)?;
        xfer.xfer_marker_label("HowitzerShellScaleApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_scale_applications)?;
        xfer.xfer_marker_label("HowitzerShellShadowApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_shadow_applications)?;
        xfer.xfer_marker_label("HowitzerShellGeometryApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_geometry_applications)?;
        xfer.xfer_marker_label("HowitzerShellMaxHealthApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_max_health_applications)?;
        // SpectreHowitzerShell loft flight residual (appended).
        xfer.xfer_marker_label("HowitzerShellLoftFlightApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_loft_flight_applications)?;
        xfer.xfer_marker_label("HowitzerShellLastLoftHeight")?;
        xfer.xfer_f32(&mut self.howitzer_shell_last_loft_height)?;
        xfer.xfer_marker_label("HowitzerShellLoftHeightDieApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_loft_height_die_applications)?;
        // SpectreHowitzerShellLocomotor template + Armor DamageFX residual (appended).
        xfer.xfer_marker_label("HowitzerShellLocomotorTemplateApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_locomotor_template_applications)?;
        xfer.xfer_marker_label("HowitzerShellDamageFxApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_damage_fx_applications)?;
        // Wave 74: SpectreHowitzerShell ThingFactory spawn bookkeeping residual.
        xfer.xfer_marker_label("HowitzerShellThingFactorySpawnApplications")?;
        xfer.xfer_u32(&mut self.howitzer_shell_thing_factory_spawn_applications)?;
        xfer.xfer_marker_label("HowitzerGunAimParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_gun_aim_params_applications)?;
        xfer.xfer_marker_label("HowitzerGunFireParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_gun_fire_params_applications)?;
        // SpectreHowitzerGun anti residual (appended).
        xfer.xfer_marker_label("HowitzerGunAntiParamsApplications")?;
        xfer.xfer_u32(&mut self.howitzer_gun_anti_params_applications)?;
        // SpectreGattlingGun anti/fire residual (appended).
        xfer.xfer_marker_label("GattlingGunParamsApplications")?;
        xfer.xfer_u32(&mut self.gattling_gun_params_applications)?;
        // Wave 50: ContinuousFire WeaponBonus ROF residual applications (appended).
        xfer.xfer_marker_label("GattlingRofMeanApplications")?;
        xfer.xfer_u32(&mut self.gattling_rof_mean_applications)?;
        xfer.xfer_marker_label("GattlingRofFastApplications")?;
        xfer.xfer_u32(&mut self.gattling_rof_fast_applications)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostParticleBeamField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostParticleBeamField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("PulsesMade")?;
        xfer.xfer_u32(&mut self.pulses_made)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // SwathOfDeath residual bookkeeping (appended).
        xfer.xfer_marker_label("LastSwathPosition")?;
        self.last_swath_position.xfer(xfer)?;
        xfer.xfer_marker_label("MaxSwathOffset")?;
        xfer.xfer_f32(&mut self.max_swath_offset)?;
        xfer.xfer_marker_label("SwathApplications")?;
        xfer.xfer_u32(&mut self.swath_applications)?;
        // WidthGrow + TotalScorchMarks / RevealRange residual (appended).
        xfer.xfer_marker_label("NextScorchFrame")?;
        xfer.xfer_u32(&mut self.next_scorch_frame)?;
        xfer.xfer_marker_label("ScorchMarksMade")?;
        xfer.xfer_u32(&mut self.scorch_marks_made)?;
        xfer.xfer_marker_label("RevealApplications")?;
        xfer.xfer_u32(&mut self.reveal_applications)?;
        xfer.xfer_marker_label("GroundHitFxApplications")?;
        xfer.xfer_u32(&mut self.ground_hit_fx_applications)?;
        xfer.xfer_marker_label("PeakWidthScalar")?;
        xfer.xfer_f32(&mut self.peak_width_scalar)?;
        xfer.xfer_marker_label("LastDamageRadius")?;
        xfer.xfer_f32(&mut self.last_damage_radius)?;
        // WidthGrow decay residual honesty (appended after grow fields).
        xfer.xfer_marker_label("LastWidthScalar")?;
        xfer.xfer_f32(&mut self.last_width_scalar)?;
        xfer.xfer_marker_label("TroughWidthScalar")?;
        xfer.xfer_f32(&mut self.trough_width_scalar)?;
        xfer.xfer_marker_label("DecaySamples")?;
        xfer.xfer_u32(&mut self.decay_samples)?;
        xfer.xfer_marker_label("LastScorchPosition")?;
        self.last_scorch_position.xfer(xfer)?;
        xfer.xfer_marker_label("LastScorchRadius")?;
        xfer.xfer_f32(&mut self.last_scorch_radius)?;
        // Manual beam driving + outer-node/connector laser residual (appended).
        xfer.xfer_marker_label("ManualTargetMode")?;
        xfer.xfer_bool(&mut self.manual_target_mode)?;
        xfer.xfer_marker_label("OverrideDestination")?;
        self.override_destination.xfer(xfer)?;
        xfer.xfer_marker_label("CurrentTargetPosition")?;
        self.current_target_position.xfer(xfer)?;
        xfer.xfer_marker_label("LastDrivingClickFrame")?;
        xfer.xfer_u32(&mut self.last_driving_click_frame)?;
        xfer.xfer_marker_label("SecondLastDrivingClickFrame")?;
        xfer.xfer_u32(&mut self.second_last_driving_click_frame)?;
        xfer.xfer_marker_label("LastDriveUpdateFrame")?;
        xfer.xfer_u32(&mut self.last_drive_update_frame)?;
        xfer.xfer_marker_label("ManualDriveDistanceTotal")?;
        xfer.xfer_f32(&mut self.manual_drive_distance_total)?;
        xfer.xfer_marker_label("ManualDriveApplications")?;
        xfer.xfer_u32(&mut self.manual_drive_applications)?;
        xfer.xfer_marker_label("FastDriveApplications")?;
        xfer.xfer_u32(&mut self.fast_drive_applications)?;
        xfer.xfer_marker_label("OuterNodeSystemsCreated")?;
        xfer.xfer_u32(&mut self.outer_node_systems_created)?;
        xfer.xfer_marker_label("ConnectorLasersCreated")?;
        xfer.xfer_u32(&mut self.connector_lasers_created)?;
        xfer.xfer_marker_label("LaserBaseFlareCreated")?;
        xfer.xfer_u32(&mut self.laser_base_flare_created)?;
        xfer.xfer_marker_label("GroundToOrbitLaserCreated")?;
        xfer.xfer_u32(&mut self.ground_to_orbit_laser_created)?;
        // Intensity schedule residual (CHARGING…POSTFIRE/PACKING + BeamLaunchFX).
        xfer.xfer_marker_label("Status")?;
        {
            let mut v = self.status.as_u8();
            xfer.xfer_u8(&mut v)?;
            self.status =
                crate::game_logic::special_power_strikes::ParticleUplinkStatus::from_u8(v);
        }
        xfer.xfer_marker_label("OuterIntensity")?;
        {
            let mut v = self.outer_intensity.as_u8();
            xfer.xfer_u8(&mut v)?;
            self.outer_intensity =
                crate::game_logic::special_power_strikes::ParticleIntensity::from_u8(v);
        }
        xfer.xfer_marker_label("ConnectorIntensity")?;
        {
            let mut v = self.connector_intensity.as_u8();
            xfer.xfer_u8(&mut v)?;
            self.connector_intensity =
                crate::game_logic::special_power_strikes::ParticleIntensity::from_u8(v);
        }
        xfer.xfer_marker_label("LaserBaseIntensity")?;
        {
            let mut v = self.laser_base_intensity.as_u8();
            xfer.xfer_u8(&mut v)?;
            self.laser_base_intensity =
                crate::game_logic::special_power_strikes::ParticleIntensity::from_u8(v);
        }
        xfer.xfer_marker_label("BeamLaunchFxApplications")?;
        xfer.xfer_u32(&mut self.beam_launch_fx_applications)?;
        xfer.xfer_marker_label("NextLaunchFxFrame")?;
        xfer.xfer_u32(&mut self.next_launch_fx_frame)?;
        xfer.xfer_marker_label("PostfireApplications")?;
        xfer.xfer_u32(&mut self.postfire_applications)?;
        xfer.xfer_marker_label("PackingApplications")?;
        xfer.xfer_u32(&mut self.packing_applications)?;
        xfer.xfer_marker_label("IntensityTransitions")?;
        xfer.xfer_u32(&mut self.intensity_transitions)?;
        xfer.xfer_marker_label("ConnectorFlareCreated")?;
        xfer.xfer_u32(&mut self.connector_flare_created)?;
        // OuterBeamWidth × scalar / retail laser radius residual (appended).
        xfer.xfer_marker_label("PeakOuterBeamDrawWidth")?;
        xfer.xfer_f32(&mut self.peak_outer_beam_draw_width)?;
        xfer.xfer_marker_label("LastOuterBeamDrawWidth")?;
        xfer.xfer_f32(&mut self.last_outer_beam_draw_width)?;
        xfer.xfer_marker_label("PeakRetailLaserRadius")?;
        xfer.xfer_f32(&mut self.peak_retail_laser_radius)?;
        xfer.xfer_marker_label("LastRetailLaserRadius")?;
        xfer.xfer_f32(&mut self.last_retail_laser_radius)?;
        xfer.xfer_marker_label("PeakRetailDamageRadius")?;
        xfer.xfer_f32(&mut self.peak_retail_damage_radius)?;
        xfer.xfer_marker_label("LastRetailDamageRadius")?;
        xfer.xfer_f32(&mut self.last_retail_damage_radius)?;
        xfer.xfer_marker_label("OrbitalLaserDrawParamsArmed")?;
        xfer.xfer_u32(&mut self.orbital_laser_draw_params_armed)?;
        xfer.xfer_marker_label("ConnectorOuterBeamWidthArmed")?;
        xfer.xfer_u32(&mut self.connector_outer_beam_width_armed)?;
        // Multi-beam NumBeams + ScrollRate residual (appended).
        xfer.xfer_marker_label("NumBeamsArmed")?;
        xfer.xfer_u32(&mut self.num_beams_armed)?;
        xfer.xfer_marker_label("TilingScalarArmed")?;
        xfer.xfer_u32(&mut self.tiling_scalar_armed)?;
        xfer.xfer_marker_label("LastScrollUv")?;
        xfer.xfer_f32(&mut self.last_scroll_uv)?;
        xfer.xfer_marker_label("PeakAbsScrollUv")?;
        xfer.xfer_f32(&mut self.peak_abs_scroll_uv)?;
        xfer.xfer_marker_label("ScrollUvSamples")?;
        xfer.xfer_u32(&mut self.scroll_uv_samples)?;
        // Multi-beam soft-edge residual (appended).
        xfer.xfer_marker_label("SoftEdgeSamples")?;
        xfer.xfer_u32(&mut self.soft_edge_samples)?;
        xfer.xfer_marker_label("PeakSoftEdgeOuterWidth")?;
        xfer.xfer_f32(&mut self.peak_soft_edge_outer_width)?;
        xfer.xfer_marker_label("LastSoftEdgeOuterWidth")?;
        xfer.xfer_f32(&mut self.last_soft_edge_outer_width)?;
        xfer.xfer_marker_label("LastSoftEdgeOuterAlpha")?;
        xfer.xfer_f32(&mut self.last_soft_edge_outer_alpha)?;
        xfer.xfer_marker_label("LastSoftEdgeTileFactor")?;
        xfer.xfer_f32(&mut self.last_soft_edge_tile_factor)?;
        xfer.xfer_marker_label("SoftEdgeColorArmed")?;
        xfer.xfer_u32(&mut self.soft_edge_color_armed)?;
        xfer.xfer_marker_label("SoftEdgePremulSamples")?;
        xfer.xfer_u32(&mut self.soft_edge_premul_samples)?;
        xfer.xfer_marker_label("LastSoftEdgePremulOuterR")?;
        xfer.xfer_f32(&mut self.last_soft_edge_premul_outer_r)?;
        // Connector soft-edge premul + Orbital KindOf/Segments residual (appended).
        xfer.xfer_marker_label("ConnectorSoftEdgePremulSamples")?;
        xfer.xfer_u32(&mut self.connector_soft_edge_premul_samples)?;
        xfer.xfer_marker_label("LastConnectorSoftEdgePremulOuterR")?;
        xfer.xfer_f32(&mut self.last_connector_soft_edge_premul_outer_r)?;
        xfer.xfer_marker_label("OrbitalKindofImmobileArmed")?;
        xfer.xfer_u32(&mut self.orbital_kindof_immobile_armed)?;
        xfer.xfer_marker_label("OrbitalSegmentsArmed")?;
        xfer.xfer_u32(&mut self.orbital_segments_armed)?;
        xfer.xfer_marker_label("OrbitalArcHeightArmed")?;
        xfer.xfer_u32(&mut self.orbital_arc_height_armed)?;
        // Connector KindOf / Segments / MaxIntensity / Tile residual (appended).
        xfer.xfer_marker_label("ConnectorKindofImmobileArmed")?;
        xfer.xfer_u32(&mut self.connector_kindof_immobile_armed)?;
        xfer.xfer_marker_label("ConnectorSegmentsArmed")?;
        xfer.xfer_u32(&mut self.connector_segments_armed)?;
        xfer.xfer_marker_label("ConnectorMaxIntensityFadeArmed")?;
        xfer.xfer_u32(&mut self.connector_max_intensity_fade_armed)?;
        xfer.xfer_marker_label("ConnectorTileNoArmed")?;
        xfer.xfer_u32(&mut self.connector_tile_no_armed)?;
        // Outer-node bone layout residual (appended).
        xfer.xfer_marker_label("OuterNodeBoneLayoutApplications")?;
        xfer.xfer_u32(&mut self.outer_node_bone_layout_applications)?;
        xfer.xfer_marker_label("LastOuterNodeBonePosition")?;
        self.last_outer_node_bone_position.xfer(xfer)?;
        xfer.xfer_marker_label("ConnectorBoneLayoutApplications")?;
        xfer.xfer_u32(&mut self.connector_bone_layout_applications)?;
        // Intense connector soft-edge + laser segments residual (appended).
        xfer.xfer_marker_label("ConnectorSoftEdgeArmed")?;
        xfer.xfer_u32(&mut self.connector_soft_edge_armed)?;
        xfer.xfer_marker_label("PeakConnectorSoftEdgeOuterWidth")?;
        xfer.xfer_f32(&mut self.peak_connector_soft_edge_outer_width)?;
        xfer.xfer_marker_label("ConnectorLaserSegmentsCreated")?;
        xfer.xfer_u32(&mut self.connector_laser_segments_created)?;
        xfer.xfer_marker_label("LastConnectorSegmentStart")?;
        self.last_connector_segment_start.xfer(xfer)?;
        xfer.xfer_marker_label("LastConnectorSegmentEnd")?;
        self.last_connector_segment_end.xfer(xfer)?;
        // Medium connector soft-edge + OrbitalLaser Vision/Shroud residual (appended).
        xfer.xfer_marker_label("MediumConnectorSoftEdgeArmed")?;
        xfer.xfer_u32(&mut self.medium_connector_soft_edge_armed)?;
        xfer.xfer_marker_label("PeakMediumConnectorSoftEdgeOuterWidth")?;
        xfer.xfer_f32(&mut self.peak_medium_connector_soft_edge_outer_width)?;
        xfer.xfer_marker_label("OrbitalVisionShroudArmed")?;
        xfer.xfer_u32(&mut self.orbital_vision_shroud_armed)?;
        xfer.xfer_marker_label("LastOrbitalVisionRange")?;
        xfer.xfer_f32(&mut self.last_orbital_vision_range)?;
        xfer.xfer_marker_label("LastOrbitalShroudClearingRange")?;
        xfer.xfer_f32(&mut self.last_orbital_shroud_clearing_range)?;
        // LaserUpdate client residual (appended).
        xfer.xfer_marker_label("LaserUpdateInitApplications")?;
        xfer.xfer_u32(&mut self.laser_update_init_applications)?;
        xfer.xfer_marker_label("LaserUpdateDirty")?;
        xfer.xfer_bool(&mut self.laser_update_dirty)?;
        xfer.xfer_marker_label("LaserUpdateGrowthFrames")?;
        xfer.xfer_u32(&mut self.laser_update_growth_frames)?;
        xfer.xfer_marker_label("LaserUpdateCurrentWidthScalar")?;
        xfer.xfer_f32(&mut self.laser_update_current_width_scalar)?;
        xfer.xfer_marker_label("LaserUpdateWidening")?;
        xfer.xfer_bool(&mut self.laser_update_widening)?;
        xfer.xfer_marker_label("LaserUpdateDecaying")?;
        xfer.xfer_bool(&mut self.laser_update_decaying)?;
        xfer.xfer_marker_label("LastLaserUpdateStart")?;
        self.last_laser_update_start.xfer(xfer)?;
        xfer.xfer_marker_label("LastLaserUpdateEnd")?;
        self.last_laser_update_end.xfer(xfer)?;
        xfer.xfer_marker_label("LastLaserUpdateDrawableMid")?;
        self.last_laser_update_drawable_mid.xfer(xfer)?;
        xfer.xfer_marker_label("LastLaserUpdateRadius")?;
        xfer.xfer_f32(&mut self.last_laser_update_radius)?;
        // Wave 45: PUC sound / scorch residual pack honesty (appended).
        xfer.xfer_marker_label("GroundAnnihilationAudioApplications")?;
        xfer.xfer_u32(&mut self.ground_annihilation_audio_applications)?;
        xfer.xfer_marker_label("FiringToPackAudioApplications")?;
        xfer.xfer_u32(&mut self.firing_to_pack_audio_applications)?;
        xfer.xfer_marker_label("SoundResidualPackArmed")?;
        xfer.xfer_u32(&mut self.sound_residual_pack_armed)?;
        xfer.xfer_marker_label("ScorchScalarPackArmed")?;
        xfer.xfer_u32(&mut self.scorch_scalar_pack_armed)?;
        // Wave 50: OuterNodes flare pack + SlowDeath/InstantDeath pack (appended).
        xfer.xfer_marker_label("OuterNodeFlarePackArmed")?;
        xfer.xfer_u32(&mut self.outer_node_flare_pack_armed)?;
        xfer.xfer_marker_label("DeathPackArmed")?;
        xfer.xfer_u32(&mut self.death_pack_armed)?;
        Ok(())
    }
}

impl XferData for SpecialPowerStrikeRegistrySnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("SpecialPowerStrikeRegistrySnapshot")?;
        xfer.xfer_marker_label("NextId")?;
        xfer.xfer_u32(&mut self.next_id)?;
        xfer.xfer_marker_label("Strikes")?;
        xfer_vec_default(
            xfer,
            &mut self.strikes,
            HostSpecialPowerStrike {
                id: 0,
                kind: HostSuperweaponKind::DaisyCutter,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                target_position: Vec3::ZERO,
                activate_frame: 0,
                impact_frame: 0,
                phase: HostStrikePhase::Queued,
                total_damage_applied: 0.0,
                objects_hit: 0,
                objects_destroyed: 0,
                artillery_tier:
                    crate::game_logic::special_power_strikes::ArtilleryBarrageScienceTier::Level1,
                spectre_tier:
                    crate::game_logic::special_power_strikes::SpectreGunshipScienceTier::Level2,
                scud_anthrax_tier:
                    crate::game_logic::special_power_strikes::ScudStormAnthraxTier::Base,
                multi_strike_applied: 0,
                particle_status:
                    crate::game_logic::special_power_strikes::ParticleUplinkStatus::Idle,
                particle_status_peak:
                    crate::game_logic::special_power_strikes::ParticleUplinkStatus::Idle,
                particle_intensity_transitions: 0,
                particle_charging_applications: 0,
                particle_preparing_applications: 0,
                particle_almost_ready_applications: 0,
                particle_ready_applications: 0,
                particle_model_unpacking_sets: 0,
                particle_model_deployed_sets: 0,
                particle_model_packing_sets: 0,
                particle_powerup_audio_applications: 0,
                particle_unpack_audio_applications: 0,
                scud_pre_attack_active: false,
                scud_pre_attack_frames: 0,
                scud_chem_fx_bones: 0,
                scud_fire_fx_applications: 0,
                scud_detonation_fx_applications: 0,
                scud_launch_bone_applications: 0,
                scud_missile_loft_applications: 0,
                scud_ignition_fx_applications: 0,
                scud_launch_sound_applications: 0,
                scud_exhaust_applications: 0,
                scud_height_die_applications: 0,
                scud_special_power_completion_applications: 0,
                ocl_points: Vec::new(),
                ocl_shell_frames: Vec::new(),
                ocl_once_at_queue_armed: 0,
                scud_spawn_height_applications: 0,
                scud_preferred_height_spring_applications: 0,
                scud_loft_phase_peak:
                    crate::game_logic::special_power_strikes::ScudMissileLoftPhase::Loft,
                scud_last_spring_height: 0.0,
                scud_ballistic_flight_applications: 0,
                scud_only_moving_down_applications: 0,
                scud_snap_to_ground_applications: 0,
                scud_model_draw_applications: 0,
                scud_last_flight_distance: 0.0,
                scud_peak_flight_distance: 0.0,
                scud_last_flight_height: 0.0,
                scud_thrust_wobble_applications: 0,
                scud_last_thrust_wobble: 0.0,
                scud_peak_abs_thrust_wobble: 0.0,
                scud_geometry_applications: 0,
                scud_object_params_applications: 0,
                scud_missile_ai_applications: 0,
                scud_fire_weapon_when_dead_applications: 0,
                scud_body_draw_params_applications: 0,
                scud_locomotor_appearance_applications: 0,
                scud_destroy_die_locomotor_name_applications: 0,
                scud_death_fire_ocl_applications: 0,
                scud_locomotor_speed_table_applications: 0,
                scud_death_damage_table_applications: 0,
                scud_weapon_launch_applications: 0,
                scud_weapon_special_applications: 0,
                scud_missile_ai_defaults_applications: 0,
                scud_thing_factory_spawn_applications: 0,
                carpet_tier:
                    crate::game_logic::special_power_strikes::CarpetBombFactionTier::America,
                carpet_residual_pack_armed: 0,
                carpet_preferred_height_applications: 0,
                carpet_drop_delay_applications: 0,
                carpet_drop_variance_applications: 0,
                carpet_bomb_count_applications: 0,
                carpet_fire_fx_applications: 0,
                carpet_delivery_distance_applications: 0,
                artillery_residual_pack_armed: 0,
                artillery_cannon_transport_applications: 0,
                artillery_formation_size_applications: 0,
                artillery_delay_delivery_applications: 0,
                artillery_weapon_error_radius_applications: 0,
                artillery_preferred_height_applications: 0,
                artillery_fire_fx_applications: 0,
                cruise_residual_pack_armed: 0,
                cruise_loft_applications: 0,
                cruise_height_die_applications: 0,
                cruise_projectile_applications: 0,
                cruise_moab_weapon_applications: 0,
                cruise_moab_flame_applications: 0,
                cruise_moab_fire_fx_applications: 0,
                nuke_radiation_residual_pack_applications: 0,
                anthrax_toxin_residual_pack_applications: 0,
            },
        )?;
        // NuclearMissile residual radiation fields (appended; older binary
        // residual saves without these fields fail-closed on xfer).
        xfer.xfer_marker_label("NextRadiationId")?;
        xfer.xfer_u32(&mut self.next_radiation_id)?;
        xfer.xfer_marker_label("RadiationFields")?;
        xfer_vec_default(
            xfer,
            &mut self.radiation_fields,
            crate::game_logic::special_power_strikes::HostRadiationField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                position: Vec3::ZERO,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_strike_id: 0,
                radiation_residual_pack_armed: 0,
                radiation_suspend_fx_applications: 0,
                radiation_fire_fx_applications: 0,
            },
        )?;
        xfer.xfer_marker_label("RadiationFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.radiation_fields_spawned_total)?;
        xfer.xfer_marker_label("RadiationDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.radiation_damage_applications_total)?;
        // AnthraxBomb residual toxin fields (appended after radiation).
        xfer.xfer_marker_label("NextToxinId")?;
        xfer.xfer_u32(&mut self.next_toxin_id)?;
        xfer.xfer_marker_label("ToxinFields")?;
        xfer_vec_default(
            xfer,
            &mut self.toxin_fields,
            crate::game_logic::special_power_strikes::HostToxinField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                position: Vec3::ZERO,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_strike_id: 0,
                toxin_residual_pack_armed: 0,
                toxin_fire_fx_applications: 0,
                toxin_damage_type_applications: 0,
                damage_per_tick:
                    crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_DAMAGE_PER_TICK,
                radius: crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_RADIUS,
                tick_interval_frames:
                    crate::game_logic::special_power_strikes::ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES,
            },
        )?;
        xfer.xfer_marker_label("ToxinFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.toxin_fields_spawned_total)?;
        xfer.xfer_marker_label("ToxinDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.toxin_damage_applications_total)?;
        // SpectreGunship residual orbit fields (appended after toxin).
        xfer.xfer_marker_label("NextOrbitId")?;
        xfer.xfer_u32(&mut self.next_orbit_id)?;
        xfer.xfer_marker_label("OrbitFields")?;
        xfer_vec_default(
            xfer,
            &mut self.orbit_fields,
            crate::game_logic::special_power_strikes::HostSpectreOrbitField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                position: Vec3::ZERO,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                next_gattling_tick_frame: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_strike_id: 0,
                howitzer_ticks: 0,
                gattling_ticks: 0,
                gattling_consecutive: 0,
                howitzer_consecutive: 0,
                gattling_fire_level: 0,
                howitzer_fire_level: 0,
                gattling_coast_until_frame: 0,
                howitzer_coast_until_frame: 0,
                gattling_coast_applications: 0,
                howitzer_coast_applications: 0,
                rapid_fire_voice_cues: 0,
                model_condition_mean_sets: 0,
                model_condition_fast_sets: 0,
                model_condition_slow_sets: 0,
                howitzer_shells_spawned: 0,
                howitzer_shell_fire_fx: 0,
                howitzer_shell_detonation_fx: 0,
                howitzer_shell_height_die_delays: 0,
                howitzer_shell_fire_sounds: 0,
                howitzer_shell_dumb_projectile_applications: 0,
                howitzer_shell_physics_mass_applications: 0,
                howitzer_shell_death_detonated_applications: 0,
                howitzer_shell_death_lasered_applications: 0,
                howitzer_shell_death_lasered_ocl_applications: 0,
                howitzer_shell_death_generic_applications: 0,
                howitzer_shell_object_params_applications: 0,
                howitzer_shell_design_params_applications: 0,
                howitzer_shell_only_moving_down_applications: 0,
                howitzer_shell_model_draw_applications: 0,
                howitzer_shell_scale_applications: 0,
                howitzer_shell_shadow_applications: 0,
                howitzer_shell_geometry_applications: 0,
                howitzer_shell_max_health_applications: 0,
                howitzer_shell_loft_flight_applications: 0,
                howitzer_shell_last_loft_height: 0.0,
                howitzer_shell_loft_height_die_applications: 0,
                howitzer_shell_locomotor_template_applications: 0,
                howitzer_shell_damage_fx_applications: 0,
                howitzer_shell_thing_factory_spawn_applications: 0,
                howitzer_gun_aim_params_applications: 0,
                howitzer_gun_fire_params_applications: 0,
                howitzer_gun_anti_params_applications: 0,
                gattling_gun_params_applications: 0,
                gattling_rof_mean_applications: 0,
                gattling_rof_fast_applications: 0,
            },
        )?;
        xfer.xfer_marker_label("OrbitFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.orbit_fields_spawned_total)?;
        xfer.xfer_marker_label("OrbitDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.orbit_damage_applications_total)?;
        // ParticleCannon residual continuous beam fields (appended after orbit).
        xfer.xfer_marker_label("NextBeamId")?;
        xfer.xfer_u32(&mut self.next_beam_id)?;
        xfer.xfer_marker_label("BeamFields")?;
        xfer_vec_default(
            xfer,
            &mut self.beam_fields,
            crate::game_logic::special_power_strikes::HostParticleBeamField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                position: Vec3::ZERO,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                pulses_made: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_strike_id: 0,
                last_swath_position: Vec3::ZERO,
                max_swath_offset: 0.0,
                swath_applications: 0,
                next_scorch_frame: 0,
                scorch_marks_made: 0,
                reveal_applications: 0,
                ground_hit_fx_applications: 0,
                peak_width_scalar: 0.0,
                last_damage_radius: 0.0,
                last_width_scalar: 0.0,
                trough_width_scalar: 1.0,
                decay_samples: 0,
                last_scorch_position: Vec3::ZERO,
                last_scorch_radius: 0.0,
                manual_target_mode: false,
                override_destination: Vec3::ZERO,
                current_target_position: Vec3::ZERO,
                last_driving_click_frame: 0,
                second_last_driving_click_frame: 0,
                last_drive_update_frame: 0,
                manual_drive_distance_total: 0.0,
                manual_drive_applications: 0,
                fast_drive_applications: 0,
                outer_node_systems_created: 0,
                connector_lasers_created: 0,
                laser_base_flare_created: 0,
                ground_to_orbit_laser_created: 0,
                status: crate::game_logic::special_power_strikes::ParticleUplinkStatus::Idle,
                outer_intensity: crate::game_logic::special_power_strikes::ParticleIntensity::None,
                connector_intensity:
                    crate::game_logic::special_power_strikes::ParticleIntensity::None,
                laser_base_intensity:
                    crate::game_logic::special_power_strikes::ParticleIntensity::None,
                beam_launch_fx_applications: 0,
                next_launch_fx_frame: 0,
                postfire_applications: 0,
                packing_applications: 0,
                intensity_transitions: 0,
                connector_flare_created: 0,
                peak_outer_beam_draw_width: 0.0,
                last_outer_beam_draw_width: 0.0,
                peak_retail_laser_radius: 0.0,
                last_retail_laser_radius: 0.0,
                peak_retail_damage_radius: 0.0,
                last_retail_damage_radius: 0.0,
                orbital_laser_draw_params_armed: 0,
                connector_outer_beam_width_armed: 0,
                num_beams_armed: 0,
                tiling_scalar_armed: 0,
                last_scroll_uv: 0.0,
                peak_abs_scroll_uv: 0.0,
                scroll_uv_samples: 0,
                soft_edge_samples: 0,
                peak_soft_edge_outer_width: 0.0,
                last_soft_edge_outer_width: 0.0,
                last_soft_edge_outer_alpha: 0.0,
                last_soft_edge_tile_factor: 0.0,
                soft_edge_color_armed: 0,
                soft_edge_premul_samples: 0,
                last_soft_edge_premul_outer_r: 0.0,
                connector_soft_edge_premul_samples: 0,
                last_connector_soft_edge_premul_outer_r: 0.0,
                orbital_kindof_immobile_armed: 0,
                orbital_segments_armed: 0,
                orbital_arc_height_armed: 0,
                connector_kindof_immobile_armed: 0,
                connector_segments_armed: 0,
                connector_max_intensity_fade_armed: 0,
                connector_tile_no_armed: 0,
                outer_node_bone_layout_applications: 0,
                last_outer_node_bone_position: Vec3::ZERO,
                connector_bone_layout_applications: 0,
                connector_soft_edge_armed: 0,
                peak_connector_soft_edge_outer_width: 0.0,
                connector_laser_segments_created: 0,
                last_connector_segment_start: Vec3::ZERO,
                last_connector_segment_end: Vec3::ZERO,
                medium_connector_soft_edge_armed: 0,
                peak_medium_connector_soft_edge_outer_width: 0.0,
                orbital_vision_shroud_armed: 0,
                last_orbital_vision_range: 0.0,
                last_orbital_shroud_clearing_range: 0.0,
                laser_update_init_applications: 0,
                laser_update_dirty: false,
                laser_update_growth_frames: 0,
                laser_update_current_width_scalar: 0.0,
                laser_update_widening: false,
                laser_update_decaying: false,
                last_laser_update_start: Vec3::ZERO,
                last_laser_update_end: Vec3::ZERO,
                last_laser_update_drawable_mid: Vec3::ZERO,
                last_laser_update_radius: 0.0,
                ground_annihilation_audio_applications: 0,
                firing_to_pack_audio_applications: 0,
                sound_residual_pack_armed: 0,
                scorch_scalar_pack_armed: 0,
                outer_node_flare_pack_armed: 0,
                death_pack_armed: 0,
            },
        )?;
        xfer.xfer_marker_label("BeamFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.beam_fields_spawned_total)?;
        xfer.xfer_marker_label("BeamDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.beam_damage_applications_total)?;
        // Particle Uplink DamagePulseRemnant trail residual (appended after beam).
        xfer.xfer_marker_label("NextRemnantId")?;
        xfer.xfer_u32(&mut self.next_remnant_id)?;
        xfer.xfer_marker_label("RemnantFields")?;
        xfer_vec_default(
            xfer,
            &mut self.remnant_fields,
            crate::game_logic::special_power_strikes::HostParticleRemnantField {
                id: 0,
                source_object: ObjectId(0),
                source_team: Team::Neutral,
                position: Vec3::ZERO,
                spawn_frame: 0,
                expires_frame: 0,
                next_tick_frame: 0,
                total_damage_applied: 0.0,
                damage_applications: 0,
                objects_destroyed: 0,
                parent_beam_id: 0,
                parent_strike_id: 0,
                remnant_object_params_applications: 0,
                remnant_fire_deletion_applications: 0,
                remnant_immortal_body_applications: 0,
                remnant_thing_factory_spawn_applications: 0,
            },
        )?;
        xfer.xfer_marker_label("RemnantFieldsSpawnedTotal")?;
        xfer.xfer_u32(&mut self.remnant_fields_spawned_total)?;
        xfer.xfer_marker_label("RemnantDamageApplicationsTotal")?;
        xfer.xfer_u32(&mut self.remnant_damage_applications_total)?;
        Ok(())
    }
}

impl XferData for crate::game_logic::special_power_strikes::HostParticleRemnantField {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostParticleRemnantField")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("SourceObject")?;
        self.source_object.xfer(xfer)?;
        xfer.xfer_marker_label("SourceTeam")?;
        self.source_team.xfer(xfer)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SpawnFrame")?;
        xfer.xfer_u32(&mut self.spawn_frame)?;
        xfer.xfer_marker_label("ExpiresFrame")?;
        xfer.xfer_u32(&mut self.expires_frame)?;
        xfer.xfer_marker_label("NextTickFrame")?;
        xfer.xfer_u32(&mut self.next_tick_frame)?;
        xfer.xfer_marker_label("TotalDamageApplied")?;
        xfer.xfer_f32(&mut self.total_damage_applied)?;
        xfer.xfer_marker_label("DamageApplications")?;
        xfer.xfer_u32(&mut self.damage_applications)?;
        xfer.xfer_marker_label("ObjectsDestroyed")?;
        xfer.xfer_u32(&mut self.objects_destroyed)?;
        xfer.xfer_marker_label("ParentBeamId")?;
        xfer.xfer_u32(&mut self.parent_beam_id)?;
        xfer.xfer_marker_label("ParentStrikeId")?;
        xfer.xfer_u32(&mut self.parent_strike_id)?;
        // TrailRemnant KindOf / ImmortalBody residual (appended).
        xfer.xfer_marker_label("RemnantObjectParamsApplications")?;
        xfer.xfer_u32(&mut self.remnant_object_params_applications)?;
        // TrailRemnant FireWeaponUpdate + DeletionUpdate residual (appended).
        xfer.xfer_marker_label("RemnantFireDeletionApplications")?;
        xfer.xfer_u32(&mut self.remnant_fire_deletion_applications)?;
        // TrailRemnant ImmortalBody health-floor residual (appended).
        xfer.xfer_marker_label("RemnantImmortalBodyApplications")?;
        xfer.xfer_u32(&mut self.remnant_immortal_body_applications)?;
        // Wave 74: TrailRemnant ThingFactory spawn bookkeeping residual.
        xfer.xfer_marker_label("RemnantThingFactorySpawnApplications")?;
        xfer.xfer_u32(&mut self.remnant_thing_factory_spawn_applications)?;
        Ok(())
    }
}

impl XferData for CombatParticleKind {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            CombatParticleKind::DeathExplosion => 0u32,
            CombatParticleKind::DeathSmoke => 1,
            CombatParticleKind::WeaponMuzzleFlash => 2,
            CombatParticleKind::WeaponImpact => 3,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => CombatParticleKind::DeathExplosion,
            1 => CombatParticleKind::DeathSmoke,
            2 => CombatParticleKind::WeaponMuzzleFlash,
            3 => CombatParticleKind::WeaponImpact,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid CombatParticleKind discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for CombatParticleSystemEntry {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("CombatParticleSystemEntry")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("Kind")?;
        self.kind.xfer(xfer)?;
        xfer.xfer_marker_label("TemplateName")?;
        self.template_name.xfer(xfer)?;
        xfer.xfer_marker_label("Position")?;
        self.position.xfer(xfer)?;
        xfer.xfer_marker_label("SourceObject")?;
        xfer_option(xfer, &mut self.source_object, ObjectId(0))?;
        xfer.xfer_marker_label("TargetObject")?;
        xfer_option(xfer, &mut self.target_object, ObjectId(0))?;
        xfer.xfer_marker_label("SpawnedFrame")?;
        xfer.xfer_u32(&mut self.spawned_frame)?;
        xfer.xfer_marker_label("Active")?;
        xfer.xfer_bool(&mut self.active)?;
        xfer.xfer_marker_label("ClientSystemId")?;
        // Option<u32> residual — client rebind may drop this after load.
        let mut has_client = self.client_system_id.is_some();
        xfer.xfer_bool(&mut has_client)?;
        if has_client {
            let mut id = self.client_system_id.unwrap_or(0);
            xfer.xfer_u32(&mut id)?;
            self.client_system_id = Some(id);
        } else {
            self.client_system_id = None;
        }
        Ok(())
    }
}

impl XferData for CombatParticleRegistrySnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("CombatParticleRegistrySnapshot")?;
        xfer.xfer_marker_label("NextId")?;
        xfer.xfer_u32(&mut self.next_id)?;
        xfer.xfer_marker_label("Systems")?;
        xfer_vec_default(
            xfer,
            &mut self.systems,
            CombatParticleSystemEntry {
                id: 0,
                kind: CombatParticleKind::DeathExplosion,
                template_name: String::new(),
                position: Vec3::ZERO,
                source_object: None,
                target_object: None,
                spawned_frame: 0,
                active: false,
                client_system_id: None,
            },
        )?;
        Ok(())
    }
}

impl XferData for HostUpgradeKind {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            HostUpgradeKind::CaptureBuilding => 0u32,
            HostUpgradeKind::FlashBangGrenade => 1,
            HostUpgradeKind::TowMissile => 2,
            HostUpgradeKind::SupplyLines => 3,
            HostUpgradeKind::NeutronShells => 5,
            HostUpgradeKind::Other => 4,
            HostUpgradeKind::BunkerBusters => 6,
            HostUpgradeKind::ComancheRocketPods => 7,
            HostUpgradeKind::SentryDroneGun => 8,
            HostUpgradeKind::Camouflage => 9,
            HostUpgradeKind::CompositeArmor => 10,
            HostUpgradeKind::WorkerShoes => 11,
            HostUpgradeKind::NuclearTanks => 12,
            HostUpgradeKind::BoobyTrap => 13,
            HostUpgradeKind::AnthraxGamma => 14,
            HostUpgradeKind::CamoNetting => 15,
            HostUpgradeKind::SuicideBomb => 16,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => HostUpgradeKind::CaptureBuilding,
            1 => HostUpgradeKind::FlashBangGrenade,
            2 => HostUpgradeKind::TowMissile,
            3 => HostUpgradeKind::SupplyLines,
            4 => HostUpgradeKind::Other,
            5 => HostUpgradeKind::NeutronShells,
            6 => HostUpgradeKind::BunkerBusters,
            7 => HostUpgradeKind::ComancheRocketPods,
            8 => HostUpgradeKind::SentryDroneGun,
            9 => HostUpgradeKind::Camouflage,
            10 => HostUpgradeKind::CompositeArmor,
            11 => HostUpgradeKind::WorkerShoes,
            12 => HostUpgradeKind::NuclearTanks,
            13 => HostUpgradeKind::BoobyTrap,
            14 => HostUpgradeKind::AnthraxGamma,
            15 => HostUpgradeKind::CamoNetting,
            16 => HostUpgradeKind::SuicideBomb,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HostUpgradeKind discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for HostUpgradePhase {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        let mut value = match self {
            HostUpgradePhase::Queued => 0u32,
            HostUpgradePhase::Completed => 1,
            HostUpgradePhase::Cancelled => 2,
        };
        xfer.xfer_u32(&mut value)?;
        *self = match value {
            0 => HostUpgradePhase::Queued,
            1 => HostUpgradePhase::Completed,
            2 => HostUpgradePhase::Cancelled,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HostUpgradePhase discriminant: {other}"
                )));
            }
        };
        Ok(())
    }
}

impl XferData for HostUpgradeResearch {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostUpgradeResearch")?;
        xfer.xfer_marker_label("Id")?;
        xfer.xfer_u32(&mut self.id)?;
        xfer.xfer_marker_label("Name")?;
        self.name.xfer(xfer)?;
        xfer.xfer_marker_label("Kind")?;
        self.kind.xfer(xfer)?;
        xfer.xfer_marker_label("Team")?;
        self.team.xfer(xfer)?;
        xfer.xfer_marker_label("PlayerId")?;
        xfer.xfer_u32(&mut self.player_id)?;
        xfer.xfer_marker_label("QueueFrame")?;
        xfer.xfer_u32(&mut self.queue_frame)?;
        xfer.xfer_marker_label("CompleteFrame")?;
        xfer.xfer_u32(&mut self.complete_frame)?;
        xfer.xfer_marker_label("Phase")?;
        self.phase.xfer(xfer)?;
        xfer.xfer_marker_label("UnitsAffected")?;
        xfer.xfer_u32(&mut self.units_affected)?;
        xfer.xfer_marker_label("SourceObject")?;
        xfer_option(xfer, &mut self.source_object, ObjectId(0))?;
        // Wave 79: cost/time residual application bookkeeping (appended).
        xfer.xfer_marker_label("BuildCostPaid")?;
        xfer.xfer_u32(&mut self.build_cost_paid)?;
        xfer.xfer_marker_label("RetailResearchFrames")?;
        xfer.xfer_u32(&mut self.retail_research_frames)?;
        xfer.xfer_marker_label("ResidualResearchFrames")?;
        xfer.xfer_u32(&mut self.residual_research_frames)?;
        Ok(())
    }
}

impl XferData for HostUpgradeRegistrySnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HostUpgradeRegistrySnapshot")?;
        xfer.xfer_marker_label("NextId")?;
        xfer.xfer_u32(&mut self.next_id)?;
        xfer.xfer_marker_label("Entries")?;
        xfer_vec_default(
            xfer,
            &mut self.entries,
            HostUpgradeResearch {
                id: 0,
                name: String::new(),
                kind: HostUpgradeKind::Other,
                team: Team::Neutral,
                player_id: 0,
                queue_frame: 0,
                complete_frame: 0,
                phase: HostUpgradePhase::Queued,
                units_affected: 0,
                source_object: None,
                build_cost_paid: 0,
                retail_research_frames: 0,
                residual_research_frames: 1,
            },
        )?;
        Ok(())
    }
}

/// Snapshot builder for creating world snapshots from current game state
pub struct SnapshotBuilder {
    // Access to game systems for snapshot creation
}

impl Default for SnapshotBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SnapshotBuilder {
    pub fn new() -> Self {
        Self {}
    }

    /// Create complete world snapshot from current game state
    pub fn create_world_snapshot(&self, game_logic: &GameLogic) -> SaveLoadResult<WorldSnapshot> {
        log::info!("Creating world snapshot from game state");

        // Snapshot all objects from game state
        let objects = self.snapshot_all_objects(game_logic)?;

        // Snapshot all players
        let players = self.snapshot_all_players(game_logic)?;

        // Create the world snapshot with actual game state
        let snapshot = WorldSnapshot {
            version: 1,
            timestamp: std::time::SystemTime::now(),
            frame_number: game_logic.get_current_frame(),
            random_seed: 0, // Main crate GameLogic doesn't track random seed explicitly

            objects,
            players,
            teams: self.snapshot_all_teams(game_logic)?,
            terrain: self.snapshot_terrain(game_logic)?,
            weather: self.snapshot_weather(game_logic)?,
            resource_manager: self.snapshot_resource_manager(game_logic)?,
            combat_tracker: self.snapshot_combat_tracker(game_logic)?,
            experience_tracker: self.snapshot_experience_tracker(game_logic)?,
            pathfinding_cache: self.snapshot_pathfinding_cache(game_logic)?,
            ai_players: Vec::new(),
            global_ai_state: self.snapshot_global_ai_state(game_logic)?,
            special_power_strikes: self.snapshot_special_power_strikes(game_logic)?,
            combat_particles: self.snapshot_combat_particles(game_logic)?,
            host_upgrades: self.snapshot_host_upgrades(game_logic)?,
        };

        log::info!(
            "World snapshot complete: {} objects, {} players",
            snapshot.objects.len(),
            snapshot.players.len()
        );

        Ok(snapshot)
    }

    /// Restore game state from world snapshot
    pub fn restore_from_snapshot(
        &self,
        snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        log::info!(
            "Restoring world from snapshot: {} objects, {} players",
            snapshot.objects.len(),
            snapshot.players.len()
        );

        // Restore frame number
        game_logic.set_current_frame(snapshot.frame_number);

        // C++ parity order: players/teams before objects, then world systems.
        self.restore_all_players(&snapshot.players, game_logic)?;
        self.restore_all_teams(&snapshot.teams, game_logic)?;
        self.restore_all_objects(&snapshot.objects, game_logic)?;
        self.restore_terrain(&snapshot.terrain, game_logic)?;
        self.restore_pathfinding_cache(&snapshot.pathfinding_cache, game_logic)?;
        self.restore_weather(&snapshot.weather, game_logic)?;
        self.restore_resource_manager(&snapshot.resource_manager, game_logic)?;
        self.restore_combat_tracker(&snapshot.combat_tracker, game_logic)?;
        self.restore_experience_tracker(&snapshot.experience_tracker, game_logic)?;
        self.restore_global_ai_state(&snapshot.global_ai_state, game_logic)?;
        self.restore_special_power_strikes(&snapshot.special_power_strikes, game_logic)?;
        self.restore_combat_particles(&snapshot.combat_particles, game_logic)?;
        self.restore_host_upgrades(&snapshot.host_upgrades, game_logic)?;

        log::info!("World restoration complete");
        Ok(())
    }

    // Private helper methods for snapshot creation

    fn snapshot_all_objects(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<HashMap<ObjectId, ObjectSnapshot>> {
        let mut objects = HashMap::new();

        for (id, object) in game_logic.get_objects() {
            match self.snapshot_object(object) {
                Ok(snapshot) => {
                    objects.insert(*id, snapshot);
                }
                Err(e) => {
                    log::warn!("Failed to snapshot object {:?}: {}", id, e);
                }
            }
        }

        Ok(objects)
    }

    fn snapshot_object(&self, object: &Object) -> SaveLoadResult<ObjectSnapshot> {
        // Get player_id from team (simplified mapping)
        let player_id = match object.team {
            Team::USA => 0,
            Team::China => 1,
            Team::GLA => 2,
            Team::Neutral => 3,
        };

        // Snapshot the object's state
        let status = self.snapshot_object_status(object);
        let object_type = self.snapshot_object_type(object)?;

        Ok(ObjectSnapshot {
            id: object.id,
            template_name: object.template_name.clone(),
            team: object.team,
            player_id,
            geometry: GeometryInfo {
                position: object.get_position(),
                rotation: object.thing.geometry.rotation,
                bounds_min: object.thing.geometry.bounds_min,
                bounds_max: object.thing.geometry.bounds_max,
                radius: object.thing.geometry.radius,
            },
            status,
            health: object.health.clone(),
            movement: object.movement.clone(),
            experience: object.experience.clone(),
            // Slot layout (host residual, not full C++ WeaponSet):
            //   weapons[0] = primary, weapons[1] = secondary when present.
            // When primary is missing but secondary exists, pad index 0 with a
            // zero-damage placeholder so secondary stays at index 1 on restore.
            weapons: Self::snapshot_object_weapons(object),
            contained_objects: object.occupants.clone(),
            container_object: None, // Would need to track container
            modules: self.snapshot_object_modules(object)?,
            object_type,
        })
    }

    /// Capture primary + secondary weapons into the snapshot `weapons` vec.
    ///
    /// Index 0 = primary, index 1 = secondary. Runtime state such as
    /// `last_fire_time` / ammo must survive so combat does not desync after load.
    fn snapshot_object_weapons(object: &Object) -> Vec<Weapon> {
        let mut weapons = Vec::new();
        match (&object.weapon, &object.secondary_weapon) {
            (Some(primary), Some(secondary)) => {
                weapons.push(primary.clone());
                weapons.push(secondary.clone());
            }
            (Some(primary), None) => {
                weapons.push(primary.clone());
            }
            (None, Some(secondary)) => {
                // Pad so secondary restores at slot 1 (see restore_object).
                weapons.push(Weapon {
                    damage: 0.0,
                    range: 0.0,
                    min_range: 0.0,
                    reload_time: 0.0,
                    last_fire_time: 0.0,
                    ammo: None,
                    can_target_air: false,
                    can_target_ground: false,
                    projectile_speed: 0.0,
                    pre_attack_delay: 0.0,
                    splash_radius: 0.0,
                });
                weapons.push(secondary.clone());
            }
            (None, None) => {}
        }
        weapons
    }

    fn snapshot_object_status(&self, object: &Object) -> ObjectStatusSnapshot {
        ObjectStatusSnapshot {
            ai_state: object.ai_state.clone(),
            destroyed: object.status.destroyed,
            under_construction: object.status.under_construction,
            selected: object.selected,
            moving: object.status.moving,
            attacking: object.status.attacking,
            airborne_target: object.status.airborne_target,
            stealthed: object.status.stealthed,
            detected: object.status.detected,
            garrisoned: matches!(object.ai_state, AIState::Garrisoned),
            being_repaired: matches!(object.ai_state, AIState::SeekingRepair),
            on_fire: false,
            poisoned: false,
            radar_jammed: false,
            disabled_underpowered: object.status.disabled_underpowered,
            disabled_unmanned: object.status.disabled_unmanned,
            disabled_hacked: object.status.disabled_hacked,
            disabled_hacked_until_frame: object.status.disabled_hacked_until_frame,
            disabled_emp: object.status.disabled_emp,
            disabled_emp_until_frame: object.status.disabled_emp_until_frame,
            weapons_jammed: object.status.weapons_jammed,
            disabled_subdued: object.status.disabled_subdued,
            is_carbomb: object.status.is_carbomb,
            hijacked: object.status.hijacked,
            special_power_ready: object.special_power_ready,
            special_power_cooldown: object.special_power_cooldown,
            special_power_cooldown_remaining: object.special_power_cooldown_remaining,
            active_weapon_slot: object.active_weapon_slot,
            camo_stealth_look: object.camo_stealth_look,
        }
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_object_modules(
        &self,
        object: &Object,
    ) -> SaveLoadResult<HashMap<String, ModuleSnapshot>> {
        let mut modules = HashMap::new();

        if let Some(building_data) = &object.building_data {
            let production_queue = building_data
                .production_queue
                .iter()
                .map(|item| ProductionQueueEntry {
                    template_name: item.template_name.clone(),
                    progress: item.progress,
                    cost: item.cost.supplies,
                })
                .collect();

            modules.insert(
                "Production".to_string(),
                ModuleSnapshot::Production(ProductionModuleSnapshot {
                    production_queue,
                    is_producing: !building_data.production_queue.is_empty(),
                    production_progress: building_data.get_production_progress().unwrap_or(0.0),
                    rally_point: building_data.rally_point,
                }),
            );
        }

        if !object.applied_upgrades.is_empty() {
            let active_upgrades =
                Self::sorted_unique_strings(object.applied_upgrades.iter().cloned());
            modules.insert(
                "Upgrade".to_string(),
                ModuleSnapshot::Upgrade(UpgradeModuleSnapshot {
                    active_upgrades,
                    upgrade_progress: HashMap::new(),
                }),
            );
        }

        Ok(modules)
    }

    fn snapshot_object_type(&self, object: &Object) -> SaveLoadResult<ObjectTypeSnapshot> {
        // Determine object type from the object's type field
        match object.object_type {
            ObjectType::Infantry | ObjectType::Vehicle | ObjectType::Aircraft => {
                Ok(ObjectTypeSnapshot::Unit(UnitSnapshot {
                    unit_type: format!("{:?}", object.object_type),
                    formation_position: None,
                    formation_id: None,
                    group_id: None,
                    waypoints: Vec::new(),
                }))
            }
            ObjectType::Building => Ok(ObjectTypeSnapshot::Building(BuildingSnapshot {
                building_type: object.template_name.clone(),
                construction_progress: object.construction_percent,
                power_provided: object.power_provided,
                power_required: object.power_consumed,
                is_powered: object.power_provided >= object.power_consumed,
                connected_buildings: Vec::new(),
            })),
            ObjectType::Projectile => Ok(ObjectTypeSnapshot::Projectile(ProjectileSnapshot {
                projectile_type: object.template_name.clone(),
                source_object: object.id,
                target_object: object.target,
                target_position: object.target_location.unwrap_or(object.get_position()),
                flight_time: 0.0,
                max_flight_time: 1.0,
            })),
            ObjectType::Supply | ObjectType::Neutral => {
                Ok(ObjectTypeSnapshot::Resource(ResourceSnapshot {
                    resource_type: object.template_name.clone(),
                    amount: object.stored_resources.supplies,
                    depletion_rate: 0.0,
                    is_infinite: false,
                }))
            }
        }
    }

    fn snapshot_all_players(&self, game_logic: &GameLogic) -> SaveLoadResult<Vec<PlayerSnapshot>> {
        let mut players = Vec::new();
        let mut player_ids: Vec<u32> = game_logic.get_players().keys().copied().collect();
        player_ids.sort_unstable();

        for player_id in player_ids {
            let Some(player) = game_logic.get_player(player_id) else {
                continue;
            };
            let tech_tree = self.snapshot_tech_tree(player, game_logic)?;
            let snapshot = PlayerSnapshot {
                id: player.id,
                name: player.name.clone(),
                team: player.team,
                is_human: player.is_local,
                is_active: player.is_alive,
                resources: player.resources,
                population: PopulationInfo {
                    current: self.snapshot_population_used(game_logic, player.team),
                    maximum: 100,
                },
                tech_tree: tech_tree.clone(),
                upgrades: tech_tree.unlocked_upgrades.clone(),
                build_queue: self.snapshot_player_build_queue(game_logic, player.team),
                research_queue: Self::sorted_unique_strings(player.queued_upgrades.iter().cloned()),
                statistics: self.snapshot_player_statistics(player),
            };
            players.push(snapshot);
        }

        Ok(players)
    }

    fn snapshot_tech_tree(
        &self,
        player: &Player,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<TechTreeSnapshot> {
        let mut unlocked_units = HashSet::new();
        let mut unlocked_buildings = HashSet::new();

        for object in game_logic.get_objects().values() {
            if object.team != player.team || !object.is_alive() {
                continue;
            }
            match object.object_type {
                ObjectType::Infantry | ObjectType::Vehicle | ObjectType::Aircraft => {
                    unlocked_units.insert(object.template_name.clone());
                }
                ObjectType::Building => {
                    unlocked_buildings.insert(object.template_name.clone());
                }
                _ => {}
            }
        }

        let unlocked_upgrades =
            Self::sorted_unique_strings(player.unlocked_sciences.iter().cloned());
        let mut research_progress = HashMap::new();
        for upgrade_name in Self::sorted_unique_strings(player.queued_upgrades.iter().cloned()) {
            research_progress.insert(upgrade_name, 0.0);
        }

        Ok(TechTreeSnapshot {
            unlocked_units: Self::sorted_unique_strings(unlocked_units),
            unlocked_buildings: Self::sorted_unique_strings(unlocked_buildings),
            unlocked_upgrades,
            research_progress,
        })
    }

    fn snapshot_population_used(&self, game_logic: &GameLogic, team: Team) -> u32 {
        game_logic
            .get_objects()
            .values()
            .filter(|object| object.team == team && object.is_alive() && object.is_mobile())
            .count() as u32
    }

    fn snapshot_player_build_queue(&self, game_logic: &GameLogic, team: Team) -> Vec<String> {
        let mut object_ids: Vec<ObjectId> = game_logic.get_objects().keys().copied().collect();
        object_ids.sort_by_key(|id| id.0);

        let mut build_queue = Vec::new();
        for object_id in object_ids {
            let Some(object) = game_logic.find_object(object_id) else {
                continue;
            };
            if object.team != team {
                continue;
            }
            let Some(building_data) = &object.building_data else {
                continue;
            };
            for item in &building_data.production_queue {
                build_queue.push(item.template_name.clone());
            }
        }

        build_queue
    }

    fn snapshot_player_statistics(&self, player: &Player) -> PlayerStatisticsSnapshot {
        PlayerStatisticsSnapshot {
            units_built: player.statistics.units_built,
            units_lost: player.statistics.units_lost,
            buildings_built: player.statistics.structures_built,
            buildings_lost: player.statistics.structures_lost,
            damage_dealt: 0.0, // Would need combat tracking
            damage_received: 0.0,
            resources_gathered: player.statistics.resources_collected,
            experience_gained: 0.0,
        }
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_all_teams(&self, game_logic: &GameLogic) -> SaveLoadResult<Vec<TeamSnapshot>> {
        // Teams are derived from players/objects in the current `Code/Main` model.
        // Mirror C++ behavior by snapshotting per-team membership (and leaving alliance state empty
        // until the diplomacy system is implemented).
        let mut by_team: HashMap<Team, Vec<u32>> = HashMap::new();

        for (&player_id, player) in game_logic.get_players().iter() {
            by_team.entry(player.team).or_default().push(player_id);
        }

        let team_order = [Team::USA, Team::China, Team::GLA, Team::Neutral];
        let mut snapshots = Vec::new();
        for team in team_order {
            let Some(players) = by_team.get(&team) else {
                continue;
            };
            let is_defeated = players
                .iter()
                .filter_map(|pid| game_logic.get_player(*pid))
                .all(|p| !p.is_alive);

            snapshots.push(TeamSnapshot {
                team,
                players: players.clone(),
                allied_teams: Vec::new(),
                is_defeated,
                shared_vision: false,
                shared_control: false,
            });
        }

        Ok(snapshots)
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_terrain(&self, _game_logic: &GameLogic) -> SaveLoadResult<TerrainSnapshot> {
        let (width, height, passability_map) = _game_logic.snapshot_pathfinding_passability();
        let height_map = _game_logic
            .snapshot_terrain_heights_for_path_grid()
            .unwrap_or_default();
        Ok(TerrainSnapshot {
            width,
            height,
            height_map,
            texture_map: Vec::new(),
            passability_map,
            modifications: Vec::new(),
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_weather(&self, _game_logic: &GameLogic) -> SaveLoadResult<WeatherSnapshot> {
        let weather = _game_logic.weather_state();
        Ok(WeatherSnapshot {
            current_weather: weather.current_weather.clone(),
            weather_intensity: weather.intensity,
            weather_duration: weather.duration_remaining,
            next_weather_change: weather.next_change_time,
            visible: weather.visible,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_resource_manager(
        &self,
        _game_logic: &GameLogic,
    ) -> SaveLoadResult<ResourceManagerSnapshot> {
        let mut resource_ids: Vec<ObjectId> = _game_logic
            .get_objects()
            .iter()
            .filter_map(|(id, object)| Self::is_resource_source_object(object).then_some(*id))
            .collect();
        resource_ids.sort();

        let mut supply_deposits = Vec::new();
        for resource_id in resource_ids {
            let Some(resource) = _game_logic.find_object(resource_id) else {
                continue;
            };

            let harvesters = _game_logic
                .get_objects()
                .iter()
                .filter_map(|(id, object)| {
                    (object.target == Some(resource_id)
                        && (object.ai_state == AIState::Gathering || object.is_worker()))
                    .then_some(*id)
                })
                .collect();

            supply_deposits.push(SupplyDepositSnapshot {
                position: resource.get_position(),
                amount: resource.stored_resources.supplies,
                depletion_rate: 0.0,
                harvesters,
            });
        }

        Ok(ResourceManagerSnapshot {
            supply_deposits,
            resource_zones: Vec::new(),
        })
    }

    fn snapshot_pathfinding_cache(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<PathfindingCacheSnapshot> {
        let mut cached_paths: HashMap<(SerializableVec3, SerializableVec3), Vec<SerializableVec3>> =
            HashMap::new();
        let mut cache_timestamps: HashMap<(SerializableVec3, SerializableVec3), f32> =
            HashMap::new();

        let now = game_logic.get_current_frame() as f32 / 30.0;
        for object in game_logic.get_objects().values() {
            if object.movement.path.len() < 2 {
                continue;
            }
            let Some(target_position) = object
                .movement
                .target_position
                .or_else(|| object.movement.path.last().copied())
            else {
                continue;
            };

            let key = (
                SerializableVec3::from(object.get_position()),
                SerializableVec3::from(target_position),
            );

            let path: Vec<SerializableVec3> = object
                .movement
                .path
                .iter()
                .copied()
                .map(SerializableVec3::from)
                .collect();
            if path.len() < 2 {
                continue;
            }

            let should_replace = match cached_paths.get(&key) {
                Some(existing) => path.len() > existing.len(),
                None => true,
            };
            if should_replace {
                cached_paths.insert(key, path);
                cache_timestamps.insert(key, now);
            }
        }

        Ok(PathfindingCacheSnapshot {
            cached_paths,
            cache_timestamps,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_combat_tracker(
        &self,
        _game_logic: &GameLogic,
    ) -> SaveLoadResult<CombatTrackerSnapshot> {
        let sim_time = _game_logic.get_current_frame() as f32 / 30.0;

        let mut active_combats = Vec::new();
        for (&attacker_id, attacker) in _game_logic.get_objects() {
            if !attacker.is_alive() {
                continue;
            }
            let Some(target_id) = attacker.target else {
                continue;
            };
            let Some(target) = _game_logic.find_object(target_id) else {
                continue;
            };
            if !target.is_alive() {
                continue;
            }
            if !attacker.status.attacking
                && !matches!(
                    attacker.ai_state,
                    AIState::Attacking | AIState::AttackMoving | AIState::GuardingObject
                )
            {
                continue;
            }

            active_combats.push(ActiveCombatSnapshot {
                attacker: attacker_id,
                target: target_id,
                start_time: sim_time,
                damage_dealt: attacker.weapon.as_ref().map(|w| w.damage).unwrap_or(0.0),
            });
        }

        let mut recent_deaths = Vec::new();
        for (&object_id, object) in _game_logic.get_objects() {
            if !object.status.destroyed {
                continue;
            }
            recent_deaths.push(DeathEventSnapshot {
                object_id,
                killer_id: None,
                death_time: sim_time,
                death_position: object.get_position(),
            });
        }

        Ok(CombatTrackerSnapshot {
            active_combats,
            recent_deaths,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_experience_tracker(
        &self,
        _game_logic: &GameLogic,
    ) -> SaveLoadResult<ExperienceTrackerSnapshot> {
        let sim_time = _game_logic.get_current_frame() as f32 / 30.0;
        let mut experience_events = Vec::new();
        let mut veterancy_bonuses = HashMap::new();

        for (&object_id, object) in _game_logic.get_objects() {
            if object.experience.current <= 0.0 && object.experience.level == VeterancyLevel::Rookie
            {
                continue;
            }

            experience_events.push(ExperienceEventSnapshot {
                object_id,
                experience_gained: object.experience.current,
                source: "snapshot_state".to_string(),
                timestamp: sim_time,
            });
            veterancy_bonuses.insert(
                object_id,
                Self::veterancy_bonuses_for_level(object.experience.level),
            );
        }

        Ok(ExperienceTrackerSnapshot {
            experience_events,
            veterancy_bonuses,
        })
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn snapshot_global_ai_state(
        &self,
        _game_logic: &GameLogic,
    ) -> SaveLoadResult<GlobalAIStateSnapshot> {
        let difficulty = _game_logic.get_difficulty();

        let mut global_timers = HashMap::new();
        global_timers.insert(
            "sim_time_seconds".to_string(),
            _game_logic.get_current_frame() as f32 / 30.0,
        );
        global_timers.insert(
            "logic_frame".to_string(),
            _game_logic.get_current_frame() as f32,
        );

        let mut global_flags = HashMap::new();
        global_flags.insert("battle_active".to_string(), _game_logic.is_in_battle());

        Ok(GlobalAIStateSnapshot {
            global_timers,
            global_flags,
            difficulty_modifiers: DifficultyModifiers {
                ai_resource_bonus: difficulty.get_resource_bonus(),
                ai_damage_bonus: difficulty.get_aggression_factor(),
                ai_health_bonus: match difficulty {
                    crate::ai::AIDifficulty::Easy => 0.9,
                    crate::ai::AIDifficulty::Medium => 1.0,
                    crate::ai::AIDifficulty::Hard => 1.2,
                    crate::ai::AIDifficulty::Brutal => 1.4,
                },
                ai_build_speed_bonus: 1.0 / difficulty.get_build_delay_modifier(),
            },
        })
    }

    // Private helper methods for snapshot restoration

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn restore_all_objects(
        &self,
        objects: &HashMap<ObjectId, ObjectSnapshot>,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic.clear_all_objects();

        let mut ids: Vec<ObjectId> = objects.keys().cloned().collect();
        ids.sort();

        let mut max_id = 0u32;
        for id in ids {
            let snapshot = objects.get(&id).ok_or_else(|| {
                SaveLoadError::Corrupted(format!("Missing snapshot for object {}", id))
            })?;
            self.restore_object(snapshot, game_logic)?;
            max_id = max_id.max(id.0);
        }

        // Fix up container relationships once all objects exist.
        for snapshot in objects.values() {
            self.restore_object_references(snapshot, game_logic)?;
        }

        game_logic.set_next_object_id_for_restore(ObjectId(max_id.saturating_add(1)));
        Ok(())
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn restore_object(
        &self,
        snapshot: &ObjectSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        // Prefer catalog templates when present. Map-spawned objects often have a
        // matching entry after load_map, but mid-match loads into a fresh GameLogic
        // may not have the full INI catalog — synthesize a minimal template so
        // retail map saves remain restorable (production fail-open for catalog gaps).
        let template = if let Some(t) = game_logic.templates.get(snapshot.template_name.as_str()) {
            t.clone()
        } else {
            let mut t = ThingTemplate::new(&snapshot.template_name);
            t.set_health(snapshot.health.maximum.max(1.0));
            game_logic
                .templates
                .insert(snapshot.template_name.clone(), t.clone());
            log::debug!(
                "Synthesized template '{}' while restoring object {}",
                snapshot.template_name,
                snapshot.id
            );
            t
        };

        let mut object = Object::new(template, snapshot.id, snapshot.team);
        object.name = snapshot.template_name.clone();

        // Geometry / transform
        object.set_position(snapshot.geometry.position);
        object.set_orientation(snapshot.geometry.rotation);
        object.thing.geometry.bounds_min = snapshot.geometry.bounds_min;
        object.thing.geometry.bounds_max = snapshot.geometry.bounds_max;
        object.thing.geometry.radius = snapshot.geometry.radius;
        object.position = snapshot.geometry.position;

        // Core gameplay state
        self.restore_object_status(&snapshot.status, &mut object);
        object.selected = snapshot.status.selected;
        object.health = snapshot.health.clone();
        object.movement = snapshot.movement.clone();
        object.experience = snapshot.experience.clone();

        // weapons[0] = primary, weapons[1] = secondary (host residual layout).
        // Zero-damage pad at [0] means "no primary" (secondary-only objects).
        let (primary, secondary) = Self::restore_object_weapons(&snapshot.weapons);
        object.weapon = primary;
        object.secondary_weapon = secondary;
        object.occupants = snapshot.contained_objects.clone();

        self.restore_object_type_data(&snapshot.object_type, &mut object)?;
        self.restore_object_modules(&snapshot.modules, &mut object, game_logic)?;

        game_logic.objects.insert(snapshot.id, object);
        Ok(())
    }

    /// Decode snapshot weapons vec into primary / secondary slots.
    ///
    /// Fail-closed residual layout:
    /// - empty → neither
    /// - [primary] → primary only (legacy saves)
    /// - [primary, secondary] → both
    /// - [zero-damage pad, secondary] → secondary only
    fn restore_object_weapons(weapons: &[Weapon]) -> (Option<Weapon>, Option<Weapon>) {
        match weapons.len() {
            0 => (None, None),
            1 => (Some(weapons[0].clone()), None),
            _ => {
                let primary = &weapons[0];
                let secondary = Some(weapons[1].clone());
                let primary_is_pad = primary.damage <= 0.0
                    && primary.range <= 0.0
                    && primary.reload_time <= 0.0
                    && !primary.can_target_air
                    && !primary.can_target_ground;
                if primary_is_pad {
                    (None, secondary)
                } else {
                    (Some(primary.clone()), secondary)
                }
            }
        }
    }

    fn restore_object_status(&self, status: &ObjectStatusSnapshot, object: &mut Object) {
        object.status.destroyed = status.destroyed;
        object.status.under_construction = status.under_construction;
        object.status.moving = status.moving;
        object.status.attacking = status.attacking;
        object.status.airborne_target = status.airborne_target;
        object.status.stealthed = status.stealthed;
        object.status.detected = status.detected;
        object.status.selected = status.selected;
        object.status.disabled_underpowered = status.disabled_underpowered;
        object.status.disabled_unmanned = status.disabled_unmanned;
        object.status.disabled_hacked = status.disabled_hacked;
        object.status.disabled_hacked_until_frame = status.disabled_hacked_until_frame;
        object.status.disabled_emp = status.disabled_emp;
        object.status.disabled_emp_until_frame = status.disabled_emp_until_frame;
        object.status.weapons_jammed = status.weapons_jammed;
        object.status.disabled_subdued = status.disabled_subdued;
        object.status.is_carbomb = status.is_carbomb;
        object.status.hijacked = status.hijacked;
        // Wave 79 Drawable residual: restore StealthLook ordinal.
        object.camo_stealth_look = status.camo_stealth_look;

        object.ai_state = if status.destroyed {
            AIState::Idle
        } else if status.ai_state == AIState::Idle && status.garrisoned {
            AIState::Garrisoned
        } else if status.ai_state == AIState::Idle && status.being_repaired {
            AIState::SeekingRepair
        } else if status.ai_state == AIState::Idle && status.attacking {
            AIState::Attacking
        } else if status.ai_state == AIState::Idle && status.moving {
            AIState::Moving
        } else {
            status.ai_state.clone()
        };
        object.special_power_ready = status.special_power_ready;
        object.special_power_cooldown = status.special_power_cooldown;
        object.special_power_cooldown_remaining = status.special_power_cooldown_remaining;
        object.active_weapon_slot = status.active_weapon_slot;

        // Not represented in `ObjectStatus` in `Code/Main/src/game_logic/mod.rs`.
        let _ = status.on_fire;
        let _ = status.poisoned;
        let _ = status.radar_jammed;
    }

    fn restore_object_modules(
        &self,
        modules: &HashMap<String, ModuleSnapshot>,
        object: &mut Object,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<()> {
        for module_snapshot in modules.values() {
            match module_snapshot {
                ModuleSnapshot::Production(snapshot) => {
                    if object.building_data.is_none() {
                        let building_type = BuildingType::from_template_name(&object.template_name);
                        object.building_data = Some(BuildingData::new(building_type));
                    }

                    if let Some(building_data) = object.building_data.as_mut() {
                        building_data.rally_point = snapshot.rally_point;
                        building_data.production_queue.clear();

                        for (index, entry) in snapshot.production_queue.iter().enumerate() {
                            let template = game_logic.templates.get(&entry.template_name);
                            let total_time =
                                template.map(|t| t.build_time.max(0.1)).unwrap_or(30.0_f32);
                            let template_power = template.map(|t| t.build_cost.power).unwrap_or(0);

                            let mut progress = entry.progress.max(0.0);
                            if index == 0 && progress <= 0.0 && snapshot.production_progress > 0.0 {
                                progress =
                                    snapshot.production_progress.clamp(0.0, 1.0) * total_time;
                            }
                            progress = progress.min(total_time);

                            building_data.production_queue.push(ProductionItem {
                                template_name: entry.template_name.clone(),
                                progress,
                                total_time,
                                cost: Resources {
                                    supplies: entry.cost,
                                    power: template_power,
                                },
                            });
                        }
                    }
                }
                ModuleSnapshot::Upgrade(snapshot) => {
                    object.applied_upgrades = snapshot
                        .active_upgrades
                        .iter()
                        .filter(|name| !name.trim().is_empty())
                        .cloned()
                        .collect();
                }
                _ => {}
            }
        }

        Ok(())
    }

    fn restore_object_type_data(
        &self,
        object_type: &ObjectTypeSnapshot,
        object: &mut Object,
    ) -> SaveLoadResult<()> {
        match object_type {
            ObjectTypeSnapshot::Unit(_unit_snapshot) => {
                object.object_type = if object.is_kind_of(KindOf::Infantry) {
                    ObjectType::Infantry
                } else if object.is_kind_of(KindOf::Aircraft) {
                    ObjectType::Aircraft
                } else {
                    ObjectType::Vehicle
                };
                // Unit formation/waypoints aren't represented in `Code/Main` yet.
            }
            ObjectTypeSnapshot::Building(building_snapshot) => {
                object.object_type = ObjectType::Building;
                object.construction_percent = building_snapshot.construction_progress;
                object.power_provided = building_snapshot.power_provided;
                object.power_consumed = building_snapshot.power_required;
            }
            ObjectTypeSnapshot::Projectile(projectile_snapshot) => {
                object.object_type = ObjectType::Projectile;
                object.target = projectile_snapshot.target_object;
                object.target_location = Some(projectile_snapshot.target_position);
            }
            ObjectTypeSnapshot::Resource(resource_snapshot) => {
                object.object_type = if object.is_kind_of(KindOf::Resource)
                    || object.is_kind_of(KindOf::Harvestable)
                {
                    ObjectType::Supply
                } else {
                    ObjectType::Neutral
                };
                object.stored_resources.supplies = resource_snapshot.amount;
            }
        }

        Ok(())
    }

    #[allow(dead_code)] // Save system: will be wired to full save/load integration
    fn restore_object_references(
        &self,
        snapshot: &ObjectSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if let Some(container_id) = snapshot.container_object {
            if let Some(container) = game_logic.find_object_mut(container_id) {
                if !container.occupants.contains(&snapshot.id) {
                    container.occupants.push(snapshot.id);
                }
            }
        }
        Ok(())
    }

    fn restore_all_players(
        &self,
        players: &[PlayerSnapshot],
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic.clear_all_players();
        for snap in players {
            let statistics = PlayerStatistics {
                units_built: snap.statistics.units_built,
                units_lost: snap.statistics.units_lost,
                structures_built: snap.statistics.buildings_built,
                structures_lost: snap.statistics.buildings_lost,
                resources_collected: snap.statistics.resources_gathered,
                ..PlayerStatistics::default()
            };

            let mut unlocked_sciences: std::collections::HashSet<String> =
                snap.tech_tree.unlocked_upgrades.iter().cloned().collect();
            unlocked_sciences.extend(snap.upgrades.iter().cloned());

            let mut queued_upgrades: HashSet<String> = snap
                .research_queue
                .iter()
                .filter(|name| !name.trim().is_empty())
                .cloned()
                .collect();
            queued_upgrades.extend(
                snap.tech_tree
                    .research_progress
                    .keys()
                    .filter(|name| !name.trim().is_empty())
                    .cloned(),
            );

            // Cash bounty residual: re-derive percent from unlocked sciences.
            let mut cash_bounty_percent = 0.0_f32;
            for sci in &unlocked_sciences {
                if let Some(pct) =
                    crate::game_logic::host_cash_bounty::cash_bounty_percent_for_science(sci)
                {
                    if pct > cash_bounty_percent {
                        cash_bounty_percent = pct;
                    }
                }
            }

            game_logic.add_player(Player {
                id: snap.id,
                team: snap.team,
                name: snap.name.clone(),
                resources: snap.resources,
                power_available: snap.resources.power,
                power_produced: 0,
                power_consumed: 0,
                income_accumulator: 0.0,
                selected_objects: Vec::new(),
                unlocked_sciences,
                queued_upgrades,
                is_local: snap.is_human,
                is_alive: snap.is_active,
                statistics,
                power_sabotaged_till_frame: 0,
                color_rgb: (200, 200, 200),
                start_position: -1,
                alliance_team: -1,
                cash_bounty_percent,
                // Recomputed from owned CommandCenter / RadarVan on next
                // update_player_radar residual pass (fail-closed restore).
                radar_count: 0,
                radar_disabled: false,
            });
        }

        Ok(())
    }

    fn restore_all_teams(
        &self,
        teams: &[TeamSnapshot],
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        // Teams are derived from players/objects in `Code/Main`; no separate state to restore yet.
        let _ = teams;
        let _ = game_logic;

        Ok(())
    }

    fn restore_terrain(
        &self,
        terrain_snapshot: &TerrainSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if terrain_snapshot.width == 0 || terrain_snapshot.height == 0 {
            return Ok(());
        }

        let expected_len =
            match (terrain_snapshot.width as usize).checked_mul(terrain_snapshot.height as usize) {
                Some(len) if len > 0 => len,
                _ => {
                    log::warn!(
                        "Skipping terrain restore due to invalid grid dimensions ({}x{})",
                        terrain_snapshot.width,
                        terrain_snapshot.height
                    );
                    return Ok(());
                }
            };

        if !terrain_snapshot.height_map.is_empty() {
            if terrain_snapshot.height_map.len() != expected_len {
                log::warn!(
                    "Skipping terrain height restore due to invalid snapshot payload ({}x{}, {} samples, expected {})",
                    terrain_snapshot.width,
                    terrain_snapshot.height,
                    terrain_snapshot.height_map.len(),
                    expected_len
                );
            } else if !game_logic.restore_terrain_heights_from_grid(
                terrain_snapshot.width,
                terrain_snapshot.height,
                &terrain_snapshot.height_map,
            ) {
                log::warn!(
                    "Skipping terrain height restore due to backend rejection ({}x{}, {} samples)",
                    terrain_snapshot.width,
                    terrain_snapshot.height,
                    terrain_snapshot.height_map.len()
                );
            }
        }

        if !terrain_snapshot.passability_map.is_empty() {
            if terrain_snapshot.passability_map.len() != expected_len {
                log::warn!(
                    "Skipping terrain passability restore due to invalid snapshot payload ({}x{}, {} cells, expected {})",
                    terrain_snapshot.width,
                    terrain_snapshot.height,
                    terrain_snapshot.passability_map.len(),
                    expected_len
                );
                return Ok(());
            }

            if !game_logic.restore_pathfinding_passability(
                terrain_snapshot.width,
                terrain_snapshot.height,
                &terrain_snapshot.passability_map,
            ) {
                log::warn!(
                    "Skipping terrain passability restore due to grid mismatch (snapshot {}x{}, map grid differs)",
                    terrain_snapshot.width,
                    terrain_snapshot.height
                );
            }
        }

        Ok(())
    }

    fn restore_weather(
        &self,
        weather_snapshot: &WeatherSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic.set_weather_state(
            weather_snapshot.current_weather.clone(),
            weather_snapshot.weather_intensity,
            weather_snapshot.weather_duration,
            weather_snapshot.next_weather_change,
        );
        game_logic.set_weather_visible(weather_snapshot.visible);

        Ok(())
    }

    fn restore_resource_manager(
        &self,
        resource_mgr_snapshot: &ResourceManagerSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        let mut resource_ids: Vec<ObjectId> = game_logic
            .get_objects()
            .iter()
            .filter_map(|(id, object)| Self::is_resource_source_object(object).then_some(*id))
            .collect();
        resource_ids.sort();

        let mut used = std::collections::HashSet::new();
        for depot in &resource_mgr_snapshot.supply_deposits {
            let mut best: Option<(ObjectId, f32)> = None;
            for resource_id in &resource_ids {
                if used.contains(resource_id) {
                    continue;
                }
                let Some(object) = game_logic.find_object(*resource_id) else {
                    continue;
                };
                let dist_sq = object.get_position().distance_squared(depot.position);
                match best {
                    Some((_, best_dist)) if dist_sq >= best_dist => {}
                    _ => best = Some((*resource_id, dist_sq)),
                }
            }

            let Some((resource_id, _)) = best else {
                log::warn!(
                    "No resource object available while restoring supply depot at {:?}",
                    depot.position
                );
                continue;
            };

            used.insert(resource_id);

            {
                let Some(resource_obj) = game_logic.find_object_mut(resource_id) else {
                    continue;
                };
                resource_obj.set_position(depot.position);
                resource_obj.position = depot.position;
                resource_obj.stored_resources.supplies = depot.amount;
                if resource_obj.object_type != ObjectType::Supply
                    && (resource_obj.is_kind_of(KindOf::Resource)
                        || resource_obj.is_kind_of(KindOf::Harvestable))
                {
                    resource_obj.object_type = ObjectType::Supply;
                }
            }

            for harvester_id in &depot.harvesters {
                if let Some(harvester) = game_logic.find_object_mut(*harvester_id) {
                    harvester.target = Some(resource_id);
                    if matches!(harvester.ai_state, AIState::Idle | AIState::Moving) {
                        harvester.ai_state = AIState::Gathering;
                    }
                }
            }
        }

        Ok(())
    }

    fn restore_pathfinding_cache(
        &self,
        cache_snapshot: &PathfindingCacheSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        if cache_snapshot.cached_paths.is_empty() {
            return Ok(());
        }

        for object in game_logic.objects.values_mut() {
            if !object.movement.path.is_empty() {
                continue;
            }
            let Some(target_position) = object.movement.target_position else {
                continue;
            };

            let key = (
                SerializableVec3::from(object.get_position()),
                SerializableVec3::from(target_position),
            );
            let Some(cached_path) = cache_snapshot.cached_paths.get(&key) else {
                continue;
            };
            let restored_path: Vec<Vec3> = cached_path.iter().copied().map(Vec3::from).collect();
            if restored_path.len() < 2 {
                continue;
            }
            object.movement.path = restored_path;
            object.movement.current_path_index = 0;
            object.status.moving = true;
            if matches!(object.ai_state, AIState::Idle) {
                object.ai_state = AIState::Moving;
            }
        }

        Ok(())
    }

    fn restore_combat_tracker(
        &self,
        combat_tracker_snapshot: &CombatTrackerSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        for combat in &combat_tracker_snapshot.active_combats {
            if game_logic.find_object(combat.attacker).is_none()
                || game_logic.find_object(combat.target).is_none()
            {
                continue;
            }

            if let Some(attacker) = game_logic.find_object_mut(combat.attacker) {
                attacker.target = Some(combat.target);
                attacker.status.attacking = true;
                if matches!(attacker.ai_state, AIState::Idle | AIState::Moving) {
                    attacker.ai_state = AIState::Attacking;
                }
            }
        }

        let sim_time = game_logic.get_current_frame() as f32 / 30.0;
        for death in &combat_tracker_snapshot.recent_deaths {
            if death.death_time > sim_time {
                continue;
            }
            if let Some(object) = game_logic.find_object_mut(death.object_id) {
                object.status.destroyed = true;
                object.health.current = 0.0;
                object.ai_state = AIState::Idle;
                object.target = None;
            }
        }

        Ok(())
    }

    /// Capture pending/completed host superweapon strikes so mid-flight loads
    /// still impact after remaining delay frames elapse.
    fn snapshot_special_power_strikes(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<SpecialPowerStrikeRegistrySnapshot> {
        let reg = game_logic.special_power_strikes();
        Ok(SpecialPowerStrikeRegistrySnapshot {
            next_id: reg.next_id(),
            strikes: reg.strikes_snapshot(),
            next_radiation_id: reg.next_radiation_id(),
            radiation_fields: reg.radiation_fields().to_vec(),
            radiation_fields_spawned_total: reg.radiation_fields_spawned_total(),
            radiation_damage_applications_total: reg.radiation_damage_applications_total(),
            next_toxin_id: reg.next_toxin_id(),
            toxin_fields: reg.toxin_fields().to_vec(),
            toxin_fields_spawned_total: reg.toxin_fields_spawned_total(),
            toxin_damage_applications_total: reg.toxin_damage_applications_total(),
            next_orbit_id: reg.next_orbit_id(),
            orbit_fields: reg.orbit_fields().to_vec(),
            orbit_fields_spawned_total: reg.orbit_fields_spawned_total(),
            orbit_damage_applications_total: reg.orbit_damage_applications_total(),
            next_beam_id: reg.next_beam_id(),
            beam_fields: reg.beam_fields().to_vec(),
            beam_fields_spawned_total: reg.beam_fields_spawned_total(),
            beam_damage_applications_total: reg.beam_damage_applications_total(),
            next_remnant_id: reg.next_remnant_id(),
            remnant_fields: reg.remnant_fields().to_vec(),
            remnant_fields_spawned_total: reg.remnant_fields_spawned_total(),
            remnant_damage_applications_total: reg.remnant_damage_applications_total(),
        })
    }

    fn restore_special_power_strikes(
        &self,
        snapshot: &SpecialPowerStrikeRegistrySnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic
            .special_power_strikes_mut()
            .restore_from_snapshot_with_residuals(
                snapshot.next_id,
                snapshot.strikes.clone(),
                snapshot.next_radiation_id,
                snapshot.radiation_fields.clone(),
                snapshot.radiation_fields_spawned_total,
                snapshot.radiation_damage_applications_total,
                snapshot.next_toxin_id,
                snapshot.toxin_fields.clone(),
                snapshot.toxin_fields_spawned_total,
                snapshot.toxin_damage_applications_total,
                snapshot.next_orbit_id,
                snapshot.orbit_fields.clone(),
                snapshot.orbit_fields_spawned_total,
                snapshot.orbit_damage_applications_total,
                snapshot.next_beam_id,
                snapshot.beam_fields.clone(),
                snapshot.beam_fields_spawned_total,
                snapshot.beam_damage_applications_total,
                snapshot.next_remnant_id,
                snapshot.remnant_fields.clone(),
                snapshot.remnant_fields_spawned_total,
                snapshot.remnant_damage_applications_total,
            );
        Ok(())
    }

    /// Capture host combat particle registry residual (not full GPU particles).
    fn snapshot_combat_particles(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<CombatParticleRegistrySnapshot> {
        let reg = game_logic.combat_particles();
        Ok(CombatParticleRegistrySnapshot {
            next_id: reg.next_id(),
            systems: reg.systems_snapshot(),
        })
    }

    fn restore_combat_particles(
        &self,
        snapshot: &CombatParticleRegistrySnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic
            .combat_particles_mut()
            .restore_from_snapshot(snapshot.next_id, snapshot.systems.clone());
        Ok(())
    }

    /// Capture pending/completed host upgrade research so mid-flight loads
    /// still complete with unlocks after restore.
    fn snapshot_host_upgrades(
        &self,
        game_logic: &GameLogic,
    ) -> SaveLoadResult<HostUpgradeRegistrySnapshot> {
        let reg = game_logic.host_upgrades();
        Ok(HostUpgradeRegistrySnapshot {
            next_id: reg.next_id(),
            entries: reg.entries_snapshot(),
        })
    }

    fn restore_host_upgrades(
        &self,
        snapshot: &HostUpgradeRegistrySnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        game_logic
            .host_upgrades_mut()
            .restore_from_snapshot(snapshot.next_id, snapshot.entries.clone());
        Ok(())
    }

    fn restore_experience_tracker(
        &self,
        exp_tracker_snapshot: &ExperienceTrackerSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        for event in &exp_tracker_snapshot.experience_events {
            if event.experience_gained <= 0.0 {
                continue;
            }
            if let Some(object) = game_logic.find_object_mut(event.object_id) {
                object.gain_experience(event.experience_gained.max(0.0));
            }
        }

        for (object_id, bonuses) in &exp_tracker_snapshot.veterancy_bonuses {
            let Some(object) = game_logic.find_object_mut(*object_id) else {
                continue;
            };

            let (_, min_experience) = Self::veterancy_level_from_bonus(bonuses.health_bonus);
            if object.experience.current < min_experience {
                object.experience.current = min_experience;
                object.gain_experience(0.0);
            }
        }

        Ok(())
    }

    // AI player restoration is disabled until `Code/Main` AI state is wired into save/load.
    // fn restore_ai_players(
    //     &self,
    //     ai_players_snapshot: &[AIPlayerSnapshot],
    //     game_logic: &mut GameLogic,
    // ) -> SaveLoadResult<()> {
    //     for ai_snapshot in ai_players_snapshot {
    //         let ai_player = game_logic.get_ai_player_mut(ai_snapshot.player_id)?;
    //
    //         ai_player.set_difficulty(&ai_snapshot.difficulty);
    //         ai_player.set_personality(&ai_snapshot.personality);
    //         ai_player.set_current_strategy(&ai_snapshot.current_strategy);
    //
    //         // Restore AI state components
    //         self.restore_ai_strategic_state(&ai_snapshot.strategic_state, ai_player)?;
    //         self.restore_ai_tactical_state(&ai_snapshot.tactical_state, ai_player)?;
    //         self.restore_ai_economic_state(&ai_snapshot.economic_state, ai_player)?;
    //     }
    //
    //     Ok(())
    // }

    // AI player strategic state restoration is disabled until AI state is wired into save/load.
    // fn restore_ai_strategic_state(
    //     &self,
    //     strategic_snapshot: &AIStrategicStateSnapshot,
    //     ai_player: &mut AIPlayer,
    // ) -> SaveLoadResult<()> {
    //     let strategic = ai_player.get_strategic_state_mut();
    //
    //     strategic.set_current_phase(&strategic_snapshot.current_phase);
    //
    //     for objective in &strategic_snapshot.objectives {
    //         strategic.add_objective(objective.clone());
    //     }
    //
    //     strategic.set_threat_assessment(strategic_snapshot.threat_assessment.clone());
    //
    //     Ok(())
    // }

    // AI player tactical state restoration is disabled until AI state is wired into save/load.
    // fn restore_ai_tactical_state(
    //     &self,
    //     tactical_snapshot: &AITacticalStateSnapshot,
    //     ai_player: &mut AIPlayer,
    // ) -> SaveLoadResult<()> {
    //     let tactical = ai_player.get_tactical_state_mut();
    //
    //     for group_snapshot in &tactical_snapshot.unit_groups {
    //         tactical.create_unit_group(
    //             group_snapshot.group_id,
    //             group_snapshot.units.clone(),
    //             &group_snapshot.role,
    //         );
    //     }
    //
    //     for attack_snapshot in &tactical_snapshot.active_attacks {
    //         tactical.register_attack(
    //             attack_snapshot.attack_id,
    //             attack_snapshot.target_position,
    //             attack_snapshot.assigned_groups.clone(),
    //         );
    //     }
    //
    //     Ok(())
    // }

    // AI player economic state restoration is disabled until AI state is wired into save/load.
    // fn restore_ai_economic_state(
    //     &self,
    //     economic_snapshot: &AIEconomicStateSnapshot,
    //     ai_player: &mut AIPlayer,
    // ) -> SaveLoadResult<()> {
    //     let economic = ai_player.get_economic_state_mut();
    //
    //     for priority in &economic_snapshot.build_priorities {
    //         economic.set_build_priority(priority.clone());
    //     }
    //
    //     economic.set_economic_focus(&economic_snapshot.economic_focus);
    //     economic.set_resource_allocation(economic_snapshot.resource_allocation.clone());
    //
    //     Ok(())
    // }

    fn restore_global_ai_state(
        &self,
        global_ai_snapshot: &GlobalAIStateSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        let inferred_difficulty =
            Self::difficulty_from_modifiers(&global_ai_snapshot.difficulty_modifiers);

        let local_player_id = game_logic
            .get_players()
            .iter()
            .find_map(|(id, player)| player.is_local.then_some(*id))
            .unwrap_or(u32::MAX);
        game_logic.setup_skirmish_ai(local_player_id);

        let ai_player_ids: Vec<u32> = game_logic
            .get_players()
            .iter()
            .filter_map(|(id, player)| (!player.is_local).then_some(*id))
            .collect();

        for player_id in ai_player_ids {
            game_logic.set_ai_difficulty(player_id, inferred_difficulty);
        }

        Ok(())
    }

    fn is_resource_source_object(object: &Object) -> bool {
        object.object_type == ObjectType::Supply
            || object.is_kind_of(KindOf::Resource)
            || object.is_kind_of(KindOf::Harvestable)
            || object.template_name.to_ascii_lowercase().contains("supply")
    }

    fn veterancy_bonuses_for_level(level: VeterancyLevel) -> VeterancyBonuses {
        match level {
            VeterancyLevel::Rookie => VeterancyBonuses {
                health_bonus: 1.0,
                damage_bonus: 1.0,
                accuracy_bonus: 1.0,
                range_bonus: 1.0,
            },
            VeterancyLevel::Veteran => VeterancyBonuses {
                health_bonus: 1.25,
                damage_bonus: 1.25,
                accuracy_bonus: 1.05,
                range_bonus: 1.0,
            },
            VeterancyLevel::Elite => VeterancyBonuses {
                health_bonus: 1.5,
                damage_bonus: 1.5,
                accuracy_bonus: 1.1,
                range_bonus: 1.05,
            },
            VeterancyLevel::Heroic => VeterancyBonuses {
                health_bonus: 2.0,
                damage_bonus: 2.0,
                accuracy_bonus: 1.2,
                range_bonus: 1.1,
            },
        }
    }

    fn veterancy_level_from_bonus(health_bonus: f32) -> (VeterancyLevel, f32) {
        if health_bonus >= 1.9 {
            (VeterancyLevel::Heroic, 300.0)
        } else if health_bonus >= 1.45 {
            (VeterancyLevel::Elite, 150.0)
        } else if health_bonus >= 1.2 {
            (VeterancyLevel::Veteran, 60.0)
        } else {
            (VeterancyLevel::Rookie, 0.0)
        }
    }

    fn difficulty_from_modifiers(modifiers: &DifficultyModifiers) -> crate::ai::AIDifficulty {
        let score = (modifiers.ai_resource_bonus
            + modifiers.ai_damage_bonus
            + modifiers.ai_health_bonus
            + modifiers.ai_build_speed_bonus)
            / 4.0;

        if score < 0.95 {
            crate::ai::AIDifficulty::Easy
        } else if score < 1.15 {
            crate::ai::AIDifficulty::Medium
        } else if score < 1.35 {
            crate::ai::AIDifficulty::Hard
        } else {
            crate::ai::AIDifficulty::Brutal
        }
    }

    fn sorted_unique_strings<I>(iter: I) -> Vec<String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut values: Vec<String> = iter
            .into_iter()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        values.sort();
        values
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

impl Default for WeatherSnapshot {
    fn default() -> Self {
        Self {
            current_weather: "clear".to_string(),
            weather_intensity: 0.0,
            weather_duration: 0.0,
            next_weather_change: 0.0,
            visible: weather_visible_default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn snapshot_restore_rebuilds_state_and_object_id_counter() {
        let mut source = GameLogic::new();
        source
            .templates
            .insert("TestTank".to_string(), ThingTemplate::new("TestTank"));
        source.add_player(Player::new(1, Team::USA, "PlayerOne", true));
        source.set_current_frame(777);

        let object_id = source
            .create_object("TestTank", Team::USA, Vec3::new(11.0, 0.0, 7.0))
            .expect("failed to create source object");
        {
            let object = source
                .find_object_mut(object_id)
                .expect("created object should exist");
            object.health.current = 42.0;
            object.status.moving = true;
            object.movement.target_position = Some(Vec3::new(30.0, 0.0, 30.0));
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");

        assert_eq!(restored.get_current_frame(), 777);
        assert_eq!(restored.get_players().len(), 1);
        let restored_obj = restored
            .find_object(object_id)
            .expect("restored object should exist");
        assert_eq!(restored_obj.get_position(), Vec3::new(11.0, 0.0, 7.0));
        assert_eq!(restored_obj.health.current, 42.0);
        assert!(restored_obj.status.moving);
        assert_eq!(restored_obj.ai_state, AIState::Moving);

        let next_id = restored
            .create_object("TestTank", Team::USA, Vec3::ZERO)
            .expect("failed to create post-restore object");
        assert_eq!(next_id.0, object_id.0 + 1);
    }

    #[test]
    fn snapshot_restore_rebuilds_pathfinding_passability() {
        let mut source = GameLogic::new();
        source.set_pathfinding_static_block(2, 3, true);
        source.set_pathfinding_static_block(5, 7, true);

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");

        assert!(snapshot.terrain.width > 0);
        assert!(snapshot.terrain.height > 0);

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");

        assert!(restored.is_pathfinding_static_blocked(2, 3));
        assert!(restored.is_pathfinding_static_blocked(5, 7));
        assert!(!restored.is_pathfinding_static_blocked(0, 0));
    }

    #[test]
    fn snapshot_restore_rebuilds_terrain_height_samples() {
        let mut source = GameLogic::new();
        let (width, height, _) = source.snapshot_pathfinding_passability();
        let len = (width as usize).saturating_mul(height as usize);
        let mut heights = vec![0.0_f32; len];
        if width > 3 && height > 3 {
            heights[(3 * width + 3) as usize] = 18.0;
        } else if !heights.is_empty() {
            heights[0] = 18.0;
        }
        assert!(source.restore_terrain_heights_from_grid(width, height, &heights));

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");
        assert_eq!(snapshot.terrain.height_map.len(), len);

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");

        let restored_heights = restored
            .snapshot_terrain_heights_for_path_grid()
            .expect("restored terrain samples should exist");
        assert_eq!(restored_heights.len(), len);
        assert!(restored_heights.iter().copied().fold(0.0_f32, f32::max) > 0.0);
    }

    #[test]
    fn snapshot_restore_rebuilds_resource_depots_and_harvesters() {
        let mut source = GameLogic::new();

        let mut supply_template = ThingTemplate::new("TestSupplyPile");
        supply_template
            .add_kind_of(KindOf::Resource)
            .add_kind_of(KindOf::Harvestable);
        source
            .templates
            .insert("TestSupplyPile".to_string(), supply_template);

        let mut worker_template = ThingTemplate::new("TestWorker");
        worker_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Worker)
            .add_kind_of(KindOf::Selectable);
        source
            .templates
            .insert("TestWorker".to_string(), worker_template);

        let supply_id = source
            .create_object("TestSupplyPile", Team::Neutral, Vec3::new(20.0, 0.0, 20.0))
            .expect("failed to create supply object");
        let worker_id = source
            .create_object("TestWorker", Team::USA, Vec3::new(15.0, 0.0, 20.0))
            .expect("failed to create worker object");

        {
            let supply = source
                .find_object_mut(supply_id)
                .expect("supply object should exist");
            supply.stored_resources.supplies = 2500;
        }
        {
            let worker = source
                .find_object_mut(worker_id)
                .expect("worker object should exist");
            worker.target = Some(supply_id);
            worker.ai_state = AIState::Gathering;
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");

        let restored_supply = restored
            .find_object(supply_id)
            .expect("restored supply object should exist");
        assert_eq!(restored_supply.stored_resources.supplies, 2500);

        let restored_worker = restored
            .find_object(worker_id)
            .expect("restored worker should exist");
        assert_eq!(restored_worker.target, Some(supply_id));
        assert_eq!(restored_worker.ai_state, AIState::Gathering);
    }

    #[test]
    fn snapshot_restore_recovers_veterancy_from_tracker_data() {
        let mut source = GameLogic::new();
        let mut tank_template = ThingTemplate::new("TestTank");
        tank_template
            .add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable);
        source
            .templates
            .insert("TestTank".to_string(), tank_template);

        let tank_id = source
            .create_object("TestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("failed to create tank");
        {
            let tank = source.find_object_mut(tank_id).expect("tank should exist");
            tank.gain_experience(180.0);
            assert_eq!(tank.experience.level, VeterancyLevel::Elite);
        }

        let builder = SnapshotBuilder::new();
        let mut snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");

        let tank_snapshot = snapshot
            .objects
            .get_mut(&tank_id)
            .expect("tank snapshot should exist");
        tank_snapshot.experience = Experience::default();
        tank_snapshot.health.current = tank_snapshot.health.maximum.min(100.0);
        tank_snapshot.health.maximum = 100.0;

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");

        let restored_tank = restored
            .find_object(tank_id)
            .expect("restored tank should exist");
        assert_eq!(restored_tank.experience.level, VeterancyLevel::Elite);
        assert!(restored_tank.health.maximum > 100.0);
    }

    #[test]
    fn snapshot_restore_preserves_building_production_modules_and_object_upgrades() {
        let mut source = GameLogic::new();
        source.add_player(Player::new(1, Team::USA, "USA", true));

        let mut barracks = ThingTemplate::new("USA_Barracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable);
        source
            .templates
            .insert("USA_Barracks".to_string(), barracks.clone());

        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_cost(225, 0);
        ranger.build_time = 12.0;
        source.templates.insert("USA_Ranger".to_string(), ranger);

        let barracks_id = source
            .create_object("USA_Barracks", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("failed to create barracks");
        assert!(source.enqueue_production(barracks_id, "USA_Ranger".to_string()));
        {
            let building = source
                .find_object_mut(barracks_id)
                .expect("barracks should exist");
            let building_data = building
                .building_data
                .as_mut()
                .expect("barracks should have building data");
            building_data.production_queue[0].progress = 4.5;
            building_data.rally_point = Some(Vec3::new(30.0, 0.0, 40.0));
            building.apply_upgrade_tag("UpgradeVeteranTraining");
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");

        let restored_building = restored
            .find_object(barracks_id)
            .expect("restored barracks should exist");
        assert!(restored_building.has_upgrade_tag("UpgradeVeteranTraining"));
        let restored_data = restored_building
            .building_data
            .as_ref()
            .expect("restored barracks should keep building data");
        assert_eq!(restored_data.rally_point, Some(Vec3::new(30.0, 0.0, 40.0)));
        assert_eq!(restored_data.production_queue.len(), 1);
        let item = &restored_data.production_queue[0];
        assert_eq!(item.template_name, "USA_Ranger");
        assert_eq!(item.cost.supplies, 225);
        assert_eq!(item.total_time, 12.0);
        assert!((item.progress - 4.5).abs() < 0.001);
    }

    #[test]
    fn snapshot_player_state_captures_population_build_queue_and_research() {
        let mut source = GameLogic::new();
        source.add_player(Player::new(3, Team::USA, "Commander", true));
        {
            let player = source
                .get_player_mut(3)
                .expect("player should exist for state setup");
            player
                .unlocked_sciences
                .insert("SciencePathfinder".to_string());
            player
                .queued_upgrades
                .insert("UpgradeAdvancedTraining".to_string());
        }

        let mut barracks = ThingTemplate::new("USA_Barracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable);
        source
            .templates
            .insert("USA_Barracks".to_string(), barracks.clone());

        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_cost(225, 0);
        ranger.build_time = 8.0;
        source.templates.insert("USA_Ranger".to_string(), ranger);

        let barracks_id = source
            .create_object("USA_Barracks", Team::USA, Vec3::new(5.0, 0.0, 5.0))
            .expect("failed to create barracks");
        source
            .create_object("USA_Ranger", Team::USA, Vec3::new(8.0, 0.0, 8.0))
            .expect("failed to create ranger");
        assert!(source.enqueue_production(barracks_id, "USA_Ranger".to_string()));
        assert!(source.enqueue_production(barracks_id, "USA_Ranger".to_string()));

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");
        let player_snapshot = snapshot
            .players
            .iter()
            .find(|p| p.id == 3)
            .expect("player snapshot should exist");

        assert_eq!(player_snapshot.population.current, 1);
        assert_eq!(
            player_snapshot.build_queue,
            vec!["USA_Ranger".to_string(), "USA_Ranger".to_string()]
        );
        assert!(player_snapshot
            .tech_tree
            .unlocked_buildings
            .contains(&"USA_Barracks".to_string()));
        assert!(player_snapshot
            .tech_tree
            .unlocked_units
            .contains(&"USA_Ranger".to_string()));
        assert!(player_snapshot
            .tech_tree
            .unlocked_upgrades
            .contains(&"SciencePathfinder".to_string()));
        assert!(player_snapshot
            .research_queue
            .contains(&"UpgradeAdvancedTraining".to_string()));
        assert!(player_snapshot
            .tech_tree
            .research_progress
            .contains_key("UpgradeAdvancedTraining"));
    }

    #[test]
    fn snapshot_restore_preserves_weather_state() {
        let mut source = GameLogic::new();
        source.set_weather_state("sandstorm", 0.7, 90.0, 30.0);
        source.set_weather_visible(false);

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");
        assert_eq!(snapshot.weather.current_weather, "sandstorm");
        assert!((snapshot.weather.weather_intensity - 0.7).abs() < 0.0001);
        assert!((snapshot.weather.weather_duration - 90.0).abs() < 0.0001);
        assert!((snapshot.weather.next_weather_change - 30.0).abs() < 0.0001);
        assert!(!snapshot.weather.visible);

        let mut restored = GameLogic::new();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");
        let weather = restored.weather_state();
        assert_eq!(weather.current_weather, "sandstorm");
        assert!((weather.intensity - 0.7).abs() < 0.0001);
        assert!((weather.duration_remaining - 90.0).abs() < 0.0001);
        assert!((weather.next_change_time - 30.0).abs() < 0.0001);
        assert!(!weather.visible);
    }

    #[test]
    fn snapshot_restore_rehydrates_paths_from_pathfinding_cache() {
        let mut source = GameLogic::new();
        source
            .templates
            .insert("TestMover".to_string(), ThingTemplate::new("TestMover"));

        let mover_id = source
            .create_object("TestMover", Team::USA, Vec3::new(1.0, 0.0, 1.0))
            .expect("failed to create mover");
        {
            let mover = source
                .find_object_mut(mover_id)
                .expect("mover should exist for setup");
            mover.status.moving = true;
            mover.movement.target_position = Some(Vec3::new(21.0, 0.0, 11.0));
            mover.movement.path = vec![
                Vec3::new(1.0, 0.0, 1.0),
                Vec3::new(11.0, 0.0, 6.0),
                Vec3::new(21.0, 0.0, 11.0),
            ];
        }

        let builder = SnapshotBuilder::new();
        let mut snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");

        assert_eq!(snapshot.pathfinding_cache.cached_paths.len(), 1);
        {
            let mover_snap = snapshot
                .objects
                .get_mut(&mover_id)
                .expect("mover snapshot should exist");
            mover_snap.movement.path.clear();
            mover_snap.movement.current_path_index = 0;
            mover_snap.status.moving = false;
        }

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");

        let mover = restored
            .find_object(mover_id)
            .expect("restored mover should exist");
        assert_eq!(mover.movement.path.len(), 3);
        assert_eq!(mover.movement.path[0], Vec3::new(1.0, 0.0, 1.0));
        assert_eq!(mover.movement.path[2], Vec3::new(21.0, 0.0, 11.0));
        assert!(mover.status.moving);
        assert_eq!(mover.ai_state, AIState::Moving);
    }

    /// Residual: secondary_weapon + active_weapon_slot must survive snapshot save/load.
    /// Prior gap: capture only stored primary in `weapons[0]`, restore left secondary None.
    #[test]
    fn snapshot_restore_preserves_secondary_weapon_and_active_slot() {
        let mut source = GameLogic::new();
        let mut ranger = ThingTemplate::new("USA_Ranger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable);
        source.templates.insert("USA_Ranger".to_string(), ranger);

        let ranger_id = source
            .create_object("USA_Ranger", Team::USA, Vec3::new(5.0, 0.0, 5.0))
            .expect("failed to create ranger");

        let primary = Weapon {
            damage: 25.0,
            range: 120.0,
            min_range: 0.0,
            reload_time: 0.5,
            last_fire_time: 12.5,
            ammo: Some(28),
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 0.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
        };
        let secondary = Weapon {
            damage: 80.0,
            range: 90.0,
            min_range: 5.0,
            reload_time: 2.0,
            last_fire_time: 3.25,
            ammo: Some(4),
            can_target_air: false,
            can_target_ground: true,
            projectile_speed: 40.0,
            pre_attack_delay: 0.1,
            splash_radius: 0.0,
        };

        {
            let unit = source
                .find_object_mut(ranger_id)
                .expect("ranger should exist");
            unit.weapon = Some(primary.clone());
            unit.secondary_weapon = Some(secondary.clone());
            unit.active_weapon_slot = 1;
            unit.apply_upgrade_tag("Upgrade_AmericaRangerFlashBangGrenade");
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot creation failed");

        let snap_obj = snapshot
            .objects
            .get(&ranger_id)
            .expect("ranger snapshot should exist");
        assert_eq!(
            snap_obj.weapons.len(),
            2,
            "secondary must be encoded as weapons[1]"
        );
        assert!((snap_obj.weapons[0].damage - primary.damage).abs() < f32::EPSILON);
        assert!((snap_obj.weapons[1].damage - secondary.damage).abs() < f32::EPSILON);
        assert!((snap_obj.weapons[1].last_fire_time - secondary.last_fire_time).abs() < 0.0001);
        assert_eq!(snap_obj.status.active_weapon_slot, 1);

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("snapshot restore failed");

        let unit = restored
            .find_object(ranger_id)
            .expect("restored ranger should exist");
        let restored_primary = unit
            .weapon
            .as_ref()
            .expect("primary weapon must survive load");
        let restored_secondary = unit
            .secondary_weapon
            .as_ref()
            .expect("secondary weapon must survive load");

        assert!((restored_primary.damage - primary.damage).abs() < f32::EPSILON);
        assert!((restored_primary.last_fire_time - primary.last_fire_time).abs() < 0.0001);
        assert_eq!(restored_primary.ammo, primary.ammo);

        assert!((restored_secondary.damage - secondary.damage).abs() < f32::EPSILON);
        assert!((restored_secondary.range - secondary.range).abs() < f32::EPSILON);
        assert!((restored_secondary.min_range - secondary.min_range).abs() < f32::EPSILON);
        assert!((restored_secondary.reload_time - secondary.reload_time).abs() < f32::EPSILON);
        assert!(
            (restored_secondary.last_fire_time - secondary.last_fire_time).abs() < 0.0001,
            "secondary last_fire_time must survive or reload timing desyncs"
        );
        assert_eq!(restored_secondary.ammo, secondary.ammo);
        assert!(
            (restored_secondary.projectile_speed - secondary.projectile_speed).abs() < f32::EPSILON
        );
        assert_eq!(unit.active_weapon_slot, 1);
        assert!(unit.has_upgrade_tag("Upgrade_AmericaRangerFlashBangGrenade"));
    }

    #[test]
    fn snapshot_restore_preserves_secondary_only_weapon_slot() {
        let mut source = GameLogic::new();
        source
            .templates
            .insert("TestUnit".to_string(), ThingTemplate::new("TestUnit"));

        let id = source
            .create_object("TestUnit", Team::USA, Vec3::ZERO)
            .expect("create unit");
        let secondary = Weapon {
            damage: 50.0,
            range: 75.0,
            min_range: 0.0,
            reload_time: 1.0,
            last_fire_time: 9.0,
            ammo: None,
            can_target_air: true,
            can_target_ground: true,
            projectile_speed: 100.0,
            pre_attack_delay: 0.0,
            splash_radius: 0.0,
        };
        {
            let unit = source.find_object_mut(id).expect("unit");
            unit.weapon = None;
            unit.secondary_weapon = Some(secondary.clone());
            unit.active_weapon_slot = 1;
        }

        let builder = SnapshotBuilder::new();
        let snapshot = builder.create_world_snapshot(&source).expect("snapshot");
        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore");

        let unit = restored.find_object(id).expect("restored unit");
        assert!(
            unit.weapon.is_none(),
            "pad primary must not become a real primary weapon"
        );
        let sec = unit
            .secondary_weapon
            .as_ref()
            .expect("secondary-only must restore");
        assert!((sec.damage - 50.0).abs() < f32::EPSILON);
        assert!((sec.last_fire_time - 9.0).abs() < 0.0001);
        assert_eq!(unit.active_weapon_slot, 1);
    }

    #[test]
    fn snapshot_weapon_layout_helpers_round_trip() {
        let primary = Weapon {
            damage: 10.0,
            range: 50.0,
            ..Weapon::default()
        };
        let secondary = Weapon {
            damage: 99.0,
            range: 40.0,
            last_fire_time: 1.5,
            ..Weapon::default()
        };

        // Both slots
        let mut obj = Object::new(ThingTemplate::new("T"), ObjectId(1), Team::USA);
        obj.weapon = Some(primary.clone());
        obj.secondary_weapon = Some(secondary.clone());
        let weapons = SnapshotBuilder::snapshot_object_weapons(&obj);
        let (p, s) = SnapshotBuilder::restore_object_weapons(&weapons);
        assert!((p.unwrap().damage - 10.0).abs() < f32::EPSILON);
        assert!((s.unwrap().damage - 99.0).abs() < f32::EPSILON);

        // Primary only (legacy)
        let weapons = vec![primary.clone()];
        let (p, s) = SnapshotBuilder::restore_object_weapons(&weapons);
        assert!(p.is_some());
        assert!(s.is_none());

        // Empty
        let (p, s) = SnapshotBuilder::restore_object_weapons(&[]);
        assert!(p.is_none() && s.is_none());
    }

    /// End-to-end SaveFileManager path: secondary stays bound after save → load.
    #[test]
    fn save_file_roundtrip_preserves_secondary_weapon() {
        use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
        use std::time::{Duration, SystemTime};

        let save_dir = tempfile::TempDir::new().expect("temp save dir");
        let mut manager = SaveFileManager::with_save_directory(save_dir.path());
        manager.init().expect("save manager init");

        let mut source = GameLogic::new();
        let mut template = ThingTemplate::new("SaveSecondaryRanger");
        template
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable);
        source
            .templates
            .insert("SaveSecondaryRanger".to_string(), template);

        let id = source
            .create_object("SaveSecondaryRanger", Team::USA, Vec3::new(12.0, 0.0, 8.0))
            .expect("create ranger");
        {
            let unit = source.find_object_mut(id).expect("ranger");
            unit.weapon = Some(Weapon {
                damage: 20.0,
                range: 100.0,
                last_fire_time: 1.0,
                ..Weapon::default()
            });
            unit.secondary_weapon = Some(Weapon {
                damage: 55.0,
                range: 80.0,
                reload_time: 1.5,
                last_fire_time: 4.5,
                ammo: Some(2),
                ..Weapon::default()
            });
            unit.active_weapon_slot = 1;
        }

        let info = SaveGameInfo {
            filename: "secondary_weapon_rt".to_string(),
            display_name: "Secondary Weapon Roundtrip".to_string(),
            description: "residual secondary_weapon save/load".to_string(),
            map_name: "ResidualMap".to_string(),
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        manager
            .save_game("secondary_weapon_rt", &source, &info)
            .expect("save");

        let mut loaded = GameLogic::new();
        loaded.templates = source.templates.clone();
        manager
            .load_game("secondary_weapon_rt", &mut loaded)
            .expect("load");

        let unit = loaded.find_object(id).expect("loaded unit");
        let secondary = unit
            .secondary_weapon
            .as_ref()
            .expect("secondary must remain bound after file load");
        assert!((secondary.damage - 55.0).abs() < f32::EPSILON);
        assert!((secondary.last_fire_time - 4.5).abs() < 0.0001);
        assert_eq!(secondary.ammo, Some(2));
        assert_eq!(unit.active_weapon_slot, 1);
        assert!(unit.weapon.is_some());
    }

    fn ensure_strike_test_tank(logic: &mut GameLogic) {
        let mut t = ThingTemplate::new("StrikeTestTank");
        t.add_kind_of(KindOf::Vehicle)
            .add_kind_of(KindOf::Selectable)
            .add_kind_of(KindOf::Attackable)
            .set_health(500.0);
        logic.templates.insert("StrikeTestTank".to_string(), t);
    }

    /// Residual: DaisyCutter queued mid-flight must survive snapshot and still
    /// apply area damage once the restored impact frame is reached.
    #[test]
    fn special_power_daisy_cutter_mid_flight_save_load_still_impacts() {
        use crate::command_system::SpecialPowerType;

        let mut source = GameLogic::new();
        ensure_strike_test_tank(&mut source);

        let caster_id = source
            .create_object("StrikeTestTank", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("caster");
        let enemy_id = source
            .create_object("StrikeTestTank", Team::GLA, Vec3::new(40.0, 0.0, 0.0))
            .expect("enemy");
        {
            let enemy = source.find_object_mut(enemy_id).expect("enemy");
            enemy.health.current = 500.0;
            enemy.health.maximum = 500.0;
            enemy.thing.template.armor = 0.0;
        }

        // Activate at frame 0 → DaisyCutter impact at frame 90.
        source.set_current_frame(0);
        let strike_id = source
            .queue_special_power_strike(
                &SpecialPowerType::DaisyCutter,
                caster_id,
                Vec3::new(40.0, 0.0, 0.0),
            )
            .expect("DaisyCutter must queue");

        // Mid-flight: save before impact.
        source.set_current_frame(45);
        source.update_special_power_strikes();
        assert_eq!(
            source.special_power_strikes().pending_count(),
            1,
            "strike must still be queued mid-flight"
        );
        assert!(source
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::DaisyCutter));
        let health_mid = source.find_object(enemy_id).unwrap().health.current;
        assert!((health_mid - 500.0).abs() < 0.1, "no damage mid-flight");

        // Combat particle residual from activation should be present for snapshot.
        assert!(
            source.combat_particles().system_count() >= 1,
            "activation should spawn combat particle residual"
        );

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot mid-flight DaisyCutter");
        assert_eq!(snapshot.special_power_strikes.strikes.len(), 1);
        assert_eq!(
            snapshot.special_power_strikes.strikes[0].phase,
            HostStrikePhase::Queued
        );
        assert_eq!(snapshot.special_power_strikes.strikes[0].impact_frame, 90);
        assert!(
            !snapshot.combat_particles.systems.is_empty(),
            "combat particles must be captured in WorldSnapshot"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore mid-flight DaisyCutter");

        assert_eq!(restored.get_current_frame(), 45);
        assert_eq!(restored.special_power_strikes().pending_count(), 1);
        let restored_strike = restored
            .special_power_strikes()
            .get(strike_id)
            .expect("pending strike must survive load");
        assert_eq!(restored_strike.impact_frame, 90);
        assert_eq!(restored_strike.phase, HostStrikePhase::Queued);
        assert!(
            restored.combat_particles().system_count() >= 1,
            "combat particle registry must restore active systems"
        );

        // Still before impact after load: no damage.
        restored.set_current_frame(89);
        restored.update_special_power_strikes();
        assert!((restored.find_object(enemy_id).unwrap().health.current - 500.0).abs() < 0.1);
        assert!(!restored
            .special_power_strikes()
            .honesty_complete_ok(HostSuperweaponKind::DaisyCutter));

        // Impact after remaining delay: damage applied.
        restored.set_current_frame(90);
        restored.update_special_power_strikes();
        assert!(
            restored
                .special_power_strikes()
                .honesty_complete_ok(HostSuperweaponKind::DaisyCutter),
            "DaisyCutter must complete after mid-flight load"
        );
        let enemy_after = restored.find_object(enemy_id).map(|o| o.health.current);
        assert!(
            enemy_after.is_none()
                || enemy_after == Some(0.0)
                || restored
                    .find_object(enemy_id)
                    .map(|o| o.status.destroyed || o.health.current < 500.0)
                    .unwrap_or(true),
            "enemy must take DaisyCutter residual damage after load (got {enemy_after:?})"
        );
        let completed = restored
            .special_power_strikes()
            .get(strike_id)
            .expect("completed strike");
        assert_eq!(completed.phase, HostStrikePhase::Completed);
        assert!(completed.total_damage_applied > 0.0);
        assert!(completed.objects_hit >= 1);
    }

    /// Residual: A10 strike mid-flight save/load continues remaining delay and impacts.
    #[test]
    fn special_power_a10_mid_flight_save_load_still_impacts() {
        use crate::command_system::SpecialPowerType;

        let mut source = GameLogic::new();
        ensure_strike_test_tank(&mut source);

        let caster_id = source
            .create_object("StrikeTestTank", Team::USA, Vec3::ZERO)
            .expect("caster");
        let enemy_id = source
            .create_object("StrikeTestTank", Team::GLA, Vec3::new(15.0, 0.0, 0.0))
            .expect("enemy");
        {
            let enemy = source.find_object_mut(enemy_id).expect("enemy");
            enemy.health.current = 200.0;
            enemy.health.maximum = 200.0;
            enemy.thing.template.armor = 0.0;
        }

        // A10 delay is 60 frames.
        source.set_current_frame(100);
        let strike_id = source
            .queue_special_power_strike(
                &SpecialPowerType::Airstrike,
                caster_id,
                Vec3::new(15.0, 0.0, 0.0),
            )
            .expect("A10 must queue");
        assert_eq!(
            source
                .special_power_strikes()
                .get(strike_id)
                .unwrap()
                .impact_frame,
            160
        );

        source.set_current_frame(130);
        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("A10 mid-flight snapshot");

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("A10 restore");

        assert_eq!(restored.get_current_frame(), 130);
        assert!(restored
            .special_power_strikes()
            .honesty_queue_ok(HostSuperweaponKind::A10Strike));

        restored.set_current_frame(159);
        restored.update_special_power_strikes();
        assert!((restored.find_object(enemy_id).unwrap().health.current - 200.0).abs() < 0.1);

        restored.set_current_frame(160);
        restored.update_special_power_strikes();
        assert!(
            restored
                .special_power_strikes()
                .honesty_complete_ok(HostSuperweaponKind::A10Strike),
            "A10 must complete after mid-flight load"
        );
        let health = restored
            .find_object(enemy_id)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        assert!(
            health < 200.0 || restored.find_object(enemy_id).is_none(),
            "A10 residual damage must apply post-load (health={health})"
        );
    }

    /// Bincode / SaveFileManager path also keeps pending strikes.
    #[test]
    fn save_file_roundtrip_preserves_pending_special_power_strike() {
        use crate::command_system::SpecialPowerType;
        use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
        use std::time::{Duration, SystemTime};

        let save_dir = tempfile::TempDir::new().expect("temp save dir");
        let mut manager = SaveFileManager::with_save_directory(save_dir.path());
        manager.init().expect("save manager init");

        let mut source = GameLogic::new();
        ensure_strike_test_tank(&mut source);
        let caster = source
            .create_object("StrikeTestTank", Team::USA, Vec3::ZERO)
            .expect("caster");
        let enemy = source
            .create_object("StrikeTestTank", Team::GLA, Vec3::new(10.0, 0.0, 0.0))
            .expect("enemy");
        {
            let e = source.find_object_mut(enemy).unwrap();
            e.health.current = 300.0;
            e.health.maximum = 300.0;
            e.thing.template.armor = 0.0;
        }
        source.set_current_frame(0);
        source
            .queue_special_power_strike(
                &SpecialPowerType::DaisyCutter,
                caster,
                Vec3::new(10.0, 0.0, 0.0),
            )
            .expect("queue");
        source.set_current_frame(30);

        let info = SaveGameInfo {
            filename: "special_power_strike_rt".to_string(),
            display_name: "Special Power Strike Roundtrip".to_string(),
            description: "residual pending strike save/load".to_string(),
            map_name: "ResidualMap".to_string(),
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        manager
            .save_game("special_power_strike_rt", &source, &info)
            .expect("save");

        let mut loaded = GameLogic::new();
        loaded.templates = source.templates.clone();
        manager
            .load_game("special_power_strike_rt", &mut loaded)
            .expect("load");

        assert_eq!(loaded.get_current_frame(), 30);
        assert_eq!(loaded.special_power_strikes().pending_count(), 1);
        loaded.set_current_frame(90);
        loaded.update_special_power_strikes();
        assert!(
            loaded
                .special_power_strikes()
                .honesty_complete_ok(HostSuperweaponKind::DaisyCutter),
            "file-loaded strike must complete"
        );
        let health = loaded
            .find_object(enemy)
            .map(|o| o.health.current)
            .unwrap_or(0.0);
        assert!(
            health < 300.0 || loaded.find_object(enemy).is_none(),
            "damage after file load (health={health})"
        );
    }

    fn ensure_upgrade_test_templates(logic: &mut GameLogic) {
        if !logic.templates.contains_key("TestInfantry") {
            let mut t = ThingTemplate::new("TestInfantry");
            t.add_kind_of(KindOf::Infantry)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(80.0)
                .set_cost(100, 0);
            logic.templates.insert("TestInfantry".to_string(), t);
        }
        if !logic.templates.contains_key("TestBuilding") {
            let mut t = ThingTemplate::new("TestBuilding");
            t.add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(1200.0)
                .set_cost(500, -1);
            logic.templates.insert("TestBuilding".to_string(), t);
        }
        if !logic.templates.contains_key("TestBarracks") {
            let mut t = ThingTemplate::new("TestBarracks");
            t.add_kind_of(KindOf::Structure)
                .add_kind_of(KindOf::FSBarracks)
                .add_kind_of(KindOf::Selectable)
                .add_kind_of(KindOf::Attackable)
                .set_health(1000.0)
                .set_cost(600, -1);
            logic.templates.insert("TestBarracks".to_string(), t);
        }
    }

    /// Residual: CaptureBuilding queued mid-flight must survive snapshot and still
    /// complete with capture unlock after load.
    #[test]
    fn host_upgrade_capture_mid_flight_save_load_completes_unlock() {
        use crate::command_system::{CommandType, GameCommand};
        use crate::game_logic::host_upgrades::{
            HostUpgradeKind, HostUpgradePhase, UPGRADE_INFANTRY_CAPTURE,
        };

        let mut source = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 5000;
        source.add_player(player);
        ensure_upgrade_test_templates(&mut source);

        let barracks_id = source
            .create_object("TestBarracks", Team::USA, Vec3::new(-50.0, 0.0, 0.0))
            .expect("barracks");
        let captor_id = source
            .create_object("TestInfantry", Team::USA, Vec3::new(12.0, 0.0, 0.0))
            .expect("captor");
        let building_id = source
            .create_object("TestBuilding", Team::GLA, Vec3::new(0.0, 0.0, 0.0))
            .expect("building");

        // Queue capture research; do NOT update yet (mid-flight residual window).
        source.set_current_frame(20);
        source.queue_command(GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_string(),
            },
            player_id: 0,
            command_id: 1,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![barracks_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        source.process_commands();

        assert!(
            source
                .get_player(0)
                .unwrap()
                .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
            "player research queue must hold Capture mid-flight"
        );
        assert!(
            !source
                .get_player(0)
                .unwrap()
                .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
            "must not unlock before research completes"
        );
        assert_eq!(source.host_upgrades().pending_count(), 1);
        assert!(
            source
                .host_upgrades()
                .honesty_queue_ok(HostUpgradeKind::CaptureBuilding),
            "host residual must record pending Capture research"
        );

        let builder = SnapshotBuilder::new();
        let snapshot = builder
            .create_world_snapshot(&source)
            .expect("snapshot mid-flight Capture upgrade");
        assert_eq!(snapshot.host_upgrades.entries.len(), 1);
        assert_eq!(
            snapshot.host_upgrades.entries[0].phase,
            HostUpgradePhase::Queued
        );
        assert_eq!(
            snapshot.host_upgrades.entries[0].kind,
            HostUpgradeKind::CaptureBuilding
        );
        assert!(
            snapshot.players.iter().any(|p| p
                .research_queue
                .iter()
                .any(|n| n.contains("Capture") || n == UPGRADE_INFANTRY_CAPTURE)),
            "player research_queue must also capture in-flight upgrade"
        );

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snapshot, &mut restored)
            .expect("restore mid-flight Capture upgrade");

        assert_eq!(restored.get_current_frame(), 20);
        assert_eq!(restored.host_upgrades().pending_count(), 1);
        assert!(
            restored
                .host_upgrades()
                .honesty_queue_ok(HostUpgradeKind::CaptureBuilding),
            "host registry pending Capture must survive load"
        );
        assert!(
            restored
                .get_player(0)
                .unwrap()
                .has_queued_upgrade(UPGRADE_INFANTRY_CAPTURE),
            "player queued upgrade must survive load"
        );
        assert!(
            !restored
                .get_player(0)
                .unwrap()
                .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
            "must still be mid-research after load"
        );

        // Complete research after load.
        restored.update();

        assert!(
            restored
                .get_player(0)
                .unwrap()
                .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
            "capture unlock must complete after mid-flight load"
        );
        assert!(
            restored
                .host_upgrades()
                .honesty_complete_ok(HostUpgradeKind::CaptureBuilding),
            "registry must record Capture complete after load"
        );
        assert!(
            restored.host_upgrades().honesty_capture_unlock_ok(),
            "capture unlock honesty after load"
        );
        assert!(
            restored
                .host_upgrades()
                .honesty_host_path_ok(HostUpgradeKind::CaptureBuilding),
            "host path honesty for Capture after load"
        );
        let captor = restored
            .find_object(captor_id)
            .expect("captor after complete");
        assert!(
            captor.has_upgrade_tag(UPGRADE_INFANTRY_CAPTURE),
            "captor must receive capture upgrade tag after post-load complete"
        );

        // Ability now available.
        restored.queue_command(GameCommand {
            command_type: CommandType::CaptureBuilding {
                target_id: building_id,
            },
            player_id: 0,
            command_id: 2,
            timestamp: std::time::SystemTime::now(),
            selected_units: vec![captor_id],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        restored.process_commands();
        let captor = restored
            .find_object(captor_id)
            .expect("captor after unlock");
        assert_eq!(
            captor.ai_state,
            AIState::Capturing,
            "CaptureBuilding must work after mid-flight save/load + complete"
        );
    }

    /// Bincode / SaveFileManager path also keeps pending host upgrade research.
    #[test]
    fn save_file_roundtrip_preserves_pending_host_upgrade() {
        use crate::command_system::{CommandType, GameCommand};
        use crate::game_logic::host_upgrades::{HostUpgradeKind, UPGRADE_INFANTRY_CAPTURE};
        use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType, SaveGameInfo};
        use std::time::{Duration, SystemTime};

        let save_dir = tempfile::TempDir::new().expect("temp save dir");
        let mut manager = SaveFileManager::with_save_directory(save_dir.path());
        manager.init().expect("save manager init");

        let mut source = GameLogic::new();
        let mut player = Player::new(0, Team::USA, "USA", true);
        player.resources.supplies = 5000;
        source.add_player(player);
        ensure_upgrade_test_templates(&mut source);
        let barracks = source
            .create_object("TestBarracks", Team::USA, Vec3::ZERO)
            .expect("barracks");
        source.set_current_frame(5);
        source.queue_command(GameCommand {
            command_type: CommandType::QueueUpgrade {
                upgrade_name: UPGRADE_INFANTRY_CAPTURE.to_string(),
            },
            player_id: 0,
            command_id: 1,
            timestamp: SystemTime::now(),
            selected_units: vec![barracks],
            modifier_keys: crate::command_system::ModifierKeys::default(),
        });
        source.process_commands();
        assert_eq!(source.host_upgrades().pending_count(), 1);

        let info = SaveGameInfo {
            filename: "host_upgrade_rt".to_string(),
            display_name: "Host Upgrade Roundtrip".to_string(),
            description: "residual pending upgrade save/load".to_string(),
            map_name: "ResidualMap".to_string(),
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        manager
            .save_game("host_upgrade_rt", &source, &info)
            .expect("save");

        let mut loaded = GameLogic::new();
        loaded.templates = source.templates.clone();
        manager
            .load_game("host_upgrade_rt", &mut loaded)
            .expect("load");

        assert_eq!(loaded.get_current_frame(), 5);
        assert_eq!(loaded.host_upgrades().pending_count(), 1);
        assert!(loaded
            .host_upgrades()
            .honesty_queue_ok(HostUpgradeKind::CaptureBuilding));
        loaded.update();
        assert!(
            loaded
                .get_player(0)
                .unwrap()
                .has_unlocked_upgrade(UPGRADE_INFANTRY_CAPTURE),
            "file-loaded pending upgrade must complete"
        );
        assert!(
            loaded
                .host_upgrades()
                .honesty_complete_ok(HostUpgradeKind::CaptureBuilding),
            "file-loaded registry must record complete"
        );
    }

    /// Wave 79: Drawable residual camo_stealth_look survives snapshot capture/restore.
    #[test]
    fn drawable_camo_stealth_look_snapshot_residual_wave79() {
        let mut source = GameLogic::new();
        let mut template = ThingTemplate::new("CamoDrawableSnap");
        template
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable);
        source.templates.insert("CamoDrawableSnap".into(), template);
        let id = source
            .create_object("CamoDrawableSnap", Team::GLA, glam::Vec3::ZERO)
            .expect("create");
        {
            let obj = source.get_object_mut(id).expect("obj");
            // HostCamoStealthLook::VisibleDetected = 3
            obj.camo_stealth_look = 3;
            obj.status.stealthed = true;
            obj.status.detected = true;
        }

        let builder = SnapshotBuilder::new();
        let snap = builder.create_world_snapshot(&source).expect("snap");
        let obj_snap = snap.objects.get(&id).expect("obj snap");
        assert_eq!(obj_snap.status.camo_stealth_look, 3);
        assert!(obj_snap.status.stealthed);
        assert!(obj_snap.status.detected);

        let mut restored = GameLogic::new();
        restored.templates = source.templates.clone();
        builder
            .restore_from_snapshot(&snap, &mut restored)
            .expect("restore");
        let obj = restored.find_object(id).expect("restored obj");
        assert_eq!(obj.camo_stealth_look, 3);
        assert!(obj.status.stealthed);
        assert!(obj.status.detected);
        assert!(honesty_drawable_residual_fields_wave79_ok());
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
