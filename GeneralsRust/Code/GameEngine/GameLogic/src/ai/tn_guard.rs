use crate::ai::states::{
    AIEnterState, AIPickUpCrateState, AttackExitConditionsInterface, AttackStateMachine,
};
use crate::ai::{object_registry::get_legacy_object, vision_factors, GuardMode, THE_AI};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::coord::*;
use crate::common::xfer::{Xfer, XferExt, XferVersion};
use crate::common::CommandSourceType;
use crate::common::*;
use crate::helpers::{game_logic_random_value, TheGameLogic, ThePartitionManager};
use crate::modules::{AIUpdateInterfaceExt, ExitDoorType};
use crate::object::*;
use crate::player::Player;
use crate::state_machine::*;
use game_engine::common::system::Snapshotable;

use std::sync::{Arc, Mutex, RwLock, Weak};

/// Close enough distance constant
const CLOSE_ENOUGH: f32 = 25.0;

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

/// Tunnel network guard state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TNGuardStateType {
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

/// Exit conditions for tunnel network guard
#[derive(Debug, Clone)]
pub struct TunnelNetworkExitConditions {
    /// Frame at which we give up attacking
    attack_give_up_frame: u32,
}

impl TunnelNetworkExitConditions {
    pub fn new() -> Self {
        Self {
            attack_give_up_frame: 0,
        }
    }

    /// Check if should exit based on conditions
    pub fn should_exit(&self, _machine: &StateMachine) -> bool {
        let current_frame = TheGameLogic::get_frame();
        current_frame >= self.attack_give_up_frame
    }

    /// Set attack give up frame
    pub fn set_attack_give_up_frame(&mut self, frame: u32) {
        self.attack_give_up_frame = frame;
    }
}

#[derive(Debug, Clone)]
struct TunnelNetworkExitConditionsHandle {
    inner: Arc<Mutex<TunnelNetworkExitConditions>>,
}

impl TunnelNetworkExitConditionsHandle {
    fn new(inner: Arc<Mutex<TunnelNetworkExitConditions>>) -> Self {
        Self { inner }
    }
}

impl AttackExitConditionsInterface for TunnelNetworkExitConditionsHandle {
    fn should_exit(&self, machine: &StateMachine) -> bool {
        let Ok(guard) = self.inner.lock() else {
            return false;
        };
        guard.should_exit(machine)
    }
}

#[derive(Debug)]
pub struct TnGuardSharedState {
    machine: Weak<Mutex<StateMachine>>,
    guard_mode: Mutex<GuardMode>,
    position_to_guard: Mutex<Coord3D>,
    nemesis_to_attack: Mutex<ObjectID>,
}

