#![allow(deprecated, unused_imports, dead_code)]

use super::attack::*;
use super::attack_machine::*;
use super::dead::*;
use super::dock::*;
use super::enter::*;
use super::face::*;
use super::follow_path::*;
use super::follow_path_core::*;
use super::guard::*;
use super::hack::*;
use super::helpers::*;
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
    AiCommandInterface, AiCommandParams, GuardMode, MoodMatrixAction, PartitionFilter, THE_AI,
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

/// Hunt state - seek and destroy
#[derive(Debug)]
pub struct AIHuntState {
    pub(crate) base: State,
    pub(crate) hunt_machine: Option<AIAttackThenIdleStateMachine>,
    pub(crate) next_enemy_scan_time: u32,
}

impl AIHuntState {
    pub fn new(machine: &StateMachine) -> Self {
        Self {
            base: State::new(machine, "AIHunt"),
            hunt_machine: None,
            next_enemy_scan_time: 0,
        }
    }

    pub(crate) fn find_hunt_victim(&self, owner: &Object) -> Option<Arc<RwLock<Object>>> {
        // Wave 257: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let owner_id = owner.get_id();
        let attack_info = resolve_attack_priority_info_for_object(owner_id);

        let team_arc = owner.get_team();
        let mut attack_common_target = false;
        let mut team_victim: Option<Arc<RwLock<Object>>> = None;
        if let Some(team) = team_arc.as_ref() {
            if let Ok(team_guard) = team.read() {
                attack_common_target = team_guard.attack_common_target();
                if attack_common_target {
                    let team_target = team_guard.get_team_target_object();
                    if team_target != INVALID_ID {
                        team_victim = get_legacy_object(team_target);
                    }
                }
            }
        }

        let mut victim = if team_victim.is_some() && attack_info.is_none() {
            team_victim.clone()
        } else {
            let enemy_id = THE_AI.read().ok().and_then(|ai| {
                ai.find_closest_enemy(
                    owner_id,
                    9999.9,
                    search_qualifiers::CAN_ATTACK,
                    attack_info.as_ref(),
                    None,
                )
                .ok()
                .flatten()
            });
            enemy_id.and_then(get_legacy_object)
        };

        if victim.is_none() {
            let units_should_hunt = owner
                .get_controlling_player()
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|guard| guard.get_units_should_hunt())
                })
                .unwrap_or(false);
            if units_should_hunt {
                let fallback_id = THE_AI.read().ok().and_then(|ai| {
                    ai.find_closest_enemy(
                        owner_id,
                        9999.9,
                        search_qualifiers::CAN_ATTACK,
                        None,
                        None,
                    )
                    .ok()
                    .flatten()
                });
                victim = fallback_id.and_then(get_legacy_object);
            }
        }

        if attack_common_target {
            if let (Some(team_target), Some(info)) = (team_victim.as_ref(), attack_info.as_ref()) {
                if victim.is_none() {
                    victim = Some(team_target.clone());
                }
                let team_priority = team_target
                    .read()
                    .ok()
                    .map(|obj| info.get_priority(obj.get_template().get_name().as_str()))
                    .unwrap_or(0);
                let victim_priority = victim
                    .as_ref()
                    .and_then(|obj| {
                        obj.read().ok().map(|guard| {
                            info.get_priority(guard.get_template().get_name().as_str())
                        })
                    })
                    .unwrap_or(0);
                if team_priority >= victim_priority {
                    victim = Some(team_target.clone());
                }
            }

            if let Some(team) = team_arc.as_ref() {
                if let Ok(mut team_guard) = team.write() {
                    let victim_id = victim
                        .as_ref()
                        .and_then(|obj| obj.read().ok().map(|guard| guard.get_id()))
                        .unwrap_or(INVALID_ID);
                    team_guard.set_team_target_object(victim_id);
                }
            }
        }

        victim
    }
}

