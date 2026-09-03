#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
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
    AIUpdateInterface, AIUpdateInterfaceExt, BodyModuleInterfaceExt, ContainModuleInterfaceExt,
    ContainWant, ExitDoorType, FAST_AS_POSSIBLE, PhysicsBehaviorExt,
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

/// Dead state - unit is dead
#[derive(Debug)]
pub struct AIDeadState {
    pub(crate) base: State,
}

impl AIDeadState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIDead"),
        }
    }
}

impl StateImplementation for AIDeadState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
    }

    fn update(&mut self) -> StateReturnType {
        self.classic_on_update().unwrap_or(StateReturnType::Failure)
    }

    fn on_exit(&mut self, _status: StateExitType) {
        let _ = self.classic_on_exit(_status);
    }
}

impl ClassicState for AIDeadState {
    fn base_state(&self) -> &State {
        &self.base
    }

    fn base_state_mut(&mut self) -> &mut State {
        &mut self.base
    }

    fn classic_on_enter(&mut self) -> Result<StateReturnType, String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                let non_dying_stuff = ModelConditionFlags::UsingWeaponA
                    | ModelConditionFlags::UsingWeaponB
                    | ModelConditionFlags::UsingWeaponC
                    | ModelConditionFlags::FiringA
                    | ModelConditionFlags::FiringB
                    | ModelConditionFlags::FiringC
                    | ModelConditionFlags::BetweenFiringShotsA
                    | ModelConditionFlags::BetweenFiringShotsB
                    | ModelConditionFlags::BetweenFiringShotsC
                    | ModelConditionFlags::ReloadingA
                    | ModelConditionFlags::ReloadingB
                    | ModelConditionFlags::ReloadingC
                    | ModelConditionFlags::PreAttackA
                    | ModelConditionFlags::PreAttackB
                    | ModelConditionFlags::PreAttackC
                    | MODELCONDITION_MOVING;

                let _ = owner_guard
                    .clear_and_set_model_condition_flags(non_dying_stuff, MODELCONDITION_DYING);

                crate::helpers::TheScriptEngine::notify_of_object_creation_or_destruction();
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.set_effectively_dead(true);

                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_locomotor_goal_none();
                    }
                }

                if owner_guard.is_kind_of(KindOf::Infantry) {
                    if let Some(phys) = owner_guard.get_physics() {
                        let vel = phys.get_velocity();
                        let vel_mag = (vel.x * vel.x + vel.y * vel.y).sqrt();
                        phys.scrub_velocity_2d(vel_mag * 0.8);
                    }
                }
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(MODELCONDITION_DYING);
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Container enemy detection helpers
// Matches C++ AIStates.cpp:findEnemyInContainer() (lines 391-412)
// ---------------------------------------------------------------------------

/// Find the first enemy inside a container (garrisoned building, transport, etc.)
/// The "killer" object's relationship perspective is used to determine enemies.
/// C++ Reference: AIStates.cpp:391 findEnemyInContainer()
pub(crate) fn find_enemy_in_container(killer: &Object, building: &Object) -> Option<ObjectID> {
    // Wave 257: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

    let Some(contain) = building.get_contain() else {
        return None;
    };
    let Ok(contain_guard) = contain.lock() else {
        return None;
    };
    let contained_ids = contain_guard.get_contained_objects();
    for &id in contained_ids {
        let Some(is_enemy) = OBJECT_REGISTRY
            .with_object(id, |contained_guard| {
                // Skip dead things (C++ line 398: isEffectivelyDead check)
                if contained_guard.is_effectively_dead() {
                    return None;
                }
                // C++ line 405: killer->getRelationship(*it) == ENEMIES
                // Order matters: we check if killer considers it an enemy, not vice versa.
                Some(killer.relationship_to(contained_guard) == Relationship::Enemies)
            })
            .flatten()
        else {
            continue;
        };
        if is_enemy {
            return Some(id);
        }
    }
    None
}

/// Kill enemies inside a container, up to max_to_kill.
/// Returns the number of enemies killed.
/// C++ Reference: AIStates.cpp:415 killEnemiesInContainer()
pub(crate) fn kill_enemies_in_container(
    killer_id: ObjectID,
    building: &Object,
    max_to_kill: i32,
) -> i32 {
    // Wave 257: empty dual-world → zero.
    if dual_world_registry_unavailable() {
        return 0;
    }

    let mut num_killed = 0;
    while num_killed < max_to_kill {
        let Some(enemy_id) = OBJECT_REGISTRY
            .with_object(killer_id, |killer_guard| {
                find_enemy_in_container(killer_guard, building)
            })
            .flatten()
        else {
            break;
        };

        // Remove from container (C++ lines 423-430)
        if let Some(container_id) = OBJECT_REGISTRY
            .with_object(enemy_id, |enemy_guard| enemy_guard.get_contained_by())
            .flatten()
        {
            if let Some(contain) = OBJECT_REGISTRY
                .with_object(container_id, |container_guard| {
                    container_guard.get_contain()
                })
                .flatten()
            {
                if let Ok(mut contain_guard) = contain.lock() {
                    let _ = contain_guard.release_object(enemy_id);
                }
            }
        }

        // Score the kill (C++ line 433) then kill (C++ line 434)
        let _ = OBJECT_REGISTRY.with_object_mut(killer_id, |killer_guard| {
            let _ = OBJECT_REGISTRY.with_object(enemy_id, |enemy_guard| {
                killer_guard.score_the_kill(enemy_guard);
            });
        });
        let _ = OBJECT_REGISTRY.with_object_mut(enemy_id, |enemy_guard| {
            enemy_guard.kill(None, None);
        });

        num_killed += 1;
    }
    num_killed
}
