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
use crate::compat::{ClassicState, legacy_transition, register_classic_state};
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

/// The AI state machine - implements all AI commands
pub struct AIStateMachine {
    /// Base state machine
    pub(crate) base: StateMachine,
    /// C++ `m_goalPath`: authored goal coordinates, separate from a computed `Path`.
    pub(crate) goal_path: Vec<Coord3D>,
    /// Goal waypoint
    pub(crate) goal_waypoint: Option<Arc<Waypoint>>,
    /// Goal squad to attack
    pub(crate) goal_squad: Option<Arc<Squad>>,
    /// Goal polygon area
    pub(crate) goal_polygon: Option<Arc<PolygonTrigger>>,
    /// Temporary state for short interruptions
    pub(crate) temporary_state_id: Option<u32>,
    /// Frame when temporary state ends
    pub(crate) temporary_state_frame_end: u32,
    /// Consumed by the next `set_temporary_state`. Set by `do_quick_exit`, which already holds the AI mutex.
    pub(crate) owner_ai_mutex_held_for_next_enter: bool,
}

impl std::fmt::Debug for AIStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AIStateMachine")
            .field("goal_path_len", &self.goal_path.len())
            .field("has_goal_waypoint", &self.goal_waypoint.is_some())
            .field("has_goal_squad", &self.goal_squad.is_some())
            .field("has_goal_polygon", &self.goal_polygon.is_some())
            .field("temporary_state_id", &self.temporary_state_id)
            .field("temporary_state_frame_end", &self.temporary_state_frame_end)
            .finish()
    }
}

impl AIStateMachine {
    pub fn new(owner: Weak<RwLock<Object>>, name: &str) -> Self {
        let mut machine = Self {
            base: StateMachine::new(Some(owner), name),
            goal_path: Vec::new(),
            goal_waypoint: None,
            goal_squad: None,
            goal_polygon: None,
            temporary_state_id: None,
            temporary_state_frame_end: 0,
            owner_ai_mutex_held_for_next_enter: false,
        };

        // Define all AI states
        machine.define_ai_states();
        machine
    }

