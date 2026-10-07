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
use crate::waypoint::WaypointId;

use std::sync::{Arc, Mutex, RwLock, Weak};

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
    // Update already holds the machine; resolve the exact owner retained by
    // the state, as C++ does through `getMachine()->getOwner()`.
    retaliate_state_owner_arc(state)
        .as_ref()
        .is_some_and(has_attacked_me_from_owner)
}

/// Owner of the state currently being tested for a conditional transition
/// (C++ `getMachine()->getOwner()`).
fn retaliate_state_owner_arc(state: &dyn StateImplementation) -> Option<Arc<RwLock<Object>>> {
    state.get_machine_owner().ok()
}

fn get_guard_enemy_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 30;
    };
    let data = ai_guard.get_ai_data();
    data.guard_enemy_scan_rate
}

fn get_guard_chase_unit_frames() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 0;
    };
    let data = ai_guard.get_ai_data();
    data.guard_chase_unit_frames
}

fn get_guard_enemy_return_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 60;
    };
    let data = ai_guard.get_ai_data();
    data.guard_enemy_return_scan_rate
}

fn issue_retaliate_return_move(ai: &mut dyn crate::modules::AIUpdateInterface, position: &Coord3D) {
    let mut params = crate::ai::AiCommandParams::new(
        crate::ai::AiCommandType::MoveToPosition,
        CommandSourceType::FromAi,
    );
    params.pos = *position;
    let _ = ai.execute_command(&params);
}

