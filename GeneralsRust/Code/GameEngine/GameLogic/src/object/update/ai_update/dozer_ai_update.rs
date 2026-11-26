//! DozerAIUpdate module data + AI update logic.
//!
//! Ported from GameLogic/Module/DozerAIUpdate.h and
//! GameLogic/Object/Update/AIUpdate/DozerAIUpdate.cpp.

use std::any::Any;
use std::sync::{Arc, Mutex, RwLock};

use crate::action_manager::ActionManager;
use crate::ai::{CommandSourceType, the_ai};
use crate::common::audio::AudioEventRts;
use crate::common::{
    AsciiString, Bool, Coord3D, INVALID_ID, Int, KindOf, MODELCONDITION_ACTIVELY_CONSTRUCTING,
    ModelConditionFlags, ObjectID, Real, SECONDS_PER_LOGICFRAME_REAL,
};
use crate::helpers::{FindPositionOptions, TheAudio, TheGameLogic, ThePartitionManager};
use crate::modules::AIUpdateInterface;
use crate::object::Object;
use crate::object::behavior::behavior_module::{
    BridgeBehaviorInterface, BridgeTowerBehaviorInterface, BridgeTowerType,
};
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::player::player_list;
use crate::state_machine::StateReturnType;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, ModuleDataHandle, ModuleDataRead, NameKeyType};

/// Wave 351: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

mod primary_owned;
mod delete_owned;
pub mod owned_state;
mod snapshot;
#[cfg(test)]
use owned_state::DozerOwnedObservation;
use owned_state::DozerStep;
pub use game_engine::common::thing::builder_ai_data::DozerAIUpdateModuleData;
pub use generals_sim::dozer::{DozerBuildSubTask, DozerDockPoint, DozerDockPointInfo, DozerTask};
use generals_sim::dozer::{
    ActionState, DOZER_NUM_DOCK_POINTS, DOZER_NUM_TASKS, DozerTaskOwner, PrimaryMachine, PrimaryState,
};
pub use snapshot::{load_original_dozer_primary, save_original_dozer_primary};

const MIN_ACTION_TOLERANCE: Real = 70.0;

/// DozerAIUpdate.cpp:481–510 issues Action as a distinct inherited move.
/// Both Dozer and Worker run this nested construction state machine.
pub(crate) fn advance_build_sub_task_to_dock(
    sub_task: &mut DozerBuildSubTask,
    parent: &mut dyn AIUpdateInterface,
    dock: Option<Coord3D>,
) -> Result<bool, String> {
    if *sub_task == DozerBuildSubTask::SelectBuildDockLocation {
        if let Some(dock) = dock {
            let mut command = crate::ai::AiCommandParams::new(
                crate::ai::AiCommandType::MoveToPosition,
                CommandSourceType::FromAi,
            );
            command.pos = dock;
            parent.execute_command(&command).map_err(|error| error.to_string())?;
        }
        *sub_task = DozerBuildSubTask::MovingToBuildDockLocation;
    }
    if *sub_task == DozerBuildSubTask::MovingToBuildDockLocation && parent.is_idle() {
        *sub_task = DozerBuildSubTask::DoBuildAtDock;
    }
    Ok(*sub_task == DozerBuildSubTask::DoBuildAtDock)
}

#[derive(Debug, Clone)]
struct DozerTaskEntry {
    target_id: ObjectID,
    order_frame: u32,
}

impl Default for DozerTaskEntry {
    fn default() -> Self {
        Self {
            target_id: INVALID_ID,
            order_frame: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DozerActionState {
    PickActionPos,
    MoveToActionPos,
    DoAction,
}

#[derive(Debug, Clone)]
struct DozerActionTask {
    task_type: DozerTask,
    target_id: ObjectID,
    dock_point: Option<Coord3D>,
    #[allow(dead_code)]
    failed_attempts: u32,
    build_total_frames: u32,
    build_max_health: f32,
    started_construction: bool,
}

pub(crate) enum ConstructionAdvance {
    Unavailable,
    InProgress,
    Complete,
}

/// Dozer AI Update module (matches C++ DozerAIUpdate).
pub struct DozerAIUpdate {
    /// C++ TheAudio of the engine this Object was born into.
    audio: Option<crate::helpers::TheAudio>,
    data: ModuleDataHandle,
    object_id: ObjectID,
    dozer_task: Option<DozerActionTask>,
    action_state: DozerActionState,
    task_owner: DozerTaskOwner<DozerTaskEntry, DozerDockPointInfo>,
    building_sound: Option<AudioEventRts>,
    building_sound_target: ObjectID,
    pending_owned_completion: Option<crate::modules::OwnedDozerBuildCompletion>,
}

impl std::fmt::Debug for DozerAIUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DozerAIUpdate")
            .field("data", &self.data)
            .field("object_id", &self.object_id)
            .field("dozer_task", &self.dozer_task)
            .field("action_state", &self.action_state)
            .field("current_task", &self.task_owner.current)
            .field("build_sub_task", &self.task_owner.build_sub_task)
            .field("is_rebuild", &self.task_owner.construct_is_rebuild)
            .field("has_owned_completion", &self.pending_owned_completion.is_some())
            .finish()
    }
}

impl DozerAIUpdate {
    /// Bind the engine this module's Object was assembled in.
    pub(crate) fn bind_audio(&mut self, audio: Option<crate::helpers::TheAudio>) {
        self.audio = audio;
    }

    pub fn new(data: ModuleDataHandle, object_id: ObjectID) -> Self {
        Self {
            audio: None,
            data,
            object_id,
            dozer_task: None,
            action_state: DozerActionState::PickActionPos,
            task_owner: DozerTaskOwner::default(),
            building_sound: None,
            building_sound_target: INVALID_ID,
            pending_owned_completion: None,
        }
    }

    fn install_placed_build_task(
        &mut self,
        target_id: ObjectID,
        start: Coord3D,
        target_position: Coord3D,
        total_build_frames: u32,
        max_health: f32,
        frame: u32,
    ) {
        let task = DozerTask::Build;
        let task_index = task.as_index();
        self.task_owner.requests[task_index] = DozerTaskEntry {
            target_id,
            order_frame: frame,
        };
        let end = Self::build_end_dock(&start, &target_position);
        for (point, position) in [
            (DozerDockPoint::Start, start),
            (DozerDockPoint::Action, start),
            (DozerDockPoint::End, end),
        ] {
            self.task_owner.docks[task_index][point.as_index()] = DozerDockPointInfo {
                valid: true,
                location: position,
            };
        }
        self.task_owner.current = task;
        self.task_owner.primary.state = PrimaryState::Idle;
        self.task_owner.primary.control.initialized = true;
        self.task_owner.primary.actions[task_index].reset();
        self.task_owner.build_sub_task = DozerBuildSubTask::SelectBuildDockLocation;
        self.task_owner.construct_is_rebuild = false;
        self.dozer_task = Some(DozerActionTask {
            task_type: task,
            target_id,
            dock_point: Some(start),
            failed_attempts: 0,
            build_total_frames: total_build_frames.max(1),
            build_max_health: max_health,
            started_construction: false,
        });
        self.action_state = DozerActionState::PickActionPos;
    }

    pub fn get_repair_health_per_second(&self) -> Real {
        self.data.read().downcast_ref::<DozerAIUpdateModuleData>().expect("DozerAIUpdate data type").repair_health_percent_per_second
    }

    pub fn get_bored_time(&self) -> Real {
        self.data.read().downcast_ref::<DozerAIUpdateModuleData>().expect("DozerAIUpdate data type").bored_time
    }

