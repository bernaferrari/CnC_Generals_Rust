use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::states::{
    AIEnterState, AIPickUpCrateState, AttackExitConditionsInterface, AttackStateMachine,
};
use crate::ai::vision_factors;
use crate::ai::{object_registry::get_legacy_object, the_ai};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::coord::*;
use crate::common::vector_ext::Vector3Ext;
use crate::common::xfer::{Xfer, XferExt, XferVersion};
use crate::common::*;
use crate::helpers::{TheGameLogic, ThePartitionManager, game_logic_random_value};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::*;
use crate::state_machine::*;

use std::sync::{Arc, Mutex};

/// Wave 429: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Close enough distance constant
const CLOSE_ENOUGH: f32 = 25.0;
/// Crate pickup range squared (matches AIPickUpCrateState)
const CRATE_PICKUP_RANGE_SQR: f32 = 100.0;
fn retaliate_attack_aggressor_condition(
    state: &dyn StateImplementation,
    _user_data: &StateTransitionUserData,
) -> bool {
    // Update already holds the machine mutex. Do not lock it again.
    let owner = if let Some(idle) = state.as_any().downcast_ref::<AIGuardRetaliateIdleState>() {
        idle.owner.upgrade()
    } else if let Some(ret) = state.as_any().downcast_ref::<AIGuardRetaliateReturnState>() {
        ret.owner.upgrade()
    } else {
        None
    };
    owner.as_ref().is_some_and(has_attacked_me_from_owner)
}

fn get_guard_enemy_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 30;
    };
    let data = ai_guard.get_ai_data();
    let Ok(data_guard) = data.read() else {
        return 30;
    };
    data_guard.guard_enemy_scan_rate
}

fn get_guard_chase_unit_frames() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 0;
    };
    let data = ai_guard.get_ai_data();
    let Ok(data_guard) = data.read() else {
        return 0;
    };
    data_guard.guard_chase_unit_frames
}

fn get_guard_enemy_return_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 60;
    };
    let data = ai_guard.get_ai_data();
    let Ok(data_guard) = data.read() else {
        return 60;
    };
    data_guard.guard_enemy_return_scan_rate
}

fn scan_guard_retaliate_inner_target(
    owner_id: ObjectID,
    pos: &Coord3D,
) -> Option<ObjectID> {
    return crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {

    if !owner_guard.is_able_to_attack() {
        return None;
    }

    let is_enter_guard = owner_guard.get_template().is_enter_guard();
    let is_hijack_guard = owner_guard.get_template().is_hijack_guard();

    let vision_range = AIGuardRetaliateMachine::get_std_guard_range(owner_guard.get_id());
    let Some(partition) = ThePartitionManager::get() else {
        return None;
    };

    partition.get_closest_object_2d(pos, vision_range, |candidate| {
        if candidate.get_id() == owner_guard.get_id() {
            return false;
        }
        if candidate.is_effectively_dead() {
            return false;
        }
        if owner_guard.is_off_map() != candidate.is_off_map() {
            return false;
        }

        if is_enter_guard {
            if is_hijack_guard {
                if owner_guard.relationship_to(candidate) != Relationship::Enemies {
                    return false;
                }
                return TheActionManager::can_hijack_vehicle(
                    &owner_guard,
                    candidate,
                    CommandSourceType::FromAi,
                );
            }

            if owner_guard.relationship_to(candidate) != Relationship::Neutral {
                return false;
            }
            return TheActionManager::can_enter_object(
                &owner_guard,
                candidate,
                CommandSourceType::FromAi,
                CanEnterType::CheckCapacity,
            );
        }

        if owner_guard.relationship_to(candidate) != Relationship::Enemies {
            return false;
        }
        if candidate.is_kind_of(KindOf::Structure) && !candidate.is_kind_of(KindOf::Defense) {
            return false;
        }
        matches!(
            owner_guard.get_able_to_attack_specific_object(
                AbleToAttackType::NewTarget,
                candidate,
                CommandSourceType::FromAi,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        )
    })
    });
}

/// Guard retaliate state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardRetaliateStateType {
    /// Attack anything within this area till death
    Inner = 5000,
    /// Wait till something shows up to attack
    Idle = 5001,
    /// Attack anything within this area that has been aggressive, until the timer expires
    Outer = 5002,
    /// Restore to a position within the inner circle
    Return = 5003,
    /// Pick up a crate from an enemy we killed
    GetCrate = 5004,
    /// Attack something that attacked me (that I can attack)
    AttackAggressor = 5005,
}

/// Exit conditions for guard retaliate attack states
#[derive(Debug, Clone)]
pub struct GuardRetaliateExitConditions {
    /// Bitmask of conditions to consider
    conditions_to_consider: u32,
    /// Center position for radius checks
    center: Coord3D,
    /// Radius squared for distance checks
    radius_sqr: f32,
    /// Frame at which we give up attacking
    attack_give_up_frame: u32,
}

