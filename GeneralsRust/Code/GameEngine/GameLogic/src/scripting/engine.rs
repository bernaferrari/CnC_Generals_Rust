//! Script Engine Implementation
//!
//! This module provides the main script engine that matches the C++ ScriptEngine class.
//! It handles script execution, condition evaluation, action processing, and state management.

use super::core::*;
use super::events::{AreaTracker, EventManager, NamedObjectTracker};
use crate::common::{
    kind_of_indices, AsciiString, KindOf, ObjectID, INVALID_ID, LOGICFRAMES_PER_SECOND,
};
use crate::helpers::{TheAudio, TheGameLogic, TheThingFactory};
use crate::object::object_types::ObjectTypes;
use crate::object::registry::OBJECT_REGISTRY;
use crate::scripting::XferSnapshot;
use crate::team::{get_team_factory, TeamID, TheTeamFactory, TEAM_ID_INVALID};
use crate::ObjectId;
use crate::{GameLogicError, GameLogicResult};
use futures::executor::block_on;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::rts::ScienceType;
use game_engine::common::system::{Xfer, XferMode, XferStatus, XferVersion};
use game_engine::common::thing::thing_factory::get_thing_factory;
use game_engine::common::thing::thing_template::ThingTemplate as EngineThingTemplate;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

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
    pub fn new() -> Self {
        Self {
            direction: 0.0,
            direction_vec: [1.0, 0.0],
            intensity: 0.0,
            lean: 0.0,
            randomness: 0.0,
            breeze_period: 120,
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
        }
    }
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
        // SAFETY: team_id is a valid stack variable
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

/// Main Script Engine matching C++ ScriptEngine
pub struct ScriptEngine {
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
    freeze_by_script: bool,
    freeze_by_debug: bool,
    objects_should_receive_difficulty_bonus: bool,
    choose_victim_always_uses_normal: bool,
    shown_mp_local_defeat_window: bool,

    // Sequential scripts
    sequential_scripts: Vec<SequentialScript>,

    // Script lists to execute (per player "side")
    side_script_lists: Vec<Option<Box<ScriptList>>>,

    // Statistics
    #[cfg(feature = "script_profiling")]
    stats: ScriptStats,

    action_handler: Option<Arc<dyn ScriptActionHandler>>,
}