    pub fn get_bored_range(&self) -> Real {
        // Wave 351: empty dual-world → base bored range.
        if dual_world_registry_unavailable() {
            return self.data.read().downcast_ref::<DozerAIUpdateModuleData>().expect("DozerAIUpdate data type").bored_range;
        }

        let mut range = self.data.read().downcast_ref::<DozerAIUpdateModuleData>().expect("DozerAIUpdate data type").bored_range;
        let player_id = TheGameLogic::find_object_by_id(self.object_id).and_then(|obj| {
            let guard = obj.read().ok()?;
            guard.get_controlling_player_id()
        });
        let is_ai = if let Some(player_id) = player_id {
            let __player_list_arc = player_list();
        if let Ok(list) = __player_list_arc.read() {
                if let Some(player) = list.get_player(player_id as i32) {
                    if let Ok(player_guard) = player.read() {
                        player_guard.is_skirmish_ai()
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
        };
        if is_ai {
            if let Ok(ai) = the_ai().read() {
                range *= ai.get_ai_data().ai_dozer_bored_radius_modifier;
            }
        }
        range
    }

    fn active_build_target(&self) -> Option<ObjectID> {
        if self.get_current_task() != DozerTask::Build {
            return None;
        }
        let target = self.get_task_target(DozerTask::Build);
        (target != INVALID_ID).then_some(target)
    }

    pub fn get_current_task(&self) -> DozerTask {
        self.task_owner.current
    }

    pub fn set_current_task(&mut self, task: DozerTask) {
        self.task_owner.current = task;
    }

    pub fn get_is_rebuild(&self) -> bool {
        self.task_owner.construct_is_rebuild
    }

    pub fn set_build_sub_task(&mut self, sub: DozerBuildSubTask) {
        self.task_owner.build_sub_task = sub;
    }

    pub fn get_build_sub_task(&self) -> DozerBuildSubTask {
        self.task_owner.build_sub_task
    }

    pub fn is_task_pending(&self, task: DozerTask) -> bool {
        if task == DozerTask::Invalid {
            return false;
        }
        self.task_owner.requests[task.as_index()].target_id != INVALID_ID
    }

    pub fn is_any_task_pending(&self) -> bool {
        for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
            if self.is_task_pending(task) {
                return true;
            }
        }
        false
    }

    pub fn get_task_target(&self, task: DozerTask) -> ObjectID {
        if task == DozerTask::Invalid {
            return INVALID_ID;
        }
        self.task_owner.requests[task.as_index()].target_id
    }

    pub fn get_most_recent_command(&self) -> DozerTask {
        self.task_owner
            .most_recent(|entry| (entry.target_id != INVALID_ID).then_some(entry.order_frame))
    }

    pub fn get_dock_point(&self, task: DozerTask, point: DozerDockPoint) -> Option<Coord3D> {
        if task == DozerTask::Invalid {
            return None;
        }
        let info = self.task_owner.docks[task.as_index()][point.as_index()];
        if info.valid {
            Some(info.location)
        } else {
            None
        }
    }

    pub fn new_task(&mut self, task: DozerTask, target_id: ObjectID) {
        self.new_task_with_locomotor(task, target_id, None);
    }

    fn new_task_with_locomotor(
        &mut self,
        task: DozerTask,
        target_id: ObjectID,
        owner_airborne: Option<bool>,
    ) {
        // Wave 351: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        if task == DozerTask::Invalid {
            return;
        }
        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };

        if task == DozerTask::Build || task == DozerTask::Repair {
            let mut position = Coord3D::ZERO;
            let dock_target_id = Self::find_good_build_or_repair_position_and_target_with_locomotor(
                &owner_guard,
                &target_guard,
                &mut position,
                owner_airborne,
            )
            .unwrap_or(target_id);
            let dock_target = if dock_target_id == target_id {
                Some(Arc::clone(&target))
            } else {
                crate::object::registry::OBJECT_REGISTRY.get_object(dock_target_id)
            };
            let target_pos = if dock_target_id == target_id {
                *target_guard.get_position()
            } else {
                dock_target
                    .as_ref()
                    .and_then(|target| target.read().ok().map(|target| *target.get_position()))
                    .unwrap_or_else(|| *target_guard.get_position())
            };
            let builder_id = owner_guard.get_id();
            drop(target_guard);
            drop(owner_guard);

            // DozerAIUpdate.cpp newTask claims the selected scaffold before
            // publishing the dock. Release the position read before setBuilder.
            if task == DozerTask::Build
                && let Some(target) = dock_target
                && let Ok(mut target) = target.write()
            {
                target.set_builder_id(builder_id);
            }
            let idx = task.as_index();
            self.task_owner.docks[idx][DozerDockPoint::Start.as_index()] = DozerDockPointInfo {
                valid: true,
                location: position,
            };
            self.task_owner.docks[idx][DozerDockPoint::Action.as_index()] = DozerDockPointInfo {
                valid: true,
                location: position,
            };
            let end_pos = Self::build_end_dock(&position, &target_pos);
            self.task_owner.docks[idx][DozerDockPoint::End.as_index()] = DozerDockPointInfo {
                valid: true,
                location: end_pos,
            };
            self.task_owner.requests[task.as_index()].target_id = dock_target_id;
        }

        if self.task_owner.requests[task.as_index()].target_id == INVALID_ID {
            self.task_owner.requests[task.as_index()].target_id = target_id;
        }
        self.task_owner.requests[task.as_index()].order_frame = TheGameLogic::get_frame();
        self.task_owner.current = task;
    }

    pub fn cancel_task(&mut self, task: DozerTask) {
        self.internal_cancel_task(task);
    }

    pub fn internal_task_complete(&mut self, task: DozerTask) {
        self.internal_task_complete_or_cancelled(task);
        if task != DozerTask::Invalid {
            self.task_owner.requests[task.as_index()] = DozerTaskEntry::default();
            for point in &mut self.task_owner.docks[task.as_index()] {
                point.valid = false;
            }
        }
    }

    /// CPP internalCancelTask2105-2111, after the owner model effect and before
    /// invoking the same cached AI's Idle command with every guard released.
    pub(crate) fn cancel_task_bookkeeping_owned(&mut self, task: DozerTask) -> Result<(), String> {
        if task == DozerTask::Invalid {
            return Err("invalid cancellation task".into());
        }
        self.task_owner.requests[task.as_index()] = DozerTaskEntry::default();
        for point in &mut self.task_owner.docks[task.as_index()] {
            point.valid = false;
        }
        Ok(())
    }

    /// The Rust task runner is part of the same Dozer operation. Once the
    /// actual parent FSM has entered Idle, a cancelled runner cannot continue
    /// construction on its next update (CPP build-state onExit clears task).
    pub(crate) fn finish_cancelled_task_on_idle_owned(
        &mut self,
        task: DozerTask,
    ) -> Result<(), String> {
        if task == DozerTask::Invalid {
            return Err("invalid cancellation task".into());
        }
        if self.task_owner.current == task {
            self.task_owner.current = DozerTask::Invalid;
            self.dozer_task = None;
            self.action_state = DozerActionState::PickActionPos;
        }
        Ok(())
    }

