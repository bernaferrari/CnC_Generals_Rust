//! AI State Machine - Modern Rust implementation
//!
//! This module provides a type-safe state machine implementation using Rust enums
//! and match statements, replacing the C++ class hierarchy with a more maintainable
//! and performance-oriented approach.
//!
//! Author: Converted from C++ by Claude, original by Michael S. Booth, January 2002

use super::integration::with_ai_integration_mut;
use super::pathfind::PathfindLayerEnum;
use super::{
    resolve_attack_priority_info_for_object, search_qualifiers, vision_factors, AiCommandInterface,
    AiCommandParams, AiCommandType, AiError, AttitudeType, GuardMode, PartitionFilter, THE_AI,
};
use crate::ai::native::{NativeState, NativeStateMachine, WaypointGraph, WaypointNode};
use crate::ai::object_registry::get_legacy_object;
use crate::ai::squad::Squad;
use crate::common::types::{Coord2D, Coord3D, Real};
use crate::common::{
    BodyDamageType, CoordOrigin, KindOf, ModelConditionFlags, ObjectID, INVALID_ID,
    LOGICFRAMES_PER_SECOND,
};
use crate::helpers::{game_logic_random_value, TheAudio, TheGameLogic};
use crate::object::object_factory::{get_object_factory, GameObjectInstance};
use crate::object::registry::OBJECT_REGISTRY;
use crate::path::{SURFACE_CLIFF, SURFACE_GROUND, SURFACE_RUBBLE, SURFACE_WATER};
use crate::team::get_team_factory;
use crate::terrain::get_terrain_logic;
use crate::waypoint::WaypointId;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::Duration;

fn is_cliff_at(pos: &Coord3D) -> bool {
    get_terrain_logic()
        .read()
        .map(|terrain| terrain.is_cliff_cell(pos.x, pos.y))
        .unwrap_or(false)
}

fn normalize_relative_angle(mut angle: Real) -> Real {
    const PI: Real = std::f32::consts::PI;
    const TAU: Real = std::f32::consts::TAU;
    while angle > PI {
        angle -= TAU;
    }
    while angle < -PI {
        angle += TAU;
    }
    angle
}

fn relative_angle_2d(owner_pos: &Coord3D, owner_orientation: Real, target_pos: &Coord3D) -> Real {
    let angle_to_target = (target_pos.y - owner_pos.y).atan2(target_pos.x - owner_pos.x);
    normalize_relative_angle(angle_to_target - owner_orientation)
}

/// AI State types - converted from C++ enum to Rust enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AiStateType {
    Idle,
    MoveTo,
    FollowWaypointPathAsTeam,
    FollowWaypointPathAsIndividuals,
    FollowWaypointPathAsTeamExact,
    FollowWaypointPathAsIndividualsExact,
    FollowPath,
    FollowExitProductionPath,
    Wait,
    AttackPosition,
    AttackObject,
    ForceAttackObject,
    AttackAndFollowObject,
    Dead,
    Dock,
    Enter,
    Guard,
    Hunt,
    Wander,
    Panic,
    AttackSquad,
    GuardTunnelNetwork,
    GetRepaired,
    MoveOutOfTheWay,
    MoveAndTighten,
    MoveAndEvacuate,
    MoveAndEvacuateAndExit,
    MoveAndDelete,
    AttackArea,
    HackInternet,
    AttackMoveTo,
    AttackFollowWaypointPathAsIndividuals,
    AttackFollowWaypointPathAsTeam,
    FaceObject,
    FacePosition,
    RappelInto,
    CombatDrop,
    Exit,
    PickUpCrate,
    MoveAwayFromRepulsors,
    WanderInPlace,
    Busy,
    ExitInstantly,
    GuardRetaliate,
}

impl Default for AiStateType {
    fn default() -> Self {
        AiStateType::Idle
    }
}

/// State return types indicating what should happen next
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateReturnType {
    Continue,      // Keep running this state
    StateComplete, // State finished successfully
    StateFailed,   // State failed
    StateBlocked,  // State is blocked, try again later
}

/// State exit conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateExitType {
    Success,
    Failure,
    Interrupted,
    Timeout,
}

/// AI State data - contains the actual state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiStateData {
    /// Current state type
    state_type: AiStateType,
    /// Goal position for movement states
    goal_position: Option<Coord3D>,
    /// Goal object for object-based states
    goal_object: Option<ObjectID>,
    /// Path to follow (as list of coordinates)
    goal_path: Vec<Coord3D>,
    /// Current path index
    path_index: usize,
    /// Waypoint chain to follow
    goal_waypoint: Option<WaypointId>,
    /// Polygon trigger for area-based commands
    goal_polygon: Option<crate::polygon_trigger::PolygonTriggerId>,
    /// Target squad for squad attacks
    goal_squad: Option<SquadId>,
    /// Squad handle (legacy machine parity)
    #[serde(skip)]
    goal_squad_handle: Option<std::sync::Arc<std::sync::Mutex<Squad>>>,
    /// Maximum shots to fire in attack states
    max_shots: i32,
    /// Guard mode for guard states
    guard_mode: GuardMode,
    /// State-specific scratch data
    scratch: AiStateScratch,
    /// Logic frame when this state was entered
    enter_frame: Option<u32>,
    /// Timeout in logic frames (if any)
    timeout_frames: Option<u32>,
    /// Whether to adjust destinations to avoid stacking
    adjust_destinations: bool,
    /// Whether this is a formation movement
    is_formation: bool,
    /// Current attitude/mood
    attitude: super::AttitudeType,
}

pub type SquadId = u32;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct AiStateScratch {
    current_position: Option<Coord3D>,
    pathfinding_requested: bool,
    path_following_index: i32,
    weapon_ready: bool,
    last_fire_frame: i32,
    guard_position: Option<Coord3D>,
    last_scan_frame: i32,
    enemy_found: ObjectID,
    last_hunt_scan_frame: i32,
    current_hunt_target: ObjectID,
    last_move_target: Option<Coord3D>,
    move_origin: Option<Coord3D>,
    wander_pause_until: i32,
    last_direction_change: i32,
    attack_area_next_scan_frame: i32,
    #[serde(skip)]
    move_sound_handle: u32,
    path_goal_position: Option<Coord3D>,
    path_timestamp: u32,
    #[serde(default)]
    face_can_turn_in_place: bool,
}

/// Generic state value for compatibility with legacy callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StateValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
    Position(Coord3D),
    ObjectId(ObjectID),
}

impl Default for AiStateData {
    fn default() -> Self {
        Self {
            state_type: AiStateType::Idle,
            goal_position: None,
            goal_object: None,
            goal_path: Vec::new(),
            path_index: 0,
            goal_waypoint: None,
            goal_polygon: None,
            goal_squad: None,
            goal_squad_handle: None,
            max_shots: 0,
            guard_mode: GuardMode::Normal,
            scratch: AiStateScratch::default(),
            enter_frame: None,
            timeout_frames: None,
            adjust_destinations: true,
            is_formation: false,
            attitude: super::AttitudeType::Normal,
        }
    }
}

/// AI State Machine implementation
#[allow(dead_code)]
#[derive(Debug)]
pub struct AiStateMachine {
    /// Owner object ID
    owner_id: ObjectID,
    /// Native Rust state machine mirror for simpler serialization/inspection
    native: NativeStateMachine,
    /// Current state
    current_state: AiStateData,
    /// Previous state (for debugging and state transitions)
    previous_state: Option<AiStateData>,
    /// Temporary state (for interruptions like move out of way)
    temporary_state: Option<AiStateData>,
    /// Logic frame when temporary state should end
    temporary_state_end_frame: Option<u32>,
    /// State machine name for debugging
    name: String,
    /// Whether state machine is paused
    paused: bool,
    /// Optional waypoint graph for native pathing
    waypoint_graph: Option<Arc<RwLock<WaypointGraph>>>,
}

impl AiStateMachine {
    /// Create new AI state machine
    pub fn new(owner_id: ObjectID, name: String) -> Self {
        Self {
            owner_id,
            native: NativeStateMachine::new(owner_id),
            current_state: AiStateData::default(),
            previous_state: None,
            temporary_state: None,
            temporary_state_end_frame: None,
            name,
            paused: false,
            waypoint_graph: None,
        }
    }

    /// Get current state type
    pub fn get_current_state(&self) -> AiStateType {
        if let Some(ref temp_state) = self.temporary_state {
            temp_state.state_type
        } else {
            self.current_state.state_type
        }
    }

    /// Set new state
    pub fn set_state(&mut self, new_state: AiStateType) -> Result<(), AiError> {
        // Exit current state
        let prev_state = self.current_state.clone();
        self.on_state_exit(&prev_state, StateExitType::Interrupted)?;

        // Store previous state
        self.previous_state = Some(prev_state);

        // Create new state
        let mut new_state_data = AiStateData::default();
        new_state_data.state_type = new_state;
        new_state_data.enter_frame = Some(TheGameLogic::get_frame());

        // Enter new state
        self.current_state = new_state_data;

        log::debug!(
            "AI {} entering state: {:?}",
            self.owner_id,
            self.current_state.state_type
        );
        let mut entering_state = std::mem::take(&mut self.current_state);
        self.on_state_enter(&mut entering_state)?;
        self.current_state = entering_state;

        self.native
            .set_state(self.native_state_for(new_state, &self.current_state));

        Ok(())
    }

    /// Set temporary state (for interruptions)
    pub fn set_temporary_state(
        &mut self,
        state: AiStateType,
        duration: Duration,
    ) -> Result<(), AiError> {
        let mut temp_state = AiStateData::default();
        temp_state.state_type = state;

        let frame_limit = Self::duration_to_frames(duration);
        let current_frame = TheGameLogic::get_frame();
        temp_state.enter_frame = Some(current_frame);

        self.on_state_enter(&mut temp_state)?;
        self.temporary_state = Some(temp_state);
        self.temporary_state_end_frame = Some(current_frame.saturating_add(frame_limit));

        let temp_state_ref = self.temporary_state.as_ref().unwrap();
        self.native.set_temporary_state(
            self.native_state_for(state, temp_state_ref),
            frame_limit,
            current_frame,
        );

        Ok(())
    }

    /// Update the cached current position for AI state logic.
    pub fn set_current_position(&mut self, position: Coord3D) {
        self.current_state.scratch.current_position = Some(position);

        if let Some(ref mut temp_state) = self.temporary_state {
            temp_state.scratch.current_position = Some(position);
        }
    }

    /// Clear temporary state
    pub fn clear_temporary_state(&mut self) {
        self.clear_temporary_state_with_exit(StateExitType::Success);
    }

    fn clear_temporary_state_with_exit(&mut self, exit: StateExitType) {
        if let Some(temp_state) = self.temporary_state.take() {
            let _ = self.on_state_exit(&temp_state, exit);
        }
        self.temporary_state_end_frame = None;
        self.native.clear_temporary_state();
    }

