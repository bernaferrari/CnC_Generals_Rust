//! Game common definitions and utility functions
//!
//! This module provides common game-related constants, enums, and utility functions
//! that are shared across the game engine.

use std::f32::consts::PI;

/// Veterancy level names corresponding to VeterancyLevel enum
pub const VETERANCY_NAMES: [&str; 5] = [
    "REGULAR", "VETERAN", "ELITE", "HEROIC", "", // NULL terminator equivalent
];

/// Relationship names corresponding to Relationship enum
pub const RELATIONSHIP_NAMES: [&str; 4] = [
    "ENEMIES", "NEUTRAL", "ALLIES", "", // NULL terminator equivalent
];

/// Game logic frame rate constants
pub const LOGIC_FRAMES_PER_SECOND: i32 = 30;
pub const MSEC_PER_SECOND: i32 = 1000;

/// Conversion constants for frame/time calculations
pub const LOGIC_FRAMES_PER_MSEC_REAL: f32 =
    (LOGIC_FRAMES_PER_SECOND as f32) / (MSEC_PER_SECOND as f32);
pub const MSEC_PER_LOGIC_FRAME_REAL: f32 =
    (MSEC_PER_SECOND as f32) / (LOGIC_FRAMES_PER_SECOND as f32);
pub const LOGIC_FRAMES_PER_SECONDS_REAL: f32 = LOGIC_FRAMES_PER_SECOND as f32;
pub const SECONDS_PER_LOGIC_FRAME_REAL: f32 = 1.0 / LOGIC_FRAMES_PER_SECONDS_REAL;

/// Maximum number of players in a game
pub const MAX_PLAYER_COUNT: usize = 16;

/// Player mask type for representing sets of players
pub type PlayerMaskType = u16;

/// Player mask constants
pub const PLAYERMASK_ALL: PlayerMaskType = 0xffff;
pub const PLAYERMASK_NONE: PlayerMaskType = 0x0;

/// Global general types
pub const MAX_GLOBAL_GENERAL_TYPES: usize = 9;
pub const GLOBAL_GENERAL_BEGIN: usize = 5;
pub const GLOBAL_GENERAL_END: usize = GLOBAL_GENERAL_BEGIN + MAX_GLOBAL_GENERAL_TYPES - 1;

/// Time constants
pub const NEVER: i32 = 0;
pub const FOREVER: i32 = 0x3fffffff;

/// Game difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameDifficulty {
    Easy,
    Normal,
    Hard,
}

impl GameDifficulty {
    pub const COUNT: usize = 3;
}

/// Player types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlayerType {
    Human,    // Player is human-controlled
    Computer, // Player is computer-controlled
}

impl PlayerType {
    pub const COUNT: usize = 2;
}

/// Cell shroud status for individual partition cells
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellShroudStatus {
    Clear,
    Fogged,
    Shrouded,
}

impl CellShroudStatus {
    pub const COUNT: usize = 3;
}

/// Object shroud status for entire objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObjectShroudStatus {
    Invalid,                 // Indeterminate state, will recompute
    Clear,                   // Object is not shrouded at all (completely visible)
    PartialClear,            // Object is partly clear (rest is shroud or fog)
    Fogged,                  // Object is completely fogged
    Shrouded,                // Object is completely shrouded
    InvalidButPreviousValid, // Indeterminate state, will recompute, BUT previous status is valid
}

impl ObjectShroudStatus {
    pub const COUNT: usize = 6;
}

/// Guard modes for units
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuardMode {
    Normal,               // Normal guard behavior
    GuardWithoutPursuit,  // No pursuit out of guard area
    GuardFlyingUnitsOnly, // Ignore non-flyers
}

/// Veterancy levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran,
    Elite,
    Heroic,
}

impl VeterancyLevel {
    pub const COUNT: usize = 4;
    pub const INVALID: i32 = 4;
    pub const FIRST: VeterancyLevel = VeterancyLevel::Regular;
    pub const LAST: VeterancyLevel = VeterancyLevel::Heroic;

    /// Get the name of this veterancy level
    pub fn name(&self) -> &'static str {
        match self {
            VeterancyLevel::Regular => VETERANCY_NAMES[0],
            VeterancyLevel::Veteran => VETERANCY_NAMES[1],
            VeterancyLevel::Elite => VETERANCY_NAMES[2],
            VeterancyLevel::Heroic => VETERANCY_NAMES[3],
        }
    }
}

/// Command source types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandSourceType {
    FromPlayer = 0,
    FromScript,
    FromAI,
    FromDozer, // Special rare command when dozer originates command to attack a mine
    DefaultSwitchWeapon, // Special case: weapon that can be chosen - default case
}

/// Attack type flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttackFlags(u32);