impl TnGuardSharedState {
    fn new(machine: &Arc<Mutex<StateMachine>>) -> Self {
        Self {
            machine: Arc::downgrade(machine),
            guard_mode: Mutex::new(GuardMode::Normal),
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
            .ok_or_else(|| "tn guard state machine context lost".to_string())?;
        let mut guard = machine
            .lock()
            .map_err(|_| "tn guard state machine lock poisoned".to_string())?;
        Ok(f(&mut guard))
    }

    fn change_state(&self, state: TNGuardStateType) -> Result<(), String> {
        self.with_machine(|machine| {
            let _ = machine.set_current_state(state as u32);
        })
    }

    fn get_guard_mode(&self) -> GuardMode {
        self.guard_mode
            .lock()
            .map(|mode| *mode)
            .unwrap_or(GuardMode::Normal)
    }

    fn set_guard_mode(&self, guard_mode: GuardMode) {
        if let Ok(mut mode) = self.guard_mode.lock() {
            *mode = guard_mode;
        }
    }

    fn get_position_to_guard(&self) -> Coord3D {
        self.position_to_guard
            .lock()
            .map(|pos| *pos)
            .unwrap_or(Coord3D::new(0.0, 0.0, 0.0))
    }

    fn set_position_to_guard(&self, pos: Coord3D) {
        if let Ok(mut guard) = self.position_to_guard.lock() {
            *guard = pos;
        }
    }

    fn get_nemesis_to_attack(&self) -> ObjectID {
        self.nemesis_to_attack
            .lock()
            .map(|id| *id)
            .unwrap_or(crate::common::INVALID_ID)
    }

    fn set_nemesis_to_attack(&self, id: ObjectID) {
        if let Ok(mut guard) = self.nemesis_to_attack.lock() {
            *guard = id;
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
    fn tn_guard_shared_state_applies_transitions() {
        let machine = Arc::new(Mutex::new(StateMachine::new(
            Some(Weak::new()),
            "test_tn_guard",
        )));
        {
            let mut locked = machine.lock().unwrap();
            locked.define_state(
                TNGuardStateType::Inner as u32,
                Box::new(DummyState),
                None,
                None,
                None,
            );
            locked.define_state(
                TNGuardStateType::Idle as u32,
                Box::new(DummyState),
                None,
                None,
                None,
            );
        }

        let shared = TnGuardSharedState::new(&machine);
        shared.change_state(TNGuardStateType::Inner).unwrap();
        shared.change_state(TNGuardStateType::Idle).unwrap();

        let current = machine.lock().unwrap().get_current_state_id();
        assert_eq!(current, Some(TNGuardStateType::Idle as u32));
    }
}

/// Tunnel Network Guard state machine
#[derive(Debug)]
pub struct AITNGuardMachine {
    /// Base state machine
    base: Arc<Mutex<StateMachine>>,
    /// Shared state for tunnel guard states
    shared: Arc<TnGuardSharedState>,
    /// Position to guard
    position_to_guard: Coord3D,
    /// Nemesis to attack
    nemesis_to_attack: ObjectID,
    /// Guard mode
    guard_mode: GuardMode,
}

impl AITNGuardMachine {
    pub fn new(owner: Weak<RwLock<Object>>) -> Self {
        let base = Arc::new(Mutex::new(StateMachine::new(
            Some(owner),
            "AITNGuardMachine",
        )));
        let shared = Arc::new(TnGuardSharedState::new(&base));

        let mut machine = Self {
            base,
            shared,
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
            guard_mode: GuardMode::Normal,
        };

        machine.define_tn_guard_states();
        if let Ok(mut guard) = machine.base.lock() {
            let _ = guard.init_default_state();
        }
        machine
    }

    fn define_tn_guard_states(&mut self) {
        let shared = self.shared.clone();
        let base_arc = self.base.clone();

        let mut base = self
            .base
            .lock()
            .expect("tn guard state machine lock poisoned");
        let attack_aggressor_conditions_return = vec![StateConditionInfo::new(
            tn_guard_attack_aggressor_return,
            TNGuardStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];
        let attack_aggressor_conditions_inner = vec![StateConditionInfo::new(
            tn_guard_attack_aggressor_inner,
            TNGuardStateType::AttackAggressor as u32,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];

        base.define_state(
            TNGuardStateType::Return as u32,
            Box::new(AITNGuardReturnState::new(&base_arc, shared.clone())),
            Some(TNGuardStateType::Idle as u32),
            Some(TNGuardStateType::Inner as u32),
            Some(&attack_aggressor_conditions_return),
        );

        base.define_state(
            TNGuardStateType::Idle as u32,
            Box::new(AITNGuardIdleState::new(&base_arc, shared.clone())),
            Some(TNGuardStateType::Inner as u32),
            Some(TNGuardStateType::Return as u32),
            None,
        );

        base.define_state(
            TNGuardStateType::Inner as u32,
            Box::new(AITNGuardInnerState::new(&base_arc, shared.clone())),
            Some(TNGuardStateType::Outer as u32),
            Some(TNGuardStateType::Outer as u32),
            Some(&attack_aggressor_conditions_inner),
        );

        base.define_state(
            TNGuardStateType::Outer as u32,
            Box::new(AITNGuardOuterState::new(&base_arc, shared.clone())),
            Some(TNGuardStateType::GetCrate as u32),
            Some(TNGuardStateType::GetCrate as u32),
            None,
        );

        base.define_state(
            TNGuardStateType::GetCrate as u32,
            Box::new(AITNGuardPickUpCrateState::new(&base_arc, shared.clone())),
            Some(TNGuardStateType::Return as u32),
            Some(TNGuardStateType::Return as u32),
            None,
        );

        base.define_state(
            TNGuardStateType::AttackAggressor as u32,
            Box::new(AITNGuardAttackAggressorState::new(
                &base_arc,
                shared.clone(),
            )),
            Some(TNGuardStateType::Return as u32),
            Some(TNGuardStateType::Return as u32),
            None,
        );
    }

    /// Get position to guard
    pub fn get_position_to_guard(&self) -> &Coord3D {
        &self.position_to_guard
    }

    /// Set target position to guard
    pub fn set_target_position_to_guard(&mut self, pos: &Coord3D) {
        self.position_to_guard = *pos;
        self.shared.set_position_to_guard(*pos);
    }

    /// Set nemesis ID
    pub fn set_nemesis_id(&mut self, id: ObjectID) {
        self.nemesis_to_attack = id;
        self.shared.set_nemesis_to_attack(id);
        if let Ok(mut guard) = self.base.lock() {
            guard.set_goal_object_by_id(Some(id));
        }
    }

    /// Get nemesis ID
    pub fn get_nemesis_id(&self) -> ObjectID {
        self.nemesis_to_attack
    }

    /// Get guard mode
    pub fn get_guard_mode(&self) -> GuardMode {
        self.guard_mode
    }

    /// Set guard mode
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

    pub fn set_state(&mut self, state: TNGuardStateType) -> StateReturnType {
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

    /// Look for inner target within tunnel network
    pub fn look_for_inner_target(&mut self) -> bool {
        let owner = self
            .base
            .lock()
            .ok()
            .and_then(|machine| machine.get_owner());
        let Some(owner_arc) = owner else {
            return false;
        };
        let owner_id = owner_arc
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        let Some(target_id) = find_tunnel_network_inner_target(owner_id) else {
            return false;
        };
        self.set_nemesis_id(target_id);
        true
    }

    /// Get standard guard range
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
        let mut nemesis_to_attack = self.nemesis_to_attack;
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

        xfer.xfer_object_id(&mut self.nemesis_to_attack)
            .map_err(|e| format!("Failed to xfer nemesis_to_attack: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.x)
            .map_err(|e| format!("Failed to xfer position_to_guard.x: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.y)
            .map_err(|e| format!("Failed to xfer position_to_guard.y: {:?}", e))?;
        xfer.xfer_real(&mut self.position_to_guard.z)
            .map_err(|e| format!("Failed to xfer position_to_guard.z: {:?}", e))?;
        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        self.shared.set_nemesis_to_attack(self.nemesis_to_attack);
        self.shared.set_position_to_guard(self.position_to_guard);
        self.shared.set_guard_mode(self.guard_mode);
        Ok(())
    }
}

// State implementations for tunnel network guard

#[derive(Debug)]
struct TnGuardState {
    base: State,
    shared: Arc<TnGuardSharedState>,
}

impl TnGuardState {
    fn new(
        machine: &Arc<Mutex<StateMachine>>,
        shared: Arc<TnGuardSharedState>,
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

    fn change_state(&self, state: TNGuardStateType) -> Result<(), String> {
        self.shared.change_state(state)
    }

    fn guard_mode(&self) -> GuardMode {
        self.shared.get_guard_mode()
    }

    fn with_machine<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut StateMachine) -> R,
    {
        self.shared.with_machine(f)
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
}

/// Inner tunnel network guard state
#[derive(Debug)]
pub struct AITNGuardInnerState {
    base: TnGuardState,
    exit_conditions: Arc<Mutex<TunnelNetworkExitConditions>>,
    scan_for_enemy: bool,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AITNGuardInnerState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TnGuardSharedState>) -> Self {
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardInner"),
            exit_conditions: Arc::new(Mutex::new(TunnelNetworkExitConditions::new())),
            scan_for_enemy: true,
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }
}

impl StateImplementation for AITNGuardInnerState {
    fn on_enter(&mut self) -> StateReturnType {
        self.scan_for_enemy = true;

        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let mut nemesis_id = self
            .base
            .state()
            .get_machine_goal_object_id()
            .unwrap_or(crate::common::INVALID_ID);
        if nemesis_id == crate::common::INVALID_ID {
            let target_id = self.base.get_nemesis_to_attack();
            if target_id != crate::common::INVALID_ID {
                nemesis_id = target_id;
                let _ = self
                    .base
                    .with_machine(|machine| machine.set_goal_object_by_id(Some(target_id)));
            }
        }
        let Some(nemesis) = get_legacy_object(nemesis_id) else {
            return StateReturnType::Success;
        };

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AITNGuardAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(TunnelNetworkExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(nemesis_id));

        let return_val = attack_machine.init_default_state();
        self.is_attacking = matches!(return_val, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if return_val == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn update(&mut self) -> StateReturnType {
        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };

        let team_target = owner.read().ok().and_then(|owner_guard| {
            owner_guard.get_team().and_then(|team_arc| {
                team_arc
                    .read()
                    .ok()
                    .map(|team_guard| team_guard.get_team_target_object())
            })
        });
        let team_target_obj = team_target
            .filter(|id| *id != crate::common::INVALID_ID)
            .and_then(get_legacy_object);

        let mut goal_id = self
            .base
            .state()
            .get_machine_goal_object_id()
            .unwrap_or(crate::common::INVALID_ID);
        let mut goal_obj = if goal_id != crate::common::INVALID_ID {
            get_legacy_object(goal_id)
        } else {
            None
        };

        if goal_obj.is_none() {
            if let Some(target) = team_target_obj.as_ref() {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    exit_guard.set_attack_give_up_frame(
                        TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
                    );
                }
                self.base
                    .with_machine(|machine| {
                        machine.set_goal_object_by_id(target.read().ok().map(|g| g.get_id()))
                    })
                    .ok();
                self.base.set_nemesis_to_attack(
                    target
                        .read()
                        .map(|guard| guard.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                );
                goal_obj = Some(target.clone());
            }
        }

        if goal_obj.is_none() {
            let mut tunnel_nemesis: Option<Arc<RwLock<Object>>> = None;
            if let Ok(owner_guard) = owner.read() {
                if let Some(player_arc) = owner_guard.get_controlling_player() {
                    if let Ok(mut player_guard) = player_arc.write() {
                        if let Some(tunnels) = player_guard.get_tunnel_system_mut() {
                            if let Ok(Some(nemesis_id)) = tunnels.get_cur_nemesis_id() {
                                tunnel_nemesis = get_legacy_object(nemesis_id);
                            }
                        }
                    }
                }
            }

            if let Some(target) = tunnel_nemesis {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    exit_guard.set_attack_give_up_frame(
                        TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
                    );
                }
                self.base
                    .with_machine(|machine| {
                        machine.set_goal_object_by_id(target.read().ok().map(|g| g.get_id()))
                    })
                    .ok();
                self.base.set_nemesis_to_attack(
                    target
                        .read()
                        .map(|guard| guard.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                );
                goal_obj = Some(target);
            }
        }

        if goal_obj.is_none() && self.scan_for_enemy {
            self.scan_for_enemy = false;
            let owner_id = owner
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID);
            if let Some(target_id) = tunnel_network_scan(owner_id) {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    exit_guard.set_attack_give_up_frame(
                        TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
                    );
                }
                self.base
                    .with_machine(|machine| machine.set_goal_object_by_id(Some(target_id)))
                    .ok();
                self.base.set_nemesis_to_attack(target_id);

                if let Ok(owner_guard) = owner.read() {
                    if let Some(player_arc) = owner_guard.get_controlling_player() {
                        if let Ok(mut player_guard) = player_arc.write() {
                            if let Some(tunnels) = player_guard.get_tunnel_system_mut() {
                                if let Some(target) = get_legacy_object(target_id) {
                                    if let Ok(target_guard) = target.read() {
                                        let _ = tunnels.update_nemesis(Some(&target_guard));
                                    }
                                }
                            }
                        }
                    }
                }

                if let Some(target) = get_legacy_object(target_id) {
                    goal_obj = Some(target);
                }
            }
        } else if let (Some(goal), Some(team_target)) = (&goal_obj, &team_target_obj) {
            if goal.read().ok().map(|g| g.get_id()) != team_target.read().ok().map(|t| t.get_id()) {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(player_arc) = owner_guard.get_controlling_player() {
                        if let Ok(mut player_guard) = player_arc.write() {
                            if let Some(tunnels) = player_guard.get_tunnel_system_mut() {
                                if let Ok(goal_guard) = goal.read() {
                                    let _ = tunnels.update_nemesis(Some(&goal_guard));
                                }
                            }
                        }
                    }
                }
                self.base
                    .with_machine(|machine| {
                        machine.set_goal_object_by_id(team_target.read().ok().map(|g| g.get_id()))
                    })
                    .ok();
                self.base.set_nemesis_to_attack(
                    team_target
                        .read()
                        .map(|guard| guard.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                );
                goal_obj = Some(team_target.clone());
            }
        }

        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };

        if let Some(goal) = goal_obj.as_ref() {
            attack_machine.set_goal_object(goal.read().ok().map(|g| g.get_id()));
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

/// Idle tunnel network guard state
#[derive(Debug)]
pub struct AITNGuardIdleState {
    base: TnGuardState,
    next_enemy_scan_time: u32,
}

impl AITNGuardIdleState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TnGuardSharedState>) -> Self {
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardIdleState"),
            next_enemy_scan_time: 0,
        }
    }

    pub fn is_guard_idle(&self) -> bool {
        true
    }
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        let mut next_enemy_scan_time = self.next_enemy_scan_time;
        xfer.xfer_unsigned_int(&mut next_enemy_scan_time)
            .map_err(|e| format!("Failed to crc next_enemy_scan_time: {:?}", e))?;
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.next_enemy_scan_time)
            .map_err(|e| format!("Failed to xfer next_enemy_scan_time: {:?}", e))?;
        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
impl StateImplementation for AITNGuardIdleState {
    fn on_enter(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_scan_rate();
        self.next_enemy_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));
        let _ = self
            .base
            .with_machine(|machine| machine.set_goal_object_by_id(None));
        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if now < self.next_enemy_scan_time {
            return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
        }

        self.next_enemy_scan_time = now.saturating_add(get_guard_enemy_scan_rate());
        let _ = self
            .base
            .with_machine(|machine| machine.set_goal_object_by_id(None));

        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
        };

        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if ai_guard.get_crate_id() != crate::common::INVALID_ID {
                        let _ = self.base.with_machine(|machine| {
                            machine.set_current_state(TNGuardStateType::GetCrate as u32)
                        });
                        return StateReturnType::Sleep(
                            self.next_enemy_scan_time.saturating_sub(now),
                        );
                    }
                }
            }
        }

        let owner_id = owner
            .read()
            .ok()
            .map(|g| g.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        if let Some(target_id) = find_tunnel_network_inner_target(owner_id) {
            self.base.set_nemesis_to_attack(target_id);

            if let Some(target) = get_legacy_object(target_id) {
                let _ = self
                    .base
                    .with_machine(|machine| machine.set_goal_object_by_id(Some(target_id)));

                if let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) {
                    if owner_guard.get_contained_by().is_some() {
                        if let Some(player_arc) = owner_guard.get_controlling_player() {
                            if let Ok(player_guard) = player_arc.read() {
                                if let Some(best_tunnel_id) =
                                    find_best_tunnel(&player_guard, target_guard.get_position())
                                {
                                    let Some(best_tunnel) = get_legacy_object(best_tunnel_id)
                                    else {
                                        return StateReturnType::Sleep(0);
                                    };
                                    let Ok(tunnel_guard) = best_tunnel.read() else {
                                        return StateReturnType::Sleep(0);
                                    };
                                    let Some(exit_interface) =
                                        tunnel_guard.get_object_exit_interface()
                                    else {
                                        return StateReturnType::Failure;
                                    };
                                    let Ok(mut exit_guard) = exit_interface.lock() else {
                                        return StateReturnType::Sleep(0);
                                    };
                                    let door = exit_guard.reserve_door_for_exit(
                                        Some(&*tunnel_guard),
                                        Some(&*owner_guard),
                                    );
                                    if door == ExitDoorType::NoneAvailable {
                                        return StateReturnType::Sleep(0);
                                    }
                                    let _ = exit_guard.exit_object_via_door(
                                        owner.read().map(|g| g.get_id()).unwrap_or(0),
                                        door,
                                    );
                                    return StateReturnType::Sleep(0);
                                }
                            }
                        }
                    }
                }
            } else {
                return StateReturnType::Sleep(0);
            }

            return StateReturnType::Success;
        }

        if let Ok(owner_guard) = owner.read() {
            if owner_guard.get_contained_by().is_none() {
                if let Some(player_arc) = owner_guard.get_controlling_player() {
                    if let Ok(player_guard) = player_arc.read() {
                        let pos = *owner_guard.get_position();
                        if find_best_tunnel(&player_guard, &pos).is_some() {
                            return StateReturnType::Failure;
                        }
                    }
                }
            }
        }

        StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now))
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // Cleanup when exiting idle state
    }
}

