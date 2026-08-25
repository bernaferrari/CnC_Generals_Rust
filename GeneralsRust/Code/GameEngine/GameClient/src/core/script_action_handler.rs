use crate::display::display::{DebugDisplayCallback, Display as GraphicsDisplay};
use crate::display::view::{
    CameraLockType, CameraShakeType, FilterMode, FilterType, Point3, Vector2, with_tactical_view,
    with_tactical_view_ref,
};
use crate::effects::weather_complete::get_weather_system_mut;
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
use std::sync::{Arc, Mutex, OnceLock};

thread_local! {
    static SCRIPT_DISPLAY: RefCell<Option<Arc<Mutex<GraphicsDisplay>>>> = const { RefCell::new(None) };
}

static PENDING_BORDER_SHROUD_LEVEL: AtomicI32 = AtomicI32::new(-1);

thread_local! {
    /// C++ ScriptActions.cpp m_messageWindow — first window from winCreateFromScript.
    static WIN_LOSE_MESSAGE_WINDOW: RefCell<Option<(String, crate::gui::WindowId)>> =
        const { RefCell::new(None) };
}

fn destroy_tracked_win_lose_window() {
    let tracked = WIN_LOSE_MESSAGE_WINDOW.with(|slot| slot.borrow_mut().take());
    let Some((_layout, window_id)) = tracked else {
        return;
    };
    crate::gui::with_window_manager(|manager| {
        if let Some(window) = manager.get_window_by_id(window_id) {
            let _ = manager.destroy_window(window);
            manager.flush_destroy_queue();
        }
    });
}

pub fn register_script_display_bridge(display: Option<Arc<Mutex<GraphicsDisplay>>>) {
    SCRIPT_DISPLAY.with(|slot| *slot.borrow_mut() = display);
    apply_pending_script_display_state();
}

pub fn set_script_display_border_shroud_level(level: u8) -> bool {
    PENDING_BORDER_SHROUD_LEVEL.store(level as i32, Ordering::Relaxed);
    let _ = with_script_display(|display| display.set_border_shroud_level(level));
    true
}

/// Latest script-owned W3D shroud border level, when one has been requested.
///
/// This is intentionally readable without a live `Display` lock so a
/// presentation frame can freeze the value alongside its FOW grid.  `None`
/// means no script override exists and consumers should retain the source
/// `GlobalData::m_shroudAlpha` initialization value instead.
pub fn script_display_border_shroud_level() -> Option<u8> {
    let level = PENDING_BORDER_SHROUD_LEVEL.load(Ordering::Relaxed);
    (0..=u8::MAX as i32).contains(&level).then_some(level as u8)
}

/// Clear the deferred script override when the script-display runtime resets.
/// The next presentation snapshot falls back to `GlobalData::m_shroudAlpha`.
pub fn clear_script_display_border_shroud_level() {
    PENDING_BORDER_SHROUD_LEVEL.store(-1, Ordering::Relaxed);
}

pub fn play_script_display_movie(movie_name: &str) -> bool {
    with_script_display(|display| display.play_movie(movie_name.to_string())).unwrap_or(false)
}

pub fn is_script_display_movie_playing() -> bool {
    with_script_display(|display| display.is_movie_playing()).unwrap_or(false)
}

pub fn stop_script_display_movie() -> bool {
    with_script_display(|display| {
        display.stop_movie();
    })
    .is_some()
}

pub fn toggle_script_display_movie_capture() -> bool {
    with_script_display(|display| {
        display.toggle_movie_capture();
    })
    .is_some()
}

pub fn is_script_display_movie_capture_enabled() -> bool {
    with_script_display(|display| display.is_movie_capture_enabled()).unwrap_or(false)
}

/// C++ `TheDisplay->setDisplayMode`. Queues a host device change when Display
/// is not installed (Main-owned swap chain).
pub fn apply_script_display_mode(xres: u32, yres: u32, bit_depth: u32, windowed: bool) -> bool {
    if let Some(ok) =
        with_script_display(|display| display.set_display_mode(xres, yres, bit_depth, windowed))
    {
        return ok;
    }
    if crate::display::display_fx::is_valid_device_mode(xres, yres, bit_depth) {
        crate::display::display_fx::queue_device_mode(
            crate::display::display_fx::PendingDeviceMode {
                xres,
                yres,
                bit_depth,
                windowed,
            },
        );
        true
    } else {
        false
    }
}