fn scan_guard_retaliate_inner_target(
    owner_arc: &Arc<RwLock<Object>>,
    pos: &Coord3D,
) -> Option<ObjectID> {
    let Ok(owner_guard) = owner_arc.read() else {
        return None;
    };

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
    #[cfg(test)]
    test_exit_acquisitions: usize,
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
            #[cfg(test)]
            test_exit_acquisitions: 0,
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
                if let Ok(owner_ref) = owner.try_read() {
                    let my_pos = owner_ref.get_position();
                    let my_range =
                        Coord3D::new(my_pos.x - self.center.x, my_pos.y - self.center.y, 0.0);
                    // Do not lock `owner` again; try_read is still held.
                    let guard_range =
                        AIGuardRetaliateMachine::get_std_guard_range(owner_ref.get_id());
                    if Vector3Ext::length_sqr(&my_range) > guard_range * guard_range {
                        return true;
                    }
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

// C++ owns these conditions in the parent state. The child only borrows
// them during its synchronous update; no retained shared handle is needed.
impl AttackExitConditionsInterface for GuardRetaliateExitConditions {
    fn should_exit(&self, machine: &StateMachine) -> bool {
        GuardRetaliateExitConditions::should_exit(self, machine)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guard_retaliate_machine_fields_are_isolated_per_machine() {
        let owner_a = Arc::new(RwLock::new(Object::new_test(91, 100.0)));
        let owner_b = Arc::new(RwLock::new(Object::new_test(92, 100.0)));

        let mut machine_a = AIGuardRetaliateMachine::new(Arc::downgrade(&owner_a));
        let mut machine_b = AIGuardRetaliateMachine::new(Arc::downgrade(&owner_b));

        machine_a.set_nemesis_id(41);
        machine_a.set_target_position_to_guard(&Coord3D::new(1.0, 2.0, 3.0));
        machine_b.set_nemesis_id(82);

        assert_eq!(machine_a.get_nemesis_id(), 41);
        assert_eq!(machine_b.get_nemesis_id(), 82);
        assert_eq!(
            machine_a.get_position_to_guard(),
            &Coord3D::new(1.0, 2.0, 3.0)
        );
        assert_eq!(
            machine_b.get_position_to_guard(),
            &Coord3D::new(0.0, 0.0, 0.0)
        );
    }

    #[test]
    fn guard_retaliate_machine_settles_in_idle_and_stays_there() {
        // A missing owner can stay idle only until its randomized scan delay
        // expires. Give this control a positive delay independent of earlier
        // tests' draws; the scoped stream restores on unwind.
        let mut random = game_engine::common::random_value::RandomState::default();
        random.set_seed_words([1, 2, 3, 4, 5, 6]);
        game_engine::common::random_value::with_logic_rng_owner(&mut random, || {
            let mut machine = AIGuardRetaliateMachine::new(Weak::new());
            machine.set_state(GuardRetaliateStateType::Idle);
            assert_eq!(
                machine.state_machine.get_current_state_id(),
                Some(GuardRetaliateStateType::Idle as u32)
            );
            machine.update();
            assert_eq!(
                machine.state_machine.get_current_state_id(),
                Some(GuardRetaliateStateType::Idle as u32)
            );
        });
    }
}

/// Main guard retaliate state machine - similar to guard but focuses on retaliation
///
/// C++ `AIGuardRetaliateMachine` owns its `StateMachine` and the guard
/// configuration as plain members. The machine here is a plain owned field,
/// states retain only a weak owner (not a machine handle), and the machine itself is loaned to each state
/// hook via `update_with_owner` / `on_enter_with_owner` — the same shape as
/// the turret conversion. No Arc, no Mutex, no Weak for the machine.
#[derive(Debug)]
pub struct AIGuardRetaliateMachine {
    /// The retaliate guard's own state machine. Owned, not shared.
    state_machine: StateMachine,
    /// State callback data is separate from the state machine so both can be
    /// borrowed at once during synchronous enter/update/exit callbacks.
    context: GuardRetaliateContext,
}

#[derive(Debug)]
struct GuardRetaliateContext {
    /// Exact weak owner retained from construction; no registry rebinding.
    owner: Weak<RwLock<Object>>,
    /// Position to guard
    position_to_guard: Coord3D,
    /// Nemesis to attack
    nemesis_to_attack: ObjectID,
    pending_goal_object: Option<ObjectID>,
}

impl AIGuardRetaliateMachine {
    pub fn new(owner: Weak<RwLock<Object>>) -> Self {
        let mut state_machine = StateMachine::new(Some(owner.clone()), "AIGuardRetaliateMachine");
        let context = GuardRetaliateContext {
            owner: owner.clone(),
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
            pending_goal_object: None,
        };
        Self::define_guard_retaliate_states(&mut state_machine);
        Self {
            state_machine,
            context,
        }
    }

    /// C++ state definitions. Order matters: the first defined state
    /// (ATTACK_AGGRESSOR) becomes the default.
    fn define_guard_retaliate_states(machine: &mut StateMachine) {
        let aggressor_id = GuardRetaliateStateType::AttackAggressor as u32;
        let return_id = GuardRetaliateStateType::Return as u32;
        let idle_id = GuardRetaliateStateType::Idle as u32;
        let inner_id = GuardRetaliateStateType::Inner as u32;
        let outer_id = GuardRetaliateStateType::Outer as u32;
        let crate_id = GuardRetaliateStateType::GetCrate as u32;

        let attack_aggressors = [StateConditionInfo::new(
            retaliate_attack_aggressor_condition,
            aggressor_id,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];

        machine.define_state(
            aggressor_id,
            Box::new(AIGuardRetaliateAttackAggressorState::new(machine)),
            Some(return_id),
            Some(return_id),
            None,
        );

        machine.define_state(
            return_id,
            Box::new(AIGuardRetaliateReturnState::new(machine)),
            Some(idle_id),
            Some(inner_id),
            Some(&attack_aggressors),
        );

        machine.define_state(
            idle_id,
            Box::new(AIGuardRetaliateIdleState::new(machine)),
            Some(inner_id),
            Some(EXIT_MACHINE_WITH_SUCCESS),
            Some(&attack_aggressors),
        );

        machine.define_state(
            inner_id,
            Box::new(AIGuardRetaliateInnerState::new(machine)),
            Some(outer_id),
            Some(outer_id),
            None,
        );

        machine.define_state(
            outer_id,
            Box::new(AIGuardRetaliateOuterState::new(machine)),
            Some(crate_id),
            Some(crate_id),
            None,
        );

        machine.define_state(
            crate_id,
            Box::new(AIGuardRetaliatePickUpCrateState::new(machine)),
            Some(return_id),
            Some(return_id),
            None,
        );
    }

    // ---- loan-safe accessors (never touch `state_machine`) -------------------
}

impl GuardRetaliateContext {
    fn friend_owner_arc(&self) -> Option<Arc<RwLock<Object>>> {
        self.owner.upgrade()
    }
    fn friend_position_to_guard(&self) -> Coord3D {
        self.position_to_guard
    }
    fn friend_nemesis_to_attack(&self) -> ObjectID {
        self.nemesis_to_attack
    }
    fn friend_set_nemesis_to_attack(&mut self, id: ObjectID) {
        self.nemesis_to_attack = id;
    }
    fn friend_set_goal_object(&mut self, id: ObjectID) {
        self.pending_goal_object = Some(id);
    }
}

impl AIGuardRetaliateMachine {
    pub fn is_idle(&self) -> bool {
        // C++ AIGuardRetaliate.cpp:181 compares to AI_IDLE (0), not retaliate idle.
        self.state_machine.get_current_state_id() == Some(0)
    }
    pub fn get_position_to_guard(&self) -> &Coord3D {
        &self.context.position_to_guard
    }
    pub fn set_target_position_to_guard(&mut self, pos: &Coord3D) {
        self.context.position_to_guard = *pos;
    }
    pub fn set_nemesis_id(&mut self, id: ObjectID) {
        self.context.nemesis_to_attack = id;
    }
    pub fn get_nemesis_id(&self) -> ObjectID {
        self.context.nemesis_to_attack
    }
    fn owner_ai_handle(&self) -> Option<Arc<Mutex<dyn crate::modules::AIUpdateInterface>>> {
        let owner = self.context.owner.upgrade()?;
        let owner = owner.read().ok()?;
        owner.get_ai_update_interface()
    }
    pub fn init_default_state(&mut self) -> StateReturnType {
        if let Some(ai) = self.owner_ai_handle() {
            if let Ok(mut ai) = ai.lock() {
                let result = self.init_default_state_with_ai(&mut *ai);
                self.apply_pending_goal_object();
                return result;
            }
        }
        let result = self
            .state_machine
            .init_default_state_with_owner(&mut self.context);
        self.apply_pending_goal_object();
        result
    }
    pub(crate) fn init_default_state_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let result = self
            .state_machine
            .init_default_state_with_ai_and_owner(ai, &mut self.context);
        self.apply_pending_goal_object();
        result
    }
    pub fn set_state(&mut self, state: GuardRetaliateStateType) -> StateReturnType {
        if let Some(ai) = self.owner_ai_handle() {
            if let Ok(mut ai) = ai.lock() {
                let result = self.set_state_with_ai(state, &mut *ai);
                self.apply_pending_goal_object();
                return result;
            }
        }
        let result = self
            .state_machine
            .set_current_state_with_owner(state as u32, &mut self.context);
        self.apply_pending_goal_object();
        result
    }
    pub(crate) fn set_state_with_ai(
        &mut self,
        state: GuardRetaliateStateType,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let result = self.state_machine.set_current_state_with_ai_and_owner(
            state as u32,
            ai,
            &mut self.context,
        );
        self.apply_pending_goal_object();
        result
    }
    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.state_machine.halt()
    }
    pub fn is_in_attack_state(&self) -> bool {
        self.state_machine.is_in_attack_state()
    }
    pub fn update(&mut self) -> StateReturnType {
        if let Some(ai) = self.owner_ai_handle() {
            if let Ok(mut ai) = ai.lock() {
                return self.update_with_ai(&mut *ai);
            }
        }
        let result = self.state_machine.update_with_owner(&mut self.context);
        self.apply_pending_goal_object();
        result
    }
    pub(crate) fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        let result = self
            .state_machine
            .update_with_ai_and_owner(ai, &mut self.context);
        self.apply_pending_goal_object();
        result
    }
    fn apply_pending_goal_object(&mut self) {
        if let Some(id) = self.context.pending_goal_object.take() {
            self.state_machine.set_goal_object_by_id(Some(id));
        }
    }
    pub fn look_for_inner_target(&mut self) -> bool {
        let Some(owner_arc) = self.context.friend_owner_arc() else {
            return false;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return false;
        };
        if !owner_guard.is_able_to_attack() {
            return false;
        }
        if let Some(team_arc) = owner_guard.get_team() {
            if let Ok(team_guard) = team_arc.read() {
                if team_guard.attack_common_target() {
                    let team_target = team_guard.get_team_target_object();
                    if team_target != crate::common::INVALID_ID {
                        self.context.friend_set_nemesis_to_attack(team_target);
                        return true;
                    }
                }
            }
        }
        let pos = self.context.position_to_guard;
        if let Some(target_id) = scan_guard_retaliate_inner_target(&owner_arc, &pos) {
            self.context.friend_set_nemesis_to_attack(target_id);
            return true;
        }
        false
    }
    pub fn get_std_guard_range(obj_id: ObjectID) -> f32 {
        let ai_store = the_ai();
        ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_adjusted_vision_range_for_object(
                    obj_id,
                    vision_factors::OWNER_TYPE | vision_factors::MOOD | vision_factors::GUARD_INNER,
                )
                .ok()
            })
            .unwrap_or(100.0)
    }
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        if version >= 2 {
            self.state_machine.xfer(xfer).map_err(|e| e.to_string())?;
        }
        xfer.xfer_object_id(&mut self.context.nemesis_to_attack)
            .map_err(|e| format!("Failed to xfer nemesis_to_attack: {:?}", e))?;
        xfer.xfer_real(&mut self.context.position_to_guard.x)
            .map_err(|e| format!("Failed to xfer position_to_guard.x: {:?}", e))?;
        xfer.xfer_real(&mut self.context.position_to_guard.y)
            .map_err(|e| format!("Failed to xfer position_to_guard.y: {:?}", e))?;
        xfer.xfer_real(&mut self.context.position_to_guard.z)
            .map_err(|e| format!("Failed to xfer position_to_guard.z: {:?}", e))?;
        Ok(())
    }
    pub fn load_post_process(&mut self) -> Result<(), String> {
        let result = self
            .state_machine
            .load_post_process_with_owner(&mut self.context)
            .map_err(|e| format!("guard retaliate load_post_process: {e}"));
        self.apply_pending_goal_object();
        result
    }
}