    /// Update state machine for one frame
    pub fn update(&mut self) -> Result<StateReturnType, AiError> {
        if self.paused {
            return Ok(StateReturnType::Continue);
        }

        let current_frame = TheGameLogic::get_frame();

        // Check if temporary state has expired
        if let Some(end_frame) = self.temporary_state_end_frame {
            if current_frame >= end_frame {
                self.clear_temporary_state();
            }
        }

        // Update current or temporary state
        let use_temporary = self.temporary_state.is_some();

        if use_temporary {
            // Handle temporary state - extract data first to avoid borrowing conflicts
            let (enter_frame, timeout_frames) = {
                if let Some(ref temp_state) = self.temporary_state {
                    (temp_state.enter_frame, temp_state.timeout_frames)
                } else {
                    return Ok(StateReturnType::StateComplete);
                }
            };

            // Check for timeout before updating
            if let (Some(enter_frame), Some(timeout_frames)) = (enter_frame, timeout_frames) {
                if current_frame.saturating_sub(enter_frame) > timeout_frames {
                    return Ok(StateReturnType::StateFailed);
                }
            }

            // Now update the temporary state
            if let Some(mut temp_state) = self.temporary_state.take() {
                let result = self.update_state(&mut temp_state);
                match result {
                    Ok(StateReturnType::StateComplete) => {
                        self.clear_temporary_state_with_exit(StateExitType::Success);
                        return Ok(StateReturnType::StateComplete);
                    }
                    Ok(StateReturnType::StateFailed) => {
                        self.clear_temporary_state_with_exit(StateExitType::Failure);
                        return Ok(StateReturnType::StateFailed);
                    }
                    _ => {
                        self.temporary_state = Some(temp_state);
                        return result;
                    }
                }
            }
        }

        // Check timeout for current state before updating
        let (enter_frame, timeout_frames) = (
            self.current_state.enter_frame,
            self.current_state.timeout_frames,
        );
        if let (Some(enter_frame), Some(timeout_frames)) = (enter_frame, timeout_frames) {
            if current_frame.saturating_sub(enter_frame) > timeout_frames {
                return Ok(StateReturnType::StateFailed);
            }
        }

        // Update the current state
        let mut current_state = std::mem::replace(&mut self.current_state, AiStateData::default());
        let result = self.update_state(&mut current_state);
        if let Ok(exit_result) = result {
            if matches!(
                exit_result,
                StateReturnType::StateComplete | StateReturnType::StateFailed
            ) {
                let exit_type = if exit_result == StateReturnType::StateComplete {
                    StateExitType::Success
                } else {
                    StateExitType::Failure
                };
                let _ = self.on_state_exit(&current_state, exit_type);
                let mut idle_state = AiStateData::default();
                idle_state.state_type = AiStateType::Idle;
                idle_state.enter_frame = Some(current_frame);
                let _ = self.on_state_enter(&mut idle_state);
                self.native
                    .set_state(self.native_state_for(AiStateType::Idle, &idle_state));
                self.current_state = idle_state;
                return Ok(exit_result);
            }
        }
        self.current_state = current_state;
        result
    }

    /// Update specific state type
    fn update_state(&mut self, state: &mut AiStateData) -> Result<StateReturnType, AiError> {
        match state.state_type {
            AiStateType::Idle => self.update_idle_state(state),
            AiStateType::MoveTo
            | AiStateType::MoveOutOfTheWay
            | AiStateType::MoveAndTighten
            | AiStateType::MoveAndEvacuate
            | AiStateType::MoveAndEvacuateAndExit
            | AiStateType::MoveAndDelete => self.update_move_to_state(state),
            AiStateType::AttackObject
            | AiStateType::AttackAndFollowObject
            | AiStateType::ForceAttackObject => self.update_attack_object_state(state),
            AiStateType::AttackPosition => self.update_attack_position_state(state),
            AiStateType::AttackMoveTo => self.update_attack_move_to_state(state),
            AiStateType::AttackArea => self.update_attack_area_state(state),
            AiStateType::AttackSquad => self.update_attack_squad_state(state),
            AiStateType::AttackFollowWaypointPathAsTeam
            | AiStateType::AttackFollowWaypointPathAsIndividuals => {
                self.update_attack_follow_waypoint_path_state(state)
            }
            AiStateType::FaceObject => self.update_face_object_state(state),
            AiStateType::FacePosition => self.update_face_position_state(state),
            AiStateType::Guard => self.update_guard_state(state),
            AiStateType::GuardTunnelNetwork => self.update_guard_tunnel_network_state(state),
            AiStateType::GuardRetaliate => self.update_guard_retaliate_state(state),
            AiStateType::Hunt => self.update_hunt_state(state),
            AiStateType::FollowPath | AiStateType::FollowExitProductionPath => {
                self.update_follow_path_state(state)
            }
            AiStateType::FollowWaypointPathAsTeam | AiStateType::FollowWaypointPathAsTeamExact => {
                self.update_follow_waypoint_path_state(state, true)
            }
            AiStateType::FollowWaypointPathAsIndividuals
            | AiStateType::FollowWaypointPathAsIndividualsExact => {
                self.update_follow_waypoint_path_state(state, false)
            }
            AiStateType::Wander => self.update_wander_state(state),
            AiStateType::WanderInPlace => self.update_wander_in_place_state(state),
            AiStateType::Panic => self.update_panic_state(state),
            AiStateType::Dead => Ok(StateReturnType::Continue), // Dead units don't do anything
            AiStateType::Busy => Ok(StateReturnType::Continue), // Busy state just continues
            AiStateType::Wait => Ok(StateReturnType::Continue), // Wait state just continues
            AiStateType::Dock
            | AiStateType::Enter
            | AiStateType::Exit
            | AiStateType::GetRepaired
            | AiStateType::HackInternet => Ok(StateReturnType::Continue),
            AiStateType::ExitInstantly => self.update_exit_instantly_state(state),
            AiStateType::MoveAwayFromRepulsors => self.update_move_away_from_repulsors(state),
            AiStateType::RappelInto => self.update_rappel_into_state(state),
            // Add other states as needed
            _ => {
                log::warn!("Unimplemented AI state: {:?}", state.state_type);
                Ok(StateReturnType::Continue)
            }
        }
    }