fn xfer_list_ascii_string(xfer: &mut dyn Xfer, list: &mut Vec<String>) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for entry in list.iter() {
                let mut value = entry.clone();
                xfer.xfer_ascii_string(&mut value)?;
            }
        }
        XferMode::Load => {
            if !list.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            list.clear();
            for _ in 0..count {
                let mut value = String::new();
                xfer.xfer_ascii_string(&mut value)?;
                list.push(value);
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_list_ascii_string_uint(
    xfer: &mut dyn Xfer,
    list: &mut Vec<(String, u32)>,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for (name, value) in list.iter() {
                let mut entry_name = name.clone();
                let mut entry_value = *value;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_unsigned_int(&mut entry_value)?;
            }
        }
        XferMode::Load => {
            if !list.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            list.clear();
            for _ in 0..count {
                let mut entry_name = String::new();
                let mut entry_value: u32 = 0;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_unsigned_int(&mut entry_value)?;
                list.push((entry_name, entry_value));
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_list_ascii_string_object_id(
    xfer: &mut dyn Xfer,
    list: &mut Vec<(String, ObjectID)>,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for (name, object_id) in list.iter() {
                let mut entry_name = name.clone();
                let mut entry_id = *object_id;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_object_id(&mut entry_id)?;
            }
        }
        XferMode::Load => {
            if !list.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            list.clear();
            for _ in 0..count {
                let mut entry_name = String::new();
                let mut entry_id: ObjectID = crate::common::INVALID_ID;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_object_id(&mut entry_id)?;
                list.push((entry_name, entry_id));
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_list_ascii_string_coord3d(
    xfer: &mut dyn Xfer,
    list: &mut Vec<(String, Coord3D)>,
) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for (name, coord) in list.iter() {
                let mut entry_name = name.clone();
                let mut entry_coord = *coord;
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_real(&mut entry_coord.x)?;
                xfer.xfer_real(&mut entry_coord.y)?;
                xfer.xfer_real(&mut entry_coord.z)?;
            }
        }
        XferMode::Load => {
            if !list.is_empty() {
                return Err(XferStatus::ListNotEmpty);
            }
            list.clear();
            for _ in 0..count {
                let mut entry_name = String::new();
                let mut entry_coord = Coord3D::zero();
                xfer.xfer_ascii_string(&mut entry_name)?;
                xfer.xfer_real(&mut entry_coord.x)?;
                xfer.xfer_real(&mut entry_coord.y)?;
                xfer.xfer_real(&mut entry_coord.z)?;
                list.push((entry_name, entry_coord));
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

fn xfer_science_vec(xfer: &mut dyn Xfer, list: &mut Vec<ScienceType>) -> Result<(), XferStatus> {
    let current_version: XferVersion = 1;
    let mut version = current_version;
    xfer.xfer_version(&mut version, current_version)?;

    let mut count: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
        list.len() as u16
    } else {
        0
    };
    xfer.xfer_unsigned_short(&mut count)?;

    match xfer.get_xfer_mode() {
        XferMode::Save => {
            for science in list.iter() {
                let mut value = *science as i32;
                xfer.xfer_int(&mut value)?;
            }
        }
        XferMode::Load => {
            list.clear();
            for _ in 0..count {
                let mut value: i32 = 0;
                xfer.xfer_int(&mut value)?;
                list.push(value as ScienceType);
            }
        }
        XferMode::Crc => {
            for science in list.iter() {
                let mut value = *science as i32;
                xfer.xfer_int(&mut value)?;
            }
        }
        XferMode::Invalid => return Err(XferStatus::ModeUnknown),
    }

    Ok(())
}

impl ScriptEngine {
    fn enum_name_to_internal_name(name: &str) -> String {
        let mut out = String::with_capacity(name.len() * 2);
        let mut prev_is_upper = true;
        for ch in name.chars() {
            let is_upper = ch.is_ascii_uppercase();
            if !out.is_empty() && is_upper && !prev_is_upper {
                out.push('_');
            }
            out.push(ch.to_ascii_uppercase());
            prev_is_upper = is_upper;
        }
        out
    }

    fn seed_template_internal_names(&mut self) {
        for idx in 0..(ScriptActionType::NumItems as u32) {
            let Some(action_type) = ScriptActionType::from_u32(idx) else {
                continue;
            };
            if action_type == ScriptActionType::NumItems {
                continue;
            }
            if let Some(template) = self.action_templates.get_mut(idx as usize) {
                let internal_name = Self::enum_name_to_internal_name(&format!("{:?}", action_type));
                template.base.internal_name = internal_name.clone();
                template.base.internal_name_key = NameKeyGenerator::name_to_key(&internal_name);
                if template.base.ui_name.is_empty() {
                    template.base.ui_name = internal_name;
                }
            }
        }

        for idx in 0..(ConditionType::NumItems as u32) {
            let Some(condition_type) = ConditionType::from_u32(idx) else {
                continue;
            };
            if condition_type == ConditionType::NumItems {
                continue;
            }
            if let Some(template) = self.condition_templates.get_mut(idx as usize) {
                let internal_name =
                    Self::enum_name_to_internal_name(&format!("{:?}", condition_type));
                template.base.internal_name = internal_name.clone();
                template.base.internal_name_key = NameKeyGenerator::name_to_key(&internal_name);
                if template.base.ui_name.is_empty() {
                    template.base.ui_name = internal_name;
                }
            }
        }
    }

    const MAX_PLAYER_COUNT: usize = crate::common::MAX_PLAYER_COUNT;

    pub fn new() -> GameLogicResult<Self> {
        let mut engine = Self {
            action_templates: Vec::with_capacity(ScriptActionType::NumItems as usize),
            condition_templates: Vec::with_capacity(ConditionType::NumItems as usize),

            counters: vec![None; MAX_COUNTERS],
            num_counters: 1,
            flags: vec![None; MAX_FLAGS],
            num_flags: 1,
            attack_priority_info: Vec::with_capacity(MAX_ATTACK_PRIORITIES),
            num_attack_info: 1,

            end_game_timer: -1,
            close_window_timer: -1,
            calling_team: None,
            calling_object: None,
            condition_team: None,
            condition_object: None,
            first_update: true,
            current_player: None,
            skirmish_human_player: None,
            current_track_name: String::new(),

            fade: TFade::None,
            min_fade: 0.0,
            max_fade: 1.0,
            cur_fade_value: 0.0,
            cur_fade_frame: 0,
            fade_frames_increase: 0,
            fade_frames_hold: 0,
            fade_frames_decrease: 0,

            frame_object_count_changed: 0,
            object_counts: HashMap::new(),
            object_types: HashMap::new(),
            object_attack_priority_sets: HashMap::new(),

            completed_video: Vec::new(),
            testing_speech: Vec::new(),
            testing_audio: Vec::new(),
            ui_interactions: Vec::new(),

            triggered_special_powers: vec![Vec::new(); Self::MAX_PLAYER_COUNT],
            midway_special_powers: vec![Vec::new(); Self::MAX_PLAYER_COUNT],
            finished_special_powers: vec![Vec::new(); Self::MAX_PLAYER_COUNT],
            completed_upgrades: vec![Vec::new(); Self::MAX_PLAYER_COUNT],
            acquired_sciences: vec![Vec::new(); Self::MAX_PLAYER_COUNT],

            topple_directions: Vec::new(),
            named_reveals: Vec::new(),
            breeze_info: BreezeInfo::new(),
            game_difficulty: crate::player::GameDifficulty::Normal,

            freeze_by_script: false,
            freeze_by_debug: false,
            objects_should_receive_difficulty_bonus: true,
            choose_victim_always_uses_normal: false,
            shown_mp_local_defeat_window: false,

            sequential_scripts: Vec::new(),

            side_script_lists: vec![None; Self::MAX_PLAYER_COUNT],

            #[cfg(feature = "script_profiling")]
            stats: ScriptStats::default(),

            action_handler: None,
        };

        if engine.counters[0].is_none() {
            engine.counters[0] = Some(TCounter::new(String::new()));
        }
        if engine.flags[0].is_none() {
            engine.flags[0] = Some(TFlag::new(String::new()));
        }
        if engine.attack_priority_info.is_empty() {
            engine.attack_priority_info.push(AttackPriorityInfo::new());
        }

        engine.initialize_templates()?;
        Ok(engine)
    }

    pub fn get_frame_object_count_changed(&self) -> u32 {
        self.frame_object_count_changed
    }

    pub fn get_current_player_name(&self) -> Option<&str> {
        self.current_player.as_deref()
    }

    pub fn get_calling_team_name(&self) -> Option<&str> {
        self.calling_team.as_deref()
    }

    pub fn get_condition_team_name(&self) -> Option<&str> {
        self.condition_team.as_deref()
    }

    /// Set temporary runtime context used by external script evaluation helpers.
    ///
    /// Returns the previous `(current_player, condition_team)` tuple so callers can restore it.
    pub fn set_external_eval_context(
        &mut self,
        current_player: Option<String>,
        condition_team: Option<String>,
    ) -> (Option<String>, Option<String>) {
        let saved = (self.current_player.clone(), self.condition_team.clone());
        self.current_player = current_player;
        self.condition_team = condition_team;
        saved
    }

    /// Restore runtime context previously returned by `set_external_eval_context`.
    pub fn restore_external_eval_context(&mut self, saved: (Option<String>, Option<String>)) {
        self.current_player = saved.0;
        self.condition_team = saved.1;
    }

    pub fn set_frame_object_count_changed(&mut self, frame: u32) {
        self.frame_object_count_changed = frame;
    }

    /// Set the script list for a player index (side).
    ///
    /// C++ Reference: `SideInfo::getScriptList()` (ScriptEngine::update loops sides and executes
    /// the side's ScriptList + ScriptGroups).
    pub fn set_script_list_for_player(
        &mut self,
        player_index: usize,
        script_list: Option<Box<ScriptList>>,
    ) -> GameLogicResult<()> {
        if player_index >= Self::MAX_PLAYER_COUNT {
            return Err(GameLogicError::Configuration(format!(
                "Player index {} out of range for ScriptEngine",
                player_index
            )));
        }
        let mut script_list = script_list;
        if let Some(list) = script_list.as_deref_mut() {
            self.initialize_script_runtime_fields_in_list(list);
        }
        self.side_script_lists[player_index] = script_list;
        Ok(())
    }

    pub fn clear_script_lists(&mut self) {
        for slot in &mut self.side_script_lists {
            *slot = None;
        }
    }

    fn set_script_active_in_chain(script: &mut Script, name: &str, active: bool) -> bool {
        let mut current: Option<&mut Script> = Some(script);
        let mut updated = false;
        while let Some(script_ref) = current {
            if script_ref.script_name == name {
                script_ref.is_active = active;
                updated = true;
            }
            current = script_ref.next_script.as_deref_mut();
        }
        updated
    }

    fn set_script_active_in_list(list: &mut ScriptList, name: &str, active: bool) -> bool {
        let mut updated = false;

        if let Some(script_head) = list.first_script.as_deref_mut() {
            updated |= Self::set_script_active_in_chain(script_head, name, active);
        }

        let mut group_opt = list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref_mut() {
                updated |= Self::set_script_active_in_chain(script_head, name, active);
            }
            group_opt = group.next_group.as_deref_mut();
        }

        updated
    }

    fn find_script_clone_in_chain(
        script: &Script,
        name: &str,
        require_subroutine: bool,
    ) -> Option<Script> {
        let mut current: Option<&Script> = Some(script);
        while let Some(script_ref) = current {
            if script_ref.script_name == name && (!require_subroutine || script_ref.is_subroutine) {
                return Some(script_ref.clone());
            }
            current = script_ref.next_script.as_deref();
        }
        None
    }

    fn find_script_clone_in_list(
        list: &ScriptList,
        name: &str,
        require_subroutine: bool,
    ) -> Option<Script> {
        if let Some(script_head) = list.first_script.as_deref() {
            if let Some(found) =
                Self::find_script_clone_in_chain(script_head, name, require_subroutine)
            {
                return Some(found);
            }
        }

        let mut group_opt = list.first_group.as_deref();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref() {
                if let Some(found) =
                    Self::find_script_clone_in_chain(script_head, name, require_subroutine)
                {
                    return Some(found);
                }
            }
            group_opt = group.next_group.as_deref();
        }

        None
    }

    /// Enable/disable a script by name across all loaded ScriptLists.
    ///
    /// Matches the behavior of C++ `ENABLE_SCRIPT` / `DISABLE_SCRIPT` actions.
    pub fn set_script_active_by_name(&mut self, script_name: &str, active: bool) -> bool {
        let mut updated = false;

        for slot in &mut self.side_script_lists {
            let Some(list) = slot.as_deref_mut() else {
                continue;
            };
            updated |= Self::set_script_active_in_list(list, script_name, active);
        }

        if let Some(handler) = self.action_handler.as_ref() {
            let _ = handler.enable_script(script_name, active);
        }

        updated
    }

    /// Find a subroutine script by name and return a clone for immediate execution.
    pub fn get_subroutine_clone_by_name(&self, name: &str) -> Option<Script> {
        for slot in &self.side_script_lists {
            let Some(list) = slot.as_deref() else {
                continue;
            };
            if let Some(found) = Self::find_script_clone_in_list(list, name, true) {
                return Some(found);
            }
        }
        None
    }

    fn execute_named_subroutine_in_chain(
        &mut self,
        script_head: &mut Script,
        name: &str,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<bool> {
        let mut current: Option<&mut Script> = Some(script_head);
        while let Some(script_ref) = current {
            if script_ref.script_name == name {
                if script_ref.is_subroutine {
                    self.execute_script(script_ref, condition_evaluator, action_dispatcher)?;
                } else {
                    log::warn!(
                        "CALL_SUBROUTINE: script '{}' exists but is not a subroutine",
                        name
                    );
                }
                return Ok(true);
            }
            current = script_ref.next_script.as_deref_mut();
        }
        Ok(false)
    }

    fn execute_named_subroutine_in_list(
        &mut self,
        list: &mut ScriptList,
        name: &str,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<bool> {
        // C++ parity: look up a subroutine group by name first.
        let mut group_opt = list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if group.group_name == name {
                if !group.is_group_subroutine {
                    log::warn!(
                        "CALL_SUBROUTINE: group '{}' exists but is not a subroutine group",
                        name
                    );
                    return Ok(true);
                }
                if group.is_group_active {
                    if let Some(script_head) = group.first_script.as_deref_mut() {
                        self.execute_scripts(script_head, condition_evaluator, action_dispatcher)?;
                    }
                }
                return Ok(true);
            }
            group_opt = group.next_group.as_deref_mut();
        }

        if let Some(script_head) = list.first_script.as_deref_mut() {
            if self.execute_named_subroutine_in_chain(
                script_head,
                name,
                condition_evaluator,
                action_dispatcher,
            )? {
                return Ok(true);
            }
        }

        let mut group_opt = list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref_mut() {
                if self.execute_named_subroutine_in_chain(
                    script_head,
                    name,
                    condition_evaluator,
                    action_dispatcher,
                )? {
                    return Ok(true);
                }
            }
            group_opt = group.next_group.as_deref_mut();
        }

        Ok(false)
    }

    /// Execute a subroutine script or subroutine group by name.
    ///
    /// Matches C++ `ScriptEngine::callSubroutine`: group lookup by name takes precedence over
    /// direct script lookup by name.
    pub fn execute_subroutine_by_name(&mut self, name: &str) -> GameLogicResult<bool> {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let exec_context = Arc::new(RwLock::new(crate::scripting::executor::ScriptContext {
            game_logic_id: 0,
            object_manager_id: 0,
            player_manager_id: 0,
            event_system_id: 0,
            camera_system_id: 0,
            audio_system_id: 0,
            partition_manager_id: 0,
            special_powers_id: 0,
            current_frame,
            suppress_new_windows: false,
        }));

        let mut action_dispatcher =
            crate::scripting::executor::ScriptActionDispatcher::new(exec_context.clone());
        let mut condition_evaluator =
            crate::scripting::executor::ScriptConditionEvaluator::new(exec_context);

        for i in 0..Self::MAX_PLAYER_COUNT {
            let Some(mut script_list) = self.side_script_lists[i].take() else {
                continue;
            };

            let found = self.execute_named_subroutine_in_list(
                &mut script_list,
                name,
                &mut condition_evaluator,
                &mut action_dispatcher,
            )?;
            self.side_script_lists[i] = Some(script_list);
            if found {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Find a script by name and return a clone (non-subroutine allowed).
    pub fn find_script_clone_by_name(&self, name: &str) -> Option<Script> {
        for slot in &self.side_script_lists {
            let Some(list) = slot.as_deref() else {
                continue;
            };
            if let Some(found) = Self::find_script_clone_in_list(list, name, false) {
                return Some(found);
            }
        }
        None
    }

    pub fn get_object_count(&self, player_index: i32, type_name: &str) -> i32 {
        self.object_counts
            .get(&(player_index, type_name.to_string()))
            .copied()
            .unwrap_or(0)
    }

    pub fn set_object_count(&mut self, player_index: i32, type_name: &str, count: i32) {
        self.object_counts
            .insert((player_index, type_name.to_string()), count);
    }

    /// Get a named ObjectTypes list (matches C++ ScriptEngine::getObjectTypes).
    pub fn get_object_types(&self, name: &str) -> Option<ObjectTypes> {
        self.object_types.get(name).cloned()
    }

    /// Register or replace a named ObjectTypes list.
    pub fn set_object_types(&mut self, name: String, types: ObjectTypes) {
        self.object_types.insert(name, types);
    }

    fn ensure_attack_priority_defaults(&mut self) {
        if self.attack_priority_info.is_empty() {
            self.attack_priority_info.push(AttackPriorityInfo::new());
        }
        if self.num_attack_info == 0 {
            self.num_attack_info = 1;
        }
    }

    fn find_attack_info_mut(
        &mut self,
        name: &str,
        add_if_missing: bool,
    ) -> Option<&mut AttackPriorityInfo> {
        self.ensure_attack_priority_defaults();
        let existing_index = (1..self.num_attack_info).find(|&i| {
            self.attack_priority_info
                .get(i)
                .map(|info| info.name == name)
                .unwrap_or(false)
        });
        if let Some(index) = existing_index {
            return self.attack_priority_info.get_mut(index);
        }

        if add_if_missing && self.num_attack_info < MAX_ATTACK_PRIORITIES {
            let mut info = AttackPriorityInfo::new();
            info.name = name.to_string();
            if self.attack_priority_info.len() <= self.num_attack_info {
                self.attack_priority_info.push(info);
            } else {
                self.attack_priority_info[self.num_attack_info] = info;
            }
            let index = self.num_attack_info;
            self.num_attack_info += 1;
            return self.attack_priority_info.get_mut(index);
        }

        None
    }

    pub fn get_attack_info(&self, name: &str) -> Option<&AttackPriorityInfo> {
        if self.attack_priority_info.is_empty() {
            return None;
        }
        for i in 1..self.num_attack_info {
            if let Some(info) = self.attack_priority_info.get(i) {
                if info.name == name {
                    return Some(info);
                }
            }
        }
        self.attack_priority_info.get(0)
    }

    pub fn set_object_attack_priority_set(&mut self, object_id: ObjectID, set_name: &str) {
        if object_id == INVALID_ID {
            return;
        }

        if set_name.is_empty() {
            self.object_attack_priority_sets.remove(&object_id);
            return;
        }

        self.object_attack_priority_sets
            .insert(object_id, set_name.to_string());
    }

    pub fn clear_object_attack_priority_set(&mut self, object_id: ObjectID) {
        self.object_attack_priority_sets.remove(&object_id);
    }

    pub fn get_object_attack_priority_set(&self, object_id: ObjectID) -> Option<&str> {
        self.object_attack_priority_sets
            .get(&object_id)
            .map(|name| name.as_str())
    }

    fn template_matches_kind(template: &EngineThingTemplate, kind: KindOf) -> bool {
        for idx in kind_of_indices(kind) {
            if template.is_kind_of((*idx) as u64) {
                return true;
            }
        }
        false
    }

    pub fn set_priority_thing(
        &mut self,
        set_name: &str,
        type_or_list: &str,
        priority: i32,
    ) -> bool {
        let object_types = self.get_object_types(type_or_list);
        let Some(info) = self.find_attack_info_mut(set_name, true) else {
            return false;
        };

        if let Some(list) = object_types {
            for type_name in list.iter() {
                if let Some(template) = TheThingFactory::find_template(type_name.as_str()) {
                    info.set_priority(template.get_name().as_str(), priority);
                } else {
                    return false;
                }
            }
            return true;
        }

        if let Some(template) = TheThingFactory::find_template(type_or_list) {
            info.set_priority(template.get_name().as_str(), priority);
            return true;
        }

        false
    }

    pub fn set_priority_kind(&mut self, set_name: &str, kind: KindOf, priority: i32) -> bool {
        let Some(info) = self.find_attack_info_mut(set_name, true) else {
            return false;
        };

        let Ok(factory_guard) = get_thing_factory() else {
            return false;
        };
        let Some(factory) = factory_guard.as_ref() else {
            return false;
        };

        let mut current = factory.first_template().cloned();
        while let Some(template) = current {
            if Self::template_matches_kind(&template, kind) {
                info.set_priority(template.get_name().as_str(), priority);
            }
            current = template.get_next_template().as_ref().cloned();
        }

        true
    }

    pub fn set_priority_default(&mut self, set_name: &str, priority: i32) -> bool {
        let Some(info) = self.find_attack_info_mut(set_name, true) else {
            return false;
        };
        info.default_priority = priority;
        true
    }

    /// Initialize action and condition templates
    fn initialize_templates(&mut self) -> GameLogicResult<()> {
        // Initialize action templates (this would normally be done from INI files)
        self.action_templates
            .resize(ScriptActionType::NumItems as usize, ActionTemplate::new());
        self.condition_templates
            .resize(ConditionType::NumItems as usize, ConditionTemplate::new());

        self.seed_template_internal_names();

        // Set up basic templates (in real implementation, this would be loaded from INI)
        self.setup_basic_templates()?;

        Ok(())
    }

    /// Set up basic templates for core actions and conditions
    fn setup_basic_templates(&mut self) -> GameLogicResult<()> {
        // Victory action
        if let Some(template) = self
            .action_templates
            .get_mut(ScriptActionType::Victory as usize)
        {
            template.base.ui_name = "Victory".to_string();
            template.base.internal_name = "Victory".to_string();
            template.base.help_text = "Triggers victory for the current player".to_string();
        }

        // Defeat action
        if let Some(template) = self
            .action_templates
            .get_mut(ScriptActionType::Defeat as usize)
        {
            template.base.ui_name = "Defeat".to_string();
            template.base.internal_name = "Defeat".to_string();
            template.base.help_text = "Triggers defeat for the current player".to_string();
        }

        // Set flag action
        if let Some(template) = self
            .action_templates
            .get_mut(ScriptActionType::SetFlag as usize)
        {
            template.base.ui_name = "Set Flag".to_string();
            template.base.internal_name = "SetFlag".to_string();
            template.base.help_text = "Sets a script flag to true or false".to_string();
            template.base.parameters = vec![ParameterType::Flag, ParameterType::Boolean];
            template.base.num_parameters = 2;
        }

        // Set counter action
        if let Some(template) = self
            .action_templates
            .get_mut(ScriptActionType::SetCounter as usize)
        {
            template.base.ui_name = "Set Counter".to_string();
            template.base.internal_name = "SetCounter".to_string();
            template.base.help_text = "Sets a script counter to a value".to_string();
            template.base.parameters = vec![ParameterType::Counter, ParameterType::Int];
            template.base.num_parameters = 2;
        }

        // Player all destroyed condition
        if let Some(template) = self
            .condition_templates
            .get_mut(ConditionType::PlayerAllDestroyed as usize)
        {
            template.base.ui_name = "Player All Destroyed".to_string();
            template.base.internal_name = "PlayerAllDestroyed".to_string();
            template.base.help_text = "True if all of a player's units are destroyed".to_string();
            template.base.parameters = vec![ParameterType::Side];
            template.base.num_parameters = 1;
        }

        // Counter condition
        if let Some(template) = self
            .condition_templates
            .get_mut(ConditionType::Counter as usize)
        {
            template.base.ui_name = "Counter".to_string();
            template.base.internal_name = "Counter".to_string();
            template.base.help_text = "Compares a counter value".to_string();
            template.base.parameters = vec![
                ParameterType::Counter,
                ParameterType::Comparison,
                ParameterType::Int,
            ];
            template.base.num_parameters = 3;
        }

        // Flag condition
        if let Some(template) = self
            .condition_templates
            .get_mut(ConditionType::Flag as usize)
        {
            template.base.ui_name = "Flag".to_string();
            template.base.internal_name = "Flag".to_string();
            template.base.help_text = "Checks if a flag is set".to_string();
            template.base.parameters = vec![ParameterType::Flag, ParameterType::Boolean];
            template.base.num_parameters = 2;
        }

        Ok(())
    }

    /// Reset the script engine
    pub fn reset(&mut self) {
        // Clear runtime state
        self.counters.iter_mut().for_each(|c| *c = None);
        self.num_counters = 1;
        self.flags.iter_mut().for_each(|f| *f = None);
        self.num_flags = 1;
        self.attack_priority_info.clear();
        self.num_attack_info = 1;

        self.end_game_timer = -1;
        self.close_window_timer = -1;
        self.calling_team = None;
        self.calling_object = None;
        self.condition_team = None;
        self.condition_object = None;
        self.first_update = true;
        self.current_player = None;
        self.skirmish_human_player = None;
        self.current_track_name.clear();

        self.fade = TFade::None;
        self.cur_fade_value = 0.0;
        self.cur_fade_frame = 0;

        self.completed_video.clear();
        self.testing_speech.clear();
        self.testing_audio.clear();
        self.ui_interactions.clear();

        for powers in &mut self.triggered_special_powers {
            powers.clear();
        }
        for powers in &mut self.midway_special_powers {
            powers.clear();
        }
        for powers in &mut self.finished_special_powers {
            powers.clear();
        }
        for upgrades in &mut self.completed_upgrades {
            upgrades.clear();
        }
        for sciences in &mut self.acquired_sciences {
            sciences.clear();
        }

        self.topple_directions.clear();
        self.named_reveals.clear();
        self.object_types.clear();
        self.object_attack_priority_sets.clear();
        self.breeze_info = BreezeInfo::new();
        self.game_difficulty = crate::player::GameDifficulty::Normal;

        self.freeze_by_script = false;
        self.freeze_by_debug = false;
        self.objects_should_receive_difficulty_bonus = true;
        self.choose_victim_always_uses_normal = false;
        self.shown_mp_local_defeat_window = false;

        self.sequential_scripts.clear();
        self.clear_script_lists();

        #[cfg(feature = "script_profiling")]
        {
            self.stats = ScriptStats::default();
        }

        if self.counters[0].is_none() {
            self.counters[0] = Some(TCounter::new(String::new()));
        }
        if self.flags[0].is_none() {
            self.flags[0] = Some(TFlag::new(String::new()));
        }
        if self.attack_priority_info.is_empty() {
            self.attack_priority_info.push(AttackPriorityInfo::new());
        }
    }

    /// Update script engine
    pub fn update(&mut self) -> GameLogicResult<()> {
        #[cfg(feature = "script_profiling")]
        let start_time = Instant::now();

        if self.first_update {
            self.create_named_cache();
            self.first_update = false;
            log::info!("ScriptEngine first update: named cache populated");
        }

        if self.end_game_timer > 0 {
            self.end_game_timer -= 1;
            if self.end_game_timer < 1 {
                log::info!("End game timer expired, clearing game data");
                let _ = TheGameLogic::clear_game_data();
            }
        }

        if self.close_window_timer >= 0 {
            self.close_window_timer -= 1;
            if self.close_window_timer <= 0 {
                log::info!("Close window timer expired");
                // In real implementation, this would close UI windows
            }
        }

        // C++ parity: freeze-by-debug stops further script update progression.
        if self.is_time_frozen_debug() {
            return Ok(());
        }

        // Update counters that are countdown timers
        for counter in &mut self.counters {
            if let Some(counter) = counter {
                if counter.is_countdown_timer && counter.value > 0 {
                    counter.value -= 1;
                }
            }
        }

        // Update fade effects
        self.update_fades();

        // If the engine is in an end-game timing-down state, C++ returns early.
        if self.end_game_timer >= 0 {
            return Ok(());
        }

        // Evaluate scripts for each player/side, matching C++ `ScriptEngine::update()`.
        self.execute_side_scripts()?;

        // Clear UI interaction flags (C++: m_uiInteractions.clear()).
        self.ui_interactions.clear();

        // Process sequential scripts
        self.evaluate_and_progress_all_sequential_scripts()?;

        #[cfg(feature = "script_profiling")]
        {
            let elapsed = start_time.elapsed();
            self.stats.cur_update_time = elapsed.as_secs_f64();
            self.stats.total_update_time += self.stats.cur_update_time;
            self.stats.num_frames += 1.0;
            if self.stats.cur_update_time > self.stats.max_update_time {
                self.stats.max_update_time = self.stats.cur_update_time;
            }
        }

        Ok(())
    }

    /// Populate the NamedObjectTracker from currently registered objects.
    ///
    /// C++ Reference: `ScriptEngine::createNamedCache()`.
    fn create_named_cache(&self) {
        let tracker = get_named_object_tracker();
        for obj_arc in OBJECT_REGISTRY.get_all_objects() {
            let Ok(obj) = obj_arc.read() else { continue };
            let name = obj.get_name();
            if name.is_empty() {
                continue;
            }
            let _ = tracker.register_named_object(name.to_string(), obj.get_id());
        }
    }

    /// Notify the script engine that objects were created or destroyed.
    /// Mirrors C++ ScriptEngine::notifyOfObjectCreationOrDestruction().
    pub fn notify_of_object_creation_or_destruction(&mut self) {
        self.create_named_cache();
    }

    fn execute_side_scripts(&mut self) -> GameLogicResult<()> {
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        // Prepare executor context for this frame (shared by action/condition evaluation).
        let exec_context = Arc::new(RwLock::new(crate::scripting::executor::ScriptContext {
            game_logic_id: 0,
            object_manager_id: 0,
            player_manager_id: 0,
            event_system_id: 0,
            camera_system_id: 0,
            audio_system_id: 0,
            partition_manager_id: 0,
            special_powers_id: 0,
            current_frame,
            suppress_new_windows: false,
        }));

        let mut action_dispatcher =
            crate::scripting::executor::ScriptActionDispatcher::new(exec_context.clone());
        let mut condition_evaluator =
            crate::scripting::executor::ScriptConditionEvaluator::new(exec_context);

        let player_list = crate::player::player_list();
        let Ok(list_guard) = player_list.read() else {
            return Err(GameLogicError::Threading(
                "Failed to lock PlayerList for ScriptEngine::update".to_string(),
            ));
        };

        let player_count = list_guard.get_player_count().min(Self::MAX_PLAYER_COUNT);
        for i in 0..player_count {
            // Match C++: `m_currentPlayer` is the nth player for the side index.
            let player_name = list_guard.get_player(i as i32).and_then(|p| {
                p.read()
                    .ok()
                    .and_then(|p| NameKeyGenerator::key_to_name(p.get_player_name_key()))
            });
            self.current_player = player_name;

            // Avoid aliasing `&mut self` with `&mut ScriptList` while calling into helpers that
            // also need `&mut self` (borrow checker parity with C++'s pointer-based traversal).
            let Some(mut script_list) = self.side_script_lists[i].take() else {
                continue;
            };

            // Execute root scripts (not in a group).
            if let Some(script_head) = script_list.first_script.as_deref_mut() {
                self.execute_scripts(
                    script_head,
                    &mut condition_evaluator,
                    &mut action_dispatcher,
                )?;
            }

            // Execute active non-subroutine groups.
            let mut group_opt = script_list.first_group.as_deref_mut();
            while let Some(group) = group_opt {
                if group.is_group_active && !group.is_group_subroutine {
                    if let Some(script_head) = group.first_script.as_deref_mut() {
                        self.execute_scripts(
                            script_head,
                            &mut condition_evaluator,
                            &mut action_dispatcher,
                        )?;
                    }
                }
                group_opt = group.next_group.as_deref_mut();
            }

            self.side_script_lists[i] = Some(script_list);
        }

        self.current_player = None;
        Ok(())
    }

    fn execute_scripts(
        &mut self,
        script_head: &mut Script,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<()> {
        let mut cur: Option<&mut Script> = Some(script_head);
        while let Some(script) = cur {
            if !script.is_subroutine {
                self.execute_script(script, condition_evaluator, action_dispatcher)?;
            }
            cur = script.next_script.as_deref_mut();
        }
        Ok(())
    }

    fn initialize_script_runtime_fields_in_list(&self, script_list: &mut ScriptList) {
        if let Some(script_head) = script_list.first_script.as_deref_mut() {
            self.initialize_script_runtime_fields_in_chain(script_head);
        }

        let mut group_opt = script_list.first_group.as_deref_mut();
        while let Some(group) = group_opt {
            if let Some(script_head) = group.first_script.as_deref_mut() {
                self.initialize_script_runtime_fields_in_chain(script_head);
            }
            group_opt = group.next_group.as_deref_mut();
        }
    }

    fn initialize_script_runtime_fields_in_chain(&self, script_head: &mut Script) {
        let mut current = Some(script_head);
        while let Some(script) = current {
            self.initialize_script_runtime_fields(script);
            current = script.next_script.as_deref_mut();
        }
    }

    fn initialize_script_runtime_fields(&self, script: &mut Script) {
        self.initialize_script_evaluation_frame(script);
        self.infer_script_condition_team_name(script);
    }

    fn initialize_script_evaluation_frame(&self, script: &mut Script) {
        if script.delay_evaluation_seconds > 0 {
            let max_offset = (2 * LOGICFRAMES_PER_SECOND as i32).max(0);
            let random_offset = crate::helpers::get_game_logic_random_value(0, max_offset).max(0);
            script.frame_to_evaluate_at = random_offset as u32;
        } else {
            script.frame_to_evaluate_at = 0;
        }
    }

    fn infer_script_condition_team_name(&self, script: &mut Script) {
        let mut singleton_team_name = String::new();
        let mut multi_team_name = String::new();
        let script_name = script.script_name.clone();

        let mut or_condition = script.condition.as_deref();
        while let Some(or_node) = or_condition {
            let mut and_condition = or_node.first_and.as_deref();
            while let Some(condition) = and_condition {
                for index in 0..condition.get_num_parameters() {
                    let Some(param) = condition.get_parameter(index) else {
                        continue;
                    };
                    if param.get_parameter_type() != ParameterType::Team {
                        continue;
                    }

                    let team_name = param.get_string().trim();
                    if team_name.is_empty() {
                        continue;
                    }

                    let Some(prototype) = get_team_factory()
                        .lock()
                        .ok()
                        .and_then(|factory| factory.find_team_prototype(team_name))
                    else {
                        continue;
                    };

                    let is_singleton =
                        prototype.is_singleton() || prototype.get_max_instances() < 2;
                    if is_singleton {
                        singleton_team_name = team_name.to_string();
                    } else if multi_team_name.is_empty() {
                        multi_team_name = team_name.to_string();
                    } else if multi_team_name != team_name {
                        log::warn!(
                            "Script '{}' contains multiple non-singleton team conditions: '{}' and '{}'",
                            script_name,
                            multi_team_name,
                            team_name
                        );
                    }
                }
                and_condition = condition.get_next();
            }
            or_condition = or_node.get_next_or_condition();
        }

        if !multi_team_name.is_empty() {
            script.condition_team_name = multi_team_name;
        } else if !singleton_team_name.is_empty() {
            script.condition_team_name = singleton_team_name;
        }
    }

    /// Execute a single script, matching C++ `ScriptEngine::executeScript`.
    fn execute_script(
        &mut self,
        script: &mut Script,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<()> {
        // If script is not active, return.
        if !script.is_active {
            return Ok(());
        }

        // Difficulty gating (C++ uses `m_currentPlayer->getPlayerDifficulty()` when available).
        let difficulty = self
            .current_player
            .as_deref()
            .and_then(|name| {
                crate::player::player_list()
                    .read()
                    .ok()
                    .and_then(|list| list.find_player_by_name(name))
                    .and_then(|p| p.read().ok().map(|p| p.get_player_difficulty()))
            })
            .unwrap_or(crate::player::GameDifficulty::Normal);

        match difficulty {
            crate::player::GameDifficulty::Easy if !script.easy => return Ok(()),
            crate::player::GameDifficulty::Normal if !script.normal => return Ok(()),
            crate::player::GameDifficulty::Hard | crate::player::GameDifficulty::Brutal
                if !script.hard =>
            {
                return Ok(());
            }
            _ => {}
        }

        // Periodic evaluation gate.
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        if current_frame < script.frame_to_evaluate_at {
            return Ok(());
        }

        // If delay is configured, schedule the next evaluation time.
        if script.delay_evaluation_seconds > 0 {
            script.frame_to_evaluate_at = current_frame
                + (script.delay_evaluation_seconds as u32) * (LOGICFRAMES_PER_SECOND as u32);
        }

        // Team-scoped condition evaluation (C++ uses `conditionTeamName` to iterate instances).
        let saved_condition_team = self.condition_team.take();

        let condition_team_name = script.condition_team_name.trim().to_string();
        if !condition_team_name.is_empty() {
            let instances = crate::team::get_team_factory()
                .lock()
                .ok()
                .map(|factory| factory.find_team_instances(&condition_team_name))
                .unwrap_or_default();

            if !instances.is_empty() {
                for team_arc in instances {
                    let team_name = team_arc
                        .read()
                        .ok()
                        .map(|t| t.get_name().to_string())
                        .unwrap_or_else(|| condition_team_name.clone());
                    self.condition_team = Some(team_name);
                    self.evaluate_and_execute_script(
                        script,
                        condition_evaluator,
                        action_dispatcher,
                        false,
                    )?;
                }
                self.condition_team = saved_condition_team;
                return Ok(());
            }
        }

        self.condition_team = None;
        self.evaluate_and_execute_script(script, condition_evaluator, action_dispatcher, true)?;
        self.condition_team = saved_condition_team;
        Ok(())
    }

    fn evaluate_and_execute_script(
        &mut self,
        script: &mut Script,
        condition_evaluator: &mut crate::scripting::executor::ScriptConditionEvaluator,
        action_dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
        deactivate_one_shot_on_false_action: bool,
    ) -> GameLogicResult<()> {
        // If no conditions, C++ treats as false (no AND chain).
        let mut condition_true = false;
        if let Some(or_cond) = script.condition.as_deref_mut() {
            condition_true = condition_evaluator
                .evaluate_or_condition(or_cond)
                .map_err(|e| {
                    GameLogicError::Configuration(format!("Script condition error: {}", e))
                })?;
        }

        if condition_true {
            let mut action_state = ActionChainExecution::Completed;
            if let Some(action_head) = script.action.as_deref() {
                action_state = self.execute_action_chain(action_head, action_dispatcher)?;
            }
            match action_state {
                ActionChainExecution::Completed => {
                    if script.is_one_shot {
                        script.is_active = false;
                    }
                }
                ActionChainExecution::Pending(frames) => {
                    self.schedule_script_pending_frames(script, frames);
                }
            }
        } else if let Some(false_action) = script.action_false.as_deref() {
            match self.execute_action_chain(false_action, action_dispatcher)? {
                ActionChainExecution::Completed => {
                    if script.is_one_shot && deactivate_one_shot_on_false_action {
                        script.is_active = false;
                    }
                }
                ActionChainExecution::Pending(frames) => {
                    self.schedule_script_pending_frames(script, frames);
                }
            }
        }

        Ok(())
    }

    fn execute_action_chain(
        &mut self,
        action_head: &ScriptAction,
        dispatcher: &mut crate::scripting::executor::ScriptActionDispatcher,
    ) -> GameLogicResult<ActionChainExecution> {
        let mut cur: Option<&ScriptAction> = Some(action_head);
        while let Some(action) = cur {
            let result = dispatcher.execute_action(action).map_err(|e| {
                GameLogicError::Configuration(format!("Script action error: {}", e))
            })?;
            match result {
                crate::scripting::executor::ScriptActionResult::Success => {}
                crate::scripting::executor::ScriptActionResult::Pending(frames) => {
                    if Self::pending_is_sequential_only_action(action.action_type) {
                        // C++ parity: these actions are implemented as sequential timers/checks and
                        // should not pause standard script action chains.
                        cur = action.next_action.as_deref();
                        continue;
                    }
                    return Ok(ActionChainExecution::Pending(frames));
                }
                crate::scripting::executor::ScriptActionResult::Failed(msg) => {
                    return Err(GameLogicError::Configuration(format!(
                        "Script action failed: {}",
                        msg
                    )));
                }
            }
            cur = action.next_action.as_deref();
        }
        Ok(ActionChainExecution::Completed)
    }

    fn schedule_script_pending_frames(&self, script: &mut Script, pending_frames: f32) {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let pending_resume_frame = Self::pending_resume_frame(current_frame, pending_frames);
        script.frame_to_evaluate_at = script.frame_to_evaluate_at.max(pending_resume_frame);
    }

    fn pending_resume_frame(current_frame: u32, pending_frames: f32) -> u32 {
        let wait_frames = pending_frames.max(1.0).ceil() as u32;
        current_frame.saturating_add(wait_frames)
    }

    fn pending_repeats_current_sequential_instruction(action_type: ScriptActionType) -> bool {
        matches!(
            action_type,
            ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll
                | ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial
                | ScriptActionType::TeamWaitForNotContainedAll
                | ScriptActionType::TeamWaitForNotContainedPartial
        )
    }

    fn pending_is_sequential_only_action(action_type: ScriptActionType) -> bool {
        Self::pending_repeats_current_sequential_instruction(action_type)
            || matches!(
                action_type,
                ScriptActionType::TeamGuardForFramecount
                    | ScriptActionType::TeamIdleForFramecount
                    | ScriptActionType::TeamSpinForFramecount
                    | ScriptActionType::UnitGuardForFramecount
                    | ScriptActionType::UnitIdleForFramecount
            )
    }

    fn pending_to_sequential_wait_frames(
        pending_frames: f32,
        repeat_current_instruction: bool,
    ) -> i32 {
        let wait_frames = pending_frames.max(0.0).ceil() as i32;
        if repeat_current_instruction {
            wait_frames.saturating_sub(1)
        } else {
            wait_frames.max(0)
        }
    }

    /// Update the victory condition manager with the current context
    pub fn update_victory_manager(
        &self,
        _context: crate::scripting::ScriptContext,
    ) -> GameLogicResult<()> {
        Ok(())
    }

    /// Update fade effects
    fn update_fades(&mut self) {
        if self.fade == TFade::None {
            return;
        }

        self.cur_fade_frame += 1;
        let mut fade = self.cur_fade_frame;

        if fade <= self.fade_frames_increase {
            let factor = self.cur_fade_frame as f32 / self.fade_frames_increase as f32;
            self.cur_fade_value = self.min_fade + factor * (self.max_fade - self.min_fade);
            return;
        }

        fade -= self.fade_frames_increase;
        if fade <= self.fade_frames_hold {
            self.cur_fade_value = self.max_fade;
            return;
        }

        fade -= self.fade_frames_hold;
        if fade <= self.fade_frames_decrease {
            let mut divisor = self.fade_frames_decrease + 1;
            if divisor == 0 {
                divisor = 1;
            }
            let factor = fade as f32 / divisor as f32;
            self.cur_fade_value = self.max_fade + factor * (self.min_fade - self.max_fade);
            return;
        }

        self.fade = TFade::None;
    }

    /// Evaluate and progress sequential scripts
    fn evaluate_and_progress_all_sequential_scripts(&mut self) -> GameLogicResult<()> {
        let saved_current_player = self.current_player.clone();
        let saved_condition_team = self.condition_team.clone();
        let saved_condition_object = self.condition_object;

        let result = (|| -> GameLogicResult<()> {
            let current_frame = crate::helpers::TheGameLogic::get_frame();
            let exec_context = Arc::new(RwLock::new(crate::scripting::executor::ScriptContext {
                game_logic_id: 0,
                object_manager_id: 0,
                player_manager_id: 0,
                event_system_id: 0,
                camera_system_id: 0,
                audio_system_id: 0,
                partition_manager_id: 0,
                special_powers_id: 0,
                current_frame,
                suppress_new_windows: false,
            }));
            let mut dispatcher =
                crate::scripting::executor::ScriptActionDispatcher::new(exec_context);

            let mut i: usize = 0;
            let mut last_i: Option<usize> = None;
            let mut spin_count: i32 = 0;

            while i < self.sequential_scripts.len() {
                if last_i == Some(i) {
                    spin_count += 1;
                } else {
                    spin_count = 0;
                }
                last_i = Some(i);

                if spin_count > MAX_SEQUENTIAL_SPIN_COUNT {
                    if let Some(seq_name) = self.sequential_scripts[i]
                        .script_to_execute_sequentially
                        .as_ref()
                        .map(|s| s.script_name.clone())
                    {
                        log::warn!(
                            "Sequential script '{}' appears to be in an infinite loop",
                            seq_name
                        );
                    }
                    i += 1;
                    continue;
                }

                if self.sequential_scripts[i]
                    .script_to_execute_sequentially
                    .is_none()
                {
                    self.cleanup_sequential_script_at(i, false);
                    continue;
                }

                let mut it_advanced = false;
                let team_name = self.sequential_scripts[i].team_to_exec_on.clone();
                let object_id = self.sequential_scripts[i].object_id;

                let team_arc = team_name.as_ref().and_then(|name| {
                    get_team_factory()
                        .lock()
                        .ok()
                        .and_then(|mut factory| factory.find_team(name))
                });
                let object_arc = if object_id != INVALID_ID {
                    TheGameLogic::find_object_by_id(object_id)
                } else {
                    None
                };

                if object_arc.is_none() && team_arc.is_none() {
                    self.cleanup_sequential_script_at(i, false);
                    continue;
                }

                self.current_player =
                    self.resolve_sequential_current_player(object_arc.as_ref(), team_arc.as_ref());

                let (obj_has_ai, obj_idle, _) = object_arc
                    .as_ref()
                    .map(Self::object_ai_status)
                    .unwrap_or((false, false, false));
                let (team_has_group, team_idle, _) = team_arc
                    .as_ref()
                    .map(|team| {
                        let (idle, dead) = Self::team_ai_status(team);
                        (true, idle, dead)
                    })
                    .unwrap_or((false, false, false));

                if obj_has_ai || team_has_group {
                    let frames_to_wait = self.sequential_scripts[i].frames_to_wait;
                    let should_progress = (((obj_has_ai && obj_idle)
                        || (team_has_group && team_idle))
                        && frames_to_wait < 1)
                        || (frames_to_wait == 0);

                    if should_progress {
                        if self.sequential_scripts[i].dont_advance_instruction {
                            self.sequential_scripts[i].dont_advance_instruction = false;
                        } else {
                            self.sequential_scripts[i].current_instruction += 1;
                        }

                        let instruction = self.sequential_scripts[i].current_instruction;
                        let action = Self::script_action_at_instruction(
                            &self.sequential_scripts[i],
                            instruction,
                        );

                        if let Some(action) = action {
                            self.condition_team = team_name;
                            self.condition_object = object_arc.as_ref().map(|_| object_id);
                            self.sequential_scripts[i].frames_to_wait = -1;

                            let result = dispatcher.execute_action(&action).map_err(|e| {
                                GameLogicError::Configuration(format!(
                                    "Sequential script action error: {}",
                                    e
                                ))
                            })?;

                            match result {
                                crate::scripting::executor::ScriptActionResult::Success => {}
                                crate::scripting::executor::ScriptActionResult::Pending(frames) => {
                                    let repeats_instruction =
                                        Self::pending_repeats_current_sequential_instruction(
                                            action.action_type,
                                        );
                                    let wait_frames = Self::pending_to_sequential_wait_frames(
                                        frames,
                                        repeats_instruction,
                                    );
                                    self.sequential_scripts[i].dont_advance_instruction =
                                        repeats_instruction;
                                    self.sequential_scripts[i].frames_to_wait = wait_frames;
                                }
                                crate::scripting::executor::ScriptActionResult::Failed(msg) => {
                                    return Err(GameLogicError::Configuration(format!(
                                        "Sequential script action failed: {}",
                                        msg
                                    )));
                                }
                            }

                            if self.sequential_scripts[i].dont_advance_instruction {
                                i += 1;
                                let _it_advanced = true;
                                continue;
                            }

                            let obj_idle_now = object_arc
                                .as_ref()
                                .map(|obj| Self::object_ai_status(obj).1)
                                .unwrap_or(false);
                            let team_idle_now = team_arc
                                .as_ref()
                                .map(|team| Self::team_ai_status(team).0)
                                .unwrap_or(false);

                            if (obj_has_ai && obj_idle_now) || (team_has_group && team_idle_now) {
                                it_advanced = true;
                            }

                            if it_advanced {
                                let obj_dead_now = object_arc
                                    .as_ref()
                                    .map(|obj| Self::object_ai_status(obj).2)
                                    .unwrap_or(false);
                                let team_dead_now = team_arc
                                    .as_ref()
                                    .map(|team| Self::team_ai_status(team).1)
                                    .unwrap_or(false);

                                if obj_dead_now || team_dead_now {
                                    self.cleanup_sequential_script_at(i, true);
                                    continue;
                                }
                            }
                        } else {
                            let times_to_loop = self.sequential_scripts[i].times_to_loop;
                            if times_to_loop != 0 {
                                let mut loop_script = self.sequential_scripts[i].clone();
                                if loop_script.times_to_loop != -1 {
                                    loop_script.times_to_loop -= 1;
                                }
                                loop_script.frames_to_wait = -1;
                                self.append_sequential_script(loop_script);
                            }
                            self.cleanup_sequential_script_at(i, false);
                            it_advanced = true;
                        }
                    } else if self.sequential_scripts[i].frames_to_wait > 0 {
                        self.sequential_scripts[i].frames_to_wait -= 1;
                    }
                }

                if !it_advanced {
                    i += 1;
                }
            }

            Ok(())
        })();

        self.current_player = saved_current_player;
        self.condition_team = saved_condition_team;
        self.condition_object = saved_condition_object;

        result
    }

    fn script_action_at_instruction(
        script: &SequentialScript,
        instruction: i32,
    ) -> Option<ScriptAction> {
        if instruction < 0 {
            return None;
        }

        let mut action = script
            .script_to_execute_sequentially
            .as_ref()
            .and_then(|seq| seq.action.as_deref());
        let mut remaining = instruction;
        while remaining > 0 {
            action = action.and_then(|node| node.get_next());
            remaining -= 1;
        }
        action.cloned()
    }

    fn object_ai_status(object_arc: &Arc<RwLock<crate::object::Object>>) -> (bool, bool, bool) {
        let Ok(object) = object_arc.read() else {
            return (false, false, true);
        };
        let has_ai = object.get_ai_update_interface().is_some();
        let idle = object.is_idle();
        let dead = object.is_effectively_dead();
        (has_ai, idle, dead)
    }

    fn team_ai_status(team_arc: &Arc<RwLock<crate::team::Team>>) -> (bool, bool) {
        let Ok(team) = team_arc.read() else {
            return (false, true);
        };

        let idle = team.is_idle();
        let mut all_dead = true;
        for &member_id in team.get_members() {
            let Some(object_arc) = TheGameLogic::find_object_by_id(member_id) else {
                continue;
            };
            let Ok(object) = object_arc.read() else {
                continue;
            };
            if !object.is_effectively_dead() {
                all_dead = false;
                break;
            }
        }

        (idle, all_dead)
    }

    fn resolve_sequential_current_player(
        &self,
        object_arc: Option<&Arc<RwLock<crate::object::Object>>>,
        team_arc: Option<&Arc<RwLock<crate::team::Team>>>,
    ) -> Option<String> {
        let player_id = if let Some(object_arc) = object_arc {
            object_arc
                .read()
                .ok()
                .and_then(|object| object.get_controlling_player_id())
        } else if let Some(team_arc) = team_arc {
            team_arc
                .read()
                .ok()
                .and_then(|team| team.get_controlling_player_id())
        } else {
            None
        }?;

        crate::player::player_list()
            .read()
            .ok()
            .and_then(|list| list.get_player(player_id as i32).cloned())
            .and_then(|player| {
                player.read().ok().and_then(|player| {
                    if player.is_skirmish_ai() {
                        NameKeyGenerator::key_to_name(player.get_player_name_key())
                    } else {
                        None
                    }
                })
            })
    }

    fn cleanup_sequential_script_at(&mut self, index: usize, clean_danglers: bool) {
        if index >= self.sequential_scripts.len() {
            return;
        }

        if clean_danglers {
            self.sequential_scripts.remove(index);
            return;
        }

        let next = self.sequential_scripts[index]
            .next_script_in_sequence
            .take();
        if let Some(next_script) = next {
            self.sequential_scripts[index] = *next_script;
        } else {
            self.sequential_scripts.remove(index);
        }
    }

    /// Allocate a counter
    pub fn allocate_counter(&mut self, name: &str) -> GameLogicResult<usize> {
        // Check if counter already exists
        for (i, counter) in self.counters.iter().enumerate() {
            if let Some(counter) = counter {
                if counter.name == name {
                    return Ok(i);
                }
            }
        }

        // Find empty slot
        for i in 0..MAX_COUNTERS {
            if self.counters[i].is_none() {
                self.counters[i] = Some(TCounter::new(name.to_string()));
                if i >= self.num_counters {
                    self.num_counters = i + 1;
                }
                return Ok(i);
            }
        }

        Err(GameLogicError::Configuration(
            "Maximum counters exceeded".to_string(),
        ))
    }

    /// Allocate a flag
    pub fn allocate_flag(&mut self, name: &str) -> GameLogicResult<usize> {
        // Check if flag already exists
        for (i, flag) in self.flags.iter().enumerate() {
            if let Some(flag) = flag {
                if flag.name == name {
                    return Ok(i);
                }
            }
        }

        // Find empty slot
        for i in 0..MAX_FLAGS {
            if self.flags[i].is_none() {
                self.flags[i] = Some(TFlag::new(name.to_string()));
                if i >= self.num_flags {
                    self.num_flags = i + 1;
                }
                return Ok(i);
            }
        }

        Err(GameLogicError::Configuration(
            "Maximum flags exceeded".to_string(),
        ))
    }

    /// Get counter by name
    pub fn get_counter(&self, name: &str) -> Option<&TCounter> {
        for counter in &self.counters {
            if let Some(counter) = counter {
                if counter.name == name {
                    return Some(counter);
                }
            }
        }
        None
    }

    /// Get flag by name
    pub fn get_flag(&self, name: &str) -> Option<&TFlag> {
        for flag in &self.flags {
            if let Some(flag) = flag {
                if flag.name == name {
                    return Some(flag);
                }
            }
        }
        None
    }

    /// Set counter value
    pub fn set_counter(&mut self, name: &str, value: i32) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            counter.value = value;
        }
        Ok(())
    }

    /// Set flag value
    pub fn set_flag(&mut self, name: &str, value: bool) -> GameLogicResult<()> {
        let index = self.allocate_flag(name)?;
        if let Some(flag) = &mut self.flags[index] {
            flag.value = value;
        }
        Ok(())
    }

    /// Increment counter value
    pub fn increment_counter(&mut self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            counter.value = counter.value.saturating_add(1);
        }
        Ok(())
    }

    /// Decrement counter value
    pub fn decrement_counter(&mut self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            counter.value = counter.value.saturating_sub(1);
        }
        Ok(())
    }

    /// Set timer (countdown counter) in frames (1 second = 30 frames at standard logic rate)
    /// C++ Reference: ScriptActions::doSetTimer() - timers count down each frame
    pub fn set_timer(&mut self, name: &str, frames: i32) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            counter.value = frames;
            counter.is_countdown_timer = true;
        }
        Ok(())
    }

    /// Set timer in seconds (converts to frames at logic frame rate)
    pub fn set_timer_seconds(&mut self, name: &str, seconds: f32) -> GameLogicResult<()> {
        let frames = (seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
        self.set_timer(name, frames)
    }

    /// C++ `SET_MILLISECOND_TIMER` script actions actually pass a real-valued second duration
    /// through the mission/script layer and then ceil the converted frame count.
    fn frames_from_millisecond_script_seconds(seconds: f32) -> i32 {
        (seconds.max(0.0) * LOGICFRAMES_PER_SECOND as f32).ceil() as i32
    }

    /// Set timer using the legacy script "msec" path semantics from C++.
    pub fn set_timer_millisecond_script_seconds(
        &mut self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        let frames = Self::frames_from_millisecond_script_seconds(seconds);
        self.set_timer(name, frames)
    }

    /// Stop/pause a timer without clearing its remaining value.
    pub fn stop_timer(&mut self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            counter.is_countdown_timer = false;
        }
        Ok(())
    }

    /// Restart a timer (reset to its original value - keeps is_countdown_timer=true)
    /// Note: Without storing original value, this just re-enables countdown at current value
    pub fn restart_timer(&mut self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            counter.is_countdown_timer = true;
        }
        Ok(())
    }

    /// Add legacy script "msec" seconds to timer.
    pub fn add_to_timer_millisecond_script_seconds(
        &mut self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        let frames = Self::frames_from_millisecond_script_seconds(seconds);
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            counter.value += frames;
        }
        Ok(())
    }

    /// Subtract legacy script "msec" seconds from timer.
    pub fn subtract_from_timer_millisecond_script_seconds(
        &mut self,
        name: &str,
        seconds: f32,
    ) -> GameLogicResult<()> {
        let frames = Self::frames_from_millisecond_script_seconds(seconds);
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            counter.value -= frames;
        }
        Ok(())
    }

    /// Start end game timer
    pub fn start_end_game_timer(&mut self) {
        self.end_game_timer = 300; // 5 seconds at 60fps
        log::info!("End game timer started");
    }

    /// Start quick end game timer
    pub fn start_quick_end_game_timer(&mut self) {
        self.end_game_timer = 60; // 1 second at 60fps
        log::info!("Quick end game timer started");
    }

    /// Start close window timer
    pub fn start_close_window_timer(&mut self) {
        self.close_window_timer = 180; // 3 seconds at 60fps
        log::info!("Close window timer started");
    }

    /// Set whether the multiplayer local defeat window has been shown.
    pub fn set_shown_mp_local_defeat_window(&mut self, shown: bool) {
        self.shown_mp_local_defeat_window = shown;
    }

    /// Return whether the multiplayer local defeat window has been shown.
    pub fn has_shown_mp_local_defeat_window(&self) -> bool {
        self.shown_mp_local_defeat_window
    }

    /// Check if game is ending
    pub fn is_game_ending(&self) -> bool {
        self.end_game_timer >= 0
    }

    /// Freeze time
    pub fn do_freeze_time(&mut self) {
        self.freeze_by_script = true;
        log::info!("Time frozen by script");
    }

    /// Unfreeze time
    pub fn do_unfreeze_time(&mut self) {
        self.freeze_by_script = false;
        log::info!("Time unfrozen by script");
    }

    /// Check if time is frozen by script
    pub fn is_time_frozen_script(&self) -> bool {
        self.freeze_by_script
    }

    /// Set debug freeze state.
    pub fn set_time_frozen_debug(&mut self, frozen: bool) {
        self.freeze_by_debug = frozen;
    }

    /// Check if time is frozen by debug controls.
    pub fn is_time_frozen_debug(&self) -> bool {
        self.freeze_by_debug
    }

    /// Check if time is frozen by any mechanism (script or debug).
    ///
    /// ## C++ Reference: GameLogic.cpp lines 3603-3604
    /// C++: `freezeTime = TheTacticalView->isTimeFrozen() ||
    ///        TheScriptEngine->isTimeFrozenDebug() ||
    ///        TheScriptEngine->isTimeFrozenScript();`
    pub fn is_time_frozen(&self) -> bool {
        self.freeze_by_debug || self.freeze_by_script
    }

    /// Get breeze info
    pub fn get_breeze_info(&self) -> &BreezeInfo {
        &self.breeze_info
    }

    /// Turn off breeze
    pub fn turn_breeze_off(&mut self) {
        self.breeze_info.intensity = 0.0;
    }

    /// Mirrors C++ ScriptEngine::setSway.
    pub fn set_breeze_info(
        &mut self,
        direction: f32,
        intensity: f32,
        lean: f32,
        breeze_period: i32,
        randomness: f32,
    ) {
        self.breeze_info.breeze_version = self.breeze_info.breeze_version.wrapping_add(1);
        self.breeze_info.direction = direction;
        self.breeze_info.direction_vec[0] = direction.sin();
        self.breeze_info.direction_vec[1] = direction.cos();
        self.breeze_info.intensity = intensity;
        self.breeze_info.lean = lean;
        self.breeze_info.breeze_period = breeze_period.max(1).clamp(1, i16::MAX as i32) as i16;
        self.breeze_info.randomness = randomness;
    }

    /// Mirrors C++ ScriptEngine::setFade.
    pub fn set_fade_parameters(
        &mut self,
        fade: TFade,
        min_fade: f32,
        max_fade: f32,
        fade_frames_increase: i32,
        fade_frames_hold: i32,
        fade_frames_decrease: i32,
    ) {
        self.fade = fade;
        self.cur_fade_frame = 0;
        self.min_fade = min_fade;
        self.max_fade = max_fade;
        self.fade_frames_increase = fade_frames_increase;
        self.fade_frames_hold = fade_frames_hold;
        self.fade_frames_decrease = fade_frames_decrease;
        self.cur_fade_value = self.min_fade;

        if self.fade_frames_increase == 0 {
            self.update_fades();
        }
    }

    /// Get fade type
    pub fn get_fade(&self) -> TFade {
        self.fade
    }

    /// Get fade value
    pub fn get_fade_value(&self) -> f32 {
        self.cur_fade_value
    }

    /// Get current track name
    pub fn get_current_track_name(&self) -> &str {
        &self.current_track_name
    }

    /// Set current track name
    pub fn set_current_track_name(&mut self, name: String) {
        self.current_track_name = name;
    }

    pub fn set_global_difficulty(&mut self, difficulty: crate::player::GameDifficulty) {
        self.game_difficulty = difficulty;
    }

    pub fn get_global_difficulty(&self) -> crate::player::GameDifficulty {
        self.game_difficulty
    }

    pub fn set_objects_should_receive_difficulty_bonus(&mut self, enable: bool) {
        self.objects_should_receive_difficulty_bonus = enable;
        for obj in OBJECT_REGISTRY.get_all_objects() {
            if let Ok(mut guard) = obj.write() {
                guard.set_receiving_difficulty_bonus(enable);
            }
        }
    }

    pub fn set_choose_victim_always_uses_normal(&mut self, enable: bool) {
        self.choose_victim_always_uses_normal = enable;
    }

    pub fn get_choose_victim_always_uses_normal(&self) -> bool {
        self.choose_victim_always_uses_normal
    }

    /// Mirrors C++ `ScriptEngine::setEnableVTune`.
    ///
    /// C++ stores this as static script-engine runtime state; Rust keeps the same
    /// singleton-style behavior in `engine.rs` shared state.
    pub fn set_enable_vtune(&mut self, enabled: bool) {
        set_enable_vtune(enabled);
    }

    /// Mirrors C++ `ScriptEngine::getEnableVTune`.
    pub fn get_enable_vtune(&self) -> bool {
        get_enable_vtune()
    }

    /// Command-level parity hook for `TheSkateDistOverride` style debug state.
    pub fn set_skate_distance_override(&mut self, value: f32) {
        set_skate_distance_override(value);
    }

    /// Command-level parity hook for `TheSkateDistOverride` style debug state.
    pub fn adjust_skate_distance_override(&mut self, delta: f32) -> f32 {
        adjust_skate_distance_override(delta)
    }

    /// Command-level parity hook for `TheSkateDistOverride` style debug state.
    pub fn get_skate_distance_override(&self) -> f32 {
        get_skate_distance_override()
    }

    /// Get action template
    pub fn get_action_template(&self, index: usize) -> Option<&ActionTemplate> {
        self.action_templates.get(index)
    }

    /// Get condition template
    pub fn get_condition_template(&self, index: usize) -> Option<&ConditionTemplate> {
        self.condition_templates.get(index)
    }

    pub fn find_condition_type_by_name_key(&self, name_key: u32) -> Option<ConditionType> {
        self.condition_templates
            .iter()
            .enumerate()
            .find_map(|(idx, template)| {
                if template.base.internal_name_key == name_key {
                    ConditionType::from_u32(idx as u32)
                } else {
                    None
                }
            })
    }

    pub fn find_action_type_by_name_key(&self, name_key: u32) -> Option<ScriptActionType> {
        self.action_templates
            .iter()
            .enumerate()
            .find_map(|(idx, template)| {
                if template.base.internal_name_key == name_key {
                    ScriptActionType::from_u32(idx as u32)
                } else {
                    None
                }
            })
    }

    /// Append sequential script
    pub fn append_sequential_script(&mut self, mut script: SequentialScript) {
        script.next_script_in_sequence = None;
        script.current_instruction = -1;

        let target_object = script.object_id;
        let target_team = script.team_to_exec_on.clone();

        for existing in &mut self.sequential_scripts {
            let object_match = target_object != INVALID_ID && existing.object_id == target_object;
            let team_match = target_team.is_some() && existing.team_to_exec_on == target_team;
            if !(object_match || team_match) {
                continue;
            }

            let mut cursor = &mut existing.next_script_in_sequence;
            loop {
                match cursor {
                    Some(next) => {
                        cursor = &mut next.next_script_in_sequence;
                    }
                    None => {
                        *cursor = Some(Box::new(script));
                        break;
                    }
                }
            }
            return;
        }

        self.sequential_scripts.push(script);
    }

    /// Remove all sequential scripts bound to a specific object.
    pub fn remove_all_sequential_scripts_for_object(&mut self, object_id: ObjectID) {
        self.sequential_scripts
            .retain(|script| script.object_id != object_id);
    }

    /// Check if a specific object has any active sequential scripts running.
    /// PARITY_NOTE: C++ ScriptConditions does not have a case for UNIT_COMPLETED_SEQUENTIAL_EXECUTION
    /// in its evaluateCondition switch (hits default DEBUG_CRASH returning false). This provides
    /// the intended semantics: returns false if scripts are still active, true if none remain.
    pub fn has_active_sequential_script_for_object(&self, object_id: ObjectID) -> bool {
        self.sequential_scripts
            .iter()
            .any(|script| script.object_id == object_id)
    }

    /// Check if a specific team has any active sequential scripts running.
    pub fn has_active_sequential_script_for_team(&self, team_name: &str) -> bool {
        self.sequential_scripts
            .iter()
            .any(|script| script.team_to_exec_on.as_deref() == Some(team_name))
    }

    /// Remove all sequential scripts bound to a specific team.
    pub fn remove_all_sequential_scripts_for_team(&mut self, team_name: &str) {
        self.sequential_scripts
            .retain(|script| script.team_to_exec_on.as_deref() != Some(team_name));
    }

    /// Set frame wait timer for all sequential scripts running on an object.
    pub fn set_sequential_timer_for_object(&mut self, object_id: ObjectID, frame_count: i32) {
        for script in &mut self.sequential_scripts {
            if script.object_id == object_id {
                script.frames_to_wait = frame_count;
                return;
            }
        }
    }

    /// Set frame wait timer for all sequential scripts running on a team.
    pub fn set_sequential_timer_for_team(&mut self, team_name: &str, frame_count: i32) {
        for script in &mut self.sequential_scripts {
            if script.team_to_exec_on.as_deref() == Some(team_name) {
                script.frames_to_wait = frame_count;
                return;
            }
        }
    }

    /// Notify of completed video
    pub fn notify_of_completed_video(&mut self, video_name: &str) {
        self.completed_video.push(video_name.to_string());
        log::debug!("Video completed: {}", video_name);
    }

    /// Notify the script engine that a special power was triggered.
    pub fn notify_of_triggered_special_power(
        &mut self,
        player_index: usize,
        power_name: &str,
        source_obj: ObjectId,
    ) {
        if player_index >= Self::MAX_PLAYER_COUNT {
            log::warn!(
                "notify_of_triggered_special_power: player index {} out of range",
                player_index
            );
            return;
        }
        self.triggered_special_powers[player_index].push((power_name.to_string(), source_obj));
    }

    /// Notify the script engine that a special power reached its midway trigger.
    pub fn notify_of_midway_special_power(
        &mut self,
        player_index: usize,
        power_name: &str,
        source_obj: ObjectId,
    ) {
        if player_index >= Self::MAX_PLAYER_COUNT {
            log::warn!(
                "notify_of_midway_special_power: player index {} out of range",
                player_index
            );
            return;
        }
        self.midway_special_powers[player_index].push((power_name.to_string(), source_obj));
    }

    /// Notify the script engine that a special power finished executing.
    pub fn notify_of_completed_special_power(
        &mut self,
        player_index: usize,
        power_name: &str,
        source_obj: ObjectId,
    ) {
        if player_index >= Self::MAX_PLAYER_COUNT {
            log::warn!(
                "notify_of_completed_special_power: player index {} out of range",
                player_index
            );
            return;
        }
        self.finished_special_powers[player_index].push((power_name.to_string(), source_obj));
    }

    /// Notify the script engine that an upgrade completed.
    pub fn notify_of_completed_upgrade(
        &mut self,
        player_index: usize,
        upgrade_name: &str,
        source_obj: ObjectId,
    ) {
        if player_index >= Self::MAX_PLAYER_COUNT {
            log::warn!(
                "notify_of_completed_upgrade: player index {} out of range",
                player_index
            );
            return;
        }
        self.completed_upgrades[player_index].push((upgrade_name.to_string(), source_obj));
    }

    fn is_named_event_in_list(
        list: &mut Vec<(String, ObjectId)>,
        event_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let matched_pos = list.iter().position(|(name, obj_id)| {
            if name != event_name {
                return false;
            }
            if source_obj == crate::common::INVALID_ID {
                return true;
            }
            *obj_id == source_obj
        });
        if let Some(pos) = matched_pos {
            if remove_from_list {
                list.remove(pos);
            }
            return true;
        }
        false
    }

    pub fn is_special_power_triggered(
        &mut self,
        player_index: usize,
        power_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let Some(list) = self.triggered_special_powers.get_mut(player_index) else {
            return false;
        };
        Self::is_named_event_in_list(list, power_name, remove_from_list, source_obj)
    }

    pub fn is_special_power_midway(
        &mut self,
        player_index: usize,
        power_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let Some(list) = self.midway_special_powers.get_mut(player_index) else {
            return false;
        };
        Self::is_named_event_in_list(list, power_name, remove_from_list, source_obj)
    }

    pub fn is_special_power_complete(
        &mut self,
        player_index: usize,
        power_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let Some(list) = self.finished_special_powers.get_mut(player_index) else {
            return false;
        };
        Self::is_named_event_in_list(list, power_name, remove_from_list, source_obj)
    }

    pub fn is_upgrade_complete(
        &mut self,
        player_index: usize,
        upgrade_name: &str,
        remove_from_list: bool,
        source_obj: ObjectId,
    ) -> bool {
        let Some(list) = self.completed_upgrades.get_mut(player_index) else {
            return false;
        };
        Self::is_named_event_in_list(list, upgrade_name, remove_from_list, source_obj)
    }

    /// Check if video is complete
    pub fn is_video_complete(&mut self, video_name: &str, remove_from_list: bool) -> bool {
        if let Some(pos) = self.completed_video.iter().position(|v| v == video_name) {
            if remove_from_list {
                self.completed_video.remove(pos);
            }
            true
        } else {
            false
        }
    }

    fn is_timed_audio_complete(
        list: &mut Vec<(String, u32)>,
        event_name: &str,
        remove_from_list: bool,
    ) -> bool {
        if event_name.trim().is_empty() {
            return false;
        }

        let position = if let Some(pos) = list.iter().position(|(name, _)| name == event_name) {
            pos
        } else {
            let audio_length_ms = TheAudio::get()
                .map(|audio| {
                    let event = crate::common::audio::AudioEventRts::new(event_name);
                    audio.get_audio_length_ms(&event)
                })
                .unwrap_or(0.0)
                .max(0.0);
            // C++ uses REAL_TO_UNSIGNEDINT(audioLength / MSEC_PER_LOGICFRAME_REAL): truncate.
            let frame_count = ((audio_length_ms / 1000.0) * LOGICFRAMES_PER_SECOND as f32) as u32;
            let completion_frame = TheGameLogic::get_frame().saturating_add(frame_count);
            list.push((event_name.to_string(), completion_frame));
            list.len() - 1
        };

        let current_frame = TheGameLogic::get_frame();
        let completed = current_frame >= list[position].1;
        if completed && remove_from_list {
            list.remove(position);
        }
        completed
    }

    pub fn is_speech_complete(&mut self, speech_name: &str, remove_from_list: bool) -> bool {
        Self::is_timed_audio_complete(&mut self.testing_speech, speech_name, remove_from_list)
    }

    pub fn is_audio_complete(&mut self, audio_name: &str, remove_from_list: bool) -> bool {
        Self::is_timed_audio_complete(&mut self.testing_audio, audio_name, remove_from_list)
    }

    /// Signal UI interaction
    pub fn signal_ui_interact(&mut self, hook_name: &str) {
        self.ui_interactions.push(hook_name.to_string());
        log::debug!("UI interaction: {}", hook_name);
    }

    /// Create named map reveal
    pub fn create_named_map_reveal(
        &mut self,
        reveal_name: &str,
        waypoint_name: &str,
        radius: f32,
        player_name: &str,
    ) {
        if self
            .named_reveals
            .iter()
            .any(|reveal| reveal.reveal_name == reveal_name)
        {
            log::warn!(
                "create_named_map_reveal: attempted to redefine named reveal '{}'",
                reveal_name
            );
            return;
        }

        let reveal = NamedReveal {
            reveal_name: reveal_name.to_string(),
            waypoint_name: waypoint_name.to_string(),
            radius_to_reveal: radius,
            player_name: player_name.to_string(),
        };
        self.named_reveals.push(reveal);
        log::debug!("Created named map reveal: {}", reveal_name);
    }

    /// Apply a named map reveal (matches C++ ScriptEngine::doNamedMapReveal).
    pub fn do_named_map_reveal(&self, reveal_name: &str) {
        let reveal = self
            .named_reveals
            .iter()
            .find(|entry| entry.reveal_name == reveal_name);
        let Some(reveal) = reveal else {
            return;
        };

        let waypoint_ascii = AsciiString::from(reveal.waypoint_name.as_str());
        let target = crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            });
        let Some(target) = target else {
            return;
        };

        let Ok(players) = crate::player::player_list().read() else {
            return;
        };
        let Some(player_arc) = players.find_player_by_name(&reveal.player_name) else {
            return;
        };
        let Ok(player) = player_arc.read() else {
            return;
        };
        let player_mask = player.get_player_mask().bits();

        let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
        if let Ok(mut shroud_mgr) = shroud_mgr.lock() {
            shroud_mgr.do_shroud_reveal(&target, reveal.radius_to_reveal, player_mask);
        }
    }

    /// Undo a named map reveal (matches C++ ScriptEngine::undoNamedMapReveal).
    pub fn undo_named_map_reveal(&self, reveal_name: &str) {
        let reveal = self
            .named_reveals
            .iter()
            .find(|entry| entry.reveal_name == reveal_name);
        let Some(reveal) = reveal else {
            return;
        };

        let waypoint_ascii = AsciiString::from(reveal.waypoint_name.as_str());
        let target = crate::terrain::get_terrain_logic()
            .read()
            .ok()
            .and_then(|terrain| {
                terrain
                    .get_waypoint_by_name(&waypoint_ascii)
                    .map(|w| *w.get_location())
            });
        let Some(target) = target else {
            return;
        };

        let Ok(players) = crate::player::player_list().read() else {
            return;
        };
        let Some(player_arc) = players.find_player_by_name(&reveal.player_name) else {
            return;
        };
        let Ok(player) = player_arc.read() else {
            return;
        };
        let player_mask = player.get_player_mask().bits();

        let shroud_mgr = crate::system::shroud_manager::get_shroud_manager();
        if let Ok(mut shroud_mgr) = shroud_mgr.lock() {
            shroud_mgr.undo_shroud_reveal(&target, reveal.radius_to_reveal, player_mask);
        }
    }

    /// Remove a named map reveal (matches C++ ScriptEngine::removeNamedMapReveal).
    pub fn remove_named_map_reveal(&mut self, reveal_name: &str) {
        if let Some(index) = self
            .named_reveals
            .iter()
            .position(|entry| entry.reveal_name == reveal_name)
        {
            self.named_reveals.remove(index);
        }
    }

    /// Set or clear a named topple direction for scripted objects.
    /// Matches C++ ScriptEngine::setToppleDirection.
    pub fn set_topple_direction(
        &mut self,
        object_name: &str,
        direction: Option<crate::common::Coord3D>,
    ) {
        if object_name.is_empty() {
            return;
        }

        if let Some(index) = self
            .topple_directions
            .iter()
            .position(|(name, _)| name == object_name)
        {
            if let Some(dir) = direction {
                self.topple_directions[index].1 = Coord3D::new(dir.x, dir.y, dir.z);
            } else {
                self.topple_directions.remove(index);
            }
            return;
        }

        if let Some(dir) = direction {
            self.topple_directions.insert(
                0,
                (object_name.to_string(), Coord3D::new(dir.x, dir.y, dir.z)),
            );
        }
    }

    /// Adjust a topple direction based on script overrides.
    /// Matches C++ ScriptEngine::adjustToppleDirection.
    pub fn adjust_topple_direction(
        &self,
        object: &crate::object::Object,
        direction: &mut crate::common::Coord3D,
    ) {
        let name = object.get_name();
        if name.is_empty() {
            return;
        }

        for (entry_name, entry_direction) in &self.topple_directions {
            if entry_name == name.as_str() {
                let mut new_dir = crate::common::Coord3D::new(
                    entry_direction.x,
                    entry_direction.y,
                    entry_direction.z,
                );
                if new_dir.length_squared() > 0.0 {
                    new_dir = new_dir.normalize();
                }
                *direction = new_dir;
                return;
            }
        }
    }

    /// Get statistics string
    #[cfg(feature = "script_profiling")]
    pub fn get_stats(&self) -> String {
        let avg_time = if self.stats.num_frames > 0.0 {
            self.stats.total_update_time / self.stats.num_frames
        } else {
            0.0
        };

        format!(
            "ScriptEngine Stats: Frames: {:.0}, Total Time: {:.6}s, Avg Time: {:.6}s, Max Time: {:.6}s, Current: {:.6}s",
            self.stats.num_frames,
            self.stats.total_update_time,
            avg_time,
            self.stats.max_update_time,
            self.stats.cur_update_time
        )
    }

    /// Get statistics (no profiling version)
    #[cfg(not(feature = "script_profiling"))]
    pub fn get_stats(&self) -> String {
        "ScriptEngine Stats: Profiling disabled".to_string()
    }

    pub fn set_action_handler(&mut self, handler: Option<Arc<dyn ScriptActionHandler>>) {
        self.action_handler = handler;
    }

    pub fn action_handler(&self) -> Option<Arc<dyn ScriptActionHandler>> {
        self.action_handler.clone()
    }
    pub fn notify_of_acquired_science(&mut self, player_index: usize, science: ScienceType) {
        if player_index < self.acquired_sciences.len() {
            self.acquired_sciences[player_index].push(science);
            log::debug!("Player {} acquired science: {:?}", player_index, science);
        }
    }

    /// Check if a science was acquired by a player (optionally remove the entry).
    pub fn is_science_acquired(
        &mut self,
        player_index: usize,
        science: ScienceType,
        remove: bool,
    ) -> bool {
        let Some(list) = self.acquired_sciences.get_mut(player_index) else {
            return false;
        };

        if let Some(pos) = list.iter().position(|s| *s == science) {
            if remove {
                list.remove(pos);
            }
            return true;
        }

        false
    }

    // =========================================================================
    // MISSING METHODS PORTED FROM C++ ScriptEngine
    // =========================================================================

    // PARITY_NOTE: C++ ScriptEngine::notifyOfObjectDestruction
    pub fn notify_of_object_destruction(&mut self, object_id: ObjectID) {
        let tracker = get_named_object_tracker();
        let name = tracker.get_object_name(object_id).ok().flatten();
        if let Some(name) = name {
            if !name.is_empty() {
                let _ = tracker.unregister_object(object_id);
            }
        }

        if self.condition_object == Some(object_id) {
            self.condition_object = None;
        }
        if self.calling_object == Some(object_id) {
            self.calling_object = None;
        }
    }

    // PARITY_NOTE: C++ ScriptEngine::notifyOfTeamDestruction
    pub fn notify_of_team_destruction(&mut self, team_name: &str) {
        if team_name.is_empty() {
            return;
        }

        self.remove_all_sequential_scripts_for_team(team_name);

        if self.calling_team.as_deref() == Some(team_name) {
            self.calling_team = None;
        }
        if self.condition_team.as_deref() == Some(team_name) {
            self.condition_team = None;
        }
    }

    // PARITY_NOTE: C++ ScriptEngine::forceUnfreezeTime
    pub fn force_unfreeze_time(&mut self) {}

    // PARITY_NOTE: C++ ScriptEngine::clearFlag
    pub fn clear_flag(&mut self, name: &str) {
        for j in 0..Self::MAX_PLAYER_COUNT {
            let mod_name = format!("{}{}", name, j);
            for i in 1..self.num_flags {
                if let Some(flag) = &mut self.flags[i] {
                    if flag.name == mod_name {
                        flag.value = false;
                    }
                }
            }
        }
    }

    // PARITY_NOTE: C++ ScriptEngine::clearTeamFlags
    pub fn clear_team_flags(&mut self) {
        self.clear_flag("USA Team is Building");
        self.clear_flag("USA Air Team Is Building");
        self.clear_flag("USA Inf Team Is Building");
        self.clear_flag("China Team is Building");
        self.clear_flag("China Air Team Is Building");
        self.clear_flag("China Inf Team Is Building");
        self.clear_flag("GLA Team is Building");
        self.clear_flag("GLA Inf Team Is Building");
    }

    // PARITY_NOTE: C++ ScriptEngine::didUnitExist
    pub fn did_unit_exist(&self, unit_name: &str) -> bool {
        let tracker = get_named_object_tracker();
        tracker.did_object_exist(unit_name).unwrap_or(false)
    }

    // PARITY_NOTE: C++ ScriptEngine::runScript
    pub fn run_script(&mut self, script_name: &str, team_name: Option<&str>) {
        if script_name.is_empty() || script_name == "<none>" {
            return;
        }

        let saved_current_player = self.current_player.clone();
        let saved_calling_team = self.calling_team.take();

        self.condition_team = None;
        self.current_player = None;

        if let Some(team_name_str) = team_name {
            self.calling_team = Some(team_name_str.to_string());
            if let Ok(mut factory) = get_team_factory().lock() {
                if let Some(team_arc) = factory.find_team(team_name_str) {
                    if let Ok(team_guard) = team_arc.read() {
                        if let Some(player_id) = team_guard.get_controlling_player_id() {
                            self.current_player =
                                crate::player::player_list().read().ok().and_then(|list| {
                                    list.get_player(player_id as i32).cloned()
                                }).and_then(|p| {
                                    p.read().ok().and_then(|p| {
                                        game_engine::common::name_key_generator::NameKeyGenerator::key_to_name(p.get_player_name_key())
                                    })
                                });
                        }
                    }
                }
            }
        }

        let _found = self
            .execute_subroutine_by_name(script_name)
            .unwrap_or(false);

        self.calling_team = saved_calling_team;
        self.current_player = saved_current_player;
    }

    // PARITY_NOTE: C++ ScriptEngine::runObjectScript
    pub fn run_object_script(&mut self, script_name: &str, object_id: ObjectID) {
        if script_name.is_empty() || script_name == "<none>" {
            return;
        }

        let saved_calling_object = self.calling_object;
        self.calling_object = Some(object_id);

        let _found = self
            .execute_subroutine_by_name(script_name)
            .unwrap_or(false);

        self.calling_object = saved_calling_object;
    }

    // PARITY_NOTE: C++ ScriptEngine::evaluateConditions
    pub fn evaluate_conditions(
        &mut self,
        script: &mut Script,
        team_name: Option<&str>,
        player_name: Option<&str>,
    ) -> bool {
        let saved_calling_team = self.calling_team.take();
        let saved_current_player = self.current_player.clone();

        self.calling_team = team_name.map(|s| s.to_string());

        if player_name.is_some() {
            self.current_player = player_name.map(|s| s.to_string());
        } else if let Some(ref tname) = self.calling_team {
            if let Ok(mut factory) = get_team_factory().lock() {
                if let Some(team_arc) = factory.find_team(tname) {
                    if let Ok(team_guard) = team_arc.read() {
                        if let Some(pid) = team_guard.get_controlling_player_id() {
                            self.current_player =
                                crate::player::player_list().read().ok().and_then(|list| {
                                    list.get_player(pid as i32).cloned()
                                }).and_then(|p| {
                                    p.read().ok().and_then(|p| {
                                        game_engine::common::name_key_generator::NameKeyGenerator::key_to_name(p.get_player_name_key())
                                    })
                                });
                        }
                    }
                }
            }
        }

        let result = if let Some(or_cond) = script.condition.as_deref_mut() {
            let mut test_value = false;
            let mut current_or = Some(or_cond);
            while let Some(or_node) = current_or {
                if let Some(and_cond) = or_node.first_and.as_deref_mut() {
                    let mut and_term = true;
                    let mut current_and: Option<&mut Condition> = Some(and_cond);
                    while let Some(cond) = current_and {
                        let cond_type = cond.get_condition_type();
                        let cond_result = match cond_type {
                            ConditionType::Counter => self.evaluate_counter_condition_inline(cond),
                            ConditionType::Flag => self.evaluate_flag_condition_inline(cond),
                            ConditionType::TimerExpired => {
                                self.evaluate_timer_condition_inline(cond)
                            }
                            ConditionType::ConditionTrue => true,
                            ConditionType::ConditionFalse => false,
                            _ => false,
                        };
                        if !cond_result {
                            and_term = false;
                            break;
                        }
                        current_and = cond.next_and_condition.as_deref_mut();
                    }
                    if and_term {
                        test_value = true;
                        break;
                    }
                }
                current_or = or_node.next_or.as_deref_mut();
            }
            test_value
        } else {
            false
        };

        self.calling_team = saved_calling_team;
        self.current_player = saved_current_player;
        result
    }

    fn evaluate_counter_condition_inline(&self, condition: &Condition) -> bool {
        let Some(param0) = condition.get_parameter(0) else {
            return false;
        };
        let Some(param1) = condition.get_parameter(1) else {
            return false;
        };
        let Some(param2) = condition.get_parameter(2) else {
            return false;
        };

        let counter_name = param0.get_string();
        let comparison = param1.get_int();
        let target_value = param2.get_int();
        let counter_value = self.get_counter(counter_name).map(|c| c.value).unwrap_or(0);

        match comparison {
            0 => counter_value < target_value,
            1 => counter_value <= target_value,
            2 => counter_value == target_value,
            3 => counter_value >= target_value,
            4 => counter_value > target_value,
            5 => counter_value != target_value,
            _ => false,
        }
    }

    fn evaluate_flag_condition_inline(&self, condition: &Condition) -> bool {
        let Some(param0) = condition.get_parameter(0) else {
            return false;
        };
        let Some(param1) = condition.get_parameter(1) else {
            return false;
        };

        let flag_name = param0.get_string();
        let target_value = param1.get_int() != 0;
        self.get_flag(flag_name).map(|f| f.value).unwrap_or(false) == target_value
    }

    fn evaluate_timer_condition_inline(&self, condition: &Condition) -> bool {
        let Some(param0) = condition.get_parameter(0) else {
            return false;
        };
        let counter_name = param0.get_string();
        self.get_counter(counter_name)
            .map(|c| c.is_countdown_timer && c.value < 1)
            .unwrap_or(false)
    }

    // PARITY_NOTE: C++ ScriptEngine::removeSequentialScript (empty body in C++)
    pub fn remove_sequential_script(&mut self, _script: &SequentialScript) {}

    // PARITY_NOTE: C++ ScriptEngine::adjustTimer
    pub fn adjust_timer(
        &mut self,
        counter_name: &str,
        value: i32,
        millisecond_timer: bool,
        add: bool,
    ) -> GameLogicResult<()> {
        let index = self.allocate_counter(counter_name)?;
        let Some(counter) = &mut self.counters[index] else {
            return Ok(());
        };
        if millisecond_timer {
            let msec_frames = Self::frames_from_millisecond_script_seconds(value as f32);
            let delta = if add { msec_frames } else { -msec_frames };
            counter.value += delta;
        } else {
            let delta = if add { value } else { -value };
            counter.value += delta;
        }
        Ok(())
    }

    // PARITY_NOTE: C++ ScriptEngine::getStats
    pub fn get_stats_detailed(&self) -> (String, f32, f32, f32) {
        #[cfg(feature = "script_profiling")]
        {
            (self.get_stats(), self.stats.cur_update_time, 0.0, 0.0)
        }
        #[cfg(not(feature = "script_profiling"))]
        {
            (
                "Script Engine Profiling disabled.".to_string(),
                0.0,
                0.0,
                0.0,
            )
        }
    }

    // PARITY_NOTE: C++ ScriptEngine::addObjectToCache
    pub fn add_object_to_cache(&mut self, object_id: ObjectID) {
        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(obj) = obj_arc.read() else { return };
        let name = obj.get_name();
        if name.is_empty() {
            return;
        }
        let tracker = get_named_object_tracker();
        let _ = tracker.register_named_object(name.to_string(), object_id);
    }

    // PARITY_NOTE: C++ ScriptEngine::removeObjectFromCache
    pub fn remove_object_from_cache(&mut self, object_id: ObjectID) {
        let tracker = get_named_object_tracker();
        let _ = tracker.unregister_object(object_id);
    }

    // PARITY_NOTE: C++ ScriptEngine::restartTimer (only restarts if value > 0)
    pub fn restart_timer_if_positive(&mut self, name: &str) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        if let Some(counter) = &mut self.counters[index] {
            if counter.value > 0 {
                counter.is_countdown_timer = true;
            }
        }
        Ok(())
    }

    // PARITY_NOTE: C++ ScriptEngine::setTimer with random/msec params
    pub fn set_timer_with_params(
        &mut self,
        name: &str,
        value: f32,
        millisecond_timer: bool,
        random: bool,
        random_max: Option<f32>,
    ) -> GameLogicResult<()> {
        let index = self.allocate_counter(name)?;
        let Some(counter) = &mut self.counters[index] else {
            return Ok(());
        };

        let effective_value = if random {
            let max = random_max.unwrap_or(value);
            crate::helpers::get_game_logic_random_value_real(value.min(max), value.max(max))
        } else {
            value
        };

        if millisecond_timer {
            counter.value = Self::frames_from_millisecond_script_seconds(effective_value);
        } else {
            counter.value = effective_value as i32;
        }
        counter.is_countdown_timer = true;
        Ok(())
    }

    // PARITY_NOTE: C++ always returns FALSE (no case in switch)
    pub fn has_unit_completed_sequential_script(
        &self,
        _object: ObjectID,
        _script_name: &str,
    ) -> bool {
        false
    }

    // PARITY_NOTE: C++ always returns FALSE (no case in switch)
    pub fn has_team_completed_sequential_script(
        &self,
        _team_name: &str,
        _script_name: &str,
    ) -> bool {
        false
    }

    /// PARITY_NOTE: C++ FRAMES_TO_SHOW_WIN_LOSE_MESSAGE = 120
    pub fn start_end_game_timer_cxx(&mut self) {
        self.end_game_timer = 120;
    }

    /// PARITY_NOTE: C++ FRAMES_TO_SHOW_WIN_LOSE_MESSAGE = 120
    pub fn start_close_window_timer_cxx(&mut self) {
        self.close_window_timer = 120;
    }

    /// PARITY_NOTE: C++ startQuickEndGameTimer = 1 frame
    pub fn start_quick_end_game_timer_cxx(&mut self) {
        self.end_game_timer = 1;
    }
}

