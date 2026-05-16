use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::states::{AIEnterState, AttackExitConditionsInterface, AttackStateMachine};
use crate::ai::vision_factors;
use crate::ai::{object_registry::get_legacy_object, THE_AI};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::coord::*;
use crate::common::vector_ext::Vector3Ext;
use crate::common::xfer::{Xfer, XferExt, XferVersion};
use crate::common::*;
use crate::helpers::{game_logic_random_value, TheGameLogic, ThePartitionManager};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::*;
use crate::state_machine::*;

use std::sync::{Arc, Mutex, RwLock, Weak};

/// Close enough distance constant
const CLOSE_ENOUGH: f32 = 25.0;
/// Crate pickup range squared (matches AIPickUpCrateState)
const CRATE_PICKUP_RANGE_SQR: f32 = 100.0;

fn get_guard_enemy_scan_rate() -> u32 {
    let Ok(ai_guard) = THE_AI.read() else {
        return 30;
    };
    let data = ai_guard.get_ai_data();
    let Ok(data_guard) = data.read() else {
        return 30;
    };
    data_guard.guard_enemy_scan_rate
}

fn get_guard_chase_unit_frames() -> u32 {
    let Ok(ai_guard) = THE_AI.read() else {
        return 0;
    };
    let data = ai_guard.get_ai_data();
    let Ok(data_guard) = data.read() else {
        return 0;
    };
    data_guard.guard_chase_unit_frames
}

