#![allow(non_upper_case_globals)]
////////////////////////////////////////////////////////////////////////////////
//																																						//
//  (c) 2001-2003 Electronic Arts Inc.																				//
//																																						//
////////////////////////////////////////////////////////////////////////////////

// FILE: GameCommon.rs ////////////////////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//
//                       Westwood Studios Pacific.
//
//                       Confidential Information
//                Copyright (C) 2001 - All Rights Reserved
//
//-----------------------------------------------------------------------------
//
// Project:    RTS3
//
// File name:  GameCommon.rs
//
// Created:    Steven Johnson, October 2001
//
// Desc:		   This is a catchall header for some basic types and definitions
//						needed by various bits of the GameLogic/GameClient, but that
//						we haven't found a good place for yet. Hopefully this file
//						should go away someday, but for now is a convenient spot.
//
//-----------------------------------------------------------------------------

//! This module provides common game constants, types, and utility functions
//! that are shared across the GameLogic and GameClient modules.

use std::f32::consts::PI;

/// Core timing constants
pub const LOGICFRAMES_PER_SECOND: u32 = 30;
pub const MSEC_PER_SECOND: u32 = 1000;

/// Timing conversion constants
pub const LOGICFRAMES_PER_MSEC_REAL: f32 = LOGICFRAMES_PER_SECOND as f32 / MSEC_PER_SECOND as f32;
pub const MSEC_PER_LOGICFRAME_REAL: f32 = MSEC_PER_SECOND as f32 / LOGICFRAMES_PER_SECOND as f32;
pub const LOGICFRAMES_PER_SECONDS_REAL: f32 = LOGICFRAMES_PER_SECOND as f32;
pub const SECONDS_PER_LOGICFRAME_REAL: f32 = 1.0f32 / LOGICFRAMES_PER_SECONDS_REAL;

/// Special time values
pub const NEVER: u32 = 0;
pub const FOREVER: u32 = 0x3fffffff; // (we use 0x3fffffff so that we can add offsets and not overflow...
                                     //  at 30fps we're still pretty safe!)

/// Maximum player count
pub const MAX_PLAYER_COUNT: usize = 16;

/// Player mask type - a bitmask that can uniquely represent each player
pub type PlayerMaskType = u16;
pub const PLAYERMASK_ALL: PlayerMaskType = 0xffff;
pub const PLAYERMASK_NONE: PlayerMaskType = 0x0;

/// Global general types
pub const MAX_GLOBAL_GENERAL_TYPES: usize = 9; // number of playable General Types, not including the boss
pub const GLOBAL_GENERAL_BEGIN: usize = 5; // The start of the playable global generals playertemplates
pub const GLOBAL_GENERAL_END: usize = GLOBAL_GENERAL_BEGIN + MAX_GLOBAL_GENERAL_TYPES - 1; // The end of the playable global generals

/// Game difficulty levels
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GameDifficulty {
    Easy = 0,
    Normal,
    Hard,
}

impl GameDifficulty {
    pub const COUNT: usize = 3;
}

/// Player types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PlayerType {
    Human = 0, // player is human-controlled
    Computer,  // player is computer-controlled
}

impl PlayerType {
    pub const COUNT: usize = 2;
}

/// Cell shroud status - A PartitionCell can be one of three states for Shroud
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CellShroudStatus {
    Clear = 0,
    Fogged,
    Shrouded,
}

impl CellShroudStatus {
    pub const COUNT: usize = 3;
}

/// Object shroud status - Since an object can take up more than a single PartitionCell,
/// this is a status that applies to the whole Object
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ObjectShroudStatus {
    Invalid = 0,             // indeterminate state, will recompute
    Clear,                   // object is not shrouded at all (ie, completely visible)
    PartialClear,            // object is partly clear (rest is shroud or fog)
    Fogged,                  // object is completely fogged
    Shrouded,                // object is completely shrouded
    InvalidButPreviousValid, // indeterminate state, will recompute, BUT previous status is valid, don't reset (used for save/load)
}

impl ObjectShroudStatus {
    pub const COUNT: usize = 6;
}

/// Guard mode enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GuardMode {
    Normal = 0,
    GuardWithoutPursuit,  // no pursuit out of guard area
    GuardFlyingUnitsOnly, // ignore nonflyers
}

/// Veterancy levels
/// NOTE NOTE NOTE: Keep TheVeterancyNames in sync with these.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VeterancyLevel {
    Regular = 0,
    Veteran,
    Elite,
    Heroic,
}

impl VeterancyLevel {
    pub const COUNT: usize = 4;
    pub const INVALID: i32 = -1;
    pub const FIRST: VeterancyLevel = VeterancyLevel::Regular;
    pub const LAST: VeterancyLevel = VeterancyLevel::Heroic;
}

/// The veterancy names (corresponds to TheVeterancyNames in C++)
pub const VETERANCY_NAMES: [&str; VeterancyLevel::COUNT] =
    ["REGULAR", "VETERAN", "ELITE", "HEROIC"];

/// Command source types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CommandSourceType {
    FromPlayer = 0,
    FromScript,
    FromAi,
    FromDozer,
    DefaultSwitchWeapon,
}

impl CommandSourceType {
    pub const FromAI: CommandSourceType = CommandSourceType::FromAi;
}

/// Attack type flags
pub const ATTACK_FORCED: u32 = 0x01;
pub const ATTACK_CONTINUED: u32 = 0x02;
pub const ATTACK_TUNNELNETWORK_GUARD: u32 = 0x04;