impl AttackFlags {
    pub const FORCED: AttackFlags = AttackFlags(0x01);
    pub const CONTINUED: AttackFlags = AttackFlags(0x02);
    pub const TUNNEL_NETWORK_GUARD: AttackFlags = AttackFlags(0x04);
}

/// Able to attack types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbleToAttackType {
    NewTarget,             // Can we attack if this is a new target?
    NewTargetForced,       // Can we attack new target via force-fire?
    ContinuedTarget,       // Can we attack continuation of existing attack?
    ContinuedTargetForced, // Can we attack continued target via force-fire?
    TunnelNetworkGuard,    // Special case for tunnel network guards
}

impl AbleToAttackType {
    /// Check if this is a forced attack
    pub fn is_forced_attack(&self) -> bool {
        matches!(
            self,
            AbleToAttackType::NewTargetForced | AbleToAttackType::ContinuedTargetForced
        )
    }

    /// Check if this is a continued attack
    pub fn is_continued_attack(&self) -> bool {
        matches!(
            self,
            AbleToAttackType::ContinuedTarget | AbleToAttackType::ContinuedTargetForced
        )
    }
}

/// Veterancy level flags type
pub type VeterancyLevelFlags = u32;

pub const VETERANCY_LEVEL_FLAGS_ALL: VeterancyLevelFlags = 0xffffffff;
pub const VETERANCY_LEVEL_FLAGS_NONE: VeterancyLevelFlags = 0x00000000;

/// Get veterancy level flag
pub fn get_veterancy_level_flag(flags: VeterancyLevelFlags, level: VeterancyLevel) -> bool {
    (flags & (1u32 << (level as u32))) != 0
}

/// Set veterancy level flag
pub fn set_veterancy_level_flag(
    flags: VeterancyLevelFlags,
    level: VeterancyLevel,
) -> VeterancyLevelFlags {
    flags | (1u32 << (level as u32))
}

/// Clear veterancy level flag
pub fn clear_veterancy_level_flag(
    flags: VeterancyLevelFlags,
    level: VeterancyLevel,
) -> VeterancyLevelFlags {
    flags & !(1u32 << (level as u32))
}

/// Relationships between players/objects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Relationship {
    Enemies = 0,
    Neutral,
    Allies,
}

impl Relationship {
    /// Get the name of this relationship
    pub fn name(&self) -> &'static str {
        match self {
            Relationship::Enemies => RELATIONSHIP_NAMES[0],
            Relationship::Neutral => RELATIONSHIP_NAMES[1],
            Relationship::Allies => RELATIONSHIP_NAMES[2],
        }
    }
}

impl From<i32> for Relationship {
    /// Convert an integer value to a Relationship.
    /// Maps: 0 -> Enemies, 1 -> Neutral, anything else -> Allies
    /// This matches the C++ serialization behavior where values are read as raw integers.
    fn from(value: i32) -> Self {
        match value {
            0 => Relationship::Enemies,
            1 => Relationship::Neutral,
            2 => Relationship::Allies,
            _ => {
                log::warn!("Relationship::from({value}) out of range, defaulting to Allies");
                Relationship::Allies
            }
        }
    }
}

/// Turret types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WhichTurretType {
    Invalid = -1,
    Main = 0,
    Alt,
}

impl WhichTurretType {
    pub const MAX_TURRETS: usize = 2;
}

/// Time/frame conversion functions

/// Convert duration from milliseconds to frames
///
/// Note: This returns a real value, not an int. Most callers will want to
/// call ceil() on the result so that partial frames get converted to full frames.
pub fn convert_duration_from_msecs_to_frames(msec: f32) -> f32 {
    msec * LOGIC_FRAMES_PER_MSEC_REAL
}

/// Convert velocity from per-second to per-frame
pub fn convert_velocity_in_secs_to_frames(dist_per_sec: f32) -> f32 {
    // This looks wrong, but is the correct conversion factor
    dist_per_sec * SECONDS_PER_LOGIC_FRAME_REAL
}

/// Convert acceleration from per-second-squared to per-frame-squared
pub fn convert_acceleration_in_secs_to_frames(dist_per_sec2: f32) -> f32 {
    // This looks wrong, but is the correct conversion factor
    const SEC_PER_LOGIC_FRAME_SQR: f32 =
        SECONDS_PER_LOGIC_FRAME_REAL * SECONDS_PER_LOGIC_FRAME_REAL;
    dist_per_sec2 * SEC_PER_LOGIC_FRAME_SQR
}

/// Convert angular velocity from degrees per second to radians per frame
pub fn convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame(deg_per_sec: f32) -> f32 {
    const RADS_PER_DEGREE: f32 = PI / 180.0;
    deg_per_sec * (SECONDS_PER_LOGIC_FRAME_REAL * RADS_PER_DEGREE)
}

