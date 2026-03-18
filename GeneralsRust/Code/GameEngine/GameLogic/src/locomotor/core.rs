//! Locomotor system - Unit movement and pathfinding
//!
//! This module provides the complete locomotor system for unit movement,
//! matching the C++ Locomotor implementation from Locomotor.h
//!
//! Supports all 9 locomotor types with full terrain interaction,
//! physics integration, and pathfinding capabilities.

use crate::ai::pathfinding_system::{MovementCapabilities, PathfindLayerEnum};
use crate::common::*;
use crate::helpers::TheTerrainLogic;
use crate::object::registry::OBJECT_REGISTRY;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::{PhysicsState, PhysicsType};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::error::Error;
use std::sync::{Arc, Mutex, RwLock};

// ============================================================================
// ENUMS AND CONSTANTS
// ============================================================================

/// Logic frames per second matching C++ TheGlobalData::m_framesPerSecond
const LOGICFRAMES_PER_SECOND: u32 = 30;

/// Donut timer delay in seconds. Matches C++ Locomotor.cpp:31 DONUT_TIME_DELAY_SECONDS
const DONUT_TIME_DELAY_SECONDS: Real = 2.5;

/// Donut distance threshold. Matches C++ Locomotor.cpp:32 DONUT_DISTANCE
const DONUT_DISTANCE: Real = 4.0 * PATHFIND_CELL_SIZE_F;

/// Maximum braking factor clamp. Matches C++ Locomotor.cpp:35 MAX_BRAKING_FACTOR
const MAX_BRAKING_FACTOR: Real = 5.0;

/// Locomotor surface type mask - bitmask for allowed terrain types
pub type LocomotorSurfaceTypeMask = u32;

// Surface type constants matching C++ implementation
pub const SURFACE_GROUND: u32 = 0x01;
pub const SURFACE_WATER: u32 = 0x02;
pub const SURFACE_CLIFF: u32 = 0x04;
pub const SURFACE_AIR: u32 = 0x08;
pub const SURFACE_RUBBLE: u32 = 0x10;

/// Locomotor appearance/type - matches C++ LocomotorAppearance (Locomotor.h)
///
/// C++ enum has exactly 9 values: LOCO_LEGS_TWO, LOCO_WHEELS_FOUR, LOCO_TREADS,
/// LOCO_HOVER, LOCO_THRUST, LOCO_WINGS, LOCO_CLIMBER, LOCO_OTHER, LOCO_MOTORCYCLE.
/// Naval/tunnel behavior is determined by surface masks and physics type, not appearance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocomotorAppearance {
    /// Two-legged infantry (C++ LOCO_LEGS_TWO / "TWO_LEGS")
    TwoLegs,
    /// Four-wheeled vehicles (C++ LOCO_WHEELS_FOUR / "FOUR_WHEELS")
    FourWheels,
    /// Tracked vehicles (C++ LOCO_TREADS / "TREADS")
    Treads,
    /// Hovering units (C++ LOCO_HOVER / "HOVER")
    Hover,
    /// Thrust-based / helicopters (C++ LOCO_THRUST / "THRUST")
    Thrust,
    /// Fixed-wing aircraft (C++ LOCO_WINGS / "WINGS")
    Wings,
    /// Cliff climbers (C++ LOCO_CLIMBER / "CLIMBER")
    Climber,
    /// Motorcycle (C++ LOCO_MOTORCYCLE / "MOTORCYCLE")
    Motorcycle,
    /// Other / default (C++ LOCO_OTHER / "OTHER")
    Other,
}

/// Locomotor priority for group movement
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocomotorPriority {
    /// Moves to back of group
    Back = 0,
    /// Stays in middle of group
    Middle = 1,
    /// Moves to front of group
    Front = 2,
}

/// Z-axis behavior - matches C++ LocomotorBehaviorZ
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocomotorBehaviorZ {
    /// No Z-axis motive force
    NoZMotiveForce,
    /// Maintain sea level
    SeaLevel,
    /// Follow surface-relative height
    SurfaceRelativeHeight,
    /// Follow absolute height
    AbsoluteHeight,
    /// Fixed surface-relative height
    FixedSurfaceRelativeHeight,
    /// Fixed absolute height
    FixedAbsoluteHeight,
    /// Relative to ground and buildings
    RelativeToGroundAndBuildings,
    /// Smooth relative to highest layer
    SmoothRelativeToHighestLayer,
}

/// Body damage type affecting locomotor performance
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyDamageType {
    Pristine,
    Damaged,
    ReallyDamaged,
    Rubble,
}

// ============================================================================
// LOCOMOTOR TEMPLATE
// ============================================================================

/// Locomotor template - defines movement characteristics
/// Matches C++ LocomotorTemplate
#[derive(Debug, Clone)]
pub struct LocomotorTemplate {
    /// Template name
    pub name: String,

    /// Legal surface types (bitmask)
    pub surfaces: LocomotorSurfaceTypeMask,

    /// Maximum speed (dist/frame)
    pub max_speed: Real,
    /// Maximum speed when damaged
    pub max_speed_damaged: Real,
    /// Minimum speed (for slowing down)
    pub min_speed: Real,

    /// Maximum turn rate (radians/frame)
    pub max_turn_rate: Real,
    /// Maximum turn rate when damaged
    pub max_turn_rate_damaged: Real,

    /// Acceleration (dist/frame^2)
    pub acceleration: Real,
    /// Acceleration when damaged
    pub acceleration_damaged: Real,

    /// Lift force (for aircraft)
    pub lift: Real,
    /// Lift when damaged
    pub lift_damaged: Real,

    /// Braking deceleration
    pub braking: Real,

    /// Minimum speed required to turn
    pub min_turn_speed: Real,

    /// Preferred flight height
    pub preferred_height: Real,
    /// Height damping factor (1.0 = aggressive, 0.1 = gradual)
    pub preferred_height_damping: Real,

    /// Circling radius for aircraft (0 = smallest possible)
    pub circling_radius: Real,

    /// Altitude change threshold for circling behavior.
    /// When > 0 and the Z delta to goal exceeds this, the aircraft circles
    /// to gain/lose altitude before resuming course.
    /// Matches C++ Locomotor::m_circleThresh (CIRCLE_FOR_LANDING, disabled by default).
    pub circle_thresh: Real,

    /// Maximum Z-axis speed
    pub speed_limit_z: Real,

    /// Extra 2D friction
    pub extra_2d_friction: Real,

    /// Maximum thrust angle (THRUST locos only)
    pub max_thrust_angle: Real,

    /// Z-axis behavior
    pub behavior_z: LocomotorBehaviorZ,

    /// Visual appearance/type
    pub appearance: LocomotorAppearance,

    /// Group movement priority
    pub move_priority: LocomotorPriority,

    // Suspension and pitch/roll parameters
    pub accel_pitch_limit: Real,
    pub decel_pitch_limit: Real,
    pub bounce_kick: Real,
    pub pitch_stiffness: Real,
    pub roll_stiffness: Real,
    pub pitch_damping: Real,
    pub roll_damping: Real,
    pub pitch_by_z_vel_coef: Real,
    pub thrust_roll: Real,
    pub wobble_rate: Real,
    pub min_wobble: Real,
    pub max_wobble: Real,
    pub forward_vel_coef: Real,
    pub lateral_vel_coef: Real,
    pub forward_accel_coef: Real,
    pub lateral_accel_coef: Real,
    pub uniform_axial_damping: Real,
    pub turn_pivot_offset: Real,

    /// Height at which unit becomes airborne target
    pub airborne_targeting_height: Int,

    /// Close enough distance to destination
    pub close_enough_dist: Real,
    /// Is close enough distance 3D?
    pub is_close_enough_dist_3d: Bool,

    /// Ultra-accurate slide factor
    pub ultra_accurate_slide_factor: Real,

    // Boolean flags
    pub locomotor_works_when_dead: Bool,
    pub allow_motive_force_while_airborne: Bool,
    pub apply_2d_friction_when_airborne: Bool,
    pub downhill_only: Bool,
    pub stick_to_ground: Bool,
    pub can_move_backward: Bool,

    // Suspension parameters
    pub has_suspension: Bool,
    pub maximum_wheel_extension: Real,
    pub maximum_wheel_compression: Real,
    pub wheel_turn_angle: Real,

    // Wander parameters
    pub wander_width_factor: Real,
    pub wander_length_factor: Real,
    pub wander_about_point_radius: Real,

    // Flight control parameters
    pub rudder_correction_degree: Real,
    pub rudder_correction_rate: Real,
    pub elevator_correction_degree: Real,
    pub elevator_correction_rate: Real,
}

impl LocomotorTemplate {
    /// Create new locomotor template with defaults
    pub fn new(name: String) -> Self {
        Self {
            name,
            surfaces: SURFACE_GROUND,
            max_speed: 10.0,
            max_speed_damaged: 5.0,
            min_speed: 0.0,
            max_turn_rate: 0.1,
            max_turn_rate_damaged: 0.05,
            acceleration: 2.0,
            acceleration_damaged: 1.0,
            lift: 0.0,
            lift_damaged: 0.0,
            braking: 3.0,
            min_turn_speed: 0.0,
            preferred_height: 0.0,
            preferred_height_damping: 0.5,
            circling_radius: 0.0,
            circle_thresh: 0.0,
            speed_limit_z: 5.0,
            extra_2d_friction: 0.0,
            max_thrust_angle: 0.0,
            behavior_z: LocomotorBehaviorZ::NoZMotiveForce,
            appearance: LocomotorAppearance::Other,
            move_priority: LocomotorPriority::Middle,
            accel_pitch_limit: 0.0,
            decel_pitch_limit: 0.0,
            bounce_kick: 0.0,
            pitch_stiffness: 0.0,
            roll_stiffness: 0.0,
            pitch_damping: 0.0,
            roll_damping: 0.0,
            pitch_by_z_vel_coef: 0.0,
            thrust_roll: 0.0,
            wobble_rate: 0.0,
            min_wobble: 0.0,
            max_wobble: 0.0,
            forward_vel_coef: 0.0,
            lateral_vel_coef: 0.0,
            forward_accel_coef: 0.0,
            lateral_accel_coef: 0.0,
            uniform_axial_damping: 0.0,
            turn_pivot_offset: 0.0,
            airborne_targeting_height: 0,
            close_enough_dist: 5.0,
            is_close_enough_dist_3d: false,
            ultra_accurate_slide_factor: 1.0,
            locomotor_works_when_dead: false,
            allow_motive_force_while_airborne: false,
            apply_2d_friction_when_airborne: false,
            downhill_only: false,
            stick_to_ground: false,
            can_move_backward: false,
            has_suspension: false,
            maximum_wheel_extension: 0.0,
            maximum_wheel_compression: 0.0,
            wheel_turn_angle: 0.0,
            wander_width_factor: 0.0,
            wander_length_factor: 0.0,
            wander_about_point_radius: 0.0,
            rudder_correction_degree: 0.0,
            rudder_correction_rate: 0.0,
            elevator_correction_degree: 0.0,
            elevator_correction_rate: 0.0,
        }
    }

    /// Create infantry locomotor template
    pub fn new_infantry(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::TwoLegs;
        template.surfaces = SURFACE_GROUND;
        template.max_speed = 8.0;
        template.max_speed_damaged = 4.0;
        template.acceleration = 3.0;
        template.max_turn_rate = 0.15;
        template.braking = 4.0;
        template.stick_to_ground = true;
        template.can_move_backward = true;
        template.close_enough_dist = 3.0;
        template
    }

    /// Create wheeled vehicle template
    pub fn new_wheeled(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::FourWheels;
        template.surfaces = SURFACE_GROUND;
        template.max_speed = 15.0;
        template.max_speed_damaged = 8.0;
        template.acceleration = 5.0;
        template.max_turn_rate = 0.08;
        template.braking = 6.0;
        template.stick_to_ground = true;
        template.has_suspension = true;
        template.can_move_backward = true;
        template.close_enough_dist = 5.0;
        template
    }

    /// Create tracked vehicle template
    pub fn new_tracked(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Treads;
        template.surfaces = SURFACE_GROUND | SURFACE_RUBBLE;
        template.max_speed = 12.0;
        template.max_speed_damaged = 7.0;
        template.acceleration = 4.0;
        template.max_turn_rate = 0.1;
        template.braking = 5.0;
        template.stick_to_ground = true;
        template.can_move_backward = true;
        template.close_enough_dist = 5.0;
        template
    }

