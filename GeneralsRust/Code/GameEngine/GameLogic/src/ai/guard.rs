use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::states::{AIEnterState, AttackExitConditionsInterface, AttackStateMachine};
use crate::ai::{object_registry::get_legacy_object, vision_factors, GuardMode, THE_AI};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::coord::*;
use crate::common::vector_ext::Vector3Ext;
use crate::common::xfer::{Xfer, XferExt, XferVersion};
use crate::common::*;
use crate::compat::{legacy_transition, register_classic_state, ClassicState};
use crate::helpers::{game_logic_random_value, TheGameLogic, ThePartitionManager};
use crate::modules::AIUpdateInterfaceExt;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::polygon_trigger::PolygonTrigger;
use crate::state_machine::*;
use crate::terrain::get_terrain_logic;

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

fn scan_guard_inner_target(
    owner_arc: &Arc<RwLock<Object>>,
    pos: &Coord3D,
    guard_mode: GuardMode,
    area: Option<&PolygonTrigger>,
) -> Option<ObjectID> {
    let Ok(owner_guard) = owner_arc.read() else {
        return None;
    };

    if !owner_guard.is_able_to_attack() {
        return None;
    }

    let is_enter_guard = owner_guard.get_template().is_enter_guard();
    let is_hijack_guard = owner_guard.get_template().is_hijack_guard();

    let mut vision_range = owner_arc
        .read()
        .ok()
        .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
        .unwrap_or(100.0);
    let mut center = *pos;
    if let Some(area) = area {
        vision_range = area.get_radius();
        center = area.get_center_point();
    }
    let flying_only = matches!(guard_mode, GuardMode::GuardFlyingUnitsOnly);
    let Some(partition) = ThePartitionManager::get() else {
        return None;
    };

    partition.get_closest_object_2d(&center, vision_range, |candidate| {
        if candidate.get_id() == owner_guard.get_id() {
            return false;
        }
        if candidate.is_effectively_dead() {
            return false;
        }
        if owner_guard.is_off_map() != candidate.is_off_map() {
            return false;
        }
        if flying_only && !candidate.is_airborne_target() && !candidate.is_kind_of(KindOf::Aircraft)
        {
            return false;
        }
        if let Some(area) = area {
            let position = candidate.get_position();
            if !area.point_in_trigger(&Coord2D::new(position.x, position.y)) {
                return false;
            }
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

/// Guard state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardStateType {
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

/// Exit conditions for attack states
#[derive(Debug, Clone)]
pub struct ExitConditions {
    /// Bitmask of conditions to consider
    conditions_to_consider: u32,
    /// Center position for radius checks
    center: Coord3D,
    /// Radius squared for distance checks
    radius_sqr: f32,
    /// Frame at which we give up attacking
    attack_give_up_frame: u32,
}

#[derive(Debug)]
pub struct GuardSharedState {
    machine: Weak<Mutex<StateMachine>>,
    target_to_guard: Mutex<ObjectID>,
    nemesis_to_attack: Mutex<ObjectID>,
    position_to_guard: Mutex<Coord3D>,
    area_to_guard: Mutex<Option<Arc<PolygonTrigger>>>,
    guard_mode: Mutex<GuardMode>,
}

impl GuardSharedState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            machine: Arc::downgrade(machine),
            target_to_guard: Mutex::new(crate::common::INVALID_ID),
            nemesis_to_attack: Mutex::new(crate::common::INVALID_ID),
            position_to_guard: Mutex::new(Coord3D::new(0.0, 0.0, 0.0)),
            area_to_guard: Mutex::new(None),
            guard_mode: Mutex::new(GuardMode::Normal),
        }
    }

    fn with_machine<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut StateMachine) -> R,
    {
        let machine = self
            .machine
            .upgrade()
            .ok_or_else(|| "guard state machine context lost".to_string())?;
        let mut guard = machine
            .lock()
            .map_err(|_| "guard state machine lock poisoned".to_string())?;
        Ok(f(&mut guard))
    }

    fn change_state(&self, state: GuardStateType) -> Result<(), String> {
        self.with_machine(|machine| {
            let _ = machine.set_current_state(state as u32);
        })
    }

    fn get_target_to_guard(&self) -> ObjectID {
        self.target_to_guard
            .lock()
            .map(|id| *id)
            .unwrap_or(crate::common::INVALID_ID)
    }

    fn set_target_to_guard(&self, id: ObjectID) {
        if let Ok(mut target) = self.target_to_guard.lock() {
            *target = id;
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

    fn get_guard_mode(&self) -> GuardMode {
        self.guard_mode
            .lock()
            .map(|mode| *mode)
            .unwrap_or(GuardMode::Normal)
    }

    fn get_area_to_guard(&self) -> Option<Arc<PolygonTrigger>> {
        self.area_to_guard
            .lock()
            .ok()
            .and_then(|area| area.as_ref().map(Arc::clone))
    }

    fn set_area_to_guard(&self, area: Option<Arc<PolygonTrigger>>) {
        if let Ok(mut current) = self.area_to_guard.lock() {
            *current = area;
        }
    }

    fn set_guard_mode(&self, guard_mode: GuardMode) {
        if let Ok(mut mode) = self.guard_mode.lock() {
            *mode = guard_mode;
        }
    }

    fn notify_state_change(&self) -> Result<(), String> {
        // Guard state notifications are not wired to external systems yet.
        Ok(())
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
    fn guard_shared_state_applies_state_transition() {
        let machine = Arc::new(Mutex::new(StateMachine::new(
            Some(Weak::new()),
            "test_guard",
        )));
        {
            let mut locked = machine.lock().unwrap();
            locked.define_state(
                GuardStateType::Inner as u32,
                Box::new(DummyState),
                None,
                None,
                None,
            );
            locked.define_state(
                GuardStateType::Outer as u32,
                Box::new(DummyState),
                None,
                None,
                None,
            );
        }

        let shared = GuardSharedState::new(&machine);
        shared.change_state(GuardStateType::Inner).unwrap();
        shared.change_state(GuardStateType::Outer).unwrap();

        let current = machine.lock().unwrap().get_current_state_id();
        assert_eq!(current, Some(GuardStateType::Outer as u32));
    }
}

/// Exit condition flags
pub mod exit_conditions {
    pub const ATTACK_EXIT_IF_OUTSIDE_RADIUS: u32 = 0x01;
    pub const ATTACK_EXIT_IF_EXPIRED_DURATION: u32 = 0x02;
    pub const ATTACK_EXIT_IF_NO_UNIT_FOUND: u32 = 0x04;
}

impl ExitConditions {
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
            return (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND)
                != 0;
        }

        if (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION) != 0 {
            if machine.get_current_frame() >= self.attack_give_up_frame {
                return true;
            }
        }

        if (self.conditions_to_consider & exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS) != 0 {
            if let Some(obj) = goal_object {
                if let Ok(obj_ref) = obj.try_read() {
                    let obj_pos = obj_ref.get_position();
                    let delta = Coord3D::new(
                        obj_pos.x - self.center.x,
                        obj_pos.y - self.center.y,
                        0.0, // Don't account for Z in distance calculation
                    );

                    if Vector3Ext::length_sqr(&delta) > self.radius_sqr {
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

#[derive(Debug, Clone)]
struct GuardExitConditionsHandle {
    inner: Arc<Mutex<ExitConditions>>,
}

impl GuardExitConditionsHandle {
    fn new(inner: Arc<Mutex<ExitConditions>>) -> Self {
        Self { inner }
    }
}

impl AttackExitConditionsInterface for GuardExitConditionsHandle {
    fn should_exit(&self, machine: &StateMachine) -> bool {
        let Ok(guard) = self.inner.lock() else {
            return false;
        };
        guard.should_exit(machine)
    }
}

/// Main guard state machine
#[derive(Debug)]
pub struct AIGuardMachine {
    /// Base state machine
    base: Arc<Mutex<StateMachine>>,
    /// Shared state used by guard states
    shared: Arc<GuardSharedState>,
    /// Object to guard by ID
    target_to_guard: ObjectID,
    /// Area to guard
    area_to_guard: Option<Arc<PolygonTrigger>>,
    /// Position to guard
    position_to_guard: Coord3D,
    /// Nemesis to attack
    nemesis_to_attack: ObjectID,
    /// Guard mode
    guard_mode: GuardMode,
}

impl AIGuardMachine {
    pub fn new(owner: Weak<RwLock<Object>>) -> Self {
        let base = Arc::new(Mutex::new(StateMachine::new(Some(owner), "AIGuardMachine")));
        let shared = Arc::new(GuardSharedState::new(&base));

        let mut machine = Self {
            base,
            shared,
            target_to_guard: crate::common::INVALID_ID,
            area_to_guard: None,
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
            guard_mode: GuardMode::Normal,
        };

        // Define states - order matters: first state is default
        machine.define_guard_states();
        if let Ok(mut guard) = machine.base.lock() {
            let _ = guard.init_default_state();
        }
        machine
    }

    fn define_guard_states(&mut self) {
        let shared = self.shared.clone();
        let base_arc = self.base.clone();

        let mut base = self.base.lock().expect("guard state machine lock poisoned");
        let attack_aggressor_conditions_inner = vec![legacy_transition(
            guard_attack_aggressor_inner,
            GuardStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];
        let attack_aggressor_conditions_return = vec![legacy_transition(
            guard_attack_aggressor_return,
            GuardStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];
        let attack_aggressor_conditions_idle = vec![legacy_transition(
            guard_attack_aggressor_idle,
            GuardStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];

        register_classic_state(
            &mut *base,
            GuardStateType::Inner as u32,
            AIGuardInnerState::new(&base_arc, shared.clone()),
            Some(GuardStateType::Outer as u32),
            Some(GuardStateType::Outer as u32),
            &attack_aggressor_conditions_inner,
        );

        register_classic_state(
            &mut *base,
            GuardStateType::Return as u32,
            AIGuardReturnState::new(&base_arc, shared.clone()),
            Some(GuardStateType::Idle as u32),
            Some(GuardStateType::Inner as u32),
            &attack_aggressor_conditions_return,
        );

        register_classic_state(
            &mut *base,
            GuardStateType::Idle as u32,
            AIGuardIdleState::new(&base_arc, shared.clone()),
            Some(GuardStateType::Inner as u32),
            Some(GuardStateType::Return as u32),
            &attack_aggressor_conditions_idle,
        );

        register_classic_state(
            &mut *base,
            GuardStateType::Outer as u32,
            AIGuardOuterState::new(&base_arc, shared.clone()),
            Some(GuardStateType::GetCrate as u32),
            Some(GuardStateType::GetCrate as u32),
            &[],
        );

        register_classic_state(
            &mut *base,
            GuardStateType::GetCrate as u32,
            AIGuardPickUpCrateState::new(&base_arc, shared.clone()),
            Some(GuardStateType::Return as u32),
            Some(GuardStateType::Return as u32),
            &[],
        );

        register_classic_state(
            &mut *base,
            GuardStateType::AttackAggressor as u32,
            AIGuardAttackAggressorState::new(&base_arc, shared.clone()),
            Some(GuardStateType::Inner as u32),
            Some(GuardStateType::Inner as u32),
            &[],
        );
    }

    pub fn find_target_to_guard_by_id(&self) -> Option<Arc<RwLock<Object>>> {
        if self.target_to_guard == crate::common::INVALID_ID {
            return None;
        }
        get_legacy_object(self.target_to_guard)
    }

    pub fn set_target_to_guard(&mut self, object: Option<&Arc<RwLock<Object>>>) {
        self.target_to_guard = if let Some(obj) = object {
            if let Ok(obj_ref) = obj.try_read() {
                obj_ref.get_id()
            } else {
                crate::common::INVALID_ID
            }
        } else {
            crate::common::INVALID_ID
        };
        self.shared.set_target_to_guard(self.target_to_guard);
    }

    pub fn get_position_to_guard(&self) -> &Coord3D {
        &self.position_to_guard
    }

    pub fn set_target_position_to_guard(&mut self, pos: &Coord3D) {
        self.position_to_guard = *pos;
        self.shared.set_position_to_guard(*pos);
    }

    pub fn get_area_to_guard(&self) -> Option<&Arc<PolygonTrigger>> {
        self.area_to_guard.as_ref()
    }

    pub fn set_area_to_guard(&mut self, area: Option<Arc<PolygonTrigger>>) {
        self.area_to_guard = area;
        self.shared.set_area_to_guard(self.area_to_guard.clone());
    }

    pub fn set_nemesis_id(&mut self, id: ObjectID) {
        self.nemesis_to_attack = id;
        self.shared.set_nemesis_to_attack(id);
    }

    pub fn get_nemesis_id(&self) -> ObjectID {
        self.shared.get_nemesis_to_attack()
    }

    pub fn get_guard_mode(&self) -> GuardMode {
        self.guard_mode
    }

    pub fn set_guard_mode(&mut self, guard_mode: GuardMode) {
        self.guard_mode = guard_mode;
        self.shared.set_guard_mode(guard_mode);
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        let Ok(mut guard) = self.base.lock() else {
            return StateReturnType::Failure;
        };
        guard.init_default_state()
    }

    pub fn set_state(&mut self, state: GuardStateType) -> StateReturnType {
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

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.base
            .lock()
            .map(|machine| machine.is_in_guard_idle_state())
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
                    if team_target != INVALID_ID {
                        self.set_nemesis_id(team_target);
                        return true;
                    }
                }
            }
        }

        let area = self.get_area_to_guard().map(Arc::clone);
        let center = if let Some(area) = area.as_ref() {
            area.get_center_point()
        } else if let Some(target_arc) = self.find_target_to_guard_by_id() {
            target_arc
                .read()
                .ok()
                .map(|target| *target.get_position())
                .unwrap_or_else(|| *self.get_position_to_guard())
        } else {
            *self.get_position_to_guard()
        };

        if let Some(target_id) =
            scan_guard_inner_target(&owner_arc, &center, self.guard_mode, area.as_deref())
        {
            self.set_nemesis_id(target_id);
            return true;
        }

        false
    }

    pub fn get_std_guard_range(obj_id: ObjectID) -> f32 {
        let ai = THE_AI.read().ok();
        ai.and_then(|ai| {
            ai.get_adjusted_vision_range_for_object(
                obj_id,
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
        let mut target_to_guard = self.target_to_guard;
        xfer.xfer_object_id(&mut target_to_guard)
            .map_err(|e| format!("Failed to crc target_to_guard: {:?}", e))?;
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
        let mut trigger_name = self
            .area_to_guard
            .as_ref()
            .map(|area| area.get_trigger_name().str().to_string())
            .unwrap_or_default();
        xfer.xfer_ascii_string(&mut trigger_name)
            .map_err(|e| format!("Failed to crc guard trigger name: {:?}", e))?;
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

        xfer.xfer_object_id(&mut self.target_to_guard)
            .map_err(|e| format!("Failed to xfer target_to_guard: {:?}", e))?;
        xfer.xfer_object_id(&mut self.nemesis_to_attack)
            .map_err(|e| format!("Failed to xfer nemesis_to_attack: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.x)
            .map_err(|e| format!("Failed to xfer position_to_guard.x: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.y)
            .map_err(|e| format!("Failed to xfer position_to_guard.y: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.z)
            .map_err(|e| format!("Failed to xfer position_to_guard.z: {:?}", e))?;

        let mut trigger_name = self
            .area_to_guard
            .as_ref()
            .map(|area| area.get_trigger_name().str().to_string())
            .unwrap_or_default();
        xfer.xfer_ascii_string(&mut trigger_name)
            .map_err(|e| format!("Failed to xfer guard trigger name: {:?}", e))?;
        if xfer.is_loading() {
            self.area_to_guard = None;
            if !trigger_name.is_empty() {
                if let Ok(terrain) = get_terrain_logic().read() {
                    if let Some(trigger) = terrain.get_trigger_area_by_name(&trigger_name) {
                        self.area_to_guard = Some(Arc::new(trigger.clone()));
                    }
                }
            }
        }

        self.shared.set_target_to_guard(self.target_to_guard);
        self.shared.set_nemesis_to_attack(self.nemesis_to_attack);
        self.shared.set_position_to_guard(self.position_to_guard);
        self.shared.set_area_to_guard(self.area_to_guard.clone());
        self.shared.set_guard_mode(self.guard_mode);

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

// State implementations

/// Shared helper for guard state implementations
#[derive(Debug)]
pub struct GuardState {
    base: State,
    shared: Arc<GuardSharedState>,
}

impl GuardState {
    fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardSharedState>, name: &str) -> Self {
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

    fn change_state(&self, state: GuardStateType) -> Result<(), String> {
        self.shared.change_state(state)
    }

    fn with_machine<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut StateMachine) -> R,
    {
        self.shared.with_machine(f)
    }

    fn get_target_to_guard(&self) -> ObjectID {
        self.shared.get_target_to_guard()
    }

    fn get_nemesis_to_attack(&self) -> ObjectID {
        self.shared.get_nemesis_to_attack()
    }

    fn set_nemesis_to_attack(&self, id: ObjectID) {
        self.shared.set_nemesis_to_attack(id);
    }

    fn get_position_to_guard(&self) -> Coord3D {
        self.shared.get_position_to_guard()
    }

    fn get_guard_mode(&self) -> GuardMode {
        self.shared.get_guard_mode()
    }

    fn get_area_to_guard(&self) -> Option<Arc<PolygonTrigger>> {
        self.shared.get_area_to_guard()
    }
}

/// Inner guard state - attack anything within area
#[derive(Debug)]
pub struct AIGuardInnerState {
    base: GuardState,
    exit_conditions: Arc<Mutex<ExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
    enter_state: Option<AIEnterState>,
}

impl AIGuardInnerState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardInner"),
            exit_conditions: Arc::new(Mutex::new(ExitConditions::new())),
            is_attacking: false,
            attack_machine: None,
            enter_state: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardInnerState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardInnerState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .state()
            .get_machine_owner()
            .ok_or_else(|| "guard inner missing owner".to_string())?;
        let nemesis_id = self.base.get_nemesis_to_attack();
        let Some(nemesis) = (nemesis_id != crate::common::INVALID_ID)
            .then(|| get_legacy_object(nemesis_id))
            .flatten()
        else {
            self.is_attacking = false;
            self.attack_machine = None;
            self.enter_state = None;
            return Ok(StateReturnType::Success);
        };

        let is_enter_guard = owner
            .read()
            .map(|guard| guard.get_template().is_enter_guard())
            .unwrap_or(false);

        if is_enter_guard {
            let machine_arc = self.base.state().get_machine()?;
            let enter_state = {
                let machine_guard = machine_arc
                    .lock()
                    .map_err(|_| "guard inner machine lock poisoned".to_string())?;
                AIEnterState::new(&machine_guard)
            };
            let nemesis_id = nemesis.read().ok().map(|g| g.get_id());
            let _ = self
                .base
                .with_machine(|machine| machine.set_goal_object_by_id(nemesis_id));

            self.is_attacking = false;
            self.attack_machine = None;
            self.enter_state = Some(enter_state);
            if let Some(enter_state) = self.enter_state.as_mut() {
                let result = enter_state.on_enter();
                if result == StateReturnType::Continue {
                    return Ok(StateReturnType::Continue);
                }
            }
            return Ok(StateReturnType::Success);
        }

        let mut center = self.base.get_position_to_guard();
        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    center = *target_guard.get_position();
                }
            }
        }

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            let radius = owner
                .read()
                .ok()
                .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
                .unwrap_or(100.0);
            exit_guard.set_center(center);
            exit_guard.set_radius_sqr(radius * radius);
            exit_guard.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIGuardAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(GuardExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(nemesis.read().ok().map(|g| g.get_id()));

        let result = attack_machine.init_default_state();
        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);
        self.enter_state = None;

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Some(attack_machine) = self.attack_machine.as_mut() {
            let target_to_guard = self.base.get_target_to_guard();
            if target_to_guard != crate::common::INVALID_ID {
                if let Some(target_arc) = get_legacy_object(target_to_guard) {
                    if let Ok(target_guard) = target_arc.read() {
                        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                            exit_guard.set_center(*target_guard.get_position());
                        }
                    }
                }
            }
            return Ok(attack_machine.update());
        }

        if let Some(enter_state) = self.enter_state.as_mut() {
            return Ok(enter_state.update());
        }

        Ok(StateReturnType::Success)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        if let Some(mut enter_state) = self.enter_state.take() {
            enter_state.on_exit(_exit);
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
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.is_attack()
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Idle guard state - wait for targets to appear
#[derive(Debug)]
pub struct AIGuardIdleState {
    base: GuardState,
    next_enemy_scan_time: u32,
    guardee_pos: Coord3D,
}

impl AIGuardIdleState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardIdleState"),
            next_enemy_scan_time: 0,
            guardee_pos: Coord3D::new(0.0, 0.0, 0.0),
        }
    }

    pub fn is_guard_idle(&self) -> bool {
        true
    }
}

impl StateImplementation for AIGuardIdleState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardIdleState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_scan_rate();
        self.next_enemy_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        let target_id = self.base.get_target_to_guard();
        if target_id != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    self.guardee_pos = *target_guard.get_position();
                    return Ok(StateReturnType::Continue);
                }
            }
        }

        self.guardee_pos = self.base.get_position_to_guard();
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now < self.next_enemy_scan_time {
            return Ok(StateReturnType::Sleep(self.next_enemy_scan_time - now));
        }

        self.next_enemy_scan_time = now.saturating_add(get_guard_enemy_scan_rate());

        let owner = self
            .base
            .state()
            .get_machine_owner()
            .ok_or_else(|| "guard idle missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "guard idle owner lock poisoned".to_string())?;

        if let Some(ai) = owner_guard.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                if ai_guard.get_crate_id() != crate::common::INVALID_ID {
                    self.base.change_state(GuardStateType::GetCrate)?;
                    return Ok(StateReturnType::Sleep(self.next_enemy_scan_time - now));
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
                                machine_guard.set_goal_object_by_id(Some(team_target));
                            }
                        }
                        return Ok(StateReturnType::Success);
                    }
                }
            }
        }

        drop(owner_guard);

        let target_id = self.base.get_target_to_guard();
        let center = if target_id != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_id) {
                target_arc
                    .read()
                    .ok()
                    .map(|target| *target.get_position())
                    .unwrap_or_else(|| self.base.get_position_to_guard())
            } else {
                self.base.get_position_to_guard()
            }
        } else {
            self.base.get_position_to_guard()
        };

        let area = self.base.get_area_to_guard();
        if let Some(target_id) =
            scan_guard_inner_target(&owner, &center, self.base.get_guard_mode(), area.as_deref())
        {
            if let Some(target_arc) = get_legacy_object(target_id) {
                self.base.set_nemesis_to_attack(target_id);
                if let Ok(machine) = self.base.state().get_machine() {
                    if let Ok(mut machine_guard) = machine.lock() {
                        machine_guard.set_goal_object_by_id(Some(target_id));
                    }
                }
                return Ok(StateReturnType::Success);
            }
        }

        if target_id != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_id) {
                if let Ok(target_guard) = target_arc.read() {
                    let target_pos = target_guard.get_position();
                    let delta = Coord3D::new(
                        target_pos.x - self.guardee_pos.x,
                        target_pos.y - self.guardee_pos.y,
                        0.0,
                    );
                    let threshold = PATHFIND_CELL_SIZE_F * 4.0;
                    if Vector3Ext::length_sqr(&delta) > threshold * threshold {
                        return Ok(StateReturnType::Failure);
                    }
                }
            }
        }

        Ok(StateReturnType::Sleep(self.next_enemy_scan_time - now))
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Cleanup when exiting idle guard state
        Ok(())
    }

    fn classic_is_guard_idle(&self) -> bool {
        true
    }
}

