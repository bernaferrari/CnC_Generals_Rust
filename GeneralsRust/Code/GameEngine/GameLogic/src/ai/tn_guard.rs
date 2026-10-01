use crate::ai::states::{
    AIEnterState, AIPickUpCrateState, AttackExitConditionsInterface, AttackStateMachine,
};
use crate::ai::{GuardMode, object_registry::get_legacy_object, the_ai, vision_factors};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::common::CommandSourceType;
use crate::common::coord::*;
use crate::common::xfer::{Xfer, XferExt, XferVersion};
use crate::common::*;
use crate::helpers::{TheGameLogic, ThePartitionManager, game_logic_random_value};
use crate::modules::{AIUpdateInterfaceExt, ExitDoorType};
use crate::object::*;
use crate::player::Player;
use crate::state_machine::*;
use game_engine::common::system::Snapshotable;

use std::sync::{Arc, Mutex, RwLock, Weak};

/// Wave 375: leftover AITNGuard must run even when the dual-world factory is empty.
/// Host-only play still writes/consumes TunnelTracker nemesis (C++ AITNGuard.cpp:168-239).
#[inline]
fn dual_world_registry_unavailable() -> bool {
    let _host_empty = crate::object::registry::OBJECT_REGISTRY.is_empty();
    false
}

/// Close enough distance constant
const CLOSE_ENOUGH: f32 = 25.0;

fn clear_attack_state_on_exit(owner_id: ObjectID) {
    let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
    owner_guard.clear_status(
        ObjectStatusMaskType::IS_FIRING_WEAPON
            | ObjectStatusMaskType::IS_AIMING_WEAPON
            | ObjectStatusMaskType::IS_ATTACKING
            | ObjectStatusMaskType::IGNORING_STEALTH,
    );
    owner_guard.clear_model_condition_state(ModelConditionFlags::ATTACKING);
    owner_guard.clear_leech_range_mode_for_all_weapons();
    if let Some(ai) = owner_guard.get_ai_update_interface() {
        if let Ok(mut ai_guard) = ai.lock() {
            ai_guard.set_current_victim(None);
            for turret in [TurretType::Primary, TurretType::Secondary] {
                ai_guard.set_turret_target_object(turret, None, false);
            }
            ai_guard.set_goal_object(None);
        }
    }
    });
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
    owner_id: ObjectID,
    fields: Mutex<TnGuardSharedFields>,
}

#[derive(Debug)]
struct TnGuardSharedFields {
    guard_mode: GuardMode,
    position_to_guard: Coord3D,
    nemesis_to_attack: ObjectID,
    pending_state: Option<u32>,
}

