#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
use super::follow_path::*;
use super::follow_path_core::*;
use super::guard::*;
use super::hack::*;
use super::helpers::*;
use super::hunt::*;
use super::idle::*;
use super::r#move::*;
use super::rappel::*;
use super::types::*;
use super::wait_busy::*;
use super::wander_panic::*;
use super::waypoint::*;
use super::*;

use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::dock::AIDockMachine;
use crate::ai::group::AIGroup;
use crate::ai::guard::{AIGuardMachine, GuardStateType};
use crate::ai::guard_retaliate::AIGuardRetaliateMachine;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::pathfind::Path;
use crate::ai::squad::Squad;
use crate::ai::tn_guard::{AITNGuardMachine, TNGuardStateType};
use crate::ai::{
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter,
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers, the_ai,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::control_bar::get_control_bar_bridge;
use crate::damage::DamageInfo;
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, get_game_logic_random_value};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE, PhysicsBehaviorExt,
};
use crate::object::production::AIFreeToExitType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::GRAVITY;
use crate::player::PlayerType;
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::engine::get_script_engine;
use crate::state_machine::cpp_state::{CppState, cpp_transition, register_cpp_state};
use crate::state_machine::*;
use crate::team::{Team, TeamID, TheTeamFactory};
use crate::terrain::get_terrain_logic;
use crate::waypoint::{Waypoint, WaypointId};
use crate::weapon::{
    NO_MAX_SHOTS_LIMIT, Weapon, WeaponChoiceCriteria, WeaponLockType, WeaponSlotType, WeaponStatus,
};
use game_engine::common::system::{GeometryType, Snapshotable, Xfer};

use crate::common::INVALID_ID;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

fn notify_state_machine_changed_for_base(base: &StateMachine) {
    let Some(owner) = base.get_owner() else {
        return;
    };
    let ai = {
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        owner_guard.get_ai_update_interface()
    };
    if let Some(ai) = ai {
        if let Ok(mut ai_guard) = ai.try_lock() {
            ai_guard.set_queue_for_path_time(0);
            return;
        }
    }
    if let Ok(mut owner_guard) = owner.write() {
        owner_guard.ai_pending_wake_path = true;
    }
}
/// Existing Rust AIStateMachine non-base fields; this is not a claim of exact C++ field layout.
pub(super) struct AIStateMachineData {
    /// C++ m_goalPath counterpart.
    pub(super) goal_path: Vec<Coord3D>,
    /// C++ m_goalWaypoint counterpart.
    pub(super) goal_waypoint: Option<Arc<Waypoint>>,
    /// C++ m_goalSquad counterpart.
    pub(super) goal_squad: Option<Arc<Squad>>,
    /// Rust extension; no direct C++ field counterpart.
    pub(super) goal_polygon: Option<Arc<PolygonTrigger>>,
    /// C++ temporary-state ID counterpart.
    pub(super) temporary_state_id: Option<u32>,
    /// C++ temporary-state end-frame counterpart.
    pub(super) temporary_state_frame_end: u32,
    /// Rust bridge marker; no direct C++ field counterpart.
    pub(super) owner_ai_mutex_held_for_next_enter: bool,
}

/// The AI state machine - implements all AI commands
pub struct AIStateMachine {
    /// Base state machine
    pub(crate) base: StateMachine,
    /// Owned Rust non-base runtime values; retains the prior Rust and Xfer order.
    pub(super) data: AIStateMachineData,
}

impl std::fmt::Debug for AIStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIStateMachine")
            .field("goal_path_len", &self.data.goal_path.len())
            .field("has_goal_waypoint", &self.data.goal_waypoint.is_some())
            .field("has_goal_squad", &self.data.goal_squad.is_some())
            .field("has_goal_polygon", &self.data.goal_polygon.is_some())
            .field("temporary_state_id", &self.data.temporary_state_id)
            .field(
                "temporary_state_frame_end",
                &self.data.temporary_state_frame_end,
            )
            .finish()
    }
}

#[path = "state_machine_driver.rs"]
mod driver;
use driver::AIStateMachineDriver;

impl AIStateMachine {
    fn driver(&mut self) -> AIStateMachineDriver<'_> {
        AIStateMachineDriver::new(&mut self.base, &mut self.data)
    }
    pub(crate) fn note_owner_ai_mutex_held_for_next_enter(&mut self) {
        self.data.owner_ai_mutex_held_for_next_enter = true;
    }
}