    pub fn internal_cancel_task(&mut self, task: DozerTask) {
        // Wave 351: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        self.internal_task_complete_or_cancelled(task);
        if self.cancel_task_bookkeeping_owned(task).is_err() {
            return;
        }
        let _ =
            crate::object::registry::OBJECT_REGISTRY.with_object(self.object_id, |owner_guard| {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        let _ = ai_guard.ai_idle();
                    }
                }
            });
    }

    fn internal_task_complete_or_cancelled(&mut self, task: DozerTask) {
        // Wave 351: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(
            self.object_id,
            |owner_guard| {
                if task == DozerTask::Build || task == DozerTask::Repair {
                    owner_guard.clear_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                }
            },
        );
    }

    pub fn on_delete(&mut self) {
        // Wave 351: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
            if self.is_task_pending(task) {
                self.cancel_task(task);
            }
        }
        for task in [DozerTask::Build, DozerTask::Repair, DozerTask::Fortify] {
            let target_id = self.get_task_target(task);
            if target_id != INVALID_ID {
                let _ =
                    crate::object::registry::OBJECT_REGISTRY.with_object_mut(target_id, |guard| {
                        guard.clear_model_condition_state(
                            ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
                        );
                    });
            }
        }
    }

    pub fn can_accept_new_repair(&self, target: &Object) -> bool {
        // Wave 351: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }

        if self.task_owner.current != DozerTask::Repair {
            return true;
        }
        let current_id = self.task_owner.requests[DozerTask::Repair.as_index()].target_id;
        if current_id == INVALID_ID {
            return true;
        }
        let Some(current_obj) = TheGameLogic::find_object_by_id(current_id) else {
            return true;
        };
        let Ok(current_guard) = current_obj.read() else {
            return true;
        };
        if current_guard.get_id() == target.get_id() {
            return false;
        }
        if current_guard.is_kind_of(KindOf::BridgeTower) && target.is_kind_of(KindOf::BridgeTower) {
            let current_bridge = Self::get_bridge_id_for_tower(&current_guard);
            let new_bridge = Self::get_bridge_id_for_tower(target);
            if current_bridge != INVALID_ID && current_bridge == new_bridge {
                return false;
            }
        }
        true
    }

    pub fn set_repair_target(&mut self, target_id: ObjectID, cmd_source: CommandSourceType) {
        // Wave 351: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_repair_object(&*owner_guard, &*target_guard, cmd_source) {
            return;
        }
        // C++ DozerAIUpdate::privateRepair — sole healer, not builder_id.
        let sole = target_guard.get_sole_healing_benefactor();
        if sole != INVALID_ID && sole != self.object_id {
            return;
        }
        self.new_task(DozerTask::Repair, target_id);
        self.dozer_task = Some(DozerActionTask {
            task_type: DozerTask::Repair,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            started_construction: false,
        });
        self.action_state = DozerActionState::PickActionPos;
    }

    pub fn set_resume_construction_target(
        &mut self,
        target_id: ObjectID,
        cmd_source: CommandSourceType,
    ) {
        // Wave 351: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        let Some(target) = TheGameLogic::find_object_by_id(target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            return;
        };
        if !ActionManager::can_resume_construction_with_builder(
            &owner_guard, &target_guard, cmd_source, self.active_build_target(),
        ) {
            return;
        }
        // C++ privateResumeConstruction -> newTask(DOZER_TASK_BUILD) uses
        // calcTimeToBuild(player), not the raw template frames.
        let player = owner_guard.get_controlling_player();
        let frames = if let Some(player) = player.as_ref() {
            if let Ok(player_guard) = player.read() {
                target_guard
                    .get_template()
                    .calc_time_to_build(Some(&*player_guard))
                    .max(1) as u32
            } else {
                target_guard.get_template().calc_time_to_build(None).max(1) as u32
            }
        } else {
            target_guard.get_template().calc_time_to_build(None).max(1) as u32
        };
        let max_health = target_guard
            .get_body_module()
            .and_then(|body| body.lock().ok().map(|g| g.get_max_health()))
            .unwrap_or(0.0);
        drop(owner_guard);
        drop(target_guard);
        self.new_task(DozerTask::Build, target_id);
        self.dozer_task = Some(DozerActionTask {
            task_type: DozerTask::Build,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: frames,
            build_max_health: max_health,
            started_construction: false,
        });
        self.action_state = DozerActionState::PickActionPos;
    }

    pub fn set_build_task(
        &mut self,
        building_id: ObjectID,
        total_build_frames: u32,
        max_health: f32,
        is_rebuild: bool,
        owner_airborne: bool,
    ) {
        self.task_owner.construct_is_rebuild = is_rebuild;
        self.new_task_with_locomotor(DozerTask::Build, building_id, Some(owner_airborne));
        self.dozer_task = Some(DozerActionTask {
            task_type: DozerTask::Build,
            target_id: building_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: total_build_frames.max(1),
            build_max_health: max_health,
            started_construction: false,
        });
        self.action_state = DozerActionState::PickActionPos;
    }

    pub fn start_building_sound(&mut self, sound: &AudioEventRts, construction_site_id: ObjectID) {
        let Some(audio) = self.audio.clone() else {
            return;
        };
        let mut event = sound.clone();
        event.set_object_id(construction_site_id);
        let handle = audio.add_audio_event(&event);
        event.set_playing_handle(handle);
        self.building_sound = Some(event);
        self.building_sound_target = construction_site_id;
    }

    /// Installed Build execution sends audio through its receiving frame
    /// owner, then retains the exact returned handle for the completion edge.
    pub(crate) fn start_building_sound_with_world(
        &mut self,
        sound: &AudioEventRts,
        construction_site_id: ObjectID,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) {
        let mut event = sound.clone();
        event.set_object_id(construction_site_id);
        let handle = world.add_audio_event(&event);
        event.set_playing_handle(handle);
        self.building_sound = Some(event);
        self.building_sound_target = construction_site_id;
    }

    pub fn finish_building_sound(&mut self) {
        if let Some(sound) = self.building_sound.take() {
            if let Some(audio) = self.audio.clone() {
                audio.remove_audio_event(sound.get_playing_handle());
            }
        }
        self.building_sound_target = INVALID_ID;
    }

    /// The owner frame calls this after receiving construction completion,
    /// outside its broad service loan but with its exact audio sink supplied.
    pub(crate) fn finish_building_sound_with_world(
        &mut self,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) {
        if let Some(sound) = self.building_sound.take() {
            world.remove_audio_event(sound.get_playing_handle());
        }
        self.building_sound_target = INVALID_ID;
    }

    fn handle_build_completion(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        target: &Arc<RwLock<Object>>,
        is_rebuild: Bool,
        parent: &mut Option<&mut dyn AIUpdateInterface>,
    ) -> Result<(), String> {
        // DozerAIUpdate.cpp:540–545 clears both status flags and visits mines
        // before stopping the construction sound.
        if let Ok(mut target_guard) = target.write() {
            target_guard.clear_construction_completion_status();
        }
        self.finish_building_sound();

        let mut target_display_name: Option<String> = None;
        let mut target_pos: Option<Coord3D> = None;
        let mut controlling_player: Option<Arc<RwLock<crate::player::Player>>> = None;

        if let Ok(mut target_guard) = target.write() {
            let _ = target_guard.clear_model_condition_flags(
                ModelConditionFlags::AWAITING_CONSTRUCTION
                    | ModelConditionFlags::PARTIALLY_CONSTRUCTED
                    | ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
            );
            target_guard.set_construction_percent(crate::object::CONSTRUCTION_COMPLETE);

            if let Some(body) = target_guard.get_body_module() {
                if let Ok(mut body_guard) = body.lock() {
                    let _ = body_guard.evaluate_visual_condition();
                }
            }

            let template = target_guard.get_template();
            // DozerAIUpdate.cpp:602 uses the translated authored name.
            let display_name = template.get_display_name();
            if display_name.is_empty() {
                let fallback = crate::helpers::TheGameText::fetch_for(&target_guard, "INI:MissingDisplayName");
                let template_name = template.get_name().as_str();
                target_display_name = Some(fallback.replace("%s", template_name));
            } else {
                target_display_name = Some(display_name.to_owned());
            }
            target_pos = Some(*target_guard.get_position());
            controlling_player = target_guard.get_controlling_player();
        }

        if let Some(player) = controlling_player {
            crate::player::Player::on_structure_construction_complete(
                &player,
                Some(self.object_id),
                target,
                is_rebuild,
            )?;
            if let Ok(mut target_guard) = target.write() {
                target_guard.on_build_complete();
            }
        }

        if let Ok(mut target_guard) = target.write() {
            // DozerAIUpdate.cpp:588 refreshes after Create and before upgrades.
            target_guard.maintain_partition_for_owning_world();
            target_guard.update_upgrade_modules_from_player();
        }

        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_locally_controlled() {
                if let Some(display_name) = target_display_name.as_ref() {
                    let format = crate::helpers::TheGameText::fetch_for(&owner_guard, "DOZER:ConstructionComplete");
                    let message = if format.contains("%s") {
                        format.replace("%s", display_name)
                    } else {
                        format!("{} {}", format, display_name)
                    };
                    crate::helpers::TheInGameUI::display_message_for(&owner_guard, &message);
                }

                if let Some(audio) = self.audio.clone() {
                    let mut event = owner_guard.get_template().get_voice_task_complete();
                    if !event.get_event_name().is_empty() {
                        event.set_object_id(owner_guard.get_id());
                        audio.add_audio_event(&event);
                    }
                }

                if let (Some(radar), Some(pos)) = (crate::helpers::TheRadar::get(), target_pos) {
                    radar.create_event(
                        &pos,
                        game_engine::common::system::radar::RadarEventType::Construction,
                        4.0,
                    );
                }
            }
        }

        let dock = self.get_dock_point(self.task_owner.current, DozerDockPoint::End);
        Self::with_action_parent(owner, parent, |ai| Self::move_to_end_dock(owner, ai, dock));
        Ok(())
    }

    // DozerAIUpdate.cpp:511–536 changes progress, the canonical body and
    // producer on the live goal before checking completion.
    pub(crate) fn advance_live_scaffold(
        scaffold: &mut Object,
        builder: ObjectID,
        frames: u32,
    ) -> ConstructionAdvance {
        let progress = scaffold.get_construction_percent();
        if progress < 0.0
            || scaffold.get_builder_id() != builder
            || scaffold.is_destroyed()
            || scaffold.is_effectively_dead()
        {
            return ConstructionAdvance::Unavailable;
        }
        let progress = progress + 100.0 / frames as f32;
        scaffold.set_construction_percent(progress);
        if let Err(error) = scaffold.add_construction_health_with_owner(frames) {
            log::debug!("Construction health update failed: {error}");
        }
        scaffold.set_producer_id(builder);
        scaffold.set_builder_id(builder);
        if progress >= 100.0 {
            ConstructionAdvance::Complete
        } else {
            ConstructionAdvance::InProgress
        }
    }

    // DozerAIUpdate.cpp:515–526 samples the builder's player every frame.
    pub(crate) fn current_build_frames(owner: &Arc<RwLock<Object>>, target: &Arc<RwLock<Object>>) -> u32 {
        let player = owner.read().ok().and_then(|actor| actor.get_controlling_player());
        let Ok(template) = target.read().map(|building| Arc::clone(building.get_template())) else {
            return 1;
        };
        let player = player.as_ref().and_then(|player| player.read().ok());
        template.calc_time_to_build(player.as_deref().map(|player| player as &dyn Any)).max(1) as u32
    }

    // DozerAIUpdate.cpp:631–639 sends the adjusted End dock to the inherited
    // AI before either builder completes its shared construction action.
    pub(crate) fn move_to_end_dock(
        owner: &Arc<RwLock<Object>>,
        ai: &mut dyn AIUpdateInterface,
        dock: Option<Coord3D>,
    ) {
        let ignored = ai.get_ignored_obstacle_id();
        let aircraft = ai.is_aircraft_that_adjusts_destination();
        let end_pos = {
            let Ok(object) = owner.read() else { return };
            let mut end_pos = dock.unwrap_or(*object.get_position());
            if let Some(loco_set) = ai.get_locomotor_set_clone() {
                let pathfinder = match object.owning_pathfinding_runtime() {
                    Some(world) => world.ai().read().ok().and_then(|ai| ai.pathfinder()),
                    None => crate::ai::the_ai().read().ok().and_then(|ai| ai.pathfinder()),
                };
                if let Some(pathfinder) = pathfinder.as_ref().and_then(|pathfinder| pathfinder.read().ok()) {
                    let _ = pathfinder.adjust_possible_destination_for_object(
                        &object, &loco_set, ignored, aircraft, &mut end_pos, None,
                    );
                }
            }
            end_pos
        };
        let mut command = crate::ai::AiCommandParams::new(
            crate::ai::AiCommandType::MoveToPosition,
            CommandSourceType::FromAi,
        );
        command.pos = end_pos;
        let _ = ai.execute_command(&command);
    }

    pub fn update(&mut self) -> StateReturnType {
        self.update_with_parent(None, None)
    }

    pub(crate) fn update_with_ai(
        &mut self,
        parent: &mut dyn AIUpdateInterface,
        owner: Option<&Arc<RwLock<Object>>>,
    ) -> StateReturnType {
        self.update_with_parent(Some(parent), owner)
    }

    fn update_with_parent(
        &mut self,
        parent: Option<&mut dyn AIUpdateInterface>,
        owner: Option<&Arc<RwLock<Object>>>,
    ) -> StateReturnType {
        // Wave 351: empty dual-world → Continue.
        if dual_world_registry_unavailable() {
            return StateReturnType::Continue;
        }

        // validate current task if repair
        if self.task_owner.current == DozerTask::Repair {
            let target_id = self.get_task_target(DozerTask::Repair);
            if let (Some(owner), Some(target)) = (
                TheGameLogic::find_object_by_id(self.object_id),
                TheGameLogic::find_object_by_id(target_id),
            ) {
                if let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) {
                    if !ActionManager::can_repair_object(
                        &*owner_guard,
                        &*target_guard,
                        CommandSourceType::FromAi,
                    ) {
                        self.cancel_task(DozerTask::Repair);
                    }
                }
            }
        }

        if self.dozer_task.is_none() {
            if self.task_owner.current == DozerTask::Invalid {
                self.task_owner.current = self.get_most_recent_command();
            }
            if self.task_owner.current != DozerTask::Invalid {
                self.spawn_dozer_task_from_current();
                if self.dozer_task.is_some() {
                    self.action_state = DozerActionState::PickActionPos;
                }
            }
        }

        if self.dozer_task.is_some() {
            if let Err(error) = self.update_dozer_task(parent, owner) {
                log::error!("Dozer construction update failed: {error}");
                return StateReturnType::Error;
            }
        }

        StateReturnType::Continue
    }

    fn spawn_dozer_task_from_current(&mut self) {
        let task = self.task_owner.current;
        if task == DozerTask::Invalid {
            return;
        }
        let target_id = self.get_task_target(task);
        if target_id == INVALID_ID {
            return;
        }
        self.dozer_task = Some(DozerActionTask {
            task_type: task,
            target_id,
            dock_point: None,
            failed_attempts: 0,
            build_total_frames: 0,
            build_max_health: 0.0,
            started_construction: false,
        });
    }

    fn with_action_parent<T>(
        owner: &Arc<RwLock<Object>>,
        parent: &mut Option<&mut dyn AIUpdateInterface>,
        action: impl FnOnce(&mut dyn AIUpdateInterface) -> T,
    ) -> Option<T> {
        if let Some(parent) = parent {
            return Some(action(&mut **parent));
        }
        let cached = owner.read().ok()?.get_ai_update_interface()?;
        let mut parent = cached.lock().ok()?;
        Some(action(&mut *parent))
    }

    fn advance_build_dock(
        &mut self,
        owner: &Arc<RwLock<Object>>,
        parent: &mut Option<&mut dyn AIUpdateInterface>,
    ) -> Result<bool, String> {
        if self.task_owner.build_sub_task == DozerBuildSubTask::DoBuildAtDock {
            return Ok(true);
        }
        let dock = self.get_dock_point(DozerTask::Build, DozerDockPoint::Action);
        let action = Self::with_action_parent(owner, parent, |ai| {
            advance_build_sub_task_to_dock(&mut self.task_owner.build_sub_task, ai, dock)
        });
        match action {
            Some(result) => result,
            // Keep the compatibility path for isolated callers without a parent.
            None => {
                self.task_owner.build_sub_task = DozerBuildSubTask::DoBuildAtDock;
                Ok(true)
            }
        }
    }

    fn update_dozer_task(
        &mut self,
        mut parent: Option<&mut dyn AIUpdateInterface>,
        installed_owner: Option<&Arc<RwLock<Object>>>,
    ) -> Result<(), String> {
        let mut failure = None;
        (|| {
        // Wave 351: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(mut task) = self.dozer_task.take() else {
            return;
        };

        let Some(owner) = TheGameLogic::find_object_by_id(self.object_id) else {
            return;
        };
        if installed_owner.is_some_and(|admitted| !Arc::ptr_eq(admitted, &owner)) {
            self.dozer_task = Some(task);
            return;
        }
        let Some(target) = TheGameLogic::find_object_by_id(task.target_id) else {
            return;
        };
        let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) else {
            self.dozer_task = Some(task);
            return;
        };
        let owner_pos = *owner_guard.get_position();
        let changed_controller = installed_owner.is_some()
            && task.task_type == DozerTask::Build
            && self.action_state == DozerActionState::DoAction
            && owner_guard.get_controlling_player_id() != target_guard.get_controlling_player_id();
        let target_pos = *target_guard.get_position();
        let target_radius = target_guard
            .get_geometry_info()
            .get_bounding_sphere_radius();
        let target_builder_id = target_guard.get_builder_id();
        let target_sole = target_guard.get_sole_healing_benefactor();
        let target_is_bridge_tower = target_guard.is_kind_of(KindOf::BridgeTower);
        let target_body_max = if installed_owner.is_some() && task.task_type == DozerTask::Build {
            0.0
        } else {
            target_guard.get_body_module()
                .and_then(|body| body.lock().ok().map(|guard| guard.get_max_health()))
                .unwrap_or(0.0)
        };
        drop(target_guard);

        if self.action_state == DozerActionState::PickActionPos {
            // DozerAIUpdate.cpp:151–216. An assigned Start dock still issues
            // the real move and leaves Pick. Only a missing dock searches.
            let dock = self.get_dock_point(self.task_owner.current, DozerDockPoint::Start)
                .unwrap_or_else(|| {
                    let direction = owner_pos - target_pos;
                    let mut options = FindPositionOptions::default();
                    options.min_radius = target_radius;
                    options.max_radius = target_radius;
                    options.start_angle = Some(direction.y.atan2(direction.x));
                    let mut position = target_pos;
                    if let Some(partition) = ThePartitionManager::get() {
                        partition.find_position_around_with_options(&target_pos, &options, &mut position);
                    }
                    position
                });
            drop(owner_guard);
            task.dock_point = Some(dock);
            let dispatch = Self::with_action_parent(&owner, &mut parent, |ai| {
                ai.ignore_obstacle_id(task.target_id)?;
                let mut command = crate::ai::AiCommandParams::new(
                    crate::ai::AiCommandType::MoveToPosition, CommandSourceType::FromAi,
                );
                command.pos = dock;
                ai.execute_command(&command)
            });
            match dispatch {
                Some(Ok(())) => self.action_state = DozerActionState::MoveToActionPos,
                Some(Err(error)) => failure = Some(error.to_string()),
                None => {}
            }
            self.dozer_task = Some(task);
            return;
        }

        let dock = task.dock_point.unwrap_or(target_pos);
        let distance = ThePartitionManager::get_distance_squared_to_pos(
            &owner_guard, &dock, crate::common::FROM_BOUNDING_SPHERE_2D,
        );
        let tolerance = MIN_ACTION_TOLERANCE.max(
            owner_guard.get_geometry_info().get_bounding_sphere_radius() + 15.0,
        );
        drop(owner_guard);

        match self.action_state {
            DozerActionState::MoveToActionPos => {
                let other_builder = task.task_type == DozerTask::Build && target_builder_id != self.object_id;
                let other_repairer = task.task_type == DozerTask::Repair
                    && target_sole != INVALID_ID && target_sole != self.object_id;
                if other_builder || other_repairer {
                    self.internal_task_complete(task.task_type);
                    self.task_owner.current = DozerTask::Invalid;
                    return;
                }
                if distance <= tolerance * tolerance {
                    if task.task_type == DozerTask::Build {
                        if let Ok(mut target) = target.write() {
                            let _ = target.clear_and_set_model_condition_flags(
                                ModelConditionFlags::AWAITING_CONSTRUCTION,
                                ModelConditionFlags::PARTIALLY_CONSTRUCTED
                                    | ModelConditionFlags::ACTIVELY_BEING_CONSTRUCTED,
                            );
                        }
                        // DozerAIUpdate.cpp:421–440 resets the distinct Action
                        // dock stage on entry, before its first update.
                        self.task_owner.build_sub_task = DozerBuildSubTask::SelectBuildDockLocation;
                    }
                    self.action_state = DozerActionState::DoAction;
                } else if Self::with_action_parent(&owner, &mut parent, |ai| ai.is_idle()).unwrap_or(false) {
                    task.dock_point = None;
                    self.action_state = DozerActionState::PickActionPos;
                }
            }
            DozerActionState::DoAction => match task.task_type {
                DozerTask::Repair => {
                    if target_sole != INVALID_ID && target_sole != self.object_id {
                        self.internal_task_complete(DozerTask::Repair);
                        return;
                    }
                    if let (Ok(owner_guard), Ok(target_guard)) = (owner.read(), target.read()) {
                        if !ActionManager::can_repair_object(
                            &*owner_guard,
                            &*target_guard,
                            CommandSourceType::FromAi,
                        ) {
                            self.internal_task_complete(DozerTask::Repair);
                            return;
                        }
                    }
                    let already_full = target
                        .read()
                        .ok()
                        .and_then(|guard| guard.get_body_module())
                        .and_then(|body| {
                            body.lock()
                                .ok()
                                .map(|body| body.get_health() == body.get_max_health())
                        })
                        .unwrap_or(false);
                    if already_full {
                        // C++ TheGameText / TheInGameUI of the dozer's engine.
                        let world = owner.read().ok().and_then(|guard| guard.owning_world_services());
                        let message = crate::helpers::TheGameText::fetch_from(world.as_ref(), "DOZER:RepairComplete");
                        crate::helpers::TheInGameUI::display_message_from(world.as_ref(), &message);
                        if target_is_bridge_tower {
                            self.remove_bridge_scaffolding(task.target_id);
                        }
                        self.internal_task_complete(DozerTask::Repair);
                        return;
                    }
                    if let Ok(mut owner_write) = owner.write() {
                        owner_write.set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                    }
                    let can_heal = !target_is_bridge_tower
                        || !self.bridge_scaffold_blocks_heal(task.target_id);
                    if can_heal {
                        let health = target_body_max
                            * self.get_repair_health_per_second()
                            * SECONDS_PER_LOGICFRAME_REAL;
                        let healed = if let Ok(mut target_write) = target.write() {
                            match target_write.attempt_healing_from_sole_benefactor_id(
                                health,
                                self.object_id,
                                2,
                            ) {
                                Ok(ok) => ok,
                                Err(_) => false,
                            }
                        } else {
                            false
                        };
                        if !healed {
                            self.internal_cancel_task(DozerTask::Repair);
                            return;
                        }
                    }
                }
                DozerTask::Build => {
                    // DozerAIUpdate.cpp:474–477 checks that the builder and
                    // scaffold still have the same controlling player.
                    if changed_controller {
                        self.internal_task_complete(DozerTask::Build);
                        return;
                    }
                    // DozerAIUpdate.cpp:481–510 issues the Action dock as a
                    // separate inherited move, even when it matches Start.
                    match self.advance_build_dock(&owner, &mut parent) {
                        Ok(true) => {}
                        result => {
                            failure = result.err();
                            self.dozer_task = Some(task);
                            return;
                        }
                    }
                    if target_builder_id != INVALID_ID && target_builder_id != self.object_id {
                        self.internal_task_complete(DozerTask::Build);
                        return;
                    }
                    // The manager only serves the standalone compatibility
                    // path. Installed builders mutate the owning scaffold.
                    let standalone_world = installed_owner.is_none()
                        .then(crate::system::engine_stores::active_services);
                    let manager_handle = standalone_world.as_ref()
                        .map(|world| &world.logic_managers().construction);
                    let frames = Self::current_build_frames(&owner, &target);
                    if !task.started_construction {
                        if let Some(manager_handle) = manager_handle {
                            let max_health = if task.build_max_health > 0.0 {
                                task.build_max_health
                            } else {
                                target_body_max
                            };
                            let (existing_percent, existing_health) = target
                                .read()
                                .ok()
                                .map(|guard| (guard.get_construction_percent(), guard.get_health()))
                                .unwrap_or((0.0, 1.0));
                            let Ok(mut manager) = manager_handle.write() else {
                                return;
                            };
                            if manager
                                .start_construction(
                                    task.target_id,
                                    self.object_id,
                                    max_health,
                                    frames,
                                    self.task_owner.construct_is_rebuild,
                                )
                                .is_ok()
                            {
                                manager.seed_from_structure(
                                    task.target_id,
                                    existing_percent,
                                    existing_health,
                                );
                            }
                            drop(manager);
                        }
                        task.started_construction = true;
                        if let Some(sound) = target.read().ok().and_then(|guard| {
                            guard.get_template().get_per_unit_sound("UnderConstruction")
                        }) {
                            self.start_building_sound(&sound, task.target_id);
                        }
                    }
                    if let Ok(mut owner_write) = owner.write() {
                        owner_write.set_model_condition_state(MODELCONDITION_ACTIVELY_CONSTRUCTING);
                    }
                    let completed = if let Some(manager_handle) = manager_handle {
                        let (completed, progress, current_health) = {
                            let Ok(mut manager) = manager_handle.write() else {
                                return;
                            };
                            manager.set_build_frames(task.target_id, frames);
                            let completed = manager.update_for_dozer(self.object_id);
                            (completed, manager.get_progress(task.target_id), manager.get_current_health(task.target_id))
                        };
                        if let Some(progress) = progress {
                            if let Ok(mut target_write) = target.write() {
                                target_write.set_construction_percent(progress);
                                if let Some(health) = current_health {
                                    let _ = target_write.set_health(health);
                                }
                                target_write.set_producer_id(self.object_id);
                                target_write.set_builder_id(self.object_id);
                            }
                        }
                        completed == Some(task.target_id)
                    } else {
                        let advance = {
                            let Ok(mut scaffold) = target.write() else { return };
                            Self::advance_live_scaffold(&mut scaffold, self.object_id, frames)
                        };
                        match advance {
                            ConstructionAdvance::Unavailable => {
                                self.internal_task_complete(DozerTask::Build);
                                return;
                            }
                            ConstructionAdvance::InProgress => false,
                            ConstructionAdvance::Complete => true,
                        }
                    };
                    if completed {
                        if let Err(error) = self.handle_build_completion(
                            &owner,
                            &target,
                            self.task_owner.construct_is_rebuild,
                            &mut parent,
                        ) {
                            failure = Some(error);
                            return;
                        }
                        self.internal_task_complete(DozerTask::Build);
                        return;
                    }
                }
                DozerTask::Fortify | DozerTask::Invalid => {
                    self.internal_task_complete(task.task_type);
                    return;
                }
            },
            DozerActionState::PickActionPos => {}
        }

        self.dozer_task = Some(task);
        })();
        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn get_bridge_id_for_tower(tower: &Object) -> ObjectID {
        for module_handle in tower.behavior_modules() {
            let bridge_id = module_handle.with_module(|module| {
                module
                    .get_bridge_tower_control_interface()
                    .map(|tower| tower.bridge_id())
            });
            if let Some(bridge_id) = bridge_id {
                return bridge_id;
            }
        }

        for mut behavior in tower.get_behavior_modules() {
            let Ok(mut guard) = behavior.access() else {
                continue;
            };
            if let Some(interface) = guard.get_bridge_tower_behavior_interface() {
                return interface.get_bridge_id();
            }
        }
        INVALID_ID
    }

    fn bridge_scaffold_blocks_heal(&self, tower_id: ObjectID) -> bool {
        if dual_world_registry_unavailable() {
            return false;
        }
        let Some(tower_obj) = TheGameLogic::find_object_by_id(tower_id) else {
            return false;
        };
        let bridge_id = {
            let Ok(tower_guard) = tower_obj.read() else {
                return false;
            };
            Self::get_bridge_id_for_tower(&tower_guard)
        };
        if bridge_id == INVALID_ID {
            return false;
        }
        let Some(bridge_obj) = TheGameLogic::find_object_by_id(bridge_id) else {
            return false;
        };
        let Ok(bridge_guard) = bridge_obj.read() else {
            return false;
        };
        let behaviors = bridge_guard.get_behavior_modules();
        drop(bridge_guard);
        for mut behavior in behaviors {
            let Ok(mut behavior_guard) = behavior.access() else {
                continue;
            };
            if let Some(interface) = behavior_guard.get_bridge_behavior_interface() {
                interface.create_scaffolding();
                return interface.is_scaffold_in_motion();
            }
        }
        false
    }

    fn remove_bridge_scaffolding(&self, bridge_tower_id: ObjectID) {
        // Wave 351: empty dual-world → no-op.
        if dual_world_registry_unavailable() {
            return;
        }

        let Some(tower_obj) = TheGameLogic::find_object_by_id(bridge_tower_id) else {
            return;
        };
        let bridge_id = {
            let Ok(tower_guard) = tower_obj.read() else {
                return;
            };
            Self::get_bridge_id_for_tower(&tower_guard)
        };
        if bridge_id == INVALID_ID {
            return;
        }

        let Some(bridge_obj) = TheGameLogic::find_object_by_id(bridge_id) else {
            return;
        };
        let Ok(bridge_guard) = bridge_obj.read() else {
            return;
        };
        let behaviors = bridge_guard.get_behavior_modules();
        drop(bridge_guard);
        for mut behavior in behaviors {
            let Ok(mut behavior_guard) = behavior.access() else {
                continue;
            };
            if let Some(interface) = behavior_guard.get_bridge_behavior_interface() {
                interface.remove_scaffolding();
                break;
            }
        }
    }

    pub fn find_good_build_or_repair_position(
        me: &Object,
        target: &Object,
        position_out: &mut Coord3D,
    ) -> bool {
        Self::find_good_build_or_repair_position_with_locomotor(
            me,
            target,
            position_out,
            me.is_using_airborne_locomotor(),
        )
    }

    fn find_good_build_or_repair_position_with_locomotor(
        me: &Object,
        target: &Object,
        position_out: &mut Coord3D,
        owner_airborne: bool,
    ) -> bool {
        let (working, options) = Self::build_or_repair_position_query(
            me.get_id(),
            me.get_position(),
            target.get_id(),
            target.get_position(),
            target.get_geometry_info().get_major_radius(),
            owner_airborne,
        );
        let mut best = working;
        let found = if let Some(partition) = ThePartitionManager::get() {
            partition.find_position_around_with_options(&working, &options, &mut best)
        } else {
            false
        };
        *position_out = if found { best } else { working };
        found
    }

    /// C++ DozerAIUpdate::findGoodBuildOrRepairPosition inputs, separated from
    /// the process partition lookup so a receiving world can execute the same
    /// search against its own partition and terrain.
    pub(crate) fn build_or_repair_position_query(
        owner_id: ObjectID,
        owner_position: &Coord3D,
        target_id: ObjectID,
        target_position: &Coord3D,
        target_major_radius: Real,
        owner_airborne: bool,
    ) -> (Coord3D, FindPositionOptions) {
        let mut working = *target_position;
        let mut offset = *owner_position - *target_position;
        let len = (offset.x * offset.x + offset.y * offset.y + offset.z * offset.z).sqrt();
        if len > 0.0 {
            offset.x /= len;
            offset.y /= len;
            offset.z /= len;
        }
        let scale = target_major_radius / 2.0;
        working.x += offset.x * scale;
        working.y += offset.y * scale;
        working.z += offset.z * scale;

        let mut options = FindPositionOptions::default();
        options.min_radius = 0.0;
        options.max_radius = 100.0;
        options.source_to_path_to_dest_id = Some(owner_id);
        if owner_airborne {
            options.ignore_object_id = Some(target_id);
        } else {
            options.max_z_delta = 10.0;
        }
        (working, options)
    }

    /// C++ DozerAIUpdate::newTask stores the End dock five path cells beyond
    /// the Action dock, in the direction away from the target.
    pub(crate) fn build_end_dock(action: &Coord3D, target: &Coord3D) -> Coord3D {
        let mut offset = Coord3D::new(action.x - target.x, action.y - target.y, 0.0);
        let len = (offset.x * offset.x + offset.y * offset.y).sqrt();
        if len > 0.0 {
            offset.x /= len;
            offset.y /= len;
            offset.x *= 5.0 * PATHFIND_CELL_SIZE_F;
            offset.y *= 5.0 * PATHFIND_CELL_SIZE_F;
        }
        let mut end = *action;
        end.x += offset.x;
        end.y += offset.y;
        end
    }

    pub fn find_good_build_or_repair_position_and_target(
        me: &Object,
        target: &Object,
        position_out: &mut Coord3D,
    ) -> Option<ObjectID> {
        Self::find_good_build_or_repair_position_and_target_with_locomotor(
            me, target, position_out, None,
        )
    }

    pub(crate) fn find_good_build_or_repair_position_and_target_with_locomotor(
        me: &Object,
        target: &Object,
        position_out: &mut Coord3D,
        owner_airborne: Option<bool>,
    ) -> Option<ObjectID> {
        // Wave 351: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }
        let owner_airborne = owner_airborne.unwrap_or_else(|| me.is_using_airborne_locomotor());

        if target.is_kind_of(KindOf::Bridge) {
            let mut best_dist_sq = f32::MAX;
            let mut best_tower: Option<ObjectID> = None;
            for tower_id in Self::get_bridge_tower_ids(target) {
                let Some(tower) = TheGameLogic::find_object_by_id(tower_id) else {
                    continue;
                };
                let Ok(tower_guard) = tower.read() else {
                    continue;
                };
                let mut tmp = Coord3D::ZERO;
                if Self::find_good_build_or_repair_position_with_locomotor(
                    me, &tower_guard, &mut tmp, owner_airborne,
                ) {
                    let dx = me.get_position().x - tmp.x;
                    let dy = me.get_position().y - tmp.y;
                    let dist_sq = dx * dx + dy * dy;
                    if dist_sq < best_dist_sq {
                        best_dist_sq = dist_sq;
                        *position_out = tmp;
                        best_tower = Some(tower_guard.get_id());
                    }
                }
            }
            if let Some(tower_id) = best_tower {
                return Some(tower_id);
            }
        }
        let _ = Self::find_good_build_or_repair_position_with_locomotor(
            me, target, position_out, owner_airborne,
        );
        Some(target.get_id())
    }

    fn get_bridge_tower_ids(target: &Object) -> Vec<ObjectID> {
        for module_handle in target.behavior_modules() {
            let ids = module_handle.with_module(|module| {
                module.get_bridge_control_interface().map(|bridge| {
                    bridge
                        .tower_ids()
                        .into_iter()
                        .filter(|id| *id != INVALID_ID)
                        .collect::<Vec<_>>()
                })
            });
            if let Some(ids) = ids {
                return ids;
            }
        }

        for mut behavior in target.get_behavior_modules() {
            let Ok(mut guard) = behavior.access() else {
                continue;
            };
            if let Some(interface) = guard.get_bridge_behavior_interface() {
                let mut ids = Vec::new();
                for tower_type in [
                    BridgeTowerType::North,
                    BridgeTowerType::South,
                    BridgeTowerType::East,
                    BridgeTowerType::West,
                ] {
                    let id = interface.get_tower_id(tower_type);
                    if id != INVALID_ID {
                        ids.push(id);
                    }
                }
                return ids;
            }
        }
        Vec::new()
    }
}