/// Exit condition flags for guard retaliate
pub mod guard_retaliate_exit_conditions {
    pub const ATTACK_EXIT_IF_OUTSIDE_RADIUS: u32 = 0x01;
    pub const ATTACK_EXIT_IF_EXPIRED_DURATION: u32 = 0x02;
    pub const ATTACK_EXIT_IF_NO_UNIT_FOUND: u32 = 0x04;
}

impl GuardRetaliateExitConditions {
    pub fn new() -> Self {
        Self {
            conditions_to_consider: 0,
            center: Coord3D::new(0.0, 0.0, 0.0),
            radius_sqr: 0.0,
            attack_give_up_frame: 0,
        }
    }

    pub fn should_exit(&self, machine: &StateMachine) -> bool {
        // Wave 429: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        let goal_object_id = machine.get_goal_object_id();

        if goal_object_id == crate::common::INVALID_ID {
            return (self.conditions_to_consider
                & guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND)
                != 0;
        }

        if (self.conditions_to_consider
            & guard_retaliate_exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION)
            != 0
        {
            if machine.get_current_frame() >= self.attack_give_up_frame {
                return true;
            }
        }

        if (self.conditions_to_consider
            & guard_retaliate_exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS)
            != 0
        {
            if let Some(true) =
                crate::object::registry::OBJECT_REGISTRY.with_object(goal_object_id, |goal_ref| {
                    let obj_pos = goal_ref.get_position();
                    let delta_aggressor =
                        Coord3D::new(obj_pos.x - self.center.x, obj_pos.y - self.center.y, 0.0);
                    if Vector3Ext::length_sqr(&delta_aggressor) > self.radius_sqr {
                        return true;
                    }
                    false
                })
            {
                return true;
            }

            if let Some(owner) = machine.get_owner() {
                if let Some(true) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_ref| {
                    let my_pos = owner_ref.get_position();
                    let my_range = Coord3D::new(my_pos.x - self.center.x, my_pos.y - self.center.y, 0.0);
                    let guard_range = AIGuardRetaliateMachine::get_std_guard_range(owner);
                    Vector3Ext::length_sqr(&my_range) > guard_range * guard_range
                }) {
                    return true;
                }
            }
        }

        false
    }

    pub fn set_conditions(&mut self, conditions: u32) {
        self.conditions_to_consider = conditions;
    }

    pub fn set_center(&mut self, center: Coord3D) {
        self.center = center;
    }

    pub fn set_radius_sqr(&mut self, radius_sqr: f32) {
        self.radius_sqr = radius_sqr;
    }

    pub fn set_attack_give_up_frame(&mut self, frame: u32) {
        self.attack_give_up_frame = frame;
    }
}

#[derive(Debug, Clone)]
struct GuardRetaliateExitConditionsHandle {
    inner: Arc<Mutex<GuardRetaliateExitConditions>>,
}

impl GuardRetaliateExitConditionsHandle {
    fn new(inner: Arc<Mutex<GuardRetaliateExitConditions>>) -> Self {
        Self { inner }
    }
}

impl AttackExitConditionsInterface for GuardRetaliateExitConditionsHandle {
    fn should_exit(&self, machine: &StateMachine) -> bool {
        let Ok(guard) = self.inner.lock() else {
            return false;
        };
        guard.should_exit(machine)
    }
}

#[derive(Debug)]
pub struct GuardRetaliateSharedState {
    owner_id: ObjectID,
    fields: Mutex<GuardRetaliateSharedFields>,
}

#[derive(Debug)]
struct GuardRetaliateSharedFields {
    position_to_guard: Coord3D,
    nemesis_to_attack: ObjectID,
    pending_state: Option<u32>,
}

impl Default for GuardRetaliateSharedState {
    fn default() -> Self {
        Self {
            owner_id: crate::common::INVALID_ID,
            fields: Mutex::new(GuardRetaliateSharedFields {
                position_to_guard: Coord3D::default(),
                nemesis_to_attack: ObjectID::default(),
                pending_state: None,
            }),
        }
    }
}

impl GuardRetaliateSharedState {
    fn new(owner_id: ObjectID) -> Self {
        Self {
            owner_id,
            fields: Mutex::new(GuardRetaliateSharedFields {
                position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
                nemesis_to_attack: crate::common::INVALID_ID,
                pending_state: None,
            }),
        }
    }

    fn owner(&self) -> Option<ObjectID> {
        Some(self.owner_id).filter(|id| *id != crate::common::INVALID_ID)
    }