impl AIStateMachine {
    pub fn new(owner: Weak<RwLock<Object>>, name: &str) -> Self {
        let mut machine = Self {
            base: StateMachine::new(Some(owner), name),
            data: AIStateMachineData {
                goal_path: Vec::new(),
                goal_waypoint: None,
                goal_squad: None,
                goal_polygon: None,
                temporary_state_id: None,
                temporary_state_frame_end: 0,
                owner_ai_mutex_held_for_next_enter: false,
            },
        };

        // Define all AI states
        machine.define_ai_states();
        machine
    }

    /// Define all AI states and their transitions
    pub(crate) fn define_ai_states(&mut self) {
        // Define basic movement states
        let idle_state = AIIdleState::new(&self.base, true);
        register_cpp_state(
            &mut self.base,
            AIStateType::Idle.into(),
            idle_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_to_state = AIMoveToState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::MoveTo.into(),
            move_to_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_out_state = AIMoveOutOfTheWayState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::MoveOutOfTheWay.into(),
            move_out_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let tighten_state = AIMoveAndTightenState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::MoveAndTighten.into(),
            tighten_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_away_state = AIMoveAwayFromRepulsorsState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::MoveAwayFromRepulsors.into(),
            move_away_state,
            Some(AIStateType::WanderInPlace as u32),
            Some(AIStateType::WanderInPlace as u32),
            &[],
        );

        let wander_in_place_state = AIWanderInPlaceState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::WanderInPlace.into(),
            wander_in_place_state,
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let follow_team_state = AIFollowWaypointPathAsTeamState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsTeam.into(),
            follow_team_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_individuals_state = AIFollowWaypointPathAsIndividualsState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsIndividuals.into(),
            follow_individuals_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_team_exact_state = AIFollowWaypointPathAsTeamExactState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsTeamExact.into(),
            follow_team_exact_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_individuals_exact_state =
            AIFollowWaypointPathAsIndividualsExactState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsIndividualsExact.into(),
            follow_individuals_exact_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_path_state = AIFollowPathState::new(&self.base, false);
        register_cpp_state(
            &mut self.base,
            AIStateType::FollowPath.into(),
            follow_path_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_exit_path_state = AIFollowExitProductionPathState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::FollowExitProductionPath.into(),
            follow_exit_path_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        // Define attack states
        let attack_object_state = AIAttackObjectState::new(&self.base, false, false);
        register_cpp_state(
            &mut self.base,
            AIStateType::AttackObject.into(),
            attack_object_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let force_attack_state = AIAttackObjectState::new(&self.base, true, false);
        register_cpp_state(
            &mut self.base,
            AIStateType::ForceAttackObject.into(),
            force_attack_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_state = AIAttackObjectState::new(&self.base, false, true);
        register_cpp_state(
            &mut self.base,
            AIStateType::AttackAndFollowObject.into(),
            attack_follow_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_position_state = AIAttackPositionState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::AttackPosition.into(),
            attack_position_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_squad_state = AIAttackSquadState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::AttackSquad.into(),
            attack_squad_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_area_state = AIAttackAreaState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::AttackArea.into(),
            attack_area_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_move_state = AIAttackMoveToState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::AttackMoveTo.into(),
            attack_move_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_team_state = AIAttackFollowWaypointPathAsTeamState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::AttackFollowWaypointPathAsTeam.into(),
            attack_follow_team_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_individual_state =
            AIAttackFollowWaypointPathAsIndividualsState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::AttackFollowWaypointPathAsIndividuals.into(),
            attack_follow_individual_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_state = AIGuardState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Guard.into(),
            guard_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_retaliate_state = AIGuardRetaliateState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::GuardRetaliate.into(),
            guard_retaliate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_tunnel_state = AITunnelNetworkGuardState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::GuardTunnelNetwork.into(),
            guard_tunnel_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let hunt_state = AIHuntState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Hunt.into(),
            hunt_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        // Define utility states
        let enter_state = AIEnterState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Enter.into(),
            enter_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let dock_state = AIDockState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Dock.into(),
            dock_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_evacuate_state = AIMoveAndEvacuateState::new(&self.base, "AIMoveAndEvacuate");
        register_cpp_state(
            &mut self.base,
            AIStateType::MoveAndEvacuate.into(),
            move_evacuate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_evacuate_exit_state =
            AIMoveAndEvacuateState::new(&self.base, "AIMoveAndEvacuateAndExit");
        register_cpp_state(
            &mut self.base,
            AIStateType::MoveAndEvacuateAndExit.into(),
            move_evacuate_exit_state,
            Some(AIStateType::MoveAndDelete as u32),
            Some(AIStateType::MoveAndDelete as u32),
            &[],
        );

        let move_and_delete_state = AIMoveAndDeleteState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::MoveAndDelete.into(),
            move_and_delete_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let wait_state = AIWaitState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Wait.into(),
            wait_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let exit_state = AIExitState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Exit.into(),
            exit_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let exit_instant_state = AIExitInstantlyState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::ExitInstantly.into(),
            exit_instant_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let pick_up_crate_state = AIPickUpCrateState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::PickUpCrate.into(),
            pick_up_crate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let wander_state = AIWanderState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Wander.into(),
            wander_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let panic_state = AIPanicState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Panic.into(),
            panic_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let dead_state = AIDeadState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Dead.into(),
            dead_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let hack_internet_state = AIHackInternetState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::HackInternet.into(),
            hack_internet_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let face_object_state = AIFaceObjectState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::FaceObject.into(),
            face_object_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let face_position_state = AIFacePositionState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::FacePosition.into(),
            face_position_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let rappel_state = AIRappelIntoState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::RappelInto.into(),
            rappel_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let combat_drop_state = AICombatDropState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::CombatDrop.into(),
            combat_drop_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let busy_state = AIBusyState::new(&self.base);
        register_cpp_state(
            &mut self.base,
            AIStateType::Busy.into(),
            busy_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        // AIStates.cpp:660-714 only defines states; the first definition is
        // the default. AIUpdate::onObjectCreated initializes it after install.
    }

    pub(crate) fn notify_state_machine_changed(&self) {
        notify_state_machine_changed_for_base(&self.base)
    }

    /// Clear the state machine
    pub fn clear(&mut self) {
        self.driver().clear()
    }

    pub(crate) fn clear_with_ai(&mut self, ai: &mut dyn crate::modules::AIUpdateInterface) {
        self.driver().clear_with_ai(ai)
    }

    /// Reset to default state
    pub fn reset_to_default_state(&mut self) -> StateReturnType {
        self.driver().reset_to_default_state()
    }

    pub fn get_current_state_id(&self) -> Option<u32> {
        self.base.get_current_state_id()
    }

    pub fn get_goal_position(&self) -> Option<Coord3D> {
        Some(self.base.get_goal_position())
    }

    /// Set state
    pub fn set_state(&mut self, new_state_id: u32) -> StateReturnType {
        self.driver().set_state(new_state_id)
    }

    /// Enter a state while reusing the AI loan held by UnitAIUpdate's command callback.
    pub(crate) fn set_state_with_ai(
        &mut self,
        new_state_id: u32,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        self.driver().set_state_with_ai(new_state_id, ai)
    }

    pub fn lock(&mut self) {
        self.base.lock();
    }

    pub fn unlock(&mut self) {
        self.base.unlock();
    }

    pub fn is_locked(&self) -> bool {
        self.base.is_locked()
    }

    pub fn set_goal_object(&mut self, obj_id: ObjectID) {
        self.driver().set_goal_object(obj_id)
    }

    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.driver().set_goal_position(pos)
    }

    pub fn get_goal_object(&self) -> Option<Arc<RwLock<Object>>> {
        // Wave 257: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let id = self.base.get_goal_object_id();
        if id == crate::common::INVALID_ID {
            return None;
        }
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
    }

    pub fn get_goal_object_id(&self) -> crate::common::ObjectID {
        self.base.get_goal_object_id()
    }

    pub fn is_idle(&self) -> bool {
        self.base.is_in_idle_state()
    }

    pub fn is_busy(&self) -> bool {
        self.base.is_in_busy_state()
    }

    pub fn is_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.base.is_in_guard_idle_state()
    }

    /// Set goal path
    pub fn set_goal_path(&mut self, path: &[Coord3D]) {
        self.driver().set_goal_path(path)
    }
    /// Stamp a path onto FollowExitProduction before its onEnter.
    /// C++ reads friend_getGoalPathPosition; Rust onEnter reads self.path.
    pub fn install_follow_exit_path(&mut self, path: &[Coord3D]) {
        self.driver().install_follow_exit_path(path)
    }

    /// Add to goal path
    pub fn add_to_goal_path(&mut self, path_point: &Coord3D) {
        self.driver().add_to_goal_path(path_point)
    }

    /// Get goal path position at index
    pub fn get_goal_path_position(&self, i: usize) -> Option<&Coord3D> {
        self.data.goal_path.get(i)
    }

    /// Get goal path size
    pub fn get_goal_path_size(&self) -> usize {
        self.data.goal_path.len()
    }

    /// Set goal waypoint
    pub fn set_goal_waypoint(&mut self, waypoint: Option<Arc<Waypoint>>) {
        self.driver().set_goal_waypoint(waypoint)
    }

    /// Get goal waypoint
    pub fn get_goal_waypoint(&self) -> Option<&Arc<Waypoint>> {
        self.data.goal_waypoint.as_ref()
    }

    /// C++ AIStates.cpp:1040-1071 owns a private membership copy. States
    /// receive immutable membership for their synchronous step; mutations
    /// refresh the base weak handle before another state can observe it.
    pub fn set_goal_team(&mut self, team: &Arc<RwLock<Team>>) {
        self.driver().set_goal_team(team)
    }

    /// Copy a caller's squad; retained handles are immutable membership values.
    pub fn set_goal_squad(&mut self, squad: Option<Arc<Squad>>) {
        self.driver().set_goal_squad(squad)
    }

    pub fn set_goal_polygon(&mut self, polygon: Option<Arc<PolygonTrigger>>) {
        self.driver().set_goal_polygon(polygon)
    }

    /// C++ copies group membership into this machine's private squad.
    pub fn set_goal_ai_group(&mut self, group: &AIGroup) {
        self.driver().set_goal_ai_group(group)
    }

    pub fn get_goal_squad(&self) -> Option<&Arc<Squad>> {
        self.data.goal_squad.as_ref()
    }

    /// Set temporary state
    pub fn set_temporary_state(&mut self, new_state_id: u32, frame_limit: u32) -> StateReturnType {
        self.driver().set_temporary_state(new_state_id, frame_limit)
    }

    /// Get temporary state ID
    pub fn get_temporary_state(&self) -> Option<u32> {
        self.data.temporary_state_id
    }

    /// Native driver retains its concrete AI borrow through body completion.
    pub(crate) fn update_state_machine<A: StateMachineAI + ?Sized>(
        &mut self,
        ai: &mut A,
        after_body: impl FnOnce(&mut AIStateMachineDriver<'_>, &mut A, &mut dyn std::any::Any),
    ) -> StateReturnType {
        if let Some(temp_state_id) = self.data.temporary_state_id {
            let goal_id = self.base.get_goal_object_id();
            let goal_pos = self.base.get_goal_position();
            let goal_squad = self.base.get_goal_squad();
            let goal_polygon = self.base.get_goal_polygon();
            let goal_waypoint = self.base.get_goal_waypoint();
            if let Some(state) = self.base.get_state_mut(temp_state_id) {
                state.bind_goal_object_id(goal_id);
                state.bind_goal_position(goal_pos);
                state.bind_goal_squad(goal_squad);
                state.bind_goal_polygon(goal_polygon);
                state.bind_goal_waypoint(goal_waypoint);
                let mut status = state.update_with_ai(ai.as_ai_update());
                if self.data.temporary_state_frame_end < TheGameLogic::get_frame() {
                    if status == StateReturnType::Continue {
                        status = StateReturnType::Success;
                    }
                }
                if status == StateReturnType::Continue {
                    return status;
                }
                state.on_exit(StateExitType::Normal);
            }
            self.data.temporary_state_id = None;
        }

        // The state-table borrow ends before the driver completes this step.
        // This is where synchronous terminal commands can acquire a loan of
        // this machine without reentering the outgoing state's callback.
        let mut owner = ();
        let data = &mut self.data;
        let mut update = self.base.begin_update_with_ai_and_owner(ai, &mut owner);
        if let StateUpdate::Body(step) = &mut update {
            step.with_driver(|base, ai, owner| {
                let mut driver = AIStateMachineDriver::new(base, data);
                after_body(&mut driver, ai, owner);
            });
        }
        update.finish()
    }

    /// Get current state name (for debugging)
    pub fn get_current_state_name(&self) -> String {
        let mut name = self.base.get_current_state_name();

        if let Some(temp_state_id) = self.data.temporary_state_id {
            if let Some(temp_name) = self.base.get_state_name_by_id(temp_state_id) {
                name.push_str(" /T/");
                name.push_str(temp_name);
            }
        }

        name
    }
}

impl AiCommandInterface for AIStateMachine {
    fn ai_do_command(&mut self, params: &AiCommandParams) -> Result<(), crate::ai::AiError> {
        self.driver().ai_do_command(params)
    }
}

impl AIStateMachine {
    pub(crate) fn ai_do_command_with_ai(
        &mut self,
        params: &AiCommandParams,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<(), crate::ai::AiError> {
        self.driver().ai_do_command_with_ai(params, ai)
    }
}

impl Snapshotable for AIStateMachine {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base
            .crc(xfer)
            .map_err(|e| format!("Failed to crc AIStateMachine base: {}", e))
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // AIStates.cpp:733-765 transfers the Coord3D vector, not Path nodes.
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer AIStateMachine version: {:?}", e))?;

        self.base
            .xfer(xfer)
            .map_err(|e| format!("Failed to xfer AIStateMachine base: {}", e))?;

        let mut count = self.data.goal_path.len() as i32;
        xfer.xfer_int(&mut count)
            .map_err(|e| format!("Failed to xfer AIStateMachine goal path size: {:?}", e))?;
        if xfer.is_loading() {
            self.data.goal_path.clear();
        }

        for i in 0..count.max(0) {
            let mut pos = if xfer.is_loading() {
                Coord3D::new(0.0, 0.0, 0.0)
            } else {
                self.data
                    .goal_path
                    .get(i as usize)
                    .copied()
                    .unwrap_or_else(|| Coord3D::new(0.0, 0.0, 0.0))
            };
            xfer.xfer_real(&mut pos.x)
                .map_err(|e| format!("Failed to xfer goal_path[{i}].x: {:?}", e))?;
            xfer.xfer_real(&mut pos.y)
                .map_err(|e| format!("Failed to xfer goal_path[{i}].y: {:?}", e))?;
            xfer.xfer_real(&mut pos.z)
                .map_err(|e| format!("Failed to xfer goal_path[{i}].z: {:?}", e))?;
            if xfer.is_loading() {
                self.data.goal_path.push(pos);
            }
        }

        let mut waypoint_name = self
            .data
            .goal_waypoint
            .as_ref()
            .map(|waypoint| waypoint.name.clone())
            .unwrap_or_default();

        xfer.xfer_ascii_string(&mut waypoint_name)
            .map_err(|e| format!("Failed to xfer AIStateMachine waypoint name: {:?}", e))?;

        if xfer.is_loading() && !waypoint_name.is_empty() {
            let mut loaded_waypoint = None;
            let lookup = AsciiString::from(waypoint_name.as_str());
            if let Ok(terrain) = get_terrain_logic().read() {
                if let Some(waypoint) = terrain.get_waypoint_by_name(&lookup) {
                    loaded_waypoint = Some(Arc::new(Waypoint::new(
                        waypoint.get_id(),
                        *waypoint.get_location(),
                        waypoint.get_name().as_str().to_string(),
                    )));
                }
            }
            self.data.goal_waypoint = loaded_waypoint;
        }
        let waypoint_id = self.data.goal_waypoint.as_ref().map(|waypoint| waypoint.id);
        self.base.set_goal_waypoint(waypoint_id);

        let mut has_squad = self.data.goal_squad.is_some();
        xfer.xfer_bool(&mut has_squad)
            .map_err(|e| format!("Failed to xfer has_squad: {:?}", e))?;

        if xfer.is_loading() {
            if has_squad && self.data.goal_squad.is_none() {
                self.data.goal_squad = Some(Arc::new(Squad::new()));
            }
        }

        if has_squad {
            if let Some(squad) = self.data.goal_squad.as_mut() {
                Arc::make_mut(squad).xfer(xfer)?;
            }
        }

        self.base.set_goal_squad(
            self.data
                .goal_squad
                .as_ref()
                .map(|value| Arc::downgrade(value)),
        );

        let mut temp_state_id = self.data.temporary_state_id.unwrap_or(INVALID_STATE_ID);

        xfer.xfer_unsigned_int(&mut temp_state_id)
            .map_err(|e| format!("Failed to xfer temporary_state_id: {:?}", e))?;

        if xfer.is_loading() && temp_state_id != INVALID_STATE_ID {
            self.data.temporary_state_id = self
                .base
                .get_state_name_by_id(temp_state_id)
                .map(|_| temp_state_id);
        }

        if temp_state_id != INVALID_STATE_ID {
            if let Some(state) = self.base.get_state_mut(temp_state_id) {
                state.xfer_snapshot(xfer)?;
            }
        }

        xfer.xfer_unsigned_int(&mut self.data.temporary_state_frame_end)
            .map_err(|e| format!("Failed to xfer temporary_state_frame_end: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base
            .load_post_process()
            .map_err(|e| format!("Failed to load_post_process AIStateMachine base: {}", e))
    }
}