/// Outer tunnel network guard state
#[derive(Debug)]
pub struct AITNGuardOuterState {
    base: TnGuardState,
    exit_conditions: Arc<Mutex<TunnelNetworkExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AITNGuardOuterState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TnGuardSharedState>) -> Self {
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardOuter"),
            exit_conditions: Arc::new(Mutex::new(TunnelNetworkExitConditions::new())),
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }
}

impl StateImplementation for AITNGuardOuterState {
    fn on_enter(&mut self) -> StateReturnType {
        if matches!(self.base.guard_mode(), GuardMode::GuardWithoutPursuit) {
            // GUARDMODE_GUARD_WITHOUT_PURSUIT: patrol mode does not chase outside guard area.
            return StateReturnType::Success;
        }

        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };
        let Some(nemesis_id) = self.base.state().get_machine_goal_object_id() else {
            return StateReturnType::Success;
        };
        let Some(nemesis) = get_legacy_object(nemesis_id) else {
            return StateReturnType::Success;
        };

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AITNGuardAttackMachine",
            false,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(TunnelNetworkExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(nemesis_id));

        let return_val = attack_machine.init_default_state();
        self.is_attacking = matches!(return_val, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if return_val == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn update(&mut self) -> StateReturnType {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };

        let mut goal_id = self
            .base
            .state()
            .get_machine_goal_object_id()
            .unwrap_or(crate::common::INVALID_ID);
        let mut goal_obj = if goal_id != crate::common::INVALID_ID {
            get_legacy_object(goal_id)
        } else {
            None
        };
        if goal_obj.is_none() {
            if let Some(owner) = self.base.state().get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    let nemesis_id = self.base.get_nemesis_to_attack();
                    if nemesis_id != crate::common::INVALID_ID {
                        goal_obj = get_legacy_object(nemesis_id);
                    }

                    let mut team_target = None;
                    if let Some(team_arc) = owner_guard.get_team() {
                        if let Ok(team_guard) = team_arc.read() {
                            if goal_obj.is_none() && team_guard.attack_common_target() {
                                let target_id = team_guard.get_team_target_object();
                                if target_id != crate::common::INVALID_ID {
                                    team_target = get_legacy_object(target_id);
                                }
                            }
                        }
                    }

                    if let Some(target) = team_target {
                        self.base
                            .with_machine(|machine| {
                                machine
                                    .set_goal_object_by_id(target.read().ok().map(|g| g.get_id()))
                            })
                            .ok();
                        self.base.set_nemesis_to_attack(
                            target
                                .read()
                                .map(|guard| guard.get_id())
                                .unwrap_or(crate::common::INVALID_ID),
                        );
                        goal_obj = Some(target);
                        let _ = attack_machine.init_default_state();
                    } else if let Some(target) = goal_obj.as_ref() {
                        self.base
                            .with_machine(|machine| {
                                machine
                                    .set_goal_object_by_id(target.read().ok().map(|g| g.get_id()))
                            })
                            .ok();
                    }
                }
            }
        }

        if let Some(goal) = goal_obj.as_ref() {
            attack_machine.set_goal_object(goal.read().ok().map(|g| g.get_id()));
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

/// Return tunnel network guard state
#[derive(Debug)]
pub struct AITNGuardReturnState {
    base: TnGuardState,
    enter_state: AIEnterState,
    next_return_scan_time: u32,
}

impl AITNGuardReturnState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TnGuardSharedState>) -> Self {
        let enter_state = {
            let guard = machine
                .lock()
                .expect("tn guard state machine lock poisoned while creating return state");
            AIEnterState::new(&guard)
        };
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardReturn"),
            enter_state,
            next_return_scan_time: 0,
        }
    }
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        Snapshotable::crc(&self.enter_state, xfer)?;
        let mut next_return_scan_time = self.next_return_scan_time;
        xfer.xfer_unsigned_int(&mut next_return_scan_time)
            .map_err(|e| format!("Failed to crc next_return_scan_time: {:?}", e))?;
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        Snapshotable::xfer(&mut self.enter_state, xfer)?;
        xfer.xfer_unsigned_int(&mut self.next_return_scan_time)
            .map_err(|e| format!("Failed to xfer next_return_scan_time: {:?}", e))?;
        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.enter_state)?;
        Ok(())
    }
}

