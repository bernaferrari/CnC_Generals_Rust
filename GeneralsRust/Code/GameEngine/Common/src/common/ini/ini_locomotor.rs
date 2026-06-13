////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_locomotor.rs
//! Author: Steven Johnson, Feb 2002 (Converted to Rust)
//! Desc: Locomotor Template parsing and management
//!
//! Matches C++ Locomotor.h and Locomotor.cpp field parse table

use once_cell::sync::OnceCell;
use std::collections::{BTreeMap, HashMap};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;
use crate::common::name_key_generator::{NameKeyGenerator, NameKeyType};

/// Result type for locomotor operations
pub type LocomotorResult<T> = Result<T, LocomotorError>;

/// Errors that can occur during locomotor parsing
#[derive(Debug, Clone, PartialEq)]
pub enum LocomotorError {
    InvalidName,
    InvalidAppearance,
    InvalidBehaviorZ,
    InvalidPriority,
    ParseError(String),
    NotFound,
    AlreadyExists,
}

impl std::fmt::Display for LocomotorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocomotorError::InvalidName => write!(f, "Invalid locomotor name"),
            LocomotorError::InvalidAppearance => write!(f, "Invalid locomotor appearance"),
            LocomotorError::InvalidBehaviorZ => write!(f, "Invalid Z-axis behavior"),
            LocomotorError::InvalidPriority => write!(f, "Invalid movement priority"),
            LocomotorError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            LocomotorError::NotFound => write!(f, "Locomotor not found"),
            LocomotorError::AlreadyExists => write!(f, "Locomotor already exists"),
        }
    }
}

impl std::error::Error for LocomotorError {}

/// Locomotor appearance types
/// Matches C++ enum LocomotorAppearance from Locomotor.h lines 30-41
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocomotorAppearance {
    LegsTWO,    // TWO_LEGS
    WheelsFOUR, // FOUR_WHEELS
    Treads,     // TREADS
    Hover,      // HOVER
    Thrust,     // THRUST
    Wings,      // WINGS
    Climber,    // CLIMBER - human climber, backs down cliffs
    Other,      // OTHER
    Motorcycle, // MOTORCYCLE
}

impl LocomotorAppearance {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "TWO_LEGS" => Some(Self::LegsTWO),
            "FOUR_WHEELS" => Some(Self::WheelsFOUR),
            "TREADS" => Some(Self::Treads),
            "HOVER" => Some(Self::Hover),
            "THRUST" => Some(Self::Thrust),
            "WINGS" => Some(Self::Wings),
            "CLIMBER" => Some(Self::Climber),
            "OTHER" => Some(Self::Other),
            "MOTORCYCLE" => Some(Self::Motorcycle),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::LegsTWO => "TWO_LEGS",
            Self::WheelsFOUR => "FOUR_WHEELS",
            Self::Treads => "TREADS",
            Self::Hover => "HOVER",
            Self::Thrust => "THRUST",
            Self::Wings => "WINGS",
            Self::Climber => "CLIMBER",
            Self::Other => "OTHER",
            Self::Motorcycle => "MOTORCYCLE",
        }
    }
}

/// Locomotor movement priority in groups
/// Matches C++ enum LocomotorPriority from Locomotor.h lines 43-48
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocomotorPriority {
    MovesBack = 0,   // In a group, moves toward the back
    MovesMiddle = 1, // In a group, stays in the middle
    MovesFront = 2,  // In a group, moves toward the front
}

impl LocomotorPriority {
    pub fn from_index(idx: i32) -> Option<Self> {
        match idx {
            0 => Some(Self::MovesBack),
            1 => Some(Self::MovesMiddle),
            2 => Some(Self::MovesFront),
            _ => None,
        }
    }
}

/// Z-axis behavior types
/// Matches C++ enum LocomotorBehaviorZ from Locomotor.h lines 68-78
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocomotorBehaviorZ {
    NoZMotiveForce,        // Does whatever physics tells it, but has no z-force of its own
    SeaLevel,              // Keep at surface-of-water level
    SurfaceRelativeHeight, // Try to follow a specific height relative to terrain/water height
    AbsoluteHeight,        // Try to follow a specific height regardless of terrain/water height
    FixedSurfaceRelativeHeight, // Stays fixed at surface-rel height, regardless of physics
    FixedAbsoluteHeight,   // Stays fixed at absolute height, regardless of physics
    RelativeToGroundAndBuildings, // Stays fixed at surface-rel height including buildings, regardless of physics
    SmoothRelativeToHighestLayer, // Try to follow a height relative to the highest layer
}

