use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::localization;
use gamelogic::scripting::core::{Script, ScriptAction, ScriptActionType, ScriptList};
use gamelogic::scripting::engine::{
    get_script_engine, initialize_script_engine, ScriptActionHandler,
};
use gamelogic::scripting::evaluator::ScriptEvaluator;
use gamelogic::team::get_team_factory;
use gamelogic::{GameLogicError, GameLogicResult};
use glam::Vec3;

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
pub struct CameraResetRequest {
    pub position: Vec3,
    pub duration_seconds: f32,
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

fn camera_coord3d_to_world(x: f32, y: f32, z: f32) -> Vec3 {
    // Generals Coord3D: (x,y) on the map plane, z = height.
    // Main renderer world: x/z on the map plane, y = height.
    Vec3::new(x, z, y)
}

#[derive(Debug, Clone)]
struct ScriptState {
    completed: bool,
    next_frame_allowed: u64,
    pending_condition_cursor: Option<ShellPendingConditionCursor>,
    pending_action_cursor: Option<ShellPendingActionCursor>,
}

#[derive(Debug, Clone, Copy)]
enum ScriptActionBranch {
    TrueAction,
    FalseAction,
}

#[derive(Debug, Clone, Copy)]
struct ShellPendingActionCursor {
    branch: ScriptActionBranch,
    next_action_index: usize,
}

#[derive(Debug, Clone, Copy)]
struct ShellPendingConditionCursor {
    next_or_index: usize,
    next_and_index: usize,
}

#[derive(Debug, Clone, Copy)]
enum ShellConditionChunkResult {
    Pending(ShellPendingConditionCursor),
    Complete(bool),
}

#[derive(Debug, Clone, Copy)]
enum ShellConditionStepOutcome {
    Advance(ShellPendingConditionCursor),
    Complete(bool),
}

impl ScriptState {
    fn new() -> Self {
        Self {
            completed: false,
            next_frame_allowed: 0,
            pending_condition_cursor: None,
            pending_action_cursor: None,
        }
    }
}

#[derive(Clone)]
struct RuntimeScript {
    name: String,
    original_name: Option<String>,
    script: Script,
    state: ScriptState,
    enabled: bool,
}

pub struct MissionScriptRuntime {
    evaluator: ScriptEvaluator,
    scripts: Vec<RuntimeScript>,
    script_lookup: HashMap<String, usize>,
    original_lookup: HashMap<String, usize>,
    frame_counter: u64,
    next_script_index: usize,
}

impl MissionScriptRuntime {
    // Shell/map menu mode should remain responsive; defer known heavy attack-wave scripts briefly
    // during startup, then allow them to run with adaptive backoff based on observed runtime.
    const SHELL_HEAVY_SCRIPT_WARMUP_FRAMES: u64 = 600;
    const SHELL_SLOW_SCRIPT_WARN_MS: u64 = 40;
    const SHELL_HEAVY_CONDITIONS_PER_SLICE: usize = 12;
    const SHELL_HEAVY_CONDITION_SLICE_MS: u64 = 4;
    const SHELL_HEAVY_ACTIONS_PER_SLICE: usize = 3;
    const SHELL_HEAVY_SLICE_MS: u64 = 6;

    fn new() -> GameLogicResult<Self> {
        let _ = initialize_script_engine();
        let engine = get_script_engine();
        let evaluator = ScriptEvaluator::new(engine.clone());
        Ok(Self {
            evaluator,
            scripts: Vec::new(),
            script_lookup: HashMap::new(),
            original_lookup: HashMap::new(),
            frame_counter: 0,
            next_script_index: 0,
        })
    }

