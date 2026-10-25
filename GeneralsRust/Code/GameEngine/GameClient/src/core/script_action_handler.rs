use crate::display::display::{DebugDisplayCallback, Display as GraphicsDisplay};
use crate::display::view::{
    CameraLockType, CameraShakeType, FilterMode, FilterType, Point3, Vector2, with_tactical_view,
    with_tactical_view_ref,
};
use crate::game_text::GameText;
use crate::gui::callbacks::control_bar_callbacks::{hide_control_bar, show_control_bar};
use crate::helpers::TheInGameUI;
use crate::terrain::TerrainVisual;
use crate::terrain::terrain_visual::get_terrain_visual;
use game_engine::common::ini::get_global_data;
use game_engine::common::system::radar::get_radar_system;
use gamelogic::GameLogicResult;
use gamelogic::commands::get_selection_manager;
use gamelogic::common::audio::AudioEventRts;
use gamelogic::common::types::ObjectID;
use gamelogic::helpers::{TheAudio, TheFXList, TheGameLogic, TheScriptEngine, TheTerrainLogic};
use gamelogic::object_manager::get_object_manager;
use gamelogic::player::player_list;
use gamelogic::scripting::engine::ScriptActionHandler;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Mutex, Weak};

/// A presentation capability belonging to one GameClient. It neither publishes
/// the device nor extends its lifetime. Native device-mode requests remain on
/// this same receiver until the driving engine admits them.
#[derive(Default)]
pub struct ScriptDisplayReceiver {
    display: Option<Weak<Mutex<GraphicsDisplay>>>,
    fx: crate::display::display_fx::DisplayFxState,
}
impl ScriptDisplayReceiver {
    pub fn bind(&mut self, display: Option<&Arc<Mutex<GraphicsDisplay>>>) {
        self.display = display.map(Arc::downgrade);
    }
    fn with_display<R>(
        &mut self,
        operation: impl FnOnce(
            &mut GraphicsDisplay,
            &mut crate::display::display_fx::DisplayFxState,
        ) -> R,
    ) -> Option<R> {
        let display = self.display.as_ref()?.upgrade()?;
        let mut display = display.lock().ok()?;
        Some(operation(&mut display, &mut self.fx))
    }
    pub fn fx(&self) -> &crate::display::display_fx::DisplayFxState {
        &self.fx
    }
    pub fn fx_mut(&mut self) -> &mut crate::display::display_fx::DisplayFxState {
        &mut self.fx
    }
    pub fn take_pending_device_mode(
        &mut self,
    ) -> Option<crate::display::display_fx::PendingDeviceMode> {
        self.fx.take_pending_device_mode()
    }
}

pub fn play_script_display_movie(receiver: &mut ScriptDisplayReceiver, movie_name: &str) -> bool {
    receiver
        .with_display(|display, fx| display.play_movie(fx, movie_name.to_string()))
        .unwrap_or(false)
}
pub fn is_script_display_movie_playing(receiver: &mut ScriptDisplayReceiver) -> bool {
    receiver
        .with_display(|display, _fx| display.is_movie_playing())
        .unwrap_or(false)
}
pub fn stop_script_display_movie(receiver: &mut ScriptDisplayReceiver) -> bool {
    receiver
        .with_display(|display, fx| display.stop_movie(fx))
        .is_some()
}
pub fn toggle_script_display_movie_capture(receiver: &mut ScriptDisplayReceiver) -> bool {
    let enabled = !receiver.fx.is_movie_capture_enabled();
    receiver.fx.set_movie_capture_enabled(enabled);
    true
}
pub fn is_script_display_movie_capture_enabled(receiver: &mut ScriptDisplayReceiver) -> bool {
    receiver.fx.is_movie_capture_enabled()
}
/// C++ setDisplayMode. A native owner with no compatibility GPU admits its own
/// validated request; there is no process-wide pending device-mode slot.
pub fn apply_script_display_mode(
    receiver: &mut ScriptDisplayReceiver,
    xres: u32,
    yres: u32,
    bit_depth: u32,
    windowed: bool,
) -> bool {
    if let Some(ok) = receiver
        .with_display(|display, fx| display.set_display_mode(fx, xres, yres, bit_depth, windowed))
    {
        return ok;
    }
    if !crate::display::display_fx::is_valid_device_mode(xres, yres, bit_depth) {
        return false;
    }
    receiver
        .fx
        .queue_device_mode(crate::display::display_fx::PendingDeviceMode {
            xres,
            yres,
            bit_depth,
            windowed,
        });
    true
}
pub fn toggle_script_display_letter_box(receiver: &mut ScriptDisplayReceiver) -> bool {
    receiver
        .fx
        .set_letterbox_enabled(!receiver.fx.letterbox_enabled());
    true
}
pub fn set_script_display_debug_callback(
    receiver: &mut ScriptDisplayReceiver,
    callback: Option<DebugDisplayCallback>,
) -> bool {
    receiver
        .with_display(|display, _fx| display.set_debug_display_callback(callback, None))
        .is_some()
}
pub fn get_script_display_debug_callback(
    receiver: &mut ScriptDisplayReceiver,
) -> Option<DebugDisplayCallback> {
    receiver
        .with_display(|display, _fx| display.get_debug_display_callback())
        .flatten()
}
pub fn set_script_display_gamma(
    receiver: &mut ScriptDisplayReceiver,
    gamma: f32,
    bright: f32,
    contrast: f32,
) -> bool {
    receiver.fx.set_gamma_state(gamma, bright, contrast);
    true
}
pub fn take_script_display_screenshot(receiver: &mut ScriptDisplayReceiver) -> Option<String> {
    receiver.with_display(|display, fx| display.take_screenshot(fx))
}

pub fn script_popup_message(
    message: &str,
    x_percent: i32,
    y_percent: i32,
    width: i32,
    pause: bool,
    pause_music: bool,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    TheInGameUI::popup_message(
        message,
        x_percent,
        y_percent,
        width,
        pause,
        pause_music,
        effects,
    );
}