/// Outer guard state - attack aggressive targets with timer
#[derive(Debug)]
pub struct AIGuardOuterState {
    base: GuardState,
    exit_conditions: Arc<Mutex<ExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AIGuardOuterState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardOuter"),
            exit_conditions: Arc::new(Mutex::new(ExitConditions::new())),
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardOuterState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardOuterState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        if matches!(self.base.get_guard_mode(), GuardMode::GuardWithoutPursuit) {
            return Ok(StateReturnType::Success);
        }

        let owner = self
            .base
            .state()
            .get_machine_owner()
            .ok_or_else(|| "guard outer missing owner".to_string())?;
        let nemesis_id = self.base.get_nemesis_to_attack();
        let nemesis = if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        };
        let Some(nemesis) = nemesis.or_else(|| self.base.state().get_machine_goal_object()) else {
            self.is_attacking = false;
            self.attack_machine = None;
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = self.base.get_target_to_guard();
        let mut center = self.base.get_position_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    center = *target_guard.get_position();
                }
            }
        }

        let mut range = {
            let ai = THE_AI
                .read()
                .map_err(|_| "guard outer AI lock poisoned".to_string())?;
            ai.get_adjusted_vision_range_for_object(
                owner
                    .read()
                    .map_err(|_| "guard outer owner lock poisoned".to_string())?
                    .get_id(),
                vision_factors::OWNER_TYPE | vision_factors::MOOD,
            )
            .unwrap_or_else(|_| {
                owner
                    .read()
                    .ok()
                    .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
                    .unwrap_or(100.0)
            })
        };

        if let Some(area) = self.base.get_area_to_guard() {
            if range < area.get_radius() {
                range = area.get_radius();
            }
            center = area.get_center_point();
        }

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_center(center);
            exit_guard.set_radius_sqr(range * range);
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            exit_guard.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIGuardAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(GuardExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(nemesis.read().ok().map(|g| g.get_id()));
        let result = attack_machine.init_default_state();

        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                        exit_guard.set_center(*target_guard.get_position());
                    }
                }
            }
        }

        if let Some(goal_obj) = self.base.state().get_machine_goal_object() {
            if let Ok(goal_guard) = goal_obj.read() {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    let delta = Coord3D::new(
                        exit_guard.center.x - goal_guard.get_position().x,
                        exit_guard.center.y - goal_guard.get_position().y,
                        exit_guard.center.z - goal_guard.get_position().z,
                    );
                    let owner = self
                        .base
                        .state()
                        .get_machine_owner()
                        .ok_or_else(|| "guard outer missing owner".to_string())?;
                    let vision = owner
                        .read()
                        .ok()
                        .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
                        .unwrap_or(100.0);
                    if Vector3Ext::length_sqr(&delta) <= vision * vision {
                        exit_guard.set_attack_give_up_frame(
                            TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
                        );
                    }
                }
            }
        }

        Ok(attack_machine.update())
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        self.is_attacking = false;
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.is_attack()
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Return guard state - move back to guard position
#[derive(Debug)]
pub struct AIGuardReturnState {
    base: GuardState,
    next_return_scan_time: u32,
    goal_position: Coord3D,
}