impl StateImplementation for AIHuntState {
    fn on_enter(&mut self) -> StateReturnType {
        self.classic_on_enter().unwrap_or(StateReturnType::Failure)
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

impl ClassicState for AIHuntState {
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
        let owner = self
            .base
            .get_machine_owner()
            .ok_or_else(|| "hunt state missing machine owner".to_string())?;
        let mut hunt_machine = AIAttackThenIdleStateMachine::new(
            Arc::downgrade(&owner),
            "AIAttackThenIdleStateMachine",
        );

        let now = TheGameLogic::get_frame();
        let jitter = GameLogicRandomValue(0, ENEMY_SCAN_RATE as i32) as u32;
        self.next_enemy_scan_time = now.saturating_add(jitter);

        let result = hunt_machine.init_default_state();
        self.hunt_machine = Some(hunt_machine);
        Ok(result)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        let now = TheGameLogic::get_frame();
        if now >= self.next_enemy_scan_time {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "hunt state missing machine owner".to_string())?;
            let owner_guard = owner
                .read()
                .map_err(|_| "hunt state owner lock poisoned".to_string())?;

            if owner_guard.is_out_of_ammo() && !owner_guard.is_kind_of(KindOf::Projectile) {
                return Ok(StateReturnType::Failure);
            }

            if let Some(ai) = owner_guard.get_ai_update_interface() {
                if let Ok(ai_guard) = ai.lock() {
                    if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                        if let Some(hunt_machine) = self.hunt_machine.as_mut() {
                            hunt_machine.set_goal_object(crate_obj.read().ok().map(|g| g.get_id()));
                            let _ = hunt_machine.set_state(AIStateType::PickUpCrate);
                            return Ok(StateReturnType::Continue);
                        }
                    }
                }
            }

            self.next_enemy_scan_time = now.saturating_add(ENEMY_SCAN_RATE);

            let units_should_hunt = owner_guard
                .get_controlling_player()
                .and_then(|player| {
                    player
                        .read()
                        .ok()
                        .map(|guard| guard.get_units_should_hunt())
                })
                .unwrap_or(false);
            let victim = self.find_hunt_victim(&owner_guard);
            drop(owner_guard);

            let Some(hunt_machine) = self.hunt_machine.as_mut() else {
                return Ok(StateReturnType::Failure);
            };
            hunt_machine.set_goal_object(
                victim
                    .as_ref()
                    .and_then(|a| a.read().ok().map(|g| g.get_id())),
            );

            if hunt_machine.get_current_state_id() == Some(AIStateType::Idle as u32)
                && victim.is_some()
            {
                let _ = hunt_machine.set_state(AIStateType::AttackObject);
            }

            if !units_should_hunt
                && hunt_machine.get_current_state_id() == Some(AIStateType::Idle as u32)
                && victim.is_none()
            {
                return Ok(StateReturnType::Success);
            }
        }

        let Some(hunt_machine) = self.hunt_machine.as_mut() else {
            return Ok(StateReturnType::Failure);
        };

        if let Ok(machine) = self.base.get_machine() {
            if let Ok(mut machine_guard) = machine.lock() {
                machine_guard.lock();
                let result = hunt_machine.update();
                machine_guard.unlock();
                return Ok(match result {
                    StateReturnType::Sleep(_) => StateReturnType::Continue,
                    other => other,
                });
            }
        }

        let result = hunt_machine.update();
        Ok(match result {
            StateReturnType::Sleep(_) => StateReturnType::Continue,
            other => other,
        })
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        if let Some(mut machine) = self.hunt_machine.take() {
            let _ = machine.halt();
        }
        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.release_weapon_lock(WeaponLockType::LockedTemporarily);
            }
        }
        Ok(())
    }

    fn classic_is_attack(&self) -> bool {
        self.hunt_machine
            .as_ref()
            .map(|machine| machine.base.is_in_attack_state())
            .unwrap_or(false)
    }
}

impl Snapshotable for AIHuntState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut has_machine = self.hunt_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to crc hunt has_machine: {:?}", e))?;

        if let Some(machine) = self.hunt_machine.as_ref() {
            machine.crc(xfer)?;
        }

        let mut next_enemy_scan_time = self.next_enemy_scan_time;
        xfer.xfer_unsigned_int(&mut next_enemy_scan_time)
            .map_err(|e| format!("Failed to crc next_enemy_scan_time: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        let mut has_machine = self.hunt_machine.is_some();
        xfer.xfer_bool(&mut has_machine)
            .map_err(|e| format!("Failed to xfer hunt has_machine: {:?}", e))?;

        if xfer.is_loading() && !has_machine {
            self.hunt_machine = None;
        } else if xfer.is_loading() && has_machine && self.hunt_machine.is_none() {
            let owner = self
                .base
                .get_machine_owner()
                .ok_or_else(|| "hunt state missing machine owner".to_string())?;
            self.hunt_machine = Some(AIAttackThenIdleStateMachine::new(
                Arc::downgrade(&owner),
                "AIAttackThenIdleStateMachine",
            ));
        }

        if let Some(machine) = self.hunt_machine.as_mut() {
            machine.xfer(xfer)?;
        }

        xfer.xfer_unsigned_int(&mut self.next_enemy_scan_time)
            .map_err(|e| format!("Failed to xfer next_enemy_scan_time: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if let Some(machine) = self.hunt_machine.as_mut() {
            machine.load_post_process()?;
        }
        Ok(())
    }
}