/// Normalize an angle to the range -PI...PI
///
/// This function handles NaN values by returning 0.
pub fn normalize_angle(mut angle: f32) -> f32 {
    // Handle NaN case
    if angle.is_nan() {
        return 0.0;
    }

    while angle > PI {
        angle -= 2.0 * PI;
    }

    while angle <= -PI {
        angle += 2.0 * PI;
    }

    angle
}

/// Calculate the difference between two angles, normalized
pub fn std_angle_diff(a1: f32, a2: f32) -> f32 {
    normalize_angle(a1 - a2)
}

/// Check if a pointer is "bogus" (has low bit set)
///
/// This is used for debugging in the original C++ code.
pub fn bogus_ptr<T>(ptr: *const T) -> bool {
    (ptr as usize) & 1 != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_angle() {
        // Test normal cases
        assert!((normalize_angle(0.0) - 0.0).abs() < f32::EPSILON);
        assert!((normalize_angle(PI) - PI).abs() < f32::EPSILON);
        assert!((normalize_angle(-PI) - PI).abs() < f32::EPSILON);

        // Test wrapping
        assert!((normalize_angle(2.0 * PI) - 0.0).abs() < f32::EPSILON);
        assert!((normalize_angle(-2.0 * PI) - 0.0).abs() < f32::EPSILON);
        assert!((normalize_angle(3.0 * PI) - PI).abs() < 0.000001);

        // Test NaN handling
        assert_eq!(normalize_angle(f32::NAN), 0.0);
    }

    #[test]
    fn test_std_angle_diff() {
        assert!((std_angle_diff(0.0, 0.0) - 0.0).abs() < f32::EPSILON);
        assert!((std_angle_diff(PI, 0.0) - PI).abs() < f32::EPSILON);
        assert!((std_angle_diff(0.0, PI) - PI).abs() < f32::EPSILON);
    }

    #[test]
    fn test_conversion_functions() {
        // Test frame conversion
        let frames = convert_duration_from_msecs_to_frames(1000.0);
        assert!((frames - 30.0).abs() < f32::EPSILON); // 1 second = 30 frames

        // Test velocity conversion
        let frame_velocity = convert_velocity_in_secs_to_frames(30.0);
        assert!((frame_velocity - 1.0).abs() < f32::EPSILON); // 30 units/sec = 1 unit/frame

        // Test angular velocity conversion
        let rads_per_frame = convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame(180.0);
        let expected = PI / 30.0; // 180 deg/sec = PI/30 rad/frame
        assert!((rads_per_frame - expected).abs() < f32::EPSILON);
    }

    #[test]
    fn test_veterancy_level_flags() {
        let mut flags = VETERANCY_LEVEL_FLAGS_NONE;

        // Test setting flags
        flags = set_veterancy_level_flag(flags, VeterancyLevel::Veteran);
        assert!(get_veterancy_level_flag(flags, VeterancyLevel::Veteran));
        assert!(!get_veterancy_level_flag(flags, VeterancyLevel::Elite));

        // Test clearing flags
        flags = clear_veterancy_level_flag(flags, VeterancyLevel::Veteran);
        assert!(!get_veterancy_level_flag(flags, VeterancyLevel::Veteran));
    }

    #[test]
    fn test_veterancy_level_names() {
        assert_eq!(VeterancyLevel::Regular.name(), "REGULAR");
        assert_eq!(VeterancyLevel::Veteran.name(), "VETERAN");
        assert_eq!(VeterancyLevel::Elite.name(), "ELITE");
        assert_eq!(VeterancyLevel::Heroic.name(), "HEROIC");
    }

    #[test]
    fn test_relationship_names() {
        assert_eq!(Relationship::Enemies.name(), "ENEMIES");
        assert_eq!(Relationship::Neutral.name(), "NEUTRAL");
        assert_eq!(Relationship::Allies.name(), "ALLIES");
    }

    #[test]
    fn test_attack_type_checks() {
        assert!(AbleToAttackType::NewTargetForced.is_forced_attack());
        assert!(!AbleToAttackType::NewTarget.is_forced_attack());

        assert!(AbleToAttackType::ContinuedTarget.is_continued_attack());
        assert!(!AbleToAttackType::NewTarget.is_continued_attack());
    }

    #[test]
    fn test_constants() {
        assert_eq!(MAX_PLAYER_COUNT, 16);
        assert_eq!(LOGIC_FRAMES_PER_SECOND, 30);
        assert_eq!(MSEC_PER_SECOND, 1000);
        assert_eq!(PLAYERMASK_ALL, 0xffff);
        assert_eq!(PLAYERMASK_NONE, 0x0);
    }
}