    fn request_state(&self, state: u32) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.pending_state = Some(state);
        }
    }

    fn take_pending_state(&self) -> Option<u32> {
        self.fields
            .lock()
            .ok()
            .and_then(|mut fields| fields.pending_state.take())
    }

    /// Request a transition. The owning machine applies it after the child
    /// update returns (see `AIGuardRetaliateMachine::update`), which preserves
    /// the legacy callback order without re-entering the machine.
    fn change_state(&self, state: GuardRetaliateStateType) {
        self.request_state(state as u32);
    }

    fn get_position_to_guard(&self) -> Coord3D {
        self.fields
            .lock()
            .map(|fields| fields.position_to_guard)
            .unwrap_or_else(|_| Coord3D::new(0.0, 0.0, 0.0))
    }

    fn set_position_to_guard(&self, pos: Coord3D) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.position_to_guard = pos;
        }
    }

    fn get_nemesis_to_attack(&self) -> ObjectID {
        self.fields
            .lock()
            .map(|fields| fields.nemesis_to_attack)
            .unwrap_or(crate::common::INVALID_ID)
    }

    fn set_nemesis_to_attack(&self, id: ObjectID) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.nemesis_to_attack = id;
        }
    }

    fn sync_from_machine(&self, position_to_guard: Coord3D, nemesis_to_attack: ObjectID) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.position_to_guard = position_to_guard;
            fields.nemesis_to_attack = nemesis_to_attack;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DummyState;

    impl StateImplementation for DummyState {
        fn update(&mut self) -> StateReturnType {
            StateReturnType::Continue
        }
    }

    #[test]
    fn guard_retaliate_pending_state_tracks_requests() {
        let mut machine = StateMachine::new_with_owner_id(crate::common::INVALID_ID, "test_retaliate");
        machine.define_state(
            GuardRetaliateStateType::Inner as u32,
            Box::new(DummyState),
            None,
            None,
            None,
        );
        machine.define_state(
            GuardRetaliateStateType::Idle as u32,
            Box::new(DummyState),
            None,
            None,
            None,
        );

        let shared = GuardRetaliateSharedState::new(crate::common::INVALID_ID);
        shared.change_state(GuardRetaliateStateType::Inner);
        let requested = shared.take_pending_state();
        assert_eq!(requested, Some(GuardRetaliateStateType::Inner as u32));
        let _ = machine.set_current_state(requested.unwrap());
        shared.change_state(GuardRetaliateStateType::Idle);
        let _ = machine.set_current_state(shared.take_pending_state().unwrap());

        assert_eq!(
            machine.get_current_state_id(),
            Some(GuardRetaliateStateType::Idle as u32)
        );
    }

    #[test]
    fn guard_retaliate_shared_fields_are_isolated_per_instance_and_shared_with_states() {
        let shared_a = Arc::new(GuardRetaliateSharedState::new(crate::common::INVALID_ID));
        let shared_b = Arc::new(GuardRetaliateSharedState::new(crate::common::INVALID_ID));
        let child_view_a = Arc::clone(&shared_a);

        shared_a.set_nemesis_to_attack(41);
        shared_a.set_position_to_guard(Coord3D::new(1.0, 2.0, 3.0));
        shared_b.set_nemesis_to_attack(82);

        assert_eq!(child_view_a.get_nemesis_to_attack(), 41);
        assert_eq!(
            child_view_a.get_position_to_guard(),
            Coord3D::new(1.0, 2.0, 3.0)
        );
        assert_eq!(shared_b.get_nemesis_to_attack(), 82);
        assert_eq!(
            shared_b.get_position_to_guard(),
            Coord3D::new(0.0, 0.0, 0.0)
        );
    }
}

/// Main guard retaliate state machine - similar to guard but focuses on retaliation
#[derive(Debug)]
pub struct AIGuardRetaliateMachine {
    /// Base state machine
    base: StateMachine,
    /// Shared state used by guard retaliate states
    shared: Arc<GuardRetaliateSharedState>,
    /// Position to guard
    position_to_guard: Coord3D,
    /// Nemesis to attack
    nemesis_to_attack: ObjectID,
}

impl AIGuardRetaliateMachine {
    pub fn new(owner_id: ObjectID) -> Self {
        let base = StateMachine::new_with_owner_id(owner_id, "AIGuardRetaliateMachine");
        let shared = Arc::new(GuardRetaliateSharedState::new(owner_id));

        let mut machine = Self {
            base,
            shared,
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
        };

        machine.define_guard_retaliate_states(owner_id);
        let _ = machine.base.init_default_state();
        machine
    }