fn get_guard_enemy_return_scan_rate() -> u32 {
    let Ok(ai_guard) = THE_AI.read() else {
        return 60;
    };
    let data = ai_guard.get_ai_data();
    let Ok(data_guard) = data.read() else {
        return 60;
    };
    data_guard.guard_enemy_return_scan_rate
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

    let vision_range = AIGuardRetaliateMachine::get_std_guard_range(owner_arc);
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
        let goal_object = machine.get_goal_object();

        if goal_object.is_none() {
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
            if let Some(goal_obj) = goal_object {
                if let Ok(goal_ref) = goal_obj.try_read() {
                    let obj_pos = goal_ref.get_position();

                    let delta_aggressor =
                        Coord3D::new(obj_pos.x - self.center.x, obj_pos.y - self.center.y, 0.0);

                    if Vector3Ext::length_sqr(&delta_aggressor) > self.radius_sqr {
                        return true;
                    }

                    if let Some(owner) = machine.get_owner() {
                        if let Ok(owner_ref) = owner.try_read() {
                            let my_pos = owner_ref.get_position();
                            let my_range = Coord3D::new(
                                my_pos.x - self.center.x,
                                my_pos.y - self.center.y,
                                0.0,
                            );
                            let guard_range = AIGuardRetaliateMachine::get_std_guard_range(&owner);
                            if Vector3Ext::length_sqr(&my_range) > guard_range * guard_range {
                                return true;
                            }
                        }
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

#[derive(Debug, Default)]
pub struct GuardRetaliateSharedState {
    machine: Weak<Mutex<StateMachine>>,
    position_to_guard: Mutex<Coord3D>,
    nemesis_to_attack: Mutex<ObjectID>,
}

impl GuardRetaliateSharedState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            machine: Arc::downgrade(machine),
            position_to_guard: Mutex::new(Coord3D::new(0.0, 0.0, 0.0)),
            nemesis_to_attack: Mutex::new(crate::common::INVALID_ID),
        }
    }

    fn with_machine<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut StateMachine) -> R,
    {
        let machine = self
            .machine
            .upgrade()
            .ok_or_else(|| "guard retaliate state machine context lost".to_string())?;
        let mut guard = machine
            .lock()
            .map_err(|_| "guard retaliate state machine lock poisoned".to_string())?;
        Ok(f(&mut guard))
    }

    fn change_state(&self, state: GuardRetaliateStateType) -> Result<(), String> {
        self.with_machine(|machine| {
            let _ = machine.set_current_state(state as u32);
        })
    }

    fn get_position_to_guard(&self) -> Coord3D {
        self.position_to_guard
            .lock()
            .map(|pos| *pos)
            .unwrap_or_else(|_| Coord3D::new(0.0, 0.0, 0.0))
    }

    fn set_position_to_guard(&self, pos: Coord3D) {
        if let Ok(mut guard_pos) = self.position_to_guard.lock() {
            *guard_pos = pos;
        }
    }

    fn get_nemesis_to_attack(&self) -> ObjectID {
        self.nemesis_to_attack
            .lock()
            .map(|id| *id)
            .unwrap_or(crate::common::INVALID_ID)
    }

    fn set_nemesis_to_attack(&self, id: ObjectID) {
        if let Ok(mut nemesis) = self.nemesis_to_attack.lock() {
            *nemesis = id;
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
    fn guard_retaliate_shared_state_tracks_state_changes() {
        let machine = Arc::new(Mutex::new(StateMachine::new(
            Some(Weak::new()),
            "test_retaliate",
        )));
        {
            let mut locked = machine.lock().unwrap();
            locked.define_state(
                GuardRetaliateStateType::Inner as u32,
                Box::new(DummyState),
                None,
                None,
                None,
            );
            locked.define_state(
                GuardRetaliateStateType::Idle as u32,
                Box::new(DummyState),
                None,
                None,
                None,
            );
        }

        let shared = GuardRetaliateSharedState::new(&machine);
        shared.change_state(GuardRetaliateStateType::Inner).unwrap();
        shared.change_state(GuardRetaliateStateType::Idle).unwrap();

        let current = machine.lock().unwrap().get_current_state_id();
        assert_eq!(current, Some(GuardRetaliateStateType::Idle as u32));
    }
}

/// Main guard retaliate state machine - similar to guard but focuses on retaliation
#[derive(Debug)]
pub struct AIGuardRetaliateMachine {
    /// Base state machine
    base: Arc<Mutex<StateMachine>>,
    /// Shared state used by guard retaliate states
    shared: Arc<GuardRetaliateSharedState>,
    /// Position to guard
    position_to_guard: Coord3D,
    /// Nemesis to attack
    nemesis_to_attack: ObjectID,
}

impl AIGuardRetaliateMachine {
    pub fn new(owner: Weak<RwLock<Object>>) -> Self {
        let base = Arc::new(Mutex::new(StateMachine::new(
            Some(owner),
            "AIGuardRetaliateMachine",
        )));
        let shared = Arc::new(GuardRetaliateSharedState::new(&base));

        let mut machine = Self {
            base,
            shared,
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
        };

        machine.define_guard_retaliate_states();
        if let Ok(mut guard) = machine.base.lock() {
            let _ = guard.init_default_state();
        }
        machine
    }

    fn define_guard_retaliate_states(&mut self) {
        let shared = self.shared.clone();
        let base_arc = self.base.clone();

        let mut base = self
            .base
            .lock()
            .expect("guard retaliate state machine lock poisoned");

        // Order matters: first state becomes default.
        base.define_state(
            GuardRetaliateStateType::AttackAggressor as u32,
            Box::new(AIGuardRetaliateAttackAggressorState::new(
                &base_arc,
                shared.clone(),
            )),
            Some(GuardRetaliateStateType::Return as u32),
            Some(GuardRetaliateStateType::Return as u32),
            None,
        );

        base.define_state(
            GuardRetaliateStateType::Return as u32,
            Box::new(AIGuardRetaliateReturnState::new(&base_arc, shared.clone())),
            Some(GuardRetaliateStateType::Idle as u32),
            Some(GuardRetaliateStateType::Inner as u32),
            None,
        );

        base.define_state(
            GuardRetaliateStateType::Idle as u32,
            Box::new(AIGuardRetaliateIdleState::new(&base_arc, shared.clone())),
            Some(GuardRetaliateStateType::Inner as u32),
            Some(EXIT_MACHINE_WITH_SUCCESS),
            None,
        );

        base.define_state(
            GuardRetaliateStateType::Inner as u32,
            Box::new(AIGuardRetaliateInnerState::new(&base_arc, shared.clone())),
            Some(GuardRetaliateStateType::Outer as u32),
            Some(GuardRetaliateStateType::Outer as u32),
            None,
        );

        base.define_state(
            GuardRetaliateStateType::Outer as u32,
            Box::new(AIGuardRetaliateOuterState::new(&base_arc, shared.clone())),
            Some(GuardRetaliateStateType::GetCrate as u32),
            Some(GuardRetaliateStateType::GetCrate as u32),
            None,
        );

        base.define_state(
            GuardRetaliateStateType::GetCrate as u32,
            Box::new(AIGuardRetaliatePickUpCrateState::new(
                &base_arc,
                shared.clone(),
            )),
            Some(GuardRetaliateStateType::Return as u32),
            Some(GuardRetaliateStateType::Return as u32),
            None,
        );
    }

    pub fn is_idle(&self) -> bool {
        self.base
            .lock()
            .map(|machine| {
                machine.get_current_state_id() == Some(GuardRetaliateStateType::Idle as u32)
            })
            .unwrap_or(false)
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
        let Ok(mut guard) = self.base.lock() else {
            return StateReturnType::Failure;
        };
        guard.init_default_state()
    }

    pub fn set_state(&mut self, state: GuardRetaliateStateType) -> StateReturnType {
        let Ok(mut guard) = self.base.lock() else {
            return StateReturnType::Failure;
        };
        guard.set_current_state(state as u32)
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Ok(mut guard) = self.base.lock() else {
            return Ok(());
        };
        guard.halt()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.base
            .lock()
            .map(|machine| machine.is_in_attack_state())
            .unwrap_or(false)
    }

    pub fn update(&mut self) -> StateReturnType {
        let Ok(mut guard) = self.base.lock() else {
            return StateReturnType::Failure;
        };
        guard.update()
    }

    pub fn look_for_inner_target(&mut self) -> bool {
        let Some(owner_arc) = self
            .base
            .lock()
            .ok()
            .and_then(|machine| machine.get_owner())
        else {
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
                        self.set_nemesis_id(team_target);
                        return true;
                    }
                }
            }
        }

        let pos = *self.get_position_to_guard();
        if let Some(target_id) = scan_guard_retaliate_inner_target(&owner_arc, &pos) {
            self.set_nemesis_id(target_id);
            return true;
        }

        false
    }

    pub fn get_std_guard_range(obj: &Arc<RwLock<Object>>) -> f32 {
        let Ok(obj_guard) = obj.read() else {
            return 100.0;
        };
        let id = obj_guard.get_id();
        drop(obj_guard);

        let ai = THE_AI.read().ok();
        ai.and_then(|ai| {
            ai.get_adjusted_vision_range_for_object(
                id,
                vision_factors::OWNER_TYPE | vision_factors::MOOD | vision_factors::GUARD_INNER,
            )
            .ok()
        })
        .unwrap_or(100.0)
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        if version >= 2 {
            if let Ok(mut guard) = self.base.lock() {
                guard.crc(xfer).map_err(|e| e.to_string())?;
            }
        }
        let mut nemesis_to_attack = self.shared.get_nemesis_to_attack();
        xfer.xfer_object_id(&mut nemesis_to_attack)
            .map_err(|e| format!("Failed to crc nemesis_to_attack: {:?}", e))?;
        let mut position_to_guard_x = self.position_to_guard.x;
        xfer.xfer_real(&mut position_to_guard_x)
            .map_err(|e| format!("Failed to crc position_to_guard.x: {:?}", e))?;
        let mut position_to_guard_y = self.position_to_guard.y;
        xfer.xfer_real(&mut position_to_guard_y)
            .map_err(|e| format!("Failed to crc position_to_guard.y: {:?}", e))?;
        let mut position_to_guard_z = self.position_to_guard.z;
        xfer.xfer_real(&mut position_to_guard_z)
            .map_err(|e| format!("Failed to crc position_to_guard.z: {:?}", e))?;
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        if version >= 2 {
            if let Ok(mut guard) = self.base.lock() {
                guard.xfer(xfer).map_err(|e| e.to_string())?;
            }
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

        self.shared.set_nemesis_to_attack(self.nemesis_to_attack);
        self.shared.set_position_to_guard(self.position_to_guard);

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// State implementations for guard retaliate

#[derive(Debug)]
struct GuardRetaliateState {
    base: State,
    shared: Arc<GuardRetaliateSharedState>,
}

impl GuardRetaliateState {
    fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<GuardRetaliateSharedState>,
        name: &str,
    ) -> Self {
        Self {
            base: State::with_machine(Some(Arc::downgrade(machine)), name),
            shared,
        }
    }

    fn state(&self) -> &State {
        &self.base
    }

    fn state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn change_state(&self, state: GuardRetaliateStateType) -> Result<(), String> {
        self.shared.change_state(state)
    }

    fn with_machine<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut StateMachine) -> R,
    {
        self.shared.with_machine(f)
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
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardRetaliateSharedState>) -> Self {
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
    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.state().get_machine_owner() else {
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

        let is_enter_guard = owner
            .read()
            .map(|guard| guard.get_template().is_enter_guard())
            .unwrap_or(false);
        if is_enter_guard {
            let machine_arc = match self.base.state().get_machine() {
                Ok(machine) => machine,
                Err(_) => return StateReturnType::Failure,
            };
            let enter_state = {
                let Ok(machine_guard) = machine_arc.lock() else {
                    return StateReturnType::Failure;
                };
                AIEnterState::new(&machine_guard)
            };
            let _ = self
                .base
                .with_machine(|machine| machine.set_goal_object(Some(Arc::downgrade(&nemesis))));

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
            let radius = 1.5 * AIGuardRetaliateMachine::get_std_guard_range(&owner);
            exit_guard.set_center(pos);
            exit_guard.set_radius_sqr(radius * radius);
            exit_guard.set_conditions(
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
        attack_machine.set_exit_conditions(Box::new(GuardRetaliateExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(&nemesis));

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

        if let Some(owner) = self.base.state().get_machine_owner() {
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

/// Idle guard retaliate state - wait for targets with retaliation focus
#[derive(Debug)]
pub struct AIGuardRetaliateIdleState {
    base: GuardRetaliateState,
    next_enemy_scan_time: u32,
}

impl AIGuardRetaliateIdleState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardRetaliateSharedState>) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliateIdleState"),
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

        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                if ai_guard.get_crate_id() != crate::common::INVALID_ID {
                    if let Ok(machine) = self.base.state().get_machine() {
                        if let Ok(mut machine_guard) = machine.lock() {
                            let _ = machine_guard
                                .set_current_state(GuardRetaliateStateType::GetCrate as u32);
                        }
                    }
                    return StateReturnType::Sleep(self.next_enemy_scan_time - now);
                }
            }
        }

        if let Some(team_arc) = owner_guard.get_team() {
            if let Ok(team_guard) = team_arc.read() {
                if team_guard.attack_common_target() {
                    let team_target = team_guard.get_team_target_object();
                    if team_target != crate::common::INVALID_ID {
                        self.base.set_nemesis_to_attack(team_target);
                        if let Ok(machine) = self.base.state().get_machine() {
                            if let Ok(mut machine_guard) = machine.lock() {
                                if let Some(target) = get_legacy_object(team_target) {
                                    machine_guard.set_goal_object(Some(Arc::downgrade(&target)));
                                }
                            }
                        }
                        return StateReturnType::Success;
                    }
                }
            }
        }

        let guard_pos = self.base.get_position_to_guard();
        drop(owner_guard);

        if let Some(target_id) = scan_guard_retaliate_inner_target(&owner, &guard_pos) {
            self.base.set_nemesis_to_attack(target_id);
            if let Ok(machine) = self.base.state().get_machine() {
                if let Ok(mut machine_guard) = machine.lock() {
                    if let Some(target) = get_legacy_object(target_id) {
                        machine_guard.set_goal_object(Some(Arc::downgrade(&target)));
                    }
                }
            }
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
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardRetaliateSharedState>) -> Self {
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
    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };

        let nemesis_id = self.base.get_nemesis_to_attack();
        let nemesis = if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        };
        let Some(nemesis) = nemesis.or_else(|| self.base.state().get_machine_goal_object()) else {
            self.is_attacking = false;
            self.attack_machine = None;
            return StateReturnType::Success;
        };

        let pos = self.base.get_position_to_guard();
        let std_guard_range = AIGuardRetaliateMachine::get_std_guard_range(&owner);
        let range = {
            let Ok(owner_guard) = owner.read() else {
                return StateReturnType::Failure;
            };
            let owner_id = owner_guard.get_id();
            drop(owner_guard);

            let Ok(ai) = THE_AI.read() else {
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

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIGuardRetaliateAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(GuardRetaliateExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(&nemesis));
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

        if let Some(goal_obj) = self.base.state().get_machine_goal_object() {
            if let Ok(goal_guard) = goal_obj.read() {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    let delta = Coord3D::new(
                        exit_guard.center.x - goal_guard.get_position().x,
                        exit_guard.center.y - goal_guard.get_position().y,
                        exit_guard.center.z - goal_guard.get_position().z,
                    );
                    if let Some(owner) = self.base.state().get_machine_owner() {
                        let vision = AIGuardRetaliateMachine::get_std_guard_range(&owner);
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
    next_return_scan_time: u32,
    goal_position: Coord3D,
}

impl AIGuardRetaliateReturnState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardRetaliateSharedState>) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliateReturn"),
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
        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };
        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    if ai_guard.is_doing_ground_movement() {
                        let _ = ai_guard.adjust_destination(&mut self.goal_position);
                    }
                    ai.ai_move_to_position(&self.goal_position, false, CommandSourceType::FromAi);
                }
            }
        }
        let _ = self
            .base
            .with_machine(|machine| machine.set_goal_position(self.goal_position));
        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if now >= self.next_return_scan_time {
            self.next_return_scan_time = now.saturating_add(get_guard_enemy_return_scan_rate());

            let Some(owner) = self.base.state().get_machine_owner() else {
                return StateReturnType::Failure;
            };
            if let Some(target_id) = scan_guard_retaliate_inner_target(&owner, &self.goal_position)
            {
                self.base.set_nemesis_to_attack(target_id);
                if let Some(target_arc) = get_legacy_object(target_id) {
                    let _ = self.base.with_machine(|machine| {
                        machine.set_goal_object(Some(Arc::downgrade(&target_arc)))
                    });
                }
                return StateReturnType::Failure;
            }
        }

        let Some(owner) = self.base.state().get_machine_owner() else {
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

    fn on_exit(&mut self, _status: StateExitType) {
        // Nothing to clean up.
    }
}

/// Pick up crate state for guard retaliate
#[derive(Debug)]
pub struct AIGuardRetaliatePickUpCrateState {
    base: GuardRetaliateState,
}

impl AIGuardRetaliatePickUpCrateState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardRetaliateSharedState>) -> Self {
        Self {
            base: GuardRetaliateState::new(machine, shared, "AIGuardRetaliatePickUpCrate"),
        }
    }
}

impl StateImplementation for AIGuardRetaliatePickUpCrateState {
    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Failure;
        };
        let Some(ai) = owner_guard.get_ai_update_interface() else {
            return StateReturnType::Success;
        };
        let Ok(ai_guard) = ai.lock() else {
            return StateReturnType::Failure;
        };
        let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() else {
            return StateReturnType::Success;
        };

        if let Ok(machine) = self.base.state().get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.set_goal_object(Some(Arc::downgrade(&crate_obj)));
            }
        }

        if let Ok(crate_guard) = crate_obj.lock() {
            ai.ai_move_to_position(crate_guard.get_position(), false, CommandSourceType::FromAi);
        }

        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(goal) = self.base.state().get_machine_goal_object() else {
            return StateReturnType::Success;
        };

        let Ok(owner_guard) = owner.read() else {
            return StateReturnType::Continue;
        };
        let Ok(goal_guard) = goal.read() else {
            return StateReturnType::Success;
        };

        let owner_pos = owner_guard.get_position();
        let goal_pos = goal_guard.get_position();
        let dx = owner_pos.x - goal_pos.x;
        let dy = owner_pos.y - goal_pos.y;
        let dist_sqr = dx * dx + dy * dy;

        if dist_sqr <= CRATE_PICKUP_RANGE_SQR {
            return StateReturnType::Success;
        }

        StateReturnType::Continue
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // Cleanup when exiting pick up crate state
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
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardRetaliateSharedState>) -> Self {
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
    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };

        let mut nemesis = self.base.state().get_machine_goal_object();
        if nemesis.is_none() {
            let nemesis_id = self.base.get_nemesis_to_attack();
            if nemesis_id != crate::common::INVALID_ID {
                nemesis = get_legacy_object(nemesis_id);
            }
        }

        if nemesis.is_none() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(body) = owner_guard.get_body_module() {
                    if let Ok(body_guard) = body.lock() {
                        if let Some(info) = body_guard.get_last_damage_info() {
                            if info.source_id != crate::common::INVALID_ID {
                                if let Some(target) = get_legacy_object(info.source_id) {
                                    if let Ok(target_guard) = target.read() {
                                        if owner_guard.relationship_to(&target_guard)
                                            == Relationship::Enemies
                                        {
                                            self.base.set_nemesis_to_attack(info.source_id);
                                            nemesis = Some(target.clone());
                                            let _ = self.base.with_machine(|machine| {
                                                machine
                                                    .set_goal_object(Some(Arc::downgrade(&target)))
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let Some(nemesis) = nemesis else {
            self.is_attacking = false;
            self.attack_machine = None;
            return StateReturnType::Success;
        };

        let pos = self.base.get_position_to_guard();
        let std_guard_range = AIGuardRetaliateMachine::get_std_guard_range(&owner);
        let range = {
            let Ok(owner_guard) = owner.read() else {
                return StateReturnType::Failure;
            };
            let owner_id = owner_guard.get_id();
            drop(owner_guard);

            let Ok(ai) = THE_AI.read() else {
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

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIGuardRetaliateAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(GuardRetaliateExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(&nemesis));
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

        if let Some(owner) = self.base.state().get_machine_owner() {
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

/// Helper function to check if an object has attacked and can be retaliated against
pub fn has_attacked_me_and_i_can_return_fire_retaliate(machine: &StateMachine) -> bool {
    if let Some(owner) = machine.get_owner() {
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
    } else {
        false
    }
}