impl LocomotorBehaviorZ {
    pub fn from_string(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "NO_Z_MOTIVE_FORCE" => Some(Self::NoZMotiveForce),
            "SEA_LEVEL" => Some(Self::SeaLevel),
            "SURFACE_RELATIVE_HEIGHT" => Some(Self::SurfaceRelativeHeight),
            "ABSOLUTE_HEIGHT" => Some(Self::AbsoluteHeight),
            "FIXED_SURFACE_RELATIVE_HEIGHT" => Some(Self::FixedSurfaceRelativeHeight),
            "FIXED_ABSOLUTE_HEIGHT" => Some(Self::FixedAbsoluteHeight),
            "FIXED_RELATIVE_TO_GROUND_AND_BUILDINGS" => Some(Self::RelativeToGroundAndBuildings),
            "RELATIVE_TO_HIGHEST_LAYER" => Some(Self::SmoothRelativeToHighestLayer),
            _ => None,
        }
    }
}

/// Locomotor surface type flags
/// Matches C++ LocomotorSurfaceTypeMask
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LocomotorSurfaceTypeMask(pub u32);

impl LocomotorSurfaceTypeMask {
    pub const GROUND: u32 = 1 << 0;
    pub const WATER: u32 = 1 << 1;
    pub const CLIFF: u32 = 1 << 2;
    pub const AIR: u32 = 1 << 3;
    pub const RUBBLE: u32 = 1 << 4;

    pub fn new() -> Self {
        Self(0)
    }

    pub fn has_surface(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    pub fn add_surface(&mut self, flag: u32) {
        self.0 |= flag;
    }
}

impl Default for LocomotorSurfaceTypeMask {
    fn default() -> Self {
        Self::new()
    }
}

/// Locomotor template definition
/// Matches C++ LocomotorTemplate from Locomotor.h lines 97-192
/// Field parse table from Locomotor.cpp lines 417-488
#[derive(Debug, Clone)]
pub struct LocomotorTemplate {
    pub name: AsciiString,

    // Basic movement parameters
    pub surfaces: LocomotorSurfaceTypeMask, // Kinds of surfaces we can use
    pub max_speed: f32,                     // Max speed (dist/frame)
    pub max_speed_damaged: f32,             // Max speed when "damaged"
    pub min_speed: f32,                     // We should never brake past this
    pub max_turn_rate: f32,                 // Max rate at which we can turn (rads/frame)
    pub max_turn_rate_damaged: f32,         // Max turn rate when "damaged"
    pub acceleration: f32,                  // Max acceleration (dist/(frame*frame))
    pub acceleration_damaged: f32,          // Max acceleration when damaged
    pub lift: f32,                          // Max lifting acceleration (flying objects only)
    pub lift_damaged: f32,                  // Max lift when damaged
    pub braking: f32,                       // Max braking (deceleration)
    pub min_turn_speed: f32,                // Must be going >= this speed to turn

    // Height and positioning
    pub preferred_height: f32,         // Our preferred height (if flying)
    pub preferred_height_damping: f32, // How aggressively to adjust to preferred height
    pub circling_radius: f32,          // For flying things, radius at which they circle
    pub speed_limit_z: f32,            // Try to avoid going up/down faster than this
    pub extra_2d_friction: f32,        // Extra 2D friction to apply (via Physics)
    pub max_thrust_angle: f32,         // THRUST locos only: how much we deflect thrust angle

    // Behavior and appearance
    pub behavior_z: LocomotorBehaviorZ,   // Z-axis behavior
    pub appearance: LocomotorAppearance,  // How we should animate this motion
    pub move_priority: LocomotorPriority, // Where we move - front, middle, back

    // Physics simulation parameters
    pub accel_pitch_limit: f32, // Maximum pitch up under acceleration (including recoil)
    pub decel_pitch_limit: f32, // Maximum pitch down under deceleration (including recoil)
    pub bounce_kick: f32,       // How much rough terrain "bounces" a wheel up
    pub pitch_stiffness: f32,   // How stiff springs are forward & back
    pub roll_stiffness: f32,    // How stiff springs are side to side
    pub pitch_damping: f32,     // How good shock absorbers are (pitch)
    pub roll_damping: f32,      // How good shock absorbers are (roll)
    pub pitch_by_z_vel_coef: f32, // How much we pitch in response to z-speed
    pub thrust_roll: f32,       // Thrust roll around X axis
    pub wobble_rate: f32,       // How fast thrust things "wobble"
    pub min_wobble: f32,        // Minimum thrust wobble
    pub max_wobble: f32,        // Maximum thrust wobble
    pub forward_vel_coef: f32,  // How much we pitch in response to speed
    pub lateral_vel_coef: f32,  // How much we roll in response to speed
    pub forward_accel_coef: f32, // How much we pitch in response to acceleration
    pub lateral_accel_coef: f32, // How much we roll in response to acceleration
    pub uniform_axial_damping: f32, // For attenuating pitch and roll rates
    pub turn_pivot_offset: f32, // Should we pivot around non-center? (-1.0=rear, 0=center, 1.0=front)
    pub airborne_targeting_height: i32, // Height transition at which to mark as AA target