impl StateImplementation for AITNGuardReturnState {
    fn on_enter(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_return_scan_rate();
        self.next_return_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };

        if let Ok(owner_guard) = owner.read() {
            if owner_guard.get_contained_by().is_some() {
                return StateReturnType::Success;
            }

            if let Some(team_arc) = owner_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    let target_id = team_guard.get_team_target_object();
                    if target_id != crate::common::INVALID_ID {
                        self.base.set_nemesis_to_attack(target_id);
                        if let Some(target) = get_legacy_object(target_id) {
                            let _ = self.base.with_machine(|machine| {
                                machine
                                    .set_goal_object_by_id(target.read().ok().map(|g| g.get_id()))
                            });
                        }
                        return StateReturnType::Failure;
                    }
                }
            }

            if let Some(player_arc) = owner_guard.get_controlling_player() {
                if let Ok(player_guard) = player_arc.read() {
                    let pos = *owner_guard.get_position();
                    if let Some(best_tunnel_id) = find_best_tunnel(&player_guard, &pos) {
                        let _ = self.base.with_machine(|machine| {
                            machine.set_goal_object_by_id(Some(best_tunnel_id))
                        });
                        self.base.set_nemesis_to_attack(crate::common::INVALID_ID);
                        return self.enter_state.on_enter();
                    }
                }
            }
        }

        StateReturnType::Failure
    }

    fn update(&mut self) -> StateReturnType {
        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };

        if let Ok(owner_guard) = owner.read() {
            if let Some(team_arc) = owner_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    let target_id = team_guard.get_team_target_object();
                    if target_id != crate::common::INVALID_ID {
                        self.base.set_nemesis_to_attack(target_id);
                        if let Some(target) = get_legacy_object(target_id) {
                            let _ = self.base.with_machine(|machine| {
                                machine
                                    .set_goal_object_by_id(target.read().ok().map(|g| g.get_id()))
                            });
                        }
                        return StateReturnType::Failure;
                    }
                }
            }

            if let Some(player_arc) = owner_guard.get_controlling_player() {
                if let Ok(mut player_guard) = player_arc.write() {
                    if let Some(tunnels) = player_guard.get_tunnel_system_mut() {
                        if let Ok(Some(nemesis_id)) = tunnels.get_cur_nemesis_id() {
                            self.base.set_nemesis_to_attack(nemesis_id);
                            let _ = self.base.with_machine(|machine| {
                                machine.set_goal_object_by_id(Some(nemesis_id))
                            });
                        }
                        return StateReturnType::Failure;
                    }
                }
            }
        }

        let ret = self.enter_state.update();
        if ret == StateReturnType::Continue {
            return StateReturnType::Continue;
        }
        StateReturnType::Success
    }

    fn on_exit(&mut self, status: StateExitType) {
        self.enter_state.on_exit(status);
    }
}

