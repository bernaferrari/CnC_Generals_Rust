//! Unit class - Moveable game entities
//!
//! Units are mobile objects that can move around the map, engage in combat,
//! and perform various actions. This includes infantry, vehicles, aircraft, etc.
//!
//! Port reference: GameLogic/Object/Unit.cpp, GameLogic/Object/Update/AIUpdate.cpp.

use crate::action_manager::{ActionManager, TheActionManager};
use crate::ai::dock::AIDockMachine;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::pathfind::PathfindLayerEnum;
use crate::ai::pathfind::{Path as AiPath, PathfindLayerEnum as AiPathLayer};
use crate::ai::pathfind_astar::PathfindLayerEnum as ClassicPathLayer;
use crate::ai::pathfinding_system::PathfindLayerEnum as PfLayer;
use crate::ai::states::{AIStateMachine, AIStateType};
use crate::ai::turret::{TurretAI, TurretStateMachine};
use crate::ai::{
    mood_matrix_adjustment, mood_matrix_parameters, search_qualifiers, AiCommandInterface,
    MoodMatrixAction, THE_AI,
};
use crate::attack::CanAttackResult;
use crate::common::ObjectID;
use crate::common::VeterancyLevel;
use crate::common::*;
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::helpers::{
    get_game_logic_random_value_real, FindPositionOptions, TheFXListStore, TheGameLogic,
    ThePartitionManager, TheTerrainLogic,
};
use crate::locomotor::{
    update_movement_with_pathfinding, BodyDamageType, Locomotor, LocomotorAppearance, LocomotorSet,
    LocomotorSurfaceTypeMask, PathFollowingState, SURFACE_AIR,
};
use crate::modules::{
    AIAttitudeType, AIUpdateInterface, AIUpdateInterfaceExt, ContainModuleInterfaceExt,
    PhysicsBehaviorExt, FAST_AS_POSSIBLE,
};
use crate::object::draw::TerrainDecalType;
use crate::object::object_factory::{get_object_factory, GameObjectInstance};
use crate::object::update::ai_update_interface::GuardTargetType;
use crate::object::update::{
    AssaultTransportAIUpdate, DeliverPayloadAIUpdate, DeployStyleAIUpdate, HackInternetAIUpdate,
    RailedTransportAIUpdate, TransportAIUpdate, WanderAIUpdate,
};
use crate::object::update::{ChinookAIUpdate, DozerAIUpdate, JetAIUpdate, TurretAIData};
use crate::object::{Object, TriggerInfo};
use crate::path::{PathfindMap, Waypoint, PATHFIND_CELL_SIZE_F, PATHFIND_CLOSE_ENOUGH};
use crate::physics::GRAVITY;
use crate::player::PlayerIndex;
#[cfg(feature = "allow_surrender")]
use crate::pow_truck_ai_update::{POWTruckAIUpdate, POWTruckAIUpdateData};
use crate::supply_system::{SupplyTruckAIUpdate, WorkerAIUpdate};
use crate::team::Team;
use crate::upgrade::center::get_upgrade_center;
use crate::weapon::{WeaponAntiMask, WeaponChoiceCriteria, WeaponSet, WeaponSlotType};
use game_engine::common::system::{Snapshotable, Xfer};
use log::error;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

const WAYPOINT_PATH_LIMIT: usize = 1024;
const AI_UPDATE_MAX_WAYPOINTS: usize = 16;

/// Movement states for units
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MovementState {
    Idle,
    Moving,
    TurningToFace,
    Attacking,
    Retreating,
    Following,
    Patrolling,
    Guarding,
    Pursuing,
    Fleeing,
    Backing,
}

/// Formation positions for group movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormationType {
    None,
    Line,
    Column,
    Wedge,
    Box,
    Scattered,
}

/// Combat modes for units
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CombatMode {
    Aggressive,   // Attack anything in range
    Defensive,    // Only attack when attacked
    HoldPosition, // Don't move to attack
    HoldFire,     // Don't attack at all
    GuardArea,    // Stay in designated area
}

/// Unit-specific data and behavior
#[derive(Debug)]
#[allow(dead_code)]
pub struct Unit {
    /// Base object functionality
    base_object: Arc<RwLock<Object>>,

    /// Movement and pathfinding
    locomotor_set: LocomotorSet,
    current_locomotor: Option<Arc<Mutex<Locomotor>>>,
    movement_state: MovementState,
    target_position: Option<Coord3D>,
    waypoint_queue: Vec<Waypoint>,
    current_path: Option<Vec<Coord2D>>,
    path_index: usize,
    path_following_state: Option<PathFollowingState>,
    path_extra_distance: Real,
    path_adjusts_destination: bool,
    movement_speed_multiplier: Real,
    current_speed: f32,
    attack_move_active: bool,
    last_target_scan_frame: u32,
    attack_move_resume_frame: u32,
    attack_target_lock_until: u32,
    mood_attack_check_rate_frames: u32,

    /// Formation and group behavior
    #[allow(dead_code)]
    formation_type: FormationType,
    formation_position: usize,
    group_leader: Option<ObjectID>,
    group_members: Vec<ObjectID>,
    follow_target: Option<ObjectID>,
    follow_distance: Real,

    /// Combat behavior
    combat_mode: CombatMode,
    attack_target: Option<ObjectID>,
    attack_position: Option<Coord3D>,
    engagement_range: Real,
    retreat_threshold: Real,
    patrol_points: Vec<Coord3D>,
    current_patrol_index: usize,
    patrol_loop: bool,
    guard_position: Option<Coord3D>,
    guard_radius: Real,

    /// Movement constraints
    can_cross_bridges: bool,
    can_swim: bool,
    can_fly: bool,
    preferred_terrain: TerrainType,
    movement_penalty_modifiers: HashMap<TerrainType, Real>,

    /// Orders and commands
    current_order: Option<UnitOrder>,
    order_queue: Vec<UnitOrder>,
    auto_acquire_enemies: bool,
    auto_acquire_attack_buildings: bool,
    auto_acquire_while_stealthed: bool,
    auto_acquire_not_while_attacking: bool,
    return_to_formation: bool,

    /// Morale and psychology
    morale_level: Real,
    fear_level: Real,
    panic_threshold: Real,
    bravery_modifier: Real,

    /// Status effects
    is_stunned: bool,
    is_suppressed: bool,
    is_pinned: bool,
    is_routing: bool,
    is_garrisoned: bool,
    garrison_building: Option<ObjectID>,

    /// Transport capabilities (for vehicles that can carry troops)
    transport_capacity: usize,
    transported_units: Vec<ObjectID>,
    can_amphibious_unload: bool,

    /// Special abilities
    can_capture_buildings: bool,
    can_sabotage: bool,
    can_hack: bool,
    stealth_detection_range: Real,

    /// Animation and visual state
    current_animation: AsciiString,
    animation_state: ModelConditionFlags,
    facing_direction: Real,
    desired_facing: Real,
    turn_rate: Real,
}

/// Orders that can be given to units
#[derive(Debug, Clone)]
pub enum UnitOrder {
    Stop,
    Move {
        destination: Coord3D,
        use_formation: bool,
        waypoints: Vec<Waypoint>,
    },
    Attack {
        target: ObjectID,
        pursue: bool,
    },
    AttackMove {
        destination: Coord3D,
        engage_enemies: bool,
    },
    Guard {
        position: Coord3D,
        area_radius: Real,
    },
    Follow {
        target: ObjectID,
        distance: Real,
    },
    Patrol {
        waypoints: Vec<Coord3D>,
        loop_patrol: bool,
    },
    Garrison {
        building: ObjectID,
    },
    Ungarrison {
        exit_position: Option<Coord3D>,
    },
    Capture {
        building: ObjectID,
    },
    Sabotage {
        target: ObjectID,
    },
    Hack {
        target: ObjectID,
    },
    PickupSupplies {
        supply_source: ObjectID,
    },
    Retreat {
        safe_position: Coord3D,
        organized: bool,
    },
}

impl Unit {
    /// Create a new Unit
    pub fn new(
        base_object: Arc<RwLock<Object>>,
        thing_template: &dyn ThingTemplate,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let locomotor_set = LocomotorSet::new();
        let current_locomotor = locomotor_set.get_default_locomotor();

        Ok(Unit {
            base_object,
            locomotor_set,
            current_locomotor,
            movement_state: MovementState::Idle,
            target_position: None,
            waypoint_queue: Vec::new(),
            current_path: None,
            path_index: 0,
            path_following_state: None,
            path_extra_distance: 0.0,
            path_adjusts_destination: true,
            movement_speed_multiplier: 1.0,
            current_speed: 0.0,
            attack_move_active: false,
            last_target_scan_frame: 0,
            attack_move_resume_frame: 0,
            attack_target_lock_until: 0,
            mood_attack_check_rate_frames: (LOGICFRAMES_PER_SECOND * 2) as u32,

            formation_type: FormationType::None,
            formation_position: 0,
            group_leader: None,
            group_members: Vec::new(),
            follow_target: None,
            follow_distance: 50.0, // Default follow distance

            combat_mode: CombatMode::Aggressive,
            attack_target: None,
            attack_position: None,
            engagement_range: thing_template.calc_vision_range(),
            retreat_threshold: 0.25, // Retreat when health below 25%
            patrol_points: Vec::new(),
            current_patrol_index: 0,
            patrol_loop: false,
            guard_position: None,
            guard_radius: 0.0,

            can_cross_bridges: thing_template.is_kind_of(KindOf::CanCrossBridges),
            can_swim: thing_template.is_kind_of(KindOf::Amphibious),
            can_fly: thing_template.is_kind_of(KindOf::Aircraft),
            preferred_terrain: TerrainType::Grass,
            movement_penalty_modifiers: HashMap::new(),

            current_order: None,
            order_queue: Vec::new(),
            auto_acquire_enemies: true,
            auto_acquire_attack_buildings: false,
            auto_acquire_while_stealthed: false,
            auto_acquire_not_while_attacking: false,
            return_to_formation: false,

            morale_level: 1.0,
            fear_level: 0.0,
            panic_threshold: 0.8,
            bravery_modifier: 1.0,

            is_stunned: false,
            is_suppressed: false,
            is_pinned: false,
            is_routing: false,
            is_garrisoned: false,
            garrison_building: None,

            transport_capacity: 0,
            transported_units: Vec::new(),
            can_amphibious_unload: thing_template.is_kind_of(KindOf::AmphibiousTransport),

            can_capture_buildings: thing_template.is_kind_of(KindOf::CanCapture),
            can_sabotage: thing_template.is_kind_of(KindOf::Saboteur),
            can_hack: thing_template.is_kind_of(KindOf::Hacker),
            stealth_detection_range: 0.0,

            current_animation: AsciiString::from("IDLE"),
            animation_state: ModelConditionFlags::empty(),
            facing_direction: 0.0,
            desired_facing: 0.0,
            turn_rate: 0.0,
        })
    }

    /// Attempt to load an occupant into this transport. Returns true on success.
    pub fn load_occupant(&mut self, occupant: ObjectID) -> bool {
        if self.transport_capacity == 0 {
            return false;
        }
        if self.transported_units.len() >= self.transport_capacity {
            return false;
        }
        if self.transported_units.contains(&occupant) {
            return false;
        }
        self.transported_units.push(occupant);
        true
    }

    /// Attempt to unload an occupant; returns true if it was present.
    pub fn unload_occupant(&mut self, occupant: ObjectID) -> bool {
        if let Some(pos) = self.transported_units.iter().position(|id| *id == occupant) {
            self.transported_units.remove(pos);
            true
        } else {
            false
        }
    }

    /// Remove all occupants, returning the list for callers to place them.
    pub fn unload_all(&mut self) -> Vec<ObjectID> {
        let mut out = Vec::new();
        std::mem::swap(&mut out, &mut self.transported_units);
        out
    }

    /// Whether this transport currently holds an occupant.
    pub fn has_occupant(&self, occupant: ObjectID) -> bool {
        self.transported_units.contains(&occupant)
    }

    /// Count current occupants.
    pub fn occupant_count(&self) -> usize {
        self.transported_units.len()
    }

    pub fn base_object(&self) -> Arc<RwLock<Object>> {
        Arc::clone(&self.base_object)
    }

    pub fn get_id(&self) -> ObjectID {
        self.base_object
            .read()
            .ok()
            .map(|guard| guard.get_id())
            .unwrap_or(INVALID_ID)
    }

    pub fn get_orientation(&self) -> Real {
        self.base_object
            .read()
            .ok()
            .map(|guard| guard.get_orientation())
            .unwrap_or(0.0)
    }

    pub fn set_orientation(&mut self, angle: Real) -> Result<(), String> {
        let Ok(mut guard) = self.base_object.write() else {
            return Err("Unit base object lock poisoned".to_string());
        };
        guard.set_orientation(angle)
    }

    pub fn get_unit_direction_vector_2d(&self) -> (f32, f32) {
        self.base_object
            .read()
            .ok()
            .map(|guard| guard.get_unit_direction_vector_2d())
            .unwrap_or((1.0, 0.0))
    }

    pub fn get_ai_update_interface(&self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>> {
        self.base_object
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
    }

    pub(crate) fn forward_command_to_flight_deck(&self, params: &crate::ai::AiCommandParams) {
        if let Ok(guard) = self.base_object.read() {
            guard.forward_command_to_flight_deck(params);
        }
    }

    /// Update unit logic for one frame
    pub fn update(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Process current order
        self.process_current_order(delta_time)?;

        // Update movement
        self.update_movement(delta_time)?;

        // Update combat behavior
        self.update_combat(delta_time)?;

        // Update facing direction
        self.update_facing(delta_time)?;

        // Check for state changes
        self.check_status_effects(delta_time)?;

        // Update animation state
        self.update_animation_state()?;

        // Update per-unit AI module (matches C++ AIUpdateInterface::update call per frame).
        if let Ok(base_guard) = self.base_object.read() {
            if let Some(ai) = base_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    let _ = ai_guard.update();
                }
            }
        }

