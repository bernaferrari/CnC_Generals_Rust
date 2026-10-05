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
use crate::modules::{AIUpdateInterfaceExt, ExitDoorType, ExitInterface};
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

/// Resolve the tunnel-guard owner object from the id copied into a state or
/// held by the machine (C++ reached the owner through the machine pointer; the
/// owned machine keeps only the id).
fn tn_guard_owner_arc_for_id(owner_id: ObjectID) -> Option<Arc<RwLock<Object>>> {
    if owner_id == crate::common::INVALID_ID {
        return None;
    }
    TheGameLogic::find_object_by_id(owner_id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(owner_id))
}

fn clear_attack_state_on_exit(owner: &Arc<RwLock<Object>>) {
    let Ok(mut owner_guard) = owner.write() else {
        return;
    };
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
}

fn get_guard_chase_unit_frames() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 0;
    };
    let data = ai_guard.get_ai_data();
    data.guard_chase_unit_frames
}

fn get_guard_enemy_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 30;
    };
    let data = ai_guard.get_ai_data();
    data.guard_enemy_scan_rate
}

fn get_guard_enemy_return_scan_rate() -> u32 {
    let ai_store = the_ai();
    let Ok(ai_guard) = ai_store.read() else {
        return 60;
    };
    let data = ai_guard.get_ai_data();
    data.guard_enemy_return_scan_rate
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

/// Shared handle for the exit conditions a tunnel-guard state hands its child
/// attack machine. This is the one deliberate interior-mutable cell left in
/// the family: `AttackExitConditionsInterface` is a shared trait object the
/// child reads while the parent state keeps tuning the same conditions, and
/// the trait lives in `ai/states` out of this conversion's scope. The state
/// machine itself carries no shared handle.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tn_guard_machine_settles_in_idle_and_stays_there() {
        // IDLE is the one tunnel-guard state whose enter does not need the
        // owner object, so it lands (and stays) even with no resolvable owner.
        let mut machine = AITNGuardMachine::new(Weak::new());
        machine.set_state(TNGuardStateType::Idle);
        assert_eq!(
            machine.state_machine.get_current_state_id(),
            Some(TNGuardStateType::Idle as u32)
        );
    }

    #[test]
    fn tn_guard_machine_fields_are_isolated_per_machine() {
        let owner_a = Arc::new(RwLock::new(Object::new_test(81, 100.0)));
        let owner_b = Arc::new(RwLock::new(Object::new_test(82, 100.0)));

        let mut machine_a = AITNGuardMachine::new(Arc::downgrade(&owner_a));
        let mut machine_b = AITNGuardMachine::new(Arc::downgrade(&owner_b));

        machine_a.set_nemesis_id(7);
        machine_a.set_target_position_to_guard(&Coord3D::new(4.0, 5.0, 6.0));
        machine_b.set_nemesis_id(9);

        assert_eq!(machine_a.get_nemesis_id(), 7);
        assert_eq!(machine_b.get_nemesis_id(), 9);
        assert_eq!(
            machine_a.get_position_to_guard(),
            &Coord3D::new(4.0, 5.0, 6.0)
        );
        assert_eq!(
            machine_b.get_position_to_guard(),
            &Coord3D::new(0.0, 0.0, 0.0)
        );
        assert_eq!(machine_a.get_guard_mode(), GuardMode::Normal);
        machine_a.set_guard_mode(GuardMode::GuardWithoutPursuit);
        assert_eq!(machine_a.get_guard_mode(), GuardMode::GuardWithoutPursuit);
        assert_eq!(machine_b.get_guard_mode(), GuardMode::Normal);

        // The nemesis is mirrored onto the machine goal (C++
        // setNemesisToAttack writes m_goalObject too).
        assert_eq!(machine_a.state_machine.get_goal_object_id(), 7);
        assert_eq!(machine_b.state_machine.get_goal_object_id(), 9);
    }
}