impl AIGuardReturnState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardReturn"),
            next_return_scan_time: 0,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
        }
    }
}

impl StateImplementation for AIGuardReturnState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardReturnState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_return_scan_rate();
        self.next_return_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        self.goal_position = self.base.get_position_to_guard();
        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    self.goal_position = *target_guard.get_position();
                }
            }
        }
        if let Some(area) = self.base.get_area_to_guard() {
            self.goal_position = area.get_center_point();
        }

        let owner = self
            .base
            .state()
            .get_machine_owner()
            .ok_or_else(|| "guard return missing owner".to_string())?;
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
        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now >= self.next_return_scan_time {
            self.next_return_scan_time = now.saturating_add(get_guard_enemy_return_scan_rate());

            let owner = self
                .base
                .state()
                .get_machine_owner()
                .ok_or_else(|| "guard return missing owner".to_string())?;
            let target_id = self.base.get_target_to_guard();
            let center = if target_id != crate::common::INVALID_ID {
                if let Some(target_arc) = get_legacy_object(target_id) {
                    target_arc
                        .read()
                        .ok()
                        .map(|target| *target.get_position())
                        .unwrap_or_else(|| self.base.get_position_to_guard())
                } else {
                    self.base.get_position_to_guard()
                }
            } else {
                self.base.get_position_to_guard()
            };
            let area = self.base.get_area_to_guard();

            if let Some(target) = scan_guard_inner_target(
                &owner,
                &center,
                self.base.get_guard_mode(),
                area.as_deref(),
            ) {
                self.base.set_nemesis_to_attack(target);
                let _ = self
                    .base
                    .with_machine(|machine| machine.set_goal_object_by_id(Some(target)));
                return Ok(StateReturnType::Failure);
            }
        }

        let owner = self
            .base
            .state()
            .get_machine_owner()
            .ok_or_else(|| "guard return missing owner".to_string())?;
        let owner_guard = owner
            .read()
            .map_err(|_| "guard return owner lock poisoned".to_string())?;
        let owner_pos = owner_guard.get_position();
        let dx = owner_pos.x - self.goal_position.x;
        let dy = owner_pos.y - self.goal_position.y;
        if dx * dx + dy * dy <= CLOSE_ENOUGH * CLOSE_ENOUGH {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Cleanup when exiting return guard state
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Pick up crate state
#[derive(Debug)]
pub struct AIGuardPickUpCrateState {
    base: GuardState,
}

impl AIGuardPickUpCrateState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardPickUpCrate"),
        }
    }
}