    fn define_guard_retaliate_states(&mut self, owner_id: ObjectID) {
        let attack_aggressors = [StateConditionInfo::new(
            retaliate_attack_aggressor_condition,
            GuardRetaliateStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];

        // Build each state first (it only copies machine snapshot data), then
        // register — avoids overlapping borrows of `self.base`.
        let attack_aggressor_state =
            AIGuardRetaliateAttackAggressorState::new(&self.base, self.shared.clone());
        let return_state =
            AIGuardRetaliateReturnState::new(&self.base, self.shared.clone(), owner_id);
        let idle_state = AIGuardRetaliateIdleState::new(&self.base, self.shared.clone(), owner_id);
        let inner_state = AIGuardRetaliateInnerState::new(&self.base, self.shared.clone());
        let outer_state = AIGuardRetaliateOuterState::new(&self.base, self.shared.clone());
        let pickup_crate_state =
            AIGuardRetaliatePickUpCrateState::new(&self.base, self.shared.clone());

        // Order matters: first state becomes default.
        self.base.define_state(
            GuardRetaliateStateType::AttackAggressor as u32,
            Box::new(attack_aggressor_state),
            Some(GuardRetaliateStateType::Return as u32),
            Some(GuardRetaliateStateType::Return as u32),
            None,
        );

        self.base.define_state(
            GuardRetaliateStateType::Return as u32,
            Box::new(return_state),
            Some(GuardRetaliateStateType::Idle as u32),
            Some(GuardRetaliateStateType::Inner as u32),
            Some(&attack_aggressors),
        );

        self.base.define_state(
            GuardRetaliateStateType::Idle as u32,
            Box::new(idle_state),
            Some(GuardRetaliateStateType::Inner as u32),
            Some(EXIT_MACHINE_WITH_SUCCESS),
            Some(&attack_aggressors),
        );

        self.base.define_state(
            GuardRetaliateStateType::Inner as u32,
            Box::new(inner_state),
            Some(GuardRetaliateStateType::Outer as u32),
            Some(GuardRetaliateStateType::Outer as u32),
            None,
        );

        self.base.define_state(
            GuardRetaliateStateType::Outer as u32,
            Box::new(outer_state),
            Some(GuardRetaliateStateType::GetCrate as u32),
            Some(GuardRetaliateStateType::GetCrate as u32),
            None,
        );

        self.base.define_state(
            GuardRetaliateStateType::GetCrate as u32,
            Box::new(pickup_crate_state),
            Some(GuardRetaliateStateType::Return as u32),
            Some(GuardRetaliateStateType::Return as u32),
            None,
        );
    }

    pub fn is_idle(&self) -> bool {
        // C++ AIGuardRetaliate.cpp:181 compares to AI_IDLE (0), not the retaliate idle state.
        self.base.get_current_state_id() == Some(0)
    }

    pub fn get_position_to_guard(&self) -> &Coord3D {
        &self.position_to_guard
    }

    pub fn set_target_position_to_guard(&mut self, pos: &Coord3D) {
        self.position_to_guard = *pos;
        self.shared.set_position_to_guard(*pos);
    }

    pub fn set_nemesis_id(&mut self, id: ObjectID) {
        self.nemesis_to_attack = id;
        self.shared.set_nemesis_to_attack(id);
    }

    pub fn get_nemesis_id(&self) -> ObjectID {
        self.shared.get_nemesis_to_attack()
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.base.init_default_state()
    }

    pub fn set_state(&mut self, state: GuardRetaliateStateType) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn update(&mut self) -> StateReturnType {
        // Child states request transitions while the machine is mid-update.
        // Apply the request only after the child update returns, preserving the
        // legacy guard retaliate callback order.
        let result = self.base.update();
        if let Some(state_id) = self.shared.take_pending_state() {
            let _ = self.base.set_current_state(state_id);
        }
        result
    }

    pub fn look_for_inner_target(&mut self) -> bool {
        let Some(owner_id) = self.base.get_owner() else {
            return false;
        };
        let team_target = crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
            if !owner_guard.is_able_to_attack() {
                return Some(crate::common::INVALID_ID);
            }
            let team_arc = owner_guard.get_team()?;
            let team_guard = team_arc.read().ok()?;
            if team_guard.attack_common_target() {
                let id = team_guard.get_team_target_object();
                if id != crate::common::INVALID_ID { return Some(id); }
            }
            None
        });
        match team_target {
            None => return false,
            Some(Some(id)) if id == crate::common::INVALID_ID => return false,
            Some(Some(id)) => { self.set_nemesis_id(id); return true; }
            Some(None) => {}
        }

        let pos = *self.get_position_to_guard();
        if let Some(target_id) = scan_guard_retaliate_inner_target(owner_id, &pos) {
            self.set_nemesis_id(target_id);
            return true;
        }