// State implementations for guard retaliate
//
// The states retain the exact weak owner through legacy `State` bookkeeping
// and receive the loaned machine through `update_with_owner` / `on_enter_with_owner`.

#[derive(Debug)]
struct GuardRetaliateState {
    base: State,
}

impl GuardRetaliateState {
    fn new(machine: &StateMachine, name: &str) -> Self {
        // `State::new` captures the machine's weak owner but no machine handle:
        // the machine owns its states, never the other way round.
        Self {
            base: State::new(machine, name),
        }
    }

    fn state(&self) -> &State {
        &self.base
    }

    fn state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn owner_arc(&self) -> Option<Arc<RwLock<Object>>> {
        self.base.get_machine_owner()
    }

    fn downcast_machine(owner: &mut dyn std::any::Any) -> Option<&mut GuardRetaliateContext> {
        owner.downcast_mut::<GuardRetaliateContext>()
    }
}

/// Inner guard retaliate state - attack anything within area with focus on retaliation
#[derive(Debug)]
pub struct AIGuardRetaliateInnerState {
    base: GuardRetaliateState,
    exit_conditions: GuardRetaliateExitConditions,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
    enter_state: Option<AIEnterState>,
}

impl AIGuardRetaliateInnerState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, "AIGuardRetaliateInner"),
            exit_conditions: GuardRetaliateExitConditions::new(),
            is_attacking: false,
            attack_machine: None,
            enter_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }

    #[cfg(test)]
    fn classic_on_enter(&mut self, machine: &mut AIGuardRetaliateMachine) -> StateReturnType {
        self.classic_on_enter_with_ai(&mut machine.context, None)
    }

    #[cfg(test)]
    fn classic_update(&mut self) -> StateReturnType {
        self.classic_update_with_ai(None)
    }

    fn classic_on_enter_with_ai(
        &mut self,
        machine: &mut GuardRetaliateContext,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };

        let nemesis_id = machine.friend_nemesis_to_attack();
        let Some(nemesis) = (nemesis_id != crate::common::INVALID_ID)
            .then(|| get_legacy_object(nemesis_id))
            .flatten()
        else {
            self.is_attacking = false;
            self.attack_machine = None;
            self.enter_state = None;
            return StateReturnType::Success;
        };

        let is_enter_guard = owner
            .read()
            .map(|guard| guard.get_template().is_enter_guard())
            .unwrap_or(false);
        if is_enter_guard {
            // State::new copies the machine's owner id; no machine handle is
            // attached, so there is nothing to lock here either.
            let scratch = StateMachine::new(Some(Arc::downgrade(&owner)), "AIEnter");
            let mut enter_state = AIEnterState::new(&scratch);
            enter_state.preset_owner = Some(owner.clone());
            enter_state.preset_goal_id = nemesis
                .read()
                .ok()
                .map(|goal| goal.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            self.is_attacking = false;
            self.attack_machine = None;
            self.enter_state = Some(enter_state);
            if let Some(enter_state) = self.enter_state.as_mut() {
                let result = match ai {
                    Some(ai) => {
                        enter_state.on_enter_with_ai(ai, nemesis_id, Coord3D::new(0.0, 0.0, 0.0))
                    }
                    None => enter_state.on_enter(),
                };
                if result == StateReturnType::Continue {
                    return StateReturnType::Continue;
                }
            }
            return StateReturnType::Success;
        }

        let pos = machine.friend_position_to_guard();
        {
            let conditions = &mut self.exit_conditions;
            let radius = 1.5
                * owner
                    .read()
                    .ok()
                    .map(|g| AIGuardRetaliateMachine::get_std_guard_range(g.get_id()))
                    .unwrap_or(100.0);
            conditions.set_center(pos);
            conditions.set_radius_sqr(radius * radius);
            conditions.set_conditions(
                guard_retaliate_exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIGuardRetaliateAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_goal_object(nemesis.read().ok().map(|g| g.get_id()));

        let result = match ai {
            Some(ai) => attack_machine.init_default_state_with_ai(ai),
            None => attack_machine.init_default_state(),
        };
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);
        self.enter_state = None;

        if result == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn classic_update_with_ai(
        &mut self,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        if let Some(attack_machine) = self.attack_machine.as_mut() {
            return match ai {
                Some(ai) => {
                    attack_machine.update_with_exit_conditions_and_ai(&self.exit_conditions, ai)
                }
                None => attack_machine.update_with_exit_conditions(&self.exit_conditions),
            };
        }
        if let Some(enter_state) = self.enter_state.as_mut() {
            return match ai {
                Some(ai) => enter_state.update_with_ai(ai),
                None => enter_state.update(),
            };
        }
        StateReturnType::Success
    }

    #[cfg(test)]
    fn classic_on_exit(&mut self, status: StateExitType) {
        self.classic_on_exit_with_ai(status, None);
    }

    fn classic_on_exit_with_ai(
        &mut self,
        _status: StateExitType,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        if let Some(mut enter_state) = self.enter_state.take() {
            match ai {
                Some(ai) => enter_state.on_exit_with_ai(_status, ai),
                None => enter_state.on_exit(_status),
            }
        }
        self.is_attacking = false;

        if let Some(owner) = self.base.owner_arc() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(crate::common::INVALID_ID);
                    }
                }
            }
        }
    }
}