    /// Create hover vehicle template
    pub fn new_hover(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Hover;
        template.surfaces = SURFACE_GROUND | SURFACE_WATER;
        template.max_speed = 14.0;
        template.max_speed_damaged = 9.0;
        template.acceleration = 4.5;
        template.max_turn_rate = 0.12;
        template.braking = 5.5;
        template.preferred_height = 3.0;
        template.preferred_height_damping = 0.8;
        template.behavior_z = LocomotorBehaviorZ::SurfaceRelativeHeight;
        template.allow_motive_force_while_airborne = true;
        template.close_enough_dist = 5.0;
        template
    }

    /// Create thrust aircraft template (helicopters)
    pub fn new_thrust(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Thrust;
        template.surfaces = SURFACE_AIR;
        template.max_speed = 20.0;
        template.max_speed_damaged = 12.0;
        template.acceleration = 3.0;
        template.lift = 15.0;
        template.max_turn_rate = 0.1;
        template.braking = 4.0;
        template.preferred_height = 50.0;
        template.preferred_height_damping = 0.5;
        template.behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
        template.allow_motive_force_while_airborne = true;
        template.airborne_targeting_height = 25;
        template.close_enough_dist = 10.0;
        template
    }

    /// Create fixed-wing aircraft template
    pub fn new_wings(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Wings;
        template.surfaces = SURFACE_AIR;
        template.max_speed = 35.0;
        template.max_speed_damaged = 20.0;
        template.acceleration = 2.0;
        template.lift = 20.0;
        template.max_turn_rate = 0.05;
        template.braking = 2.0;
        template.min_turn_speed = 10.0;
        template.preferred_height = 80.0;
        template.preferred_height_damping = 0.3;
        template.circling_radius = 50.0;
        template.behavior_z = LocomotorBehaviorZ::AbsoluteHeight;
        template.allow_motive_force_while_airborne = true;
        template.airborne_targeting_height = 40;
        template.close_enough_dist = 15.0;
        template
    }

    /// Create climber template
    pub fn new_climber(name: String) -> Self {
        let mut template = Self::new(name);
        template.appearance = LocomotorAppearance::Climber;
        template.surfaces = SURFACE_GROUND | SURFACE_CLIFF;
        template.max_speed = 6.0;
        template.max_speed_damaged = 3.0;
        template.acceleration = 2.5;
        template.max_turn_rate = 0.12;
        template.braking = 3.5;
        template.stick_to_ground = true;
        template.can_move_backward = true;
        template.close_enough_dist = 3.0;
        template
    }

}

// ============================================================================
// LOCOMOTOR INSTANCE
// ============================================================================

/// Active path being followed
#[derive(Debug, Clone)]
pub struct ActivePath {
    /// Full path waypoints
    pub waypoints: Vec<Coord3D>,
    /// Layer per waypoint
    pub layers: Vec<PathfindLayerEnum>,
    /// Current waypoint index
    pub current_waypoint: usize,
    /// Distance remaining to current waypoint
    pub distance_to_waypoint: Real,
    /// Total path distance
    pub total_distance: Real,
    /// Distance traveled so far
    pub distance_traveled: Real,
    /// Path start frame
    pub start_frame: u32,
}

impl ActivePath {
    /// Create new active path
    pub fn new(waypoints: Vec<Coord3D>, start_frame: u32) -> Self {
        let layers = vec![PathfindLayerEnum::Ground; waypoints.len()];
        Self::new_with_layers(waypoints, layers, start_frame)
    }

    /// Create new active path with explicit layers per waypoint.
    pub fn new_with_layers(
        waypoints: Vec<Coord3D>,
        layers: Vec<PathfindLayerEnum>,
        start_frame: u32,
    ) -> Self {
        let total_distance = Self::calculate_path_distance(&waypoints);
        let distance_to_waypoint = if waypoints.len() >= 2 {
            (waypoints[1] - waypoints[0]).length()
        } else {
            0.0
        };

        let mut layers = layers;
        if layers.len() != waypoints.len() {
            layers.resize(waypoints.len(), PathfindLayerEnum::Ground);
        }

        Self {
            waypoints,
            layers,
            current_waypoint: 0,
            distance_to_waypoint,
            total_distance,
            distance_traveled: 0.0,
            start_frame,
        }
    }

    /// Calculate total path distance
    fn calculate_path_distance(waypoints: &[Coord3D]) -> Real {
        let mut total = 0.0;
        for i in 1..waypoints.len() {
            total += (waypoints[i] - waypoints[i - 1]).length();
        }
        total
    }

    /// Get current target waypoint
    pub fn current_target(&self) -> Option<Coord3D> {
        if self.current_waypoint < self.waypoints.len() {
            Some(self.waypoints[self.current_waypoint])
        } else {
            None
        }
    }

    pub fn current_layer(&self) -> Option<PathfindLayerEnum> {
        if self.current_waypoint < self.layers.len() {
            Some(self.layers[self.current_waypoint])
        } else {
            None
        }
    }

    /// Get next waypoint after current
    pub fn next_waypoint(&self) -> Option<Coord3D> {
        if self.current_waypoint + 1 < self.waypoints.len() {
            Some(self.waypoints[self.current_waypoint + 1])
        } else {
            None
        }
    }

    /// Advance to next waypoint
    pub fn advance_waypoint(&mut self) -> bool {
        if self.current_waypoint + 1 < self.waypoints.len() {
            self.distance_traveled += self.distance_to_waypoint;
            self.current_waypoint += 1;

            if self.current_waypoint + 1 < self.waypoints.len() {
                self.distance_to_waypoint = (self.waypoints[self.current_waypoint + 1]
                    - self.waypoints[self.current_waypoint])
                    .length();
            } else {
                self.distance_to_waypoint = 0.0;
            }
            true
        } else {
            false
        }
    }

    /// Get distance remaining on path
    pub fn distance_remaining(&self) -> Real {
        self.total_distance - self.distance_traveled - self.distance_to_waypoint
    }

    /// Check if path is complete
    pub fn is_complete(&self) -> bool {
        self.current_waypoint + 1 >= self.waypoints.len()
    }

    /// Get number of waypoints
    pub fn waypoint_count(&self) -> usize {
        self.waypoints.len()
    }

    /// Append a waypoint to the active path and update distance totals.
    pub fn append_waypoint(&mut self, waypoint: Coord3D) {
        if let Some(last) = self.waypoints.last().copied() {
            let delta = (waypoint - last).length();
            self.total_distance += delta;
            if self.current_waypoint + 1 >= self.waypoints.len() {
                self.distance_to_waypoint = delta;
            }
        } else {
            self.total_distance = 0.0;
            self.distance_to_waypoint = 0.0;
        }
        self.waypoints.push(waypoint);
        self.layers.push(PathfindLayerEnum::Ground);
    }

    /// Update the last waypoint and recompute path distance.
    pub fn set_last_waypoint(&mut self, waypoint: Coord3D) {
        if let Some(last) = self.waypoints.last_mut() {
            *last = waypoint;
            self.total_distance = Self::calculate_path_distance(&self.waypoints);
            if self.current_waypoint + 1 < self.waypoints.len() {
                self.distance_to_waypoint = (self.waypoints[self.current_waypoint + 1]
                    - self.waypoints[self.current_waypoint])
                    .length();
            } else {
                self.distance_to_waypoint = 0.0;
            }
        }
    }
}

/// Locomotor instance - runtime state for a unit's locomotor
#[derive(Debug, Clone)]
pub struct Locomotor {
    /// Reference to template
    pub template: Arc<LocomotorTemplate>,

    /// Current maximum speed (can be modified by upgrades)
    max_speed: Real,
    /// Current maximum turn rate
    max_turn_rate: Real,
    /// Current maximum acceleration
    max_accel: Real,
    /// Current maximum lift
    max_lift: Real,
    /// Current maximum braking
    max_braking: Real,

    /// Current preferred height (can be modified)
    pub preferred_height: Real,
    /// Preferred height damping
    pub preferred_height_damping: Real,

    /// Close enough distance (can be modified)
    close_enough_dist: Real,

    /// Braking factor for smooth deceleration
    braking_factor: Real,

    /// Wander angle offset for infantry
    angle_offset: Real,
    /// Wander offset increment
    offset_increment: Real,

    /// Active path being followed
    pub active_path: Option<ActivePath>,

    /// Last obstacle detection time
    last_obstacle_check: u32,

    /// Donut timer frame for wheels braking near destination
    /// Matches C++ Locomotor::m_donutTimer
    donut_timer: u32,

    /// Flags
    flags: u32,
}

// Locomotor flags - Matches C++ Locomotor.h:395-407
const FLAG_IS_BRAKING: u32 = 0x01;
const FLAG_ALLOW_INVALID_POS: u32 = 0x02;
const FLAG_MAINTAIN_POS_VALID: u32 = 0x04;
const FLAG_PRECISE_Z_POS: u32 = 0x08;
const FLAG_NO_SLOW_DOWN: u32 = 0x10;
const FLAG_ULTRA_ACCURATE: u32 = 0x20;
const FLAG_CLOSE_ENOUGH_3D: u32 = 0x40;
const FLAG_MOVING_BACKWARDS: u32 = 0x80;
const FLAG_DOING_THREE_POINT_TURN: u32 = 0x100;
const FLAG_CLIMBING: u32 = 0x200;
const FLAG_OVER_WATER: u32 = 0x400;
const FLAG_OFFSET_INCREASING: u32 = 0x800;
const FLAG_SLIDING_INTO_PLACE: u32 = 0x1000;

impl Locomotor {
    /// Create new locomotor from template
    /// Matches C++ Locomotor.cpp:629-651
    pub fn new(template: Arc<LocomotorTemplate>) -> Self {
        // Random initial wander offset (C++ lines 647-649)
        let angle_offset = (rand::random::<f32>() - 0.5) * (std::f32::consts::PI / 3.0);
        let offset_increment = (std::f32::consts::PI / 40.0)
            * ((rand::random::<f32>() * 0.4 + 0.8) / template.wander_length_factor.max(0.01));

        Self {
            max_speed: template.max_speed,
            max_turn_rate: template.max_turn_rate,
            max_accel: template.acceleration,
            max_lift: template.lift,
            max_braking: template.braking,
            preferred_height: template.preferred_height,
            preferred_height_damping: template.preferred_height_damping,
            close_enough_dist: template.close_enough_dist,
            braking_factor: 1.0,
            angle_offset,
            offset_increment,
            active_path: None,
            last_obstacle_check: 0,
            donut_timer: 0,
            flags: if template.is_close_enough_dist_3d {
                FLAG_CLOSE_ENOUGH_3D
                    | (if rand::random::<bool>() {
                        FLAG_OFFSET_INCREASING
                    } else {
                        0
                    })
            } else {
                if rand::random::<bool>() {
                    FLAG_OFFSET_INCREASING
                } else {
                    0
                }
            },
            template,
        }
    }

    /// Get maximum speed for given damage condition
    pub fn get_max_speed_for_condition(&self, condition: BodyDamageType) -> Real {
        match condition {
            BodyDamageType::Pristine => self.max_speed,
            BodyDamageType::Damaged => self.template.max_speed_damaged,
            BodyDamageType::ReallyDamaged => self.template.max_speed_damaged * 0.5,
            BodyDamageType::Rubble => 0.0,
        }
    }

    /// Get maximum turn rate for given damage condition
    pub fn get_max_turn_rate(&self, condition: BodyDamageType) -> Real {
        match condition {
            BodyDamageType::Pristine => self.max_turn_rate,
            BodyDamageType::Damaged => self.template.max_turn_rate_damaged,
            BodyDamageType::ReallyDamaged => self.template.max_turn_rate_damaged * 0.5,
            BodyDamageType::Rubble => 0.0,
        }
    }

    /// Get maximum acceleration for given damage condition
    pub fn get_max_acceleration(&self, condition: BodyDamageType) -> Real {
        match condition {
            BodyDamageType::Pristine => self.max_accel,
            BodyDamageType::Damaged => self.template.acceleration_damaged,
            BodyDamageType::ReallyDamaged => self.template.acceleration_damaged * 0.5,
            BodyDamageType::Rubble => 0.0,
        }
    }

    /// Get maximum lift for given damage condition
    pub fn get_max_lift(&self, condition: BodyDamageType) -> Real {
        match condition {
            BodyDamageType::Pristine => self.max_lift,
            BodyDamageType::Damaged => self.template.lift_damaged,
            BodyDamageType::ReallyDamaged => self.template.lift_damaged * 0.5,
            BodyDamageType::Rubble => 0.0,
        }
    }