        false
    }

    pub fn get_std_guard_range(obj_id: ObjectID) -> f32 {
        let ai_store = the_ai();
        let ai = ai_store.read().ok();
        ai.and_then(|ai| {
            ai.get_adjusted_vision_range_for_object(
                obj_id,
                vision_factors::OWNER_TYPE | vision_factors::MOOD | vision_factors::GUARD_INNER,
            )
            .ok()
        })
        .unwrap_or(100.0)
    }

    pub fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ AIGuardRetaliateMachine::crc is empty.
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        if version >= 2 {
            self.base.xfer(xfer).map_err(|e| e.to_string())?;
        }

        if !xfer.is_loading() {
            self.nemesis_to_attack = self.shared.get_nemesis_to_attack();
        }

        xfer.xfer_object_id(&mut self.nemesis_to_attack)
            .map_err(|e| format!("Failed to xfer nemesis_to_attack: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.x)
            .map_err(|e| format!("Failed to xfer position_to_guard.x: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.y)
            .map_err(|e| format!("Failed to xfer position_to_guard.y: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.z)
            .map_err(|e| format!("Failed to xfer position_to_guard.z: {:?}", e))?;

        self.shared
            .sync_from_machine(self.position_to_guard, self.nemesis_to_attack);

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        self.base
            .load_post_process()
            .map_err(|e| format!("guard retaliate load_post_process: {e}"))
    }
}

// State implementations for guard retaliate

#[derive(Debug)]
struct GuardRetaliateState {
    base: State,
    shared: Arc<GuardRetaliateSharedState>,
}

impl GuardRetaliateState {
    fn new(machine: &StateMachine, shared: Arc<GuardRetaliateSharedState>, name: &str) -> Self {
        Self {
            base: State::new(machine, name),
            shared,
        }
    }

    fn state(&self) -> &State {
        &self.base
    }

    fn state_mut(&mut self) -> &mut State {
        &mut self.base
    }
    fn owner_arc(&self) -> Option<ObjectID> {
        self.shared.owner()
    }


    fn get_position_to_guard(&self) -> Coord3D {
        self.shared.get_position_to_guard()
    }

    fn get_nemesis_to_attack(&self) -> ObjectID {
        self.shared.get_nemesis_to_attack()
    }

    fn set_nemesis_to_attack(&self, id: ObjectID) {
        self.shared.set_nemesis_to_attack(id);
    }
}

/// Inner guard retaliate state - attack anything within area with focus on retaliation
#[derive(Debug)]
pub struct AIGuardRetaliateInnerState {
    base: GuardRetaliateState,
    exit_conditions: Arc<Mutex<GuardRetaliateExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
    enter_state: Option<AIEnterState>,
}

impl AIGuardRetaliateInnerState {
    pub fn new(machine: &StateMachine, shared: Arc<GuardRetaliateSharedState>) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliateInner"),
            exit_conditions: Arc::new(Mutex::new(GuardRetaliateExitConditions::new())),
            is_attacking: false,
            attack_machine: None,
            enter_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardRetaliateInnerState {
    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.owner_arc() else {
            return StateReturnType::Failure;
        };

        let nemesis_id = self.base.get_nemesis_to_attack();
        let Some(nemesis) = (nemesis_id != crate::common::INVALID_ID)
            .then(|| get_legacy_object(nemesis_id))
            .flatten()
        else {
            self.is_attacking = false;
            self.attack_machine = None;
            self.enter_state = None;
            return StateReturnType::Success;
        };

        let is_enter_guard = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner, |guard| guard.get_template().is_enter_guard())
            .unwrap_or(false);
        if is_enter_guard {
            let scratch = StateMachine::new_with_owner_id(owner, "AIEnter");
            let mut enter_state = AIEnterState::new(&scratch);
            enter_state.preset_goal_id = nemesis;
            self.is_attacking = false;
            self.attack_machine = None;
            self.enter_state = Some(enter_state);
            if let Some(enter_state) = self.enter_state.as_mut() {
                let result = enter_state.on_enter();
                if result == StateReturnType::Continue {
                    return StateReturnType::Continue;
                }
            }
            return StateReturnType::Success;
        }

        let pos = self.base.get_position_to_guard();
        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            let radius = 1.5 * AIGuardRetaliateMachine::get_std_guard_range(owner);
            exit_guard.set_center(pos);
            exit_guard.set_radius_sqr(radius * radius);
            exit_guard.set_conditions(
                guard_retaliate_exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::with_owner_id(
            owner_id,
            "AIGuardRetaliateAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(GuardRetaliateExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(nemesis));

        let result = attack_machine.init_default_state();
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);
        self.enter_state = None;

        if result == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn update(&mut self) -> StateReturnType {
        if let Some(attack_machine) = self.attack_machine.as_mut() {
            return attack_machine.update();
        }
        if let Some(enter_state) = self.enter_state.as_mut() {
            return enter_state.update();
        }
        StateReturnType::Success
    }

    fn on_exit(&mut self, _status: StateExitType) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        if let Some(mut enter_state) = self.enter_state.take() {
            enter_state.on_exit(_status);
        }
        self.is_attacking = false;

        if let Some(owner) = self.base.owner_arc() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(crate::common::INVALID_ID);
                    }
                }
            });
        }
    }
}

