#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
use super::follow_path::*;
use super::follow_path_core::*;
use super::guard::*;
use super::hack::*;
use super::helpers::*;
use super::hunt::*;
use super::idle::*;
use super::r#move::*;
use super::rappel::*;
use super::state_machine::*;
use super::types::*;
use super::wait_busy::*;
use super::wander_panic::*;
use super::waypoint::*;
use super::*;

use crate::action_manager::{CanEnterType, TheActionManager};
use crate::ai::dock::AIDockMachine;
use crate::ai::group::AIGroup;
use crate::ai::guard::{AIGuardMachine, GuardStateType};
use crate::ai::guard_retaliate::AIGuardRetaliateMachine;
use crate::ai::object_registry::get_legacy_object;
use crate::ai::pathfind::Path;
use crate::ai::squad::Squad;
use crate::ai::tn_guard::{AITNGuardMachine, TNGuardStateType};
use crate::ai::{
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter, the_ai,
    mood_matrix_adjustment, mood_matrix_parameters, resolve_attack_priority_info_for_object,
    search_qualifiers,
};
use crate::attack::{AbleToAttackType, CanAttackResult};
use crate::command_button::CommandButton;
use crate::common::coord::*;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::compat::{ClassicState, legacy_transition, register_classic_state};
use crate::control_bar::get_control_bar_bridge;
use crate::damage::DamageInfo;
use crate::helpers::{TheAudio, TheGameLogic, ThePartitionManager, get_game_logic_random_value};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{
    AIUpdateInterface, AIUpdateInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE,
};
use crate::object::production::AIFreeToExitType;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::physics::GRAVITY;
use crate::player::PlayerType;
use crate::polygon_trigger::PolygonTrigger;
use crate::scripting::engine::get_script_engine;
use crate::state_machine::*;
use crate::team::{Team, TeamID, TheTeamFactory};
use crate::terrain::get_terrain_logic;
use crate::waypoint::{Waypoint, WaypointId};
use crate::weapon::{
    NO_MAX_SHOTS_LIMIT, Weapon, WeaponChoiceCriteria, WeaponLockType, WeaponSlotType, WeaponStatus,
};
use game_engine::common::system::{GeometryType, Snapshotable, Xfer};

use crate::common::INVALID_ID;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock, Weak};

// ---------------------------------------------------------------------------
// in_weapon_range_object helper (negation of out_of_weapon_range_object)
// Used by portable structure chase conditions in the attack state machine.
// C++ Reference: AIStates.cpp:1135 inWeaponRangeObject()
// ---------------------------------------------------------------------------

/// Returns true when the owner IS within weapon range of the goal object.
/// This is the logical inverse of `out_of_weapon_range_object_state`.
/// C++ Reference: AIStates.cpp:1135 inWeaponRangeObject()
pub(crate) fn in_weapon_range_object_state(base: &State) -> Result<bool, String> {
    let out_of_range = out_of_weapon_range_object_state(base)?;
    Ok(!out_of_range)
}

/// Typed version of in_weapon_range_object for the pursue state on portable
/// structures. When a rider (e.g., infantry on a tank) is attacking, it cannot
/// control its own movement, so the chase state is a no-op that falls back to
/// aim when the weapon is in range.
/// C++ Reference: AIStates.cpp:314-316 portableStructureChaseConditions
pub(crate) fn in_weapon_range_object_chase(
    state: &AIAttackPursueTargetState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    // C++ AIStates.cpp:313-326 portableStructureChaseConditions uses
    // inWeaponRangeObject — not a permanent true. Riders stay in AIM/FIRE
    // only when the current weapon can already reach the goal object.
    in_weapon_range_object_state(&state.base.base)
}

// ---------------------------------------------------------------------------
// Attack state machine for more complex attack behavior
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttackSubStateId {
    // C++ AIStateMachine.h AttackStateMachine::StateType
    PursueTarget = 0,   // CHASE_TARGET
    ApproachTarget = 1, // APPROACH_TARGET
    AimAtTarget = 2,    // AIM_AT_TARGET
    FireWeapon = 3,     // FIRE_WEAPON
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct AttackContinuationData {
    pub(crate) attack_type: AbleToAttackType,
    pub(crate) force_attacking: Bool,
}

pub(crate) fn out_of_weapon_range_object_state(base: &State) -> Result<bool, String> {
    let owner = base
        .get_machine_owner()
        .ok_or_else(|| "attack condition missing owner".to_string())?;
    let target_id = base
        .get_machine_goal_object_id()
        .ok_or_else(|| "attack condition missing target".to_string())?;
    let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
    let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
        return Ok(false);
    };
    if weapon.has_leech_range() {
        return Ok(false);
    }
    Ok(!weapon.is_within_attack_range(owner_guard.get_id(), Some(target_id), None))
    });
    return __owner_checkout.ok_or_else(|| "attack condition owner lock poisoned".to_string())?;
}

pub(crate) fn out_of_weapon_range_position_state(base: &State) -> Result<bool, String> {
    let owner = base
        .get_machine_owner()
        .ok_or_else(|| "attack condition missing owner".to_string())?;
    let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
    let Some(pos) = base.get_machine_goal_position() else {
        return Ok(false);
    };
    let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
        return Ok(false);
    };
    Ok(!weapon.is_within_attack_range(owner_guard.get_id(), None, Some(&pos)))
    });
    return __owner_checkout.ok_or_else(|| "attack condition owner lock poisoned".to_string())?;
}

pub(crate) fn want_to_squish_target_state(base: &State) -> Result<bool, String> {
    // Wave 257: empty dual-world → Ok(false).
    if dual_world_registry_unavailable() {
        return Ok(false);
    }

    let owner = base
        .get_machine_owner()
        .ok_or_else(|| "attack condition missing owner".to_string())?;
    let target_id = base
        .get_machine_goal_object_id()
        .ok_or_else(|| "attack condition missing target".to_string())?;
    let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
    let nested = crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target_guard| {

    if target_guard.get_contained_by().is_some() {
        return Ok(false);
    }

    let turret = owner_guard.ai_fire_which_turret;
    if turret == TurretType::Invalid {
        return Ok(false);
    }

    let is_computer = owner_guard
        .get_controlling_player()
        .and_then(|player_idx| {
            crate::player::with_player(player_idx, |player_guard| {
                player_guard.get_player_type() == PlayerType::Computer
            })
        })
        .unwrap_or(false);
    if !is_computer {
        return Ok(false);
    }

    if owner_guard.get_crusher_level() == 0 {
        return Ok(false);
    }

    if !target_guard.is_kind_of(KindOf::Infantry) {
        return Ok(false);
    }

    Ok(true)
    });
    nested.unwrap_or(Err("attack condition missing target".to_string()))
    });
    return __owner_checkout.ok_or_else(|| "attack condition owner lock poisoned".to_string())?;
}

pub(crate) fn cannot_possibly_attack_object_state(
    base: &State,
    user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    // Wave 257: empty dual-world → Ok(false).
    if dual_world_registry_unavailable() {
        return Ok(false);
    }

    let owner = base
        .get_machine_owner()
        .ok_or_else(|| "attack condition missing owner".to_string())?;
    let target_id = base
        .get_machine_goal_object_id()
        .ok_or_else(|| "attack condition missing target".to_string())?;
    let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
    let nested = crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target_guard| {

    if !owner_guard.is_able_to_attack() {
        return Ok(true);
    }

    let attack_type = user_data
        .data
        .as_ref()
        .and_then(|payload| payload.downcast_ref::<AttackContinuationData>())
        .map(|data| data.attack_type)
        .unwrap_or(AbleToAttackType::NewTarget);

    let cmd_source = owner_guard.ai_fire_last_command_source;

    let result =
        owner_guard.get_able_to_attack_specific_object(attack_type, &target_guard, cmd_source);
    Ok(!matches!(
        result,
        CanAttackResult::Possible | CanAttackResult::PossibleAfterMoving
    ))
    });
    nested.unwrap_or(Err("attack condition missing target".to_string()))
    });
    return __owner_checkout.ok_or_else(|| "attack condition owner lock poisoned".to_string())?;
}

pub(crate) fn out_of_weapon_range_object_aim(
    state: &AIAttackAimAtTargetState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    out_of_weapon_range_object_state(state.base_state())
}

pub(crate) fn out_of_weapon_range_object_fire(
    state: &AIAttackFireWeaponState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    out_of_weapon_range_object_state(state.base_state())
}

pub(crate) fn out_of_weapon_range_position_aim(
    state: &AIAttackAimAtTargetState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    out_of_weapon_range_position_state(state.base_state())
}

pub(crate) fn out_of_weapon_range_position_fire(
    state: &AIAttackFireWeaponState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    out_of_weapon_range_position_state(state.base_state())
}