impl StateImplementation for AIGuardRetaliateInnerState {
    /// Retaliate states are only stepped through their owner's machine, which
    /// always loans the machine via `update_with_owner`.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter_with_ai(machine, None),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(_) => self.classic_update_with_ai(None),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.classic_on_exit_with_ai(_status, None);
    }

    fn on_enter_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: ObjectID,
        _goal_pos: Coord3D,
        _waypoint: Option<WaypointId>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |machine| {
            self.classic_on_enter_with_ai(machine, Some(ai))
        })
    }

    fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _machine_locked: bool,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |_| {
            self.classic_update_with_ai(Some(ai))
        })
    }

    fn on_exit_with_ai_and_owner(
        &mut self,
        status: StateExitType,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _owner: &mut dyn std::any::Any,
    ) {
        self.classic_on_exit_with_ai(status, Some(ai));
    }

    /// C++ `loadPostProcess` re-runs `onEnter` to rebuild the attack child.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        if let Some(machine) = GuardRetaliateState::downcast_machine(owner) {
            let _ = self.classic_on_enter_with_ai(machine, None);
        }
        Ok(())
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        self.base.base.bind_owner(&owner);
    }

    fn get_machine_owner(&self) -> Result<std::sync::Arc<std::sync::RwLock<Object>>, String> {
        self.base
            .base
            .get_machine_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_attack(&self) -> bool {
        self.is_attack()
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Idle guard retaliate state - wait for targets with retaliation focus
#[derive(Debug)]
pub struct AIGuardRetaliateIdleState {
    base: GuardRetaliateState,
    next_enemy_scan_time: u32,
    requested_state: Option<u32>,
}

impl AIGuardRetaliateIdleState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, "AIGuardRetaliateIdleState"),
            next_enemy_scan_time: 0,
            requested_state: None,
        }
    }

    pub fn is_guard_idle(&self) -> bool {
        true
    }

    fn classic_on_enter(&mut self, _machine: &mut GuardRetaliateContext) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_scan_rate();
        self.next_enemy_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));
        StateReturnType::Continue
    }

    fn classic_update_with_ai(
        &mut self,
        machine: &mut GuardRetaliateContext,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if now < self.next_enemy_scan_time {
            return StateReturnType::Sleep(self.next_enemy_scan_time - now);
        }

        self.next_enemy_scan_time = now.saturating_add(get_guard_enemy_scan_rate());

        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        let has_crate = match ai {
            Some(ai) => ai.get_crate_id() != crate::common::INVALID_ID,
            None => owner_guard.get_ai_update_interface().is_some_and(|ai| {
                ai.lock()
                    .is_ok_and(|a| a.get_crate_id() != crate::common::INVALID_ID)
            }),
        };
        if has_crate {
            self.requested_state = Some(GuardRetaliateStateType::GetCrate as u32);
            return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
        }

        if let Some(team_arc) = owner_guard.get_team() {
            if let Ok(team_guard) = team_arc.read() {
                if team_guard.attack_common_target() {
                    let team_target = team_guard.get_team_target_object();
                    if team_target != crate::common::INVALID_ID {
                        machine.friend_set_nemesis_to_attack(team_target);
                        return StateReturnType::Success;
                    }
                }
            }
        }

        let guard_pos = machine.friend_position_to_guard();
        drop(owner_guard);

        if let Some(target_id) = scan_guard_retaliate_inner_target(&owner, &guard_pos) {
            machine.friend_set_nemesis_to_attack(target_id);
            return StateReturnType::Success;
        }

        StateReturnType::Failure
    }
}