    /// Define all AI states and their transitions
    pub(crate) fn define_ai_states(&mut self) {
        // Define basic movement states
        let idle_state = AIIdleState::new(&self.base, true);
        register_classic_state(
            &mut self.base,
            AIStateType::Idle.into(),
            idle_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_to_state = AIMoveToState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveTo.into(),
            move_to_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_out_state = AIMoveOutOfTheWayState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveOutOfTheWay.into(),
            move_out_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let tighten_state = AIMoveAndTightenState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAndTighten.into(),
            tighten_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_away_state = AIMoveAwayFromRepulsorsState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAwayFromRepulsors.into(),
            move_away_state,
            Some(AIStateType::WanderInPlace as u32),
            Some(AIStateType::WanderInPlace as u32),
            &[],
        );

        let wander_in_place_state = AIWanderInPlaceState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::WanderInPlace.into(),
            wander_in_place_state,
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let follow_team_state = AIFollowWaypointPathAsTeamState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsTeam.into(),
            follow_team_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_individuals_state = AIFollowWaypointPathAsIndividualsState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsIndividuals.into(),
            follow_individuals_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_team_exact_state = AIFollowWaypointPathAsTeamExactState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsTeamExact.into(),
            follow_team_exact_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_individuals_exact_state =
            AIFollowWaypointPathAsIndividualsExactState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowWaypointPathAsIndividualsExact.into(),
            follow_individuals_exact_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_path_state = AIFollowPathState::new(&self.base, false);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowPath.into(),
            follow_path_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let follow_exit_path_state = AIFollowExitProductionPathState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FollowExitProductionPath.into(),
            follow_exit_path_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        // Define attack states
        let attack_object_state = AIAttackObjectState::new(&self.base, false, false);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackObject.into(),
            attack_object_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let force_attack_state = AIAttackObjectState::new(&self.base, true, false);
        register_classic_state(
            &mut self.base,
            AIStateType::ForceAttackObject.into(),
            force_attack_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_state = AIAttackObjectState::new(&self.base, false, true);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackAndFollowObject.into(),
            attack_follow_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_position_state = AIAttackPositionState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackPosition.into(),
            attack_position_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_squad_state = AIAttackSquadState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackSquad.into(),
            attack_squad_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_area_state = AIAttackAreaState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackArea.into(),
            attack_area_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_move_state = AIAttackMoveToState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackMoveTo.into(),
            attack_move_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_team_state = AIAttackFollowWaypointPathAsTeamState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackFollowWaypointPathAsTeam.into(),
            attack_follow_team_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let attack_follow_individual_state =
            AIAttackFollowWaypointPathAsIndividualsState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::AttackFollowWaypointPathAsIndividuals.into(),
            attack_follow_individual_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_state = AIGuardState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Guard.into(),
            guard_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_retaliate_state = AIGuardRetaliateState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::GuardRetaliate.into(),
            guard_retaliate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let guard_tunnel_state = AITunnelNetworkGuardState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::GuardTunnelNetwork.into(),
            guard_tunnel_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let hunt_state = AIHuntState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Hunt.into(),
            hunt_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        // Define utility states
        let enter_state = AIEnterState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Enter.into(),
            enter_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let dock_state = AIDockState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Dock.into(),
            dock_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_evacuate_state = AIMoveAndEvacuateState::new(&self.base, "AIMoveAndEvacuate");
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAndEvacuate.into(),
            move_evacuate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let move_evacuate_exit_state =
            AIMoveAndEvacuateState::new(&self.base, "AIMoveAndEvacuateAndExit");
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAndEvacuateAndExit.into(),
            move_evacuate_exit_state,
            Some(AIStateType::MoveAndDelete as u32),
            Some(AIStateType::MoveAndDelete as u32),
            &[],
        );

        let move_and_delete_state = AIMoveAndDeleteState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::MoveAndDelete.into(),
            move_and_delete_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let wait_state = AIWaitState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Wait.into(),
            wait_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let exit_state = AIExitState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Exit.into(),
            exit_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let exit_instant_state = AIExitInstantlyState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::ExitInstantly.into(),
            exit_instant_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let pick_up_crate_state = AIPickUpCrateState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::PickUpCrate.into(),
            pick_up_crate_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let wander_state = AIWanderState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Wander.into(),
            wander_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let panic_state = AIPanicState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Panic.into(),
            panic_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::MoveAwayFromRepulsors as u32),
            &[],
        );

        let dead_state = AIDeadState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::Dead.into(),
            dead_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let hack_internet_state = AIHackInternetState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::HackInternet.into(),
            hack_internet_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let face_object_state = AIFaceObjectState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FaceObject.into(),
            face_object_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let face_position_state = AIFacePositionState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::FacePosition.into(),
            face_position_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let rappel_state = AIRappelIntoState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::RappelInto.into(),
            rappel_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let combat_drop_state = AICombatDropState::new(&self.base);
        register_classic_state(
            &mut self.base,
            AIStateType::CombatDrop.into(),
            combat_drop_state,
            Some(AIStateType::Idle as u32),
            Some(AIStateType::Idle as u32),
            &[],
        );

        let busy_state = AIBusyState::new(&self.base);
        register_classic_state(
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
        let Some(owner) = self.base.get_owner() else {
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

    /// Clear the state machine
    pub fn clear(&mut self) {
        // C++ AIStateMachine::clear() calls StateMachine::clear(), not reset().
        self.base.clear();
        self.goal_path.clear();
        self.goal_waypoint = None;
        self.goal_squad = None;
        self.goal_polygon = None;
        self.base.set_goal_squad(None);
        self.base.set_goal_polygon(None);
        self.notify_state_machine_changed();
    }

    /// Reset to default state
    pub fn reset_to_default_state(&mut self) -> StateReturnType {
        let ret = self.base.reset_to_default_state();
        self.notify_state_machine_changed();
        ret
    }

    pub fn get_current_state_id(&self) -> Option<u32> {
        self.base.get_current_state_id()
    }

    pub fn get_goal_position(&self) -> Option<Coord3D> {
        Some(self.base.get_goal_position())
    }

    /// Set state
    pub fn set_state(&mut self, new_state_id: u32) -> StateReturnType {
        let old_id = self.base.get_current_state_id();
        let ret = self.base.set_current_state(new_state_id);

        if old_id != Some(new_state_id) {
            self.notify_state_machine_changed();
        }

        ret
    }

    /// Enter a state while reusing the AI loan held by UnitAIUpdate's command callback.
    pub(crate) fn set_state_with_ai(
        &mut self,
        new_state_id: u32,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let old_id = self.base.get_current_state_id();
        let ret = self
            .base
            .set_current_state_with_ai_and_owner(new_state_id, ai, &mut ());

        if old_id != Some(new_state_id) {
            // C++ AIStateMachine::setState calls onStateMachineChanged synchronously.
            // The caller already owns this AI, so do not try-lock the installed handle.
            ai.set_queue_for_path_time(0);
        }

        ret
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
        self.base.set_goal_object_by_id(Some(obj_id));
    }

    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.base.set_goal_position(pos);
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
        self.goal_path = path.to_vec();
    }
    /// Stamp a path onto FollowExitProduction before its onEnter.
    /// C++ reads friend_getGoalPathPosition; Rust onEnter reads self.path.
    pub fn install_follow_exit_path(&mut self, path: &[Coord3D]) {
        let id = AIStateType::FollowExitProductionPath as u32;
        if let Some(state) = self.base.get_state_mut(id) {
            if let Some(follow) =
                crate::ai::states::follow_path::state_follow_path_kind(state.as_mut())
            {
                follow.set_path(path.to_vec(), None);
            }
        }
    }

    /// Add to goal path
    pub fn add_to_goal_path(&mut self, path_point: &Coord3D) {
        if self.goal_path.is_empty() {
            self.goal_path.push(*path_point);
            return;
        }

        if let Some(final_point) = self.goal_path.last() {
            if final_point.x == path_point.x
                && final_point.y == path_point.y
                && final_point.z == path_point.z
            {
                return;
            }
        }

        self.goal_path.push(*path_point);
    }

    /// Get goal path position at index
    pub fn get_goal_path_position(&self, i: usize) -> Option<&Coord3D> {
        self.goal_path.get(i)
    }

    /// Get goal path size
    pub fn get_goal_path_size(&self) -> usize {
        self.goal_path.len()
    }

    /// Set goal waypoint
    pub fn set_goal_waypoint(&mut self, waypoint: Option<Arc<Waypoint>>) {
        self.goal_waypoint = waypoint;
        let waypoint_id = self.goal_waypoint.as_ref().map(|w| w.id);
        self.base.set_goal_waypoint(waypoint_id);
    }

    /// Get goal waypoint
    pub fn get_goal_waypoint(&self) -> Option<&Arc<Waypoint>> {
        self.goal_waypoint.as_ref()
    }

    /// C++ AIStates.cpp:1040-1071 owns a private membership copy. States
    /// receive immutable membership for their synchronous step; mutations
    /// refresh the base weak handle before another state can observe it.
    pub fn set_goal_team(&mut self, team: &Arc<RwLock<Team>>) {
        let squad = self
            .goal_squad
            .get_or_insert_with(|| Arc::new(Squad::new()));
        if let Ok(team_guard) = team.read() {
            Arc::make_mut(squad).squad_from_team(&team_guard, true);
        }
        self.base
            .set_goal_squad(self.goal_squad.as_ref().map(Arc::downgrade));
    }

    /// Copy a caller's squad; retained handles are immutable membership values.
    pub fn set_goal_squad(&mut self, squad: Option<Arc<Squad>>) {
        match squad {
            Some(source) => {
                let target = self
                    .goal_squad
                    .get_or_insert_with(|| Arc::new(Squad::new()));
                if !Arc::ptr_eq(target, &source) {
                    *Arc::make_mut(target) = source.as_ref().clone();
                }
            }
            None => self.goal_squad = None,
        }
        self.base
            .set_goal_squad(self.goal_squad.as_ref().map(Arc::downgrade));
    }

    pub fn set_goal_polygon(&mut self, polygon: Option<Arc<PolygonTrigger>>) {
        self.goal_polygon = polygon.clone();
        self.base
            .set_goal_polygon(polygon.map(|value| Arc::downgrade(&value)));
    }

    /// C++ copies group membership into this machine's private squad.
    pub fn set_goal_ai_group(&mut self, group: &AIGroup) {
        let squad = self
            .goal_squad
            .get_or_insert_with(|| Arc::new(Squad::new()));
        Arc::make_mut(squad).squad_from_ai_group(group, true);
        self.base
            .set_goal_squad(self.goal_squad.as_ref().map(Arc::downgrade));
    }

    pub fn get_goal_squad(&self) -> Option<&Arc<Squad>> {
        self.goal_squad.as_ref()
    }

    /// Set temporary state
    pub fn set_temporary_state(&mut self, new_state_id: u32, frame_limit: u32) -> StateReturnType {
        let owner_ai_mutex_held =
            std::mem::replace(&mut self.owner_ai_mutex_held_for_next_enter, false);
        if let Some(current_id) = self.temporary_state_id.take() {
            if let Some(state) = self.base.get_state_mut(current_id) {
                state.on_exit(StateExitType::Reset);
            }
        }

        let goal_id = self.base.get_goal_object_id();
        let goal_pos = self.base.get_goal_position();
        let goal_squad = self.base.get_goal_squad();
        let goal_polygon = self.base.get_goal_polygon();
        let goal_waypoint = self.base.get_goal_waypoint();
        if let Some(state) = self.base.get_state_mut(new_state_id) {
            state.bind_goal_object_id(goal_id);
            state.bind_goal_position(goal_pos);
            state.bind_goal_squad(goal_squad);
            state.bind_goal_polygon(goal_polygon);
            state.bind_goal_waypoint(goal_waypoint);
            if owner_ai_mutex_held {
                if let Some(kind) =
                    crate::ai::states::follow_path::state_follow_path_kind(state.as_mut())
                {
                    kind.note_owner_ai_mutex_held(true);
                }
            }
            let ret = state.on_enter();
            if owner_ai_mutex_held {
                if let Some(kind) =
                    crate::ai::states::follow_path::state_follow_path_kind(state.as_mut())
                {
                    kind.note_owner_ai_mutex_held(false);
                }
            }
            if ret != StateReturnType::Continue {
                state.on_exit(StateExitType::Normal);
                return ret;
            }

            let max_limit = 60 * LOGICFRAMES_PER_SECOND;
            let capped_limit = frame_limit.min(max_limit);
            self.temporary_state_frame_end = TheGameLogic::get_frame().saturating_add(capped_limit);
            self.temporary_state_id = Some(new_state_id);
            return ret;
        }

        StateReturnType::Failure
    }

    /// Get temporary state ID
    pub fn get_temporary_state(&self) -> Option<u32> {
        self.temporary_state_id
    }

    /// Update state machine
    pub fn update_state_machine(&mut self, ai: &mut dyn AIUpdateInterface) -> StateReturnType {
        if let Some(temp_state_id) = self.temporary_state_id {
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
                let mut status = state.update_with_ai(ai);
                if self.temporary_state_frame_end < TheGameLogic::get_frame() {
                    if status == StateReturnType::Continue {
                        status = StateReturnType::Success;
                    }
                }
                if status == StateReturnType::Continue {
                    return status;
                }
                state.on_exit(StateExitType::Normal);
            }
            self.temporary_state_id = None;
        }

        // Update main state machine
        self.base.update_with_ai(ai)
    }

    /// Get current state name (for debugging)
    pub fn get_current_state_name(&self) -> String {
        let mut name = self.base.get_current_state_name();

        if let Some(temp_state_id) = self.temporary_state_id {
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
        self.ai_do_command_impl(params, None)
    }
}