/// Able to attack type enumeration  
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AbleToAttackType {
    NewTarget = 0,                      // can we attack if this is a new target?
    NewTargetForced = ATTACK_FORCED,    // can we attack if this is a new target, via force-fire?
    ContinuedTarget = ATTACK_CONTINUED, // can we attack if this is continuation of an existing attack?
    ContinuedTargetForced = ATTACK_FORCED | ATTACK_CONTINUED, // continued + forced
    TunnelNetworkGuard = ATTACK_TUNNELNETWORK_GUARD, // Special case that bypasses some checks for units guarding from within tunnel networks
}

/// Check if attack is forced
pub fn is_forced_attack(attack_type: AbleToAttackType) -> bool {
    (attack_type as u32 & ATTACK_FORCED) != 0
}

/// Check if attack is continued
pub fn is_continued_attack(attack_type: AbleToAttackType) -> bool {
    (attack_type as u32 & ATTACK_CONTINUED) != 0
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

/// Turret type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WhichTurretType {
    Invalid = -1,
    Main = 0,
    Alt,
}

impl WhichTurretType {
    pub const MAX_TURRETS: usize = 2;
}

/// Maximum number of turrets supported (matches C++ MAX_TURRETS).
pub const MAX_TURRETS: usize = WhichTurretType::MAX_TURRETS;

/// Relationship types  
/// NOTE NOTE NOTE: Keep TheRelationshipNames in sync with this enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Relationship {
    Enemies = 0,
    Neutral,
    Allies,
}

/// The relationship names (corresponds to TheRelationshipNames in C++)
pub const RELATIONSHIP_NAMES: [&str; 3] = ["ENEMIES", "NEUTRAL", "ALLIES"];

/// Timing Conversion Functions
///
/// Note that this returns a REAL value, not an int... most callers will want to
/// call ceil() on the result, so that partial frames get converted to full frames!
pub fn convert_duration_from_msecs_to_frames(msec: f32) -> f32 {
    msec * LOGICFRAMES_PER_MSEC_REAL
}

/// Convert velocity from per-second to per-frame
pub fn convert_velocity_in_secs_to_frames(dist_per_sec: f32) -> f32 {
    // this looks wrong, but is the correct conversion factor.
    dist_per_sec * SECONDS_PER_LOGICFRAME_REAL
}

/// Convert acceleration from per-second-squared to per-frame-squared
pub fn convert_acceleration_in_secs_to_frames(dist_per_sec2: f32) -> f32 {
    // this looks wrong, but is the correct conversion factor.
    const SEC_PER_LOGICFRAME_SQR: f32 = SECONDS_PER_LOGICFRAME_REAL * SECONDS_PER_LOGICFRAME_REAL;
    dist_per_sec2 * SEC_PER_LOGICFRAME_SQR
}

/// Convert angular velocity from degrees per second to radians per frame
pub fn convert_angular_velocity_in_degrees_per_sec_to_rads_per_frame(deg_per_sec: f32) -> f32 {
    const RADS_PER_DEGREE: f32 = PI / 180.0f32;
    deg_per_sec * (SECONDS_PER_LOGICFRAME_REAL * RADS_PER_DEGREE)
}

/// Normalize an angle to the range -PI...PI
pub fn normalize_angle(angle: f32) -> f32 {
    let mut normalized = angle;
    while normalized > PI {
        normalized -= 2.0 * PI;
    }
    while normalized <= -PI {
        normalized += 2.0 * PI;
    }
    normalized
}

/// Return the difference between two angles, normalized
pub fn std_angle_diff(a1: f32, a2: f32) -> f32 {
    normalize_angle(a1 - a2)
}

/// Check if a pointer appears bogus (ported from C++ macro)
pub fn is_bogus_ptr<T>(ptr: *const T) -> bool {
    (ptr as usize & 1) != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timing_constants() {
        assert_eq!(LOGICFRAMES_PER_SECOND, 30);
        assert_eq!(MSEC_PER_SECOND, 1000);
        assert_eq!(LOGICFRAMES_PER_MSEC_REAL, 0.03);
        assert!((MSEC_PER_LOGICFRAME_REAL - (1000.0 / 30.0)).abs() < 0.00001);
    }

    #[test]
    fn test_conversion_functions() {
        let frames = convert_duration_from_msecs_to_frames(1000.0);
        assert_eq!(frames, 30.0); // 1 second = 30 frames

        let velocity = convert_velocity_in_secs_to_frames(30.0);
        assert_eq!(velocity, 1.0); // 30 units/sec = 1 unit/frame
    }

    #[test]
    fn test_angle_functions() {
        let angle = normalize_angle(4.0 * PI);
        assert!((angle - 0.0).abs() < f32::EPSILON);

        let diff = std_angle_diff(PI / 2.0, -PI / 2.0);
        assert!((diff - PI).abs() < f32::EPSILON);
    }

    #[test]
    fn test_veterancy_flags() {
        let mut flags = VETERANCY_LEVEL_FLAGS_NONE;
        flags = set_veterancy_level_flag(flags, VeterancyLevel::Veteran);

        assert!(get_veterancy_level_flag(flags, VeterancyLevel::Veteran));
        assert!(!get_veterancy_level_flag(flags, VeterancyLevel::Elite));

        flags = clear_veterancy_level_flag(flags, VeterancyLevel::Veteran);
        assert!(!get_veterancy_level_flag(flags, VeterancyLevel::Veteran));
    }

    #[test]
    fn test_attack_type_checks() {
        assert!(is_forced_attack(AbleToAttackType::NewTargetForced));
        assert!(!is_forced_attack(AbleToAttackType::NewTarget));

        assert!(is_continued_attack(AbleToAttackType::ContinuedTarget));
        assert!(!is_continued_attack(AbleToAttackType::NewTarget));
    }
}