    // Movement accuracy
    pub close_enough_dist: f32, // How close to approach end of path before stopping
    pub is_close_enough_dist_3d: bool, // Is that calculation 3D
    pub ultra_accurate_slide_into_place_factor: f32, // How much we can fudge turning when ultra-accurate

    // Special behaviors
    pub locomotor_works_when_dead: bool, // Should locomotor continue working when object is "dead"
    pub allow_motive_force_while_airborne: bool, // Can we apply motive when airborne
    pub apply_2d_friction_when_airborne: bool, // Apply "2d friction" even when airborne
    pub downhill_only: bool,             // Pinewood derby, moves only by gravity pulling downhill
    pub stick_to_ground: bool,           // If true, can't leave ground
    pub can_move_backward: bool,         // If true, can move backwards

    // Suspension system
    pub has_suspension: bool, // If true, calculate 4 wheel independent suspension
    pub maximum_wheel_extension: f32, // Maximum distance wheels can move down (negative value)
    pub maximum_wheel_compression: f32, // Maximum distance wheels can move up (positive value)
    pub wheel_turn_angle: f32, // How far the front wheels can turn

    // Wander locomotor fields
    pub wander_width_factor: f32,
    pub wander_length_factor: f32,
    pub wander_about_point_radius: f32,

    // Correction parameters
    pub rudder_correction_degree: f32,
    pub rudder_correction_rate: f32,
    pub elevator_correction_degree: f32,
    pub elevator_correction_rate: f32,
}

impl LocomotorTemplate {
    /// Create a new locomotor template with default values
    /// Matches C++ LocomotorTemplate::LocomotorTemplate() from Locomotor.cpp lines 268-334
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            surfaces: LocomotorSurfaceTypeMask::new(),
            max_speed: 0.0,
            max_speed_damaged: -1.0, // -1 means "use max_speed"
            min_speed: 0.0,
            max_turn_rate: 0.0,
            max_turn_rate_damaged: -1.0, // -1 means "use max_turn_rate"
            acceleration: 0.0,
            acceleration_damaged: -1.0, // -1 means "use acceleration"
            lift: 0.0,
            lift_damaged: -1.0, // -1 means "use lift"
            braking: 0.0,
            min_turn_speed: 0.0,
            preferred_height: 0.0,
            preferred_height_damping: 1.0,
            circling_radius: 0.0,
            speed_limit_z: 1000000.0, // Very large default
            extra_2d_friction: 0.0,
            max_thrust_angle: 0.0,
            behavior_z: LocomotorBehaviorZ::NoZMotiveForce,
            appearance: LocomotorAppearance::Other,
            move_priority: LocomotorPriority::MovesMiddle,
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
            close_enough_dist: 1.0,
            is_close_enough_dist_3d: false,
            ultra_accurate_slide_into_place_factor: 0.0,
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
            wander_length_factor: 1.0,
            wander_about_point_radius: 0.0,
            rudder_correction_degree: 0.0,
            rudder_correction_rate: 0.0,
            elevator_correction_degree: 0.0,
            elevator_correction_rate: 0.0,
        }
    }

    /// Validate and fix up locomotor template after parsing
    /// Matches C++ LocomotorTemplate::validate() from Locomotor.cpp lines 343-406
    pub fn validate(&mut self) -> LocomotorResult<()> {
        // For 'damaged' stuff that was omitted, set to be same as 'undamaged'
        if self.max_speed_damaged < 0.0 {
            self.max_speed_damaged = self.max_speed;
        }

        if self.max_turn_rate_damaged < 0.0 {
            self.max_turn_rate_damaged = self.max_turn_rate;
        }

        if self.acceleration_damaged < 0.0 {
            self.acceleration_damaged = self.acceleration;
        }

        if self.lift_damaged < 0.0 {
            self.lift_damaged = self.lift;
        }

        // Wings validation
        if self.appearance == LocomotorAppearance::Wings {
            if self.min_speed <= 0.0 {
                eprintln!("WINGS should always have positive minSpeeds (otherwise, they hover)");
                self.min_speed = 0.01;
            }
            if self.min_turn_speed <= 0.0 {
                eprintln!("WINGS should always have positive minTurnSpeed");
                self.min_turn_speed = 0.01;
            }
        }

        // Thrust validation
        if self.appearance == LocomotorAppearance::Thrust {
            if self.behavior_z != LocomotorBehaviorZ::NoZMotiveForce
                || self.lift != 0.0
                || self.lift_damaged != 0.0
            {
                return Err(LocomotorError::ParseError(
                    "THRUST locos may not use ZAxisBehavior or lift!".to_string(),
                ));
            }

            if self.max_speed <= 0.0 {
                eprintln!("THRUST locos may not have zero max_speed; healing...");
                self.max_speed = 0.01;
            }
            if self.max_speed_damaged <= 0.0 {
                eprintln!("THRUST locos may not have zero max_speed_damaged; healing...");
                self.max_speed_damaged = 0.01;
            }
            if self.min_speed <= 0.0 {
                eprintln!("THRUST locos may not have zero min_speed; healing...");
                self.min_speed = 0.01;
            }
        }

        Ok(())
    }
}