impl AIStateMachine {
    pub(crate) fn ai_do_command_with_ai(
        &mut self,
        params: &AiCommandParams,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> Result<(), crate::ai::AiError> {
        self.ai_do_command_impl(params, Some(ai))
    }

    fn ai_do_command_impl(
        &mut self,
        params: &AiCommandParams,
        mut ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<(), crate::ai::AiError> {
        let is_follow_path_cmd = matches!(
            params.cmd,
            AiCommandType::FollowPath
                | AiCommandType::FollowExitProductionPath
                | AiCommandType::FollowUserPath
                | AiCommandType::FollowPathAppend
        );
        if !is_follow_path_cmd {
            if let Some(obj_id) = params.obj {
                self.base.set_goal_object_by_id(Some(obj_id));
            } else {
                self.base.set_goal_object_by_id(None);
            }
        } else {
            self.base.set_goal_object_by_id(None);
        }

        if params.pos != Coord3D::new(0.0, 0.0, 0.0) {
            self.base.set_goal_position(params.pos);
        }

        if let Some(team_name) = params.team.as_ref() {
            if let Ok(mut factory) = TheTeamFactory().lock() {
                if let Some(team) = factory.find_team(team_name) {
                    self.set_goal_team(&team);
                }
            }
        }

        if let Some(trigger_id) = params.polygon {
            if let Ok(terrain_guard) = get_terrain_logic().read() {
                if let Some(trigger) = terrain_guard.get_trigger_areas().get_by_id(trigger_id) {
                    let trigger_arc = Arc::new(trigger.clone());
                    self.set_goal_polygon(Some(trigger_arc));
                }
            }
        }

        if let Some(waypoint_id) = params.waypoint {
            if let Ok(terrain_guard) = get_terrain_logic().read() {
                if let Some(waypoint) = terrain_guard.get_waypoint_by_id(waypoint_id) {
                    let arc = Arc::new(Waypoint::from_terrain(waypoint));
                    self.set_goal_waypoint(Some(arc));
                } else {
                    self.set_goal_waypoint(None);
                }
            }
        }

        if matches!(
            params.cmd,
            AiCommandType::FollowPath
                | AiCommandType::FollowExitProductionPath
                | AiCommandType::FollowUserPath
        ) {
            self.set_goal_path(&params.coords);
            let target_state = if matches!(params.cmd, AiCommandType::FollowExitProductionPath) {
                AIStateType::FollowExitProductionPath
            } else {
                AIStateType::FollowPath
            };
            if let Some(state) = self.base.get_state_mut(target_state as u32) {
                if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                    path_state.set_path(params.coords.clone(), params.obj);
                }
            }
        } else if matches!(params.cmd, AiCommandType::FollowPathAppend) {
            let append_pos = params.pos;
            self.add_to_goal_path(&append_pos);
            if let Some(state_id) = self.base.get_current_state_id() {
                if let Some(state) = self.base.get_state_mut(state_id) {
                    if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                        path_state.append_path(append_pos);
                    }
                }
            } else if let Some(state) = self.base.get_state_mut(AIStateType::FollowPath as u32) {
                if let Some(path_state) = state_follow_path_kind(state.as_mut()) {
                    path_state.append_path(append_pos);
                }
            }
        }

        let state = match params.cmd {
            AiCommandType::Idle => AIStateType::Idle,
            AiCommandType::MoveToPosition
            | AiCommandType::MoveToObject
            | AiCommandType::MoveToPositionEvenIfSleeping => AIStateType::MoveTo,
            AiCommandType::FollowWaypointPath => AIStateType::FollowWaypointPathAsIndividuals,
            AiCommandType::FollowWaypointPathAsTeam => AIStateType::FollowWaypointPathAsTeam,
            AiCommandType::FollowWaypointPathExact => {
                AIStateType::FollowWaypointPathAsIndividualsExact
            }
            AiCommandType::FollowWaypointPathAsTeamExact => {
                AIStateType::FollowWaypointPathAsTeamExact
            }
            AiCommandType::FollowPath => AIStateType::FollowPath,
            AiCommandType::FollowExitProductionPath => AIStateType::FollowExitProductionPath,
            AiCommandType::FollowUserPath => AIStateType::FollowPath,
            AiCommandType::FollowPathAppend => AIStateType::FollowPath,
            AiCommandType::MoveToPositionAndEvacuate => AIStateType::MoveAndEvacuate,
            AiCommandType::MoveToPositionAndEvacuateAndExit => AIStateType::MoveAndEvacuateAndExit,
            AiCommandType::AttackObject => AIStateType::AttackObject,
            AiCommandType::ForceAttackObject => AIStateType::ForceAttackObject,
            AiCommandType::AttackPosition => AIStateType::AttackPosition,
            AiCommandType::AttackMoveToPosition => AIStateType::AttackMoveTo,
            AiCommandType::AttackFollowWaypointPath => {
                AIStateType::AttackFollowWaypointPathAsIndividuals
            }
            AiCommandType::AttackFollowWaypointPathAsTeam => {
                AIStateType::AttackFollowWaypointPathAsTeam
            }
            AiCommandType::AttackTeam => AIStateType::AttackSquad,
            AiCommandType::Hunt => AIStateType::Hunt,
            AiCommandType::AttackArea => AIStateType::AttackArea,
            AiCommandType::Repair => AIStateType::Busy,
            AiCommandType::ResumeConstruction => AIStateType::Busy,
            AiCommandType::GetHealed => AIStateType::Enter,
            AiCommandType::GetRepaired => AIStateType::Dock,
            AiCommandType::Enter => AIStateType::Enter,
            AiCommandType::Dock => AIStateType::Dock,
            AiCommandType::Exit => AIStateType::Exit,
            AiCommandType::ExitInstantly => AIStateType::ExitInstantly,
            AiCommandType::Evacuate => AIStateType::Exit,
            AiCommandType::EvacuateInstantly => AIStateType::ExitInstantly,
            AiCommandType::ExecuteRailedTransport => AIStateType::Busy,
            AiCommandType::GoProne => AIStateType::Busy,
            AiCommandType::GuardPosition => AIStateType::Guard,
            AiCommandType::GuardObject => AIStateType::Guard,
            AiCommandType::GuardArea => AIStateType::Guard,
            AiCommandType::GuardTunnelNetwork => AIStateType::GuardTunnelNetwork,
            AiCommandType::GuardRetaliate => AIStateType::GuardRetaliate,
            AiCommandType::HackInternet => AIStateType::HackInternet,
            AiCommandType::FaceObject => AIStateType::FaceObject,
            AiCommandType::FacePosition => AIStateType::FacePosition,
            AiCommandType::RappelInto => AIStateType::RappelInto,
            AiCommandType::CombatDrop => AIStateType::CombatDrop,
            AiCommandType::PickUpPrisoner => AIStateType::PickUpCrate,
            AiCommandType::Wander => AIStateType::Wander,
            AiCommandType::WanderInPlace => AIStateType::WanderInPlace,
            AiCommandType::Panic => AIStateType::Panic,
            AiCommandType::Busy => AIStateType::Busy,
            AiCommandType::MoveAwayFromUnit => AIStateType::MoveOutOfTheWay,
            AiCommandType::TightenToPosition => AIStateType::MoveAndTighten,
            AiCommandType::ReturnPrisoners => AIStateType::Busy,
            AiCommandType::DoSpecialPower => AIStateType::Busy,
            AiCommandType::DoSpecialPowerAtObject => AIStateType::Busy,
            AiCommandType::DoSpecialPowerAtLocation => AIStateType::Busy,
            AiCommandType::Sell => AIStateType::Busy,
            AiCommandType::ToggleOvercharge => AIStateType::Busy,
            AiCommandType::Surrender => AIStateType::Busy,
            AiCommandType::Cheer => AIStateType::Busy,
            _ => AIStateType::Idle,
        };

        if matches!(
            params.cmd,
            AiCommandType::GuardPosition
                | AiCommandType::GuardObject
                | AiCommandType::GuardArea
                | AiCommandType::GuardTunnelNetwork
        ) {
            self.base.set_guard_mode_raw(params.int_value);
        }

        if let Some(ai) = ai.as_deref_mut() {
            self.set_state_with_ai(state as u32, ai);
        } else {
            self.set_state(state as u32);
        }
        Ok(())
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

        let mut count = self.goal_path.len() as i32;
        xfer.xfer_int(&mut count)
            .map_err(|e| format!("Failed to xfer AIStateMachine goal path size: {:?}", e))?;
        if xfer.is_loading() {
            self.goal_path.clear();
        }

        for i in 0..count.max(0) {
            let mut pos = if xfer.is_loading() {
                Coord3D::new(0.0, 0.0, 0.0)
            } else {
                self.goal_path
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
                self.goal_path.push(pos);
            }
        }

        let mut waypoint_name = self
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
            self.goal_waypoint = loaded_waypoint;
        }
        let waypoint_id = self.goal_waypoint.as_ref().map(|waypoint| waypoint.id);
        self.base.set_goal_waypoint(waypoint_id);