/// Tunnel Network Guard state machine
///
/// C++ `AITNGuardMachine` owns its `StateMachine` and the guard configuration
/// as plain members. The machine here is a plain owned field, states carry no
/// handles, and the machine itself is loaned to each state hook via
/// `update_with_owner` / `on_enter_with_owner` — the same shape as the turret
/// conversion. No Arc, no Mutex, no Weak for the machine.
#[derive(Debug)]
pub struct AITNGuardMachine {
    /// The tunnel guard's own state machine. Owned, not shared.
    state_machine: StateMachine,
    /// Owner object id (C++ `getOwner()`), resolved per use.
    owner_id: ObjectID,
    /// Position to guard
    position_to_guard: Coord3D,
    /// Nemesis to attack
    nemesis_to_attack: ObjectID,
    /// Guard mode
    guard_mode: GuardMode,
    /// Transition a state requested while the machine was loaned out; applied
    /// by [`AITNGuardMachine::update`] after the step returns.
    pending_state: Option<u32>,
}

impl AITNGuardMachine {
    pub fn new(owner: Weak<RwLock<Object>>) -> Self {
        let owner_id = owner
            .upgrade()
            .and_then(|arc| arc.read().ok().map(|owner_ref| owner_ref.get_id()))
            .unwrap_or(crate::common::INVALID_ID);

        let mut machine = Self {
            state_machine: StateMachine::empty(),
            owner_id,
            position_to_guard: Coord3D::new(0.0, 0.0, 0.0),
            nemesis_to_attack: crate::common::INVALID_ID,
            guard_mode: GuardMode::Normal,
            pending_state: None,
        };
        machine.build_state_machine();
        machine
    }

    /// Defines the six tunnel-guard states and enters the default one (C++
    /// `AITNGuardMachine` ctor tail + `initDefaultState()`).
    fn build_state_machine(&mut self) {
        if !self.state_machine.is_empty() {
            return;
        }
        let mut machine = StateMachine::new_with_owner_id(self.owner_id, "AITNGuardMachine");
        self.define_tn_guard_states(&mut machine);
        let _ = machine.init_default_state_with_owner(self);
        self.state_machine = machine;
    }

    /// C++ state definitions. Order matters: the first defined state
    /// (RETURN) becomes the default.
    fn define_tn_guard_states(&self, machine: &mut StateMachine) {
        let return_id = TNGuardStateType::Return as u32;
        let idle_id = TNGuardStateType::Idle as u32;
        let inner_id = TNGuardStateType::Inner as u32;
        let outer_id = TNGuardStateType::Outer as u32;
        let crate_id = TNGuardStateType::GetCrate as u32;
        let aggressor_id = TNGuardStateType::AttackAggressor as u32;

        let attack_aggressor_conditions = vec![StateConditionInfo::new(
            tn_guard_attack_aggressor_condition,
            aggressor_id,
            StateTransitionUserData::new(),
            "has_attacked_me_and_i_can_return_fire",
        )];

        machine.define_state(
            return_id,
            Box::new(AITNGuardReturnState::new(machine)),
            Some(idle_id),
            Some(inner_id),
            Some(&attack_aggressor_conditions),
        );

        machine.define_state(
            idle_id,
            Box::new(AITNGuardIdleState::new(machine)),
            Some(inner_id),
            Some(return_id),
            None,
        );

        machine.define_state(
            inner_id,
            Box::new(AITNGuardInnerState::new(machine)),
            Some(outer_id),
            Some(outer_id),
            Some(&attack_aggressor_conditions),
        );

        machine.define_state(
            outer_id,
            Box::new(AITNGuardOuterState::new(machine)),
            Some(crate_id),
            Some(crate_id),
            None,
        );

        machine.define_state(
            crate_id,
            Box::new(AITNGuardPickUpCrateState::new(machine)),
            Some(return_id),
            Some(return_id),
            None,
        );

        machine.define_state(
            aggressor_id,
            Box::new(AITNGuardAttackAggressorState::new(machine)),
            Some(return_id),
            Some(return_id),
            None,
        );
    }

    // ---- loan-safe accessors (never touch `state_machine`) -------------------

    fn friend_request_state(&mut self, state: u32) {
        self.pending_state = Some(state);
    }