/// Idle guard retaliate state - wait for targets with retaliation focus
#[derive(Debug)]
pub struct AIGuardRetaliateIdleState {
    base: GuardRetaliateState,
    owner_id: ObjectID,
    next_enemy_scan_time: u32,
}

impl AIGuardRetaliateIdleState {
    pub fn new(
        machine: &StateMachine,
        shared: Arc<GuardRetaliateSharedState>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliateIdleState"),
            owner_id,
            next_enemy_scan_time: 0,
        }
    }

    pub fn is_guard_idle(&self) -> bool {
        true
    }
}

impl StateImplementation for AIGuardRetaliateIdleState {
    fn on_enter(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_scan_rate();
        self.next_enemy_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));
        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if now < self.next_enemy_scan_time {
            return StateReturnType::Sleep(self.next_enemy_scan_time - now);
        }

        self.next_enemy_scan_time = now.saturating_add(get_guard_enemy_scan_rate());

        let Some(owner) = self.base.owner_arc() else {
            return StateReturnType::Failure;
        };
        let early = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if ai_guard.get_crate_id() != crate::common::INVALID_ID {
                        return 1u8;
                    }
                }
            }
            if let Some(team_arc) = owner_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    if team_guard.attack_common_target()
                        && team_guard.get_team_target_object() != crate::common::INVALID_ID
                    {
                        return 2u8;
                    }
                }
            }
            0u8
        });
        match early {
            None => return StateReturnType::Failure,
            Some(1) => {
                self.base.shared.request_state(GuardRetaliateStateType::GetCrate as u32);
                return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
            }
            Some(2) => {
                if let Some(team_target) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |og| {
                    og.get_team().and_then(|t| t.read().ok().map(|g| g.get_team_target_object()))
                }).flatten() {
                    if team_target != crate::common::INVALID_ID {
                        self.base.set_nemesis_to_attack(team_target);
                        return StateReturnType::Success;
                    }
                }
            }
            _ => {}
        }

        let guard_pos = self.base.get_position_to_guard();
        drop(owner_guard);

        if let Some(target_id) = scan_guard_retaliate_inner_target(owner, &guard_pos) {
            self.base.set_nemesis_to_attack(target_id);
            return StateReturnType::Success;
        }

        StateReturnType::Failure
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // Cleanup when exiting idle guard retaliate state
    }
}