        let mut has_squad = self.goal_squad.is_some();
        xfer.xfer_bool(&mut has_squad)
            .map_err(|e| format!("Failed to xfer has_squad: {:?}", e))?;

        if xfer.is_loading() {
            if has_squad && self.goal_squad.is_none() {
                self.goal_squad = Some(Arc::new(Squad::new()));
            }
        }

        if has_squad {
            if let Some(squad) = self.goal_squad.as_mut() {
                Arc::make_mut(squad).xfer(xfer)?;
            }
        }

        self.base
            .set_goal_squad(self.goal_squad.as_ref().map(|value| Arc::downgrade(value)));

        let mut temp_state_id = self.temporary_state_id.unwrap_or(INVALID_STATE_ID);

        xfer.xfer_unsigned_int(&mut temp_state_id)
            .map_err(|e| format!("Failed to xfer temporary_state_id: {:?}", e))?;

        if xfer.is_loading() && temp_state_id != INVALID_STATE_ID {
            self.temporary_state_id = self
                .base
                .get_state_name_by_id(temp_state_id)
                .map(|_| temp_state_id);
        }

        if temp_state_id != INVALID_STATE_ID {
            if let Some(state) = self.base.get_state_mut(temp_state_id) {
                state.xfer_snapshot(xfer)?;
            }
        }

        xfer.xfer_unsigned_int(&mut self.temporary_state_frame_end)
            .map_err(|e| format!("Failed to xfer temporary_state_frame_end: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base
            .load_post_process()
            .map_err(|e| format!("Failed to load_post_process AIStateMachine base: {}", e))
    }
}