impl XferSnapshot for ScriptEngine {
    fn crc(&mut self, _xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), XferStatus> {
        let current_version: XferVersion = 6;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;

        let mut sequential_script_count: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                self.sequential_scripts.len() as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut sequential_script_count)?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for script in self.sequential_scripts.iter_mut() {
                    script.xfer(xfer)?;
                }
            }
            XferMode::Load => {
                if !self.sequential_scripts.is_empty() {
                    return Err(XferStatus::ListNotEmpty);
                }
                self.sequential_scripts.clear();
                for _ in 0..sequential_script_count {
                    let mut script = SequentialScript::new();
                    script.xfer(xfer)?;
                    self.sequential_scripts.push(script);
                }
            }
            XferMode::Invalid => return Err(XferStatus::ModeUnknown),
        }

        let mut counters_size: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                self.num_counters as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut counters_size)?;
        if counters_size as usize > MAX_COUNTERS {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..counters_size as usize {
            if xfer.get_xfer_mode() == XferMode::Load && self.counters[i].is_none() {
                self.counters[i] = Some(TCounter::new(String::new()));
            }
            let Some(counter) = self.counters[i].as_mut() else {
                return Err(XferStatus::InvalidParameters);
            };
            xfer.xfer_int(&mut counter.value)?;
            xfer.xfer_ascii_string(&mut counter.name)?;
            xfer.xfer_bool(&mut counter.is_countdown_timer)?;
        }

        let mut num_counters = self.num_counters as i32;
        xfer.xfer_int(&mut num_counters)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.num_counters = num_counters as usize;
        }

        let mut flags_size: u16 = if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc)
        {
            self.num_flags as u16
        } else {
            0
        };
        xfer.xfer_unsigned_short(&mut flags_size)?;
        if flags_size as usize > MAX_FLAGS {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..flags_size as usize {
            if xfer.get_xfer_mode() == XferMode::Load && self.flags[i].is_none() {
                self.flags[i] = Some(TFlag::new(String::new()));
            }
            let Some(flag) = self.flags[i].as_mut() else {
                return Err(XferStatus::InvalidParameters);
            };
            xfer.xfer_bool(&mut flag.value)?;
            xfer.xfer_ascii_string(&mut flag.name)?;
        }

        let mut num_flags = self.num_flags as i32;
        xfer.xfer_int(&mut num_flags)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.num_flags = num_flags as usize;
        }

        let mut attack_priority_size: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                self.num_attack_info as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut attack_priority_size)?;
        if attack_priority_size as usize > MAX_ATTACK_PRIORITIES {
            return Err(XferStatus::InvalidParameters);
        }
        if xfer.get_xfer_mode() == XferMode::Load {
            self.attack_priority_info.clear();
            self.attack_priority_info
                .resize_with(attack_priority_size as usize, AttackPriorityInfo::new);
        }
        for i in 0..attack_priority_size as usize {
            self.attack_priority_info[i].xfer(xfer)?;
        }

        let mut num_attack_info = self.num_attack_info as i32;
        xfer.xfer_int(&mut num_attack_info)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.num_attack_info = num_attack_info as usize;
        }

        if version >= 6 {
            let mut object_priority_count: u16 =
                if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                    self.object_attack_priority_sets
                        .len()
                        .min(u16::MAX as usize) as u16
                } else {
                    0
                };
            xfer.xfer_unsigned_short(&mut object_priority_count)?;

            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    let mut entries: Vec<(ObjectID, String)> = self
                        .object_attack_priority_sets
                        .iter()
                        .map(|(object_id, set_name)| (*object_id, set_name.clone()))
                        .collect();
                    entries.sort_by_key(|(object_id, _)| *object_id);
                    for (object_id, mut set_name) in entries {
                        let mut object_id = object_id;
                        xfer.xfer_object_id(&mut object_id)?;
                        xfer.xfer_ascii_string(&mut set_name)?;
                    }
                }
                XferMode::Load => {
                    self.object_attack_priority_sets.clear();
                    for _ in 0..object_priority_count {
                        let mut object_id: ObjectID = crate::common::INVALID_ID;
                        let mut set_name = String::new();
                        xfer.xfer_object_id(&mut object_id)?;
                        xfer.xfer_ascii_string(&mut set_name)?;
                        if object_id == crate::common::INVALID_ID || set_name.is_empty() {
                            continue;
                        }
                        self.object_attack_priority_sets.insert(object_id, set_name);
                    }
                }
                XferMode::Invalid => return Err(XferStatus::ModeUnknown),
            }
        } else if xfer.get_xfer_mode() == XferMode::Load {
            self.object_attack_priority_sets.clear();
        }

        xfer.xfer_int(&mut self.end_game_timer)?;
        xfer.xfer_int(&mut self.close_window_timer)?;

        let named_object_tracker = get_named_object_tracker();
        let named_objects: Vec<(String, ObjectID)> =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                let mut entries = named_object_tracker
                    .get_all_named_objects()
                    .unwrap_or_default();
                entries.sort_by(|a, b| a.0.cmp(&b.0));
                entries
            } else {
                Vec::new()
            };
        let mut named_objects_count: u16 =
            if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                named_objects.len() as u16
            } else {
                0
            };
        xfer.xfer_unsigned_short(&mut named_objects_count)?;

        match xfer.get_xfer_mode() {
            XferMode::Save | XferMode::Crc => {
                for (name, object_id) in named_objects.iter() {
                    let mut entry_name = name.clone();
                    let mut entry_id = *object_id;
                    xfer.xfer_ascii_string(&mut entry_name)?;
                    xfer.xfer_object_id(&mut entry_id)?;
                }
            }
            XferMode::Load => {
                named_object_tracker
                    .clear()
                    .map_err(|_| XferStatus::ErrorUnknown)?;
                for _ in 0..named_objects_count {
                    let mut entry_name = String::new();
                    let mut entry_id: ObjectID = crate::common::INVALID_ID;
                    xfer.xfer_ascii_string(&mut entry_name)?;
                    xfer.xfer_object_id(&mut entry_id)?;
                    if entry_id != crate::common::INVALID_ID
                        && OBJECT_REGISTRY.get_object(entry_id).is_none()
                    {
                        return Err(XferStatus::InvalidParameters);
                    }
                    named_object_tracker
                        .register_named_object(entry_name, entry_id)
                        .map_err(|_| XferStatus::ErrorUnknown)?;
                }
            }
            XferMode::Invalid => return Err(XferStatus::ModeUnknown),
        }

        xfer.xfer_bool(&mut self.first_update)?;

        let mut fade_value: i32 = match self.fade {
            TFade::None => 0,
            TFade::Subtract => 1,
            TFade::Add => 2,
            TFade::Saturate => 3,
            TFade::Multiply => 4,
        };
        xfer.xfer_int(&mut fade_value)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.fade = match fade_value {
                0 => TFade::None,
                1 => TFade::Subtract,
                2 => TFade::Add,
                3 => TFade::Saturate,
                4 => TFade::Multiply,
                _ => return Err(XferStatus::InvalidParameters),
            };
        }

        xfer.xfer_real(&mut self.min_fade)?;
        xfer.xfer_real(&mut self.max_fade)?;
        xfer.xfer_real(&mut self.cur_fade_value)?;
        xfer.xfer_int(&mut self.cur_fade_frame)?;
        xfer.xfer_int(&mut self.fade_frames_increase)?;
        xfer.xfer_int(&mut self.fade_frames_hold)?;
        xfer.xfer_int(&mut self.fade_frames_decrease)?;

        xfer_list_ascii_string(xfer, &mut self.completed_video)?;
        xfer_list_ascii_string_uint(xfer, &mut self.testing_speech)?;
        xfer_list_ascii_string_uint(xfer, &mut self.testing_audio)?;
        xfer_list_ascii_string(xfer, &mut self.ui_interactions)?;

        let mut triggered_special_powers_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut triggered_special_powers_size)?;
        if triggered_special_powers_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..triggered_special_powers_size as usize {
            xfer_list_ascii_string_object_id(xfer, &mut self.triggered_special_powers[i])?;
        }

        let mut midway_special_powers_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut midway_special_powers_size)?;
        if midway_special_powers_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..midway_special_powers_size as usize {
            xfer_list_ascii_string_object_id(xfer, &mut self.midway_special_powers[i])?;
        }

        let mut finished_special_powers_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut finished_special_powers_size)?;
        if finished_special_powers_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..finished_special_powers_size as usize {
            xfer_list_ascii_string_object_id(xfer, &mut self.finished_special_powers[i])?;
        }

        let mut completed_upgrades_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut completed_upgrades_size)?;
        if completed_upgrades_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..completed_upgrades_size as usize {
            xfer_list_ascii_string_object_id(xfer, &mut self.completed_upgrades[i])?;
        }

        let mut acquired_sciences_size: u16 = Self::MAX_PLAYER_COUNT as u16;
        xfer.xfer_unsigned_short(&mut acquired_sciences_size)?;
        if acquired_sciences_size != Self::MAX_PLAYER_COUNT as u16 {
            return Err(XferStatus::InvalidParameters);
        }
        for i in 0..acquired_sciences_size as usize {
            xfer_science_vec(xfer, &mut self.acquired_sciences[i])?;
        }

        xfer_list_ascii_string_coord3d(xfer, &mut self.topple_directions)?;

        xfer.xfer_real(&mut self.breeze_info.direction)?;
        xfer.xfer_real(&mut self.breeze_info.direction_vec[0])?;
        xfer.xfer_real(&mut self.breeze_info.direction_vec[1])?;
        xfer.xfer_real(&mut self.breeze_info.intensity)?;
        xfer.xfer_real(&mut self.breeze_info.lean)?;
        xfer.xfer_real(&mut self.breeze_info.randomness)?;
        xfer.xfer_short(&mut self.breeze_info.breeze_period)?;
        xfer.xfer_short(&mut self.breeze_info.breeze_version)?;

        let mut difficulty_value: i32 = match self.game_difficulty {
            crate::player::GameDifficulty::Easy => 0,
            crate::player::GameDifficulty::Normal => 1,
            crate::player::GameDifficulty::Hard => 2,
            crate::player::GameDifficulty::Brutal => 3,
        };
        xfer.xfer_int(&mut difficulty_value)?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.game_difficulty = match difficulty_value {
                0 => crate::player::GameDifficulty::Easy,
                1 => crate::player::GameDifficulty::Normal,
                2 => crate::player::GameDifficulty::Hard,
                3 => crate::player::GameDifficulty::Brutal,
                _ => return Err(XferStatus::InvalidParameters),
            };
        }

        xfer.xfer_bool(&mut self.freeze_by_script)?;

        if version >= 2 {
            let mut named_reveal_count: u16 =
                if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                    self.named_reveals.len() as u16
                } else {
                    0
                };
            xfer.xfer_unsigned_short(&mut named_reveal_count)?;
            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    for reveal in self.named_reveals.iter_mut() {
                        xfer.xfer_ascii_string(&mut reveal.reveal_name)?;
                        xfer.xfer_ascii_string(&mut reveal.waypoint_name)?;
                        xfer.xfer_real(&mut reveal.radius_to_reveal)?;
                        xfer.xfer_ascii_string(&mut reveal.player_name)?;
                    }
                }
                XferMode::Load => {
                    if !self.named_reveals.is_empty() {
                        return Err(XferStatus::ListNotEmpty);
                    }
                    self.named_reveals.clear();
                    for _ in 0..named_reveal_count {
                        let mut reveal = NamedReveal {
                            reveal_name: String::new(),
                            waypoint_name: String::new(),
                            radius_to_reveal: 0.0,
                            player_name: String::new(),
                        };
                        xfer.xfer_ascii_string(&mut reveal.reveal_name)?;
                        xfer.xfer_ascii_string(&mut reveal.waypoint_name)?;
                        xfer.xfer_real(&mut reveal.radius_to_reveal)?;
                        xfer.xfer_ascii_string(&mut reveal.player_name)?;
                        self.named_reveals.push(reveal);
                    }
                }
                XferMode::Invalid => return Err(XferStatus::ModeUnknown),
            }

            let mut all_object_types_count: u16 =
                if matches!(xfer.get_xfer_mode(), XferMode::Save | XferMode::Crc) {
                    self.object_types.len() as u16
                } else {
                    0
                };
            xfer.xfer_unsigned_short(&mut all_object_types_count)?;

            match xfer.get_xfer_mode() {
                XferMode::Save | XferMode::Crc => {
                    let mut ordered_lists: Vec<&ObjectTypes> = self.object_types.values().collect();
                    ordered_lists
                        .sort_by(|a, b| a.list_name().as_str().cmp(b.list_name().as_str()));
                    for entry in ordered_lists.iter() {
                        let current_version: XferVersion = 1;
                        let mut obj_version = current_version;
                        xfer.xfer_version(&mut obj_version, current_version)?;

                        let mut list_name = entry.list_name().as_str().to_string();
                        xfer.xfer_ascii_string(&mut list_name)?;

                        let mut object_type_count: u16 = entry.list_size() as u16;
                        xfer.xfer_unsigned_short(&mut object_type_count)?;
                        for object_type in entry.iter() {
                            let mut object_type_name = object_type.as_str().to_string();
                            xfer.xfer_ascii_string(&mut object_type_name)?;
                        }
                    }
                }
                XferMode::Load => {
                    if !self.object_types.is_empty() {
                        return Err(XferStatus::ListNotEmpty);
                    }
                    self.object_types.clear();
                    for _ in 0..all_object_types_count {
                        let current_version: XferVersion = 1;
                        let mut obj_version = current_version;
                        xfer.xfer_version(&mut obj_version, current_version)?;

                        let mut list_name = String::new();
                        xfer.xfer_ascii_string(&mut list_name)?;

                        let mut object_type_count: u16 = 0;
                        xfer.xfer_unsigned_short(&mut object_type_count)?;

                        let mut list =
                            ObjectTypes::with_list_name(AsciiString::from(list_name.as_str()));
                        for _ in 0..object_type_count {
                            let mut object_type_name = String::new();
                            xfer.xfer_ascii_string(&mut object_type_name)?;
                            list.add_object_type(AsciiString::from(object_type_name.as_str()));
                        }

                        let key = list.list_name().as_str().to_string();
                        self.object_types.insert(key, list);
                    }
                }
                XferMode::Invalid => return Err(XferStatus::ModeUnknown),
            }
        }

        if version >= 3 {
            xfer.xfer_bool(&mut self.objects_should_receive_difficulty_bonus)?;
        } else {
            self.objects_should_receive_difficulty_bonus = true;
        }

        if version >= 4 {
            xfer.xfer_ascii_string(&mut self.current_track_name)?;
        }

        if version >= 5 {
            xfer.xfer_bool(&mut self.choose_victim_always_uses_normal)?;
        } else {
            self.choose_victim_always_uses_normal = false;
        }

        if xfer.get_xfer_mode() == XferMode::Load && self.fade == TFade::None {
            self.fade = TFade::Multiply;
            self.cur_fade_frame = 0;
            self.min_fade = 1.0;
            self.max_fade = 0.0;
            self.fade_frames_increase = 0;
            self.fade_frames_hold = 0;
            self.fade_frames_decrease = FRAMES_TO_FADE_IN_AT_START;
            self.cur_fade_value = 0.0;
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), XferStatus> {
        Ok(())
    }
}