    fn install_lists(&mut self, lists: &[ScriptList]) {
        self.scripts.clear();
        self.script_lookup.clear();
        self.original_lookup.clear();
        self.frame_counter = 0;
        self.next_script_index = 0;

        for (list_index, list) in lists.iter().enumerate() {
            self.collect_chain(
                format!("List{}", list_index),
                list.first_script.as_deref(),
                true,
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
                self.collect_chain(
                    group_prefix,
                    script_group.get_script(),
                    script_group.is_active(),
                );
                group = script_group.get_next();
                group_index += 1;
            }
        }

        log::info!(
            "Mission script runtime registered {} WW3D scripts",
            self.scripts.len()
        );
        let enabled_count = self.scripts.iter().filter(|script| script.enabled).count();
        log::info!(
            "Mission script runtime enabled {} scripts at install",
            enabled_count
        );
        for script in self.scripts.iter().filter(|script| {
            script.name.contains("Move_Camera")
                || script.original_name.as_deref().is_some_and(|name| {
                    matches!(
                        name,
                        "move camera"
                            | "restart camera script"
                            | "restart camera"
                            | "restart camera really"
                            | "unshroud"
                            | "turn off sirens"
                    )
                })
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

    fn update_shell_budgeted(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.update_budgeted_internal(current_frame, max_scripts_per_frame, true)
    }

    fn update_budgeted(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.update_budgeted_internal(current_frame, max_scripts_per_frame, false)
    }

    fn update_budgeted_internal(
        &mut self,
        current_frame: u64,
        max_scripts_per_frame: Option<usize>,
        shell_safe_mode: bool,
    ) -> GameLogicResult<()> {
        if self.scripts.is_empty() {
            return Ok(());
        }
        self.frame_counter = current_frame;
        if current_frame <= 2 {
            let enabled: Vec<_> = self
                .scripts
                .iter()
                .filter(|script| script.enabled)
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
                    if shell_safe_mode
                        && Self::should_defer_heavy_shell_script(
                            &self.scripts[index],
                            self.frame_counter,
                        )
                    {
                        self.next_script_index = (self.next_script_index + 1) % len;
                        continue;
                    }
                    self.evaluate_script(index, shell_safe_mode)?;
                    self.next_script_index = (self.next_script_index + 1) % len;
                }
            }
            None => {
                for index in 0..self.scripts.len() {
                    if shell_safe_mode
                        && Self::should_defer_heavy_shell_script(
                            &self.scripts[index],
                            self.frame_counter,
                        )
                    {
                        continue;
                    }
                    self.evaluate_script(index, shell_safe_mode)?;
                }
                self.next_script_index = 0;
            }
        }
        Ok(())
    }

    fn should_defer_heavy_shell_script(script: &RuntimeScript, frame_counter: u64) -> bool {
        frame_counter < Self::SHELL_HEAVY_SCRIPT_WARMUP_FRAMES
            && Self::is_shell_heavy_attack_script(script)
    }

    fn is_shell_heavy_attack_script(script: &RuntimeScript) -> bool {
        let runtime = script.name.to_ascii_lowercase();
        let original = script
            .original_name
            .as_deref()
            .unwrap_or_default()
            .to_ascii_lowercase();

        let combined = format!("{runtime} {original}");
        (combined.contains("spawn") && combined.contains("attack"))
            || combined.contains("go_hunt")
            || combined.contains("go hunt")
            || combined.contains("gount")
            || combined.contains("mig")
                && (combined.contains("hunt") || combined.contains("attack"))
    }

    fn shell_slow_script_cooldown_frames(elapsed: Duration) -> u64 {
        let ms = elapsed.as_millis() as u64;
        if ms >= 500 {
            600
        } else if ms >= 250 {
            360
        } else if ms >= 120 {
            180
        } else if ms >= 80 {
            90
        } else if ms >= Self::SHELL_SLOW_SCRIPT_WARN_MS {
            30
        } else {
            0
        }
    }

    fn set_script_enabled(&mut self, name: &str, enabled: bool) -> GameLogicResult<()> {
        let idx = self.script_lookup.get(name).copied().or_else(|| {
            self.original_lookup
                .get(&name.to_ascii_lowercase())
                .copied()
        });

        if let Some(idx) = idx {
            let entry = &mut self.scripts[idx];
            entry.enabled = enabled;
            entry.script.set_active(enabled);
            entry.state.pending_condition_cursor = None;
            entry.state.pending_action_cursor = None;
            if enabled {
                entry.state.completed = false;
                entry.state.next_frame_allowed = self.frame_counter;
            }
            log::debug!(
                "Mission script runtime set '{}' enabled={} (runtime='{}')",
                name,
                enabled,
                entry.name
            );
        } else {
            log::warn!("Enable/Disable requested for unknown script '{}'", name);
        }
        Ok(())
    }

    fn collect_chain(&mut self, prefix: String, script: Option<&Script>, group_active: bool) {
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

            let original_key = if base.is_empty() {
                None
            } else {
                Some(base.to_ascii_lowercase())
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
                enabled: group_active && node.is_active(),
            });

            current = node.get_next();
            ordinal += 1;
        }
    }

    fn evaluate_script(&mut self, index: usize, shell_safe_mode: bool) -> GameLogicResult<()> {
        let entry = &mut self.scripts[index];
        if !entry.enabled {
            entry.state.pending_condition_cursor = None;
            entry.state.pending_action_cursor = None;
            return Ok(());
        }

        if entry.script.is_one_shot() && entry.state.completed {
            entry.state.pending_condition_cursor = None;
            entry.state.pending_action_cursor = None;
            return Ok(());
        }

        if self.frame_counter < entry.state.next_frame_allowed {
            return Ok(());
        }

        let shell_chunk_heavy = shell_safe_mode && Self::is_shell_heavy_attack_script(entry);
        let eval_started = Instant::now();
        let (condition_result, yielded) = if shell_chunk_heavy {
            Self::evaluate_shell_heavy_script_chunked(&self.evaluator, entry)?
        } else {
            entry.state.pending_condition_cursor = None;
            entry.state.pending_action_cursor = None;
            (self.evaluator.evaluate_script(&mut entry.script)?, false)
        };
        let eval_elapsed = eval_started.elapsed();
        if eval_elapsed >= Duration::from_millis(Self::SHELL_SLOW_SCRIPT_WARN_MS) {
            log::warn!(
                "Slow mission script evaluate: '{}' original={:?} took {:?} (frame={})",
                entry.name,
                entry.original_name,
                eval_elapsed,
                self.frame_counter
            );
        }

        if yielded {
            entry.state.next_frame_allowed = self.frame_counter.saturating_add(1);
            return Ok(());
        }

        if condition_result && entry.script.is_one_shot() {
            entry.state.completed = true;
        } else {
            entry.state.next_frame_allowed =
                self.frame_counter + delay_frames(entry.script.delay_evaluation_seconds);
        }

        if shell_safe_mode {
            let cooldown = Self::shell_slow_script_cooldown_frames(eval_elapsed);
            if cooldown > 0 {
                let deferred_until = self.frame_counter.saturating_add(cooldown);
                entry.state.next_frame_allowed = entry.state.next_frame_allowed.max(deferred_until);
                log::warn!(
                    "Shell script backoff: '{}' original={:?} eval={:?} cooldown={}f until_frame={}",
                    entry.name,
                    entry.original_name,
                    eval_elapsed,
                    cooldown,
                    entry.state.next_frame_allowed
                );
            }
        }

        Ok(())
    }