impl StateImplementation for AIGuardRetaliateIdleState {
    fn take_requested_state_change(&mut self) -> Option<u32> {
        self.requested_state.take()
    }

    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine),
            None => StateReturnType::Failure,
        }
    }

    fn on_enter_with_ai_and_owner(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: ObjectID,
        _goal_pos: Coord3D,
        _waypoint: Option<WaypointId>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |machine| {
            self.classic_on_enter(machine)
        })
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(machine) => self.classic_update_with_ai(machine, None),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // Cleanup when exiting idle guard retaliate state
    }

    fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _machine_locked: bool,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |machine| {
            self.classic_update_with_ai(machine, Some(ai))
        })
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        self.base.base.bind_owner(&owner);
    }

    fn get_machine_owner(&self) -> Result<std::sync::Arc<std::sync::RwLock<Object>>, String> {
        self.base
            .base
            .get_machine_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_guard_idle(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Outer guard retaliate state - attack aggressive targets with timer and retaliation priority
#[derive(Debug)]
pub struct AIGuardRetaliateOuterState {
    base: GuardRetaliateState,
    exit_conditions: GuardRetaliateExitConditions,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AIGuardRetaliateOuterState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, "AIGuardRetaliateOuter"),
            exit_conditions: GuardRetaliateExitConditions::new(),
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }

    #[cfg(test)]
    fn classic_on_enter(&mut self, machine: &mut AIGuardRetaliateMachine) -> StateReturnType {
        self.classic_on_enter_with_ai(&mut machine.context, None)
    }

    #[cfg(test)]
    fn classic_update(&mut self) -> StateReturnType {
        self.classic_update_with_ai(None)
    }

    fn classic_on_enter_with_ai(
        &mut self,
        machine: &mut GuardRetaliateContext,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };

        let mut nemesis_id = machine.friend_nemesis_to_attack();
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

        let pos = machine.friend_position_to_guard();
        let std_guard_range = owner
            .read()
            .ok()
            .map(|g| AIGuardRetaliateMachine::get_std_guard_range(g.get_id()))
            .unwrap_or(100.0);
        let range = {
            let Ok(owner_guard) = owner.read() else {
                return StateReturnType::Failure;
            };
            let owner_id = owner_guard.get_id();
            drop(owner_guard);

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

        {
            let conditions = &mut self.exit_conditions;
            let radius = 0.67 * (range + std_guard_range);
            conditions.set_center(pos);
            conditions.set_radius_sqr(radius * radius);
            conditions.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            conditions.set_conditions(
                guard_retaliate_exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIGuardRetaliateAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_goal_object(nemesis.read().ok().map(|g| g.get_id()));
        let result = match ai {
            Some(ai) => attack_machine.init_default_state_with_ai(ai),
            None => attack_machine.init_default_state(),
        };

        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if result == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn classic_update_with_ai(
        &mut self,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
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
                {
                    let conditions = &mut self.exit_conditions;
                    let delta = Coord3D::new(
                        conditions.center.x - goal_pos.x,
                        conditions.center.y - goal_pos.y,
                        conditions.center.z - goal_pos.z,
                    );
                    if let Some(vision) = self.base.owner_arc().and_then(|owner| {
                        owner
                            .read()
                            .ok()
                            .map(|g| AIGuardRetaliateMachine::get_std_guard_range(g.get_id()))
                    }) {
                        if Vector3Ext::length_sqr(&delta) <= vision * vision {
                            conditions.set_attack_give_up_frame(
                                TheGameLogic::get_frame()
                                    .saturating_add(get_guard_chase_unit_frames()),
                            );
                        }
                    }
                }
            }
        }

        match ai {
            Some(ai) => {
                attack_machine.update_with_exit_conditions_and_ai(&self.exit_conditions, ai)
            }
            None => attack_machine.update_with_exit_conditions(&self.exit_conditions),
        }
    }

    fn classic_on_exit(&mut self, _status: StateExitType) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        self.is_attacking = false;
    }
}

