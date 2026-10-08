use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::ai::the_ai;
use crate::common::{
    BodyDamageType, Coord3D, KindOf, LOGICFRAMES_PER_SECOND, ModelConditionFlags,
    ObjectStatusTypes, PathfindLayerEnum, UnsignedInt, Xfer, XferExt, XferMode, XferVersion,
};
use crate::helpers::{TheAudio, TheGameLogic};
use crate::locomotor::LocomotorAppearance;
use crate::modules::{AIUpdateInterface, PhysicsBehavior};
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::state_machine::{StateExitType, StateMachine, StateReturnType};
use crate::terrain::get_terrain_logic;

// C++ authority: GameLogic/AI/AIStates.cpp::AIInternalMoveToState.
/// Internal movement helper driven by a caller's AI runtime.
/// Owner-driven machines bind their exact object; standalone entry points can
/// resolve an unbound helper's owner ID at their existing admission boundary.
#[derive(Debug)]
pub struct AIInternalMoveToState {
    name: String,
    goal_position: Coord3D,
    goal_object_id: crate::common::ObjectID,
    owner_id: crate::common::ObjectID,
    owner: Option<Weak<RwLock<Object>>>,
    goal_layer: PathfindLayerEnum,
    waiting_for_path: bool,
    path_goal_position: Coord3D,
    path_timestamp: UnsignedInt,
    blocked_repath_timestamp: UnsignedInt,
    try_one_more_repath: bool,
    adjusts_destination: bool,
    ambient_playing_handle: u32,
}

const MIN_REPATH_TIME: UnsignedInt = 10;

fn is_cliff_at(pos: &Coord3D) -> bool {
    get_terrain_logic()
        .read()
        .map(|terrain| terrain.is_cliff_cell(pos.x, pos.y))
        .unwrap_or(false)
}

impl AIInternalMoveToState {
    /// Construct inertly; owning guard and dock machines bind their object
    /// before invoking callbacks through the borrowed AI runtime.
    pub fn new_with_owner_id(owner_id: crate::common::ObjectID, name: String) -> Self {
        Self {
            name,
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            goal_object_id: crate::common::INVALID_ID,
            owner_id,
            owner: None,
            goal_layer: PathfindLayerEnum::Invalid,
            waiting_for_path: false,
            path_goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            try_one_more_repath: true,
            adjusts_destination: true,
            ambient_playing_handle: 0,
        }
    }

    fn owner_ai(&self) -> Result<Arc<Mutex<dyn AIUpdateInterface>>, String> {
        let owner = self.get_machine_owner()?;
        let ai = owner
            .read()
            .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?
            .get_ai_update_interface()
            .ok_or_else(|| "AIInternalMoveToState missing AIUpdateInterface".to_string())?;
        Ok(ai)
    }