        Ok(())
    }

    /// Process the current order
    fn process_current_order(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.advance_order_queue();
        let order = self.current_order.take();
        match order {
            None => {
                // No current order, check for auto-behaviors
                if self.auto_acquire_enemies {
                    self.look_for_enemies()?;
                }

                if self.return_to_formation {
                    self.return_to_formation_position()?;
                }
            }
            Some(current_order) => {
                if !matches!(current_order, UnitOrder::AttackMove { .. }) {
                    self.attack_move_active = false;
                }

                match current_order {
                    UnitOrder::Stop => {
                        self.stop_movement();
                        // Don't restore — order is consumed
                        self.advance_order_queue();
                    }

                    UnitOrder::Move {
                        destination,
                        use_formation,
                        waypoints,
                    } => {
                        if self.movement_state == MovementState::Idle
                            && self.target_position.is_none()
                            && self.waypoint_queue.is_empty()
                        {
                            let delta = self.get_position() - destination;
                            if (delta.x * delta.x + delta.y * delta.y).sqrt() <= 1.0 {
                                // Don't restore — order completed
                                self.advance_order_queue();
                                return Ok(());
                            }
                        }
                        self.process_move_order(destination, use_formation, &waypoints)?;
                        // Restore — move order continues across frames
                        self.current_order = Some(UnitOrder::Move {
                            destination,
                            use_formation,
                            waypoints,
                        });
                    }

                    UnitOrder::Attack { target, pursue } => {
                        self.process_attack_order(target, pursue)?;
                        self.current_order = Some(UnitOrder::Attack { target, pursue });
                    }

                    UnitOrder::AttackMove {
                        destination,
                        engage_enemies,
                    } => {
                        self.process_attack_move_order(destination, engage_enemies)?;
                        self.current_order = Some(UnitOrder::AttackMove {
                            destination,
                            engage_enemies,
                        });
                    }

                    UnitOrder::Guard {
                        position,
                        area_radius,
                    } => {
                        self.process_guard_order(position, area_radius)?;
                        self.current_order = Some(UnitOrder::Guard {
                            position,
                            area_radius,
                        });
                    }

                    UnitOrder::Follow { target, distance } => {
                        self.process_follow_order(target, distance)?;
                        self.current_order = Some(UnitOrder::Follow { target, distance });
                    }

                    UnitOrder::Patrol {
                        waypoints,
                        loop_patrol,
                    } => {
                        self.process_patrol_order(&waypoints, loop_patrol)?;
                        self.current_order = Some(UnitOrder::Patrol {
                            waypoints,
                            loop_patrol,
                        });
                    }

                    UnitOrder::Garrison { building } => {
                        self.process_garrison_order(building)?;
                        self.current_order = Some(UnitOrder::Garrison { building });
                    }

                    UnitOrder::Ungarrison { exit_position } => {
                        self.process_ungarrison_order(exit_position)?;
                        self.current_order = Some(UnitOrder::Ungarrison { exit_position });
                    }

                    UnitOrder::Capture { building } => {
                        self.process_capture_order(building)?;
                        self.current_order = Some(UnitOrder::Capture { building });
                    }

                    UnitOrder::Retreat {
                        safe_position,
                        organized,
                    } => {
                        self.process_retreat_order(safe_position, organized)?;
                        self.current_order = Some(UnitOrder::Retreat {
                            safe_position,
                            organized,
                        });
                    }

                    other => {
                        // Restore unhandled order types
                        self.current_order = Some(other);
                    }
                }
            }
        }

        Ok(())
    }

    fn advance_order_queue(&mut self) {
        if self.current_order.is_none() && !self.order_queue.is_empty() {
            self.current_order = Some(self.order_queue.remove(0));
        }
    }

    /// Update movement based on current state
    fn update_movement(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let prev_movement_state = self.movement_state;
        let mut completed_move = false;

        let should_stop_for_dead_ai = self
            .base_object
            .read()
            .ok()
            .and_then(|obj_guard| obj_guard.get_ai_update_interface())
            .and_then(|ai| {
                ai.lock()
                    .ok()
                    .map(|ai_guard| ai_guard.is_ai_in_dead_state())
            })
            .unwrap_or(false)
            && self
                .current_locomotor
                .as_ref()
                .and_then(|locomotor| {
                    locomotor
                        .lock()
                        .ok()
                        .map(|loc_guard| !loc_guard.template.locomotor_works_when_dead)
                })
                .unwrap_or(false);
        if should_stop_for_dead_ai {
            self.stop_movement();
            self.target_position = None;
            self.path_following_state = None;
            self.current_path = None;
            self.current_speed = 0.0;
            return Ok(());
        }

        if self.is_movement_active() {
            if let Some(target) = self.target_position {
                // Get position before entering the mutable borrow scope
                let current_pos = self.get_position();
                let current_angle = self.facing_direction;
                let current_speed = self.current_speed;

                // Track whether we need to handle a waypoint after the borrow ends
                let mut handle_waypoint: Option<Coord3D> = None;

                let (desired_speed, condition, _blocked) = {
                    let mut speed = FAST_AS_POSSIBLE;
                    let mut body_condition = BodyDamageType::Pristine;
                    let mut blocked = false;
                    if let Ok(obj_guard) = self.base_object.read() {
                        if let Some(ai) = obj_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                speed = ai_guard.get_desired_speed();
                                blocked = ai_guard.get_num_frames_blocked() > 0;
                                speed = ai_guard.apply_bump_speed_limit(speed, blocked);
                            }
                        }
                        if let Some(body) = obj_guard.get_body_module() {
                            if let Ok(body_guard) = body.lock() {
                                body_condition =
                                    to_locomotor_body_damage_type(body_guard.get_damage_state());
                            }
                        }
                    }
                    (speed, body_condition, blocked)
                };

                if let (Some(path_state), Some(locomotor)) = (
                    self.path_following_state.as_mut(),
                    self.current_locomotor.as_ref(),
                ) {
                    // Clone the locomotor Arc so we don't keep borrowing self
                    let locomotor_clone = locomotor.clone();

                    if let Ok(ai_guard) = THE_AI.read() {
                        if let Some(pathfinding) = ai_guard.pathfinding_system() {
                            if let Ok(mut loc_guard) = locomotor_clone.lock() {
                                let current_frame = TheGameLogic::get_frame() as u32;
                                let delta_time =
                                    (delta_time * self.movement_speed_multiplier) as f32;

                                match update_movement_with_pathfinding(
                                    self.base_object
                                        .read()
                                        .map(|obj| obj.get_id())
                                        .unwrap_or(INVALID_ID),
                                    &mut loc_guard,
                                    path_state,
                                    &current_pos,
                                    current_angle,
                                    current_speed,
                                    condition,
                                    desired_speed,
                                    current_frame,
                                    delta_time,
                                    pathfinding,
                                ) {
                                    Ok(Some((new_pos, new_angle, new_speed))) => {
                                        if let Ok(mut obj_guard) = self.base_object.write() {
                                            let _ = obj_guard.set_position(&new_pos);
                                            let _ = obj_guard.set_orientation(new_angle as Real);
                                            if let Some(physics) = obj_guard.get_physics() {
                                                if let Ok(mut phys_guard) = physics.lock() {
                                                    let delta = new_pos - current_pos;
                                                    let velocity = if delta_time > 0.0 {
                                                        delta / delta_time.max(0.0001)
                                                    } else {
                                                        Vec3D::ZERO
                                                    };
                                                    phys_guard.set_velocity(&velocity);
                                                    if delta_time > 0.0 {
                                                        let mut yaw_delta =
                                                            new_angle - current_angle;
                                                        let two_pi = std::f32::consts::PI * 2.0;
                                                        while yaw_delta > std::f32::consts::PI {
                                                            yaw_delta -= two_pi;
                                                        }
                                                        while yaw_delta < -std::f32::consts::PI {
                                                            yaw_delta += two_pi;
                                                        }
                                                        phys_guard.set_yaw_rate(
                                                            (yaw_delta / delta_time.max(0.0001))
                                                                as Real,
                                                        );
                                                        let turning = if yaw_delta > 0.0 {
                                                            1
                                                        } else if yaw_delta < 0.0 {
                                                            -1
                                                        } else {
                                                            0
                                                        };
                                                        phys_guard.set_turning(turning);
                                                        if matches!(
                                                            loc_guard.get_appearance(),
                                                            LocomotorAppearance::Thrust
                                                                | LocomotorAppearance::Wings
                                                                | LocomotorAppearance::Hover
                                                        ) {
                                                            let pitch_rate = loc_guard
                                                                .template
                                                                .pitch_by_z_vel_coef
                                                                * velocity.z;
                                                            let mut pitch_rate = pitch_rate;
                                                            if loc_guard.template.pitch_stiffness
                                                                > 0.0
                                                            {
                                                                pitch_rate *= loc_guard
                                                                    .template
                                                                    .pitch_stiffness;
                                                            }
                                                            if loc_guard.template.pitch_damping
                                                                > 0.0
                                                            {
                                                                pitch_rate *= (1.0
                                                                    - loc_guard
                                                                        .template
                                                                        .pitch_damping)
                                                                    .clamp(0.0, 1.0);
                                                            }
                                                            phys_guard.set_pitch_rate(pitch_rate);
                                                            let mut roll_rate =
                                                                loc_guard.template.thrust_roll
                                                                    * new_speed;
                                                            if loc_guard.template.roll_stiffness
                                                                > 0.0
                                                            {
                                                                roll_rate *= loc_guard
                                                                    .template
                                                                    .roll_stiffness;
                                                            }
                                                            if loc_guard.template.roll_damping > 0.0
                                                            {
                                                                roll_rate *= (1.0
                                                                    - loc_guard
                                                                        .template
                                                                        .roll_damping)
                                                                    .clamp(0.0, 1.0);
                                                            }
                                                            if loc_guard.template.wobble_rate > 0.0
                                                            {
                                                                let frame =
                                                                    TheGameLogic::get_frame()
                                                                        as f32;
                                                                let phase = (obj_guard.get_id()
                                                                    as f32)
                                                                    * 0.01;
                                                                let wobble_min =
                                                                    loc_guard.template.min_wobble;
                                                                let wobble_max =
                                                                    loc_guard.template.max_wobble;
                                                                let wobble_amp = wobble_max
                                                                    .max(wobble_min)
                                                                    - wobble_min;
                                                                if wobble_amp > 0.0 {
                                                                    let wobble = (frame
                                                                        * loc_guard
                                                                            .template
                                                                            .wobble_rate
                                                                        + phase)
                                                                        .sin()
                                                                        * wobble_amp
                                                                        + wobble_min;
                                                                    roll_rate += wobble;
                                                                }
                                                            }
                                                            phys_guard.set_roll_rate(roll_rate);
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                        self.facing_direction = new_angle as Real;
                                        self.current_speed = new_speed;
                                        return Ok(());
                                    }
                                    Ok(None) => {
                                        self.movement_state = MovementState::Idle;
                                        self.target_position = None;
                                        self.path_following_state = None;
                                        self.current_speed = 0.0;
                                        completed_move = true;

                                        if !self.waypoint_queue.is_empty() {
                                            let next_waypoint = self.waypoint_queue.remove(0);
                                            handle_waypoint = Some(next_waypoint.position);
                                        }
                                    }
                                    Err(_) => {
                                        self.path_following_state = None;
                                        self.current_speed = 0.0;
                                    }
                                }
                            }
                        }
                    }
                }

                // Handle waypoint outside of the borrow scope
                if let Some(waypoint_pos) = handle_waypoint {
                    self.move_to_position(waypoint_pos, false)?;
                    return Ok(());
                }

                if completed_move && self.target_position.is_none() {
                    // Movement finished during pathfinding update; skip extra movement math.
                    // Completion is handled after the movement state switch below.
                } else {
                    let current_pos = self.get_position();
                    let path_len = self.current_path.as_ref().map(|path| path.len());
                    let (active_target, using_path) = if let Some(path) = &self.current_path {
                        if self.path_index < path.len() {
                            (
                                Coord3D::new(
                                    path[self.path_index].x,
                                    path[self.path_index].y,
                                    target.z,
                                ),
                                true,
                            )
                        } else {
                            (target, false)
                        }
                    } else {
                        (target, false)
                    };

                    let dx = current_pos.x - active_target.x;
                    let dy = current_pos.y - active_target.y;
                    let distance = (dx * dx + dy * dy).sqrt();
                    let reach_distance = if (using_path || !self.waypoint_queue.is_empty())
                        && self.path_extra_distance > 0.0
                    {
                        self.path_extra_distance
                    } else {
                        1.0
                    };

                    if distance < reach_distance {
                        if using_path {
                            self.path_index += 1;
                            if let Some(path_len) = path_len {
                                if self.path_index >= path_len {
                                    self.current_path = None;
                                    let final_dx = current_pos.x - target.x;
                                    let final_dy = current_pos.y - target.y;
                                    let final_distance =
                                        (final_dx * final_dx + final_dy * final_dy).sqrt();
                                    if final_distance < 1.0 {
                                        self.movement_state = MovementState::Idle;
                                        self.target_position = None;
                                        self.current_speed = 0.0;
                                        completed_move = true;

                                        // Process next waypoint if available
                                        if !self.waypoint_queue.is_empty() {
                                            let next_waypoint = self.waypoint_queue.remove(0);
                                            self.move_to_position(next_waypoint.position, false)?;
                                        }
                                    }
                                }
                            }
                        } else {
                            // Close enough
                            self.movement_state = MovementState::Idle;
                            self.target_position = None;
                            self.current_speed = 0.0;
                            completed_move = true;

                            // Process next waypoint if available
                            if !self.waypoint_queue.is_empty() {
                                let next_waypoint = self.waypoint_queue.remove(0);
                                self.move_to_position(next_waypoint.position, false)?;
                            }
                        }
                    } else {
                        // Continue moving towards target
                        if let Some(locomotor) = &self.current_locomotor {
                            if let Ok(mut loc_guard) = locomotor.lock() {
                                let effective_delta = delta_time * self.movement_speed_multiplier;
                                let current = self.get_position();
                                let prev_angle = self.facing_direction;
                                let (new_pos, new_angle, new_speed) = loc_guard.move_towards(
                                    current,
                                    prev_angle,
                                    self.current_speed,
                                    active_target,
                                    desired_speed,
                                    condition,
                                    effective_delta,
                                );
                                self.current_speed = new_speed;
                                if let Ok(mut obj_guard) = self.base_object.write() {
                                    let _ = obj_guard.set_position(&new_pos);
                                    let _ = obj_guard.set_orientation(new_angle as Real);
                                    if let Some(physics) = obj_guard.get_physics() {
                                        if let Ok(mut phys_guard) = physics.lock() {
                                            let delta = new_pos - current;
                                            let velocity = if effective_delta > 0.0 {
                                                delta / effective_delta.max(0.0001)
                                            } else {
                                                Vec3D::ZERO
                                            };
                                            phys_guard.set_velocity(&velocity);
                                            if effective_delta > 0.0 {
                                                let mut yaw_delta = new_angle - prev_angle;
                                                let two_pi = std::f32::consts::PI * 2.0;
                                                while yaw_delta > std::f32::consts::PI {
                                                    yaw_delta -= two_pi;
                                                }
                                                while yaw_delta < -std::f32::consts::PI {
                                                    yaw_delta += two_pi;
                                                }
                                                phys_guard.set_yaw_rate(
                                                    (yaw_delta / effective_delta.max(0.0001))
                                                        as Real,
                                                );
                                                let turning = if yaw_delta > 0.0 {
                                                    1
                                                } else if yaw_delta < 0.0 {
                                                    -1
                                                } else {
                                                    0
                                                };
                                                phys_guard.set_turning(turning);
                                                if matches!(
                                                    loc_guard.get_appearance(),
                                                    LocomotorAppearance::Thrust
                                                        | LocomotorAppearance::Wings
                                                        | LocomotorAppearance::Hover
                                                ) {
                                                    let pitch_rate =
                                                        loc_guard.template.pitch_by_z_vel_coef
                                                            * velocity.z;
                                                    let mut pitch_rate = pitch_rate;
                                                    if loc_guard.template.pitch_stiffness > 0.0 {
                                                        pitch_rate *=
                                                            loc_guard.template.pitch_stiffness;
                                                    }
                                                    if loc_guard.template.pitch_damping > 0.0 {
                                                        pitch_rate *= (1.0
                                                            - loc_guard.template.pitch_damping)
                                                            .clamp(0.0, 1.0);
                                                    }
                                                    phys_guard.set_pitch_rate(pitch_rate);
                                                    let mut roll_rate =
                                                        loc_guard.template.thrust_roll * new_speed;
                                                    if loc_guard.template.roll_stiffness > 0.0 {
                                                        roll_rate *=
                                                            loc_guard.template.roll_stiffness;
                                                    }
                                                    if loc_guard.template.roll_damping > 0.0 {
                                                        roll_rate *= (1.0
                                                            - loc_guard.template.roll_damping)
                                                            .clamp(0.0, 1.0);
                                                    }
                                                    if loc_guard.template.wobble_rate > 0.0 {
                                                        let frame =
                                                            TheGameLogic::get_frame() as f32;
                                                        let phase =
                                                            (obj_guard.get_id() as f32) * 0.01;
                                                        let wobble_min =
                                                            loc_guard.template.min_wobble;
                                                        let wobble_max =
                                                            loc_guard.template.max_wobble;
                                                        let wobble_amp =
                                                            wobble_max.max(wobble_min) - wobble_min;
                                                        if wobble_amp > 0.0 {
                                                            let wobble = (frame
                                                                * loc_guard.template.wobble_rate
                                                                + phase)
                                                                .sin()
                                                                * wobble_amp
                                                                + wobble_min;
                                                            roll_rate += wobble;
                                                        }
                                                    }
                                                    phys_guard.set_roll_rate(roll_rate);
                                                }
                                            }
                                        }
                                    }
                                }
                                self.facing_direction = new_angle;
                            }
                        }
                    }
                }
            }
        } else if self.movement_state == MovementState::TurningToFace {
            self.current_speed = 0.0;
            let angle_diff = self.desired_facing - self.facing_direction;
            let normalized_diff = Self::normalize_angle(angle_diff);

            if normalized_diff.abs() < 0.1 {
                // Finished turning
                self.facing_direction = self.desired_facing;
                self.movement_state = MovementState::Idle;
            } else {
                // Continue turning
                let turn_amount = self.turn_rate * delta_time;
                if normalized_diff > 0.0 {
                    self.facing_direction += turn_amount.min(normalized_diff);
                } else {
                    self.facing_direction += (-turn_amount).max(normalized_diff);
                }
            }
        } else {
            // Other movement states handled elsewhere
            self.current_speed = 0.0;
        }

        if completed_move {
            match prev_movement_state {
                MovementState::Patrolling => {
                    let order = self.current_order.take();
                    if let Some(UnitOrder::Patrol {
                        waypoints,
                        loop_patrol,
                    }) = order
                    {
                        let _ = self.process_patrol_order(&waypoints, loop_patrol);
                        self.current_order = Some(UnitOrder::Patrol {
                            waypoints,
                            loop_patrol,
                        });
                    }
                }
                MovementState::Following => {
                    let order = self.current_order.take();
                    if let Some(UnitOrder::Follow { target, distance }) = order {
                        let _ = self.process_follow_order(target, distance);
                        self.current_order = Some(UnitOrder::Follow { target, distance });
                    }
                }
                MovementState::Retreating => {
                    if matches!(self.current_order, Some(UnitOrder::Retreat { .. }))
                        && self.target_position.is_none()
                    {
                        self.current_order = None;
                        self.advance_order_queue();
                    }
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Update combat behavior
    fn update_combat(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.auto_acquire_enemies && self.attack_target.is_none() {
            return Ok(());
        }

        match self.combat_mode {
            CombatMode::Aggressive => {
                if self.attack_target.is_none() {
                    self.acquire_target()?;
                }
            }

            CombatMode::Defensive => {
                // Only attack if we're being attacked
                if self.is_under_attack() && self.attack_target.is_none() {
                    self.acquire_target()?;
                }
            }

            CombatMode::HoldPosition => {
                // Attack but don't move to engage
                if self.attack_target.is_none() {
                    self.acquire_target_in_range()?;
                }
            }

            CombatMode::HoldFire => {
                // Don't attack at all
                self.attack_target = None;
            }

            CombatMode::GuardArea => {
                // Only attack enemies in our guard area
                if self.attack_target.is_none() {
                    self.acquire_target_in_guard_area()?;
                    if self.attack_target.is_none() {
                        if let Some(guard_pos) = self.guard_position {
                            let current_pos = self.get_position();
                            let dx = guard_pos.x - current_pos.x;
                            let dy = guard_pos.y - current_pos.y;
                            let distance = (dx * dx + dy * dy).sqrt();
                            if distance > 1.0
                                && !self.is_movement_active()
                                && self.target_position.is_none()
                            {
                                self.move_to_position(guard_pos, false)?;
                            }
                        }
                    }
                }
            }
        }

        // Process attack if we have a target
        if let Some(target_id) = self.attack_target {
            self.engage_target(target_id, delta_time)?;
        } else if self.attack_move_active && self.is_movement_active() {
            self.acquire_target()?;
        }

        if self.attack_move_active && self.movement_state == MovementState::Attacking {
            const ATTACK_MOVE_SHOT_GRACE: u32 = 15;
            let current_frame = TheGameLogic::get_frame() as u32;
            let last_shot = self
                .base_object
                .read()
                .map(|guard| guard.get_last_shot_fired_frame())
                .unwrap_or(0);
            if current_frame >= self.attack_move_resume_frame
                && current_frame.saturating_sub(last_shot) > ATTACK_MOVE_SHOT_GRACE
            {
                self.movement_state = match self.current_order {
                    Some(UnitOrder::Patrol { .. }) => MovementState::Patrolling,
                    Some(UnitOrder::Follow { .. }) => MovementState::Following,
                    Some(UnitOrder::Guard { .. }) => MovementState::Guarding,
                    _ => MovementState::Moving,
                };
            }
        }

        if self.attack_move_active && self.movement_state == MovementState::Idle {
            let destination = match &self.current_order {
                Some(UnitOrder::AttackMove { destination, .. }) => *destination,
                Some(UnitOrder::Patrol { .. }) => {
                    return Ok(());
                }
                _ => {
                    self.attack_move_active = false;
                    return Ok(());
                }
            };

            let current_pos = self.get_position();
            let dx = destination.x - current_pos.x;
            let dy = destination.y - current_pos.y;
            let distance = (dx * dx + dy * dy).sqrt();
            if distance > 1.0 {
                self.move_to_position(destination, false)?;
            } else {
                self.attack_move_active = false;
                self.movement_state = MovementState::Idle;
            }
        }

        if !self.attack_move_active
            && self.attack_target.is_none()
            && self.movement_state == MovementState::Attacking
        {
            let mut resume_state = match self.current_order {
                Some(UnitOrder::Follow { .. }) => MovementState::Following,
                Some(UnitOrder::Patrol { .. }) => MovementState::Patrolling,
                Some(UnitOrder::Retreat { .. }) => MovementState::Retreating,
                Some(UnitOrder::Guard { .. }) => MovementState::Idle,
                Some(UnitOrder::Move { .. }) => MovementState::Moving,
                _ => MovementState::Idle,
            };
            if matches!(
                resume_state,
                MovementState::Moving
                    | MovementState::Following
                    | MovementState::Patrolling
                    | MovementState::Retreating
            ) && self.target_position.is_none()
            {
                resume_state = MovementState::Idle;
            }
            self.movement_state = resume_state;
        }

        if matches!(self.current_order, Some(UnitOrder::Attack { .. }))
            && self.attack_target.is_none()
        {
            self.current_order = None;
            self.advance_order_queue();
        }

        Ok(())
    }

    /// Issue a move order to the unit
    pub fn give_move_order(
        &mut self,
        destination: Coord3D,
        waypoints: Vec<Waypoint>,
        use_formation: bool,
        queue_order: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let order = UnitOrder::Move {
            destination,
            use_formation,
            waypoints,
        };

        if queue_order {
            self.order_queue.push(order);
        } else {
            self.current_order = Some(order);
            self.order_queue.clear();
        }

        Ok(())
    }

    /// Issue an attack order to the unit
    pub fn give_attack_order(
        &mut self,
        target: ObjectID,
        pursue: bool,
        queue_order: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let order = UnitOrder::Attack { target, pursue };

        if queue_order {
            self.order_queue.push(order);
        } else {
            self.current_order = Some(order);
            self.order_queue.clear();
        }

        self.attack_target = Some(target);

        Ok(())
    }

    /// Issue a capture building order to the unit.
    pub fn give_capture_order(
        &mut self,
        building: ObjectID,
        queue_order: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let order = UnitOrder::Capture { building };

        if queue_order {
            self.order_queue.push(order);
        } else {
            self.current_order = Some(order);
            self.order_queue.clear();
        }

        Ok(())
    }

    /// Set combat mode
    pub fn set_combat_mode(&mut self, mode: CombatMode) {
        self.combat_mode = mode;

        // Clear attack target if switching to hold fire
        if mode == CombatMode::HoldFire {
            self.attack_target = None;
        }
    }

    /// Check if unit can move
    pub fn can_move(&self) -> bool {
        !self.is_stunned
            && !self.is_pinned
            && !self.is_garrisoned
            && self.current_locomotor.is_some()
    }

    /// Check if unit can attack
    pub fn can_attack(&self) -> bool {
        !self.is_stunned
            && !self.is_suppressed
            && self.combat_mode != CombatMode::HoldFire
            && self.has_weapons()
    }

    /// Get current position
    pub fn get_position(&self) -> Coord3D {
        if let Ok(obj_guard) = self.base_object.read() {
            *obj_guard.get_position()
        } else {
            Coord3D::new(0.0, 0.0, 0.0)
        }
    }

    /// Get current health percentage
    pub fn get_health_percentage(&self) -> Real {
        if let Ok(obj_guard) = self.base_object.read() {
            let current = obj_guard.get_health();
            let max = obj_guard.get_max_health();
            if max > 0.0 {
                current / max
            } else {
                0.0
            }
        } else {
            0.0
        }
    }

    /// Check if unit has weapons
    pub fn has_weapons(&self) -> bool {
        if let Ok(obj_guard) = self.base_object.read() {
            obj_guard.has_any_weapon()
        } else {
            false
        }
    }

    /// Private helper methods
    fn process_move_order(
        &mut self,
        destination: Coord3D,
        use_formation: bool,
        waypoints: &[Waypoint],
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_move() {
            let should_repath = self
                .target_position
                .map(|pos| (pos - destination).length() > 0.1 || !self.is_movement_active())
                .unwrap_or(true);
            if should_repath {
                self.move_to_position(destination, use_formation)?;
                // Set up waypoint queue
                self.waypoint_queue = waypoints.to_vec();
            }
        }
        Ok(())
    }

    fn process_attack_order(
        &mut self,
        target: ObjectID,
        _pursue: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_attack() {
            self.attack_target = Some(target);
            // Additional attack logic would go here
        }
        Ok(())
    }

    fn move_to_position(
        &mut self,
        destination: Coord3D,
        _use_formation: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.target_position = Some(destination);
        self.movement_state = MovementState::Moving;
        self.current_speed = 0.0;
        self.current_path = None;
        self.path_index = 0;
        self.path_following_state = Some(PathFollowingState::new(destination));

        Ok(())
    }

    pub fn get_pathfind_layer(&self) -> PathfindLayerEnum {
        if self.can_fly {
            PathfindLayerEnum::Top
        } else {
            PathfindLayerEnum::Ground
        }
    }

    pub fn get_locomotor_surface_mask(&self) -> Option<LocomotorSurfaceTypeMask> {
        self.current_locomotor
            .as_ref()
            .and_then(|locomotor| locomotor.lock().ok())
            .map(|guard| guard.get_legal_surfaces())
    }

    pub fn get_crusher_level(&self) -> u32 {
        self.base_object
            .read()
            .map(|guard| guard.get_crusher_level())
            .unwrap_or(0)
    }

    fn stop_movement(&mut self) {
        self.movement_state = MovementState::Idle;
        self.target_position = None;
        self.current_path = None;
        self.path_following_state = None;
        self.current_speed = 0.0;
        self.attack_move_active = false;
        self.path_extra_distance = 0.0;
        self.attack_move_resume_frame = 0;
        self.attack_target_lock_until = 0;
        self.waypoint_queue.clear();
    }

    fn is_movement_active(&self) -> bool {
        matches!(
            self.movement_state,
            MovementState::Moving
                | MovementState::Following
                | MovementState::Patrolling
                | MovementState::Guarding
                | MovementState::Pursuing
                | MovementState::Retreating
                | MovementState::Backing
                | MovementState::Fleeing
        )
    }

    fn normalize_angle(angle: Real) -> Real {
        use std::f32::consts::PI;
        let mut result = angle;
        while result > PI {
            result -= 2.0 * PI;
        }
        while result < -PI {
            result += 2.0 * PI;
        }
        result
    }

    fn process_attack_move_order(
        &mut self,
        destination: Coord3D,
        engage_enemies: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_move() {
            let should_repath = self
                .target_position
                .map(|pos| (pos - destination).length() > 0.1 || !self.is_movement_active())
                .unwrap_or(true);

            if should_repath {
                self.move_to_position(destination, false)?;
            }
        }

        self.attack_target = None;
        self.auto_acquire_enemies = engage_enemies;
        if engage_enemies {
            self.combat_mode = CombatMode::Aggressive;
        }
        self.attack_move_active = true;
        Ok(())
    }
    fn process_guard_order(
        &mut self,
        position: Coord3D,
        area_radius: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.can_move() {
            let should_repath = self
                .target_position
                .map(|pos| (pos - position).length() > 0.1 || !self.is_movement_active())
                .unwrap_or(true);

            if should_repath {
                self.move_to_position(position, false)?;
            }
        }

        self.guard_position = Some(position);
        self.guard_radius = area_radius;
        self.combat_mode = CombatMode::GuardArea;
        self.auto_acquire_enemies = true;
        Ok(())
    }
    fn process_follow_order(
        &mut self,
        target: ObjectID,
        distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.follow_target = Some(target);
        self.follow_distance = distance.max(0.0);
        let target_obj = match crate::object::registry::OBJECT_REGISTRY.get_object(target) {
            Some(obj) => obj,
            None => {
                self.follow_target = None;
                self.current_order = None;
                self.advance_order_queue();
                return Ok(());
            }
        };
        let target_pos = target_obj.read().ok().map(|g| *g.get_position());
        let Some(target_pos) = target_pos else {
            return Ok(());
        };
        let current_pos = self.get_position();
        let dx = target_pos.x - current_pos.x;
        let dy = target_pos.y - current_pos.y;
        let distance_to_target = (dx * dx + dy * dy).sqrt();
        if distance_to_target > self.follow_distance + 1.0 {
            let mut desired = target_pos;
            if distance_to_target > 0.001 {
                let scale =
                    (distance_to_target - self.follow_distance).max(0.0) / distance_to_target;
                desired.x = current_pos.x + dx * scale;
                desired.y = current_pos.y + dy * scale;
            }
            if self.can_move() {
                let should_repath = self
                    .target_position
                    .map(|pos| (pos - desired).length() > 0.1 || !self.is_movement_active())
                    .unwrap_or(true);
                if should_repath {
                    self.move_to_position(desired, false)?;
                    self.movement_state = MovementState::Following;
                }
            }
        } else if matches!(
            self.movement_state,
            MovementState::Moving | MovementState::Following
        ) {
            self.movement_state = MovementState::Idle;
            self.target_position = None;
        }
        Ok(())
    }
    fn process_patrol_order(
        &mut self,
        waypoints: &[Coord3D],
        loop_patrol: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let is_same_path = !self.patrol_points.is_empty() && self.patrol_points == waypoints;
        if !is_same_path {
            self.patrol_points = waypoints.to_vec();
            self.current_patrol_index = 0;
            self.patrol_loop = loop_patrol;
        } else {
            self.patrol_loop = loop_patrol;
        }
        if self.patrol_points.is_empty() {
            self.current_order = None;
            self.advance_order_queue();
            return Ok(());
        }
        self.combat_mode = CombatMode::Aggressive;
        self.auto_acquire_enemies = true;
        self.attack_move_active = true;
        if self.can_move()
            && self.movement_state == MovementState::Idle
            && self.target_position.is_none()
        {
            if self.current_patrol_index >= self.patrol_points.len() {
                if self.patrol_loop {
                    self.current_patrol_index = 0;
                } else {
                    self.current_order = None;
                    self.advance_order_queue();
                    return Ok(());
                }
            }
            let dest = self.patrol_points[self.current_patrol_index];
            self.current_patrol_index = self.current_patrol_index.saturating_add(1);
            self.move_to_position(dest, false)?;
            self.movement_state = MovementState::Patrolling;
        }
        Ok(())
    }
    fn process_garrison_order(
        &mut self,
        building: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_garrisoned {
            self.current_order = None;
            self.advance_order_queue();
            return Ok(());
        }
        self.garrison_building = Some(building);
        let enter_command = self.base_object.read().ok().and_then(|obj_guard| {
            let ai = obj_guard.get_ai_update_interface()?;
            let cmd_source = ai
                .lock()
                .ok()
                .map(|ai_guard| ai_guard.get_last_command_source())
                .unwrap_or(CommandSourceType::FromPlayer);
            Some((ai, cmd_source))
        });
        if let Some((ai, cmd_source)) = enter_command {
            ai.ai_enter(building, cmd_source);
            self.current_order = None;
            self.advance_order_queue();
            return Ok(());
        }
        if let Some(container) = TheGameLogic::find_object_by_id(building) {
            if let Ok(container_guard) = container.read() {
                if let Some(contain) = container_guard.get_contain() {
                    if let Ok(mut contain_guard) = contain.lock() {
                        if let Ok(base_guard) = self.base_object.read() {
                            let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                &*base_guard,
                                crate::modules::ContainWant::WantsToEnter,
                            );
                        }
                    }
                }
                if self.can_move() && self.movement_state == MovementState::Idle {
                    self.move_to_position(*container_guard.get_position(), false)?;
                }
            }
        }
        Ok(())
    }
    fn process_ungarrison_order(
        &mut self,
        exit_position: Option<Coord3D>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let container_id = self
            .base_object
            .read()
            .ok()
            .and_then(|guard| guard.get_contained_by());
        if let Ok(obj_guard) = self.base_object.read() {
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                let cmd_source = ai
                    .lock()
                    .ok()
                    .map(|ai_guard| ai_guard.get_last_command_source())
                    .unwrap_or(CommandSourceType::FromPlayer);
                let mut params =
                    crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Exit, cmd_source);
                params.obj = container_id;
                let _ = ai
                    .lock()
                    .ok()
                    .map(|mut guard| guard.execute_command(&params));
            }
        }
        if let Some(container_id) = container_id {
            if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                if let Ok(container_guard) = container.read() {
                    if let Some(contain) = container_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            if let Ok(base_guard) = self.base_object.read() {
                                let _ = contain_guard.on_object_wants_to_enter_or_exit(
                                    &*base_guard,
                                    crate::modules::ContainWant::WantsToExit,
                                );
                            }
                        }
                    }
                }
            }
        }
        self.is_garrisoned = false;
        self.garrison_building = None;
        if let Some(pos) = exit_position {
            self.order_queue.insert(
                0,
                UnitOrder::Move {
                    destination: pos,
                    use_formation: false,
                    waypoints: Vec::new(),
                },
            );
        }
        self.current_order = None;
        self.advance_order_queue();
        Ok(())
    }
    fn process_capture_order(
        &mut self,
        building: ObjectID,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_capture_buildings {
            return Ok(());
        }

        let Some(building_arc) = TheGameLogic::find_object_by_id(building) else {
            return Ok(());
        };

        let (unit_pos, unit_radius, unit_player_id) = {
            let Ok(unit_guard) = self.base_object.read() else {
                return Ok(());
            };
            let player_id = unit_guard.get_player_id();
            let radius = unit_guard.get_geometry_info().get_bounding_circle_radius();
            (*unit_guard.get_position(), radius, player_id)
        };

        let (building_pos, building_radius, can_capture) = {
            let Ok(building_guard) = building_arc.read() else {
                return Ok(());
            };
            let radius = building_guard
                .get_geometry_info()
                .get_bounding_circle_radius();
            let can_capture = TheActionManager::can_capture_building(
                &*self.base_object.read().map_err(|_| "Unit lock poisoned")?,
                &*building_guard,
                CommandSourceType::FromAi,
            );
            (*building_guard.get_position(), radius, can_capture)
        };

        if !can_capture {
            return Ok(());
        }

        let dx = unit_pos.x - building_pos.x;
        let dy = unit_pos.y - building_pos.y;
        let dist_sq = dx * dx + dy * dy;
        let capture_range = unit_radius + building_radius + PATHFIND_CLOSE_ENOUGH;

        if dist_sq > capture_range * capture_range {
            self.move_to_position(building_pos, false)?;
            return Ok(());
        }

        let Some(player_id) = unit_player_id else {
            return Ok(());
        };

        if let Ok(mut factory) = get_object_factory().write() {
            if let Some(GameObjectInstance::Structure(structure_arc)) =
                factory.get_object_mut(building)
            {
                if let Ok(mut structure_guard) = structure_arc.write() {
                    let _ = structure_guard.mark_capture_activity(player_id);
                }
            }
        }
        Ok(())
    }
    fn process_retreat_order(
        &mut self,
        safe_position: Coord3D,
        _organized: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.attack_target = None;
        self.attack_move_active = false;
        self.auto_acquire_enemies = false;

        if self.can_move() {
            let should_repath = self
                .target_position
                .map(|pos| (pos - safe_position).length() > 0.1 || !self.is_movement_active())
                .unwrap_or(true);
            if should_repath {
                self.move_to_position(safe_position, false)?;
                self.movement_state = MovementState::Retreating;
            }
        }
        Ok(())
    }
    fn look_for_enemies(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.auto_acquire_enemies {
            return Ok(());
        }

        if self.auto_acquire_not_while_attacking && self.is_currently_attacking() {
            return Ok(());
        }

        if !self.auto_acquire_while_stealthed
            && self
                .base_object
                .read()
                .map(|guard| guard.is_stealthed())
                .unwrap_or(false)
        {
            return Ok(());
        }

        match self.combat_mode {
            CombatMode::GuardArea => self.acquire_target_in_guard_area()?,
            CombatMode::HoldPosition | CombatMode::HoldFire => self.acquire_target_in_range()?,
            CombatMode::Aggressive | CombatMode::Defensive => self.acquire_target()?,
        }

        Ok(())
    }
    fn return_to_formation_position(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let leader_id = match self.group_leader {
            Some(id) => id,
            None => {
                self.return_to_formation = false;
                return Ok(());
            }
        };
        let leader = match crate::object::registry::OBJECT_REGISTRY.get_object(leader_id) {
            Some(obj) => obj,
            None => {
                self.group_leader = None;
                self.return_to_formation = false;
                return Ok(());
            }
        };
        let leader_pos = leader.read().ok().map(|g| *g.get_position());
        let Some(leader_pos) = leader_pos else {
            self.return_to_formation = false;
            return Ok(());
        };
        let current_pos = self.get_position();
        let dx = leader_pos.x - current_pos.x;
        let dy = leader_pos.y - current_pos.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance > self.follow_distance && self.can_move() && !self.is_movement_active() {
            self.move_to_position(leader_pos, false)?;
        } else if distance <= self.follow_distance {
            self.return_to_formation = false;
        }
        Ok(())
    }
    fn update_facing(
        &mut self,
        delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let target_angle = self.desired_facing;
        let current = self.facing_direction;
        let mut delta = target_angle - current;
        let two_pi = std::f32::consts::PI * 2.0;
        while delta > std::f32::consts::PI {
            delta -= two_pi;
        }
        while delta < -std::f32::consts::PI {
            delta += two_pi;
        }
        let max_turn = (self.turn_rate.max(0.0) * delta_time).max(0.0);
        let adjust = delta.clamp(-max_turn, max_turn);
        let new_angle = current + adjust;
        self.facing_direction = new_angle;
        if let Ok(mut obj_guard) = self.base_object.write() {
            let _ = obj_guard.set_orientation(new_angle as Real);
        }
        Ok(())
    }
    fn check_status_effects(
        &mut self,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let contained_by = self
            .base_object
            .read()
            .ok()
            .and_then(|guard| guard.get_contained_by());
        match contained_by {
            Some(container) => {
                self.is_garrisoned = true;
                self.garrison_building = Some(container);
                if matches!(self.current_order, Some(UnitOrder::Garrison { .. })) {
                    self.current_order = None;
                    self.advance_order_queue();
                }
                self.stop_movement();
            }
            None => {
                self.is_garrisoned = false;
                self.garrison_building = None;
            }
        }
        Ok(())
    }
    fn update_animation_state(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(mut obj_guard) = self.base_object.write() {
            match self.movement_state {
                MovementState::Moving
                | MovementState::TurningToFace
                | MovementState::Following
                | MovementState::Patrolling
                | MovementState::Guarding
                | MovementState::Pursuing
                | MovementState::Retreating
                | MovementState::Backing
                | MovementState::Fleeing => {
                    obj_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                }
                MovementState::Idle | MovementState::Attacking => {
                    obj_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
            }
            if matches!(self.movement_state, MovementState::Attacking) {
                obj_guard.set_model_condition_state(ModelConditionFlags::ATTACKING);
            } else {
                obj_guard.clear_model_condition_state(ModelConditionFlags::ATTACKING);
            }
        }
        Ok(())
    }
    fn acquire_target(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_auto_acquire_now() {
            return Ok(());
        }

        if !self.should_scan_for_targets(self.engagement_range) {
            return Ok(());
        }

        if let Some((target_id, _)) = self.find_closest_enemy_with_buildings(
            self.get_position(),
            self.engagement_range,
            self.engagement_range,
        ) {
            self.attack_target = Some(target_id);
        }
        Ok(())
    }
    fn acquire_target_in_range(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_auto_acquire_now() {
            return Ok(());
        }

        if !self.should_scan_for_targets(self.engagement_range) {
            return Ok(());
        }

        if let Some((target_id, _)) = self.find_closest_enemy_with_buildings(
            self.get_position(),
            self.engagement_range,
            self.engagement_range,
        ) {
            self.attack_target = Some(target_id);
        }
        Ok(())
    }
    fn acquire_target_in_guard_area(
        &mut self,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if !self.can_auto_acquire_now() {
            return Ok(());
        }

        let guard_pos = match self.guard_position {
            Some(pos) => pos,
            None => return Ok(()),
        };

        if !self.should_scan_for_targets(self.guard_radius) {
            return Ok(());
        }

        let guard_radius = if self.guard_radius > 0.0 {
            self.guard_radius
        } else {
            self.engagement_range
        };

        if let Some((target_id, _)) =
            self.find_closest_enemy_with_buildings(guard_pos, guard_radius, self.engagement_range)
        {
            self.attack_target = Some(target_id);
        }
        Ok(())
    }
    fn engage_target(
        &mut self,
        target_id: ObjectID,
        _delta_time: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (target_pos, target_relationship, detected) =
            match crate::object::registry::OBJECT_REGISTRY.get_object(target_id) {
                Some(obj) => {
                    let guard = obj.read().ok();
                    let pos = guard.as_ref().map(|g| *g.get_position());
                    let rel = guard
                        .as_ref()
                        .and_then(|g| {
                            self.base_object
                                .read()
                                .ok()
                                .map(|me| me.relationship_to(&g))
                        })
                        .unwrap_or(Relationship::Neutral);
                    let detected = guard.map(|g| g.is_detected()).unwrap_or(false);
                    (pos, rel, detected)
                }
                None => (None, Relationship::Neutral, false),
            };

        let target_pos = match target_pos {
            Some(pos) => pos,
            None => {
                self.attack_target = None;
                return Ok(());
            }
        };

        if !matches!(target_relationship, Relationship::Enemies) {
            self.attack_target = None;
            return Ok(());
        }

        let current_pos = self.get_position();
        let dx = target_pos.x - current_pos.x;
        let dy = target_pos.y - current_pos.y;
        let distance = (dx * dx + dy * dy).sqrt();

        if !detected && !self.can_detect_target_distance(distance) {
            self.attack_target = None;
            return Ok(());
        }

        if distance > self.engagement_range {
            if self.attack_move_active {
                self.attack_target = None;
                return Ok(());
            }

            if self.can_move() && !self.is_movement_active() {
                self.move_to_position(target_pos, false)?;
            }
        } else {
            if matches!(self.combat_mode, CombatMode::GuardArea) {
                if let Some(guard_pos) = self.guard_position {
                    let guard_radius = if self.guard_radius > 0.0 {
                        self.guard_radius
                    } else {
                        self.engagement_range
                    };
                    let dx_guard = target_pos.x - guard_pos.x;
                    let dy_guard = target_pos.y - guard_pos.y;
                    let dist_guard = (dx_guard * dx_guard + dy_guard * dy_guard).sqrt();
                    if dist_guard > guard_radius {
                        self.attack_target = None;
                        return Ok(());
                    }
                }
            }
            self.movement_state = MovementState::Attacking;
            if self.attack_move_active {
                const ATTACK_MOVE_PAUSE_FRAMES: u32 = 30;
                let current_frame = TheGameLogic::get_frame() as u32;
                self.attack_move_resume_frame =
                    current_frame.saturating_add(ATTACK_MOVE_PAUSE_FRAMES);
            }
            const TARGET_LOCK_FRAMES: u32 = 30;
            let current_frame = TheGameLogic::get_frame() as u32;
            self.attack_target_lock_until = current_frame.saturating_add(TARGET_LOCK_FRAMES);
        }

        Ok(())
    }
    fn should_scan_for_targets(&mut self, max_distance: Real) -> bool {
        const TARGET_LOCK_GRACE: u32 = 30;

        let current_frame = TheGameLogic::get_frame() as u32;
        if let Ok(guard) = self.base_object.read() {
            let last_shot = guard.get_last_shot_fired_frame();
            if current_frame.saturating_sub(last_shot) < TARGET_LOCK_GRACE {
                return false;
            }
        }
        if current_frame < self.attack_target_lock_until {
            return false;
        }

        if let Some(target_id) = self.attack_target {
            if let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(target_id) {
                if let Ok(target_guard) = obj.read() {
                    if matches!(
                        self.base_object
                            .read()
                            .ok()
                            .map(|guard| guard.relationship_to(&target_guard)),
                        Some(Relationship::Enemies)
                    ) {
                        let target_pos = *target_guard.get_position();
                        let self_pos = self.get_position();
                        let dx = target_pos.x - self_pos.x;
                        let dy = target_pos.y - self_pos.y;
                        let distance = (dx * dx + dy * dy).sqrt();
                        if distance <= max_distance
                            && self.can_detect_target(&target_guard, distance)
                        {
                            return false;
                        }
                    }
                }
            }
        }
        let interval = self.mood_attack_check_rate_frames.max(1);
        if current_frame.saturating_sub(self.last_target_scan_frame) < interval {
            return false;
        }

        self.last_target_scan_frame = current_frame;
        true
    }
    fn find_closest_enemy(
        &self,
        center: Coord3D,
        max_distance: Real,
        vision_distance: Real,
    ) -> Option<(ObjectID, Real)> {
        let all_objects = crate::object::registry::OBJECT_REGISTRY.get_all_objects();
        let self_id = self
            .base_object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(0);
        let mut closest: Option<(ObjectID, Real)> = None;

        if let Some(current_target) = self.attack_target {
            if let Some(obj) = crate::object::registry::OBJECT_REGISTRY.get_object(current_target) {
                if let Ok(target_guard) = obj.read() {
                    if matches!(
                        self.base_object
                            .read()
                            .ok()
                            .map(|guard| guard.relationship_to(&target_guard)),
                        Some(Relationship::Enemies)
                    ) {
                        let target_pos = *target_guard.get_position();
                        let dx_center = target_pos.x - center.x;
                        let dy_center = target_pos.y - center.y;
                        let dist_to_center = (dx_center * dx_center + dy_center * dy_center).sqrt();
                        let self_pos = self.get_position();
                        let dx_self = target_pos.x - self_pos.x;
                        let dy_self = target_pos.y - self_pos.y;
                        let dist_to_self = (dx_self * dx_self + dy_self * dy_self).sqrt();

                        if dist_to_center <= max_distance
                            && dist_to_self <= vision_distance
                            && self.can_detect_target(&target_guard, dist_to_self)
                        {
                            closest = Some((current_target, dist_to_self * 1.1));
                        }
                    }
                }
            }
        }

        for obj in all_objects {
            let obj_guard = match obj.read() {
                Ok(guard) => guard,
                Err(_) => continue,
            };

            let obj_id = obj_guard.get_id();
            if obj_id == self_id {
                continue;
            }

            if !obj_guard.is_kind_of(KindOf::Unit) {
                continue;
            }

            if !matches!(
                self.base_object
                    .read()
                    .ok()
                    .map(|guard| guard.relationship_to(&obj_guard)),
                Some(Relationship::Enemies)
            ) {
                continue;
            }

            let obj_pos = *obj_guard.get_position();
            let dx_center = obj_pos.x - center.x;
            let dy_center = obj_pos.y - center.y;
            let dist_to_center = (dx_center * dx_center + dy_center * dy_center).sqrt();

            if dist_to_center > max_distance {
                continue;
            }

            let self_pos = self.get_position();
            let dx_self = obj_pos.x - self_pos.x;
            let dy_self = obj_pos.y - self_pos.y;
            let dist_to_self = (dx_self * dx_self + dy_self * dy_self).sqrt();

            if dist_to_self > vision_distance {
                continue;
            }

            if !self.can_detect_target(&obj_guard, dist_to_self) {
                continue;
            }

            let mut weighted_dist = dist_to_self;
            if self.is_under_attack() {
                weighted_dist *= 0.9;
            }
            if dist_to_self <= self.engagement_range {
                weighted_dist *= 0.8;
            }

            match closest {
                Some((current_id, best_dist)) if weighted_dist >= best_dist => {
                    // Keep current target unless new target is meaningfully closer.
                    if current_id == obj_id {
                        closest = Some((obj_id, weighted_dist));
                    }
                }
                _ => closest = Some((obj_id, weighted_dist)),
            }
        }

        closest
    }

    fn find_closest_enemy_with_buildings(
        &self,
        center: Coord3D,
        max_distance: Real,
        vision_distance: Real,
    ) -> Option<(ObjectID, Real)> {
        if !self.auto_acquire_attack_buildings {
            return self.find_closest_enemy(center, max_distance, vision_distance);
        }

        let all_objects = crate::object::registry::OBJECT_REGISTRY.get_all_objects();
        let self_id = self
            .base_object
            .read()
            .map(|guard| guard.get_id())
            .unwrap_or(0);
        let mut closest: Option<(ObjectID, Real)> = None;

        for obj in all_objects {
            let obj_guard = match obj.read() {
                Ok(guard) => guard,
                Err(_) => continue,
            };

            let obj_id = obj_guard.get_id();
            if obj_id == self_id {
                continue;
            }

            let is_unit = obj_guard.is_kind_of(KindOf::Unit);
            let is_structure = obj_guard.is_kind_of(KindOf::Structure);
            if !is_unit && !is_structure {
                continue;
            }

            if !matches!(
                self.base_object
                    .read()
                    .ok()
                    .map(|guard| guard.relationship_to(&obj_guard)),
                Some(Relationship::Enemies)
            ) {
                continue;
            }

            let obj_pos = *obj_guard.get_position();
            let dx_center = obj_pos.x - center.x;
            let dy_center = obj_pos.y - center.y;
            let dist_to_center = (dx_center * dx_center + dy_center * dy_center).sqrt();

            if dist_to_center > max_distance {
                continue;
            }

            let self_pos = self.get_position();
            let dx_self = obj_pos.x - self_pos.x;
            let dy_self = obj_pos.y - self_pos.y;
            let dist_to_self = (dx_self * dx_self + dy_self * dy_self).sqrt();

            if dist_to_self > vision_distance {
                continue;
            }

            if !self.can_detect_target(&obj_guard, dist_to_self) {
                continue;
            }

            let mut weighted_dist = dist_to_self;
            if self.is_under_attack() {
                weighted_dist *= 0.9;
            }
            if dist_to_self <= self.engagement_range {
                weighted_dist *= 0.8;
            }

            match closest {
                Some((current_id, best_dist)) if weighted_dist >= best_dist => {
                    if current_id == obj_id {
                        closest = Some((obj_id, weighted_dist));
                    }
                }
                _ => closest = Some((obj_id, weighted_dist)),
            }
        }

        closest
    }
    fn can_detect_target(&self, target: &Object, distance: Real) -> bool {
        if target.is_detected() {
            return true;
        }

        self.can_detect_target_distance(distance)
    }

    fn can_detect_target_distance(&self, distance: Real) -> bool {
        let base_range = self
            .base_object
            .read()
            .ok()
            .map(|guard| guard.get_stealth_detection_range() as Real)
            .unwrap_or(0.0);
        let detection_range = self.stealth_detection_range.max(base_range);

        if detection_range <= 0.0 {
            return false;
        }

        distance <= detection_range
    }
    fn is_under_attack(&self) -> bool {
        let Some(body) = self
            .base_object
            .read()
            .ok()
            .and_then(|guard| guard.get_body_module())
        else {
            return false;
        };

        let Ok(body_guard) = body.lock() else {
            return false;
        };

        let Some(last) = body_guard.get_last_damage_info() else {
            return false;
        };

        if matches!(
            last.input.damage_type,
            DamageType::Healing | DamageType::Penalty
        ) {
            return false;
        }

        let last_frame = body_guard.get_last_damage_timestamp();
        if last_frame == u32::MAX {
            return false;
        }

        let current_frame = TheGameLogic::get_frame() as u32;
        current_frame.saturating_sub(last_frame) <= LOGICFRAMES_PER_SECOND
    }

    fn is_currently_attacking(&self) -> bool {
        matches!(
            self.current_order,
            Some(UnitOrder::Attack { .. }) | Some(UnitOrder::AttackMove { .. })
        ) || self.movement_state == MovementState::Attacking
    }

    fn can_auto_acquire_now(&self) -> bool {
        if !self.auto_acquire_enemies {
            return false;
        }

        if self.auto_acquire_not_while_attacking && self.is_currently_attacking() {
            return false;
        }

        if !self.auto_acquire_while_stealthed {
            let stealthed = self
                .base_object
                .read()
                .map(|guard| guard.is_stealthed())
                .unwrap_or(false);
            if stealthed {
                return false;
            }
        }

        true
    }
}

/// Extension trait for Object to provide Unit-specific functionality
pub trait UnitExt {
    /// Get unit-specific data if this object is a unit
    fn as_unit(&self) -> Option<&Unit>;
    fn as_unit_mut(&mut self) -> Option<&mut Unit>;
}

#[derive(Debug, Clone)]
struct RappelState {
    rappel_rate: Real,
    dest_z: Real,
    target_is_bldg: bool,
    target_id: Option<ObjectID>,
}

fn find_enemy_in_container(killer_id: ObjectID, container_id: ObjectID) -> Option<ObjectID> {
    let container = crate::object::registry::OBJECT_REGISTRY.get_object(container_id)?;
    let contained_ids = {
        let guard = container.read().ok()?;
        let contain = guard.get_contain()?;
        let contain_guard = contain.lock().ok()?;
        contain_guard.get_contained_objects().to_vec()
    };

    for id in contained_ids {
        let enemy = crate::object::registry::OBJECT_REGISTRY.get_object(id)?;
        let enemy_guard = enemy.read().ok()?;
        if enemy_guard.is_effectively_dead() {
            continue;
        }
        let Some(killer) = crate::object::registry::OBJECT_REGISTRY.get_object(killer_id) else {
            continue;
        };
        let killer_guard = killer.read().ok()?;
        if killer_guard.relationship_to(&enemy_guard) == Relationship::Enemies {
            return Some(id);
        }
    }
    None
}

fn kill_enemies_in_container(killer_id: ObjectID, container_id: ObjectID, max_to_kill: i32) -> i32 {
    let mut num_killed = 0;
    while num_killed < max_to_kill {
        let Some(enemy_id) = find_enemy_in_container(killer_id, container_id) else {
            break;
        };

        if let Some(enemy) = crate::object::registry::OBJECT_REGISTRY.get_object(enemy_id) {
            if let Ok(mut enemy_guard) = enemy.write() {
                if let Some(contained_by_id) = enemy_guard.get_contained_by() {
                    if let Some(container) =
                        crate::object::registry::OBJECT_REGISTRY.get_object(contained_by_id)
                    {
                        if let Ok(container_guard) = container.read() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    let _ = contain_guard.release_object(enemy_id);
                                }
                            }
                        }
                    }
                }

                if let Some(killer) = crate::object::registry::OBJECT_REGISTRY.get_object(killer_id)
                {
                    if let Ok(mut killer_guard) = killer.write() {
                        killer_guard.score_the_kill(&enemy_guard);
                    }
                }
                enemy_guard.kill(None, None);
                num_killed += 1;
            }
        } else {
            break;
        }
    }

    num_killed
}