pub(crate) fn want_to_squish_target_aim(
    state: &AIAttackAimAtTargetState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    want_to_squish_target_state(state.base_state())
}

pub(crate) fn want_to_squish_target_fire(
    state: &AIAttackFireWeaponState,
    _user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    want_to_squish_target_state(state.base_state())
}

pub(crate) fn cannot_possibly_attack_object_aim(
    state: &AIAttackAimAtTargetState,
    user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    cannot_possibly_attack_object_state(state.base_state(), user_data)
}

pub(crate) fn cannot_possibly_attack_object_fire(
    state: &AIAttackFireWeaponState,
    user_data: &StateTransitionUserData,
) -> Result<bool, String> {
    cannot_possibly_attack_object_state(state.base_state(), user_data)
}

pub struct AttackStateMachine {
    pub(crate) base: StateMachine,
    pub(crate) exit_conditions: Option<Box<dyn AttackExitConditionsInterface>>,
    pub(crate) follow: Bool,
    pub(crate) attacking_object: Bool,
    pub(crate) force_attacking: Bool,
}

impl std::fmt::Debug for AttackStateMachine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AttackStateMachine")
            .field("base", &self.base)
            .field("follow", &self.follow)
            .field("attacking_object", &self.attacking_object)
            .field("force_attacking", &self.force_attacking)
            .finish()
    }
}

/// Interface for attack exit conditions
pub trait AttackExitConditionsInterface: Send + Sync {
    fn should_exit(&self, machine: &StateMachine) -> bool;
}

impl AttackStateMachine {
    pub fn new(
        owner: crate::common::ObjectID,
        name: &str,
        follow: Bool,
        attacking_object: Bool,
        force_attacking: Bool,
    ) -> Self {
        let mut base = StateMachine::new_with_owner_id(owner, name);
        let aim_state = AIAttackAimAtTargetState::new(&base, attacking_object, force_attacking);
        let fire_state = AIAttackFireWeaponState::new(&base, attacking_object);
        let pursue_state =
            AIAttackPursueTargetState::new(&base, follow, attacking_object, force_attacking);
        let approach_state =
            AIAttackApproachTargetState::new(&base, follow, attacking_object, force_attacking);

        let object_conditions_aim = if force_attacking {
            vec![
                legacy_transition::<AIAttackAimAtTargetState>(
                    out_of_weapon_range_object_aim,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "out_of_weapon_range_object",
                ),
                legacy_transition::<AIAttackAimAtTargetState>(
                    cannot_possibly_attack_object_aim,
                    EXIT_MACHINE_WITH_FAILURE,
                    StateTransitionUserData::with_data(AttackContinuationData {
                        attack_type: AbleToAttackType::ContinuedTargetForced,
                        force_attacking,
                    }),
                    "cannot_possibly_attack_object",
                ),
                legacy_transition::<AIAttackAimAtTargetState>(
                    want_to_squish_target_aim,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "want_to_squish_target",
                ),
            ]
        } else {
            vec![
                legacy_transition::<AIAttackAimAtTargetState>(
                    out_of_weapon_range_object_aim,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "out_of_weapon_range_object",
                ),
                legacy_transition::<AIAttackAimAtTargetState>(
                    want_to_squish_target_aim,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "want_to_squish_target",
                ),
                legacy_transition::<AIAttackAimAtTargetState>(
                    cannot_possibly_attack_object_aim,
                    EXIT_MACHINE_WITH_FAILURE,
                    StateTransitionUserData::with_data(AttackContinuationData {
                        attack_type: AbleToAttackType::ContinuedTarget,
                        force_attacking,
                    }),
                    "cannot_possibly_attack_object",
                ),
            ]
        };

        let object_conditions_fire = if force_attacking {
            vec![
                legacy_transition::<AIAttackFireWeaponState>(
                    out_of_weapon_range_object_fire,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "out_of_weapon_range_object",
                ),
                legacy_transition::<AIAttackFireWeaponState>(
                    cannot_possibly_attack_object_fire,
                    EXIT_MACHINE_WITH_FAILURE,
                    StateTransitionUserData::with_data(AttackContinuationData {
                        attack_type: AbleToAttackType::ContinuedTargetForced,
                        force_attacking,
                    }),
                    "cannot_possibly_attack_object",
                ),
                legacy_transition::<AIAttackFireWeaponState>(
                    want_to_squish_target_fire,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "want_to_squish_target",
                ),
            ]
        } else {
            vec![
                legacy_transition::<AIAttackFireWeaponState>(
                    out_of_weapon_range_object_fire,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "out_of_weapon_range_object",
                ),
                legacy_transition::<AIAttackFireWeaponState>(
                    want_to_squish_target_fire,
                    AttackSubStateId::PursueTarget as u32,
                    StateTransitionUserData::new(),
                    "want_to_squish_target",
                ),
                legacy_transition::<AIAttackFireWeaponState>(
                    cannot_possibly_attack_object_fire,
                    EXIT_MACHINE_WITH_FAILURE,
                    StateTransitionUserData::with_data(AttackContinuationData {
                        attack_type: AbleToAttackType::ContinuedTarget,
                        force_attacking,
                    }),
                    "cannot_possibly_attack_object",
                ),
            ]
        };

        let position_conditions_aim = vec![legacy_transition::<AIAttackAimAtTargetState>(
            out_of_weapon_range_position_aim,
            AttackSubStateId::PursueTarget as u32,
            StateTransitionUserData::new(),
            "out_of_weapon_range_position",
        )];

        let position_conditions_fire = vec![legacy_transition::<AIAttackFireWeaponState>(
            out_of_weapon_range_position_fire,
            AttackSubStateId::PursueTarget as u32,
            StateTransitionUserData::new(),
            "out_of_weapon_range_position",
        )];

        register_classic_state(
            &mut base,
            AttackSubStateId::AimAtTarget as u32,
            aim_state,
            Some(AttackSubStateId::FireWeapon as u32),
            Some(EXIT_MACHINE_WITH_FAILURE),
            if attacking_object {
                &object_conditions_aim
            } else {
                &position_conditions_aim
            },
        );

        register_classic_state(
            &mut base,
            AttackSubStateId::FireWeapon as u32,
            fire_state,
            Some(AttackSubStateId::AimAtTarget as u32),
            Some(AttackSubStateId::AimAtTarget as u32),
            if attacking_object {
                &object_conditions_fire
            } else {
                &position_conditions_fire
            },
        );

        register_classic_state(
            &mut base,
            AttackSubStateId::PursueTarget as u32,
            pursue_state,
            Some(AttackSubStateId::ApproachTarget as u32),
            Some(AttackSubStateId::ApproachTarget as u32),
            &[],
        );

        register_classic_state(
            &mut base,
            AttackSubStateId::ApproachTarget as u32,
            approach_state,
            Some(AttackSubStateId::AimAtTarget as u32),
            Some(EXIT_MACHINE_WITH_FAILURE),
            &[],
        );

        Self {
            base,
            exit_conditions: None,
            follow,
            attacking_object,
            force_attacking,
        }
    }

    /// Set exit conditions
    pub fn set_exit_conditions(&mut self, conditions: Box<dyn AttackExitConditionsInterface>) {
        self.exit_conditions = Some(conditions);
    }

    /// Check if should exit attack
    pub fn should_exit_attack(&self) -> bool {
        if let Some(ref conditions) = self.exit_conditions {
            conditions.should_exit(&self.base)
        } else {
            false
        }
    }

    pub fn set_goal_object(&mut self, obj_id: Option<ObjectID>) {
        self.base.set_goal_object_by_id(obj_id);
    }

    pub fn get_goal_object_id(&self) -> ObjectID {
        self.base.get_goal_object_id()
    }

    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.base.set_goal_position(pos);
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.base.init_default_state()
    }

    pub fn set_state(&mut self, state: AttackSubStateId) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn update(&mut self) -> StateReturnType {
        if self.should_exit_attack() {
            return StateReturnType::Success;
        }
        self.base.update()
    }

    pub fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        if self.should_exit_attack() {
            return StateReturnType::Success;
        }
        self.base.update_with_ai(ai)
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer).map_err(|err| err.to_string())
    }

    pub fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer).map_err(|err| err.to_string())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process().map_err(|err| err.to_string())
    }
}

#[derive(Debug)]
pub struct AIAttackAimAtTargetState {
    pub(crate) base: State,
    pub(crate) attacking_object: Bool,
    pub(crate) force_attacking: Bool,
    pub(crate) can_turn_in_place: Bool,
    pub(crate) set_locomotor: Bool,
}

impl AIAttackAimAtTargetState {
    pub fn new(machine: &StateMachine, attacking_object: Bool, force_attacking: Bool) -> Self {
        Self {
            base: State::new(machine, "AIAttackAimAtTarget"),
            attacking_object,
            force_attacking,
            can_turn_in_place: false,
            set_locomotor: false,
        }
    }
}

