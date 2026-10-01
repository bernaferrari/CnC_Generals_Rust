use crate::object::registry::OBJECT_REGISTRY;

use crate::ai::the_ai;
use crate::common::{
    BodyDamageType, Coord3D, KindOf, LOGICFRAMES_PER_SECOND, ModelConditionFlags,
    ObjectStatusTypes, PathfindLayerEnum, UnsignedInt, Xfer, XferExt, XferMode, XferVersion,
};
use crate::helpers::{TheAudio, TheGameLogic};
use crate::locomotor::LocomotorAppearance;
use crate::object::Object;
use crate::path::PATHFIND_CELL_SIZE_F;
use crate::state_machine::{StateExitType, StateReturnType};
use crate::terrain::get_terrain_logic;

/// Internal move-to helper bridging legacy AI move states to the modern state machine.
///
/// Matches the core behavior of C++ AIInternalMoveToState (path request, block handling,
/// and goal completion checks), tailored to the current Rust locomotor/pathing pipeline.
#[derive(Debug)]
pub struct AIInternalMoveToState {
    name: String,
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
    /// Create a new move helper. Goal data is bound by the owning machine
    /// before each step; the helper keeps only per-instance move state.
    pub fn new(name: String) -> Self {
        Self {
            name,
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
        }
    }