fn to_locomotor_body_damage_type(value: crate::common::BodyDamageType) -> BodyDamageType {
    match value {
        crate::common::BodyDamageType::Pristine => BodyDamageType::Pristine,
        crate::common::BodyDamageType::Damaged => BodyDamageType::Damaged,
        crate::common::BodyDamageType::ReallyDamaged => BodyDamageType::ReallyDamaged,
        crate::common::BodyDamageType::Rubble => BodyDamageType::Rubble,
    }
}

fn xfer_unit_coord3d(xfer: &mut dyn Xfer, coord: &mut Coord3D) -> Result<(), String> {
    xfer.xfer_real(&mut coord.x).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut coord.y).map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut coord.z).map_err(|e| e.to_string())?;
    Ok(())
}

fn xfer_unit_icoord2d(xfer: &mut dyn Xfer, coord: &mut ICoord2D) -> Result<(), String> {
    xfer.xfer_int(&mut coord.x).map_err(|e| e.to_string())?;
    xfer.xfer_int(&mut coord.y).map_err(|e| e.to_string())?;
    Ok(())
}

fn guard_target_type_from_u32(value: u32) -> Result<GuardTargetType, String> {
    match value {
        0 => Ok(GuardTargetType::Location),
        1 => Ok(GuardTargetType::Object),
        2 => Ok(GuardTargetType::Area),
        3 => Ok(GuardTargetType::None_),
        _ => Err(format!("Invalid AIUpdate guard target type {value}")),
    }
}

fn xfer_guard_target_type(
    xfer: &mut dyn Xfer,
    guard_target_type: &mut GuardTargetType,
) -> Result<(), String> {
    let mut value = *guard_target_type as u32;
    xfer.xfer_unsigned_int(&mut value)
        .map_err(|e| e.to_string())?;
    *guard_target_type = guard_target_type_from_u32(value)?;
    Ok(())
}

fn locomotor_set_type_from_i32(value: i32) -> Result<LocomotorSetType, String> {
    match value {
        -1 => Ok(LocomotorSetType::Invalid),
        0 => Ok(LocomotorSetType::Normal),
        1 => Ok(LocomotorSetType::NormalUpgraded),
        2 => Ok(LocomotorSetType::Freefall),
        3 => Ok(LocomotorSetType::Wander),
        4 => Ok(LocomotorSetType::Panic),
        5 => Ok(LocomotorSetType::Taxiing),
        6 => Ok(LocomotorSetType::Supersonic),
        7 => Ok(LocomotorSetType::Sluggish),
        _ => Err(format!("Invalid AIUpdate locomotor set type {value}")),
    }
}

/// Basic AI update interface that bridges AI commands to unit orders.
pub struct UnitAIUpdate {
    unit: Weak<RwLock<Unit>>,
    crate_created: Mutex<ObjectID>,
    supply_truck_ai: Option<SupplyTruckAIUpdate>,
    chinook_ai: Option<ChinookAIUpdate>,
    jet_ai: Option<JetAIUpdate>,
    worker_ai: Option<WorkerAIUpdate>,
    dozer_ai: Option<DozerAIUpdate>,
    #[cfg(feature = "allow_surrender")]
    pow_truck_ai: Option<POWTruckAIUpdate>,
    railed_transport_ai: Option<RailedTransportAIUpdate>,
    hack_internet_ai: Option<HackInternetAIUpdate>,
    assault_transport_ai: Option<AssaultTransportAIUpdate>,
    deliver_payload_ai: Option<DeliverPayloadAIUpdate>,
    transport_ai: Option<TransportAIUpdate>,
    deploy_style_ai: Option<DeployStyleAIUpdate>,
    wander_ai: Option<WanderAIUpdate>,
    dock_machine: Option<AIDockMachine>,
    ai_state_machine: Option<Arc<Mutex<AIStateMachine>>>,
    can_path_through_units: bool,
    allow_chase: bool,
    attitude: AIAttitudeType,
    last_command_source: CommandSourceType,
    current_command: Option<crate::ai::AiCommandType>,
    pending_command: Option<crate::ai::AiCommandType>,
    surrendered_frames_left: UnsignedInt,
    surrendered_player_index: Option<PlayerIndex>,
    surrender_duration_frames: UnsignedInt,
    demoralized_frames_left: UnsignedInt,
    auto_acquire_enemies_when_idle: u32,
    mood_attack_check_rate_frames: UnsignedInt,
    forbid_player_commands: Bool,
    turrets_linked: Bool,
    turret_primary_data: Option<TurretAIData>,
    turret_secondary_data: Option<TurretAIData>,
    locomotor_upgraded: Bool,
    current_locomotor_set: LocomotorSetType,
    locomotor_sets: HashMap<LocomotorSetType, Vec<AsciiString>>,
    turret_primary_enabled: Bool,
    turret_secondary_enabled: Bool,
    turret_primary_natural: Bool,
    turret_secondary_natural: Bool,
    turret_primary_machine: Option<TurretStateMachine>,
    turret_secondary_machine: Option<TurretStateMachine>,
    enter_target: Option<ObjectID>,
    desired_speed: Real,
    prior_waypoint_id: Option<crate::waypoint::WaypointId>,
    current_waypoint_id: Option<crate::waypoint::WaypointId>,
    completed_waypoint_id: Option<crate::waypoint::WaypointId>,
    current_goal_path_index: i32,
    rappel_state: Option<RappelState>,
    original_victim_pos: Option<Coord3D>,
    pending_safe_path: Option<Vec<Coord3D>>,
    guard_target_type: [GuardTargetType; 2],
    location_to_guard: Coord3D,
    object_to_guard: ObjectID,
    planning_waypoint_queue: [Coord3D; AI_UPDATE_MAX_WAYPOINTS],
    planning_waypoint_count: Int,
    planning_waypoint_index: Int,
    executing_waypoint_queue: Bool,
    requested_victim_id: ObjectID,
    requested_destination: Coord3D,
    requested_destination2: Coord3D,
    current_path_snapshot: Option<AiPath>,
    pathfind_goal_cell: ICoord2D,
    pathfind_cur_cell: ICoord2D,
    pathfind_goal_layer: ClassicPathLayer,
    move_out_of_way_1: ObjectID,
    move_out_of_way_2: ObjectID,
    repulsor1: ObjectID,
    repulsor2: ObjectID,
    ignore_obstacle_id: ObjectID,
    ignore_collisions_until: UnsignedInt,
    waiting_for_path: Bool,
    queue_for_path_frame: UnsignedInt,
    path_timestamp: UnsignedInt,
    ai_dead: Bool,
    is_recruitable: Bool,
    next_enemy_scan_time: UnsignedInt,
    final_position: Coord3D,
    do_final_position: Bool,
    is_attack_path: Bool,
    is_final_goal: Bool,
    is_approach_path: Bool,
    is_safe_path: Bool,
    movement_complete: Bool,
    locomotor_goal_type: u32,
    locomotor_goal_data: Coord3D,
    is_blocked: Bool,
    blocked_and_stuck: Bool,
    retry_path: Bool,
    blocked_frames: u32,
    cur_max_blocked_speed: Real,
    bump_speed_limit: Real,
}

impl std::fmt::Debug for UnitAIUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("UnitAIUpdate")
            .field("can_path_through_units", &self.can_path_through_units)
            .field("allow_chase", &self.allow_chase)
            .field("last_command_source", &self.last_command_source)
            .field("current_command", &self.current_command)
            .field("pending_command", &self.pending_command)
            .field("ai_dead", &self.ai_dead)
            .finish()
    }
}

impl UnitAIUpdate {
    pub fn new(
        unit: Weak<RwLock<Unit>>,
        supply_truck_ai: Option<SupplyTruckAIUpdate>,
        chinook_ai: Option<ChinookAIUpdate>,
        jet_ai: Option<JetAIUpdate>,
        worker_ai: Option<WorkerAIUpdate>,
        dozer_ai: Option<DozerAIUpdate>,
        #[cfg(feature = "allow_surrender")] pow_truck_ai: Option<POWTruckAIUpdate>,
        railed_transport_ai: Option<RailedTransportAIUpdate>,
        hack_internet_ai: Option<HackInternetAIUpdate>,
        assault_transport_ai: Option<AssaultTransportAIUpdate>,
        deliver_payload_ai: Option<DeliverPayloadAIUpdate>,
        transport_ai: Option<TransportAIUpdate>,
        deploy_style_ai: Option<DeployStyleAIUpdate>,
        wander_ai: Option<WanderAIUpdate>,
    ) -> Self {
        let ai_state_machine = unit.upgrade().and_then(|unit_arc| {
            let owner = unit_arc
                .read()
                .ok()
                .map(|guard| Arc::downgrade(&guard.base_object))?;
            Some(Arc::new(Mutex::new(AIStateMachine::new(
                owner,
                "AIStateMachine",
            ))))
        });

        Self {
            unit,
            crate_created: Mutex::new(crate::common::INVALID_ID),
            supply_truck_ai,
            chinook_ai,
            jet_ai,
            worker_ai,
            dozer_ai,
            #[cfg(feature = "allow_surrender")]
            pow_truck_ai,
            railed_transport_ai,
            hack_internet_ai,
            assault_transport_ai,
            deliver_payload_ai,
            transport_ai,
            deploy_style_ai,
            wander_ai,
            dock_machine: None,
            ai_state_machine,
            can_path_through_units: false,
            allow_chase: false,
            attitude: AIAttitudeType::Normal,
            last_command_source: CommandSourceType::FromAi,
            current_command: None,
            pending_command: None,
            surrendered_frames_left: 0,
            surrendered_player_index: None,
            surrender_duration_frames: LOGICFRAMES_PER_SECOND * 120,
            demoralized_frames_left: 0,
            auto_acquire_enemies_when_idle: 0,
            mood_attack_check_rate_frames: LOGICFRAMES_PER_SECOND * 2,
            forbid_player_commands: false,
            turrets_linked: false,
            turret_primary_data: None,
            turret_secondary_data: None,
            locomotor_upgraded: false,
            current_locomotor_set: LocomotorSetType::Normal,
            locomotor_sets: HashMap::new(),
            turret_primary_enabled: true,
            turret_secondary_enabled: true,
            turret_primary_natural: true,
            turret_secondary_natural: true,
            turret_primary_machine: None,
            turret_secondary_machine: None,
            enter_target: None,
            desired_speed: FAST_AS_POSSIBLE,
            prior_waypoint_id: None,
            current_waypoint_id: None,
            completed_waypoint_id: None,
            current_goal_path_index: -1,
            rappel_state: None,
            original_victim_pos: None,
            pending_safe_path: None,
            guard_target_type: [GuardTargetType::None_; 2],
            location_to_guard: Coord3D::ZERO,
            object_to_guard: INVALID_ID,
            planning_waypoint_queue: [Coord3D::ZERO; AI_UPDATE_MAX_WAYPOINTS],
            planning_waypoint_count: 0,
            planning_waypoint_index: 0,
            executing_waypoint_queue: false,
            requested_victim_id: INVALID_ID,
            requested_destination: Coord3D::ZERO,
            requested_destination2: Coord3D::ZERO,
            current_path_snapshot: None,
            pathfind_goal_cell: ICoord2D::new(-1, -1),
            pathfind_cur_cell: ICoord2D::new(-1, -1),
            pathfind_goal_layer: ClassicPathLayer::Invalid,
            move_out_of_way_1: INVALID_ID,
            move_out_of_way_2: INVALID_ID,
            repulsor1: INVALID_ID,
            repulsor2: INVALID_ID,
            ignore_obstacle_id: INVALID_ID,
            ignore_collisions_until: 0,
            waiting_for_path: false,
            queue_for_path_frame: 0,
            path_timestamp: 0,
            ai_dead: false,
            is_recruitable: true,
            next_enemy_scan_time: 0,
            final_position: Coord3D::ZERO,
            do_final_position: false,
            is_attack_path: false,
            is_final_goal: false,
            is_approach_path: false,
            is_safe_path: false,
            movement_complete: false,
            locomotor_goal_type: 0,
            locomotor_goal_data: Coord3D::ZERO,
            is_blocked: false,
            blocked_and_stuck: false,
            retry_path: false,
            blocked_frames: 0,
            cur_max_blocked_speed: FAST_AS_POSSIBLE,
            bump_speed_limit: FAST_AS_POSSIBLE,
        }
    }

    fn push_guard_target_type(&mut self, target_type: GuardTargetType) {
        if self.guard_target_type[1] == GuardTargetType::None_ {
            self.guard_target_type[1] = target_type;
        } else {
            self.guard_target_type[0] = target_type;
        }
    }

    fn clear_guard_target_type(&mut self) {
        self.guard_target_type[1] = self.guard_target_type[0];
        self.guard_target_type[0] = GuardTargetType::None_;
    }

    fn set_current_path_snapshot_from_coords(&mut self, path: &[Coord3D]) {
        let mut snapshot = AiPath::new();
        for pos in path {
            snapshot.append_node(pos, AiPathLayer::Ground);
        }
        self.current_path_snapshot = Some(snapshot);
    }

    fn append_current_path_snapshot_goal(&mut self, goal: &Coord3D) {
        match self.current_path_snapshot.as_mut() {
            Some(path) => path.append_node(goal, AiPathLayer::Ground),
            None => self.set_current_path_snapshot_from_coords(&[*goal]),
        }
    }

    fn should_force_direct_path_for_off_map_start(&self, destination: &Coord3D) -> bool {
        let Ok(terrain) = crate::terrain::get_terrain_logic().read() else {
            return false;
        };
        let extent = terrain.get_maximum_pathfind_extent();
        if Self::is_in_region_no_z(&extent, destination) {
            return false;
        }
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let position = guard.get_position();
        !Self::is_in_region_no_z(&extent, &position)
    }

    fn is_in_region_no_z(region: &Region3D, position: &Coord3D) -> bool {
        position.x >= region.lo.x
            && position.x <= region.hi.x
            && position.y >= region.lo.y
            && position.y <= region.hi.y
    }

