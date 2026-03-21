use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData};
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
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ObjectStatusSnapshot {
    pub destroyed: bool,
    pub under_construction: bool,
    pub selected: bool,
    pub moving: bool,
    pub attacking: bool,
    pub airborne_target: bool,
    pub stealthed: bool,
    pub garrisoned: bool,
    pub being_repaired: bool,
    pub on_fire: bool,
    pub poisoned: bool,
    pub radar_jammed: bool,
    pub disabled_underpowered: bool,
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

        self.version.xfer(xfer)?;
        xfer.xfer_marker_label("Timestamp")?;
        // Note: SystemTime serialization would need custom implementation

        self.frame_number.xfer(xfer)?;
        self.random_seed.xfer(xfer)?;

        xfer.xfer_marker_label("Objects")?;
        // HashMap serialization needs special handling

        xfer.xfer_marker_label("Players")?;
        // Manual Vec serialization since generic Vec<T> can't be used with trait objects
        let mut len = self.players.len() as u32;
        xfer.xfer_u32(&mut len)?;
        for player in &mut self.players {
            player.xfer(xfer)?;
        }

        // Continue for all fields...
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
        self.id.xfer(xfer)?;
        self.template_name.xfer(xfer)?;
        self.team.xfer(xfer)?;
        // Continue for all fields...
        Ok(())
    }
}

impl XferData for PlayerSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("PlayerSnapshot")?;
        self.id.xfer(xfer)?;
        self.name.xfer(xfer)?;
        self.team.xfer(xfer)?;
        self.is_human.xfer(xfer)?;
        self.is_active.xfer(xfer)?;
        // Continue for all fields...
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
            weapons: object.weapon.clone().map(|w| vec![w]).unwrap_or_default(),
            contained_objects: object.occupants.clone(),
            container_object: None, // Would need to track container
            modules: self.snapshot_object_modules(object)?,
            object_type,
        })
    }

    fn snapshot_object_status(&self, object: &Object) -> ObjectStatusSnapshot {
        ObjectStatusSnapshot {
            destroyed: object.status.destroyed,
            under_construction: object.status.under_construction,
            selected: object.selected,
            moving: object.status.moving,
            attacking: object.status.attacking,
            airborne_target: object.status.airborne_target,
            stealthed: object.status.stealthed,
            garrisoned: matches!(object.ai_state, AIState::Garrisoned),
            being_repaired: matches!(object.ai_state, AIState::SeekingRepair),
            on_fire: false,
            poisoned: false,
            radar_jammed: false,
            disabled_underpowered: object.status.disabled_underpowered,
        }
    }

    #[allow(dead_code)]
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
            unlocked_units: Self::sorted_unique_strings(unlocked_units.into_iter()),
            unlocked_buildings: Self::sorted_unique_strings(unlocked_buildings.into_iter()),
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
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

    #[allow(dead_code)]
    fn restore_object(
        &self,
        snapshot: &ObjectSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        let template = game_logic
            .templates
            .get(snapshot.template_name.as_str())
            .cloned()
            .ok_or_else(|| {
                SaveLoadError::Corrupted(format!(
                    "Template not found while restoring object {}: {}",
                    snapshot.id, snapshot.template_name
                ))
            })?;

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

        object.weapon = snapshot.weapons.first().cloned();
        object.occupants = snapshot.contained_objects.clone();

        self.restore_object_type_data(&snapshot.object_type, &mut object)?;
        self.restore_object_modules(&snapshot.modules, &mut object, game_logic)?;

        game_logic.objects.insert(snapshot.id, object);
        Ok(())
    }

    fn restore_object_status(&self, status: &ObjectStatusSnapshot, object: &mut Object) {
        object.status.destroyed = status.destroyed;
        object.status.under_construction = status.under_construction;
        object.status.moving = status.moving;
        object.status.attacking = status.attacking;
        object.status.airborne_target = status.airborne_target;
        object.status.stealthed = status.stealthed;
        object.status.selected = status.selected;
        object.status.disabled_underpowered = status.disabled_underpowered;

        object.ai_state = if status.destroyed {
            AIState::Idle
        } else if status.garrisoned {
            AIState::Garrisoned
        } else if status.being_repaired {
            AIState::SeekingRepair
        } else if status.attacking {
            AIState::Attacking
        } else if status.moving {
            AIState::Moving
        } else {
            AIState::Idle
        };

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

    #[allow(dead_code)]
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

            game_logic.add_player(Player {
                id: snap.id,
                team: snap.team,
                name: snap.name.clone(),
                resources: snap.resources.clone(),
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

        let expected_len = match (terrain_snapshot.width as usize)
            .checked_mul(terrain_snapshot.height as usize)
        {
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
}
