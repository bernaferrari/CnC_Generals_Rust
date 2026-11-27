//! State tags of the receiving actor’s base AI and its presentation projection.
use serde::{Deserialize, Serialize};

/// C++ AttackStateMachine substates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AttackSubState {
    /// C++ AIM_AT_TARGET (default on enter).
    #[default]
    AimAtTarget,
    /// C++ FIRE_WEAPON.
    FireWeapon,
    /// C++ APPROACH_TARGET.
    ApproachTarget,
    /// C++ CHASE_TARGET (pursuit collapses to approach when the target is stationary).
    ChaseTarget,
}

impl AttackSubState {
    pub fn to_ordinal(self) -> u8 {
        match self {
            AttackSubState::AimAtTarget => 0,
            AttackSubState::FireWeapon => 1,
            AttackSubState::ApproachTarget => 2,
            AttackSubState::ChaseTarget => 3,
        }
    }

    pub fn from_ordinal(v: u8) -> Self {
        match v {
            1 => AttackSubState::FireWeapon,
            2 => AttackSubState::ApproachTarget,
            3 => AttackSubState::ChaseTarget,
            _ => AttackSubState::AimAtTarget,
        }
    }
}

/// Active attack child of a Guard or GuardRetaliate machine.
/// This is an internal continuation, not a C++ state ID or wire layout.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GuardChasePhase {
    #[default]
    None,
    Inner,
    Outer,
    Aggressor,
    RetaliateAggressor,
}

/// AttackAggressor child of C++ AIGuardRetaliateMachine.
pub const GUARD_CHASE_PHASE_RETALIATE: GuardChasePhase = GuardChasePhase::RetaliateAggressor;

/// AI behavior states
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AIState {
    Idle,
    Moving,
    Attacking,
    AttackMoving,
    AttackingGround,
    Gathering,
    ReturningResources,
    Constructing,
    Repairing,
    GuardingArea,
    GuardingObject,
    /// C++ AI_GUARD_RETALIATE — attack aggressor with guard restrictions.
    GuardRetaliating,
    Patrolling,
    Docked,
    Garrisoned,
    SpecialAbility,
    SeekingRepair,
    SeekingHealing,
    Entering,
    Docking,
    Capturing,
    /// C++ `AI_FACE_OBJECT`.
    FacingObject,
    /// C++ `AI_FACE_POSITION`.
    FacingPosition,
    /// C++ AI_PANIC: follow authored waypoint links while allowing infantry bounce.
    /// Appended to preserve existing bincode variant indices.
    Panic,
    /// C++ AI_EXIT: passenger remains contained while its door is unavailable.
    /// Appended so existing save variant indices retain their meanings.
    Exiting,
    /// StateMachine::INVALID_STATE_ID between clear and a new entry.
    Uninitialized,
    /// Original AI_HUNT parent, distinct from its active attack/idle child.
    Hunting,
    JetReturningForLanding,
    JetReturnToDeadAirfield,
    JetCirclingDeadAirfield,
    /// C++ AI_DEAD (AIUpdate.cpp:1085); entered at the next admitted AI update.
    Dead,
}

// Stable AI state ordinals used by presentation and world snapshots.
impl AIState {
    pub fn to_ordinal(&self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Moving => 1,
            Self::Attacking => 2,
            Self::AttackMoving => 3,
            Self::AttackingGround => 4,
            Self::Gathering => 5,
            Self::ReturningResources => 6,
            Self::Constructing => 7,
            Self::Repairing => 8,
            Self::GuardingArea => 9,
            Self::GuardingObject => 10,
            Self::GuardRetaliating => 20,
            Self::Patrolling => 11,
            Self::Docked => 12,
            Self::Garrisoned => 13,
            Self::SpecialAbility => 14,
            Self::SeekingRepair => 15,
            Self::SeekingHealing => 16,
            Self::Entering => 17,
            Self::Docking => 18,
            Self::Capturing => 19,
            Self::FacingObject => 21,
            Self::FacingPosition => 22,
            Self::Panic => 23,
            Self::Exiting => 24,
            Self::Uninitialized => 25,
            Self::Hunting => 26,
            Self::JetReturningForLanding => 27,
            Self::JetReturnToDeadAirfield => 28,
            Self::JetCirclingDeadAirfield => 29,
            Self::Dead => 30,
        }
    }

    pub fn from_ordinal(ordinal: u8) -> Self {
        match ordinal {
            1 => Self::Moving,
            2 => Self::Attacking,
            3 => Self::AttackMoving,
            4 => Self::AttackingGround,
            5 => Self::Gathering,
            6 => Self::ReturningResources,
            7 => Self::Constructing,
            8 => Self::Repairing,
            9 => Self::GuardingArea,
            10 => Self::GuardingObject,
            11 => Self::Patrolling,
            12 => Self::Docked,
            13 => Self::Garrisoned,
            14 => Self::SpecialAbility,
            15 => Self::SeekingRepair,
            16 => Self::SeekingHealing,
            17 => Self::Entering,
            18 => Self::Docking,
            19 => Self::Capturing,
            20 => Self::GuardRetaliating,
            21 => Self::FacingObject,
            22 => Self::FacingPosition,
            23 => Self::Panic,
            24 => Self::Exiting,
            25 => Self::Uninitialized,
            26 => Self::Hunting,
            27 => Self::JetReturningForLanding,
            28 => Self::JetReturnToDeadAirfield,
            29 => Self::JetCirclingDeadAirfield,
            30 => Self::Dead,
            _ => Self::Idle,
        }
    }
}