    fn should_use_direct_path_for_line_passable_non_final_goal(
        &self,
        destination: &Coord3D,
    ) -> bool {
        if self.is_final_goal {
            return false;
        }

        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let surfaces = {
            let set_surfaces = guard.locomotor_set.get_valid_surfaces();
            if set_surfaces != 0 {
                set_surfaces
            } else {
                guard.get_locomotor_surface_mask().unwrap_or(0)
            }
        };
        if surfaces == 0 {
            return false;
        }
        let position = guard.get_position();
        drop(guard);

        let Some(ai) = THE_AI.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        let ignore = if self.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.ignore_obstacle_id)
        };
        pf_guard.is_line_passable_for_surfaces(&position, destination, surfaces, ignore)
    }

    fn has_current_path(&self) -> bool {
        if self.current_path_snapshot.is_some() {
            return true;
        }
        self.unit
            .upgrade()
            .and_then(|unit| unit.read().ok().map(|guard| guard.current_path.is_some()))
            .unwrap_or(false)
    }

    fn current_locomotor_is_ultra_accurate(&self) -> bool {
        self.unit
            .upgrade()
            .and_then(|unit| {
                unit.read().ok().and_then(|guard| {
                    guard.current_locomotor.as_ref().and_then(|locomotor| {
                        locomotor.lock().ok().map(|loc| loc.is_ultra_accurate())
                    })
                })
            })
            .unwrap_or(false)
    }

    fn path_with_cpp_final_node(&self, path: &[Coord3D]) -> Result<Vec<Coord3D>, String> {
        if path.is_empty() {
            return Err("set_path_from_coords missing path points".to_string());
        }

        let mut installed_path = path.to_vec();
        if self.current_locomotor_is_ultra_accurate() {
            if let Some(last) = installed_path.last_mut() {
                *last = self.requested_destination;
            }
        }
        Ok(installed_path)
    }

    fn try_install_closest_path_for_invalid_destination(
        &mut self,
        destination: &Coord3D,
    ) -> Result<bool, String> {
        let request = self.build_classic_path_request(*destination, false)?;
        let locomotor_set = self
            .unit
            .upgrade()
            .and_then(|unit| unit.read().ok().map(|guard| guard.locomotor_set.clone()))
            .ok_or_else(|| "unit no longer available".to_string())?;
        let Some(ai) = THE_AI.read().ok() else {
            return Ok(false);
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return Ok(false);
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return Ok(false);
        };

        if pf_guard.valid_movement_position(
            &locomotor_set,
            request.is_crusher,
            destination,
            request.ignore_obstacle_id,
        ) {
            return Ok(false);
        }

        if self.has_current_path() {
            if self.blocked_and_stuck {
                self.stop_stuck_old_path_after_failed_path()?;
            } else {
                self.path_timestamp = TheGameLogic::get_frame();
                self.blocked_frames = 0;
                self.blocked_and_stuck = false;
            }
            return Ok(true);
        }

        self.retry_path = true;
        let result = pf_guard.find_closest_path_result(request);
        if result.success && !result.waypoints.is_empty() {
            self.set_path_from_coords(&result.waypoints)?;
        } else {
            self.path_timestamp = TheGameLogic::get_frame();
            self.blocked_frames = 0;
            self.blocked_and_stuck = false;
        }

        Ok(true)
    }

    fn stop_stuck_old_path_after_failed_path(&mut self) -> Result<(), String> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let current_pos = unit
            .read()
            .map_err(|_| "unit lock poisoned".to_string())?
            .get_position();

        let snapped = THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pathfinder| {
                pathfinder
                    .read()
                    .ok()
                    .map(|pf| pf.snap_position(&current_pos))
            })
            .unwrap_or(current_pos);

        self.destroy_path();
        self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
        {
            let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
            guard.target_position = Some(snapped);
            guard.path_index = 0;
            guard.current_speed = 0.0;
            guard.movement_state = MovementState::Idle;
        }
        self.set_locomotor_goal_none();
        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.is_blocked = false;
        self.blocked_and_stuck = false;
        Ok(())
    }

    fn do_queued_pathfind_now(&mut self) -> Result<bool, String> {
        if !self.waiting_for_path {
            return Ok(false);
        }

        self.waiting_for_path = false;
        self.set_queue_for_path_time(0);
        self.retry_path = false;
        let mut destination = self.requested_destination;

        if self.is_safe_path {
            return self.do_queued_safe_pathfind_now();
        }

        if self.is_approach_path && !self.is_doing_ground_movement() {
            self.is_approach_path = false;
        }
        if self.is_approach_path {
            return self.do_queued_approach_pathfind_now(destination);
        }

        if self.is_attack_path {
            self.prepare_queued_attack_path_fallback()?;
            destination = self.requested_destination;
        }

        if self.try_install_closest_path_for_invalid_destination(&destination)? {
            return Ok(true);
        }

        let request = self.build_classic_path_request(destination, false)?;
        let path_result =
            THE_AI
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder
                        .read()
                        .ok()
                        .map(|pf| pf.find_path_result(request.clone()))
                });

        if let Some(result) = path_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                return Ok(true);
            }
        }

        if self.has_current_path() {
            if self.blocked_and_stuck {
                self.stop_stuck_old_path_after_failed_path()?;
            } else {
                self.path_timestamp = TheGameLogic::get_frame();
                self.blocked_frames = 0;
                self.blocked_and_stuck = false;
            }
            return Ok(true);
        }

        self.retry_path = true;
        let closest_result =
            THE_AI
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder
                        .read()
                        .ok()
                        .map(|pf| pf.find_closest_path_result(request))
                });
        if let Some(result) = closest_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                return Ok(true);
            }
        }

        self.path_timestamp = TheGameLogic::get_frame();
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        Ok(false)
    }

    fn prepare_queued_attack_path_fallback(&mut self) -> Result<(), String> {
        self.is_attack_path = false;
        if self.requested_victim_id == INVALID_ID {
            return Ok(());
        }

        let Some(victim) = get_legacy_object(self.requested_victim_id) else {
            return Ok(());
        };
        let victim_pos = victim
            .read()
            .map_err(|_| "victim lock poisoned".to_string())?
            .get_position()
            .to_owned();
        self.requested_destination = victim_pos;
        let _ = self.ignore_obstacle(Some(&victim));
        Ok(())
    }

    fn do_queued_approach_pathfind_now(&mut self, destination: Coord3D) -> Result<bool, String> {
        self.destroy_path();

        let request = self.build_classic_path_request(destination, false)?;
        let closest_result =
            THE_AI
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder
                        .read()
                        .ok()
                        .map(|pf| pf.find_closest_path_result(request))
                });

        if let Some(result) = closest_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn do_queued_safe_pathfind_now(&mut self) -> Result<bool, String> {
        self.destroy_path();

        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        let obj_guard = guard
            .base_object
            .read()
            .map_err(|_| "unit base object lock poisoned".to_string())?;
        let owner_pos = *obj_guard.get_position();
        let owner_vision_range = obj_guard.get_vision_range();
        drop(obj_guard);
        drop(guard);

        let repulsor_pos1 = get_legacy_object(self.repulsor1)
            .and_then(|repulsor| {
                repulsor
                    .read()
                    .ok()
                    .map(|repulsor_guard| *repulsor_guard.get_position())
            })
            .unwrap_or_else(|| Coord3D::new(-1000.0, -1000.0, 0.0));
        let repulsor_pos2 = get_legacy_object(self.repulsor2)
            .and_then(|repulsor| {
                repulsor
                    .read()
                    .ok()
                    .map(|repulsor_guard| *repulsor_guard.get_position())
            })
            .unwrap_or(repulsor_pos1);
        let repulsed_distance = THE_AI
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| data.repulsed_distance)
            })
            .unwrap_or(0.0);
        let safe_radius = owner_vision_range + repulsed_distance;
        let request = self.build_classic_path_request(owner_pos, false)?;
        let safe_result =
            THE_AI
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder.read().ok().map(|pf| {
                        pf.find_safe_path_result(
                            request,
                            &repulsor_pos1,
                            &repulsor_pos2,
                            safe_radius,
                        )
                    })
                });

        if let Some(result) = safe_result {
            if result.success && !result.waypoints.is_empty() {
                self.set_path_from_coords(&result.waypoints)?;
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn install_direct_path_from_current_position(&mut self, destination: &Coord3D) -> bool {
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(mut guard) = unit.write() else {
            return false;
        };

        let mut start = guard.get_position();
        start.z = destination.z;
        guard.current_path = Some(vec![
            Coord2D::new(start.x, start.y),
            Coord2D::new(destination.x, destination.y),
        ]);
        guard.path_following_state = None;
        guard.path_index = 0;
        guard.target_position = Some(*destination);
        guard.movement_state = MovementState::Moving;
        guard.current_speed = 0.0;
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.waiting_for_path = false;
        self.path_timestamp = TheGameLogic::get_frame();
        self.movement_complete = false;
        self.locomotor_goal_type = 1;
        self.locomotor_goal_data = Coord3D::ZERO;
        drop(guard);
        self.set_current_path_snapshot_from_coords(&[start, *destination]);
        true
    }

    fn xfer_locomotor_set_state(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Some(unit) = self.unit.upgrade() {
            let mut guard = unit
                .write()
                .map_err(|_| "unit lock poisoned during locomotor xfer".to_string())?;
            let guard = &mut *guard;
            guard
                .locomotor_set
                .xfer_self_and_cur_loco_ptr(xfer, &mut guard.current_locomotor)?;
        } else {
            let mut empty_set = LocomotorSet::new();
            let mut current_locomotor = None;
            empty_set.xfer_self_and_cur_loco_ptr(xfer, &mut current_locomotor)?;
        }

        let mut current_locomotor_set = self.current_locomotor_set as i32;
        xfer.xfer_int(&mut current_locomotor_set)
            .map_err(|e| e.to_string())?;
        if xfer.is_loading() {
            self.current_locomotor_set = locomotor_set_type_from_i32(current_locomotor_set)?;
        }
        Ok(())
    }

    pub fn apply_ai_update_module_data(
        &mut self,
        data: &crate::object::update::AIUpdateModuleData,
    ) {
        self.surrender_duration_frames = data.surrender_duration_frames();
        self.auto_acquire_enemies_when_idle = data.auto_acquire_enemies_when_idle();
        self.mood_attack_check_rate_frames = data.mood_attack_check_rate();
        self.forbid_player_commands = data.forbid_player_commands();
        self.turrets_linked = data.turrets_linked();
        self.turret_primary_data = data.turret_primary().cloned();
        self.turret_secondary_data = data.turret_secondary().cloned();
        self.locomotor_sets = data.locomotor_sets().clone();

        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                let allow = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE)
                    != 0;
                let deny = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE_NO)
                    != 0;
                guard.auto_acquire_enemies = allow && !deny;
                guard.auto_acquire_while_stealthed = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE_STEALTHED)
                    != 0;
                guard.auto_acquire_not_while_attacking = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE_NOT_WHILE_ATTACKING)
                    != 0;
                guard.auto_acquire_attack_buildings = (self.auto_acquire_enemies_when_idle
                    & crate::object::update::AUTO_ACQUIRE_IDLE_ATTACK_BUILDINGS)
                    != 0;
                guard.mood_attack_check_rate_frames = data.mood_attack_check_rate();
            }
        }

        if let Some(mut jet_ai) = self.jet_ai.take() {
            jet_ai.on_object_created(self);
            self.jet_ai = Some(jet_ai);
        }

        if self.turret_primary_data.is_some() {
            let _ = self.ensure_turret_machine(TurretType::Primary);
        }
        if self.turret_secondary_data.is_some() {
            let _ = self.ensure_turret_machine(TurretType::Secondary);
        }

        let _ = self.choose_locomotor_set(LocomotorSetType::Normal);
    }

    fn build_classic_path_request(
        &self,
        destination: Coord3D,
        allow_partial: bool,
    ) -> Result<crate::ai::pathfind_complete::PathRequest, String> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        let obj_guard = guard
            .base_object
            .read()
            .map_err(|_| "unit base object lock poisoned".to_string())?;
        let surfaces = guard
            .get_locomotor_surface_mask()
            .unwrap_or(crate::locomotor::SURFACE_GROUND);
        Ok(crate::ai::pathfind_complete::PathRequest {
            object_id: obj_guard.get_id(),
            from: *obj_guard.get_position(),
            to: destination,
            surfaces,
            is_crusher: obj_guard.get_crusher_level() > 0,
            unit_radius: obj_guard.get_geometry_info().get_major_radius(),
            allow_partial,
            move_allies: self.can_path_through_units,
            ignore_obstacle_id: if self.ignore_obstacle_id == INVALID_ID {
                None
            } else {
                Some(self.ignore_obstacle_id)
            },
        })
    }

    fn queue_path_request_now(&self, destination: Coord3D) -> Result<(), String> {
        let request = self.build_classic_path_request(destination, false)?;

        if let Some(ai) = THE_AI.read().ok() {
            if let Some(pathfinder) = ai.pathfinder() {
                pathfinder
                    .read()
                    .map_err(|_| "pathfinder lock poisoned".to_string())?
                    .queue_for_path_request(request)
                    .map_err(|err| err.to_string())?;
            }
        }

        Ok(())
    }

    fn ensure_turret_machine(&mut self, turret: TurretType) -> Option<&mut TurretStateMachine> {
        match turret {
            TurretType::Primary => {
                if self.turret_primary_machine.is_none() {
                    self.turret_primary_machine = self.build_turret_machine(TurretType::Primary);
                }
                self.turret_primary_machine.as_mut()
            }
            TurretType::Secondary => {
                if self.turret_secondary_machine.is_none() {
                    self.turret_secondary_machine =
                        self.build_turret_machine(TurretType::Secondary);
                }
                self.turret_secondary_machine.as_mut()
            }
            TurretType::Invalid => None,
        }
    }

    fn build_turret_machine(&self, turret: TurretType) -> Option<TurretStateMachine> {
        let unit = self.unit.upgrade()?;
        let base_object = unit.read().ok().map(|guard| guard.base_object())?;
        let owner = Arc::downgrade(&base_object);
        let turret_ai = Arc::new(Mutex::new(TurretAI::new(Arc::downgrade(&base_object))));
        if let Ok(mut guard) = turret_ai.lock() {
            let slot = match turret {
                TurretType::Primary => WeaponSlotType::Primary,
                TurretType::Secondary => WeaponSlotType::Secondary,
                TurretType::Invalid => WeaponSlotType::Primary,
            };
            guard.set_weapon_slot(slot);
            let mask = match slot {
                WeaponSlotType::Primary => 1u32 << 0,
                WeaponSlotType::Secondary => 1u32 << 1,
                WeaponSlotType::Tertiary => 1u32 << 2,
            };
            let data = match turret {
                TurretType::Primary => self.turret_primary_data.as_ref(),
                TurretType::Secondary => self.turret_secondary_data.as_ref(),
                TurretType::Invalid => None,
            };

            if let Some(data) = data {
                data.apply_to(&mut guard);
                if data.turret_weapon_slots == 0 {
                    error!("TurretAIData missing ControlledWeaponSlots; applying slot fallback.");
                    guard.set_turret_weapon_slots_mask(mask);
                }
            } else {
                guard.set_turret_weapon_slots_mask(mask);
            }
        }
        Some(TurretStateMachine::new(Some(turret_ai), owner, "TurretAI"))
    }

    fn xfer_turret_ai(machine: &TurretStateMachine, xfer: &mut dyn Xfer) -> Result<(), String> {
        if let Some(turret_ai) = machine.get_turret_ai() {
            let mut guard = turret_ai
                .lock()
                .map_err(|_| "TurretAI lock poisoned during AIUpdate xfer".to_string())?;
            guard.xfer(xfer)?;
        }
        Ok(())
    }

    fn start_rappel_state(&mut self, target_id: Option<ObjectID>) -> Result<(), String> {
        let unit = self.unit.upgrade().ok_or("unit no longer available")?;
        let base_object = unit.read().map_err(|_| "unit lock poisoned")?.base_object();

        let mut obj_guard = base_object
            .write()
            .map_err(|_| "base object lock poisoned")?;

        if !obj_guard.is_kind_of(KindOf::CanRappel) {
            return Err("unit cannot rappel".to_string());
        }

        obj_guard.set_model_condition_state(ModelConditionFlags::RAPPELLING);

        if let Some(physics) = obj_guard.get_physics() {
            physics.reset_dynamic_physics();
        }

        let mut target_is_bldg = false;
        let mut target_valid = None;
        if let Some(target_id) = target_id {
            if let Some(target) = crate::object::registry::OBJECT_REGISTRY.get_object(target_id) {
                if let Ok(target_guard) = target.read() {
                    if !target_guard.is_effectively_dead()
                        && target_guard.is_kind_of(KindOf::Structure)
                    {
                        target_is_bldg = true;
                        target_valid = Some(target_id);
                    }
                }
            }
        }

        let Some(terrain) = TheTerrainLogic::get() else {
            return Err("terrain logic unavailable".to_string());
        };

        let pos = *obj_guard.get_position();
        let layer = terrain.get_highest_layer_for_destination(&pos);
        let mut dest_z = terrain.get_layer_height(pos.x, pos.y, layer);

        if target_is_bldg {
            if let Some(target_id) = target_valid {
                if let Some(target) = crate::object::registry::OBJECT_REGISTRY.get_object(target_id)
                {
                    if let Ok(target_guard) = target.read() {
                        dest_z += target_guard
                            .get_geometry_info()
                            .get_max_height_above_position();
                    }
                }
            }
        } else {
            obj_guard.set_layer(layer);
            obj_guard.set_destination_layer(layer);
        }

        let max_rappel_rate = GRAVITY.abs() * (LOGICFRAMES_PER_SECOND as Real) * 2.5;
        let rappel_rate = -self.desired_speed.min(max_rappel_rate);

        self.rappel_state = Some(RappelState {
            rappel_rate,
            dest_z,
            target_is_bldg,
            target_id: target_valid,
        });

        Ok(())
    }

    fn finish_rappel_state(&mut self) {
        let unit = self.unit.upgrade();
        if let Some(unit) = unit {
            let base = unit.read().ok().map(|guard| guard.base_object());
            if let Some(base) = base {
                if let Ok(mut obj_guard) = base.write() {
                    obj_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                }
            }
        }
        self.desired_speed = FAST_AS_POSSIBLE;
        self.rappel_state = None;
        if self.current_command == Some(crate::ai::AiCommandType::RappelInto) {
            self.current_command = None;
        }
    }

    fn update_rappel_state(&mut self) {
        let Some(mut current_state) = self.rappel_state.take() else {
            return;
        };

        let Some(unit) = self.unit.upgrade() else {
            self.finish_rappel_state();
            return;
        };

        let base_object = {
            let unit_guard = unit.read().ok();
            unit_guard.map(|guard| guard.base_object())
        };

        let Some(base_object) = base_object else {
            self.finish_rappel_state();
            return;
        };

        let mut obj_guard = match base_object.write() {
            Ok(guard) => guard,
            Err(_) => {
                self.finish_rappel_state();
                return;
            }
        };

        if obj_guard.is_effectively_dead() {
            drop(obj_guard);
            self.finish_rappel_state();
            return;
        }

        let Some(terrain) = TheTerrainLogic::get() else {
            drop(obj_guard);
            self.finish_rappel_state();
            return;
        };

        if current_state.target_is_bldg {
            let target_gone = current_state
                .target_id
                .and_then(|id| crate::object::registry::OBJECT_REGISTRY.get_object(id))
                .map(|target| {
                    target
                        .read()
                        .ok()
                        .map(|target_guard| {
                            target_guard.is_effectively_dead()
                                || !target_guard.is_kind_of(KindOf::Structure)
                        })
                        .unwrap_or(true)
                })
                .unwrap_or(true);
            if target_gone {
                current_state.target_is_bldg = false;
                let pos = obj_guard.get_position();
                current_state.dest_z = terrain.get_ground_height(pos.x, pos.y, None);
            }
        }

        if let Some(physics) = obj_guard.get_physics() {
            physics.scrub_velocity_2d(0.0);
            physics.scrub_velocity_z(current_state.rappel_rate);
        }

        if !current_state.target_is_bldg {
            let pos = obj_guard.get_position();
            current_state.dest_z = terrain.get_layer_height(pos.x, pos.y, obj_guard.get_layer());
        }

        let pos = *obj_guard.get_position();
        if pos.z <= current_state.dest_z {
            let mut landing = pos;
            landing.z = current_state.dest_z;
            if let Err(err) = obj_guard.set_position(&landing) {
                log::debug!(
                    "Unit::update_rappel_state failed to set landing position for {}: {}",
                    obj_guard.get_id(),
                    err
                );
            }

            if current_state.target_is_bldg {
                let target_id = current_state.target_id;
                if let Some(target_id) = target_id {
                    let max_to_kill = 2;
                    let num_killed =
                        kill_enemies_in_container(obj_guard.get_id(), target_id, max_to_kill);
                    if num_killed > 0 {
                        if let Some(fx) = TheFXListStore::lookup_fx_list("CombatDropKillFX") {
                            if let Some(target) =
                                crate::object::registry::OBJECT_REGISTRY.get_object(target_id)
                            {
                                if let Err(err) = fx.do_fx_obj(&target, None) {
                                    log::debug!(
                                        "Unit::update_rappel_state CombatDropKillFX failed for target {}: {}",
                                        target_id,
                                        err
                                    );
                                }
                            }
                        } else {
                            log::warn!(
                                "Unit::update_rappel_state unresolved FXList 'CombatDropKillFX'"
                            );
                        }
                    }

                    if num_killed == max_to_kill {
                        obj_guard.kill(None, None);
                    } else {
                        let target = crate::object::registry::OBJECT_REGISTRY.get_object(target_id);
                        if let Some(target) = target {
                            if let Ok(target_guard) = target.read() {
                                if let Some(contain) = target_guard.get_contain() {
                                    if contain.is_valid_container_for(&obj_guard, true) {
                                        contain.add_to_contain(&obj_guard);
                                    } else {
                                        let exit_angle = target_guard.get_orientation();
                                        let offset = obj_guard
                                            .get_geometry_info()
                                            .get_bounding_circle_radius()
                                            .min(
                                                target_guard
                                                    .get_geometry_info()
                                                    .get_bounding_circle_radius(),
                                            );
                                        let angle = get_game_logic_random_value_real(PI, 2.0 * PI);
                                        let mut start_position = *target_guard.get_position();
                                        start_position.x += offset * angle.cos();
                                        start_position.y += offset * angle.sin();
                                        start_position.z = terrain.get_ground_height(
                                            start_position.x,
                                            start_position.y,
                                            None,
                                        );

                                        if let Err(err) = obj_guard.set_position(&start_position) {
                                            log::debug!(
                                                "Unit::update_rappel_state failed to set start position for {}: {}",
                                                obj_guard.get_id(),
                                                err
                                            );
                                        }
                                        if let Err(err) = obj_guard.set_orientation(exit_angle) {
                                            log::debug!(
                                                "Unit::update_rappel_state failed to set exit orientation for {}: {}",
                                                obj_guard.get_id(),
                                                err
                                            );
                                        }

                                        let mut options = FindPositionOptions::default();
                                        options.start_angle = Some(1.5 * PI);
                                        options.max_radius = 200.0;
                                        let mut end_position = Coord3D::new(0.0, 0.0, 0.0);
                                        let found_position = ThePartitionManager::get()
                                            .map(|partition| {
                                                partition.find_position_around_with_options(
                                                    &start_position,
                                                    &options,
                                                    &mut end_position,
                                                )
                                            })
                                            .unwrap_or(false);

                                        if found_position {
                                            let mut used_ai_path = false;
                                            if let Ok(unit_guard) = unit.read() {
                                                if let Some(ai) =
                                                    unit_guard.get_ai_update_interface()
                                                {
                                                    ai.ai_follow_path(
                                                        &[end_position],
                                                        current_state.target_id,
                                                        CommandSourceType::FromAi,
                                                    );
                                                    used_ai_path = true;
                                                }
                                            }
                                            if !used_ai_path {
                                                if let Ok(mut unit_guard) = unit.write() {
                                                    if let Err(err) = unit_guard.give_move_order(
                                                        end_position,
                                                        Vec::new(),
                                                        false,
                                                        false,
                                                    ) {
                                                        log::debug!(
                                                            "Unit::update_rappel_state give_move_order failed: {}",
                                                            err
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            drop(obj_guard);
            self.finish_rappel_state();
            return;
        }

        self.rappel_state = Some(current_state);
    }

    fn clip_goal_position(
        &self,
        guard: &Unit,
        mut pos: Coord3D,
        cmd_source: CommandSourceType,
    ) -> Coord3D {
        if cmd_source != CommandSourceType::FromPlayer {
            return pos;
        }

        let mut fudge = PATHFIND_CELL_SIZE_F * 0.5;
        let is_aircraft = guard
            .base_object
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Aircraft))
            .unwrap_or(false);
        if is_aircraft {
            let above_terrain = guard
                .base_object
                .read()
                .ok()
                .map(|obj| obj.is_significantly_above_terrain())
                .unwrap_or(false);
            if above_terrain {
                let preferred = guard
                    .current_locomotor
                    .as_ref()
                    .and_then(|loc| loc.lock().ok())
                    .map(|loc| loc.preferred_height)
                    .unwrap_or(0.0);
                if preferred > fudge {
                    fudge = preferred;
                }
            }
        }

        if let Ok(terrain_guard) = crate::terrain::get_terrain_logic().read() {
            let extent = terrain_guard.get_maximum_pathfind_extent();
            let min_x = extent.lo.x + fudge;
            let max_x = extent.hi.x - fudge;
            let min_y = extent.lo.y + fudge;
            let max_y = extent.hi.y - fudge;
            pos.x = pos.x.clamp(min_x, max_x);
            pos.y = pos.y.clamp(min_y, max_y);
        }

        pos
    }

    fn compute_pathfind_radius_and_center(unit: &Unit) -> (i32, bool) {
        let radius = unit
            .base_object
            .read()
            .ok()
            .map(|obj| obj.get_geometry_info().get_bounding_circle_radius())
            .unwrap_or(PATHFIND_CELL_SIZE_F * 0.5);
        let mut diameter = 2.0 * radius;
        if diameter > PATHFIND_CELL_SIZE_F && diameter < 2.0 * PATHFIND_CELL_SIZE_F {
            diameter = 2.0 * PATHFIND_CELL_SIZE_F;
        }

        let mut radius = (diameter / PATHFIND_CELL_SIZE_F + 0.3).floor() as i32;
        let mut center_in_cell = false;

        if radius == 0 {
            radius = 1;
        }
        if (radius & 1) != 0 {
            center_in_cell = true;
        }
        radius /= 2;
        if radius > 2 {
            radius = 2;
            center_in_cell = true;
        }

        (radius, center_in_cell)
    }

    fn compute_goal_cell(pos: &Coord3D, center_in_cell: bool) -> ICoord2D {
        if center_in_cell {
            ICoord2D::new(
                (pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
            )
        } else {
            ICoord2D::new(
                (0.5 + pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (0.5 + pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
            )
        }
    }

    fn remove_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
    ) {
        if self.pathfind_goal_cell.x < 0 || self.pathfind_goal_cell.y < 0 {
            self.pathfind_goal_cell = ICoord2D::new(-1, -1);
            self.pathfind_goal_layer = ClassicPathLayer::Invalid;
            return;
        }

        let clear_ground = true;
        let clear_layer = self.pathfind_goal_layer != ClassicPathLayer::Ground
            && self.pathfind_goal_layer != ClassicPathLayer::Invalid;
        pathfinder.clear_goal_cells(
            unit_id,
            self.pathfind_goal_cell,
            radius,
            center_in_cell,
            self.pathfind_goal_layer,
            clear_ground,
            clear_layer,
        );
        pathfinder.clear_aircraft_goal_cells(
            unit_id,
            self.pathfind_goal_cell,
            radius,
            center_in_cell,
        );

        self.pathfind_goal_cell = ICoord2D::new(-1, -1);
        self.pathfind_goal_layer = ClassicPathLayer::Invalid;
    }

    fn update_ground_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        layer: ClassicPathLayer,
        radius: i32,
        center_in_cell: bool,
        interacts_with_bridge_end: bool,
    ) {
        let layer_changed = self.pathfind_goal_layer != layer;
        if !layer_changed
            && self.pathfind_goal_cell.x == new_cell.x
            && self.pathfind_goal_cell.y == new_cell.y
        {
            return;
        }

        self.remove_goal_cells(pathfinder, unit_id, radius, center_in_cell);

        self.pathfind_goal_cell = new_cell;
        self.pathfind_goal_layer = layer;

        let mut do_ground = layer == ClassicPathLayer::Ground;
        let do_layer = layer != ClassicPathLayer::Ground;
        if do_layer && interacts_with_bridge_end {
            do_ground = true;
        }

        pathfinder.set_goal_cells(
            unit_id,
            new_cell,
            radius,
            center_in_cell,
            layer,
            do_ground,
            do_layer,
        );
    }

    fn update_aircraft_goal_cells(
        &mut self,
        pathfinder: &mut crate::ai::Pathfinder,
        unit_id: ObjectID,
        new_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        self.remove_goal_cells(pathfinder, unit_id, radius, center_in_cell);

        if !self.is_aircraft_that_adjusts_destination() {
            return;
        }

        self.pathfind_goal_cell = new_cell;
        self.pathfind_goal_layer = ClassicPathLayer::Ground;

        pathfinder.set_aircraft_goal_cells(unit_id, new_cell, radius, center_in_cell);
    }

    fn has_valid_locomotor_surfaces(&self) -> bool {
        self.unit
            .upgrade()
            .and_then(|unit| {
                unit.read()
                    .ok()
                    .and_then(|guard| guard.get_locomotor_surface_mask())
            })
            .map(|surfaces| surfaces != 0)
            .unwrap_or(false)
    }

    fn safe_path_search_distance(vision_range: Real, repulsed_distance: Real) -> Real {
        vision_range + repulsed_distance
    }

    fn current_path_extra_distance(&self) -> Real {
        self.unit
            .upgrade()
            .and_then(|unit| unit.read().ok().map(|guard| guard.path_extra_distance))
            .unwrap_or(0.0)
    }

    fn finish_completed_movement_like_cpp(&mut self) {
        if !self.movement_complete {
            return;
        }

        self.set_queue_for_path_time(0);
        self.destroy_path();
        self.set_locomotor_goal_none();

        if let Some(unit) = self.unit.upgrade() {
            if let Ok(guard) = unit.read() {
                if let Ok(mut object) = guard.base_object.write() {
                    object.clear_model_condition_state(ModelConditionFlags::MOVING);
                }
            }
        }

        self.movement_complete = false;
        self.ignore_obstacle_id = INVALID_ID;
    }
}

impl AIUpdateInterface for UnitAIUpdate {
    fn xfer_ai_update_state(&mut self, xfer: &mut dyn Xfer) -> Result<bool, String> {
        const FACADE_WAYPOINT_ID: u32 = 0x00FA_CADE;

        let is_loading = xfer.is_reading();

        let mut prior_waypoint_id = self.prior_waypoint_id.unwrap_or(FACADE_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut prior_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.prior_waypoint_id =
                (prior_waypoint_id != FACADE_WAYPOINT_ID).then_some(prior_waypoint_id);
        }

        let mut current_waypoint_id = self.current_waypoint_id.unwrap_or(FACADE_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut current_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.current_waypoint_id =
                (current_waypoint_id != FACADE_WAYPOINT_ID).then_some(current_waypoint_id);
        }

        if let Some(state_machine) = self.ai_state_machine.as_ref() {
            let mut machine = state_machine
                .lock()
                .map_err(|_| "AIUpdate state machine lock poisoned during xfer".to_string())?;
            machine.xfer(xfer)?;
        }

        xfer.xfer_bool(&mut self.ai_dead)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_recruitable)
            .map_err(|e| e.to_string())?;

        xfer.xfer_unsigned_int(&mut self.next_enemy_scan_time)
            .map_err(|e| e.to_string())?;

        let mut current_victim_id = self.get_current_victim().unwrap_or(INVALID_ID);
        xfer.xfer_object_id(&mut current_victim_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.set_current_victim((current_victim_id != INVALID_ID).then_some(current_victim_id));
        }

        xfer.xfer_real(&mut self.desired_speed)
            .map_err(|e| e.to_string())?;

        let mut last_command_source = self.last_command_source as u32;
        xfer.xfer_unsigned_int(&mut last_command_source)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.last_command_source = match last_command_source {
                0 => CommandSourceType::FromPlayer,
                1 => CommandSourceType::FromScript,
                2 => CommandSourceType::FromAi,
                3 => CommandSourceType::FromDozer,
                4 => CommandSourceType::DefaultSwitchWeapon,
                _ => CommandSourceType::FromAi,
            };
        }

        xfer_guard_target_type(xfer, &mut self.guard_target_type[0])?;
        xfer_guard_target_type(xfer, &mut self.guard_target_type[1])?;
        xfer_unit_coord3d(xfer, &mut self.location_to_guard)?;
        xfer.xfer_object_id(&mut self.object_to_guard)
            .map_err(|e| e.to_string())?;

        // Area trigger and attack-info names still need their engine registries wired to UnitAIUpdate.
        let mut area_to_guard_name = String::new();
        xfer.xfer_ascii_string(&mut area_to_guard_name)
            .map_err(|e| e.to_string())?;
        let mut attack_info_name = String::new();
        xfer.xfer_ascii_string(&mut attack_info_name)
            .map_err(|e| e.to_string())?;

        xfer.xfer_int(&mut self.planning_waypoint_count)
            .map_err(|e| e.to_string())?;
        if self.planning_waypoint_count < 0
            || self.planning_waypoint_count as usize > AI_UPDATE_MAX_WAYPOINTS
        {
            return Err(format!(
                "Invalid AIUpdate waypoint count {}, max {}",
                self.planning_waypoint_count, AI_UPDATE_MAX_WAYPOINTS
            ));
        }
        for waypoint in self
            .planning_waypoint_queue
            .iter_mut()
            .take(self.planning_waypoint_count as usize)
        {
            xfer_unit_coord3d(xfer, waypoint)?;
        }
        xfer.xfer_int(&mut self.planning_waypoint_index)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.executing_waypoint_queue)
            .map_err(|e| e.to_string())?;

        let mut completed_waypoint_id = self
            .completed_waypoint_id
            .unwrap_or(crate::common::INVALID_WAYPOINT_ID);
        xfer.xfer_unsigned_int(&mut completed_waypoint_id)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.completed_waypoint_id = (completed_waypoint_id
                != crate::common::INVALID_WAYPOINT_ID)
                .then_some(completed_waypoint_id);
        }

        xfer.xfer_bool(&mut self.waiting_for_path)
            .map_err(|e| e.to_string())?;
        if is_loading && !self.waiting_for_path {
            self.queue_for_path_frame = 0;
        }

        let mut got_path = self.current_path_snapshot.is_some();
        xfer.xfer_bool(&mut got_path).map_err(|e| e.to_string())?;
        if is_loading {
            self.current_path_snapshot = got_path.then(AiPath::new);
        }
        if let Some(path) = self.current_path_snapshot.as_mut().filter(|_| got_path) {
            path.xfer(xfer)?;
        }

        xfer.xfer_object_id(&mut self.requested_victim_id)
            .map_err(|e| e.to_string())?;
        xfer_unit_coord3d(xfer, &mut self.requested_destination)?;
        xfer_unit_coord3d(xfer, &mut self.requested_destination2)?;

        xfer.xfer_object_id(&mut self.ignore_obstacle_id)
            .map_err(|e| e.to_string())?;
        let mut path_extra_distance = self.current_path_extra_distance();
        xfer.xfer_real(&mut path_extra_distance)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.set_path_extra_distance(path_extra_distance)
                .map_err(|e| e.to_string())?;
        }
        xfer_unit_icoord2d(xfer, &mut self.pathfind_goal_cell)?;
        xfer_unit_icoord2d(xfer, &mut self.pathfind_cur_cell)?;

        xfer.xfer_unsigned_int(&mut self.ignore_collisions_until)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_int(&mut self.queue_for_path_frame)
            .map_err(|e| e.to_string())?;

        xfer_unit_coord3d(xfer, &mut self.final_position)?;
        xfer.xfer_bool(&mut self.do_final_position)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_attack_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_final_goal)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_approach_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.is_safe_path)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.movement_complete)
            .map_err(|e| e.to_string())?;
        let mut is_safe_path_duplicate = self.is_safe_path;
        xfer.xfer_bool(&mut is_safe_path_duplicate)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.is_safe_path = is_safe_path_duplicate;
        }

        xfer.xfer_bool(&mut self.locomotor_upgraded)
            .map_err(|e| e.to_string())?;
        xfer.xfer_bool(&mut self.can_path_through_units)
            .map_err(|e| e.to_string())?;
        let mut randomly_offset_mood_check = false;
        xfer.xfer_bool(&mut randomly_offset_mood_check)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.repulsor1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.repulsor2)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.move_out_of_way_1)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.move_out_of_way_2)
            .map_err(|e| e.to_string())?;

        self.xfer_locomotor_set_state(xfer)?;

        xfer.xfer_unsigned_int(&mut self.locomotor_goal_type)
            .map_err(|e| e.to_string())?;
        xfer_unit_coord3d(xfer, &mut self.locomotor_goal_data)?;

        if let Some(machine) = self.turret_primary_machine.as_ref() {
            Self::xfer_turret_ai(machine, xfer)?;
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            Self::xfer_turret_ai(machine, xfer)?;
        }

        let mut turret_sync_flag: u32 = 0;
        xfer.xfer_unsigned_int(&mut turret_sync_flag)
            .map_err(|e| e.to_string())?;
        let mut attitude = self.attitude as u32;
        xfer.xfer_unsigned_int(&mut attitude)
            .map_err(|e| e.to_string())?;

        let mut next_mood_check_time = self.get_next_mood_check_time();
        xfer.xfer_unsigned_int(&mut next_mood_check_time)
            .map_err(|e| e.to_string())?;
        if is_loading {
            self.set_next_mood_check_time(next_mood_check_time);
        }

        let mut crate_created = self
            .crate_created
            .lock()
            .map(|id| *id)
            .unwrap_or(INVALID_ID);
        xfer.xfer_object_id(&mut crate_created)
            .map_err(|e| e.to_string())?;
        if is_loading {
            if let Ok(mut id) = self.crate_created.lock() {
                *id = crate_created;
            }
        }

        Ok(true)
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.is_blocked {
            self.blocked_frames = self.blocked_frames.saturating_add(1);
        } else if self.blocked_frames > 1 {
            self.blocked_frames = 1;
        } else {
            self.blocked_frames = 0;
            self.blocked_and_stuck = false;
        }
        self.is_blocked = false;
        self.cur_max_blocked_speed = FAST_AS_POSSIBLE;

        if self.rappel_state.is_some() {
            self.update_rappel_state();
        }

        if self.demoralized_frames_left > 0 {
            let next = self.demoralized_frames_left.saturating_sub(1);
            self.set_demoralized(next);
        }

        if self.surrendered_frames_left > 0 {
            self.surrendered_frames_left = self.surrendered_frames_left.saturating_sub(1);
            if self.surrendered_frames_left == 0 {
                self.surrendered_player_index = None;
            }
        }

        #[cfg(feature = "allow_surrender")]
        if let Some(mut pow_ai) = self.pow_truck_ai.take() {
            let owner_id = self
                .unit
                .upgrade()
                .and_then(|unit| unit.read().ok().map(|guard| guard.get_id()))
                .unwrap_or(crate::common::INVALID_ID);
            let _ = pow_ai.update(owner_id, self);
            self.pow_truck_ai = Some(pow_ai);
        }

        if let Some(mut railed_ai) = self.railed_transport_ai.take() {
            let _ = railed_ai.update(self);
            self.railed_transport_ai = Some(railed_ai);
        }

        if let Some(mut hack_ai) = self.hack_internet_ai.take() {
            let _ = hack_ai.update(self);
            self.hack_internet_ai = Some(hack_ai);
        }

        if let Some(mut assault_ai) = self.assault_transport_ai.take() {
            let _ = assault_ai.update(self);
            self.assault_transport_ai = Some(assault_ai);
        }

        if let Some(mut deliver_ai) = self.deliver_payload_ai.take() {
            let _ = deliver_ai.update(self);
            self.deliver_payload_ai = Some(deliver_ai);
        }

        if let Some(mut deploy_ai) = self.deploy_style_ai.take() {
            let _ = deploy_ai.update(self);
            self.deploy_style_ai = Some(deploy_ai);
        }

        if let Some(mut chinook_ai) = self.chinook_ai.take() {
            let _ = chinook_ai.update(self);
            self.chinook_ai = Some(chinook_ai);
        }

        if let Some(mut supply_ai) = self.supply_truck_ai.take() {
            supply_ai.update();
            self.supply_truck_ai = Some(supply_ai);
        }
        if let Some(mut worker_ai) = self.worker_ai.take() {
            worker_ai.update();
            self.worker_ai = Some(worker_ai);
        }

        if let Some(mut wander_ai) = self.wander_ai.take() {
            let _ = wander_ai.update(self);
            self.wander_ai = Some(wander_ai);
        }
        if let Some(mut dozer_ai) = self.dozer_ai.take() {
            dozer_ai.update();
            self.dozer_ai = Some(dozer_ai);
        }

        if let Some(mut jet_ai) = self.jet_ai.take() {
            jet_ai.update_with_ai(self);
            self.jet_ai = Some(jet_ai);
        }

        if let Some(state_machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut machine) = state_machine.lock() {
                if self.ai_dead && machine.get_current_state_id() != Some(AIStateType::Dead as u32)
                {
                    machine.clear();
                    let _ = machine.set_state(AIStateType::Dead as u32);
                    machine.lock();
                }
                let _ = machine.update_state_machine();
            }
        }

        self.finish_completed_movement_like_cpp();

        let now = TheGameLogic::get_frame();
        if self.waiting_for_path
            && (self.queue_for_path_frame == 0 || now >= self.queue_for_path_frame)
        {
            let _ = self.do_queued_pathfind_now();
        } else if self.queue_for_path_frame != 0 && now >= self.queue_for_path_frame {
            self.queue_for_path_frame = 0;
            let _ = self.queue_path_request_now(self.requested_destination);
        }

        let update_turrets = self
            .unit
            .upgrade()
            .and_then(|unit| unit.read().ok().map(|guard| guard.base_object()))
            .and_then(|base| {
                base.read().ok().map(|obj| {
                    !obj.is_effectively_dead()
                        && !obj.is_disabled_by_type(DisabledType::Paralyzed)
                        && !obj.is_disabled_by_type(DisabledType::DisabledUnmanned)
                        && !obj.is_disabled_by_type(DisabledType::DisabledEmp)
                        && !obj.is_disabled_by_type(DisabledType::DisabledSubdued)
                        && !obj.is_disabled_by_type(DisabledType::DisabledHacked)
                })
            })
            .unwrap_or(false);

        if update_turrets {
            if let Some(machine) = self.turret_primary_machine.as_ref() {
                if let Some(turret) = machine.get_turret_ai() {
                    let _ = TurretAI::update_turret_ai_handle(&turret);
                }
            }
            if let Some(machine) = self.turret_secondary_machine.as_ref() {
                if let Some(turret) = machine.get_turret_ai() {
                    let _ = TurretAI::update_turret_ai_handle(&turret);
                }
            }
        }

        if let Some(mut dock_machine) = self.dock_machine.take() {
            let update_result = dock_machine
                .state_machine
                .lock()
                .map(|mut machine| machine.update())
                .unwrap_or(crate::state_machine::StateReturnType::Failure);

            match update_result.convert_sleep_to_continue() {
                crate::state_machine::StateReturnType::Continue
                | crate::state_machine::StateReturnType::Blocked => {
                    self.dock_machine = Some(dock_machine);
                }
                _ => {
                    let _ = dock_machine.halt();
                    let _ = self.set_can_path_through_units(false);
                    if self.current_command == Some(crate::ai::AiCommandType::Dock) {
                        self.current_command = None;
                    }
                }
            }
        }
        let mut pending_params: Option<crate::ai::AiCommandParams> = None;
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if jet_ai.has_pending_command()
                && (self.current_command.is_none()
                    || self.current_command == Some(crate::ai::AiCommandType::Idle))
                && !self.is_reloading()
            {
                pending_params = Some(jet_ai.reconstitute_command_params());
            }
        }
        if let Some(params) = pending_params {
            if let Some(jet_ai) = self.jet_ai.as_mut() {
                jet_ai.set_has_pending_command(false);
            }
            let _ = self.execute_command(&params);
        }
        if self.jet_ai.is_some()
            && (self.current_command.is_none()
                || self.current_command == Some(crate::ai::AiCommandType::Idle))
            && !self
                .jet_ai
                .as_ref()
                .map(|jet| jet.has_pending_command())
                .unwrap_or(false)
        {
            self.pending_command = None;
        }

        let is_reloading = self.is_reloading();
        let mut queued_enter_command: Option<crate::ai::AiCommandParams> = None;
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            let takeoff = matches!(
                self.current_command,
                Some(crate::ai::AiCommandType::Exit)
                    | Some(crate::ai::AiCommandType::FollowExitProductionPath)
            );
            let landing = matches!(
                self.current_command,
                Some(crate::ai::AiCommandType::Enter) | Some(crate::ai::AiCommandType::Dock)
            );
            let taxiing = takeoff || landing;
            jet_ai.set_takeoff_in_progress(takeoff);
            jet_ai.set_landing_in_progress(landing);
            jet_ai.set_taxi_in_progress(taxiing);
            if taxiing {
                jet_ai.set_allow_air_loco(false);
            }
            jet_ai.set_has_pending_command(self.pending_command.is_some());
            if jet_ai.allow_air_loco() && jet_ai.is_out_of_special_reload_ammo() {
                jet_ai.set_use_special_return_loco(true);
            } else if !jet_ai.allow_air_loco() {
                jet_ai.set_use_special_return_loco(false);
            }
            if !jet_ai.has_pending_command()
                && jet_ai.allow_air_loco()
                && jet_ai.is_out_of_special_reload_ammo()
                && !is_reloading
                && !matches!(
                    self.current_command,
                    Some(crate::ai::AiCommandType::Enter) | Some(crate::ai::AiCommandType::Dock)
                )
            {
                let producer_id = self
                    .unit
                    .upgrade()
                    .and_then(|unit| unit.read().ok().map(|guard| guard.base_object()))
                    .and_then(|obj| obj.read().ok().map(|guard| guard.get_producer_id()))
                    .unwrap_or(crate::common::INVALID_ID);
                if producer_id != crate::common::INVALID_ID {
                    jet_ai.set_has_pending_command(true);
                    jet_ai.set_suppress_command_store(true);
                    let mut params = crate::ai::AiCommandParams::new(
                        crate::ai::AiCommandType::Enter,
                        crate::ai::CommandSourceType::FromAi,
                    );
                    params.obj = Some(producer_id);
                    queued_enter_command = Some(params);
                }
            }
            if let Some(desired) = jet_ai.desired_locomotor_set() {
                let _ = self.choose_locomotor_set(desired);
            } else if jet_ai.allow_air_loco()
                && self.current_locomotor_set == LocomotorSetType::Taxiing
            {
                let _ = self.choose_locomotor_set(LocomotorSetType::Normal);
            } else if !jet_ai.allow_air_loco()
                && self.current_locomotor_set != LocomotorSetType::Taxiing
            {
                let _ = self.choose_locomotor_set(LocomotorSetType::Taxiing);
            }
        }
        if let Some(params) = queued_enter_command {
            let _ = self.execute_command(&params);
        }
        Ok(())
    }

    fn apply_bump_speed_limit(&mut self, mut desired_speed: Real, mut blocked: bool) -> Real {
        if blocked && desired_speed > self.cur_max_blocked_speed {
            desired_speed = self.cur_max_blocked_speed;
            if self.bump_speed_limit > desired_speed {
                self.bump_speed_limit = desired_speed;
            }
            self.bump_speed_limit *= 0.95;
            desired_speed = self.bump_speed_limit;
        } else {
            blocked = false;
            if self.bump_speed_limit < FAST_AS_POSSIBLE {
                let min_limit = desired_speed * 0.2;
                if self.bump_speed_limit < min_limit {
                    self.bump_speed_limit = min_limit;
                }
                self.bump_speed_limit *= 1.05;
            }
            if desired_speed > self.bump_speed_limit {
                desired_speed = self.bump_speed_limit;
            }
        }
        if !blocked && self.blocked_frames > 1 {
            self.blocked_frames = 1;
        }
        desired_speed
    }

    fn is_attacking(&self) -> bool {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(guard) = machine.lock() {
                if guard.is_attack_state() {
                    return true;
                }
            }
        }
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        guard
            .base_object
            .read()
            .ok()
            .map(|obj| obj.test_status(ObjectStatusTypes::OBJECT_STATUS_IS_ATTACKING))
            .unwrap_or(false)
            || matches!(
                guard.current_order,
                Some(UnitOrder::Attack { .. }) | Some(UnitOrder::AttackMove { .. })
            )
            || guard.movement_state == MovementState::Attacking
    }

    fn get_enter_target(&self) -> Option<ObjectID> {
        self.enter_target
    }

    fn set_demoralized(&mut self, duration_frames: UnsignedInt) {
        let prev = self.demoralized_frames_left;
        self.demoralized_frames_left = duration_frames;

        if (prev == 0 && self.demoralized_frames_left > 0)
            || (prev > 0 && self.demoralized_frames_left == 0)
        {
            self.evaluate_morale_bonus();
        }
    }

    fn get_which_turret_for_cur_weapon(&self) -> TurretType {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_owners_cur_weapon_on_turret())
                    .unwrap_or(false)
                {
                    return TurretType::Primary;
                }
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_owners_cur_weapon_on_turret())
                    .unwrap_or(false)
                {
                    return TurretType::Secondary;
                }
            }
        }
        TurretType::Invalid
    }

    fn get_which_turret_for_weapon_slot(&self, slot: WeaponSlotType) -> TurretType {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_weapon_slot_on_turret(slot))
                    .unwrap_or(false)
                {
                    return TurretType::Primary;
                }
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| guard.is_weapon_slot_on_turret(slot))
                    .unwrap_or(false)
                {
                    return TurretType::Secondary;
                }
            }
        }
        TurretType::Invalid
    }

    fn set_turret_enabled(&mut self, turret: TurretType, enabled: bool) {
        match turret {
            TurretType::Primary => {
                self.turret_primary_enabled = enabled;
                if let Some(machine) = self.turret_primary_machine.as_ref() {
                    if let Some(ai) = machine.get_turret_ai() {
                        if let Ok(mut guard) = ai.lock() {
                            guard.set_turret_enabled(enabled);
                        }
                    }
                }
                if self.turrets_linked {
                    self.turret_secondary_enabled = enabled;
                    if let Some(machine) = self.turret_secondary_machine.as_ref() {
                        if let Some(ai) = machine.get_turret_ai() {
                            if let Ok(mut guard) = ai.lock() {
                                guard.set_turret_enabled(enabled);
                            }
                        }
                    }
                }
            }
            TurretType::Secondary => {
                self.turret_secondary_enabled = enabled;
                if let Some(machine) = self.turret_secondary_machine.as_ref() {
                    if let Some(ai) = machine.get_turret_ai() {
                        if let Ok(mut guard) = ai.lock() {
                            guard.set_turret_enabled(enabled);
                        }
                    }
                }
                if self.turrets_linked {
                    self.turret_primary_enabled = enabled;
                    if let Some(machine) = self.turret_primary_machine.as_ref() {
                        if let Some(ai) = machine.get_turret_ai() {
                            if let Ok(mut guard) = ai.lock() {
                                guard.set_turret_enabled(enabled);
                            }
                        }
                    }
                }
            }
            TurretType::Invalid => {}
        }
    }

    fn recenter_turret(&mut self, turret: TurretType) {
        match turret {
            TurretType::Primary => {
                self.turret_primary_natural = true;
                if let Some(machine) = self.turret_primary_machine.as_ref() {
                    if let Some(ai) = machine.get_turret_ai() {
                        if let Ok(mut guard) = ai.lock() {
                            guard.recenter_turret();
                        }
                    }
                }
                if self.turrets_linked {
                    self.turret_secondary_natural = true;
                    if let Some(machine) = self.turret_secondary_machine.as_ref() {
                        if let Some(ai) = machine.get_turret_ai() {
                            if let Ok(mut guard) = ai.lock() {
                                guard.recenter_turret();
                            }
                        }
                    }
                }
            }
            TurretType::Secondary => {
                self.turret_secondary_natural = true;
                if let Some(machine) = self.turret_secondary_machine.as_ref() {
                    if let Some(ai) = machine.get_turret_ai() {
                        if let Ok(mut guard) = ai.lock() {
                            guard.recenter_turret();
                        }
                    }
                }
                if self.turrets_linked {
                    self.turret_primary_natural = true;
                    if let Some(machine) = self.turret_primary_machine.as_ref() {
                        if let Some(ai) = machine.get_turret_ai() {
                            if let Ok(mut guard) = ai.lock() {
                                guard.recenter_turret();
                            }
                        }
                    }
                }
            }
            TurretType::Invalid => {}
        }
    }

    fn is_turret_in_natural_position(&self, turret: TurretType) -> bool {
        match turret {
            TurretType::Primary => self
                .turret_primary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|guard| guard.is_turret_in_natural_position())
                })
                .unwrap_or(false),
            TurretType::Secondary => self
                .turret_secondary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|guard| guard.is_turret_in_natural_position())
                })
                .unwrap_or(false),
            TurretType::Invalid => false,
        }
    }

    fn is_turret_enabled(&self, turret: TurretType) -> bool {
        match turret {
            TurretType::Primary => self
                .turret_primary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| ai.lock().ok().map(|guard| guard.is_turret_enabled()))
                .unwrap_or(false),
            TurretType::Secondary => self
                .turret_secondary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| ai.lock().ok().map(|guard| guard.is_turret_enabled()))
                .unwrap_or(false),
            TurretType::Invalid => false,
        }
    }

    fn get_turret_rot_and_pitch(&self, turret: TurretType) -> Option<(Real, Real)> {
        match turret {
            TurretType::Primary => self
                .turret_primary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|guard| (guard.get_turret_angle(), guard.get_turret_pitch()))
                }),
            TurretType::Secondary => self
                .turret_secondary_machine
                .as_ref()
                .and_then(|machine| machine.get_turret_ai())
                .and_then(|ai| {
                    ai.lock()
                        .ok()
                        .map(|guard| (guard.get_turret_angle(), guard.get_turret_pitch()))
                }),
            TurretType::Invalid => None,
        }
    }

    fn get_turret_angle(&self, turret: TurretType) -> Real {
        self.get_turret_rot_and_pitch(turret)
            .map(|(angle, _)| angle)
            .unwrap_or(0.0)
    }

    fn get_turret_pitch(&self, turret: TurretType) -> Real {
        self.get_turret_rot_and_pitch(turret)
            .map(|(_, pitch)| pitch)
            .unwrap_or(0.0)
    }

    fn is_weapon_slot_on_turret_and_aiming_at_target(
        &self,
        slot: WeaponSlotType,
        target: ObjectID,
    ) -> bool {
        if let Some(machine) = self.turret_primary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| {
                        guard.is_weapon_slot_on_turret(slot)
                            && guard.is_trying_to_aim_at_target(target)
                    })
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        if let Some(machine) = self.turret_secondary_machine.as_ref() {
            if let Some(ai) = machine.get_turret_ai() {
                if ai
                    .lock()
                    .ok()
                    .map(|guard| {
                        guard.is_weapon_slot_on_turret(slot)
                            && guard.is_trying_to_aim_at_target(target)
                    })
                    .unwrap_or(false)
                {
                    return true;
                }
            }
        }
        false
    }

    fn is_moving(&self) -> bool {
        if self.is_idle() {
            return false;
        }
        self.unit
            .upgrade()
            .and_then(|unit| {
                unit.read().ok().map(|guard| {
                    guard.is_movement_active()
                        || guard
                            .path_following_state
                            .as_ref()
                            .map(|state| state.waiting_for_path)
                            .unwrap_or(false)
                        || guard.current_path.is_some()
                        || guard.target_position.is_some()
                })
            })
            .unwrap_or(false)
    }

    fn is_idle(&self) -> bool {
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if jet_ai.should_block_idle(self.pending_command) {
                return false;
            }
        }
        if let Some(hack_ai) = self.hack_internet_ai.as_ref() {
            if hack_ai.has_pending_command() {
                return false;
            }
        }
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(guard) = machine.lock() {
                if !guard.is_idle() {
                    return false;
                }
            }
        }
        self.unit
            .upgrade()
            .and_then(|unit| {
                unit.read().ok().map(|guard| {
                    guard.movement_state == MovementState::Idle
                        && !guard
                            .path_following_state
                            .as_ref()
                            .map(|state| state.waiting_for_path)
                            .unwrap_or(false)
                        && guard.current_path.is_none()
                        && guard.target_position.is_none()
                })
            })
            .unwrap_or(false)
    }

    fn is_busy(&self) -> bool {
        self.ai_state_machine
            .as_ref()
            .and_then(|machine| machine.lock().ok())
            .map(|guard| guard.is_busy())
            .unwrap_or(false)
    }

    fn set_attitude(
        &mut self,
        attitude: AIAttitudeType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.attitude = attitude;
        Ok(())
    }

    fn get_attitude(&self) -> AIAttitudeType {
        self.attitude
    }

    fn is_idle_unrestricted(&self) -> bool {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(guard) = machine.lock() {
                if !guard.is_idle() {
                    return false;
                }
            }
        }
        self.unit
            .upgrade()
            .and_then(|unit| {
                unit.read().ok().map(|guard| {
                    guard.movement_state == MovementState::Idle
                        && !guard
                            .path_following_state
                            .as_ref()
                            .map(|state| state.waiting_for_path)
                            .unwrap_or(false)
                        && guard.current_path.is_none()
                        && guard.target_position.is_none()
                })
            })
            .unwrap_or(false)
    }

    fn set_movement_target(&mut self, target: &Coord3D) -> Result<(), String> {
        if let Some(path) = self.pending_safe_path.take() {
            return self.set_path_from_coords(&path);
        }
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard
            .give_move_order(*target, Vec::new(), false, false)
            .map_err(|err| err.to_string())
    }

    fn set_current_goal_path_index(
        &mut self,
        index: i32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.current_goal_path_index = index;
        Ok(())
    }

    fn get_current_goal_path_index(&self) -> i32 {
        self.current_goal_path_index
    }

    fn set_can_path_through_units(
        &mut self,
        value: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.can_path_through_units = value;
        if value {
            self.blocked_and_stuck = false;
        }
        Ok(())
    }

    fn get_can_path_through_units(&self) -> bool {
        self.can_path_through_units
    }

    fn is_blocked_and_stuck(&self) -> bool {
        const BLOCKED_RECOMPUTE_THRESHOLD: u32 = 60;
        if self.blocked_and_stuck {
            return true;
        }
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        guard.path_following_state.as_ref().map_or(false, |state| {
            state.frames_blocked > BLOCKED_RECOMPUTE_THRESHOLD
        })
    }

    fn set_is_blocked(&mut self, blocked: bool) {
        self.is_blocked = blocked;
    }

    fn set_blocked_and_stuck(&mut self, blocked: bool) {
        self.blocked_and_stuck = blocked;
    }

    fn get_num_frames_blocked(&self) -> u32 {
        let mut frames = self.blocked_frames;
        let Some(unit) = self.unit.upgrade() else {
            return frames;
        };
        let Ok(guard) = unit.read() else {
            return frames;
        };
        if let Some(state) = guard.path_following_state.as_ref() {
            frames = frames.max(state.frames_blocked);
        }
        frames
    }

    fn destroy_path(&mut self) {
        self.current_path_snapshot = None;
        self.waiting_for_path = false;
        self.is_attack_path = false;
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                guard.current_path = None;
                guard.path_following_state = None;
            }
        }
        self.set_locomotor_goal_none();
    }

    fn clear_move_out_of_way(&mut self) {
        self.move_out_of_way_1 = INVALID_ID;
        self.move_out_of_way_2 = INVALID_ID;
    }

    fn execute_command(
        &mut self,
        command: &crate::ai::AiCommandParams,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if self.forbid_player_commands
            && command.cmd_source == crate::ai::CommandSourceType::FromPlayer
        {
            return Ok(());
        }

        if let Some(deliver_ai) = self.deliver_payload_ai.as_ref() {
            if !deliver_ai.is_allowed_to_respond_to_ai_commands() {
                return Ok(());
            }
        }

        if self.railed_transport_ai.is_some()
            && command.cmd_source == crate::ai::CommandSourceType::FromPlayer
            && !matches!(
                command.cmd,
                crate::ai::AiCommandType::ExecuteRailedTransport
                    | crate::ai::AiCommandType::Evacuate
            )
        {
            return Ok(());
        }

        if let Some(mut assault_ai) = self.assault_transport_ai.take() {
            assault_ai.handle_command(command);
            self.assault_transport_ai = Some(assault_ai);
        }

        if let Some(mut hack_ai) = self.hack_internet_ai.take() {
            if hack_ai.handle_command(command, self) {
                self.hack_internet_ai = Some(hack_ai);
                return Ok(());
            }
            self.hack_internet_ai = Some(hack_ai);
        }

        if let Some(mut chinook_ai) = self.chinook_ai.take() {
            if chinook_ai.handle_command(command, self) {
                self.chinook_ai = Some(chinook_ai);
                return Ok(());
            }
            self.chinook_ai = Some(chinook_ai);
        }

        if let Some(mut jet_ai) = self.jet_ai.take() {
            if jet_ai.handle_command(command, self) {
                self.jet_ai = Some(jet_ai);
                return Ok(());
            }
            self.jet_ai = Some(jet_ai);
        }

        if let Some(jet_ai) = self.jet_ai.as_mut() {
            if jet_ai.suppress_command_store() {
                jet_ai.set_suppress_command_store(false);
            } else {
                jet_ai.store_most_recent_command(command);
            }
        }

        self.last_command_source = command.cmd_source;
        self.current_command = Some(command.cmd);
        if self.jet_ai.is_some() {
            self.pending_command = Some(command.cmd);
        } else {
            self.pending_command = None;
        }
        if let Some(supply_ai) = self.supply_truck_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                supply_ai.private_idle(command.cmd_source);
            }
        }
        if let Some(chinook_ai) = self.chinook_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                chinook_ai.private_idle(command.cmd_source);
            }
        }
        if let Some(worker_ai) = self.worker_ai.as_mut() {
            if command.cmd == crate::ai::AiCommandType::Idle {
                worker_ai.private_idle(command.cmd_source);
            }
        }
        if command.cmd != crate::ai::AiCommandType::Enter {
            self.enter_target = None;
        }
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;

        guard.forward_command_to_flight_deck(command);

        match command.cmd {
            crate::ai::AiCommandType::Repair => {
                if let Some(target_id) = command.obj {
                    if let Some(worker_ai) = self.worker_ai.as_mut() {
                        worker_ai.set_repair_target(target_id, command.cmd_source);
                    } else if let Some(dozer_ai) = self.dozer_ai.as_mut() {
                        dozer_ai.set_repair_target(target_id, command.cmd_source);
                    }
                }
            }
            crate::ai::AiCommandType::ResumeConstruction => {
                if let Some(target_id) = command.obj {
                    if let Some(worker_ai) = self.worker_ai.as_mut() {
                        worker_ai.set_resume_construction_target(target_id, command.cmd_source);
                    } else if let Some(dozer_ai) = self.dozer_ai.as_mut() {
                        dozer_ai.set_resume_construction_target(target_id, command.cmd_source);
                    }
                }
            }
            crate::ai::AiCommandType::MoveToPosition
            | crate::ai::AiCommandType::MoveToPositionEvenIfSleeping
            | crate::ai::AiCommandType::MoveToPositionAndEvacuate
            | crate::ai::AiCommandType::MoveToPositionAndEvacuateAndExit => {
                let clipped = self.clip_goal_position(&guard, command.pos, command.cmd_source);
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        if command.cmd_source == CommandSourceType::FromAi && !self.is_idle() {
                            machine.set_goal_position(clipped);
                            let _ = machine.set_temporary_state(
                                AIStateType::MoveTo as u32,
                                LOGICFRAMES_PER_SECOND * 20,
                            );
                        } else {
                            let mut params = command.clone();
                            params.pos = clipped;
                            machine.clear();
                            let _ = machine.ai_do_command(&params);
                        }
                        return Ok(());
                    }
                }

                guard.give_move_order(clipped, Vec::new(), false, false)?;
            }
            crate::ai::AiCommandType::TightenToPosition => {
                let is_mobile = guard.current_locomotor.is_some();
                if !is_mobile {
                    return Ok(());
                }
                let clipped = self.clip_goal_position(&guard, command.pos, command.cmd_source);
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let mut params = command.clone();
                        params.pos = clipped;
                        machine.clear();
                        let _ = machine.ai_do_command(&params);
                        return Ok(());
                    }
                }

                guard.give_move_order(clipped, Vec::new(), false, false)?;
            }
            crate::ai::AiCommandType::RappelInto => {
                let _ = self.start_rappel_state(command.obj);
            }
            crate::ai::AiCommandType::MoveToObject => {
                if let Some(target_id) = command.obj {
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            guard.give_move_order(
                                *target_guard.get_position(),
                                Vec::new(),
                                false,
                                false,
                            )?;
                        }
                    }
                }
            }
            crate::ai::AiCommandType::MoveAwayFromUnit => {
                if !self.is_allowed_to_move_away_from_unit() {
                    return Ok(());
                }
                if self.is_ai_in_dead_state() {
                    return Ok(());
                }
                let is_mobile = guard.current_locomotor.is_some();
                if !is_mobile {
                    return Ok(());
                }
                if let Some(target_id) = command.obj {
                    if (target_id == self.move_out_of_way_1 || target_id == self.move_out_of_way_2)
                        && self.is_blocked_and_stuck()
                    {
                        self.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
                        return Ok(());
                    }
                    self.move_out_of_way_2 = self.move_out_of_way_1;
                    self.move_out_of_way_1 = target_id;
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            let my_pos = guard.get_position();
                            let other_pos = target_guard.get_position();
                            let mut dir =
                                Coord3D::new(my_pos.x - other_pos.x, my_pos.y - other_pos.y, 0.0);
                            let len = (dir.x * dir.x + dir.y * dir.y).sqrt();
                            if len > 0.001 {
                                dir.x /= len;
                                dir.y /= len;
                            } else {
                                dir.x = 1.0;
                                dir.y = 0.0;
                            }
                            let mut desired = my_pos;
                            desired.x += dir.x * (PATHFIND_CELL_SIZE_F * 2.0);
                            desired.y += dir.y * (PATHFIND_CELL_SIZE_F * 2.0);
                            let clipped =
                                self.clip_goal_position(&guard, desired, command.cmd_source);

                            if let Some(state_machine) = self.ai_state_machine.as_ref() {
                                if let Ok(mut machine) = state_machine.lock() {
                                    machine.set_goal_position(clipped);
                                    let _ = machine.set_temporary_state(
                                        AIStateType::MoveOutOfTheWay as u32,
                                        LOGICFRAMES_PER_SECOND * 10,
                                    );
                                    return Ok(());
                                }
                            }

                            guard.give_move_order(clipped, Vec::new(), false, false)?;
                        }
                    }
                }
            }
            crate::ai::AiCommandType::FollowPath
            | crate::ai::AiCommandType::FollowExitProductionPath
            | crate::ai::AiCommandType::FollowUserPath => {
                let is_mobile = guard.current_locomotor.is_some();
                if !is_mobile {
                    return Ok(());
                }
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                if command.coords.is_empty() {
                    return Ok(());
                }
                let mut coords = command.coords.clone();
                let first = coords.remove(0);
                let waypoints = coords
                    .iter()
                    .map(|pos| Waypoint::new(INVALID_ID, *pos, String::new()))
                    .collect::<Vec<_>>();
                guard.give_move_order(first, waypoints, false, false)?;
            }
            crate::ai::AiCommandType::FollowPathAppend => {
                let is_mobile = guard.current_locomotor.is_some();
                if !is_mobile {
                    return Ok(());
                }
                let effectively_moving = !self.is_idle() || self.is_waiting_for_path();
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_follow_path = matches!(
                            machine.get_current_state_id(),
                            Some(id) if id == AIStateType::FollowPath as u32
                        );
                        if is_follow_path && machine.get_goal_path_size() > 0 && effectively_moving
                        {
                            let _ = machine.ai_do_command(command);
                            return Ok(());
                        }
                        if effectively_moving {
                            if let Some(goal) = machine.get_goal_position() {
                                let mut params = command.clone();
                                params.cmd = crate::ai::AiCommandType::FollowPath;
                                params.coords = vec![goal, command.pos];
                                machine.clear();
                                let _ = machine.ai_do_command(&params);
                            }
                            return Ok(());
                        }
                        let mut params = command.clone();
                        params.cmd = crate::ai::AiCommandType::FollowPath;
                        params.coords = vec![command.pos];
                        machine.clear();
                        let _ = machine.ai_do_command(&params);
                        return Ok(());
                    }
                }

                if effectively_moving {
                    let mut coords = Vec::new();
                    if let Some(goal) = guard
                        .target_position
                        .or_else(|| guard.path_following_state.as_ref().map(|s| s.goal_position))
                    {
                        coords.push(goal);
                    }
                    coords.push(command.pos);
                    let first = coords.remove(0);
                    let waypoints = coords
                        .iter()
                        .map(|pos| Waypoint::new(INVALID_ID, *pos, String::new()))
                        .collect::<Vec<_>>();
                    guard.give_move_order(first, waypoints, false, false)?;
                } else {
                    guard.give_move_order(command.pos, Vec::new(), false, false)?;
                }
            }
            crate::ai::AiCommandType::AttackMoveToPosition => {
                let clipped = self.clip_goal_position(&guard, command.pos, command.cmd_source);
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let mut params = command.clone();
                        params.pos = clipped;
                        machine.clear();
                        let _ = machine.ai_do_command(&params);
                        if let Ok(mut obj_guard) = guard.base_object.write() {
                            obj_guard.set_current_weapon_max_shot_count(command.int_value);
                        }
                        return Ok(());
                    }
                }

                guard.process_attack_move_order(clipped, true)?;
                if let Ok(mut obj_guard) = guard.base_object.write() {
                    obj_guard.set_current_weapon_max_shot_count(command.int_value);
                }
            }
            crate::ai::AiCommandType::AttackPosition => {
                let base_object = guard.base_object.clone();
                let mut local_pos =
                    self.clip_goal_position(&guard, command.pos, command.cmd_source);
                let mut max_shots = command.int_value;
                let continue_range = base_object
                    .read()
                    .ok()
                    .and_then(|obj_guard| {
                        obj_guard
                            .get_current_weapon()
                            .map(|(weapon, _)| weapon.get_lock_on_range())
                    })
                    .unwrap_or(0.0);

                if continue_range > 0.0 {
                    if let Ok(mut obj_guard) = base_object.write() {
                        obj_guard.set_status(ObjectStatusMaskType::IGNORING_STEALTH, true);
                    }

                    let target_id =
                        crate::helpers::ThePartitionManager::get().and_then(|partition| {
                            let obj_guard = base_object.read().ok()?;
                            partition.get_closest_object(
                                &command.pos,
                                continue_range,
                                |candidate| {
                                    matches!(
                                        ActionManager::get_can_attack_object(
                                            &*obj_guard,
                                            candidate,
                                            command.cmd_source,
                                            crate::attack::AbleToAttackType::NewTarget
                                        ),
                                        CanAttackResult::Possible
                                            | CanAttackResult::PossibleAfterMoving
                                    )
                                },
                            )
                        });

                    if let Ok(mut obj_guard) = base_object.write() {
                        obj_guard.set_status(ObjectStatusMaskType::IGNORING_STEALTH, false);
                    }

                    if let Some(target_id) = target_id {
                        if let Some(state_machine) = self.ai_state_machine.as_ref() {
                            if let Ok(mut machine) = state_machine.lock() {
                                let mut attack_params = crate::ai::AiCommandParams::new(
                                    crate::ai::AiCommandType::AttackObject,
                                    command.cmd_source,
                                );
                                attack_params.obj = Some(target_id);
                                attack_params.int_value = max_shots;
                                machine.clear();
                                let _ = machine.ai_do_command(&attack_params);
                                if let Ok(mut obj_guard) = guard.base_object.write() {
                                    obj_guard.set_current_weapon_max_shot_count(max_shots);
                                }
                                if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                                    chinook_ai.private_attack_object(
                                        target_id,
                                        max_shots,
                                        command.cmd_source,
                                    );
                                }
                                if let Some(transport_ai) = self.transport_ai.as_ref() {
                                    transport_ai.private_attack_object(
                                        target_id,
                                        max_shots,
                                        command.cmd_source,
                                    );
                                }
                                return Ok(());
                            }
                        }

                        guard.give_attack_order(target_id, true, false)?;
                        if let Ok(mut obj_guard) = guard.base_object.write() {
                            obj_guard.set_current_weapon_max_shot_count(max_shots);
                        }
                        if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                            chinook_ai.private_attack_object(
                                target_id,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        if let Some(transport_ai) = self.transport_ai.as_ref() {
                            transport_ai.private_attack_object(
                                target_id,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        return Ok(());
                    }
                    max_shots = 1;
                }

                let weapon_is_contact = base_object
                    .read()
                    .ok()
                    .map(|obj_guard| {
                        obj_guard
                            .get_current_weapon()
                            .map(|(weapon, _)| weapon.is_contact_weapon())
                            .unwrap_or(false)
                    })
                    .unwrap_or(false);
                if weapon_is_contact {
                    let mut path_available = true;
                    if let Some(locomotor) = guard.current_locomotor.as_ref().cloned() {
                        if let Ok(loco_guard) = locomotor.lock() {
                            if let Ok(ai_guard) = THE_AI.read() {
                                if let Some(system) = ai_guard.pathfinding_system() {
                                    if let Ok(mut system_guard) = system.write() {
                                        let capabilities = loco_guard.to_movement_capabilities();
                                        let unit_radius = base_object
                                            .read()
                                            .ok()
                                            .map(|obj_guard| {
                                                obj_guard.get_geometry_info().get_major_radius()
                                            })
                                            .unwrap_or(0.0);
                                        let request = crate::ai::pathfinding_system::PathRequest {
                                            requester: guard.get_id(),
                                            start: guard.get_position(),
                                            goal: local_pos,
                                            capabilities,
                                            unit_size: unit_radius,
                                            priority: 0,
                                            allow_partial: false,
                                            frame_requested: TheGameLogic::get_frame(),
                                            move_allies: self.can_path_through_units,
                                            ignore_obstacle_id: if self.ignore_obstacle_id
                                                == INVALID_ID
                                            {
                                                None
                                            } else {
                                                Some(self.ignore_obstacle_id)
                                            },
                                        };
                                        path_available = matches!(
                                            system_guard.find_path_immediate(&request),
                                            crate::ai::pathfinding_system::PathResult::Success(_)
                                        );
                                    }
                                }
                            }
                        }
                    }
                    if !path_available {
                        if let Some(partition) = ThePartitionManager::get() {
                            let mut options = FindPositionOptions::default();
                            options.min_radius = 0.0;
                            options.max_radius = 100.0;
                            options.source_to_path_to_dest_id = Some(guard.get_id());
                            let mut adjusted = local_pos;
                            if partition.find_position_around_with_options(
                                &local_pos,
                                &options,
                                &mut adjusted,
                            ) {
                                local_pos = adjusted;
                            }
                        }
                    }
                }

                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let mut params = command.clone();
                        params.pos = local_pos;
                        params.int_value = max_shots;
                        machine.clear();
                        let _ = machine.ai_do_command(&params);
                        if let Ok(mut obj_guard) = guard.base_object.write() {
                            obj_guard.set_current_weapon_max_shot_count(max_shots);
                        }
                        if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                            chinook_ai.private_attack_position(
                                &local_pos,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        if let Some(transport_ai) = self.transport_ai.as_ref() {
                            transport_ai.private_attack_position(
                                &local_pos,
                                max_shots,
                                command.cmd_source,
                            );
                        }
                        return Ok(());
                    }
                }

                guard.process_attack_move_order(local_pos, true)?;
                if let Ok(mut obj_guard) = guard.base_object.write() {
                    obj_guard.set_current_weapon_max_shot_count(max_shots);
                }
                if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                    chinook_ai.private_attack_position(&local_pos, max_shots, command.cmd_source);
                }
                if let Some(transport_ai) = self.transport_ai.as_ref() {
                    transport_ai.private_attack_position(&local_pos, max_shots, command.cmd_source);
                }
            }
            crate::ai::AiCommandType::AttackObject
            | crate::ai::AiCommandType::ForceAttackObject => {
                if let Some(target_id) = command.obj {
                    if let Some(state_machine) = self.ai_state_machine.as_ref() {
                        if let Ok(mut machine) = state_machine.lock() {
                            machine.clear();
                            let _ = machine.ai_do_command(command);
                            if let Ok(mut obj_guard) = guard.base_object.write() {
                                obj_guard.set_current_weapon_max_shot_count(command.int_value);
                            }
                            if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                                if command.cmd == crate::ai::AiCommandType::ForceAttackObject {
                                    chinook_ai.private_force_attack_object(
                                        target_id,
                                        command.int_value,
                                        command.cmd_source,
                                    );
                                } else {
                                    chinook_ai.private_attack_object(
                                        target_id,
                                        command.int_value,
                                        command.cmd_source,
                                    );
                                }
                            }
                            if let Some(transport_ai) = self.transport_ai.as_ref() {
                                if command.cmd == crate::ai::AiCommandType::ForceAttackObject {
                                    transport_ai.private_force_attack_object(
                                        target_id,
                                        command.int_value,
                                        command.cmd_source,
                                    );
                                } else {
                                    transport_ai.private_attack_object(
                                        target_id,
                                        command.int_value,
                                        command.cmd_source,
                                    );
                                }
                            }
                            return Ok(());
                        }
                    }

                    guard.give_attack_order(target_id, true, false)?;
                    if let Ok(mut obj_guard) = guard.base_object.write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                    if let Some(chinook_ai) = self.chinook_ai.as_ref() {
                        if command.cmd == crate::ai::AiCommandType::ForceAttackObject {
                            chinook_ai.private_force_attack_object(
                                target_id,
                                command.int_value,
                                command.cmd_source,
                            );
                        } else {
                            chinook_ai.private_attack_object(
                                target_id,
                                command.int_value,
                                command.cmd_source,
                            );
                        }
                    }
                    if let Some(transport_ai) = self.transport_ai.as_ref() {
                        if command.cmd == crate::ai::AiCommandType::ForceAttackObject {
                            transport_ai.private_force_attack_object(
                                target_id,
                                command.int_value,
                                command.cmd_source,
                            );
                        } else {
                            transport_ai.private_attack_object(
                                target_id,
                                command.int_value,
                                command.cmd_source,
                            );
                        }
                    }
                }
            }
            crate::ai::AiCommandType::AttackTeam => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        if let Ok(mut obj_guard) = guard.base_object.write() {
                            obj_guard.set_current_weapon_max_shot_count(command.int_value);
                        }
                        return Ok(());
                    }
                }

                if let Some(team_name) = command.team.as_ref() {
                    if let Ok(mut factory) = crate::team::get_team_factory().lock() {
                        if let Some(team) = factory.find_team(team_name) {
                            if let Ok(team_guard) = team.read() {
                                let target_id = if team_guard.get_team_target_object() != INVALID_ID
                                {
                                    team_guard.get_team_target_object()
                                } else {
                                    team_guard
                                        .get_members()
                                        .first()
                                        .copied()
                                        .unwrap_or(INVALID_ID)
                                };
                                if target_id != INVALID_ID {
                                    guard.give_attack_order(target_id, true, false)?;
                                    if let Ok(mut obj_guard) = guard.base_object.write() {
                                        obj_guard
                                            .set_current_weapon_max_shot_count(command.int_value);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::GuardPosition => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_object
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                guard.current_order = Some(UnitOrder::Guard {
                    position: command.pos,
                    area_radius: guard.engagement_range,
                });
                guard.order_queue.clear();
            }
            crate::ai::AiCommandType::GuardObject => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_object
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                if let Some(target_id) = command.obj {
                    if let Some(target_arc) = get_legacy_object(target_id) {
                        if let Ok(target_guard) = target_arc.read() {
                            guard.current_order = Some(UnitOrder::Guard {
                                position: *target_guard.get_position(),
                                area_radius: guard.engagement_range,
                            });
                            guard.order_queue.clear();
                        }
                    }
                }
            }
            crate::ai::AiCommandType::GuardArea => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_object
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
                guard.current_order = Some(UnitOrder::Guard {
                    position: command.pos,
                    area_radius: guard.engagement_range,
                });
                guard.order_queue.clear();
            }
            crate::ai::AiCommandType::GuardTunnelNetwork => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_object
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::GuardRetaliate => {
                if let Some(target_id) = command.obj {
                    if let Some(state_machine) = self.ai_state_machine.as_ref() {
                        if let Ok(mut machine) = state_machine.lock() {
                            machine.clear();
                            let _ = machine.ai_do_command(command);
                            if let Ok(mut obj_guard) = guard.base_object.write() {
                                obj_guard.set_current_weapon_max_shot_count(command.int_value);
                            }
                            return Ok(());
                        }
                    }

                    guard.current_order = Some(UnitOrder::Guard {
                        position: command.pos,
                        area_radius: guard.engagement_range,
                    });
                    guard.order_queue.clear();
                    guard.give_attack_order(target_id, true, false)?;
                    if let Ok(mut obj_guard) = guard.base_object.write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                }
            }
            crate::ai::AiCommandType::Enter => {
                self.enter_target = command.obj;
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        if command.obj.is_some() {
                            machine.clear();
                            let _ = machine.ai_do_command(command);
                            return Ok(());
                        }
                    }
                }
                if let Some(container_id) = command.obj {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    if let Some(unit) = self.unit.upgrade() {
                                        if let Ok(unit_guard) = unit.read() {
                                            let base_arc = unit_guard.base_object();
                                            drop(unit_guard);
                                            let base_lock = base_arc.read();
                                            if let Ok(base_guard) = base_lock {
                                                let _ = contain_guard
                                                    .on_object_wants_to_enter_or_exit(
                                                        &base_guard,
                                                        crate::modules::ContainWant::WantsToEnter,
                                                    );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::Exit => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                let container_id = command.obj.or_else(|| {
                    self.unit.upgrade().and_then(|unit| {
                        let unit_guard = unit.read().ok()?;
                        let base_arc = unit_guard.base_object();
                        drop(unit_guard);
                        let base_guard = base_arc.read().ok()?;
                        base_guard.get_contained_by()
                    })
                });
                if let Some(container_id) = container_id {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    if let Some(unit) = self.unit.upgrade() {
                                        if let Ok(unit_guard) = unit.read() {
                                            let base_arc = unit_guard.base_object();
                                            drop(unit_guard);
                                            let base_lock = base_arc.read();
                                            if let Ok(base_guard) = base_lock {
                                                let _ = contain_guard
                                                    .on_object_wants_to_enter_or_exit(
                                                        &base_guard,
                                                        crate::modules::ContainWant::WantsToExit,
                                                    );
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::ExitInstantly => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
                let container_id = command.obj.or_else(|| {
                    self.unit.upgrade().and_then(|unit| {
                        let unit_guard = unit.read().ok()?;
                        let base_arc = unit_guard.base_object();
                        drop(unit_guard);
                        let base_guard = base_arc.read().ok()?;
                        base_guard.get_contained_by()
                    })
                });
                if let Some(container_id) = container_id {
                    if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                        if let Ok(container_guard) = container.write() {
                            if let Some(contain) = container_guard.get_contain() {
                                if let Ok(mut contain_guard) = contain.lock() {
                                    if let Some(unit) = self.unit.upgrade() {
                                        if let Ok(unit_guard) = unit.read() {
                                            let base_arc = unit_guard.base_object();
                                            drop(unit_guard);
                                            let base_lock = base_arc.read();
                                            if let Ok(base_guard) = base_lock {
                                                let _ = contain_guard
                                                    .on_object_wants_to_enter_or_exit(
                                                        &base_guard,
                                                        crate::modules::ContainWant::WantsToExit,
                                                    );
                                                let _ = contain_guard
                                                    .release_object(base_guard.get_id());
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::Dock => {
                if let Some(supply_ai) = self.supply_truck_ai.as_mut() {
                    supply_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(chinook_ai) = self.chinook_ai.as_mut() {
                    chinook_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(worker_ai) = self.worker_ai.as_mut() {
                    worker_ai.private_dock(command.obj, command.cmd_source);
                }
                if let Some(mut existing) = self.dock_machine.take() {
                    let _ = existing.halt();
                }
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        if command.obj.is_some() {
                            machine.clear();
                            let _ = machine.ai_do_command(command);
                            return Ok(());
                        }
                    }
                }
                if let Some(target_id) = command.obj {
                    let target_arc = TheGameLogic::find_object_by_id(target_id);
                    let Some(target_arc) = target_arc else {
                        return Ok(());
                    };

                    let has_dock = target_arc
                        .read()
                        .ok()
                        .and_then(|guard| guard.with_dock_update_interface(|_| true))
                        .unwrap_or(false);
                    if !has_dock {
                        return Ok(());
                    }

                    if let Some(mut existing) = self.dock_machine.take() {
                        let _ = existing.halt();
                    }

                    let owner_object = guard.base_object();
                    let dock_machine =
                        AIDockMachine::new(owner_object.clone()).map_err(|err| err.to_string())?;
                    if let Ok(mut machine) = dock_machine.state_machine.lock() {
                        machine.set_goal_object(Some(Arc::downgrade(&target_arc)));
                        let _ = machine.init_default_state();
                    }
                    let _ = self.set_can_path_through_units(true);
                    self.dock_machine = Some(dock_machine);
                }
            }
            crate::ai::AiCommandType::ExecuteRailedTransport => {
                if let Some(mut railed_ai) = self.railed_transport_ai.take() {
                    let _ = railed_ai.handle_execute_railed_transport(command.cmd_source, self);
                    self.railed_transport_ai = Some(railed_ai);
                }
            }
            crate::ai::AiCommandType::HackInternet => {
                if let Some(mut hack_ai) = self.hack_internet_ai.take() {
                    hack_ai.hack_internet();
                    self.hack_internet_ai = Some(hack_ai);
                }
            }
            crate::ai::AiCommandType::Evacuate | crate::ai::AiCommandType::EvacuateInstantly => {
                let instantly = command.cmd == crate::ai::AiCommandType::EvacuateInstantly;
                if let Ok(obj_guard) = guard.base_object.write() {
                    if let Some(contain) = obj_guard.get_contain() {
                        if let Ok(mut contain_guard) = contain.lock() {
                            let _ = contain_guard
                                .order_all_passengers_to_exit(command.cmd_source, instantly);
                        }
                    }
                }
                if let Some(mut railed_ai) = self.railed_transport_ai.take() {
                    let _ = railed_ai.handle_evacuate(command.int_value, command.cmd_source, self);
                    self.railed_transport_ai = Some(railed_ai);
                }
            }
            crate::ai::AiCommandType::CombatDrop => {
                if let Some(mut chinook_ai) = self.chinook_ai.take() {
                    chinook_ai.private_combat_drop(
                        command.obj,
                        command.pos,
                        command.cmd_source,
                        self,
                    );
                    self.chinook_ai = Some(chinook_ai);
                }
            }
            crate::ai::AiCommandType::GetHealed => {
                if let Some(target_id) = command.obj {
                    let can_heal = guard
                        .base_object
                        .read()
                        .ok()
                        .and_then(|base_guard| {
                            let target = get_legacy_object(target_id)?;
                            let target_guard = target.read().ok()?;
                            Some(TheActionManager::can_get_healed_at(
                                &*base_guard,
                                &*target_guard,
                                command.cmd_source,
                            ))
                        })
                        .unwrap_or(false);
                    if !can_heal {
                        return Ok(());
                    }

                    let mut enter_params = command.clone();
                    enter_params.cmd = crate::ai::AiCommandType::Enter;
                    self.enter_target = enter_params.obj;
                    if let Some(state_machine) = self.ai_state_machine.as_ref() {
                        if let Ok(mut machine) = state_machine.lock() {
                            let is_mobile = guard.current_locomotor.is_some();
                            if !is_mobile {
                                return Ok(());
                            }
                            if enter_params.obj.is_some() {
                                machine.clear();
                                let _ = machine.ai_do_command(&enter_params);
                                return Ok(());
                            }
                        }
                    }
                    if let Some(container_id) = enter_params.obj {
                        if let Some(container) = TheGameLogic::find_object_by_id(container_id) {
                            if let Ok(container_guard) = container.write() {
                                if let Some(contain) = container_guard.get_contain() {
                                    if let Ok(mut contain_guard) = contain.lock() {
                                        if let Some(unit) = self.unit.upgrade() {
                                            if let Ok(unit_guard) = unit.read() {
                                                let base_arc = unit_guard.base_object();
                                                drop(unit_guard);
                                                let base_lock = base_arc.read();
                                                if let Ok(base_guard) = base_lock {
                                                    let _ = contain_guard
                                                        .on_object_wants_to_enter_or_exit(
                                                        &base_guard,
                                                        crate::modules::ContainWant::WantsToEnter,
                                                    );
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            crate::ai::AiCommandType::GetRepaired => {
                if let Some(target_id) = command.obj {
                    if let Some(mut chinook_ai) = self.chinook_ai.take() {
                        chinook_ai.private_get_repaired(target_id, command.cmd_source, self);
                        self.chinook_ai = Some(chinook_ai);
                        return Ok(());
                    }
                }

                if let Some(target_id) = command.obj {
                    let can_repair = guard
                        .base_object
                        .read()
                        .ok()
                        .and_then(|base_guard| {
                            let target = get_legacy_object(target_id)?;
                            let target_guard = target.read().ok()?;
                            Some(TheActionManager::can_get_repaired_at(
                                &*base_guard,
                                &*target_guard,
                                command.cmd_source,
                            ))
                        })
                        .unwrap_or(false);
                    if !can_repair {
                        return Ok(());
                    }

                    let mut dock_params = command.clone();
                    dock_params.cmd = crate::ai::AiCommandType::Dock;
                    if let Some(supply_ai) = self.supply_truck_ai.as_mut() {
                        supply_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(chinook_ai) = self.chinook_ai.as_mut() {
                        chinook_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(worker_ai) = self.worker_ai.as_mut() {
                        worker_ai.private_dock(dock_params.obj, dock_params.cmd_source);
                    }
                    if let Some(mut existing) = self.dock_machine.take() {
                        let _ = existing.halt();
                    }
                    if let Some(state_machine) = self.ai_state_machine.as_ref() {
                        if let Ok(mut machine) = state_machine.lock() {
                            let is_mobile = guard.current_locomotor.is_some();
                            if !is_mobile {
                                return Ok(());
                            }
                            if dock_params.obj.is_some() {
                                machine.clear();
                                let _ = machine.ai_do_command(&dock_params);
                                return Ok(());
                            }
                        }
                    }
                    if let Some(target_id) = dock_params.obj {
                        let target_arc = TheGameLogic::find_object_by_id(target_id);
                        let Some(target_arc) = target_arc else {
                            return Ok(());
                        };

                        let has_dock = target_arc
                            .read()
                            .ok()
                            .and_then(|guard| guard.with_dock_update_interface(|_| true))
                            .unwrap_or(false);
                        if !has_dock {
                            return Ok(());
                        }

                        if let Some(mut existing) = self.dock_machine.take() {
                            let _ = existing.halt();
                        }

                        let owner_object = guard.base_object();
                        let dock_machine = AIDockMachine::new(owner_object.clone())
                            .map_err(|err| err.to_string())?;
                        if let Ok(mut machine) = dock_machine.state_machine.lock() {
                            machine.set_goal_object(Some(Arc::downgrade(&target_arc)));
                            let _ = machine.init_default_state();
                        }
                        let _ = self.set_can_path_through_units(true);
                        self.dock_machine = Some(dock_machine);
                    }
                }
            }
            #[cfg(feature = "allow_surrender")]
            crate::ai::AiCommandType::PickUpPrisoner => {
                if let (Some(prisoner_id), Some(mut pow_ai)) =
                    (command.obj, self.pow_truck_ai.take())
                {
                    let owner_id = guard.get_id();
                    let _ = pow_ai.handle_pick_up_prisoner(
                        owner_id,
                        prisoner_id,
                        command.cmd_source,
                        self,
                    );
                    self.pow_truck_ai = Some(pow_ai);
                }
            }
            #[cfg(feature = "allow_surrender")]
            crate::ai::AiCommandType::ReturnPrisoners => {
                if let Some(mut pow_ai) = self.pow_truck_ai.take() {
                    let owner_id = guard.get_id();
                    let _ = pow_ai.handle_return_prisoners(
                        owner_id,
                        command.obj,
                        command.cmd_source,
                        self,
                    );
                    self.pow_truck_ai = Some(pow_ai);
                }
            }
            crate::ai::AiCommandType::Idle => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
                guard.stop_movement();
            }
            crate::ai::AiCommandType::Busy => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::Wander
            | crate::ai::AiCommandType::WanderInPlace
            | crate::ai::AiCommandType::Panic => {
                if guard.current_locomotor.is_none() {
                    return Ok(());
                }
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::Hunt => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_object
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }

                guard.attack_target = None;
                guard.auto_acquire_enemies = true;
                guard.combat_mode = CombatMode::Aggressive;
                guard.attack_move_active = true;
            }
            crate::ai::AiCommandType::AttackArea => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        let is_projectile = guard
                            .base_object
                            .read()
                            .ok()
                            .map(|obj| obj.is_any_kind_of(&[KindOf::Projectile]))
                            .unwrap_or(false);
                        if is_projectile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        return Ok(());
                    }
                }
            }
            crate::ai::AiCommandType::FollowWaypointPath
            | crate::ai::AiCommandType::FollowWaypointPathExact
            | crate::ai::AiCommandType::FollowWaypointPathAsTeam
            | crate::ai::AiCommandType::FollowWaypointPathAsTeamExact
            | crate::ai::AiCommandType::AttackFollowWaypointPath
            | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam => {
                if let Some(state_machine) = self.ai_state_machine.as_ref() {
                    if let Ok(mut machine) = state_machine.lock() {
                        let is_mobile = guard.current_locomotor.is_some();
                        if !is_mobile {
                            return Ok(());
                        }
                        machine.clear();
                        let _ = machine.ai_do_command(command);
                        if matches!(
                            command.cmd,
                            crate::ai::AiCommandType::AttackFollowWaypointPath
                                | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam
                        ) {
                            if let Ok(mut obj_guard) = guard.base_object.write() {
                                obj_guard.set_current_weapon_max_shot_count(command.int_value);
                            }
                        }
                        return Ok(());
                    }
                }

                if matches!(
                    command.cmd,
                    crate::ai::AiCommandType::AttackFollowWaypointPath
                        | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam
                ) {
                    guard.combat_mode = CombatMode::Aggressive;
                    guard.attack_move_active = true;
                }

                if let Some(start_id) = command.waypoint {
                    let mut chain = Vec::new();
                    if let Ok(terrain_guard) = crate::terrain::get_terrain_logic().read() {
                        let mut current = terrain_guard.get_waypoint_by_id(start_id);
                        while let Some(node) = current {
                            chain.push(Waypoint::new(
                                node.get_id(),
                                *node.get_location(),
                                String::new(),
                            ));
                            if node.get_num_links() > 1 {
                                break;
                            }
                            current = node
                                .get_link(0)
                                .and_then(|next_id| terrain_guard.get_waypoint_by_id(next_id));
                        }
                    }

                    if let Some(first) = chain.first().cloned() {
                        let mut remaining = chain;
                        remaining.remove(0);
                        guard.give_move_order(first.position, remaining, false, false)?;
                    }
                }

                if matches!(
                    command.cmd,
                    crate::ai::AiCommandType::AttackFollowWaypointPath
                        | crate::ai::AiCommandType::AttackFollowWaypointPathAsTeam
                ) {
                    if let Ok(mut obj_guard) = guard.base_object.write() {
                        obj_guard.set_current_weapon_max_shot_count(command.int_value);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn get_preferred_height(&self) -> Option<Real> {
        let locomotor = self.unit.upgrade().and_then(|unit| {
            unit.read()
                .ok()
                .and_then(|guard| guard.current_locomotor.as_ref().cloned())
        })?;
        locomotor.lock().ok().map(|loc| loc.preferred_height)
    }

    fn is_allowed_to_adjust_destination(&self) -> bool {
        if let Some(chinook_ai) = self.chinook_ai.as_ref() {
            let invalid_allowed = self
                .unit
                .upgrade()
                .and_then(|unit| {
                    let guard = unit.read().ok()?;
                    let locomotor = guard.current_locomotor.as_ref()?.clone();
                    drop(guard);
                    let loc_guard = locomotor.lock().ok()?;
                    Some(loc_guard.is_allowing_invalid_positions())
                })
                .unwrap_or(false);
            if invalid_allowed {
                return false;
            }
            chinook_ai.is_allowed_to_adjust_destination()
        } else {
            true
        }
    }

    fn get_ai_free_to_exit(&self, exiter: &Object) -> crate::object::production::AIFreeToExitType {
        if let Some(chinook_ai) = self.chinook_ai.as_ref() {
            chinook_ai.get_ai_free_to_exit(exiter)
        } else {
            crate::object::production::AIFreeToExitType::FreeToExit
        }
    }

    fn set_path_extra_distance(
        &mut self,
        distance: Real,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                guard.path_extra_distance = distance;
            }
        }
        Ok(())
    }

    fn set_path_from_waypoint(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        group_offset: &Coord2D,
    ) -> Result<(), String> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let start_pos = unit
            .read()
            .map_err(|_| "unit lock poisoned".to_string())?
            .get_position();

        let terrain = crate::terrain::get_terrain_logic()
            .read()
            .map_err(|_| "terrain lock poisoned".to_string())?;

        self.destroy_path();

        // Build a chain following link 0 to match the classic path order.
        let mut visited = std::collections::HashSet::new();
        let mut waypoints = Vec::new();
        let mut path_coords = vec![start_pos];
        let mut current = waypoint.clone();
        for _ in 0..=WAYPOINT_PATH_LIMIT {
            if !visited.insert(current.id) {
                break;
            }
            let next_id = current.get_link(0);
            let mut adjusted = current.clone();
            adjusted.position.x += group_offset.x;
            adjusted.position.y += group_offset.y;
            adjusted.position.z =
                terrain.get_ground_height(adjusted.position.x, adjusted.position.y, None);
            if next_id.is_none() {
                adjusted.position = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| ai.pathfinder())
                    .and_then(|pathfinder| {
                        pathfinder
                            .read()
                            .ok()
                            .map(|pf| pf.snap_position(&adjusted.position))
                    })
                    .unwrap_or(adjusted.position);
            }
            path_coords.push(adjusted.position);
            waypoints.push(adjusted);

            let Some(next_id) = next_id else {
                break;
            };
            let Some(next) = terrain.get_waypoint_by_id(next_id) else {
                break;
            };
            current = crate::waypoint::Waypoint::from_terrain(next);
        }

        if waypoints.is_empty() {
            return Ok(());
        }

        let last = waypoints
            .last()
            .map(|waypoint| waypoint.position)
            .expect("waypoints is not empty");
        if let Ok(mut guard) = unit.write() {
            guard.target_position = Some(last);
            guard.movement_state = MovementState::Moving;
            guard.current_speed = 0.0;
            guard.path_index = 0;
            guard.path_following_state = None;
            guard.current_path = Some(
                path_coords
                    .iter()
                    .map(|pos| Coord2D::new(pos.x, pos.y))
                    .collect(),
            );
            guard.waypoint_queue.clear();
        }
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.waiting_for_path = false;
        self.queue_for_path_frame = 0;
        self.path_timestamp = TheGameLogic::get_frame();
        self.set_current_path_snapshot_from_coords(&path_coords);

        Ok(())
    }

    fn is_waypoint_queue_empty(&self) -> bool {
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(guard) = unit.read() {
                return guard.waypoint_queue.is_empty();
            }
        }
        true
    }

    fn is_waiting_for_path(&self) -> bool {
        if self.waiting_for_path {
            return true;
        }
        if self.queue_for_path_frame > TheGameLogic::get_frame() {
            return true;
        }
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(guard) = unit.read() {
                return guard
                    .path_following_state
                    .as_ref()
                    .map(|state| state.waiting_for_path)
                    .unwrap_or(false);
            }
        }
        false
    }

    fn queue_waypoint(&mut self, pos: &Coord3D) {
        if (self.planning_waypoint_count as usize) < AI_UPDATE_MAX_WAYPOINTS {
            self.planning_waypoint_queue[self.planning_waypoint_count as usize] = *pos;
            self.planning_waypoint_count += 1;
            if let Some(unit) = self.unit.upgrade() {
                if let Ok(mut guard) = unit.write() {
                    guard
                        .waypoint_queue
                        .push(Waypoint::new(0, *pos, String::new()));
                }
            }
        }
    }

    fn execute_waypoint_queue(&mut self) {
        if self.planning_waypoint_count > 0 {
            self.planning_waypoint_index = 0;
            self.executing_waypoint_queue = true;
        }
        let first_pos = {
            let unit = match self.unit.upgrade() {
                Some(u) => u,
                None => return,
            };
            let mut guard = match unit.write() {
                Ok(g) => g,
                Err(_) => return,
            };
            if guard.waypoint_queue.is_empty() {
                return;
            }
            let first = guard.waypoint_queue.remove(0);
            first.position
        };
        if let Err(e) = self.ai_move_to_position(&first_pos) {
            log::warn!("execute_waypoint_queue failed: {}", e);
        }
    }

    fn append_goal_position_to_path(&mut self, goal: &Coord3D) -> Result<(), String> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;

        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                if let Some(active_path) = loc_guard.active_path.as_mut() {
                    active_path.append_waypoint(*goal);
                    self.append_current_path_snapshot_goal(goal);
                    return Ok(());
                }
            }
        }

        if let Some(path) = guard.current_path.as_mut() {
            path.push(Coord2D::new(goal.x, goal.y));
            self.append_current_path_snapshot_goal(goal);
            return Ok(());
        }

        Ok(())
    }

    fn set_path_from_coords(&mut self, path: &[Coord3D]) -> Result<(), String> {
        let installed_path = self.path_with_cpp_final_node(path)?;
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;

        let last = *installed_path.last().unwrap();
        guard.target_position = Some(last);
        guard.movement_state = MovementState::Moving;
        guard.current_speed = 0.0;
        guard.path_index = 0;
        guard.path_following_state = None;
        guard.current_path = Some({
            let mut v = Vec::with_capacity(installed_path.len());
            v.extend(installed_path.iter().map(|pos| Coord2D::new(pos.x, pos.y)));
            v
        });
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.queue_for_path_frame = 0;
        self.path_timestamp = TheGameLogic::get_frame();
        self.movement_complete = false;
        self.locomotor_goal_type = 1;
        self.locomotor_goal_data = Coord3D::ZERO;

        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                loc_guard.clear_path();
            }
        }
        drop(guard);
        self.set_current_path_snapshot_from_coords(&installed_path);
        if self.is_final_goal && self.is_doing_ground_movement() {
            let layer = TheTerrainLogic::get()
                .map(|terrain| terrain.get_layer_for_destination(&last))
                .unwrap_or(crate::common::PathfindLayerEnum::Ground);
            self.update_goal_position(&last, layer)?;
        }

        Ok(())
    }

    fn request_safe_path(&mut self, repulsor_id: ObjectID) -> Result<bool, String> {
        self.is_final_goal = false;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = false;
        self.is_safe_path = true;
        self.waiting_for_path = true;
        if repulsor_id != self.repulsor1 {
            self.repulsor2 = self.repulsor1;
        }
        self.repulsor1 = repulsor_id;
        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return Ok(false);
        }
        self.set_queue_for_path_time(0);
        self.path_timestamp = now;
        Ok(true)
    }

    fn is_doing_ground_movement(&self) -> bool {
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            return jet_ai.is_doing_ground_movement();
        }
        let unit = self.unit.upgrade();
        let Some(unit) = unit else {
            return true;
        };
        let Ok(guard) = unit.read() else {
            return true;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return true;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return true;
        };

        !matches!(
            loc_guard.get_appearance(),
            LocomotorAppearance::Hover | LocomotorAppearance::Thrust | LocomotorAppearance::Wings
        )
    }

    fn is_allowed_to_move_away_from_unit(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_allowed_to_move_away_from_unit())
            .unwrap_or(true)
    }

    fn get_sneaky_targeting_offset(&self, offset: &mut Coord3D) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.get_sneaky_targeting_offset(offset))
            .unwrap_or(false)
    }

    fn is_temporarily_preventing_aim_success(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_temporarily_preventing_aim_success())
            .unwrap_or(false)
    }

    fn add_targeter(&mut self, id: ObjectID, add: bool) {
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            jet_ai.add_targeter(id, add);
        }
    }

    fn are_turrets_linked(&self) -> Bool {
        self.turrets_linked
    }

    fn set_turret_target_object(
        &mut self,
        turret: TurretType,
        target: Option<&Arc<RwLock<Object>>>,
        force_attacking: bool,
    ) {
        if let Some(machine) = self.ensure_turret_machine(turret) {
            if let Some(turret_ai) = machine.get_turret_ai() {
                if let Ok(mut guard) = turret_ai.lock() {
                    guard.set_current_target_with_force(target.cloned(), force_attacking);
                }
            }
        }
    }

    fn set_turret_target_position(&mut self, turret: TurretType, pos: &Coord3D) {
        if let Some(machine) = self.ensure_turret_machine(turret) {
            if let Some(turret_ai) = machine.get_turret_ai() {
                if let Ok(mut guard) = turret_ai.lock() {
                    guard.set_target_position(Some(*pos));
                }
            }
        }
    }

    fn is_out_of_special_reload_ammo(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_out_of_special_reload_ammo())
            .unwrap_or(false)
    }

    fn get_treat_as_aircraft_for_loco_dist_to_goal(&self) -> bool {
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            return jet_ai.get_treat_as_aircraft_for_loco_dist_to_goal();
        }
        let Some(unit) = self.unit.upgrade() else {
            return true;
        };
        let Ok(guard) = unit.read() else {
            return true;
        };

        let mut treat_as_aircraft = !self.is_doing_ground_movement();
        if guard.path_extra_distance > PATHFIND_CLOSE_ENOUGH {
            treat_as_aircraft = true;
        }
        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(loc_guard) = locomotor.lock() {
                if loc_guard.get_appearance() == LocomotorAppearance::Hover {
                    treat_as_aircraft = true;
                }
            }
        }
        treat_as_aircraft
    }

    fn update_goal_position(
        &mut self,
        goal: &Coord3D,
        layer: crate::common::PathfindLayerEnum,
    ) -> Result<(), String> {
        let is_ground_movement = self.is_doing_ground_movement();
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;

        let owner_id = guard
            .base_object
            .read()
            .ok()
            .map(|obj| obj.get_id())
            .unwrap_or(INVALID_ID);
        let mut adjusted = *goal;
        let mut interacts_with_bridge_end = false;
        let terrain_layer = match layer {
            crate::common::PathfindLayerEnum::Invalid => crate::path::PathfindLayerEnum::Invalid,
            crate::common::PathfindLayerEnum::Ground => crate::path::PathfindLayerEnum::Ground,
            crate::common::PathfindLayerEnum::Top => crate::path::PathfindLayerEnum::Top,
            crate::common::PathfindLayerEnum::Bridge1 => crate::path::PathfindLayerEnum::Bridge1,
            crate::common::PathfindLayerEnum::Bridge2 => crate::path::PathfindLayerEnum::Bridge2,
            crate::common::PathfindLayerEnum::Bridge3 => crate::path::PathfindLayerEnum::Bridge3,
            crate::common::PathfindLayerEnum::Bridge4 => crate::path::PathfindLayerEnum::Bridge4,
            crate::common::PathfindLayerEnum::Wall => crate::path::PathfindLayerEnum::Wall,
            crate::common::PathfindLayerEnum::Tunnel
            | crate::common::PathfindLayerEnum::Water
            | crate::common::PathfindLayerEnum::Air
            | crate::common::PathfindLayerEnum::Last => crate::path::PathfindLayerEnum::Ground,
        };
        if let Ok(terrain) = crate::terrain::get_terrain_logic().read() {
            if layer == crate::common::PathfindLayerEnum::Wall {
                adjusted.z = crate::ai::THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| ai.get_ai_data().read().ok().map(|data| data.wall_height))
                    .unwrap_or(adjusted.z);
            } else {
                adjusted.z =
                    terrain.get_layer_height(adjusted.x, adjusted.y, terrain_layer, None, true);
            }

            let mut dest_layer = layer;
            if layer != crate::common::PathfindLayerEnum::Ground {
                if let Ok(obj_guard) = guard.base_object.read() {
                    interacts_with_bridge_end =
                        terrain.object_interacts_with_bridge_layer(&obj_guard, terrain_layer, true);
                }
            }
            if layer != crate::common::PathfindLayerEnum::Ground && !interacts_with_bridge_end {
                dest_layer = crate::common::PathfindLayerEnum::Ground;
            }
            if let Ok(mut obj_guard) = guard.base_object.write() {
                obj_guard.set_destination_layer(dest_layer);
            }
        }

        guard.target_position = Some(adjusted);
        if let Some(state) = guard.path_following_state.as_mut() {
            state.goal_position = adjusted;
            state.path_goal_position = adjusted;
        }

        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                if let Some(active_path) = loc_guard.active_path.as_mut() {
                    active_path.set_last_waypoint(adjusted);
                }
            }
        }

        let is_immobile = guard
            .base_object
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Immobile))
            .unwrap_or(false);
        if is_immobile {
            return Ok(());
        }

        let path_layer = match layer {
            crate::common::PathfindLayerEnum::Ground => ClassicPathLayer::Ground,
            _ => ClassicPathLayer::Top,
        };
        let (radius, center_in_cell) = Self::compute_pathfind_radius_and_center(&guard);
        let new_cell = Self::compute_goal_cell(&adjusted, center_in_cell);
        let is_unmanned_heli = guard
            .base_object
            .read()
            .ok()
            .map(|obj| {
                obj.is_kind_of(KindOf::ProducedAtHelipad)
                    && obj.is_disabled_by_type(crate::common::DisabledType::DisabledUnmanned)
            })
            .unwrap_or(false);

        if let Ok(ai_lock) = THE_AI.read() {
            if let Some(pathfinder) = ai_lock.pathfinder() {
                if let Ok(mut pf_guard) = pathfinder.write() {
                    if !is_ground_movement && !is_unmanned_heli {
                        self.update_aircraft_goal_cells(
                            &mut pf_guard,
                            owner_id,
                            new_cell,
                            radius,
                            center_in_cell,
                        );
                    } else {
                        self.update_ground_goal_cells(
                            &mut pf_guard,
                            owner_id,
                            new_cell,
                            path_layer,
                            radius,
                            center_in_cell,
                            interacts_with_bridge_end,
                        );
                    }
                }
            }
        }

        Ok(())
    }

    fn adjust_destination(&mut self, goal: &mut Coord3D) -> bool {
        let unit = self.unit.upgrade();
        let Some(unit) = unit else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return false;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return false;
        };

        let caps = loc_guard.to_movement_capabilities();
        let surfaces = loc_guard.get_legal_surfaces();
        let mut is_crusher = false;
        drop(loc_guard);
        if let Ok(obj_guard) = guard.base_object.read() {
            is_crusher = obj_guard.get_crusher_level() > 0;
        }
        let ignore_obstacle_id = if self.ignore_obstacle_id != INVALID_ID {
            Some(self.ignore_obstacle_id)
        } else {
            None
        };
        let owner_id = guard
            .base_object
            .read()
            .ok()
            .map(|obj| obj.get_id())
            .unwrap_or(INVALID_ID);
        let from_pos = guard
            .base_object
            .read()
            .ok()
            .map(|obj| *obj.get_position())
            .unwrap_or(*goal);
        let unit_radius = guard
            .base_object
            .read()
            .ok()
            .map(|obj| obj.get_geometry_info().get_bounding_circle_radius())
            .unwrap_or(PATHFIND_CELL_SIZE_F * 0.5);
        drop(guard);

        let mut adjusted = THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.pathfinding_system())
            .and_then(|pathfinding| {
                pathfinding
                    .read()
                    .ok()
                    .and_then(|pf| pf.adjust_destination(goal, &caps))
            });

        if adjusted.is_none() {
            let fallback_request = crate::ai::pathfind_complete::PathRequest {
                object_id: owner_id,
                from: from_pos,
                to: *goal,
                surfaces,
                is_crusher,
                unit_radius,
                allow_partial: false,
                move_allies: self.get_can_path_through_units(),
                ignore_obstacle_id,
            };
            adjusted = THE_AI
                .read()
                .ok()
                .and_then(|ai| ai.pathfinder())
                .and_then(|pathfinder| {
                    pathfinder
                        .read()
                        .ok()
                        .map(|pf| pf.find_closest_path_result(fallback_request))
                })
                .and_then(|result| {
                    if result.success {
                        result.waypoints.last().copied()
                    } else {
                        None
                    }
                });
        }

        let Some(mut new_goal) = adjusted else {
            return false;
        };

        if caps.layer == PfLayer::Ground {
            if let Ok(terrain) = crate::terrain::get_terrain_logic().read() {
                new_goal.z = terrain.get_ground_height(new_goal.x, new_goal.y, None);
            }
        }

        *goal = new_goal;
        true
    }

    fn set_adjusts_destination(&mut self, adjust: bool) {
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                guard.path_adjusts_destination = adjust;
                if let Some(state) = guard.path_following_state.as_mut() {
                    state.adjusts_destination = adjust;
                }
            }
        }
    }

    fn set_allow_invalid_position(
        &mut self,
        allow: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                loc_guard.set_allow_invalid_position(allow);
            }
        }
        Ok(())
    }

    fn set_allow_chase(&mut self, allowed: bool) {
        self.allow_chase = allowed;
    }

    fn set_locomotor_upgrade(
        &mut self,
        enabled: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.locomotor_upgraded = enabled;
        if matches!(
            self.current_locomotor_set,
            LocomotorSetType::Normal | LocomotorSetType::NormalUpgraded
        ) {
            let _ = self.choose_locomotor_set(LocomotorSetType::Normal);
        }
        Ok(())
    }

    fn choose_locomotor_set(
        &mut self,
        set: LocomotorSetType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let mut target_set = set;
        if target_set == LocomotorSetType::Normal && self.locomotor_upgraded {
            target_set = LocomotorSetType::NormalUpgraded;
        }
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if let Some(desired) = jet_ai.desired_locomotor_set() {
                target_set = desired;
            }
        }

        if target_set == self.current_locomotor_set {
            return Ok(());
        }

        let Some(unit) = self.unit.upgrade() else {
            return Ok(());
        };
        let Some(locomotors) = self.locomotor_sets.get(&target_set) else {
            return Ok(());
        };

        self.current_locomotor_set = target_set;

        let mut new_set = LocomotorSet::new();
        for locomotor_name in locomotors {
            if let Some(template) =
                crate::locomotor::LOCOMOTOR_STORE.get_template(locomotor_name.as_str())
            {
                let loco = Arc::new(Mutex::new(Locomotor::new(template)));
                new_set.add_locomotor(locomotor_name.as_str().to_string(), loco);
            } else {
                log::warn!("Locomotor template '{}' not found", locomotor_name.as_str());
            }
        }

        let mut guard = unit.write().map_err(|_| "unit lock poisoned")?;
        let prev_locomotor = guard.current_locomotor.as_ref().cloned();
        guard.locomotor_set = new_set;
        guard.current_locomotor = guard.locomotor_set.get_default_locomotor();

        if let (Some(prev), Some(current)) = (prev_locomotor, guard.current_locomotor.as_ref()) {
            if !Arc::ptr_eq(&prev, current) {
                if let Ok(mut loco_guard) = current.lock() {
                    loco_guard.set_precise_z_pos(false);
                    loco_guard.set_no_slow_down(false);
                    loco_guard.set_ultra_accurate(false);
                }
            }
        }

        Ok(())
    }

    fn set_ultra_accurate(
        &mut self,
        ultra: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                loc_guard.set_ultra_accurate(ultra);
            }
        }
        Ok(())
    }

    fn set_precise_z_pos(
        &mut self,
        precise: bool,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let guard = unit.read().map_err(|_| "unit lock poisoned".to_string())?;
        if let Some(locomotor) = guard.current_locomotor.as_ref() {
            if let Ok(mut loc_guard) = locomotor.lock() {
                loc_guard.set_precise_z_pos(precise);
            }
        }
        Ok(())
    }

    fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        self.unit.upgrade().and_then(|unit| {
            unit.read()
                .ok()
                .and_then(|guard| guard.current_locomotor.as_ref().cloned())
        })
    }

    fn get_path_destination(&self) -> Option<Coord3D> {
        let unit = self.unit.upgrade()?;
        let guard = unit.read().ok()?;
        if let Some(state) = guard.path_following_state.as_ref() {
            return Some(state.goal_position);
        }
        if let Some(path) = guard.current_path.as_ref() {
            let last = path.last()?;
            let z = guard
                .target_position
                .map(|pos| pos.z)
                .unwrap_or_else(|| guard.get_position().z);
            return Some(Coord3D::new(last.x, last.y, z));
        }
        None
    }

    fn get_locomotor_distance_to_goal(&self) -> Real {
        let Some(unit) = self.unit.upgrade() else {
            return 0.0;
        };
        let Ok(guard) = unit.read() else {
            return 0.0;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return 0.0;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return 0.0;
        };

        let obj_pos = guard.get_position();
        let is_projectile = guard
            .base_object()
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Projectile))
            .unwrap_or(false);
        let mut treat_as_aircraft = guard.path_extra_distance > PATHFIND_CLOSE_ENOUGH
            || loc_guard.get_appearance() == LocomotorAppearance::Hover;
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            treat_as_aircraft = jet_ai.get_treat_as_aircraft_for_loco_dist_to_goal();
        }

        if let Some(active_path) = loc_guard.active_path.as_ref() {
            let last_waypoint = active_path.waypoints.last().copied();
            let goal_pos = last_waypoint
                .or(guard.target_position)
                .or_else(|| {
                    guard
                        .path_following_state
                        .as_ref()
                        .map(|state| state.goal_position)
                })
                .unwrap_or(obj_pos);

            if loc_guard.is_close_enough_dist_3d() || is_projectile {
                return (goal_pos - obj_pos).length();
            }

            if treat_as_aircraft {
                let delta = goal_pos - obj_pos;
                let dist = delta.length();
                let dist_sqr = delta.x * delta.x + delta.y * delta.y;
                if dist * dist > dist_sqr {
                    return dist_sqr.sqrt();
                }
                return dist;
            }

            let dist_remaining = active_path.distance_remaining().max(0.0);
            let dist = if let Some(current_target) = active_path.current_target() {
                let delta = current_target - obj_pos;
                (delta.x * delta.x + delta.y * delta.y).sqrt() + dist_remaining
            } else {
                dist_remaining
            };

            let dx = goal_pos.x - obj_pos.x;
            let dy = goal_pos.y - obj_pos.y;
            let dist_sqr = dx * dx + dy * dy;
            if dist < PATHFIND_CELL_SIZE_F || dist * dist < dist_sqr {
                return dist_sqr.sqrt();
            }
            return dist;
        }

        if let Some(state) = guard.path_following_state.as_ref() {
            let delta = state.goal_position - obj_pos;
            return (delta.x * delta.x + delta.y * delta.y).sqrt();
        }

        0.0
    }

    fn get_speed(&self) -> f32 {
        self.unit
            .upgrade()
            .and_then(|unit| unit.read().ok().map(|guard| guard.current_speed))
            .unwrap_or(0.0)
    }

    fn get_last_command_source(&self) -> CommandSourceType {
        self.last_command_source
    }

    fn set_last_command_source(&mut self, source: CommandSourceType) {
        self.last_command_source = source;
    }

    fn get_current_command(&self) -> Option<crate::ai::AiCommandType> {
        self.current_command
    }

    fn get_pending_command_type(&self) -> Option<crate::ai::AiCommandType> {
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if let Some(cmd) = jet_ai.pending_command_type() {
                return Some(cmd);
            }
        }
        self.pending_command
    }

    fn purge_pending_command(&mut self) {
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            jet_ai.set_has_pending_command(false);
        }
        self.pending_command = None;
    }

    fn is_taxiing_to_parking(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_taxiing_to_parking())
            .unwrap_or(false)
    }

    fn is_reloading(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_reloading())
            .unwrap_or(false)
    }

    fn is_clearing_mines(&self) -> bool {
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let obj = guard.base_object();
        let Ok(obj_guard) = obj.read() else {
            return false;
        };
        if !obj_guard.test_status(ObjectStatusTypes::OBJECT_STATUS_IS_ATTACKING) {
            return false;
        }
        let Some((weapon, _slot)) = obj_guard.get_current_weapon() else {
            return false;
        };
        (weapon.get_anti_mask() & WeaponAntiMask::MINE) != 0
    }

    fn is_takeoff_or_landing_in_progress(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.is_takeoff_or_landing_in_progress())
            .unwrap_or(false)
    }

    fn get_current_state_id(&self) -> Option<u32> {
        self.ai_state_machine.as_ref().and_then(|machine| {
            machine
                .lock()
                .ok()
                .and_then(|guard| guard.get_current_state_id())
        })
    }

    fn get_parking_offset(&self) -> Real {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.parking_offset())
            .unwrap_or(0.0)
    }

    fn keeps_parking_space_when_airborne(&self) -> bool {
        self.jet_ai
            .as_ref()
            .map(|jet| jet.keeps_parking_space_when_airborne())
            .unwrap_or(true)
    }

    fn get_desired_speed(&self) -> Real {
        self.desired_speed
    }

    fn set_desired_speed(&mut self, speed: Real) {
        self.desired_speed = speed;
    }

    fn is_in_rappel_state(&self) -> bool {
        self.rappel_state.is_some()
    }

    fn is_doing_combat_drop(&self) -> bool {
        self.chinook_ai
            .as_ref()
            .map(|ai| ai.is_doing_combat_drop())
            .unwrap_or(false)
    }

    fn is_aircraft_that_adjusts_destination(&self) -> bool {
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return false;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return false;
        };
        matches!(
            loc_guard.get_appearance(),
            LocomotorAppearance::Hover | LocomotorAppearance::Wings
        )
    }

    fn is_moving_away_from(&self, obj_id: ObjectID) -> bool {
        let is_temp_move_out = self
            .ai_state_machine
            .as_ref()
            .and_then(|machine| machine.lock().ok())
            .map(|guard| guard.get_temporary_state() == Some(AIStateType::MoveOutOfTheWay as u32))
            .unwrap_or(false);
        if !is_temp_move_out {
            return false;
        }
        self.move_out_of_way_1 == obj_id || self.move_out_of_way_2 == obj_id
    }

    fn set_ignore_collision_time(&mut self, duration_frames: UnsignedInt) {
        self.ignore_collisions_until = TheGameLogic::get_frame().saturating_add(duration_frames);
    }

    fn get_ignore_collisions_until(&self) -> UnsignedInt {
        self.ignore_collisions_until
    }

    fn set_queue_for_path_time(&mut self, frames: UnsignedInt) {
        self.queue_for_path_frame = if frames == 0 {
            0
        } else {
            TheGameLogic::get_frame().saturating_add(frames)
        };
    }

    fn ignore_obstacle(
        &mut self,
        obj: Option<&Arc<RwLock<Object>>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.ignore_obstacle_id = obj
            .and_then(|handle| handle.read().ok().map(|guard| guard.get_id()))
            .unwrap_or(INVALID_ID);
        Ok(())
    }

    fn get_ignored_obstacle_id(&self) -> ObjectID {
        self.ignore_obstacle_id
    }

    fn is_ai_in_dead_state(&self) -> bool {
        self.ai_dead
    }

    fn mark_as_dead(&mut self) {
        self.ai_dead = true;
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(unit_guard) = unit.read() {
                if let Ok(mut object_guard) = unit_guard.base_object.write() {
                    object_guard.set_effectively_dead(true);
                }
            }
        }
    }

    fn set_is_recruitable(&mut self, recruitable: Bool) {
        self.is_recruitable = recruitable;
    }

    fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        let Some(machine) = self.ai_state_machine.as_ref() else {
            return None;
        };
        let Ok(guard) = machine.lock() else {
            return None;
        };
        guard.get_goal_object()
    }

    fn set_goal_object(&mut self, obj: Option<&Arc<RwLock<Object>>>) {
        let Some(machine) = self.ai_state_machine.as_ref() else {
            return;
        };
        let Ok(mut guard) = machine.lock() else {
            return;
        };
        let was_locked = guard.is_locked();
        guard.unlock();
        let obj_id = obj
            .and_then(|handle| handle.read().ok().map(|o| o.get_id()))
            .unwrap_or(INVALID_ID);
        guard.set_goal_object(obj_id);
        if was_locked {
            guard.lock();
        }
    }

    fn is_path_available(&self, destination: &Coord3D) -> bool {
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(ai) = THE_AI.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        let pos = guard.get_position();
        let ignore = if self.ignore_obstacle_id == INVALID_ID {
            None
        } else {
            Some(self.ignore_obstacle_id)
        };
        pf_guard.client_safe_quick_does_path_exist_with_ignore(
            &guard.locomotor_set,
            &pos,
            destination,
            ignore,
        )
    }

    fn request_path(&mut self, destination: &Coord3D, _is_final_goal: bool) -> Result<(), String> {
        self.requested_destination = *destination;
        self.is_final_goal = _is_final_goal;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = false;
        self.is_safe_path = false;
        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        let _ = self.ignore_obstacle(None);
        if self.can_compute_quick_path() {
            self.compute_quick_path(destination);
            return Ok(());
        }
        self.retry_path = false;
        if self.should_force_direct_path_for_off_map_start(destination)
            && self.install_direct_path_from_current_position(destination)
        {
            return Ok(());
        }
        if (self.get_current_state_id() == Some(u32::from(AIStateType::FollowExitProductionPath))
            || self.current_command == Some(crate::ai::AiCommandType::FollowExitProductionPath))
            && self.can_path_through_units
            && self.install_direct_path_from_current_position(destination)
        {
            let _ = self.set_can_path_through_units(false);
            return Ok(());
        }
        if self.should_use_direct_path_for_line_passable_non_final_goal(destination)
            && self.install_direct_path_from_current_position(destination)
        {
            return Ok(());
        }
        self.waiting_for_path = true;
        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND);
            if self.blocked_and_stuck {
                self.set_ignore_collision_time(LOGICFRAMES_PER_SECOND * 2);
                self.blocked_frames = 0;
                self.is_blocked = false;
                self.blocked_and_stuck = false;
            }
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*destination);
        self.path_timestamp = now;
        Ok(())
    }

    fn request_attack_path(
        &mut self,
        victim_id: ObjectID,
        victim_pos: &Coord3D,
    ) -> Result<(), String> {
        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        self.requested_destination = *victim_pos;
        self.requested_victim_id = victim_id;
        self.is_attack_path = true;
        self.is_approach_path = false;
        self.is_safe_path = false;
        self.waiting_for_path = true;
        let victim = get_legacy_object(victim_id);
        let _ = self.set_goal_object(victim.as_ref());
        let _ = self.ignore_obstacle(victim.as_ref());
        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            self.set_locomotor_goal_none();
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*victim_pos);
        self.path_timestamp = now;
        Ok(())
    }

    fn request_approach_path(&mut self, destination: &Coord3D) -> Result<(), String> {
        if !self.has_valid_locomotor_surfaces() {
            return Err("Attempting to path immobile unit".to_string());
        }
        self.requested_destination = *destination;
        self.is_final_goal = true;
        self.is_attack_path = false;
        self.requested_victim_id = INVALID_ID;
        self.is_approach_path = true;
        self.is_safe_path = false;
        self.waiting_for_path = true;
        let _ = self.ignore_obstacle(None);
        let now = TheGameLogic::get_frame();
        if self.path_timestamp > now.saturating_sub(3) {
            self.set_queue_for_path_time(LOGICFRAMES_PER_SECOND * 2);
            return Ok(());
        }
        self.set_queue_for_path_time(0);
        let _ = self.queue_path_request_now(*destination);
        self.path_timestamp = now;
        Ok(())
    }

    fn can_compute_quick_path(&self) -> bool {
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let locomotor = guard
            .current_locomotor
            .as_ref()
            .cloned()
            .or_else(|| guard.locomotor_set.get_default_locomotor());
        let Some(locomotor) = locomotor else {
            return false;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return false;
        };
        let surfaces = loc_guard.get_legal_surfaces();
        drop(loc_guard);
        drop(guard);
        let land_bound = (surfaces & SURFACE_AIR) == 0;
        if land_bound {
            return false;
        }
        !self.is_doing_ground_movement()
    }

    fn compute_quick_path(&mut self, destination: &Coord3D) -> bool {
        if !self.can_compute_quick_path() {
            return false;
        }
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(guard) = unit.read() {
                if let Some(path) = guard.current_path.as_ref() {
                    if let Some(last) = path.last() {
                        let dx = destination.x - last.x;
                        let dy = destination.y - last.y;
                        let path_goal_z = guard
                            .target_position
                            .unwrap_or_else(|| guard.get_position())
                            .z;
                        let dz = destination.z - path_goal_z;
                        if dx * dx + dy * dy + dz * dz < 0.25 {
                            return true;
                        }
                    }
                }
            }
        }

        self.install_direct_path_from_current_position(destination)
    }

    fn is_quick_path_available(&self, destination: &Coord3D) -> bool {
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(ai) = THE_AI.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        let pos = guard.get_position();
        pf_guard.client_safe_quick_does_path_exist_for_ui(&guard.locomotor_set, &pos, destination)
    }

    fn is_valid_locomotor_position(&self, pos: &Coord3D) -> bool {
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(ai) = THE_AI.read().ok() else {
            return false;
        };
        let Some(pathfinder) = ai.pathfinder() else {
            return false;
        };
        let Ok(pf_guard) = pathfinder.read() else {
            return false;
        };
        pf_guard.valid_movement_position(
            &guard.locomotor_set,
            guard.get_crusher_level() > 0,
            pos,
            if self.ignore_obstacle_id == INVALID_ID {
                None
            } else {
                Some(self.ignore_obstacle_id)
            },
        )
    }

    fn need_to_rotate(&self) -> bool {
        if self.is_waiting_for_path() {
            return true;
        }
        let Some(unit) = self.unit.upgrade() else {
            return false;
        };
        let Ok(guard) = unit.read() else {
            return false;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return false;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return false;
        };
        if loc_guard.template.wander_width_factor > 0.0 {
            return false;
        }
        let Some(active_path) = loc_guard.active_path.as_ref() else {
            return false;
        };
        let Some(target) = active_path.current_target() else {
            return false;
        };
        let pos = guard.get_position();
        let mut path_point = target;
        if active_path.current_waypoint + 1 < active_path.waypoints.len() {
            let start = active_path.waypoints[active_path.current_waypoint];
            let end = active_path.waypoints[active_path.current_waypoint + 1];
            let seg = Coord3D::new(end.x - start.x, end.y - start.y, 0.0);
            let seg_len_sqr = seg.x * seg.x + seg.y * seg.y;
            if seg_len_sqr > f32::EPSILON {
                let to_pos = Coord3D::new(pos.x - start.x, pos.y - start.y, 0.0);
                let mut t = (to_pos.x * seg.x + to_pos.y * seg.y) / seg_len_sqr;
                if t < 0.0 {
                    t = 0.0;
                } else if t > 1.0 {
                    t = 1.0;
                }
                path_point = Coord3D::new(start.x + seg.x * t, start.y + seg.y * t, pos.z);
            }
        }
        let delta = path_point - pos;
        if delta.length_squared() < f32::EPSILON {
            return false;
        }
        let desired_angle = delta.y.atan2(delta.x);
        let current_angle = guard.get_orientation();
        let mut delta_angle = desired_angle - current_angle;
        while delta_angle > std::f32::consts::PI {
            delta_angle -= std::f32::consts::PI * 2.0;
        }
        while delta_angle < -std::f32::consts::PI {
            delta_angle += std::f32::consts::PI * 2.0;
        }
        delta_angle.abs() > (std::f32::consts::PI / 30.0)
    }

    fn get_cur_locomotor_set_type(&self) -> LocomotorSetType {
        self.current_locomotor_set
    }

    fn has_locomotor_for_surface(&self, surface: crate::common::LocomotorSurfaceTypeMask) -> bool {
        let Some(entries) = self.locomotor_sets.get(&self.current_locomotor_set) else {
            return false;
        };
        for name in entries {
            if let Some(template) = crate::locomotor::LOCOMOTOR_STORE.get_template(name.as_str()) {
                if (template.surfaces & surface) != 0 {
                    return true;
                }
            }
        }
        false
    }

    fn get_cur_locomotor_speed(&self) -> Real {
        let Some(unit) = self.unit.upgrade() else {
            return 0.0;
        };
        let Ok(guard) = unit.read() else {
            return 0.0;
        };
        let Some(locomotor) = guard.current_locomotor.as_ref() else {
            return 0.0;
        };
        let Ok(loc_guard) = locomotor.lock() else {
            return 0.0;
        };
        let body_state = guard
            .base_object()
            .read()
            .ok()
            .and_then(|obj| obj.get_body_module())
            .and_then(|body| {
                body.lock()
                    .ok()
                    .map(|b| to_locomotor_body_damage_type(b.get_damage_state()))
            })
            .unwrap_or(BodyDamageType::Pristine);
        loc_guard.get_max_speed_for_condition(body_state)
    }

    fn get_cur_max_blocked_speed(&self) -> Real {
        self.cur_max_blocked_speed
    }

    fn set_cur_max_blocked_speed(&mut self, speed: Real) {
        self.cur_max_blocked_speed = speed;
    }

    fn set_locomotor_goal_none(&mut self) {
        self.locomotor_goal_type = 0;
        self.locomotor_goal_data = Coord3D::ZERO;
        if let Some(jet_ai) = self.jet_ai.as_ref() {
            if jet_ai.is_takeoff_or_landing_in_progress()
                && jet_ai.allow_air_loco()
                && !jet_ai.allow_circling()
            {
                if let Some(unit) = self.unit.upgrade() {
                    if let Ok(guard) = unit.read() {
                        let (dir_x, dir_y) = guard.get_unit_direction_vector_2d();
                        let mut desired = guard.get_position();
                        desired.x += dir_x * 1000.0;
                        desired.y += dir_y * 1000.0;
                        let _ = self.set_movement_target(&desired);
                        return;
                    }
                }
            }
        }

        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                guard.stop_movement();
            }
        }
    }

    fn set_locomotor_goal_orientation(&mut self, angle: Real) {
        self.locomotor_goal_type = 3;
        self.locomotor_goal_data = Coord3D::new(angle, 0.0, 0.0);
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                let _ = guard.set_orientation(angle);
            }
        }
    }

    fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
        self.locomotor_goal_type = 2;
        self.locomotor_goal_data = pos;
        let _ = self.set_movement_target(&pos);
    }

    fn friend_ending_move(&mut self) {
        self.queue_for_path_frame = 0;
        self.ignore_obstacle_id = INVALID_ID;
        self.movement_complete = true;
        self.locomotor_goal_type = 0;
        self.locomotor_goal_data = Coord3D::ZERO;
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                guard.stop_movement();
            }
        }
    }

    fn friend_starting_move(&mut self) {
        self.blocked_frames = 0;
        self.blocked_and_stuck = false;
        self.movement_complete = false;
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                guard.movement_state = MovementState::Moving;
            }
        }
    }

    fn evaluate_morale_bonus(&mut self) {
        let Some(unit_arc) = self.unit.upgrade() else {
            return;
        };
        let base_object = match unit_arc.read() {
            Ok(guard) => guard.base_object(),
            Err(_) => return,
        };
        let Ok(mut obj_guard) = base_object.write() else {
            return;
        };

        let mut nationalism = false;
        let mut fanaticism = false;
        if let Some(player) = obj_guard.get_controlling_player() {
            if let Ok(player_guard) = player.read() {
                if let Ok(center) = get_upgrade_center().read() {
                    if let Some(upgrade) = center.find_upgrade("Upgrade_Nationalism") {
                        if player_guard.has_upgrade_complete(&upgrade) {
                            nationalism = true;
                        }
                    }
                    if let Some(upgrade) = center.find_upgrade("Upgrade_Fanaticism") {
                        if player_guard.has_upgrade_complete(&upgrade) {
                            fanaticism = true;
                        }
                    }
                }
            }
        }

        let mut horde = false;
        let mut allow_nationalism = true;
        obj_guard.with_horde_update_interface(|hui| {
            if hui.is_in_horde() {
                horde = true;
                if !hui.is_allowed_nationalism() {
                    allow_nationalism = false;
                }
            }
        });

        if !allow_nationalism {
            nationalism = false;
            fanaticism = false;
        }

        let demoralized = self.demoralized_frames_left > 0;

        if !demoralized {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Demoralized);
        }

        if horde {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Horde);
        } else {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Horde);
        }

        if nationalism {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            if fanaticism {
                obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
            } else {
                obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
            }
        } else {
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);
        }

        if demoralized {
            obj_guard.set_weapon_bonus_condition(WeaponBonusConditionType::Demoralized);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Horde);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Nationalism);
            obj_guard.clear_weapon_bonus_condition(WeaponBonusConditionType::Fanaticism);

            if !obj_guard.is_kind_of(KindOf::PortableStructure) {
                if let Some(drawable) = obj_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        draw_guard.set_terrain_decal(TerrainDecalType::Demoralized);
                    }
                }
            }
        }
    }

    fn set_surrendered(&mut self, to_object: Option<&Arc<RwLock<Object>>>, surrendered: bool) {
        if surrendered {
            self.surrendered_frames_left = self.surrender_duration_frames;
            self.surrendered_player_index = to_object
                .and_then(|obj| obj.read().ok())
                .and_then(|guard| guard.get_controlling_player_id())
                .map(|idx| idx as PlayerIndex);
        } else {
            self.surrendered_frames_left = 0;
            self.surrendered_player_index = None;
        }
    }

    fn transfer_attack(&mut self, from_id: ObjectID, to_id: ObjectID) {
        use crate::helpers::TheGameLogic;

        let new_target = TheGameLogic::find_object_by_id(to_id);

        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                if guard.attack_target == Some(from_id) {
                    guard.attack_target = Some(to_id);
                }
            }
        }

        let goal_obj = self.get_goal_object();
        if let Some(ref obj) = goal_obj {
            if let Ok(g) = obj.read() {
                if g.get_id() == from_id {
                    self.set_goal_object(new_target.as_ref());
                }
            }
        }

        for turret in [TurretType::Primary, TurretType::Secondary] {
            let turret_ai = match turret {
                TurretType::Primary => self
                    .turret_primary_machine
                    .as_ref()
                    .and_then(|m| m.get_turret_ai()),
                TurretType::Secondary => self
                    .turret_secondary_machine
                    .as_ref()
                    .and_then(|m| m.get_turret_ai()),
                _ => continue,
            };
            let Some(turret_ai) = turret_ai else {
                continue;
            };
            let needs_transfer = if let Ok(ai_guard) = turret_ai.lock() {
                if let Some(target_obj) = ai_guard.get_current_target() {
                    if let Ok(tg) = target_obj.read() {
                        tg.get_id() == from_id
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
            if needs_transfer {
                self.set_turret_target_object(turret, new_target.as_ref(), true);
            }
        }
    }

    fn is_surrendered(&self) -> bool {
        self.surrendered_frames_left > 0
    }

    fn get_surrendered_player_index(&self) -> Option<PlayerIndex> {
        self.surrendered_player_index
    }

    fn ai_move_to_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.give_move_order(*pos, Vec::new(), false, false)?;
        Ok(())
    }

    fn ai_idle(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(state_machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut machine) = state_machine.lock() {
                machine.clear();
                let params = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::Idle,
                    crate::ai::CommandSourceType::FromAi,
                );
                let _ = machine.ai_do_command(&params);
                return Ok(());
            }
        }
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut guard) = unit.write() {
                guard.stop_movement();
            }
        }
        Ok(())
    }

    fn ai_busy(
        &mut self,
        cmd_source: crate::ai::CommandSourceType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let params = crate::ai::AiCommandParams::new(crate::ai::AiCommandType::Busy, cmd_source);
        self.execute_command(&params)
    }

    fn ai_attack_object(
        &mut self,
        target: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let target_id = target
            .read()
            .map(|guard| guard.get_id())
            .map_err(|_| "target lock poisoned")?;
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.give_attack_order(target_id, true, false)?;
        Ok(())
    }

    fn ai_guard_position(
        &mut self,
        pos: &Coord3D,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        self.push_guard_target_type(GuardTargetType::Location);
        self.location_to_guard = *pos;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.current_order = Some(UnitOrder::Guard {
            position: *pos,
            area_radius: guard.engagement_range,
        });
        guard.order_queue.clear();
        Ok(())
    }

    fn get_crate_id(&self) -> ObjectID {
        self.crate_created
            .lock()
            .map(|id| *id)
            .unwrap_or(crate::common::INVALID_ID)
    }

    fn get_current_victim(&self) -> Option<ObjectID> {
        let unit = self.unit.upgrade()?;
        let guard = unit.read().ok()?;
        guard.attack_target
    }

    fn set_current_victim(&mut self, victim: Option<ObjectID>) {
        let unit = match self.unit.upgrade() {
            Some(u) => u,
            None => return,
        };
        let mut guard = match unit.write() {
            Ok(g) => g,
            Err(_) => return,
        };

        if victim.is_none() && guard.attack_target.is_some() {
            let old_id = guard.attack_target.unwrap();
            if let Some(old_victim) = crate::helpers::TheGameLogic::find_object_by_id(old_id) {
                if let Ok(old_guard) = old_victim.read() {
                    if let Some(ai) = old_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            if let Ok(self_guard) = unit.read() {
                                ai_guard.add_targeter(self_guard.get_id(), false);
                            }
                        }
                    }
                }
            }
        }

        guard.attack_target = victim;
    }

    fn check_for_crate_to_pickup(&self) -> Option<Arc<RwLock<Object>>> {
        {
            let Ok(mut guard) = self.crate_created.lock() else {
                return None;
            };
            if *guard == crate::common::INVALID_ID {
                return None;
            }
            // C++ clears m_crateCreated before the lookup, so the processed marker
            // does not yield a crate object from this path.
            *guard = crate::common::INVALID_ID;
        }
        get_legacy_object(crate::common::INVALID_ID)
    }

    fn get_next_mood_target(
        &mut self,
        use_existing_target: bool,
        _ignore_attacked: bool,
    ) -> Option<Arc<RwLock<Object>>> {
        let unit = self.unit.upgrade()?;
        let guard = unit.read().ok()?;
        if !guard.can_auto_acquire_now() {
            return None;
        }

        let max_range = guard.engagement_range;
        if use_existing_target {
            if let Some(existing_id) = guard.attack_target {
                if let Some(existing_arc) =
                    crate::object::registry::OBJECT_REGISTRY.get_object(existing_id)
                {
                    if let Ok(existing_guard) = existing_arc.read() {
                        let relationship = guard
                            .base_object
                            .read()
                            .ok()
                            .map(|base| base.relationship_to(&existing_guard))
                            .unwrap_or(Relationship::Neutral);
                        if relationship == Relationship::Enemies {
                            let target_pos = *existing_guard.get_position();
                            let self_pos = guard.get_position();
                            let dx = target_pos.x - self_pos.x;
                            let dy = target_pos.y - self_pos.y;
                            let dist = (dx * dx + dy * dy).sqrt();
                            if dist <= max_range && guard.can_detect_target(&existing_guard, dist) {
                                return Some(existing_arc.clone());
                            }
                        }
                    }
                }
            }
        }

        let ai = THE_AI.read().ok()?;
        let ai_data = ai.get_ai_data();
        let ai_data_guard = ai_data.read().ok()?;

        let mut qualifiers = search_qualifiers::CAN_ATTACK;
        if ai_data_guard.attack_uses_line_of_sight {
            qualifiers |= search_qualifiers::CAN_SEE;
        }
        if ai_data_guard.attack_ignore_insignificant_buildings {
            qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
        }
        if guard.auto_acquire_attack_buildings {
            qualifiers |= search_qualifiers::ATTACK_BUILDINGS;
        }

        let target_id = ai
            .find_closest_enemy(guard.get_id(), max_range, qualifiers, None, None)
            .ok()
            .flatten()?;

        get_legacy_object(target_id)
    }

    fn get_next_mood_check_time(&self) -> u32 {
        let unit = self.unit.upgrade();
        let Some(unit) = unit else {
            return TheGameLogic::get_frame();
        };
        let Ok(guard) = unit.read() else {
            return TheGameLogic::get_frame();
        };
        let interval = guard.mood_attack_check_rate_frames.max(1);
        guard.last_target_scan_frame.saturating_add(interval)
    }

    fn reset_next_mood_check_time(&mut self) {
        let Some(unit) = self.unit.upgrade() else {
            return;
        };
        let Ok(mut guard) = unit.write() else {
            return;
        };
        guard.last_target_scan_frame = TheGameLogic::get_frame();
    }

    fn set_next_mood_check_time(&mut self, frame: u32) {
        let Some(unit) = self.unit.upgrade() else {
            return;
        };
        let Ok(mut guard) = unit.write() else {
            return;
        };
        let interval = guard.mood_attack_check_rate_frames.max(1);
        guard.last_target_scan_frame = frame.saturating_sub(interval);
    }

    fn get_mood_matrix_value(&self) -> u32 {
        if self.ai_state_machine.is_none() {
            return 0;
        }

        let Some(unit_arc) = self.unit.upgrade() else {
            return 0;
        };
        let Ok(unit_guard) = unit_arc.read() else {
            return 0;
        };
        let owner_arc = unit_guard.base_object();
        let Ok(owner_guard) = owner_arc.read() else {
            return 0;
        };
        let Some(player_arc) = owner_guard.get_controlling_player() else {
            return 0;
        };
        let Ok(player_guard) = player_arc.read() else {
            return 0;
        };

        let mut value = 0u32;
        if player_guard.get_player_type() == crate::player::PlayerType::Human {
            value |= mood_matrix_parameters::CONTROLLER_PLAYER;
        } else {
            value |= mood_matrix_parameters::CONTROLLER_AI;
            value |= match self.attitude {
                AIAttitudeType::Passive => mood_matrix_parameters::MOOD_PASSIVE,
                AIAttitudeType::Defensive => mood_matrix_parameters::MOOD_ALERT,
                AIAttitudeType::Aggressive => mood_matrix_parameters::MOOD_AGGRESSIVE,
                AIAttitudeType::Sleep => mood_matrix_parameters::MOOD_SLEEP,
                AIAttitudeType::Normal => mood_matrix_parameters::MOOD_NORMAL,
            };
        }

        let is_air = unit_guard
            .get_locomotor_surface_mask()
            .map(|surfaces| (surfaces & SURFACE_AIR) != 0)
            .unwrap_or(false);
        if is_air {
            value |= mood_matrix_parameters::UNITTYPE_AIR;
        } else if self.turret_primary_machine.is_some() {
            value |= mood_matrix_parameters::UNITTYPE_TURRETED;
        } else {
            value |= mood_matrix_parameters::UNITTYPE_NON_TURRETED;
        }

        value
    }

    fn get_mood_matrix_action_adjustment(&mut self, action: MoodMatrixAction) -> u32 {
        let Some(unit_arc) = self.unit.upgrade() else {
            return mood_matrix_adjustment::ACTION_OK;
        };
        let Ok(unit_guard) = unit_arc.read() else {
            return mood_matrix_adjustment::ACTION_OK;
        };
        let owner_arc = unit_guard.base_object();
        let Ok(owner_guard) = owner_arc.read() else {
            return mood_matrix_adjustment::ACTION_OK;
        };

        // Mirror C++ mob-member special case that ignores mood conversions.
        if owner_guard.is_kind_of(KindOf::Infantry) && owner_guard.is_kind_of(KindOf::IgnoredInGui)
        {
            return mood_matrix_adjustment::ACTION_OK;
        }

        let mood_matrix = self.get_mood_matrix_value();
        if (mood_matrix & mood_matrix_parameters::CONTROLLER_PLAYER) != 0 {
            return mood_matrix_adjustment::ACTION_OK;
        }

        match action {
            MoodMatrixAction::Idle => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                mood_matrix_parameters::MOOD_PASSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_WAIT_FOR_ATTACK
                }
                mood_matrix_parameters::MOOD_ALERT => {
                    mood_matrix_adjustment::ACTION_OK | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                }
                mood_matrix_parameters::MOOD_AGGRESSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::Move => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_TO_IDLE
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                mood_matrix_parameters::MOOD_PASSIVE => {
                    mood_matrix_adjustment::ACTION_OK
                        | mood_matrix_adjustment::AFFECT_RANGE_WAIT_FOR_ATTACK
                }
                mood_matrix_parameters::MOOD_ALERT => {
                    mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE
                        | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                }
                mood_matrix_parameters::MOOD_AGGRESSIVE => {
                    mood_matrix_adjustment::ACTION_TO_ATTACK_MOVE
                        | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::Attack => match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                mood_matrix_parameters::MOOD_SLEEP => {
                    mood_matrix_adjustment::ACTION_TO_IDLE
                        | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                }
                _ => mood_matrix_adjustment::ACTION_OK,
            },
            MoodMatrixAction::AttackMove => {
                match mood_matrix & mood_matrix_parameters::MOOD_BITMASK {
                    mood_matrix_parameters::MOOD_SLEEP => {
                        mood_matrix_adjustment::ACTION_TO_IDLE
                            | mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL
                    }
                    mood_matrix_parameters::MOOD_ALERT => {
                        mood_matrix_adjustment::ACTION_OK
                            | mood_matrix_adjustment::AFFECT_RANGE_ALERT
                    }
                    mood_matrix_parameters::MOOD_AGGRESSIVE => {
                        mood_matrix_adjustment::ACTION_OK
                            | mood_matrix_adjustment::AFFECT_RANGE_AGGRESSIVE
                    }
                    _ => mood_matrix_adjustment::ACTION_OK,
                }
            }
        }
    }

    fn notify_fired(&mut self) {}

    fn notify_new_victim_chosen(&mut self, victim: ObjectID) {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                guard.set_goal_object(victim);
            }
        }
        if let Some(unit) = self.unit.upgrade() {
            if let Ok(mut unit_guard) = unit.write() {
                unit_guard.attack_target = Some(victim);
            }
        }
    }

    fn is_weapon_slot_ok_to_fire(&self, _wslot: WeaponSlotType) -> Bool {
        if self.turrets_linked {
            return true;
        }

        let has_primary = self.turret_primary_machine.is_some();
        let has_secondary = self.turret_secondary_machine.is_some();
        if !has_primary && !has_secondary {
            return true;
        }

        match _wslot {
            WeaponSlotType::Primary => has_primary && self.turret_primary_enabled,
            WeaponSlotType::Secondary => has_secondary && self.turret_secondary_enabled,
            WeaponSlotType::Tertiary => !has_primary && !has_secondary,
        }
    }

    fn get_original_victim_pos(&self) -> Option<Coord3D> {
        self.original_victim_pos
    }

    fn set_original_victim_pos(&mut self, pos: Option<Coord3D>) {
        self.original_victim_pos = pos;
    }

    fn is_in_attack_state(&self) -> bool {
        self.ai_state_machine
            .as_ref()
            .and_then(|machine| machine.lock().ok().map(|guard| guard.is_in_attack_state()))
            .unwrap_or(false)
    }

    fn is_in_guard_idle_state(&self) -> bool {
        self.ai_state_machine
            .as_ref()
            .and_then(|machine| {
                machine
                    .lock()
                    .ok()
                    .map(|guard| guard.is_in_guard_idle_state())
            })
            .unwrap_or(false)
    }

    fn set_temporary_state(&mut self, state: AIStateType, frame_limit: UnsignedInt) {
        if let Some(machine) = self.ai_state_machine.as_ref() {
            if let Ok(mut guard) = machine.lock() {
                let _ = guard.set_temporary_state(state as u32, frame_limit);
            }
        }
    }

    fn notify_crate(&mut self, crate_id: ObjectID) {
        if let Ok(mut guard) = self.crate_created.lock() {
            *guard = crate_id;
        }
    }

    fn notify_victim_is_dead(&mut self) {
        if let Some(jet_ai) = self.jet_ai.as_mut() {
            jet_ai.notify_victim_is_dead();
        }
    }

    fn set_prior_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.prior_waypoint_id = Some(waypoint_id);
    }

    fn set_current_waypoint_id(&mut self, waypoint_id: crate::waypoint::WaypointId) {
        self.current_waypoint_id = Some(waypoint_id);
    }

    fn set_completed_waypoint_id(&mut self, waypoint_id: Option<crate::waypoint::WaypointId>) {
        self.completed_waypoint_id = waypoint_id;
    }

    fn get_completed_waypoint_id(&self) -> Option<crate::waypoint::WaypointId> {
        self.completed_waypoint_id
    }

    fn get_supply_truck_ai_interface(&self) -> Option<&dyn crate::modules::SupplyTruckAIInterface> {
        if let Some(ai) = self.chinook_ai.as_ref() {
            Some(ai as &dyn crate::modules::SupplyTruckAIInterface)
        } else if let Some(ai) = self.supply_truck_ai.as_ref() {
            Some(ai as &dyn crate::modules::SupplyTruckAIInterface)
        } else {
            self.worker_ai
                .as_ref()
                .map(|ai| ai as &dyn crate::modules::SupplyTruckAIInterface)
        }
    }

    fn get_supply_truck_ai_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::SupplyTruckAIInterface> {
        if let Some(ai) = self.chinook_ai.as_mut() {
            Some(ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        } else if let Some(ai) = self.supply_truck_ai.as_mut() {
            Some(ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        } else {
            self.worker_ai
                .as_mut()
                .map(|ai| ai as &mut dyn crate::modules::SupplyTruckAIInterface)
        }
    }

    fn get_pow_truck_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::POWTruckAIUpdateInterface> {
        #[cfg(feature = "allow_surrender")]
        {
            return self
                .pow_truck_ai
                .as_mut()
                .map(|ai| ai as &mut dyn crate::modules::POWTruckAIUpdateInterface);
        }
        #[cfg(not(feature = "allow_surrender"))]
        {
            None
        }
    }

    fn get_hack_internet_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::HackInternetAIUpdateInterface> {
        self.hack_internet_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::HackInternetAIUpdateInterface)
    }

    fn get_assault_transport_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::AssaultTransportAIUpdateInterface> {
        self.assault_transport_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::AssaultTransportAIUpdateInterface)
    }

    fn get_worker_ai_update_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::WorkerAIUpdateInterface> {
        self.worker_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::WorkerAIUpdateInterface)
    }

    fn get_dozer_ai_update_interface_mut(
        &mut self,
    ) -> Option<&mut dyn crate::modules::DozerAIUpdateInterface> {
        self.dozer_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::DozerAIUpdateInterface)
    }

    fn get_deliver_payload_ai_update_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::DeliverPayloadAIUpdateInterface> {
        self.deliver_payload_ai
            .as_mut()
            .map(|ai| ai as &mut dyn crate::modules::DeliverPayloadAIUpdateInterface)
    }

    fn ai_guard_object(
        &mut self,
        obj_to_guard: &Arc<RwLock<Object>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (target_id, target_pos) = obj_to_guard
            .read()
            .map(|guard| (guard.get_id(), *guard.get_position()))
            .map_err(|_| "target lock poisoned")?;
        let unit = self
            .unit
            .upgrade()
            .ok_or_else(|| "unit no longer available".to_string())?;
        self.push_guard_target_type(GuardTargetType::Object);
        self.object_to_guard = target_id;
        let mut guard = unit.write().map_err(|_| "unit lock poisoned".to_string())?;
        guard.current_order = Some(UnitOrder::Guard {
            position: target_pos,
            area_radius: guard.engagement_range,
        });
        guard.order_queue.clear();
        Ok(())
    }

    fn ai_go_prone(&mut self, damage_info: &DamageInfo, _cmd_source: crate::ai::CommandSourceType) {
        let Some(unit) = self.unit.upgrade() else {
            return;
        };
        let Ok(unit_guard) = unit.read() else {
            return;
        };
        let obj_arc = unit_guard.base_object();
        let module = {
            let Ok(obj_guard) = obj_arc.read() else {
                return;
            };
            obj_guard.find_update_module("ProneUpdate")
        };
        let Some(module) = module else {
            return;
        };
        let damage = damage_info.output.actual_damage_dealt as i32;
        module.with_module(|module| {
            if let Some(prone) = module.get_prone_control_interface() {
                prone.go_prone(damage);
            }
        });
    }
}

