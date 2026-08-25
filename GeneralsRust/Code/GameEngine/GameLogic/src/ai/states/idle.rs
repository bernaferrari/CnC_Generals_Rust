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
use super::hunt::*;
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

// AI State implementations

/// Idle state - do nothing
/// Matches C++ AIIdleState from AIStates.cpp lines 1246-1448
#[derive(Debug)]
pub struct AIIdleState {
    pub(crate) base: State,
    /// Random offset for idle sleep to avoid spikes
    pub(crate) initial_sleep_offset: u16,
    /// Whether to look for targets while idle
    pub(crate) should_look_for_targets: bool,
    /// Whether initialization has been done
    pub(crate) inited: bool,
}
/// C++ `AIIdleState::doInitIdleState` (`AIStates.cpp:1323-1347`).
/// The first pathfinder `updateGoal` is independent of locomotor / ultraAccurate.
/// `ultraAccurate` (loco non-null AND `isUltraAccurate()`) only gates the later snap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IdlePathfinderRestakePlan {
    pub first_restake: bool,
    pub snap: bool,
}

pub(crate) fn idle_pathfinder_restake_plan(
    is_idle: bool,
    is_doing_ground_movement: bool,
    pos: Coord3D,
    ultra_accurate: bool,
) -> IdlePathfinderRestakePlan {
    let pos_valid = pos.x != 0.0 || pos.y != 0.0 || pos.z != 0.0;
    if !(is_idle && is_doing_ground_movement && pos_valid) {
        return IdlePathfinderRestakePlan {
            first_restake: false,
            snap: false,
        };
    }
    IdlePathfinderRestakePlan {
        first_restake: true,
        snap: !ultra_accurate,
    }
}

impl AIIdleState {
    /// Create new idle state
    /// C++ constructor from AIStates.cpp line 1249
    pub fn new(machine: &StateMachine, should_look_for_targets: bool) -> Self {
        Self {
            base: State::new(machine, "AIIdle"),
            initial_sleep_offset: 0,
            should_look_for_targets,
            inited: false,
        }
    }

    pub fn is_idle(&self) -> bool {
        true
    }

    /// Initialize idle state - C++ AIIdleState::doInitIdleState() from AIStates.cpp line 1311
    pub(crate) fn do_init_idle_state(&mut self) {
        if !self.inited {
            return;
        }

        self.inited = false;

        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        // Pathfinder grid restake (C++ AIStates.cpp:1320-1358):
                        // first updateGoal always runs; ultraAccurate only gates the snap.
                        let ultra_accurate = ai_guard
                            .get_cur_locomotor()
                            .and_then(|loco| loco.lock().ok().map(|l| l.is_ultra_accurate()))
                            .unwrap_or(false);
                        let pos = *owner_guard.get_position();
                        let plan = idle_pathfinder_restake_plan(
                            ai_guard.is_idle(),
                            ai_guard.is_doing_ground_movement(),
                            pos,
                            ultra_accurate,
                        );
                        if plan.first_restake {
                            let owner_id = owner_guard.get_id();
                            let layer = match owner_guard.get_layer() {
                                PathfindLayerEnum::Invalid | PathfindLayerEnum::Last => {
                                    crate::ai::pathfind::PathfindLayerEnum::Invalid
                                }
                                PathfindLayerEnum::Wall => {
                                    crate::ai::pathfind::PathfindLayerEnum::Wall
                                }
                                PathfindLayerEnum::Ground
                                | PathfindLayerEnum::Tunnel
                                | PathfindLayerEnum::Water
                                | PathfindLayerEnum::Air => {
                                    crate::ai::pathfind::PathfindLayerEnum::Ground
                                }
                                _ => crate::ai::pathfind::PathfindLayerEnum::Top,
                            };
                            let _ =
                                crate::ai::pathfind::update_goal_for_object(owner_id, &pos, layer);
                            if plan.snap {
                                if let Some(snapped) = crate::ai::pathfind::goal_position(&pos) {
                                    let frame = TheGameLogic::get_frame();
                                    if frame <= 1 {
                                        drop(owner_guard);
                                        if let Ok(mut obj_w) = owner.write() {
                                            if let Err(err) = obj_w.set_position(&snapped) {
                                                log::warn!(
                                                    "Failed to snap AI owner position: {}",
                                                    err
                                                );
                                            }
                                        }
                                    }
                                    let _ = crate::ai::pathfind::update_goal_for_object(
                                        owner_id, &snapped, layer,
                                    );
                                }
                            }
                        }

                        // C++ line 1361: ai->setLocomotorGoalNone()
                        ai_guard.set_locomotor_goal_none();

                        // C++ line 1362: ai->setCurrentVictim(NULL)
                        ai_guard.set_current_victim(None);
                    }
                }
            }
        }
    }
}

