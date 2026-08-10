#![allow(deprecated, unused_imports, dead_code, hidden_glob_reexports, ambiguous_glob_reexports)]

//! AI state implementations split from the former monolithic `states.rs`.

mod helpers;
mod follow_path_core;
mod types;
mod state_machine;
mod idle;
#[path = "move.rs"]
mod r#move;
mod follow_path;
mod wait_busy;
mod wander_panic;
mod face;
mod hack;
mod rappel;
mod waypoint;
mod attack;
mod attack_machine;
mod guard;
mod hunt;
mod dock;
mod enter;
mod dead;

#[cfg(test)]
#[path = "tests.rs"]
mod ai_state_machine_parity_tests;

pub use types::{AIStateType, AICommandType, AiCommandType, AICommandParms, AICommandParmsStorage};
pub use state_machine::{AIStateMachine};
pub use idle::{AIIdleState};
pub use r#move::{AIMoveToState, AIMoveAwayFromRepulsorsState, AIWanderInPlaceState, AIMoveOutOfTheWayState, AIMoveAndTightenState, AIMoveAndDeleteState, AIMoveAndEvacuateState};
pub use follow_path::{AIFollowPathState, AIFollowExitProductionPathState, AIFollowState};
pub use wait_busy::{AIWaitState, AIBusyState};
pub use wander_panic::{AIWanderState, AIPanicState};
pub use face::{AIFaceObjectState, AIFacePositionState};
pub use hack::{AIHackInternetState};
pub use rappel::{AIRappelIntoState, AICombatDropState};
pub use waypoint::{AIFollowWaypointPathAsTeamState, AIFollowWaypointPathAsTeamExactState, AIFollowWaypointPathAsIndividualsState, AIFollowWaypointPathAsIndividualsExactState};
pub use attack::{AIAttackMoveToState, AIAttackFollowWaypointPathAsTeamState, AIAttackFollowWaypointPathAsIndividualsState, AIAttackObjectState, AIAttackPositionState, AIAttackThenIdleStateMachine, AIPickUpCrateState, AIAttackSquadState, AIAttackAreaState};
pub use attack_machine::{AttackSubStateId, AttackStateMachine, AttackExitConditionsInterface, AIAttackAimAtTargetState, AIAttackFireWeaponState, AIAttackPursueTargetState, AIAttackApproachTargetState, AIAttackMoveStateMachine};
pub use guard::{AIGuardState, AIGuardRetaliateState, AITunnelNetworkGuardState};
pub use hunt::{AIHuntState};
pub use dock::{AIDockState};
pub use enter::{AIEnterState, AIExitState, AIExitInstantlyState};
pub use dead::{AIDeadState};

pub(crate) use helpers::*;
pub(crate) use follow_path_core::*;
pub use types::*;
pub use state_machine::*;
pub use idle::*;
pub use r#move::*;
pub use follow_path::*;
pub use wait_busy::*;
pub use wander_panic::*;
pub use face::*;
pub use hack::*;
pub use rappel::*;
pub use waypoint::*;
pub use attack::*;
pub use attack_machine::*;
pub use guard::*;
pub use hunt::*;
pub use dock::*;
pub use enter::*;
pub use dead::*;