    fn evaluate_shell_heavy_script_chunked(
        evaluator: &ScriptEvaluator,
        entry: &mut RuntimeScript,
    ) -> GameLogicResult<(bool, bool)> {
        let (branch, condition_result, start_index) = match entry.state.pending_action_cursor {
            Some(cursor) => (
                cursor.branch,
                matches!(cursor.branch, ScriptActionBranch::TrueAction),
                cursor.next_action_index,
            ),
            None => {
                let condition_result = match Self::evaluate_shell_heavy_conditions_chunked(
                    evaluator,
                    &mut entry.script,
                    entry.state.pending_condition_cursor,
                )? {
                    ShellConditionChunkResult::Pending(cursor) => {
                        entry.state.pending_condition_cursor = Some(cursor);
                        return Ok((false, true));
                    }
                    ShellConditionChunkResult::Complete(result) => {
                        entry.state.pending_condition_cursor = None;
                        result
                    }
                };
                let branch = if condition_result {
                    ScriptActionBranch::TrueAction
                } else {
                    ScriptActionBranch::FalseAction
                };
                (branch, condition_result, 0)
            }
        };

        let Some(first_action) = Self::script_action_for_branch(&entry.script, branch) else {
            entry.state.pending_action_cursor = None;
            return Ok((condition_result, false));
        };

        let Some(action) = Self::script_action_at_index(first_action, start_index) else {
            entry.state.pending_action_cursor = None;
            return Ok((condition_result, false));
        };

        let (next_action_index, yielded, has_more_actions) =
            Self::execute_script_action_slice(evaluator, action, start_index, true, &entry.name)?;

        if yielded && has_more_actions {
            entry.state.pending_action_cursor = Some(ShellPendingActionCursor {
                branch,
                next_action_index,
            });
            log::debug!(
                "Chunked heavy shell script '{}' at action {}",
                entry.name,
                next_action_index
            );
            Ok((condition_result, true))
        } else {
            entry.state.pending_action_cursor = None;
            Ok((condition_result, false))
        }
    }

    fn evaluate_shell_heavy_conditions_chunked(
        evaluator: &ScriptEvaluator,
        script: &mut Script,
        pending_cursor: Option<ShellPendingConditionCursor>,
    ) -> GameLogicResult<ShellConditionChunkResult> {
        if script.condition.is_none() {
            return Ok(ShellConditionChunkResult::Complete(true));
        }

        let mut cursor = pending_cursor.unwrap_or(ShellPendingConditionCursor {
            next_or_index: 0,
            next_and_index: 0,
        });
        let slice_started = Instant::now();
        let mut evaluated_conditions = 0usize;

        loop {
            let step = Self::evaluate_shell_condition_step(evaluator, script, cursor)?;
            match step {
                ShellConditionStepOutcome::Complete(result) => {
                    return Ok(ShellConditionChunkResult::Complete(result));
                }
                ShellConditionStepOutcome::Advance(next_cursor) => {
                    cursor = next_cursor;
                    evaluated_conditions += 1;
                    if evaluated_conditions >= Self::SHELL_HEAVY_CONDITIONS_PER_SLICE
                        || slice_started.elapsed()
                            >= Duration::from_millis(Self::SHELL_HEAVY_CONDITION_SLICE_MS)
                    {
                        return Ok(ShellConditionChunkResult::Pending(cursor));
                    }
                }
            }
        }
    }