/// Outer guard retaliate state - attack aggressive targets with timer and retaliation priority
#[derive(Debug)]
pub struct AIGuardRetaliateOuterState {
    base: GuardRetaliateState,
    exit_conditions: Arc<Mutex<GuardRetaliateExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AIGuardRetaliateOuterState {
    pub fn new(machine: &StateMachine, shared: Arc<GuardRetaliateSharedState>) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliateOuter"),
            exit_conditions: Arc::new(Mutex::new(GuardRetaliateExitConditions::new())),
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardRetaliateOuterState {
    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.owner_arc() else {
            return StateReturnType::Failure;
        };

        let mut nemesis_id = self.base.get_nemesis_to_attack();
        if nemesis_id == crate::common::INVALID_ID {
            nemesis_id = self
                .base
                .state()
                .get_machine_goal_object_id()
                .unwrap_or(crate::common::INVALID_ID);
        }
        let Some(nemesis) = (if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        }) else {
            self.is_attacking = false;
            self.attack_machine = None;
            return StateReturnType::Success;
        };

        let pos = self.base.get_position_to_guard();
        let std_guard_range = AIGuardRetaliateMachine::get_std_guard_range(owner);
        let range = {
            let owner_id = owner;
            let ai_store = the_ai();
            let Ok(ai) = ai_store.read() else {
                return StateReturnType::Failure;
            };
            ai.get_adjusted_vision_range_for_object(
                owner_id,
                vision_factors::OWNER_TYPE | vision_factors::MOOD,
            )
            .unwrap_or(std_guard_range)
        };

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            let radius = 0.67 * (range + std_guard_range);
            exit_guard.set_center(pos);
            exit_guard.set_radius_sqr(radius * radius);
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            exit_guard.set_conditions(
                guard_retaliate_exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::with_owner_id(
            owner_id,
            "AIGuardRetaliateAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(GuardRetaliateExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(nemesis));
        let result = attack_machine.init_default_state();

        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if result == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn update(&mut self) -> StateReturnType {
        // Wave 429: empty dual-world → Continue.
        if dual_world_registry_unavailable() {
            return StateReturnType::Continue;
        }

        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };

        let goal_id = attack_machine.get_goal_object_id();
        if goal_id != crate::common::INVALID_ID {
            if let Some(goal_pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(goal_id, |goal_guard| *goal_guard.get_position())
            {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    let delta = Coord3D::new(
                        exit_guard.center.x - goal_pos.x,
                        exit_guard.center.y - goal_pos.y,
                        exit_guard.center.z - goal_pos.z,
                    );
                    if let Some(owner) = self.base.owner_arc() {
                        let vision = AIGuardRetaliateMachine::get_std_guard_range(owner);
                        if Vector3Ext::length_sqr(&delta) <= vision * vision {
                            exit_guard.set_attack_give_up_frame(
                                TheGameLogic::get_frame()
                                    .saturating_add(get_guard_chase_unit_frames()),
                            );
                        }
                    }
                }
            }
        }

        attack_machine.update()
    }

    fn on_exit(&mut self, _status: StateExitType) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        self.is_attacking = false;
    }
}

/// Return guard retaliate state - move back to guard position
#[derive(Debug)]
pub struct AIGuardRetaliateReturnState {
    base: GuardRetaliateState,
    owner_id: ObjectID,
    next_return_scan_time: u32,
    goal_position: Coord3D,
}

impl AIGuardRetaliateReturnState {
    pub fn new(
        machine: &StateMachine,
        shared: Arc<GuardRetaliateSharedState>,
        owner_id: ObjectID,
    ) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliateReturn"),
            owner_id,
            next_return_scan_time: 0,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl StateImplementation for AIGuardRetaliateReturnState {
    fn on_enter(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_return_scan_rate();
        self.next_return_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        self.goal_position = self.base.get_position_to_guard();
        let Some(owner) = self.base.owner_arc() else {
            return StateReturnType::Failure;
        };
        let goal = self.goal_position;
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
            if let Some(ai) = owner_guard.get_ai_update_interface_mut() {
                if ai.is_doing_ground_movement() {
                    let _ = ai.adjust_destination(&mut self.goal_position);
                }
                ai.ai_move_to_position(&goal, false, CommandSourceType::FromAi);
            }
        });
        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if now >= self.next_return_scan_time {
            self.next_return_scan_time = now.saturating_add(get_guard_enemy_return_scan_rate());

            let Some(owner) = self.base.owner_arc() else {
                return StateReturnType::Failure;
            };
            if let Some(target_id) = scan_guard_retaliate_inner_target(owner, &self.goal_position)
            {
                self.base.set_nemesis_to_attack(target_id);
                return StateReturnType::Failure;
            }
        }

        let Some(owner) = self.base.owner_arc() else {
            return StateReturnType::Failure;
        };
        let Some(owner_pos) = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner, |owner_guard| *owner_guard.get_position())
        else {
            return StateReturnType::Failure;
        };
        let dx = owner_pos.x - self.goal_position.x;
        let dy = owner_pos.y - self.goal_position.y;
        if dx * dx + dy * dy <= CLOSE_ENOUGH * CLOSE_ENOUGH {
            return StateReturnType::Success;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // Nothing to clean up.
    }
}

/// Pick up crate state for guard retaliate
#[derive(Debug)]
pub struct AIGuardRetaliatePickUpCrateState {
    base: GuardRetaliateState,
    pickup: Option<AIPickUpCrateState>,
}

impl AIGuardRetaliatePickUpCrateState {
    pub fn new(machine: &StateMachine, shared: Arc<GuardRetaliateSharedState>) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliatePickUpCrate"),
            pickup: None,
        }
    }
}

impl StateImplementation for AIGuardRetaliatePickUpCrateState {
    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.owner_arc() else {
            return StateReturnType::Failure;
        };
        let Some(crate_id) = crate::object::registry::OBJECT_REGISTRY
            .with_object(owner, |owner_guard| owner_guard.ai_fire_crate_id)
        else {
            return StateReturnType::Failure;
        };
        if crate_id == crate::common::INVALID_ID {
            return StateReturnType::Success;
        }
        let crate_pos = crate::object::registry::OBJECT_REGISTRY
            .with_object(crate_id, |goal| *goal.get_position());

        let scratch = StateMachine::new_with_owner_id(owner, "AIPickUpCrate");
        let mut pickup = AIPickUpCrateState::new(&scratch);
        pickup.preset_goal_id = crate_id;
        if let Some(pos) = crate_pos {
            pickup.goal_position = pos;
            pickup.base.goal_position = pos;
        }
        let result = pickup.on_enter();
        self.pickup = Some(pickup);
        result
    }

    fn update(&mut self) -> StateReturnType {
        let Some(pickup) = self.pickup.as_mut() else {
            return StateReturnType::Success;
        };
        pickup.update()
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // C++ AIGuardRetaliatePickUpCrateState::onExit is empty.
        self.pickup = None;
    }
}