impl crate::modules::DozerAIUpdateInterface for DozerAIUpdate {
    fn admit_placed_build(
        &mut self,
        scaffold: &Object,
        dock: Coord3D,
        total_build_frames: u32,
        max_health: f32,
        frame: u32,
    ) -> Result<(), String> {
        if self.is_any_task_pending() || self.task_owner.current != DozerTask::Invalid {
            return Err("Dozer already has an admitted task".into());
        }
        if scaffold.get_builder_id() != self.object_id || !scaffold.is_under_construction() {
            return Err("placed scaffold does not belong to this Dozer".into());
        }
        self.install_placed_build_task(
            scaffold.get_id(),
            dock,
            *scaffold.get_position(),
            total_build_frames,
            max_health,
            frame,
        );
        Ok(())
    }

    fn active_build_target(&self) -> Option<ObjectID> {
        DozerAIUpdate::active_build_target(self)
    }

    #[cfg(test)]
    fn owned_state_observation_for_test(&self) -> Option<DozerOwnedObservation> {
        Some(DozerAIUpdate::owned_state_observation_for_test(self))
    }

    #[cfg(test)]
    fn construction_move_for_test(&self) -> Option<(Coord3D, bool)> {
        let dock = self.get_dock_point(DozerTask::Build, DozerDockPoint::Start)?;
        Some((dock, self.action_state == DozerActionState::MoveToActionPos))
    }

