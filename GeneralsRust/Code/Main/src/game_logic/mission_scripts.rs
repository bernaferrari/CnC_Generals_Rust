use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use crate::localization;
use gamelogic::scripting::core::{Script, ScriptAction, ScriptActionType, ScriptList};
use gamelogic::scripting::engine::{
    ScriptActionHandler, get_script_engine, initialize_script_engine,
};
use gamelogic::scripting::evaluator::ScriptEvaluator;
use gamelogic::{GameLogicError, GameLogicResult};
use glam::Vec3;

const SPEECH_SUBTITLE_DURATION_MS: i32 = 8000;

/// Live-only identity allocator for C++'s one active InGamePopupMessage WND.
///
/// This deliberately outlives individual `MissionScriptHooks` / `GameLogic`
/// instances.  Map loads and whole-world replacement create new hook objects;
/// keeping the counter there would let a delayed acknowledgement for old popup
/// #1 accidentally match a new world's popup #1.  It is neither gameplay nor
/// presentation/save/Xfer data.
static NEXT_LIVE_POPUP_GENERATION: AtomicUsize = AtomicUsize::new(1);

fn next_live_popup_generation() -> usize {
    // Zero is the explicit fail-closed "no active host popup" value.  Skip it
    // if an effectively-unreachable usize wrap occurs rather than publishing
    // a token Main intentionally refuses to acknowledge.
    loop {
        let generation = NEXT_LIVE_POPUP_GENERATION.fetch_add(1, Ordering::Relaxed);
        if generation != 0 {
            return generation;
        }
    }
}

fn speech_subtitle_label(name: &str) -> String {
    format!("DIALOGEVENT:{}Subtitle", name)
}

fn speech_subtitle_label_if_displayable<F>(name: &str, lookup: F) -> Option<String>
where
    F: FnOnce(&str) -> Option<String>,
{
    let label = speech_subtitle_label(name);
    let subtitle = lookup(&label)?;
    if subtitle.is_empty() || subtitle.starts_with('*') {
        return None;
    }
    Some(label)
}

/// C++ `ScriptEngine::isSpeechComplete` completion frame
/// (`ScriptEngine.cpp:7278-7284`, leftover `named_trackers.rs`).
/// `REAL_TO_UNSIGNEDINT(TheAudio->getAudioLengthMS / MSEC_PER_LOGICFRAME_REAL)`.
fn speech_frames_from_length_ms(audio_length_ms: f32) -> u64 {
    ((audio_length_ms.max(0.0) / 1000.0)
        * game_engine::common::game_common::LOGICFRAMES_PER_SECOND as f32) as u64
}

fn speech_completion_frame(now: u64, name: &str) -> u64 {
    let audio_length_ms = gamelogic::helpers::TheAudio::get()
        .map(|audio| {
            let event = gamelogic::common::audio::AudioEventRts::new(name);
            audio.get_audio_length_ms(&event)
        })
        .unwrap_or(0.0);
    now.saturating_add(speech_frames_from_length_ms(audio_length_ms))
}

#[derive(Debug, Clone)]
pub struct ObjectiveUpdate {
    pub name: String,
    pub description: String,
    pub completed: bool,
}