impl StateImplementation for AIIdleState {
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

impl ClassicState for AIIdleState {
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
        // C++ AIIdleState::onEnter() from AIStates.cpp line 1290
        // Reset mood checking timers
        // Object *obj = getMachineOwner();
        // AIUpdateInterface *ai = obj->getAI();
        // if (ai) ai->resetNextMoodCheckTime();

        self.inited = true;

        // Randomize idle countdown to avoid spikes (C++ AIStates.cpp line 1304).
        self.initial_sleep_offset =
            get_game_logic_random_value(0, (LOGICFRAMES_PER_SECOND * 2) as i32) as u16;

        if let Some(owner) = self.base.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.reset_next_mood_check_time();
                    }
                }
            }
        }

        Ok(StateReturnType::Continue)
    }

    fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 257: empty dual-world → fail-closed state.
        if dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);
        }

        // C++ AIIdleState::update() from AIStates.cpp line 1369

        // Do initialization on first update (C++ line 1373)
        self.do_init_idle_state();

        let mut time_to_sleep = 60u32 + self.initial_sleep_offset as u32; // IDLE_COUNTDOWN_DELAY
        let old_sleep_offset = self.initial_sleep_offset;
        self.initial_sleep_offset = 0;

        // Check if we should look for targets (C++ line 1381)
        if self.should_look_for_targets {
            if let Ok(machine) = self.base.get_machine() {
                if let Ok(machine_guard) = machine.lock() {
                    if machine_guard.is_locked() {
                        return Ok(StateReturnType::Sleep(time_to_sleep));
                    }
                }
            }

            // Object *obj = getMachineOwner();
            // AIUpdateInterface *ai = obj->getAI();

            // Repulsor logic (C++ line 1388)
            // if (obj->isKindOf(KINDOF_CAN_BE_REPULSED) && ai->isIdle())
            // {
            //     Object* enemy = TheAI->findClosestRepulsor(obj, obj->getVisionRange());
            //     if (enemy) {
            //         getMachine()->setState(AI_MOVE_AWAY_FROM_REPULSORS);
            //         return Ok(StateReturnType::Continue);
            //     }
            // }

            // Check for crate to pickup (C++ line 1399)
            // Object* crate = ai->checkForCrateToPickup();
            // if (crate) {
            //     ai->aiMoveToObject(crate, CMD_FROM_AI);
            //     return Ok(StateReturnType::Continue);
            // }

            // Mood targeting - attack enemies based on mood settings (C++ line 1415)
            // if not disabled by paralysis/emp/etc
            // {
            //     UnsignedInt moodAdjust = ai->getMoodMatrixActionAdjustment(MM_Action_Idle);
            //     if ((moodAdjust & MAA_Affect_Range_IgnoreAll) == 0)
            //     {
            //         Object* enemy = ai->getNextMoodTarget(true, true);
            //         if (enemy) {
            //             ai->aiAttackObject(enemy, NO_MAX_SHOTS_LIMIT, CMD_FROM_AI);
            //             return Ok(StateReturnType::Continue);
            //         }
            //     }
            // }
            if let Some(owner) = self.base.get_machine_owner() {
                if let Ok(owner_guard) = owner.read() {
                    if let Some(ai) = owner_guard.get_ai_update_interface() {
                        if let Ok(mut ai_guard) = ai.lock() {
                            if owner_guard.is_kind_of(KindOf::CanBeRepulsed) && ai_guard.is_idle() {
                                let enemy = THE_AI
                                    .read()
                                    .ok()
                                    .and_then(|ai| {
                                        ai.find_closest_repulsor(
                                            owner_guard.get_id(),
                                            owner_guard.get_vision_range(),
                                        )
                                        .ok()
                                        .flatten()
                                    })
                                    .and_then(get_legacy_object);
                                if enemy.is_some() {
                                    if let Ok(machine) = self.base.get_machine() {
                                        machine.lock().ok().map(|mut guard| {
                                            guard.set_current_state(
                                                AIStateType::MoveAwayFromRepulsors.into(),
                                            );
                                        });
                                    }
                                    return Ok(StateReturnType::Continue);
                                }
                            }

                            if let Some(crate_obj) = ai_guard.check_for_crate_to_pickup() {
                                let crate_id = crate_obj.read().ok().map(|c| c.get_id());
                                if let Some(crate_id) = crate_id {
                                    ai.ai_move_to_object(crate_id, CommandSourceType::FromAi);
                                    return Ok(StateReturnType::Continue);
                                }
                            }

                            if !owner_guard.is_disabled_by_type(DisabledType::Paralyzed)
                                && !owner_guard.is_disabled_by_type(DisabledType::DisabledUnmanned)
                                && !owner_guard.is_disabled_by_type(DisabledType::DisabledEmp)
                                && !owner_guard.is_disabled_by_type(DisabledType::DisabledSubdued)
                                && !owner_guard.is_disabled_by_type(DisabledType::DisabledHacked)
                            {
                                let mood_adjust = ai_guard
                                    .get_mood_matrix_action_adjustment(MoodMatrixAction::Idle);
                                if (mood_adjust & mood_matrix_adjustment::AFFECT_RANGE_IGNORE_ALL)
                                    == 0
                                {
                                    if let Some(enemy) = ai_guard.get_next_mood_target(true, true) {
                                        ai.ai_attack_object(
                                            enemy.read().ok().map(|g| g.get_id()).unwrap_or(0),
                                            NO_MAX_SHOTS_LIMIT,
                                            CommandSourceType::FromAi,
                                        );
                                        return Ok(StateReturnType::Continue);
                                    }
                                }
                            }

                            let now = TheGameLogic::get_frame();
                            let next_mood_check = ai_guard.get_next_mood_check_time();
                            if next_mood_check > now {
                                let mood_sleep = next_mood_check - now;
                                if mood_sleep < time_to_sleep {
                                    time_to_sleep = mood_sleep;
                                    self.initial_sleep_offset = old_sleep_offset;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sleep until next check (C++ line 1446)
        // STATE_SLEEP(timeToSleep) macro
        Ok(StateReturnType::Sleep(time_to_sleep))
    }

    fn classic_on_exit(&mut self, _exit: StateExitType) -> Result<(), String> {
        // Idle state has no cleanup
        Ok(())
    }

    fn classic_is_idle(&self) -> bool {
        true
    }
}

impl Snapshotable for AIIdleState {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc version: {:?}", e))?;

        let mut initial_sleep_offset = self.initial_sleep_offset;
        xfer.xfer_unsigned_short(&mut initial_sleep_offset)
            .map_err(|e| format!("Failed to crc initial_sleep_offset: {:?}", e))?;
        let mut should_look_for_targets = self.should_look_for_targets;
        xfer.xfer_bool(&mut should_look_for_targets)
            .map_err(|e| format!("Failed to crc should_look_for_targets: {:?}", e))?;
        let mut inited = self.inited;
        xfer.xfer_bool(&mut inited)
            .map_err(|e| format!("Failed to crc inited: {:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer.xfer_unsigned_short(&mut self.initial_sleep_offset)
            .map_err(|e| format!("Failed to xfer initial_sleep_offset: {:?}", e))?;
        xfer.xfer_bool(&mut self.should_look_for_targets)
            .map_err(|e| format!("Failed to xfer should_look_for_targets: {:?}", e))?;
        xfer.xfer_bool(&mut self.inited)
            .map_err(|e| format!("Failed to xfer inited: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_restake_runs_without_loco_and_for_ultra_accurate() {
        let pos = Coord3D::new(10.0, 20.0, 0.0);
        let loco_less = idle_pathfinder_restake_plan(true, true, pos, false);
        assert!(loco_less.first_restake);
        assert!(loco_less.snap);

        let ultra = idle_pathfinder_restake_plan(true, true, pos, true);
        assert!(ultra.first_restake);
        assert!(!ultra.snap);
    }

    #[test]
    fn restake_skips_when_not_idle_or_zero_pos() {
        let pos = Coord3D::new(10.0, 0.0, 0.0);
        let not_idle = idle_pathfinder_restake_plan(false, true, pos, false);
        assert!(!not_idle.first_restake);
        assert!(!not_idle.snap);

        let zero = idle_pathfinder_restake_plan(true, true, Coord3D::new(0.0, 0.0, 0.0), false);
        assert!(!zero.first_restake);
    }
}