impl StateImplementation for AIAttackAimAtTargetState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackAimAtTargetState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack aim missing owner".to_string())?;
        let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
        let weapon = owner_guard
            .get_current_weapon()
            .map(|(weapon, _slot)| weapon)
            .ok_or_else(|| "attack aim missing weapon".to_string())?;

        let target_pos = self.base.get_machine_goal_position();
        let mut in_range = false;
        let mut preventing = false;
        self.set_locomotor = false;

        self.can_turn_in_place = owner_guard.ai_fire_can_turn_in_place;

        let mut used_contain = false;
        if let Some(container_id) = owner_guard.get_contained_by() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object(container_id, |container_guard| {
                    if let Some(contain) = container_guard.get_contain() {
                        { let contain_guard = contain;
                            if contain_guard.is_enclosing_container_for(&*owner_guard) {
                                used_contain = true;
                                if let Some(goal_id) = self.base.get_machine_goal_object_id() {
                                    in_range = contain_guard.attempt_best_fire_point_position(
                                        owner,
                                        weapon,
                                        goal_id,
                                    );
                                } else if let Some(pos) = target_pos {
                                    in_range = contain_guard.attempt_best_fire_point_position_coord(
                                        owner,
                                        weapon,
                                        &pos,
                                    );
                                }
                            }
                        }
                    }
            });
        }

        if self.attacking_object {
            let target_id = self
                .base
                .get_machine_goal_object_id()
                .ok_or_else(|| "attack aim missing target".to_string())?;
            let missing_target = crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target_guard| {
            if !used_contain {
                in_range = weapon.is_within_attack_range(
                    owner_guard.get_id(),
                    Some(target_guard.get_id()),
                    None,
                );
            }
            if let Some(ai) = target_guard.get_ai_update_interface() {
                { let ai_guard = ai;
                    ai_guard.add_targeter(owner_guard.get_id(), true);
                    preventing = ai_guard.is_temporarily_preventing_aim_success();
                }
            }

            if owner_guard.ai_fire_turrets_linked {
                for turret in [TurretType::Primary, TurretType::Secondary] {
                    owner_guard.ai_pending_turret_objects.push((
                        turret,
                        Some(target_id),
                        self.force_attacking,
                    ));
                }
            } else {
                let turret = owner_guard.ai_fire_which_turret;
                if turret != TurretType::Invalid {
                    owner_guard.ai_pending_turret_objects.push((
                        turret,
                        Some(target_id),
                        self.force_attacking,
                    ));
                } else if weapon.is_contact_weapon() && in_range && !preventing {
                    return Some(StateReturnType::Success);
                }
            }
            None
            });
            if missing_target.is_none() {
                return Err("attack aim missing target".to_string());
            }
            if let Some(Some(code)) = missing_target {
                return Ok(code);
            }
        } else if let Some(pos) = target_pos {
            if !used_contain {
                let goal_id = self.base.get_machine_goal_object_id();
                in_range = if let Some(goal_id) = goal_id {
                    weapon.is_within_attack_range(owner_guard.get_id(), Some(goal_id), None)
                } else {
                    weapon.is_within_attack_range(owner_guard.get_id(), None, Some(&pos))
                };
            }
            if let Some(target_id) = self.base.get_machine_goal_object_id() {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target_guard| {
                    if let Some(ai) = target_guard.get_ai_update_interface() {
                        { let ai_guard = ai;
                            ai_guard.add_targeter(owner_guard.get_id(), true);
                            preventing = ai_guard.is_temporarily_preventing_aim_success();
                        }
                    }
                });
            if owner_guard.ai_fire_turrets_linked {
                for turret in [TurretType::Primary, TurretType::Secondary] {
                    owner_guard
                        .ai_pending_turret_positions
                        .push((turret, pos));
                }
            } else {
                let turret = owner_guard.ai_fire_which_turret;
                if turret != TurretType::Invalid {
                    owner_guard.ai_pending_turret_positions.push((turret, pos));
                } else if weapon.is_contact_weapon() && in_range && !preventing {
                    return Ok(StateReturnType::Success);
                }
            }
        } else {
            return Ok(StateReturnType::Failure);
        }
        }

        owner_guard.set_status(ObjectStatusMaskType::IS_AIMING_WEAPON, true);
        Ok(StateReturnType::Continue)
        });
        return __owner_checkout.ok_or_else(|| "attack aim owner lock poisoned".to_string())?;
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack aim missing owner".to_string())?;

        let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {

        if !owner_guard.has_any_weapon() {
            return Ok(StateReturnType::Failure);
        }

        let target_pos = if self.attacking_object {
            let target_id = self
                .base
                .get_machine_goal_object_id()
                .ok_or_else(|| "attack aim missing target".to_string())?;
            let aim_step = crate::object::registry::OBJECT_REGISTRY.with_object(target_id, |target_guard| {
            if target_guard.is_effectively_dead() {
                return Err(StateReturnType::Failure);
            }
            let turret = owner_guard.ai_fire_which_turret;
            if turret != TurretType::Invalid {
                owner_guard.ai_pending_turret_objects.push((
                    turret,
                    Some(target_id),
                    self.force_attacking,
                ));
                let turn_rate = if turret == TurretType::Primary {
                    owner_guard.ai_fire_primary_turn_rate
                } else {
                    owner_guard.ai_fire_secondary_turn_rate
                };
                if turn_rate != 0.0 {
                    return Err(StateReturnType::Continue);
                }
            }
            Ok(*target_guard.get_position())
            }).ok_or_else(|| "attack aim missing target".to_string())?;
            match aim_step {
                Err(code) => return Ok(code),
                Ok(pos) => pos,
            }
        } else if let Some(pos) = self.base.get_machine_goal_position() {
            let turret = owner_guard.ai_fire_which_turret;
            if turret != TurretType::Invalid {
                owner_guard.ai_pending_turret_positions.push((turret, pos));
                let turn_rate = if turret == TurretType::Primary {
                    owner_guard.ai_fire_primary_turn_rate
                } else {
                    owner_guard.ai_fire_secondary_turn_rate
                };
                if turn_rate != 0.0 {
                    return Ok(StateReturnType::Continue);
                }
            }
            pos
        } else {
            return Ok(StateReturnType::Failure);
        };

        let owner_pos = *owner_guard.get_position();
        let owner_angle = owner_guard.get_orientation();
        let dx = target_pos.x - owner_pos.x;
        let dy = target_pos.y - owner_pos.y;
        // C++ getRelativeAngle2D returns 0 when the target is on the unit.
        let rel_angle = if dx == 0.0 && dy == 0.0 {
            0.0
        } else {
            normalize_angle(dy.atan2(dx) - owner_angle)
        };

        const REL_THRESH: Real = 0.035;
        // C++ uses 0, then raises it to REL_THRESH, when there is no current weapon.
        let aim_delta = owner_guard
            .get_current_weapon()
            .map(|(weapon, _slot)| weapon.get_template().aim_delta)
            .unwrap_or(0.0)
            .max(REL_THRESH);

        if self.can_turn_in_place {
            if rel_angle.abs() > aim_delta {
                owner_guard.ai_pending_goal_orientation = Some(owner_angle + rel_angle);
                self.set_locomotor = true;
            }
        } else {
            owner_guard.ai_pending_goal_position = Some(target_pos);
        }

        if rel_angle.abs() < aim_delta {
            // C++ uses getMachineGoalObject even when this is not an object attack.
            let goal_id = self.base.get_machine_goal_object_id();
            let prevented = goal_id.and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY.with_object(id, |target_guard| {
                    if let Some(ai) = target_guard.get_ai_update_interface() {
                        { let ai_guard = ai;
                            ai_guard.add_targeter(owner_guard.get_id(), true);
                            return ai_guard.is_temporarily_preventing_aim_success();
                        }
                    }
                    false
                })
            }).unwrap_or(false);
            if prevented {
                return Ok(StateReturnType::Continue);
            }
            return Ok(StateReturnType::Success);
        }

        if owner_guard.is_disabled_by_type(DisabledType::Held) {
            let owner_id = owner_guard.get_id();
            let target_id = if self.attacking_object {
                self.base.get_machine_goal_object_id()
            } else {
                None
            };
            let in_range = owner_guard.get_current_weapon().is_some_and(|(weapon, _slot)| {
                if self.attacking_object {
                    target_id.is_some_and(|id| {
                        weapon.is_within_attack_range(owner_id, Some(id), None)
                    })
                } else {
                    weapon.is_within_attack_range(owner_id, None, Some(&target_pos))
                }
            });
            if !in_range {
                return Ok(StateReturnType::Failure);
            }
        }

        Ok(StateReturnType::Continue)
        });
        return __owner_checkout.ok_or_else(|| "attack aim owner lock poisoned".to_string())?;
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |guard| {
                guard.set_status(ObjectStatusMaskType::IS_AIMING_WEAPON, false);
                if self.can_turn_in_place && self.set_locomotor {
                    guard.ai_pending_goal_none = true;
                }
            });
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