impl StateImplementation for AIGuardPickUpCrateState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardPickUpCrateState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .state()
            .get_machine_owner()
            .ok_or_else(|| "pick up crate missing owner".to_string())?;

        let owner_guard = owner
            .read()
            .map_err(|_| "pick up crate owner lock poisoned".to_string())?;
        let Some(ai) = owner_guard.get_ai_update_interface() else {
            return Ok(StateReturnType::Success);
        };
        let Ok(ai_guard) = ai.lock() else {
            return Ok(StateReturnType::Failure);
        };
        let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() else {
            return Ok(StateReturnType::Success);
        };

        let crate_id = crate_obj.read().ok().map(|g| g.get_id());
        let _ = self
            .base
            .shared
            .with_machine(|machine| machine.set_goal_object_by_id(crate_id));

        if let Ok(crate_guard) = crate_obj.read() {
            ai.ai_move_to_position(crate_guard.get_position(), false, CommandSourceType::FromAi);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .state()
            .get_machine_owner()
            .ok_or_else(|| "pick up crate missing owner".to_string())?;
        let goal = match self.base.state().get_machine_goal_object() {
            Some(goal) => goal,
            None => return Ok(StateReturnType::Success),
        };

        let owner_guard = owner
            .read()
            .map_err(|_| "pick up crate owner lock poisoned".to_string())?;
        let goal_guard = match goal.read() {
            Ok(goal_guard) => goal_guard,
            Err(_) => return Ok(StateReturnType::Success),
        };

        let owner_pos = owner_guard.get_position();
        let goal_pos = goal_guard.get_position();
        let dx = owner_pos.x - goal_pos.x;
        let dy = owner_pos.y - goal_pos.y;
        let dist_sqr = dx * dx + dy * dy;

        if dist_sqr <= CRATE_PICKUP_RANGE_SQR {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Cleanup when exiting pick up crate state
        Ok(())
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Attack aggressor state - attack something that attacked us
#[derive(Debug)]
pub struct AIGuardAttackAggressorState {
    base: GuardState,
    exit_conditions: Arc<Mutex<ExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AIGuardAttackAggressorState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<GuardSharedState>) -> Self {
        Self {
            base: GuardState::new(machine, shared, "AIGuardAttackAggressor"),
            exit_conditions: Arc::new(Mutex::new(ExitConditions::new())),
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
}

impl StateImplementation for AIGuardAttackAggressorState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, status: StateExitType) {
        let _ = self.classic_on_exit(status);
    }
}

impl ClassicState for AIGuardAttackAggressorState {
    fn base_state(&self) -> &State {
        self.base.state()
    }

    fn base_state_mut(&mut self) -> &mut State {
        self.base.state_mut()
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .state()
            .get_machine_owner()
            .ok_or_else(|| "guard aggressor missing owner".to_string())?;

        let mut nemesis = self.base.state().get_machine_goal_object();
        if nemesis.is_none() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(body) = owner_guard.get_body_module() {
                    if let Ok(body_guard) = body.lock() {
                        if let Some(info) = body_guard.get_last_damage_info() {
                            nemesis = get_legacy_object(info.source_id);
                            if let Some(target) = nemesis.as_ref() {
                                let target_id = target.read().ok().map(|g| g.get_id());
                                let _ = self.base.with_machine(|machine| {
                                    machine.set_goal_object_by_id(target_id)
                                });
                            }
                        }
                    }
                }
            }
        }

        let Some(nemesis) = nemesis else {
            self.is_attacking = false;
            self.attack_machine = None;
            return Ok(StateReturnType::Success);
        };

        let mut center = self.base.get_position_to_guard();
        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    center = *target_guard.get_position();
                }
            }
        }

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            let radius = owner
                .read()
                .ok()
                .map(|g| AIGuardMachine::get_std_guard_range(g.get_id()))
                .unwrap_or(100.0);
            exit_guard.set_center(center);
            exit_guard.set_radius_sqr(radius * radius);
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
            exit_guard.set_conditions(
                exit_conditions::ATTACK_EXIT_IF_EXPIRED_DURATION
                    | exit_conditions::ATTACK_EXIT_IF_OUTSIDE_RADIUS
                    | exit_conditions::ATTACK_EXIT_IF_NO_UNIT_FOUND,
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AIGuardAttackMachine",
            true,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(GuardExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(nemesis.read().ok().map(|g| g.get_id()));
        let result = attack_machine.init_default_state();

        self.is_attacking = matches!(result, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if result == StateReturnType::Continue {
            Ok(StateReturnType::Continue)
        } else {
            Ok(StateReturnType::Success)
        }
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return Ok(StateReturnType::Success);
        };

        let target_to_guard = self.base.get_target_to_guard();
        if target_to_guard != crate::common::INVALID_ID {
            if let Some(target_arc) = get_legacy_object(target_to_guard) {
                if let Ok(target_guard) = target_arc.read() {
                    if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                        exit_guard.set_center(*target_guard.get_position());
                    }
                }
            }
        }

        Ok(attack_machine.update())
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
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
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.is_attack()
    }

    fn classic_is_busy(&self) -> bool {
        true
    }
}