impl TnGuardSharedState {
    fn new(owner_id: ObjectID) -> Self {
        Self {
            owner_id,
            fields: Mutex::new(TnGuardSharedFields {
                guard_mode: GuardMode::Normal,
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

    /// Request a transition; the owning machine applies it after the child
    /// update returns, preserving the legacy callback order.
    #[allow(dead_code)]
    fn request_tn_state(&self, state: TNGuardStateType) {
        self.request_state(state as u32);
    }
    fn get_guard_mode(&self) -> GuardMode {
        self.fields
            .lock()
            .map(|fields| fields.guard_mode)
            .unwrap_or(GuardMode::Normal)
    }

    fn set_guard_mode(&self, guard_mode: GuardMode) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.guard_mode = guard_mode;
        }
    }

    fn get_position_to_guard(&self) -> Coord3D {
        self.fields
            .lock()
            .map(|fields| fields.position_to_guard)
            .unwrap_or(Coord3D::new(0.0, 0.0, 0.0))
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

    fn sync_from_machine(
        &self,
        nemesis_to_attack: ObjectID,
        position_to_guard: Coord3D,
        guard_mode: GuardMode,
    ) {
        if let Ok(mut fields) = self.fields.lock() {
            fields.nemesis_to_attack = nemesis_to_attack;
            fields.position_to_guard = position_to_guard;
            fields.guard_mode = guard_mode;
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
        let mut machine = StateMachine::new(Some(Weak::new()), "test_tn_guard");
        machine.define_state(
            TNGuardStateType::Inner as u32,
            Box::new(DummyState),
            None,
            None,
            None,
        );
        machine.define_state(
            TNGuardStateType::Idle as u32,
            Box::new(DummyState),
            None,
            None,
            None,
        );

        let shared = TnGuardSharedState::new(Weak::new());
        shared.request_state(TNGuardStateType::Inner as u32);
        let _ = machine.set_current_state(shared.take_pending_state().unwrap());
        shared.request_state(TNGuardStateType::Idle as u32);
        let _ = machine.set_current_state(shared.take_pending_state().unwrap());

        let current = machine.get_current_state_id();
        assert_eq!(current, Some(TNGuardStateType::Idle as u32));
    }

    #[test]
    fn tn_guard_shared_fields_are_isolated_per_instance_and_shared_with_states() {
        let shared_a = Arc::new(TnGuardSharedState::new(Weak::new()));
        let shared_b = Arc::new(TnGuardSharedState::new(Weak::new()));
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

/// Tunnel Network Guard state machine
#[derive(Debug)]
pub struct AITNGuardMachine {
    /// Base state machine, owned by this machine.
    base: StateMachine,
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
    pub fn new(owner_id: ObjectID) -> Self {
        let base = StateMachine::new_with_owner_id(owner_id, "AITNGuardMachine");
        let shared = Arc::new(TnGuardSharedState::new(owner_id));

        let mut machine = Self {
            base,
            shared,
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
            guard_mode: GuardMode::Normal,
        };

        machine.define_tn_guard_states();
        let _ = machine.base.init_default_state();
        machine
    }

    fn define_tn_guard_states(&mut self) {
        let shared = self.shared.clone();

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

        let return_state = AITNGuardReturnState::new(&self.base, shared.clone());
        self.base.define_state(
            TNGuardStateType::Return as u32,
            Box::new(return_state),
            Some(TNGuardStateType::Idle as u32),
            Some(TNGuardStateType::Inner as u32),
            Some(&attack_aggressor_conditions_return),
        );

        let idle_state = AITNGuardIdleState::new(&self.base, shared.clone());
        self.base.define_state(
            TNGuardStateType::Idle as u32,
            Box::new(idle_state),
            Some(TNGuardStateType::Inner as u32),
            Some(TNGuardStateType::Return as u32),
            None,
        );

        let inner_state = AITNGuardInnerState::new(&self.base, shared.clone());
        self.base.define_state(
            TNGuardStateType::Inner as u32,
            Box::new(inner_state),
            Some(TNGuardStateType::Outer as u32),
            Some(TNGuardStateType::Outer as u32),
            Some(&attack_aggressor_conditions_inner),
        );

        let outer_state = AITNGuardOuterState::new(&self.base, shared.clone());
        self.base.define_state(
            TNGuardStateType::Outer as u32,
            Box::new(outer_state),
            Some(TNGuardStateType::GetCrate as u32),
            Some(TNGuardStateType::GetCrate as u32),
            None,
        );

        let crate_state = AITNGuardPickUpCrateState::new(&self.base, shared.clone());
        self.base.define_state(
            TNGuardStateType::GetCrate as u32,
            Box::new(crate_state),
            Some(TNGuardStateType::Return as u32),
            Some(TNGuardStateType::Return as u32),
            None,
        );

        let aggressor_state = AITNGuardAttackAggressorState::new(&self.base, shared.clone());
        self.base.define_state(
            TNGuardStateType::AttackAggressor as u32,
            Box::new(aggressor_state),
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
        self.base.set_goal_object_by_id(Some(id));
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
        self.base.init_default_state()
    }

    pub fn set_state(&mut self, state: TNGuardStateType) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.base.is_in_guard_idle_state()
    }

    pub fn update(&mut self) -> StateReturnType {
        let result = self.base.update();
        if let Some(state_id) = self.shared.take_pending_state() {
            let _ = self.base.set_current_state(state_id);
        }
        result
    }

    /// Look for inner target within tunnel network
    pub fn look_for_inner_target(&mut self) -> bool {
        let Some(owner_id) = self.base.get_owner() else {
            return false;
        };
        let Some(target_id) = find_tunnel_network_inner_target(owner_id) else {
            return false;
        };
        self.set_nemesis_id(target_id);
        true
    }

    /// Get standard guard range
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
        // C++ AITNGuardMachine::crc is empty.
        Ok(())
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 2;
        xfer.xfer_version(&mut version, 2)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        if version >= 2 {
            self.base.xfer(xfer).map_err(|e| e.to_string())?;
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
        self.shared.sync_from_machine(
            self.nemesis_to_attack,
            self.position_to_guard,
            self.guard_mode,
        );
        self.base
            .load_post_process()
            .map_err(|e| format!("tn guard load_post_process: {e}"))
    }
}

// State implementations for tunnel network guard

#[derive(Debug)]
struct TnGuardState {
    base: State,
    shared: Arc<TnGuardSharedState>,
}

impl TnGuardState {
    fn new(machine: &StateMachine, shared: Arc<TnGuardSharedState>, name: &str) -> Self {
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



    fn guard_mode(&self) -> GuardMode {
        self.shared.get_guard_mode()
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
    pub fn new(machine: &StateMachine, shared: Arc<TnGuardSharedState>) -> Self {
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
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn on_enter(&mut self) -> StateReturnType {
        self.scan_for_enemy = true;

        let Some(owner) = self.base.shared.owner() else {
            return StateReturnType::Failure;
        };
        let nemesis_id = self.base.get_nemesis_to_attack();
        let Some(nemesis) = get_legacy_object(nemesis_id) else {
            return StateReturnType::Success;
        };

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
        }

        let mut attack_machine = AttackStateMachine::with_owner_id(
            owner,
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
        let Some(owner) = self.base.shared.owner() else {
            return StateReturnType::Failure;
        };

        let team_target = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            owner_guard.get_team().and_then(|team_arc| {
                team_arc.read().ok().map(|team_guard| team_guard.get_team_target_object())
            })
        }).flatten();
        let team_target_obj = team_target
            .filter(|id| *id != crate::common::INVALID_ID)
            .and_then(get_legacy_object);

        let mut goal_id = self.base.get_nemesis_to_attack();
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
                self.base.set_nemesis_to_attack(*target);
                return StateReturnType::Continue;
            }
        }

        if goal_obj.is_none() {
            let tunnel_nemesis = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                let player_arc = owner_guard.get_controlling_player()?;
                let mut player_guard = player_arc.write().ok()?;
                let tunnels = player_guard.get_tunnel_system_mut()?;
                let nemesis_id = tunnels.get_cur_nemesis_id().ok().flatten()?;
                get_legacy_object(nemesis_id)
            }).flatten();

            if let Some(target) = tunnel_nemesis {
                if let Ok(mut exit_guard) = self.exit_conditions.lock() {
                    exit_guard.set_attack_give_up_frame(
                        TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
                    );
                }
                self.base.set_nemesis_to_attack(*target);
                return StateReturnType::Continue;
            }
        }

        if goal_obj.is_none() && self.scan_for_enemy {
            self.scan_for_enemy = false;
            if let Some(target_id) = tunnel_network_scan(owner) {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                    let player_arc = owner_guard.get_controlling_player()?;
                    let mut player_guard = player_arc.write().ok()?;
                    let tunnels = player_guard.get_tunnel_system_mut()?;
                    crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target_guard| {
                        let _ = tunnels.update_nemesis(Some(target_guard));
                    });
                    Some(())
                });
                self.attack_machine = None;
                clear_attack_state_on_exit(owner);
                let mut attack_machine = AttackStateMachine::with_owner_id(
                    owner,
                    "AITNGuardAttackMachine",
                    false,
                    true,
                    false,
                );
                attack_machine.set_exit_conditions(Box::new(
                    TunnelNetworkExitConditionsHandle::new(self.exit_conditions.clone()),
                ));
                attack_machine.set_goal_object(Some(target_id));
                let return_val = attack_machine.init_default_state();
                self.is_attacking = matches!(return_val, StateReturnType::Continue);
                self.attack_machine = Some(attack_machine);
                return return_val;
            }
        } else if let (Some(goal), Some(team_target)) = (goal_obj, team_target_obj) {
            if goal != team_target {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                    let player_arc = owner_guard.get_controlling_player()?;
                    let mut player_guard = player_arc.write().ok()?;
                    let tunnels = player_guard.get_tunnel_system_mut()?;
                    crate::object::registry::OBJECT_REGISTRY.with_object(goal, |goal_guard| {
                        let _ = tunnels.update_nemesis(Some(goal_guard));
                    });
                    Some(())
                });
                self.base.set_nemesis_to_attack(team_target);
                goal_obj = Some(team_target);
            }
        }

        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };

        attack_machine.update()
    }

    fn on_exit(&mut self, _status: StateExitType) {
        if let Some(owner) = self.base.shared.owner() {
            clear_attack_state_on_exit(owner);
        }
        self.attack_machine = None;
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
    pub fn new(machine: &StateMachine, shared: Arc<TnGuardSharedState>) -> Self {
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardIdleState"),
            next_enemy_scan_time: 0,
        }
    }

    pub fn is_guard_idle(&self) -> bool {
        true
    }
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
        if let Some(owner) = self.base.shared.owner() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                if let Some(ai) = owner_guard.get_ai_update_interface_mut() {
                    ai.set_goal_object(None);
                }
            });
        }
        StateReturnType::Continue
    }

    fn update(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if now < self.next_enemy_scan_time {
            return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
        }

        self.next_enemy_scan_time = now.saturating_add(get_guard_enemy_scan_rate());

        let Some(owner) = self.base.shared.owner() else {
            return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
        };

        let wants_crate = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
            let ai = owner_guard.get_ai_update_interface_mut()?;
            ai.set_goal_object(None);
            Some(ai.get_crate_id() != crate::common::INVALID_ID)
        }).flatten().unwrap_or(false);
        if wants_crate {
            self.base.shared.request_state(TNGuardStateType::GetCrate as u32);
            return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
        }

        if let Some(target_id) = find_tunnel_network_inner_target(owner) {
            self.base.set_nemesis_to_attack(target_id);

            if get_legacy_object(target_id).is_none() {
                return StateReturnType::Sleep(0);
            }
            let hurry = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                if owner_guard.get_contained_by().is_none() {
                    return None;
                }
                let player_arc = owner_guard.get_controlling_player()?;
                let player_guard = player_arc.read().ok()?;
                let target_pos = crate::object::registry::OBJECT_REGISTRY
                    .with_object(target_id, |target_guard| *target_guard.get_position())?;
                let best_tunnel_id = find_best_tunnel(&player_guard, &target_pos)?;
                crate::object::registry::OBJECT_REGISTRY.with_object(best_tunnel_id, |tunnel_guard| {
                    let exit_interface = tunnel_guard.get_object_exit_interface()?;
                    Some(Ok((exit_interface, owner)))
                }).unwrap_or(Some(Err(StateReturnType::Sleep(0))))
            }).flatten();
            // Owner read ends with the closure. Hurry queues on that object when its
            // AI mutex is already held, and try_write cannot run under the read guard.
            match hurry {
                Some(Err(status)) => return status,
                Some(Ok((exit_interface, hurry_owner_id))) => {
                    let Ok(mut exit_guard) = exit_interface.lock() else {
                        return StateReturnType::Sleep(0);
                    };
                    if exit_guard.is_exit_busy() {
                        return StateReturnType::Sleep(0);
                    }
                    let _ = exit_guard.exit_object_in_a_hurry(hurry_owner_id);
                    return StateReturnType::Sleep(0);
                }
                None => {}
            }

            return StateReturnType::Success;
        }

        let should_fail = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            if owner_guard.get_contained_by().is_some() {
                return false;
            }
            let player_arc = owner_guard.get_controlling_player()?;
            let player_guard = player_arc.read().ok()?;
            let pos = *owner_guard.get_position();
            Some(find_best_tunnel(&player_guard, &pos).is_some())
        }).flatten().unwrap_or(false);
        if should_fail {
            return StateReturnType::Failure;
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
    pub fn new(machine: &StateMachine, shared: Arc<TnGuardSharedState>) -> Self {
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
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn on_enter(&mut self) -> StateReturnType {
        if matches!(self.base.guard_mode(), GuardMode::GuardWithoutPursuit) {
            // GUARDMODE_GUARD_WITHOUT_PURSUIT: patrol mode does not chase outside guard area.
            return StateReturnType::Success;
        }

        let Some(owner) = self.base.shared.owner() else {
            return StateReturnType::Failure;
        };
        let nemesis_id = self.base.get_nemesis_to_attack();
        let Some(_nemesis) = (nemesis_id != crate::common::INVALID_ID)
            .then(|| get_legacy_object(nemesis_id))
            .flatten()
        else {
            return StateReturnType::Success;
        };

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
        }

        let mut attack_machine = AttackStateMachine::with_owner_id(
            owner,
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

        let mut goal_id = attack_machine.get_goal_object_id();
        if goal_id == crate::common::INVALID_ID {
            goal_id = self.base.get_nemesis_to_attack();
        }
        let mut goal_obj = if goal_id != crate::common::INVALID_ID {
            get_legacy_object(goal_id)
        } else {
            None
        };
        if goal_obj.is_none() {
            if let Some(owner) = self.base.shared.owner() {
                let team_info = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                    let team_arc = owner_guard.get_team()?;
                    let team_guard = team_arc.read().ok()?;
                    if !team_guard.attack_common_target() {
                        return None;
                    }
                    Some(team_guard.get_team_target_object())
                }).flatten();
                if let Some(target_id) = team_info {
                    if target_id != crate::common::INVALID_ID && get_legacy_object(target_id).is_some() {
                        attack_machine.set_goal_object(Some(target_id));
                    } else {
                        attack_machine.set_goal_object(None);
                    }
                    return attack_machine.init_default_state();
                }
            }
        }

        if let Some(goal) = goal_obj {
            attack_machine.set_goal_object(Some(goal));
        }

        attack_machine.update()
    }

    fn on_exit(&mut self, _status: StateExitType) {
        if let Some(owner) = self.base.shared.owner() {
            clear_attack_state_on_exit(owner);
        }
        self.attack_machine = None;
        self.is_attacking = false;
    }
}
#[derive(Debug)]
pub struct AITNGuardReturnState {
    base: TnGuardState,
    enter_state: AIEnterState,
    next_return_scan_time: u32,
}

