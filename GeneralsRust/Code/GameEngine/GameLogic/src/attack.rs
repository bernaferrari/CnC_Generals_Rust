//! Attack system enums and helpers.

use crate::common::*;

/// Can attack result enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanAttackResult {
    Possible,
    PossibleAfterMoving,
    NotPossible,
    InvalidShot,
}

/// Able to attack type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbleToAttackType {
    CanAttackSpecific = 0,
    CanAttackArea = 1,
    ContinuedTarget = 2,
    ContinuedTargetForced = 3,
}

// Attack result constants
pub const ATTACKRESULT_POSSIBLE: CanAttackResult = CanAttackResult::Possible;
pub const ATTACKRESULT_POSSIBLE_AFTER_MOVING: CanAttackResult =
    CanAttackResult::PossibleAfterMoving;
pub const ATTACKRESULT_NOT_POSSIBLE: CanAttackResult = CanAttackResult::NotPossible;
pub const ATTACKRESULT_INVALID_SHOT: CanAttackResult = CanAttackResult::InvalidShot;
