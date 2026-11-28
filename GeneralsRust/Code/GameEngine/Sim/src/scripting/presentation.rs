//! Presentation callbacks and window tracking for one script interpreter.

use crate::GameLogicResult;
use std::cell::RefCell;
use std::sync::Arc;

#[derive(Default)]
struct ScriptPresentationState {
    suppress_new_windows: bool,
    win_lose_window_layout: Option<String>,
    campaign_victorious: bool,
    shown_mp_local_defeat_window: bool,
}

/// The interpreter tracks each window change before invoking the receiving UI.
/// Callback registration survives an interpreter reset.
#[derive(Default)]
pub struct ScriptPresentation {
    state: RefCell<ScriptPresentationState>,
    handler: Option<Arc<dyn ScriptActionHandler>>,
}

impl ScriptPresentation {
    pub(super) fn reset(&mut self) {
        *self.state.get_mut() = ScriptPresentationState::default();
    }

    pub(super) fn set_action_handler(&mut self, handler: Option<Arc<dyn ScriptActionHandler>>) {
        self.handler = handler;
    }

    pub fn action_handler(&self) -> Option<Arc<dyn ScriptActionHandler>> {
        self.handler.clone()
    }

    pub fn notify_script_activation(&self, name: &str, active: bool) {
        if let Some(handler) = &self.handler {
            let _ = handler.enable_script(name, active);
        }
    }

    pub fn record_window_close(&self, suppress_new_windows: bool) {
        let mut state = self.state.borrow_mut();
        state.suppress_new_windows = suppress_new_windows;
        state.win_lose_window_layout = None;
    }

    /// Record a creation attempt before dispatching to the UI.
    pub fn record_window_open(&self, layout: &str) -> bool {
        let mut state = self.state.borrow_mut();
        if state.suppress_new_windows {
            return false;
        }
        state.win_lose_window_layout = Some(layout.to_owned());
        true
    }

    pub fn win_lose_window(&self) -> Option<String> {
        self.state.borrow().win_lose_window_layout.clone()
    }

    pub fn suppresses_new_windows(&self) -> bool {
        self.state.borrow().suppress_new_windows
    }

    pub fn set_campaign_victorious(&self, victorious: bool) {
        self.state.borrow_mut().campaign_victorious = victorious;
        if let Some(handler) = &self.handler {
            let _ = handler.set_campaign_victorious(victorious);
        }
    }

    pub fn set_shown_mp_local_defeat_window(&self, shown: bool) {
        self.state.borrow_mut().shown_mp_local_defeat_window = shown;
    }

    pub fn has_shown_mp_local_defeat_window(&self) -> bool {
        self.state.borrow().shown_mp_local_defeat_window
    }

    pub fn campaign_victorious(&self) -> bool {
        self.state.borrow().campaign_victorious
    }
}

/// Host-side callbacks for script actions that require integration with the game loop.
pub trait ScriptActionHandler: Send + Sync {
    /// Native cinematic teardown for the interpreter driving this request.
    /// The authoritative input gate has already been changed by its world.
    fn input_enabled(&self, _enabled: bool) -> GameLogicResult<()> {
        Ok(())
    }

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

    fn camera_follow_object(&self, _object_id: u32, _snap_to_unit: bool) -> GameLogicResult<()> {
        Ok(())
    }

    fn camera_tether_object(
        &self,
        object_id: u32,
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
        _object_id: u32,
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
    fn hide_object_superweapon_display_by_script(&self, _object_id: u32) -> GameLogicResult<()> {
        Ok(())
    }

    /// Mirrors InGameUI::showObjectSuperweaponDisplayByScript(object)
    fn show_object_superweapon_display_by_script(&self, _object_id: u32) -> GameLogicResult<()> {
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