impl Drop for UnitAIUpdate {
    fn drop(&mut self) {
        let Some(unit) = self.unit.upgrade() else {
            return;
        };
        let Ok(guard) = unit.read() else {
            return;
        };
        let owner_id = guard
            .base_object
            .read()
            .ok()
            .map(|obj| obj.get_id())
            .unwrap_or(INVALID_ID);
        let is_immobile = guard
            .base_object
            .read()
            .ok()
            .map(|obj| obj.is_kind_of(KindOf::Immobile))
            .unwrap_or(false);
        if is_immobile {
            return;
        }
        let (radius, center_in_cell) = Self::compute_pathfind_radius_and_center(&guard);
        drop(guard);

        if let Ok(ai_lock) = THE_AI.read() {
            if let Some(pathfinder) = ai_lock.pathfinder() {
                if let Ok(mut pf_guard) = pathfinder.write() {
                    self.remove_goal_cells(&mut pf_guard, owner_id, radius, center_in_cell);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locomotor::LocomotorTemplate;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn unit_ai_update_without_unit() -> UnitAIUpdate {
        UnitAIUpdate::new(
            Weak::new(),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        )
    }

    fn test_turret_machine() -> TurretStateMachine {
        let turret_ai = Arc::new(Mutex::new(TurretAI::new(Weak::new())));
        TurretStateMachine::new(Some(turret_ai), Weak::new(), "TurretAI")
    }

    #[test]
    fn mark_as_dead_sets_owner_effectively_dead_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(42, 100.0)));
        let template = DefaultThingTemplate::new("TestUnit".to_string());
        let unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        ai.mark_as_dead();

        assert!(ai.is_ai_in_dead_state());
        assert!(base_object.read().unwrap().is_effectively_dead());
    }

    #[test]
    fn compute_quick_path_preserves_cpp_start_and_destination_nodes() {
        let base_object = Arc::new(RwLock::new(Object::new_test(43, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(3.0, 4.0, 2.0));
        }
        let template = DefaultThingTemplate::new("AirUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_thrust("AirLoco".to_string()));
        unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let destination = Coord3D::new(30.0, 40.0, 12.0);
        assert!(ai.compute_quick_path(&destination));

        {
            let unit_guard = unit.read().unwrap();
            let path = unit_guard.current_path.as_ref().unwrap();
            assert_eq!(
                path,
                &vec![Coord2D::new(3.0, 4.0), Coord2D::new(30.0, 40.0)]
            );
        }
    }

    #[test]
    fn request_path_for_off_map_start_uses_direct_path_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(44, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(-100.0, -100.0, 5.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let destination = Coord3D::new(-50.0, -25.0, 9.0);
        ai.request_path(&destination, true).unwrap();

        let unit_guard = unit.read().unwrap();
        let path = unit_guard.current_path.as_ref().unwrap();
        assert_eq!(
            path,
            &vec![Coord2D::new(-100.0, -100.0), Coord2D::new(-50.0, -25.0)]
        );
        assert_eq!(unit_guard.target_position, Some(destination));
        assert_eq!(ai.queue_for_path_frame, 0);
    }

    #[test]
    fn request_path_for_exit_production_uses_direct_path_and_clears_unit_phasing_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(45, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(0.0, 0.0, 2.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        ai.set_can_path_through_units(true).unwrap();
        ai.current_command = Some(crate::ai::AiCommandType::FollowExitProductionPath);

        let destination = Coord3D::new(0.0, 0.0, 6.0);
        ai.request_path(&destination, true).unwrap();

        assert!(!ai.can_path_through_units);
        assert_eq!(ai.queue_for_path_frame, 0);
        let unit_guard = unit.read().unwrap();
        assert_eq!(
            unit_guard.current_path.as_ref().unwrap(),
            &vec![Coord2D::new(0.0, 0.0), Coord2D::new(0.0, 0.0)]
        );
        assert_eq!(unit_guard.target_position, Some(destination));
    }

    #[test]
    fn request_path_for_non_final_line_passable_ground_move_uses_direct_path_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(46, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        let destination = Coord3D::new(16.0, 0.0, 3.0);
        ai.retry_path = true;
        ai.request_path(&destination, false).unwrap();

        assert!(!ai.retry_path);
        assert_eq!(ai.queue_for_path_frame, 0);
        let unit_guard = unit.read().unwrap();
        assert_eq!(
            unit_guard.current_path.as_ref().unwrap(),
            &vec![Coord2D::new(0.0, 0.0), Coord2D::new(16.0, 0.0)]
        );
        assert_eq!(unit_guard.target_position, Some(destination));
    }

    #[test]
    fn line_passable_direct_path_requires_non_final_goal_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(47, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let destination = Coord3D::new(16.0, 0.0, 3.0);

        ai.is_final_goal = true;
        assert!(!ai.should_use_direct_path_for_line_passable_non_final_goal(&destination));

        ai.is_final_goal = false;
        assert!(ai.should_use_direct_path_for_line_passable_non_final_goal(&destination));
    }

    #[test]
    fn invalid_destination_falls_back_to_closest_path_and_marks_retry_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(48, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(10.0, 0.0, 1.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        assert!(ai
            .try_install_closest_path_for_invalid_destination(&Coord3D::new(-5.0, 0.0, 3.0))
            .unwrap());

        assert!(ai.retry_path);
        assert_eq!(ai.queue_for_path_frame, 0);
        let unit_guard = unit.read().unwrap();
        let path = unit_guard.current_path.as_ref().unwrap();
        assert_eq!(path.first(), Some(&Coord2D::new(10.0, 0.0)));
        assert_ne!(
            unit_guard.target_position,
            Some(Coord3D::new(-5.0, 0.0, 3.0))
        );
    }

    #[test]
    fn stuck_old_path_failure_stops_and_waits_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(49, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(10.0, 0.0, 1.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
        unit.current_path = Some(vec![Coord2D::new(10.0, 0.0), Coord2D::new(20.0, 0.0)]);
        unit.path_index = 1;
        unit.target_position = Some(Coord3D::new(20.0, 0.0, 0.0));
        unit.movement_state = MovementState::Moving;
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        ai.set_current_path_snapshot_from_coords(&[
            Coord3D::new(10.0, 0.0, 1.0),
            Coord3D::new(20.0, 0.0, 0.0),
        ]);
        ai.is_blocked = true;
        ai.blocked_and_stuck = true;
        ai.blocked_frames = 12;
        ai.locomotor_goal_type = 1;
        ai.locomotor_goal_data = Coord3D::new(20.0, 0.0, 0.0);

        assert!(ai
            .try_install_closest_path_for_invalid_destination(&Coord3D::new(-5.0, 0.0, 3.0))
            .unwrap());

        assert_eq!(
            ai.queue_for_path_frame,
            TheGameLogic::get_frame().saturating_add(LOGICFRAMES_PER_SECOND)
        );
        assert_eq!(ai.blocked_frames, 0);
        assert!(!ai.is_blocked);
        assert!(!ai.blocked_and_stuck);
        assert_eq!(ai.locomotor_goal_type, 0);
        assert_eq!(ai.locomotor_goal_data, Coord3D::ZERO);
        assert!(ai.current_path_snapshot.is_none());
        let unit_guard = unit.read().unwrap();
        assert!(unit_guard.current_path.is_none());
        assert_eq!(unit_guard.path_index, 0);
        assert_eq!(unit_guard.movement_state, MovementState::Idle);
        assert_ne!(
            unit_guard.target_position,
            Some(Coord3D::new(20.0, 0.0, 0.0))
        );
    }

    #[test]
    fn set_path_from_waypoint_prepends_current_position_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(57, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(3.0, 4.0, 2.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
        unit.current_path = Some(vec![Coord2D::new(99.0, 99.0)]);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        ai.set_current_path_snapshot_from_coords(&[Coord3D::new(99.0, 99.0, 0.0)]);

        let waypoint = crate::waypoint::Waypoint::new(
            5700,
            Coord3D::new(31.3, 42.7, 17.0),
            "Terminal".to_string(),
        );
        let raw_terminal = Coord3D::new(33.8, 39.2, 0.0);
        let expected_terminal = THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.pathfinder())
            .and_then(|pathfinder| {
                pathfinder
                    .read()
                    .ok()
                    .map(|pf| pf.snap_position(&raw_terminal))
            })
            .unwrap_or(raw_terminal);

        ai.set_path_from_waypoint(&waypoint, &Coord2D::new(2.5, -3.5))
            .unwrap();

        let unit_guard = unit.read().unwrap();
        let path = unit_guard.current_path.as_ref().unwrap();
        assert_eq!(path.len(), 2);
        assert_eq!(path[0], Coord2D::new(3.0, 4.0));
        assert_eq!(
            path[1],
            Coord2D::new(expected_terminal.x, expected_terminal.y)
        );
        assert_eq!(unit_guard.target_position, Some(expected_terminal));
        assert_eq!(unit_guard.movement_state, MovementState::Moving);

        let snapshot = ai.current_path_snapshot.as_ref().unwrap();
        assert_eq!(
            snapshot.get_first_node().unwrap().get_position(),
            &Coord3D::new(3.0, 4.0, 2.0)
        );
        assert!(!ai.waiting_for_path);
    }

    #[test]
    fn check_for_crate_to_pickup_consumes_marker_before_lookup_like_cpp() {
        let crate_id = 58;
        let crate_object = Arc::new(RwLock::new(Object::new_test(crate_id, 100.0)));
        crate::ai::object_registry::register_legacy_object(&crate_object);

        let mut ai = unit_ai_update_without_unit();
        ai.notify_crate(crate_id);

        assert!(ai.check_for_crate_to_pickup().is_none());
        assert_eq!(ai.get_crate_id(), INVALID_ID);

        crate::ai::object_registry::unregister_legacy_object(crate_id);
    }

    #[test]
    fn unit_choose_locomotor_set_preserves_current_when_set_missing_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(59, 100.0)));
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(Arc::clone(&locomotor));
        let unit = Arc::new(RwLock::new(unit));

        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        ai.current_locomotor_set = LocomotorSetType::Normal;
        ai.locomotor_sets.clear();

        ai.choose_locomotor_set(LocomotorSetType::Wander).unwrap();

        assert_eq!(ai.current_locomotor_set, LocomotorSetType::Normal);
        let unit_guard = unit.read().unwrap();
        assert!(unit_guard
            .current_locomotor
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &locomotor)));
    }

    #[test]
    fn update_consumes_completed_movement_cleanup_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(60, 100.0)));
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        unit.current_path = Some(vec![Coord2D::new(1.0, 1.0), Coord2D::new(2.0, 2.0)]);
        unit.target_position = Some(Coord3D::new(2.0, 2.0, 0.0));
        unit.movement_state = MovementState::Moving;
        let unit = Arc::new(RwLock::new(unit));

        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        ai.set_current_path_snapshot_from_coords(&[
            Coord3D::new(1.0, 1.0, 0.0),
            Coord3D::new(2.0, 2.0, 0.0),
        ]);
        ai.movement_complete = true;
        ai.queue_for_path_frame = TheGameLogic::get_frame().saturating_add(20);
        ai.ignore_obstacle_id = 1234;
        ai.locomotor_goal_type = 2;
        ai.locomotor_goal_data = Coord3D::new(2.0, 2.0, 0.0);