    fn evaluate_shell_condition_step(
        evaluator: &ScriptEvaluator,
        script: &mut Script,
        cursor: ShellPendingConditionCursor,
    ) -> GameLogicResult<ShellConditionStepOutcome> {
        let Some(first_or) = script.condition.as_deref_mut() else {
            return Ok(ShellConditionStepOutcome::Complete(true));
        };

        let mut current_or = Some(first_or);
        for _ in 0..cursor.next_or_index {
            current_or = current_or.and_then(|or_node| or_node.next_or.as_deref_mut());
        }
        let Some(or_node) = current_or else {
            return Ok(ShellConditionStepOutcome::Complete(false));
        };

        let mut current_and = or_node.first_and.as_deref_mut();
        for _ in 0..cursor.next_and_index {
            current_and =
                current_and.and_then(|and_node| and_node.next_and_condition.as_deref_mut());
        }

        let Some(and_node) = current_and else {
            if or_node.next_or.is_some() {
                return Ok(ShellConditionStepOutcome::Advance(
                    ShellPendingConditionCursor {
                        next_or_index: cursor.next_or_index + 1,
                        next_and_index: 0,
                    },
                ));
            }
            return Ok(ShellConditionStepOutcome::Complete(false));
        };

        let (condition_result, has_next_and) = {
            let result = evaluator.evaluate_condition(and_node)?;
            (result, and_node.next_and_condition.is_some())
        };

        if condition_result {
            if has_next_and {
                Ok(ShellConditionStepOutcome::Advance(
                    ShellPendingConditionCursor {
                        next_or_index: cursor.next_or_index,
                        next_and_index: cursor.next_and_index + 1,
                    },
                ))
            } else {
                Ok(ShellConditionStepOutcome::Complete(true))
            }
        } else if or_node.next_or.is_some() {
            Ok(ShellConditionStepOutcome::Advance(
                ShellPendingConditionCursor {
                    next_or_index: cursor.next_or_index + 1,
                    next_and_index: 0,
                },
            ))
        } else {
            Ok(ShellConditionStepOutcome::Complete(false))
        }
    }

    fn execute_script_action_slice(
        evaluator: &ScriptEvaluator,
        start_action: &ScriptAction,
        start_index: usize,
        shell_safe_mode: bool,
        script_name: &str,
    ) -> GameLogicResult<(usize, bool, bool)> {
        let slice_started = Instant::now();
        let mut current_action = Some(start_action);
        let mut executed = 0usize;
        let mut next_action_index = start_index;

        while let Some(action) = current_action {
            if shell_safe_mode && Self::should_skip_shell_pathological_action(action) {
                let team_ctx = Self::describe_action_team_context(action);
                log::warn!(
                    "Skipping pathological shell action script='{}' action={:?}{} to preserve menu responsiveness",
                    script_name,
                    action.get_action_type(),
                    team_ctx
                );
                executed += 1;
                next_action_index += 1;
                current_action = action.get_next();
                continue;
            }
            let action_started = Instant::now();
            evaluator.execute_action(action)?;
            let action_elapsed = action_started.elapsed();
            if action_elapsed >= Duration::from_millis(Self::SHELL_SLOW_SCRIPT_WARN_MS) {
                let team_ctx = Self::describe_action_team_context(action);
                log::warn!(
                    "Slow mission script action execute: script='{}' action={:?} took {:?}{}",
                    script_name,
                    action.get_action_type(),
                    action_elapsed,
                    team_ctx
                );
            }
            executed += 1;
            next_action_index += 1;
            let next = action.get_next();
            let has_more = next.is_some();

            if has_more
                && (executed >= Self::SHELL_HEAVY_ACTIONS_PER_SLICE
                    || slice_started.elapsed() >= Duration::from_millis(Self::SHELL_HEAVY_SLICE_MS))
            {
                return Ok((next_action_index, true, true));
            }

            current_action = next;
        }

        Ok((next_action_index, false, false))
    }

    fn should_skip_shell_pathological_action(action: &ScriptAction) -> bool {
        let _ = action;
        false
    }

    fn describe_action_team_context(action: &ScriptAction) -> String {
        let Some(team_name_param) = action.get_parameter(0) else {
            return String::new();
        };
        let team_name = team_name_param.get_string();
        if team_name.is_empty() {
            return String::new();
        }

        let mut total_members = 0usize;
        let mut alive_members = 0usize;
        if let Ok(mut factory) = get_team_factory().lock() {
            if let Some(team_arc) = factory.find_team(team_name) {
                if let Ok(team) = team_arc.read() {
                    let members = team.get_members();
                    total_members = members.len();
                    let mut alive = 0usize;
                    for member in members {
                        let Some(obj) =
                            gamelogic::helpers::TheGameLogic::find_object_by_id(*member)
                        else {
                            continue;
                        };
                        let Ok(obj_guard) = obj.read() else {
                            continue;
                        };
                        if !obj_guard.is_effectively_dead() {
                            alive += 1;
                        }
                    }
                    alive_members = alive;
                }
            }
        }

        format!(
            " team='{}' members={} alive={}",
            team_name, total_members, alive_members
        )
    }

    fn script_action_for_branch(
        script: &Script,
        branch: ScriptActionBranch,
    ) -> Option<&ScriptAction> {
        match branch {
            ScriptActionBranch::TrueAction => script.get_action(),
            ScriptActionBranch::FalseAction => script.get_false_action(),
        }
    }