// Static instances (in real implementation these would be proper singletons)
lazy_static::lazy_static! {
    static ref SCRIPT_ENGINE: Arc<RwLock<Option<ScriptEngine>>> = Arc::new(RwLock::new(None));
    static ref EVENT_MANAGER: Arc<EventManager> = Arc::new(EventManager::new());
    static ref NAMED_OBJECT_TRACKER: Arc<NamedObjectTracker> = Arc::new(NamedObjectTracker::new());
    static ref AREA_TRACKER: Arc<AreaTracker> = Arc::new(AreaTracker::new());
    static ref VTUNE_ENABLED_STATE: RwLock<bool> = RwLock::new(false);
    static ref SKATE_DISTANCE_OVERRIDE_STATE: RwLock<f32> = RwLock::new(0.0);
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
pub fn get_script_engine() -> Arc<RwLock<Option<ScriptEngine>>> {
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
    let Some(obj_arc) = OBJECT_REGISTRY.get_object(to_object_id) else {
        return Err(GameLogicError::InvalidObject(to_object_id));
    };

    let tracker = get_named_object_tracker();
    if let Ok(Some(old_id)) = tracker.get_object_id(from_name.as_str()) {
        let _ = tracker.unregister_object(old_id);
    }

    if let Ok(mut guard) = obj_arc.write() {
        guard.set_name(from_name.clone());
    } else {
        return Err(GameLogicError::Threading(
            "Failed to acquire object write lock".to_string(),
        ));
    }

    tracker.register_named_object(from_name.to_string(), to_object_id)?;
    Ok(())
}

/// Get reference to global area tracker
pub fn get_area_tracker() -> Arc<AreaTracker> {
    AREA_TRACKER.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_script_engine_creation() {
        let engine = ScriptEngine::new().unwrap();
        // Slot 0 is reserved, matching runtime reset semantics.
        assert_eq!(engine.num_counters, 1);
        assert_eq!(engine.num_flags, 1);
        assert_eq!(engine.fade, TFade::None);
        assert!(!engine.is_game_ending());
    }

    #[test]
    fn test_counter_operations() {
        let mut engine = ScriptEngine::new().unwrap();

        // Set a counter
        engine.set_counter("test_counter", 42).unwrap();
        assert_eq!(engine.num_counters, 2);

        // Get the counter
        let counter = engine.get_counter("test_counter").unwrap();
        assert_eq!(counter.value, 42);
        assert_eq!(counter.name, "test_counter");
    }

    #[test]
    fn test_flag_operations() {
        let mut engine = ScriptEngine::new().unwrap();

        // Set a flag
        engine.set_flag("test_flag", true).unwrap();
        assert_eq!(engine.num_flags, 2);

        // Get the flag
        let flag = engine.get_flag("test_flag").unwrap();
        assert!(flag.value);
        assert_eq!(flag.name, "test_flag");
    }

    #[test]
    fn test_end_game_timer() {
        let mut engine = ScriptEngine::new().unwrap();
        assert!(!engine.is_game_ending());

        engine.start_end_game_timer();
        assert!(engine.is_game_ending());
        assert_eq!(engine.end_game_timer, 300);
    }

    #[test]
    fn test_time_freeze() {
        let mut engine = ScriptEngine::new().unwrap();
        assert!(!engine.is_time_frozen_script());

        engine.do_freeze_time();
        assert!(engine.is_time_frozen_script());

        engine.do_unfreeze_time();
        assert!(!engine.is_time_frozen_script());
    }

    #[test]
    fn test_debug_freeze_stops_update_progression() {
        let mut engine = ScriptEngine::new().unwrap();
        engine.set_timer("freeze_timer", 10).unwrap();
        engine.set_time_frozen_debug(true);

        engine
            .update()
            .expect("update should succeed while debug-frozen");
        let frozen_counter = engine.get_counter("freeze_timer").unwrap();
        assert_eq!(
            frozen_counter.value, 10,
            "debug freeze should prevent countdown-timer advancement"
        );

        engine.set_time_frozen_debug(false);
        engine
            .update()
            .expect("update should succeed when debug freeze is cleared");
        let resumed_counter = engine.get_counter("freeze_timer").unwrap();
        assert_eq!(
            resumed_counter.value, 9,
            "timer should resume once debug freeze is cleared"
        );
    }

    #[test]
    fn pending_resume_frame_is_next_frame_for_single_frame_wait() {
        assert_eq!(ScriptEngine::pending_resume_frame(100, 1.0), 101);
        assert_eq!(ScriptEngine::pending_resume_frame(100, 0.0), 101);
    }

    #[test]
    fn sequential_pending_wait_conversion_matches_cxx_wait_patterns() {
        // Retry-style wait actions re-run the same instruction on the next frame.
        assert_eq!(
            ScriptEngine::pending_to_sequential_wait_frames(1.0, true),
            0
        );
        assert_eq!(
            ScriptEngine::pending_to_sequential_wait_frames(3.0, true),
            2
        );

        // Framecount actions wait before advancing to the next instruction.
        assert_eq!(
            ScriptEngine::pending_to_sequential_wait_frames(1.0, false),
            1
        );
        assert_eq!(
            ScriptEngine::pending_to_sequential_wait_frames(3.0, false),
            3
        );
    }

    #[test]
    fn sequential_pending_retry_action_classification_matches_cxx() {
        assert!(
            ScriptEngine::pending_repeats_current_sequential_instruction(
                ScriptActionType::SkirmishWaitForCommandbuttonAvailableAll
            )
        );
        assert!(
            ScriptEngine::pending_repeats_current_sequential_instruction(
                ScriptActionType::SkirmishWaitForCommandbuttonAvailablePartial
            )
        );
        assert!(
            ScriptEngine::pending_repeats_current_sequential_instruction(
                ScriptActionType::TeamWaitForNotContainedAll
            )
        );
        assert!(
            ScriptEngine::pending_repeats_current_sequential_instruction(
                ScriptActionType::TeamWaitForNotContainedPartial
            )
        );
        assert!(
            !ScriptEngine::pending_repeats_current_sequential_instruction(
                ScriptActionType::TeamGuardForFramecount
            )
        );

        assert!(ScriptEngine::pending_is_sequential_only_action(
            ScriptActionType::TeamGuardForFramecount
        ));
        assert!(ScriptEngine::pending_is_sequential_only_action(
            ScriptActionType::TeamWaitForNotContainedAll
        ));
        assert!(!ScriptEngine::pending_is_sequential_only_action(
            ScriptActionType::TeamGuardPosition
        ));
    }

    #[test]
    fn set_script_list_initializes_delay_evaluation_frame_offset() {
        let mut engine = ScriptEngine::new().unwrap();

        let mut delayed_script = Script::new();
        delayed_script.delay_evaluation_seconds = 3;
        delayed_script.frame_to_evaluate_at = 9999;

        let mut script_list = ScriptList::new();
        script_list.append_script(Box::new(delayed_script));

        engine
            .set_script_list_for_player(0, Some(Box::new(script_list)))
            .unwrap();

        let script = engine.side_script_lists[0]
            .as_ref()
            .and_then(|list| list.first_script.as_deref())
            .expect("script should be present");
        assert!(script.frame_to_evaluate_at <= (2 * LOGICFRAMES_PER_SECOND as u32));
    }

    #[test]
    fn set_script_list_infers_condition_team_name_for_singleton_team() {
        let mut engine = ScriptEngine::new().unwrap();
        let team_name = "RuntimeInitSingletonTeam".to_string();

        if let Ok(mut factory) = get_team_factory().lock() {
            let _ = factory.init_team(team_name.clone().into(), "PlyrCivilian".into(), true, None);
        }

        let mut team_param = Parameter::new(ParameterType::Team);
        team_param.string_value = team_name.clone();
        team_param.initialized = true;

        let mut condition = Condition::new(ConditionType::ConditionTrue);
        condition.add_parameter(team_param).unwrap();

        let mut or_condition = OrCondition::new();
        or_condition.set_first_and_condition(Some(Box::new(condition)));

        let mut script = Script::new();
        script.condition = Some(Box::new(or_condition));
        script.condition_team_name.clear();

        let mut script_list = ScriptList::new();
        script_list.append_script(Box::new(script));

        engine
            .set_script_list_for_player(0, Some(Box::new(script_list)))
            .unwrap();

        let stored_script = engine.side_script_lists[0]
            .as_ref()
            .and_then(|list| list.first_script.as_deref())
            .expect("script should be present");
        assert_eq!(stored_script.condition_team_name, team_name);
    }

    #[test]
    fn call_subroutine_executes_in_place_and_persists_one_shot_state() {
        let mut engine = ScriptEngine::new().unwrap();

        let mut condition = Condition::new(ConditionType::ConditionTrue);
        let mut or_condition = OrCondition::new();
        or_condition.set_first_and_condition(Some(Box::new(condition)));

        let mut subroutine = Script::new();
        subroutine.script_name = "SubroutinePersist".to_string();
        subroutine.is_subroutine = true;
        subroutine.is_one_shot = true;
        subroutine.condition = Some(Box::new(or_condition));

        let mut script_list = ScriptList::new();
        script_list.append_script(Box::new(subroutine));
        engine
            .set_script_list_for_player(0, Some(Box::new(script_list)))
            .unwrap();

        assert!(engine
            .execute_subroutine_by_name("SubroutinePersist")
            .unwrap());

        let stored = engine.side_script_lists[0]
            .as_ref()
            .and_then(|list| list.first_script.as_deref())
            .expect("subroutine should still exist");
        assert!(!stored.is_active);
    }

    #[test]
    fn call_subroutine_resolves_subroutine_group_name_first() {
        let mut engine = ScriptEngine::new().unwrap();

        let mut condition = Condition::new(ConditionType::ConditionTrue);
        let mut or_condition = OrCondition::new();
        or_condition.set_first_and_condition(Some(Box::new(condition)));

        let mut grouped_subroutine = Script::new();
        grouped_subroutine.script_name = "InnerGroupedSubroutine".to_string();
        grouped_subroutine.is_subroutine = false;
        grouped_subroutine.is_one_shot = true;
        grouped_subroutine.condition = Some(Box::new(or_condition));

        let mut group = ScriptGroup::new();
        group.group_name = "NamedSubroutineGroup".to_string();
        group.is_group_subroutine = true;
        group.is_group_active = true;
        group.first_script = Some(Box::new(grouped_subroutine));

        let mut script_list = ScriptList::new();
        script_list.first_group = Some(Box::new(group));
        engine
            .set_script_list_for_player(0, Some(Box::new(script_list)))
            .unwrap();

        assert!(engine
            .execute_subroutine_by_name("NamedSubroutineGroup")
            .unwrap());

        let grouped_script = engine.side_script_lists[0]
            .as_ref()
            .and_then(|list| list.first_group.as_deref())
            .and_then(|grp| grp.first_script.as_deref())
            .expect("grouped subroutine should exist");
        assert!(!grouped_script.is_active);
    }

    #[test]
    fn millisecond_script_seconds_use_ceil_frame_conversion() {
        let mut engine = ScriptEngine::new().unwrap();
        engine
            .set_timer_millisecond_script_seconds("shell_camera", 0.12)
            .unwrap();

        let index = engine.allocate_counter("shell_camera").unwrap();
        let counter = engine.counters[index].as_ref().unwrap();
        assert_eq!(counter.value, 4);
        assert!(counter.is_countdown_timer);
    }

    #[test]
    fn stop_timer_preserves_remaining_value() {
        let mut engine = ScriptEngine::new().unwrap();
        engine.set_timer("pause_test", 17).unwrap();
        engine.stop_timer("pause_test").unwrap();

        let index = engine.allocate_counter("pause_test").unwrap();
        let counter = engine.counters[index].as_ref().unwrap();
        assert_eq!(counter.value, 17);
        assert!(!counter.is_countdown_timer);
    }

    #[test]
    fn subtract_millisecond_script_seconds_can_drive_timer_negative() {
        let mut engine = ScriptEngine::new().unwrap();
        engine.set_timer("negative_test", 2).unwrap();
        engine
            .subtract_from_timer_millisecond_script_seconds("negative_test", 0.12)
            .unwrap();

        let index = engine.allocate_counter("negative_test").unwrap();
        let counter = engine.counters[index].as_ref().unwrap();
        assert_eq!(counter.value, -2);
    }

    #[test]
    fn vtune_enable_parity_state_round_trips_through_script_engine() {
        let _lock = crate::test_sync::lock();
        set_enable_vtune(false);

        let mut engine = ScriptEngine::new().unwrap();
        assert!(!engine.get_enable_vtune());
        engine.set_enable_vtune(true);
        assert!(engine.get_enable_vtune());
        engine.set_enable_vtune(false);
        assert!(!engine.get_enable_vtune());
    }

    #[test]
    fn skate_override_parity_state_matches_cxx_delta_steps() {
        let _lock = crate::test_sync::lock();
        set_skate_distance_override(0.0);

        let mut engine = ScriptEngine::new().unwrap();
        let up = engine.adjust_skate_distance_override(0.25);
        assert!((up - 0.25).abs() < f32::EPSILON);
        assert!((engine.get_skate_distance_override() - 0.25).abs() < f32::EPSILON);

        let down = engine.adjust_skate_distance_override(-0.25);
        assert!(down.abs() < f32::EPSILON);
        assert!(engine.get_skate_distance_override().abs() < f32::EPSILON);
    }
}