    /// Hook invoked when the enclosing state machine enters the move helper.
    pub fn on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self.get_machine_owner()?;
        let ai = {
            let owner_guard = owner
                .read()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                return Ok(StateReturnType::Failure);
            }
            owner_guard
                .get_ai_update_interface()
                .ok_or_else(|| "AIInternalMoveToState missing AIUpdateInterface".to_string())?
        };
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIInternalMoveToState AI lock poisoned".to_string())?;
        self.on_enter_with_ai(
            &mut crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(&mut *ai_guard),
        )
    }

    /// Owner-borrowed enter path for callers that already hold this unit's AI
    /// interface. It performs the same movement setup without reacquiring the
    /// AI handle stored on the owner object.
    pub fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        let owner = self.get_machine_owner()?;
        self.ambient_playing_handle = 0;
        self.waiting_for_path = ai.is_waiting_for_path();
        {
            let owner_guard = owner
                .read()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                return Ok(StateReturnType::Failure);
            }
        }

        let mut ultra_accurate = false;
        ai.with_cur_locomotor_mut(&mut |loco| {
            loco.start_move();
            ultra_accurate = loco.is_ultra_accurate();
        });
        if ultra_accurate {
            self.adjusts_destination = false;
        }
        self.try_one_more_repath = true;
        ai.friend_starting_move();

        let rider8 = {
            let mut owner_guard = owner
                .write()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            let owner_pos = *owner_guard.get_position();
            let is_motive = owner_guard
                .get_physics()
                .and_then(|physics| physics.access().ok().map(|physics| physics.is_motive()))
                .unwrap_or(false);
            if is_motive {
                if is_cliff_at(&owner_pos) {
                    owner_guard.set_model_condition_state(ModelConditionFlags::CLIMBING);
                } else {
                    owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                }
            } else if owner_guard.is_kind_of(KindOf::Dozer)
                && owner_guard.is_kind_of(KindOf::Harvester)
            {
                owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
            }
            owner_guard.test_status(ObjectStatusTypes::Rider8)
        };
        let parachuting = owner
            .read()
            .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?
            .test_status(ObjectStatusTypes::Parachuting);
        let ai_allows_adjustment = ai.is_allowed_to_adjust_destination();
        let adjust_destination =
            self.get_adjusts_destination_for(parachuting, ai_allows_adjustment);

        if adjust_destination && !rider8 {
            if !ai.adjust_destination(&mut self.goal_position) {
                let owner_guard = owner
                    .read()
                    .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
                ai.snap_closest_goal_position(&owner_guard, &mut self.goal_position);
                drop(owner_guard);
            }
            let goal_layer = get_terrain_logic()
                .read()
                .map(|terrain| {
                    PathfindLayerEnum::from_u32(
                        terrain.get_layer_for_destination(&self.goal_position) as u32,
                    )
                })
                .unwrap_or(PathfindLayerEnum::Invalid);
            ai.update_goal_position(&self.goal_position, goal_layer)?;
        }

        if let Err(error) = self.compute_path_with_ai(adjust_destination, ai) {
            ai.friend_ending_move();
            return Err(error);
        }
        let _ = ai.set_path_extra_distance(0.0);
        ai.set_desired_speed(crate::modules::FAST_AS_POSSIBLE);

        let owner_guard = owner
            .read()
            .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
        self.start_move_sound(&owner_guard);
        Ok(StateReturnType::Continue)
    }

    fn get_adjusts_destination_for(
        &self,
        is_parachuting: bool,
        ai_allows_adjustment: bool,
    ) -> bool {
        self.adjusts_destination && !is_parachuting && ai_allows_adjustment
    }

    fn compute_path_with_ai(
        &mut self,
        adjust: bool,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<(), String> {
        self.waiting_for_path = true;
        ai.request_path(&self.goal_position, adjust)?;
        ai.friend_starting_move();
        Ok(())
    }

    /// Update hook – drives path recompute and completion checks.
    pub fn update(&mut self) -> Result<StateReturnType, String> {
        let ai = self.owner_ai()?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIInternalMoveToState AI lock poisoned".to_string())?;
        self.update_with_ai(
            &mut crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(&mut *ai_guard),
        )
    }

    /// Owner-borrowed update path for callers that already hold this unit's AI
    /// interface. The helper keeps its path, timing, and goal state unchanged.
    pub fn update_with_ai(
        &mut self,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<StateReturnType, String> {
        let owner = self.get_machine_owner()?;
        if self.waiting_for_path {
            self.path_timestamp = TheGameLogic::get_frame();
            if ai.is_waiting_for_path() {
                return Ok(StateReturnType::Continue);
            }
            if ai.get_path().is_none() {
                return Ok(StateReturnType::Failure);
            }
            self.waiting_for_path = false;
            self.path_goal_position = self.goal_position;
            let is_parachuting = owner
                .read()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?
                .test_status(ObjectStatusTypes::Parachuting);
            let ai_allows_adjustment = ai.is_allowed_to_adjust_destination();
            let adjust = self.get_adjusts_destination_for(is_parachuting, ai_allows_adjustment);
            if adjust {
                if let (Some(last), Some(layer)) =
                    (ai.get_path_last_node(), ai.installed_path_last_layer())
                {
                    ai.update_goal_position(&last, PathfindLayerEnum::from_u32(u32::from(layer)))?;
                }
            } else {
                ai.remove_pathfinder_goal();
            }
            if !ai.get_retry_path() {
                self.try_one_more_repath = false;
            }
        }

        let path_present = ai.get_path().is_some();
        let mut force_recompute = !path_present;
        let frames_blocked = ai.get_num_frames_blocked();
        let blocked = ai.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        if blocked {
            force_recompute = true;
            self.blocked_repath_timestamp = TheGameLogic::get_frame();
        }

        let owner_pos = {
            let owner_guard = owner
                .read()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            *owner_guard.get_position()
        };
        let mut moving_backwards = false;
        ai.with_cur_locomotor(&mut |loco| moving_backwards = loco.is_moving_backwards());
        let cliff = is_cliff_at(&owner_pos);
        {
            let mut owner_guard = owner
                .write()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            if cliff {
                let condition = if moving_backwards {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                    ModelConditionFlags::RAPPELLING
                } else {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                    ModelConditionFlags::CLIMBING
                };
                if frames_blocked <= LOGICFRAMES_PER_SECOND / 4 {
                    owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                    owner_guard.set_model_condition_state(condition);
                }
            } else if frames_blocked <= LOGICFRAMES_PER_SECOND / 4 {
                owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
            }
            if frames_blocked > LOGICFRAMES_PER_SECOND / 4 {
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            }
        }

        if ai.can_compute_quick_path() {
            let owner_guard = owner
                .read()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            if owner_guard.is_kind_of(KindOf::Projectile) {
                self.path_timestamp = 0;
                force_recompute = true;
            }
        }
        if path_present {
            ai.set_locomotor_goal_position_on_path();
        }
        let now = TheGameLogic::get_frame();
        let time_up = now.saturating_sub(self.path_timestamp) > MIN_REPATH_TIME;
        if force_recompute || time_up {
            let (is_parachuting, goal_moved) = {
                let owner_guard = owner
                    .read()
                    .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
                (
                    owner_guard.test_status(ObjectStatusTypes::Parachuting),
                    !self.is_same_position(
                        owner_guard.get_position(),
                        &self.path_goal_position,
                        &self.goal_position,
                    ),
                )
            };
            let ai_allows_adjustment = ai.is_allowed_to_adjust_destination();
            let adjust = self.get_adjusts_destination_for(is_parachuting, ai_allows_adjustment);
            if force_recompute || goal_moved {
                if let Err(error) = self.compute_path_with_ai(adjust, ai) {
                    ai.friend_ending_move();
                    return Err(error);
                }
                if ai.get_path().is_none() {
                    return Ok(StateReturnType::Continue);
                }
                ai.set_locomotor_goal_position_on_path();
            }
        }

        let mut close_enough = None;
        ai.with_cur_locomotor(&mut |loco| close_enough = Some(loco.get_close_enough_dist()));
        let Some(close_enough) = close_enough else {
            return Ok(StateReturnType::Continue);
        };
        if ai.get_locomotor_distance_to_goal(Some(self.goal_position)) < close_enough {
            if ai.is_doing_ground_movement() {
                let goal = ai.get_path_last_node().unwrap_or(self.goal_position);
                let owner_guard = owner
                    .read()
                    .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
                let pos = owner_guard.get_position();
                let dx = pos.x - goal.x;
                let dy = pos.y - goal.y;
                if (dx * dx + dy * dy).sqrt() > 4.0 * PATHFIND_CELL_SIZE_F {
                    return Ok(StateReturnType::Continue);
                }
            }
            let is_parachuting = owner
                .read()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?
                .test_status(ObjectStatusTypes::Parachuting);
            let ai_allows_adjustment = ai.is_allowed_to_adjust_destination();
            let adjust = self.get_adjusts_destination_for(is_parachuting, ai_allows_adjustment);
            if adjust {
                ai.set_locomotor_goal_none();
            }
            return Ok(StateReturnType::Success);
        }
        Ok(StateReturnType::Continue)
    }

    /// Called when the state exits (successfully or otherwise).
    pub fn on_exit(&mut self, status: StateExitType) -> Result<(), String> {
        let Ok(ai) = self.owner_ai() else {
            self.cleanup_exit_without_ai();
            return Ok(());
        };
        let Ok(mut ai_guard) = ai.lock() else {
            self.cleanup_exit_without_ai();
            return Ok(());
        };
        self.on_exit_with_ai(
            status,
            &mut crate::modules::ai_state_runtime::AiUpdateRuntimeAdapter(&mut *ai_guard),
        )
    }

    /// Owner-borrowed exit path for callers that already hold this unit's AI
    /// interface.
    pub fn on_exit_with_ai(
        &mut self,
        _status: StateExitType,
        ai: &mut dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> Result<(), String> {
        self.stop_move_sound();
        ai.friend_ending_move();
        let mut ultra_accurate = false;
        ai.with_cur_locomotor(&mut |loco| ultra_accurate = loco.is_ultra_accurate());
        if ai.is_doing_ground_movement() && ultra_accurate {
            let owner = self.get_machine_owner()?;
            let owner_guard = owner
                .read()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            let dx = self.goal_position.x - owner_guard.get_position().x;
            let dy = self.goal_position.y - owner_guard.get_position().y;
            let should_snap = owner_guard.get_team().is_some()
                && dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F;
            drop(owner_guard);
            if should_snap {
                ai.set_final_position(&self.goal_position);
            }
        }
        Ok(())
    }

    fn stop_move_sound(&mut self) {
        if self.ambient_playing_handle != 0 {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.ambient_playing_handle);
            }
            self.ambient_playing_handle = 0;
        }
    }

    fn cleanup_exit_without_ai(&mut self) {
        self.stop_move_sound();
    }

    fn start_move_sound(&mut self, owner_guard: &Object) {
        let mut use_damaged = false;
        if let Some(body) = owner_guard.get_body_module() {
            if let Ok(body_guard) = body.lock() {
                use_damaged = body_guard.get_damage_state() > BodyDamageType::Damaged;
            }
        }

        let template = owner_guard.get_template();
        let mut start_sound = if use_damaged {
            template.get_sound_move_start_damaged()
        } else {
            template.get_sound_move_start()
        };
        let mut loop_sound = if use_damaged {
            template.get_sound_move_loop_damaged()
        } else {
            template.get_sound_move_loop()
        };

        if let Some(audio) = TheAudio::get() {
            if !start_sound.get_event_name().is_empty() {
                start_sound.set_object_id(owner_guard.get_id());
                audio.add_audio_event(&start_sound);
            } else if !loop_sound.get_event_name().is_empty() {
                loop_sound.set_object_id(owner_guard.get_id());
                self.ambient_playing_handle = audio.add_audio_event(&loop_sound);
            }
        }
    }

    /// Set the target goal position for the underlying move helper.
    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.goal_position = pos;
    }

    fn is_same_position(
        &self,
        our_pos: &Coord3D,
        prev_target_pos: &Coord3D,
        cur_target_pos: &Coord3D,
    ) -> bool {
        let diff_x = cur_target_pos.x - prev_target_pos.x;
        let diff_y = cur_target_pos.y - prev_target_pos.y;

        let to_target_x = cur_target_pos.x - our_pos.x;
        let to_target_y = cur_target_pos.y - our_pos.y;

        let tolerance_sqr = (to_target_x * to_target_x + to_target_y * to_target_y) * (1.0 / 100.0);
        diff_x * diff_x + diff_y * diff_y <= tolerance_sqr
    }

    pub fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("AIInternalMoveToState xfer version failed: {:?}", e))?;

        xfer.xfer_real(&mut self.goal_position.x)
            .map_err(|e| format!("AIInternalMoveToState xfer goal_position.x failed: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.y)
            .map_err(|e| format!("AIInternalMoveToState xfer goal_position.y failed: {:?}", e))?;
        xfer.xfer_real(&mut self.goal_position.z)
            .map_err(|e| format!("AIInternalMoveToState xfer goal_position.z failed: {:?}", e))?;

        let mut goal_layer_value = self.goal_layer as u32;
        xfer.xfer_unsigned_int(&mut goal_layer_value)
            .map_err(|e| format!("AIInternalMoveToState xfer goal_layer failed: {:?}", e))?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.goal_layer = PathfindLayerEnum::from_u32(goal_layer_value);
        }

        game_engine::system::Xfer::xfer_bool(xfer, &mut self.waiting_for_path).map_err(|e| {
            format!(
                "AIInternalMoveToState xfer waiting_for_path failed: {:?}",
                e
            )
        })?;

        xfer.xfer_real(&mut self.path_goal_position.x)
            .map_err(|e| {
                format!(
                    "AIInternalMoveToState xfer path_goal_position.x failed: {:?}",
                    e
                )
            })?;
        xfer.xfer_real(&mut self.path_goal_position.y)
            .map_err(|e| {
                format!(
                    "AIInternalMoveToState xfer path_goal_position.y failed: {:?}",
                    e
                )
            })?;
        xfer.xfer_real(&mut self.path_goal_position.z)
            .map_err(|e| {
                format!(
                    "AIInternalMoveToState xfer path_goal_position.z failed: {:?}",
                    e
                )
            })?;
        xfer.xfer_unsigned_int(&mut self.path_timestamp)
            .map_err(|e| format!("AIInternalMoveToState xfer path_timestamp failed: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.blocked_repath_timestamp)
            .map_err(|e| {
                format!(
                    "AIInternalMoveToState xfer blocked_repath_timestamp failed: {:?}",
                    e
                )
            })?;
        game_engine::system::Xfer::xfer_bool(xfer, &mut self.adjusts_destination).map_err(|e| {
            format!(
                "AIInternalMoveToState xfer adjusts_destination failed: {:?}",
                e
            )
        })?;

        Ok(())
    }

    pub fn load_post_process(&mut self) -> Result<(), String> {
        if let Ok(owner) = self.get_machine_owner() {
            if let Ok(owner_guard) = owner.read() {
                self.start_move_sound(&owner_guard);
            }
        }
        Ok(())
    }

    /// Access the machine goal object if present.
    pub fn get_machine_goal_object(&self) -> Result<Option<Arc<RwLock<Object>>>, String> {
        let id = self.get_machine_goal_object_id()?;
        Ok(id.and_then(|goal_id| {
            crate::helpers::TheGameLogic::find_object_by_id(goal_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(goal_id))
        }))
    }

    /// Access the machine owner object.
    pub fn get_machine_owner(&self) -> Result<Arc<RwLock<Object>>, String> {
        if let Some(owner) = &self.owner {
            return owner
                .upgrade()
                .ok_or_else(|| "state machine owner expired".to_string());
        }
        let id = self.get_machine_owner_id()?;
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
            .ok_or_else(|| "state machine owner not set".to_string())
    }
    pub fn note_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.goal_object_id = id;
    }

    pub fn note_owner_id(&mut self, id: crate::common::ObjectID) {
        if self.owner_id != id {
            self.owner = None;
        }
        self.owner_id = id;
    }

    pub(crate) fn bind_owner(&mut self, owner: &Arc<RwLock<Object>>) {
        self.owner = Some(Arc::downgrade(owner));
    }

    pub(crate) fn bind_machine_owner(&mut self, machine: &StateMachine) {
        self.owner_id = machine.get_owner_id();
        self.owner = machine.owner_reference();
    }

    pub fn get_machine_goal_object_id(&self) -> Result<Option<crate::common::ObjectID>, String> {
        Ok((self.goal_object_id != crate::common::INVALID_ID).then_some(self.goal_object_id))
    }

    pub fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.owner.is_some() {
            return (self.owner_id != crate::common::INVALID_ID)
                .then_some(self.owner_id)
                .ok_or_else(|| "state machine owner not set".to_string());
        }
        (self.owner_id != crate::common::INVALID_ID)
            .then_some(self.owner_id)
            .ok_or_else(|| "state machine owner not set".to_string())
    }

    /// Whether the move helper adjusts its destination on the fly.
    pub fn get_adjusts_destination(&self) -> bool {
        if !self.adjusts_destination {
            return false;
        }
        if let Ok(owner) = self.get_machine_owner() {
            if let Ok(guard) = owner.read() {
                if guard.test_status(ObjectStatusTypes::Parachuting) {
                    return false;
                }
                if let Some(ai) = guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        if !ai_guard.is_allowed_to_adjust_destination() {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    fn get_adjusts_destination_with_ai(
        &self,
        ai: &dyn crate::modules::ai_state_runtime::AiStateRuntime,
    ) -> bool {
        if !self.adjusts_destination {
            return false;
        }
        if let Ok(owner) = self.get_machine_owner() {
            if let Ok(guard) = owner.read() {
                if guard.test_status(ObjectStatusTypes::Parachuting) {
                    return false;
                }
            }
        }
        ai.is_allowed_to_adjust_destination()
    }

    /// Configure whether the helper adjusts its destination dynamically.
    pub fn set_adjusts_destination(&mut self, adjust: bool) {
        self.adjusts_destination = adjust;
    }
}

#[cfg(test)]
#[path = "ai_internal_move_to_state_tests.rs"]
mod tests;