/// Pick up crate state for tunnel network guard
#[derive(Debug)]
pub struct AITNGuardPickUpCrateState {
    base: TnGuardState,
    pick_up_state: AIPickUpCrateState,
}

impl AITNGuardPickUpCrateState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TnGuardSharedState>) -> Self {
        let pick_up_state = {
            let guard = machine
                .lock()
                .expect("tn guard state machine lock poisoned while creating crate state");
            AIPickUpCrateState::new(&guard)
        };
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardPickUpCrate"),
            pick_up_state,
        }
    }
}

impl StateImplementation for AITNGuardPickUpCrateState {
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

        let _ = self.base.with_machine(|machine| {
            machine.set_goal_object_by_id(crate_obj.read().ok().map(|g| g.get_id()))
        });
        self.pick_up_state.on_enter()
    }

    fn update(&mut self) -> StateReturnType {
        self.pick_up_state.update()
    }

    fn on_exit(&mut self, status: StateExitType) {
        self.pick_up_state.on_exit(status);
    }
}

/// Attack aggressor state for tunnel network guard
#[derive(Debug)]
pub struct AITNGuardAttackAggressorState {
    base: TnGuardState,
    exit_conditions: Arc<Mutex<TunnelNetworkExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AITNGuardAttackAggressorState {
    pub fn new(machine: &Arc<Mutex<StateMachine>>, shared: Arc<TnGuardSharedState>) -> Self {
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardAttackAggressor"),
            exit_conditions: Arc::new(Mutex::new(TunnelNetworkExitConditions::new())),
            is_attacking: false,
            attack_machine: None,
        }
    }

