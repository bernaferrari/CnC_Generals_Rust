// Script-engine types, RefCell inner state, and re-entrancy accessors.
//
// Included into `engine/mod.rs` so private fields and the current
// RefCell / TLS re-entrancy pattern stay in the parent module.

pub const MAX_COUNTERS: usize = 256;
pub const MAX_FLAGS: usize = 256;
pub const MAX_ATTACK_PRIORITIES: usize = 256;
const FRAMES_TO_FADE_IN_AT_START: i32 = 33;
const MAX_SEQUENTIAL_SPIN_COUNT: i32 = 20;

#[derive(Debug, Clone, Copy, PartialEq)]
enum ActionChainExecution {
    Completed,
    Pending(f32),
}

/// Host-side callbacks for script actions that require integration with the game loop.
pub trait ScriptActionHandler: Send + Sync {
    fn enable_script(&self, _name: &str, _enabled: bool) -> GameLogicResult<()> {
        Ok(())
    }

    fn display_text(&self, _text: &str) -> GameLogicResult<()> {
        Ok(())
    }

    fn display_cinematic_text(
        &self,
        _text: &str,
        _font_type: &str,
        _duration_seconds: i32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn set_border_shroud_level(&self, _level: u8) -> GameLogicResult<()> {
        Ok(())
    }

    fn military_caption(&self, _text: &str, _duration_ms: i32) -> GameLogicResult<()> {
        Ok(())
    }

    fn play_sound_effect(&self, _name: &str) -> GameLogicResult<()> {
        Ok(())
    }

    fn play_sound_effect_at(&self, _name: &str, _x: f32, _y: f32, _z: f32) -> GameLogicResult<()> {
        Ok(())
    }

    fn move_camera(&self, _x: f32, _y: f32, _z: f32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `TheTacticalView->moveCameraTo(dest, ms, shutter, orient, easeIn, easeOut)`.
    fn move_camera_to(
        &self,
        x: f32,
        y: f32,
        z: f32,
        _seconds: f32,
        _camera_stutter_seconds: f32,
        _ease_in_seconds: f32,
        _ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.move_camera(x, y, z)
    }

    fn move_camera_along_waypoint_path(
        &self,
        _waypoint_path: &str,
        _seconds: f32,
        _camera_stutter_seconds: f32,
        _ease_in_seconds: f32,
        _ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn move_camera_to_selection(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_move_home(&self) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `W3DView::isCameraMovementFinished` queried by `Condition::CAMERA_MOVEMENT_FINISHED`.
    fn is_camera_movement_finished(&self) -> bool {
        true
    }

    fn camera_follow_object(
        &self,
        _object_id: ObjectID,
        _snap_to_unit: bool,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_tether_object(
        &self,
        object_id: ObjectID,
        snap_to_unit: bool,
        _play: f32,
    ) -> GameLogicResult<()> {
        self.camera_follow_object(object_id, snap_to_unit)
    }

    fn stop_camera_follow(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn reset_camera_to(
        &self,
        _x: f32,
        _y: f32,
        _z: f32,
        _duration_seconds: f32,
        _ease_in_seconds: f32,
        _ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        Ok(())
    }


    fn zoom_camera(
        &self,
        zoom: f32,
        seconds: f32,
        _ease_in_seconds: f32,
        _ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.set_camera_zoom(zoom, seconds)
    }

    fn set_camera_zoom(&self, _zoom: f32, _duration_seconds: f32) -> GameLogicResult<()> {
        Ok(())
    }

    fn set_camera_pitch(
        &self,
        _pitch: f32,
        _seconds: f32,
        _ease_in_seconds: f32,
        _ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn rotate_camera(
        &self,
        _rotations: f32,
        _seconds: f32,
        _ease_in_seconds: f32,
        _ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_mod_set_final_zoom(
        &self,
        zoom: f32,
        _ease_in: f32,
        _ease_out: f32,
    ) -> GameLogicResult<()> {
        self.set_camera_zoom(zoom, 0.0)
    }

    fn camera_mod_set_final_pitch(
        &self,
        pitch: f32,
        _ease_in: f32,
        _ease_out: f32,
    ) -> GameLogicResult<()> {
        self.set_camera_pitch(pitch, 0.0, 0.0, 0.0)
    }

    fn camera_mod_freeze_time(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_mod_freeze_angle(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_mod_set_final_speed_multiplier(&self, _multiplier: i32) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_mod_set_rolling_average(&self, _frames: i32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors ScriptActions::doOversizeTheTerrain.
    fn oversize_terrain(&self, _amount: i32) -> GameLogicResult<()> {
        Ok(())
    }

    fn setup_camera(
        &self,
        x: f32,
        y: f32,
        z: f32,
        zoom: f32,
        pitch: f32,
        look_toward_x: f32,
        look_toward_y: f32,
        look_toward_z: f32,
    ) -> GameLogicResult<()> {
        self.move_camera(x, y, z)?;
        let _ = self.camera_look_toward_waypoint(
            look_toward_x,
            look_toward_y,
            look_toward_z,
            0.0,
            0.0,
            0.0,
            false,
        );
        self.camera_mod_set_final_pitch(pitch, 0.0, 0.0)?;
        self.camera_mod_set_final_zoom(zoom, 0.0, 0.0)
    }

    fn camera_look_toward_object(
        &self,
        _object_id: ObjectID,
        _seconds: f32,
        _hold_seconds: f32,
        _ease_in_seconds: f32,
        _ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_look_toward_waypoint(
        &self,
        _x: f32,
        _y: f32,
        _z: f32,
        _seconds: f32,
        _ease_in_seconds: f32,
        _ease_out_seconds: f32,
        _reverse_rotation: bool,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_mod_look_toward(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        self.camera_look_toward_waypoint(x, y, z, 0.0, 0.0, 0.0, false)
    }

    fn camera_mod_final_look_toward(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        self.camera_look_toward_waypoint(x, y, z, 0.0, 0.0, 0.0, false)
    }

    fn camera_letterbox_begin(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_letterbox_end(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_set_default(
        &self,
        _pitch: f32,
        _angle: f32,
        _max_height: f32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_enable_slave_mode(
        &self,
        _thing_template_name: &str,
        _bone_name: &str,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_disable_slave_mode(&self) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `TheTacticalView->shake(&pos, (View::CameraShakeType)intensity)`.
    fn screen_shake(&self, _intensity: i32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `TheTacticalView->Add_Camera_Shake(pos, radius, duration_seconds, amplitude)`.
    fn camera_add_shaker_at(
        &self,
        _x: f32,
        _y: f32,
        _z: f32,
        _amplitude: f32,
        _duration_seconds: f32,
        _radius: f32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn movie_play_fullscreen(&self, _filename: &str) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `TheInGameUI->playMovie(name)` used by ScriptActions::doMoviePlayRadar.
    fn movie_play_radar(&self, _filename: &str) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `ScriptEngine::isVideoComplete(name, flush)` used by `Condition::HAS_FINISHED_VIDEO`.
    fn is_video_complete(&self, _name: &str, _flush: bool) -> bool {
        false
    }

    fn speech_play(&self, _name: &str, _allow_overlap: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ `ScriptActions::doSoundPlayFromNamed` — play as though from a unit.
    fn sound_play_named(&self, _sound: &str, _unit_name: &str) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ `ScriptActions::doEnableObjectSound` / `DISABLE_OBJECT_SOUND`.
    fn enable_object_sound(&self, _unit_name: &str, _enable: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `ScriptEngine::isSpeechComplete(name, flush)` used by `Condition::HAS_FINISHED_SPEECH`.
    fn is_speech_complete(&self, _name: &str, _flush: bool) -> bool {
        false
    }

    /// Mirrors `ScriptEngine::isAudioComplete(name, flush)` used by `Condition::HAS_FINISHED_AUDIO`.
    fn is_audio_complete(&self, _name: &str, _flush: bool) -> bool {
        false
    }

    fn music_set_track(
        &self,
        _track: &str,
        _fade_out: bool,
        _fade_in: bool,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `TheAudio->hasMusicTrackCompleted(track, int)` used by `Condition::MUSIC_TRACK_HAS_COMPLETED`.
    fn has_music_track_completed(&self, _track: &str, _param: i32) -> bool {
        false
    }

    fn stop_music(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn freeze_time(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn unfreeze_time(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn set_visual_speed_multiplier(&self, _multiplier: i32) -> GameLogicResult<()> {
        Ok(())
    }

    fn set_fps_limit(&self, _fps: i32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `TheInGameUI->popupMessage(message, x, y, width, pause, pauseMusic)`.
    fn popup_message(
        &self,
        _message: &str,
        _x_percent: i32,
        _y_percent: i32,
        _width: i32,
        _pause: bool,
        _pause_music: bool,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors `TheTacticalView->setGuardBandBias(&Coord2D{gbx,gby})`.
    fn resize_view_guardband(&self, _gbx: f32, _gby: f32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors ScriptActions::doBlackWhiteMode.
    fn set_camera_bw_mode(&self, _enabled: bool, _frames: i32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors legacy `W3DView::set3DWireFrameMode`.
    fn set_3d_wireframe_mode(&self, _enabled: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors ScriptActions::doSkyBox.
    fn set_skybox_enabled(&self, _enabled: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors ScriptActions::doWeather.
    fn set_weather_visible(&self, _visible: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors ScriptActions::doCameraMotionBlur.
    fn camera_motion_blur(&self, _zoom_in: bool, _saturate: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors ScriptActions::doCameraMotionBlurJump.
    fn camera_motion_blur_jump(
        &self,
        x: f32,
        y: f32,
        z: f32,
        _saturate: bool,
    ) -> GameLogicResult<()> {
        self.move_camera_to(x, y, z, 0.0, 0.0, 0.0, 0.0)
    }

    /// Mirrors ScriptActions CAMERA_MOTION_BLUR_FOLLOW.
    fn camera_motion_blur_follow(&self, _amount: i32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors ScriptActions CAMERA_MOTION_BLUR_END_FOLLOW.
    fn camera_motion_blur_end_follow(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn set_radar_enabled(&self, _enabled: bool) -> GameLogicResult<()> {
        Ok(())
    }

    fn set_radar_forced(&self, _forced: bool) -> GameLogicResult<()> {
        Ok(())
    }

    fn create_radar_event(
        &self,
        _x: f32,
        _y: f32,
        _z: f32,
        _event_type: i32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn set_objective(
        &self,
        _name: &str,
        _description: &str,
        _completed: bool,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    fn spawn_effect(&self, _effect_type: &str, _x: f32, _y: f32, _z: f32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors ScriptActions::doCameoFlash -> ControlBar cameo flash count.
    fn cameo_flash(&self, _command_button_name: &str, _flash_count: i32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors InGameUI::addNamedTimer(name, text, countdown)
    fn add_named_timer(&self, _name: &str, _text: &str, _countdown: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors InGameUI::removeNamedTimer(name)
    fn remove_named_timer(&self, _name: &str) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors InGameUI::showNamedTimerDisplay(show)
    fn show_named_timer_display(&self, _show: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors InGameUI::setSuperweaponDisplayEnabledByScript(enabled)
    fn set_superweapon_display_enabled_by_script(&self, _enabled: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors InGameUI::hideObjectSuperweaponDisplayByScript(object)
    fn hide_object_superweapon_display_by_script(
        &self,
        _object_id: ObjectID,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors InGameUI::showObjectSuperweaponDisplayByScript(object)
    fn show_object_superweapon_display_by_script(
        &self,
        _object_id: ObjectID,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ ScriptActions.cpp:4066 doNamedStop/StartSpecialPowerCountdown.
    fn pause_named_special_power_countdown(
        &self,
        _unit_name: &str,
        _power_name: &str,
        _pause: bool,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ ScriptActions.cpp:4085 doNamedSetSpecialPowerCountdown.
    fn set_named_special_power_countdown(
        &self,
        _unit_name: &str,
        _power_name: &str,
        _seconds: i32,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ ScriptActions.cpp:4103 doNamedAddSpecialPowerCountdown.
    fn add_named_special_power_countdown(
        &self,
        _unit_name: &str,
        _power_name: &str,
        _seconds: i32,
    ) -> GameLogicResult<()> {
        Ok(())
    }


    /// C++ ScriptActions.cpp:174/208/232/250 TheCampaignManager->SetVictorious.
    fn set_campaign_victorious(&self, _victorious: bool) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ ScriptActions.cpp:193-247 TheWindowManager->winCreateFromScript.
    fn create_win_lose_window(&self, _layout_filename: &str) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ ScriptActions.cpp:156-163 TheWindowManager->winDestroy(m_messageWindow).
    fn destroy_win_lose_window(&self) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ GameLogic::closeWindows GameLogicDispatch.cpp:202-219.
    fn close_game_windows(&self) -> GameLogicResult<()> {
        Ok(())
    }

    /// C++ ScriptActions.cpp:5079 doSetWarehouseValue → setCashValue.
    fn set_warehouse_value(&self, _warehouse_name: &str, _cash_value: i32) -> GameLogicResult<()> {
        Ok(())
    }
}

/// C++ Scripts.h skirmish trigger names.
pub(crate) const MY_INNER_PERIMETER: &str = "[Skirmish]MyInnerPerimeter";
pub(crate) const MY_OUTER_PERIMETER: &str = "[Skirmish]MyOuterPerimeter";
pub(crate) const ENEMY_INNER_PERIMETER: &str = "[Skirmish]EnemyInnerPerimeter";
pub(crate) const ENEMY_OUTER_PERIMETER: &str = "[Skirmish]EnemyOuterPerimeter";
pub(crate) const INNER_PERIMETER: &str = "InnerPerimeter";
pub(crate) const OUTER_PERIMETER: &str = "OuterPerimeter";

/// C++ ScriptEngine::getQualifiedTriggerAreaByName rewrite (ScriptEngine.cpp:5888-5916).
pub(crate) fn qualify_trigger_area_name(
    area_name: &str,
    current_player_name: Option<&str>,
) -> Option<String> {
    if area_name == MY_INNER_PERIMETER || area_name == MY_OUTER_PERIMETER {
        let player_name = current_player_name?;
        let ndx = crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.find_player_by_name(player_name))
            .and_then(|player| player.read().ok().map(|guard| guard.get_mp_start_index() + 1))?;
        if area_name == MY_INNER_PERIMETER {
            return Some(format!("{INNER_PERIMETER}{ndx}"));
        }
        return Some(format!("{OUTER_PERIMETER}{ndx}"));
    }

    if area_name == ENEMY_INNER_PERIMETER || area_name == ENEMY_OUTER_PERIMETER {
        let mut mp_ndx = -1;
        if let Some(player_name) = current_player_name {
            if let Ok(list) = crate::player::player_list().read() {
                if let Some(player) = list.find_player_by_name(player_name) {
                    if let Ok(guard) = player.read() {
                        if let Some(enemy_index) = guard.get_current_enemy_player_index() {
                            if let Some(enemy) = list.get_player(enemy_index) {
                                if let Ok(enemy_guard) = enemy.read() {
                                    mp_ndx = enemy_guard.get_mp_start_index() + 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        if area_name == ENEMY_INNER_PERIMETER {
            return Some(format!("{INNER_PERIMETER}{mp_ndx}"));
        }
        return Some(format!("{OUTER_PERIMETER}{mp_ndx}"));
    }

    Some(area_name.to_string())
}


/// Fade types matching C++ TFade enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TFade {
    None = 0,
    Subtract = 1,
    Add = 2,
    Saturate = 3,
    Multiply = 4,
}

/// Breeze info structure matching C++ BreezeInfo
#[derive(Debug, Clone)]
pub struct BreezeInfo {
    pub direction: f32,          // Direction in radians, 0 == +x direction
    pub direction_vec: [f32; 2], // sin/cos of direction for efficiency
    pub intensity: f32,          // How far to sway back & forth in radians
    pub lean: f32,               // How far to lean with the wind in radians
    pub randomness: f32,         // Randomness 0=perfectly uniform, 1 = +- up to 50%
    pub breeze_period: i16,      // Frames to sway forward & back
    pub breeze_version: i16,     // Incremented when settings updated
}

impl BreezeInfo {
    /// C++ `ScriptEngine` reset defaults (`ScriptEngine.cpp` ~5291-5298).
    pub fn new() -> Self {
        let direction = std::f32::consts::PI / 3.0;
        let amplitude = 0.07 * std::f32::consts::PI / 4.0;
        Self {
            direction,
            direction_vec: [direction.sin(), direction.cos()],
            intensity: amplitude,
            lean: amplitude,
            randomness: 0.2,
            breeze_period: (crate::common::LOGICFRAMES_PER_SECOND * 5) as i16,
            breeze_version: 0,
        }
    }
}

/// Named reveal structure matching C++ NamedReveal
#[derive(Debug, Clone)]
pub struct NamedReveal {
    pub reveal_name: String,
    pub waypoint_name: String,
    pub radius_to_reveal: f32,
    pub player_name: String,
}

/// Counter structure matching C++ TCounter
#[derive(Debug, Clone)]
pub struct TCounter {
    pub value: i32,
    pub name: String,
    pub is_countdown_timer: bool,
}

impl TCounter {
    pub fn new(name: String) -> Self {
        Self {
            value: 0,
            name,
            is_countdown_timer: false,
        }
    }
}

/// Flag structure matching C++ TFlag
#[derive(Debug, Clone)]
pub struct TFlag {
    pub value: bool,
    pub name: String,
}

impl TFlag {
    pub fn new(name: String) -> Self {
        Self { value: false, name }
    }
}

/// Attack Priority Info matching C++ AttackPriorityInfo
#[derive(Debug, Clone)]
pub struct AttackPriorityInfo {
    pub name: String,
    pub default_priority: i32,
    pub priority_map: HashMap<String, i32>, // ThingTemplate name -> priority
}

impl AttackPriorityInfo {
    pub fn new() -> Self {
        Self {
            name: String::new(),
            default_priority: 1,
            priority_map: HashMap::new(),
        }
    }

    pub fn set_priority(&mut self, thing_template: &str, priority: i32) {
        self.priority_map
            .insert(thing_template.to_string(), priority);
    }

    pub fn get_priority(&self, thing_template: &str) -> i32 {
        self.priority_map
            .get(thing_template)
            .copied()
            .unwrap_or(self.default_priority)
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }
}

impl XferSnapshot for AttackPriorityInfo {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        xfer.xfer_ascii_string(&mut self.name)?;
        xfer.xfer_int(&mut self.default_priority)?;

        let mut priority_map_count: u16 =
            if xfer.get_xfer_mode() == game_engine::system::XferMode::Save {
                self.priority_map.len().min(u16::MAX as usize) as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut priority_map_count)?;

        if xfer.get_xfer_mode() == game_engine::system::XferMode::Save {
            let mut count_written: u16 = 0;
            for (template_name, priority) in self.priority_map.iter() {
                count_written = count_written.saturating_add(1);
                let mut name = template_name.clone();
                xfer.xfer_ascii_string(&mut name)?;
                let mut value = *priority;
                xfer.xfer_int(&mut value)?;
            }
            if count_written != priority_map_count {
                return Err(XferStatus::InvalidData);
            }
        } else {
            self.priority_map.clear();
            for _ in 0..priority_map_count {
                let mut name = String::new();
                xfer.xfer_ascii_string(&mut name)?;
                if TheThingFactory::find_template(name.as_str()).is_none() {
                    return Err(XferStatus::InvalidData);
                }
                let mut priority = 0;
                xfer.xfer_int(&mut priority)?;
                self.priority_map.insert(name, priority);
            }
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

/// Sequential Script matching C++ SequentialScript
#[derive(Debug, Clone)]
pub struct SequentialScript {
    pub team_to_exec_on: Option<String>, // Team name instead of pointer
    pub object_id: u32,
    pub script_to_execute_sequentially: Option<Box<Script>>,
    pub current_instruction: i32, // Which action currently executing
    pub times_to_loop: i32,       // 0 = once, >0 = loop till 0, <0 = infinite
    pub frames_to_wait: i32,      // 0 = next instruction, >0 = countdown
    pub dont_advance_instruction: bool, // Set by instruction requesting wait
    pub next_script_in_sequence: Option<Box<SequentialScript>>,
    /// Process-local identity for safe post-dispatch reconciliation.
    ///
    /// C++ keeps a stable `SequentialScript *` while an action runs. Rust must
    /// not keep a borrow into `Vec<SequentialScript>` across that immediate
    /// action, because the action can re-enter the engine. This token lets the
    /// evaluator find the same logical node after it releases the borrow. It
    /// is deliberately not serialized: save/load rebuilds runtime ownership.
    runtime_token: u64,
}

impl SequentialScript {
    pub fn new() -> Self {
        Self {
            team_to_exec_on: None,
            object_id: 0,
            script_to_execute_sequentially: None,
            current_instruction: -1, // START_INSTRUCTION
            times_to_loop: 0,
            frames_to_wait: -1,
            dont_advance_instruction: false,
            next_script_in_sequence: None,
            runtime_token: 0,
        }
    }
}

/// C++ `SequentialScript::xfer` payload (ScriptEngine.cpp:8127-8198).
/// Heads only: `m_nextScriptInSequence` is not serialized.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SequentialScriptSnapshot {
    pub team_id: TeamID,
    pub object_id: ObjectID,
    pub script_name: String,
    pub current_instruction: i32,
    pub times_to_loop: i32,
    pub frames_to_wait: i32,
    pub dont_advance_instruction: bool,
}


impl XferSnapshot for SequentialScript {
    fn crc(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        self.xfer(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut team_id: TeamID = TEAM_ID_INVALID;
        if xfer.get_xfer_mode() == game_engine::system::XferMode::Save {
            if let Some(team_name) = self.team_to_exec_on.as_deref() {
                if let Ok(mut factory) = TheTeamFactory().lock() {
                    if let Some(team) = factory.find_team(team_name) {
                        if let Ok(team_guard) = team.read() {
                            team_id = team_guard.get_id();
                        }
                    }
                }
            }
        }
        // SAFETY: `team_id` is an initialized stack `TeamID`; `xfer_user`
        // moves exactly `size_of::<TeamID>()` bytes within this call and
        // never retains the pointer.
        unsafe {
            xfer.xfer_user(
                &mut team_id as *mut TeamID as *mut u8,
                std::mem::size_of::<TeamID>(),
            )?
        };
        if xfer.get_xfer_mode() == game_engine::system::XferMode::Load {
            if team_id == TEAM_ID_INVALID {
                self.team_to_exec_on = None;
            } else if let Ok(factory) = TheTeamFactory().lock() {
                if let Some(team) = factory.find_team_by_id(team_id) {
                    if let Ok(team_guard) = team.read() {
                        self.team_to_exec_on = Some(team_guard.get_name().to_string());
                    } else {
                        return Err(XferStatus::InvalidData);
                    }
                } else {
                    return Err(XferStatus::InvalidData);
                }
            }
        }

        let mut object_id = self.object_id;
        xfer.xfer_object_id(&mut object_id)?;
        self.object_id = object_id;

        let mut script_name = String::new();
        if xfer.get_xfer_mode() == game_engine::system::XferMode::Save {
            if let Some(script) = self.script_to_execute_sequentially.as_ref() {
                script_name = script.script_name.clone();
            }
        }
        xfer.xfer_ascii_string(&mut script_name)?;
        if xfer.get_xfer_mode() == game_engine::system::XferMode::Load {
            if script_name.is_empty() {
                self.script_to_execute_sequentially = None;
            } else if let Ok(engine_lock) = get_script_engine().read() {
                if let Some(engine) = engine_lock.as_ref() {
                    if let Some(found) = engine.find_script_clone_by_name(&script_name) {
                        self.script_to_execute_sequentially = Some(Box::new(found));
                    } else {
                        return Err(XferStatus::InvalidData);
                    }
                }
            }
        }

        xfer.xfer_int(&mut self.current_instruction)?;
        xfer.xfer_int(&mut self.times_to_loop)?;
        xfer.xfer_int(&mut self.frames_to_wait)?;
        xfer.xfer_bool(&mut self.dont_advance_instruction)?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

/// Script execution statistics
#[derive(Debug, Clone, Default)]
pub struct ScriptStats {
    pub num_frames: f64,
    pub total_update_time: f64,
    pub max_update_time: f64,
    pub cur_update_time: f64,
}

/// Main Script Engine matching C++ ScriptEngine.
///
/// Mutable state lives in `RefCell<ScriptEngineInner>` so nested
/// `CALL_SUBROUTINE` (C++ runs the callee immediately) can re-enter through
/// `&ScriptEngine` + `lock_inner_mut` without minting a second `&mut ScriptEngine`.
/// `RefCell` enforces exclusive/shared borrows; `with_inner` cannot alias a
/// live `lock_inner_mut` (that used to be a comment-only rule over `UnsafeCell`).
pub struct ScriptEngine {
    /// Stable identity for lexical active-list ownership.  Unlike the old TLS
    /// raw pointer this is never dereferenced; it only prevents a nested,
    /// different `ScriptEngine` from borrowing the outer engine's lists.
    instance_id: u64,
    inner: std::cell::RefCell<ScriptEngineInner>,
}

/// Mutable script-engine body. Field layout matches the previous `ScriptEngine`.
///
/// Accessed via:
/// - `Deref` / `DerefMut` when the caller has exclusive `&mut ScriptEngine`
/// - `lock_inner_mut` when only `&ScriptEngine` is live (TLS nested path)
pub struct ScriptEngineInner {
    // Template registrations
    action_templates: Vec<ActionTemplate>,
    condition_templates: Vec<ConditionTemplate>,

    // Runtime state
    pub(crate) counters: Vec<Option<TCounter>>,
    pub(crate) num_counters: usize,
    flags: Vec<Option<TFlag>>,
    num_flags: usize,
    attack_priority_info: Vec<AttackPriorityInfo>,
    num_attack_info: usize,

    // Game state
    end_game_timer: i32,
    close_window_timer: i32,
    calling_team: Option<String>,   // Team name instead of pointer
    calling_object: Option<u32>,    // Object ID instead of pointer
    condition_team: Option<String>, // Team name instead of pointer
    condition_object: Option<u32>,  // Object ID instead of pointer
    first_update: bool,
    current_player: Option<String>, // Player name instead of pointer
    skirmish_human_player: Option<String>, // Player name instead of pointer
    current_track_name: String,

    // Fade state
    fade: TFade,
    min_fade: f32,
    max_fade: f32,
    cur_fade_value: f32,
    cur_fade_frame: i32,
    fade_frames_increase: i32,
    fade_frames_hold: i32,
    fade_frames_decrease: i32,

    // Object tracking
    frame_object_count_changed: u32,
    object_counts: HashMap<(i32, String), i32>,
    object_types: HashMap<String, ObjectTypes>,
    object_attack_priority_sets: HashMap<ObjectID, String>,

    // Event tracking
    completed_video: Vec<String>,
    testing_speech: Vec<(String, u32)>,
    testing_audio: Vec<(String, u32)>,
    ui_interactions: Vec<String>,

    // Special power tracking per player
    triggered_special_powers: Vec<Vec<(String, u32)>>,
    midway_special_powers: Vec<Vec<(String, u32)>>,
    finished_special_powers: Vec<Vec<(String, u32)>>,
    completed_upgrades: Vec<Vec<(String, u32)>>,
    acquired_sciences: Vec<Vec<ScienceType>>,

    // Named reveals and effects
    topple_directions: Vec<(String, Coord3D)>,
    named_reveals: Vec<NamedReveal>,
    breeze_info: BreezeInfo,
    game_difficulty: crate::player::GameDifficulty,

    // System state
    campaign_victorious: bool,

    freeze_by_script: bool,
    freeze_by_debug: bool,
    objects_should_receive_difficulty_bonus: bool,
    choose_victim_always_uses_normal: bool,
    shown_mp_local_defeat_window: bool,
    /// C++ ScriptActions.cpp m_suppressNewWindows / m_messageWindow.
    suppress_new_windows: bool,
    win_lose_window_layout: Option<String>,

    // Sequential scripts
    sequential_scripts: Vec<SequentialScript>,
    /// Next opaque identity for a live sequential-script node. Zero is
    /// reserved for snapshots/direct test construction that has not yet been
    /// admitted to the runtime list.
    next_sequential_runtime_token: u64,

    // Script lists to execute (per player "side")
    side_script_lists: Vec<Option<Box<ScriptList>>>,

    // Statistics
    #[cfg(feature = "script_profiling")]
    stats: ScriptStats,

    action_handler: Option<Arc<dyn ScriptActionHandler>>,
}

/// RAII exclusive borrow of `ScriptEngineInner` from a shared `&ScriptEngine`.
///
/// Dropping the `RefMut` ends the exclusive borrow so nested `lock_inner_mut`
/// can run after the outer section finishes.
struct InnerMutGuard<'a> {
    inner: std::cell::RefMut<'a, ScriptEngineInner>,
}

impl Deref for InnerMutGuard<'_> {
    type Target = ScriptEngineInner;
    fn deref(&self) -> &ScriptEngineInner {
        &self.inner
    }
}

impl DerefMut for InnerMutGuard<'_> {
    fn deref_mut(&mut self) -> &mut ScriptEngineInner {
        &mut self.inner
    }
}

impl ScriptEngine {
    /// Scoped shared read of inner state. The `&ScriptEngineInner` cannot escape `f`.
    ///
    /// Panics if an `InnerMutGuard` is live (`RefCell` already borrowed).
    pub fn with_inner<R>(&self, f: impl FnOnce(&ScriptEngineInner) -> R) -> R {
        f(&self.inner.borrow())
    }

    /// Exclusive inner access from `&mut ScriptEngine`. Safe: no other alias exists.
    pub fn with_inner_mut<R>(&mut self, f: impl FnOnce(&mut ScriptEngineInner) -> R) -> R {
        f(self.inner.get_mut())
    }

    /// Exclusive inner borrow from `&ScriptEngine` (nested CALL_SUBROUTINE path).
    ///
    /// Panics if another `InnerMutGuard` or `with_inner` borrow is live.
    /// Callers must drop the guard before `dispatcher.execute_action` / nested
    /// `with_script_engine_mut`, matching C++ immediate nested execution.
    fn lock_inner_mut(&self) -> InnerMutGuard<'_> {
        InnerMutGuard {
            inner: self.inner.borrow_mut(),
        }
    }

    /// Allocate an opaque identity for one live sequential-script node.
    ///
    /// This is intentionally separate from snapshot state: it exists only to
    /// recover the same C++-style node after an immediate, re-entrant action
    /// has run with all `RefCell` guards released.
    fn allocate_sequential_runtime_token(inner: &mut ScriptEngineInner) -> u64 {
        let token = inner.next_sequential_runtime_token;
        inner.next_sequential_runtime_token = inner.next_sequential_runtime_token.wrapping_add(1);
        if inner.next_sequential_runtime_token == 0 {
            inner.next_sequential_runtime_token = 1;
        }
        token
    }

    fn ensure_sequential_runtime_token_at(
        inner: &mut ScriptEngineInner,
        index: usize,
    ) -> Option<u64> {
        let needs_token = inner
            .sequential_scripts
            .get(index)
            .map(|script| script.runtime_token == 0)?;
        if needs_token {
            let token = Self::allocate_sequential_runtime_token(inner);
            inner.sequential_scripts[index].runtime_token = token;
        }
        Some(inner.sequential_scripts[index].runtime_token)
    }
}

/// Exclusive handle to the process-global script engine.
///
/// `read`/`write` are both exclusive (`Mutex`) so `ScriptEngine` does not
/// need to be `Sync`. Call sites keep `.read()` / `.write()`.
#[derive(Clone)]
pub struct ScriptEngineHandle {
    inner: Arc<Mutex<Option<ScriptEngine>>>,
}

impl ScriptEngineHandle {
    pub fn from_engine(engine: ScriptEngine) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Some(engine))),
        }
    }

    pub fn write(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, Option<ScriptEngine>>> {
        self.inner.lock()
    }

    pub fn read(&self) -> std::sync::LockResult<std::sync::MutexGuard<'_, Option<ScriptEngine>>> {
        self.inner.lock()
    }

    pub fn try_write(
        &self,
    ) -> std::sync::TryLockResult<std::sync::MutexGuard<'_, Option<ScriptEngine>>> {
        self.inner.try_lock()
    }

    pub fn try_read(
        &self,
    ) -> std::sync::TryLockResult<std::sync::MutexGuard<'_, Option<ScriptEngine>>> {
        self.inner.try_lock()
    }
}

// Static instances (in real implementation these would be proper singletons)
lazy_static::lazy_static! {
    static ref SCRIPT_ENGINE: ScriptEngineHandle = ScriptEngineHandle {
        inner: Arc::new(Mutex::new(None)),
    };
    static ref EVENT_MANAGER: Arc<EventManager> = Arc::new(EventManager::new());
    static ref NAMED_OBJECT_TRACKER: Arc<NamedObjectTracker> = Arc::new(NamedObjectTracker::new());
    static ref AREA_TRACKER: Arc<AreaTracker> = Arc::new(AreaTracker::new());
    static ref VTUNE_ENABLED_STATE: RwLock<bool> = RwLock::new(false);
    static ref SKATE_DISTANCE_OVERRIDE_STATE: RwLock<f32> = RwLock::new(0.0);
}

// Re-entrancy for nested CALL_SUBROUTINE / timer / flag mutations.
// std::sync::RwLock is not re-entrant: holding write while nested code
// calls get_script_engine().write()/read() deadlocks the host thread.
// Campaign maps (MD_USA01) hang on frame-0 "SUB-Generate Random Number"
// which CALL_SUBROUTINEs while the outer execute still holds the lock.
//
// Scoped TLS carries a checked, lexical `&ScriptEngine` through nested script
// execution.  This is intentionally not a raw pointer: `scoped_tls::set`
// restores the prior value during normal return and unwinding, while nested
// sets restore the outer engine.  C++ `callSubroutine` → `executeScript` runs
// immediately; we retain that order rather than queueing work.
scoped_tls::scoped_thread_local!(static ACTIVE_SCRIPT_ENGINE: ScriptEngine);

/// A location in the C++ `ScriptList` linked-list layout.  Keeping the
/// location rather than a pointer lets a script be removed temporarily while
/// its action chain is dispatched, then reinserted at exactly the same spot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ScriptLocation {
    side_index: usize,
    container: ScriptContainer,
    script_index: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScriptContainer {
    Root,
    Group(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GroupLocation {
    side_index: usize,
    group_index: usize,
}

/// Metadata retained while a Script is detached for immediate execution.
/// The script itself remains owned by the executing stack frame, so this
/// record never manufactures a second mutable reference to it.
#[derive(Debug)]
struct ExecutingScript {
    location: ScriptLocation,
    name: String,
    is_subroutine: bool,
}

/// Lexically owned copy of every side's ScriptList during script execution.
///
/// C++ searches all side lists while an action is running.  Moving every list
/// into this scoped store keeps that lookup visible without holding a
/// `RefCell` borrow across `ScriptActionDispatcher::execute_action`.
#[derive(Debug)]
struct ActiveScriptLists {
    owner_id: u64,
    lists: Vec<Option<Box<ScriptList>>>,
    executing_scripts: Vec<ExecutingScript>,
    executing_groups: Vec<GroupLocation>,
}

/// Result of an exact C++ `findGroup` then `findScript` lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubroutineLookup {
    Group {
        location: GroupLocation,
        is_subroutine: bool,
        is_active: bool,
    },
    Script {
        location: ScriptLocation,
        is_subroutine: bool,
    },
    /// The matching script is currently executing in this immediate call
    /// stack.  It is deliberately distinguished from `Missing`: C++ would
    /// recurse through the same object, which cannot be represented with two
    /// safe mutable borrows of this linked-list node.
    ReentrantScript {
        location: ScriptLocation,
        is_subroutine: bool,
    },
    Missing,
}

// The active list store is lexical just like the active engine. Nested calls
// for the same engine share this store; a different engine installs its own
// nested store and restores the outer one automatically.
scoped_tls::scoped_thread_local!(static ACTIVE_SCRIPT_LISTS: std::cell::RefCell<ActiveScriptLists>);

/// Restores all side-list ownership even when a script action unwinds.  The
/// invariants are deliberately narrow: all list borrows and detached-script
/// guards must be dropped before this guard, and neither may span dispatch.
struct ActiveScriptListsRestore<'a> {
    engine: &'a ScriptEngine,
    active_lists: &'a std::cell::RefCell<ActiveScriptLists>,
}

/// Temporarily owns one executing Script.  Dropping it reinstalls the node at
/// the original linked-list position before its enclosing list store restores
/// to `ScriptEngineInner`.
struct DetachedScript<'a> {
    active_lists: &'a std::cell::RefCell<ActiveScriptLists>,
    location: ScriptLocation,
    script: Option<Box<Script>>,
}

impl<'a> DetachedScript<'a> {
    fn take(
        active_lists: &'a std::cell::RefCell<ActiveScriptLists>,
        location: ScriptLocation,
    ) -> Option<Self> {
        let mut active_lists_mut = active_lists.borrow_mut();
        let script = ScriptEngine::take_script_at_location(&mut active_lists_mut, location)?;
        let executing = ExecutingScript {
            location,
            name: script.script_name.clone(),
            is_subroutine: script.is_subroutine,
        };
        active_lists_mut.executing_scripts.push(executing);
        drop(active_lists_mut);
        Some(Self {
            active_lists,
            location,
            script: Some(script),
        })
    }

    fn script_mut(&mut self) -> &mut Script {
        self.script
            .as_deref_mut()
            .expect("DetachedScript always owns its Script until Drop")
    }
}

impl Drop for DetachedScript<'_> {
    fn drop(&mut self) {
        let Some(script) = self.script.take() else {
            return;
        };
        let mut active_lists = self.active_lists.borrow_mut();
        if !ScriptEngine::restore_script_at_location(&mut active_lists, self.location, script) {
            // Replacing/clearing a side list while one of its scripts is on the
            // native-style execution stack is invalid in C++ as well.  Keep it
            // loud rather than pretending the script was never present.
            log::error!(
                "lost detached Script at side {}, location {:?} while restoring immediate execution",
                self.location.side_index,
                self.location
            );
        }
        active_lists
            .executing_scripts
            .retain(|executing| executing.location != self.location);
    }
}

/// Marks a subroutine group as executing without borrowing the group over an
/// action dispatch.  A nested call to that exact group is reported explicitly
/// instead of being accidentally invisible.
struct ExecutingGroupGuard<'a> {
    active_lists: &'a std::cell::RefCell<ActiveScriptLists>,
    location: GroupLocation,
}

impl Drop for ExecutingGroupGuard<'_> {
    fn drop(&mut self) {
        self.active_lists
            .borrow_mut()
            .executing_groups
            .retain(|location| *location != self.location);
    }
}

impl Drop for ActiveScriptListsRestore<'_> {
    fn drop(&mut self) {
        let lists = std::mem::take(&mut self.active_lists.borrow_mut().lists);
        self.engine.lock_inner_mut().side_script_lists = lists;
    }
}

static NEXT_SCRIPT_ENGINE_INSTANCE_ID: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

thread_local! {
    /// Depth of nested CALL_SUBROUTINE / execute_subroutine_by_name on this thread.
    /// Campaign random-number scripts can re-enter; unbounded nesting hangs the host.
    static SUBROUTINE_CALL_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Hard cap on nested CALL_SUBROUTINE depth (C++ is stack-bound; we fail closed).
const MAX_SUBROUTINE_CALL_DEPTH: u32 = 32;

struct SubroutineDepthGuard;

impl SubroutineDepthGuard {
    fn enter() -> Option<Self> {
        SUBROUTINE_CALL_DEPTH.with(|depth| {
            let current = depth.get();
            if current >= MAX_SUBROUTINE_CALL_DEPTH {
                log::warn!(
                    "CALL_SUBROUTINE depth limit ({}) exceeded; aborting nested call",
                    MAX_SUBROUTINE_CALL_DEPTH
                );
                None
            } else {
                depth.set(current + 1);
                Some(SubroutineDepthGuard)
            }
        })
    }
}

impl Drop for SubroutineDepthGuard {
    fn drop(&mut self) {
        SUBROUTINE_CALL_DEPTH.with(|depth| {
            depth.set(depth.get().saturating_sub(1));
        });
    }
}

impl ScriptEngine {
    /// Run `f` with this engine installed as the current lexical nested target.
    /// `scoped_tls` restores an outer active engine even if `f` panics.
    fn with_active<R>(&self, f: impl FnOnce() -> R) -> R {
        ACTIVE_SCRIPT_ENGINE.set(self, f)
    }

    /// Run a short, non-dispatching operation against this engine's lexical
    /// side-list store, if one is active.  The closure must return an owned
    /// result; its `RefCell` borrow never crosses an action dispatch.
    fn with_current_active_script_lists<R>(
        &self,
        f: impl FnOnce(&mut ActiveScriptLists) -> R,
    ) -> Option<R> {
        if !ACTIVE_SCRIPT_LISTS.is_set() {
            return None;
        }
        ACTIVE_SCRIPT_LISTS.with(|active_lists| {
            let mut active_lists = active_lists.borrow_mut();
            (active_lists.owner_id == self.instance_id).then(|| f(&mut active_lists))
        })
    }

    /// Move all side lists into lexical storage for an immediate action stack.
    /// Nested calls on this engine reuse the existing store, so C++ global
    /// `findGroup` / `findScript` ordering remains observable at every depth.
    fn with_active_script_lists<R>(&self, f: impl FnOnce() -> R) -> R {
        if self
            .with_current_active_script_lists(|_active_lists| ())
            .is_some()
        {
            return f();
        }

        let lists = {
            let mut inner = self.lock_inner_mut();
            std::mem::take(&mut inner.side_script_lists)
        };
        let active_lists = std::cell::RefCell::new(ActiveScriptLists {
            owner_id: self.instance_id,
            lists,
            executing_scripts: Vec::new(),
            executing_groups: Vec::new(),
        });

        ACTIVE_SCRIPT_LISTS.set(&active_lists, || {
            let _restore = ActiveScriptListsRestore {
                engine: self,
                active_lists: &active_lists,
            };
            f()
        })
    }
}

/// Run `f` against the currently executing ScriptEngine (nested path), if any.
///
/// Hands out `&ScriptEngine` only. Nested mutations use `lock_inner_mut`
/// (no aliased `&mut ScriptEngine`). C++ `CALL_SUBROUTINE` runs immediately.
pub fn with_active_script_engine_mut<R>(f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
    ACTIVE_SCRIPT_ENGINE
        .is_set()
        .then(|| ACTIVE_SCRIPT_ENGINE.with(f))
}

/// Run `f` against the currently executing ScriptEngine (shared view), if any.
///
/// If an exclusive `lock_inner_mut` guard is live, skip (do not alias).
pub fn with_active_script_engine_ref<R>(f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
    if !ACTIVE_SCRIPT_ENGINE.is_set() {
        return None;
    }
    ACTIVE_SCRIPT_ENGINE.with(|engine| {
        if engine.inner.try_borrow().is_err() {
            None
        } else {
            Some(f(engine))
        }
    })
}

/// Whether the current thread is evaluating a live `ScriptEngine` scope.
///
/// Callers that carry an alternate `ScriptEngineHandle` must distinguish an
/// absent active scope from an active scope whose interior state cannot be
/// borrowed at that instant.  The former may safely use their own handle; the
/// latter must fail closed rather than re-locking the active global engine.
pub fn is_script_engine_active() -> bool {
    ACTIVE_SCRIPT_ENGINE.is_set()
}

/// Mutate the global ScriptEngine with re-entrant nesting support.
///
/// Prefers the TLS active engine (no lock). Otherwise acquires the global
/// write lock (thread exclusion; mutations are interior via `lock_inner_mut`),
/// installs TLS for the duration of `f`, and runs `f` with `&ScriptEngine`.
///
/// Nested `with_script_engine_mut` from inside `f` runs **immediately**
/// (C++ `ScriptEngine::callSubroutine` → `executeScript` order).
pub fn with_script_engine_mut<R>(f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
    // Manual branch so we do not consume `f` when the active path is empty.
    if ACTIVE_SCRIPT_ENGINE.is_set() {
        return with_active_script_engine_mut(f);
    }
    let arc = get_script_engine();
    let guard = arc.write().ok()?;
    let engine = guard.as_ref()?;
    Some(engine.with_active(|| f(engine)))
}

/// Read the global ScriptEngine with re-entrant nesting support.
pub fn with_script_engine_ref<R>(f: impl FnOnce(&ScriptEngine) -> R) -> Option<R> {
    if ACTIVE_SCRIPT_ENGINE.is_set() {
        return with_active_script_engine_ref(f);
    }
    let arc = get_script_engine();
    let guard = arc.read().ok()?;
    let engine = guard.as_ref()?;
    Some(f(engine))
}

/// Initialize the global script engine
pub fn initialize_script_engine() -> GameLogicResult<()> {
    let mut global = SCRIPT_ENGINE.write().map_err(|e| {
        GameLogicError::Threading(format!("Failed to acquire script engine lock: {}", e))
    })?;
    // Make initialization idempotent to avoid test flakiness and accidental
    // state loss from repeated initialization calls.
    if global.is_none() {
        *global = Some(ScriptEngine::new()?);
    }
    Ok(())
}

/// Get reference to global script engine
pub fn get_script_engine() -> ScriptEngineHandle {
    SCRIPT_ENGINE.clone()
}

/// ScriptEngine parity state for `ScriptEngine::setEnableVTune/getEnableVTune`.
pub fn set_enable_vtune(enabled: bool) {
    if let Ok(mut guard) = VTUNE_ENABLED_STATE.write() {
        *guard = enabled;
    }
}

/// ScriptEngine parity state for `ScriptEngine::setEnableVTune/getEnableVTune`.
pub fn get_enable_vtune() -> bool {
    VTUNE_ENABLED_STATE
        .read()
        .map(|guard| *guard)
        .unwrap_or(false)
}

/// Debug parity state for `TheSkateDistOverride` command plumbing.
pub fn set_skate_distance_override(value: f32) {
    if let Ok(mut guard) = SKATE_DISTANCE_OVERRIDE_STATE.write() {
        *guard = value;
    }
}

/// Debug parity state for `TheSkateDistOverride` command plumbing.
pub fn adjust_skate_distance_override(delta: f32) -> f32 {
    if let Ok(mut guard) = SKATE_DISTANCE_OVERRIDE_STATE.write() {
        *guard += delta;
        return *guard;
    }
    0.0
}

/// Debug parity state for `TheSkateDistOverride` command plumbing.
pub fn get_skate_distance_override() -> f32 {
    SKATE_DISTANCE_OVERRIDE_STATE
        .read()
        .map(|guard| *guard)
        .unwrap_or(0.0)
}

/// Get reference to global event manager
pub fn get_event_manager() -> Arc<EventManager> {
    EVENT_MANAGER.clone()
}

/// Get reference to global named object tracker
pub fn get_named_object_tracker() -> Arc<NamedObjectTracker> {
    NAMED_OBJECT_TRACKER.clone()
}

/// Transfer a script-visible object name to another object (C++ ScriptEngine::transferObjectName).
pub fn transfer_object_name(
    from_name: &AsciiString,
    to_object_id: ObjectID,
) -> GameLogicResult<()> {
    // Wave 348: empty dual-world → Ok(()).
    if dual_world_registry_unavailable() {
        return Ok(());
    }

    let tracker = get_named_object_tracker();
    if let Ok(Some(old_id)) = tracker.get_object_id(from_name.as_str()) {
        let _ = tracker.unregister_object(old_id);
    }

    let Some(()) = OBJECT_REGISTRY.with_object_mut(to_object_id, |guard| {
        guard.set_name(from_name.clone());
    }) else {
        return Err(GameLogicError::InvalidObject(to_object_id));
    };

    tracker.register_named_object(from_name.to_string(), to_object_id)?;
    Ok(())
}

/// Get reference to global area tracker
pub fn get_area_tracker() -> Arc<AreaTracker> {
    AREA_TRACKER.clone()
}
