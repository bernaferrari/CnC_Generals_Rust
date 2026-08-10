//! Unit construct, identity, occupants, and Object extension trait.

#![allow(unused_imports)]

use super::imports::*;
use super::types::*;

pub struct Unit {
    /// Base object id (resolve for the duration of an op)
    pub(super) object_id: ObjectID,

    /// Movement and pathfinding
    pub(super) locomotor_set: LocomotorSet,
    pub(super) current_locomotor: Option<Arc<Mutex<Locomotor>>>,
    pub(super) movement_state: MovementState,
    pub(super) target_position: Option<Coord3D>,
    pub(super) waypoint_queue: Vec<Waypoint>,
    pub(super) current_path: Option<Vec<Coord2D>>,
    pub(super) path_index: usize,
    pub(super) path_following_state: Option<PathFollowingState>,
    pub(super) path_extra_distance: Real,
    pub(super) path_adjusts_destination: bool,
    pub(super) movement_speed_multiplier: Real,
    pub(super) current_speed: f32,
    pub(super) attack_move_active: bool,
    pub(super) last_target_scan_frame: u32,
    pub(super) attack_move_resume_frame: u32,
    pub(super) attack_target_lock_until: u32,
    pub(super) mood_attack_check_rate_frames: u32,

    /// Formation and group behavior
    #[allow(dead_code)]
    pub(super) formation_type: FormationType,
    pub(super) formation_position: usize,
    pub(super) group_leader: Option<ObjectID>,
    pub(super) group_members: Vec<ObjectID>,
    pub(super) follow_target: Option<ObjectID>,
    pub(super) follow_distance: Real,

    /// Combat behavior
    pub(super) combat_mode: CombatMode,
    pub(super) attack_target: Option<ObjectID>,
    pub(super) attack_position: Option<Coord3D>,
    pub(super) engagement_range: Real,
    pub(super) retreat_threshold: Real,
    pub(super) patrol_points: Vec<Coord3D>,
    pub(super) current_patrol_index: usize,
    pub(super) patrol_loop: bool,
    pub(super) guard_position: Option<Coord3D>,
    pub(super) guard_radius: Real,

    /// Movement constraints
    pub(super) can_cross_bridges: bool,
    pub(super) can_swim: bool,
    pub(super) can_fly: bool,
    pub(super) preferred_terrain: TerrainType,
    pub(super) movement_penalty_modifiers: HashMap<TerrainType, Real>,

    /// Orders and commands
    pub(super) current_order: Option<UnitOrder>,
    pub(super) order_queue: Vec<UnitOrder>,
    pub(super) auto_acquire_enemies: bool,
    pub(super) auto_acquire_attack_buildings: bool,
    pub(super) auto_acquire_while_stealthed: bool,
    pub(super) auto_acquire_not_while_attacking: bool,
    pub(super) return_to_formation: bool,

    /// Morale and psychology
    pub(super) morale_level: Real,
    pub(super) fear_level: Real,
    pub(super) panic_threshold: Real,
    pub(super) bravery_modifier: Real,

    /// Status effects
    pub(super) is_stunned: bool,
    pub(super) is_suppressed: bool,
    pub(super) is_pinned: bool,
    pub(super) is_routing: bool,
    pub(super) is_garrisoned: bool,
    pub(super) garrison_building: Option<ObjectID>,

    /// Transport capabilities (for vehicles that can carry troops)
    pub(super) transport_capacity: usize,
    pub(super) transported_units: Vec<ObjectID>,
    pub(super) can_amphibious_unload: bool,

    /// Special abilities
    pub(super) can_capture_buildings: bool,
    pub(super) can_sabotage: bool,
    pub(super) can_hack: bool,
    pub(super) stealth_detection_range: Real,

    /// Animation and visual state
    pub(super) current_animation: AsciiString,
    pub(super) animation_state: ModelConditionFlags,
    pub(super) facing_direction: Real,
    pub(super) desired_facing: Real,
    pub(super) turn_rate: Real,
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
            object_id: {
                let id = base_object
                    .read()
                    .ok()
                    .map(|g| g.get_id())
                    .unwrap_or(INVALID_ID);
                if id != INVALID_ID {
                    crate::object::registry::OBJECT_REGISTRY.register_object(id, &base_object);
                    crate::ai::object_registry::register_legacy_object(&base_object);
                }
                id
            },
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
    pub fn base_object(&self) -> Option<Arc<RwLock<Object>>> {
        self.get_base_object()
    }
    pub fn object_id(&self) -> ObjectID {
        self.object_id
    }
    pub(super) fn get_base_object(&self) -> Option<Arc<RwLock<Object>>> {
        if self.object_id == INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
            .or_else(|| crate::ai::object_registry::get_legacy_object(self.object_id))
    }
    pub(super) fn base_arc(&self) -> Arc<RwLock<Object>> {
        self.get_base_object()
            .expect("Unit base object unavailable — register via Unit::new / OBJECT_REGISTRY")
    }
    pub fn get_id(&self) -> ObjectID {
        self.object_id
    }
    pub fn get_orientation(&self) -> Real {
        self.base_arc()
            .read()
            .ok()
            .map(|guard| guard.get_orientation())
            .unwrap_or(0.0)
    }
    pub fn set_orientation(&mut self, angle: Real) -> Result<(), String> {
        let base = self.base_arc();
        let Ok(mut guard) = base.write() else {
            return Err("Unit base object lock poisoned".to_string());
        };
        guard.set_orientation(angle)
    }
    pub fn get_unit_direction_vector_2d(&self) -> (f32, f32) {
        self.base_arc()
            .read()
            .ok()
            .map(|guard| guard.get_unit_direction_vector_2d())
            .unwrap_or((1.0, 0.0))
    }
    pub fn get_ai_update_interface(&self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>> {
        self.base_arc()
            .read()
            .ok()
            .and_then(|guard| guard.get_ai_update_interface())
    }
    pub(crate) fn forward_command_to_flight_deck(&self, params: &crate::ai::AiCommandParams) {
        if let Ok(guard) = self.base_arc().read() {
            guard.forward_command_to_flight_deck(params);
        }
    }
}

/// Extension trait for Object to provide Unit-specific functionality
pub trait UnitExt {
    /// Get unit-specific data if this object is a unit
    fn as_unit(&self) -> Option<&Unit>;
    fn as_unit_mut(&mut self) -> Option<&mut Unit>;
}