/// Main's offline authority bridge opens the C++-equivalent single popup with
/// an opaque live-instance id. The id is not text/template authority; it only
/// lets Main reject a delayed acknowledgement after a replacement popup wins.
pub fn script_popup_message_with_host_generation(
    message: &str,
    x_percent: i32,
    y_percent: i32,
    width: i32,
    pause: bool,
    pause_music: bool,
    host_popup_generation: Option<usize>,
    effects: &mut crate::gui::game_window::WindowEventEffects<'_>,
) {
    TheInGameUI::popup_message_with_host_generation(
        message,
        x_percent,
        y_percent,
        width,
        pause,
        pause_music,
        host_popup_generation,
        effects,
    );
}

pub fn script_resize_view_guardband(gbx: f32, gby: f32) {
    with_tactical_view(|view| view.set_guard_band_bias(Vector2::new(gbx, gby)));
}

pub fn script_set_skybox_enabled(enabled: bool) {
    // C++ doSkyBoxSet (W3DWater.cpp:150-154) writes the single
    // TheWritableGlobalData; both Rust stores must move together or the
    // terrain skybox gate (common::global_data draw_sky_box) never sees it.
    game_engine::common::global_data::write().draw_sky_box = enabled;
    if let Some(global) = get_global_data() {
        global.write().draw_sky_box = if enabled { 1.0 } else { 0.0 };
    }
}

pub fn script_set_3d_wireframe_mode(enabled: bool) {
    game_engine::common::global_data::write().writable.wireframe = enabled;

    with_tactical_view(|view| {
        view.set_3d_wireframe_mode(enabled);
    });
}

pub fn script_set_camera_bw_mode(enabled: bool, frames: i32) {
    with_tactical_view(|view| {
        if enabled {
            view.set_view_filter_mode(FilterMode::BWBlackAndWhite);
            view.set_view_filter(FilterType::BlackAndWhite);
            view.set_fade_parameters(frames, 1);
        } else if view.get_view_filter_type() == FilterType::BlackAndWhite {
            view.set_fade_parameters(frames, -1);
        }
    });
}

pub fn script_camera_motion_blur(zoom_in: bool, saturate: bool) {
    with_tactical_view(|view| {
        if view.set_view_filter(FilterType::MotionBlur) {
            let mode = if saturate {
                if zoom_in {
                    FilterMode::MBInSaturate
                } else {
                    FilterMode::MBOutSaturate
                }
            } else if zoom_in {
                FilterMode::MBInAlpha
            } else {
                FilterMode::MBOutAlpha
            };
            if !view.set_view_filter_mode(mode) {
                view.set_view_filter(FilterType::Null);
            }
        };
    });
}

pub fn script_camera_motion_blur_jump(x: f32, y: f32, z: f32, saturate: bool) -> bool {
    with_tactical_view(|view| {
        let target = Point3::new(x, y, z);
        let mut passed = false;
        if view.set_view_filter(FilterType::MotionBlur) {
            passed = true;
            let mode = if saturate {
                FilterMode::MBInAndOutSaturate
            } else {
                FilterMode::MBInAndOutAlpha
            };
            if !view.set_view_filter_mode(mode) {
                view.set_view_filter(FilterType::Null);
                passed = false;
            }
            if passed {
                view.set_view_filter_pos(&target);
            }
        }
        if !passed {
            view.look_at(&target);
        }
        passed
    })
}

pub fn script_camera_motion_blur_follow(amount: i32) {
    with_tactical_view(|view| {
        view.set_motion_blur_follow_mode(amount);
    });
}

pub fn script_camera_motion_blur_end_follow() {
    with_tactical_view(|view| {
        view.set_view_filter_mode(FilterMode::MBEndPanAlpha);
        view.set_view_filter(FilterType::MotionBlur);
    });
}

pub fn script_cameo_flash(command_button_name: &str, flash_count: i32) {
    with_script_runtime(|state| {
        state
            .ui
            .cameo_flash_count
            .insert(command_button_name.to_string(), flash_count.max(0));
    });
}

pub fn script_add_named_timer(name: &str, text: &str, countdown: bool) {
    crate::gui::ingame_ui::add_named_timer(name, text, countdown);
    with_script_runtime(|state| {
        state
            .ui
            .named_timers
            .insert(name.to_string(), (text.to_string(), countdown));
    });
}

pub fn script_remove_named_timer(name: &str) {
    crate::gui::ingame_ui::remove_named_timer(name);
    with_script_runtime(|state| {
        state.ui.named_timers.remove(name);
    });
}

pub fn script_show_named_timer_display(show: bool) {
    crate::gui::ingame_ui::show_named_timer_display(show);
    with_script_runtime(|state| {
        state.ui.named_timer_display_shown = show;
    });
}

pub fn script_set_superweapon_display_enabled(enabled: bool) {
    with_script_runtime(|state| {
        state.ui.superweapon_display_enabled = enabled;
    });
}

pub fn script_hide_object_superweapon_display(object_id: ObjectID) {
    with_script_runtime(|state| {
        state.ui.superweapon_hidden_objects.insert(object_id);
    });
}

pub fn script_show_object_superweapon_display(object_id: ObjectID) {
    with_script_runtime(|state| {
        state.ui.superweapon_hidden_objects.remove(&object_id);
    });
}

pub fn reset_script_action_runtime_state() {
    with_script_runtime(|state| {
        *state = ScriptRuntimeState::default();
    });
    game_engine::common::global_data::write().writable.wireframe = false;
    with_tactical_view(|view| view.reset_3d_wireframe_mode());
    TheGameLogic::set_intro_movie_playing(false);
}

pub fn apply_pending_script_display_state() {
    let wireframe = game_engine::common::global_data::read().writable.wireframe;
    with_tactical_view(|view| {
        if view.pending_3d_wireframe_mode() != wireframe {
            view.set_3d_wireframe_mode(wireframe);
        }
    });

    if let Some(disable) = gamelogic::helpers::TheInGameUI::take_cinematic_input_lock() {
        apply_cinematic_input_lock(disable);
    }
}