/// C++ `AIAttackFireWeaponState::onEnter` (`AIStates.cpp:5153-5156`).
/// First shot from an AttackCommonTarget team seeds the shared victim.
pub(crate) fn should_seed_attack_common_target(
    has_victim: bool,
    attack_common_target: bool,
    team_target: ObjectID,
) -> bool {
    has_victim && attack_common_target && team_target == INVALID_ID
}

pub(crate) fn seed_team_target_if_attack_common(team: &mut Team, victim_id: ObjectID) {
    if should_seed_attack_common_target(
        true,
        team.attack_common_target(),
        team.get_team_target_object(),
    ) {
        team.set_team_target_object(victim_id);
    }
}

#[derive(Debug)]
pub struct AIAttackFireWeaponState {
    pub(crate) base: State,
    pub(crate) attacking_object: Bool,
}

impl AIAttackFireWeaponState {
    pub fn new(machine: &StateMachine, attacking_object: Bool) -> Self {
        Self {
            base: State::new(machine, "AIAttackFireWeapon"),
            attacking_object,
        }
    }
}

impl StateImplementation for AIAttackFireWeaponState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.goal_position_copied = Some(pos);
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIAttackFireWeaponState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack fire missing owner".to_string())?;
        let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {

        if !owner_guard.ai_fire_attack_ok {
            return Ok(StateReturnType::Failure);
        }

        let victim_id = self.base.get_machine_goal_object_id();
        // C++ AIAttackFireWeaponState::onEnter: first shot seeds AttackCommonTarget.
        if let Some(victim_id) = victim_id {
            if let Some(team_arc) = owner_guard.get_team() {
                crate::team::with_team_mut(team_arc, |team_guard| {
                    seed_team_target_if_attack_common(team_guard, victim_id);
                });
            }
        }

        owner_guard.set_status(
            ObjectStatusMaskType::from_status(ObjectStatusTypes::IsFiringWeapon),
            true,
        );
        owner_guard.pre_fire_current_weapon(victim_id);
        Ok(StateReturnType::Continue)
        });
        return __owner_checkout.ok_or_else(|| "attack fire owner lock poisoned".to_string())?;
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack fire missing owner".to_string())?;
        let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {

        let victim_id = self.base.get_machine_goal_object_id();
        if self.attacking_object {
            let dead = victim_id.and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY.with_object(id, |victim_guard| victim_guard.is_effectively_dead())
            });
            if dead != Some(false) {
                return Ok(StateReturnType::Failure);
            }
        }

        let (slot, status, continue_range) = {
            let (weapon, slot) = owner_guard
                .get_current_weapon()
                .ok_or_else(|| "attack fire missing weapon".to_string())?;
            (
                slot,
                weapon.get_status(),
                weapon.get_continue_attack_range(),
            )
        };
        if status == WeaponStatus::PreAttack {
            return Ok(StateReturnType::Continue);
        }
        if status != WeaponStatus::ReadyToFire {
            return Ok(StateReturnType::Failure);
        }
        let slot_ok = if owner_guard.ai_fire_turrets_linked
            || (!owner_guard.ai_fire_has_primary && !owner_guard.ai_fire_has_secondary)
        {
            true
        } else {
            match slot {
                WeaponSlotType::Primary => {
                    owner_guard.ai_fire_has_primary && owner_guard.ai_fire_primary_enabled
                }
                WeaponSlotType::Secondary => {
                    owner_guard.ai_fire_has_secondary && owner_guard.ai_fire_secondary_enabled
                }
                WeaponSlotType::Tertiary => {
                    !owner_guard.ai_fire_has_primary && !owner_guard.ai_fire_has_secondary
                }
            }
        };
        if !slot_ok {
            return Ok(StateReturnType::Failure);
        }

        owner_guard.set_firing_condition_for_current_weapon();

        if self.attacking_object {
            if let Some(vid) = victim_id {
                let snap = crate::object::registry::OBJECT_REGISTRY.with_object(vid, |target_guard| {
                    let _ = owner_guard.fire_current_weapon_at_object(target_guard);
                    (
                        target_guard.get_controlling_player_id(),
                        target_guard.is_destroyed()
                            || target_guard.is_effectively_dead()
                            || (target_guard.is_kind_of(KindOf::Mine)
                                && target_guard.test_status(ObjectStatusTypes::Masked)),
                    )
                });
                let victim_id = Some(vid);
                let (victim_player, should_continue) = snap.unwrap_or((None, false));

                if let Some(current_victim) = owner_guard.ai_fire_current_victim {
                    if Some(current_victim) != victim_id {
                        owner_guard.ai_fire_pending_victim = Some(current_victim);
                    }
                }

                owner_guard.clear_status(ObjectStatusMaskType::from_status(
                    ObjectStatusTypes::IgnoringStealth,
                ));

                if continue_range > 0.0 {

                    if should_continue {
                        // C++ uses getOriginalVictimPos only. A null pos skips the search.
                        if let Some(pos) = owner_guard.ai_fire_original_victim_pos {
                            if let Some(partition) = ThePartitionManager::get() {
                                let same_map_status = owner_guard.is_off_map();
                                let last_cmd_source = owner_guard.ai_fire_last_command_source;
                                let closest =
                                    partition.get_closest_object_2d(&pos, continue_range, |obj| {
                                        // C++ SamePlayer matches a null player to other null players.
                                        if obj.get_controlling_player_id() != victim_player {
                                            return false;
                                        }
                                        if obj.is_destroyed() || obj.is_effectively_dead() {
                                            return false;
                                        }
                                        if obj.is_off_map() != same_map_status {
                                            return false;
                                        }
                                        match owner_guard.get_able_to_attack_specific_object(
                                            AbleToAttackType::NewTarget,
                                            obj,
                                            last_cmd_source,
                                        ) {
                                            CanAttackResult::Possible
                                            | CanAttackResult::PossibleAfterMoving => true,
                                            _ => false,
                                        }
                                    });
                                if let Some(new_id) = closest {
                                    owner_guard.ai_fire_pending_victim = Some(new_id);
                                }
                            }
                        }
                    }
                }
            }
        } else {
            if let Some(pos) = self.base.get_machine_goal_position() {
                let linked = owner_guard.ai_fire_turrets_linked;

                if linked {
                    for slot_index in 0..crate::common::WEAPONSLOT_COUNT {
                        let slot = match slot_index {
                            0 => WeaponSlotType::Primary,
                            1 => WeaponSlotType::Secondary,
                            _ => WeaponSlotType::Tertiary,
                        };
                        owner_guard.fire_linked_turret_slot_at_position(slot, &pos);
                    }
                } else {
                    let _ = owner_guard.fire_current_weapon_at_position(&pos);
                }
            }

            // C++ clears this on the position arm whether or not a barrel fired.
            owner_guard.clear_status(ObjectStatusMaskType::from_status(
                ObjectStatusTypes::IgnoringStealth,
            ));
        }



        Ok(StateReturnType::Success)
        });
        return __owner_checkout.ok_or_else(|| "attack fire owner lock poisoned".to_string())?;
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack fire missing owner".to_string())?;
        let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {

        owner_guard.clear_status(ObjectStatusMaskType::from_status(
            ObjectStatusTypes::IsFiringWeapon,
        ));
        owner_guard.clear_status(ObjectStatusMaskType::from_status(
            ObjectStatusTypes::IgnoringStealth,
        ));

        if let Some((weapon, _)) = owner_guard.get_current_weapon() {
            if weapon.get_status() == WeaponStatus::PreAttack {
                owner_guard.cancel_pre_attack_for_current_weapon();
            }
        }

        Ok(())
        });
        return __owner_checkout.ok_or_else(|| "attack fire owner lock poisoned".to_string())?;
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

pub(crate) const ATTACK_MIN_RECOMPUTE_TIME: u32 = 10;

pub(crate) fn attack_view_blocked(
    source: &Object,
    victim: Option<&Object>,
    victim_pos: &Coord3D,
) -> bool {
    the_ai()
        .read()
        .ok()
        .and_then(|ai| {
            let pathfinder = ai.pathfinder()?;
            let guard = pathfinder.read().ok()?;
            Some(guard.is_attack_view_blocked_by_obstacle(
                source,
                source.get_position(),
                victim,
                victim_pos,
            ))
        })
        .unwrap_or(false)
}

