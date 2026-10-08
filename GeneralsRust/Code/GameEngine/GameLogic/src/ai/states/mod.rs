#![allow(
    deprecated,
    unused_imports,
    dead_code,
    hidden_glob_reexports,
    ambiguous_glob_reexports
)]

//! AI state implementations split from the former monolithic `states.rs`.

mod attack;
pub(crate) mod attack_machine;
mod dead;
mod dock;
mod enter;
mod face;
pub(crate) mod follow_path;
mod follow_path_core;
mod guard;
mod hack;
pub(crate) mod helpers;
mod hunt;
mod idle;
#[path = "move.rs"]
mod r#move;
mod rappel;
mod state_machine;
mod types;
mod wait_busy;
mod wander_panic;

/// Full attack command emitted by native terminal callbacks.
#[derive(Debug)]
pub(crate) struct TerminalAttackCommand {
    params: crate::ai::AiCommandParams,
}

impl TerminalAttackCommand {
    fn attack_object(
        target_id: crate::common::ObjectID,
        max_shots: i32,
        source: crate::ai::CommandSourceType,
    ) -> Self {
        let mut params =
            crate::ai::AiCommandParams::new(crate::ai::AiCommandType::AttackObject, source);
        params.obj = Some(target_id);
        params.int_value = max_shots;
        Self { params }
    }

    pub(crate) fn params(&self) -> &crate::ai::AiCommandParams {
        &self.params
    }
}

/// Operation-local typed callback output. Never stored or serialized.
struct TerminalCommandContext {
    command: Option<TerminalAttackCommand>,
    state: Option<AIStateType>,
    parent_is_idle: bool,
    parent_is_attacking: bool,
}

impl TerminalCommandContext {
    fn new() -> Self {
        Self {
            command: None,
            state: None,
            parent_is_idle: false,
            parent_is_attacking: false,
        }
    }

    fn request_attack_object(
        &mut self,
        target_id: crate::common::ObjectID,
        max_shots: i32,
        source: crate::ai::CommandSourceType,
    ) {
        assert!(
            self.command.is_none() && self.state.is_none(),
            "one terminal command per state step"
        );
        self.command = Some(TerminalAttackCommand::attack_object(
            target_id, max_shots, source,
        ));
    }

    fn take(&mut self) -> Option<TerminalAttackCommand> {
        self.command.take()
    }

    fn request_state(&mut self, state: AIStateType) {
        assert!(
            self.command.is_none() && self.state.is_none(),
            "one terminal operation per state step"
        );
        self.state = Some(state);
    }
}

/// Short-lived mutable loan of the native AI machine's real control and data.
pub(crate) struct AIStateMachineDriver<'a> {
    base: &'a mut crate::state_machine::StateMachine,
    data: &'a mut state_machine::AIStateMachineData,
}
mod waypoint;

pub(crate) use attack_machine::seed_team_target_if_attack_common;
#[cfg(test)]
#[path = "tests.rs"]
mod ai_state_machine_parity_tests;

#[cfg(test)]
mod machine_snapshot_tests;

#[cfg(test)]
mod busy_update_tests;

#[cfg(test)]
mod busy_classification_tests;

#[cfg(test)]
mod construction_tests;

#[cfg(test)]
mod squad_owner_tests;

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