    fn friend_owner_arc(&self) -> Option<Arc<RwLock<Object>>> {
        tn_guard_owner_arc_for_id(self.owner_id)
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

    fn friend_guard_mode(&self) -> GuardMode {
        self.guard_mode
    }

    // ---- public API ----------------------------------------------------------

    /// Get position to guard
    pub fn get_position_to_guard(&self) -> &Coord3D {
        &self.position_to_guard
    }

    /// Set target position to guard
    pub fn set_target_position_to_guard(&mut self, pos: &Coord3D) {
        self.position_to_guard = *pos;
    }

    /// Set nemesis ID
    pub fn set_nemesis_id(&mut self, id: ObjectID) {
        self.nemesis_to_attack = id;
        self.state_machine.set_goal_object_by_id(Some(id));
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
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        if self.state_machine.is_empty() {
            return StateReturnType::Failure;
        }
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let result = machine.init_default_state_with_owner(self);
        self.state_machine = machine;
        result
    }

    pub fn set_state(&mut self, state: TNGuardStateType) -> StateReturnType {
        if self.state_machine.is_empty() {
            return StateReturnType::Failure;
        }
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let result = machine.set_current_state_with_owner(state as u32, self);
        self.state_machine = machine;
        result
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.state_machine.halt()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.state_machine.is_in_attack_state()
    }

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.state_machine.is_in_guard_idle_state()
    }

    pub fn update(&mut self) -> StateReturnType {
        if self.state_machine.is_empty() {
            return StateReturnType::Failure;
        }
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let result = machine.update_with_owner(self);
        if let Some(state_id) = self.pending_state.take() {
            let _ = machine.set_current_state_with_owner(state_id, self);
        }
        self.state_machine = machine;
        result
    }

    /// Look for inner target within tunnel network
    pub fn look_for_inner_target(&mut self) -> bool {
        let owner_id = self.owner_id;
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
            self.state_machine.xfer(xfer).map_err(|e| e.to_string())?;
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
        if self.state_machine.is_empty() {
            return Ok(());
        }
        let mut machine = std::mem::replace(&mut self.state_machine, StateMachine::empty());
        let result = machine
            .load_post_process_with_owner(self)
            .map_err(|e| format!("tn guard load_post_process: {e}"));
        self.state_machine = machine;
        result
    }
}

// State implementations for tunnel network guard
//
// The states embed only the legacy `State` bookkeeping (id/name plus the
// machine's owner id copied once at define time) and receive the loaned
// machine through `update_with_owner` / `on_enter_with_owner`.

#[derive(Debug)]
struct TnGuardState {
    base: State,
}

impl TnGuardState {
    fn new(machine: &StateMachine, name: &str) -> Self {
        // `State::new` copies the machine's owner id and attaches NO machine
        // handle: the machine owns its states, never the other way round.
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
        tn_guard_owner_arc_for_id(self.base.owner_id)
    }

    fn downcast_machine(owner: &mut dyn std::any::Any) -> Option<&mut AITNGuardMachine> {
        owner.downcast_mut::<AITNGuardMachine>()
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
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: TnGuardState::new(machine, "AITNGuardInner"),
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
        Ok(())
    }

    fn classic_on_enter(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        self.scan_for_enemy = true;

        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };
        let nemesis_id = machine.friend_nemesis_to_attack();
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

