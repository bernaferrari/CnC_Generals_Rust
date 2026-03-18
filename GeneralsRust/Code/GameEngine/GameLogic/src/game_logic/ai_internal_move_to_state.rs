use std::sync::{Arc, Mutex, RwLock, Weak};

use crate::ai::THE_AI;
use crate::common::{
    BodyDamageType, Coord3D, KindOf, ModelConditionFlags, ObjectStatusTypes, PathfindLayerEnum,
    UnsignedInt, Xfer, XferExt, XferMode, XferVersion, LOGICFRAMES_PER_SECOND,
};
use crate::helpers::{TheAudio, TheGameLogic};
use crate::locomotor::LocomotorAppearance;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::state_machine::{StateExitType, StateMachine, StateReturnType};
use crate::terrain::get_terrain_logic;

/// Internal move-to helper bridging legacy AI move states to the modern state machine.
///
/// Matches the core behavior of C++ AIInternalMoveToState (path request, block handling,
/// and goal completion checks), tailored to the current Rust locomotor/pathing pipeline.
#[derive(Debug)]
pub struct AIInternalMoveToState {
    name: String,
    machine: Weak<Mutex<StateMachine>>,
    goal_position: Coord3D,
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
        let mut guard = machine.lock().map_err(|_| {
            format!(
                "AIInternalMoveToState '{}' failed to lock machine",
                self.name
            )
        })?;
        Ok(f(&mut guard))
    }

    /// Hook invoked when the enclosing state machine enters the move helper.
    pub fn on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner = self.get_machine_owner()?;
        let mut owner_guard = owner
            .write()
            .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;

        if owner_guard.test_status(ObjectStatusTypes::Immobile) {
            return Ok(StateReturnType::Failure);
        }

        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "AIInternalMoveToState missing AIUpdateInterface".to_string())?;
        owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
        if is_cliff_at(owner_guard.get_position()) {
            owner_guard.set_model_condition_state(ModelConditionFlags::CLIMBING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
        }
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIInternalMoveToState AI lock poisoned".to_string())?;

        if let Some(locomotor) = ai_guard.get_cur_locomotor() {
            if let Ok(loco_guard) = locomotor.lock() {
                if loco_guard.is_ultra_accurate() {
                    self.set_adjusts_destination(false);
                }
            }
        }

        ai_guard.set_adjusts_destination(self.get_adjusts_destination());

        if let Ok(goal) = self.get_machine_goal_position() {
            self.goal_position = goal;
        }
        if let Ok(Some(goal_obj)) = self.get_machine_goal_object() {
            if let Ok(goal_guard) = goal_obj.read() {
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
                self.goal_position = goal_pos;
            }
        }

        self.waiting_for_path = false;
        self.try_one_more_repath = true;
        self.path_goal_position = self.goal_position;
        self.path_timestamp = TheGameLogic::get_frame();
        self.ambient_playing_handle = 0;

        ai_guard
            .set_movement_target(&self.goal_position)
            .map_err(|err| format!("AIInternalMoveToState set_movement_target failed: {}", err))?;
        let _ = ai_guard.set_path_extra_distance(0.0);

        self.start_move_sound(&owner_guard);
        Ok(StateReturnType::Continue)
    }

    /// Update hook – drives path recompute and completion checks.
    pub fn update(&mut self) -> Result<StateReturnType, String> {
        let owner = self.get_machine_owner()?;
        let mut owner_guard = owner
            .write()
            .map_err(|_| "AIInternalMoveToState owner lock poisoned".to_string())?;
        let owner_pos = *owner_guard.get_position();

        let ai = owner_guard
            .get_ai_update_interface()
            .ok_or_else(|| "AIInternalMoveToState missing AIUpdateInterface".to_string())?;
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIInternalMoveToState AI lock poisoned".to_string())?;

        if let Ok(Some(goal_obj)) = self.get_machine_goal_object() {
            if let Ok(goal_guard) = goal_obj.read() {
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
                self.goal_position = new_goal;
                if !self.is_same_position(&owner_pos, &self.path_goal_position, &new_goal) {
                    self.path_timestamp = 0;
                }
            }
        }

        let frames_blocked = ai_guard.get_num_frames_blocked();
        let blocked =
            ai_guard.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        if blocked {
            owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
            owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
            let now = TheGameLogic::get_frame();
            let repath_delay = LOGICFRAMES_PER_SECOND;
            if now.saturating_sub(self.blocked_repath_timestamp) >= repath_delay {
                self.blocked_repath_timestamp = now;
                ai_guard
                    .set_movement_target(&self.goal_position)
                    .map_err(|err| format!("AIInternalMoveToState repath failed: {}", err))?;
                self.path_goal_position = self.goal_position;
                self.path_timestamp = now;
            }
        } else {
            let mut set_condition_flag = ModelConditionFlags::MOVING;
            if is_cliff_at(owner_guard.get_position()) {
                let moving_backwards = ai_guard
                    .get_cur_locomotor()
                    .and_then(|loc| loc.lock().ok().map(|loco| loco.is_moving_backwards()))
                    .unwrap_or(false);
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
                ai_guard
                    .set_movement_target(&self.goal_position)
                    .map_err(|err| format!("AIInternalMoveToState repath failed: {}", err))?;
                self.path_goal_position = self.goal_position;
                self.path_timestamp = now;
            }
        }

        let close_enough = ai_guard
            .get_cur_locomotor()
            .and_then(|loc| loc.lock().ok().map(|loco| loco.get_close_enough_dist()))
            .unwrap_or(0.0);

        let dist_remaining = ai_guard.get_locomotor_distance_to_goal();
        if dist_remaining <= close_enough {
            return Ok(StateReturnType::Success);
        }

        Ok(StateReturnType::Continue)
    }

    /// Called when the state exits (successfully or otherwise).
    pub fn on_exit(&mut self, _status: StateExitType) -> Result<(), String> {
        if self.ambient_playing_handle != 0 {
            if let Some(audio) = TheAudio::get() {
                audio.remove_audio_event(self.ambient_playing_handle);
            }
            self.ambient_playing_handle = 0;
        }
        if let Ok(owner) = self.get_machine_owner() {
            if let Ok(mut owner_guard) = owner.write() {
                if let Some(ai) = owner_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.friend_ending_move();
                        if let Some(locomotor) = ai_guard.get_cur_locomotor() {
                            if let Ok(loco_guard) = locomotor.lock() {
                                if loco_guard.is_ultra_accurate()
                                    && !matches!(
                                        loco_guard.get_appearance(),
                                        LocomotorAppearance::Hover
                                            | LocomotorAppearance::Thrust
                                            | LocomotorAppearance::Wings
                                    )
                                {
                                    let dx = self.goal_position.x - owner_guard.get_position().x;
                                    let dy = self.goal_position.y - owner_guard.get_position().y;
                                    if dx * dx + dy * dy
                                        < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F
                                    {
                                        let _ = owner_guard.set_position(&self.goal_position);
                                    }
                                }
                            }
                        }
                        ai_guard.destroy_path();
                    }
                }
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            }
        }
        Ok(())
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
        let machine = self.upgrade_machine()?;
        let guard = machine
            .lock()
            .map_err(|_| "AIInternalMoveToState machine lock poisoned".to_string())?;
        Ok(guard.get_goal_position())
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

    pub fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        _xfer
            .xfer_version(&mut version, 1)
            .map_err(|e| format!("AIInternalMoveToState xfer version failed: {:?}", e))?;

        _xfer
            .xfer_real(&mut self.goal_position.x)
            .map_err(|e| format!("AIInternalMoveToState xfer goal_position.x failed: {:?}", e))?;
        _xfer
            .xfer_real(&mut self.goal_position.y)
            .map_err(|e| format!("AIInternalMoveToState xfer goal_position.y failed: {:?}", e))?;
        _xfer
            .xfer_real(&mut self.goal_position.z)
            .map_err(|e| format!("AIInternalMoveToState xfer goal_position.z failed: {:?}", e))?;

        let mut goal_layer_value = self.goal_layer as u32;
        _xfer
            .xfer_unsigned_int(&mut goal_layer_value)
            .map_err(|e| format!("AIInternalMoveToState xfer goal_layer failed: {:?}", e))?;
        if _xfer.get_xfer_mode() == XferMode::Load {
            self.goal_layer = PathfindLayerEnum::from_u32(goal_layer_value);
        }

        game_engine::system::Xfer::xfer_bool(_xfer, &mut self.waiting_for_path).map_err(|e| {
            format!(
                "AIInternalMoveToState xfer waiting_for_path failed: {:?}",
                e
            )
        })?;

        _xfer
            .xfer_real(&mut self.path_goal_position.x)
            .map_err(|e| {
                format!(
                    "AIInternalMoveToState xfer path_goal_position.x failed: {:?}",
                    e
                )
            })?;
        _xfer
            .xfer_real(&mut self.path_goal_position.y)
            .map_err(|e| {
                format!(
                    "AIInternalMoveToState xfer path_goal_position.y failed: {:?}",
                    e
                )
            })?;
        _xfer
            .xfer_real(&mut self.path_goal_position.z)
            .map_err(|e| {
                format!(
                    "AIInternalMoveToState xfer path_goal_position.z failed: {:?}",
                    e
                )
            })?;
        _xfer
            .xfer_unsigned_int(&mut self.path_timestamp)
            .map_err(|e| format!("AIInternalMoveToState xfer path_timestamp failed: {:?}", e))?;
        _xfer
            .xfer_unsigned_int(&mut self.blocked_repath_timestamp)
            .map_err(|e| {
                format!(
                    "AIInternalMoveToState xfer blocked_repath_timestamp failed: {:?}",
                    e
                )
            })?;
        game_engine::system::Xfer::xfer_bool(_xfer, &mut self.adjusts_destination).map_err(
            |e| {
                format!(
                    "AIInternalMoveToState xfer adjusts_destination failed: {:?}",
                    e
                )
            },
        )?;

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
        let machine = self.upgrade_machine()?;
        let guard = machine.lock().map_err(|_| {
            format!(
                "AIInternalMoveToState '{}' failed to lock machine",
                self.name
            )
        })?;
        Ok(guard.get_goal_object())
    }

    /// Access the machine owner object.
    pub fn get_machine_owner(&self) -> Result<Arc<RwLock<Object>>, String> {
        let machine = self.upgrade_machine()?;
        let guard = machine.lock().map_err(|_| {
            format!(
                "AIInternalMoveToState '{}' failed to lock machine",
                self.name
            )
        })?;
        guard
            .get_owner()
            .ok_or_else(|| "state machine owner not set".to_string())
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
        self.adjusts_destination
    }

    /// Configure whether the helper adjusts its destination dynamically.
    pub fn set_adjusts_destination(&mut self, adjust: bool) {
        self.adjusts_destination = adjust;
    }
}