pub fn toggle_script_display_letter_box() -> bool {
    with_script_display(|display| {
        let enabled = !display.is_letter_box_enabled();
        display.enable_letter_box(enabled);
    })
    .is_some()
}

pub fn set_script_display_debug_callback(callback: Option<DebugDisplayCallback>) -> bool {
    with_script_display(|display| display.set_debug_display_callback(callback, None)).is_some()
}

pub fn get_script_display_debug_callback() -> Option<DebugDisplayCallback> {
    with_script_display(|display| display.get_debug_display_callback()).flatten()
}

pub fn set_script_display_gamma(gamma: f32, bright: f32, contrast: f32) -> bool {
    with_script_display(|display| display.set_gamma(gamma, bright, contrast, false)).is_some()
}

pub fn take_script_display_screenshot() -> Option<String> {
    with_script_display(|display| display.take_screenshot())
}

pub fn script_popup_message(
    message: &str,
    x_percent: i32,
    y_percent: i32,
    width: i32,
    pause: bool,
    pause_music: bool,
) {
    TheInGameUI::popup_message(message, x_percent, y_percent, width, pause, pause_music);
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
) {
    TheInGameUI::popup_message_with_host_generation(
        message,
        x_percent,
        y_percent,
        width,
        pause,
        pause_music,
        host_popup_generation,
    );
}

pub fn script_resize_view_guardband(gbx: f32, gby: f32) {
    with_tactical_view(|view| view.set_guard_band_bias(Vector2::new(gbx, gby)));
}

pub fn script_set_skybox_enabled(enabled: bool) {
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
    if let Ok(mut state) = script_ui_state_slot().lock() {
        state
            .cameo_flash_count
            .insert(command_button_name.to_string(), flash_count.max(0));
    }
}

pub fn script_add_named_timer(name: &str, text: &str, countdown: bool) {
    crate::gui::ingame_ui::add_named_timer(name, text, countdown);
    if let Ok(mut state) = script_ui_state_slot().lock() {
        state
            .named_timers
            .insert(name.to_string(), (text.to_string(), countdown));
    }
}

pub fn script_remove_named_timer(name: &str) {
    crate::gui::ingame_ui::remove_named_timer(name);
    if let Ok(mut state) = script_ui_state_slot().lock() {
        state.named_timers.remove(name);
    }
}

pub fn script_show_named_timer_display(show: bool) {
    crate::gui::ingame_ui::show_named_timer_display(show);
    if let Ok(mut state) = script_ui_state_slot().lock() {
        state.named_timer_display_shown = show;
    }
}

pub fn script_set_superweapon_display_enabled(enabled: bool) {
    if let Ok(mut state) = script_ui_state_slot().lock() {
        state.superweapon_display_enabled = enabled;
    }
}

pub fn script_hide_object_superweapon_display(object_id: ObjectID) {
    if let Ok(mut state) = script_ui_state_slot().lock() {
        state.superweapon_hidden_objects.insert(object_id);
    }
}

pub fn script_show_object_superweapon_display(object_id: ObjectID) {
    if let Ok(mut state) = script_ui_state_slot().lock() {
        state.superweapon_hidden_objects.remove(&object_id);
    }
}

pub fn reset_script_action_runtime_state() {
    if let Ok(mut pending) = fullscreen_movie_wait_slot().lock() {
        pending.clear();
    }
    if let Ok(mut pending) = radar_movie_wait_slot().lock() {
        pending.clear();
    }
    if let Ok(mut pending) = audio_wait_slot().lock() {
        pending.clear();
    }
    if let Ok(mut pending) = speech_wait_slot().lock() {
        pending.clear();
    }
    if let Ok(mut pending) = music_wait_slot().lock() {
        pending.clear();
    }
    if let Ok(mut completed) = music_completed_slot().lock() {
        completed.clear();
    }
    if let Ok(mut state) = script_ui_state_slot().lock() {
        *state = ScriptUiState::default();
    }
    clear_script_display_border_shroud_level();
    game_engine::common::global_data::write().writable.wireframe = false;
    with_tactical_view(|view| view.reset_3d_wireframe_mode());
    TheGameLogic::set_intro_movie_playing(false);
}

