#![allow(
    deprecated,
    unused_imports,
    dead_code,
    hidden_glob_reexports,
    ambiguous_glob_reexports
)]

//! AI state implementations split from the former monolithic `states.rs`.

mod attack;
mod attack_machine;
mod dead;
mod dock;
mod enter;
mod face;
mod follow_path;
mod follow_path_core;
mod guard;
mod hack;
mod helpers;
mod hunt;
mod idle;
#[path = "move.rs"]
mod r#move;
mod rappel;
mod state_machine;
mod types;
mod wait_busy;
mod wander_panic;
mod waypoint;

#[cfg(test)]
#[path = "tests.rs"]
mod ai_state_machine_parity_tests;

pub use attack::{
    AIAttackAreaState, AIAttackFollowWaypointPathAsIndividualsState,
    AIAttackFollowWaypointPathAsTeamState, AIAttackMoveToState, AIAttackObjectState,
    AIAttackPositionState, AIAttackSquadState, AIAttackThenIdleStateMachine, AIPickUpCrateState,
};
pub use attack_machine::{
    AIAttackAimAtTargetState, AIAttackApproachTargetState, AIAttackFireWeaponState,
    AIAttackMoveStateMachine, AIAttackPursueTargetState, AttackExitConditionsInterface,
    AttackStateMachine, AttackSubStateId,
};
pub use dead::AIDeadState;
pub use dock::AIDockState;
pub use enter::{AIEnterState, AIExitInstantlyState, AIExitState};
pub use face::{AIFaceObjectState, AIFacePositionState};
pub use follow_path::{AIFollowExitProductionPathState, AIFollowPathState, AIFollowState};
pub use guard::{AIGuardRetaliateState, AIGuardState, AITunnelNetworkGuardState};
pub use hack::AIHackInternetState;
pub use hunt::AIHuntState;
pub use idle::AIIdleState;
pub use r#move::{
    AIMoveAndDeleteState, AIMoveAndEvacuateState, AIMoveAndTightenState,
    AIMoveAwayFromRepulsorsState, AIMoveOutOfTheWayState, AIMoveToState, AIWanderInPlaceState,
};
pub use rappel::{AICombatDropState, AIRappelIntoState};
pub use state_machine::AIStateMachine;
pub use types::{AICommandParms, AICommandParmsStorage, AICommandType, AIStateType, AiCommandType};
pub use wait_busy::{AIBusyState, AIWaitState};
pub use wander_panic::{AIPanicState, AIWanderState};
pub use waypoint::{
    AIFollowWaypointPathAsIndividualsExactState, AIFollowWaypointPathAsIndividualsState,
    AIFollowWaypointPathAsTeamExactState, AIFollowWaypointPathAsTeamState,
};

pub use attack::*;
pub use attack_machine::*;
pub use dead::*;
pub use dock::*;
pub use enter::*;
pub use face::*;
pub use follow_path::*;
pub(crate) use follow_path_core::*;
pub use guard::*;
pub use hack::*;
pub(crate) use helpers::*;
pub use hunt::*;
pub use idle::*;
pub use r#move::*;
pub use rappel::*;
pub use state_machine::*;
pub use types::*;
pub use wait_busy::*;
pub use wander_panic::*;
pub use waypoint::*;