/// Locomotor template store
pub struct LocomotorStore {
    templates: BTreeMap<NameKeyType, LocomotorTemplate>,
}

impl LocomotorStore {
    pub fn new() -> Self {
        Self {
            templates: BTreeMap::new(),
        }
    }

    fn template_key(name: &AsciiString) -> NameKeyType {
        NameKeyGenerator::name_to_key(name.as_str())
    }

    fn template_key_from_str(name: &str) -> NameKeyType {
        NameKeyGenerator::name_to_key(name)
    }

    pub fn add_template(&mut self, template: LocomotorTemplate) -> LocomotorResult<()> {
        let key = Self::template_key(&template.name);
        if self.templates.contains_key(&key) {
            // In C++, this would be an override situation
            self.templates.insert(key, template);
            Ok(())
        } else {
            self.templates.insert(key, template);
            Ok(())
        }
    }

    pub fn find_template(&self, name: &str) -> Option<&LocomotorTemplate> {
        self.templates.get(&Self::template_key_from_str(name))
    }

    pub fn find_template_mut(&mut self, name: &str) -> Option<&mut LocomotorTemplate> {
        self.templates.get_mut(&Self::template_key_from_str(name))
    }

    pub fn get_template_names(&self) -> Vec<&AsciiString> {
        self.templates
            .values()
            .map(|template| &template.name)
            .collect()
    }
}

impl Default for LocomotorStore {
    fn default() -> Self {
        Self::new()
    }
}

/// Global locomotor store
static LOCOMOTOR_STORE: OnceCell<RwLock<LocomotorStore>> = OnceCell::new();

/// Get the global locomotor store
pub fn get_locomotor_store() -> RwLockReadGuard<'static, LocomotorStore> {
    LOCOMOTOR_STORE
        .get_or_init(|| RwLock::new(LocomotorStore::new()))
        .read()
        .unwrap()
}

/// Get mutable access to the global locomotor store
pub fn get_locomotor_store_mut() -> RwLockWriteGuard<'static, LocomotorStore> {
    LOCOMOTOR_STORE
        .get_or_init(|| RwLock::new(LocomotorStore::new()))
        .write()
        .unwrap()
}

fn parse_cpp_bool(field_name: &str, value: &str) -> LocomotorResult<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" => Ok(true),
        "no" => Ok(false),
        _ => Err(LocomotorError::ParseError(format!(
            "{}: invalid boolean token '{}' (expected Yes or No)",
            field_name, value
        ))),
    }
}