    pub fn is_attack(&self) -> bool {
        self.is_attacking
    }
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }
}

impl StateImplementation for AITNGuardAttackAggressorState {
    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.state().get_machine_owner() else {
            return StateReturnType::Failure;
        };

        let mut nemesis_id = self
            .base
            .state()
            .get_machine_goal_object_id()
            .unwrap_or(crate::common::INVALID_ID);
        if nemesis_id == crate::common::INVALID_ID {
            let id = self.base.get_nemesis_to_attack();
            if id != crate::common::INVALID_ID {
                nemesis_id = id;
                self.base
                    .with_machine(|machine| machine.set_goal_object_by_id(Some(id)))
                    .ok();
            }
        }
        if nemesis_id == crate::common::INVALID_ID {
            if let Ok(owner_guard) = owner.read() {
                if let Some(body) = owner_guard.get_body_module() {
                    if let Ok(body_guard) = body.lock() {
                        if let Some(info) = body_guard.get_last_damage_info() {
                            if info.source_id != crate::common::INVALID_ID {
                                nemesis_id = info.source_id;
                                self.base.set_nemesis_to_attack(info.source_id);
                                self.base
                                    .with_machine(|machine| {
                                        machine.set_goal_object_by_id(Some(info.source_id))
                                    })
                                    .ok();
                            }
                        }
                    }
                }
            }
        }
        let mut nemesis = if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        };

        let Some(nemesis) = nemesis else {
            return StateReturnType::Success;
        };
        self.base.set_nemesis_to_attack(
            nemesis
                .read()
                .map(|guard| guard.get_id())
                .unwrap_or(crate::common::INVALID_ID),
        );

        if let Ok(owner_guard) = owner.read() {
            if let Some(player_arc) = owner_guard.get_controlling_player() {
                if let Ok(mut player_guard) = player_arc.write() {
                    if let Some(tunnels) = player_guard.get_tunnel_system_mut() {
                        if let Ok(nemesis_guard) = nemesis.read() {
                            let _ = tunnels.update_nemesis(Some(&nemesis_guard));
                        }
                    }
                }
            }
        }

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
        }

        let mut attack_machine = AttackStateMachine::new(
            Arc::downgrade(&owner),
            "AITNGuardAttackMachine",
            true,
            true,
            false,
        );
        attack_machine.set_exit_conditions(Box::new(TunnelNetworkExitConditionsHandle::new(
            self.exit_conditions.clone(),
        )));
        attack_machine.set_goal_object(Some(nemesis_id));

        let return_val = attack_machine.init_default_state();
        self.is_attacking = matches!(return_val, StateReturnType::Continue);
        self.attack_machine = Some(attack_machine);

        if return_val == StateReturnType::Continue {
            StateReturnType::Continue
        } else {
            StateReturnType::Success
        }
    }

    fn update(&mut self) -> StateReturnType {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };

        if let Some(goal_id) = self.base.state().get_machine_goal_object_id() {
            self.base.set_nemesis_to_attack(goal_id);
            if let Some(owner) = self.base.state().get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(player_arc) = owner_guard.get_controlling_player() {
                        if let Ok(mut player_guard) = player_arc.write() {
                            if let Some(tunnels) = player_guard.get_tunnel_system_mut() {
                                if let Some(goal) = get_legacy_object(goal_id) {
                                    if let Ok(goal_guard) = goal.read() {
                                        let _ = tunnels.update_nemesis(Some(&goal_guard));
                                    }
                                }
                            }
                        }
                    }
                }
            }
            attack_machine.set_goal_object(Some(goal_id));
        }

        attack_machine.update()
    }

    fn on_exit(&mut self, _status: StateExitType) {
        if let Some(mut machine) = self.attack_machine.take() {
            let _ = machine.halt();
        }
        if let Some(owner) = self.base.state().get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(crate::common::INVALID_ID);
                    }
                }
            }
        }
        self.is_attacking = false;
    }
}