impl StateImplementation for AIGuardRetaliateOuterState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter_with_ai(machine, None),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(_) => self.classic_update_with_ai(None),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.classic_on_exit(_status);
    }

    fn on_enter_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: ObjectID,
        _goal_pos: Coord3D,
        _waypoint: Option<WaypointId>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |machine| {
            self.classic_on_enter_with_ai(machine, Some(ai))
        })
    }

    fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _machine_locked: bool,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |_| {
            self.classic_update_with_ai(Some(ai))
        })
    }

    /// C++ `loadPostProcess` re-runs `onEnter`.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        if let Some(machine) = GuardRetaliateState::downcast_machine(owner) {
            let _ = self.classic_on_enter_with_ai(machine, None);
        }
        Ok(())
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        self.base.base.bind_owner(&owner);
    }

    fn get_machine_owner(&self) -> Result<std::sync::Arc<std::sync::RwLock<Object>>, String> {
        self.base
            .base
            .get_machine_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_attack(&self) -> bool {
        self.is_attack()
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Return guard retaliate state - move back to guard position
#[derive(Debug)]
pub struct AIGuardRetaliateReturnState {
    base: GuardRetaliateState,
    next_return_scan_time: u32,
    goal_position: Coord3D,
}

impl AIGuardRetaliateReturnState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, "AIGuardRetaliateReturn"),
            next_return_scan_time: 0,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    fn classic_on_enter_with_ai(
        &mut self,
        machine: &mut GuardRetaliateContext,
        mut ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_return_scan_rate();
        self.next_return_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        self.goal_position = machine.friend_position_to_guard();
        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };
        if let Some(ai) = ai.as_deref_mut() {
            if ai.is_doing_ground_movement() {
                let _ = ai.adjust_destination(&mut self.goal_position);
            }
            issue_retaliate_return_move(ai, &self.goal_position);
        } else if let Ok(owner_guard) = owner.try_read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    if ai_guard.is_doing_ground_movement() {
                        let _ = ai_guard.adjust_destination(&mut self.goal_position);
                    }
                    ai.ai_move_to_position(&self.goal_position, false, CommandSourceType::FromAi);
                }
            }
        }
        StateReturnType::Continue
    }

    fn classic_update(&mut self, machine: &mut GuardRetaliateContext) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if now >= self.next_return_scan_time {
            self.next_return_scan_time = now.saturating_add(get_guard_enemy_return_scan_rate());

            let Some(owner) = machine.friend_owner_arc() else {
                return StateReturnType::Failure;
            };
            if let Some(target_id) = scan_guard_retaliate_inner_target(&owner, &self.goal_position)
            {
                machine.friend_set_nemesis_to_attack(target_id);
                return StateReturnType::Failure;
            }
        }

        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        let owner_pos = owner_guard.get_position();
        let dx = owner_pos.x - self.goal_position.x;
        let dy = owner_pos.y - self.goal_position.y;
        if dx * dx + dy * dy <= CLOSE_ENOUGH * CLOSE_ENOUGH {
            return StateReturnType::Success;
        }

        StateReturnType::Continue
    }
}