    /// Hook invoked when the enclosing state machine enters the move helper.
    pub fn on_enter(&mut self) -> Result<StateReturnType, String> {
        let owner_id = self.get_machine_owner_id()?;
        let ai = OBJECT_REGISTRY
            .with_object_mut(owner_id, |owner_guard| {
                if owner_guard.test_status(ObjectStatusTypes::Immobile) {
                    return Err("immobile".to_string());
                }
                let ai = owner_guard.get_ai_update_interface().ok_or_else(|| {
                    "AIInternalMoveToState missing AIUpdateInterface".to_string()
                })?;
                owner_guard.set_model_condition_state(ModelConditionFlags::MOVING);
                if is_cliff_at(owner_guard.get_position()) {
                    owner_guard.set_model_condition_state(ModelConditionFlags::CLIMBING);
                    owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                }
                Ok(ai)
            })
            .ok_or_else(|| "state machine owner not set".to_string())?;
        let ai = match ai {
            Err(err) if err == "immobile" => return Ok(StateReturnType::Failure),
            other => other?,
        };
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIInternalMoveToState AI lock poisoned".to_string())?;

        ai_guard.with_cur_locomotor(&mut |loco| {
            if loco.is_ultra_accurate() {
                self.set_adjusts_destination(false);
            }
            // C++ AIInternalMoveToState::onEnter (AIStates.cpp:1604-1605).
            loco.start_move();
        });

        ai_guard.set_adjusts_destination(self.get_adjusts_destination());

        if let Ok(goal) = self.get_machine_goal_position() {
            self.goal_position = goal;
        }
        let goal_pos = if let Ok(Some(goal_id)) = self.get_machine_goal_object_id() {
            OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
                    let mut goal_pos = *goal_guard.get_position();
                    if owner_guard.is_kind_of(KindOf::Projectile) {
                        let half_height =
                            goal_guard.get_geometry_info().get_max_height_above_position() * 0.5;
                        goal_pos.z += half_height;
                        if goal_guard.get_position().z < goal_pos.z {
                            goal_pos.z += half_height;
                        }
                    }
                    goal_pos
                })
            })
            .flatten()
        } else {
            None
        };
        if let Some(goal_pos) = goal_pos {
            self.goal_position = goal_pos;
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
        drop(ai_guard);

        OBJECT_REGISTRY
            .with_object(owner_id, |owner_guard| {
                self.start_move_sound(owner_guard);
            })
            .ok_or_else(|| "state machine owner not set".to_string())?;
        Ok(StateReturnType::Continue)
    }

    /// Update hook – drives path recompute and completion checks.
    pub fn update(&mut self) -> Result<StateReturnType, String> {
        let owner_id = self.get_machine_owner_id()?;
        let ai = OBJECT_REGISTRY
            .with_object(owner_id, |owner_guard| owner_guard.get_ai_update_interface())
            .ok_or_else(|| "state machine owner not set".to_string())?
            .ok_or_else(|| "AIInternalMoveToState missing AIUpdateInterface".to_string())?;
        let mut moving_backwards = false;
        let mut close_enough = 0.0;
        {
            let ai_guard = ai
                .lock()
                .map_err(|_| "AIInternalMoveToState AI lock poisoned".to_string())?;
            ai_guard.with_cur_locomotor(&mut |loco| {
                moving_backwards = loco.is_moving_backwards();
                close_enough = loco.get_close_enough_dist();
            });
        }
        let mut ai_guard = ai
            .lock()
            .map_err(|_| "AIInternalMoveToState AI lock poisoned".to_string())?;
        let owner_snapshot = OBJECT_REGISTRY
            .with_object(owner_id, |owner_guard| {
                let owner_pos = *owner_guard.get_position();
                let projectile = owner_guard.is_kind_of(KindOf::Projectile);
                let cliff = is_cliff_at(owner_guard.get_position());
                (owner_pos, projectile, cliff)
            })
            .ok_or_else(|| "state machine owner not set".to_string())?;
        let (owner_pos, projectile, cliff) = owner_snapshot;

        if let Ok(Some(goal_id)) = self.get_machine_goal_object_id() {
            if let Some(new_goal) = OBJECT_REGISTRY.with_object(goal_id, |goal_guard| {
                let mut new_goal = *goal_guard.get_position();
                if projectile {
                    let half_height =
                        goal_guard.get_geometry_info().get_max_height_above_position() * 0.5;
                    new_goal.z += half_height;
                    if goal_guard.get_position().z < new_goal.z {
                        new_goal.z += half_height;
                    }
                }
                new_goal
            }) {
                self.goal_position = new_goal;
                if !self.is_same_position(&owner_pos, &self.path_goal_position, &new_goal) {
                    self.path_timestamp = 0;
                }
            }
        }

        let frames_blocked = ai_guard.get_num_frames_blocked();
        let blocked =
            ai_guard.is_blocked_and_stuck() || frames_blocked > 2 * LOGICFRAMES_PER_SECOND;
        let mut repath = false;
        OBJECT_REGISTRY
            .with_object_mut(owner_id, |owner_guard| {
                if blocked {
                    owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
                    owner_guard.clear_model_condition_state(ModelConditionFlags::CLIMBING);
                    owner_guard.clear_model_condition_state(ModelConditionFlags::RAPPELLING);
                } else {
                    let mut set_condition_flag = ModelConditionFlags::MOVING;
                    if cliff {
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
                            let clear_flag = if set_condition_flag == ModelConditionFlags::CLIMBING
                            {
                                ModelConditionFlags::RAPPELLING
                            } else {
                                ModelConditionFlags::CLIMBING
                            };
                            owner_guard.clear_model_condition_state(clear_flag);
                            owner_guard.set_model_condition_state(set_condition_flag);
                        }
                    }
                }
            })
            .ok_or_else(|| "state machine owner not set".to_string())?;
        if blocked {
            let now = TheGameLogic::get_frame();
            let repath_delay = LOGICFRAMES_PER_SECOND;
            if now.saturating_sub(self.blocked_repath_timestamp) >= repath_delay {
                self.blocked_repath_timestamp = now;
                repath = true;
                self.path_goal_position = self.goal_position;
                self.path_timestamp = now;
            }
        } else {
            let now = TheGameLogic::get_frame();
            if now.saturating_sub(self.path_timestamp) > MIN_REPATH_TIME
                && !self.is_same_position(&owner_pos, &self.path_goal_position, &self.goal_position)
            {
                repath = true;
                self.path_goal_position = self.goal_position;
                self.path_timestamp = now;
            }
        }
        if repath {
            ai_guard
                .set_movement_target(&self.goal_position)
                .map_err(|err| format!("AIInternalMoveToState repath failed: {}", err))?;
        }

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
        if let Ok(owner_id) = self.get_machine_owner_id() {
            let ai = OBJECT_REGISTRY
                .with_object(owner_id, |guard| guard.get_ai_update_interface())
                .flatten();
            if let Some(ai) = ai {
                if let Ok(mut ai_guard) = ai.lock() {
                    ai_guard.friend_ending_move();
                    let goal = self.goal_position;
                    let mut snap = false;
                    ai_guard.with_cur_locomotor(&mut |loco| {
                        snap = loco.is_ultra_accurate()
                            && !matches!(
                                loco.get_appearance(),
                                LocomotorAppearance::Hover
                                    | LocomotorAppearance::Thrust
                                    | LocomotorAppearance::Wings
                            );
                    });
                    if snap {
                        OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                            let dx = goal.x - owner_guard.get_position().x;
                            let dy = goal.y - owner_guard.get_position().y;
                            if dx * dx + dy * dy < PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F {
                                let _ = owner_guard.set_position(&goal);
                            }
                        });
                    }
                    ai_guard.destroy_path();
                }
            }
            OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                owner_guard.clear_model_condition_state(ModelConditionFlags::MOVING);
            });
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
    /// The machine goal stays authoritative: it rebinds before every step.
    pub fn set_goal_position(&mut self, pos: Coord3D) {
        self.goal_position = pos;
    }

    fn get_machine_goal_position(&self) -> Result<Coord3D, String> {
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
        if let Ok(owner_id) = self.get_machine_owner_id() {
            OBJECT_REGISTRY.with_object(owner_id, |owner_guard| {
                self.start_move_sound(owner_guard);
            });
        }
        Ok(())
    }

    /// Access the machine goal object id if present.
    pub fn get_machine_goal_object(&self) -> Result<Option<crate::common::ObjectID>, String> {
        self.get_machine_goal_object_id()
    }

    /// Access the machine owner id.
    pub fn get_machine_owner(&self) -> Result<crate::common::ObjectID, String> {
        self.get_machine_owner_id()
    }
    pub fn note_goal_object_id(&mut self, id: crate::common::ObjectID) {
        self.goal_object_id = id;
    }

    pub fn note_owner_id(&mut self, id: crate::common::ObjectID) {
        self.owner_id = id;
    }

    pub fn get_machine_goal_object_id(&self) -> Result<Option<crate::common::ObjectID>, String> {
        if self.goal_object_id == crate::common::INVALID_ID {
            Ok(None)
        } else {
            Ok(Some(self.goal_object_id))
        }
    }

    pub fn get_machine_owner_id(&self) -> Result<crate::common::ObjectID, String> {
        if self.owner_id == crate::common::INVALID_ID {
            Err("state machine owner not set".to_string())
        } else {
            Ok(self.owner_id)
        }
    }

    /// Whether the move helper adjusts its destination on the fly.
    pub fn get_adjusts_destination(&self) -> bool {
        if !self.adjusts_destination {
            return false;
        }
        if let Ok(owner_id) = self.get_machine_owner_id() {
            let blocked = OBJECT_REGISTRY.with_object(owner_id, |guard| {
                if guard.test_status(ObjectStatusTypes::Parachuting) {
                    return true;
                }
                if let Some(ai) = guard.get_ai_update_interface() {
                    if let Ok(ai_guard) = ai.lock() {
                        if !ai_guard.is_allowed_to_adjust_destination() {
                            return true;
                        }
                    }
                }
                false
            });
            if blocked == Some(true) {
                return false;
            }
        }
        self.adjusts_destination
    }

    /// Configure whether the helper adjusts its destination dynamically.
    pub fn set_adjusts_destination(&mut self, adjust: bool) {
        self.adjusts_destination = adjust;
    }
}