    fn classic_update(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        let Some(owner) = machine.friend_owner_arc() else {
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

        let mut goal_id = machine.friend_nemesis_to_attack();
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
                machine.friend_set_nemesis_to_attack(
                    target
                        .read()
                        .map(|guard| guard.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                );
                return StateReturnType::Continue;
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
                machine.friend_set_nemesis_to_attack(
                    target
                        .read()
                        .map(|guard| guard.get_id())
                        .unwrap_or(crate::common::INVALID_ID),
                );
                return StateReturnType::Continue;
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
                self.attack_machine = None;
                if let Ok(mut owner_guard) = owner.write() {
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
                }
                let mut attack_machine = AttackStateMachine::new(
                    Arc::downgrade(&owner),
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
        } else if let (Some(goal), Some(team_target)) = (&goal_obj, &team_target_obj) {
            if goal.read().ok().map(|g| g.get_id()) != team_target.read().ok().map(|t| t.get_id())
            {
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
                machine.friend_set_nemesis_to_attack(
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

        attack_machine.update()
    }

    fn classic_on_exit(&mut self, _status: StateExitType) {
        if let Some(owner) = self.base.owner_arc() {
            clear_attack_state_on_exit(&owner);
        }
        self.attack_machine = None;
        self.is_attacking = false;
    }
}

impl StateImplementation for AITNGuardInnerState {
    /// Tunnel-guard states are only stepped through their owner's machine,
    /// which always loans the machine via `update_with_owner`.
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_update(machine),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.classic_on_exit(_status);
    }

    /// C++ `loadPostProcess` re-runs `onEnter` to rebuild the attack child.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        if let Some(machine) = TnGuardState::downcast_machine(owner) {
            let _ = self.classic_on_enter(machine);
        }
        Ok(())
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
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

/// Idle tunnel network guard state
#[derive(Debug)]
pub struct AITNGuardIdleState {
    base: TnGuardState,
    next_enemy_scan_time: u32,
}

impl AITNGuardIdleState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: TnGuardState::new(machine, "AITNGuardIdleState"),
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

    fn classic_on_enter(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_scan_rate();
        self.next_enemy_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));
        if let Some(owner) = machine.friend_owner_arc() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_goal_object(None);
                    }
                }
            }
        }
        StateReturnType::Continue
    }

    fn classic_update(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        if now < self.next_enemy_scan_time {
            return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
        }

        self.next_enemy_scan_time = now.saturating_add(get_guard_enemy_scan_rate());

        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Sleep(self.next_enemy_scan_time.saturating_sub(now));
        };

        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    ai_guard.set_goal_object(None);
                    if ai_guard.get_crate_id() != crate::common::INVALID_ID {
                        machine.friend_request_state(TNGuardStateType::GetCrate as u32);
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
            machine.friend_set_nemesis_to_attack(target_id);

            let Some(target) = get_legacy_object(target_id) else {
                return StateReturnType::Sleep(0);
            };
            let hurry = (|| {
                let owner_guard = owner.read().ok()?;
                let target_guard = target.read().ok()?;
                if owner_guard.get_contained_by().is_none() {
                    return None;
                }
                let player_arc = owner_guard.get_controlling_player()?;
                let player_guard = player_arc.read().ok()?;
                let best_tunnel_id = find_best_tunnel(&player_guard, target_guard.get_position())?;
                let hurry_owner_id = owner_guard.get_id();
                let Some(best_tunnel) = get_legacy_object(best_tunnel_id) else {
                    return Some(Err(StateReturnType::Sleep(0)));
                };
                let Ok(tunnel_guard) = best_tunnel.read() else {
                    return Some(Err(StateReturnType::Sleep(0)));
                };
                let Some(exit_interface) = tunnel_guard.get_object_exit_interface() else {
                    return Some(Err(StateReturnType::Failure));
                };
                Some(Ok((exit_interface, hurry_owner_id)))
            })();
            // Owner read ends with the closure. Hurry queues on that object when its
            // AI mutex is already held, and try_write cannot run under the read guard.
            match hurry {
                Some(Err(status)) => return status,
                Some(Ok((mut exit_interface, hurry_owner_id))) => {
                    if exit_interface.is_exit_busy() {
                        return StateReturnType::Sleep(0);
                    }
                    let _ = exit_interface.exit_object_in_a_hurry(hurry_owner_id);
                    return StateReturnType::Sleep(0);
                }
                None => {}
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
}

impl StateImplementation for AITNGuardIdleState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_update(machine),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        // Cleanup when exiting idle state
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
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

/// Outer tunnel network guard state
#[derive(Debug)]
pub struct AITNGuardOuterState {
    base: TnGuardState,
    exit_conditions: Arc<Mutex<TunnelNetworkExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AITNGuardOuterState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: TnGuardState::new(machine, "AITNGuardOuter"),
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
        Ok(())
    }

    fn classic_on_enter(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        if matches!(machine.friend_guard_mode(), GuardMode::GuardWithoutPursuit) {
            // GUARDMODE_GUARD_WITHOUT_PURSUIT: patrol mode does not chase outside guard area.
            return StateReturnType::Success;
        }

        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };
        let nemesis_id = machine.friend_nemesis_to_attack();
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

    fn classic_update(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };

        let mut goal_id = attack_machine.get_goal_object_id();
        if goal_id == crate::common::INVALID_ID {
            goal_id = machine.friend_nemesis_to_attack();
        }
        let mut goal_obj = if goal_id != crate::common::INVALID_ID {
            get_legacy_object(goal_id)
        } else {
            None
        };
        if goal_obj.is_none() {
            if let Some(owner) = machine.friend_owner_arc() {
                if let Ok(owner_guard) = owner.read() {
                    let mut team_target = None;
                    let mut attack_common = false;
                    if let Some(team_arc) = owner_guard.get_team() {
                        if let Ok(team_guard) = team_arc.read() {
                            attack_common = team_guard.attack_common_target();
                            if attack_common {
                                let target_id = team_guard.get_team_target_object();
                                if target_id != crate::common::INVALID_ID {
                                    team_target = get_legacy_object(target_id);
                                }
                            }
                        }
                    }
                    if attack_common {
                        if let Some(target) = team_target {
                            let id = target
                                .read()
                                .map(|guard| guard.get_id())
                                .unwrap_or(crate::common::INVALID_ID);
                            attack_machine.set_goal_object(Some(id));
                        } else {
                            attack_machine.set_goal_object(None);
                        }
                        return attack_machine.init_default_state();
                    }
                }
            }
        }

        if let Some(goal) = goal_obj.as_ref() {
            attack_machine.set_goal_object(goal.read().ok().map(|g| g.get_id()));
        }

        attack_machine.update()
    }

    fn classic_on_exit(&mut self, _status: StateExitType) {
        if let Some(owner) = self.base.owner_arc() {
            clear_attack_state_on_exit(&owner);
        }
        self.attack_machine = None;
        self.is_attacking = false;
    }
}