pub(crate) fn attack_can_pursue(source: &Object, weapon: &Weapon, victim: &Object) -> bool {
    if victim.get_physics().is_none() {
        return false;
    }

    if source.ai_fire_which_turret == TurretType::Invalid {
        return false;
    }

    let ai_store = the_ai();
    let ai_crushes_infantry = ai_store
        .read()
        .ok()
        .and_then(|ai| {
            ai.get_ai_data()
                .read()
                .ok()
                .map(|data| data.ai_crushes_infantry)
        })
        .unwrap_or(true);
    if ai_crushes_infantry {
        let is_computer = source
            .get_controlling_player()
            .and_then(|player| {
                crate::player::with_player(player, |player_guard| {
                    player_guard.get_player_type() == PlayerType::Computer
                })
            })
            .unwrap_or(false);
        if is_computer && source.get_crusher_level() > 0 && victim.is_kind_of(KindOf::Infantry) {
            return true;
        }
    }

    if weapon.is_too_close(source.get_id(), Some(victim.get_id()), None) {
        return false;
    }

    let our_max_speed = source.ai_fire_locomotor_speed;
    if our_max_speed <= 0.0 {
        return false;
    }

    let victim_speed = victim
        .get_physics()
        .map(|physics| physics.get_forward_speed_2d())
        .unwrap_or(0.0);

    if victim_speed >= our_max_speed {
        return false;
    }
    if victim_speed < our_max_speed / 10.0 {
        return false;
    }

    let source_pos = source.get_position();
    let victim_pos = victim.get_position();
    let dx = victim_pos.x - source_pos.x;
    let dy = victim_pos.y - source_pos.y;
    let (victim_dir_x, victim_dir_y) = victim.get_unit_direction_vector_2d();
    if dx * victim_dir_x + dy * victim_dir_y < 0.0 {
        return false;
    }

    true
}

#[derive(Debug)]
pub struct AIAttackPursueTargetState {
    pub(crate) base: AIMoveToState,
    pub(crate) prev_victim_pos: Coord3D,
    pub(crate) approach_timestamp: UnsignedInt,
    pub(crate) follow: Bool,
    pub(crate) attacking_object: Bool,
    pub(crate) stop_if_in_range: Bool,
    pub(crate) is_initial_approach: Bool,
    pub(crate) force_attacking: Bool,
    live_no_path: Option<bool>,
    live_stuck: Option<bool>,
}

impl AIAttackPursueTargetState {
    pub fn new(
        machine: &StateMachine,
        follow: Bool,
        attacking_object: Bool,
        force_attacking: Bool,
    ) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIAttackPursueTargetState".to_string();
        base.is_move_to = false;
        Self {
            base,
            prev_victim_pos: Coord3D::new(0.0, 0.0, 0.0),
            approach_timestamp: 0,
            follow,
            attacking_object,
            stop_if_in_range: false,
            is_initial_approach: true,
            force_attacking,
            live_no_path: None,
            live_stuck: None,
        }
    }

    pub(crate) fn compute_path(&mut self) -> Result<bool, String> {
        let live_no_path = self.live_no_path.take();
        let live_stuck = self.live_stuck.take();
        // Wave 257: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack pursue missing owner".to_string())?;
        let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
        if owner_guard.is_kind_of(KindOf::Immobile) {
            return Ok(false);
        }

        if live_stuck.unwrap_or(owner_guard.ai_fire_blocked_and_stuck) {
            return Ok(false);
        }
        if self.base.waiting_for_path {
            return Ok(true);
        }

        let mut force_repath = false;
        if live_no_path.unwrap_or(
            !owner_guard.ai_fire_has_path && !owner_guard.ai_fire_waiting_for_path,
        ) {
            force_repath = true;
        }
        if !force_repath
            && TheGameLogic::get_frame().saturating_sub(self.approach_timestamp)
                < ATTACK_MIN_RECOMPUTE_TIME
        {
            return Ok(true);
        }

        self.approach_timestamp = TheGameLogic::get_frame();

        let Some(victim_id) = self.base.base.get_machine_goal_object_id() else {
            return Ok(false);
        };
        let pursue = crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |victim_guard| {
            if !force_repath
                && self.base.is_same_position(
                    owner_guard.get_position(),
                    &self.prev_victim_pos,
                    victim_guard.get_position(),
                )
            {
                return Err(true);
            }
            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return Err(false);
            };
            if !attack_can_pursue(owner_guard, weapon, victim_guard) {
                return Err(false);
            }
            Ok(*victim_guard.get_position())
        });
        let Some(pursue) = pursue else {
            return Ok(false);
        };
        let victim_pos = match pursue {
            Err(flag) => return Ok(flag),
            Ok(pos) => pos,
        };
        self.prev_victim_pos = victim_pos;
        self.base.set_adjusts_destination(true);
        self.base.goal_position = self.prev_victim_pos;
        self.base.waiting_for_path = true;
        owner_guard.ai_pending_path_goal = Some(self.base.goal_position);
        self.stop_if_in_range = false;

        Ok(true)
        });
        return __owner_checkout.ok_or_else(|| "attack pursue owner lock poisoned".to_string())?;
    }

    pub(crate) fn update_internal(
        &mut self,
        ai: Option<&mut dyn crate::modules::AIUpdateInterface>,
    ) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack pursue missing owner".to_string())?;

        if self
            .base
            .base
            .get_machine_goal_object_id()
            .and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(id, |guard| guard.is_effectively_dead())
            })
            .unwrap_or(true)
        {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                owner_guard.ai_pending_victim_dead = true;
            });
            return Ok(StateReturnType::Failure);
        }

        self.stop_if_in_range = false;

        let Some(victim_id) = self.base.base.get_machine_goal_object_id() else {
            return Ok(StateReturnType::Failure);
        };
        let stealthed = crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |victim_guard| {
            victim_guard.test_status(ObjectStatusTypes::Stealthed)
                && !victim_guard.test_status(ObjectStatusTypes::Detected)
                && !victim_guard.test_status(ObjectStatusTypes::Disguised)
        });
        if stealthed.is_none() {
            return Ok(StateReturnType::Failure);
        }
        if stealthed == Some(true) {
            return Ok(StateReturnType::Failure);
        }

        if !self.compute_path()? {
            return Ok(StateReturnType::Failure);
        }

        let code = if let Some(ai) = ai {
            self.base.update_with_ai(ai)
        } else {
            self.base.classic_on_update()?
        };
        if code != StateReturnType::Continue {
            return Ok(StateReturnType::Success);
        }

        let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
            owner,
            |owner_guard| -> Result<StateReturnType, String> {
                // C++ AttackPursueGoalState::update: victim borrowed from the
                // registry while the owner stays checked out (different ids).
                let __victim_checkout = crate::object::registry::OBJECT_REGISTRY
                    .with_object(victim_id, |victim_guard| -> Result<StateReturnType, String> {
                        let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                            return Ok(StateReturnType::Failure);
                        };
                        let turret = owner_guard.ai_fire_which_turret;
                        if turret == TurretType::Invalid {
                            return Ok(StateReturnType::Success);
                        }

                        let view_blocked = attack_view_blocked(
                            owner_guard,
                            Some(victim_guard),
                            victim_guard.get_position(),
                        );
                        if !view_blocked
                            && weapon.is_within_attack_range(
                                owner_guard.get_id(),
                                Some(victim_guard.get_id()),
                                None,
                            )
                        {
                            owner_guard.ai_pending_turret_objects.push((
                                turret,
                                Some(victim_guard.get_id()),
                                self.force_attacking,
                            ));
                            self.is_initial_approach = false;

                            let mut desired_speed = victim_guard
                                .get_physics()
                                .map(|physics| physics.get_forward_speed_2d())
                                .unwrap_or(FAST_AS_POSSIBLE);
                            desired_speed *= 0.95;
                            if owner_guard.can_crush_or_squish(
                                victim_guard,
                                CrushSquishTestType::TestCrushOrSquish,
                            ) {
                                desired_speed = FAST_AS_POSSIBLE;
                            }
                            owner_guard.ai_pending_desired_speed = Some(desired_speed.max(0.0));
                        } else {
                            owner_guard.ai_pending_desired_speed = Some(FAST_AS_POSSIBLE);
                        }

                        Ok(code)
                    });
                __victim_checkout.unwrap_or_else(|| Ok(StateReturnType::Failure))
            },
        );
        return __owner_checkout.ok_or_else(|| "attack pursue owner lock poisoned".to_string())?;
    }
}