/// Helper function to check if an object has attacked and can be retaliated against
pub fn has_attacked_me_and_i_can_return_fire(machine: &StateMachine) -> bool {
    if let Some(owner) = machine.get_owner() {
        if let Ok(owner_ref) = owner.try_read() {
            if let Some(body_module) = owner_ref.get_body_module() {
                if let Ok(mut body_guard) = body_module.lock() {
                    let last_attacker = body_guard.get_clearable_last_attacker();
                    if last_attacker == crate::common::INVALID_ID {
                        return false;
                    }

                    body_guard.clear_last_attacker();

                    let Some(target_arc) = TheGameLogic::find_object_by_id(last_attacker) else {
                        return false;
                    };
                    let Ok(target_guard) = target_arc.read() else {
                        return false;
                    };

                    if owner_ref.relationship_to(&target_guard) != Relationship::Enemies {
                        return false;
                    }

                    if target_guard.is_effectively_dead() {
                        return false;
                    }

                    matches!(
                        owner_ref.get_able_to_attack_specific_object(
                            AbleToAttackType::NewTarget,
                            &target_guard,
                            CommandSourceType::FromAi,
                        ),
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

fn guard_attack_aggressor_common<T: ClassicState>(state: &T) -> Result<bool, String> {
    let machine = state.base_state().get_machine()?;
    let guard = machine
        .lock()
        .map_err(|_| "guard state machine lock poisoned".to_string())?;
    Ok(has_attacked_me_and_i_can_return_fire(&guard))
}

fn guard_attack_aggressor_inner(
    state: &AIGuardInnerState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    guard_attack_aggressor_common(state)
}

fn guard_attack_aggressor_return(
    state: &AIGuardReturnState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    guard_attack_aggressor_common(state)
}

fn guard_attack_aggressor_idle(
    state: &AIGuardIdleState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    guard_attack_aggressor_common(state)
}