    fn script_action_at_index(start_action: &ScriptAction, index: usize) -> Option<&ScriptAction> {
        let mut current = Some(start_action);
        let mut offset = 0usize;
        while offset < index {
            current = current?.get_next();
            offset += 1;
        }
        current
    }
}

pub struct MissionScriptHooks {
    runtime: Mutex<MissionScriptRuntime>,
    pending_script_enabled_updates: Mutex<Vec<(String, bool)>>,
    messages: Mutex<Vec<String>>,
    sounds: Mutex<Vec<String>>,
    sound_events: Mutex<Vec<ScriptSoundEvent>>,
    camera_moves: Mutex<Vec<Vec3>>,
    camera_follows: Mutex<Vec<CameraFollowRequest>>,
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
    radar_enabled_updates: Mutex<Vec<bool>>,
    weather_visibility_updates: Mutex<Vec<bool>>,
    music_stop_requests: Mutex<Vec<()>>,
    oversize_terrain_requests: Mutex<Vec<i32>>,
    border_shroud_levels: Mutex<Vec<u8>>,
    camera_movement_finished: AtomicBool,
    frame_counter: AtomicU64,
    video_complete_frame: Mutex<HashMap<String, u64>>,
    speech_complete_frame: Mutex<HashMap<String, u64>>,
    audio_complete_frame: Mutex<HashMap<String, u64>>,
    music_complete_frame: Mutex<HashMap<String, u64>>,
}

impl MissionScriptHooks {
    pub fn new() -> GameLogicResult<Arc<Self>> {
        Ok(Arc::new(Self {
            runtime: Mutex::new(MissionScriptRuntime::new()?),
            pending_script_enabled_updates: Mutex::new(Vec::new()),
            messages: Mutex::new(Vec::new()),
            sounds: Mutex::new(Vec::new()),
            sound_events: Mutex::new(Vec::new()),
            camera_moves: Mutex::new(Vec::new()),
            camera_follows: Mutex::new(Vec::new()),
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
            radar_enabled_updates: Mutex::new(Vec::new()),
            weather_visibility_updates: Mutex::new(Vec::new()),
            music_stop_requests: Mutex::new(Vec::new()),
            oversize_terrain_requests: Mutex::new(Vec::new()),
            border_shroud_levels: Mutex::new(Vec::new()),
            camera_movement_finished: AtomicBool::new(true),
            frame_counter: AtomicU64::new(0),
            video_complete_frame: Mutex::new(HashMap::new()),
            speech_complete_frame: Mutex::new(HashMap::new()),
            audio_complete_frame: Mutex::new(HashMap::new()),
            music_complete_frame: Mutex::new(HashMap::new()),
        }))
    }

    pub fn install_lists(&self, lists: &[ScriptList]) {
        if let Ok(mut runtime) = self.runtime.lock() {
            runtime.install_lists(lists);
        }
    }

    pub fn update(&self, frame: u64) -> GameLogicResult<()> {
        self.update_budgeted(frame, None)
    }

    pub fn update_shell_budgeted(
        &self,
        frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.frame_counter.store(frame, Ordering::Relaxed);
        let pending = self
            .pending_script_enabled_updates
            .lock()
            .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
            .unwrap_or_default();
        let mut runtime = self.runtime.lock().map_err(|_| {
            GameLogicError::Configuration("Mission script runtime mutex poisoned".to_string())
        })?;
        for (name, enabled) in pending {
            runtime.set_script_enabled(&name, enabled)?;
        }
        runtime.update_shell_budgeted(frame, max_scripts_per_frame)?;
        Ok(())
    }