impl StateImplementation for AIAttackPursueTargetState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.base.goal_position_copied = Some(pos);
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        self.live_no_path = Some(ai.get_path().is_none() && !ai.is_waiting_for_path());
        self.live_stuck = Some(ai.is_blocked_and_stuck());
        let code = match self.update_internal(Some(ai)) {
            Ok(code) => code,
            Err(_) => return StateReturnType::Failure,
        };
        if self.is_initial_approach {
            if let Some(owner) = self.base.base.get_machine_owner() {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                    let turret = owner_guard.ai_fire_which_turret;
                    if turret != TurretType::Invalid {
                        if let Some(mood_id) = owner_guard.ai_fire_mood_target {
                            owner_guard.ai_pending_turret_objects.push((
                                turret,
                                Some(mood_id),
                                self.force_attacking,
                            ));
                        }
                    }
                });
            }
        }
        code
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackPursueTargetState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack pursue missing owner".to_string())?;
let __early = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<Option<StateReturnType>, String> {

        if owner_guard.is_kind_of(KindOf::Projectile) {
            return Ok(Some(StateReturnType::Success));
        }
        if self
            .base
            .base
            .get_machine_goal_object_id()
            .and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(id, |guard| guard.is_effectively_dead())
            })
            .unwrap_or(true)
        {
            return Ok(Some(StateReturnType::Success));
        }
        if !self.attacking_object {
            return Ok(Some(StateReturnType::Success));
        }

        self.base.set_adjusts_destination(false);

        if owner_guard.ai_fire_state_id != Some(AIStateType::GuardRetaliate as u32) {
            let is_human = owner_guard
                .get_controlling_player()
                .and_then(|player| {
                    crate::player::with_player(player, |player_guard| {
                        player_guard.get_player_type() == PlayerType::Human
                    })
                })
                .unwrap_or(false);
            if is_human
                && owner_guard.ai_fire_last_command_source == CommandSourceType::FromAi
            {
                return Ok(Some(StateReturnType::Success));
            }
        }

        self.prev_victim_pos = Coord3D::new(0.0, 0.0, 0.0);
        self.approach_timestamp = 0u32.wrapping_sub(ATTACK_MIN_RECOMPUTE_TIME);

        let Some(victim_id) = self.base.base.get_machine_goal_object_id() else {
            return Ok(Some(StateReturnType::Success));
        };
        let pursue = crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |victim_guard| {
            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return Some(StateReturnType::Failure);
            };
            if !attack_can_pursue(owner_guard, weapon, victim_guard) {
                return Some(StateReturnType::Success);
            }
            let turret = owner_guard.ai_fire_which_turret;
            if turret == TurretType::Invalid {
                return Some(StateReturnType::Success);
            }
            owner_guard.ai_pending_turret_objects.push((
                turret,
                Some(victim_id),
                self.force_attacking,
            ));
            None
        });
        if pursue.is_none() {
            return Ok(Some(StateReturnType::Success));
        }
        if let Some(Some(code)) = pursue {
            return Ok(Some(code));
        }
        Ok(None)
        });
        let __early = __early.ok_or_else(|| "attack pursue owner lock poisoned".to_string())?;
        if let Some(code) = __early? {
            return Ok(code);
        }

        if !self.compute_path()? {
            return Ok(StateReturnType::Success);
        }

        self.base.classic_on_enter()
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let code = self.update_internal(None)?;

        if self.is_initial_approach {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack pursue missing owner".to_string())?;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                let turret = owner_guard.ai_fire_which_turret;
                if turret != TurretType::Invalid {
                    if let Some(mood_id) = owner_guard.ai_fire_mood_target {
                        owner_guard.ai_pending_turret_objects.push((
                            turret,
                            Some(mood_id),
                            self.force_attacking,
                        ));
                    }
                }
            });
        }

        Ok(code)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;
        self.is_initial_approach = false;
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct AIAttackApproachTargetState {
    pub(crate) base: AIMoveToState,
    pub(crate) prev_victim_pos: Coord3D,
    pub(crate) approach_timestamp: UnsignedInt,
    pub(crate) follow: Bool,
    pub(crate) attacking_object: Bool,
    pub(crate) stop_if_in_range: Bool,
    pub(crate) is_initial_approach: Bool,
    pub(crate) force_attacking: Bool,
    live_no_path: Option<bool>,
    live_stuck: Option<bool>,
}

impl AIAttackApproachTargetState {
    pub fn new(
        machine: &StateMachine,
        follow: Bool,
        attacking_object: Bool,
        force_attacking: Bool,
    ) -> Self {
        let mut base = AIMoveToState::new(machine);
        base.base.name = "AIAttackApproachTargetState".to_string();
        base.is_move_to = false;
        Self {
            base,
            prev_victim_pos: Coord3D::new(0.0, 0.0, 0.0),
            approach_timestamp: 0,
            follow,
            attacking_object,
            stop_if_in_range: false,
            is_initial_approach: true,
            force_attacking,
            live_no_path: None,
            live_stuck: None,
        }
    }

    pub(crate) fn compute_path(&mut self) -> Result<bool, String> {
        let live_no_path = self.live_no_path.take();
        let live_stuck = self.live_stuck.take();
        // Wave 257: empty dual-world → Ok(false).
        if dual_world_registry_unavailable() {
            return Ok(false);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack approach missing owner".to_string())?;
        let __owner_checkout = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
        if owner_guard.is_kind_of(KindOf::Immobile) {
            return Ok(false);
        }

        let mut force_repath = live_stuck.unwrap_or(owner_guard.ai_fire_blocked_and_stuck);
        if self.base.waiting_for_path {
            return Ok(true);
        }
        if !force_repath
            && live_no_path.unwrap_or(
                !owner_guard.ai_fire_has_path && !owner_guard.ai_fire_waiting_for_path,
            )
        {
            force_repath = true;
        }
        if !force_repath
            && TheGameLogic::get_frame().saturating_sub(self.approach_timestamp)
                < ATTACK_MIN_RECOMPUTE_TIME
        {
            return Ok(true);
        }

        self.approach_timestamp = TheGameLogic::get_frame();

        if let Some(victim_id) = self.base.base.get_machine_goal_object_id() {
            if let Some(__victim_step) = crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |victim_guard| {
            if !force_repath
                && self.base.is_same_position(
                    owner_guard.get_position(),
                    &self.prev_victim_pos,
                    victim_guard.get_position(),
                )
            {
                return Ok(true);
            }

            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return Ok(false);
            };

            self.prev_victim_pos = *victim_guard.get_position();
            if attack_can_pursue(&owner_guard, weapon, &victim_guard) {
                return Ok(false);
            }

            self.base.set_adjusts_destination(true);
            if weapon.is_contact_weapon() {
                owner_guard.ai_pending_ignore_id = Some(victim_guard.get_id());
                self.base.set_adjusts_destination(false);
                owner_guard.ai_pending_path_extra = Some(10.0 * PATHFIND_CELL_SIZE_F);
            }

            self.base.goal_position = self.prev_victim_pos;
            self.base.waiting_for_path = true;
            let victim_center = victim_guard
                .get_geometry_info()
                .get_center_position(victim_guard.get_position());
            owner_guard.ai_pending_attack_path = Some((victim_guard.get_id(), victim_center));
            self.stop_if_in_range = false;
            Ok(true)
            }) {
                if let Some(step) = __victim_step {
                    return step;
                }
            }
        }