        ai.update().unwrap();

        assert!(!ai.movement_complete);
        assert_eq!(ai.queue_for_path_frame, 0);
        assert_eq!(ai.ignore_obstacle_id, INVALID_ID);
        assert_eq!(ai.locomotor_goal_type, 0);
        assert_eq!(ai.locomotor_goal_data, Coord3D::ZERO);
        assert!(ai.current_path_snapshot.is_none());

        let unit_guard = unit.read().unwrap();
        assert!(unit_guard.current_path.is_none());
        assert_eq!(unit_guard.movement_state, MovementState::Idle);
    }

    #[test]
    fn queue_waypoint_does_not_append_past_cpp_limit() {
        let base_object = Arc::new(RwLock::new(Object::new_test(61, 100.0)));
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let unit = Arc::new(RwLock::new(
            Unit::new(Arc::clone(&base_object), &template).unwrap(),
        ));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        for idx in 0..=AI_UPDATE_MAX_WAYPOINTS {
            ai.queue_waypoint(&Coord3D::new(idx as Real, 0.0, 0.0));
        }

        assert_eq!(ai.planning_waypoint_count, AI_UPDATE_MAX_WAYPOINTS as Int);
        assert_eq!(
            unit.read().unwrap().waypoint_queue.len(),
            AI_UPDATE_MAX_WAYPOINTS
        );
        assert_eq!(
            ai.planning_waypoint_queue[AI_UPDATE_MAX_WAYPOINTS - 1],
            Coord3D::new((AI_UPDATE_MAX_WAYPOINTS - 1) as Real, 0.0, 0.0)
        );
    }

    #[test]
    fn destroy_path_clears_attack_and_locomotor_goal_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(62, 100.0)));
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        unit.current_path = Some(vec![Coord2D::new(0.0, 0.0), Coord2D::new(8.0, 0.0)]);
        unit.target_position = Some(Coord3D::new(8.0, 0.0, 0.0));
        unit.movement_state = MovementState::Moving;
        let unit = Arc::new(RwLock::new(unit));

        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        ai.is_attack_path = true;
        ai.waiting_for_path = true;
        ai.locomotor_goal_type = 2;
        ai.locomotor_goal_data = Coord3D::new(8.0, 0.0, 0.0);
        ai.set_current_path_snapshot_from_coords(&[
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(8.0, 0.0, 0.0),
        ]);

        ai.destroy_path();

        assert!(ai.current_path_snapshot.is_none());
        assert!(!ai.waiting_for_path);
        assert!(!ai.is_attack_path);
        assert_eq!(ai.locomotor_goal_type, 0);
        assert_eq!(ai.locomotor_goal_data, Coord3D::ZERO);

        let unit_guard = unit.read().unwrap();
        assert!(unit_guard.current_path.is_none());
        assert_eq!(unit_guard.movement_state, MovementState::Idle);
    }

    #[test]
    fn request_path_waits_until_queued_pathfind_installs_path_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(50, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let destination = Coord3D::new(0.0, 0.0, 0.0);

        ai.request_path(&destination, true).unwrap();

        assert!(ai.waiting_for_path);
        assert!(ai.is_waiting_for_path());
        {
            let unit_guard = unit.read().unwrap();
            assert!(unit_guard.target_position.is_none());
            assert!(unit_guard.current_path.is_none());
        }

        ai.update().unwrap();

        assert!(!ai.waiting_for_path);
        assert!(!ai.is_waiting_for_path());
        let unit_guard = unit.read().unwrap();
        assert!(unit_guard.target_position.is_some());
        assert!(unit_guard.current_path.is_some());
    }

    #[test]
    fn request_attack_path_enters_wait_state_before_repath_delay_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(53, 100.0)));
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let now = TheGameLogic::get_frame();
        ai.path_timestamp = now.saturating_add(1);
        let destination = Coord3D::new(12.0, 4.0, 0.0);

        ai.request_attack_path(INVALID_ID, &destination).unwrap();

        assert!(ai.is_attack_path);
        assert!(ai.waiting_for_path);
        assert!(ai.is_waiting_for_path());
        assert_eq!(
            ai.queue_for_path_frame,
            now.saturating_add(LOGICFRAMES_PER_SECOND * 2)
        );
    }

    #[test]
    fn queued_attack_path_fallback_clears_attack_and_tracks_live_victim_like_cpp() {
        let owner_id = 57;
        let victim_id = 157;
        let base_object = Arc::new(RwLock::new(Object::new_test(owner_id, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
        }
        let victim = Arc::new(RwLock::new(Object::new_test(victim_id, 100.0)));
        {
            let mut object = victim.write().unwrap();
            let _ = object.set_position(&Coord3D::new(20.0, 0.0, 0.0));
        }
        crate::object::registry::OBJECT_REGISTRY.register_object(owner_id, &base_object);
        crate::object::registry::OBJECT_REGISTRY.register_object(victim_id, &victim);
        crate::ai::object_registry::register_legacy_object(&base_object);
        crate::ai::object_registry::register_legacy_object(&victim);

        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        ai.request_attack_path(victim_id, &Coord3D::new(10.0, 0.0, 0.0))
            .unwrap();

        assert!(ai.is_attack_path);
        assert_eq!(ai.requested_destination, Coord3D::new(10.0, 0.0, 0.0));

        ai.update().unwrap();

        assert!(!ai.is_attack_path);
        assert!(!ai.waiting_for_path);
        assert_eq!(ai.requested_destination, Coord3D::new(20.0, 0.0, 0.0));
        assert_eq!(ai.ignore_obstacle_id, victim_id);
        let unit_guard = unit.read().unwrap();
        assert_eq!(
            unit_guard.target_position,
            Some(Coord3D::new(20.0, 0.0, 0.0))
        );
        assert!(unit_guard.current_path.is_some());

        crate::object::registry::OBJECT_REGISTRY.unregister_object(owner_id);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(victim_id);
        crate::ai::object_registry::unregister_legacy_object(owner_id);
        crate::ai::object_registry::unregister_legacy_object(victim_id);
    }

    #[test]
    fn request_approach_path_enters_wait_state_before_repath_delay_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(54, 100.0)));
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let now = TheGameLogic::get_frame();
        ai.path_timestamp = now.saturating_add(1);
        let destination = Coord3D::new(18.0, 6.0, 0.0);

        ai.request_approach_path(&destination).unwrap();

        assert!(ai.is_approach_path);
        assert!(ai.waiting_for_path);
        assert!(ai.is_waiting_for_path());
        assert_eq!(
            ai.queue_for_path_frame,
            now.saturating_add(LOGICFRAMES_PER_SECOND * 2)
        );
    }

    #[test]
    fn request_approach_path_defers_closest_path_until_queued_update_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(56, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(0.0, 0.0, 1.0));
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        let old_destination = Coord3D::new(10.0, 0.0, 0.0);
        ai.set_path_from_coords(&[Coord3D::new(0.0, 0.0, 1.0), old_destination])
            .unwrap();
        ai.path_timestamp = 0;
        let approach_destination = Coord3D::new(24.0, 0.0, 0.0);

        ai.request_approach_path(&approach_destination).unwrap();

        assert!(ai.waiting_for_path);
        {
            let unit_guard = unit.read().unwrap();
            assert_eq!(unit_guard.target_position, Some(old_destination));
            assert!(unit_guard.current_path.is_some());
        }

        ai.update().unwrap();

        assert!(!ai.waiting_for_path);
        let unit_guard = unit.read().unwrap();
        assert!(unit_guard.current_path.is_some());
        assert_eq!(unit_guard.target_position, Some(approach_destination));
    }

    #[test]
    fn request_safe_path_enters_wait_state_before_repath_delay_like_cpp() {
        let mut ai = unit_ai_update_without_unit();
        let previous_repulsor = 71;
        let next_repulsor = 72;
        ai.repulsor1 = previous_repulsor;
        let now = TheGameLogic::get_frame();
        ai.path_timestamp = now.saturating_add(1);

        assert!(!ai.request_safe_path(next_repulsor).unwrap());

        assert_eq!(ai.repulsor2, previous_repulsor);
        assert_eq!(ai.repulsor1, next_repulsor);
        assert!(ai.is_safe_path);
        assert!(!ai.is_approach_path);
        assert!(!ai.is_attack_path);
        assert_eq!(ai.requested_victim_id, INVALID_ID);
        assert!(ai.waiting_for_path);
        assert!(ai.is_waiting_for_path());
        assert_eq!(
            ai.queue_for_path_frame,
            now.saturating_add(LOGICFRAMES_PER_SECOND * 2)
        );
    }

    #[test]
    fn request_safe_path_defers_safe_pathfind_until_queued_update_like_cpp() {
        let owner_id = 55;
        let repulsor_id = 155;
        let base_object = Arc::new(RwLock::new(Object::new_test(owner_id, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            let _ = object.set_position(&Coord3D::new(100.0, 100.0, 1.0));
            object.set_vision_range(30.0);
        }
        let repulsor = Arc::new(RwLock::new(Object::new_test(repulsor_id, 100.0)));
        {
            let mut object = repulsor.write().unwrap();
            let _ = object.set_position(&Coord3D::new(100.0, 100.0, 0.0));
        }
        crate::object::registry::OBJECT_REGISTRY.register_object(owner_id, &base_object);
        crate::object::registry::OBJECT_REGISTRY.register_object(repulsor_id, &repulsor);

        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let locomotor = Arc::new(Mutex::new(Locomotor::new(loco_template)));
        unit.locomotor_set
            .add_locomotor("GroundLoco".to_string(), Arc::clone(&locomotor));
        unit.current_locomotor = Some(locomotor);
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );

        assert!(ai.request_safe_path(repulsor_id).unwrap());

        assert!(ai.waiting_for_path);
        assert!(ai.pending_safe_path.is_none());
        {
            let unit_guard = unit.read().unwrap();
            assert!(unit_guard.current_path.is_none());
            assert!(unit_guard.target_position.is_none());
        }

        ai.update().unwrap();

        assert!(!ai.waiting_for_path);
        let unit_guard = unit.read().unwrap();
        assert!(unit_guard.current_path.is_some());
        assert!(unit_guard.target_position.is_some());

        crate::object::registry::OBJECT_REGISTRY.unregister_object(owner_id);
        crate::object::registry::OBJECT_REGISTRY.unregister_object(repulsor_id);
    }

    #[test]
    fn installed_path_uses_exact_requested_destination_for_ultra_accurate_loco_like_cpp() {
        let base_object = Arc::new(RwLock::new(Object::new_test(51, 100.0)));
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        let mut loco = Locomotor::new(loco_template);
        loco.set_ultra_accurate(true);
        unit.current_locomotor = Some(Arc::new(Mutex::new(loco)));
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        ai.requested_destination = Coord3D::new(14.25, 2.5, 3.0);

        ai.set_path_from_coords(&[Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(14.0, 2.0, 0.0)])
            .unwrap();

        let unit_guard = unit.read().unwrap();
        assert_eq!(unit_guard.target_position, Some(ai.requested_destination));
        assert_eq!(
            unit_guard.current_path.as_ref().unwrap().last(),
            Some(&Coord2D::new(14.25, 2.5))
        );
        assert!(ai.current_path_snapshot.is_some());
    }

    #[test]
    fn final_ground_path_install_updates_goal_layer_like_cpp_do_pathfind() {
        let base_object = Arc::new(RwLock::new(Object::new_test(52, 100.0)));
        {
            let mut object = base_object.write().unwrap();
            object.set_destination_layer(crate::common::PathfindLayerEnum::Top);
        }
        let template = DefaultThingTemplate::new("GroundUnit".to_string());
        let mut unit = Unit::new(Arc::clone(&base_object), &template).unwrap();
        let loco_template = Arc::new(LocomotorTemplate::new_wheeled("GroundLoco".to_string()));
        unit.current_locomotor = Some(Arc::new(Mutex::new(Locomotor::new(loco_template))));
        let unit = Arc::new(RwLock::new(unit));
        let mut ai = UnitAIUpdate::new(
            Arc::downgrade(&unit),
            None,
            None,
            None,
            None,
            None,
            #[cfg(feature = "allow_surrender")]
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        );
        ai.is_final_goal = true;
        ai.requested_destination = Coord3D::new(32.0, 64.0, 0.0);

        ai.set_path_from_coords(&[Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(32.0, 64.0, 0.0)])
            .unwrap();

        assert_eq!(
            base_object.read().unwrap().get_destination_layer(),
            crate::common::PathfindLayerEnum::Ground
        );
        let unit_guard = unit.read().unwrap();
        let target = unit_guard.target_position.unwrap();
        assert_eq!((target.x, target.y), (32.0, 64.0));
    }

    fn save_unit_ai_update(ai: &mut UnitAIUpdate) -> Vec<u8> {
        let mut bytes = Vec::new();
        {
            let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);
            ai.xfer_ai_update_state(&mut xfer).unwrap();
        }
        bytes
    }

    #[test]
    fn unit_ai_update_blocked_speed_uses_cur_max_before_bump_decay() {
        let mut ai = unit_ai_update_without_unit();
        ai.cur_max_blocked_speed = 10.0;
        ai.bump_speed_limit = FAST_AS_POSSIBLE;
        ai.blocked_frames = 3;

        let speed = ai.apply_bump_speed_limit(25.0, true);

        assert!((speed - 9.5).abs() < 0.001);
        assert!((ai.bump_speed_limit - 9.5).abs() < 0.001);
        assert_eq!(ai.blocked_frames, 3);
    }

    #[test]
    fn unit_ai_update_bump_limit_recovers_and_caps_blocked_frames_when_unblocked() {
        let mut ai = unit_ai_update_without_unit();
        ai.bump_speed_limit = 10.0;
        ai.blocked_frames = 4;

        let speed = ai.apply_bump_speed_limit(20.0, false);

        assert!((speed - 10.5).abs() < 0.001);
        assert!((ai.bump_speed_limit - 10.5).abs() < 0.001);
        assert_eq!(ai.blocked_frames, 1);
    }

    #[test]
    fn unit_ai_update_cur_max_blocked_speed_defaults_to_fast_as_possible() {
        let ai = unit_ai_update_without_unit();

        assert_eq!(ai.get_cur_max_blocked_speed(), FAST_AS_POSSIBLE);
    }

    #[test]
    fn unit_ai_update_rejects_path_requests_without_valid_locomotor_surfaces() {
        let mut ai = unit_ai_update_without_unit();
        let destination = Coord3D::new(10.0, 20.0, 0.0);

        assert_eq!(
            ai.request_path(&destination, true).unwrap_err(),
            "Attempting to path immobile unit"
        );
        assert_eq!(
            ai.request_attack_path(INVALID_ID, &destination)
                .unwrap_err(),
            "Attempting to path immobile unit"
        );
        assert_eq!(
            ai.request_approach_path(&destination).unwrap_err(),
            "Attempting to path immobile unit"
        );
    }

    #[test]
    fn unit_ai_update_safe_path_distance_matches_cpp_inputs() {
        assert!((UnitAIUpdate::safe_path_search_distance(120.0, 35.0) - 155.0).abs() < 0.001);
    }

    #[test]
    fn unit_ai_update_xfer_serializes_turret_ai_snapshots_before_sync_flag() {
        let mut without_turret = unit_ai_update_without_unit();
        let without_turret_bytes = save_unit_ai_update(&mut without_turret);

        let mut with_primary = unit_ai_update_without_unit();
        with_primary.turret_primary_machine = Some(test_turret_machine());
        let with_primary_bytes = save_unit_ai_update(&mut with_primary);

        let mut with_both = unit_ai_update_without_unit();
        with_both.turret_primary_machine = Some(test_turret_machine());
        with_both.turret_secondary_machine = Some(test_turret_machine());
        let with_both_bytes = save_unit_ai_update(&mut with_both);

        assert!(with_primary_bytes.len() > without_turret_bytes.len());
        assert!(with_both_bytes.len() > with_primary_bytes.len());
        assert_eq!(
            with_primary_bytes.len() - without_turret_bytes.len(),
            with_both_bytes.len() - with_primary_bytes.len()
        );
    }

    #[test]
    fn unit_ai_update_xfer_roundtrips_next_enemy_scan_time() {
        let mut saved = unit_ai_update_without_unit();
        saved.next_enemy_scan_time = 12_345;
        let bytes = save_unit_ai_update(&mut saved);

        let mut loaded = unit_ai_update_without_unit();
        {
            let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
            loaded.xfer_ai_update_state(&mut xfer).unwrap();
        }

        assert_eq!(loaded.next_enemy_scan_time, 12_345);
    }

    #[test]
    fn unit_ai_update_guard_target_slots_match_cpp_shift_semantics() {
        let mut ai = unit_ai_update_without_unit();

        ai.push_guard_target_type(GuardTargetType::Location);
        ai.push_guard_target_type(GuardTargetType::Object);
        ai.clear_guard_target_type();

        assert_eq!(ai.guard_target_type[0], GuardTargetType::None_);
        assert_eq!(ai.guard_target_type[1], GuardTargetType::Object);
    }

    #[test]
    fn unit_ai_update_xfer_roundtrips_guard_target_slots() {
        let mut saved = unit_ai_update_without_unit();
        saved.push_guard_target_type(GuardTargetType::Location);
        saved.location_to_guard = Coord3D::new(11.0, 22.0, 3.0);
        saved.push_guard_target_type(GuardTargetType::Object);
        saved.object_to_guard = 91;
        let bytes = save_unit_ai_update(&mut saved);

        let mut loaded = unit_ai_update_without_unit();
        {
            let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
            loaded.xfer_ai_update_state(&mut xfer).unwrap();
        }

        assert_eq!(loaded.guard_target_type[0], GuardTargetType::Object);
        assert_eq!(loaded.guard_target_type[1], GuardTargetType::Location);
        assert_eq!(loaded.location_to_guard, Coord3D::new(11.0, 22.0, 3.0));
        assert_eq!(loaded.object_to_guard, 91);
    }

    #[test]
    fn unit_ai_update_xfer_roundtrips_requested_path_and_locomotor_slots() {
        let mut saved = unit_ai_update_without_unit();
        saved.requested_victim_id = 77;
        saved.requested_destination = Coord3D::new(10.0, 20.0, 3.0);
        saved.requested_destination2 = Coord3D::new(30.0, 40.0, 5.0);
        saved.pathfind_goal_cell = ICoord2D::new(11, 12);
        saved.pathfind_cur_cell = ICoord2D::new(13, 14);
        saved.final_position = Coord3D::new(50.0, 60.0, 7.0);
        saved.do_final_position = true;
        saved.is_attack_path = true;
        saved.is_final_goal = true;
        saved.is_approach_path = true;
        saved.is_safe_path = true;
        saved.movement_complete = true;
        saved.current_locomotor_set = LocomotorSetType::Supersonic;
        saved.locomotor_goal_type = 2;
        saved.locomotor_goal_data = Coord3D::new(70.0, 80.0, 9.0);
        let bytes = save_unit_ai_update(&mut saved);

        let mut loaded = unit_ai_update_without_unit();
        {
            let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
            loaded.xfer_ai_update_state(&mut xfer).unwrap();
        }

        assert_eq!(loaded.requested_victim_id, 77);
        assert_eq!(loaded.requested_destination, Coord3D::new(10.0, 20.0, 3.0));
        assert_eq!(loaded.requested_destination2, Coord3D::new(30.0, 40.0, 5.0));
        assert_eq!(loaded.pathfind_goal_cell, ICoord2D::new(11, 12));
        assert_eq!(loaded.pathfind_cur_cell, ICoord2D::new(13, 14));
        assert_eq!(loaded.final_position, Coord3D::new(50.0, 60.0, 7.0));
        assert!(loaded.do_final_position);
        assert!(loaded.is_attack_path);
        assert!(loaded.is_final_goal);
        assert!(loaded.is_approach_path);
        assert!(loaded.is_safe_path);
        assert!(loaded.movement_complete);
        assert_eq!(loaded.current_locomotor_set, LocomotorSetType::Supersonic);
        assert_eq!(loaded.locomotor_goal_type, 2);
        assert_eq!(loaded.locomotor_goal_data, Coord3D::new(70.0, 80.0, 9.0));
    }

    #[test]
    fn unit_ai_update_rejects_invalid_locomotor_set_type() {
        assert_eq!(
            locomotor_set_type_from_i32(8).unwrap_err(),
            "Invalid AIUpdate locomotor set type 8"
        );
    }

    #[test]
    fn unit_ai_update_xfer_roundtrips_current_path_snapshot() {
        let mut saved = unit_ai_update_without_unit();
        saved.set_current_path_snapshot_from_coords(&[
            Coord3D::new(1.0, 2.0, 3.0),
            Coord3D::new(4.0, 5.0, 6.0),
        ]);
        let bytes = save_unit_ai_update(&mut saved);

        let mut loaded = unit_ai_update_without_unit();
        {
            let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
            loaded.xfer_ai_update_state(&mut xfer).unwrap();
        }

        let path = loaded.current_path_snapshot.as_ref().unwrap();
        assert_eq!(
            *path.get_first_node().unwrap().get_position(),
            Coord3D::new(1.0, 2.0, 3.0)
        );
    }

    #[test]
    fn unit_ai_update_xfer_roundtrips_planning_waypoint_queue() {
        let mut saved = unit_ai_update_without_unit();
        saved.queue_waypoint(&Coord3D::new(1.0, 2.0, 3.0));
        saved.queue_waypoint(&Coord3D::new(4.0, 5.0, 6.0));
        saved.execute_waypoint_queue();
        let bytes = save_unit_ai_update(&mut saved);

        let mut loaded = unit_ai_update_without_unit();
        {
            let mut xfer = XferLoad::new(Cursor::new(bytes), 1);
            loaded.xfer_ai_update_state(&mut xfer).unwrap();
        }

        assert_eq!(loaded.planning_waypoint_count, 2);
        assert_eq!(loaded.planning_waypoint_index, 0);
        assert!(loaded.executing_waypoint_queue);
        assert_eq!(
            loaded.planning_waypoint_queue[0],
            Coord3D::new(1.0, 2.0, 3.0)
        );
        assert_eq!(
            loaded.planning_waypoint_queue[1],
            Coord3D::new(4.0, 5.0, 6.0)
        );
    }

    #[test]
    fn unit_ai_update_xfer_rejects_invalid_planning_waypoint_count() {
        let mut ai = unit_ai_update_without_unit();
        ai.planning_waypoint_count = AI_UPDATE_MAX_WAYPOINTS as Int + 1;
        let mut bytes = Vec::new();
        let mut xfer = XferSave::new(Cursor::new(&mut bytes), 1);

        let err = ai.xfer_ai_update_state(&mut xfer).unwrap_err();

        assert!(err.contains("Invalid AIUpdate waypoint count"));
    }
}

// This would need to be implemented for the actual Object type
// impl UnitExt for Object {
//     fn as_unit(&self) -> Option<&Unit> {
//         // Implementation would check if this object is actually a unit
//         None
//     }
//
//     fn as_unit_mut(&mut self) -> Option<&mut Unit> {
//         // Implementation would check if this object is actually a unit
//         None
//     }
// }