impl AITNGuardReturnState {
    pub fn new(machine: &StateMachine, shared: Arc<TnGuardSharedState>) -> Self {
        let enter_state = AIEnterState::new(machine);
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardReturn"),
            enter_state,
            next_return_scan_time: 0,
        }
    }
    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ AITNGuardReturnState::crc is AIEnterState::crc only.
        Snapshotable::crc(&self.enter_state, xfer)
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
    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.enter_state)
    }

    fn on_enter(&mut self) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_return_scan_rate();
        self.next_return_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        let Some(owner) = self.base.shared.owner() else {
            return StateReturnType::Failure;
        };

        enum EnterStep { Success, Failure(ObjectID), Tunnel(Option<ObjectID>) }
        let step = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            if owner_guard.get_contained_by().is_some() {
                return EnterStep::Success;
            }
            if let Some(team_arc) = owner_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    let target_id = team_guard.get_team_target_object();
                    if target_id != crate::common::INVALID_ID {
                        return EnterStep::Failure(target_id);
                    }
                }
            }
            let tunnel = owner_guard.get_controlling_player().and_then(|player_arc| {
                let player_guard = player_arc.read().ok()?;
                let pos = *owner_guard.get_position();
                find_best_tunnel(&player_guard, &pos)
            });
            EnterStep::Tunnel(tunnel)
        }).unwrap_or(EnterStep::Tunnel(None));
        let enter_tunnel = match step {
            EnterStep::Success => return StateReturnType::Success,
            EnterStep::Failure(target_id) => {
                self.base.set_nemesis_to_attack(target_id);
                return StateReturnType::Failure;
            }
            EnterStep::Tunnel(id) => id,
        };
        if let Some(best_tunnel_id) = enter_tunnel {
            self.enter_state.preset_owner = Some(owner);
            self.enter_state.preset_goal_id = best_tunnel_id;
            if let Some(pos) = crate::object::registry::OBJECT_REGISTRY
                .with_object(best_tunnel_id, |tunnel_guard| *tunnel_guard.get_position())
            {
                self.enter_state.goal_position = pos;
            }
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                if let Some(ai) = owner_guard.get_ai_update_interface_mut() {
                    ai.set_goal_object(Some(best_tunnel_id));
                }
            });

            return self.enter_state.on_enter();
        }

        StateReturnType::Failure
    }

    fn update(&mut self) -> StateReturnType {
        let Some(owner) = self.base.shared.owner() else {
            return StateReturnType::Failure;
        };

        let redirect = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            if let Some(team_arc) = owner_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    let target_id = team_guard.get_team_target_object();
                    if target_id != crate::common::INVALID_ID {
                        return Some(target_id);
                    }
                }
            }
            let player_arc = owner_guard.get_controlling_player()?;
            let mut player_guard = player_arc.write().ok()?;
            let tunnels = player_guard.get_tunnel_system_mut()?;
            tunnels.get_cur_nemesis_id().ok().flatten()
        }).flatten();
        if let Some(nemesis_id) = redirect {
            self.base.set_nemesis_to_attack(nemesis_id);
            return StateReturnType::Failure;
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
    pub fn new(machine: &StateMachine, shared: Arc<TnGuardSharedState>) -> Self {
        let pick_up_state = AIPickUpCrateState::new(machine);
        Self {
            base: TnGuardState::new(machine, shared, "AITNGuardPickUpCrate"),
            pick_up_state,
        }
    }
}

