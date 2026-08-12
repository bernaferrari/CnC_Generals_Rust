//! Object snapshot types and Xfer residual.

use super::xfer_helpers::{xfer_hashmap_default, xfer_option, xfer_vec_default, xfer_vec_vec3};
use super::*;
use crate::game_logic::*;
use crate::save_load::{SaveLoadError, SaveLoadResult, Xfer, XferData, XferMode};
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::SystemTime;

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

    /// Live C++ `SpecialAbilityUpdate` state for a parsed Hacker Disable
    /// Building channel.  This is deliberately trailing: bincode snapshots
    /// written before the channel have a distinct predecessor mirror, while
    /// the current schema preserves a source/target channel across save/load.
    #[serde(default)]
    pub hacker_disable_channel: Option<HackerDisableChannelState>,
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
    /// Active concrete WeaponSet slot (`0` primary, `1` secondary, `2` tertiary).
    #[serde(default)]
    pub active_weapon_slot: u8,
    /// C++ WeaponSet lock mode.  Defaults for save files created before lock
    /// state was persisted.
    #[serde(default)]
    pub weapon_lock_type: WeaponLockType,
    /// Concrete slot guarded by `weapon_lock_type`.
    #[serde(default)]
    pub weapon_lock_slot: u8,
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
            weapon_lock_type: WeaponLockType::NotLocked,
            weapon_lock_slot: 0,
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
    /// C++ QueueProductionExitUpdate::m_currentDelay presentation mirror.
    /// The integer field below is authoritative for parsed Queue exits.
    #[serde(default)]
    pub exit_delay_remaining: f32,
    /// C++ QueueProductionExitUpdate::m_currentDelay in logic frames.
    #[serde(default)]
    pub exit_delay_remaining_frames: u32,
    /// C++ QueueProductionExitUpdate::m_currentBurstCount.
    #[serde(default)]
    pub exit_burst_remaining: u32,
    /// Distinguishes legacy float-only saves from parsed Queue runtime state.
    #[serde(default)]
    pub queue_exit_state_initialized: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProductionQueueEntry {
    pub template_name: String,
    pub progress: f32,
    pub cost: u32,
    /// C++ `ProductionEntry::m_framesUnderConstruction`.  Old float-only
    /// snapshots deserialize as zero and are reconstructed once from progress
    /// by `ProductionItem` on their first production update.
    ///
    /// Keep this trailing in the serde record so legacy serialized entries
    /// retain their original template/progress/cost prefix.
    #[serde(default)]
    pub construction_frames: u32,
    /// C++ ProductionEntry quantity state.  It must survive with Queue exit
    /// delay so a save after the first member of a modifier batch does not
    /// recreate or discard remaining members on load.
    #[serde(default = "production_quantity_one")]
    pub quantity_total: u32,
    #[serde(default)]
    pub quantity_produced: u32,
    /// `false` is C++ PRODUCTION_UNIT, preserving legacy snapshot behavior.
    #[serde(default)]
    pub is_upgrade: bool,
}

fn production_quantity_one() -> u32 {
    1
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

impl ObjectSnapshot {
    /// Transfer the positional object record for a known outer world-schema
    /// version.  Marker labels are deliberately no-ops in Common Xfer, so an
    /// appended field cannot discover an older stream at the object tail.
    /// The enclosing `WorldSnapshot` reads its version before its object map
    /// and supplies the only safe compatibility boundary here.
    pub(super) fn xfer_for_world_version(
        &mut self,
        xfer: &mut dyn Xfer,
        world_version: u32,
    ) -> SaveLoadResult<()> {
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

        if world_version >= WORLD_SNAPSHOT_BINCODE_VERSION {
            xfer.xfer_marker_label("HackerDisableChannel")?;
            xfer_option(
                xfer,
                &mut self.hacker_disable_channel,
                HackerDisableChannelState::new(
                    ObjectId(0),
                    HackerDisableChannelPhase::Unpacking,
                    0,
                ),
            )?;
        } else if xfer.get_mode() == XferMode::Load {
            // v1/v2 direct-Xfer records ended at ObjectType.  Do not let a
            // pre-seeded default accidentally manufacture a live channel.
            self.hacker_disable_channel = None;
        }

        Ok(())
    }
}

// Implement XferData for callers that serialize a standalone current object
// record.  WorldSnapshot uses the version-aware method above for historical
// direct-Xfer streams.
impl XferData for ObjectSnapshot {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        self.xfer_for_world_version(xfer, WORLD_SNAPSHOT_BINCODE_VERSION)
    }
}