    #[cfg(test)]
    fn build_dock_sub_task_for_test(&self) -> DozerBuildSubTask {
        self.get_build_sub_task()
    }

    #[cfg(test)]
    fn build_end_dock_for_test(&self) -> Option<Coord3D> {
        self.get_dock_point(DozerTask::Build, DozerDockPoint::End)
    }

    fn validate_owned_snapshot(&self) -> Result<(), String> {
        DozerAIUpdate::validate_owned_snapshot(self)
    }
    fn xfer_owned_snapshot_tail(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer_owned_tail(xfer)
    }
    fn finish_cancelled_task_with_world(
        &mut self,
        task: DozerTask,
        world: &mut dyn crate::modules::ai_world::AiWorld,
    ) -> Result<(), String> {
        DozerAIUpdate::finish_cancelled_task_with_world(self, task, world)
    }

    fn set_build_task(
        &mut self,
        building_id: ObjectID,
        total_build_frames: u32,
        max_health: f32,
        is_rebuild: bool,
        owner_airborne: bool,
    ) {
        self.set_build_task(building_id, total_build_frames, max_health, is_rebuild, owner_airborne);
    }

    fn cancel_task(&mut self, task: DozerTask) {
        self.cancel_task(task);
    }

    fn supports_owned_task_cancel(&self) -> bool {
        true
    }
    fn cancel_task_bookkeeping_owned(&mut self, task: DozerTask) -> Result<(), String> {
        DozerAIUpdate::cancel_task_bookkeeping_owned(self, task)
    }
    fn finish_cancelled_task_on_idle_owned(&mut self, task: DozerTask) -> Result<(), String> {
        DozerAIUpdate::finish_cancelled_task_on_idle_owned(self, task)
    }