        self.base.set_adjusts_destination(true);
        self.stop_if_in_range = false;
        let Some(goal_position) = self.base.base.get_machine_goal_position() else {
            return Ok(false);
        };
        self.base.goal_position = goal_position;
        if !force_repath {
            return Ok(true);
        }
        self.base.waiting_for_path = true;
        owner_guard.ai_pending_attack_path = Some((INVALID_ID, self.base.goal_position));
        Ok(true)
        });
        return __owner_checkout.ok_or_else(|| "attack approach owner lock poisoned".to_string())?;
    }

    pub(crate) fn update_internal(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack approach missing owner".to_string())?;

        if self
            .base
            .base
            .get_machine_goal_object_id()
            .and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(id, |guard| guard.is_effectively_dead())
            })
            .unwrap_or(false)
        {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                owner_guard.ai_pending_victim_dead = true;
            });
            return Ok(StateReturnType::Failure);
        }

        self.stop_if_in_range = false;

        if let Some(victim_id) = self.base.base.get_machine_goal_object_id() {
            if let Some(__victim_step) = crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |victim_guard| {
                let early = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                if owner_guard
                    .get_controlling_player()
                    .and_then(|player| {
                        crate::player::with_player(player, |player_guard| {
                            player_guard.get_player_type() == PlayerType::Computer
                        })
                    })
                    .unwrap_or(false)
                {
                    let hunt = owner_guard.ai_fire_state_id == Some(AIStateType::Hunt as u32);
                    if !hunt
                        && victim_guard.is_kind_of(KindOf::Aircraft)
                        && victim_guard.is_airborne_target()
                    {
                        return Some(StateReturnType::Failure);
                    }
                }
                if victim_guard.test_status(ObjectStatusTypes::Stealthed)
                    && !victim_guard.test_status(ObjectStatusTypes::Detected)
                    && !victim_guard.test_status(ObjectStatusTypes::Disguised)
                {
                    return Some(StateReturnType::Failure);
                }

                if let Some((weapon, _slot)) = owner_guard.get_current_weapon() {
                    if weapon.is_contact_weapon()
                        && weapon.is_within_attack_range(
                            owner_guard.get_id(),
                            Some(victim_guard.get_id()),
                            None,
                        )
                    {
                        return Some(StateReturnType::Success);
                    }
                    if self.stop_if_in_range
                        && weapon.is_within_attack_range(
                            owner_guard.get_id(),
                            Some(victim_guard.get_id()),
                            None,
                        )
                        && !attack_view_blocked(
                            &owner_guard,
                            Some(&victim_guard),
                            victim_guard.get_position(),
                        )
                    {
                        return Some(StateReturnType::Success);
                    }
                }
                    None
                });
                if early.is_none() {
                    return Err("attack approach owner lock poisoned".to_string());
                }
                if let Some(Some(code)) = early {
                    return Ok(code);
                }
            }) {
                if let Some(step) = __victim_step {
                    return step;
                }
            }
            if !self.compute_path()? {
                return Ok(StateReturnType::Success);
            }
            let code = self.base.classic_on_update()?;
            if code != StateReturnType::Continue {
                return Ok(StateReturnType::Success);
            }
            return Ok(code);
        }

        {
            let in_range = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                if self.stop_if_in_range {
                    if let Some((weapon, _slot)) = owner_guard.get_current_weapon() {
                        if weapon.is_within_attack_range(
                            owner_guard.get_id(),
                            None,
                            Some(&self.base.goal_position),
                        ) && !attack_view_blocked(owner_guard, None, &self.base.goal_position)
                        {
                            return true;
                        }
                    }
                }
                false
            }).unwrap_or(false);
            if in_range {
                return Ok(StateReturnType::Success);
            }
        }

        if !self.compute_path()? {
            return Ok(StateReturnType::Failure);
        }
        Ok(StateReturnType::Continue)
    }
}

impl StateImplementation for AIAttackApproachTargetState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn bind_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.base.base.goal_object_id = id;
    }

    fn bind_goal_position(&mut self, pos: Coord3D) {
        self.base.base.goal_position_copied = Some(pos);
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        self.live_no_path = Some(ai.get_path().is_none() && !ai.is_waiting_for_path());
        self.live_stuck = Some(ai.is_blocked_and_stuck());
        let prepared = match self.update_internal() {
            Ok(code) => code,
            Err(_) => return StateReturnType::Failure,
        };
        if prepared != StateReturnType::Continue {
            return prepared;
        }
        let mut code = self.base.update_with_ai(ai);
        if self.follow && self.attacking_object {
            let keep_following = self
                .base
                .base
                .get_machine_owner()
                .and_then(|owner| {
                    crate::object::registry::OBJECT_REGISTRY
                        .with_object(owner, |owner_guard| !owner_guard.is_kind_of(KindOf::Immobile))
                })
                .flatten()
                .unwrap_or(false)
                && self
                    .base
                    .base
                    .get_machine_goal_object_id()
                    .and_then(|id| {
                        crate::object::registry::OBJECT_REGISTRY
                            .with_object(id, |victim_guard| !victim_guard.is_kind_of(KindOf::Immobile))
                    })
                    .flatten()
                    .unwrap_or(false);
            if keep_following {
                if code != StateReturnType::Continue {
                    self.is_initial_approach = false;
                }
                code = StateReturnType::Continue;
            }
        }
        if self.is_initial_approach {
            if let Some(owner) = self.base.base.get_machine_owner() {
                let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                    let turret = owner_guard.ai_fire_which_turret;
                    if turret != TurretType::Invalid {
                        if let Some(mood_id) = owner_guard.ai_fire_mood_target {
                            owner_guard.ai_pending_turret_objects.push((
                                turret,
                                Some(mood_id),
                                self.force_attacking,
                            ));
                        }
                    }
                });
            }
        }
        code
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }

    fn xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }
}

impl ClassicState for AIAttackApproachTargetState {
    fn base_state(&self) -> &State {
        &self.base.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base.base
    }

    fn classic_xfer_snapshot(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        Snapshotable::xfer(self, xfer)
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let owner = self
            .base
            .base
            .get_machine_owner()
            .ok_or_else(|| "attack approach missing owner".to_string())?;
let __early = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| -> Result<Option<StateReturnType>, String> {

        if self
            .base
            .base
            .get_machine_goal_object_id()
            .and_then(|id| {
                crate::object::registry::OBJECT_REGISTRY
                    .with_object(id, |guard| guard.is_effectively_dead())
            })
            .unwrap_or(false)
        {
            return Ok(Some(StateReturnType::Success));
        }

        self.prev_victim_pos = Coord3D::new(0.0, 0.0, 0.0);
        self.approach_timestamp = 0u32.wrapping_sub(ATTACK_MIN_RECOMPUTE_TIME);

        if let Some(victim_id) = self.base.base.get_machine_goal_object_id() {
            if let Some(__victim_step) = crate::object::registry::OBJECT_REGISTRY.with_object(victim_id, |victim_guard| {
            let Some((weapon, _slot)) = owner_guard.get_current_weapon() else {
                return Ok(Some(StateReturnType::Failure));
            };
            if weapon.is_within_attack_range(
                owner_guard.get_id(),
                Some(victim_guard.get_id()),
                None,
            ) && !attack_view_blocked(
                &owner_guard,
                Some(&victim_guard),
                victim_guard.get_position(),
            ) {
                return Ok(Some(StateReturnType::Success));
            }

            if owner_guard.ai_fire_state_id != Some(AIStateType::GuardRetaliate as u32) {
                let is_human = owner_guard
                    .get_controlling_player()
                    .and_then(|player| {
                        crate::player::with_player(player, |player_guard| {
                            player_guard.get_player_type() == PlayerType::Human
                        })
                    })
                    .unwrap_or(false);
                if is_human
                    && owner_guard.ai_fire_last_command_source == CommandSourceType::FromAi
                    && !weapon.is_contact_weapon()
                {
                    return Ok(Some(StateReturnType::Failure));
                }

                let is_computer = owner_guard
                    .get_controlling_player()
                    .and_then(|player| {
                        crate::player::with_player(player, |player_guard| {
                            player_guard.get_player_type() == PlayerType::Computer
                        })
                    })
                    .unwrap_or(false);
                if is_computer
                    && owner_guard.ai_fire_state_id != Some(AIStateType::Hunt as u32)
                    && victim_guard.is_kind_of(KindOf::Aircraft)
                    && victim_guard.is_airborne_target()
                {
                    return Ok(Some(StateReturnType::Failure));
                }
            }

            if attack_can_pursue(&owner_guard, weapon, &victim_guard) {
                return Ok(Some(StateReturnType::Success));
            }
            Ok(None)
            }) {
                if let Some(step) = __victim_step {
                    return step;
                }
            }
        } else {
            owner_guard.ai_pending_destroy_path = true;
        }

        let turret = owner_guard.ai_fire_which_turret;
        if turret != TurretType::Invalid {
            if self.attacking_object {
                if let Some(victim_id) = self.base.base.get_machine_goal_object_id() {
                    owner_guard.ai_pending_turret_objects.push((
                        turret,
                        Some(victim_id),
                        self.force_attacking,
                    ));
                }
            } else if let Some(goal_position) = self.base.base.get_machine_goal_position() {
                owner_guard
                    .ai_pending_turret_positions
                    .push((turret, goal_position));
            }
        }
        Ok(None)
        });
        let __early = __early.ok_or_else(|| "attack approach owner lock poisoned".to_string())?;
        if let Some(code) = __early? {
            return Ok(code);
        }

        if !self.compute_path()? {
            return Ok(StateReturnType::Failure);
        }

        self.base.set_adjusts_destination(false);
        let ret = self.base.classic_on_enter();
        self.base.set_adjusts_destination(true);
        ret
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        let mut code = self.update_internal()?;
        if code == StateReturnType::Continue {
            code = self.base.classic_on_update()?;
        }

        if self.follow && self.attacking_object {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack approach missing owner".to_string())?;
            let keep_following = crate::object::registry::OBJECT_REGISTRY.with_object(owner, |owner_guard| {
                self.base.base.get_machine_goal_object_id().and_then(|id| {
                    crate::object::registry::OBJECT_REGISTRY.with_object(id, |victim_guard| {
                        !owner_guard.is_kind_of(KindOf::Immobile)
                            && !victim_guard.is_kind_of(KindOf::Immobile)
                    })
                }).flatten().unwrap_or(false)
            }).unwrap_or(false);

            if keep_following {
                if code != StateReturnType::Continue {
                    self.is_initial_approach = false;
                }
                code = StateReturnType::Continue;
            }
        }