    /// Enter state callback
    fn on_state_enter(&mut self, state: &mut AiStateData) -> Result<(), AiError> {
        log::debug!(
            "AI {} entering state: {:?}",
            self.owner_id,
            state.state_type
        );

        match state.state_type {
            AiStateType::MoveTo
            | AiStateType::AttackMoveTo
            | AiStateType::MoveOutOfTheWay
            | AiStateType::MoveAndEvacuate
            | AiStateType::MoveAndEvacuateAndExit
            | AiStateType::MoveAndDelete
            | AiStateType::MoveAndTighten => {
                // Request pathfinding from current position to goal
                // Reference: C++ AIInternalMoveToState::onEnter() from AIStates.cpp
                if let Some(goal) = state.goal_position {
                    log::debug!("Moving to position: {:?}", goal);

                    // Store pathfinding request state
                    state.scratch.pathfinding_requested = false;
                    state.scratch.path_following_index = 0;
                    state.scratch.path_goal_position = Some(goal);
                    state.scratch.path_timestamp = TheGameLogic::get_frame();
                }

                if let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_id) {
                    if let Ok(mut owner_guard) = owner.write() {
                        owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                        if is_cliff_at(owner_guard.get_position()) {
                            owner_guard.set_model_condition_state(ModelConditionFlags::CLIMBING);
                            owner_guard
                                .clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                        }
                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                if owner_guard
                                    .test_status(crate::common::ObjectStatusTypes::Parachuting)
                                    || !ai_guard.is_allowed_to_adjust_destination()
                                {
                                    state.adjust_destinations = false;
                                }
                                if let Some(locomotor) = ai_guard.get_cur_locomotor() {
                                    if let Ok(loco_guard) = locomotor.lock() {
                                        if loco_guard.is_ultra_accurate() {
                                            state.adjust_destinations = false;
                                        }
                                    }
                                }
                                ai_guard.set_adjusts_destination(state.adjust_destinations);
                                let _ = ai_guard.set_path_extra_distance(0.0);
                            }
                        }
                    }
                }
                self.start_move_sound(state);
            }
            AiStateType::AttackObject | AiStateType::AttackPosition => {
                // Initialize attack parameters
                // Reference: C++ AIAttackState::onEnter() from AIStates.cpp
                if let Some(target) = state.goal_object {
                    log::debug!("Starting attack on object: {}", target);
                }

                // Initialize weapon ready state
                state.scratch.weapon_ready = false;
                state.scratch.last_fire_frame = 0;
            }
            AiStateType::AttackArea => {
                // Stagger scans to avoid spikes (matches C++ AIAttackAreaState::onEnter)
                const ENEMY_SCAN_RATE: i32 = LOGICFRAMES_PER_SECOND as i32;
                let now = TheGameLogic::get_frame() as i32;
                let jitter = game_logic_random_value(0, ENEMY_SCAN_RATE as u32) as i32;
                state.scratch.attack_area_next_scan_frame = now + jitter;
            }
            AiStateType::Guard => {
                // Set up guard parameters - Reference: C++ AIGuardState from AIGuard.cpp
                if let Some(pos) = state.goal_position {
                    log::debug!("Starting guard at position: {:?}", pos);

                    // Store guard position and last scan frame
                    state.scratch.guard_position = Some(pos);
                    state.scratch.last_scan_frame = 0;
                    state.scratch.enemy_found = 0;
                }
            }
            AiStateType::Hunt => {
                // Initialize hunt state - Reference: C++ AIHuntState from AIStates.cpp
                state.scratch.last_hunt_scan_frame = 0;
                state.scratch.current_hunt_target = 0;
            }
            AiStateType::FaceObject | AiStateType::FacePosition => {
                // C++ AIFaceState::onEnter caches whether this locomotor can turn in place.
                state.scratch.face_can_turn_in_place = false;
                if let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_id) {
                    if let Ok(owner_guard) = owner.read() {
                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                            if let Ok(ai_guard) = ai.lock() {
                                if let Some(locomotor) = ai_guard.get_cur_locomotor() {
                                    if let Ok(loco_guard) = locomotor.lock() {
                                        state.scratch.face_can_turn_in_place =
                                            loco_guard.template.min_speed == 0.0;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {} // Most states don't need special enter logic
        }

        Ok(())
    }

    /// Exit state callback
    fn on_state_exit(&self, state: &AiStateData, exit_type: StateExitType) -> Result<(), AiError> {
        log::debug!(
            "AI {} exiting state: {:?} with {:?}",
            self.owner_id,
            state.state_type,
            exit_type
        );

        match state.state_type {
            AiStateType::MoveTo
            | AiStateType::AttackMoveTo
            | AiStateType::MoveOutOfTheWay
            | AiStateType::MoveAndEvacuate
            | AiStateType::MoveAndEvacuateAndExit
            | AiStateType::MoveAndDelete
            | AiStateType::MoveAndTighten => {
                if state.scratch.move_sound_handle != 0 {
                    if let Some(audio) = TheAudio::get() {
                        audio.remove_audio_event(state.scratch.move_sound_handle);
                    }
                }
                if let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_id) {
                    if let Ok(mut owner_guard) = owner.write() {
                        if let Some(ai) = owner_guard.get_ai_update_interface() {
                            if let Ok(mut ai_guard) = ai.lock() {
                                ai_guard.destroy_path();
                            }
                        }
                        owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                        owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                        owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                    }
                }
            }
            AiStateType::AttackObject => {
                // Stop attack animations, release weapon lock, etc.
            }
            _ => {} // Most states don't need special exit logic
        }

        Ok(())
    }

    // State update implementations

    /// Update idle state - scan for targets based on attitude
    /// Reference: C++ AIIdleState::update() from AIStates.cpp line ~1000
    fn update_idle_state(&self, state: &mut AiStateData) -> Result<StateReturnType, AiError> {
        // Idle state scans for enemies if aggressive/alert
        // Reference: C++ AIIdleState uses attitude to determine whether to scan

        match state.attitude {
            AttitudeType::Aggressive | AttitudeType::Alert => {
                // Scan for nearby enemies (would need object manager integration)
                // For now, just continue idle
                log::trace!(
                    "AI {} idle (aggressive/alert), should scan for enemies",
                    self.owner_id
                );
            }
            AttitudeType::Normal => {
                // Normal attitude - just wait
                log::trace!("AI {} idle (normal)", self.owner_id);
            }
            AttitudeType::Passive | AttitudeType::Sleep => {
                // Passive/sleep - do nothing
                log::trace!("AI {} idle (passive/sleep)", self.owner_id);
            }
            _ => {}
        }

        Ok(StateReturnType::Continue)
    }

    /// Update move to state - handle pathfinding and movement
    /// Reference: C++ AIInternalMoveToState::update() from AIStates.cpp line ~2000
    fn update_move_to_state(&self, state: &mut AiStateData) -> Result<StateReturnType, AiError> {
        const MIN_REPATH_TIME: u32 = 10;

        if let Some(goal_obj_id) = state.goal_object {
            if let Some(goal_obj) = OBJECT_REGISTRY.get_object(goal_obj_id) {
                if let Ok(goal_guard) = goal_obj.read() {
                    let mut new_goal = *goal_guard.get_position();
                    if let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_id) {
                        if let Ok(owner_guard) = owner.read() {
                            if owner_guard.is_kind_of(KindOf::Projectile) {
                                let half_height = goal_guard
                                    .get_geometry_info()
                                    .get_max_height_above_position()
                                    * 0.5;
                                new_goal.z += half_height;
                                if goal_guard.get_position().z < new_goal.z {
                                    new_goal.z += half_height;
                                }
                            }
                        }
                    }

                    let mut repath = false;
                    if let Some(prev_goal) = state.scratch.path_goal_position {
                        if let Some(owner_pos) = self.resolve_current_position(state) {
                            let diff = new_goal - prev_goal;
                            let to_target = new_goal - owner_pos;
                            let tolerance_sqr =
                                (to_target.x * to_target.x + to_target.y * to_target.y) * 0.01;
                            if diff.x * diff.x + diff.y * diff.y > tolerance_sqr {
                                let now = TheGameLogic::get_frame();
                                if now.saturating_sub(state.scratch.path_timestamp)
                                    > MIN_REPATH_TIME
                                {
                                    repath = true;
                                    state.scratch.path_timestamp = now;
                                }
                            }
                        }
                    }

                    state.goal_position = Some(new_goal);
                    if repath {
                        state.goal_path.clear();
                        state.path_index = 0;
                        state.scratch.pathfinding_requested = false;
                        state.scratch.last_move_target = None;
                        state.scratch.path_goal_position = Some(new_goal);
                    }
                }
            }
        }

        let goal = state.goal_position.ok_or(AiError::InvalidTarget)?;

        if matches!(
            state.state_type,
            AiStateType::MoveAndEvacuate | AiStateType::MoveAndEvacuateAndExit
        ) && state.scratch.move_origin.is_none()
        {
            state.scratch.move_origin = self.resolve_current_position(state);
        }

        if let Some(obj) = OBJECT_REGISTRY.get_object(self.owner_id) {
            if let Ok(obj_guard) = obj.read() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        let blocked = ai_guard.is_blocked_and_stuck()
                            || ai_guard.get_num_frames_blocked() > 2 * LOGICFRAMES_PER_SECOND;
                        if blocked {
                            state.goal_path.clear();
                            state.path_index = 0;
                            state.scratch.pathfinding_requested = false;
                            state.scratch.last_move_target = None;
                            return Ok(StateReturnType::Continue);
                        }
                    }
                }
            }
        }

        if let Some(obj) = OBJECT_REGISTRY.get_object(self.owner_id) {
            if let Ok(mut obj_guard) = obj.write() {
                let mut frames_blocked = 0;
                let mut moving_backwards = false;
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        frames_blocked = ai_guard.get_num_frames_blocked();
                        moving_backwards = ai_guard
                            .get_cur_locomotor()
                            .and_then(|loc| loc.lock().ok().map(|loco| loco.is_moving_backwards()))
                            .unwrap_or(false);
                    }
                }

                if frames_blocked > LOGICFRAMES_PER_SECOND / 4 {
                    obj_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                    obj_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                    obj_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                } else {
                    obj_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                    let mut set_flag = ModelConditionFlags::MOVING;
                    if is_cliff_at(obj_guard.get_position()) {
                        set_flag = if moving_backwards {
                            ModelConditionFlags::RAPPELLING
                        } else {
                            ModelConditionFlags::CLIMBING
                        };
                    }

                    if set_flag == ModelConditionFlags::MOVING {
                        obj_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                        obj_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                    } else {
                        let clear_flag = if set_flag == ModelConditionFlags::CLIMBING {
                            ModelConditionFlags::RAPPELLING
                        } else {
                            ModelConditionFlags::CLIMBING
                        };
                        obj_guard.clear_model_condition_state(clear_flag);
                        obj_guard.set_model_condition_state(set_flag);
                    }
                }
            }
        }

        // Check if we need to request pathfinding
        let pathfinding_requested = state.scratch.pathfinding_requested;

        if !pathfinding_requested {
            // Request pathfinding from integration manager
            // Reference: C++ AIInternalMoveToState uses AIGroup::requestPath
            log::debug!("AI {} requesting pathfinding to {:?}", self.owner_id, goal);

            // Mark pathfinding as requested
            state.scratch.pathfinding_requested = true;

            state.scratch.path_goal_position = Some(goal);
            state.scratch.path_timestamp = TheGameLogic::get_frame();

            let mut acceptable_surfaces = 0u32;
            let mut is_crusher = false;
            let mut layer = PathfindLayerEnum::Ground;
            let mut surfaces_from_locomotor = None;

            if let Some(obj) = OBJECT_REGISTRY.get_object(self.owner_id) {
                if let Ok(obj_guard) = obj.read() {
                    if obj_guard.is_kind_of(KindOf::Amphibious)
                        || obj_guard.is_kind_of(KindOf::AmphibiousTransport)
                    {
                        acceptable_surfaces |= SURFACE_WATER;
                    }

                    if obj_guard.is_kind_of(KindOf::CliffJumper) {
                        acceptable_surfaces |= SURFACE_CLIFF;
                    }

                    if obj_guard.get_crusher_level() > 0 {
                        acceptable_surfaces |= SURFACE_RUBBLE;
                        is_crusher = true;
                    }

                    if obj_guard.is_kind_of(KindOf::Aircraft) {
                        layer = PathfindLayerEnum::Top;
                        acceptable_surfaces |= SURFACE_WATER | SURFACE_CLIFF | SURFACE_RUBBLE;
                    }
                }
            }

            if let Ok(factory_guard) = get_object_factory().read() {
                if let Some(GameObjectInstance::Unit(unit)) =
                    factory_guard.get_object(self.owner_id)
                {
                    if let Ok(unit_guard) = unit.read() {
                        surfaces_from_locomotor = unit_guard.get_locomotor_surface_mask();
                        layer = unit_guard.get_pathfind_layer();
                        if unit_guard.get_crusher_level() > 0 {
                            is_crusher = true;
                            acceptable_surfaces |= SURFACE_RUBBLE;
                        }
                    }
                }
            }

            if let Some(surface_mask) = surfaces_from_locomotor {
                acceptable_surfaces = surface_mask;
                if is_crusher {
                    acceptable_surfaces |= SURFACE_RUBBLE;
                }
                if (acceptable_surfaces & SURFACE_GROUND) == 0 {
                    acceptable_surfaces |= SURFACE_GROUND;
                }
            } else if acceptable_surfaces == 0 {
                acceptable_surfaces = SURFACE_GROUND;
            }

            let _ = (acceptable_surfaces, is_crusher, layer);
            state.goal_path = vec![goal];
            state.path_index = 0;

            return Ok(StateReturnType::Continue);
        }

        if state.goal_path.is_empty() {
            state.goal_path = vec![goal];
            state.path_index = 0;
        }

        // Follow the path if we have one
        if !state.goal_path.is_empty() && state.path_index < state.goal_path.len() {
            let current_waypoint = state.goal_path[state.path_index];

            self.issue_movement_target(state, current_waypoint);

            // In a real implementation, we would:
            // 1. Get unit's current position from object manager
            // 2. Check distance to current waypoint
            // 3. If close enough, advance to next waypoint
            // 4. Otherwise, apply locomotor movement toward waypoint

            let current_pos = self.resolve_current_position(state);

            let dist_to_waypoint = current_pos
                .map(|pos| (current_waypoint - pos).length())
                .unwrap_or(10.0);

            let mut close_enough = 5.0;
            if let Some(obj) = OBJECT_REGISTRY.get_object(self.owner_id) {
                if let Ok(obj_guard) = obj.read() {
                    if let Some(ai) = obj_guard.get_ai_update_interface() {
                        if let Ok(ai_guard) = ai.lock() {
                            if let Some(locomotor) = ai_guard.get_cur_locomotor() {
                                if let Ok(loco_guard) = locomotor.lock() {
                                    close_enough = loco_guard.get_close_enough_dist();
                                }
                            }
                        }
                    }
                }
            }

            if dist_to_waypoint < close_enough {
                // Reached this waypoint, advance to next
                state.path_index += 1;

                if state.path_index >= state.goal_path.len() {
                    // Reached destination
                    log::debug!("AI {} reached destination", self.owner_id);
                    return self.finish_move_state(state);
                }

                log::debug!(
                    "AI {} reached waypoint {}/{}",
                    self.owner_id,
                    state.path_index,
                    state.goal_path.len()
                );
            }

            // Continue moving
            return Ok(StateReturnType::Continue);
        }

        // No path or reached end of path
        if state.goal_path.is_empty() {
            log::warn!("AI {} has no path to follow", self.owner_id);
            return Ok(StateReturnType::StateFailed);
        }

        // Reached destination
        self.finish_move_state(state)
    }

    /// Update attack object state - fire weapons at target object
    /// Reference: C++ AIAttackFireWeaponState::update() from AIStates.cpp line ~4500
    fn update_attack_object_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let target_id = state.goal_object.ok_or(AiError::InvalidTarget)?;

        let target_arc = OBJECT_REGISTRY
            .get_object(target_id)
            .ok_or(AiError::InvalidTarget)?;
        let Ok(target_guard) = target_arc.read() else {
            return Err(AiError::LockFailed);
        };
        if target_guard.is_effectively_dead() {
            log::debug!(
                "AI {} attack target {} is no longer valid",
                self.owner_id,
                target_id
            );
            return Ok(StateReturnType::StateFailed);
        }

        if let Some(attacker_arc) = OBJECT_REGISTRY.get_object(self.owner_id) {
            if let Ok(attacker_guard) = attacker_arc.read() {
                let attack_uses_los = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.get_ai_data()
                            .read()
                            .ok()
                            .map(|data| data.attack_uses_line_of_sight)
                    })
                    .unwrap_or(false);

                if attack_uses_los
                    && !attacker_guard.is_significantly_above_terrain()
                    && !target_guard.is_significantly_above_terrain()
                {
                    let blocked = THE_AI
                        .read()
                        .ok()
                        .and_then(|ai| {
                            let pf_arc = ai.pathfinder()?;
                            let pf = pf_arc.read().ok()?;
                            Some(pf.is_attack_view_blocked_by_obstacle(
                                &attacker_guard,
                                attacker_guard.get_position(),
                                Some(&target_guard),
                                target_guard.get_position(),
                            ))
                        })
                        .unwrap_or(false);
                    if blocked {
                        return Ok(StateReturnType::StateBlocked);
                    }
                }
            }
        }

        let current_frame = self.current_frame();
        self.try_fire_weapon(state, current_frame, format!("target {}", target_id))
    }

    /// Update attack position state - fire weapons at ground position
    /// Reference: C++ AIAttackFireWeaponState for position attacks from AIStates.cpp
    fn update_attack_position_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let target_pos = state.goal_position.ok_or(AiError::InvalidTarget)?;

        if let Some(attacker_arc) = OBJECT_REGISTRY.get_object(self.owner_id) {
            if let Ok(attacker_guard) = attacker_arc.read() {
                let attack_uses_los = THE_AI
                    .read()
                    .ok()
                    .and_then(|ai| {
                        ai.get_ai_data()
                            .read()
                            .ok()
                            .map(|data| data.attack_uses_line_of_sight)
                    })
                    .unwrap_or(false);
                if attack_uses_los && !attacker_guard.is_significantly_above_terrain() {
                    let blocked = THE_AI
                        .read()
                        .ok()
                        .and_then(|ai| {
                            let pf_arc = ai.pathfinder()?;
                            let pf = pf_arc.read().ok()?;
                            Some(pf.is_attack_view_blocked_by_obstacle(
                                &attacker_guard,
                                attacker_guard.get_position(),
                                None,
                                &target_pos,
                            ))
                        })
                        .unwrap_or(false);
                    if blocked {
                        return Ok(StateReturnType::StateBlocked);
                    }
                }
            }
        }

        let current_frame = self.current_frame();
        self.try_fire_weapon(state, current_frame, format!("position {:?}", target_pos))
    }

    /// Update attack-move state - advance toward goal while engaging enemies
    fn update_attack_move_to_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let current_frame = self.current_frame();
        let move_result = self.update_move_to_state(state)?;

        let (scan_rate, qualifiers, range) = {
            let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
            let ai_data = ai.get_ai_data();
            let Ok(ai_data_guard) = ai_data.read() else {
                return Err(AiError::LockFailed);
            };
            let mut qualifiers =
                search_qualifiers::CAN_ATTACK | search_qualifiers::ATTACK_BUILDINGS;
            if ai_data_guard.attack_uses_line_of_sight {
                qualifiers |= search_qualifiers::CAN_SEE;
            }
            if ai_data_guard.attack_ignore_insignificant_buildings {
                qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
            }
            let factors = vision_factors::OWNER_TYPE | vision_factors::MOOD;
            let range = ai.get_adjusted_vision_range_for_object(self.owner_id, factors)?;
            let scan_rate = (ai_data_guard.guard_enemy_scan_rate.max(1) / 2).max(1) as i32;
            (scan_rate, qualifiers, range)
        };

        if (current_frame - state.scratch.last_hunt_scan_frame) >= scan_rate {
            state.scratch.last_hunt_scan_frame = current_frame;
            let attack_priority = resolve_attack_priority_info_for_object(self.owner_id);
            let new_target = {
                let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
                ai.find_closest_enemy(
                    self.owner_id,
                    range,
                    qualifiers,
                    attack_priority.as_ref(),
                    None,
                )?
            };
            if let Some(target_id) = new_target {
                state.goal_object = Some(target_id);
                let attack_result = self.update_attack_object_state(state)?;
                if matches!(
                    attack_result,
                    StateReturnType::StateFailed | StateReturnType::StateComplete
                ) {
                    state.goal_object = None;
                }
            }
        }

        if matches!(move_result, StateReturnType::StateComplete) {
            return Ok(StateReturnType::StateComplete);
        }

        Ok(StateReturnType::Continue)
    }

    /// Update attack area state - scan within polygon/area and attack targets
    fn update_attack_area_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        const ENEMY_SCAN_RATE: i32 = LOGICFRAMES_PER_SECOND as i32;
        let current_frame = self.current_frame();

        if state.scratch.attack_area_next_scan_frame == 0 {
            let jitter = game_logic_random_value(0, ENEMY_SCAN_RATE as u32) as i32;
            state.scratch.attack_area_next_scan_frame = current_frame + jitter;
        }

        if current_frame >= state.scratch.attack_area_next_scan_frame {
            let owner_arc = OBJECT_REGISTRY
                .get_object(self.owner_id)
                .ok_or(AiError::InvalidTarget)?;
            let Ok(owner_guard) = owner_arc.read() else {
                return Err(AiError::LockFailed);
            };
            if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                return Ok(StateReturnType::StateFailed);
            }

            state.scratch.attack_area_next_scan_frame = current_frame + ENEMY_SCAN_RATE;

            let mut filter_polygon = None;
            let range = if let Some(polygon_id) = state.goal_polygon {
                if let Ok(terrain_guard) = get_terrain_logic().read() {
                    if let Some(trigger) = terrain_guard.get_trigger_areas().get_by_id(polygon_id) {
                        let center = trigger.get_center_point();
                        let min = trigger.get_bounds_min();
                        let max = trigger.get_bounds_max();
                        let dx = (center.x - min.x as f32)
                            .abs()
                            .max((center.x - max.x as f32).abs());
                        let dy = (center.y - min.y as f32)
                            .abs()
                            .max((center.y - max.y as f32).abs());
                        let radius = (dx * dx + dy * dy).sqrt();
                        filter_polygon = Some(trigger.clone());
                        radius
                    } else {
                        0.0
                    }
                } else {
                    0.0
                }
            } else {
                0.0
            };

            let qualifiers = search_qualifiers::CAN_ATTACK;
            let attack_priority = resolve_attack_priority_info_for_object(self.owner_id);

            let enemy_found = {
                let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
                if let Some(polygon) = filter_polygon {
                    struct PolygonFilter {
                        polygon: crate::polygon_trigger::PolygonTrigger,
                    }

                    impl PartitionFilter for PolygonFilter {
                        fn allow(&self, obj: ObjectID) -> bool {
                            let Some(target_arc) = OBJECT_REGISTRY.get_object(obj) else {
                                return false;
                            };
                            let Ok(target) = target_arc.read() else {
                                return false;
                            };
                            let pos = target.get_position();
                            let point = Coord2D::new(pos.x, pos.y);
                            self.polygon.point_in_trigger(&point)
                        }

                        fn debug_get_name(&self) -> &str {
                            "PolygonFilter"
                        }
                    }

                    let filter = PolygonFilter { polygon };
                    ai.find_closest_enemy(
                        self.owner_id,
                        range.max(1.0),
                        qualifiers,
                        attack_priority.as_ref(),
                        Some(&filter),
                    )?
                } else {
                    ai.find_closest_enemy(
                        self.owner_id,
                        range.max(1.0),
                        qualifiers,
                        attack_priority.as_ref(),
                        None,
                    )?
                }
            };

            if let Some(enemy_id) = enemy_found {
                state.goal_object = Some(enemy_id);
                let attack_result = self.update_attack_object_state(state)?;
                if matches!(
                    attack_result,
                    StateReturnType::StateFailed | StateReturnType::StateComplete
                ) {
                    state.goal_object = None;
                }
                return Ok(StateReturnType::Continue);
            }

            return Ok(StateReturnType::StateComplete);
        }

        if state.goal_object.is_some() {
            let attack_result = self.update_attack_object_state(state)?;
            if matches!(
                attack_result,
                StateReturnType::StateFailed | StateReturnType::StateComplete
            ) {
                state.goal_object = None;
            }
        }

        Ok(StateReturnType::Continue)
    }

    /// Update attack squad state - pick victim from target squad and attack
    fn update_attack_squad_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let Some(squad_arc) = state.goal_squad_handle.clone() else {
            return Ok(StateReturnType::StateFailed);
        };
        let Ok(mut squad_guard) = squad_arc.lock() else {
            return Ok(StateReturnType::StateFailed);
        };

        let owner_pos = self.resolve_current_position(state);
        let mut best_target = None;
        let mut best_dist = f32::INFINITY;

        for id in squad_guard.get_live_object_ids() {
            let Some(target_arc) = OBJECT_REGISTRY.get_object(id) else {
                continue;
            };
            let Ok(target) = target_arc.read() else {
                continue;
            };
            if target.is_effectively_dead() {
                continue;
            }

            if let Some(owner_pos) = owner_pos {
                let pos = target.get_position();
                let dx = pos.x - owner_pos.x;
                let dy = pos.y - owner_pos.y;
                let dist = dx * dx + dy * dy;
                if dist < best_dist {
                    best_dist = dist;
                    best_target = Some(id);
                }
            } else {
                best_target = Some(id);
                break;
            }
        }

        let Some(target_id) = best_target else {
            return Ok(StateReturnType::StateComplete);
        };

        state.goal_object = Some(target_id);
        let result = self.update_attack_object_state(state)?;
        if matches!(
            result,
            StateReturnType::StateFailed | StateReturnType::StateComplete
        ) {
            state.goal_object = None;
        }

        Ok(StateReturnType::Continue)
    }

    /// Update attack-follow waypoint path state - follow path and attack opportunistically
    fn update_attack_follow_waypoint_path_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let move_result = if matches!(
            state.state_type,
            AiStateType::AttackFollowWaypointPathAsTeam
        ) {
            self.update_follow_waypoint_path_state(state, true)?
        } else {
            self.update_follow_waypoint_path_state(state, false)?
        };

        let current_frame = self.current_frame();
        let (scan_rate, qualifiers, range) = {
            let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
            let ai_data = ai.get_ai_data();
            let Ok(ai_data_guard) = ai_data.read() else {
                return Err(AiError::LockFailed);
            };

            let mut qualifiers =
                search_qualifiers::CAN_ATTACK | search_qualifiers::ATTACK_BUILDINGS;
            if ai_data_guard.attack_uses_line_of_sight {
                qualifiers |= search_qualifiers::CAN_SEE;
            }
            if ai_data_guard.attack_ignore_insignificant_buildings {
                qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
            }

            let factors = vision_factors::OWNER_TYPE | vision_factors::MOOD;
            let range = ai.get_adjusted_vision_range_for_object(self.owner_id, factors)?;
            let scan_rate = (ai_data_guard.guard_enemy_scan_rate.max(1) / 2).max(1) as i32;
            (scan_rate, qualifiers, range)
        };

        if (current_frame - state.scratch.last_hunt_scan_frame) >= scan_rate {
            state.scratch.last_hunt_scan_frame = current_frame;
            let attack_priority = resolve_attack_priority_info_for_object(self.owner_id);
            let enemy = {
                let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
                ai.find_closest_enemy(
                    self.owner_id,
                    range,
                    qualifiers,
                    attack_priority.as_ref(),
                    None,
                )?
            };
            if let Some(enemy_id) = enemy {
                state.goal_object = Some(enemy_id);
                let result = self.update_attack_object_state(state)?;
                if matches!(
                    result,
                    StateReturnType::StateFailed | StateReturnType::StateComplete
                ) {
                    state.goal_object = None;
                }
            }
        }

        if matches!(move_result, StateReturnType::StateComplete) {
            return Ok(StateReturnType::StateComplete);
        }

        Ok(StateReturnType::Continue)
    }

    fn update_face_object_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let Some(goal_id) = state.goal_object else {
            return Ok(StateReturnType::StateFailed);
        };
        let Some(target_arc) = OBJECT_REGISTRY.get_object(goal_id) else {
            return Ok(StateReturnType::StateFailed);
        };
        let Ok(target_guard) = target_arc.read() else {
            return Err(AiError::LockFailed);
        };
        self.update_face_towards(state, *target_guard.get_position())
    }

    fn update_face_position_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let Some(goal_pos) = state.goal_position else {
            return Ok(StateReturnType::StateFailed);
        };
        self.update_face_towards(state, goal_pos)
    }

    fn update_face_towards(
        &self,
        state: &mut AiStateData,
        target_pos: Coord3D,
    ) -> Result<StateReturnType, AiError> {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.owner_id) else {
            return Ok(StateReturnType::StateFailed);
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return Err(AiError::LockFailed);
        };
        let Some(ai) = owner_guard.get_ai_update_interface() else {
            return Ok(StateReturnType::StateFailed);
        };
        let Ok(mut ai_guard) = ai.lock() else {
            return Err(AiError::LockFailed);
        };

        let owner_pos = owner_guard.get_position();
        let owner_orientation = owner_guard.get_orientation();
        let rel_angle = relative_angle_2d(owner_pos, owner_orientation, &target_pos);

        const REL_THRESH: Real = 0.035;
        if rel_angle.abs() < REL_THRESH {
            return Ok(StateReturnType::StateComplete);
        }

        if state.scratch.face_can_turn_in_place {
            let desired_angle = owner_orientation + rel_angle;
            ai_guard.set_locomotor_goal_orientation(desired_angle);
        } else {
            ai_guard.set_locomotor_goal_position_explicit(target_pos);
        }

        Ok(StateReturnType::Continue)
    }

    fn update_guard_tunnel_network_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        self.update_guard_state(state)
    }

    fn update_guard_retaliate_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        self.update_guard_state(state)
    }

    fn update_exit_instantly_state(
        &self,
        _state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.owner_id) else {
            return Ok(StateReturnType::StateFailed);
        };
        let Ok(mut owner_guard) = owner_arc.write() else {
            return Err(AiError::LockFailed);
        };

        if owner_guard.is_effectively_dead() {
            return Ok(StateReturnType::StateFailed);
        }

        self.release_from_container(&owner_guard);
        self.evacuate_contents(&mut owner_guard);
        Ok(StateReturnType::StateComplete)
    }

    /// Update guard state - scan for enemies and defend position
    /// Reference: C++ AIGuardInnerState/AIGuardOuterState from AIGuard.cpp
    fn update_guard_state(&self, state: &mut AiStateData) -> Result<StateReturnType, AiError> {
        struct GuardFlyingOnlyFilter;

        impl PartitionFilter for GuardFlyingOnlyFilter {
            fn allow(&self, obj: ObjectID) -> bool {
                let Some(target_arc) = OBJECT_REGISTRY.get_object(obj) else {
                    return false;
                };
                let Ok(target) = target_arc.read() else {
                    return false;
                };
                target.is_airborne_target() || target.is_kind_of(KindOf::Aircraft)
            }

            fn debug_get_name(&self) -> &str {
                "GuardFlyingOnlyFilter"
            }
        }

        let guard_pos = state
            .scratch
            .guard_position
            .or(state.goal_position)
            .or_else(|| self.resolve_current_position(state))
            .ok_or(AiError::InvalidTarget)?;
        state.scratch.guard_position = Some(guard_pos);

        // Get last scan frame
        let last_scan = state.scratch.last_scan_frame;

        let current_frame = self.current_frame();

        let (guard_scan_rate, qualifiers, range) = {
            let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
            let ai_data = ai.get_ai_data();
            let Ok(ai_data_guard) = ai_data.read() else {
                return Err(AiError::LockFailed);
            };

            let mut qualifiers =
                search_qualifiers::CAN_ATTACK | search_qualifiers::ATTACK_BUILDINGS;
            if ai_data_guard.attack_uses_line_of_sight {
                qualifiers |= search_qualifiers::CAN_SEE;
            }
            if ai_data_guard.attack_ignore_insignificant_buildings {
                qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
            }

            let mut factors = vision_factors::OWNER_TYPE | vision_factors::MOOD;
            if matches!(state.guard_mode, GuardMode::GuardWithoutPursuit) {
                // Patrol mode limits pursuit to the guard area.
                factors |= vision_factors::GUARD_INNER;
            }

            let range = ai.get_adjusted_vision_range_for_object(self.owner_id, factors)?;
            let scan_rate = ai_data_guard.guard_enemy_scan_rate.max(1) as i32;
            (scan_rate, qualifiers, range)
        };

        if (current_frame - last_scan) >= guard_scan_rate {
            state.scratch.last_scan_frame = current_frame;

            log::trace!(
                "AI {} scanning for enemies at guard position {:?}",
                self.owner_id,
                guard_pos
            );

            let attack_priority = resolve_attack_priority_info_for_object(self.owner_id);
            let enemy_found = {
                let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
                if matches!(state.guard_mode, GuardMode::GuardFlyingUnitsOnly) {
                    let filter = GuardFlyingOnlyFilter;
                    ai.find_closest_enemy(
                        self.owner_id,
                        range,
                        qualifiers,
                        attack_priority.as_ref(),
                        Some(&filter),
                    )?
                } else {
                    ai.find_closest_enemy(
                        self.owner_id,
                        range,
                        qualifiers,
                        attack_priority.as_ref(),
                        None,
                    )?
                }
            };

            if let Some(enemy_id) = enemy_found {
                state.scratch.enemy_found = enemy_id;
                log::debug!(
                    "AI {} found enemy {} while guarding",
                    self.owner_id,
                    enemy_id
                );
                return Ok(StateReturnType::StateComplete);
            }
        }

        // Continue guarding
        Ok(StateReturnType::Continue)
    }

    /// Update hunt state - actively seek and destroy enemies
    /// Reference: C++ AIHuntState from AIStates.cpp line ~5800
    fn update_hunt_state(&self, state: &mut AiStateData) -> Result<StateReturnType, AiError> {
        let last_scan = state.scratch.last_hunt_scan_frame;
        let current_hunt_target = state.scratch.current_hunt_target;

        let current_frame = self.current_frame();

        let (hunt_scan_rate, qualifiers, range) = {
            let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
            let ai_data = ai.get_ai_data();
            let Ok(ai_data_guard) = ai_data.read() else {
                return Err(AiError::LockFailed);
            };

            let mut qualifiers =
                search_qualifiers::CAN_ATTACK | search_qualifiers::ATTACK_BUILDINGS;
            if ai_data_guard.attack_uses_line_of_sight {
                qualifiers |= search_qualifiers::CAN_SEE;
            }
            if ai_data_guard.attack_ignore_insignificant_buildings {
                qualifiers |= search_qualifiers::IGNORE_INSIGNIFICANT_BUILDINGS;
            }

            let factors = vision_factors::OWNER_TYPE | vision_factors::MOOD;
            let range = ai.get_adjusted_vision_range_for_object(self.owner_id, factors)?;
            let scan_rate = (ai_data_guard.guard_enemy_scan_rate.max(1) / 2).max(1) as i32;
            (scan_rate, qualifiers, range)
        };

        if current_hunt_target == 0 || (current_frame - last_scan) >= hunt_scan_rate {
            state.scratch.last_hunt_scan_frame = current_frame;

            log::trace!("AI {} hunting for enemies", self.owner_id);

            let attack_priority = resolve_attack_priority_info_for_object(self.owner_id);
            let new_target = {
                let ai = THE_AI.read().map_err(|_| AiError::LockFailed)?;
                ai.find_closest_enemy(
                    self.owner_id,
                    range,
                    qualifiers,
                    attack_priority.as_ref(),
                    None,
                )?
            };

            if let Some(target_id) = new_target {
                if target_id != current_hunt_target {
                    state.scratch.current_hunt_target = target_id;
                    log::debug!("AI {} acquired hunt target {}", self.owner_id, target_id);
                    return Ok(StateReturnType::StateComplete);
                }
            }
        }

        // Continue hunting
        Ok(StateReturnType::Continue)
    }

    fn update_follow_path_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        if state.goal_path.is_empty() {
            return Ok(StateReturnType::StateComplete);
        }

        if state.path_index >= state.goal_path.len() {
            return Ok(StateReturnType::StateComplete);
        }

        let current_target = state.goal_path[state.path_index];
        self.issue_movement_target(state, current_target);
        let current_pos = self.resolve_current_position(state);

        if let Some(pos) = current_pos {
            let dist_to_waypoint = (current_target - pos).length();
            const CLOSE_ENOUGH_DIST: Real = 5.0;

            if dist_to_waypoint <= CLOSE_ENOUGH_DIST {
                state.path_index += 1;
            }
        } else {
            // Fallback to previous behavior when position data is unavailable.
            state.path_index += 1;
        }

        Ok(StateReturnType::Continue)
    }

    fn issue_movement_target(&self, state: &mut AiStateData, target: Coord3D) {
        const TARGET_EPSILON: Real = 0.1;

        if let Some(last_target) = state.scratch.last_move_target {
            if (target - last_target).length() <= TARGET_EPSILON {
                return;
            }
        }

        state.scratch.last_move_target = Some(target);

        if let Some(obj) = OBJECT_REGISTRY.get_object(self.owner_id) {
            if let Ok(obj_guard) = obj.read() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.set_movement_target(&target);
                        return;
                    }
                }
            }
        }

        if let Ok(factory_guard) = get_object_factory().read() {
            if let Some(game_object) = factory_guard.get_object(self.owner_id) {
                if let GameObjectInstance::Unit(unit) = game_object {
                    if let Ok(mut unit_guard) = unit.write() {
                        let _ = unit_guard.give_move_order(target, Vec::new(), false, false);
                    }
                }
            }
        }
    }

    fn start_move_sound(&self, state: &mut AiStateData) {
        let Some(owner) = OBJECT_REGISTRY.get_object(self.owner_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let mut use_damaged = false;
        if let Some(body) = owner_guard.get_body_module() {
            if let Ok(body_guard) = body.lock() {
                use_damaged = body_guard.get_damage_state() > BodyDamageType::Damaged;
            }
        }

        let template = owner_guard.get_template();
        let mut start_sound = if use_damaged {
            template.get_sound_move_start_damaged()
        } else {
            template.get_sound_move_start()
        };
        let loop_sound = if use_damaged {
            template.get_sound_move_loop_damaged()
        } else {
            template.get_sound_move_loop()
        };

        if start_sound.get_event_name().is_empty() {
            start_sound = loop_sound.clone();
        }

        if start_sound.get_event_name().is_empty() {
            return;
        }

        start_sound.set_object_id(owner_guard.get_id());
        if let Some(audio) = TheAudio::get() {
            if start_sound.get_event_name() == loop_sound.get_event_name()
                && !loop_sound.get_event_name().is_empty()
            {
                let handle = audio.add_audio_event(&start_sound);
                state.scratch.move_sound_handle = handle;
            } else {
                audio.add_audio_event(&start_sound);
            }
        }
    }

    fn resolve_current_position(&self, state: &AiStateData) -> Option<Coord3D> {
        state.scratch.current_position.or_else(|| {
            OBJECT_REGISTRY
                .get_object(self.owner_id)
                .and_then(|obj| obj.read().ok().map(|guard| *guard.get_position()))
        })
    }

    fn native_state_for(&self, state_type: AiStateType, state: &AiStateData) -> NativeState {
        match state_type {
            AiStateType::Idle => NativeState::Idle,
            AiStateType::MoveTo => NativeState::MoveTo {
                destination: state.goal_position.unwrap_or_else(Coord3D::origin),
            },
            AiStateType::AttackMoveTo => NativeState::AttackMoveTo {
                destination: state.goal_position.unwrap_or_else(Coord3D::origin),
            },
            AiStateType::AttackObject => NativeState::AttackObject {
                target_id: state.goal_object.unwrap_or(INVALID_ID),
            },
            AiStateType::AttackAndFollowObject => NativeState::AttackAndFollowObject {
                target_id: state.goal_object.unwrap_or(INVALID_ID),
            },
            AiStateType::AttackPosition => NativeState::AttackPosition {
                target: state.goal_position.unwrap_or_else(Coord3D::origin),
            },
            AiStateType::FollowWaypointPathAsTeam
            | AiStateType::FollowWaypointPathAsIndividuals
            | AiStateType::AttackFollowWaypointPathAsIndividuals
            | AiStateType::AttackFollowWaypointPathAsTeam => NativeState::FollowWaypointPath {
                path_id: state.goal_waypoint.unwrap_or(0),
                index: state.path_index,
            },
            AiStateType::FollowPath | AiStateType::FollowExitProductionPath => {
                NativeState::FollowPath {
                    path_len: state.goal_path.len(),
                    index: state.path_index,
                }
            }
            AiStateType::Guard => NativeState::Guard {
                position: state.goal_position.unwrap_or_else(Coord3D::origin),
            },
            AiStateType::Hunt => NativeState::Hunt,
            AiStateType::Wander => NativeState::Wander,
            AiStateType::WanderInPlace => NativeState::WanderInPlace {
                center: state.goal_position.unwrap_or_else(Coord3D::origin),
            },
            AiStateType::Panic => NativeState::Panic,
            other => NativeState::Custom(format!("{:?}", other)),
        }
    }

    fn current_frame(&self) -> i32 {
        TheGameLogic::get_frame() as i32
    }

    fn duration_to_frames(duration: Duration) -> u32 {
        let logic_fps = LOGICFRAMES_PER_SECOND as f64;
        if logic_fps <= 0.0 {
            return 0;
        }

        (duration.as_secs_f64() * logic_fps).ceil() as u32
    }

    fn frames_to_duration(frames: u32) -> Duration {
        let logic_fps = LOGICFRAMES_PER_SECOND as f64;
        if logic_fps <= 0.0 {
            return Duration::from_secs(0);
        }

        Duration::from_secs_f64(frames as f64 / logic_fps)
    }

    fn try_fire_weapon(
        &self,
        state: &mut AiStateData,
        current_frame: i32,
        label: String,
    ) -> Result<StateReturnType, AiError> {
        const WEAPON_DELAY_FRAMES: i32 = 30; // ~0.5 seconds at 60fps
        let can_fire = (current_frame - state.scratch.last_fire_frame) >= WEAPON_DELAY_FRAMES;

        if can_fire {
            log::debug!("AI {} firing at {}", self.owner_id, label);
            state.scratch.last_fire_frame = current_frame;

            if state.max_shots > 0 {
                state.max_shots -= 1;
                if state.max_shots <= 0 {
                    log::debug!("AI {} completed max shots", self.owner_id);
                    return Ok(StateReturnType::StateComplete);
                }
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn finish_move_state(&self, state: &mut AiStateData) -> Result<StateReturnType, AiError> {
        if matches!(
            state.state_type,
            AiStateType::MoveAndEvacuate
                | AiStateType::MoveAndEvacuateAndExit
                | AiStateType::MoveAndDelete
        ) {
            let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.owner_id) else {
                return Ok(StateReturnType::StateFailed);
            };
            let Ok(mut owner_guard) = owner_arc.write() else {
                return Err(AiError::LockFailed);
            };

            if owner_guard.is_effectively_dead() {
                return Ok(StateReturnType::StateFailed);
            }

            if matches!(
                state.state_type,
                AiStateType::MoveAndEvacuate | AiStateType::MoveAndEvacuateAndExit
            ) {
                self.evacuate_contents(&mut owner_guard);
                if let Some(origin) = state.scratch.move_origin {
                    state.goal_position = Some(origin);
                }
            }

            if matches!(
                state.state_type,
                AiStateType::MoveAndEvacuateAndExit | AiStateType::MoveAndDelete
            ) {
                let owner_id = owner_guard.get_id();
                drop(owner_guard);
                let _ = TheGameLogic::destroy_object_by_id(owner_id);
            }
        }

        Ok(StateReturnType::StateComplete)
    }

    fn evacuate_contents(&self, owner: &mut crate::object::Object) {
        if let Some(contain) = owner.get_contain() {
            if let Ok(mut contain_guard) = contain.lock() {
                let ids: Vec<ObjectID> = contain_guard.get_contained_objects().to_vec();
                for id in ids {
                    let _ = contain_guard.release_object(id);
                }
            }
        }

        if let Some(team) = owner.get_team() {
            if let Ok(mut team_guard) = team.write() {
                team_guard.set_active();
            }
        }
    }

    fn release_from_container(&self, owner: &crate::object::Object) {
        if let Some(container_arc) = owner.get_container() {
            if let Ok(container) = container_arc.write() {
                if let Some(contain) = container.get_contain() {
                    if let Ok(mut contain_guard) = contain.lock() {
                        let _ = contain_guard.release_object(owner.get_id());
                    }
                }
            }
        }
    }

    fn select_next_waypoint(
        &self,
        state: &mut AiStateData,
        node: &WaypointNode,
    ) -> Option<WaypointId> {
        if node.links.is_empty() {
            return None;
        }

        let next_index = state.path_index % node.links.len();
        state.path_index = state.path_index.saturating_add(1);
        Some(node.links[next_index])
    }

    /// Update follow waypoint path state - navigate waypoint chain
    /// Reference: C++ AIFollowWaypointPathState from AIStates.cpp line ~3200
    fn update_follow_waypoint_path_state(
        &self,
        state: &mut AiStateData,
        as_team: bool,
    ) -> Result<StateReturnType, AiError> {
        let waypoint_id = match state.goal_waypoint {
            Some(id) => id,
            None => return Ok(StateReturnType::StateComplete),
        };

        log::trace!(
            "AI {} following waypoint {} (team mode: {})",
            self.owner_id,
            waypoint_id,
            as_team
        );

        let Some(graph) = self.waypoint_graph.as_ref() else {
            return Ok(StateReturnType::Continue);
        };

        let Ok(graph_guard) = graph.read() else {
            return Ok(StateReturnType::Continue);
        };

        let Some(node) = graph_guard.get_by_id(waypoint_id) else {
            return Ok(StateReturnType::StateComplete);
        };

        let target_pos = node.position;
        self.issue_movement_target(state, target_pos);

        let current_pos = self.resolve_current_position(state).unwrap_or(target_pos);
        let dist_to_waypoint = (target_pos - current_pos).length();
        const CLOSE_ENOUGH_DIST: Real = 5.0;

        if dist_to_waypoint <= CLOSE_ENOUGH_DIST {
            state.goal_waypoint = self.select_next_waypoint(state, node);
            if state.goal_waypoint.is_none() {
                return Ok(StateReturnType::StateComplete);
            }
        }

        Ok(StateReturnType::Continue)
    }

    /// Update wander state - randomly move around following waypoints
    /// Reference: C++ AIWanderState from AIStates.cpp line ~3600
    fn update_wander_state(&self, state: &mut AiStateData) -> Result<StateReturnType, AiError> {
        // Wander state is similar to follow waypoint, but:
        // 1. Chooses random waypoints from nearby set
        // 2. Occasionally pauses at waypoints
        // 3. May change direction randomly

        // Reference: C++ AIWanderState inherits from AIFollowWaypointPathState
        // and adds random pausing and direction changes

        let pause_time = state.scratch.wander_pause_until;

        let current_frame = self.current_frame();

        if current_frame < pause_time {
            // Still pausing
            log::trace!("AI {} pausing during wander", self.owner_id);
            return Ok(StateReturnType::Continue);
        }

        // Not pausing, continue wandering
        // In real implementation, would pick random nearby waypoint and move there

        log::trace!("AI {} wandering", self.owner_id);

        // Occasionally pause (simulate with random chance)
        // Would use random number generator in real implementation
        let should_pause = false;

        if should_pause {
            let pause_duration = 60; // ~1 second at 60fps
            state.scratch.wander_pause_until = current_frame + pause_duration;
        }

        Ok(StateReturnType::Continue)
    }

    /// Update wander in place state - move around central point
    /// Reference: C++ AIWanderInPlaceState from AIStates.cpp line ~3800
    fn update_wander_in_place_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        // Wander in place moves around a fixed central point within a radius
        // Reference: C++ AIWanderInPlaceState moves to random positions near origin

        let center_pos = state.goal_position.ok_or(AiError::InvalidTarget)?;

        // Get or initialize random direction
        let last_direction_change = state.scratch.last_direction_change;

        let current_frame = self.current_frame();
        const DIRECTION_CHANGE_INTERVAL: i32 = 120; // Change direction every 2 seconds

        if (current_frame - last_direction_change) >= DIRECTION_CHANGE_INTERVAL {
            // Time to change direction
            // In real implementation:
            // 1. Pick random angle
            // 2. Pick random distance within wander radius
            // 3. Calculate new target position near center
            // 4. Start moving to new position

            state.scratch.last_direction_change = current_frame;

            log::trace!(
                "AI {} changing wander direction near {:?}",
                self.owner_id,
                center_pos
            );
        }

        Ok(StateReturnType::Continue)
    }

    /// Update panic state - run around frantically
    /// Reference: C++ AIPanicState from AIStates.cpp line ~4000
    fn update_panic_state(&self, state: &mut AiStateData) -> Result<StateReturnType, AiError> {
        // Panic state makes units run around wildly, typically when:
        // - Building is destroyed and garrisoned units panic
        // - Unit is under heavy fire with low health
        // - Special panic-inducing weapons hit unit

        // Reference: C++ AIPanicState behaves like AIWanderState but:
        // - Moves faster (uses panic speed modifier)
        // - Changes direction more frequently
        // - Plays panic animations/sounds
        // - Typically has a timeout duration

        let panic_start_frame = state.enter_frame.unwrap_or(TheGameLogic::get_frame());
        let panic_duration_frames = 10u32.saturating_mul(LOGICFRAMES_PER_SECOND as u32);

        if TheGameLogic::get_frame().saturating_sub(panic_start_frame) > panic_duration_frames {
            // Panic over, transition back to idle or previous state
            log::debug!("AI {} panic duration expired", self.owner_id);
            return Ok(StateReturnType::StateComplete);
        }

        // Continue panicking - move erratically
        log::trace!("AI {} panicking!", self.owner_id);

        // In real implementation:
        // 1. Pick random direction more frequently than wander
        // 2. Use higher movement speed
        // 3. Play panic animations/sounds
        // 4. Ignore combat and other stimuli

        Ok(StateReturnType::Continue)
    }

    /// Move away from repulsor objects (matches C++ AIMoveAwayFromRepulsorsState).
    /// Inherits move-to behavior; clears MODELCONDITION_PANICKING on exit.
    fn update_move_away_from_repulsors(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        let result = self.update_move_to_state(state);
        if matches!(
            result,
            Ok(StateReturnType::StateComplete) | Ok(StateReturnType::StateFailed)
        ) {
            if let Some(obj) = OBJECT_REGISTRY.get_object(self.owner_id) {
                if let Ok(mut guard) = obj.write() {
                    guard
                        .clear_model_condition_state(crate::common::ModelConditionFlags::PANICKING);
                }
            }
        }
        result
    }

    /// Rappel into building (matches C++ AIRappelState).
    /// Transitions infantry from airborne to garrisoned by moving them to the target position.
    fn update_rappel_into_state(
        &self,
        state: &mut AiStateData,
    ) -> Result<StateReturnType, AiError> {
        // If goal building is destroyed, complete the state.
        if let Some(goal_id) = state.goal_object {
            if let Some(obj) = OBJECT_REGISTRY.get_object(goal_id) {
                if let Ok(guard) = obj.read() {
                    if guard.is_effectively_dead() {
                        return Ok(StateReturnType::StateFailed);
                    }
                }
            }
        }
        // Delegate to move-to for the actual movement.
        self.update_move_to_state(state)
    }

    // State configuration methods

    /// Set goal position for movement states
    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.current_state.goal_position = Some(pos);
        self.native.set_goal_position(Some(pos));
    }

    /// Set goal object for object-based states
    pub fn set_goal_object(&mut self, obj_id: ObjectID) {
        self.current_state.goal_object = Some(obj_id);
        self.native.set_goal_object(Some(obj_id));
    }

    /// Set path to follow
    pub fn set_goal_path(&mut self, path: Vec<Coord3D>) {
        self.current_state.goal_path = path;
        self.current_state.path_index = 0;
        if matches!(
            self.current_state.state_type,
            AiStateType::FollowPath | AiStateType::FollowExitProductionPath
        ) {
            self.native.set_state(
                self.native_state_for(self.current_state.state_type, &self.current_state),
            );
        }
    }

    /// Add point to path
    pub fn add_to_goal_path(&mut self, point: Coord3D) {
        if self.current_state.goal_path.is_empty() {
            self.current_state.goal_path.push(point);
        } else {
            let should_append = self
                .current_state
                .goal_path
                .last()
                .map(|last| last.x != point.x || last.y != point.y || last.z != point.z)
                .unwrap_or(true);
            if should_append {
                self.current_state.goal_path.push(point);
            }
        }
        if matches!(
            self.current_state.state_type,
            AiStateType::FollowPath | AiStateType::FollowExitProductionPath
        ) {
            self.native.set_state(
                self.native_state_for(self.current_state.state_type, &self.current_state),
            );
        }
    }

    /// Set waypoint chain to follow
    pub fn set_goal_polygon(&mut self, polygon_id: crate::polygon_trigger::PolygonTriggerId) {
        self.current_state.goal_polygon = Some(polygon_id);
        self.native.set_goal_polygon(Some(polygon_id));
    }

    pub fn set_goal_waypoint(&mut self, waypoint: WaypointId) {
        self.current_state.goal_waypoint = Some(waypoint);
        self.native.set_goal_waypoint(Some(waypoint));
    }

    /// Set squad to attack
    pub fn set_goal_squad(&mut self, squad: SquadId) {
        self.current_state.goal_squad = Some(squad);
        let mut handle = None;
        if let Some(obj) = get_legacy_object(self.owner_id) {
            let mut squad_obj = Squad::new();
            if let Ok(obj_guard) = obj.read() {
                let team = obj_guard.get_team();
                if let Some(team) = team.as_ref() {
                    if let Ok(team_guard) = team.read() {
                        squad_obj.squad_from_team(&team_guard, true);
                    }
                }
            }
            handle = Some(std::sync::Arc::new(std::sync::Mutex::new(squad_obj)));
        }
        self.current_state.goal_squad_handle = handle;
    }

    /// Set maximum shots for attack states
    pub fn set_max_shots(&mut self, shots: i32) {
        self.current_state.max_shots = shots;
    }

    /// Set guard mode
    pub fn set_guard_mode(&mut self, mode: GuardMode) {
        self.current_state.guard_mode = mode;
    }

    /// Set waypoint graph for native pathing helpers
    pub fn set_waypoint_graph(&mut self, graph: Option<Arc<RwLock<WaypointGraph>>>) {
        self.waypoint_graph = graph;
    }

    /// Set state timeout
    pub fn set_timeout(&mut self, timeout: Duration) {
        self.current_state.timeout_frames = Some(Self::duration_to_frames(timeout));
    }

    /// Set custom state data
    pub fn set_state_value(&mut self, key: String, value: StateValue) {
        match (key.as_str(), value) {
            ("current_position", StateValue::Position(pos)) => {
                self.current_state.scratch.current_position = Some(pos);
            }
            ("pathfinding_requested", StateValue::Bool(flag)) => {
                self.current_state.scratch.pathfinding_requested = flag;
            }
            ("path_following_index", StateValue::Int(index)) => {
                self.current_state.scratch.path_following_index = index;
            }
            ("weapon_ready", StateValue::Bool(flag)) => {
                self.current_state.scratch.weapon_ready = flag;
            }
            ("last_fire_frame", StateValue::Int(frame)) => {
                self.current_state.scratch.last_fire_frame = frame;
            }
            ("guard_position", StateValue::Position(pos)) => {
                self.current_state.scratch.guard_position = Some(pos);
            }
            ("last_scan_frame", StateValue::Int(frame)) => {
                self.current_state.scratch.last_scan_frame = frame;
            }
            ("enemy_found", StateValue::ObjectId(id)) => {
                self.current_state.scratch.enemy_found = id;
            }
            ("last_hunt_scan_frame", StateValue::Int(frame)) => {
                self.current_state.scratch.last_hunt_scan_frame = frame;
            }
            ("current_hunt_target", StateValue::ObjectId(id)) => {
                self.current_state.scratch.current_hunt_target = id;
            }
            ("last_move_target", StateValue::Position(pos)) => {
                self.current_state.scratch.last_move_target = Some(pos);
            }
            ("wander_pause_until", StateValue::Int(frame)) => {
                self.current_state.scratch.wander_pause_until = frame;
            }
            ("last_direction_change", StateValue::Int(frame)) => {
                self.current_state.scratch.last_direction_change = frame;
            }
            _ => {
                log::debug!(
                    "AI state value '{}' ignored for native scratch mapping",
                    key
                );
            }
        }
    }

    /// Get custom state data
    pub fn get_state_value(&self, key: &str) -> Option<StateValue> {
        match key {
            "current_position" => self
                .current_state
                .scratch
                .current_position
                .map(StateValue::Position),
            "pathfinding_requested" => Some(StateValue::Bool(
                self.current_state.scratch.pathfinding_requested,
            )),
            "path_following_index" => Some(StateValue::Int(
                self.current_state.scratch.path_following_index,
            )),
            "weapon_ready" => Some(StateValue::Bool(self.current_state.scratch.weapon_ready)),
            "last_fire_frame" => Some(StateValue::Int(self.current_state.scratch.last_fire_frame)),
            "guard_position" => self
                .current_state
                .scratch
                .guard_position
                .map(StateValue::Position),
            "last_scan_frame" => Some(StateValue::Int(self.current_state.scratch.last_scan_frame)),
            "enemy_found" => Some(StateValue::ObjectId(self.current_state.scratch.enemy_found)),
            "last_hunt_scan_frame" => Some(StateValue::Int(
                self.current_state.scratch.last_hunt_scan_frame,
            )),
            "current_hunt_target" => Some(StateValue::ObjectId(
                self.current_state.scratch.current_hunt_target,
            )),
            "last_move_target" => self
                .current_state
                .scratch
                .last_move_target
                .map(StateValue::Position),
            "wander_pause_until" => Some(StateValue::Int(
                self.current_state.scratch.wander_pause_until,
            )),
            "last_direction_change" => Some(StateValue::Int(
                self.current_state.scratch.last_direction_change,
            )),
            _ => None,
        }
    }

    /// Pause/resume state machine
    pub fn set_paused(&mut self, paused: bool) {
        self.paused = paused;
    }

    /// Check if state machine is idle
    pub fn is_idle(&self) -> bool {
        self.get_current_state() == AiStateType::Idle
    }

    /// Check if state machine is busy (in busy state)
    pub fn is_busy(&self) -> bool {
        self.get_current_state() == AiStateType::Busy
    }

    /// Check if state is an attack state
    pub fn is_attack_state(&self) -> bool {
        matches!(
            self.get_current_state(),
            AiStateType::AttackObject
                | AiStateType::AttackPosition
                | AiStateType::ForceAttackObject
                | AiStateType::AttackAndFollowObject
                | AiStateType::AttackSquad
                | AiStateType::AttackArea
                | AiStateType::AttackMoveTo
                | AiStateType::AttackFollowWaypointPathAsIndividuals
                | AiStateType::AttackFollowWaypointPathAsTeam
        )
    }

    /// Check if state is a guard state
    pub fn is_guard_state(&self) -> bool {
        matches!(
            self.get_current_state(),
            AiStateType::Guard | AiStateType::GuardTunnelNetwork | AiStateType::GuardRetaliate
        )
    }

    /// Get state machine statistics for debugging
    pub fn get_debug_info(&self) -> AiStateMachineDebugInfo {
        AiStateMachineDebugInfo {
            owner_id: self.owner_id,
            current_state: self.current_state.state_type,
            previous_state: self.previous_state.as_ref().map(|s| s.state_type),
            temporary_state: self.temporary_state.as_ref().map(|s| s.state_type),
            paused: self.paused,
            state_duration: self.current_state.enter_frame.map(|frame| {
                Self::frames_to_duration(TheGameLogic::get_frame().saturating_sub(frame))
            }),
            has_timeout: self.current_state.timeout_frames.is_some(),
        }
    }

    /// Get current temporary state (matches C++ getTemporaryState for move-away logic).
    pub fn get_temporary_state(&self) -> Option<AiStateType> {
        self.temporary_state.as_ref().map(|s| s.state_type)
    }
}