impl StateImplementation for AIGuardRetaliateReturnState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter_with_ai(machine, None),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(machine) => self.classic_update(machine),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // Nothing to clean up.
    }

    fn on_enter_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: ObjectID,
        _goal_pos: Coord3D,
        _waypoint: Option<WaypointId>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |machine| {
            self.classic_on_enter_with_ai(machine, Some(ai))
        })
    }

    fn update_with_ai_and_owner(
        &mut self,
        _ai: &mut dyn crate::modules::AIUpdateInterface,
        _machine_locked: bool,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |machine| {
            self.classic_update(machine)
        })
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        self.base.base.bind_owner(&owner);
    }

    fn get_machine_owner(&self) -> Result<std::sync::Arc<std::sync::RwLock<Object>>, String> {
        self.base
            .base
            .get_machine_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Pick up crate state for guard retaliate
#[derive(Debug)]
pub struct AIGuardRetaliatePickUpCrateState {
    base: GuardRetaliateState,
    pickup: Option<AIPickUpCrateState>,
}

impl AIGuardRetaliatePickUpCrateState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, "AIGuardRetaliatePickUpCrate"),
            pickup: None,
        }
    }

    fn classic_on_enter_with_ai(
        &mut self,
        machine: &mut GuardRetaliateContext,
        _ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        let crate_id = owner_guard.ai_fire_crate_id;
        if crate_id == crate::common::INVALID_ID {
            return StateReturnType::Success;
        }
        machine.friend_set_goal_object(crate_id);
        let crate_pos = TheGameLogic::find_object_by_id(crate_id)
            .and_then(|crate_obj| crate_obj.read().ok().map(|goal| *goal.get_position()));
        drop(owner_guard);

        let scratch = StateMachine::new(Some(Arc::downgrade(&owner)), "AIPickUpCrate");
        let mut pickup = AIPickUpCrateState::new(&scratch);
        pickup.preset_goal_id = crate_id;
        pickup.base.preset_owner = Some(owner.clone());
        if let Some(pos) = crate_pos {
            pickup.goal_position = pos;
            pickup.base.goal_position = pos;
        }
        let result = match _ai {
            Some(ai) => pickup.on_enter_with_ai(
                ai,
                crate_id,
                crate_pos.unwrap_or(Coord3D::new(0.0, 0.0, 0.0)),
            ),
            None => pickup.on_enter(),
        };
        self.pickup = Some(pickup);
        result
    }

    fn classic_update_with_ai(
        &mut self,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(pickup) = self.pickup.as_mut() else {
            return StateReturnType::Success;
        };
        match ai {
            Some(ai) => pickup.update_with_ai(ai),
            None => pickup.update(),
        }
    }

    fn classic_on_exit(&mut self, _status: StateExitType) {
        // C++ AIGuardRetaliatePickUpCrateState::onExit is empty.
        self.pickup = None;
    }
}

impl StateImplementation for AIGuardRetaliatePickUpCrateState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter_with_ai(machine, None),
            None => StateReturnType::Failure,
        }
    }

    fn on_enter_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: ObjectID,
        _goal_pos: Coord3D,
        _waypoint: Option<WaypointId>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |machine| {
            self.classic_on_enter_with_ai(machine, Some(ai))
        })
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(_) => self.classic_update_with_ai(None),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.classic_on_exit(_status);
    }

    fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _machine_locked: bool,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |_| {
            self.classic_update_with_ai(Some(ai))
        })
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        self.base.base.bind_owner(&owner);
    }

    fn get_machine_owner(&self) -> Result<std::sync::Arc<std::sync::RwLock<Object>>, String> {
        self.base
            .base
            .get_machine_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Attack aggressor state for guard retaliate - enhanced retaliation behavior
#[derive(Debug)]
pub struct AIGuardRetaliateAttackAggressorState {
    base: GuardRetaliateState,
    exit_conditions: GuardRetaliateExitConditions,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AIGuardRetaliateAttackAggressorState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, "AIGuardRetaliateAttackAggressor"),
            exit_conditions: GuardRetaliateExitConditions::new(),
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }

    #[cfg(test)]
    fn classic_on_enter(&mut self, machine: &mut AIGuardRetaliateMachine) -> StateReturnType {
        self.classic_on_enter_with_ai(&mut machine.context, None)
    }

    #[cfg(test)]
    fn classic_update(&mut self) -> StateReturnType {
        self.classic_update_with_ai(None)
    }

    fn classic_on_enter_with_ai(
        &mut self,
        machine: &mut GuardRetaliateContext,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };

        // C++ AIGuardRetaliateAttackAggressorState::onEnter (745-760):
        // prefer machine nemesis, then last-damage source only if not DAMAGE_HEALING.
        let mut nemesis_id = machine.friend_nemesis_to_attack();
        if nemesis_id == crate::common::INVALID_ID {
            if let Ok(owner_guard) = owner.try_read() {
                if let Some(body) = owner_guard.get_body_module() {
                    if let Ok(body_guard) = body.lock() {
                        if let Some(info) = body_guard.get_last_damage_info() {
                            if info.source_id != crate::common::INVALID_ID
                                && info.input.damage_type != crate::damage::DamageType::Healing
                            {
                                if let Some(target) = get_legacy_object(info.source_id) {
                                    if let Ok(target_guard) = target.read() {
                                        if owner_guard.relationship_to(&target_guard)
                                            == Relationship::Enemies
                                        {
                                            machine.friend_set_nemesis_to_attack(info.source_id);
                                        }
                                    }
                                }
                                nemesis_id = info.source_id;
                            }
                        }
                    }
                }
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

        let pos = machine.friend_position_to_guard();
        let std_guard_range = owner
            .read()
            .ok()
            .map(|g| AIGuardRetaliateMachine::get_std_guard_range(g.get_id()))
            .unwrap_or(100.0);
        let range = {
            let Ok(owner_guard) = owner.read() else {
                return StateReturnType::Failure;
            };
            let owner_id = owner_guard.get_id();
            drop(owner_guard);

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

        {
            let conditions = &mut self.exit_conditions;
            conditions.set_center(pos);
            conditions.set_radius_sqr((range + std_guard_range) * (range + std_guard_range));
            conditions.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            conditions.set_conditions(
                guard_retaliate_exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | guard_retaliate_exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIGuardRetaliateAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_goal_object(nemesis.read().ok().map(|g| g.get_id()));
        let result = match ai {
            Some(ai) => attack_machine.init_default_state_with_ai(ai),
            None => attack_machine.init_default_state(),
        };

        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if result == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn classic_update_with_ai(
        &mut self,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> StateReturnType {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };
        match ai {
            Some(ai) => {
                attack_machine.update_with_exit_conditions_and_ai(&self.exit_conditions, ai)
            }
            None => attack_machine.update_with_exit_conditions(&self.exit_conditions),
        }
    }

    fn classic_on_exit(&mut self, _status: StateExitType) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        self.is_attacking = false;

        if let Some(owner) = self.base.owner_arc() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(crate::common::INVALID_ID);
                    }
                }
            }
        }
    }
}