impl StateImplementation for AITNGuardOuterState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_update(machine),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.classic_on_exit(_status);
    }

    /// C++ `loadPostProcess` re-runs `onEnter`.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        if let Some(machine) = TnGuardState::downcast_machine(owner) {
            let _ = self.classic_on_enter(machine);
        }
        Ok(())
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
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

/// Return tunnel network guard state
#[derive(Debug)]
pub struct AITNGuardReturnState {
    base: TnGuardState,
    enter_state: AIEnterState,
    next_return_scan_time: u32,
}

impl AITNGuardReturnState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: TnGuardState::new(machine, "AITNGuardReturn"),
            enter_state: AIEnterState::new(machine),
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

    fn classic_on_enter(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        let now = TheGameLogic::get_frame();
        let scan_rate = get_guard_enemy_return_scan_rate();
        self.next_return_scan_time = now.saturating_add(game_logic_random_value(0, scan_rate));

        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };

        let mut enter_tunnel = None;
        if let Ok(owner_guard) = owner.read() {
            if owner_guard.get_contained_by().is_some() {
                return StateReturnType::Success;
            }

            if let Some(team_arc) = owner_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    let target_id = team_guard.get_team_target_object();
                    if target_id != crate::common::INVALID_ID {
                        machine.friend_set_nemesis_to_attack(target_id);
                        return StateReturnType::Failure;
                    }
                }
            }

            if let Some(player_arc) = owner_guard.get_controlling_player() {
                if let Ok(player_guard) = player_arc.read() {
                    let pos = *owner_guard.get_position();
                    enter_tunnel = find_best_tunnel(&player_guard, &pos);
                }
            }
        }
        if let Some(best_tunnel_id) = enter_tunnel {
            self.enter_state.preset_owner = Some(owner.clone());
            self.enter_state.preset_goal_id = best_tunnel_id;
            if let Some(tunnel) = get_legacy_object(best_tunnel_id) {
                if let Ok(tunnel_guard) = tunnel.read() {
                    self.enter_state.goal_position = *tunnel_guard.get_position();
                }
            }
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_goal_object(Some(best_tunnel_id));
                    }
                }
            }

            return self.enter_state.on_enter();
        }

        StateReturnType::Failure
    }

    fn classic_update(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };

        if let Ok(owner_guard) = owner.read() {
            if let Some(team_arc) = owner_guard.get_team() {
                if let Ok(team_guard) = team_arc.read() {
                    let target_id = team_guard.get_team_target_object();
                    if target_id != crate::common::INVALID_ID {
                        machine.friend_set_nemesis_to_attack(target_id);
                        return StateReturnType::Failure;
                    }
                }
            }

            if let Some(player_arc) = owner_guard.get_controlling_player() {
                if let Ok(mut player_guard) = player_arc.write() {
                    if let Some(tunnels) = player_guard.get_tunnel_system_mut() {
                        if let Ok(Some(nemesis_id)) = tunnels.get_cur_nemesis_id() {
                            machine.friend_set_nemesis_to_attack(nemesis_id);
                            return StateReturnType::Failure;
                        }
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

    fn classic_on_exit(&mut self, status: StateExitType) {
        self.enter_state.on_exit(status);
    }
}

impl StateImplementation for AITNGuardReturnState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_update(machine),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, status: StateExitType) {
        self.classic_on_exit(status);
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.enter_state)
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
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

/// Pick up crate state for tunnel network guard
#[derive(Debug)]
pub struct AITNGuardPickUpCrateState {
    base: TnGuardState,
    pick_up_state: AIPickUpCrateState,
}

impl AITNGuardPickUpCrateState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: TnGuardState::new(machine, "AITNGuardPickUpCrate"),
            pick_up_state: AIPickUpCrateState::new(machine),
        }
    }

    fn classic_on_enter(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
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
        let crate_pos = TheGameLogic::find_object_by_id(crate_id)
            .and_then(|crate_obj| crate_obj.read().ok().map(|goal| *goal.get_position()));
        drop(owner_guard);
        self.pick_up_state.preset_goal_id = crate_id;
        self.pick_up_state.base.preset_owner = Some(owner);
        if let Some(pos) = crate_pos {
            self.pick_up_state.goal_position = pos;
            self.pick_up_state.base.goal_position = pos;
        }
        self.pick_up_state.on_enter()
    }

    fn classic_update(&mut self) -> StateReturnType {
        self.pick_up_state.update()
    }

    fn classic_on_exit(&mut self, _status: StateExitType) {
        // C++ AITNGuardPickUpCrateState::onExit is empty.
    }
}