    fn get_current_task(&self) -> DozerTask {
        self.get_current_task()
    }

    fn is_task_pending(&self, task: DozerTask) -> bool {
        self.is_task_pending(task)
    }

    fn is_any_task_pending(&self) -> bool {
        self.is_any_task_pending()
    }
}

/// Module wrapper for DozerAIUpdate to align with module system expectations.
#[derive(Debug)]
pub struct DozerAIUpdateModule {
    module_name_key: NameKeyType,
    data: ModuleDataHandle,
    inherited: crate::object::update::AIUpdateInterfaceModule,
}

impl DozerAIUpdateModule {
    pub fn new(module_name_key: NameKeyType, data: ModuleDataHandle) -> Self {
        let inherited = crate::object::update::AIUpdateInterfaceModule::new(
            module_name_key,
            data.clone(),
        );
        Self {
            module_name_key,
            data,
            inherited,
        }
    }

    pub(crate) fn set_runtime_ai(&mut self, runtime: Arc<Mutex<dyn AIUpdateInterface>>) {
        self.inherited.set_runtime_ai(runtime);
    }
    pub(crate) fn runtime_ai_owned(&self) -> Option<Arc<Mutex<dyn AIUpdateInterface>>> {
        self.inherited.runtime_ai_owned()
    }
    pub(crate) fn xfer_with_locomotors(
        &mut self,
        xfer: &mut dyn Xfer,
        lookup: &mut crate::locomotor::LocomotorTemplateLookup<'_>,
    ) -> Result<(), String> {
        self.xfer_owned(xfer, Some(lookup))
    }
    fn xfer_owned(
        &mut self,
        xfer: &mut dyn Xfer,
        lookup: Option<&mut crate::locomotor::LocomotorTemplateLookup<'_>>,
    ) -> Result<(), String> {
        let runtime = self
            .inherited
            .runtime_ai_owned()
            .ok_or("Dozer snapshot runtime absent")?;
        if !xfer.is_reading() {
            runtime
                .lock()
                .map_err(|_| "Dozer snapshot runtime poisoned")?
                .validate_dozer_snapshot()?;
        }
        let mut version = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|error| error.to_string())?;
        match lookup {
            Some(lookup) => self.inherited.xfer_with_locomotors(xfer, lookup)?,
            None => self.inherited.xfer(xfer)?,
        }
        runtime
            .lock()
            .map_err(|_| "Dozer snapshot runtime poisoned")?
            .xfer_dozer_snapshot_tail(xfer)?;
        Ok(())
    }
    /// Only the inherited AI/Update portion, not a complete Dozer snapshot.
    #[cfg(test)]
    pub(crate) fn xfer_inherited_with_locomotors(
        &mut self,
        xfer: &mut dyn Xfer,
        lookup: &mut crate::locomotor::LocomotorTemplateLookup<'_>,
    ) -> Result<(), String> {
        self.inherited.xfer_with_locomotors(xfer, lookup)
    }
}