/// C++ ScriptActions::doDisableInput / doEnableInput cinematic teardown.
fn apply_cinematic_input_lock(disable: bool) {
    crate::helpers::TheInGameUI::set_input_enabled(!disable);
    crate::input::mouse::with_mouse(|mouse| {
        mouse.set_visibility(!disable);
    });
    if !disable {
        return;
    }
    crate::helpers::TheInGameUI::deselect_all();
    if let Ok(list) = player_list().read() {
        let local_index = list.get_local_player_index();
        if local_index != gamelogic::player::PLAYER_INDEX_INVALID {
            let selection_manager = get_selection_manager();
            if let Ok(mut manager) = selection_manager.write() {
                if let Some(selection) = manager.get_player_selection(local_index) {
                    selection.clear_selection();
                }
            }
        }
    }
    crate::helpers::TheInGameUI::clear_attack_move_to_mode();
    crate::helpers::TheInGameUI::set_waypoint_mode(false);
    request_look_at_reset_modes();
}

fn request_look_at_reset_modes() {
    LOOK_AT_RESET.store(true, std::sync::atomic::Ordering::Relaxed);
}

static LOOK_AT_RESET: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Drain a pending LookAtTranslator::resetModes request.
pub fn take_look_at_reset_modes() -> bool {
    LOOK_AT_RESET.swap(false, std::sync::atomic::Ordering::Relaxed)
}

/// Script-owned runtime handoff state (C++ `ScriptActions` members).
///
/// THREAD: single game thread. Script actions play media and queue the wait
/// entries, the `is_*_complete` script conditions drain them, and the
/// GameClient init/reset/shutdown paths clear them — all on the same thread
/// (C++ runs logic and client from one loop), so this is single-owner state
/// and carries no lock.
#[derive(Default)]
struct ScriptRuntimeState {
    audio_waits: HashMap<String, Vec<u32>>,
    speech_waits: HashMap<String, Vec<u32>>,
    music_waits: HashMap<String, Vec<u32>>,
    ui: ScriptUiState,
}

thread_local! {
    static SCRIPT_RUNTIME_STATE: RefCell<ScriptRuntimeState> =
        RefCell::new(ScriptRuntimeState::default());
}

fn with_script_runtime<R>(f: impl FnOnce(&mut ScriptRuntimeState) -> R) -> R {
    SCRIPT_RUNTIME_STATE.with(|state| f(&mut state.borrow_mut()))
}

#[derive(Default)]
struct ScriptUiState {
    named_timers: HashMap<String, (String, bool)>,
    named_timer_display_shown: bool,
    superweapon_display_enabled: bool,
    superweapon_hidden_objects: HashSet<ObjectID>,
    cameo_flash_count: HashMap<String, i32>,
}

fn normalize_media_name(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn find_object_by_template_name(template_name: &str) -> Option<ObjectID> {
    let template_name = template_name.trim();
    if template_name.is_empty() {
        return None;
    }

    let manager = get_object_manager();
    let manager = manager.read().ok()?;
    for object_id in manager.all_object_ids() {
        let Some(object) = manager.get_object(object_id) else {
            continue;
        };
        let Ok(instance) = object.read() else {
            continue;
        };
        if instance
            .get_template_name()
            .eq_ignore_ascii_case(template_name)
        {
            return Some(object_id);
        }
    }
    None
}

fn track_audio_handle(wait_map: &mut HashMap<String, Vec<u32>>, name: &str, handle: u32) {
    if handle == 0 {
        return;
    }
    let key = normalize_media_name(name);
    if key.is_empty() {
        return;
    }
    wait_map.entry(key).or_default().push(handle);
}

fn is_named_audio_complete(
    wait_map: &mut HashMap<String, Vec<u32>>,
    name: &str,
    flush: bool,
) -> bool {
    let key = normalize_media_name(name);
    if key.is_empty() {
        return false;
    }

    let Some(handles) = wait_map.get_mut(&key) else {
        return false;
    };

    let Some(audio) = TheAudio::get() else {
        if flush {
            wait_map.remove(&key);
        }
        return true;
    };

    handles.retain(|h| audio.is_currently_playing(*h));
    let completed = handles.is_empty();
    if completed && flush {
        wait_map.remove(&key);
    }
    completed
}

static SCRIPT_VISUAL_SPEED_MULTIPLIER: AtomicI32 = AtomicI32::new(1);
static SCRIPT_FPS_LIMIT: AtomicI32 = AtomicI32::new(0);
const SUBTITLE_DURATION_MS: i32 = 8_000;
const AHSV_STOP_THE_MUSIC: u32 = 0xFFFFFFF0;
const AHSV_STOP_THE_MUSIC_FADE: u32 = 0xFFFFFFF1;

pub fn get_script_visual_speed_multiplier() -> i32 {
    SCRIPT_VISUAL_SPEED_MULTIPLIER.load(Ordering::Relaxed)
}

pub fn set_script_visual_speed_multiplier(multiplier: i32) {
    SCRIPT_VISUAL_SPEED_MULTIPLIER.store(multiplier, Ordering::Relaxed);
}

pub fn get_script_fps_limit() -> i32 {
    SCRIPT_FPS_LIMIT.load(Ordering::Relaxed)
}

pub fn set_script_fps_limit(fps: i32) {
    SCRIPT_FPS_LIMIT.store(fps, Ordering::Relaxed);
}

mod music_audio;

pub struct GameClientScriptActionHandler;

impl Default for GameClientScriptActionHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl GameClientScriptActionHandler {
    pub fn new() -> Self {
        Self
    }

    fn seconds_to_ms(seconds: f32) -> i32 {
        if seconds <= 0.0 {
            0
        } else {
            (seconds * 1000.0).round() as i32
        }
    }

    fn seconds_to_ms_f32(seconds: f32) -> f32 {
        if seconds <= 0.0 {
            0.0
        } else {
            seconds * 1000.0
        }
    }

    fn normalize_angle(mut angle: f32) -> f32 {
        while angle < -std::f32::consts::PI {
            angle += 2.0 * std::f32::consts::PI;
        }
        while angle > std::f32::consts::PI {
            angle -= 2.0 * std::f32::consts::PI;
        }
        angle
    }

    fn look_toward_angle(center: Point3, target: Point3) -> Option<f32> {
        let dx = target.x - center.x;
        let dy = target.y - center.y;
        if dx.abs() < f32::EPSILON && dy.abs() < f32::EPSILON {
            return None;
        }
        Some(Self::normalize_angle(
            dy.atan2(dx) - std::f32::consts::PI * 0.5,
        ))
    }

    fn rotate_toward(
        target: Point3,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
        reverse_rotation: bool,
    ) {
        let milliseconds = Self::seconds_to_ms(seconds).max(1);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);
        with_tactical_view(|view| {
            let center = Point3::new(
                view.position().x + view.width() as f32 * 0.5,
                view.position().y + view.height() as f32 * 0.5,
                view.position().z,
            );
            let Some(mut angle) = Self::look_toward_angle(center, target) else {
                return;
            };

            let current_angle = view.angle();
            if reverse_rotation {
                if current_angle < angle {
                    angle -= 2.0 * std::f32::consts::PI;
                } else {
                    angle += 2.0 * std::f32::consts::PI;
                }
            }

            let delta = angle - current_angle;
            view.rotate_camera(
                delta / (2.0 * std::f32::consts::PI),
                milliseconds,
                ease_in_ms,
                ease_out_ms,
            );
        });
    }

    fn map_shake_type(intensity: i32) -> CameraShakeType {
        match intensity {
            i if i <= 0 => CameraShakeType::Subtle,
            1 => CameraShakeType::Subtle,
            2 => CameraShakeType::Normal,
            3 => CameraShakeType::Strong,
            4 => CameraShakeType::Severe,
            5 => CameraShakeType::CineExtreme,
            _ => CameraShakeType::CineInsane,
        }
    }

    fn local_player_index() -> Option<u32> {
        let players = player_list().read().ok()?;
        let index = players.get_local_player_index();
        (index >= 0).then_some(index as u32)
    }

    fn maybe_show_speech_subtitle(
        name: &str,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
        manager: &mut crate::gui::window_manager::WindowManager,
    ) {
        let subtitle_label = format!("DIALOGEVENT:{name}Subtitle");
        let subtitle = GameText::fetch(&subtitle_label);
        let fallback = format!("LOC:{subtitle_label}");
        if subtitle.is_empty() || subtitle.starts_with('*') || subtitle == fallback {
            return;
        }
        ui.push_military_subtitle(&subtitle_label, SUBTITLE_DURATION_MS, manager);
    }
    pub(crate) fn speech_play_receiving(
        name: &str,
        allow_overlap: bool,
        ui: &mut crate::core::subsystems::InGameUISubsystem,
        manager: &mut crate::gui::window_manager::WindowManager,
    ) -> GameLogicResult<()> {
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new(name);
            event.set_is_logical_audio(true);
            event.set_uninterruptable(!allow_overlap);
            if let Some(player_index) = Self::local_player_index() {
                event.set_player_index(player_index);
            }
            let handle = audio.add_audio_event(&event);
            with_script_runtime(|state| track_audio_handle(&mut state.speech_waits, name, handle));
        }
        Self::maybe_show_speech_subtitle(name, ui, manager);
        Ok(())
    }
}