/// Parse a locomotor template definition from INI
/// Matches C++ LocomotorStore::parseLocomotorTemplateDefinition from Locomotor.cpp lines 529-569
pub fn parse_locomotor_template_definition(
    name: &str,
    properties: &HashMap<String, String>,
) -> LocomotorResult<LocomotorTemplate> {
    let mut template = LocomotorTemplate::new(AsciiString::from(name));

    // Parse all fields from the properties map
    // Matches field parse table from Locomotor.cpp lines 419-485

    for (key, value) in properties {
        match key.as_str() {
            "Speed" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("Speed: {}", e)))?;
                template.max_speed = parsed / 30.0;
            }
            "SpeedDamaged" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("SpeedDamaged: {}", e)))?;
                template.max_speed_damaged = parsed / 30.0;
            }
            "TurnRate" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("TurnRate: {}", e)))?;
                template.max_turn_rate = parsed * std::f32::consts::PI / (180.0 * 30.0);
            }
            "TurnRateDamaged" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("TurnRateDamaged: {}", e)))?;
                template.max_turn_rate_damaged = parsed * std::f32::consts::PI / (180.0 * 30.0);
            }
            "Acceleration" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("Acceleration: {}", e)))?;
                template.acceleration = parsed / 900.0;
            }
            "AccelerationDamaged" => {
                let parsed: f32 = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("AccelerationDamaged: {}", e))
                })?;
                template.acceleration_damaged = parsed / 900.0;
            }
            "Lift" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("Lift: {}", e)))?;
                template.lift = parsed / 900.0;
            }
            "LiftDamaged" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("LiftDamaged: {}", e)))?;
                template.lift_damaged = parsed / 900.0;
            }
            "Braking" => {
                let parsed: f32 = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("Braking: {}", e)))?;
                template.braking = parsed / 900.0;
            }
            "MinSpeed" => {
                template.min_speed = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("MinSpeed: {}", e)))?
            }
            "MinTurnSpeed" => {
                template.min_turn_speed = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("MinTurnSpeed: {}", e)))?
            }
            "PreferredHeight" => {
                template.preferred_height = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("PreferredHeight: {}", e)))?
            }
            "PreferredHeightDamping" => {
                template.preferred_height_damping = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("PreferredHeightDamping: {}", e))
                })?
            }
            "CirclingRadius" => {
                template.circling_radius = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("CirclingRadius: {}", e)))?
            }
            "SpeedLimitZ" => {
                template.speed_limit_z = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("SpeedLimitZ: {}", e)))?
            }
            "MaxThrustAngle" => {
                template.max_thrust_angle = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("MaxThrustAngle: {}", e)))?
            }
            "ZAxisBehavior" => {
                template.behavior_z = LocomotorBehaviorZ::from_string(value)
                    .ok_or_else(|| LocomotorError::InvalidBehaviorZ)?;
            }
            "Appearance" => {
                template.appearance = LocomotorAppearance::from_string(value)
                    .ok_or_else(|| LocomotorError::InvalidAppearance)?;
            }
            "AccelerationPitchLimit" => {
                template.accel_pitch_limit = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("AccelerationPitchLimit: {}", e))
                })?
            }
            "DecelerationPitchLimit" => {
                template.decel_pitch_limit = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("DecelerationPitchLimit: {}", e))
                })?
            }
            "BounceAmount" => {
                template.bounce_kick = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("BounceAmount: {}", e)))?
            }
            "PitchStiffness" => {
                template.pitch_stiffness = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("PitchStiffness: {}", e)))?
            }
            "RollStiffness" => {
                template.roll_stiffness = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("RollStiffness: {}", e)))?
            }
            "PitchDamping" => {
                template.pitch_damping = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("PitchDamping: {}", e)))?
            }
            "RollDamping" => {
                template.roll_damping = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("RollDamping: {}", e)))?
            }
            "ThrustRoll" => {
                template.thrust_roll = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("ThrustRoll: {}", e)))?
            }
            "ThrustWobbleRate" => {
                template.wobble_rate = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("ThrustWobbleRate: {}", e)))?
            }
            "ThrustMinWobble" => {
                template.min_wobble = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("ThrustMinWobble: {}", e)))?
            }
            "ThrustMaxWobble" => {
                template.max_wobble = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("ThrustMaxWobble: {}", e)))?
            }
            "PitchInDirectionOfZVelFactor" => {
                template.pitch_by_z_vel_coef = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("PitchInDirectionOfZVelFactor: {}", e))
                })?
            }
            "ForwardVelocityPitchFactor" => {
                template.forward_vel_coef = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("ForwardVelocityPitchFactor: {}", e))
                })?
            }
            "LateralVelocityRollFactor" => {
                template.lateral_vel_coef = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("LateralVelocityRollFactor: {}", e))
                })?
            }
            "ForwardAccelerationPitchFactor" => {
                template.forward_accel_coef = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("ForwardAccelerationPitchFactor: {}", e))
                })?
            }
            "LateralAccelerationRollFactor" => {
                template.lateral_accel_coef = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("LateralAccelerationRollFactor: {}", e))
                })?
            }
            "UniformAxialDamping" => {
                template.uniform_axial_damping = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("UniformAxialDamping: {}", e))
                })?
            }
            "TurnPivotOffset" => {
                template.turn_pivot_offset = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("TurnPivotOffset: {}", e)))?
            }
            "Apply2DFrictionWhenAirborne" => {
                template.apply_2d_friction_when_airborne = parse_cpp_bool(key, value)?
            }
            "DownhillOnly" => {
                template.downhill_only = parse_cpp_bool(key, value)?;
            }
            "AllowAirborneMotiveForce" => {
                template.allow_motive_force_while_airborne = parse_cpp_bool(key, value)?
            }
            "LocomotorWorksWhenDead" => {
                template.locomotor_works_when_dead = parse_cpp_bool(key, value)?
            }
            "AirborneTargetingHeight" => {
                template.airborne_targeting_height = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("AirborneTargetingHeight: {}", e))
                })?
            }
            "StickToGround" => {
                template.stick_to_ground = parse_cpp_bool(key, value)?;
            }
            "CanMoveBackwards" => template.can_move_backward = parse_cpp_bool(key, value)?,
            "HasSuspension" => {
                template.has_suspension = parse_cpp_bool(key, value)?;
            }
            "FrontWheelTurnAngle" => {
                template.wheel_turn_angle = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("FrontWheelTurnAngle: {}", e))
                })?
            }
            "MaximumWheelExtension" => {
                template.maximum_wheel_extension = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("MaximumWheelExtension: {}", e))
                })?
            }
            "MaximumWheelCompression" => {
                template.maximum_wheel_compression = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("MaximumWheelCompression: {}", e))
                })?
            }
            "CloseEnoughDist" => {
                template.close_enough_dist = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("CloseEnoughDist: {}", e)))?
            }
            "CloseEnoughDist3D" => template.is_close_enough_dist_3d = parse_cpp_bool(key, value)?,
            "SlideIntoPlaceTime" => {
                template.ultra_accurate_slide_into_place_factor = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("SlideIntoPlaceTime: {}", e)))?
            }
            "WanderWidthFactor" => {
                template.wander_width_factor = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("WanderWidthFactor: {}", e)))?
            }
            "WanderLengthFactor" => {
                template.wander_length_factor = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("WanderLengthFactor: {}", e)))?
            }
            "WanderAboutPointRadius" => {
                template.wander_about_point_radius = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("WanderAboutPointRadius: {}", e))
                })?
            }
            "RudderCorrectionDegree" => {
                template.rudder_correction_degree = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("RudderCorrectionDegree: {}", e))
                })?
            }
            "RudderCorrectionRate" => {
                template.rudder_correction_rate = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("RudderCorrectionRate: {}", e))
                })?
            }
            "ElevatorCorrectionDegree" => {
                template.elevator_correction_degree = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("ElevatorCorrectionDegree: {}", e))
                })?
            }
            "ElevatorCorrectionRate" => {
                template.elevator_correction_rate = value.parse().map_err(|e| {
                    LocomotorError::ParseError(format!("ElevatorCorrectionRate: {}", e))
                })?
            }
            "Surfaces" => {
                for part in value.split('|') {
                    match part.trim().to_uppercase().as_str() {
                        "GROUND" => template
                            .surfaces
                            .add_surface(LocomotorSurfaceTypeMask::GROUND),
                        "WATER" => template
                            .surfaces
                            .add_surface(LocomotorSurfaceTypeMask::WATER),
                        "CLIFF" => template
                            .surfaces
                            .add_surface(LocomotorSurfaceTypeMask::CLIFF),
                        "AIR" => template.surfaces.add_surface(LocomotorSurfaceTypeMask::AIR),
                        "RUBBLE" => template
                            .surfaces
                            .add_surface(LocomotorSurfaceTypeMask::RUBBLE),
                        _ => {}
                    }
                }
            }
            "Extra2DFriction" => {
                template.extra_2d_friction = value
                    .parse()
                    .map_err(|e| LocomotorError::ParseError(format!("Extra2DFriction: {}", e)))?
            }
            "GroupMovementPriority" => match value.trim().to_uppercase().as_str() {
                "MOVES_BACK" | "BACK" => template.move_priority = LocomotorPriority::MovesBack,
                "MOVES_MIDDLE" | "MIDDLE" => {
                    template.move_priority = LocomotorPriority::MovesMiddle
                }
                "MOVES_FRONT" | "FRONT" => template.move_priority = LocomotorPriority::MovesFront,
                _ => {
                    let idx: i32 = value.parse().map_err(|e| {
                        LocomotorError::ParseError(format!("GroupMovementPriority: {}", e))
                    })?;
                    template.move_priority = LocomotorPriority::from_index(idx)
                        .ok_or(LocomotorError::InvalidPriority)?;
                }
            },
            _ => {
                // Unknown field - log warning but don't fail
                eprintln!("Warning: Unknown locomotor field: {}", key);
            }
        }
    }

    // Validate the template
    template.validate()?;

    Ok(template)
}