fn find_tunnel_network_inner_target(owner_id: ObjectID) -> Option<ObjectID> {
    let owner = TheGameLogic::find_object_by_id(owner_id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(owner_id))?;
    let owner_guard = owner.read().ok()?;

    if let Some(team_arc) = owner_guard.get_team() {
        if let Ok(team_guard) = team_arc.read() {
            if team_guard.attack_common_target() {
                let team_target = team_guard.get_team_target_object();
                if team_target != crate::common::INVALID_ID
                    && TheGameLogic::find_object_by_id(team_target).is_some()
                {
                    return Some(team_target);
                }
            }
        }
    }

    let player_arc = owner_guard.get_controlling_player()?;
    let mut player_guard = player_arc.write().ok()?;
    let tunnels = player_guard.get_tunnel_system_mut()?;

    if let Ok(Some(nemesis_id)) = tunnels.get_cur_nemesis_id() {
        return Some(nemesis_id);
    }

    let container_list = tunnels.get_container_list().ok()?;
    for tunnel_id in container_list {
        let Some(tunnel_arc) = TheGameLogic::find_object_by_id(tunnel_id) else {
            continue;
        };
        let Ok(tunnel_guard) = tunnel_arc.read() else {
            continue;
        };

        if let Some(ai) = tunnel_guard.get_ai_update_interface() {
            if let Ok(ai_guard) = ai.lock() {
                let victim_id = ai_guard.get_goal_object_id();
                if victim_id != crate::common::INVALID_ID {
                    if let Some(is_enemy) = crate::object::registry::OBJECT_REGISTRY.with_object(
                        victim_id,
                        |victim_guard| {
                            owner_guard.relationship_to(victim_guard) == Relationship::Enemies
                        },
                    ) {
                        if is_enemy {
                            return Some(victim_id);
                        }
                    }
                }
            }
        }

        let Some(body) = tunnel_guard.get_body_module() else {
            continue;
        };
        let Ok(body_guard) = body.lock() else {
            continue;
        };
        let Some(info) = body_guard.get_last_damage_info() else {
            continue;
        };
        if info.output.no_effect {
            continue;
        }
        let scan_rate = get_guard_enemy_scan_rate();
        if body_guard.get_last_damage_timestamp() + scan_rate <= TheGameLogic::get_frame() {
            continue;
        }

        let attacker_id = info.source_id;
        let Some(attacker) = TheGameLogic::find_object_by_id(attacker_id) else {
            continue;
        };
        let Ok(attacker_guard) = attacker.read() else {
            continue;
        };
        if owner_guard.relationship_to(&attacker_guard) != Relationship::Enemies {
            continue;
        }
        let can_attack = matches!(
            owner_guard.get_able_to_attack_specific_object(
                AbleToAttackType::NewTarget,
                &attacker_guard,
                CommandSourceType::FromAi,
            ),
            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
        );
        if !can_attack {
            continue;
        }

        if let Some(team_arc) = owner_guard.get_team() {
            if let Ok(mut team_guard) = team_arc.write() {
                team_guard.set_team_target_object(attacker_id);
            }
        }
        let _ = tunnels.update_nemesis(Some(&attacker_guard));
        return Some(attacker_id);
    }

    None
}