impl StateImplementation for AIGuardRetaliateAttackAggressorState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter_with_ai(machine, None),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match GuardRetaliateState::downcast_machine(owner) {
            Some(_) => self.classic_update_with_ai(None),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.classic_on_exit(_status);
    }

    fn on_enter_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _goal_id: ObjectID,
        _goal_pos: Coord3D,
        _waypoint: Option<WaypointId>,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |machine| {
            self.classic_on_enter_with_ai(machine, Some(ai))
        })
    }

    fn update_with_ai_and_owner(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
        _machine_locked: bool,
        owner: &mut dyn std::any::Any,
    ) -> StateReturnType {
        GuardRetaliateState::downcast_machine(owner).map_or(StateReturnType::Failure, |_| {
            self.classic_update_with_ai(Some(ai))
        })
    }

    /// C++ `loadPostProcess` re-runs `onEnter`.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        if let Some(machine) = GuardRetaliateState::downcast_machine(owner) {
            let _ = self.classic_on_enter_with_ai(machine, None);
        }
        Ok(())
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        self.base.base.bind_owner(&owner);
    }

    fn get_machine_owner(&self) -> Result<std::sync::Arc<std::sync::RwLock<Object>>, String> {
        self.base
            .base
            .get_machine_owner()
            .ok_or_else(|| "state machine owner not attached".to_string())
    }

    fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.base.base.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not attached".to_string())
        } else {
            Ok(self.base.base.owner_id)
        }
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn is_attack(&self) -> bool {
        self.is_attack()
    }

    fn is_busy(&self) -> bool {
        true
    }

    fn get_name(&self) -> &str {
        self.base.state().get_name()
    }

    fn get_id(&self) -> StateId {
        self.base.state().get_id()
    }

    fn set_id(&mut self, id: StateId) {
        self.base.state_mut().set_id(id);
    }
}

/// Helper function to check if an object has attacked and can be retaliated against
pub fn has_attacked_me_and_i_can_return_fire_retaliate(machine: &StateMachine) -> bool {
    machine
        .get_owner()
        .as_ref()
        .is_some_and(has_attacked_me_from_owner)
}

#[cfg(test)]
#[path = "guard_retaliate_owner_tests.rs"]
mod owner_tests;

fn has_attacked_me_from_owner(owner: &Arc<RwLock<Object>>) -> bool {
    if let Ok(owner_ref) = owner.try_read() {
        if let Some(body_module) = owner_ref.get_body_module() {
            if let Ok(mut body_guard) = body_module.lock() {
                let last_attacker = body_guard.get_clearable_last_attacker();
                if last_attacker == crate::common::INVALID_ID {
                    return false;
                }

                // Clear the attacker to prevent repeated checks
                body_guard.clear_last_attacker();

                let Some(attacker_arc) = get_legacy_object(last_attacker) else {
                    return false;
                };
                let Ok(attacker_guard) = attacker_arc.read() else {
                    return false;
                };
                if attacker_guard.is_effectively_dead() {
                    return false;
                }
                if owner_ref.relationship_to(&*attacker_guard) != Relationship::Enemies {
                    return false;
                }
                if !owner_ref.is_able_to_attack() {
                    return false;
                }
                let can_attack = owner_ref.get_able_to_attack_specific_object(
                    AbleToAttackType::NewTarget,
                    &*attacker_guard,
                    CommandSourceType::FromAi,
                );
                matches!(
                    can_attack,
                    CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                )
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    }
}

#[cfg(test)]
#[path = "guard_retaliate_condition_owner_tests.rs"]
mod condition_owner_tests;