#[derive(Debug, Clone)]
pub struct ScriptEffectRequest {
    pub effect_type: String,
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct RadarScriptEventRequest {
    pub position: Vec3,
    pub event_type: i32,
}

#[derive(Debug, Clone)]
pub struct MilitaryCaptionRequest {
    pub text: String,
    pub duration_ms: i32,
}

#[derive(Debug, Clone)]
pub struct ScriptSoundEvent {
    pub sound_name: String,
    pub position: Option<Vec3>,
}

#[derive(Debug, Clone)]
pub struct CameraFollowRequest {
    pub object_id: u32,
    pub snap_to_unit: bool,
}

#[derive(Debug, Clone)]
pub struct CameraTetherRequest {
    pub object_id: u32,
    pub snap_to_unit: bool,
    pub play: f32,
}

#[derive(Debug, Clone)]
pub struct CameraResetRequest {
    pub position: Vec3,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraZoomRequest {
    pub zoom: f32,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraPitchRequest {
    pub pitch: f32,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraRotateRequest {
    pub rotations: f32,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraModFinalZoomRequest {
    pub zoom: f32,
    pub ease_in: f32,
    pub ease_out: f32,
}

#[derive(Debug, Clone)]
pub struct CameraModFinalPitchRequest {
    pub pitch: f32,
    pub ease_in: f32,
    pub ease_out: f32,
}

#[derive(Debug, Clone)]
pub struct CameraModFinalSpeedMultiplierRequest {
    pub multiplier: i32,
}

#[derive(Debug, Clone)]
pub struct CameraModRollingAverageRequest {
    pub frames: i32,
}

#[derive(Debug, Clone)]
pub struct VisualSpeedMultiplierRequest {
    pub multiplier: i32,
}

#[derive(Debug, Clone)]
pub struct SetFpsLimitRequest {
    pub fps: i32,
}

#[derive(Debug, Clone)]
pub struct CameraSetupRequest {
    pub position: Vec3,
    pub zoom: f32,
    pub pitch: f32,
    pub look_toward: Vec3,
}

#[derive(Debug, Clone)]
pub struct CameraLookTowardObjectRequest {
    pub object_id: u32,
    pub duration_seconds: f32,
    pub hold_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraLookTowardWaypointRequest {
    pub position: Vec3,
    pub duration_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
    pub reverse_rotation: bool,
}

#[derive(Debug, Clone)]
pub struct CameraModLookTowardRequest {
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct CameraModFinalLookTowardRequest {
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct CameraSetDefaultRequest {
    pub pitch: f32,
    pub angle: f32,
    pub max_height: f32,
}

#[derive(Debug, Clone)]
pub struct CameraSlaveModeRequest {
    pub thing_template_name: String,
    pub bone_name: String,
}

#[derive(Debug, Clone)]
pub struct ScreenShakeRequest {
    pub intensity: i32,
}

#[derive(Debug, Clone)]
pub struct CameraAddShakerRequest {
    pub position: Vec3,
    pub amplitude: f32,
    pub duration_seconds: f32,
    pub radius: f32,
}

#[derive(Debug, Clone)]
pub struct CameraPathRequest {
    pub waypoint: String,
    pub seconds: f32,
    pub camera_stutter_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone)]
pub struct CameraMoveToRequest {
    pub position: Vec3,
    pub seconds: f32,
    pub camera_stutter_seconds: f32,
    pub ease_in_seconds: f32,
    pub ease_out_seconds: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScriptPopupMessageRequest {
    pub message: String,
    pub x_percent: i32,
    pub y_percent: i32,
    pub width: i32,
    pub pause: bool,
    pub pause_music: bool,
    /// Opaque live-session identity assigned by `MissionScriptHooks` when the
    /// request enters its queue. It is deliberately not presentation/save/Xfer
    /// data: Main uses it only to reject a delayed acknowledgement for a popup
    /// that C++ has already replaced.
    pub popup_generation: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ViewGuardbandRequest {
    pub x_bias: f32,
    pub y_bias: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameraBwModeRequest {
    pub enabled: bool,
    pub frames: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CameraMotionBlurRequest {
    Basic { zoom_in: bool, saturate: bool },
    Jump { position: Vec3, saturate: bool },
    Follow { amount: i32 },
    EndFollow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CameoFlashRequest {
    pub command_button_name: String,
    pub flash_count: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NamedTimerMutation {
    Add {
        name: String,
        text: String,
        countdown: bool,
    },
    Remove {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SuperweaponObjectDisplayMutation {
    Hide { object_id: u32 },
    Show { object_id: u32 },
}

/// C++ ScriptActions NAMED_*_SPECIAL_POWER_COUNTDOWN residual.
#[derive(Debug, Clone, PartialEq)]
pub struct NamedSpecialPowerCountdownMutation {
    pub unit_name: String,
    pub power_name: String,
    pub op: crate::game_logic::NamedSpecialPowerCountdownOp,
    pub seconds: i32,
}

fn camera_coord3d_to_world(x: f32, y: f32, z: f32) -> Vec3 {
    // Generals Coord3D: (x,y) on the map plane, z = height.
    // Main renderer world: x/z on the map plane, y = height.
    Vec3::new(x, z, y)
}

#[derive(Debug, Clone)]
struct ScriptState {
    completed: bool,
    next_frame_allowed: u64,
}

impl ScriptState {
    fn new() -> Self {
        Self {
            completed: false,
            next_frame_allowed: 0,
        }
    }
}

#[derive(Clone)]
struct RuntimeScript {
    name: String,
    original_name: Option<String>,
    script: Script,
    state: ScriptState,
    /// `None` means a root ScriptList entry.  A group index preserves C++'s
    /// per-group active gate rather than baking it into the script at load.
    group_index: Option<usize>,
    is_subroutine: bool,
    enabled: bool,
}

/// Runtime identity/state for one C++ `ScriptGroup`.
///
/// `ENABLE_SCRIPT` / `DISABLE_SCRIPT` can name a group.  C++ looks up groups
/// independently from scripts and toggles only the group active bit; members
/// retain their own active/one-shot state.
#[derive(Clone)]
struct RuntimeScriptGroup {
    name: String,
    active: bool,
    is_subroutine: bool,
}

pub struct MissionScriptRuntime {
    evaluator: ScriptEvaluator,
    scripts: Vec<RuntimeScript>,
    groups: Vec<RuntimeScriptGroup>,
    script_lookup: HashMap<String, usize>,
    original_lookup: HashMap<String, usize>,
    group_lookup: HashMap<String, usize>,
    /// Action handlers cannot take the runtime mutex recursively.  They queue
    /// ENABLE/DISABLE requests here; the regular C++-ordered walk applies the
    /// queue immediately after each completed script before visiting the next
    /// declaration.
    pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    frame_counter: u64,
    next_script_index: usize,
}

impl MissionScriptRuntime {
    fn new() -> GameLogicResult<Self> {
        Self::new_with_pending_script_enabled_updates(Arc::new(Mutex::new(Vec::new())))
    }

    fn new_with_pending_script_enabled_updates(
        pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    ) -> GameLogicResult<Self> {
        let _ = initialize_script_engine();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());
        Ok(Self {
            evaluator,
            scripts: Vec::new(),
            groups: Vec::new(),
            script_lookup: HashMap::new(),
            original_lookup: HashMap::new(),
            group_lookup: HashMap::new(),
            pending_script_enabled_updates,
            frame_counter: 0,
            next_script_index: 0,
        })
    }

    fn install_lists(&mut self, lists: &[ScriptList]) {
        self.scripts.clear();
        self.groups.clear();
        self.script_lookup.clear();
        self.original_lookup.clear();
        self.group_lookup.clear();
        self.frame_counter = 0;
        self.next_script_index = 0;

        for (list_index, list) in lists.iter().enumerate() {
            self.collect_chain(
                format!("List{}", list_index),
                list.first_script.as_deref(),
                None,
            );

            let mut group = list.first_group.as_deref();
            let mut group_index = 0usize;
            while let Some(script_group) = group {
                let group_prefix = if script_group.get_name().is_empty() {
                    format!("List{}::Group{}", list_index, group_index)
                } else {
                    format!(
                        "List{}::{}",
                        list_index,
                        script_group.get_name().replace(' ', "_")
                    )
                };
                let runtime_group_index = self.groups.len();
                self.group_lookup
                    .entry(script_group.get_name().to_string())
                    .or_insert(runtime_group_index);
                self.groups.push(RuntimeScriptGroup {
                    name: script_group.get_name().to_string(),
                    active: script_group.is_active(),
                    is_subroutine: script_group.is_subroutine(),
                });
                self.collect_chain(
                    group_prefix,
                    script_group.get_script(),
                    Some(runtime_group_index),
                );
                group = script_group.get_next();
                group_index += 1;
            }
        }

        log::info!(
            "Mission script runtime registered {} WW3D scripts",
            self.scripts.len()
        );
        let enabled_count = self
            .scripts
            .iter()
            .filter(|script| self.is_regular_script_eligible(script) && script.enabled)
            .count();
        log::info!(
            "Mission script runtime has {} frame-eligible scripts at install",
            enabled_count
        );
        for script in self.scripts.iter().filter(|script| {
            self.is_regular_script_eligible(script)
                && (script.name.contains("Move_Camera")
                    || script.original_name.as_deref().is_some_and(|name| {
                        matches!(
                            name.to_ascii_lowercase().as_str(),
                            "move camera"
                                | "restart camera script"
                                | "restart camera"
                                | "restart camera really"
                                | "unshroud"
                                | "turn off sirens"
                        )
                    }))
        }) {
            log::debug!(
                "Mission script install: runtime='{}' original={:?} enabled={} script_active={}",
                script.name,
                script.original_name,
                script.enabled,
                script.script.is_active()
            );
        }
    }

    fn update(&mut self, current_frame: u64) -> GameLogicResult<()> {
        self.update_budgeted(current_frame, None)
    }

    fn update_budgeted(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.update_budgeted_internal(current_frame, max_scripts_per_frame)
    }

    fn update_budgeted_internal(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        if self.scripts.is_empty() {
            return Ok(());
        }
        self.frame_counter = current_frame;
        gamelogic::scripting::sync_host_trigger_flags_from_snapshot(current_frame as u32);

        self.apply_pending_script_enabled_updates()?;
        if current_frame <= 2 {
            let enabled: Vec<_> = self
                .scripts
                .iter()
                .filter(|script| self.is_regular_script_eligible(script) && script.enabled)
                .map(|script| script.name.as_str())
                .collect();
            log::debug!(
                "Mission script runtime frame {} enabled scripts sample: {:?}",
                current_frame,
                enabled.into_iter().take(24).collect::<Vec<_>>()
            );
        }
        match max_scripts_per_frame {
            Some(0) => return Ok(()),
            Some(budget) => {
                let len = self.scripts.len();
                let to_evaluate = budget.min(len);
                for _ in 0..to_evaluate {
                    let index = self.next_script_index % len;
                    let group_is_eligible = self.is_regular_script_eligible(&self.scripts[index]);
                    self.evaluate_script(index, group_is_eligible)?;
                    self.apply_pending_script_enabled_updates()?;
                    self.next_script_index = (self.next_script_index + 1) % len;
                }
            }
            None => {
                self.update_full_cxx_order()?;
                self.next_script_index = 0;
            }
        }
        Ok(())
    }

    fn set_script_enabled(&mut self, name: &str, enabled: bool) -> GameLogicResult<()> {
        let script_index = self
            .script_lookup
            .get(name)
            .copied()
            .or_else(|| self.original_lookup.get(name).copied());
        let group_index = self.group_lookup.get(name).copied();

        // C++ ScriptEngine.cpp:6800-6823 finds groups and scripts separately.
        // Keep the mutation order visible to immediate/re-entrant actions:
        // ENABLE toggles group then script; DISABLE toggles script then group.
        if enabled {
            if let Some(group_index) = group_index {
                self.groups[group_index].active = true;
            }
            if let Some(script_index) = script_index {
                self.set_runtime_script_active(script_index, true);
            }
        } else {
            if let Some(script_index) = script_index {
                self.set_runtime_script_active(script_index, false);
            }
            if let Some(group_index) = group_index {
                self.groups[group_index].active = false;
            }
        }

        if let Some(script_index) = script_index {
            log::debug!(
                "Mission script runtime set '{}' enabled={} (runtime='{}')",
                name,
                enabled,
                self.scripts[script_index].name
            );
        }
        if let Some(group_index) = group_index {
            log::debug!(
                "Mission script runtime set group '{}' active={} (runtime='{}')",
                name,
                enabled,
                self.groups[group_index].name
            );
        }
        if script_index.is_none() && group_index.is_none() {
            log::warn!(
                "Enable/Disable requested for unknown script/group '{}'",
                name
            );
        }
        Ok(())
    }

    fn apply_pending_script_enabled_updates(&mut self) -> GameLogicResult<()> {
        let pending = self
            .pending_script_enabled_updates
            .lock()
            .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
            .map_err(|_| {
                GameLogicError::Configuration(
                    "Mission script enable queue mutex poisoned".to_string(),
                )
            })?;
        for (name, enabled) in pending {
            self.set_script_enabled(&name, enabled)?;
        }
        Ok(())
    }

    fn set_runtime_script_active(&mut self, script_index: usize, enabled: bool) {
        let entry = &mut self.scripts[script_index];
        entry.enabled = enabled;
        entry.script.set_active(enabled);
        if enabled {
            entry.state.completed = false;
            entry.state.next_frame_allowed = self.frame_counter;
        }
    }

    fn collect_chain(
        &mut self,
        prefix: String,
        script: Option<&Script>,
        group_index: Option<usize>,
    ) {
        let mut current = script;
        let mut ordinal = 0usize;

        while let Some(node) = current {
            let base = node.get_name().trim();
            let mut name = if base.is_empty() {
                format!("{}::Script{}", prefix, ordinal)
            } else {
                format!("{}::{}", prefix, base.replace(' ', "_"))
            };

            if self.script_lookup.contains_key(&name) {
                let suffix = format!("#{}", self.script_lookup.len());
                name.push_str(&suffix);
            }

            // C++ `findScript` compares its AsciiString name verbatim.  The
            // generated runtime path below may normalize display whitespace,
            // but action lookup must retain authored spelling and case.
            let original_key = if node.get_name().is_empty() {
                None
            } else {
                Some(node.get_name().to_string())
            };

            if let Some(ref key) = original_key {
                self.original_lookup
                    .entry(key.clone())
                    .or_insert(self.scripts.len());
            }

            self.script_lookup.insert(name.clone(), self.scripts.len());
            self.scripts.push(RuntimeScript {
                name,
                original_name: original_key,
                script: node.clone(),
                state: ScriptState::new(),
                group_index,
                is_subroutine: node.is_subroutine(),
                enabled: node.is_active(),
            });

            current = node.get_next();
            ordinal += 1;
        }
    }

    fn is_regular_script_eligible(&self, script: &RuntimeScript) -> bool {
        if script.is_subroutine {
            return false;
        }
        script.group_index.map_or(true, |group_index| {
            self.groups
                .get(group_index)
                .is_some_and(|group| group.active && !group.is_subroutine)
        })
    }

    /// C++ samples an ordinary group's active/subroutine gate when it reaches
    /// that group in `ScriptEngine::update`, then walks the whole chain.  A
    /// member that disables its own group therefore affects the next frame,
    /// not remaining siblings in the already-entered chain.
    fn update_full_cxx_order(&mut self) -> GameLogicResult<()> {
        let mut current_group = None;
        let mut entered_group_is_eligible = true;

        for index in 0..self.scripts.len() {
            let group_index = self.scripts[index].group_index;
            if group_index != current_group {
                current_group = group_index;
                entered_group_is_eligible = group_index.map_or(true, |group_index| {
                    self.groups
                        .get(group_index)
                        .is_some_and(|group| group.active && !group.is_subroutine)
                });
            }

            if !entered_group_is_eligible || self.scripts[index].is_subroutine {
                continue;
            }
            self.evaluate_script(index, true)?;
            self.apply_pending_script_enabled_updates()?;
        }
        Ok(())
    }

    fn evaluate_script(&mut self, index: usize, group_is_eligible: bool) -> GameLogicResult<()> {
        let entry = &mut self.scripts[index];
        if !group_is_eligible || entry.is_subroutine || !entry.enabled || !entry.script.is_active()
        {
            return Ok(());
        }

        if entry.script.is_one_shot() && entry.state.completed {
            return Ok(());
        }

        if self.frame_counter < entry.state.next_frame_allowed {
            return Ok(());
        }

        let condition_result = self.evaluator.evaluate_script(&mut entry.script)?;

        if condition_result && entry.script.is_one_shot() {
            entry.state.completed = true;
        } else {
            entry.state.next_frame_allowed =
                self.frame_counter + delay_frames(entry.script.delay_evaluation_seconds);
        }

        Ok(())
    }
}

pub struct MissionScriptHooks {
    runtime: Mutex<MissionScriptRuntime>,
    pending_script_enabled_updates: Arc<Mutex<Vec<(String, bool)>>>,
    messages: Mutex<Vec<String>>,
    sounds: Mutex<Vec<String>>,
    sound_events: Mutex<Vec<ScriptSoundEvent>>,
    camera_moves: Mutex<Vec<Vec3>>,
    camera_follows: Mutex<Vec<CameraFollowRequest>>,
    camera_tethers: Mutex<Vec<CameraTetherRequest>>,
    camera_path_moves: Mutex<Vec<CameraPathRequest>>,
    camera_move_to: Mutex<Vec<CameraMoveToRequest>>,
    camera_move_to_selection_requests: Mutex<Vec<()>>,
    camera_move_home_requests: Mutex<Vec<()>>,
    camera_resets: Mutex<Vec<CameraResetRequest>>,
    camera_zoom_requests: Mutex<Vec<CameraZoomRequest>>,
    camera_pitch_requests: Mutex<Vec<CameraPitchRequest>>,
    camera_rotate_requests: Mutex<Vec<CameraRotateRequest>>,
    camera_mod_final_zoom_requests: Mutex<Vec<CameraModFinalZoomRequest>>,
    camera_mod_final_pitch_requests: Mutex<Vec<CameraModFinalPitchRequest>>,
    camera_mod_freeze_time_requests: Mutex<Vec<()>>,
    camera_mod_freeze_angle_requests: Mutex<Vec<()>>,
    camera_mod_final_speed_multiplier_requests: Mutex<Vec<CameraModFinalSpeedMultiplierRequest>>,
    camera_mod_rolling_average_requests: Mutex<Vec<CameraModRollingAverageRequest>>,
    visual_speed_multiplier_requests: Mutex<Vec<VisualSpeedMultiplierRequest>>,
    script_freeze_time_requests: Mutex<Vec<bool>>,
    set_fps_limit_requests: Mutex<Vec<SetFpsLimitRequest>>,
    camera_setup_requests: Mutex<Vec<CameraSetupRequest>>,
    camera_look_toward_object_requests: Mutex<Vec<CameraLookTowardObjectRequest>>,
    camera_look_toward_waypoint_requests: Mutex<Vec<CameraLookTowardWaypointRequest>>,
    camera_mod_look_toward_requests: Mutex<Vec<CameraModLookTowardRequest>>,
    camera_mod_final_look_toward_requests: Mutex<Vec<CameraModFinalLookTowardRequest>>,
    camera_set_default_requests: Mutex<Vec<CameraSetDefaultRequest>>,
    camera_slave_mode_enable_requests: Mutex<Vec<CameraSlaveModeRequest>>,
    camera_slave_mode_disable_requests: Mutex<Vec<()>>,
    screen_shake_requests: Mutex<Vec<ScreenShakeRequest>>,
    camera_add_shaker_requests: Mutex<Vec<CameraAddShakerRequest>>,
    named_special_power_countdown_mutations: Mutex<Vec<NamedSpecialPowerCountdownMutation>>,

    popup_message_requests: Mutex<Vec<ScriptPopupMessageRequest>>,
    view_guardband_requests: Mutex<Vec<ViewGuardbandRequest>>,
    camera_bw_mode_requests: Mutex<Vec<CameraBwModeRequest>>,
    skybox_enabled_updates: Mutex<Vec<bool>>,
    camera_motion_blur_requests: Mutex<Vec<CameraMotionBlurRequest>>,
    cameo_flash_requests: Mutex<Vec<CameoFlashRequest>>,
    named_timer_mutations: Mutex<Vec<NamedTimerMutation>>,
    named_timer_display_updates: Mutex<Vec<bool>>,
    superweapon_display_enabled_updates: Mutex<Vec<bool>>,
    superweapon_object_display_mutations: Mutex<Vec<SuperweaponObjectDisplayMutation>>,
    cinematic_text: Mutex<Vec<(String, String, i32)>>,
    military_captions: Mutex<Vec<MilitaryCaptionRequest>>,
    letterbox_events: Mutex<Vec<bool>>,
    movie_requests: Mutex<Vec<String>>,
    radar_movie_requests: Mutex<Vec<String>>,
    objective_updates: Mutex<Vec<ObjectiveUpdate>>,
    effect_requests: Mutex<Vec<ScriptEffectRequest>>,
    radar_event_requests: Mutex<Vec<RadarScriptEventRequest>>,
    radar_enabled_updates: Mutex<Vec<bool>>,
    radar_forced_updates: Mutex<Vec<bool>>,
    weather_visibility_updates: Mutex<Vec<bool>>,
    music_stop_requests: Mutex<Vec<()>>,
    oversize_terrain_requests: Mutex<Vec<i32>>,
    border_shroud_levels: Mutex<Vec<u8>>,
    camera_movement_finished: AtomicBool,
    frame_counter: AtomicU64,
    speech_complete_frame: Mutex<HashMap<String, u64>>,
    speech_handles: Mutex<HashMap<String, Vec<u32>>>,
    audio_complete_frame: Mutex<HashMap<String, u64>>,
}

impl MissionScriptHooks {
    pub fn new() -> GameLogicResult<Arc<Self>> {
        let pending_script_enabled_updates = Arc::new(Mutex::new(Vec::new()));
        Ok(Arc::new(Self {
            runtime: Mutex::new(
                MissionScriptRuntime::new_with_pending_script_enabled_updates(Arc::clone(
                    &pending_script_enabled_updates,
                ))?,
            ),
            pending_script_enabled_updates,
            messages: Mutex::new(Vec::new()),
            sounds: Mutex::new(Vec::new()),
            sound_events: Mutex::new(Vec::new()),
            camera_moves: Mutex::new(Vec::new()),
            camera_follows: Mutex::new(Vec::new()),
            camera_tethers: Mutex::new(Vec::new()),
            camera_path_moves: Mutex::new(Vec::new()),
            camera_move_to: Mutex::new(Vec::new()),
            camera_move_to_selection_requests: Mutex::new(Vec::new()),
            camera_move_home_requests: Mutex::new(Vec::new()),
            camera_resets: Mutex::new(Vec::new()),
            camera_zoom_requests: Mutex::new(Vec::new()),
            camera_pitch_requests: Mutex::new(Vec::new()),
            camera_rotate_requests: Mutex::new(Vec::new()),
            camera_mod_final_zoom_requests: Mutex::new(Vec::new()),
            camera_mod_final_pitch_requests: Mutex::new(Vec::new()),
            camera_mod_freeze_time_requests: Mutex::new(Vec::new()),
            camera_mod_freeze_angle_requests: Mutex::new(Vec::new()),
            camera_mod_final_speed_multiplier_requests: Mutex::new(Vec::new()),
            camera_mod_rolling_average_requests: Mutex::new(Vec::new()),
            visual_speed_multiplier_requests: Mutex::new(Vec::new()),
            script_freeze_time_requests: Mutex::new(Vec::new()),
            set_fps_limit_requests: Mutex::new(Vec::new()),
            camera_setup_requests: Mutex::new(Vec::new()),
            camera_look_toward_object_requests: Mutex::new(Vec::new()),
            camera_look_toward_waypoint_requests: Mutex::new(Vec::new()),
            camera_mod_look_toward_requests: Mutex::new(Vec::new()),
            camera_mod_final_look_toward_requests: Mutex::new(Vec::new()),
            camera_set_default_requests: Mutex::new(Vec::new()),
            camera_slave_mode_enable_requests: Mutex::new(Vec::new()),
            camera_slave_mode_disable_requests: Mutex::new(Vec::new()),
            screen_shake_requests: Mutex::new(Vec::new()),
            camera_add_shaker_requests: Mutex::new(Vec::new()),
            named_special_power_countdown_mutations: Mutex::new(Vec::new()),

            popup_message_requests: Mutex::new(Vec::new()),
            view_guardband_requests: Mutex::new(Vec::new()),
            camera_bw_mode_requests: Mutex::new(Vec::new()),
            skybox_enabled_updates: Mutex::new(Vec::new()),
            camera_motion_blur_requests: Mutex::new(Vec::new()),
            cameo_flash_requests: Mutex::new(Vec::new()),
            named_timer_mutations: Mutex::new(Vec::new()),
            named_timer_display_updates: Mutex::new(Vec::new()),
            superweapon_display_enabled_updates: Mutex::new(Vec::new()),
            superweapon_object_display_mutations: Mutex::new(Vec::new()),
            cinematic_text: Mutex::new(Vec::new()),
            military_captions: Mutex::new(Vec::new()),
            letterbox_events: Mutex::new(Vec::new()),
            movie_requests: Mutex::new(Vec::new()),
            radar_movie_requests: Mutex::new(Vec::new()),
            objective_updates: Mutex::new(Vec::new()),
            effect_requests: Mutex::new(Vec::new()),
            radar_event_requests: Mutex::new(Vec::new()),
            radar_enabled_updates: Mutex::new(Vec::new()),
            radar_forced_updates: Mutex::new(Vec::new()),
            weather_visibility_updates: Mutex::new(Vec::new()),
            music_stop_requests: Mutex::new(Vec::new()),
            oversize_terrain_requests: Mutex::new(Vec::new()),
            border_shroud_levels: Mutex::new(Vec::new()),
            camera_movement_finished: AtomicBool::new(true),
            frame_counter: AtomicU64::new(0),
            speech_complete_frame: Mutex::new(HashMap::new()),
            speech_handles: Mutex::new(HashMap::new()),
            audio_complete_frame: Mutex::new(HashMap::new()),
        }))
    }

    pub fn install_lists(&self, lists: &[ScriptList]) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.install_lists(lists);
        }
    }

    /// C++ `ScriptEngine::newMap` fade-in from black (33-frame `FADE_MULTIPLY`).
    /// Live map load calls this after leftover `reset()` so the overlay starts
    /// even when the crate engine handle is taken out for `update()`.
    pub fn start_new_map_fade(&self) {
        if let Ok(mut engine_guard) = gamelogic::scripting::engine::get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.new_map();
            }
        }
    }

    /// Advance hook completion clocks without walking scripts.
    ///
    /// C++ GameLogic.cpp:3600 has one `TheScriptEngine->UPDATE()` per logic
    /// frame.  Live host evaluation is crate `ScriptEngine::update`; this only
    /// stamps `frame_counter` so video/speech/audio/music completion queries
    /// stay frame-accurate after the second walker was removed (hq-fxq1).
    pub fn note_logic_frame(&self, frame: u64) {
        self.frame_counter.store(frame, Ordering::Relaxed);
    }

    pub fn update(&self, frame: u64) -> GameLogicResult<()> {
        self.update_budgeted(frame, None)
    }

    pub fn update_budgeted(
        &self,
        frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.frame_counter.store(frame, Ordering::Relaxed);
        let mut runtime = self.runtime.lock().map_err(|_| {
            GameLogicError::Configuration("Mission script runtime mutex poisoned".to_string())
        })?;
        runtime.update_budgeted(frame, max_scripts_per_frame)?;
        Ok(())
    }

    pub fn set_script_enabled(&self, name: &str, enabled: bool) -> GameLogicResult<()> {
        let mut queue = self.pending_script_enabled_updates.lock().map_err(|_| {
            GameLogicError::Configuration("Mission script enable queue mutex poisoned".to_string())
        })?;
        queue.push((name.to_string(), enabled));
        Ok(())
    }

    pub fn push_message(&self, text: String) {
        if let Ok(mut queue) = self.messages.lock() {
            let localized = localization::localize_with_args(
                "hud.script.broadcast",
                "Transmission: {message}",
                &[("message", text.as_str())],
            );
            queue.push(localized);
        }
    }

    pub fn push_sound(&self, name: String) {
        if let Ok(mut queue) = self.sounds.lock() {
            queue.push(name);
        }
    }

    pub fn push_sound_event(&self, event: ScriptSoundEvent) {
        if let Ok(mut queue) = self.sound_events.lock() {
            queue.push(event);
        }
    }

    pub fn push_camera_move(&self, position: Vec3) {
        if let Ok(mut queue) = self.camera_moves.lock() {
            queue.push(position);
        }
    }

    pub fn push_camera_tether(&self, request: CameraTetherRequest) {
        if let Ok(mut queue) = self.camera_tethers.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_follow(&self, request: CameraFollowRequest) {
        if let Ok(mut queue) = self.camera_follows.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_path_move(&self, request: CameraPathRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_path_moves.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_move_to(&self, request: CameraMoveToRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_move_to.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_move_to_selection(&self) {
        if let Ok(mut queue) = self.camera_move_to_selection_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_camera_move_home(&self) {
        if let Ok(mut queue) = self.camera_move_home_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_camera_reset(&self, request: CameraResetRequest) {
        if let Ok(mut queue) = self.camera_resets.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_zoom(&self, request: CameraZoomRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_zoom_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_pitch(&self, request: CameraPitchRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_pitch_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_rotate(&self, request: CameraRotateRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_rotate_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_final_zoom(&self, request: CameraModFinalZoomRequest) {
        if let Ok(mut queue) = self.camera_mod_final_zoom_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_final_pitch(&self, request: CameraModFinalPitchRequest) {
        if let Ok(mut queue) = self.camera_mod_final_pitch_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_freeze_time(&self) {
        if let Ok(mut queue) = self.camera_mod_freeze_time_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_camera_mod_freeze_angle(&self) {
        if let Ok(mut queue) = self.camera_mod_freeze_angle_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_camera_mod_final_speed_multiplier(
        &self,
        request: CameraModFinalSpeedMultiplierRequest,
    ) {
        if let Ok(mut queue) = self.camera_mod_final_speed_multiplier_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_rolling_average(&self, request: CameraModRollingAverageRequest) {
        if let Ok(mut queue) = self.camera_mod_rolling_average_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_visual_speed_multiplier(&self, request: VisualSpeedMultiplierRequest) {
        if let Ok(mut queue) = self.visual_speed_multiplier_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_script_freeze_time(&self, freeze: bool) {
        if let Ok(mut queue) = self.script_freeze_time_requests.lock() {
            queue.push(freeze);
        }
    }

    pub fn push_set_fps_limit(&self, request: SetFpsLimitRequest) {
        if let Ok(mut queue) = self.set_fps_limit_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_setup(&self, request: CameraSetupRequest) {
        if let Ok(mut queue) = self.camera_setup_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_look_toward_object(&self, request: CameraLookTowardObjectRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_look_toward_object_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_look_toward_waypoint(&self, request: CameraLookTowardWaypointRequest) {
        self.camera_movement_finished
            .store(false, Ordering::Relaxed);
        if let Ok(mut queue) = self.camera_look_toward_waypoint_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_look_toward(&self, request: CameraModLookTowardRequest) {
        if let Ok(mut queue) = self.camera_mod_look_toward_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_mod_final_look_toward(&self, request: CameraModFinalLookTowardRequest) {
        if let Ok(mut queue) = self.camera_mod_final_look_toward_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_set_default(&self, request: CameraSetDefaultRequest) {
        if let Ok(mut queue) = self.camera_set_default_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_slave_mode_enable(&self, request: CameraSlaveModeRequest) {
        if let Ok(mut queue) = self.camera_slave_mode_enable_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_slave_mode_disable(&self) {
        if let Ok(mut queue) = self.camera_slave_mode_disable_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_screen_shake(&self, request: ScreenShakeRequest) {
        if let Ok(mut queue) = self.screen_shake_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_add_shaker(&self, request: CameraAddShakerRequest) {
        if let Ok(mut queue) = self.camera_add_shaker_requests.lock() {
            queue.push(request);
        }
    }

    pub fn set_camera_movement_finished(&self, finished: bool) {
        self.camera_movement_finished
            .store(finished, Ordering::Relaxed);
    }

    pub fn is_camera_movement_finished(&self) -> bool {
        self.camera_movement_finished.load(Ordering::Relaxed)
    }

    pub fn push_cinematic_text(&self, text: String, font: String, duration_seconds: i32) {
        if let Ok(mut queue) = self.cinematic_text.lock() {
            queue.push((text, font, duration_seconds));
        }
    }

    pub fn push_military_caption(&self, text: String, duration_ms: i32) {
        if let Ok(mut queue) = self.military_captions.lock() {
            queue.push(MilitaryCaptionRequest { text, duration_ms });
        }
    }

    pub fn push_letterbox(&self, enabled: bool) {
        if let Ok(mut queue) = self.letterbox_events.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_movie_request(&self, filename: String) {
        if let Ok(mut queue) = self.movie_requests.lock() {
            queue.push(filename);
        }
    }

    pub fn push_radar_movie_request(&self, filename: String) {
        if let Ok(mut queue) = self.radar_movie_requests.lock() {
            queue.push(filename);
        }
    }

    pub fn push_objective_update(&self, update: ObjectiveUpdate) {
        if let Ok(mut queue) = self.objective_updates.lock() {
            queue.push(update);
        }
    }

    pub fn push_effect_request(&self, request: ScriptEffectRequest) {
        if let Ok(mut queue) = self.effect_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_radar_event_request(&self, request: RadarScriptEventRequest) {
        if let Ok(mut queue) = self.radar_event_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_radar_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.radar_enabled_updates.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_radar_forced(&self, forced: bool) {
        if let Ok(mut queue) = self.radar_forced_updates.lock() {
            queue.push(forced);
        }
    }

    pub fn push_weather_visible(&self, visible: bool) {
        if let Ok(mut queue) = self.weather_visibility_updates.lock() {
            queue.push(visible);
        }
    }

    pub fn push_popup_message(&self, mut request: ScriptPopupMessageRequest) {
        // Keep this opaque and monotonic rather than deriving authority from
        // popup text/layout fields. Acknowledge only the exact live instance.
        request.popup_generation = next_live_popup_generation();
        if let Ok(mut queue) = self.popup_message_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_view_guardband(&self, request: ViewGuardbandRequest) {
        if let Ok(mut queue) = self.view_guardband_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_bw_mode(&self, request: CameraBwModeRequest) {
        if let Ok(mut queue) = self.camera_bw_mode_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_skybox_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.skybox_enabled_updates.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_camera_motion_blur(&self, request: CameraMotionBlurRequest) {
        if let Ok(mut queue) = self.camera_motion_blur_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_cameo_flash(&self, request: CameoFlashRequest) {
        if let Ok(mut queue) = self.cameo_flash_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_named_timer_mutation(&self, request: NamedTimerMutation) {
        if let Ok(mut queue) = self.named_timer_mutations.lock() {
            queue.push(request);
        }
    }

    pub fn push_named_timer_display(&self, show: bool) {
        if let Ok(mut queue) = self.named_timer_display_updates.lock() {
            queue.push(show);
        }
    }

    pub fn push_superweapon_display_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.superweapon_display_enabled_updates.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_named_special_power_countdown_mutation(
        &self,
        request: NamedSpecialPowerCountdownMutation,
    ) {
        if let Ok(mut queue) = self.named_special_power_countdown_mutations.lock() {
            queue.push(request);
        }
    }

    pub fn push_superweapon_object_display_mutation(
        &self,
        request: SuperweaponObjectDisplayMutation,
    ) {
        if let Ok(mut queue) = self.superweapon_object_display_mutations.lock() {
            queue.push(request);
        }
    }

    pub fn push_music_stop(&self) {
        if let Ok(mut queue) = self.music_stop_requests.lock() {
            queue.push(());
        }
    }

    pub fn push_oversize_terrain(&self, amount: i32) {
        if let Ok(mut queue) = self.oversize_terrain_requests.lock() {
            queue.push(amount);
        }
    }

    pub fn note_speech_started(&self, name: &str) {
        self.note_speech_started_with_handle(name, 0);
    }

    pub fn note_speech_started_with_handle(&self, name: &str, handle: u32) {
        if name.trim().is_empty() {
            return;
        }
        let now = self.frame_counter.load(Ordering::Relaxed);
        if let Ok(mut map) = self.speech_complete_frame.lock() {
            map.insert(name.to_string(), speech_completion_frame(now, name));
        }
        if handle != 0 {
            if let Ok(mut handles) = self.speech_handles.lock() {
                handles.entry(name.to_string()).or_default().push(handle);
            }
        }
    }

    pub fn note_audio_started(&self, name: &str) {
        // C++ isAudioComplete starts the TheAudio length timer on first query,
        // not on play. Do not stamp now+1 (that made HAS_FINISHED_AUDIO true
        // next frame).
        let _ = name;
    }

    pub fn note_music_started(&self, name: &str) {
        // C++ MUSIC_TRACK_HAS_COMPLETED is TheAudio loop count, not a frame stamp.
        let _ = name;
    }

    pub fn mark_music_stopped(&self) {
        // C++ stop-music does not mark hasMusicTrackCompleted; Miles walks
        // playing streams only. Stopping a track makes the condition false.
    }

    pub fn is_video_complete(&self, name: &str, flush: bool) -> bool {
        // C++ ScriptEngine::isVideoComplete: true only if name is on
        // m_completedVideo. Untracked / never-finished names stay false.
        gamelogic::scripting::engine::with_script_engine_ref(|engine| {
            engine.is_video_complete(name, flush)
        })
        .unwrap_or(false)
    }

    pub fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        if name.trim().is_empty() {
            return false;
        }
        // Leftover GameClient `is_named_audio_complete`: a live Miles/rodio
        // handle is still playing, so the line is not finished yet.
        if let Ok(mut handles) = self.speech_handles.lock() {
            if let Some(pending) = handles.get_mut(name) {
                match gamelogic::helpers::TheAudio::get() {
                    Some(audio) => pending.retain(|handle| audio.is_currently_playing(*handle)),
                    None => pending.clear(),
                }
                if !pending.is_empty() {
                    return false;
                }
                if flush {
                    handles.remove(name);
                }
            }
        }
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut map) = self.speech_complete_frame.lock() else {
            return true;
        };
        let done_frame = match map.get(name).copied() {
            Some(done_frame) => done_frame,
            None => {
                // C++ first HAS_FINISHED_SPEECH query starts the TheAudio timer.
                let done_frame = speech_completion_frame(now, name);
                map.insert(name.to_string(), done_frame);
                done_frame
            }
        };
        let done = now >= done_frame;
        if done && flush {
            map.remove(name);
        }
        done
    }

    pub fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        if name.trim().is_empty() {
            return false;
        }
        // C++ ScriptEngine::isAudioComplete: first query starts leftover
        // TheAudio length timer; true only after that frame. Use the live
        // frame clock — leftover TheGameLogic::get_frame is not the host.
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut map) = self.audio_complete_frame.lock() else {
            return false;
        };
        let done_frame = match map.get(name).copied() {
            Some(done_frame) => done_frame,
            None => {
                let done_frame = speech_completion_frame(now, name);
                map.insert(name.to_string(), done_frame);
                done_frame
            }
        };
        let done = now >= done_frame;
        if done && flush {
            map.remove(name);
        }
        done
    }

    pub fn has_music_track_completed(&self, track: &str, times: i32) -> bool {
        let key = track.trim();
        if key.is_empty() {
            return false;
        }
        // C++ TheAudio->hasMusicTrackCompleted(track, N). Unplayed / missing = false.
        gamelogic::helpers::TheAudio::get()
            .map(|audio| audio.has_music_track_completed(key, times))
            .unwrap_or(false)
    }

    pub fn drain_messages(&self) -> Vec<String> {
        self.messages
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_sounds(&self) -> Vec<String> {
        self.sounds
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_sound_events(&self) -> Vec<ScriptSoundEvent> {
        self.sound_events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_moves(&self) -> Vec<Vec3> {
        self.camera_moves
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_follows(&self) -> Vec<CameraFollowRequest> {
        self.camera_follows
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_tethers(&self) -> Vec<CameraTetherRequest> {
        self.camera_tethers
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_path_moves(&self) -> Vec<CameraPathRequest> {
        self.camera_path_moves
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_to(&self) -> Vec<CameraMoveToRequest> {
        self.camera_move_to
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_to_selection_requests(&self) -> Vec<()> {
        self.camera_move_to_selection_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_move_home_requests(&self) -> Vec<()> {
        self.camera_move_home_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_resets(&self) -> Vec<CameraResetRequest> {
        self.camera_resets
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_zoom_requests(&self) -> Vec<CameraZoomRequest> {
        self.camera_zoom_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_pitch_requests(&self) -> Vec<CameraPitchRequest> {
        self.camera_pitch_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_rotate_requests(&self) -> Vec<CameraRotateRequest> {
        self.camera_rotate_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_zoom_requests(&self) -> Vec<CameraModFinalZoomRequest> {
        self.camera_mod_final_zoom_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_pitch_requests(&self) -> Vec<CameraModFinalPitchRequest> {
        self.camera_mod_final_pitch_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_freeze_time_requests(&self) -> Vec<()> {
        self.camera_mod_freeze_time_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_freeze_angle_requests(&self) -> Vec<()> {
        self.camera_mod_freeze_angle_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_speed_multiplier_requests(
        &self,
    ) -> Vec<CameraModFinalSpeedMultiplierRequest> {
        self.camera_mod_final_speed_multiplier_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_rolling_average_requests(&self) -> Vec<CameraModRollingAverageRequest> {
        self.camera_mod_rolling_average_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_visual_speed_multiplier_requests(&self) -> Vec<VisualSpeedMultiplierRequest> {
        self.visual_speed_multiplier_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_script_freeze_time_requests(&self) -> Vec<bool> {
        self.script_freeze_time_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_set_fps_limit_requests(&self) -> Vec<SetFpsLimitRequest> {
        self.set_fps_limit_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_setup_requests(&self) -> Vec<CameraSetupRequest> {
        self.camera_setup_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_look_toward_object_requests(&self) -> Vec<CameraLookTowardObjectRequest> {
        self.camera_look_toward_object_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_look_toward_waypoint_requests(
        &self,
    ) -> Vec<CameraLookTowardWaypointRequest> {
        self.camera_look_toward_waypoint_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_look_toward_requests(&self) -> Vec<CameraModLookTowardRequest> {
        self.camera_mod_look_toward_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_mod_final_look_toward_requests(
        &self,
    ) -> Vec<CameraModFinalLookTowardRequest> {
        self.camera_mod_final_look_toward_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_set_default_requests(&self) -> Vec<CameraSetDefaultRequest> {
        self.camera_set_default_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_slave_mode_enable_requests(&self) -> Vec<CameraSlaveModeRequest> {
        self.camera_slave_mode_enable_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_slave_mode_disable_requests(&self) -> Vec<()> {
        self.camera_slave_mode_disable_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_screen_shake_requests(&self) -> Vec<ScreenShakeRequest> {
        self.screen_shake_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_add_shaker_requests(&self) -> Vec<CameraAddShakerRequest> {
        self.camera_add_shaker_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_cinematic_text(&self) -> Vec<(String, String, i32)> {
        self.cinematic_text
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_military_captions(&self) -> Vec<MilitaryCaptionRequest> {
        self.military_captions
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_letterbox_events(&self) -> Vec<bool> {
        self.letterbox_events
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_movie_requests(&self) -> Vec<String> {
        self.movie_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_movie_requests(&self) -> Vec<String> {
        self.radar_movie_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_objective_updates(&self) -> Vec<ObjectiveUpdate> {
        self.objective_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_effect_requests(&self) -> Vec<ScriptEffectRequest> {
        self.effect_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_event_requests(&self) -> Vec<RadarScriptEventRequest> {
        self.radar_event_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_enabled_updates(&self) -> Vec<bool> {
        self.radar_enabled_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_radar_forced_updates(&self) -> Vec<bool> {
        self.radar_forced_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_weather_visibility_updates(&self) -> Vec<bool> {
        self.weather_visibility_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_popup_message_requests(&self) -> Vec<ScriptPopupMessageRequest> {
        self.popup_message_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_view_guardband_requests(&self) -> Vec<ViewGuardbandRequest> {
        self.view_guardband_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_bw_mode_requests(&self) -> Vec<CameraBwModeRequest> {
        self.camera_bw_mode_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_skybox_enabled_updates(&self) -> Vec<bool> {
        self.skybox_enabled_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_camera_motion_blur_requests(&self) -> Vec<CameraMotionBlurRequest> {
        self.camera_motion_blur_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_cameo_flash_requests(&self) -> Vec<CameoFlashRequest> {
        self.cameo_flash_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_timer_mutations(&self) -> Vec<NamedTimerMutation> {
        self.named_timer_mutations
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_timer_display_updates(&self) -> Vec<bool> {
        self.named_timer_display_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_superweapon_display_enabled_updates(&self) -> Vec<bool> {
        self.superweapon_display_enabled_updates
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_named_special_power_countdown_mutations(
        &self,
    ) -> Vec<NamedSpecialPowerCountdownMutation> {
        self.named_special_power_countdown_mutations
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_superweapon_object_display_mutations(
        &self,
    ) -> Vec<SuperweaponObjectDisplayMutation> {
        self.superweapon_object_display_mutations
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_music_stop_requests(&self) -> Vec<()> {
        self.music_stop_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn push_border_shroud_level(&self, level: u8) {
        if let Ok(mut queue) = self.border_shroud_levels.lock() {
            queue.push(level);
        }
    }

    pub fn drain_border_shroud_levels(&self) -> Vec<u8> {
        self.border_shroud_levels
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }

    pub fn drain_oversize_terrain_requests(&self) -> Vec<i32> {
        self.oversize_terrain_requests
            .lock()
            .map(|mut q| q.drain(..).collect())
            .unwrap_or_default()
    }
}

pub struct MissionScriptActionHandler {
    hooks: Arc<MissionScriptHooks>,
}

impl MissionScriptActionHandler {
    pub fn new(hooks: Arc<MissionScriptHooks>) -> Self {
        Self { hooks }
    }

    pub fn hooks(&self) -> Arc<MissionScriptHooks> {
        Arc::clone(&self.hooks)
    }

    fn local_player_index() -> Option<u32> {
        let players = gamelogic::player::player_list().read().ok()?;
        let index = players.get_local_player_index();
        (index >= 0).then_some(index as u32)
    }

    /// C++ `ScriptActions::doMusicTrackChange` (ScriptActions.cpp:3271-3286):
    /// `TheAudio->removeAudioEvent(AHSV_StopTheMusic[Fade])` then
    /// `TheAudio->addAudioEvent` of the named track (GameMusic / MusicManager).
    fn play_music_track_through_the_audio(track: &str, fade_out: bool, fade_in: bool) {
        const AHSV_STOP_THE_MUSIC: u32 = 0xFFFF_FFF0;
        const AHSV_STOP_THE_MUSIC_FADE: u32 = 0xFFFF_FFF1;

        let Some(audio) = gamelogic::helpers::TheAudio::get() else {
            return;
        };
        audio.remove_audio_event(if fade_out {
            AHSV_STOP_THE_MUSIC_FADE
        } else {
            AHSV_STOP_THE_MUSIC
        });

        let mut event = gamelogic::common::audio::AudioEventRts::new(track);
        event.set_should_fade(fade_in);
        if let Some(player_index) = Self::local_player_index() {
            event.set_player_index(player_index);
        }
        let _handle = audio.add_audio_event(&event);
    }

    /// C++ `ScriptActions::doSpeechPlay` (ScriptActions.cpp:2743-2764):
    /// `AudioEventRTS` + `setIsLogicalAudio(true)` + local player index +
    /// `setUninterruptable(!allowOverlap)` + `TheAudio->addAudioEvent`.
    fn play_speech_through_the_audio(name: &str, allow_overlap: bool) -> u32 {
        let Some(audio) = gamelogic::helpers::TheAudio::get() else {
            return 0;
        };
        let mut event = gamelogic::common::audio::AudioEventRts::new(name);
        event.set_is_logical_audio(true);
        event.set_uninterruptable(!allow_overlap);
        if let Some(player_index) = Self::local_player_index() {
            event.set_player_index(player_index);
        }
        audio.add_audio_event(&event)
    }

    /// C++ `ScriptActions::doSoundPlayFromNamed` (ScriptActions.cpp:2723-2733):
    /// `AudioEventRTS(soundName, pUnit->getID())` + `setIsLogicalAudio(true)`.
    fn play_named_sound_through_the_audio(name: &str, object_id: u32) -> u32 {
        let Some(audio) = gamelogic::helpers::TheAudio::get() else {
            return 0;
        };
        let mut event = gamelogic::common::audio::AudioEventRts::new(name);
        event.set_object_id(object_id);
        event.set_is_logical_audio(true);
        audio.add_audio_event(&event)
    }
}

impl ScriptActionHandler for MissionScriptActionHandler {
    fn enable_script(&self, name: &str, enabled: bool) -> GameLogicResult<()> {
        self.hooks.set_script_enabled(name, enabled)
    }

    fn display_text(&self, text: &str) -> GameLogicResult<()> {
        self.hooks.push_message(text.to_string());
        Ok(())
    }

    fn display_cinematic_text(
        &self,
        text: &str,
        font_type: &str,
        duration_seconds: i32,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_cinematic_text(text.to_string(), font_type.to_string(), duration_seconds);
        Ok(())
    }

    fn set_border_shroud_level(&self, level: u8) -> GameLogicResult<()> {
        self.hooks.push_border_shroud_level(level);
        Ok(())
    }

    fn oversize_terrain(&self, amount: i32) -> GameLogicResult<()> {
        self.hooks.push_oversize_terrain(amount);
        Ok(())
    }

    fn military_caption(&self, text: &str, duration_ms: i32) -> GameLogicResult<()> {
        self.hooks
            .push_military_caption(text.to_string(), duration_ms);
        Ok(())
    }

    fn play_sound_effect(&self, sound: &str) -> GameLogicResult<()> {
        // Live GAME_SHELL installs this handler (initialize_scripts), not
        // GameClientScriptActionHandler. C++ PLAY_SOUND_EFFECT always reaches
        // TheAudio via doPlaySoundEffect (setIsLogicalAudio + local player).
        // Do not drain a second unlocal leftover_world_sfx_event / rodio play.
        let result = game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .play_sound_effect(sound);
        self.hooks.note_audio_started(sound);
        result
    }

    fn play_sound_effect_at(&self, sound: &str, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        let result = game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .play_sound_effect_at(sound, x, y, z);
        self.hooks.note_audio_started(sound);
        result
    }

    fn move_camera(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        static DEBUG_CAMERA_MOVE_LOGS: AtomicUsize = AtomicUsize::new(0);
        let position = camera_coord3d_to_world(x, y, z);
        if DEBUG_CAMERA_MOVE_LOGS.fetch_add(1, Ordering::Relaxed) < 16 {
            eprintln!(
                "DEBUG_SHELL_CAMERA_ACTION: move_camera raw=({x:.3}, {y:.3}, {z:.3}) world={position:?}"
            );
        }
        self.hooks.push_camera_move(position);
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
        static DEBUG_CAMERA_MOVE_TO_LOGS: AtomicUsize = AtomicUsize::new(0);
        let position = camera_coord3d_to_world(x, y, z);
        if DEBUG_CAMERA_MOVE_TO_LOGS.fetch_add(1, Ordering::Relaxed) < 16 {
            eprintln!(
                "DEBUG_SHELL_CAMERA_ACTION: move_camera_to raw=({x:.3}, {y:.3}, {z:.3}) world={position:?} seconds={seconds:.3}"
            );
        }
        if seconds <= 0.0 {
            self.hooks.push_camera_move(position);
            return Ok(());
        }
        self.hooks.push_camera_move_to(CameraMoveToRequest {
            position,
            seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn move_camera_along_waypoint_path(
        &self,
        waypoint_path: &str,
        seconds: f32,
        camera_stutter_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_path_move(CameraPathRequest {
            waypoint: waypoint_path.to_string(),
            seconds,
            camera_stutter_seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn move_camera_to_selection(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_move_to_selection();
        Ok(())
    }

    fn camera_move_home(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_move_home();
        Ok(())
    }

    fn is_camera_movement_finished(&self) -> bool {
        self.hooks.is_camera_movement_finished()
    }

    fn camera_follow_object(
        &self,
        object_id: gamelogic::common::ObjectID,
        snap_to_unit: bool,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_follow(CameraFollowRequest {
            object_id,
            snap_to_unit,
        });
        Ok(())
    }

    fn camera_tether_object(
        &self,
        object_id: gamelogic::common::ObjectID,
        snap_to_unit: bool,
        play: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_tether(CameraTetherRequest {
            object_id,
            snap_to_unit,
            play,
        });
        Ok(())
    }

    fn stop_camera_follow(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_follow(CameraFollowRequest {
            object_id: 0,
            snap_to_unit: false,
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
        self.hooks.push_camera_reset(CameraResetRequest {
            position: camera_coord3d_to_world(x, y, z),
            duration_seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn set_camera_zoom(&self, zoom: f32, duration_seconds: f32) -> GameLogicResult<()> {
        self.hooks.push_camera_zoom(CameraZoomRequest {
            zoom,
            duration_seconds,
            ease_in_seconds: 0.0,
            ease_out_seconds: 0.0,
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
        self.hooks.push_camera_zoom(CameraZoomRequest {
            zoom,
            duration_seconds: seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn set_camera_pitch(
        &self,
        pitch: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_pitch(CameraPitchRequest {
            pitch,
            duration_seconds: seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn rotate_camera(
        &self,
        rotations: f32,
        seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_rotate(CameraRotateRequest {
            rotations,
            duration_seconds: seconds,
            ease_in_seconds,
            ease_out_seconds,
        });
        Ok(())
    }

    fn camera_mod_set_final_zoom(
        &self,
        zoom: f32,
        ease_in: f32,
        ease_out: f32,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_final_zoom(CameraModFinalZoomRequest {
                zoom,
                ease_in,
                ease_out,
            });
        Ok(())
    }

    fn camera_mod_set_final_pitch(
        &self,
        pitch: f32,
        ease_in: f32,
        ease_out: f32,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_final_pitch(CameraModFinalPitchRequest {
                pitch,
                ease_in,
                ease_out,
            });
        Ok(())
    }

    fn camera_mod_freeze_time(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_mod_freeze_time();
        Ok(())
    }

    fn camera_mod_freeze_angle(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_mod_freeze_angle();
        Ok(())
    }

    fn camera_mod_set_final_speed_multiplier(&self, multiplier: i32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_final_speed_multiplier(CameraModFinalSpeedMultiplierRequest {
                multiplier,
            });
        Ok(())
    }

    fn camera_mod_set_rolling_average(&self, frames: i32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_rolling_average(CameraModRollingAverageRequest { frames });
        Ok(())
    }

    fn set_visual_speed_multiplier(&self, multiplier: i32) -> GameLogicResult<()> {
        self.hooks
            .push_visual_speed_multiplier(VisualSpeedMultiplierRequest { multiplier });
        Ok(())
    }

    fn freeze_time(&self) -> GameLogicResult<()> {
        self.hooks.push_script_freeze_time(true);
        Ok(())
    }

    fn unfreeze_time(&self) -> GameLogicResult<()> {
        self.hooks.push_script_freeze_time(false);
        Ok(())
    }

    fn set_fps_limit(&self, fps: i32) -> GameLogicResult<()> {
        self.hooks.push_set_fps_limit(SetFpsLimitRequest { fps });
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
        self.hooks.push_popup_message(ScriptPopupMessageRequest {
            message: message.to_string(),
            x_percent,
            y_percent,
            width,
            pause,
            pause_music,
            popup_generation: 0,
        });
        Ok(())
    }

    fn resize_view_guardband(&self, gbx: f32, gby: f32) -> GameLogicResult<()> {
        self.hooks.push_view_guardband(ViewGuardbandRequest {
            x_bias: gbx,
            y_bias: gby,
        });
        Ok(())
    }

    fn set_camera_bw_mode(&self, enabled: bool, frames: i32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_bw_mode(CameraBwModeRequest { enabled, frames });
        Ok(())
    }

    fn set_skybox_enabled(&self, enabled: bool) -> GameLogicResult<()> {
        self.hooks.push_skybox_enabled(enabled);
        Ok(())
    }

    fn camera_motion_blur(&self, zoom_in: bool, saturate: bool) -> GameLogicResult<()> {
        self.hooks
            .push_camera_motion_blur(CameraMotionBlurRequest::Basic { zoom_in, saturate });
        Ok(())
    }

    fn camera_motion_blur_jump(
        &self,
        x: f32,
        y: f32,
        z: f32,
        saturate: bool,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_motion_blur(CameraMotionBlurRequest::Jump {
                position: camera_coord3d_to_world(x, y, z),
                saturate,
            });
        Ok(())
    }

    fn camera_motion_blur_follow(&self, amount: i32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_motion_blur(CameraMotionBlurRequest::Follow { amount });
        Ok(())
    }

    fn camera_motion_blur_end_follow(&self) -> GameLogicResult<()> {
        self.hooks
            .push_camera_motion_blur(CameraMotionBlurRequest::EndFollow);
        Ok(())
    }

    fn cameo_flash(&self, command_button_name: &str, flash_count: i32) -> GameLogicResult<()> {
        self.hooks.push_cameo_flash(CameoFlashRequest {
            command_button_name: command_button_name.to_string(),
            flash_count: flash_count.max(0),
        });
        Ok(())
    }

    fn add_named_timer(&self, name: &str, text: &str, countdown: bool) -> GameLogicResult<()> {
        self.hooks
            .push_named_timer_mutation(NamedTimerMutation::Add {
                name: name.to_string(),
                text: text.to_string(),
                countdown,
            });
        Ok(())
    }

    fn remove_named_timer(&self, name: &str) -> GameLogicResult<()> {
        self.hooks
            .push_named_timer_mutation(NamedTimerMutation::Remove {
                name: name.to_string(),
            });
        Ok(())
    }

    fn show_named_timer_display(&self, show: bool) -> GameLogicResult<()> {
        self.hooks.push_named_timer_display(show);
        Ok(())
    }

    fn set_superweapon_display_enabled_by_script(&self, enabled: bool) -> GameLogicResult<()> {
        self.hooks.push_superweapon_display_enabled(enabled);
        Ok(())
    }

    fn hide_object_superweapon_display_by_script(
        &self,
        object_id: gamelogic::common::ObjectID,
    ) -> GameLogicResult<()> {
        self.hooks.push_superweapon_object_display_mutation(
            SuperweaponObjectDisplayMutation::Hide { object_id },
        );
        Ok(())
    }

    fn show_object_superweapon_display_by_script(
        &self,
        object_id: gamelogic::common::ObjectID,
    ) -> GameLogicResult<()> {
        self.hooks.push_superweapon_object_display_mutation(
            SuperweaponObjectDisplayMutation::Show { object_id },
        );
        Ok(())
    }

    fn pause_named_special_power_countdown(
        &self,
        unit_name: &str,
        power_name: &str,
        pause: bool,
    ) -> GameLogicResult<()> {
        self.hooks.push_named_special_power_countdown_mutation(
            NamedSpecialPowerCountdownMutation {
                unit_name: unit_name.to_string(),
                power_name: power_name.to_string(),
                op: if pause {
                    crate::game_logic::NamedSpecialPowerCountdownOp::Stop
                } else {
                    crate::game_logic::NamedSpecialPowerCountdownOp::Start
                },
                seconds: 0,
            },
        );
        Ok(())
    }

    fn set_named_special_power_countdown(
        &self,
        unit_name: &str,
        power_name: &str,
        seconds: i32,
    ) -> GameLogicResult<()> {
        self.hooks.push_named_special_power_countdown_mutation(
            NamedSpecialPowerCountdownMutation {
                unit_name: unit_name.to_string(),
                power_name: power_name.to_string(),
                op: crate::game_logic::NamedSpecialPowerCountdownOp::Set,
                seconds,
            },
        );
        Ok(())
    }

    fn add_named_special_power_countdown(
        &self,
        unit_name: &str,
        power_name: &str,
        seconds: i32,
    ) -> GameLogicResult<()> {
        self.hooks.push_named_special_power_countdown_mutation(
            NamedSpecialPowerCountdownMutation {
                unit_name: unit_name.to_string(),
                power_name: power_name.to_string(),
                op: crate::game_logic::NamedSpecialPowerCountdownOp::Add,
                seconds,
            },
        );
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
        self.hooks.push_camera_setup(CameraSetupRequest {
            position: camera_coord3d_to_world(x, y, z),
            zoom,
            pitch,
            look_toward: camera_coord3d_to_world(look_toward_x, look_toward_y, look_toward_z),
        });
        Ok(())
    }

    fn camera_look_toward_object(
        &self,
        object_id: gamelogic::common::ObjectID,
        seconds: f32,
        hold_seconds: f32,
        ease_in_seconds: f32,
        ease_out_seconds: f32,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_look_toward_object(CameraLookTowardObjectRequest {
                object_id,
                duration_seconds: seconds,
                hold_seconds,
                ease_in_seconds,
                ease_out_seconds,
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
        self.hooks
            .push_camera_look_toward_waypoint(CameraLookTowardWaypointRequest {
                position: camera_coord3d_to_world(x, y, z),
                duration_seconds: seconds,
                ease_in_seconds,
                ease_out_seconds,
                reverse_rotation,
            });
        Ok(())
    }

    fn camera_mod_look_toward(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_look_toward(CameraModLookTowardRequest {
                position: camera_coord3d_to_world(x, y, z),
            });
        Ok(())
    }

    fn camera_mod_final_look_toward(&self, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        self.hooks
            .push_camera_mod_final_look_toward(CameraModFinalLookTowardRequest {
                position: camera_coord3d_to_world(x, y, z),
            });
        Ok(())
    }

    fn camera_letterbox_begin(&self) -> GameLogicResult<()> {
        self.hooks.push_letterbox(true);
        Ok(())
    }

    fn camera_letterbox_end(&self) -> GameLogicResult<()> {
        self.hooks.push_letterbox(false);
        Ok(())
    }

    fn camera_set_default(&self, pitch: f32, angle: f32, max_height: f32) -> GameLogicResult<()> {
        self.hooks.push_camera_set_default(CameraSetDefaultRequest {
            pitch,
            angle,
            max_height,
        });
        Ok(())
    }

    fn camera_enable_slave_mode(
        &self,
        thing_template_name: &str,
        bone_name: &str,
    ) -> GameLogicResult<()> {
        self.hooks
            .push_camera_slave_mode_enable(CameraSlaveModeRequest {
                thing_template_name: thing_template_name.to_string(),
                bone_name: bone_name.to_string(),
            });
        Ok(())
    }

    fn camera_disable_slave_mode(&self) -> GameLogicResult<()> {
        self.hooks.push_camera_slave_mode_disable();
        Ok(())
    }

    fn screen_shake(&self, intensity: i32) -> GameLogicResult<()> {
        self.hooks
            .push_screen_shake(ScreenShakeRequest { intensity });
        Ok(())
    }

    fn camera_add_shaker_at(
        &self,
        x: f32,
        y: f32,
        z: f32,
        amplitude: f32,
        duration_seconds: f32,
        radius: f32,
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_add_shaker(CameraAddShakerRequest {
            position: camera_coord3d_to_world(x, y, z),
            amplitude,
            duration_seconds,
            radius,
        });
        Ok(())
    }

    fn movie_play_fullscreen(&self, filename: &str) -> GameLogicResult<()> {
        self.hooks.push_movie_request(filename.to_string());
        Ok(())
    }

    fn movie_play_radar(&self, filename: &str) -> GameLogicResult<()> {
        self.hooks.push_radar_movie_request(filename.to_string());
        Ok(())
    }

    fn is_video_complete(&self, name: &str, flush: bool) -> bool {
        self.hooks.is_video_complete(name, flush)
    }

    fn speech_play(&self, name: &str, allow_overlap: bool) -> GameLogicResult<()> {
        // Live GAME_SHELL installs this handler (initialize_scripts), not
        // GameClientScriptActionHandler. C++ SPEECH_PLAY always reaches
        // TheAudio via doSpeechPlay — do not leave this as a UI SFX.
        let handle = Self::play_speech_through_the_audio(name, allow_overlap);
        self.hooks.note_speech_started_with_handle(name, handle);
        if let Some(label) = speech_subtitle_label_if_displayable(name, localization::translate) {
            self.hooks
                .push_military_caption(label, SPEECH_SUBTITLE_DURATION_MS);
        }
        Ok(())
    }

    fn sound_play_named(&self, sound: &str, unit_name: &str) -> GameLogicResult<()> {
        let Some(object_id) =
            gamelogic::scripting::host_script_named_unit_id(unit_name).or_else(|| {
                gamelogic::scripting::engine::get_named_object_tracker()
                    .get_object_id(unit_name)
                    .ok()
                    .flatten()
            })
        else {
            return Ok(());
        };
        let handle = Self::play_named_sound_through_the_audio(sound, object_id);
        self.hooks.note_audio_started(sound);
        let _ = handle;
        Ok(())
    }

    fn enable_object_sound(&self, _unit_name: &str, _enable: bool) -> GameLogicResult<()> {
        // Leftover dispatcher already queued HostScriptObjectSoundRequest.
        Ok(())
    }

    fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        self.hooks.is_speech_complete(name, flush)
    }

    fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        self.hooks.is_audio_complete(name, flush)
    }

    fn music_set_track(&self, track: &str, fade_out: bool, fade_in: bool) -> GameLogicResult<()> {
        // Live GAME_SHELL installs this handler (initialize_scripts), not
        // GameClientScriptActionHandler. C++ MUSIC_SET_TRACK always reaches
        // TheAudio via doMusicTrackChange — do not leave this as a UI note.
        Self::play_music_track_through_the_audio(track, fade_out, fade_in);
        self.hooks.note_music_started(track);
        // C++ doMusicTrackChange: TheAudio only — no InGameUI / broadcast.
        Ok(())
    }

    fn has_music_track_completed(&self, track: &str, param: i32) -> bool {
        self.hooks.has_music_track_completed(track, param)
    }

    fn stop_music(&self) -> GameLogicResult<()> {
        const AHSV_STOP_THE_MUSIC_FADE: u32 = 0xFFFF_FFF1;
        if let Some(audio) = gamelogic::helpers::TheAudio::get() {
            audio.remove_audio_event(AHSV_STOP_THE_MUSIC_FADE);
        }
        self.hooks.mark_music_stopped();
        self.hooks.push_music_stop();
        Ok(())
    }

    fn set_radar_enabled(&self, enabled: bool) -> GameLogicResult<()> {
        self.hooks.push_radar_enabled(enabled);
        Ok(())
    }

    fn set_radar_forced(&self, forced: bool) -> GameLogicResult<()> {
        self.hooks.push_radar_forced(forced);
        Ok(())
    }

    fn create_radar_event(&self, x: f32, y: f32, z: f32, event_type: i32) -> GameLogicResult<()> {
        self.hooks
            .push_radar_event_request(RadarScriptEventRequest {
                position: Vec3::new(x, y, z),
                event_type,
            });
        Ok(())
    }

    fn set_weather_visible(&self, visible: bool) -> GameLogicResult<()> {
        self.hooks.push_weather_visible(visible);
        Ok(())
    }

    fn set_objective(&self, name: &str, description: &str, completed: bool) -> GameLogicResult<()> {
        self.hooks.push_objective_update(ObjectiveUpdate {
            name: name.to_string(),
            description: description.to_string(),
            completed,
        });
        Ok(())
    }

    fn spawn_effect(&self, effect_type: &str, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        // Generals Coord3D: x/y on map plane, z height. Main uses x/z plane.
        let position = camera_coord3d_to_world(x, y, z);
        self.hooks.push_effect_request(ScriptEffectRequest {
            effect_type: effect_type.to_string(),
            position,
        });
        Ok(())
    }

    fn set_campaign_victorious(&self, victorious: bool) -> GameLogicResult<()> {
        game_client::gui::campaign_manager::get_campaign_manager().set_victorious(victorious);
        Ok(())
    }

    fn create_win_lose_window(&self, layout_filename: &str) -> GameLogicResult<()> {
        // C++ ScriptActions.cpp:201/204/225/228/247 TheWindowManager->winCreateFromScript.
        // Live host initialize_scripts overwrites GameClientScriptActionHandler; the
        // trait default is a no-op, so forward to the GameClient load_window path.
        game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .create_win_lose_window(layout_filename)
    }

    fn destroy_win_lose_window(&self) -> GameLogicResult<()> {
        // C++ ScriptActions.cpp:160-162 TheWindowManager->winDestroy(m_messageWindow).
        game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .destroy_win_lose_window()
    }

    fn close_game_windows(&self) -> GameLogicResult<()> {
        // C++ GameLogic::closeWindows GameLogicDispatch.cpp:202-219.
        game_client::core::script_action_handler::GameClientScriptActionHandler::new()
            .close_game_windows()
    }

    fn set_warehouse_value(&self, warehouse_name: &str, cash_value: i32) -> GameLogicResult<()> {
        crate::game_logic::host_supply_gather::queue_warehouse_set_value(
            warehouse_name,
            cash_value,
        );
        Ok(())
    }
}

fn delay_frames(seconds: i32) -> u64 {
    if seconds <= 0 {
        1
    } else {
        (seconds as u64 * 30).max(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::scripting::core::{
        Condition, ConditionType, Coord3D, OrCondition, Parameter, ParameterType, ScriptActionType,
        ScriptGroup,
    };
    use gamelogic::scripting::engine::{ScriptEngine, ScriptEngineHandle};

    #[derive(Clone)]
    struct RecordingScriptHandler {
        events: Arc<Mutex<Vec<String>>>,
        enabled_updates: Option<Arc<Mutex<Vec<(String, bool)>>>>,
    }

    impl ScriptActionHandler for RecordingScriptHandler {
        fn display_text(&self, text: &str) -> GameLogicResult<()> {
            self.events
                .lock()
                .expect("recording script handler mutex should not be poisoned")
                .push(text.to_string());
            Ok(())
        }

        fn enable_script(&self, name: &str, enabled: bool) -> GameLogicResult<()> {
            if let Some(enabled_updates) = self.enabled_updates.as_ref() {
                enabled_updates
                    .lock()
                    .expect("recording script enable queue mutex should not be poisoned")
                    .push((name.to_string(), enabled));
            }
            Ok(())
        }
    }

    fn private_runtime_recording_into(
        events: Arc<Mutex<Vec<String>>>,
        lists: &[ScriptList],
    ) -> MissionScriptRuntime {
        let mut private_engine =
            ScriptEngine::new().expect("private script engine should initialize");
        private_engine.set_action_handler(Some(Arc::new(RecordingScriptHandler {
            events,
            enabled_updates: None,
        })));
        for (side_index, list) in lists.iter().enumerate() {
            private_engine
                .set_script_list_for_player(side_index, Some(Box::new(list.clone())))
                .expect("private script engine should accept its ScriptList");
        }

        let mut runtime =
            MissionScriptRuntime::new().expect("mission script runtime should initialize");
        runtime.evaluator = ScriptEvaluator::new(ScriptEngineHandle::from_engine(private_engine));
        runtime
    }

    fn display_text_action(text: &str) -> Box<ScriptAction> {
        let mut action = ScriptAction::new(ScriptActionType::DisplayText);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::TextString,
                text.to_string(),
            ))
            .expect("display text action should accept its text parameter");
        Box::new(action)
    }

    fn script_enable_action(name: &str, enabled: bool) -> Box<ScriptAction> {
        let action_type = if enabled {
            ScriptActionType::EnableScript
        } else {
            ScriptActionType::DisableScript
        };
        let mut action = ScriptAction::new(action_type);
        action
            .add_parameter(Parameter::with_string(
                ParameterType::Script,
                name.to_string(),
            ))
            .expect("script toggle action should accept its target name");
        Box::new(action)
    }

    fn one_shot_script(name: &str, action: Box<ScriptAction>) -> Box<Script> {
        let mut script = Script::new();
        script.set_name(name.to_string());
        script.set_one_shot(true);
        script.set_action(Some(action));
        Box::new(script)
    }

    fn cxx_true_one_shot_script(name: &str, action: Box<ScriptAction>) -> Box<Script> {
        let mut script = one_shot_script(name, action);
        let mut or_condition = OrCondition::new();
        or_condition
            .set_first_and_condition(Some(Box::new(Condition::new(ConditionType::ConditionTrue))));
        script.set_or_condition(Some(Box::new(or_condition)));
        script
    }

    #[test]
    fn dense_host_lists_keep_attack_random_and_cinematic_scripts_in_cxx_order() {
        // C++ ScriptEngine::update (ScriptEngine.cpp:5479-5574, 7653-7667)
        // walks root scripts first, then active non-subroutine groups, without
        // a density/name filter.  Keep this list above the old 48-script
        // threshold and include the campaign patterns that the host used to
        // erase before frame zero.
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut list = ScriptList::new();

        list.append_script(one_shot_script(
            "Spawn Techs And Attack",
            display_text_action("attack-wave"),
        ));

        let mut random_driver = display_text_action("random-before-call");
        let mut call = ScriptAction::new(ScriptActionType::CallSubroutine);
        call.add_parameter(Parameter::with_string(
            ParameterType::ScriptSubroutine,
            "SUB-Generate Random Number".to_string(),
        ))
        .expect("CALL_SUBROUTINE should accept its name");
        call.set_next_action(Some(display_text_action("random-after-call")));
        random_driver.set_next_action(Some(Box::new(call)));
        list.append_script(one_shot_script("Generate Random Number", random_driver));

        let mut cinematic = display_text_action("cinematic-camera");
        let mut camera_move = ScriptAction::new(ScriptActionType::MoveCameraTo);
        camera_move
            .add_parameter(Parameter::with_coord(
                ParameterType::Coord3D,
                Coord3D::new(150.0, 275.0, 40.0),
            ))
            .expect("MOVE_CAMERA_TO should accept a coordinate target");
        cinematic.set_next_action(Some(Box::new(camera_move)));
        list.append_script(one_shot_script("Cinematic Camera", cinematic));

        for ordinal in 0..48 {
            list.append_script(one_shot_script(
                &format!("Dense Filler {ordinal:02}"),
                display_text_action(&format!("filler-{ordinal:02}")),
            ));
        }

        let mut active_group = ScriptGroup::new();
        active_group.set_name("Post Root Camera Group".to_string());
        active_group.set_active(true);
        active_group.append_script(one_shot_script(
            "Active Group Cinematic",
            display_text_action("active-group"),
        ));
        list.append_group(Box::new(active_group));

        let mut subroutine_group = ScriptGroup::new();
        subroutine_group.set_name("SUB-Generate Random Number".to_string());
        subroutine_group.set_active(true);
        subroutine_group.set_subroutine(true);
        subroutine_group.append_script(cxx_true_one_shot_script(
            "Subroutine Body",
            display_text_action("random-subroutine"),
        ));
        list.append_group(Box::new(subroutine_group));

        let mut runtime = private_runtime_recording_into(Arc::clone(&events), &[list.clone()]);
        runtime.install_lists(&[list]);
        runtime
            .update(9001)
            .expect("a dense non-shell list should complete one ordered frame walk");

        let events = events
            .lock()
            .expect("recording script handler mutex should not be poisoned")
            .clone();
        assert_eq!(
            &events[..5],
            [
                "attack-wave",
                "random-before-call",
                "random-subroutine",
                "random-after-call",
                "cinematic-camera",
            ],
            "attack, CALL_SUBROUTINE/random, and cinematic scripts must retain declaration order"
        );
        assert_eq!(
            events.len(),
            54,
            "the bounded walk must not skip dense scripts"
        );
        assert_eq!(events.last().map(String::as_str), Some("active-group"));

        assert!(
            runtime
                .scripts
                .iter()
                .filter(|entry| runtime.is_regular_script_eligible(entry))
                .all(|entry| entry.state.completed),
            "every active root/group one-shot must run on this logic frame"
        );
        let subroutine = runtime
            .scripts
            .iter()
            .find(|entry| entry.original_name.as_deref() == Some("Subroutine Body"))
            .expect("subroutine script should remain discoverable for CALL_SUBROUTINE");
        assert!(!runtime.is_regular_script_eligible(subroutine));
        assert!(
            !subroutine.state.completed,
            "a subroutine must not be evaluated by the regular frame walk"
        );
    }

    #[test]
    fn shell_named_attack_scripts_use_the_same_complete_frame_walk() {
        // GAME_SHELL does not give C++ ScriptEngine::update a separate budget,
        // warm-up, or continuation interpreter.  In particular, a script name
        // that used to trigger the Rust-only shell throttle must not change
        // whether every declared script runs on this logic frame.
        let events = Arc::new(Mutex::new(Vec::new()));
        let mut list = ScriptList::new();
        for name in [
            "Spawn Bikes And Attack",
            "Shell Script 01",
            "Shell Script 02",
            "Shell Script 03",
            "Shell Script 04",
            "Shell Script 05",
            "Shell Script 06",
            "Shell Script 07",
            "Shell Script 08",
            "Shell Script 09",
        ] {
            list.append_script(one_shot_script(name, display_text_action(name)));
        }

        let mut runtime = private_runtime_recording_into(Arc::clone(&events), &[list.clone()]);
        runtime.install_lists(&[list]);
        runtime
            .update(1)
            .expect("shell-equivalent script frame should complete");

        assert_eq!(
            events
                .lock()
                .expect("recording event mutex should not be poisoned")
                .as_slice(),
            [
                "Spawn Bikes And Attack",
                "Shell Script 01",
                "Shell Script 02",
                "Shell Script 03",
                "Shell Script 04",
                "Shell Script 05",
                "Shell Script 06",
                "Shell Script 07",
                "Shell Script 08",
                "Shell Script 09",
            ],
            "every root script must run in declaration order on frame one"
        );
    }

    #[test]
    fn group_name_toggles_apply_at_cxx_group_boundaries_without_skipping_siblings() {
        // C++ enableScript/disableScript toggles a named group independently
        // (ScriptEngine.cpp:6797-6823).  A root action can enable a later
        // group in this same update; once a group has been entered, disabling
        // it takes effect next frame rather than skipping its remaining chain.
        let events = Arc::new(Mutex::new(Vec::new()));
        let pending_enabled_updates = Arc::new(Mutex::new(Vec::new()));
        let mut list = ScriptList::new();
        list.append_script(one_shot_script(
            "Enable Dormant Attack Group",
            script_enable_action("Dormant Attack Group", true),
        ));
        list.append_script(one_shot_script(
            "Root After Enable",
            display_text_action("root-after-enable"),
        ));

        let mut dormant_group = ScriptGroup::new();
        dormant_group.set_name("Dormant Attack Group".to_string());
        dormant_group.set_active(false);
        let mut dormant_member = Script::new();
        dormant_member.set_name("Dormant Attack Member".to_string());
        dormant_member.set_one_shot(false);
        dormant_member.set_action(Some(display_text_action("dormant-group")));
        dormant_group.append_script(Box::new(dormant_member));
        list.append_group(Box::new(dormant_group));

        let mut self_disabling_group = ScriptGroup::new();
        self_disabling_group.set_name("Self Disabling Group".to_string());
        self_disabling_group.set_active(true);
        self_disabling_group.append_script(one_shot_script(
            "Disable This Group",
            script_enable_action("Self Disabling Group", false),
        ));
        let mut sibling = Script::new();
        sibling.set_name("Sibling In Entered Group".to_string());
        sibling.set_one_shot(false);
        sibling.set_action(Some(display_text_action("self-group-sibling")));
        self_disabling_group.append_script(Box::new(sibling));
        list.append_group(Box::new(self_disabling_group));

        let mut private_engine =
            ScriptEngine::new().expect("private script engine should initialize");
        private_engine.set_action_handler(Some(Arc::new(RecordingScriptHandler {
            events: Arc::clone(&events),
            enabled_updates: Some(Arc::clone(&pending_enabled_updates)),
        })));
        private_engine
            .set_script_list_for_player(0, Some(Box::new(list.clone())))
            .expect("private script engine should accept the test ScriptList");

        let mut runtime = MissionScriptRuntime::new_with_pending_script_enabled_updates(
            Arc::clone(&pending_enabled_updates),
        )
        .expect("mission script runtime should initialize");
        runtime.evaluator = ScriptEvaluator::new(ScriptEngineHandle::from_engine(private_engine));
        runtime.install_lists(&[list]);

        runtime
            .update(17)
            .expect("first C++-ordered group frame should run");
        assert_eq!(
            events
                .lock()
                .expect("recording event mutex should not be poisoned")
                .as_slice(),
            ["root-after-enable", "dormant-group", "self-group-sibling"],
            "root enable must admit its later group, while an entered group finishes its sibling chain"
        );
        assert!(runtime.groups[0].active);
        assert!(!runtime.groups[1].active);

        runtime
            .update(18)
            .expect("second C++-ordered group frame should run");
        assert_eq!(
            events
                .lock()
                .expect("recording event mutex should not be poisoned")
                .as_slice(),
            [
                "root-after-enable",
                "dormant-group",
                "self-group-sibling",
                "dormant-group",
            ],
            "a disabled group must be skipped on the following frame without disabling other groups"
        );
    }

    #[test]
    fn script_and_group_toggles_keep_cxx_authored_name_case() {
        // ScriptEngine::findGroup/findScript use exact AsciiString equality;
        // an action authored with a case mismatch must not enable either
        // target, even though both have runtime-derived display names.
        let mut root_script = Script::new();
        root_script.set_name("Mixed Case Root Script".to_string());
        root_script.set_active(false);

        let mut group = ScriptGroup::new();
        group.set_name("Mixed Case Group".to_string());
        group.set_active(false);

        let mut list = ScriptList::new();
        list.append_script(Box::new(root_script));
        list.append_group(Box::new(group));

        let mut runtime =
            MissionScriptRuntime::new().expect("mission script runtime should initialize");
        runtime.install_lists(&[list]);

        runtime
            .set_script_enabled("mixed case root script", true)
            .expect("mismatched script name should be a harmless no-op");
        runtime
            .set_script_enabled("mixed case group", true)
            .expect("mismatched group name should be a harmless no-op");
        assert!(!runtime.scripts[0].enabled);
        assert!(!runtime.groups[0].active);

        runtime
            .set_script_enabled("Mixed Case Root Script", true)
            .expect("exact script name should enable the target");
        runtime
            .set_script_enabled("Mixed Case Group", true)
            .expect("exact group name should enable the target");
        assert!(runtime.scripts[0].enabled);
        assert!(runtime.groups[0].active);
    }

    #[test]
    fn handler_forwards_camera_pitch_rotate_and_mod_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .set_camera_pitch(1.25, 2.0, 0.5, 0.25)
            .expect("pitch action should succeed");
        handler
            .rotate_camera(0.5, 3.0, 0.2, 0.4)
            .expect("rotate action should succeed");
        handler
            .camera_mod_set_final_zoom(0.8, 0.3, 0.1)
            .expect("camera mod final zoom should succeed");
        handler
            .camera_mod_set_final_pitch(1.1, 0.25, 0.15)
            .expect("camera mod final pitch should succeed");
        handler
            .camera_mod_freeze_time()
            .expect("camera mod freeze time should succeed");
        handler
            .camera_mod_freeze_angle()
            .expect("camera mod freeze angle should succeed");
        handler
            .camera_mod_set_final_speed_multiplier(4)
            .expect("camera mod final speed multiplier should succeed");
        handler
            .camera_mod_set_rolling_average(6)
            .expect("camera mod rolling average should succeed");
        handler
            .set_visual_speed_multiplier(3)
            .expect("visual speed multiplier should succeed");
        handler.freeze_time().expect("freeze time should succeed");
        handler
            .unfreeze_time()
            .expect("unfreeze time should succeed");
        handler
            .set_fps_limit(120)
            .expect("set fps limit should succeed");

        let pitch = hooks.drain_camera_pitch_requests();
        assert_eq!(pitch.len(), 1);
        assert!((pitch[0].pitch - 1.25).abs() < f32::EPSILON);
        assert!((pitch[0].duration_seconds - 2.0).abs() < f32::EPSILON);
        assert!((pitch[0].ease_in_seconds - 0.5).abs() < f32::EPSILON);
        assert!((pitch[0].ease_out_seconds - 0.25).abs() < f32::EPSILON);

        let rotate = hooks.drain_camera_rotate_requests();
        assert_eq!(rotate.len(), 1);
        assert!((rotate[0].rotations - 0.5).abs() < f32::EPSILON);
        assert!((rotate[0].duration_seconds - 3.0).abs() < f32::EPSILON);
        assert!((rotate[0].ease_in_seconds - 0.2).abs() < f32::EPSILON);
        assert!((rotate[0].ease_out_seconds - 0.4).abs() < f32::EPSILON);

        let final_zoom = hooks.drain_camera_mod_final_zoom_requests();
        assert_eq!(final_zoom.len(), 1);
        assert!((final_zoom[0].zoom - 0.8).abs() < f32::EPSILON);
        assert!((final_zoom[0].ease_in - 0.3).abs() < f32::EPSILON);
        assert!((final_zoom[0].ease_out - 0.1).abs() < f32::EPSILON);

        let final_pitch = hooks.drain_camera_mod_final_pitch_requests();
        assert_eq!(final_pitch.len(), 1);
        assert!((final_pitch[0].pitch - 1.1).abs() < f32::EPSILON);
        assert!((final_pitch[0].ease_in - 0.25).abs() < f32::EPSILON);
        assert!((final_pitch[0].ease_out - 0.15).abs() < f32::EPSILON);

        let freeze_time = hooks.drain_camera_mod_freeze_time_requests();
        assert_eq!(freeze_time.len(), 1);
        let freeze_angle = hooks.drain_camera_mod_freeze_angle_requests();
        assert_eq!(freeze_angle.len(), 1);
        let final_speed = hooks.drain_camera_mod_final_speed_multiplier_requests();
        assert_eq!(final_speed.len(), 1);
        assert_eq!(final_speed[0].multiplier, 4);
        let rolling_average = hooks.drain_camera_mod_rolling_average_requests();
        assert_eq!(rolling_average.len(), 1);
        assert_eq!(rolling_average[0].frames, 6);
        let visual_speed = hooks.drain_visual_speed_multiplier_requests();
        assert_eq!(visual_speed.len(), 1);
        assert_eq!(visual_speed[0].multiplier, 3);
        let script_freeze = hooks.drain_script_freeze_time_requests();
        assert_eq!(script_freeze, vec![true, false]);
        let fps_limit = hooks.drain_set_fps_limit_requests();
        assert_eq!(fps_limit.len(), 1);
        assert_eq!(fps_limit[0].fps, 120);
    }

    #[test]
    fn handler_forwards_oversize_terrain_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .oversize_terrain(2)
            .expect("oversize terrain request should succeed");
        handler
            .oversize_terrain(0)
            .expect("reset oversize terrain request should succeed");

        let requests = hooks.drain_oversize_terrain_requests();
        assert_eq!(requests, vec![2, 0]);
    }

    #[test]
    fn handler_forwards_border_shroud_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .set_border_shroud_level(32)
            .expect("set_border_shroud_level should succeed");
        handler
            .set_border_shroud_level(128)
            .expect("set_border_shroud_level should succeed");

        let requests = hooks.drain_border_shroud_levels();
        assert_eq!(requests, vec![32, 128]);
    }

    #[test]
    fn handler_forwards_military_caption_duration_as_milliseconds() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .military_caption("SCRIPT:Briefing", 2500)
            .expect("military caption request should succeed");

        let captions = hooks.drain_military_captions();
        assert_eq!(captions.len(), 1);
        assert_eq!(captions[0].text, "SCRIPT:Briefing");
        assert_eq!(captions[0].duration_ms, 2500);
    }

    #[test]
    fn speech_subtitle_label_matches_cpp_dialogevent_shape() {
        assert_eq!(
            speech_subtitle_label("USA01Intro"),
            "DIALOGEVENT:USA01IntroSubtitle"
        );
    }

    #[test]
    fn speech_subtitle_requires_displayable_localized_text() {
        assert_eq!(
            speech_subtitle_label_if_displayable("Briefing", |label| {
                assert_eq!(label, "DIALOGEVENT:BriefingSubtitle");
                Some("Commander online".to_string())
            }),
            Some("DIALOGEVENT:BriefingSubtitle".to_string())
        );
        assert_eq!(
            speech_subtitle_label_if_displayable("Briefing", |_| None),
            None
        );
        assert_eq!(
            speech_subtitle_label_if_displayable("Briefing", |_| Some(String::new())),
            None
        );
        assert_eq!(
            speech_subtitle_label_if_displayable("Briefing", |_| Some("* hidden".to_string())),
            None
        );
    }

    #[test]
    fn speech_frames_from_length_ms_truncates_like_cpp() {
        // C++ REAL_TO_UNSIGNEDINT(audioLength / MSEC_PER_LOGICFRAME_REAL).
        assert_eq!(speech_frames_from_length_ms(0.0), 0);
        assert_eq!(speech_frames_from_length_ms(5_000.0), 150);
        assert_eq!(speech_frames_from_length_ms(33.3), 0);
        assert_eq!(speech_frames_from_length_ms(33.34), 1);
        assert_eq!(speech_frames_from_length_ms(1_000.0), 30);
    }

    #[test]
    fn has_finished_speech_uses_audio_length_not_one_frame() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        hooks.note_logic_frame(10);
        assert!(
            !hooks.is_speech_complete("", false),
            "empty speech name is not complete"
        );

        // Seed a 5s VO completion (150 frames) as TheAudio would.
        {
            let mut map = hooks.speech_complete_frame.lock().expect("map");
            map.insert(
                "Briefing".to_string(),
                10 + speech_frames_from_length_ms(5_000.0),
            );
        }
        assert!(
            !hooks.is_speech_complete("Briefing", false),
            "HAS_FINISHED_SPEECH must stay false one frame after a 5s line"
        );
        hooks.note_logic_frame(11);
        assert!(
            !hooks.is_speech_complete("Briefing", false),
            "HAS_FINISHED_SPEECH must stay false until TheAudio length elapses"
        );
        hooks.note_logic_frame(159);
        assert!(!hooks.is_speech_complete("Briefing", false));
        hooks.note_logic_frame(160);
        assert!(hooks.is_speech_complete("Briefing", true));
        assert!(
            hooks
                .speech_complete_frame
                .lock()
                .expect("map")
                .get("Briefing")
                .is_none(),
            "flush removes the completed speech tracker"
        );
    }

    #[test]
    fn has_finished_audio_uses_the_audio_length_not_one_frame() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        hooks.note_logic_frame(10);
        assert!(
            !hooks.is_audio_complete("", false),
            "empty audio name is not complete"
        );

        // Seed a 5s SFX completion (150 frames) as leftover TheAudio would.
        {
            let mut map = hooks.audio_complete_frame.lock().expect("map");
            map.insert(
                "Boom".to_string(),
                10 + speech_frames_from_length_ms(5_000.0),
            );
        }
        assert!(
            !hooks.is_audio_complete("Boom", false),
            "HAS_FINISHED_AUDIO must stay false one frame after a 5s SFX"
        );
        hooks.note_logic_frame(11);
        assert!(
            !hooks.is_audio_complete("Boom", false),
            "HAS_FINISHED_AUDIO must stay false until leftover TheAudio length elapses"
        );
        hooks.note_logic_frame(159);
        assert!(!hooks.is_audio_complete("Boom", false));
        hooks.note_logic_frame(160);
        assert!(hooks.is_audio_complete("Boom", true));
        assert!(
            hooks
                .audio_complete_frame
                .lock()
                .expect("map")
                .get("Boom")
                .is_none(),
            "flush removes the completed audio tracker"
        );
    }

    #[test]
    fn has_finished_video_waits_leftover_list_unknown_names_false() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        assert!(
            !handler.is_video_complete("IntroMovie", false),
            "unknown / never-finished names stay false"
        );

        handler
            .movie_play_fullscreen("IntroMovie")
            .expect("movie play should queue");
        hooks.note_logic_frame(1);
        assert!(
            !handler.is_video_complete("IntroMovie", false),
            "HAS_FINISHED_VIDEO must not complete one frame after play"
        );

        gamelogic::helpers::TheScriptEngine::notify_of_completed_video("IntroMovie");
        assert!(
            handler.is_video_complete("IntroMovie", true),
            "leftover m_completedVideo membership is true"
        );
        assert!(
            !handler.is_video_complete("IntroMovie", false),
            "flush removes the leftover completed-video entry"
        );
    }

    #[test]
    fn handler_forwards_radar_force_updates() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .set_radar_forced(true)
            .expect("radar force request should succeed");
        handler
            .set_radar_forced(false)
            .expect("radar revert request should succeed");

        assert_eq!(hooks.drain_radar_forced_updates(), vec![true, false]);
        assert!(hooks.drain_radar_forced_updates().is_empty());
    }

    #[test]
    fn handler_forwards_radar_event_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .create_radar_event(10.0, 20.0, 5.0, 3)
            .expect("radar event request should succeed");

        let requests = hooks.drain_radar_event_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].position, Vec3::new(10.0, 20.0, 5.0));
        assert_eq!(requests[0].event_type, 3);
    }

    #[test]
    fn zoom_camera_preserves_script_ease_parameters() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .zoom_camera(0.65, 4.0, 1.5, 1.0)
            .expect("zoom action should succeed");

        let zoom = hooks.drain_camera_zoom_requests();
        assert_eq!(zoom.len(), 1);
        assert!((zoom[0].zoom - 0.65).abs() < f32::EPSILON);
        assert!((zoom[0].duration_seconds - 4.0).abs() < f32::EPSILON);
        assert!((zoom[0].ease_in_seconds - 1.5).abs() < f32::EPSILON);
        assert!((zoom[0].ease_out_seconds - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn start_new_map_fade_calls_leftover_new_map() {
        let src = include_str!("mission_scripts.rs");
        assert!(src.contains("pub fn start_new_map_fade"));
        assert!(src.contains("engine.new_map()"));
        let load = include_str!("world_scripts/add_object_selection.rs");
        assert!(load.contains("engine.new_map()"));
        assert!(load.contains("FADE_MULTIPLY"));
    }

    #[test]
    fn handler_forwards_setup_and_look_toward_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .setup_camera(10.0, 20.0, 30.0, 0.7, 1.1, 40.0, 50.0, 60.0)
            .expect("setup camera should succeed");
        handler
            .camera_look_toward_object(42, 3.0, 1.5, 0.4, 0.6)
            .expect("look toward object should succeed");
        handler
            .camera_look_toward_waypoint(100.0, 200.0, 5.0, 2.0, 0.5, 0.25, true)
            .expect("look toward waypoint should succeed");
        handler
            .camera_mod_look_toward(70.0, 80.0, 90.0)
            .expect("camera mod look toward should succeed");
        handler
            .camera_mod_final_look_toward(15.0, 25.0, 35.0)
            .expect("camera mod final look toward should succeed");
        handler
            .move_camera_to_selection()
            .expect("move camera to selection should succeed");
        handler
            .camera_move_home()
            .expect("camera move home should succeed");
        handler
            .camera_set_default(0.75, 12.0, 1.8)
            .expect("camera set default should succeed");
        handler
            .camera_enable_slave_mode("CineCameraRig", "CameraBone")
            .expect("camera enable slave mode should succeed");
        handler
            .camera_disable_slave_mode()
            .expect("camera disable slave mode should succeed");
        handler
            .screen_shake(3)
            .expect("screen shake should succeed");
        handler
            .camera_add_shaker_at(5.0, 6.0, 7.0, 8.5, 2.5, 90.0)
            .expect("camera add shaker should succeed");
        handler
            .camera_follow_object(77, true)
            .expect("camera follow should succeed");
        handler
            .stop_camera_follow()
            .expect("camera stop follow should succeed");

        let setup = hooks.drain_camera_setup_requests();
        assert_eq!(setup.len(), 1);
        assert_eq!(setup[0].position, Vec3::new(10.0, 30.0, 20.0));
        assert!((setup[0].zoom - 0.7).abs() < f32::EPSILON);
        assert!((setup[0].pitch - 1.1).abs() < f32::EPSILON);
        assert_eq!(setup[0].look_toward, Vec3::new(40.0, 60.0, 50.0));

        let object = hooks.drain_camera_look_toward_object_requests();
        assert_eq!(object.len(), 1);
        assert_eq!(object[0].object_id, 42);
        assert!((object[0].duration_seconds - 3.0).abs() < f32::EPSILON);
        assert!((object[0].hold_seconds - 1.5).abs() < f32::EPSILON);
        assert!((object[0].ease_in_seconds - 0.4).abs() < f32::EPSILON);
        assert!((object[0].ease_out_seconds - 0.6).abs() < f32::EPSILON);

        let waypoint = hooks.drain_camera_look_toward_waypoint_requests();
        assert_eq!(waypoint.len(), 1);
        assert_eq!(waypoint[0].position, Vec3::new(100.0, 5.0, 200.0));
        assert!((waypoint[0].duration_seconds - 2.0).abs() < f32::EPSILON);
        assert!((waypoint[0].ease_in_seconds - 0.5).abs() < f32::EPSILON);
        assert!((waypoint[0].ease_out_seconds - 0.25).abs() < f32::EPSILON);
        assert!(waypoint[0].reverse_rotation);

        let mod_look = hooks.drain_camera_mod_look_toward_requests();
        assert_eq!(mod_look.len(), 1);
        assert_eq!(mod_look[0].position, Vec3::new(70.0, 90.0, 80.0));

        let mod_final_look = hooks.drain_camera_mod_final_look_toward_requests();
        assert_eq!(mod_final_look.len(), 1);
        assert_eq!(mod_final_look[0].position, Vec3::new(15.0, 35.0, 25.0));

        let move_to_selection = hooks.drain_camera_move_to_selection_requests();
        assert_eq!(move_to_selection.len(), 1);

        let move_home = hooks.drain_camera_move_home_requests();
        assert_eq!(move_home.len(), 1);

        let set_default = hooks.drain_camera_set_default_requests();
        assert_eq!(set_default.len(), 1);
        assert!((set_default[0].pitch - 0.75).abs() < f32::EPSILON);
        assert!((set_default[0].angle - 12.0).abs() < f32::EPSILON);
        assert!((set_default[0].max_height - 1.8).abs() < f32::EPSILON);

        let slave_enable = hooks.drain_camera_slave_mode_enable_requests();
        assert_eq!(slave_enable.len(), 1);
        assert_eq!(slave_enable[0].thing_template_name, "CineCameraRig");
        assert_eq!(slave_enable[0].bone_name, "CameraBone");
        let slave_disable = hooks.drain_camera_slave_mode_disable_requests();
        assert_eq!(slave_disable.len(), 1);

        let screen_shakes = hooks.drain_screen_shake_requests();
        assert_eq!(screen_shakes.len(), 1);
        assert_eq!(screen_shakes[0].intensity, 3);

        let shakers = hooks.drain_camera_add_shaker_requests();
        assert_eq!(shakers.len(), 1);
        assert_eq!(shakers[0].position, Vec3::new(5.0, 7.0, 6.0));
        assert!((shakers[0].amplitude - 8.5).abs() < f32::EPSILON);
        assert!((shakers[0].duration_seconds - 2.5).abs() < f32::EPSILON);
        assert!((shakers[0].radius - 90.0).abs() < f32::EPSILON);

        let follows = hooks.drain_camera_follows();
        assert_eq!(follows.len(), 2);
        assert_eq!(follows[0].object_id, 77);
        assert!(follows[0].snap_to_unit);
        assert_eq!(follows[1].object_id, 0);
        assert!(!follows[1].snap_to_unit);
    }

    #[test]
    fn music_track_completion_is_not_immediate_and_respects_flush() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        assert!(
            !handler.has_music_track_completed("TrackA", 1),
            "unknown / unplayed tracks stay false (C++ Miles loop count)"
        );

        handler
            .music_set_track("TrackA", false, false)
            .expect("music set track should succeed");
        assert!(
            !handler.has_music_track_completed("TrackA", 1),
            "track should not complete on the next frame without Miles loop count"
        );

        hooks.update(1).expect("frame advance should succeed");
        assert!(
            !handler.has_music_track_completed("TrackA", 1),
            "one logic frame is not a Miles loop completion"
        );
    }

    #[test]
    fn stop_music_does_not_fail_open_music_complete() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .music_set_track("TrackB", false, false)
            .expect("music set track should succeed");
        assert!(
            !handler.has_music_track_completed("TrackB", 1),
            "newly started track should be incomplete before stop"
        );

        handler.stop_music().expect("stop music should succeed");
        assert!(
            !handler.has_music_track_completed("TrackB", 1),
            "stop music does not mark hasMusicTrackCompleted; unplayed stays false"
        );
    }

    #[test]
    fn music_set_track_queues_named_track_on_the_audio() {
        // C++ ScriptActions::doMusicTrackChange → TheAudio->addAudioEvent(track).
        // Live GAME_SHELL uses MissionScriptActionHandler, which previously only
        // noted the name and never queued AR_Play.
        let manager = game_engine::common::audio::game_audio::initialize_global_audio_manager();
        let before = {
            let guard = manager.lock().expect("THE_AUDIO lock");
            (
                guard.pending_play_request_count(),
                guard.get_music_track_name(),
            )
        };

        if let Ok(mut guard) = manager.lock() {
            if guard.find_audio_event_info("ShellMapMusic").is_none() {
                guard.register_audio_event_info(game_engine::common::audio::AudioEventInfo {
                    sound_type: game_engine::common::audio::AudioType::Music,
                    control: 0,
                    audio_name: "ShellMapMusic".to_string(),
                    volume: 0.8,
                    sounds_morning: Vec::new(),
                    sounds: Vec::new(),
                    sounds_night: Vec::new(),
                    sounds_evening: Vec::new(),
                    attack_sounds: Vec::new(),
                    decay_sounds: Vec::new(),
                    pitch_shift_min: 1.0,
                    pitch_shift_max: 1.0,
                    volume_shift: 0.0,
                    min_volume: 0.0,
                    limit: 0,
                    loop_count: 1,
                    delay_min: 0.0,
                    delay_max: 0.0,
                    filename: String::new(),
                    sound_type_field: game_engine::common::audio::AudioType::Music,
                    type_field: 0,
                    priority: game_engine::common::audio::AudioPriority::Normal,
                    min_distance: 25.0,
                    max_distance: 1000.0,
                    low_pass_freq: 1.0,
                    is_level_specific: false,
                });
            }
        }

        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks);
        handler
            .music_set_track("ShellMapMusic", false, true)
            .expect("MUSIC_SET_TRACK must succeed");

        let after = {
            let guard = manager.lock().expect("THE_AUDIO lock");
            (
                guard.pending_play_request_count(),
                guard.get_music_track_name(),
            )
        };
        assert!(
            after.0 > before.0 || after.1 == "ShellMapMusic",
            "MUSIC_SET_TRACK must queue TheAudio AR_Play for the script track (before={before:?}, after={after:?})"
        );
        assert_eq!(
            after.1, "ShellMapMusic",
            "TheAudio music name must be the script track, not a leftover"
        );
    }

    #[test]
    fn music_set_track_does_not_broadcast_track_name() {
        // C++ ScriptActions::doMusicTrackChange has no TheInGameUI->message.
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());
        handler
            .music_set_track("ShellMapMusic", false, true)
            .expect("MUSIC_SET_TRACK must succeed");
        let messages = hooks.drain_messages();
        assert!(
            messages.iter().all(|m| !m.contains("Music track:")),
            "MUSIC_SET_TRACK must not broadcast Music track: name: {messages:?}"
        );
    }

    #[test]
    fn handler_forwards_weather_visibility_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .set_weather_visible(false)
            .expect("set weather visible should succeed");
        handler
            .set_weather_visible(true)
            .expect("set weather visible should succeed");

        assert_eq!(hooks.drain_weather_visibility_updates(), vec![false, true]);
    }

    #[test]
    fn popup_generation_remains_unique_across_replaced_hook_instances() {
        let popup = |message: &str| ScriptPopupMessageRequest {
            message: message.to_string(),
            x_percent: 50,
            y_percent: 50,
            width: 40,
            pause: false,
            pause_music: false,
            popup_generation: 0,
        };

        // A map load replaces GameLogic and its MissionScriptHooks. The
        // live-only token must therefore not restart at one per hook object.
        let old_world = MissionScriptHooks::new().expect("old world hooks");
        old_world.push_popup_message(popup("old popup"));
        let old_generation = old_world.drain_popup_message_requests()[0].popup_generation;

        let replacement_world = MissionScriptHooks::new().expect("replacement world hooks");
        replacement_world.push_popup_message(popup("replacement popup"));
        let replacement_generation =
            replacement_world.drain_popup_message_requests()[0].popup_generation;

        assert_ne!(old_generation, 0);
        assert_ne!(replacement_generation, 0);
        assert_ne!(
            old_generation, replacement_generation,
            "a stale old-world acknowledgement must not ABA-match the replacement world"
        );
    }

    #[test]
    fn handler_forwards_popup_guardband_motion_blur_and_ui_display_requests() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .popup_message("Incoming transmission", 35, 55, 420, true, false)
            .expect("popup message should succeed");
        handler
            .resize_view_guardband(1.25, 0.75)
            .expect("resize view guardband should succeed");
        handler
            .set_camera_bw_mode(true, 24)
            .expect("set camera bw mode should succeed");
        handler
            .set_skybox_enabled(false)
            .expect("set skybox enabled should succeed");
        handler
            .camera_motion_blur(false, true)
            .expect("camera motion blur should succeed");
        handler
            .camera_motion_blur_jump(10.0, 20.0, 30.0, false)
            .expect("camera motion blur jump should succeed");
        handler
            .camera_motion_blur_follow(8)
            .expect("camera motion blur follow should succeed");
        handler
            .camera_motion_blur_end_follow()
            .expect("camera motion blur end follow should succeed");
        handler
            .cameo_flash("Command_ConstructChinaBarracks", 7)
            .expect("cameo flash should succeed");
        handler
            .add_named_timer("TimerA", "Launch Window", true)
            .expect("add named timer should succeed");
        handler
            .remove_named_timer("TimerA")
            .expect("remove named timer should succeed");
        handler
            .show_named_timer_display(true)
            .expect("show named timer display should succeed");
        handler
            .set_superweapon_display_enabled_by_script(false)
            .expect("set superweapon display enabled should succeed");
        handler
            .hide_object_superweapon_display_by_script(77)
            .expect("hide object superweapon display should succeed");
        handler
            .show_object_superweapon_display_by_script(77)
            .expect("show object superweapon display should succeed");

        let popups = hooks.drain_popup_message_requests();
        assert_eq!(popups.len(), 1);
        assert_eq!(popups[0].message, "Incoming transmission");
        assert_eq!(popups[0].x_percent, 35);
        assert_eq!(popups[0].y_percent, 55);
        assert_eq!(popups[0].width, 420);
        assert!(popups[0].pause);
        assert!(!popups[0].pause_music);

        let guardbands = hooks.drain_view_guardband_requests();
        assert_eq!(
            guardbands,
            vec![ViewGuardbandRequest {
                x_bias: 1.25,
                y_bias: 0.75
            }]
        );

        let bw = hooks.drain_camera_bw_mode_requests();
        assert_eq!(
            bw,
            vec![CameraBwModeRequest {
                enabled: true,
                frames: 24
            }]
        );

        assert_eq!(hooks.drain_skybox_enabled_updates(), vec![false]);

        let blur = hooks.drain_camera_motion_blur_requests();
        assert_eq!(blur.len(), 4);
        assert_eq!(
            blur[0],
            CameraMotionBlurRequest::Basic {
                zoom_in: false,
                saturate: true
            }
        );
        assert_eq!(
            blur[1],
            CameraMotionBlurRequest::Jump {
                position: Vec3::new(10.0, 30.0, 20.0),
                saturate: false
            }
        );
        assert_eq!(blur[2], CameraMotionBlurRequest::Follow { amount: 8 });
        assert_eq!(blur[3], CameraMotionBlurRequest::EndFollow);

        let cameo = hooks.drain_cameo_flash_requests();
        assert_eq!(cameo.len(), 1);
        assert_eq!(
            cameo[0].command_button_name,
            "Command_ConstructChinaBarracks"
        );
        assert_eq!(cameo[0].flash_count, 7);

        let timers = hooks.drain_named_timer_mutations();
        assert_eq!(
            timers,
            vec![
                NamedTimerMutation::Add {
                    name: "TimerA".to_string(),
                    text: "Launch Window".to_string(),
                    countdown: true
                },
                NamedTimerMutation::Remove {
                    name: "TimerA".to_string()
                }
            ]
        );
        assert_eq!(hooks.drain_named_timer_display_updates(), vec![true]);
        assert_eq!(
            hooks.drain_superweapon_display_enabled_updates(),
            vec![false]
        );
        assert_eq!(
            hooks.drain_superweapon_object_display_mutations(),
            vec![
                SuperweaponObjectDisplayMutation::Hide { object_id: 77 },
                SuperweaponObjectDisplayMutation::Show { object_id: 77 }
            ]
        );
    }

    fn win_lose_layout_is_open(root_name: &str, child_name: &str) -> bool {
        game_client::gui::with_window_manager_ref(|wm| {
            wm.find_window_by_name(root_name).is_some()
                || wm.find_window_by_name(child_name).is_some()
        })
    }

    #[test]
    fn host_create_win_lose_window_loads_victorious_and_defeat_layouts() {
        // C++ ScriptActions.cpp:204/228 TheWindowManager->winCreateFromScript
        // ("Menus/Victorious.wnd" / "Menus/Defeat.wnd"). Live host handler must
        // not stay the ScriptActionHandler default no-op.
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks);
        let _ = handler.destroy_win_lose_window();

        handler
            .create_win_lose_window("Menus/Victorious.wnd")
            .expect("create Victorious.wnd");
        assert!(
            win_lose_layout_is_open("Victorious.wnd:", "Victorious.wnd:Victorious"),
            "host MissionScriptActionHandler must load Menus/Victorious.wnd"
        );

        handler
            .create_win_lose_window("Menus/Defeat.wnd")
            .expect("create Defeat.wnd");
        assert!(
            win_lose_layout_is_open("Defeat.wnd:Defeat", "Defeat.wnd:DefeatImage"),
            "host MissionScriptActionHandler must load Menus/Defeat.wnd"
        );
        assert!(
            !win_lose_layout_is_open("Victorious.wnd:", "Victorious.wnd:Victorious"),
            "creating Defeat.wnd must destroy the prior Victorious.wnd"
        );

        handler
            .destroy_win_lose_window()
            .expect("destroy Defeat.wnd");
        assert!(
            !win_lose_layout_is_open("Defeat.wnd:Defeat", "Defeat.wnd:DefeatImage"),
            "destroy_win_lose_window must remove the tracked message window"
        );
    }

    #[test]
    fn live_script_engine_victory_opens_victorious_wnd_via_host_handler() {
        // C++ ScriptActions::doVictory ScriptActions.cpp:191-209.
        initialize_script_engine().expect("script engine should initialize");
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler: Arc<dyn ScriptActionHandler> =
            Arc::new(MissionScriptActionHandler::new(hooks));
        if let Ok(mut guard) = get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.set_action_handler(Some(Arc::clone(&handler)));
                engine.close_windows(false);
                engine.create_win_lose_window("Menus/Victorious.wnd");
                assert_eq!(
                    engine.current_win_lose_window().as_deref(),
                    Some("Menus/Victorious.wnd")
                );
            }
        }
        assert!(
            win_lose_layout_is_open("Victorious.wnd:", "Victorious.wnd:Victorious"),
            "live ScriptEngine + host handler must materialise Victorious.wnd"
        );
        let _ = handler.destroy_win_lose_window();
    }

    #[test]
    fn live_quick_victory_starts_timer_then_posts_clear_game_data() {
        // C++ ScriptActions.cpp:169-176 doQuickVictory → startQuickEndGameTimer.
        // ScriptEngine.cpp:5514-5518 expiry appends MSG_CLEAR_GAME_DATA.
        use game_engine::common::message_stream::{GameMessageType, get_message_stream};
        use gamelogic::scripting::core::ScriptAction;
        use gamelogic::scripting::evaluator::ScriptEvaluator;

        initialize_script_engine().expect("script engine should initialize");
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler: Arc<dyn ScriptActionHandler> =
            Arc::new(MissionScriptActionHandler::new(hooks));
        {
            let stream = get_message_stream();
            let mut stream = stream.write().unwrap_or_else(|e| e.into_inner());
            stream.clear_messages();
        }
        if let Ok(mut guard) = get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine.set_action_handler(Some(Arc::clone(&handler)));
                engine.close_windows(false);
                engine.set_campaign_victorious(false);
            }
        }

        let evaluator = ScriptEvaluator::new(get_script_engine());
        evaluator
            .execute_action(&ScriptAction::new(ScriptActionType::Quickvictory))
            .expect("QuickVictory should execute");

        {
            let engine = get_script_engine();
            let guard = engine.read().expect("script engine");
            let engine = guard.as_ref().expect("initialized");
            assert!(
                engine.is_game_ending(),
                "C++ startQuickEndGameTimer must arm m_endGameTimer"
            );
            assert!(
                engine.is_campaign_victorious(),
                "C++ ScriptActions.cpp:175 SetVictorious(TRUE)"
            );
        }

        if let Ok(mut guard) = get_script_engine().write() {
            if let Some(engine) = guard.as_mut() {
                engine
                    .update()
                    .expect("one-frame quick-end timer should expire");
            }
        }

        let stream = get_message_stream();
        let stream = stream.read().unwrap_or_else(|e| e.into_inner());
        assert!(
            stream.contains_message_of_type(&GameMessageType::ClearGameData),
            "timer expiry must append MSG_CLEAR_GAME_DATA (ScriptEngine.cpp:5518)"
        );
        let _ = handler.destroy_win_lose_window();
    }

    #[test]
    fn reset_camera_to_forwards_scripted_ease() {
        let hooks = MissionScriptHooks::new().expect("hooks");
        let handler = MissionScriptActionHandler::new(hooks.clone());
        handler
            .reset_camera_to(10.0, 20.0, 3.0, 2.0, 0.4, 0.6)
            .expect("reset");
        let resets = hooks.drain_camera_resets();
        assert_eq!(resets.len(), 1);
        assert_eq!(resets[0].duration_seconds, 2.0);
        assert_eq!(resets[0].ease_in_seconds, 0.4);
        assert_eq!(resets[0].ease_out_seconds, 0.6);
        assert_eq!(resets[0].position, Vec3::new(10.0, 3.0, 20.0));
    }
}