impl ScriptActionHandler for GameClientScriptActionHandler {
    fn input_enabled(&self, enabled: bool) -> GameLogicResult<()> {
        apply_cinematic_input_lock(!enabled);
        Ok(())
    }

    fn set_border_shroud_level(&self, level: u8) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "display script action requires the receiving presentation execution driver".into(),
        ))
    }

    fn display_text(&self, text: &str) -> GameLogicResult<()> {
        TheInGameUI::message(text);
        Ok(())
    }

    fn display_cinematic_text(
        &self,
        text: &str,
        _font_type: &str,
        _duration_seconds: i32,
    ) -> GameLogicResult<()> {
        TheInGameUI::message(text);
        Ok(())
    }

    fn military_caption(&self, text: &str, duration_ms: i32) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "military caption requires its receiving script UI driver".into(),
        ))
    }

    fn play_sound_effect(&self, name: &str) -> GameLogicResult<()> {
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new(name);
            event.set_is_logical_audio(true);
            if let Some(player_index) = Self::local_player_index() {
                event.set_player_index(player_index);
            }
            let handle = audio.add_audio_event(&event);
            with_script_runtime(|state| track_audio_handle(&mut state.audio_waits, name, handle));
        }
        Ok(())
    }

    fn play_sound_effect_at(&self, name: &str, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new(name);
            event.set_position(&(x, y, z));
            event.set_is_logical_audio(true);
            if let Some(player_index) = Self::local_player_index() {
                event.set_player_index(player_index);
            }
            let handle = audio.add_audio_event(&event);
            with_script_runtime(|state| track_audio_handle(&mut state.audio_waits, name, handle));
        }
        Ok(())
    }

    fn move_camera(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.look_at(&Point3::new(x, y, z));
        });
        Ok(())
    }

    fn move_camera_to(
        &self,
        x: f32,
        y: f32,
        z: f32,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        let target = Point3::new(x, y, z);
        let milliseconds = Self::seconds_to_ms(seconds);
        let stutter_ms = Self::seconds_to_ms(camera_stutter_seconds);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);
        with_tactical_view(|view| {
            view.move_camera_to(
                &target,
                milliseconds,
                stutter_ms,
                true,
                ease_in_ms,
                ease_out_ms,
            );
        });
        Ok(())
    }

    fn move_camera_to_selection(&self) -> GameLogicResult<()> {
        let local_player_id = player_list()
            .read()
            .ok()
            .map(|list| list.get_local_player_index())
            .unwrap_or(-1);
        if local_player_id < 0 {
            return Ok(());
        }

        let selection_manager = get_selection_manager();
        let Ok(manager) = selection_manager.read() else {
            return Ok(());
        };
        let Some(selection) = manager.get_player_selection_ref(local_player_id) else {
            return Ok(());
        };
        let selected = selection.get_selected_objects_info();
        if selected.is_empty() {
            return Ok(());
        }

        let count = selected.len() as f32;
        let sum = selected
            .iter()
            .fold((0.0f32, 0.0f32, 0.0f32), |acc, entry| {
                (
                    acc.0 + entry.position.x,
                    acc.1 + entry.position.y,
                    acc.2 + entry.position.z,
                )
            });
        with_tactical_view(|view| {
            view.camera_mod_final_move_to(&Point3::new(sum.0 / count, sum.1 / count, sum.2 / count))
        });
        Ok(())
    }

    fn move_camera_along_waypoint_path(
        &self,
        waypoint_name: &str,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        let milliseconds = Self::seconds_to_ms(seconds);
        let stutter_ms = Self::seconds_to_ms(camera_stutter_seconds);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);

        let mut points = Vec::new();
        if let Some(terrain) = TheTerrainLogic::get() {
            points = terrain
                .get_waypoint_chain_by_name(waypoint_name, 64)
                .into_iter()
                .map(|p| Point3::new(p.x, p.y, p.z))
                .collect();
        }

        if !points.is_empty() {
            with_tactical_view(|view| {
                view.move_camera_along_waypoint_path(
                    &points,
                    milliseconds,
                    stutter_ms,
                    true,
                    ease_in_ms,
                    ease_out_ms,
                )
            });
            return Ok(());
        }

        // Compatibility fallback for legacy scripts that pass a path label instead of a start waypoint.
        let center = with_tactical_view_ref(|view| {
            gamelogic::common::types::Coord3D::new(
                view.position().x + view.width() as f32 * 0.5,
                view.position().y + view.height() as f32 * 0.5,
                view.position().z,
            )
        });

        if let Some(terrain) = TheTerrainLogic::get() {
            if let Some(target) = terrain.get_closest_waypoint_on_path(&center, waypoint_name) {
                return self.move_camera_to(
                    target.x,
                    target.y,
                    target.z,
                    seconds,
                    camera_stutter_seconds,
                    ease_in_seconds,
                    ease_out_seconds,
                );
            }
        }

        Ok(())
    }

    fn camera_move_home(&self) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.set_angle_and_pitch_to_default();
            view.set_zoom_to_default();
        });
        Ok(())
    }

    fn is_camera_movement_finished(&self) -> bool {
        with_tactical_view(|view| view.is_camera_movement_finished())
    }

    fn camera_follow_object(&self, object_id: ObjectID, snap_to_unit: bool) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.set_camera_lock(Some(object_id));
            if snap_to_unit {
                view.snap_to_camera_lock();
            }
        });
        Ok(())
    }

    fn camera_tether_object(
        &self,
        object_id: ObjectID,
        snap_to_unit: bool,
        play: f32,
    ) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.set_camera_lock(Some(object_id));
            if snap_to_unit {
                view.snap_to_camera_lock();
            }
            view.set_snap_mode(CameraLockType::Tether, play.max(0.0));
        });
        Ok(())
    }

    fn stop_camera_follow(&self) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.set_camera_lock(None);
            view.set_snap_mode(CameraLockType::Follow, 0.0);
        });
        Ok(())
    }

    fn reset_camera_to(
        &self,
        x: f32,
        y: f32,
        z: f32,
        duration_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        let target = Point3::new(x, y, z);
        let milliseconds = Self::seconds_to_ms(duration_seconds);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);
        with_tactical_view(|view| {
            view.reset_camera(&target, milliseconds, ease_in_ms, ease_out_ms)
        });
        Ok(())
    }

    fn zoom_camera(
        &self,
        zoom: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        let milliseconds = Self::seconds_to_ms(seconds);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);
        with_tactical_view(|view| view.zoom_camera(zoom, milliseconds, ease_in_ms, ease_out_ms));
        Ok(())
    }

    fn set_camera_zoom(&self, zoom: f32, duration_seconds: f32) -> GameLogicResult<()> {
        let milliseconds = Self::seconds_to_ms(duration_seconds);
        with_tactical_view(|view| view.zoom_camera(zoom, milliseconds, 0.0, 0.0));
        Ok(())
    }

    fn set_camera_pitch(
        &self,
        pitch: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        let milliseconds = Self::seconds_to_ms(seconds);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);
        with_tactical_view(|view| view.pitch_camera(pitch, milliseconds, ease_in_ms, ease_out_ms));
        Ok(())
    }

    fn rotate_camera(
        &self,
        rotations: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        let milliseconds = Self::seconds_to_ms(seconds);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);
        with_tactical_view(|view| {
            view.rotate_camera(rotations, milliseconds, ease_in_ms, ease_out_ms)
        });
        Ok(())
    }

    fn camera_mod_set_final_zoom(
        &self,
        zoom: f32,
        ease_in: f32,
        ease_out: f32,
    ) -> GameLogicResult<()> {
        with_tactical_view(|view| view.camera_mod_final_zoom(zoom, ease_in, ease_out));
        Ok(())
    }

    fn camera_mod_set_final_pitch(
        &self,
        pitch: f32,
        ease_in: f32,
        ease_out: f32,
    ) -> GameLogicResult<()> {
        with_tactical_view(|view| view.camera_mod_final_pitch(pitch, ease_in, ease_out));
        Ok(())
    }

    fn camera_mod_freeze_time(&self) -> GameLogicResult<()> {
        with_tactical_view(|view| view.camera_mod_freeze_time());
        Ok(())
    }

    fn camera_mod_freeze_angle(&self) -> GameLogicResult<()> {
        with_tactical_view(|view| view.camera_mod_freeze_angle());
        Ok(())
    }

    fn camera_mod_set_final_speed_multiplier(&self, _multiplier: i32) -> GameLogicResult<()> {
        with_tactical_view(|view| view.camera_mod_final_time_multiplier(_multiplier));
        Ok(())
    }

    fn camera_mod_set_rolling_average(&self, _frames: i32) -> GameLogicResult<()> {
        with_tactical_view(|view| view.camera_mod_rolling_average(_frames));
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
        self.set_camera_zoom(zoom, 0.0)?;
        self.set_camera_pitch(pitch, 0.0, 0.0, 0.0)?;
        self.camera_look_toward_waypoint(
            look_toward_x,
            look_toward_y,
            look_toward_z,
            0.0,
            0.0,
            0.0,
            false,
        )
    }

    fn camera_look_toward_object(
        &self,
        object_id: ObjectID,
        seconds: f32,
        hold_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        let milliseconds = Self::seconds_to_ms(seconds);
        let hold_ms = Self::seconds_to_ms(hold_seconds);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);
        with_tactical_view(|view| {
            view.rotate_camera_toward_object(
                object_id,
                milliseconds,
                hold_ms,
                ease_in_ms,
                ease_out_ms,
            );
        });
        Ok(())
    }

    fn camera_look_toward_waypoint(
        &self,
        x: f32,
        y: f32,
        z: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
        reverse_rotation: bool,
    ) -> GameLogicResult<()> {
        let milliseconds = Self::seconds_to_ms(seconds);
        let ease_in_ms = Self::seconds_to_ms_f32(ease_in_seconds);
        let ease_out_ms = Self::seconds_to_ms_f32(ease_out_seconds);
        with_tactical_view(|view| {
            view.rotate_camera_toward_position(
                &Point3::new(x, y, z),
                milliseconds,
                ease_in_ms,
                ease_out_ms,
                reverse_rotation,
            );
        });
        Ok(())
    }

    fn camera_mod_look_toward(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        with_tactical_view(|view| view.camera_mod_look_toward(&Point3::new(x, y, z)));
        Ok(())
    }

    fn camera_mod_final_look_toward(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        with_tactical_view(|view| view.camera_mod_final_look_toward(&Point3::new(x, y, z)));
        Ok(())
    }

    fn camera_enable_slave_mode(
        &self,
        thing_template_name: &str,
        bone_name: &str,
    ) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.camera_enable_slave_mode(thing_template_name.trim(), bone_name.trim());
        });
        Ok(())
    }

    fn camera_disable_slave_mode(&self) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.camera_disable_slave_mode();
            view.set_camera_lock(None);
            view.set_snap_mode(CameraLockType::Follow, 0.0);
            view.set_mouse_lock(false);
        });
        Ok(())
    }

    fn camera_letterbox_begin(&self) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "display script action requires the receiving presentation execution driver".into(),
        ))
    }

    fn camera_letterbox_end(&self) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "display script action requires the receiving presentation execution driver".into(),
        ))
    }

    fn camera_set_default(&self, pitch: f32, angle: f32, max_height: f32) -> GameLogicResult<()> {
        with_tactical_view(|view| view.set_default_view(pitch, angle, max_height));
        Ok(())
    }

    fn screen_shake(&self, intensity: i32) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            let epicenter = view.get_3d_camera_position();
            view.shake(&epicenter, Self::map_shake_type(intensity));
        });
        Ok(())
    }

    fn camera_add_shaker_at(
        &self,
        x: f32,
        y: f32,
        z: f32,
        amplitude: f32,
        _duration_seconds: f32,
        _radius: f32,
    ) -> GameLogicResult<()> {
        let shake = if amplitude < 0.25 {
            CameraShakeType::Subtle
        } else if amplitude < 0.5 {
            CameraShakeType::Normal
        } else if amplitude < 1.0 {
            CameraShakeType::Strong
        } else if amplitude < 2.0 {
            CameraShakeType::Severe
        } else {
            CameraShakeType::CineExtreme
        };
        with_tactical_view(|view| {
            view.shake(&Point3::new(x, y, z), shake);
        });
        Ok(())
    }

    fn speech_play(&self, _name: &str, _allow_overlap: bool) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "speech subtitles require their receiving script UI driver".into(),
        ))
    }

    fn sound_play_named(&self, sound: &str, unit_name: &str) -> GameLogicResult<()> {
        let object_id = gamelogic::scripting::host_script_named_unit_id(unit_name).or_else(|| {
            gamelogic::scripting::engine::get_named_object_tracker()
                .get_object_id(unit_name)
                .ok()
                .flatten()
        });
        let Some(object_id) = object_id else {
            return Ok(());
        };
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new(sound);
            event.set_object_id(object_id);
            event.set_is_logical_audio(true);
            let handle = audio.add_audio_event(&event);
            with_script_runtime(|state| track_audio_handle(&mut state.audio_waits, sound, handle));
        }
        Ok(())
    }

    fn enable_object_sound(&self, unit_name: &str, enable: bool) -> GameLogicResult<()> {
        let object_id = gamelogic::scripting::host_script_named_unit_id(unit_name).or_else(|| {
            gamelogic::scripting::engine::get_named_object_tracker()
                .get_object_id(unit_name)
                .ok()
                .flatten()
        });
        let Some(object_id) = object_id else {
            return Ok(());
        };
        if let Some(obj_arc) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(obj_guard) = obj_arc.read() {
                if let Some(drawable) = obj_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        draw_guard.enable_ambient_sound_from_script(enable);
                    }
                }
            }
        }
        Ok(())
    }

    fn music_set_track(&self, track: &str, fade_out: bool, fade_in: bool) -> GameLogicResult<()> {
        if let Some(audio) = TheAudio::get() {
            let mut recipient = audio;
            let handle = self.music_set_track_with_recipient(
                &mut recipient,
                track,
                fade_out,
                fade_in,
                Self::local_player_index(),
            );
            with_script_runtime(|state| track_audio_handle(&mut state.music_waits, track, handle));
        }
        Ok(())
    }

    fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        with_script_runtime(|state| is_named_audio_complete(&mut state.speech_waits, name, flush))
    }

    fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        with_script_runtime(|state| is_named_audio_complete(&mut state.audio_waits, name, flush))
    }

    fn has_music_track_completed(&self, track: &str, param: i32) -> bool {
        let key = track.trim();
        if key.is_empty() {
            return false;
        }
        let required = param.max(1);
        TheAudio::get().is_some_and(|audio| audio.has_music_track_completed(key, required))
    }

    fn stop_music(&self) -> GameLogicResult<()> {
        if let Some(audio) = TheAudio::get() {
            let mut recipient = audio;
            self.stop_music_with_recipient(&mut recipient);
        }
        Ok(())
    }

    fn movie_play_fullscreen(&self, filename: &str) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "display script action requires the receiving presentation execution driver".into(),
        ))
    }

    fn movie_play_radar(&self, filename: &str) -> GameLogicResult<()> {
        let media_name = normalize_media_name(filename);
        if media_name.is_empty() {
            return Ok(());
        }

        Err(gamelogic::GameLogicError::Configuration(
            "radar movie requires receiving presentation execution driver".into(),
        ))
    }

    fn freeze_time(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn unfreeze_time(&self) -> GameLogicResult<()> {
        Ok(())
    }

    fn set_visual_speed_multiplier(&self, multiplier: i32) -> GameLogicResult<()> {
        set_script_visual_speed_multiplier(multiplier);
        Ok(())
    }

    fn set_fps_limit(&self, fps: i32) -> GameLogicResult<()> {
        set_script_fps_limit(fps);
        Ok(())
    }

    fn popup_message(
        &self,
        message: &str,
        x_percent: i32,
        y_percent: i32,
        width: i32,
        pause: bool,
        pause_music: bool,
    ) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "popup requires a receiving script driver and world".into(),
        ))
    }

    fn resize_view_guardband(&self, gbx: f32, gby: f32) -> GameLogicResult<()> {
        with_tactical_view(|view| view.set_guard_band_bias(Vector2::new(gbx, gby)));
        Ok(())
    }

    fn set_skybox_enabled(&self, enabled: bool) -> GameLogicResult<()> {
        // C++ DRAW_SKYBOX_BEGIN/END -> ScriptActions::doSkyBox -> doSkyBoxSet
        // writes the single TheWritableGlobalData->m_drawSkyBox; keep both
        // Rust stores in sync (render gate reads common::global_data).
        game_engine::common::global_data::write().draw_sky_box = enabled;
        if let Some(global) = get_global_data() {
            global.write().draw_sky_box = if enabled { 1.0 } else { 0.0 };
        }
        Ok(())
    }
    fn set_weather_visible(&self, visible: bool) -> GameLogicResult<()> {
        // C++ ScriptActions.cpp: TheSnowManager->setVisible(showWeather)
        if let Some(snow) = crate::snow::get_snow_manager() {
            if let Ok(mut guard) = snow.lock() {
                guard.set_visible(visible);
            }
        }
        Ok(())
    }

    fn camera_motion_blur_jump(
        &self,
        x: f32,
        y: f32,
        z: f32,
        saturate: bool,
    ) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            let target = Point3::new(x, y, z);
            let mut passed = false;
            if view.set_view_filter(FilterType::MotionBlur) {
                passed = true;
                let mode = if saturate {
                    FilterMode::MBInAndOutSaturate
                } else {
                    FilterMode::MBInAndOutAlpha
                };
                if !view.set_view_filter_mode(mode) {
                    view.set_view_filter(FilterType::Null);
                    passed = false;
                }
                if passed {
                    view.set_view_filter_pos(&target);
                }
            }
            if !passed {
                view.look_at(&target);
            }
        });
        Ok(())
    }

    fn set_radar_enabled(&self, enabled: bool) -> GameLogicResult<()> {
        if let Ok(mut radar) = get_radar_system().write() {
            radar.hide(!enabled);
        }
        Ok(())
    }

    fn set_camera_bw_mode(&self, enabled: bool, frames: i32) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            if enabled {
                view.set_view_filter_mode(FilterMode::BWBlackAndWhite);
                view.set_view_filter(FilterType::BlackAndWhite);
                view.set_fade_parameters(frames, 1);
            } else if view.get_view_filter_type() == FilterType::BlackAndWhite {
                view.set_fade_parameters(frames, -1);
            }
        });
        Ok(())
    }

    fn set_3d_wireframe_mode(&self, enabled: bool) -> GameLogicResult<()> {
        script_set_3d_wireframe_mode(enabled);
        Ok(())
    }

    fn camera_motion_blur(&self, zoom_in: bool, saturate: bool) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            if view.set_view_filter(FilterType::MotionBlur) {
                let mode = if saturate {
                    if zoom_in {
                        FilterMode::MBInSaturate
                    } else {
                        FilterMode::MBOutSaturate
                    }
                } else if zoom_in {
                    FilterMode::MBInAlpha
                } else {
                    FilterMode::MBOutAlpha
                };
                if !view.set_view_filter_mode(mode) {
                    view.set_view_filter(FilterType::Null);
                }
            };
        });
        Ok(())
    }

    fn camera_motion_blur_follow(&self, amount: i32) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.set_motion_blur_follow_mode(amount);
        });
        Ok(())
    }

    fn camera_motion_blur_end_follow(&self) -> GameLogicResult<()> {
        with_tactical_view(|view| {
            view.set_view_filter_mode(FilterMode::MBEndPanAlpha);
            view.set_view_filter(FilterType::MotionBlur);
        });
        Ok(())
    }

    fn oversize_terrain(&self, _amount: i32) -> GameLogicResult<()> {
        if let Ok(mut guard) = get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                visual.oversize_terrain(_amount);
            }
        }

        if let Some(amount) = Some(_amount) {
            let amount = amount as f32;
            with_tactical_view(|view| {
                let jitter = Vector2::new(0.0001, 0.0001);
                view.scroll_by(&jitter);
                view.scroll_by(&Vector2::new(-0.0001, -0.0001));
            });
        }
        Ok(())
    }

    fn cameo_flash(&self, command_button_name: &str, flash_count: i32) -> GameLogicResult<()> {
        with_script_runtime(|state| {
            state
                .ui
                .cameo_flash_count
                .insert(command_button_name.to_string(), flash_count.max(0));
        });
        Ok(())
    }

    fn add_named_timer(&self, name: &str, text: &str, countdown: bool) -> GameLogicResult<()> {
        with_script_runtime(|state| {
            state
                .ui
                .named_timers
                .insert(name.to_string(), (text.to_string(), countdown));
        });
        Ok(())
    }

    fn remove_named_timer(&self, name: &str) -> GameLogicResult<()> {
        with_script_runtime(|state| {
            state.ui.named_timers.remove(name);
        });
        Ok(())
    }

    fn show_named_timer_display(&self, show: bool) -> GameLogicResult<()> {
        with_script_runtime(|state| {
            state.ui.named_timer_display_shown = show;
        });
        Ok(())
    }

    fn set_superweapon_display_enabled_by_script(&self, enabled: bool) -> GameLogicResult<()> {
        with_script_runtime(|state| {
            state.ui.superweapon_display_enabled = enabled;
        });
        Ok(())
    }

    fn hide_object_superweapon_display_by_script(
        &self,
        object_id: ObjectID,
    ) -> GameLogicResult<()> {
        with_script_runtime(|state| {
            state.ui.superweapon_hidden_objects.insert(object_id);
        });
        Ok(())
    }

    fn show_object_superweapon_display_by_script(
        &self,
        object_id: ObjectID,
    ) -> GameLogicResult<()> {
        with_script_runtime(|state| {
            state.ui.superweapon_hidden_objects.remove(&object_id);
        });
        Ok(())
    }

    fn set_objective(
        &self,
        _name: &str,
        description: &str,
        completed: bool,
    ) -> GameLogicResult<()> {
        if completed {
            TheInGameUI::message(&format!("Objective Complete: {}", description));
        } else {
            TheInGameUI::message(description);
        }
        Ok(())
    }

    fn spawn_effect(&self, effect_type: &str, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        if let Some(fx) = TheFXList::get() {
            fx.do_fx_at_position(
                effect_type,
                &gamelogic::common::types::Coord3D::new(x, y, z),
            );
        }
        Ok(())
    }

    fn set_campaign_victorious(&self, victorious: bool) -> GameLogicResult<()> {
        let _ = victorious;
        Err(gamelogic::GameLogicError::Configuration(
            "campaign victory must be delivered by the receiving application's script UI owner"
                .into(),
        ))
    }

    fn create_win_lose_window(&self, layout_filename: &str) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "window actions require the receiving synchronous script driver".into(),
        ))
    }

    fn destroy_win_lose_window(&self) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "window actions require the receiving synchronous script driver".into(),
        ))
    }

    fn close_game_windows(&self) -> GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "window actions require the receiving synchronous script driver".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::helpers::TheGameLogic;

    #[test]
    fn reset_script_action_runtime_state_clears_audio_and_ui_state() {
        with_script_runtime(|state| {
            state.audio_waits.insert("audio".into(), vec![1]);
            state.speech_waits.insert("speech".into(), vec![2]);
            state.music_waits.insert("music".into(), vec![3]);
            state.ui.named_timer_display_shown = true;
        });
        TheGameLogic::set_intro_movie_playing(true);
        reset_script_action_runtime_state();
        with_script_runtime(|state| {
            assert!(state.audio_waits.is_empty());
            assert!(state.speech_waits.is_empty());
            assert!(state.music_waits.is_empty());
            assert!(!state.ui.named_timer_display_shown);
        });
        assert!(!TheGameLogic::is_intro_movie_playing());
    }

    #[test]
    fn border_shroud_handoff_persists_without_display_and_resets_cleanly() {
        clear_script_display_border_shroud_level();
        assert_eq!(script_display_border_shroud_level(), None);

        let handler = GameClientScriptActionHandler::new();
        handler
            .set_border_shroud_level(211)
            .expect("DisableBorderShroud handoff");
        assert_eq!(script_display_border_shroud_level(), Some(211));

        handler
            .set_border_shroud_level(37)
            .expect("EnableBorderShroud handoff");
        assert_eq!(script_display_border_shroud_level(), Some(37));

        clear_script_display_border_shroud_level();
        assert_eq!(script_display_border_shroud_level(), None);
    }

    #[test]
    fn create_win_lose_window_loads_victorious_layout_when_present() {
        // C++ ScriptActions.cpp:204 TheWindowManager->winCreateFromScript("Menus/Victorious.wnd")
        let owner = crate::gui::window_manager::WindowManagerOwner::new();
        owner.with_scope(|| {
            GameClientScriptActionHandler::new()
                .create_win_lose_window("Menus/Victorious.wnd")
                .expect("create Victorious.wnd");
            crate::gui::with_window_manager(|manager| {
                if let Some(window_id) = manager.script_message_window_id_for_test() {
                    assert!(
                        manager.get_window_by_id(window_id).is_some(),
                        "winCreateFromScript must keep the loaded Victorious window"
                    );
                }
            });
            GameClientScriptActionHandler::new()
                .destroy_win_lose_window()
                .expect("destroy Victorious.wnd");
            crate::gui::with_window_manager(|manager| {
                assert_eq!(manager.script_message_window_id_for_test(), None);
            });
        });
    }

    #[test]
    fn music_set_track_fade_does_not_hard_stop_before_ahsv_fade() {
        let handler = GameClientScriptActionHandler::new();
        let mut manager = music_audio::test_audio(&["HqOb62ThemeA", "HqOb62ThemeB"]);
        let mut peer = music_audio::test_audio(&["HqOb62ThemeA"]);
        let first = handler.music_set_track_with_recipient(
            &mut manager,
            "HqOb62ThemeA",
            false,
            false,
            Some(0),
        );
        let peer_handle = handler.music_set_track_with_recipient(
            &mut peer,
            "HqOb62ThemeA",
            false,
            false,
            Some(0),
        );
        assert_eq!(
            first, peer_handle,
            "distinct owners may allocate the same logical handle"
        );
        music_audio::tick_test_audio(&mut manager);
        music_audio::tick_test_audio(&mut peer);
        assert!(
            manager.active_event_count() >= 1,
            "first MUSIC_SET_TRACK must be active so fade has a stream"
        );
        assert_eq!(manager.fading_audio_count(), 0);
        handler.music_set_track_with_recipient(&mut manager, "HqOb62ThemeB", true, true, Some(0));
        assert_eq!(
            manager.fading_audio_count(),
            1,
            "AHSV_StopTheMusicFade must start Miles fade; hard-stop first would leave 0"
        );
        assert_eq!(peer.active_event_count(), 1);
        assert_eq!(peer.fading_audio_count(), 0);
        manager.reset();
        assert!(!manager.is_currently_playing(first));
        assert!(peer.is_currently_playing(peer_handle));
        assert_eq!(peer.active_event_count(), 1);
        assert_eq!(peer.fading_audio_count(), 0);
    }

    #[test]
    fn stop_music_uses_ahsv_fade_not_hard_stop() {
        let handler = GameClientScriptActionHandler::new();
        let mut manager = music_audio::test_audio(&["HqOb62StopTheme"]);
        handler.music_set_track_with_recipient(
            &mut manager,
            "HqOb62StopTheme",
            false,
            false,
            Some(0),
        );
        music_audio::tick_test_audio(&mut manager);
        assert!(
            manager.active_event_count() >= 1,
            "track must be active before stop_music"
        );
        handler.stop_music_with_recipient(&mut manager);
        assert_eq!(
            manager.fading_audio_count(),
            1,
            "stop_music must issue AHSV_StopTheMusicFade, not remove tracked handles first"
        );
    }
}
