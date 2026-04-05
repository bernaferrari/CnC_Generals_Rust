//! Attack system enums and helpers.

use crate::common::*;

/// Can attack result enumeration (C++ discriminant values)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum CanAttackResult {
    NotPossible = 0,
    InvalidShot = 1,
    PossibleAfterMoving = 2,
    Possible = 3,
}

// Re-export canonical AbleToAttackType from Common (C++ bitmask: FORCED=0x01, CONTINUED=0x02, TUNNEL=0x04)
pub use game_engine::common::game_common::{
    is_continued_attack, is_forced_attack, AbleToAttackType,
};

// Attack result constants
pub const ATTACKRESULT_POSSIBLE: CanAttackResult = CanAttackResult::Possible;
pub const ATTACKRESULT_POSSIBLE_AFTER_MOVING: CanAttackResult =
    CanAttackResult::PossibleAfterMoving;
pub const ATTACKRESULT_NOT_POSSIBLE: CanAttackResult = CanAttackResult::NotPossible;
pub const ATTACKRESULT_INVALID_SHOT: CanAttackResult = CanAttackResult::InvalidShot;