    pub fn update_budgeted(
        &self,
        frame: u64,
        max_scripts_per_frame: Option<usize>,
    ) -> GameLogicResult<()> {
        self.frame_counter.store(frame, Ordering::Relaxed);
        let pending = self
            .pending_script_enabled_updates
            .lock()
            .map(|mut queue| queue.drain(..).collect::<Vec<_>>())
            .unwrap_or_default();
        let mut runtime = self.runtime.lock().map_err(|_| {
            GameLogicError::Configuration("Mission script runtime mutex poisoned".to_string())
        })?;
        for (name, enabled) in pending {
            runtime.set_script_enabled(&name, enabled)?;
        }
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

    pub fn disable_attack_wave_scripts(&self) -> usize {
        let mut disabled = 0usize;
        if let Ok(mut runtime) = self.runtime.lock() {
            for entry in runtime.scripts.iter_mut() {
                let runtime_name = entry.name.to_ascii_lowercase();
                let original_name = entry.original_name.clone().unwrap_or_default();
                let original_name = original_name.to_ascii_lowercase();
                let looks_like_attack_wave = (runtime_name.contains("attack")
                    && runtime_name.contains("spawn"))
                    || (original_name.contains("and attack") && original_name.contains("spawn"));
                if looks_like_attack_wave && entry.enabled {
                    entry.enabled = false;
                    entry.script.set_active(false);
                    entry.state.completed = true;
                    disabled += 1;
                }
            }
        }
        disabled
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
        if let Ok(mut queue) = self.camera_zoom_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_pitch(&self, request: CameraPitchRequest) {
        if let Ok(mut queue) = self.camera_pitch_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_rotate(&self, request: CameraRotateRequest) {
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
        if let Ok(mut queue) = self.camera_look_toward_object_requests.lock() {
            queue.push(request);
        }
    }

    pub fn push_camera_look_toward_waypoint(&self, request: CameraLookTowardWaypointRequest) {
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
        let now = self.frame_counter.load(Ordering::Relaxed);
        if let Ok(mut map) = self.video_complete_frame.lock() {
            map.insert(filename.clone(), now.saturating_add(1));
        }
        if let Ok(mut queue) = self.movie_requests.lock() {
            queue.push(filename);
        }
    }

    pub fn push_radar_movie_request(&self, filename: String) {
        let now = self.frame_counter.load(Ordering::Relaxed);
        if let Ok(mut map) = self.video_complete_frame.lock() {
            map.insert(filename.clone(), now.saturating_add(1));
        }
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

    pub fn push_radar_enabled(&self, enabled: bool) {
        if let Ok(mut queue) = self.radar_enabled_updates.lock() {
            queue.push(enabled);
        }
    }

    pub fn push_weather_visible(&self, visible: bool) {
        if let Ok(mut queue) = self.weather_visibility_updates.lock() {
            queue.push(visible);
        }
    }

    pub fn push_popup_message(&self, request: ScriptPopupMessageRequest) {
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
        let now = self.frame_counter.load(Ordering::Relaxed);
        if let Ok(mut map) = self.speech_complete_frame.lock() {
            map.insert(name.to_string(), now.saturating_add(1));
        }
    }

    pub fn note_audio_started(&self, name: &str) {
        let now = self.frame_counter.load(Ordering::Relaxed);
        if let Ok(mut map) = self.audio_complete_frame.lock() {
            map.insert(name.to_string(), now.saturating_add(1));
        }
    }

    pub fn note_music_started(&self, name: &str) {
        let now = self.frame_counter.load(Ordering::Relaxed);
        if let Ok(mut map) = self.music_complete_frame.lock() {
            map.insert(name.to_string(), now.saturating_add(1));
        }
    }

    pub fn mark_music_stopped(&self) {
        let now = self.frame_counter.load(Ordering::Relaxed);
        if let Ok(mut map) = self.music_complete_frame.lock() {
            for done_frame in map.values_mut() {
                *done_frame = now;
            }
        }
    }

    pub fn is_video_complete(&self, name: &str, flush: bool) -> bool {
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut map) = self.video_complete_frame.lock() else {
            return true;
        };
        let Some(&done_frame) = map.get(name) else {
            return true;
        };
        let done = now >= done_frame;
        if done && flush {
            map.remove(name);
        }
        done
    }

    pub fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut map) = self.speech_complete_frame.lock() else {
            return true;
        };
        let Some(&done_frame) = map.get(name) else {
            return true;
        };
        let done = now >= done_frame;
        if done && flush {
            map.remove(name);
        }
        done
    }

    pub fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut map) = self.audio_complete_frame.lock() else {
            return true;
        };
        let Some(&done_frame) = map.get(name) else {
            return true;
        };
        let done = now >= done_frame;
        if done && flush {
            map.remove(name);
        }
        done
    }

    pub fn has_music_track_completed(&self, track: &str, flush: bool) -> bool {
        let now = self.frame_counter.load(Ordering::Relaxed);
        let Ok(mut map) = self.music_complete_frame.lock() else {
            return true;
        };
        let Some(&done_frame) = map.get(track) else {
            return true;
        };
        let done = now >= done_frame;
        if done && flush {
            map.remove(track);
        }
        done
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

    pub fn drain_radar_enabled_updates(&self) -> Vec<bool> {
        self.radar_enabled_updates
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
        self.hooks.note_audio_started(sound);
        self.hooks.push_sound(sound.to_string());
        Ok(())
    }

    fn play_sound_effect_at(&self, sound: &str, x: f32, y: f32, z: f32) -> GameLogicResult<()> {
        self.hooks.note_audio_started(sound);
        self.hooks.push_sound_event(ScriptSoundEvent {
            sound_name: sound.to_string(),
            position: Some(camera_coord3d_to_world(x, y, z)),
        });
        Ok(())
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
            object_id: object_id as u32,
            snap_to_unit,
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
    ) -> GameLogicResult<()> {
        self.hooks.push_camera_reset(CameraResetRequest {
            position: camera_coord3d_to_world(x, y, z),
            duration_seconds,
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
            SuperweaponObjectDisplayMutation::Hide {
                object_id: object_id as u32,
            },
        );
        Ok(())
    }

    fn show_object_superweapon_display_by_script(
        &self,
        object_id: gamelogic::common::ObjectID,
    ) -> GameLogicResult<()> {
        self.hooks.push_superweapon_object_display_mutation(
            SuperweaponObjectDisplayMutation::Show {
                object_id: object_id as u32,
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
                object_id: object_id as u32,
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

    fn speech_play(&self, name: &str, _allow_overlap: bool) -> GameLogicResult<()> {
        self.hooks.note_speech_started(name);
        self.hooks.push_sound(name.to_string());
        Ok(())
    }

    fn is_speech_complete(&self, name: &str, flush: bool) -> bool {
        self.hooks.is_speech_complete(name, flush)
    }

    fn is_audio_complete(&self, name: &str, flush: bool) -> bool {
        self.hooks.is_audio_complete(name, flush)
    }

    fn music_set_track(&self, track: &str, _fade_out: bool, _fade_in: bool) -> GameLogicResult<()> {
        self.hooks.note_music_started(track);
        self.hooks.push_message(format!("Music track: {}", track));
        Ok(())
    }

    fn has_music_track_completed(&self, track: &str, param: i32) -> bool {
        self.hooks.has_music_track_completed(track, param != 0)
    }

    fn stop_music(&self) -> GameLogicResult<()> {
        self.hooks.mark_music_stopped();
        self.hooks.push_music_stop();
        Ok(())
    }

    fn set_radar_enabled(&self, enabled: bool) -> GameLogicResult<()> {
        self.hooks.push_radar_enabled(enabled);
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
    use gamelogic::scripting::core::{Condition, ConditionType, OrCondition, ScriptActionType};

    fn build_noop_action_chain(count: usize) -> Option<Box<ScriptAction>> {
        let mut head = None;
        for _ in 0..count {
            let mut action = Box::new(ScriptAction::new(ScriptActionType::NoOp));
            action.set_next_action(head);
            head = Some(action);
        }
        head
    }

    fn build_true_condition_chain(count: usize) -> Option<Box<OrCondition>> {
        let mut head = None;
        for _ in 0..count {
            let mut condition = Box::new(Condition::new(ConditionType::ConditionTrue));
            condition.set_next_condition(head);
            head = Some(condition);
        }
        let mut or_condition = Box::new(OrCondition::new());
        or_condition.set_first_and_condition(head);
        Some(or_condition)
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
            handler.has_music_track_completed("TrackA", 0),
            "unknown tracks should be treated as completed"
        );

        handler
            .music_set_track("TrackA", false, false)
            .expect("music set track should succeed");
        assert!(
            !handler.has_music_track_completed("TrackA", 0),
            "track should not be completed on the same frame it starts"
        );

        hooks.update(1).expect("frame advance should succeed");
        assert!(
            handler.has_music_track_completed("TrackA", 1),
            "track should complete after at least one frame"
        );
        assert!(
            handler.has_music_track_completed("TrackA", 0),
            "flushed completed track should be treated as done"
        );
    }

    #[test]
    fn stop_music_marks_tracked_music_as_complete() {
        let hooks = MissionScriptHooks::new().expect("mission script hooks should initialize");
        let handler = MissionScriptActionHandler::new(hooks.clone());

        handler
            .music_set_track("TrackB", false, false)
            .expect("music set track should succeed");
        assert!(
            !handler.has_music_track_completed("TrackB", 0),
            "newly started track should be incomplete before stop"
        );

        handler.stop_music().expect("stop music should succeed");
        assert!(
            handler.has_music_track_completed("TrackB", 0),
            "stop music should immediately complete tracked music"
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

    #[test]
    fn heavy_shell_scripts_only_defer_during_warmup() {
        let heavy = RuntimeScript {
            name: "List3::Tech_Attacks::Spawn_Techs_And_Attack".to_string(),
            original_name: Some("spawn techs and attack".to_string()),
            script: Script::new(),
            state: ScriptState::new(),
            enabled: true,
        };

        assert!(MissionScriptRuntime::should_defer_heavy_shell_script(
            &heavy,
            MissionScriptRuntime::SHELL_HEAVY_SCRIPT_WARMUP_FRAMES - 1
        ));
        assert!(!MissionScriptRuntime::should_defer_heavy_shell_script(
            &heavy,
            MissionScriptRuntime::SHELL_HEAVY_SCRIPT_WARMUP_FRAMES
        ));
    }

    #[test]
    fn shell_heavy_script_detection_matches_attack_wave_patterns() {
        let heavy = RuntimeScript {
            name: "List3::Combat_Cycle_Attacks::Spawn_Bikes_And_Attack".to_string(),
            original_name: Some("spawn bikes and attack".to_string()),
            script: Script::new(),
            state: ScriptState::new(),
            enabled: true,
        };
        let allowed = RuntimeScript {
            name: "List0::Move_Camera".to_string(),
            original_name: Some("move camera".to_string()),
            script: Script::new(),
            state: ScriptState::new(),
            enabled: true,
        };

        assert!(MissionScriptRuntime::is_shell_heavy_attack_script(&heavy));
        assert!(!MissionScriptRuntime::is_shell_heavy_attack_script(
            &allowed
        ));
    }

    #[test]
    fn heavy_shell_script_actions_are_chunked_across_frames() {
        let runtime =
            MissionScriptRuntime::new().expect("mission script runtime should initialize");
        let mut script = Script::new();
        script.set_name("spawn techs and attack".to_string());
        script.set_action(build_noop_action_chain(5));

        let mut entry = RuntimeScript {
            name: "List3::Tech_Attacks::Spawn_Techs_And_Attack".to_string(),
            original_name: Some("spawn techs and attack".to_string()),
            script,
            state: ScriptState::new(),
            enabled: true,
        };

        let (condition_result, yielded) =
            MissionScriptRuntime::evaluate_shell_heavy_script_chunked(
                &runtime.evaluator,
                &mut entry,
            )
            .expect("first shell chunk should run");
        assert!(condition_result);
        assert!(yielded);
        let first = &entry;

        assert!(
            first.state.pending_action_cursor.is_some(),
            "heavy shell script should yield after first action slice"
        );
        assert!(
            !first.state.completed,
            "one-shot script should not complete until the full action chain runs"
        );

        let (_, second_yielded) = MissionScriptRuntime::evaluate_shell_heavy_script_chunked(
            &runtime.evaluator,
            &mut entry,
        )
        .expect("second shell chunk should complete remaining actions");
        assert!(!second_yielded);
        if entry.script.is_one_shot() {
            entry.state.completed = true;
        }

        let second = &entry;
        assert!(
            second.state.pending_action_cursor.is_none(),
            "second slice should finish the action chain"
        );
        assert!(second.state.completed, "one-shot script should complete");
    }

    #[test]
    fn heavy_shell_script_conditions_are_chunked_across_frames() {
        let runtime =
            MissionScriptRuntime::new().expect("mission script runtime should initialize");
        let mut script = Script::new();
        script.set_name("spawn bikes and attack".to_string());
        script.set_or_condition(build_true_condition_chain(
            MissionScriptRuntime::SHELL_HEAVY_CONDITIONS_PER_SLICE + 6,
        ));
        script.set_action(build_noop_action_chain(1));

        let mut entry = RuntimeScript {
            name: "List3::Combat_Cycle_Attacks::Spawn_Bikes_And_Attack".to_string(),
            original_name: Some("spawn bikes and attack".to_string()),
            script,
            state: ScriptState::new(),
            enabled: true,
        };

        let (_, yielded_first) = MissionScriptRuntime::evaluate_shell_heavy_script_chunked(
            &runtime.evaluator,
            &mut entry,
        )
        .expect("condition slice should run");
        assert!(
            yielded_first,
            "long heavy condition chains should yield to the next frame"
        );
        assert!(
            entry.state.pending_condition_cursor.is_some(),
            "yielded condition pass must retain continuation cursor"
        );
        assert!(
            entry.state.pending_action_cursor.is_none(),
            "action evaluation must not begin before condition pass completes"
        );
    }

    #[test]
    fn disabling_script_clears_pending_shell_action_cursor() {
        let mut runtime =
            MissionScriptRuntime::new().expect("mission script runtime should initialize");
        let mut script = Script::new();
        script.set_name("spawn techs and attack".to_string());
        script.set_action(build_noop_action_chain(5));

        runtime.scripts.push(RuntimeScript {
            name: "List3::Tech_Attacks::Spawn_Techs_And_Attack".to_string(),
            original_name: Some("spawn techs and attack".to_string()),
            script,
            state: ScriptState::new(),
            enabled: true,
        });
        runtime
            .original_lookup
            .insert("spawn techs and attack".to_string(), 0);
        runtime.scripts[0].state.pending_condition_cursor = Some(ShellPendingConditionCursor {
            next_or_index: 1,
            next_and_index: 0,
        });
        runtime.scripts[0].state.pending_action_cursor = Some(ShellPendingActionCursor {
            branch: ScriptActionBranch::TrueAction,
            next_action_index: 2,
        });

        assert!(runtime.scripts[0].state.pending_condition_cursor.is_some());
        assert!(runtime.scripts[0].state.pending_action_cursor.is_some());

        runtime
            .set_script_enabled("spawn techs and attack", false)
            .expect("disable script should succeed");
        assert!(
            runtime.scripts[0].state.pending_condition_cursor.is_none(),
            "disabling must clear pending condition cursor to avoid stale continuation"
        );
        assert!(
            runtime.scripts[0].state.pending_action_cursor.is_none(),
            "disabling must clear pending cursor to avoid stale continuation"
        );
    }

    #[test]
    fn shell_slow_script_backoff_scales_with_runtime() {
        assert_eq!(
            MissionScriptRuntime::shell_slow_script_cooldown_frames(Duration::from_millis(10)),
            0
        );
        assert_eq!(
            MissionScriptRuntime::shell_slow_script_cooldown_frames(Duration::from_millis(45)),
            30
        );
        assert_eq!(
            MissionScriptRuntime::shell_slow_script_cooldown_frames(Duration::from_millis(120)),
            180
        );
        assert_eq!(
            MissionScriptRuntime::shell_slow_script_cooldown_frames(Duration::from_millis(500)),
            600
        );
    }
}