impl StateImplementation for AITNGuardPickUpCrateState {
    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.shared.owner() else {
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
        self.pick_up_state.preset_goal_id = crate_id;
        self.pick_up_state.base.preset_owner = Some(owner);
        if let Some(pos) = crate_pos {
            self.pick_up_state.goal_position = pos;
            self.pick_up_state.base.goal_position = pos;
        }
        self.pick_up_state.on_enter()
    }

    fn update(&mut self) -> StateReturnType {
        self.pick_up_state.update()
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // C++ AITNGuardPickUpCrateState::onExit is empty.
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
    pub fn new(machine: &StateMachine, shared: Arc<TnGuardSharedState>) -> Self {
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
    pub fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
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
    fn load_post_process(&mut self) -> Result<(), String> {
        let _ = self.on_enter();
        Ok(())
    }

    fn on_enter(&mut self) -> StateReturnType {
        let Some(owner) = self.base.shared.owner() else {
            return StateReturnType::Failure;
        };

        let mut nemesis_id = self.base.get_nemesis_to_attack();
        if let Some(source_id) = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            let body = owner_guard.get_body_module()?;
            let body_guard = body.lock().ok()?;
            let info = body_guard.get_last_damage_info()?;
            if info.source_id != crate::common::INVALID_ID { Some(info.source_id) } else { None }
        }).flatten() {
            nemesis_id = source_id;
            self.base.set_nemesis_to_attack(source_id);
        }
        let mut nemesis = if nemesis_id != crate::common::INVALID_ID {
            get_legacy_object(nemesis_id)
        } else {
            None
        };

        let Some(nemesis) = nemesis else {
            return StateReturnType::Success;
        };
        self.base.set_nemesis_to_attack(nemesis);
        let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
            let player_arc = owner_guard.get_controlling_player()?;
            let mut player_guard = player_arc.write().ok()?;
            let tunnels = player_guard.get_tunnel_system_mut()?;
            crate::object::registry::OBJECT_REGISTRY.with_object(nemesis, |nemesis_guard| {
                let _ = tunnels.update_nemesis(Some(nemesis_guard));
            });
            Some(())
        });

        if let Ok(mut exit_guard) = self.exit_conditions.lock() {
            exit_guard.set_attack_give_up_frame(
                TheGameLogic::get_frame().saturating_add(get_guard_chase_unit_frames()),
            );
        }

        let mut attack_machine = AttackStateMachine::with_owner_id(
            owner,
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

        if attack_machine.base.get_current_state_id()
            == Some(crate::ai::states::attack_machine::AttackSubStateId::FireWeapon as u32)
        {
            let nemesis_id = self.base.get_nemesis_to_attack();
            if let Some(owner) = self.base.shared.owner() {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                    let player_arc = owner_guard.get_controlling_player()?;
                    let mut player_guard = player_arc.write().ok()?;
                    let tunnels = player_guard.get_tunnel_system_mut()?;
                    if get_legacy_object(nemesis_id).is_some() {
                        crate::object::registry::OBJECT_REGISTRY.with_object(nemesis_id, |nemesis_guard| {
                            let _ = tunnels.update_nemesis(Some(nemesis_guard));
                        });
                    } else {
                        let _ = tunnels.update_nemesis(None);
                    }
                    Some(())
                });
            }
        }

        attack_machine.update()
    }

    fn on_exit(&mut self, _status: StateExitType) {
        if let Some(owner) = self.base.shared.owner() {
            clear_attack_state_on_exit(owner);
        }
        self.attack_machine = None;
        if let Some(owner) = self.base.shared.owner() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(crate::common::INVALID_ID);
                    }
                }
            });
        }
        self.is_attacking = false;
    }
}