fn tunnel_network_scan(owner_id: ObjectID) -> Option<ObjectID> {
    let partition = ThePartitionManager::get()?;
    let owner = TheGameLogic::find_object_by_id(owner_id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(owner_id))?;
    let owner_guard = owner.read().ok()?;
    let vision_range = AITNGuardMachine::get_std_guard_range(owner_id);
    let owner_pos = *owner_guard.get_position();

    partition.get_closest_object_2d(&owner_pos, vision_range, |candidate| {
        if candidate.get_id() == owner_id {
            return false;
        }
        if candidate.is_effectively_dead() {
            return false;
        }
        if candidate.is_off_map() {
            return false;
        }
        if owner_guard.relationship_to(candidate) != Relationship::Enemies {
            return false;
        }
        if candidate.is_stealthed() && !candidate.is_detected() {
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

/// Helper function to find best tunnel for a position
pub fn find_best_tunnel(owner_player: &Player, pos: &Coord3D) -> Option<ObjectID> {
    let tunnels = owner_player.get_tunnel_system()?;
    let list = tunnels.get_container_list().ok()?;

    let mut best: Option<(ObjectID, Real)> = None;
    for tunnel_id in list {
        let Some(tunnel_arc) = TheGameLogic::find_object_by_id(tunnel_id) else {
            continue;
        };
        let Ok(tunnel_guard) = tunnel_arc.read() else {
            continue;
        };
        let tunnel_pos = *tunnel_guard.get_position();
        let delta = tunnel_pos - *pos;
        let dist_sqr = delta.length_squared();
        let better = best
            .as_ref()
            .map(|(_, best_dist)| dist_sqr < *best_dist)
            .unwrap_or(true);
        if better {
            best = Some((tunnel_id, dist_sqr));
        }
    }

    best.map(|(id, _)| id)
}

/// Helper function to check if an object has attacked and can be retaliated against
/// through the tunnel network
pub fn has_attacked_me_and_i_can_return_fire_tn(machine: &StateMachine) -> bool {
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

fn tn_guard_attack_aggressor_return(
    state: &dyn StateImplementation,
    _user_data: &StateTransitionUserData,
) -> bool {
    let Ok(machine) = state.get_machine() else {
        return false;
    };
    let Ok(guard) = machine.lock() else {
        return false;
    };
    has_attacked_me_and_i_can_return_fire_tn(&guard)
}

fn tn_guard_attack_aggressor_inner(
    state: &dyn StateImplementation,
    _user_data: &StateTransitionUserData,
) -> bool {
    let Ok(machine) = state.get_machine() else {
        return false;
    };
    let Ok(guard) = machine.lock() else {
        return false;
    };
    has_attacked_me_and_i_can_return_fire_tn(&guard)
}