    /// Get braking
    pub fn get_braking(&self) -> Real {
        self.max_braking
    }

    /// Get appearance
    pub fn get_appearance(&self) -> LocomotorAppearance {
        self.template.appearance
    }

    /// Check if locomotor uses 3D close-enough distance.
    pub fn is_close_enough_dist_3d(&self) -> Bool {
        (self.flags & FLAG_CLOSE_ENOUGH_3D) != 0
    }

    /// Get legal surfaces
    pub fn get_legal_surfaces(&self) -> LocomotorSurfaceTypeMask {
        self.template.surfaces
    }

    /// Get template name
    pub fn get_template_name(&self) -> &str {
        &self.template.name
    }

    /// Calculate slow down distance needed to reach desired speed
    /// Matches C++ Locomotor.cpp:62-73 calcSlowDownDist
    fn calc_slow_down_dist(cur_speed: Real, desired_speed: Real, max_braking: Real) -> Real {
        let delta = cur_speed - desired_speed;
        if delta <= 0.0 {
            return 0.0;
        }

        let dist = (delta * delta / max_braking.abs()) * 0.5;

        // Use a little fudge so that things can stop "on a dime" more easily
        const FUDGE: Real = 1.05;
        dist * FUDGE
    }

    /// Move towards position - Treads locomotor (tanks) with full physics
    /// Matches C++ Locomotor.cpp:1144-1255 moveTowardsPositionTreads
    pub fn move_towards_position_treads_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_downhill_only(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_jump_slowdown(desired_speed, current_pos, goal_pos);
        let max_acceleration = self.get_max_acceleration(condition);

        // Calculate relative angle to goal (with turn pivot offset)
        // C++ uses rotateTowardsPosition which also sets physics->setTurning
        let desired_angle =
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, self.is_braking());
        let rel_angle = Self::std_angle_diff(desired_angle, current_angle);

        // Modulate speed according to turning
        // C++ Locomotor.cpp:1170-1173
        const QUARTER_PI: Real = std::f32::consts::PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTER_PI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;
        goal_speed = self.apply_naval_turn_limit(goal_speed, current_angle, desired_angle);

        // Check if close to target and turning - slow down for precision
        // C++ Locomotor.cpp:1190-1192
        let dx = current_pos.x - goal_pos.x;
        let dy = current_pos.y - goal_pos.y;
        if (dx * dx + dy * dy) < (2.0 * PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F) && angle_coeff > 0.05 {
            goal_speed = current_speed * 0.6;
        }

        // Braking logic - matches C++ Locomotor.cpp:1187-1221
        // C++ uses actualSpeed / getBraking() for time and actualSpeed/1.50f * time for dist
        let braking = self.get_braking();
        let slow_down_time = if braking > 0.0 { current_speed / braking } else { 0.0 };
        let slow_down_dist = (current_speed / 1.5) * slow_down_time;

        // Start braking if close enough and not already braking
        // C++ Locomotor.cpp:1194-1198
        if on_path_dist_to_goal < slow_down_dist
            && !self.is_braking()
            && !self.no_slow_down_approaching_dest()
        {
            self.set_flag(FLAG_IS_BRAKING, true);
            self.braking_factor = 1.1;
        }

        // Stop braking if far enough from goal
        // C++ Locomotor.cpp:1200-1203
        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F && on_path_dist_to_goal > 2.0 * slow_down_dist {
            self.set_flag(FLAG_IS_BRAKING, false);
        }

        // Apply braking factor and reduce speed
        // C++ Locomotor.cpp:1205-1221
        if self.is_braking() {
            if on_path_dist_to_goal > 0.0 {
                self.braking_factor = slow_down_dist / on_path_dist_to_goal;
            }
            self.braking_factor *= self.braking_factor;
            if self.braking_factor > MAX_BRAKING_FACTOR {
                self.braking_factor = MAX_BRAKING_FACTOR;
            }

            if slow_down_dist > on_path_dist_to_goal {
                goal_speed = current_speed - braking;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else if slow_down_dist > on_path_dist_to_goal * 0.75 {
                goal_speed = current_speed - braking / 2.0;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else {
                goal_speed = current_speed;
            }
        }

        // Calculate acceleration force - matches C++ Locomotor.cpp:1230-1254
        // C++ uses mass * acceleration and clamps accelForce <= mass * speedDelta
        // We return the acceleration directly; the caller applies mass.
        let speed_delta = goal_speed - current_speed;
        let acceleration = if speed_delta > 0.0 {
            max_acceleration
        } else {
            -self.braking_factor * braking
        };

        (current_pos, desired_angle, acceleration)
    }

    /// Move towards position - Wheels locomotor (trucks, vehicles) with full physics
    /// Matches C++ Locomotor.cpp:1258-1498 moveTowardsPositionWheels
    pub fn move_towards_position_wheels_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
        major_radius: Real,
        current_frame: u32,
    ) -> (Coord3D, Real, Real, bool) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let max_turn_rate = self.get_max_turn_rate(condition);
        let max_acceleration = self.get_max_acceleration(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_downhill_only(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);

        let mut turn_speed = self.template.min_turn_speed;
        let mut desired_angle =
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, false);
        let mut rel_angle = Self::std_angle_diff(desired_angle, current_angle);

        let mut move_backwards = false;

        // Wheeled vehicles can only turn while moving, so make sure the turn speed is reasonable.
        // C++ Locomotor.cpp:1283-1286
        if turn_speed < max_speed / 4.0 {
            turn_speed = max_speed / 4.0;
        }

        let mut actual_speed = current_speed;
        let mut do3point_turn = false;

        // 3-point turn logic - C++ Locomotor.cpp:1292-1313
        if actual_speed == 0.0 {
            self.set_flag(FLAG_MOVING_BACKWARDS, false);
            if self.template.can_move_backward && rel_angle.abs() > std::f32::consts::PI / 2.0 {
                self.set_flag(FLAG_MOVING_BACKWARDS, true);
                self.set_flag(
                    FLAG_DOING_THREE_POINT_TURN,
                    on_path_dist_to_goal > 5.0 * major_radius,
                );
            }
        }
        if self.is_moving_backwards() {
            if rel_angle.abs() < std::f32::consts::PI / 2.0 {
                move_backwards = false;
                self.set_flag(FLAG_MOVING_BACKWARDS, false);
            } else {
                move_backwards = true;
                self.set_flag(
                    FLAG_DOING_THREE_POINT_TURN,
                    on_path_dist_to_goal > 5.0 * major_radius,
                );
                do3point_turn = self.get_flag(FLAG_DOING_THREE_POINT_TURN);
                if !do3point_turn {
                    desired_angle = Self::normalize_angle(desired_angle + std::f32::consts::PI);
                    rel_angle = Self::std_angle_diff(desired_angle, current_angle);
                }
            }
        }

        // Reduce speed when turning sharply - C++ Locomotor.cpp:1316-1323
        const SMALL_TURN: Real = std::f32::consts::PI / 20.0;
        if rel_angle.abs() > SMALL_TURN && desired_speed > turn_speed {
            desired_speed = turn_speed;
        }

        let mut goal_speed = desired_speed;
        if move_backwards {
            actual_speed = -actual_speed;
        }
        goal_speed = self.apply_naval_turn_limit(goal_speed, current_angle, desired_angle);

        // Braking distance calculation - C++ Locomotor.cpp:1332-1337
        let braking = self.get_braking();
        let slow_down_time = if braking > 0.0 { actual_speed / braking + 1.0 } else { 0.0 };
        let slow_down_dist = (actual_speed / 1.5) * slow_down_time + actual_speed;
        let mut effective_slow_down_dist = slow_down_dist;
        if effective_slow_down_dist < 1.0 * PATHFIND_CELL_SIZE_F {
            effective_slow_down_dist = 1.0 * PATHFIND_CELL_SIZE_F;
        }

        // Start braking if close enough - C++ Locomotor.cpp:1393-1403
        if on_path_dist_to_goal < effective_slow_down_dist
            && !self.is_braking()
            && !self.no_slow_down_approaching_dest()
        {
            self.set_flag(FLAG_IS_BRAKING, true);
            self.braking_factor = 1.1;
        }

        if on_path_dist_to_goal > PATHFIND_CELL_SIZE_F && on_path_dist_to_goal > 2.0 * slow_down_dist {
            self.set_flag(FLAG_IS_BRAKING, false);
        }

        // Donut timer - stop near destination for precise positioning
        // C++ Locomotor.cpp:1405-1411
        if on_path_dist_to_goal > DONUT_DISTANCE {
            self.donut_timer = current_frame
                + (DONUT_TIME_DELAY_SECONDS * LOGICFRAMES_PER_SECOND as Real) as u32;
        } else if current_frame >= self.donut_timer {
            self.set_flag(FLAG_IS_BRAKING, true);
        }

        // Apply braking factor - C++ Locomotor.cpp:1413-1430
        if self.is_braking() {
            if on_path_dist_to_goal > 0.0 {
                self.braking_factor = slow_down_dist / on_path_dist_to_goal;
            }
            self.braking_factor *= self.braking_factor;
            if self.braking_factor > MAX_BRAKING_FACTOR {
                self.braking_factor = MAX_BRAKING_FACTOR;
            }
            // C++ sets m_brakingFactor = 1.0f after the clamp above (line 1420)
            // This means the braking factor calculation is effectively unused for wheels
            // and the code below uses the raw braking values.
            self.braking_factor = 1.0;

            if slow_down_dist > on_path_dist_to_goal {
                goal_speed = actual_speed - braking;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else if slow_down_dist > on_path_dist_to_goal * 0.75 {
                goal_speed = actual_speed - braking / 2.0;
                if goal_speed < 0.0 {
                    goal_speed = 0.0;
                }
            } else {
                goal_speed = actual_speed;
            }
        }

        // Turn rate based on speed - C++ Locomotor.cpp:1438-1444
        // (Turn factor is used for rotateObjAroundLocoPivot; we incorporate it into desired_angle)
        let turn_factor = if turn_speed > 0.0 {
            (actual_speed / turn_speed).abs().min(1.0)
        } else {
            0.0
        };
        let _turn_amount = turn_factor * max_turn_rate;

        // Acceleration force - C++ Locomotor.cpp:1458-1496
        let mut speed_delta = goal_speed - actual_speed;
        if move_backwards {
            speed_delta = -goal_speed + actual_speed;
        }
        let acceleration = if speed_delta == 0.0 {
            0.0
        } else if move_backwards {
            if speed_delta < 0.0 {
                -max_acceleration
            } else {
                self.braking_factor * braking
            }
        } else {
            if speed_delta > 0.0 {
                max_acceleration
            } else {
                -self.braking_factor * braking
            }
        };

        (current_pos, desired_angle, acceleration, move_backwards)
    }

    /// Move towards position - Legs locomotor (infantry) with full physics
    /// Matches C++ Locomotor.cpp:1594-1687 moveTowardsPositionLegs
    pub fn move_towards_position_legs_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        // C++ Locomotor.cpp:1596-1598 - downhill only check for legs
        if self.template.downhill_only && current_pos.z < goal_pos.z {
            return (current_pos, current_angle, 0.0);
        }

        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_jump_slowdown(desired_speed, current_pos, goal_pos);
        let max_acceleration = self.get_max_acceleration(condition);

