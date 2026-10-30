//! Retained AI machine control. Current child and parent are distinct states;
//! the effective state is a read-only view, never a second writable FSM.
use super::*;
use crate::game_logic::{AIState, AttackSubState, ObjectId};
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
    pub(crate) object: Option<ObjectId>,
    pub(crate) position: Vec3,
    pub(crate) path: Vec<Vec3>,
    pub(crate) waypoint: Option<String>,
    pub(crate) squad: Option<Vec<ObjectId>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TemporaryAiMove {
    pub(crate) deadline: u32,
    pub(crate) goal: Vec3,
    pub(crate) saved_destination: Option<Vec3>,
    pub(crate) saved_path_goal: Option<Vec3>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ParentMachineContinuation {
    pub(crate) state: AIState,
    pub(crate) goals: AiMachineGoals,
}

/// Detached native value of the one live owner. Fields are crate-readable;
/// live Object access exposes no mutable dereference to this value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseAiContinuation {
    pub ai_state: AIState,
    pub attack_substate: AttackSubState,
    pub auto_acquire_when_idle: bool,
    pub approach_timestamp: u32,
    pub prev_victim_pos: Option<Vec3>,
    pub face_can_turn_in_place: bool,
    pub face_active: bool,
    pub face_goal_pos: Option<Vec3>,
    pub face_loco_frame: u32,
    pub adjust_destinations: bool,
    pub wander_width: f32,

    pub(crate) guard: GuardRuntime,
    pub(crate) hunt_scan: Option<u32>,
    pub(crate) quick_exit: Option<u32>,
    pub(crate) wander: Option<WanderState>,
    pub(crate) parent: Option<ParentMachineContinuation>,
    pub(crate) locked: bool,
    pub(crate) goals: AiMachineGoals,
    pub(crate) normal_move_goal: Option<Vec3>,
    pub(crate) temporary_move: Option<TemporaryAiMove>,
    pub(crate) guard_target_types: [GuardTargetType; 2],
    pub guard_position: Option<Vec3>,
    pub guard_target: Option<ObjectId>,
    pub guard_area_trigger: Option<String>,
    pub(crate) idle_initial_sleep_offset: u16,
    pub(crate) idle_entry_frame: u32,
    pub(crate) idle_initialized: bool,
    /// C++ AIUpdateInterface::m_isAiDead, distinct from Object death/effectively-dead flags.
    #[serde(default)]
    pub(crate) ai_dead: bool,
}
impl Default for BaseAiContinuation {
    fn default() -> Self {
        Self {
            ai_state: AIState::Idle,
            attack_substate: AttackSubState::AimAtTarget,
            auto_acquire_when_idle: true,
            approach_timestamp: 0,
            prev_victim_pos: None,
            face_can_turn_in_place: false,
            face_active: false,
            face_goal_pos: None,
            face_loco_frame: 0,
            adjust_destinations: true,
            wander_width: 0.0,

            guard: Default::default(),
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
    pub(crate) fn parent_state(&self) -> Option<AIState> {
        self.parent.as_ref().map(|parent| parent.state)
    }
    pub(crate) fn root_state(&self) -> AIState {
        self.parent_state().unwrap_or(self.ai_state)
    }
    pub(crate) fn validate(&self) -> Result<(), String> {
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
        if self.guard_position.is_some_and(|p| !p.is_finite())
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
impl UnitAiRuntime {
    pub(crate) fn machine(&self) -> &BaseAiContinuation {
        &self.machine
    }
    pub(crate) fn capture_machine(&self) -> BaseAiContinuation {
        self.machine.clone()
    }
    pub(crate) fn restore_machine(&mut self, value: BaseAiContinuation) -> Result<(), String> {
        value.validate()?;
        self.machine = value;
        Ok(())
    }
    pub(crate) fn enter_root(&mut self, state: AIState) {
        self.machine.ai_state = state;
        self.machine.parent = None;
    }
    pub(crate) fn can_enter_child(&self) -> bool {
        matches!(
            self.machine.root_state(),
            AIState::GuardingArea
                | AIState::GuardingObject
                | AIState::GuardRetaliating
                | AIState::Hunting
                | AIState::AttackMoving
        )
    }
    pub(crate) fn enter_child(&mut self, state: AIState) -> bool {
        if !self.can_enter_child() {
            return false;
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
    pub(crate) fn resume_parent(&mut self) -> bool {
        let Some(parent) = self.machine.parent.take() else {
            return false;
        };
        self.machine.ai_state = parent.state;
        self.machine.goals = parent.goals;
        true
    }
    pub(crate) fn machine_locked(&self) -> bool {
        self.machine.locked
    }
    pub(crate) fn lock_machine(&mut self) {
        self.machine.locked = true;
    }
    pub(crate) fn mark_ai_dead(&mut self) {
        self.machine.ai_dead = true;
    }
    pub(crate) fn is_ai_dead(&self) -> bool {
        self.machine.ai_dead
    }
    pub(crate) fn clear_machine_after_exit(&mut self) {
        self.machine.parent = None;
        self.machine.ai_state = AIState::Uninitialized;
        self.machine.goals = Default::default();
    }
    pub(crate) fn enter_idle(&mut self, frame: u32, initial_sleep_offset: u16) {
        self.enter_root(AIState::Idle);
        self.machine.idle_entry_frame = frame;
        self.machine.idle_initial_sleep_offset = initial_sleep_offset;
        self.machine.idle_initialized = true;
    }
    pub(crate) fn set_machine_goal_object(&mut self, target: Option<(ObjectId, Vec3)>) {
        if self.machine.locked {
            return;
        }
        self.machine.goals.object = target.map(|(id, _)| id);
        if let Some((_, position)) = target {
            self.machine.goals.position = position;
        }
    }
    pub(crate) fn set_machine_goal_position(&mut self, pos: Vec3) {
        if !self.machine.locked {
            self.machine.goals.position = pos;
        }
    }
    pub(crate) fn guard_target_type(&self) -> GuardTargetType {
        self.machine.guard_target_types[0]
    }
    pub(crate) fn set_guard_target_type(&mut self, kind: GuardTargetType) {
        self.machine.guard_target_types[0] = kind;
        self.machine.guard_target_types[1] = GuardTargetType::None;
    }
    /// C++ uses slot 1 for the active type and slot 0 for a pending type.
    /// This owner stores them in the reverse order: active, then pending.
    pub(crate) fn stage_guard_target_type(&mut self, kind: GuardTargetType) {
        let slot = usize::from(self.machine.guard_target_types[0] != GuardTargetType::None);
        self.machine.guard_target_types[slot] = kind;
    }
    pub(crate) fn exit_guard_target_type(&mut self) {
        self.machine.guard_target_types[0] = self.machine.guard_target_types[1];
        self.machine.guard_target_types[1] = GuardTargetType::None;
    }
    pub(crate) fn set_raw_guard_position(&mut self, pos: Option<Vec3>) {
        self.machine.guard_position = pos;
    }
    pub(crate) fn set_raw_guard_target(&mut self, id: Option<ObjectId>) {
        self.machine.guard_target = id;
    }
    pub(crate) fn set_raw_guard_area(&mut self, name: Option<String>) {
        self.machine.guard_area_trigger = name;
    }
    pub(crate) fn raw_guard_position(&self) -> Option<Vec3> {
        self.machine.guard_position
    }
    pub(crate) fn raw_guard_target(&self) -> Option<ObjectId> {
        self.machine.guard_target
    }
    pub(crate) fn raw_guard_area(&self) -> Option<&str> {
        self.machine.guard_area_trigger.as_deref()
    }
    pub(crate) fn temporary_move(&self) -> Option<&TemporaryAiMove> {
        self.machine.temporary_move.as_ref()
    }
    pub(crate) fn begin_temporary_move(&mut self, value: TemporaryAiMove) {
        self.machine.temporary_move = Some(value);
    }
    pub(crate) fn take_temporary_move(&mut self) -> Option<TemporaryAiMove> {
        self.machine.temporary_move.take()
    }
}

impl UnitAiRuntime {
    pub(crate) fn set_attack_substate(&mut self, state: AttackSubState) {
        self.machine.attack_substate = state;
    }
    pub(crate) fn set_auto_acquire(&mut self, value: bool) {
        self.machine.auto_acquire_when_idle = value;
    }
    pub(crate) fn enter_hunt(&mut self) {
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
    pub(crate) fn leave_hunt(&mut self) {
        if self.machine.root_state() == AIState::Hunting {
            if let Some(parent) = self.machine.parent.take() {
                self.machine.goals = parent.goals;
            }
        }
        self.machine.hunt_scan = None;
    }
}

impl UnitAiRuntime {
    pub(crate) fn with_auto_acquire(value: bool) -> Self {
        let mut owner = Self::default();
        owner.machine.auto_acquire_when_idle = value;
        owner
    }
}

impl UnitAiRuntime {
    pub(crate) fn set_approach_timestamp(&mut self, value: u32) {
        self.machine.approach_timestamp = value;
    }
    pub(crate) fn set_prev_victim_pos(&mut self, value: Option<Vec3>) {
        self.machine.prev_victim_pos = value;
    }
    pub(crate) fn set_face_can_turn_in_place(&mut self, value: bool) {
        self.machine.face_can_turn_in_place = value;
    }
    pub(crate) fn set_face_active(&mut self, value: bool) {
        self.machine.face_active = value;
    }
    pub(crate) fn set_face_goal_pos(&mut self, value: Option<Vec3>) {
        self.machine.face_goal_pos = value;
    }
    pub(crate) fn set_face_loco_frame(&mut self, value: u32) {
        self.machine.face_loco_frame = value;
    }
    pub(crate) fn set_adjust_destinations(&mut self, value: bool) {
        self.machine.adjust_destinations = value;
    }
    pub(crate) fn set_wander_width(&mut self, value: f32) {
        self.machine.wander_width = value;
    }
}

impl BaseAiContinuation {
    /// Before the staged-target contract slot 1 held a previous value, not
    /// an in-flight transition. Never promote it when restoring that format.
    pub(crate) fn discard_legacy_guard_history(&mut self) {
        self.guard_target_types[1] = GuardTargetType::None;
    }
    pub(crate) fn is_inert(&self) -> bool {
        matches!(self.ai_state, AIState::Idle | AIState::Uninitialized)
            && self.parent.is_none()
            && !self.locked
            && self.temporary_move.is_none()
            && self.normal_move_goal.is_none()
            && self.guard_target_types[0] == GuardTargetType::None
            && self.guard_target_types[1] == GuardTargetType::None
            && self.guard.phase.is_none()
            && self.guard.scan.is_none()
            && self.hunt_scan.is_none()
            && self.quick_exit.is_none()
            && self.wander.is_none()
            && !self.face_active
            && self.attack_substate == AttackSubState::AimAtTarget
            && self.approach_timestamp == 0
            && self.prev_victim_pos.is_none()
            && !self.idle_initialized
            && self.goals.object.is_none()
            && self.goals.position == Vec3::ZERO
            && self.goals.path.is_empty()
            && self.goals.waypoint.is_none()
            && self.goals.squad.is_none()
    }
}

impl BaseAiContinuation {
    pub(crate) fn projected_state(&self) -> AIState {
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
    pub(crate) fn record_idle_entry_body(&mut self, frame: u32, offset: u16) {
        self.machine.idle_entry_frame = frame;
        self.machine.idle_initial_sleep_offset = offset;
        self.machine.idle_initialized = true;
    }
    pub(crate) fn consume_idle_initialization(&mut self) -> bool {
        let pending = self.machine.idle_initialized;
        self.machine.idle_initialized = false;
        pending
    }
    pub(crate) fn current_leaf_state(&self) -> AIState {
        self.machine.ai_state
    }
    pub(crate) fn root_state(&self) -> AIState {
        self.machine.root_state()
    }
    pub(crate) fn parent_state(&self) -> Option<AIState> {
        self.machine.parent_state()
    }
    pub(crate) fn set_current_child_state(&mut self, state: AIState) -> bool {
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
    pub(crate) fn enter_normal_move_body(&mut self, goal: Vec3) {
        self.machine.normal_move_goal = Some(goal);
    }
}

impl BaseAiContinuation {
    pub(crate) fn deadline_projection(&self) -> (Option<u32>, Option<u32>, Option<u32>) {
        (self.guard.scan, self.hunt_scan, self.quick_exit)
    }
}