impl AiCommandInterface for AiStateMachine {
    fn ai_do_command(&mut self, params: &AiCommandParams) -> Result<(), AiError> {
        let state = match params.cmd {
            AiCommandType::Idle => AiStateType::Idle,
            AiCommandType::Busy => AiStateType::Busy,
            AiCommandType::MoveToPosition
            | AiCommandType::MoveToObject
            | AiCommandType::MoveToPositionEvenIfSleeping => AiStateType::MoveTo,
            AiCommandType::AttackObject | AiCommandType::ForceAttackObject => {
                AiStateType::AttackObject
            }
            AiCommandType::AttackPosition => AiStateType::AttackPosition,
            AiCommandType::AttackMoveToPosition => AiStateType::AttackMoveTo,
            AiCommandType::AttackFollowWaypointPath => {
                AiStateType::AttackFollowWaypointPathAsIndividuals
            }
            AiCommandType::AttackFollowWaypointPathAsTeam => {
                AiStateType::AttackFollowWaypointPathAsTeam
            }
            AiCommandType::AttackTeam => AiStateType::AttackSquad,
            AiCommandType::AttackArea => AiStateType::AttackArea,
            AiCommandType::GuardPosition
            | AiCommandType::GuardObject
            | AiCommandType::GuardArea => AiStateType::Guard,
            AiCommandType::Hunt => AiStateType::Hunt,
            AiCommandType::FollowWaypointPath => AiStateType::FollowWaypointPathAsIndividuals,
            AiCommandType::FollowWaypointPathAsTeam => AiStateType::FollowWaypointPathAsTeam,
            AiCommandType::FollowWaypointPathExact => {
                AiStateType::FollowWaypointPathAsIndividualsExact
            }
            AiCommandType::FollowWaypointPathAsTeamExact => {
                AiStateType::FollowWaypointPathAsTeamExact
            }
            AiCommandType::FollowPath
            | AiCommandType::FollowExitProductionPath
            | AiCommandType::FollowUserPath
            | AiCommandType::FollowPathAppend => AiStateType::FollowPath,
            AiCommandType::Wander => AiStateType::Wander,
            AiCommandType::WanderInPlace => AiStateType::WanderInPlace,
            AiCommandType::Panic => AiStateType::Panic,
            AiCommandType::FaceObject => AiStateType::FaceObject,
            AiCommandType::FacePosition => AiStateType::FacePosition,
            AiCommandType::GuardTunnelNetwork => AiStateType::GuardTunnelNetwork,
            AiCommandType::GuardRetaliate => AiStateType::GuardRetaliate,
            AiCommandType::MoveAwayFromUnit => AiStateType::MoveOutOfTheWay,
            AiCommandType::MoveToPositionAndEvacuate => AiStateType::MoveAndEvacuate,
            AiCommandType::MoveToPositionAndEvacuateAndExit => AiStateType::MoveAndEvacuateAndExit,
            AiCommandType::TightenToPosition => AiStateType::MoveAndTighten,
            AiCommandType::EvacuateInstantly => AiStateType::ExitInstantly,
            AiCommandType::Evacuate => AiStateType::Exit,
            AiCommandType::Repair => AiStateType::GetRepaired,
            AiCommandType::GetRepaired | AiCommandType::GetHealed => AiStateType::GetRepaired,
            AiCommandType::HackInternet => AiStateType::HackInternet,
            AiCommandType::PickUpPrisoner => AiStateType::PickUpCrate,
            AiCommandType::Dock => AiStateType::Dock,
            AiCommandType::Enter => AiStateType::Enter,
            AiCommandType::Exit => AiStateType::Exit,
            _ => AiStateType::Idle,
        };

        self.set_state(state)?;

        if let Some(obj_id) = params.obj {
            self.set_goal_object(obj_id);
        }

        if params.pos != Coord3D::new(0.0, 0.0, 0.0) {
            self.set_goal_position(params.pos);
        }

        if params.int_value != 0 {
            self.set_max_shots(params.int_value);
        }

        if !params.coords.is_empty() {
            self.set_goal_path(params.coords.clone());
        }

        if let Some(waypoint) = params.waypoint {
            self.set_goal_waypoint(waypoint);
        }

        if matches!(params.cmd, AiCommandType::AttackTeam) {
            if let Some(team_name) = params.team.as_ref() {
                let mut squad = Squad::new();
                if let Ok(mut factory_guard) = get_team_factory().lock() {
                    if let Some(team) = factory_guard.find_team(team_name) {
                        if let Ok(team_guard) = team.read() {
                            squad.squad_from_team(&team_guard, true);
                        }
                    }
                }
                self.current_state.goal_squad_handle =
                    Some(std::sync::Arc::new(std::sync::Mutex::new(squad)));
            }
        }

        if matches!(
            params.cmd,
            AiCommandType::GuardPosition | AiCommandType::GuardObject | AiCommandType::GuardArea
        ) {
            let mode = match params.int_value {
                1 => GuardMode::GuardWithoutPursuit,
                2 => GuardMode::GuardFlyingUnitsOnly,
                _ => GuardMode::Normal,
            };
            self.set_guard_mode(mode);
        }

        if let Some(trigger_id) = params.polygon {
            self.set_goal_polygon(trigger_id);
            if let Ok(terrain_guard) = get_terrain_logic().read() {
                if let Some(trigger) = terrain_guard.get_trigger_areas().get_by_id(trigger_id) {
                    if self.current_state.goal_position.is_none() {
                        self.set_goal_position(trigger.get_center_point());
                    }
                }
            }
        }

        Ok(())
    }
}