/// Attack aggressor state for guard retaliate - enhanced retaliation behavior
#[derive(Debug)]
pub struct AIGuardRetaliateAttackAggressorState {
    base: GuardRetaliateState,
    exit_conditions: Arc<Mutex<GuardRetaliateExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AIGuardRetaliateAttackAggressorState {
    pub fn new(machine: &StateMachine, shared: Arc<GuardRetaliateSharedState>) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliateAttackAggressor"),
            exit_conditions: Arc::new(Mutex::new(GuardRetaliateExitConditions::new())),
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardRetaliateAttackAggressorState {
    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.owner_arc() else {
            return StateReturnType::Failure;
        };

        // C++ AIGuardRetaliateAttackAggressorState::onEnter (745-760):
        // prefer machine nemesis, then last-damage source only if not DAMAGE_HEALING.
        let mut nemesis_id = self.base.get_nemesis_to_attack();
        if nemesis_id == crate::common::INVALID_ID {
            let found = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                let body = owner_guard.get_body_module()?;
                let body_guard = body.lock().ok()?;
                let info = body_guard.get_last_damage_info()?;
                if info.source_id == crate::common::INVALID_ID
                    || info.input.damage_type == crate::damage::DamageType::Healing
                {
                    return None;
                }
                Some(info.source_id)
            }).flatten();
            if let Some(source_id) = found {
                let enemy = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                    crate::object::registry::OBJECT_REGISTRY.with_object(source_id, |target_guard| {
                        owner_guard.relationship_to(target_guard) == Relationship::Enemies
                    }).unwrap_or(false)
                }).unwrap_or(false);
                if enemy {
                    self.base.set_nemesis_to_attack(source_id);
                }
                nemesis_id = source_id;
            }
        }
        let nemesis = if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        };

        let Some(nemesis) = nemesis else {
            self.is_attacking = false;
            self.attack_machine = None;
            return StateReturnType::Success;
        };

        let pos = self.base.get_position_to_guard();
        let std_guard_range = AIGuardRetaliateMachine::get_std_guard_range(owner);
        let range = {
            let owner_id = owner;
            let ai_store = the_ai();
            let Ok(ai) = ai_store.read() else {
                return StateReturnType::Failure;
            };
            ai.get_adjusted_vision_range_for_object(
                owner_id,
                vision_factors::OWNER_TYPE | vision_factors::MOOD,
            )
            .unwrap_or(std_guard_range)
        };

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_center(pos);
            exit_guard.set_radius_sqr((range + std_guard_range) * (range + std_guard_range));
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            exit_guard.set_conditions(
                guard_retaliate_exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::with_owner_id(
            owner_id,
            "AIGuardRetaliateAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(GuardRetaliateExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(nemesis));
        let result = attack_machine.init_default_state();

        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if result == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn update(&mut self) -> StateReturnType {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };
        attack_machine.update()
    }

    fn on_exit(&mut self, _status: StateExitType) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        self.is_attacking = false;

        if let Some(owner) = self.base.owner_arc() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(crate::common::INVALID_ID);
                    }
                }
            });
        }
    }
}

/// Helper function to check if an object has attacked and can be retaliated against
pub fn has_attacked_me_and_i_can_return_fire_retaliate(owner_id: ObjectID) -> bool {
    has_attacked_me_from_owner(owner_id)
}

fn has_attacked_me_from_owner(owner_id: ObjectID) -> bool {
    let Some(last_attacker) = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_ref| {
        let body_module = owner_ref.get_body_module()?;
        let mut body_guard = body_module.lock().ok()?;
        let last_attacker = body_guard.get_clearable_last_attacker();
        if last_attacker == crate::common::INVALID_ID {
            return None;
        }
        body_guard.clear_last_attacker();
        Some(last_attacker)
    }).flatten() else {
        return false;
    };
    crate::object::registry::OBJECT_REGISTRY
        .with_object(owner_id, |owner_ref| {
            crate::object::registry::OBJECT_REGISTRY.with_object(last_attacker, |attacker_guard| {
                if attacker_guard.is_effectively_dead() {
                    return false;
                }
                if owner_ref.relationship_to(attacker_guard) != Relationship::Enemies {
                    return false;
                }
                if !owner_ref.is_able_to_attack() {
                    return false;
                }
                matches!(
                    owner_ref.get_able_to_attack_specific_object(
                        AbleToAttackType::NewTarget,
                        attacker_guard,
                        CommandSourceType::FromAi,
                    ),
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                )
            })
        })
        .flatten()
        .unwrap_or(false)
}

