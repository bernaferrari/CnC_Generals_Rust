//! Retained AI machine control. Current child and parent are distinct states;
//! the effective state is a read-only view, never a second writable FSM.
use super::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GuardTargetType {
    Location,
    Object,
    Area,
    #[default]
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct AiMachineGoals {
    pub object: Option<ObjectId>,
    pub position: Vec3,
    path: Vec<Vec3>,
    waypoint: Option<String>,
    squad: Option<Vec<ObjectId>>,
}

impl AiMachineGoals {
    /// Authored goal points in FollowPath order.
    pub fn path(&self) -> &[Vec3] {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporaryAiMove {
    pub deadline: u32,
    pub goal: Vec3,
    pub saved_destination: Option<Vec3>,
    pub saved_path_goal: Option<Vec3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ParentMachineContinuation {
    state: AIState,
    goals: AiMachineGoals,
}

/// Detached snapshot of the live machine. The actor grants read-only
/// access to its continuation and validates any wholesale restore.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseAiContinuation {
    pub ai_state: AIState,
    pub attack_substate: AttackSubState,
    pub auto_acquire_when_idle: bool,
    pub approach_timestamp: u32,
    pub prev_victim_pos: Option<Vec3>,
    /// C++ AIAttackState::m_originalVictimPos, fixed on attack entry.
    #[serde(default)]
    pub attack_original_victim_pos: Option<Vec3>,
    pub face_can_turn_in_place: bool,
    pub face_active: bool,
    pub face_goal_pos: Option<Vec3>,
    pub face_loco_frame: u32,
    pub adjust_destinations: bool,
    /// AIMoveAndTightenState::m_checkForPath, owned and saved by this actor.
    #[serde(default)]
    tighten_check_for_path: bool,
    /// The active AIMoveAndTightenState and its single blocked retry.
    #[serde(default)]
    tighten_repath_available: Option<bool>,
    pub wander_width: f32,

    pub(super) guard: GuardRuntime,
    #[serde(default)]
    pub(super) retaliate: GuardRetaliateRuntime,
    pub(super) hunt_scan: Option<u32>,
    pub(super) quick_exit: Option<u32>,
    pub(super) wander: Option<WanderState>,
    parent: Option<ParentMachineContinuation>,
    locked: bool,
    pub goals: AiMachineGoals,
    pub normal_move_goal: Option<Vec3>,
    temporary_move: Option<TemporaryAiMove>,
    guard_target_types: [GuardTargetType; 2],
    pub guard_position: Option<Vec3>,
    pub guard_target: Option<ObjectId>,
    pub guard_area_trigger: Option<String>,
    idle_initial_sleep_offset: u16,
    idle_entry_frame: u32,
    pub idle_initialized: bool,
    /// C++ AIUpdateInterface::m_isAiDead, distinct from Object death/effectively-dead flags.
    #[serde(default)]
    pub ai_dead: bool,
}
impl Default for BaseAiContinuation {
    fn default() -> Self {
        Self {
            ai_state: AIState::Idle,
            attack_substate: AttackSubState::AimAtTarget,
            auto_acquire_when_idle: true,
            approach_timestamp: 0,
            prev_victim_pos: None,
            attack_original_victim_pos: None,
            face_can_turn_in_place: false,
            face_active: false,
            face_goal_pos: None,
            face_loco_frame: 0,
            adjust_destinations: true,
            tighten_check_for_path: false,
            tighten_repath_available: None,
            wander_width: 0.0,

            guard: Default::default(),
            retaliate: Default::default(),
            hunt_scan: None,
            quick_exit: None,
            wander: None,
            parent: None,
            locked: false,
            goals: Default::default(),
            normal_move_goal: None,
            temporary_move: None,
            guard_target_types: [GuardTargetType::None; 2],
            guard_position: None,
            guard_target: None,
            guard_area_trigger: None,
            idle_initial_sleep_offset: 0,
            idle_entry_frame: 0,
            idle_initialized: false,
            ai_dead: false,
        }
    }
}
impl BaseAiContinuation {
    pub fn parent_state(&self) -> Option<AIState> {
        self.parent.as_ref().map(|parent| parent.state)
    }
    pub fn root_state(&self) -> AIState {
        self.parent_state().unwrap_or(self.ai_state)
    }

    /// Goals retained by the parent while the child runs.
    pub fn parent_goals(&self) -> Option<&AiMachineGoals> {
        self.parent.as_ref().map(|parent| &parent.goals)
    }

    /// Pending temporary move and the destination it interrupted.
    pub fn temporary_move(&self) -> Option<&TemporaryAiMove> {
        self.temporary_move.as_ref()
    }
    pub fn validate_follow_path_index(&self, index: Option<u32>) -> Result<(), String> {
        match index {
            None if self.goals.path.is_empty() => Ok(()),
            Some(index)
                if self.ai_state == AIState::Moving
                    && self.parent.is_none()
                    && !self.locked
                    && (index as usize) < self.goals.path.len() =>
            {
                Ok(())
            }
            _ => Err("FollowPath index/path requires its active moving root".into()),
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.retaliate.victim == Some(ObjectId(0)) {
            return Err("guard retaliation victim cannot be the invalid object ID".into());
        }
        if self
            .retaliate
            .anchor
            .is_some_and(|position| !position.is_finite())
        {
            return Err("guard retaliation anchor must be finite".into());
        }

        if self.guard.chase_phase == GuardChasePhase::None
            && self.guard.chase_give_up_frame.is_some()
        {
            return Err("guard chase deadline requires an active chase".into());
        }
        if self.tighten_repath_available.is_some() && self.root_state() != AIState::Moving {
            return Err("tighten retry requires an active move root".into());
        }
        if let Some(parent) = &self.parent {
            let goals = &parent.goals;
            if !goals.position.is_finite()
                || goals.path.iter().any(|p| !p.is_finite())
                || goals.path.len() > 65536
                || goals.squad.as_ref().is_some_and(|v| v.len() > 65536)
            {
                return Err("invalid retained parent goals".into());
            }
            if !matches!(
                parent.state,
                AIState::GuardingArea
                    | AIState::GuardingObject
                    | AIState::GuardRetaliating
                    | AIState::Hunting
                    | AIState::AttackMoving
            ) {
                return Err("invalid retained AI parent".into());
            }
            if !matches!(
                self.ai_state,
                AIState::Attacking | AIState::AttackingGround | AIState::Moving | AIState::Idle
            ) && !(parent.state == AIState::Hunting && self.ai_state == AIState::Patrolling)
            {
                return Err("invalid retained AI child".into());
            }
        }
        if self.goals.path.len() > 65536
            || self.goals.squad.as_ref().is_some_and(|v| v.len() > 65536)
        {
            return Err("oversized AI machine continuation".into());
        }
        if !self.goals.position.is_finite() || self.goals.path.iter().any(|p| !p.is_finite()) {
            return Err("nonfinite AI machine goal".into());
        }
        if self
            .attack_original_victim_pos
            .is_some_and(|p| !p.is_finite())
            || self.guard_position.is_some_and(|p| !p.is_finite())
            || self.temporary_move.as_ref().is_some_and(|m| {
                !m.goal.is_finite()
                    || m.saved_destination.is_some_and(|p| !p.is_finite())
                    || m.saved_path_goal.is_some_and(|p| !p.is_finite())
            })
        {
            return Err("nonfinite AI state body".into());
        }
        if self.normal_move_goal.is_some_and(|v| !v.is_finite())
            || self.prev_victim_pos.is_some_and(|v| !v.is_finite())
            || self.face_goal_pos.is_some_and(|v| !v.is_finite())
            || !self.wander_width.is_finite()
            || self.guard.anchor.is_some_and(|v| !v.is_finite())
            || matches!(self.guard.phase, Some(GuardPhase::Return { goal }) if !goal.is_finite())
        {
            return Err("nonfinite retained AI body".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
pub struct FollowPathLeg {
    pub position: Vec3,
    pub extra_distance: Option<f32>,
}

impl UnitAiRuntime {
    pub fn start_follow_path(&mut self, goals: Vec<Vec3>) -> FollowPathLeg {
        self.machine.goals.path = goals;
        self.follow_path_index = Some(0);
        self.current_follow_path_leg()
            .expect("admitted nonempty FollowPath")
    }

    pub fn append_follow_path_goal(&mut self, point: Vec3) -> bool {
        let path = &mut self.machine.goals.path;
        if path.last() == Some(&point) {
            return true;
        }
        if path.len() == 65_536 {
            return false;
        }
        path.push(point);
        true
    }

    fn current_follow_path_leg(&self) -> Option<FollowPathLeg> {
        let index = self.follow_path_index? as usize;
        let path = &self.machine.goals.path;
        let position = *path.get(index)?;
        let extra_distance = path.get(index + 1).map(|next| {
            let delta = *next - position;
            let cell = PATHFIND_CELL_SIZE_F;
            delta.x.hypot(delta.z)
                + if index + 2 < path.len() {
                    4.0 * cell
                } else {
                    0.0
                }
        });
        Some(FollowPathLeg {
            position,
            extra_distance,
        })
    }

    /// C++ AIStates.cpp:3318-3348. One reported internal move result, then
    /// skip every authored point nearer than a cell, including on a retry.
    pub fn consume_follow_path_move_result(
        &mut self,
        current: Vec3,
        failed: bool,
    ) -> Option<FollowPathLeg> {
        let mut index = self.follow_path_index? as usize;
        if failed && self.follow_path_retries > 0 {
            self.follow_path_retries -= 1;
        } else {
            index += 1;
        }
        let path = &self.machine.goals.path;
        let cell = PATHFIND_CELL_SIZE_F;
        while path.get(index).is_some_and(|point| {
            let delta = *point - current;
            delta.x * delta.x + delta.z * delta.z < cell * cell
        }) {
            index += 1;
        }
        if index == path.len() {
            return None;
        }
        self.follow_path_index = Some(index as u32);
        self.current_follow_path_leg()
    }

    pub fn machine(&self) -> &BaseAiContinuation {
        &self.machine
    }
    pub fn capture_machine(&self) -> BaseAiContinuation {
        self.machine.clone()
    }
    pub fn restore_machine(&mut self, value: BaseAiContinuation) -> Result<(), String> {
        value.validate()?;
        self.follow_path_index = None;
        self.follow_path_retries = initial_follow_path_retries();
        self.machine = value;
        Ok(())
    }
    pub fn follow_path_index(&self) -> Option<u32> {
        self.follow_path_index
    }
    pub fn restore_follow_path_index(&mut self, index: u32) -> Result<(), String> {
        self.machine.validate_follow_path_index(Some(index))?;
        self.follow_path_index = Some(index);
        Ok(())
    }
    fn clear_follow_path(&mut self) {
        if self.follow_path_index.take().is_some() {
            self.machine.goals.path.clear();
        }
    }
    pub fn enter_root(&mut self, state: AIState) {
        self.clear_follow_path();
        if state != AIState::Moving {
            self.machine.tighten_check_for_path = false;
            self.machine.tighten_repath_available = None;
        }
        if self.machine.ai_state == AIState::Attacking && state != AIState::Attacking {
            self.machine.attack_original_victim_pos = None;
        }
        self.machine.ai_state = state;
        self.machine.parent = None;
    }
    pub fn can_enter_child(&self) -> bool {
        matches!(
            self.machine.root_state(),
            AIState::GuardingArea
                | AIState::GuardingObject
                | AIState::GuardRetaliating
                | AIState::Hunting
                | AIState::AttackMoving
        )
    }
    pub fn enter_child(&mut self, state: AIState) -> bool {
        if !self.can_enter_child() {
            return false;
        }
        if self.machine.ai_state == AIState::Attacking && state != AIState::Attacking {
            self.machine.attack_original_victim_pos = None;
        }
        if self.machine.parent.is_none() {
            self.machine.parent = Some(ParentMachineContinuation {
                state: self.machine.ai_state,
                goals: std::mem::take(&mut self.machine.goals),
            });
        }
        self.machine.ai_state = state;
        true
    }
    pub fn resume_parent(&mut self) -> bool {
        let Some(parent) = self.machine.parent.take() else {
            return false;
        };
        if self.machine.ai_state == AIState::Attacking {
            self.machine.attack_original_victim_pos = None;
        }
        self.machine.ai_state = parent.state;
        self.machine.goals = parent.goals;
        true
    }
    pub fn machine_locked(&self) -> bool {
        self.machine.locked
    }
    pub fn lock_machine(&mut self) {
        self.machine.locked = true;
    }
    pub fn mark_ai_dead(&mut self) {
        self.machine.ai_dead = true;
    }
    pub fn is_ai_dead(&self) -> bool {
        self.machine.ai_dead
    }
    pub fn clear_machine_after_exit(&mut self) {
        self.clear_follow_path();
        self.machine.parent = None;
        self.machine.ai_state = AIState::Uninitialized;
        self.machine.goals = Default::default();
    }
    pub fn enter_idle(&mut self, frame: u32, initial_sleep_offset: u16) {
        self.enter_root(AIState::Idle);
        self.machine.idle_entry_frame = frame;
        self.machine.idle_initial_sleep_offset = initial_sleep_offset;
        self.machine.idle_initialized = true;
    }
    pub fn set_machine_goal_object(&mut self, target: Option<(ObjectId, Vec3)>) {
        if self.machine.locked {
            return;
        }
        self.machine.goals.object = target.map(|(id, _)| id);
        if let Some((_, position)) = target {
            self.machine.goals.position = position;
        }
    }
    pub fn set_machine_goal_position(&mut self, pos: Vec3) {
        if !self.machine.locked {
            self.machine.goals.position = pos;
        }
    }
    /// The outer AttackMove goal remains distinct from a child attack's path request.
    pub fn set_attack_move_goal(&mut self, pos: Vec3) {
        if self.machine.locked {
            return;
        }
        if let Some(parent) = self.machine.parent.as_mut() {
            if parent.state == AIState::AttackMoving {
                parent.goals.position = pos;
            }
        } else if self.machine.ai_state == AIState::AttackMoving {
            self.machine.goals.position = pos;
        }
    }
    pub fn attack_move_goal(&self) -> Option<Vec3> {
        match self.machine.parent.as_ref() {
            Some(parent) if parent.state == AIState::AttackMoving => Some(parent.goals.position),
            None if self.machine.ai_state == AIState::AttackMoving => {
                Some(self.machine.goals.position)
            }
            _ => None,
        }
    }
    pub fn set_attack_original_victim_pos(&mut self, pos: Vec3) {
        self.machine.attack_original_victim_pos = Some(pos);
    }
    pub fn clear_attack_original_victim_pos(&mut self) {
        self.machine.attack_original_victim_pos = None;
    }
    pub fn guard_target_type(&self) -> GuardTargetType {
        self.machine.guard_target_types[0]
    }
    pub fn set_guard_target_type(&mut self, kind: GuardTargetType) {
        self.machine.guard_target_types[0] = kind;
        self.machine.guard_target_types[1] = GuardTargetType::None;
    }
    /// C++ uses slot 1 for the active type and slot 0 for a pending type.
    /// This owner stores them in the reverse order: active, then pending.
    pub fn stage_guard_target_type(&mut self, kind: GuardTargetType) {
        let slot = usize::from(self.machine.guard_target_types[0] != GuardTargetType::None);
        self.machine.guard_target_types[slot] = kind;
    }
    pub fn exit_guard_target_type(&mut self) {
        self.machine.guard_target_types[0] = self.machine.guard_target_types[1];
        self.machine.guard_target_types[1] = GuardTargetType::None;
    }
    pub fn set_raw_guard_position(&mut self, pos: Option<Vec3>) {
        self.machine.guard_position = pos;
    }
    pub fn set_raw_guard_target(&mut self, id: Option<ObjectId>) {
        self.machine.guard_target = id;
    }
    pub fn set_raw_guard_area(&mut self, name: Option<String>) {
        self.machine.guard_area_trigger = name;
    }
    pub fn raw_guard_position(&self) -> Option<Vec3> {
        self.machine.guard_position
    }
    pub fn raw_guard_target(&self) -> Option<ObjectId> {
        self.machine.guard_target
    }
    pub fn raw_guard_area(&self) -> Option<&str> {
        self.machine.guard_area_trigger.as_deref()
    }
    pub fn temporary_move(&self) -> Option<&TemporaryAiMove> {
        self.machine.temporary_move()
    }
    pub fn begin_temporary_move(&mut self, value: TemporaryAiMove) {
        self.machine.temporary_move = Some(value);
    }
    pub fn take_temporary_move(&mut self) -> Option<TemporaryAiMove> {
        self.machine.temporary_move.take()
    }
}

impl UnitAiRuntime {
    pub fn set_attack_substate(&mut self, state: AttackSubState) {
        self.machine.attack_substate = state;
    }
    pub fn set_auto_acquire(&mut self, value: bool) {
        self.machine.auto_acquire_when_idle = value;
    }
    pub fn enter_hunt(&mut self) {
        self.clear_follow_path();
        if self.machine.parent_state() != Some(AIState::Hunting) {
            self.machine.parent = Some(ParentMachineContinuation {
                state: AIState::Hunting,
                goals: std::mem::take(&mut self.machine.goals),
            });
        }
        if !matches!(
            self.machine.ai_state,
            AIState::Attacking | AIState::AttackingGround
        ) {
            self.machine.ai_state = AIState::Patrolling;
        }
    }
    pub fn leave_hunt(&mut self) {
        if self.machine.root_state() == AIState::Hunting {
            if let Some(parent) = self.machine.parent.take() {
                self.machine.goals = parent.goals;
            }
        }
        self.machine.hunt_scan = None;
    }
}

impl UnitAiRuntime {
    pub fn with_auto_acquire(value: bool) -> Self {
        let mut owner = Self::default();
        owner.machine.auto_acquire_when_idle = value;
        owner
    }
}

impl UnitAiRuntime {
    pub fn set_approach_timestamp(&mut self, value: u32) {
        self.machine.approach_timestamp = value;
    }
    pub fn set_prev_victim_pos(&mut self, value: Option<Vec3>) {
        self.machine.prev_victim_pos = value;
    }
    pub fn set_face_can_turn_in_place(&mut self, value: bool) {
        self.machine.face_can_turn_in_place = value;
    }
    pub fn set_face_active(&mut self, value: bool) {
        self.machine.face_active = value;
    }
    pub fn set_face_goal_pos(&mut self, value: Option<Vec3>) {
        self.machine.face_goal_pos = value;
    }
    pub fn set_face_loco_frame(&mut self, value: u32) {
        self.machine.face_loco_frame = value;
    }
    pub fn set_adjust_destinations(&mut self, value: bool) {
        self.machine.adjust_destinations = value;
    }
    pub fn begin_tighten_path_check(&mut self) {
        self.machine.tighten_check_for_path = true;
        self.machine.tighten_repath_available = Some(true);
    }
    pub fn cancel_tighten_path_check(&mut self) {
        self.machine.tighten_check_for_path = false;
        self.machine.tighten_repath_available = None;
    }
    /// Return whether the active tighten state can retry, consuming its allowance.
    pub fn take_tighten_repath(&mut self) -> Option<bool> {
        self.machine
            .tighten_repath_available
            .as_mut()
            .map(|available| std::mem::replace(available, false))
    }
    pub fn update_tighten_path_check(&mut self, path_ready: bool) {
        if self.machine.tighten_check_for_path && path_ready {
            self.machine.adjust_destinations = true;
            self.machine.tighten_check_for_path = false;
        }
    }
    pub fn set_wander_width(&mut self, value: f32) {
        self.machine.wander_width = value;
    }
}

impl BaseAiContinuation {
    /// Before the staged-target contract slot 1 held a previous value, not
    /// an in-flight transition. Never promote it when restoring that format.
    pub fn discard_legacy_guard_history(&mut self) {
        self.guard_target_types[1] = GuardTargetType::None;
    }
    pub fn is_inert(&self) -> bool {
        matches!(self.ai_state, AIState::Idle | AIState::Uninitialized)
            && self.parent.is_none()
            && !self.locked
            && self.temporary_move.is_none()
            && self.normal_move_goal.is_none()
            && self.guard_target_types[0] == GuardTargetType::None
            && self.guard_target_types[1] == GuardTargetType::None
            && self.guard.phase.is_none()
            && self.guard.scan.is_none()
            && self.guard.chase_phase == GuardChasePhase::None
            && self.guard.chase_give_up_frame.is_none()
            && self.hunt_scan.is_none()
            && self.quick_exit.is_none()
            && self.wander.is_none()
            && !self.face_active
            && !self.tighten_check_for_path
            && self.tighten_repath_available.is_none()
            && self.attack_substate == AttackSubState::AimAtTarget
            && self.approach_timestamp == 0
            && self.prev_victim_pos.is_none()
            && self.attack_original_victim_pos.is_none()
            && self.retaliate.victim.is_none()
            && self.retaliate.anchor.is_none()
            && !self.idle_initialized
            && self.goals.object.is_none()
            && self.goals.position == Vec3::ZERO
            && self.goals.path.is_empty()
            && self.goals.waypoint.is_none()
            && self.goals.squad.is_none()
    }
}

impl BaseAiContinuation {
    pub fn projected_state(&self) -> AIState {
        match self.root_state() {
            AIState::Hunting => AIState::Patrolling,
            AIState::AttackMoving => AIState::AttackMoving,
            AIState::JetReturningForLanding | AIState::JetReturnToDeadAirfield => AIState::Moving,
            AIState::JetCirclingDeadAirfield => AIState::Idle,
            _ => self.ai_state,
        }
    }
}

impl UnitAiRuntime {
    pub fn record_idle_entry_body(&mut self, frame: u32, offset: u16) {
        self.machine.idle_entry_frame = frame;
        self.machine.idle_initial_sleep_offset = offset;
        self.machine.idle_initialized = true;
    }
    pub fn consume_idle_initialization(&mut self) -> bool {
        let pending = self.machine.idle_initialized;
        self.machine.idle_initialized = false;
        pending
    }
    pub fn current_leaf_state(&self) -> AIState {
        self.machine.ai_state
    }
    pub fn root_state(&self) -> AIState {
        self.machine.root_state()
    }
    pub fn parent_state(&self) -> Option<AIState> {
        self.machine.parent_state()
    }
    pub fn set_current_child_state(&mut self, state: AIState) -> bool {
        if self.machine.parent.is_none() {
            return self.enter_child(state);
        }
        if state == AIState::Idle {
            if let Some(parent) = self.machine.parent.as_ref() {
                self.machine.goals = parent.goals.clone();
            }
        }
        self.machine.ai_state = state;
        true
    }
}

impl UnitAiRuntime {
    pub fn enter_normal_move_body(&mut self, goal: Vec3) {
        self.machine.normal_move_goal = Some(goal);
    }
}

impl BaseAiContinuation {
    pub fn deadline_projection(&self) -> (Option<u32>, Option<u32>, Option<u32>) {
        (self.guard.scan, self.hunt_scan, self.quick_exit)
    }
}