/// Debug information for the state machine
#[derive(Debug)]
pub struct AiStateMachineDebugInfo {
    pub owner_id: ObjectID,
    pub current_state: AiStateType,
    pub previous_state: Option<AiStateType>,
    pub temporary_state: Option<AiStateType>,
    pub paused: bool,
    pub state_duration: Option<Duration>,
    pub has_timeout: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::locomotor::core::{Locomotor, LocomotorTemplate};
    use crate::modules::AIUpdateInterface;
    use crate::object::registry::OBJECT_REGISTRY;
    use crate::object::Object;
    use std::sync::{Arc, Mutex, RwLock};

    #[derive(Debug, Default, Clone)]
    struct FaceAiCapture {
        orientation_calls: u32,
        position_calls: u32,
        last_orientation_goal: Option<Real>,
        last_position_goal: Option<Coord3D>,
    }

    #[derive(Debug)]
    struct FaceTestAI {
        locomotor: Arc<Mutex<Locomotor>>,
        capture: Arc<Mutex<FaceAiCapture>>,
    }

    impl AIUpdateInterface for FaceTestAI {
        fn update(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
            Ok(())
        }

        fn is_moving(&self) -> bool {
            false
        }

        fn is_idle(&self) -> bool {
            true
        }

        fn set_movement_target(&mut self, _target: &Coord3D) -> Result<(), String> {
            Ok(())
        }