        if self.is_initial_approach {
            let owner = self
                .base
                .base
                .get_machine_owner()
                .ok_or_else(|| "attack approach missing owner".to_string())?;
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                let turret = owner_guard.ai_fire_which_turret;
                if turret != TurretType::Invalid {
                    if let Some(mood_id) = owner_guard.ai_fire_mood_target {
                        owner_guard.ai_pending_turret_objects.push((
                            turret,
                            Some(mood_id),
                            self.force_attacking,
                        ));
                    }
                }
            });
        }

        Ok(code)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        self.base.classic_on_exit(_exit)?;

        if let Some(owner) = self.base.base.get_machine_owner() {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner, |owner_guard| {
                if owner_guard.ai_fire_ground_movement {
                    let dx = self.base.goal_position.x - owner_guard.get_position().x;
                    let dy = self.base.goal_position.y - owner_guard.get_position().y;
                    if dx * dx + dy * dy
                        < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * 0.125
                    {
                        let _ = owner_guard.set_position(&self.base.goal_position);
                    }
                }
                owner_guard.ai_pending_clear_ignore = true;
            });
        }

        self.is_initial_approach = false;
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        true
    }
}

#[derive(Debug)]
pub struct AIAttackMoveStateMachine {
    pub(crate) base: StateMachine,
}

impl AIAttackMoveStateMachine {
    pub fn new(owner: crate::common::ObjectID, name: &str) -> Self {
        let mut base = StateMachine::new_with_owner_id(owner, name);
        let idle_state = AIIdleState::new(&base, false);
        register_classic_state(
            &mut base,
            AIStateType::Idle as u32,
            idle_state,
            None,
            None,
            &[],
        );
        let pickup_state = AIPickUpCrateState::new(&base);
        register_classic_state(
            &mut base,
            AIStateType::PickUpCrate as u32,
            pickup_state,
            None,
            None,
            &[],
        );
        let attack_state = AIAttackObjectState::new(&base, false, true);
        register_classic_state(
            &mut base,
            AIStateType::AttackObject as u32,
            attack_state,
            None,
            None,
            &[],
        );
        Self { base }
    }

    pub fn clear(&mut self) {
        self.base.clear();
    }

    pub fn init_default_state(&mut self) -> StateReturnType {
        self.base.init_default_state()
    }

    pub fn set_state(&mut self, state: AIStateType) -> StateReturnType {
        self.base.set_current_state(state as u32)
    }

    pub fn set_goal_object(&mut self, obj: Option<ObjectID>) {
        self.base.set_goal_object_by_id(obj);
    }

    pub fn set_goal_object_by_id(&mut self, object_id: Option<ObjectID>) {
        self.base.set_goal_object_by_id(object_id);
    }

    pub fn update(&mut self) -> StateReturnType {
        self.base.update()
    }

    pub fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::AIUpdateInterface,
    ) -> StateReturnType {
        self.base.update_with_ai(ai)
    }

    pub fn is_in_idle_state(&self) -> bool {
        self.base.is_in_idle_state()
    }

    pub fn is_in_attack_state(&self) -> bool {
        self.base.is_in_attack_state()
    }

    pub fn is_in_guard_idle_state(&self) -> bool {
        self.base.is_in_guard_idle_state()
    }

    pub fn halt(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.halt()
    }
}

impl Snapshotable for AIAttackPursueTargetState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        Snapshotable::crc(&self.base, xfer)?;
        let mut prev_victim_pos = self.prev_victim_pos.clone();
        xfer.xfer_coord3d(&mut prev_victim_pos);
        let mut approach_timestamp = self.approach_timestamp;
        xfer.xfer_unsigned_int(&mut approach_timestamp)
            .map_err(|e| format!("Failed to crc pursue approach_timestamp: {:?}", e))?;
        let mut follow = self.follow;
        xfer.xfer_bool(&mut follow)
            .map_err(|e| format!("Failed to crc pursue follow: {:?}", e))?;
        let mut attacking_object = self.attacking_object;
        xfer.xfer_bool(&mut attacking_object)
            .map_err(|e| format!("Failed to crc pursue attacking_object: {:?}", e))?;
        let mut stop_if_in_range = self.stop_if_in_range;
        xfer.xfer_bool(&mut stop_if_in_range)
            .map_err(|e| format!("Failed to crc pursue stop_if_in_range: {:?}", e))?;
        let mut is_initial_approach = self.is_initial_approach;
        xfer.xfer_bool(&mut is_initial_approach)
            .map_err(|e| format!("Failed to crc pursue is_initial_approach: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_coord3d(&mut self.prev_victim_pos);
        xfer.xfer_unsigned_int(&mut self.approach_timestamp)
            .map_err(|e| format!("Failed to xfer pursue approach_timestamp: {:?}", e))?;
        xfer.xfer_bool(&mut self.follow)
            .map_err(|e| format!("Failed to xfer pursue follow: {:?}", e))?;
        xfer.xfer_bool(&mut self.attacking_object)
            .map_err(|e| format!("Failed to xfer pursue attacking_object: {:?}", e))?;
        xfer.xfer_bool(&mut self.stop_if_in_range)
            .map_err(|e| format!("Failed to xfer pursue stop_if_in_range: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_initial_approach)
            .map_err(|e| format!("Failed to xfer pursue is_initial_approach: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl Snapshotable for AIAttackApproachTargetState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        Snapshotable::crc(&self.base, xfer)?;
        let mut prev_victim_pos = self.prev_victim_pos.clone();
        xfer.xfer_coord3d(&mut prev_victim_pos);
        let mut approach_timestamp = self.approach_timestamp;
        xfer.xfer_unsigned_int(&mut approach_timestamp)
            .map_err(|e| format!("Failed to crc approach approach_timestamp: {:?}", e))?;
        let mut follow = self.follow;
        xfer.xfer_bool(&mut follow)
            .map_err(|e| format!("Failed to crc approach follow: {:?}", e))?;
        let mut attacking_object = self.attacking_object;
        xfer.xfer_bool(&mut attacking_object)
            .map_err(|e| format!("Failed to crc approach attacking_object: {:?}", e))?;
        let mut stop_if_in_range = self.stop_if_in_range;
        xfer.xfer_bool(&mut stop_if_in_range)
            .map_err(|e| format!("Failed to crc approach stop_if_in_range: {:?}", e))?;
        let mut is_initial_approach = self.is_initial_approach;
        xfer.xfer_bool(&mut is_initial_approach)
            .map_err(|e| format!("Failed to crc approach is_initial_approach: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        Snapshotable::xfer(&mut self.base, xfer)?;
        xfer.xfer_coord3d(&mut self.prev_victim_pos);
        xfer.xfer_unsigned_int(&mut self.approach_timestamp)
            .map_err(|e| format!("Failed to xfer approach approach_timestamp: {:?}", e))?;
        xfer.xfer_bool(&mut self.follow)
            .map_err(|e| format!("Failed to xfer approach follow: {:?}", e))?;
        xfer.xfer_bool(&mut self.attacking_object)
            .map_err(|e| format!("Failed to xfer approach attacking_object: {:?}", e))?;
        xfer.xfer_bool(&mut self.stop_if_in_range)
            .map_err(|e| format!("Failed to xfer approach stop_if_in_range: {:?}", e))?;
        xfer.xfer_bool(&mut self.is_initial_approach)
            .map_err(|e| format!("Failed to xfer approach is_initial_approach: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Snapshotable::load_post_process(&mut self.base)
    }
}

impl Snapshotable for AIAttackAimAtTargetState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut can_turn_in_place = self.can_turn_in_place;
        xfer.xfer_bool(&mut can_turn_in_place)
            .map_err(|e| format!("Failed to crc can_turn_in_place: {:?}", e))?;
        let mut set_locomotor = self.set_locomotor;
        xfer.xfer_bool(&mut set_locomotor)
            .map_err(|e| format!("Failed to crc set_locomotor: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_bool(&mut self.can_turn_in_place)
            .map_err(|e| format!("Failed to xfer can_turn_in_place: {:?}", e))?;
        xfer.xfer_bool(&mut self.set_locomotor)
            .map_err(|e| format!("Failed to xfer set_locomotor: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for AIAttackMoveStateMachine {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;
        self.base
            .crc(xfer)
            .map_err(|e| format!("Failed to crc attack move machine: {:?}", e))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        self.base
            .xfer(xfer)
            .map_err(|e| format!("Failed to xfer attack move machine: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base
            .load_post_process()
            .map_err(|e| format!("Failed to load post process attack move machine: {:?}", e))
    }
}