fn with_script_display<R>(f: impl FnOnce(&mut GraphicsDisplay) -> R) -> Option<R> {
    SCRIPT_DISPLAY.with(|slot| {
        let display = slot.borrow().clone()?;
        let mut guard = display.lock().ok()?;
        Some(f(&mut guard))
    })
}

pub fn apply_pending_script_display_state() {
    let level = PENDING_BORDER_SHROUD_LEVEL.load(Ordering::Relaxed);
    if level >= 0 {
        let _ = with_script_display(|display| display.set_border_shroud_level(level as u8));
    }

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
    let _ = crate::gui::gui_callbacks::control_bar_popup_description::delete_build_tooltip_layout();
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

fn fullscreen_movie_wait_slot() -> &'static Mutex<HashSet<String>> {
    static SLOT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn radar_movie_wait_slot() -> &'static Mutex<HashSet<String>> {
    static SLOT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn clear_pending_fullscreen_movie_key(key: &str) {
    if let Ok(mut pending) = fullscreen_movie_wait_slot().lock() {
        pending.remove(key);
    }
}

fn clear_pending_radar_movie_key(key: &str) {
    if let Ok(mut pending) = radar_movie_wait_slot().lock() {
        pending.remove(key);
    }
}

fn audio_wait_slot() -> &'static Mutex<HashMap<String, Vec<u32>>> {
    static SLOT: OnceLock<Mutex<HashMap<String, Vec<u32>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn speech_wait_slot() -> &'static Mutex<HashMap<String, Vec<u32>>> {
    static SLOT: OnceLock<Mutex<HashMap<String, Vec<u32>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn music_wait_slot() -> &'static Mutex<HashMap<String, Vec<u32>>> {
    static SLOT: OnceLock<Mutex<HashMap<String, Vec<u32>>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(HashMap::new()))
}

fn music_completed_slot() -> &'static Mutex<HashMap<String, i32>> {
    static SLOT: OnceLock<Mutex<HashMap<String, i32>>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Default)]
struct ScriptUiState {
    named_timers: HashMap<String, (String, bool)>,
    named_timer_display_shown: bool,
    superweapon_display_enabled: bool,
    superweapon_hidden_objects: HashSet<ObjectID>,
    cameo_flash_count: HashMap<String, i32>,
}

fn script_ui_state_slot() -> &'static Mutex<ScriptUiState> {
    static SLOT: OnceLock<Mutex<ScriptUiState>> = OnceLock::new();
    SLOT.get_or_init(|| Mutex::new(ScriptUiState::default()))
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

fn track_audio_handle(slot: &Mutex<HashMap<String, Vec<u32>>>, name: &str, handle: u32) {
    if handle == 0 {
        return;
    }
    let key = normalize_media_name(name);
    if key.is_empty() {
        return;
    }
    if let Ok(mut wait_map) = slot.lock() {
        wait_map.entry(key).or_default().push(handle);
    }
}