        fn get_cur_locomotor(&self) -> Option<Arc<Mutex<Locomotor>>> {
            Some(self.locomotor.clone())
        }

        fn set_locomotor_goal_orientation(&mut self, angle: Real) {
            if let Ok(mut cap) = self.capture.lock() {
                cap.orientation_calls = cap.orientation_calls.saturating_add(1);
                cap.last_orientation_goal = Some(angle);
            }
        }

        fn set_locomotor_goal_position_explicit(&mut self, pos: Coord3D) {
            if let Ok(mut cap) = self.capture.lock() {
                cap.position_calls = cap.position_calls.saturating_add(1);
                cap.last_position_goal = Some(pos);
            }
        }
    }

    fn register_face_test_object(
        id: ObjectID,
        min_speed: Real,
        position: Coord3D,
        orientation: Real,
    ) -> (Arc<RwLock<Object>>, Arc<Mutex<FaceAiCapture>>) {
        let object = Arc::new(RwLock::new(Object::new_test(id, 100.0)));

        let mut template = LocomotorTemplate::new(format!("face_test_{id}"));
        template.min_speed = min_speed;
        let locomotor = Arc::new(Mutex::new(Locomotor::new(Arc::new(template))));

        let capture = Arc::new(Mutex::new(FaceAiCapture::default()));
        let ai: Arc<Mutex<dyn AIUpdateInterface>> = Arc::new(Mutex::new(FaceTestAI {
            locomotor,
            capture: capture.clone(),
        }));

        {
            let Ok(mut guard) = object.write() else {
                panic!("failed to lock face test object for setup");
            };
            let _ = guard.set_position(&position);
            let _ = guard.set_orientation(orientation);
            guard.set_ai_update_interface(Some(ai));
        }

        OBJECT_REGISTRY.register_object(id, &object);
        (object, capture)
    }