impl StateImplementation for AITNGuardPickUpCrateState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(_) => self.classic_update(),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.classic_on_exit(_status);
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
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

/// Attack aggressor state for tunnel network guard
#[derive(Debug)]
pub struct AITNGuardAttackAggressorState {
    base: TnGuardState,
    exit_conditions: Arc<Mutex<TunnelNetworkExitConditions>>,
    is_attacking: bool,
    attack_machine: Option<AttackStateMachine>,
}

impl AITNGuardAttackAggressorState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: TnGuardState::new(machine, "AITNGuardAttackAggressor"),
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
        Ok(())
    }

    fn classic_on_enter(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        let Some(owner) = machine.friend_owner_arc() else {
            return StateReturnType::Failure;
        };

        let mut nemesis_id = machine.friend_nemesis_to_attack();
        if let Ok(owner_guard) = owner.read() {
            if let Some(body) = owner_guard.get_body_module() {
                if let Ok(body_guard) = body.lock() {
                    if let Some(info) = body_guard.get_last_damage_info() {
                        if info.source_id != crate::common::INVALID_ID {
                            nemesis_id = info.source_id;
                            machine.friend_set_nemesis_to_attack(info.source_id);
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
        machine.friend_set_nemesis_to_attack(
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

    fn classic_update(&mut self, machine: &mut AITNGuardMachine) -> StateReturnType {
        let Some(attack_machine) = self.attack_machine.as_mut() else {
            return StateReturnType::Success;
        };

        if attack_machine.base.get_current_state_id()
            == Some(crate::ai::states::attack_machine::AttackSubStateId::FireWeapon as u32)
        {
            let nemesis_id = machine.friend_nemesis_to_attack();
            if let Some(owner) = machine.friend_owner_arc() {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(player_arc) = owner_guard.get_controlling_player() {
                        if let Ok(mut player_guard) = player_arc.write() {
                            if let Some(tunnels) = player_guard.get_tunnel_system_mut() {
                                if let Some(nemesis) = get_legacy_object(nemesis_id) {
                                    if let Ok(nemesis_guard) = nemesis.read() {
                                        let _ = tunnels.update_nemesis(Some(&nemesis_guard));
                                    }
                                } else {
                                    let _ = tunnels.update_nemesis(None);
                                }
                            }
                        }
                    }
                }
            }
        }

        attack_machine.update()
    }

    fn classic_on_exit(&mut self, _status: StateExitType) {
        if let Some(owner) = self.base.owner_arc() {
            clear_attack_state_on_exit(&owner);
        }
        self.attack_machine = None;
        if let Some(owner) = self.base.owner_arc() {
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

impl StateImplementation for AITNGuardAttackAggressorState {
    fn update(&mut self) -> StateReturnType {
        StateReturnType::Failure
    }

    fn on_enter_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_on_enter(machine),
            None => StateReturnType::Failure,
        }
    }

    fn update_with_owner(&mut self, owner: &mut dyn std::any::Any) -> StateReturnType {
        match TnGuardState::downcast_machine(owner) {
            Some(machine) => self.classic_update(machine),
            None => StateReturnType::Failure,
        }
    }

    fn on_exit(&mut self, _status: StateExitType) {
        self.classic_on_exit(_status);
    }

    /// C++ `loadPostProcess` re-runs `onEnter`.
    fn load_post_process_with_owner(
        &mut self,
        owner: &mut dyn std::any::Any,
    ) -> Result<(), String> {
        if let Some(machine) = TnGuardState::downcast_machine(owner) {
            let _ = self.classic_on_enter(machine);
        }
        Ok(())
    }

    fn note_step_owner(&mut self, owner: std::sync::Arc<std::sync::RwLock<Object>>) {
        if let Ok(owner_guard) = owner.read() {
            self.base.base.owner_id = owner_guard.get_id();
        }
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

fn find_tunnel_network_inner_target(owner_id: ObjectID) -> Option<ObjectID> {
    // Wave 375: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

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
                AbleToAttackType::TunnelNetworkGuard,
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
    // Wave 375: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

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
        let Some(tunnel_arc) = TheGameLogic::find_object_by_id(tunnel_id) else {
            continue;
        };
        let Ok(tunnel_guard) = tunnel_arc.read() else {
            continue;
        };
        let tunnel_pos = *tunnel_guard.get_position();

        let dx = tunnel_pos.x - pos.x;
        let dy = tunnel_pos.y - pos.y;
        let dist_sqr = dx * dx + dy * dy;
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
    if dual_world_registry_unavailable() {
        return false;
    }
    let Some(owner) = machine.get_owner() else {
        return false;
    };
    has_attacked_tn_owner(&owner)
}

fn has_attacked_tn_owner(owner: &Arc<RwLock<Object>>) -> bool {
    if dual_world_registry_unavailable() {
        return false;
    }
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
}

/// C++ `AITNGuardState`'s per-state `ATTACK_AGGRESSOR` condition. The states
/// no longer carry a machine handle, so the owner comes from the state's
/// copied owner id — the loaned-machine equivalent of `getMachine()->getOwner()`.
fn tn_guard_attack_aggressor_condition(
    state: &dyn StateImplementation,
    _user_data: &StateTransitionUserData,
) -> bool {
    let Some(owner) = state
        .get_machine_owner_id()
        .ok()
        .and_then(tn_guard_owner_arc_for_id)
    else {
        return false;
    };
    has_attacked_tn_owner(&owner)
}