fn is_named_audio_complete(
    slot: &Mutex<HashMap<String, Vec<u32>>>,
    name: &str,
    flush: bool,
) -> bool {
    let key = normalize_media_name(name);
    if key.is_empty() {
        return false;
    }

    let Ok(mut wait_map) = slot.lock() else {
        return true;
    };
    let Some(handles) = wait_map.get_mut(&key) else {
        return false;
    };

    let Some(audio) = TheAudio::get() else {
        let completed = true;
        if completed && flush {
            wait_map.remove(&key);
        }
        return completed;
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

    fn maybe_show_speech_subtitle(name: &str) {
        let subtitle_label = format!("DIALOGEVENT:{name}Subtitle");
        let subtitle = GameText::fetch(&subtitle_label);
        let fallback = format!("LOC:{subtitle_label}");
        if subtitle.is_empty() || subtitle.starts_with('*') || subtitle == fallback {
            return;
        }
        TheInGameUI::military_subtitle(&subtitle_label, SUBTITLE_DURATION_MS);
    }

    fn music_completed_count_for(track_key: &str) -> i32 {
        let Some(audio) = TheAudio::get() else {
            return music_completed_slot()
                .lock()
                .ok()
                .and_then(|counts| counts.get(track_key).copied())
                .unwrap_or(0);
        };

        let completed_now = if let Ok(mut wait_map) = music_wait_slot().lock() {
            let mut completed_now = 0_i32;
            let mut remove_entry = false;
            if let Some(handles) = wait_map.get_mut(track_key) {
                let before = handles.len();
                handles.retain(|h| audio.is_currently_playing(*h));
                completed_now = before.saturating_sub(handles.len()) as i32;
                remove_entry = handles.is_empty();
            }
            if remove_entry {
                wait_map.remove(track_key);
            }
            completed_now
        } else {
            0
        };

        let mut total = 0_i32;
        if let Ok(mut completed_map) = music_completed_slot().lock() {
            if completed_now > 0 {
                *completed_map.entry(track_key.to_string()).or_default() += completed_now;
            }
            total = completed_map.get(track_key).copied().unwrap_or(0);
        }
        total
    }
}

impl ScriptActionHandler for GameClientScriptActionHandler {
    fn set_border_shroud_level(&self, level: u8) -> GameLogicResult<()> {
        // Keep the value even before a Display is registered.  The frozen
        // projected-shroud snapshot consumes this script handoff at
        // presentation-build time, while `set_script_display...` also updates
        // an active Display when one exists.
        let _ = set_script_display_border_shroud_level(level);
        Ok(())
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
        TheInGameUI::military_subtitle(text, duration_ms);
        Ok(())
    }

    fn play_sound_effect(&self, name: &str) -> GameLogicResult<()> {
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new(name);
            event.set_is_logical_audio(true);
            if let Some(player_index) = Self::local_player_index() {
                event.set_player_index(player_index);
            }
            let handle = audio.add_audio_event(&event);
            track_audio_handle(audio_wait_slot(), name, handle);
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
            track_audio_handle(audio_wait_slot(), name, handle);
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
        let _ = hide_control_bar(true);
        let _ = with_script_display(|display| display.enable_letter_box(true));
        Ok(())
    }

    fn camera_letterbox_end(&self) -> GameLogicResult<()> {
        let _ = show_control_bar(false);
        let _ = with_script_display(|display| display.enable_letter_box(false));
        Ok(())
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

    fn speech_play(&self, name: &str, allow_overlap: bool) -> GameLogicResult<()> {
        if let Some(audio) = TheAudio::get() {
            let mut event = AudioEventRts::new(name);
            event.set_is_logical_audio(true);
            event.set_uninterruptable(!allow_overlap);
            if let Some(player_index) = Self::local_player_index() {
                event.set_player_index(player_index);
            }
            let handle = audio.add_audio_event(&event);
            track_audio_handle(speech_wait_slot(), name, handle);
        }
        Self::maybe_show_speech_subtitle(name);
        Ok(())
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
            track_audio_handle(audio_wait_slot(), sound, handle);
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
            // C++ ScriptActions::doMusicTrackChange (ScriptActions.cpp:3271-3285)
            // only issues AHSV_StopTheMusicFade / AHSV_StopTheMusic. Removing
            // tracked handles first hard-stops the Miles stream so fade never
            // starts (hq-ob62m).
            audio.remove_audio_event(if fade_out {
                AHSV_STOP_THE_MUSIC_FADE
            } else {
                AHSV_STOP_THE_MUSIC
            });
            let mut event = AudioEventRts::new(track);
            event.set_should_fade(fade_in);
            if let Some(player_index) = Self::local_player_index() {
                event.set_player_index(player_index);
            }
            let handle = audio.add_audio_event(&event);
            track_audio_handle(music_wait_slot(), track, handle);
        }
        Ok(())
    }

    fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        is_named_audio_complete(speech_wait_slot(), name, flush)
    }

    fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        is_named_audio_complete(audio_wait_slot(), name, flush)
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
            // C++ script/UI music stops use AHSV_StopTheMusicFade only
            // (ScriptActions.cpp:3274-3275, LoadScreen.cpp:1296).
            audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
        }
        Ok(())
    }

    fn movie_play_fullscreen(&self, filename: &str) -> GameLogicResult<()> {
        let media_name = normalize_media_name(filename);
        if media_name.is_empty() {
            return Ok(());
        }

        clear_pending_fullscreen_movie_key(&media_name);
        let started = play_script_display_movie(filename);
        if started {
            if let Ok(mut pending) = fullscreen_movie_wait_slot().lock() {
                pending.insert(media_name);
            }
        }
        TheGameLogic::set_intro_movie_playing(started);
        Ok(())
    }

    fn movie_play_radar(&self, filename: &str) -> GameLogicResult<()> {
        let media_name = normalize_media_name(filename);
        if media_name.is_empty() {
            return Ok(());
        }

        clear_pending_radar_movie_key(&media_name);
        let started = TheInGameUI::play_movie(filename);
        if started {
            if let Ok(mut pending) = radar_movie_wait_slot().lock() {
                pending.insert(media_name);
            }
        }
        Ok(())
    }

    fn is_video_complete(&self, name: &str, flush: bool) -> bool {
        let key = normalize_media_name(name);
        if key.is_empty() {
            return false;
        }

        let display_playing =
            with_script_display(|display| display.is_movie_playing()).unwrap_or(false);
        let ui_playing = TheInGameUI::is_movie_playing(name);
        let Ok(mut fullscreen_pending) = fullscreen_movie_wait_slot().lock() else {
            return false;
        };
        let Ok(mut radar_pending) = radar_movie_wait_slot().lock() else {
            return false;
        };

        if TheScriptEngine::is_video_complete(&key, flush) {
            if flush {
                fullscreen_pending.remove(&key);
                radar_pending.remove(&key);
            }
            if fullscreen_pending.is_empty() && !display_playing {
                TheGameLogic::set_intro_movie_playing(false);
            }
            return true;
        }

        if fullscreen_pending.contains(&key) {
            if display_playing {
                TheGameLogic::set_intro_movie_playing(true);
                return false;
            }

            if flush {
                fullscreen_pending.remove(&key);
            }

            if fullscreen_pending.is_empty() {
                TheGameLogic::set_intro_movie_playing(false);
            }
            return true;
        }

        if radar_pending.contains(&key) {
            if ui_playing {
                return false;
            }

            if flush {
                radar_pending.remove(&key);
            }
            return true;
        }

        if display_playing {
            TheGameLogic::set_intro_movie_playing(true);
            return false;
        }

        if fullscreen_pending.is_empty() {
            TheGameLogic::set_intro_movie_playing(false);
        }
        false
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
        TheInGameUI::popup_message(message, x_percent, y_percent, width, pause, pause_music);
        Ok(())
    }

    fn resize_view_guardband(&self, gbx: f32, gby: f32) -> GameLogicResult<()> {
        with_tactical_view(|view| view.set_guard_band_bias(Vector2::new(gbx, gby)));
        Ok(())
    }

    fn set_skybox_enabled(&self, enabled: bool) -> GameLogicResult<()> {
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
        if let Ok(mut weather_guard) = get_weather_system_mut() {
            if let Some(weather) = weather_guard.as_mut() {
                weather.set_enabled(visible);
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
        if let Ok(mut state) = script_ui_state_slot().lock() {
            state
                .cameo_flash_count
                .insert(command_button_name.to_string(), flash_count.max(0));
        }
        Ok(())
    }

    fn add_named_timer(&self, name: &str, text: &str, countdown: bool) -> GameLogicResult<()> {
        if let Ok(mut state) = script_ui_state_slot().lock() {
            state
                .named_timers
                .insert(name.to_string(), (text.to_string(), countdown));
        }
        Ok(())
    }

    fn remove_named_timer(&self, name: &str) -> GameLogicResult<()> {
        if let Ok(mut state) = script_ui_state_slot().lock() {
            state.named_timers.remove(name);
        }
        Ok(())
    }

    fn show_named_timer_display(&self, show: bool) -> GameLogicResult<()> {
        if let Ok(mut state) = script_ui_state_slot().lock() {
            state.named_timer_display_shown = show;
        }
        Ok(())
    }

    fn set_superweapon_display_enabled_by_script(&self, enabled: bool) -> GameLogicResult<()> {
        if let Ok(mut state) = script_ui_state_slot().lock() {
            state.superweapon_display_enabled = enabled;
        }
        Ok(())
    }

    fn hide_object_superweapon_display_by_script(
        &self,
        object_id: ObjectID,
    ) -> GameLogicResult<()> {
        if let Ok(mut state) = script_ui_state_slot().lock() {
            state.superweapon_hidden_objects.insert(object_id);
        }
        Ok(())
    }

    fn show_object_superweapon_display_by_script(
        &self,
        object_id: ObjectID,
    ) -> GameLogicResult<()> {
        if let Ok(mut state) = script_ui_state_slot().lock() {
            state.superweapon_hidden_objects.remove(&object_id);
        }
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
        crate::gui::campaign_manager::get_campaign_manager().set_victorious(victorious);
        Ok(())
    }

    fn create_win_lose_window(&self, layout_filename: &str) -> GameLogicResult<()> {
        // C++ ScriptActions.cpp:201/204/225/228/247 TheWindowManager->winCreateFromScript.
        destroy_tracked_win_lose_window();
        let created =
            crate::gui::with_window_manager(|manager| manager.load_window(layout_filename));
        if let Ok(window) = created {
            let window_id = window.borrow().get_id();
            WIN_LOSE_MESSAGE_WINDOW.with(|slot| {
                *slot.borrow_mut() = Some((layout_filename.to_string(), window_id));
            });
        }
        Ok(())
    }

    fn destroy_win_lose_window(&self) -> GameLogicResult<()> {
        // C++ ScriptActions.cpp:160-162 TheWindowManager->winDestroy(m_messageWindow).
        destroy_tracked_win_lose_window();
        Ok(())
    }

    fn close_game_windows(&self) -> GameLogicResult<()> {
        // C++ GameLogic::closeWindows GameLogicDispatch.cpp:202-219.
        let _ = crate::gui::callbacks::diplomacy::hide_diplomacy(false);
        let _ = crate::gui::callbacks::diplomacy::reset_diplomacy();
        let _ = crate::gui::callbacks::ingame_callbacks::hide_in_game_chat(false);
        let _ = crate::gui::callbacks::ingame_callbacks::reset_in_game_chat();
        crate::helpers::TheControlBar::hide_purchase_science();
        crate::helpers::TheControlBar::hide_special_power_shortcut();
        crate::gui::callbacks::quit_menu::hide_quit_menu();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::helpers::TheGameLogic;

    fn clear_movie_wait_slots() {
        fullscreen_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        radar_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    #[test]
    fn fullscreen_movie_wait_completion_clears_intro_state_when_not_playing() {
        clear_movie_wait_slots();
        TheGameLogic::set_intro_movie_playing(true);
        fullscreen_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert("intro".to_string());

        let handler = GameClientScriptActionHandler::new();
        assert!(handler.is_video_complete("intro", true));
        assert!(!TheGameLogic::is_intro_movie_playing());
        assert!(
            !fullscreen_movie_wait_slot()
                .lock()
                .unwrap()
                .contains("intro")
        );
    }

    #[test]
    fn radar_movie_wait_completion_does_not_clear_intro_state() {
        clear_movie_wait_slots();
        TheGameLogic::set_intro_movie_playing(true);
        radar_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert("radar".to_string());

        let handler = GameClientScriptActionHandler::new();
        assert!(handler.is_video_complete("radar", true));
        assert!(TheGameLogic::is_intro_movie_playing());
        assert!(
            !radar_movie_wait_slot()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains("radar")
        );
    }

    #[test]
    fn reset_script_action_runtime_state_clears_movie_waits_and_intro_state() {
        fullscreen_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert("intro".to_string());
        radar_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert("radar".to_string());
        TheGameLogic::set_intro_movie_playing(true);

        reset_script_action_runtime_state();

        assert!(
            fullscreen_movie_wait_slot()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
        );
        assert!(
            radar_movie_wait_slot()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_empty()
        );
        assert!(!TheGameLogic::is_intro_movie_playing());
    }

    #[test]
    fn clear_pending_fullscreen_movie_key_preserves_radar_wait_lane() {
        clear_movie_wait_slots();
        fullscreen_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert("shared".to_string());
        radar_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert("shared".to_string());

        clear_pending_fullscreen_movie_key("shared");

        assert!(
            !fullscreen_movie_wait_slot()
                .lock()
                .unwrap()
                .contains("shared")
        );
        assert!(
            radar_movie_wait_slot()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains("shared")
        );
    }

    #[test]
    fn clear_pending_radar_movie_key_preserves_fullscreen_wait_lane() {
        clear_movie_wait_slots();
        fullscreen_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert("shared".to_string());
        radar_movie_wait_slot()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert("shared".to_string());

        clear_pending_radar_movie_key("shared");

        assert!(
            fullscreen_movie_wait_slot()
                .lock()
                .unwrap()
                .contains("shared")
        );
        assert!(
            !radar_movie_wait_slot()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .contains("shared")
        );
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
        let handler = GameClientScriptActionHandler::new();
        handler
            .create_win_lose_window("Menus/Victorious.wnd")
            .expect("create Victorious.wnd");
        let tracked = WIN_LOSE_MESSAGE_WINDOW.with(|slot| slot.borrow().clone());
        if let Some((layout, window_id)) = tracked {
            assert_eq!(layout, "Menus/Victorious.wnd");
            crate::gui::with_window_manager(|manager| {
                assert!(
                    manager.get_window_by_id(window_id).is_some(),
                    "winCreateFromScript must keep the loaded Victorious window"
                );
            });
        }
        handler
            .destroy_win_lose_window()
            .expect("destroy Victorious.wnd");
        assert!(WIN_LOSE_MESSAGE_WINDOW.with(|slot| slot.borrow().is_none()));
    }

    fn register_script_music_track(name: &str) {
        use game_engine::common::audio::{AudioEventInfo, AudioPriority, AudioType};
        let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
        if let Ok(mut guard) = manager.lock() {
            if guard.find_audio_event_info(name).is_none() {
                guard.register_audio_event_info(AudioEventInfo {
                    sound_type: AudioType::Music,
                    audio_name: name.to_string(),
                    volume: 1.0,
                    sounds: vec!["theme.wav".to_string()],
                    filename: "theme.wav".to_string(),
                    sound_type_field: AudioType::Music,
                    priority: AudioPriority::Normal,
                    min_distance: 25.0,
                    max_distance: 1000.0,
                    ..Default::default()
                });
            }
        }
    }

    #[test]
    fn music_set_track_fade_does_not_hard_stop_before_ahsv_fade() {
        // C++ ScriptActions::doMusicTrackChange (ScriptActions.cpp:3271-3278)
        // fadeout=true → only TheAudio->removeAudioEvent(AHSV_StopTheMusicFade).
        // Hard-stopping tracked handles first leaves nothing for Miles fade.
        reset_script_action_runtime_state();
        register_script_music_track("HqOb62ThemeA");
        register_script_music_track("HqOb62ThemeB");

        let handler = GameClientScriptActionHandler::new();
        handler
            .music_set_track("HqOb62ThemeA", false, false)
            .expect("play first track");
        if let Some(audio) = TheAudio::get() {
            audio.update();
        }

        let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
        {
            let guard = manager.lock().expect("THE_AUDIO lock");
            assert!(
                guard.active_event_count() >= 1,
                "first MUSIC_SET_TRACK must be active so fade has a stream"
            );
            assert_eq!(guard.fading_audio_count(), 0);
        }

        handler
            .music_set_track("HqOb62ThemeB", true, true)
            .expect("fade to second track");

        let guard = manager.lock().expect("THE_AUDIO lock");
        assert_eq!(
            guard.fading_audio_count(),
            1,
            "AHSV_StopTheMusicFade must start Miles fade; hard-stop first would leave 0"
        );
    }

    #[test]
    fn stop_music_uses_ahsv_fade_not_hard_stop() {
        // C++ LoadScreen.cpp:1296 / doMusicTrackChange fade path: fade sentinel only.
        reset_script_action_runtime_state();
        register_script_music_track("HqOb62StopTheme");

        let handler = GameClientScriptActionHandler::new();
        handler
            .music_set_track("HqOb62StopTheme", false, false)
            .expect("play track");
        if let Some(audio) = TheAudio::get() {
            audio.update();
        }

        let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
        {
            let guard = manager.lock().expect("THE_AUDIO lock");
            assert!(
                guard.active_event_count() >= 1,
                "track must be active before stop_music"
            );
        }

        handler.stop_music().expect("stop music");

        let guard = manager.lock().expect("THE_AUDIO lock");
        assert_eq!(
            guard.fading_audio_count(),
            1,
            "stop_music must issue AHSV_StopTheMusicFade, not remove tracked handles first"
        );
    }
}