        let mut desired_angle =
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, false);

        // Wander logic for infantry - C++ Locomotor.cpp:1618-1633
        if self.template.wander_width_factor != 0.0 {
            let angle_limit = std::f32::consts::PI / 8.0 * self.template.wander_width_factor;
            if self.is_offset_increasing() {
                self.angle_offset += self.offset_increment * current_speed;
                if self.angle_offset > angle_limit {
                    self.set_flag(FLAG_OFFSET_INCREASING, false);
                }
            } else {
                self.angle_offset -= self.offset_increment * current_speed;
                if self.angle_offset < -angle_limit {
                    self.set_flag(FLAG_OFFSET_INCREASING, true);
                }
            }
            desired_angle = Self::normalize_angle(desired_angle + self.angle_offset);
        }

        let rel_angle = Self::std_angle_diff(desired_angle, current_angle);

        // Modulate speed according to turning - C++ Locomotor.cpp:1641-1646
        const QUARTER_PI: Real = std::f32::consts::PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTER_PI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;
        goal_speed = self.apply_naval_turn_limit(goal_speed, current_angle, desired_angle);

        // Slow down as approaching destination - C++ Locomotor.cpp:1649-1653
        let braking = self.get_braking();
        let slow_down_dist =
            Self::calc_slow_down_dist(current_speed, self.template.min_speed, braking);
        if on_path_dist_to_goal < slow_down_dist && !self.no_slow_down_approaching_dest() {
            goal_speed = self.template.min_speed;
        }

        // Calculate acceleration - C++ Locomotor.cpp:1660-1686
        // C++ applies mass * acceleration as force, clamped to mass * speedDelta
        // We return the acceleration directly.
        let speed_delta = goal_speed - current_speed;
        let acceleration = if speed_delta > 0.0 {
            max_acceleration
        } else {
            -braking
        };

        (current_pos, desired_angle, acceleration)
    }

    /// Move towards position - Hover locomotor with full physics
    /// Matches C++ Locomotor.cpp:1863-1888 moveTowardsPositionHover
    pub fn move_towards_position_hover_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        // Hover uses the "Other" movement logic for 2D
        self.move_towards_position_other_physics(
            current_pos,
            current_angle,
            goal_pos,
            on_path_dist_to_goal,
            desired_speed,
            current_speed,
            condition,
        )
    }

    /// Move towards position - Other/generic locomotor with full physics
    /// Matches C++ Locomotor.cpp:2326-2404 moveTowardsPositionOther
    ///
    /// Returns (current_pos, desired_angle, acceleration).
    /// When ULTRA_ACCURATE is set and close enough, desired_angle is overridden
    /// to point directly at the goal (C++ slides without rotating the model).
    pub fn move_towards_position_other_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_downhill_only(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);
        desired_speed = self.apply_jump_slowdown(desired_speed, current_pos, goal_pos);
        if self.template.appearance == LocomotorAppearance::Wings
            && desired_speed < self.template.min_turn_speed
        {
            desired_speed = self.template.min_turn_speed;
        }
        let max_acceleration = self.get_max_acceleration(condition);

        // C++ Locomotor.cpp:2344-2366: ULTRA_ACCURATE slide-into-place logic
        // When close enough, don't turn -- just slide in the right direction.
        // C++ uses dirToApplyForce directly toward goal instead of unit direction vector.
        let mut goal_speed = desired_speed;
        let mut desired_angle = if matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) {
            self.apply_air_corrections(
                current_angle,
                self.apply_wings_circling(
                    current_pos,
                    goal_pos,
                    (goal_pos.y - current_pos.y).atan2(goal_pos.x - current_pos.x),
                ),
            )
        } else {
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, self.is_braking())
        };

        let mut sliding_into_place = false;
        self.set_flag(FLAG_SLIDING_INTO_PLACE, false);
        if self.is_ultra_accurate() {
            let slide_threshold = desired_speed * self.template.ultra_accurate_slide_factor;
            if (goal_pos.x - current_pos.x).abs() <= slide_threshold
                && (goal_pos.y - current_pos.y).abs() <= slide_threshold
            {
                // C++ Locomotor.cpp:2356-2360: override force direction toward goal,
                // don't turn (TURN_NONE). We return desired_angle pointing at goal
                // so the caller advances toward it, and set sliding flag so
                // step_angle skips rotation.
                let dx = goal_pos.x - current_pos.x;
                let dy = goal_pos.y - current_pos.y;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.001 {
                    desired_angle = dy.atan2(dx);
                }
                sliding_into_place = true;
                self.set_flag(FLAG_SLIDING_INTO_PLACE, true);
            }
        }

        // C++ Locomotor.cpp:2363-2366: rotateTowardsPosition only if not sliding
        // (handled by step_angle in the caller via sliding_into_place concept;
        // we encode it by returning the angle diff = 0 for ultra_accurate slides)

        let rel_angle = if sliding_into_place {
            // When sliding into place, angle_coeff stays 0 so we don't slow down.
            0.0
        } else {
            Self::std_angle_diff(desired_angle, current_angle)
        };

        const QUARTER_PI: Real = std::f32::consts::PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTER_PI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        goal_speed = (1.0 - angle_coeff) * desired_speed;
        goal_speed = self.apply_naval_turn_limit(goal_speed, current_angle, desired_angle);

        // C++ Locomotor.cpp:2368-2374: uses minSpeed, not 0.0
        if !self.no_slow_down_approaching_dest() {
            let slow_down_dist = Self::calc_slow_down_dist(current_speed, self.template.min_speed, self.get_braking());
            if on_path_dist_to_goal < slow_down_dist {
                goal_speed = self.template.min_speed;
            }
        }

        // C++ Locomotor.cpp:2380-2401: maintain goal speed
        // C++ clamps accelForce to mass * speedDelta to avoid overshooting.
        let speed_delta = goal_speed - current_speed;
        let acceleration = if speed_delta == 0.0 {
            0.0
        } else if speed_delta > 0.0 {
            max_acceleration
        } else {
            -self.get_braking()
        };

        (current_pos, desired_angle, acceleration)
    }

    /// Set active path from pathfinding result
    /// Matches C++ Locomotor path integration
    pub fn set_path(&mut self, path: crate::ai::pathfinding_system::Path, current_frame: u32) {
        let waypoints: Vec<Coord3D> = path.waypoints.iter().map(|wp| wp.position).collect();
        let layers: Vec<PathfindLayerEnum> = path.waypoints.iter().map(|wp| wp.layer).collect();

        if !waypoints.is_empty() {
            self.active_path = Some(ActivePath::new_with_layers(
                waypoints,
                layers,
                current_frame,
            ));
        }
    }

    /// Clear active path
    pub fn clear_path(&mut self) {
        self.active_path = None;
    }

    /// Update path following - main locomotor update for path-based movement
    /// Matches C++ Locomotor::Move and path following logic
    pub fn update_path_following(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        current_speed: Real,
        condition: BodyDamageType,
        desired_speed: Real,
        current_frame: u32,
        delta_time: Real,
    ) -> Option<(Coord3D, Real, Real)> {
        let use_3d_close_enough = self.is_close_enough_dist_3d();
        let close_enough_dist = self.close_enough_dist;
        let path = self.active_path.as_mut()?;

        // Get current target waypoint
        let target = path.current_target()?;

        // Check if we've reached current waypoint
        let delta_to_target = target - current_pos;
        let distance_to_target = if use_3d_close_enough {
            delta_to_target.length()
        } else {
            (delta_to_target.x * delta_to_target.x + delta_to_target.y * delta_to_target.y).sqrt()
        };
        if distance_to_target < close_enough_dist {
            // Advance to next waypoint
            if !path.advance_waypoint() {
                // Path complete
                self.active_path = None;
                return None;
            }
        }

        // Update distance to waypoint
        if let Some(path) = self.active_path.as_mut() {
            path.distance_to_waypoint = distance_to_target;
        }

        // Get next target after advancing
        let target = self.active_path.as_ref()?.current_target()?;

        // Calculate distance remaining on path
        let on_path_dist_to_goal = self
            .active_path
            .as_ref()
            .map(|p| p.distance_remaining())
            .unwrap_or(distance_to_target);

        // Desired speed based on path following and AI constraints
        let max_speed = self.get_max_speed_for_condition(condition);
        let desired_speed = desired_speed.min(max_speed);

        // Use locomotor-specific movement based on appearance
        match self.template.appearance {
            LocomotorAppearance::Treads => {
                let (_pos, desired_angle, accel) = self.move_towards_position_treads_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::FourWheels | LocomotorAppearance::Motorcycle => {
                let (_pos, desired_angle, acceleration, move_backwards) = self
                    .move_towards_position_wheels_physics(
                        current_pos,
                        current_angle,
                        target,
                        on_path_dist_to_goal,
                        desired_speed,
                        current_speed,
                        condition,
                        self.close_enough_dist, // major_radius proxy
                        current_frame,
                    );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + acceleration * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    move_backwards,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::TwoLegs => {
                let (_pos, desired_angle, accel) = self.move_towards_position_legs_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Hover => {
                let (_pos, desired_angle, accel) = self.move_towards_position_hover_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Thrust => {
                let (_pos, desired_angle, accel) = self.move_towards_position_thrust_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Wings => {
                let (_pos, desired_angle, accel) = self.move_towards_position_wings_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Climber => {
                let (_pos, desired_angle, accel, move_backwards) = self
                    .move_towards_position_climber_physics(
                        current_pos,
                        current_angle,
                        target,
                        on_path_dist_to_goal,
                        desired_speed,
                        current_speed,
                        condition,
                    );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    move_backwards,
                );
                Some((new_pos, new_angle, new_speed))
            }
            LocomotorAppearance::Other => {
                let (_pos, desired_angle, accel) = self.move_towards_position_other_physics(
                    current_pos,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let mut new_pos = self.advance_position(
                    current_pos,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    false,
                );
                if (self.template.surfaces & SURFACE_CLIFF) != 0 {
                    new_pos.z = current_pos.z.min(new_pos.z);
                }
                Some((new_pos, new_angle, new_speed))
            }
        }
    }

    /// Check for obstacles and request path replan if needed
    /// Matches C++ obstacle detection and dynamic replanning
    pub fn check_obstacles(
        &mut self,
        current_pos: Coord3D,
        pathfinding: &crate::ai::pathfinding_system::PathfindingSystem,
        current_frame: u32,
        requester: ObjectID,
    ) -> bool {
        // Only check every N frames to avoid performance issues
        const OBSTACLE_CHECK_INTERVAL: u32 = 15; // ~0.5 seconds at 30fps

        if current_frame - self.last_obstacle_check < OBSTACLE_CHECK_INTERVAL {
            return false;
        }

        self.last_obstacle_check = current_frame;

        // Get next waypoint to check line of sight
        let path = match self.active_path.as_ref() {
            Some(p) => p,
            None => return false,
        };
        let next_waypoint = match path.next_waypoint() {
            Some(wp) => wp,
            None => return false,
        };

        // Check if path to next waypoint is blocked
        let capabilities = self.to_movement_capabilities();
        let start_coord =
            crate::ai::pathfinding_system::GridCoord::from_world(&current_pos, capabilities.layer);
        let next_coord = crate::ai::pathfinding_system::GridCoord::from_world(
            &next_waypoint,
            capabilities.layer,
        );

        // Detect newly blocked movement between current position and the next waypoint.
        let line_clear = pathfinding.is_line_clear_between(&current_pos, &next_waypoint);

        let terrain_layer = match capabilities.layer {
            crate::ai::pathfinding_system::PathfindLayerEnum::Ground
            | crate::ai::pathfinding_system::PathfindLayerEnum::Tunnel
            | crate::ai::pathfinding_system::PathfindLayerEnum::Invalid => {
                crate::common::PathfindLayerEnum::Ground
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Air => {
                crate::common::PathfindLayerEnum::Top
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Water => {
                crate::common::PathfindLayerEnum::Water
            }
        };

        let terrain_blocked = pathfinding
            .terrain_at(&next_waypoint, terrain_layer)
            .map(|terrain| {
                matches!(
                    terrain,
                    crate::ai::pathfinding_system::TerrainType::Obstacle
                        | crate::ai::pathfinding_system::TerrainType::Impassable
                )
            })
            .unwrap_or(true);

        let obstacle_detected = !line_clear || terrain_blocked;

        if obstacle_detected {
            log::trace!(
                "Locomotor obstacle detected for object {} from {:?} to {:?}",
                requester,
                start_coord,
                next_coord
            );
        }

        obstacle_detected
    }

    /// Get terrain height at position from pathfinding grid
    /// Matches C++ terrain height queries
    pub fn get_terrain_height(
        &self,
        pos: &Coord3D,
        _pathfinding: &crate::ai::pathfinding_system::PathfindingSystem,
    ) -> Real {
        let capabilities = self.to_movement_capabilities();
        let terrain_layer = match capabilities.layer {
            crate::ai::pathfinding_system::PathfindLayerEnum::Ground => {
                crate::common::PathfindLayerEnum::Ground
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Air => {
                crate::common::PathfindLayerEnum::Top
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Water => {
                crate::common::PathfindLayerEnum::Water
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Tunnel => {
                crate::common::PathfindLayerEnum::Tunnel
            }
            crate::ai::pathfinding_system::PathfindLayerEnum::Invalid => {
                crate::common::PathfindLayerEnum::Ground
            }
        };

        // Get terrain height from terrain logic.
        match self.template.appearance {
            LocomotorAppearance::Thrust | LocomotorAppearance::Wings => self.preferred_height,
            _ => TheTerrainLogic::get()
                .map(|terrain| terrain.get_layer_height(pos.x, pos.y, terrain_layer))
                .unwrap_or(pos.z),
        }
    }

    /// Helper: normalize angle to [-PI, PI]
    /// Matches C++ normalizeAngle
    fn normalize_angle(angle: Real) -> Real {
        let mut a = angle;
        while a > std::f32::consts::PI {
            a -= 2.0 * std::f32::consts::PI;
        }
        while a < -std::f32::consts::PI {
            a += 2.0 * std::f32::consts::PI;
        }
        a
    }

    /// Helper: standard angle difference
    /// Matches C++ stdAngleDiff
    fn std_angle_diff(angle1: Real, angle2: Real) -> Real {
        Self::normalize_angle(angle1 - angle2)
    }

    fn compute_z_target(&self, current: Coord3D, target: Coord3D) -> Option<Real> {
        let (ground_z, highest_z, surface_z) = TheTerrainLogic::get()
            .map(|terrain| {
                let mut ground = terrain.get_ground_height(target.x, target.y, None);
                let mut layer = terrain.get_highest_layer_for_destination(&target);
                let mut highest = terrain.get_layer_height(target.x, target.y, layer);
                let mut water_z = 0.0;
                let mut terrain_z = 0.0;
                let underwater = terrain.is_underwater(
                    target.x,
                    target.y,
                    Some(&mut water_z),
                    Some(&mut terrain_z),
                );

                if self.template.behavior_z == LocomotorBehaviorZ::SmoothRelativeToHighestLayer {
                    let current_layer = terrain.get_layer_for_destination(&current);
                    if current_layer != crate::common::PathfindLayerEnum::Ground {
                        layer = current_layer;
                    } else {
                        layer = terrain.get_highest_layer_for_destination(&current);
                    }
                    ground = terrain.get_ground_height(current.x, current.y, None);
                    highest = terrain.get_layer_height(current.x, current.y, layer);
                }

                let surface = match self.template.appearance {
                    LocomotorAppearance::Thrust
                    | LocomotorAppearance::Wings
                    | LocomotorAppearance::Hover => highest.max(ground),
                    _ => ground,
                };
                (ground, highest.max(ground), surface)
            })
            .unwrap_or((target.z, target.z, target.z));

        let mut desired_z = match self.template.behavior_z {
            LocomotorBehaviorZ::NoZMotiveForce => {
                if self.is_close_enough_dist_3d() {
                    target.z
                } else {
                    return None;
                }
            }
            LocomotorBehaviorZ::SeaLevel => surface_z,
            LocomotorBehaviorZ::AbsoluteHeight | LocomotorBehaviorZ::FixedAbsoluteHeight => {
                self.preferred_height
            }
            LocomotorBehaviorZ::SurfaceRelativeHeight
            | LocomotorBehaviorZ::FixedSurfaceRelativeHeight => surface_z + self.preferred_height,
            LocomotorBehaviorZ::RelativeToGroundAndBuildings
            | LocomotorBehaviorZ::SmoothRelativeToHighestLayer => highest_z + self.preferred_height,
        };

        if self.uses_precise_z_pos() {
            desired_z = target.z;
        }

        if self.preferred_height_damping > 0.0 && !self.uses_precise_z_pos() {
            let delta = desired_z - current.z;
            desired_z = current.z + delta * self.preferred_height_damping;
        }
        if matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) && self.template.elevator_correction_degree > 0.0
        {
            let max_delta = self
                .template
                .elevator_correction_degree
                .max(0.0)
                .to_radians();
            let z_delta = (desired_z - current.z).clamp(-max_delta, max_delta);
            desired_z = current.z + z_delta;
        }

        Some(desired_z)
    }

    fn is_naval_blocked_at(&self, pos: Coord3D) -> bool {
        if (self.template.surfaces & SURFACE_WATER) == 0 {
            return false;
        }
        if let Some(terrain) = TheTerrainLogic::get() {
            let mut water_z = 0.0;
            let mut terrain_z = 0.0;
            return !terrain.is_underwater(pos.x, pos.y, Some(&mut water_z), Some(&mut terrain_z));
        }
        false
    }

    fn apply_downhill_only(&self, desired_speed: Real, current: Coord3D, target: Coord3D) -> Real {
        if self.template.downhill_only && target.z > current.z + 0.01 {
            0.0
        } else {
            desired_speed
        }
    }

    fn is_tunnel_too_shallow(&self, current: Coord3D, target: Coord3D) -> bool {
        if (self.template.surfaces & SURFACE_CLIFF) == 0 {
            return false;
        }
        if let Some(terrain) = TheTerrainLogic::get() {
            let surface = terrain.get_ground_height(target.x, target.y, None);
            return target.z > surface - 0.5 || current.z > surface - 0.5;
        }
        false
    }

    fn apply_tunnel_depth_constraint(
        &self,
        desired_speed: Real,
        current: Coord3D,
        target: Coord3D,
    ) -> Real {
        if self.is_tunnel_too_shallow(current, target) {
            0.0
        } else {
            desired_speed
        }
    }

    fn apply_jump_slowdown(&self, desired_speed: Real, current: Coord3D, target: Coord3D) -> Real {
        // Jump slowdown applies to infantry-like appearances
        if !matches!(
            self.template.appearance,
            LocomotorAppearance::TwoLegs | LocomotorAppearance::Climber
        ) {
            return desired_speed;
        }
        let dist = (target - current).length();
        if dist < self.template.wander_about_point_radius.max(1.0) {
            desired_speed * 0.5
        } else {
            desired_speed
        }
    }

    fn apply_naval_turn_limit(
        &self,
        desired_speed: Real,
        current_angle: Real,
        desired_angle: Real,
    ) -> Real {
        if (self.template.surfaces & SURFACE_WATER) == 0 {
            return desired_speed;
        }
        let rel = Self::std_angle_diff(desired_angle, current_angle).abs();
        let limit = std::f32::consts::PI / 6.0;
        if rel > limit {
            desired_speed * 0.6
        } else {
            desired_speed
        }
    }

    fn apply_wings_circling(&self, current: Coord3D, target: Coord3D, desired_angle: Real) -> Real {
        if self.template.appearance != LocomotorAppearance::Wings {
            return desired_angle;
        }
        if self.template.circling_radius <= 0.0 {
            return desired_angle;
        }
        let delta = target - current;
        let dist = delta.length();
        if dist <= self.template.circling_radius {
            let base_angle = (delta.y).atan2(delta.x);
            let dir = if self.template.turn_pivot_offset >= 0.0 {
                1.0
            } else {
                -1.0
            };
            return Self::normalize_angle(base_angle + dir * (std::f32::consts::PI / 2.0));
        }
        desired_angle
    }

    fn apply_air_corrections(&self, current_angle: Real, desired_angle: Real) -> Real {
        if !matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) {
            return desired_angle;
        }
        let rel = Self::std_angle_diff(desired_angle, current_angle);
        let max_deg = self.template.rudder_correction_degree.max(0.0).to_radians();
        if max_deg <= 0.0 {
            return desired_angle;
        }
        let clamped = rel.clamp(-max_deg, max_deg);
        Self::normalize_angle(current_angle + clamped)
    }

    fn desired_angle_with_pivot(
        &self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        is_braking: bool,
    ) -> Real {
        let mut pivot_offset = self.template.turn_pivot_offset;
        if is_braking {
            pivot_offset = 0.0;
        }
        if pivot_offset.abs() < 0.0001 {
            return (goal_pos.y - current_pos.y).atan2(goal_pos.x - current_pos.x);
        }

        // Approximate bounding radius using close-enough distance as a proxy.
        let offset = pivot_offset * self.close_enough_dist.max(1.0);
        let dir_x = current_angle.cos();
        let dir_y = current_angle.sin();
        let turn_x = current_pos.x + dir_x * offset;
        let turn_y = current_pos.y + dir_y * offset;
        let dx = goal_pos.x - turn_x;
        let dy = goal_pos.y - turn_y;
        if dx.abs() < 0.1 && dy.abs() < 0.1 {
            current_angle
        } else {
            dy.atan2(dx)
        }
    }

    fn step_angle(
        &self,
        current_angle: Real,
        desired_angle: Real,
        condition: BodyDamageType,
        delta_time: Real,
    ) -> Real {
        // C++ Locomotor.cpp:2356: when ULTRA_ACCURATE and sliding into place,
        // TURN_NONE is set so the model does not rotate.
        if self.get_flag(FLAG_SLIDING_INTO_PLACE) {
            return current_angle;
        }

        let mut max_turn = self.get_max_turn_rate(condition) * delta_time.max(0.0);
        if matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) && self.template.rudder_correction_rate > 0.0
        {
            let rudder_limit = self.template.rudder_correction_rate * delta_time.max(0.0);
            if rudder_limit > 0.0 {
                max_turn = max_turn.min(rudder_limit);
            }
        }
        if max_turn <= 0.0 {
            return current_angle;
        }

        let diff = Self::std_angle_diff(desired_angle, current_angle);
        current_angle + diff.clamp(-max_turn, max_turn)
    }

    fn advance_position(
        &self,
        current: Coord3D,
        target: Coord3D,
        angle: Real,
        speed: Real,
        delta_time: Real,
        move_backwards: bool,
    ) -> Coord3D {
        let mut new_pos = current;
        let step = speed.max(0.0) * delta_time.max(0.0);
        if step > 0.0 {
            let dir_sign = if move_backwards { -1.0 } else { 1.0 };
            new_pos.x += angle.cos() * step * dir_sign;
            new_pos.y += angle.sin() * step * dir_sign;
        }

        if let Some(z_target) = self.compute_z_target(current, target) {
            let z_delta = z_target - current.z;
            if z_delta.abs() > f32::EPSILON {
                let mut z_speed = self.template.speed_limit_z.max(0.0);
                if matches!(
                    self.template.appearance,
                    LocomotorAppearance::Wings | LocomotorAppearance::Thrust
                ) && self.template.elevator_correction_rate > 0.0
                {
                    z_speed = z_speed.min(self.template.elevator_correction_rate).max(0.0);
                }
                let z_step = if z_speed > 0.0 {
                    z_delta.signum() * (z_speed * delta_time.max(0.0)).min(z_delta.abs())
                } else {
                    z_delta
                };
                new_pos.z += z_step;
            }
        }

        new_pos
    }

    /// Move towards position - Thrust locomotor (helicopters) using core steering.
    /// Matches C++ Locomotor.cpp:1891-2003 moveTowardsPositionThrust
    pub fn move_towards_position_thrust_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.clamp(self.template.min_speed, max_speed);
        let braking = self.get_braking();

        // Slow down approaching destination - C++ Locomotor.cpp:1899-1904
        if braking > 0.0 {
            let slow_down_dist = Self::calc_slow_down_dist(
                current_speed,
                self.template.min_speed,
                braking,
            );
            if on_path_dist_to_goal < slow_down_dist && !self.no_slow_down_approaching_dest() {
                desired_speed = self.template.min_speed;
            }
        }

        // Preferred height adjustment - C++ Locomotor.cpp:1914-1934
        let mut local_goal_pos = goal_pos;
        if self.preferred_height != 0.0 && !self.uses_precise_z_pos() {
            // C++ getSurfaceHtAtPt: checks if underwater, returns waterZ or terrainZ
            let surface_ht = TheTerrainLogic::get()
                .map(|terrain| {
                    let mut water_z = 0.0;
                    let mut terrain_z = 0.0;
                    if terrain.is_underwater(
                        current_pos.x,
                        current_pos.y,
                        Some(&mut water_z),
                        Some(&mut terrain_z),
                    ) {
                        water_z
                    } else {
                        terrain_z
                    }
                })
                .unwrap_or(0.0);
            local_goal_pos.z = self.preferred_height + surface_ht;
            let delta = local_goal_pos.z - current_pos.z;
            let damped_delta = delta * self.preferred_height_damping;
            local_goal_pos.z = current_pos.z + damped_delta;
        }

        // Desired heading toward goal with thrust angle clamping
        // C++ Locomotor.cpp:1936-1950
        let raw_desired_angle = (local_goal_pos.y - current_pos.y).atan2(local_goal_pos.x - current_pos.x);
        let mut desired_angle = if matches!(
            self.template.appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) {
            self.apply_air_corrections(
                current_angle,
                self.apply_wings_circling(current_pos, local_goal_pos, raw_desired_angle),
            )
        } else {
            self.desired_angle_with_pivot(current_pos, current_angle, local_goal_pos, self.is_braking())
        };

        // C++ Locomotor.cpp:1948-1950: clamp thrust angle relative to forward direction
        if self.template.max_thrust_angle > 0.0 {
            let max_turn_rate = self.get_max_turn_rate(condition);
            if max_turn_rate > 0.0 {
                let rel = Self::std_angle_diff(desired_angle, current_angle);
                let clamped = rel.clamp(
                    -self.template.max_thrust_angle,
                    self.template.max_thrust_angle,
                );
                desired_angle = Self::normalize_angle(current_angle + clamped);
            }
        }

        // Speed delta and acceleration - C++ Locomotor.cpp:1939-1940, 1982-2002
        let speed_delta = desired_speed - current_speed;
        let max_accel = if speed_delta > 0.0 || braking == 0.0 {
            self.get_max_acceleration(condition)
        } else {
            -braking
        };

        // C++ Locomotor.cpp:1988-1991: damping factor
        let max_forward_speed = if max_speed <= 0.0 { 0.01 } else { max_speed };
        let damping = (0.0f32).max(max_accel / max_forward_speed).min(1.0);

        // Net acceleration = thrust_accel - velocity_damping
        // C++ applies: accelVec = thrustDir * maxAccel - curVel * damping
        // We simplify: the acceleration returned is the net effect per frame
        let acceleration = max_accel - current_speed * damping;

        (current_pos, desired_angle, acceleration)
    }

    /// Move towards position - Wings (fixed-wing aircraft) locomotor.
    /// Matches C++ Locomotor.cpp:1821-1860 moveTowardsPositionWings
    ///
    /// Key behaviors:
    /// - Circle-for-landing: when `circle_thresh > 0` and the Z delta to goal
    ///   exceeds the threshold, the aircraft aims for a point on the opposite
    ///   side of a circle around the goal to gain/lose altitude before resuming.
    /// - Enforces minimum turn speed (wings cannot fly below min_turn_speed).
    /// - Applies circling correction when within `circling_radius` of target.
    /// - Applies air corrections (rudder correction degree limiting).
    /// - Otherwise delegates to the same physics as Other locomotors.
    pub fn move_towards_position_wings_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real) {
        // C++ Locomotor.cpp:1821-1860 moveTowardsPositionWings
        //
        // The C++ code has the circle-for-landing logic guarded by #ifdef CIRCLE_FOR_LANDING,
        // which is disabled (#define NO_CIRCLE_FOR_LANDING) in the shipped game.
        // We implement it here but gate it on circle_thresh > 0 (default 0.0) so it
        // matches the disabled-by-default behavior while remaining available.
        //
        // When circle_thresh > 0 and the vertical distance (dz) to the goal exceeds
        // the threshold, we compute a point on the opposite side of a circle centered
        // on the goal and aim for that instead. This makes the aircraft circle to
        // gain/lose altitude before resuming its course.

        let mut effective_goal = goal_pos;

        if self.template.circle_thresh > 0.0 {
            let dz = (goal_pos.z - current_pos.z).abs();

            if dz > self.template.circle_thresh {
                // Compute direction toward the goal position (2D only)
                let dx = goal_pos.x - current_pos.x;
                let dy = goal_pos.y - current_pos.y;

                // C++ Locomotor.cpp:1837-1840: use current orientation if dx,dy are ~zero
                let angle_toward_pos = if dx.abs() < 0.001 && dy.abs() < 0.001 {
                    current_angle
                } else {
                    dy.atan2(dx)
                };

                // C++ Locomotor.cpp:1842-1843: aim for the opposite side of the circle
                // aimDir = PI - PI/8 = 7*PI/8
                let aim_dir = std::f32::consts::PI - std::f32::consts::FRAC_PI_8;
                let circle_angle = angle_toward_pos + aim_dir;

                // C++ Locomotor.cpp:1846: turnRadius = calcMinTurnRadius * 4
                let turn_radius = self.calc_min_turn_radius(condition) * 4.0;

                // C++ Locomotor.cpp:1849-1851: project a spot "radius" dist away from goal
                effective_goal = Coord3D {
                    x: goal_pos.x + circle_angle.cos() * turn_radius,
                    y: goal_pos.y + circle_angle.sin() * turn_radius,
                    z: goal_pos.z,
                };

                // C++ Locomotor.cpp:1852: moveTowardsPositionOther with the adjusted goal
                return self.move_towards_position_other_physics(
                    current_pos,
                    current_angle,
                    effective_goal,
                    0.0, // onPathDistToGoal = 0 (not on path when circling)
                    desired_speed,
                    current_speed,
                    condition,
                );
            }
        }

        // C++ Locomotor.cpp:1859: handle the 2D component via moveTowardsPositionOther
        self.move_towards_position_other_physics(
            current_pos,
            current_angle,
            effective_goal,
            on_path_dist_to_goal,
            desired_speed,
            current_speed,
            condition,
        )
    }

    /// Move towards position - Climber locomotor (cliff climbing).
    /// Matches C++ Locomotor.cpp:1690-1818 moveTowardsPositionClimb
    pub fn move_towards_position_climber_physics(
        &mut self,
        current_pos: Coord3D,
        current_angle: Real,
        goal_pos: Coord3D,
        on_path_dist_to_goal: Real,
        desired_speed: Real,
        current_speed: Real,
        condition: BodyDamageType,
    ) -> (Coord3D, Real, Real, bool) {
        let max_speed = self.get_max_speed_for_condition(condition);
        let mut desired_speed = desired_speed.min(max_speed);
        if self.is_naval_blocked_at(current_pos) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current_pos, goal_pos);
        let max_acceleration = self.get_max_acceleration(condition);
        let braking = self.get_braking();

        // Climbing detection - C++ Locomotor.cpp:1711-1716
        // Uses PATHFIND_CELL_SIZE_F for the threshold (10.0)
        let dz = current_pos.z - goal_pos.z;
        if dz * dz > PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
            self.set_flag(FLAG_CLIMBING, true);
        }
        if dz.abs() < 1.0 {
            self.set_flag(FLAG_CLIMBING, false);
        }

        let mut move_backwards = false;

        // Climbing behavior - check ground slope ahead - C++ Locomotor.cpp:1721-1740
        if self.is_climbing() {
            // C++ normalizes the 2D direction from pos to goalPos, then adds pos back
            // to get a point exactly 1 unit away in that direction:
            //   delta = goalPos; delta -= pos; delta.z=0; delta.normalize();
            //   delta += pos; delta.z = getGroundHeight(delta.x, delta.y);
            let delta = goal_pos - current_pos;
            let delta_len = (delta.x * delta.x + delta.y * delta.y).sqrt();
            let mut forward_x = current_pos.x;
            let mut forward_y = current_pos.y;
            if delta_len > 0.001 {
                forward_x = current_pos.x + delta.x / delta_len;
                forward_y = current_pos.y + delta.y / delta_len;
            }
            let ground_z = TheTerrainLogic::get()
                .map(|terrain| terrain.get_ground_height(forward_x, forward_y, None))
                .unwrap_or(current_pos.z);

            if ground_z < current_pos.z - 0.1 {
                move_backwards = true;
            }

            // C++ Locomotor.cpp:1734-1739 - reduce speed based on slope
            let ground_slope = (ground_z - current_pos.z).abs();
            let ground_slope = if ground_slope < 1.0 { 1.0 } else { ground_slope };
            if ground_slope > 1.0 {
                desired_speed /= ground_slope * 4.0;
            }
        }
        self.set_flag(FLAG_MOVING_BACKWARDS, move_backwards);

        // Orient toward goal - C++ Locomotor.cpp:1746-1757
        let mut desired_angle =
            self.desired_angle_with_pivot(current_pos, current_angle, goal_pos, false);
        if move_backwards {
            desired_angle = Self::normalize_angle(desired_angle + std::f32::consts::PI);
        }
        let rel_angle = Self::std_angle_diff(desired_angle, current_angle);

        // Modulate speed by turn angle - C++ Locomotor.cpp:1762-1767
        const QUARTER_PI: Real = std::f32::consts::PI / 4.0;
        let mut angle_coeff = rel_angle.abs() / QUARTER_PI;
        if angle_coeff > 1.0 {
            angle_coeff = 1.0;
        }

        let mut goal_speed = (1.0 - angle_coeff) * desired_speed;

        let mut actual_speed = current_speed;
        if move_backwards {
            actual_speed = -actual_speed;
        }

        // Slow down approaching destination - C++ Locomotor.cpp:1776-1780
        let slow_down_dist =
            Self::calc_slow_down_dist(actual_speed.abs(), self.template.min_speed, braking);
        if on_path_dist_to_goal < slow_down_dist && !self.no_slow_down_approaching_dest() {
            goal_speed = self.template.min_speed;
        }

        // Acceleration with backward sign swap - C++ Locomotor.cpp:1785-1817
        let mut speed_delta = goal_speed - actual_speed;
        if move_backwards {
            speed_delta = -goal_speed + actual_speed;
        }
        let acceleration = if speed_delta == 0.0 {
            0.0
        } else if move_backwards {
            if speed_delta < 0.0 {
                -max_acceleration
            } else {
                braking
            }
        } else {
            if speed_delta > 0.0 {
                max_acceleration
            } else {
                -braking
            }
        };

        (current_pos, desired_angle, acceleration, move_backwards)
    }

    /// Integrate movement toward the requested target using locomotor rules.
    /// Matches the intent of C++ Locomotor::Move by honoring turn rates, braking, and locomotor type.
    pub fn move_towards(
        &mut self,
        current: Coord3D,
        current_angle: Real,
        current_speed: Real,
        target: Coord3D,
        desired_speed: Real,
        condition: BodyDamageType,
        delta_time: Real,
    ) -> (Coord3D, Real, Real) {
        let on_path_dist_to_goal = (target - current).length();
        let mut desired_speed = desired_speed;
        if self.is_naval_blocked_at(current) {
            desired_speed = 0.0;
        }
        desired_speed = self.apply_downhill_only(desired_speed, current, target);
        desired_speed = self.apply_tunnel_depth_constraint(desired_speed, current, target);
        desired_speed = self.apply_jump_slowdown(desired_speed, current, target);

        match self.template.appearance {
            LocomotorAppearance::Treads => {
                let (_pos, desired_angle, accel) = self.move_towards_position_treads_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos =
                    self.advance_position(current, target, new_angle, new_speed, delta_time, false);
                (new_pos, new_angle, new_speed)
            }
            LocomotorAppearance::FourWheels | LocomotorAppearance::Motorcycle => {
                let (_pos, desired_angle, acceleration, move_backwards) = self
                    .move_towards_position_wheels_physics(
                        current,
                        current_angle,
                        target,
                        on_path_dist_to_goal,
                        desired_speed,
                        current_speed,
                        condition,
                        self.close_enough_dist, // major_radius proxy
                        0, // no frame available in move_towards context
                    );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + acceleration * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    move_backwards,
                );
                (new_pos, new_angle, new_speed)
            }
            LocomotorAppearance::TwoLegs => {
                let (_pos, desired_angle, accel) = self.move_towards_position_legs_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos =
                    self.advance_position(current, target, new_angle, new_speed, delta_time, false);
                (new_pos, new_angle, new_speed)
            }
            LocomotorAppearance::Hover => {
                let (_pos, desired_angle, accel) = self.move_towards_position_hover_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos =
                    self.advance_position(current, target, new_angle, new_speed, delta_time, false);
                (new_pos, new_angle, new_speed)
            }
            LocomotorAppearance::Thrust => {
                let (_pos, desired_angle, accel) = self.move_towards_position_thrust_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos =
                    self.advance_position(current, target, new_angle, new_speed, delta_time, false);
                (new_pos, new_angle, new_speed)
            }
            LocomotorAppearance::Wings => {
                let (_pos, desired_angle, accel) = self.move_towards_position_wings_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos =
                    self.advance_position(current, target, new_angle, new_speed, delta_time, false);
                (new_pos, new_angle, new_speed)
            }
            LocomotorAppearance::Climber => {
                let (_pos, desired_angle, accel, move_backwards) = self
                    .move_towards_position_climber_physics(
                        current,
                        current_angle,
                        target,
                        on_path_dist_to_goal,
                        desired_speed,
                        current_speed,
                        condition,
                    );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let new_pos = self.advance_position(
                    current,
                    target,
                    new_angle,
                    new_speed,
                    delta_time,
                    move_backwards,
                );
                (new_pos, new_angle, new_speed)
            }
            LocomotorAppearance::Other => {
                let (_pos, desired_angle, accel) = self.move_towards_position_other_physics(
                    current,
                    current_angle,
                    target,
                    on_path_dist_to_goal,
                    desired_speed,
                    current_speed,
                    condition,
                );
                let max_speed = self.get_max_speed_for_condition(condition);
                let new_speed = (current_speed + accel * delta_time.max(0.0)).clamp(0.0, max_speed);
                let new_angle =
                    self.step_angle(current_angle, desired_angle, condition, delta_time);
                let mut new_pos =
                    self.advance_position(current, target, new_angle, new_speed, delta_time, false);
                if (self.template.surfaces & SURFACE_CLIFF) != 0 {
                    new_pos.z = current.z.min(new_pos.z);
                }
                (new_pos, new_angle, new_speed)
            }
        }
    }

    /// Request path from pathfinding system
    /// Matches C++ Locomotor.cpp pathfinding integration
    pub fn request_path(
        &self,
        requester: ObjectID,
        start: Coord3D,
        end: Coord3D,
        pathfinding: &mut crate::ai::pathfinding_system::PathfindingSystem,
    ) -> Result<Option<crate::ai::pathfinding_system::Path>, Box<dyn Error>> {
        use crate::ai::pathfinding_system::{PathRequest, PathResult};

        // Convert locomotor capabilities to pathfinding capabilities
        let capabilities = self.to_movement_capabilities();

        // Create path request
        let mut move_allies = false;
        let mut ignore_obstacle_id = None;
        let unit_size = if let Some(obj) = OBJECT_REGISTRY.get_object(requester) {
            if let Ok(guard) = obj.read() {
                if let Some(ai) = guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        move_allies = ai_guard.get_can_path_through_units();
                        let ignored = ai_guard.get_ignored_obstacle_id();
                        if ignored != crate::common::INVALID_ID {
                            ignore_obstacle_id = Some(ignored);
                        }
                    }
                }
                guard.get_geometry_info().get_major_radius()
            } else {
                self.template.close_enough_dist
            }
        } else {
            self.template.close_enough_dist
        };

        let request = PathRequest {
            requester,
            start,
            goal: end,
            capabilities,
            unit_size,
            priority: 1,
            allow_partial: true,
            frame_requested: crate::helpers::TheGameLogic::get_frame(),
            move_allies,
            ignore_obstacle_id,
        };

        // Request path (synchronous for now)
        match pathfinding.find_path_immediate(&request) {
            PathResult::Success(path) => Ok(Some(path)),
            PathResult::Failed(_reason) => Ok(None),
            PathResult::Pending => Ok(None),
        }
    }

    /// Find simple straight-line path (fallback when pathfinding unavailable)
    pub fn find_path_simple(
        &self,
        start: Coord2D,
        end: Coord2D,
    ) -> Result<Option<Vec<Coord2D>>, Box<dyn Error>> {
        if (start - end).length_squared() <= f32::EPSILON {
            return Ok(None);
        }

        // Simple straight-line path
        Ok(Some(vec![start, end]))
    }

    /// Calculate minimum turn radius for this locomotor
    /// Matches C++ calcMinTurnRadius() lines 1567-1590
    pub fn calc_min_turn_radius(&self, condition: BodyDamageType) -> Real {
        let min_speed = self.template.min_speed;
        let max_turn_rate = self.get_max_turn_rate(condition);

        if max_turn_rate > 0.0 {
            min_speed / max_turn_rate
        } else {
            f32::INFINITY
        }
    }

    /// Get surface height at point (water or ground)
    /// Matches C++ getSurfaceHtAtPt() lines 2007-2019
    pub fn get_surface_height_at_point(
        &self,
        x: Real,
        y: Real,
        terrain_height: Real,
        water_height: Option<Real>,
    ) -> Real {
        if let Some(water_z) = water_height {
            if terrain_height < water_z {
                return water_z;
            }
        }
        terrain_height
    }

    /// Convert to pathfinding movement capabilities
    pub fn to_movement_capabilities(&self) -> MovementCapabilities {
        let layer = match self.template.appearance {
            LocomotorAppearance::Thrust | LocomotorAppearance::Wings => PathfindLayerEnum::Air,
            _ => PathfindLayerEnum::Ground,
        };

        let amphibious = (self.template.surfaces & SURFACE_WATER) != 0
            && (self.template.surfaces & SURFACE_GROUND) != 0;

        let climber = (self.template.surfaces & SURFACE_CLIFF) != 0;

        let flying = matches!(
            self.template.appearance,
            LocomotorAppearance::Thrust | LocomotorAppearance::Wings
        );

        let tunneling = (self.template.surfaces & SURFACE_CLIFF) != 0;

        MovementCapabilities {
            layer,
            amphibious,
            crusher: false, // Would be set by unit type
            climber,
            flying,
            tunneling,
            surface_mask: self.template.surfaces,
        }
    }

    /// Apply locomotor settings to physics state
    pub fn apply_to_physics(&self, physics: &mut PhysicsState, _condition: BodyDamageType) {
        // Set physics type based on locomotor
        physics.physics_type = match self.template.appearance {
            LocomotorAppearance::Thrust | LocomotorAppearance::Wings => PhysicsType::Aircraft,
            LocomotorAppearance::Hover => PhysicsType::Hover,
            _ => PhysicsType::Normal,
        };

        // Set height parameters
        physics.target_hover_height = self.preferred_height;
        physics.hover_damping = self.preferred_height_damping as f32;
        physics.target_altitude = self.preferred_height;

        // Set terrain capabilities
        physics.can_cross_water = (self.template.surfaces & SURFACE_WATER) != 0;

        // Set gravity behavior
        physics.affected_by_gravity = !matches!(
            self.template.appearance,
            LocomotorAppearance::Hover | LocomotorAppearance::Thrust | LocomotorAppearance::Wings
        );

        // Set friction
        physics.friction = if self.template.stick_to_ground {
            0.9
        } else {
            0.7
        };
        physics.drag = if self.template.apply_2d_friction_when_airborne {
            0.95
        } else {
            0.98
        };

        physics.allow_motive_force_while_airborne = self.template.allow_motive_force_while_airborne;
    }

    // Flag helpers
    fn set_flag(&mut self, flag: u32, value: bool) {
        if value {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    fn get_flag(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    pub fn is_braking(&self) -> bool {
        self.get_flag(FLAG_IS_BRAKING)
    }

    pub fn is_moving_backwards(&self) -> bool {
        self.get_flag(FLAG_MOVING_BACKWARDS)
    }

    pub fn is_climbing(&self) -> bool {
        self.get_flag(FLAG_CLIMBING)
    }

    pub fn is_offset_increasing(&self) -> bool {
        self.get_flag(FLAG_OFFSET_INCREASING)
    }

    // Setters
    pub fn set_max_speed(&mut self, speed: Real) {
        self.max_speed = speed;
    }

    pub fn set_max_turn_rate(&mut self, rate: Real) {
        self.max_turn_rate = rate;
    }

    pub fn set_max_acceleration(&mut self, accel: Real) {
        self.max_accel = accel;
    }

    pub fn set_max_lift(&mut self, lift: Real) {
        self.max_lift = lift;
    }

    pub fn set_preferred_height(&mut self, height: Real) {
        self.preferred_height = height;
    }

    pub fn set_close_enough_dist(&mut self, dist: Real) {
        self.close_enough_dist = dist;
    }

    pub fn get_close_enough_dist(&self) -> Real {
        self.close_enough_dist
    }

    pub fn set_precise_z_pos(&mut self, enable: bool) {
        if enable {
            self.flags |= FLAG_PRECISE_Z_POS;
        } else {
            self.flags &= !FLAG_PRECISE_Z_POS;
        }
    }

    pub fn set_no_slow_down(&mut self, enable: bool) {
        if enable {
            self.flags |= FLAG_NO_SLOW_DOWN;
        } else {
            self.flags &= !FLAG_NO_SLOW_DOWN;
        }
    }

    pub fn set_allow_invalid_position(&mut self, enable: bool) {
        if enable {
            self.flags |= FLAG_ALLOW_INVALID_POS;
        } else {
            self.flags &= !FLAG_ALLOW_INVALID_POS;
        }
    }

    pub fn is_allowing_invalid_positions(&self) -> bool {
        (self.flags & FLAG_ALLOW_INVALID_POS) != 0
    }

    pub fn set_ultra_accurate(&mut self, enable: bool) {
        if enable {
            self.flags |= FLAG_ULTRA_ACCURATE;
        } else {
            self.flags &= !FLAG_ULTRA_ACCURATE;
        }
    }

    // Getters for flags
    pub fn uses_precise_z_pos(&self) -> bool {
        (self.flags & FLAG_PRECISE_Z_POS) != 0
    }

    pub fn no_slow_down_approaching_dest(&self) -> bool {
        (self.flags & FLAG_NO_SLOW_DOWN) != 0
    }

    pub fn allows_invalid_position(&self) -> bool {
        (self.flags & FLAG_ALLOW_INVALID_POS) != 0
    }

    pub fn is_ultra_accurate(&self) -> bool {
        (self.flags & FLAG_ULTRA_ACCURATE) != 0
    }
}

// ============================================================================
// LOCOMOTOR SET
// ============================================================================

/// Locomotor set for managing multiple locomotors per unit
/// Matches C++ LocomotorSet.h
#[derive(Debug, Clone)]
pub struct LocomotorSet {
    locomotors: HashMap<String, Arc<Mutex<Locomotor>>>,
    active_locomotor: Option<String>,
    /// Bitmask of valid surfaces across all added locomotors
    /// Matches C++ LocomotorSet::m_validLocomotorSurfaces
    valid_surfaces: LocomotorSurfaceTypeMask,
    /// Whether this set only allows downhill movement
    /// Matches C++ LocomotorSet::m_downhillOnly
    downhill_only: bool,
}

impl LocomotorSet {
    pub fn new() -> Self {
        Self {
            locomotors: HashMap::new(),
            active_locomotor: None,
            valid_surfaces: 0,
            downhill_only: false,
        }
    }

    /// Clear all locomotors - matches C++ LocomotorSet::clear()
    pub fn clear(&mut self) {
        self.locomotors.clear();
        self.active_locomotor = None;
        self.valid_surfaces = 0;
        self.downhill_only = false;
    }

    /// Add a locomotor from a template - matches C++ LocomotorSet::addLocomotor()
    pub fn add_locomotor(&mut self, name: String, locomotor: Arc<Mutex<Locomotor>>) {
        // Accumulate valid surfaces - matches C++ addLocomotor
        if let Ok(loco) = locomotor.lock() {
            self.valid_surfaces |= loco.get_legal_surfaces();
            if loco.template.downhill_only {
                self.downhill_only = true;
            }
        }
        if self.active_locomotor.is_none() {
            self.active_locomotor = Some(name.clone());
        }
        self.locomotors.insert(name, locomotor);
    }

    /// Find a locomotor that supports the given surface type mask
    /// Matches C++ LocomotorSet::findLocomotor(LocomotorSurfaceTypeMask t)
    pub fn find_locomotor(&self, surface_mask: LocomotorSurfaceTypeMask) -> Option<Arc<Mutex<Locomotor>>> {
        // C++ iterates m_locomotors and returns the first one whose template
        // surfaces overlap with the requested mask
        for (_name, loco) in &self.locomotors {
            if let Ok(l) = loco.lock() {
                if (l.get_legal_surfaces() & surface_mask) != 0 {
                    return Some(loco.clone());
                }
            }
        }
        None
    }

    pub fn set_active(&mut self, name: &str) -> bool {
        if self.locomotors.contains_key(name) {
            self.active_locomotor = Some(name.to_string());
            true
        } else {
            false
        }
    }

    pub fn get_active(&self) -> Option<Arc<Mutex<Locomotor>>> {
        self.active_locomotor
            .as_ref()
            .and_then(|name| self.locomotors.get(name).cloned())
    }

    pub fn get_locomotor(&self, name: &str) -> Option<Arc<Mutex<Locomotor>>> {
        self.locomotors.get(name).cloned()
    }

    /// Get the valid surface mask across all locomotors
    /// Matches C++ LocomotorSet::getValidSurfaces()
    pub fn get_valid_surfaces(&self) -> LocomotorSurfaceTypeMask {
        self.valid_surfaces
    }

    /// Check if this set only allows downhill movement
    /// Matches C++ LocomotorSet::isDownhillOnly()
    pub fn is_downhill_only(&self) -> bool {
        self.downhill_only
    }

    /// Returns the currently active locomotor (or the first entry) matching the C++ default logic.
    pub fn get_default_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
        if let Some(active) = self.get_active() {
            return Some(active);
        }
        self.locomotors.values().next().cloned()
    }

    /// Get number of locomotors in set
    pub fn len(&self) -> usize {
        self.locomotors.len()
    }

    /// Check if set is empty
    pub fn is_empty(&self) -> bool {
        self.locomotors.is_empty()
    }

    /// Iterate over all locomotors
    pub fn iter(&self) -> impl Iterator<Item = (&String, &Arc<Mutex<Locomotor>>)> {
        self.locomotors.iter()
    }
}

// ============================================================================
// TERRAIN SPEED MULTIPLIERS
// ============================================================================

/// Terrain speed multiplier table
#[derive(Debug, Clone)]
pub struct TerrainSpeedTable {
    multipliers: HashMap<(LocomotorAppearance, u8), Real>,
}

impl TerrainSpeedTable {
    pub fn new() -> Self {
        let mut table = Self {
            multipliers: HashMap::new(),
        };
        table.init_default_multipliers();
        table
    }

    fn init_default_multipliers(&mut self) {
        use LocomotorAppearance::*;

        // Terrain types: 0=clear, 1=rough, 2=very_rough, 3=water, 4=cliff, 5=road

        // Infantry
        self.set(TwoLegs, 0, 1.0); // Clear
        self.set(TwoLegs, 1, 0.8); // Rough
        self.set(TwoLegs, 2, 0.6); // Very rough
        self.set(TwoLegs, 3, 0.0); // Water (can't cross)
        self.set(TwoLegs, 4, 0.4); // Cliff (slow climb)
        self.set(TwoLegs, 5, 1.0); // Road (no bonus)

        // Wheeled
        self.set(FourWheels, 0, 1.0);
        self.set(FourWheels, 1, 0.7);
        self.set(FourWheels, 2, 0.4);
        self.set(FourWheels, 3, 0.0);
        self.set(FourWheels, 4, 0.0); // Can't climb
        self.set(FourWheels, 5, 1.5); // Road bonus

        // Tracked
        self.set(Treads, 0, 1.0);
        self.set(Treads, 1, 0.9);
        self.set(Treads, 2, 0.7);
        self.set(Treads, 3, 0.0);
        self.set(Treads, 4, 0.0);
        self.set(Treads, 5, 1.2); // Slight road bonus

        // Hover
        self.set(Hover, 0, 1.0);
        self.set(Hover, 1, 1.0);
        self.set(Hover, 2, 1.0);
        self.set(Hover, 3, 1.0); // Can cross water
        self.set(Hover, 4, 0.7); // Slower over cliffs
        self.set(Hover, 5, 1.0);

        // Aircraft (ignore terrain)
        for terrain in 0..6 {
            self.set(Thrust, terrain, 1.0);
            self.set(Wings, terrain, 1.0);
        }

        // Climber
        self.set(Climber, 0, 1.0);
        self.set(Climber, 1, 0.8);
        self.set(Climber, 2, 0.7);
        self.set(Climber, 3, 0.0);
        self.set(Climber, 4, 0.8); // Can climb cliffs
        self.set(Climber, 5, 1.0);

        // Other (generic)
        for terrain in 0..6 {
            self.set(Other, terrain, 1.0);
        }
    }

    fn set(&mut self, appearance: LocomotorAppearance, terrain: u8, multiplier: Real) {
        self.multipliers.insert((appearance, terrain), multiplier);
    }

    pub fn get_multiplier(&self, appearance: LocomotorAppearance, terrain: u8) -> Real {
        *self.multipliers.get(&(appearance, terrain)).unwrap_or(&1.0)
    }
}

// ============================================================================
// LOCOMOTOR STORE (GLOBAL REGISTRY)
// ============================================================================

/// Global locomotor template store
pub struct LocomotorStore {
    templates: RwLock<HashMap<String, Arc<LocomotorTemplate>>>,
    terrain_speeds: TerrainSpeedTable,
}

impl LocomotorStore {
    pub fn new() -> Self {
        Self {
            templates: RwLock::new(HashMap::new()),
            terrain_speeds: TerrainSpeedTable::new(),
        }
    }

    pub fn register_template(&self, template: LocomotorTemplate) {
        let name = template.name.clone();
        if let Ok(mut templates) = self.templates.write() {
            templates.insert(name, Arc::new(template));
        }
    }

    pub fn get_template(&self, name: &str) -> Option<Arc<LocomotorTemplate>> {
        if let Ok(templates) = self.templates.read() {
            templates.get(name).cloned()
        } else {
            None
        }
    }

    pub fn create_locomotor(&self, template_name: &str) -> Option<Locomotor> {
        self.get_template(template_name)
            .map(|template| Locomotor::new(template))
    }

    pub fn get_terrain_multiplier(&self, appearance: LocomotorAppearance, terrain: u8) -> Real {
        self.terrain_speeds.get_multiplier(appearance, terrain)
    }
}

// Global instance
pub static LOCOMOTOR_STORE: Lazy<Arc<LocomotorStore>> = Lazy::new(|| {
    let store = Arc::new(LocomotorStore::new());

    // Register default templates
    store.register_template(LocomotorTemplate::new_infantry("Infantry".to_string()));
    store.register_template(LocomotorTemplate::new_wheeled("Wheeled".to_string()));
    store.register_template(LocomotorTemplate::new_tracked("Tracked".to_string()));
    store.register_template(LocomotorTemplate::new_hover("Hover".to_string()));
    store.register_template(LocomotorTemplate::new_thrust("Thrust".to_string()));
    store.register_template(LocomotorTemplate::new_wings("Wings".to_string()));
    store.register_template(LocomotorTemplate::new_climber("Climber".to_string()));

    store
});

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locomotor_creation() {
        let template = Arc::new(LocomotorTemplate::new_infantry("TestInfantry".to_string()));
        let loco = Locomotor::new(template);

        assert_eq!(loco.get_appearance(), LocomotorAppearance::TwoLegs);
        assert!(loco.get_legal_surfaces() & SURFACE_GROUND != 0);
    }

    #[test]
    fn test_damage_affects_speed() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("TestVehicle".to_string()));
        let loco = Locomotor::new(template);

        let pristine_speed = loco.get_max_speed_for_condition(BodyDamageType::Pristine);
        let damaged_speed = loco.get_max_speed_for_condition(BodyDamageType::Damaged);

        assert!(damaged_speed < pristine_speed);
    }

    #[test]
    fn test_terrain_speed_multipliers() {
        let table = TerrainSpeedTable::new();

        // Wheeled gets road bonus
        assert_eq!(
            table.get_multiplier(LocomotorAppearance::FourWheels, 5),
            1.5
        );

        // Aircraft ignore terrain
        assert_eq!(table.get_multiplier(LocomotorAppearance::Wings, 2), 1.0);

        // Treads get road bonus
        assert_eq!(
            table.get_multiplier(LocomotorAppearance::Treads, 5),
            1.2
        );
    }

    #[test]
    fn test_movement_capabilities_conversion_basic() {
        let hover_template = Arc::new(LocomotorTemplate::new_hover("TestHover".to_string()));
        let hover = Locomotor::new(hover_template);

        let caps = hover.to_movement_capabilities();
        assert!(caps.amphibious);
        assert_eq!(caps.layer, PathfindLayerEnum::Ground);
    }

    #[test]
    fn test_locomotor_store() {
        let template = LOCOMOTOR_STORE.get_template("Infantry");
        assert!(template.is_some());

        let loco = LOCOMOTOR_STORE.create_locomotor("Infantry");
        assert!(loco.is_some());
    }

    #[test]
    fn test_active_path_creation() {
        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
        ];

        let path = ActivePath::new(waypoints.clone(), 0);
        assert_eq!(path.waypoint_count(), 3);
        assert_eq!(path.current_waypoint, 0);
        assert!((path.total_distance - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_active_path_navigation() {
        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(10.0, 10.0, 0.0),
        ];

        let mut path = ActivePath::new(waypoints, 0);

        // First waypoint
        assert_eq!(path.current_target().unwrap(), Coord3D::new(0.0, 0.0, 0.0));

        // Advance to next
        assert!(path.advance_waypoint());
        assert_eq!(path.current_target().unwrap(), Coord3D::new(10.0, 0.0, 0.0));

        // Advance to last
        assert!(path.advance_waypoint());
        assert_eq!(
            path.current_target().unwrap(),
            Coord3D::new(10.0, 10.0, 0.0)
        );

        // No more waypoints
        assert!(!path.advance_waypoint());
        assert!(path.is_complete());
    }

    #[test]
    fn test_path_request_integration() {
        use crate::ai::pathfinding_system::{create_pathfinding_system, PathfindingSystem};

        let template = Arc::new(LocomotorTemplate::new_infantry("TestInfantry".to_string()));
        let loco = Locomotor::new(template);

        let pathfinding = create_pathfinding_system(100, 100);

        let start = Coord3D::new(0.0, 0.0, 0.0);
        let end = Coord3D::new(50.0, 50.0, 0.0);

        let mut pathfinding_sys = pathfinding.write().unwrap();
        let result = loco.request_path(1, start, end, &mut *pathfinding_sys);

        assert!(result.is_ok());
    }

    #[test]
    fn test_path_following_update() {
        let template = Arc::new(LocomotorTemplate::new_infantry("TestInfantry".to_string()));
        let mut loco = Locomotor::new(template);

        // Set up a simple path
        let waypoints = vec![
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(10.0, 0.0, 0.0),
            Coord3D::new(20.0, 0.0, 0.0),
        ];
        let path = crate::ai::pathfinding_system::Path {
            waypoints: waypoints
                .iter()
                .enumerate()
                .map(|(i, pos)| crate::ai::pathfinding_system::PathWaypoint {
                    position: *pos,
                    layer: crate::ai::pathfinding_system::PathfindLayerEnum::Ground,
                    distance: (i * 10) as f32,
                })
                .collect(),
            total_cost: 20.0,
            complete: true,
            optimized: false,
            created_frame: 0,
        };

        loco.set_path(path, 0);
        assert!(loco.active_path.is_some());

        // Simulate update
        let current_pos = Coord3D::new(0.0, 0.0, 0.0);
        let result = loco.update_path_following(
            current_pos,
            0.0,
            0.0,
            BodyDamageType::Pristine,
            0.0,
            0,
            0.033,
        );

        assert!(result.is_some());
    }

    #[test]
    fn test_movement_capabilities_conversion() {
        // Test ground unit
        let ground_template = Arc::new(LocomotorTemplate::new_infantry("Infantry".to_string()));
        let ground_loco = Locomotor::new(ground_template);
        let ground_caps = ground_loco.to_movement_capabilities();
        assert_eq!(
            ground_caps.layer,
            crate::ai::pathfinding_system::PathfindLayerEnum::Ground
        );
        assert!(!ground_caps.amphibious);

        // Test air unit
        let air_template = Arc::new(LocomotorTemplate::new_thrust("Helicopter".to_string()));
        let air_loco = Locomotor::new(air_template);
        let air_caps = air_loco.to_movement_capabilities();
        assert_eq!(
            air_caps.layer,
            crate::ai::pathfinding_system::PathfindLayerEnum::Air
        );
        assert!(air_caps.flying);

        // Test hover unit
        let hover_template = Arc::new(LocomotorTemplate::new_hover("Hovercraft".to_string()));
        let hover_loco = Locomotor::new(hover_template);
        let hover_caps = hover_loco.to_movement_capabilities();
        assert!(hover_caps.amphibious);
    }

    #[test]
    fn test_braking_distance_calculation() {
        let template = Arc::new(LocomotorTemplate::new_wheeled("TestVehicle".to_string()));
        let loco = Locomotor::new(template);

        let current_speed = 10.0;
        let desired_speed = 0.0;
        let braking = loco.get_braking();

        let slow_down_dist = Locomotor::calc_slow_down_dist(current_speed, desired_speed, braking);

        // Should have a reasonable braking distance
        assert!(slow_down_dist > 0.0);
        assert!(slow_down_dist < 100.0); // Should not be excessively long
    }
}
