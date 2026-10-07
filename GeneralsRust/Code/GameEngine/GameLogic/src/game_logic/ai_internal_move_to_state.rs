use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::ai::the_ai;
use crate::common::{
    BodyDamageType, Coord3D, KindOf, LOGICFRAMES_PER_SECOND, ModelConditionFlags,
    ObjectStatusTypes, PathfindLayerEnum, UnsignedInt, Xfer, XferExt, XferMode, XferVersion,
};
use crate::helpers::{TheAudio, TheGameLogic};
use crate::locomotor::LocomotorAppearance;
use crate::modules::AIUpdateInterface;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::state_machine::{StateExitType, StateMachine, StateReturnType};
use crate::terrain::get_terrain_logic;

// C++ authority: GameLogic/AI/AIStates.cpp::AIInternalMoveToState. The public
// entry points retain id-based compatibility; owner-driven machines can lend
// their already-held AI interface through the *_with_ai methods.
/// Internal move-to helper bridging legacy AI move states to the modern state machine.
///
/// Matches the core behavior of C++ AIInternalMoveToState (path request, block handling,
/// and goal completion checks), tailored to the current Rust locomotor/pathing pipeline.
#[derive(Debug)]
pub struct AIInternalMoveToState {
    name: String,
    machine: Weak<Mutex<StateMachine>>,
    goal_position: Coord3D,
    goal_object_id: crate::common::ObjectID,
    owner_id: crate::common::ObjectID,
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
    /// Create a new helper bound to the provided state machine.
    pub fn new(machine: &Arc<Mutex<StateMachine>>, name: String) -> Result<Self, String> {
        Ok(Self {
            name,
            machine: Arc::downgrade(machine),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            goal_object_id: crate::common::INVALID_ID,
            owner_id: crate::common::INVALID_ID,
            goal_layer: PathfindLayerEnum::Invalid,
            waiting_for_path: false,
            path_goal_position: Coord3D::new(0.0, 0.0, 0.0),
            path_timestamp: 0,
            blocked_repath_timestamp: 0,
            try_one_more_repath: true,
            adjusts_destination: true,
            ambient_playing_handle: 0,
        })
    }

    /// Machine-less variant for helpers driven by a state machine their AI owns
    /// outright (guard machines, C++ `AIGuardReturnState :
    /// AIInternalMoveToState`). Every machine lookup falls back to the copied
    /// ids, exactly as when the weak handle expires, so the helper resolves the
    /// owner through the id-keyed registries instead of a machine handle.
    pub fn new_with_owner_id(owner_id: crate::common::ObjectID, name: String) -> Self {
        Self {
            name,
            machine: Weak::new(),
            goal_position: Coord3D::new(0.0, 0.0, 0.0),
            goal_object_id: crate::common::INVALID_ID,
            owner_id,
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

    fn upgrade_machine(&self) -> Result<Arc<Mutex<StateMachine>>, String> {
        self.machine.upgrade().ok_or_else(|| {
            format!(
                "AIInternalMoveToState '{}' lost its machine context",
                self.name
            )
        })
    }

    fn with_machine<F, R>(&self, f: F) -> Result<R, String>
    where
        F: FnOnce(&mut StateMachine) -> R,
    {
        let machine = self.upgrade_machine()?;
        let Ok(mut guard) = machine.try_lock() else {
            return Err(format!(
                "AIInternalMoveToState '{}' machine lock busy",
                self.name
            ));
        };
        Ok(f(&mut guard))
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
        self.on_enter_with_ai(&mut *ai_guard)
    }

    /// Owner-borrowed enter path for callers that already hold this unit's AI
    /// interface. It performs the same movement setup without reacquiring the
    /// AI handle stored on the owner object.
    pub fn on_enter_with_ai(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        let owner = self.get_machine_owner()?;
        {
            let mut owner_guard = owner
                .write()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                return Ok(StateReturnType::Failure);
            }
            owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
            let owner_pos = *owner_guard.get_position();
            if is_cliff_at(&owner_pos) {
                owner_guard.set_model_condition_state(ModelConditionFlags::CLIMBING);
                owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
            }
        }

        ai.with_cur_locomotor_mut(&mut |loco| {
            if loco.is_ultra_accurate() {
                self.set_adjusts_destination(false);
            }
            // C++ AIInternalMoveToState::onEnter (AIStates.cpp:1604-1605).
            loco.start_move();
        });

        ai.set_adjusts_destination(self.get_adjusts_destination_with_ai(ai));

        if let Ok(goal) = self.get_machine_goal_position() {
            self.goal_position = goal;
        }
        {
            let mut owner_guard = owner
                .write()
                .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
            if let Ok(Some(goal_id)) = self.get_machine_goal_object_id() {
                if let Some(goal_pos) =
                    crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
                        let mut goal_pos = *goal_guard.get_position();
                        if owner_guard.is_kind_of(KindOf::Projectile) {
                            let half_height = goal_guard
                                .get_geometry_info()
                                .get_max_height_above_position()
                                * 0.5;
                            goal_pos.z += half_height;
                            if goal_guard.get_position().z < goal_pos.z {
                                goal_pos.z += half_height;
                            }
                        }
                        goal_pos
                    })
                {
                    self.goal_position = goal_pos;
                }
            }
        }