impl Module for DozerAIUpdateModule {
    fn get_initial_wake_frame(&mut self) -> Option<u32> {
        self.inherited.get_initial_wake_frame()
    }
    fn record_scheduled_wake(
        &mut self,
        frame: u32,
        phase: game_engine::common::thing::update_module::SleepyUpdatePhase,
    ) {
        self.inherited.record_scheduled_wake(frame, phase);
    }
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.read().get_module_tag_name_key()
    }

    fn get_module_data(&self) -> ModuleDataRead<'_> {
        ModuleDataRead::shared(&self.data)
    }
}

impl Snapshotable for DozerAIUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // CPP DozerAIUpdate.cpp2421: CRC extends only AIUpdateInterface.
        self.inherited.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.xfer_owned(xfer, None)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.inherited.load_post_process()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_dock_query_matches_cpp_bias_and_locomotor_options() {
        let owner = Coord3D::new(100.0, 0.0, 0.0);
        let target = Coord3D::new(0.0, 0.0, 0.0);
        let (ground_center, ground) = DozerAIUpdate::build_or_repair_position_query(
            17, &owner, 29, &target, 20.0, false,
        );
        assert_eq!(ground_center, Coord3D::new(10.0, 0.0, 0.0));
        assert_eq!(ground.min_radius, 0.0);
        assert_eq!(ground.max_radius, 100.0);
        assert_eq!(ground.max_z_delta, 10.0);
        assert_eq!(ground.source_to_path_to_dest_id, Some(17));
        assert_eq!(ground.ignore_object_id, None);

        let (air_center, air) = DozerAIUpdate::build_or_repair_position_query(
            17, &owner, 29, &target, 20.0, true,
        );
        assert_eq!(air_center, ground_center);
        assert_eq!(air.max_z_delta, FindPositionOptions::default().max_z_delta);
        assert_eq!(air.source_to_path_to_dest_id, Some(17));
        assert_eq!(air.ignore_object_id, Some(29));
    }

    #[test]
    fn build_end_dock_is_five_cells_outward_and_preserves_action_height() {
        let action = Coord3D::new(10.0, 20.0, 5.0);
        let target = Coord3D::new(0.0, 0.0, -2.0);
        let end = DozerAIUpdate::build_end_dock(&action, &target);
        let distance = 5.0 * PATHFIND_CELL_SIZE_F;
        let step_x = distance / 5.0_f32.sqrt();
        assert!((end.x - (10.0 + step_x)).abs() < 0.0001);
        assert!((end.y - (20.0 + 2.0 * step_x)).abs() < 0.0001);
        assert_eq!(end.z, action.z);
        let action_with_negative_zero = Coord3D::new(action.x, action.y, -0.0);
        assert_eq!(
            DozerAIUpdate::build_end_dock(&action_with_negative_zero, &target)
                .z
                .to_bits(),
            (-0.0_f32).to_bits()
        );
        assert_eq!(
            DozerAIUpdate::build_end_dock(&target, &target),
            target,
            "a zero XY direction leaves the End dock at the Action point",
        );
    }


}

#[cfg(test)]
#[path = "construction_callback_tests.rs"]
pub(crate) mod construction_callback_tests;