fn find_tunnel_network_inner_target(owner_id: ObjectID) -> Option<ObjectID> {
    // Wave 375: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }
    let reg = &crate::object::registry::OBJECT_REGISTRY;
    let team_target = reg.with_object(owner_id, |owner_guard| {
        let team_arc = owner_guard.get_team()?;
        let team_guard = team_arc.read().ok()?;
        if !team_guard.attack_common_target() {
            return None;
        }
        let team_target = team_guard.get_team_target_object();
        if team_target != crate::common::INVALID_ID && reg.with_object(team_target, |_| ()).is_some() {
            Some(team_target)
        } else {
            None
        }
    }).flatten();
    if let Some(team_target) = team_target {
        return Some(team_target);
    }

    reg.with_object(owner_id, |owner_guard| {
        let player_arc = owner_guard.get_controlling_player()?;
        let mut player_guard = player_arc.write().ok()?;
        let tunnels = player_guard.get_tunnel_system_mut()?;
        if let Ok(Some(nemesis_id)) = tunnels.get_cur_nemesis_id() {
            return Some(nemesis_id);
        }
        let container_list = tunnels.get_container_list().ok()?;
        for tunnel_id in container_list {
            let found = reg.with_object(tunnel_id, |tunnel_guard| {
                if let Some(ai) = tunnel_guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        let victim_id = ai_guard.get_goal_object_id();
                        if victim_id != crate::common::INVALID_ID {
                            let is_enemy = reg.with_object(victim_id, |victim_guard| {
                                owner_guard.relationship_to(victim_guard) == Relationship::Enemies
                            }).unwrap_or(false);
                            if is_enemy {
                                return Some(victim_id);
                            }
                        }
                    }
                }
                let body = tunnel_guard.get_body_module()?;
                let body_guard = body.lock().ok()?;
                let info = body_guard.get_last_damage_info()?;
                if info.output.no_effect {
                    return None;
                }
                let scan_rate = get_guard_enemy_scan_rate();
                if body_guard.get_last_damage_timestamp() + scan_rate <= TheGameLogic::get_frame() {
                    return None;
                }
                let attacker_id = info.source_id;
                let can_attack = reg.with_object(attacker_id, |attacker_guard| {
                    owner_guard.relationship_to(attacker_guard) == Relationship::Enemies
                        && matches!(
                            owner_guard.get_able_to_attack_specific_object(
                                AbleToAttackType::TunnelNetworkGuard,
                                attacker_guard,
                                CommandSourceType::FromAi,
                            ),
                            CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
                        )
                }).unwrap_or(false);
                if !can_attack {
                    return None;
                }
                if let Some(team_arc) = owner_guard.get_team() {
                    if let Ok(mut team_guard) = team_arc.write() {
                        team_guard.set_team_target_object(attacker_id);
                    }
                }
                let _ = reg.with_object(attacker_id, |attacker_guard| {
                    let _ = tunnels.update_nemesis(Some(attacker_guard));
                });
                Some(attacker_id)
            }).flatten();
            if let Some(id) = found {
                return Some(id);
            }
        }
        None
    }).flatten()
}