    fn unregister_face_test_object(id: ObjectID) {
        OBJECT_REGISTRY.unregister_object(id);
    }

    #[test]
    fn test_state_machine_creation() {
        let machine = AiStateMachine::new(123, "test".to_string());
        assert_eq!(machine.get_current_state(), AiStateType::Idle);
        assert_eq!(machine.owner_id, 123);
        assert_eq!(machine.name, "test");
    }

    #[test]
    fn test_state_transitions() {
        let mut machine = AiStateMachine::new(123, "test".to_string());

        assert!(machine.set_state(AiStateType::MoveTo).is_ok());
        assert_eq!(machine.get_current_state(), AiStateType::MoveTo);

        assert!(machine.set_state(AiStateType::AttackObject).is_ok());
        assert_eq!(machine.get_current_state(), AiStateType::AttackObject);
    }

    #[test]
    fn test_temporary_state() {
        let mut machine = AiStateMachine::new(123, "test".to_string());

        machine.set_state(AiStateType::MoveTo).unwrap();
        assert_eq!(machine.get_current_state(), AiStateType::MoveTo);

        machine
            .set_temporary_state(AiStateType::MoveOutOfTheWay, Duration::from_millis(100))
            .unwrap();
        assert_eq!(machine.get_current_state(), AiStateType::MoveOutOfTheWay);

        // After clearing temporary state, should return to original
        machine.clear_temporary_state();
        assert_eq!(machine.get_current_state(), AiStateType::MoveTo);
    }