/// Error type for bulk locomotor INI loading.
#[derive(Debug)]
pub enum LocomotorLoadError {
    Parse { line: usize, message: String },
}

impl std::fmt::Display for LocomotorLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LocomotorLoadError::Parse { line, message } => {
                write!(f, "Locomotor parse error at line {}: {}", line, message)
            }
        }
    }
}

impl std::error::Error for LocomotorLoadError {}

/// Load locomotor templates from raw INI text content.
///
/// Parses the `Locomotor <Name> ... End` block format and populates the
/// global `LocomotorStore` via `get_locomotor_store_mut()`.
///
/// Matches C++ `TheLocomotorStore->load()` / `INIParser::loadFromData()`
/// used at `GameEngine.cpp:442`.
///
/// Returns the number of templates successfully loaded.
pub fn load_locomotors_from_str(content: &str) -> Result<usize, LocomotorLoadError> {
    let mut count = 0usize;
    let mut current_name: Option<String> = None;
    let mut current_props: HashMap<String, String> = HashMap::new();

    for (idx, raw_line) in content.lines().enumerate() {
        let line_no = idx + 1;
        // Strip comments (everything after ';')
        let line = raw_line.split(';').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        // Detect "Locomotor <Name>" block header
        if let Some(rest) = line.strip_prefix("Locomotor ") {
            let name = rest.trim();
            if name.is_empty() {
                return Err(LocomotorLoadError::Parse {
                    line: line_no,
                    message: "Missing locomotor name after 'Locomotor' keyword".into(),
                });
            }
            if current_name.is_some() {
                return Err(LocomotorLoadError::Parse {
                    line: line_no,
                    message: format!(
                        "Nested Locomotor block encountered (still inside '{}')",
                        current_name.as_deref().unwrap_or("?")
                    ),
                });
            }
            current_name = Some(name.to_string());
            current_props.clear();
            continue;
        }

        // Detect "End" to close the current block
        if line.eq_ignore_ascii_case("End") {
            if let Some(name) = current_name.take() {
                match parse_locomotor_template_definition(&name, &current_props) {
                    Ok(template) => {
                        get_locomotor_store_mut().add_template(template).ok();
                        count += 1;
                    }
                    Err(e) => {
                        return Err(LocomotorLoadError::Parse {
                            line: line_no,
                            message: format!("Failed to parse locomotor '{}': {}", name, e),
                        });
                    }
                }
                current_props.clear();
            }
            // stray "End" outside a block is tolerated (matches C++ behavior)
            continue;
        }

        // Inside a block: parse "Key = Value" lines
        if current_name.is_some() {
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value = line[eq_pos + 1..].trim().to_string();
                current_props.insert(key, value);
            }
            // Lines without '=' inside a block are silently ignored
        }
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_locomotor_appearance_parsing() {
        assert_eq!(
            LocomotorAppearance::from_string("TWO_LEGS"),
            Some(LocomotorAppearance::LegsTWO)
        );
        assert_eq!(
            LocomotorAppearance::from_string("FOUR_WHEELS"),
            Some(LocomotorAppearance::WheelsFOUR)
        );
        assert_eq!(
            LocomotorAppearance::from_string("WINGS"),
            Some(LocomotorAppearance::Wings)
        );
        assert_eq!(LocomotorAppearance::from_string("INVALID"), None);
    }

    #[test]
    fn test_locomotor_behavior_z_parsing() {
        assert_eq!(
            LocomotorBehaviorZ::from_string("NO_Z_MOTIVE_FORCE"),
            Some(LocomotorBehaviorZ::NoZMotiveForce)
        );
        assert_eq!(
            LocomotorBehaviorZ::from_string("SEA_LEVEL"),
            Some(LocomotorBehaviorZ::SeaLevel)
        );
        assert_eq!(LocomotorBehaviorZ::from_string("INVALID"), None);
    }

    #[test]
    fn test_locomotor_template_defaults() {
        let template = LocomotorTemplate::new(AsciiString::from("TestLoco"));

        assert_eq!(template.name.to_str(), "TestLoco");
        assert_eq!(template.max_speed, 0.0);
        assert_eq!(template.max_speed_damaged, -1.0);
        assert_eq!(template.preferred_height_damping, 1.0);
        assert_eq!(template.appearance, LocomotorAppearance::Other);
        assert_eq!(template.behavior_z, LocomotorBehaviorZ::NoZMotiveForce);
    }

    #[test]
    fn test_locomotor_validation() {
        let mut template = LocomotorTemplate::new(AsciiString::from("Test"));
        template.max_speed = 10.0;
        template.max_speed_damaged = -1.0;

        template.validate().unwrap();

        // Should have been fixed up
        assert_eq!(template.max_speed_damaged, 10.0);
    }

    #[test]
    fn test_locomotor_wings_validation() {
        let mut template = LocomotorTemplate::new(AsciiString::from("Plane"));
        template.appearance = LocomotorAppearance::Wings;
        template.min_speed = 0.0; // Invalid for wings

        template.validate().unwrap();

        // Should have been healed
        assert!(template.min_speed > 0.0);
    }

    #[test]
    fn test_locomotor_thrust_validation() {
        let mut template = LocomotorTemplate::new(AsciiString::from("Rocket"));
        template.appearance = LocomotorAppearance::Thrust;
        template.behavior_z = LocomotorBehaviorZ::SeaLevel; // Invalid for thrust

        let result = template.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_locomotor_store() {
        let mut store = LocomotorStore::new();
        let template = LocomotorTemplate::new(AsciiString::from("TestLoco"));

        store.add_template(template).unwrap();

        let found = store.find_template("TestLoco");
        assert!(found.is_some());
        assert_eq!(found.unwrap().name.to_str(), "TestLoco");
    }

    #[test]
    fn locomotor_store_enumerates_in_name_key_order() {
        NameKeyGenerator::reset();

        let mut store = LocomotorStore::new();
        for name in ["GammaLoco", "AlphaLoco", "BetaLoco"] {
            store
                .add_template(LocomotorTemplate::new(AsciiString::from(name)))
                .unwrap();
        }

        let names: Vec<&str> = store
            .get_template_names()
            .into_iter()
            .map(|name| name.as_str())
            .collect();

        assert_eq!(names, vec!["GammaLoco", "AlphaLoco", "BetaLoco"]);
    }

    #[test]
    fn test_locomotor_unit_conversions() {
        let mut props = HashMap::new();
        props.insert("Speed".to_string(), "300".to_string());
        props.insert("TurnRate".to_string(), "180".to_string());
        props.insert("Acceleration".to_string(), "900".to_string());
        props.insert("Appearance".to_string(), "TREADS".to_string());

        let template = parse_locomotor_template_definition("TestConv", &props).unwrap();

        // Speed: 300 / 30 = 10.0
        assert!(
            (template.max_speed - 10.0).abs() < 1e-6,
            "Speed should be 10.0, got {}",
            template.max_speed
        );

        // TurnRate: 180 * PI / (180 * 30) = PI / 30
        let expected_turn = std::f32::consts::PI / 30.0;
        assert!(
            (template.max_turn_rate - expected_turn).abs() < 1e-5,
            "TurnRate should be PI/30 ({:.6}), got {:.6}",
            expected_turn,
            template.max_turn_rate
        );

        // Acceleration: 900 / 900 = 1.0
        assert!(
            (template.acceleration - 1.0).abs() < 1e-6,
            "Acceleration should be 1.0, got {}",
            template.acceleration
        );
    }

    #[test]
    fn locomotor_cpp_bool_fields_accept_yes_no() {
        let mut props = HashMap::new();
        props.insert("Apply2DFrictionWhenAirborne".to_string(), "Yes".to_string());
        props.insert("DownhillOnly".to_string(), "No".to_string());
        props.insert("AllowAirborneMotiveForce".to_string(), "Yes".to_string());
        props.insert("LocomotorWorksWhenDead".to_string(), "No".to_string());
        props.insert("StickToGround".to_string(), "Yes".to_string());
        props.insert("CanMoveBackwards".to_string(), "No".to_string());
        props.insert("HasSuspension".to_string(), "Yes".to_string());
        props.insert("CloseEnoughDist3D".to_string(), "No".to_string());

        let template = parse_locomotor_template_definition("BoolLoco", &props).unwrap();

        assert!(template.apply_2d_friction_when_airborne);
        assert!(!template.downhill_only);
        assert!(template.allow_motive_force_while_airborne);
        assert!(!template.locomotor_works_when_dead);
        assert!(template.stick_to_ground);
        assert!(!template.can_move_backward);
        assert!(template.has_suspension);
        assert!(!template.is_close_enough_dist_3d);
    }

    #[test]
    fn locomotor_cpp_bool_fields_reject_invalid_tokens() {
        for field in [
            "Apply2DFrictionWhenAirborne",
            "DownhillOnly",
            "AllowAirborneMotiveForce",
            "LocomotorWorksWhenDead",
            "StickToGround",
            "CanMoveBackwards",
            "HasSuspension",
            "CloseEnoughDist3D",
        ] {
            let mut props = HashMap::new();
            props.insert(field.to_string(), "maybe".to_string());

            assert!(
                parse_locomotor_template_definition("BadBoolLoco", &props).is_err(),
                "{field} should reject invalid C++ bool token"
            );
        }
    }
}