        self.waiting_for_path = false;
        self.try_one_more_repath = true;
        self.path_goal_position = self.goal_position;
        self.path_timestamp = TheGameLogic::get_frame();
        self.ambient_playing_handle = 0;

        ai.set_movement_target(&self.goal_position)
            .map_err(|err| format!("AIInternalMoveToState set_movement_target failed: {}", err))?;
        let _ = ai.set_path_extra_distance(0.0);

        let owner_guard = owner
            .read()
            .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
        self.start_move_sound(&owner_guard);
        Ok(StateReturnType::Continue)
    }

    /// Update hook – drives path recompute and completion checks.
    pub fn update(&mut self) -> Result<StateReturnType, String> {
        let ai = self.owner_ai()?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIInternalMoveToState AI lock poisoned".to_string())?;
        self.update_with_ai(&mut *ai_guard)
    }

    /// Owner-borrowed update path for callers that already hold this unit's AI
    /// interface. The helper keeps its path, timing, and goal state unchanged.
    pub fn update_with_ai(
        &mut self,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<StateReturnType, String> {
        let owner = self.get_machine_owner()?;
        let mut moving_backwards = false;
        let mut close_enough = 0.0;
        ai.with_cur_locomotor(&mut |loco| {
            moving_backwards = loco.is_moving_backwards();
            close_enough = loco.get_close_enough_dist();
        });
        let frames_blocked = ai.get_num_frames_blocked();
        let blocked = ai.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        let goal_object_id = self.get_machine_goal_object_id().ok().flatten();
        let mut owner_guard = owner
            .write()
            .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
        let owner_pos = *owner_guard.get_position();
        if let Some(goal_id) = goal_object_id {
            if let Some(new_goal) =
                crate::object::registry::OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
                    let mut new_goal = *goal_guard.get_position();
                    if owner_guard.is_kind_of(KindOf::Projectile) {
                        let half_height = goal_guard
                            .get_geometry_info()
                            .get_max_height_above_position()
                            * 0.5;
                        new_goal.z += half_height;
                        if goal_guard.get_position().z < new_goal.z {
                            new_goal.z += half_height;
                        }
                    }
                    new_goal
                })
            {
                self.goal_position = new_goal;
                if !self.is_same_position(&owner_pos, &self.path_goal_position, &new_goal) {
                    self.path_timestamp = 0;
                }
            }
        }

        let mut repath_target = None;
        if blocked {
            owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
            let now = TheGameLogic::get_frame();
            let repath_delay = LOGICFRAMES_PER_SECOND;
            if now.saturating_sub(self.blocked_repath_timestamp) >= repath_delay {
                self.blocked_repath_timestamp = now;
                repath_target = Some((self.goal_position, now));
            }
        } else {
            let mut set_condition_flag = ModelConditionFlags::MOVING;
            if is_cliff_at(owner_guard.get_position()) {
                set_condition_flag = if moving_backwards {
                    ModelConditionFlags::RAPPELLING
                } else {
                    ModelConditionFlags::CLIMBING
                };
            }

            if frames_blocked > LOGICFRAMES_PER_SECOND / 4 {
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
            } else {
                owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                if set_condition_flag == ModelConditionFlags::MOVING {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                    owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                } else {
                    let clear_flag = if set_condition_flag == ModelConditionFlags::CLIMBING {
                        ModelConditionFlags::RAPPELLING
                    } else {
                        ModelConditionFlags::CLIMBING
                    };
                    owner_guard.clear_model_condition_state(clear_flag);
                    owner_guard.set_model_condition_state(set_condition_flag);
                }
            }

            let now = TheGameLogic::get_frame();
            if now.saturating_sub(self.path_timestamp) > MIN_REPATH_TIME
                && !self.is_same_position(&owner_pos, &self.path_goal_position, &self.goal_position)
            {
                repath_target = Some((self.goal_position, now));
            }
        }
        drop(owner_guard);
        if let Some((target, now)) = repath_target {
            ai.set_movement_target(&target)
                .map_err(|err| format!("AIInternalMoveToState repath failed: {}", err))?;
            self.path_goal_position = target;
            self.path_timestamp = now;
        }
        let dist_remaining = ai.get_locomotor_distance_to_goal();
        if dist_remaining <= close_enough {
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
        self.on_exit_with_ai(status, &mut *ai_guard)
    }

    /// Owner-borrowed exit path for callers that already hold this unit's AI
    /// interface.
    pub fn on_exit_with_ai(
        &mut self,
        _status: StateExitType,
        ai: &mut dyn AIUpdateInterface,
    ) -> Result<(), String> {
        self.stop_move_sound();
        if let Ok(owner) = self.get_machine_owner() {
            ai.friend_ending_move();
            let goal = self.goal_position;
            let mut snap = false;
            ai.with_cur_locomotor(&mut |loco| {
                snap = loco.is_ultra_accurate()
                    && !matches!(
                        loco.get_appearance(),
                        LocomotorAppearance::Hover
                            | LocomotorAppearance::Thrust
                            | LocomotorAppearance::Wings
                    );
            });
            if snap {
                if let Ok(mut owner_guard) = owner.write() {
                    let dx = goal.x - owner_guard.get_position().x;
                    let dy = goal.y - owner_guard.get_position().y;
                    if dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
                        let _ = owner_guard.set_position(&goal);
                    }
                }
            }
            ai.destroy_path();
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
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
        if let Ok(owner) = self.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            }
        }
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
        let _ = self.with_machine(|machine| machine.set_goal_position(pos));
    }

    fn get_machine_goal_position(&self) -> Result<Coord3D, String> {
        if let Ok(machine) = self.upgrade_machine() {
            if let Ok(guard) = machine.try_lock() {
                return Ok(guard.get_goal_position());
            }
        }
        Ok(self.goal_position)
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
        let id = self.get_machine_owner_id()?;
        crate::helpers::TheGameLogic::find_object_by_id(id)
            .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(id))
            .ok_or_else(|| "state machine owner not set".to_string())
    }
    pub fn note_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.goal_object_id = id;
    }

    pub fn note_owner_id(&mut self, id: crate::common::ObjectID) {
        self.owner_id = id;
    }

    pub fn get_machine_goal_object_id(&self) -> Result<Option<crate::common::ObjectID>, String> {
        if let Ok(machine) = self.upgrade_machine() {
            if let Ok(guard) = machine.try_lock() {
                let id = guard.get_goal_object_id();
                if id != crate::common::INVALID_ID {
                    return Ok(Some(id));
                }
            }
        }
        if self.goal_object_id == crate::common::INVALID_ID {
            Ok(None)
        } else {
            Ok(Some(self.goal_object_id))
        }
    }

    pub fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if let Ok(machine) = self.upgrade_machine() {
            if let Ok(guard) = machine.try_lock() {
                let id = guard.get_owner_id();
                if id != crate::common::INVALID_ID {
                    return Ok(id);
                }
            }
        }
        if self.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not set".to_string())
        } else {
            Ok(self.owner_id)
        }
    }

    /// Obtain a handle to the underlying state machine.
    pub fn get_machine(&self) -> Result<Arc<Mutex<StateMachine>>, String> {
        self.upgrade_machine()
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

    fn get_adjusts_destination_with_ai(&self, ai: &dyn AIUpdateInterface) -> bool {
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