    #[test]
    fn test_state_queries() {
        let mut machine = AiStateMachine::new(123, "test".to_string());

        assert!(machine.is_idle());
        assert!(!machine.is_attack_state());
        assert!(!machine.is_guard_state());

        machine.set_state(AiStateType::AttackObject).unwrap();
        assert!(!machine.is_idle());
        assert!(machine.is_attack_state());
        assert!(!machine.is_guard_state());

        machine.set_state(AiStateType::Guard).unwrap();
        assert!(!machine.is_idle());
        assert!(!machine.is_attack_state());
        assert!(machine.is_guard_state());
    }

    #[test]
    fn test_goal_setting() {
        let mut machine = AiStateMachine::new(123, "test".to_string());

        let pos = Coord3D::new(10.0, 20.0, 30.0);
        machine.set_goal_position(pos);
        assert_eq!(machine.current_state.goal_position, Some(pos));

        machine.set_goal_object(456);
        assert_eq!(machine.current_state.goal_object, Some(456));

        let path = vec![Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(10.0, 10.0, 0.0)];
        machine.set_goal_path(path.clone());
        assert_eq!(machine.current_state.goal_path, path);
        assert_eq!(machine.current_state.path_index, 0);

        machine.set_goal_polygon(99);
        assert_eq!(machine.current_state.goal_polygon, Some(99));
        assert_eq!(machine.native.goals.goal_polygon, Some(99));
    }

    #[test]
    fn test_add_to_goal_path_deduplicates_terminal_point() {
        let mut machine = AiStateMachine::new(123, "test".to_string());
        let p0 = Coord3D::new(1.0, 2.0, 3.0);
        let p1 = Coord3D::new(4.0, 5.0, 6.0);

        machine.add_to_goal_path(p0);
        machine.add_to_goal_path(p0);
        machine.add_to_goal_path(p1);

        assert_eq!(machine.current_state.goal_path.len(), 2);
        assert_eq!(machine.current_state.goal_path[0], p0);
        assert_eq!(machine.current_state.goal_path[1], p1);
    }

    #[test]
    fn test_face_position_turn_in_place_sets_orientation_goal() {
        let object_id = 700_001;
        let (_object, capture) =
            register_face_test_object(object_id, 0.0, Coord3D::new(0.0, 0.0, 0.0), 0.0);

        let mut machine = AiStateMachine::new(object_id, "face_position_turn".to_string());
        machine.set_state(AiStateType::FacePosition).unwrap();
        machine.set_goal_position(Coord3D::new(0.0, 10.0, 0.0));

        let result = machine.update().unwrap();
        assert_eq!(result, StateReturnType::Continue);

        let snapshot = capture.lock().expect("capture lock poisoned").clone();
        assert_eq!(snapshot.orientation_calls, 1);
        assert_eq!(snapshot.position_calls, 0);
        assert!(snapshot.last_orientation_goal.is_some());

        unregister_face_test_object(object_id);
    }

    #[test]
    fn test_face_position_non_turn_in_place_sets_position_goal() {
        let object_id = 700_002;
        let target = Coord3D::new(0.0, 12.0, 0.0);
        let (_object, capture) =
            register_face_test_object(object_id, 1.0, Coord3D::new(0.0, 0.0, 0.0), 0.0);

        let mut machine = AiStateMachine::new(object_id, "face_position_move".to_string());
        machine.set_state(AiStateType::FacePosition).unwrap();
        machine.set_goal_position(target);

        let result = machine.update().unwrap();
        assert_eq!(result, StateReturnType::Continue);

        let snapshot = capture.lock().expect("capture lock poisoned").clone();
        assert_eq!(snapshot.orientation_calls, 0);
        assert_eq!(snapshot.position_calls, 1);
        assert_eq!(snapshot.last_position_goal, Some(target));

        unregister_face_test_object(object_id);
    }

    #[test]
    fn test_face_position_within_threshold_completes_without_locomotor_command() {
        let object_id = 700_003;
        let (_object, capture) =
            register_face_test_object(object_id, 0.0, Coord3D::new(0.0, 0.0, 0.0), 0.0);

        let mut machine = AiStateMachine::new(object_id, "face_position_done".to_string());
        machine.set_state(AiStateType::FacePosition).unwrap();
        machine.set_goal_position(Coord3D::new(1.0, 0.01, 0.0));

        let result = machine.update().unwrap();
        assert_eq!(result, StateReturnType::StateComplete);

        let snapshot = capture.lock().expect("capture lock poisoned").clone();
        assert_eq!(snapshot.orientation_calls, 0);
        assert_eq!(snapshot.position_calls, 0);

        unregister_face_test_object(object_id);
    }

    #[test]
    fn test_face_object_without_goal_fails() {
        let mut machine = AiStateMachine::new(700_004, "face_object_missing_goal".to_string());
        machine.set_state(AiStateType::FaceObject).unwrap();
        let result = machine.update().unwrap();
        assert_eq!(result, StateReturnType::StateFailed);
    }

    #[test]
    fn test_relative_angle_2d_wraps_across_pi_boundary() {
        let owner = Coord3D::new(0.0, 0.0, 0.0);
        let target = Coord3D::new(-1.0, -0.01, 0.0);
        let rel = relative_angle_2d(&owner, 3.13, &target);
        assert!(rel.abs() < 0.1);
    }

    #[test]
    fn test_relative_angle_2d_respects_cpp_face_threshold() {
        let owner = Coord3D::new(0.0, 0.0, 0.0);
        let near_target = Coord3D::new(1.0, 0.03, 0.0);
        let far_target = Coord3D::new(1.0, 0.05, 0.0);

        assert!(relative_angle_2d(&owner, 0.0, &near_target).abs() < 0.035);
        assert!(relative_angle_2d(&owner, 0.0, &far_target).abs() > 0.035);
    }
}