fn tunnel_network_scan(owner_id: ObjectID) -> Option<ObjectID> {
    // Wave 375: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }
    let partition = ThePartitionManager::get()?;
    let vision_range = AITNGuardMachine::get_std_guard_range(owner_id);
    crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
        let owner_pos = *owner_guard.get_position();
        partition.get_closest_object_2d(&owner_pos, vision_range, |candidate| {
            if candidate.get_id() == owner_id {
                return false;
            }
            if candidate.is_effectively_dead() {
                return false;
            }
            if owner_guard.is_off_map() != candidate.is_off_map() {
                return false;
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
    }).flatten()
}

/// Helper function to find best tunnel for a position
pub fn find_best_tunnel(owner_player: &Player, pos: &Coord3D) -> Option<ObjectID> {
    // Wave 375: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }
    let tunnels = owner_player.get_tunnel_system()?;
    let list = tunnels.get_container_list().ok()?;
    let mut best: Option<(ObjectID, Real)> = None;
    for tunnel_id in list {
        let Some(tunnel_pos) = crate::object::registry::OBJECT_REGISTRY
            .with_object(tunnel_id, |tunnel_guard| *tunnel_guard.get_position())
        else {
            continue;
        };
        let dx = tunnel_pos.x - pos.x;
        let dy = tunnel_pos.y - pos.y;
        let dist_sqr = dx * dx + dy * dy;
        let better = best.as_ref().map(|(_, best_dist)| dist_sqr < *best_dist).unwrap_or(true);
        if better {
            best = Some((tunnel_id, dist_sqr));
        }
    }
    best.map(|(id, _)| id)
}

/// Helper function to check if an object has attacked and can be retaliated against
/// through the tunnel network
pub fn has_attacked_me_and_i_can_return_fire_tn(machine: &StateMachine) -> bool {
    if dual_world_registry_unavailable() {
        return false;
    }
    let owner_id = machine.get_owner_id();
    if owner_id == crate::common::INVALID_ID {
        return false;
    }
    has_attacked_tn_owner(owner_id)
}

fn has_attacked_tn_owner(owner_id: ObjectID) -> bool {
    if dual_world_registry_unavailable() {
        return false;
    }
    let Some(last_attacker) = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_ref| {
        let body_module = owner_ref.get_body_module()?;
        let mut body_guard = body_module.lock().ok()?;
        let last_attacker = body_guard.get_clearable_last_attacker();
        if last_attacker == crate::common::INVALID_ID { return None; }
        body_guard.clear_last_attacker();
        Some(last_attacker)
    }).flatten() else { return false; };
    crate::object::registry::OBJECT_REGISTRY.with_object(owner_id, |owner_ref| {
        crate::object::registry::OBJECT_REGISTRY.with_object(last_attacker, |target_guard| {
            if owner_ref.relationship_to(target_guard) != Relationship::Enemies { return false; }
            if target_guard.is_effectively_dead() { return false; }
            matches!(
                owner_ref.get_able_to_attack_specific_object(
                    AbleToAttackType::NewTarget,
                    target_guard,
                    CommandSourceType::FromAi,
                ),
                CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
            )
        })
    }).flatten().unwrap_or(false)
}

fn tn_guard_owner(state: &dyn StateImplementation) -> Option<ObjectID> {
    if let Some(ret) = state.as_any().downcast_ref::<AITNGuardReturnState>() {
        return ret.base.shared.owner();
    }
    if let Some(inner) = state.as_any().downcast_ref::<AITNGuardInnerState>() {
        return inner.base.shared.owner();
    }
    None
}

fn tn_guard_attack_aggressor_return(
    state: &dyn StateImplementation,
    _user_data: &StateTransitionUserData,
) -> bool {
    let Some(owner) = tn_guard_owner(state) else {
        return false;
    };
    has_attacked_tn_owner(owner)
}

fn tn_guard_attack_aggressor_inner(
    state: &dyn StateImplementation,
    _user_data: &StateTransitionUserData,
) -> bool {
    let Some(owner) = tn_guard_owner(state) else {
        return false;
    };
    has_attacked_tn_owner(owner)
}