impl XferData for HackerDisableChannelState {
    fn xfer(&mut self, xfer: &mut dyn Xfer) -> SaveLoadResult<()> {
        xfer.xfer_marker_label("HackerDisableChannelState")?;
        xfer.xfer_marker_label("TargetId")?;
        self.target_id.xfer(xfer)?;
        let mut phase = match self.phase {
            HackerDisableChannelPhase::Unpacking => 0,
            HackerDisableChannelPhase::Preparing => 1,
            HackerDisableChannelPhase::Packing => 2,
            HackerDisableChannelPhase::Approaching => 3,
        };
        xfer.xfer_marker_label("Phase")?;
        xfer.xfer_u8(&mut phase)?;
        self.phase = match phase {
            0 => HackerDisableChannelPhase::Unpacking,
            1 => HackerDisableChannelPhase::Preparing,
            2 => HackerDisableChannelPhase::Packing,
            3 => HackerDisableChannelPhase::Approaching,
            other => {
                return Err(SaveLoadError::Corrupted(format!(
                    "Invalid HackerDisableChannelPhase value in object snapshot: {other}"
                )));
            }
        };
        xfer.xfer_marker_label("RemainingSeconds")?;
        xfer.xfer_f32(&mut self.remaining_seconds)?;
        if !self.remaining_seconds.is_finite() || self.remaining_seconds < 0.0 {
            return Err(SaveLoadError::Corrupted(
                "Invalid HackerDisableChannelState remaining seconds".to_string(),
            ));
        }
        Ok(())
    }
}

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
        // Appended concrete WeaponSet lock state.  Map explicitly rather than
        // relying on a Rust enum discriminant for serialized compatibility.
        let mut weapon_lock_type = weapon_lock_type_to_snapshot_value(self.weapon_lock_type);
        xfer.xfer_marker_label("WeaponLockType")?;
        xfer.xfer_u8(&mut weapon_lock_type)?;
        self.weapon_lock_type = weapon_lock_type_from_snapshot_value(weapon_lock_type);
        xfer.xfer_marker_label("WeaponLockSlot")?;
        xfer.xfer_u8(&mut self.weapon_lock_slot)?;
        Ok(())
    }
}

fn weapon_lock_type_to_snapshot_value(lock_type: WeaponLockType) -> u8 {
    match lock_type {
        WeaponLockType::NotLocked => 0,
        WeaponLockType::LockedTemporarily => 1,
        WeaponLockType::LockedPermanently => 2,
    }
}

fn weapon_lock_type_from_snapshot_value(value: u8) -> WeaponLockType {
    match value {
        0 => WeaponLockType::NotLocked,
        1 => WeaponLockType::LockedTemporarily,
        2 => WeaponLockType::LockedPermanently,
        _ => WeaponLockType::NotLocked,
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
            AIState::GuardRetaliating => 20,
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
            20 => AIState::GuardRetaliating,
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
        // Version the per-entry payload so old Main snapshots retain their
        // float-progress migration path.  Retail C++ stores this exact frame
        // counter in `ProductionUpdate::xferSnapshot`.
        const CURRENT_VERSION: crate::save_load::XferVersion = 3;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)?;
        xfer.xfer_marker_label("ProductionQueueEntry")?;
        xfer.xfer_marker_label("TemplateName")?;
        self.template_name.xfer(xfer)?;
        xfer.xfer_marker_label("Progress")?;
        xfer.xfer_f32(&mut self.progress)?;
        if version >= 2 {
            xfer.xfer_marker_label("ConstructionFrames")?;
            xfer.xfer_u32(&mut self.construction_frames)?;
        }
        xfer.xfer_marker_label("Cost")?;
        xfer.xfer_u32(&mut self.cost)?;
        if version >= 3 {
            xfer.xfer_marker_label("QuantityTotal")?;
            xfer.xfer_u32(&mut self.quantity_total)?;
            xfer.xfer_marker_label("QuantityProduced")?;
            xfer.xfer_u32(&mut self.quantity_produced)?;
            xfer.xfer_marker_label("IsUpgrade")?;
            xfer.xfer_bool(&mut self.is_upgrade)?;
        }
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
                construction_frames: 0,
                cost: 0,
                quantity_total: 1,
                quantity_produced: 0,
                is_upgrade: false,
            },
        )?;
        xfer.xfer_marker_label("IsProducing")?;
        xfer.xfer_bool(&mut self.is_producing)?;
        xfer.xfer_marker_label("ProductionProgress")?;
        xfer.xfer_f32(&mut self.production_progress)?;
        xfer.xfer_marker_label("RallyPoint")?;
        xfer_option(xfer, &mut self.rally_point, glam::Vec3::ZERO)?;
        // These are appended to the module record.  The active bincode save
        // path uses serde defaults for old snapshots; paired Xfer save/load
        // streams retain the full C++ Queue runtime counters.
        xfer.xfer_marker_label("ExitDelayRemaining")?;
        xfer.xfer_f32(&mut self.exit_delay_remaining)?;
        xfer.xfer_marker_label("ExitDelayRemainingFrames")?;
        xfer.xfer_u32(&mut self.exit_delay_remaining_frames)?;
        xfer.xfer_marker_label("ExitBurstRemaining")?;
        xfer.xfer_u32(&mut self.exit_burst_remaining)?;
        xfer.xfer_marker_label("QueueExitStateInitialized")?;
        xfer.xfer_bool(&mut self.queue_exit_state_initialized)?;
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
                        exit_delay_remaining: 0.0,
                        exit_delay_remaining_frames: 0,
                        exit_burst_remaining: 0,
                        queue_exit_state_initialized: false,
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
