#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

/// Private staging error preserves the actual failed logical owner. Public
/// SaveLoadError remains unchanged only AFTER the receiving owner retains it.
struct RestoreStagingFailure {
    reason: String,
    retained_logic: Option<Box<crate::game_logic::GameLogic>>,
}
impl From<String> for RestoreStagingFailure {
    fn from(reason: String) -> Self {
        Self {
            reason,
            retained_logic: None,
        }
    }
}
impl From<&str> for RestoreStagingFailure {
    fn from(reason: &str) -> Self {
        reason.to_owned().into()
    }
}
impl std::fmt::Display for RestoreStagingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.reason, f)
    }
}
impl std::fmt::Debug for RestoreStagingFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestoreStagingFailure")
            .field("reason", &self.reason)
            .field("retains_logic", &self.retained_logic.is_some())
            .finish()
    }
}
impl std::ops::Deref for RestoreStagingFailure {
    type Target = str;
    fn deref(&self) -> &str {
        &self.reason
    }
}

/// A decoded save restored into a private host `GameLogic` plus the exact
/// process-global runtime bundle that map loading touched while staging it.
/// The bundle stays opaque until the single combined commit boundary below.
struct StagedRestoreWorld {
    native_map: super::map_native_admission::NativeMapCandidate,
    logic: crate::game_logic::GameLogic,
    info: SaveGameInfo,
    runtime_world: gamelogic::runtime_world_transaction::StagedHostRuntimeWorld,
    shroud: gamelogic::system::shroud_manager::ShroudSnapshot,
    /// Renderer-owned Drawable state decoded with the logical world.  It is
    /// queued only after the staged world has committed, then validated
    /// against the first fresh presentation topology by RenderPipeline.
    client_drawables: crate::save_load::snapshot::ClientDrawableWorldSnapshot,
    /// Decoded client state stays detached until the candidate world commits.
    game_client_xfer_bytes: Option<Vec<u8>>,
    /// Native terrain effects stay detached until this candidate commits.
    terrain_visual: Option<crate::save_load::snapshot::PreparedTerrainVisual>,
    /// Immutable successful source map configuration receipt; not a second owner.
    map_water_configuration: Option<gamelogic::terrain_water::WaterGridState>,
    client_persist: crate::save_load::snapshot::WorldPersistV18,
    camera_continuation: Option<NativeCameraContinuation>,
    owns_session_camera: bool,
    /// Detached groups decoded before candidate admission; no pending slot.
    control_groups: Option<std::collections::HashMap<i32, std::collections::HashMap<u8, Vec<u32>>>>,
    /// Driving-instance Rust RNG state capsule captured at save time.
    /// The commit boundary restores the staged instance from these words
    /// rather than leaving it on the game-start seed. `[0; 6]` marks a
    /// pre-v22 save: keep that stream.
    logic_rng_seed_words: [u32; 6],
}

/// A staging error leaves the old match intact. An error after world install
/// requires the C++ GameState load-failure reset before the menu can render.
#[derive(Debug)]
pub(super) enum HostLoadFailure {
    Staging(String),
    Committed(String),
}

impl HostLoadFailure {
    fn requires_match_reset(&self) -> bool {
        matches!(self, Self::Committed(_))
    }

    fn into_message(self) -> String {
        match self {
            Self::Staging(message) | Self::Committed(message) => message,
        }
    }
}

/// Transfer only after staged restore preflight. The receiving window's
/// viewport is captured before moving its old SessionView out of the world.
/// Saved cameras and legacy pose-only saves both retain that same viewport.
fn admit_staged_session_view(
    receiver: &mut crate::presentation_frame::SessionView,
    candidate: &mut crate::presentation_frame::SessionView,
    owns_session_camera: bool,
) {
    let receiving_viewport = receiver.camera.viewport;
    if !owns_session_camera {
        let candidate_focus_previous = candidate.pending_focus_previous_target;
        let candidate_camera_settings = candidate.camera.settings;
        let candidate_camera_clip = candidate.camera.clip_policy;
        let candidate_camera_shake = candidate.camera.shake_policy;
        let candidate_camera_movement = candidate.camera.movement_policy;
        let candidate_camera_input = candidate.camera.input_policy;
        let candidate_scripted = std::mem::take(&mut candidate.scripted);
        // Map actions already applied to the candidate's receiving filter.
        #[cfg(feature = "game_client")]
        let candidate_filter = std::mem::take(&mut candidate.filter);
        *candidate = std::mem::take(receiver);
        candidate.pending_focus_previous_target = candidate_focus_previous;
        candidate.camera.settings = candidate_camera_settings;
        candidate.camera.clip_policy = candidate_camera_clip;
        candidate.camera.shake_policy = candidate_camera_shake;
        candidate.camera.movement_policy = candidate_camera_movement;
        candidate.camera.input_policy = candidate_camera_input;
        candidate.scripted = candidate_scripted;
        #[cfg(feature = "game_client")]
        {
            candidate.filter = candidate_filter;
        }
    }
    // View dimensions belong to the receiving window, never to a saved world.
    // V2 may replace the complete camera continuation but not its viewport.
    candidate.pending_letterbox_visibility.clear();
    candidate.letterbox_visibility_delivered_since_present = false;
    candidate.camera.viewport = receiving_viewport;
    candidate.camera.rebuild_projection();
}

#[cfg(test)]
#[path = "host_authority/restore_viewport_owner_tests.rs"]
mod restore_viewport_owner_tests;

/// Definitions and their actual numeric-name namespace travel together into
/// a detached saved-world candidate. This borrow cannot select another world.
#[derive(Clone, Copy)]
pub(super) struct RestoreTemplateCatalog<'a> {
    body_damage_baseline: crate::game_logic::body_damage_rules::BodyDamagePolicy,
    lighting_baseline: &'a crate::game_logic::lighting_rules::LightingContentRules,
    templates: &'a std::collections::HashMap<String, crate::game_logic::ThingTemplate>,
    modules:
        &'a Option<game_engine::common::thing::receiving_module_content::ReceivingModuleContent>,
    object_definitions: &'a Option<crate::assets::ReceivingObjectDefinitions>,
    baseline: &'a Option<std::collections::HashMap<String, crate::game_logic::ThingTemplate>>,
}

impl<'a> RestoreTemplateCatalog<'a> {
    pub(super) fn from_world(world: &'a crate::game_logic::GameLogic) -> Self {
        Self {
            templates: &world.templates,
            modules: &world.receiving_module_content,
            object_definitions: &world.receiving_object_definitions,
            baseline: &world.receiving_template_baseline,
            lighting_baseline: world.lighting_rules_baseline(),
            body_damage_baseline: world.body_damage_baseline(),
        }
    }
}

impl CnCGameEngine {
    /// Reserve the next receiving identity without touching the live world.
    #[inline]
    pub(super) fn next_native_world_epoch(&self) -> u64 {
        let next = self.host_direct_visual_world_epoch.wrapping_add(1);
        if next == 0 { 1 } else { next }
    }
    pub(super) fn commit_native_world_epoch(&mut self, epoch: u64) {
        debug_assert_eq!(epoch, self.next_native_world_epoch());
        #[cfg(feature = "game_client")]
        self.control_bar.clear_observer_target_owned();
        self.host_direct_visual_world_epoch = epoch;
        self.native_display.reset_world(epoch);
    }
    #[cfg(feature = "game_client")]
    pub(super) fn discard_receiving_world_candidate(
        &mut self,
        world: &mut crate::game_logic::GameLogic,
        candidate: &mut super::map_native_admission::NativeMapCandidate,
    ) -> Result<(), String> {
        use crate::game_logic::NativeMapAdmission;
        if candidate.retirement_failed() {
            return Err("native candidate destructor previously failed; retry prohibited".into());
        }
        let policy = super::native_view_effects::receiving_native_drawable_policy_for(world);
        let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
        let mut attempt = candidate.attempt(
            self.render_pipeline.borrow_native_draw_services(),
            &mut self.graphics_system,
            self.current_player_id,
            policy,
            &mut retirement,
        )?;
        world
            .destroy_all_objects_immediate_with_effects_checked(&mut attempt.effects())
            .map_err(|error| error.to_string())?;
        attempt.discard_attempt()
    }

    #[cfg(not(feature = "game_client"))]
    pub(super) fn discard_receiving_world_candidate(
        &mut self,
        world: &mut crate::game_logic::GameLogic,
        candidate: &mut super::map_native_admission::NativeMapCandidate,
    ) -> Result<(), String> {
        world
            .destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
            .map_err(|error| error.to_string())?;
        // This build owns only the detached CPU/audio payload, no Drawable.
        *candidate = Default::default();
        Ok(())
    }

    fn reject_logical_restore_candidate(
        manager: &mut SaveFileManager,
        mut world: crate::game_logic::GameLogic,
        reason: String,
    ) -> String {
        match world.destroy_all_objects_immediate_with_effects_checked(&mut Default::default()) {
            Ok(()) => reason,
            Err(failure) => {
                manager.retain_rejected_logical_world(Box::new(world));
                format!("{reason}; logical candidate cleanup: {failure}")
            }
        }
    }
    fn reject_receiving_world_candidate(
        &mut self,
        mut world: crate::game_logic::GameLogic,
        mut candidate: super::map_native_admission::NativeMapCandidate,
    ) -> Result<(), String> {
        match self.discard_receiving_world_candidate(&mut world, &mut candidate) {
            Ok(()) => Ok(()),
            Err(reason) => {
                self.rejected_native_worlds.retain(
                    Some(Box::new(world)),
                    candidate,
                    reason.clone(),
                );
                Err(reason)
            }
        }
    }
    fn retain_staging_failure(
        &mut self,
        failure: RestoreStagingFailure,
        candidate: super::map_native_admission::NativeMapCandidate,
    ) -> String {
        let RestoreStagingFailure {
            reason,
            retained_logic,
        } = failure;
        if retained_logic.is_some() {
            self.rejected_native_worlds
                .retain(retained_logic, candidate, reason.clone());
        }
        reason
    }

    /// Begin a new transient direct-visual world identity.
    ///
    /// This is deliberately separate from durable world/save identity.  A
    /// successful world replacement reconstructs client Drawables, while a
    /// failed staged/load attempt must leave the active associations valid.
    #[inline]
    pub(super) fn host_advance_direct_visual_world_epoch(&mut self) {
        self.commit_native_world_epoch(self.next_native_world_epoch());
    }

    #[inline]
    pub(super) fn host_update_ui_state(&mut self, player_id: u32) -> crate::ui::GameUIState {
        // Wave 585/862/872/880: prefer last presentation UI residual when freeze installed.
        if let Some(ui) = self.last_ui_state.clone() {
            let _ = player_id;
            return ui;
        }
        // Wave 880: rebuild UI residual from presentation freeze without live dual-read.
        if let Some(pres) = self.last_presentation_frame.clone() {
            let mut ui = crate::ui::GameUIState::default();
            pres.apply_to_ui_state(&mut ui);
            self.last_ui_state = Some(ui.clone());
            let _ = player_id;
            return ui;
        }
        // Wave 899: fail-closed boot default (no dual-read). Stamp empty residual.
        let _ = player_id;
        let ui = crate::ui::GameUIState::default();
        self.last_ui_state = Some(ui.clone());
        ui
    }

    /// Wave 585: host shell-map probe residual (`isInShellGame`).
    #[inline]
    pub(super) fn host_is_in_shell_game(&self) -> bool {
        // Wave 585/845/896: prefer host_match_in_shell residual after match stamp.
        if let Some(v) = self.host_match_in_shell {
            return v;
        }
        // Wave 896: fail-closed boot default true (menu/shell before match stamp).
        true
    }

    /// Wave 585: host world-size override residual (minimap/heightmap repair path).
    #[inline]
    pub(super) fn host_override_world_size(&mut self, width: f32, height: f32) {
        // Wave 585/865/891/915/933: world size via session-control authority.
        // Skip authority write when residual bounds already match requested size.
        let half_w = width * 0.5;
        let half_h = height * 0.5;
        let min = glam::Vec3::new(-half_w, 0.0, -half_h);
        let max = glam::Vec3::new(half_w, 0.0, half_h);
        if self.host_match_world_bounds != Some((min, max)) {
            self.with_native_camera_input_effects(|logic, effects, _| {
                logic.apply_session_control_op_with_effects(
                    crate::game_logic::SessionControlOp::OverrideWorldSize { width, height },
                    effects,
                )
            });
            self.host_match_world_bounds = Some((min, max));
        }
    }

    /// Wave 585: host world-bounds residual (boot path without presentation freeze).
    #[inline]
    pub(super) fn host_world_bounds(&self) -> (glam::Vec3, glam::Vec3) {
        // Wave 585/862: prefer freeze / host residual before live dual-read.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.world_env.world_bounds_vec3();
        }
        if let Some(b) = self.host_match_world_bounds {
            return b;
        }
        // Wave 898: fail-closed boot default (no dual-read).
        (glam::Vec3::ZERO, glam::Vec3::ZERO)
    }

    /// Wave 585: host first-opponent residual (debug victory hotkey).
    #[inline]
    pub(super) fn host_first_opponent_id(&self, player_id: u32) -> Option<u32> {
        // Wave 585/863: prefer host residual when stamped for local player.
        if player_id
            == self
                .host_match_local_player_id
                .unwrap_or(self.current_player_id)
        {
            if let Some(cached) = self.host_match_first_opponent_id {
                return cached;
            }
        } else if let Some(players) = self.host_match_diplomacy_players.as_ref() {
            // Warm diplomacy residual: fail-closed opponent lookup without live dual-read.
            return players.iter().find(|p| p.id != player_id).map(|p| p.id);
        } else if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres
                .players
                .iter()
                .find(|p| p.id != player_id)
                .map(|p| p.id);
        }
        // Wave 898: fail-closed boot default (no dual-read).
        let _ = player_id;
        None
    }

    /// Wave 584: host object-alive probe residual (upgrade honesty boot fallback).
    #[inline]
    pub(super) fn host_object_is_alive(&self, id: crate::game_logic::ObjectId) -> bool {
        // Wave 584/851/897: prefer host-stamped alive residual before fail-closed.
        if let Some(alive) = self.host_match_alive_object_ids.as_ref() {
            return alive.contains(&id.0);
        }
        // Wave 897: fail-closed boot default (no dual-read).
        let _ = id;
        false
    }

    /// Wave 584: presentation freeze owns object-alive residual when installed.
    /// Boot residual without freeze uses host object_is_alive probe.
    #[inline]
    pub(super) fn presentation_or_boot_object_alive(
        &self,
        id: crate::game_logic::ObjectId,
    ) -> bool {
        // Wave 584/851: presentation-first alive residual (ui_object_alive).
        if self.ui_object_alive(id) {
            return true;
        }
        // Wave 851: host-stamped alive residual before live boot probe.
        if let Some(alive) = self.host_match_alive_object_ids.as_ref() {
            return alive.contains(&id.0);
        }
        // Wave 895: fail-closed boot default (no dual-read). Alive residual is
        // stamped via host_refresh_local_train_producer_residuals.
        let _ = id;
        false
    }

    /// Wave 584: host shell-map tick residual (menu frame budgeted update).
    #[inline]
    pub(super) fn host_update_shell_with_budget(
        &mut self,
        dt: f32,
        budget: usize,
    ) -> std::result::Result<(), String> {
        // Wave 584/872/908/920/934: shell update via host-support authority.
        // Skip empty-dt authority shell tick dual-write.
        if dt <= 0.0 {
            let _ = budget;
            return Ok(());
        }
        self.drive_native_command_phase(dt, Some(budget), true, None)?;
        Ok(())
    }

    /// Wave 584: host logic-frame tick residual (timing/dt + optional headless budget).
    ///
    /// Each simulation update scopes its driving authority; the ordered
    /// residual callback runs after that scope restores the previous snapshot.
    /// Logic RNG ownership spans both update and delivery. The engine passes
    /// its actual GameLogic rather than borrowing the entire application.
    #[inline]
    pub(super) fn host_update_logic_frame(
        logic: &mut crate::game_logic::GameLogic,
        game_paused: bool,
        dt: f32,
        timing: Option<&FrameTiming>,
        budget: Option<usize>,
        before_step: impl FnMut(u32),
        after_step: impl FnMut(&mut crate::game_logic::GameLogic),
    ) -> crate::game_logic::SimTimingSnapshot {
        Self::host_update_logic_frame_with_script_effects(
            logic,
            game_paused,
            dt,
            timing,
            budget,
            Default::default(),
            before_step,
            after_step,
        )
    }

    pub(super) fn host_update_logic_frame_with_script_effects(
        logic: &mut crate::game_logic::GameLogic,
        game_paused: bool,
        dt: f32,
        timing: Option<&FrameTiming>,
        budget: Option<usize>,
        ui: crate::game_logic::ScriptPresentationEffects<'_>,
        before_step: impl FnMut(u32),
        after_step: impl FnMut(&mut crate::game_logic::GameLogic),
    ) -> crate::game_logic::SimTimingSnapshot {
        if game_paused {
            return crate::game_logic::SimTimingSnapshot::default();
        }
        // C++ GameEngine.cpp:749 uses the engine pause decision.
        if logic.is_paused() {
            logic.set_paused(false);
        }
        logic.tick_logic_frame_with_script_effects(dt, timing, budget, ui, before_step, after_step)
    }

    /// Wave 908: stamp sim timing residuals from a post-tick snapshot payload.
    #[inline]
    pub(super) fn host_stamp_sim_timing_from_snapshot(
        &mut self,
        snap: crate::game_logic::SimTimingSnapshot,
    ) {
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            // Freeze still preferred when installed (presentation owns residual clocks).
            self.host_match_visual_speed = Some(pres.visual_speed_multiplier);
            self.host_match_time_frozen = Some(pres.time_frozen_for_simulation || self.game_paused);
            self.host_match_total_play_time = Some(pres.total_play_time_seconds);
            self.host_match_logic_frame = Some(pres.frame.0);
            self.host_match_logic_steps = Some((
                pres.logic_steps_run,
                pres.logic_steps_budget_hit,
                pres.logic_steps_accumulated_seconds,
            ));
            return;
        }
        self.host_match_visual_speed = self.host_match_visual_speed.or(Some(1.0));
        self.host_match_time_frozen = Some(self.game_paused);
        self.host_match_total_play_time = self.host_match_total_play_time.or(Some(0.0));
        self.host_match_logic_frame = Some(snap.frame);
        self.host_match_logic_steps = Some((
            snap.steps_run as u32,
            snap.budget_hit,
            snap.accumulated_time_seconds,
        ));
    }

    /// Wave 909: runtime-host supplies floor residual.
    /// Skip authority write when presentation residual already meets floor.
    #[inline]
    pub(super) fn host_ensure_player_min_supplies_residual(&mut self, floor: u32) {
        // Wave 909/921/934: supplies residual via host-support authority.
        let pid = self.current_player_id;
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            if frame.local_player_id == pid && (frame.local_supplies as u32) >= floor {
                return;
            }
        } else if self.host_match_local_supplies.is_some_and(|s| s >= floor) {
            return;
        }
        self.host_game_logic_mut().apply_host_support_op(
            crate::game_logic::HostSupportOp::EnsurePlayerMinSupplies {
                player_id: pid,
                floor,
            },
        );
    }

    /// Wave 871: clear all host_match_* residuals (reset/load/start boundaries).
    #[inline]
    pub(super) fn host_clear_match_residuals(&mut self) {
        // A completed/reset/replaced world must not leave a modal legacy popup
        // WND or its typed acknowledgement queued over the next match.  The
        // helper clears only popup-specific bridge work, not arbitrary HUD
        // commands captured for another authority boundary.
        #[cfg(feature = "game_client")]
        self.host_invalidate_active_popup_for_world_boundary();
        #[cfg(feature = "game_client")]
        game_client::gui::campaign_launch_host_bridge::clear_host_campaign_launch_descriptor();

        // Carrier provenance cannot cross reset/load/new-match boundaries: an
        // object ID from an earlier world must never qualify a later deposit.
        self.physical_gather_carrier_ids.clear();
        self.popup_host_pause_owned = false;
        self.host_match_map_name = None;
        self.host_match_local_player_id = None;
        self.host_match_ai_difficulty = None;
        self.host_match_visual_speed = None;
        self.host_match_time_frozen = None;
        self.host_match_total_play_time = None;
        self.host_match_logic_frame = None;
        self.host_match_logic_steps = None;
        self.host_visual_logic_frame_applied = None;
        self.host_snow_logic_frame_applied = None;
        self.host_match_in_replay = None;
        self.host_match_in_shell = None;
        self.host_match_local_team = None;
        self.host_match_diplomacy_players = None;
        self.host_match_known_template_names = None;
        self.host_match_unlocked_sciences = None;
        self.game_logic.session_view.camera.camera_follow_factor = -1.0;
        self.host_match_local_barracks_ids = None;
        self.host_match_local_producer_ids = None;
        self.host_match_local_unfinished_producer_ids = None;
        self.host_match_local_team_sample_pos = None;
        self.host_match_over = None;
        self.host_match_victory_label = None;
        self.host_match_victory_winner = None;
        self.host_match_victory_summary = None;
        self.host_match_selected_ids = None;
        self.host_match_alive_object_ids = None;
        self.host_match_purchasable_sciences = None;
        self.host_match_local_science_purchase_points = None;
        self.host_match_local_supplies = None;
        self.host_match_special_power_ready_ids = None;
        self.host_match_boot_victory_condition = None;
        self.host_legal_build_cache_frame = None;
        self.host_legal_build_cache.clear();
        // Wave 871: also clear residuals added after Wave 844.
        self.host_match_first_opponent_id = None;
        self.host_match_game_mode = None;
        self.host_match_in_multiplayer = None;
        self.host_match_world_bounds = None;
    }

    /// Wave 870: keep host_match_* sim timing residuals warm after host ticks.
    #[inline]
    pub(super) fn host_stamp_sim_timing_residuals(&mut self) {
        // Wave 893/908/909: prefer presentation freeze; cold path keeps prior residual
        // or fail-closed zeros (ticks stamp via SimTimingSnapshot return — no live probe).
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_visual_speed = Some(pres.visual_speed_multiplier);
            self.host_match_time_frozen = Some(pres.time_frozen_for_simulation || self.game_paused);
            self.host_match_total_play_time = Some(pres.total_play_time_seconds);
            self.host_match_logic_frame = Some(pres.frame.0);
            self.host_match_logic_steps = Some((
                pres.logic_steps_run,
                pres.logic_steps_budget_hit,
                pres.logic_steps_accumulated_seconds,
            ));
            return;
        }
        // Wave 909: no sim_timing_snapshot dual-read on cold residual path.
        self.host_match_visual_speed = self.host_match_visual_speed.or(Some(1.0));
        self.host_match_time_frozen = Some(self.game_paused);
        self.host_match_total_play_time = self.host_match_total_play_time.or(Some(0.0));
        self.host_match_logic_frame = self.host_match_logic_frame.or(Some(0));
        self.host_match_logic_steps = self.host_match_logic_steps.or(Some((0, false, 0.0)));
    }

    /// Wave 584: host multiplayer gate residual (network frame-data readiness).
    #[inline]
    pub(super) fn host_is_in_multiplayer_game(&self) -> bool {
        // Wave 584/861: prefer host_match_in_multiplayer residual after stamp.
        if let Some(v) = self.host_match_in_multiplayer {
            return v;
        }
        // Wave 898: fail-closed boot default (single-player host).
        false
    }

    /// Wave 584: host special-power ready residual (boot-only UI fallback).
    #[inline]
    pub(super) fn host_is_special_power_ready_for(
        &self,
        id: crate::game_logic::ObjectId,
        power: &crate::command_system::SpecialPowerType,
    ) -> bool {
        // Wave 584/854: host-stamped ready-set peels obvious negatives before live probe.
        // If residual is warm and object is not in the ready set, fail closed without
        // dual-reading GameLogic. Positive residual still defers to live type/cooldown.
        if let Some(ready) = self.host_match_special_power_ready_ids.as_ref() {
            // Wave 898: residual owns ready set (no live type/cooldown dual-read).
            let _ = power;
            return ready.contains(&id.0);
        }
        // Wave 898: fail-closed boot default.
        let _ = (id, power);
        false
    }

    /// Wave 584: presentation freeze owns victory summary residual when installed.
    /// Boot residual without freeze uses host build_victory_summary.
    #[inline]
    pub(super) fn presentation_or_boot_victory_summary(
        &self,
        winner: Option<u32>,
    ) -> crate::game_logic::VictorySummary {
        // Prefer presentation-frozen summary when available (no live re-aggregate).
        // Wave 584/849/859: presentation freeze owns victory summary residual when installed.
        if let Some(summary) = self
            .last_presentation_frame
            .as_ref()
            .and_then(|f| f.victory_summary.clone())
        {
            return summary;
        }
        // Wave 859: warm host residual is fail-closed (even empty summary).
        if let Some(summary) = self.host_match_victory_summary.clone() {
            return summary;
        }
        // Boot residual only — no presentation summary yet. Boot default when
        // match outcome residual was never stamped.
        // Prefer host_match_over stamp path: if match not over and residual cold, empty.
        // Wave 895: fail-closed boot default (no dual-read). Match-over residual
        // and freeze already covered above; cold residual yields empty summary.
        let _ = winner;
        crate::game_logic::VictorySummary::default()
    }

    /// Wave 584: host match reset residual (GameLogic::reset boundary).
    #[inline]
    pub(super) fn host_reset_game_logic(
        &mut self,
    ) -> std::result::Result<(), NativeBoundaryFailure> {
        self.require_native_world_boundary("GameLogic reset")?;
        // Wave 584/871/933: host reset residual via session-control authority.
        self.host_game_logic_mut()
            .apply_session_control_op(crate::game_logic::SessionControlOp::Reset);
        if let Some(shadow) = self.gameworld_shadow.as_mut() {
            shadow.reset_for_world_boundary();
        }
        self.host_advance_direct_visual_world_epoch();
        #[cfg(feature = "game_client")]
        self.game_client.invalidate_presentation_drawable_world();
        self.render_pipeline.invalidate_world_visual_state();
        #[cfg(feature = "game_client")]
        self.render_pipeline.clear_native_terrain();
        self.invalidate_presentation_terrain_cache();
        self.host_clear_match_residuals();
        self.selected_objects.clear();
        self.last_presentation_frame = None;
        self.last_ui_state = None;
        Ok(())
    }

    /// Wave 584: host destroy-object residual (debug Shift+Delete path).
    #[inline]
    pub(super) fn host_destroy_object(&mut self, id: crate::game_logic::ObjectId) {
        // Wave 584/867/916/920/931: host destroy residual via object-lifecycle authority.
        // Skip authority destroy when presentation residual already marks destroyed.
        let already_destroyed = self.last_presentation_frame.as_ref().is_some_and(|pres| {
            pres.objects
                .iter()
                .any(|o| o.id == id && (o.destroyed || o.health_current <= 0.0))
        });
        if !already_destroyed {
            let _ = self.with_native_camera_input_effects(|world, effects, _| {
                world.apply_object_lifecycle_op_with_effects(
                    crate::game_logic::ObjectLifecycleOp::Destroy { id },
                    effects,
                )
            });
            if self.last_presentation_frame.is_none() {
                self.host_refresh_local_train_producer_residuals();
            }
        }
        // Drop destroyed id from selection residuals if present.
        if let Some(sel) = self.host_match_selected_ids.as_mut() {
            sel.retain(|x| *x != id);
        }
        self.selected_objects.retain(|x| *x != id);
    }

    /// Wave 584: host science purchase capability residual (boot-only).
    #[inline]
    pub(super) fn host_player_can_purchase_science(&self, player_id: u32, name: &str) -> bool {
        // Wave 584/852/861: warm purchasable residual is fail-closed (no live dual-read).
        if let Some(map) = self.host_match_purchasable_sciences.as_ref() {
            return map
                .get(&player_id)
                .map(|set| set.iter().any(|s| s.eq_ignore_ascii_case(name)))
                .unwrap_or(false);
        }
        // Wave 897: fail-closed boot default (no dual-read).
        let _ = (player_id, name);
        false
    }

    /// Wave 584: host clear unit path residual (waypoint clear path).
    #[inline]
    pub(super) fn host_clear_unit_movement_path(
        &mut self,
        id: crate::game_logic::ObjectId,
    ) -> bool {
        // Wave 584/869/918/920/931: clear path via object-lifecycle authority.
        // Skip authority clear when presentation residual already has no move destination.
        if self.last_presentation_frame.as_ref().is_some_and(|pres| {
            pres.objects
                .iter()
                .any(|o| o.id == id && o.move_destination.is_none())
        }) {
            return true;
        }
        let ok = matches!(
            self.with_native_camera_input_effects(|world, effects, _| world
                .apply_object_lifecycle_op_with_effects(
                    crate::game_logic::ObjectLifecycleOp::ClearMovementPath { id },
                    effects
                )),
            crate::game_logic::ObjectLifecycleResult::Bool(true)
        );
        if ok && self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        ok
    }

    /// Wave 584: host guard-radius adjust residual.
    #[inline]
    pub(super) fn host_adjust_unit_guard_radius(
        &mut self,
        id: crate::game_logic::ObjectId,
        delta: f32,
    ) -> Option<f32> {
        // Wave 584/869/919/920/931: guard radius via object-lifecycle authority.
        // Skip authority adjust when delta is a no-op; return presentation residual.
        if delta.abs() <= f32::EPSILON {
            return self.last_presentation_frame.as_ref().and_then(|pres| {
                pres.objects
                    .iter()
                    .find(|o| o.id == id)
                    .map(|o| o.guard_radius)
            });
        }
        let r = match self.with_native_camera_input_effects(|world, effects, _| {
            world.apply_object_lifecycle_op_with_effects(
                crate::game_logic::ObjectLifecycleOp::AdjustGuardRadius { id, delta },
                effects,
            )
        }) {
            crate::game_logic::ObjectLifecycleResult::Radius(r) => r,
            _ => None,
        };
        if r.is_some() && self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        r
    }

    /// Wave 583: host force-complete construction residual (runtime train honesty).
    #[inline]
    pub(super) fn host_force_complete_construction(
        &mut self,
        id: crate::game_logic::ObjectId,
    ) -> bool {
        // Wave 583/867/917/920/931: force-complete via object-lifecycle authority.
        // Skip authority force-complete when presentation residual is already complete.
        if self.last_presentation_frame.as_ref().is_some_and(|pres| {
            pres.objects
                .iter()
                .any(|o| o.id == id && !o.destroyed && !o.under_construction)
        }) {
            return true;
        }
        let ok = matches!(
            self.with_native_camera_input_effects(|world, effects, _| world
                .apply_object_lifecycle_op_with_effects(
                    crate::game_logic::ObjectLifecycleOp::ForceCompleteConstruction { id },
                    effects
                )),
            crate::game_logic::ObjectLifecycleResult::Bool(true)
        );
        if ok && self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        ok
    }

    /// Wave 583/723: host barracks building-data residual (opt-in producer pick path).
    #[inline]
    pub(super) fn host_ensure_barracks_building_data(
        &mut self,
        id: crate::game_logic::ObjectId,
    ) -> bool {
        // Wave 583/723/872/917/920/934: barracks ensure via host-support authority.
        // Skip authority ensure when residual already lists this barracks producer.
        if self
            .host_match_local_barracks_ids
            .as_ref()
            .is_some_and(|ids| ids.contains(&id))
        {
            return true;
        }
        let ok = matches!(
            self.host_game_logic_mut().apply_host_support_op(
                crate::game_logic::HostSupportOp::EnsureBarracksBuildingData { id },
            ),
            crate::game_logic::HostSupportResult::Bool(true)
        );
        if ok && self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        ok
    }

    #[inline]
    pub(super) fn host_force_ensure_barracks_building_data(
        &mut self,
        id: crate::game_logic::ObjectId,
    ) -> bool {
        // Wave 834/872/917/920/934: force barracks ensure via host-support authority.
        // Skip force ensure when residual already lists this barracks producer.
        if self
            .host_match_local_barracks_ids
            .as_ref()
            .is_some_and(|ids| ids.contains(&id))
        {
            return true;
        }
        let ok = matches!(
            self.host_game_logic_mut().apply_host_support_op(
                crate::game_logic::HostSupportOp::ForceEnsureBarracksBuildingData { id },
            ),
            crate::game_logic::HostSupportResult::Bool(true)
        );
        if ok && self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        ok
    }

    /// Wave 929: single direct-order authority boundary + residual stamp.
    #[inline]
    pub(super) fn host_issue_direct_player_order(
        &mut self,
        order: crate::game_logic::DirectPlayerOrder,
    ) {
        // Wave 929/930: single GameLogic direct-order authority boundary + stamp.
        self.with_native_camera_input_effects(|world, effects, _| {
            world.apply_direct_player_order_with_effects(order, effects)
        });
        self.host_stamp_after_authority_command();
    }

    pub(super) fn host_stamp_after_authority_command(&mut self) {
        // Wave 917/927: skip mid-command stamp when presentation freeze owns clocks.
        if self.last_presentation_frame.is_none() {
            self.host_stamp_sim_timing_residuals();
        }
    }

    pub(super) fn host_command_attack(
        &mut self,
        player_id: u32,
        target: crate::game_logic::ObjectId,
    ) {
        // Wave 583/871/917/927/929: host attack residual via direct-order boundary.
        self.host_issue_direct_player_order(crate::game_logic::DirectPlayerOrder::Attack {
            player_id,
            target,
        });
    }

    /// Wave 583: host stop-selected residual (runtime honesty path).
    #[inline]
    pub(super) fn host_command_stop(&mut self, player_id: u32) {
        // Wave 583/871/917/927/929: host stop residual via direct-order boundary.
        self.host_issue_direct_player_order(crate::game_logic::DirectPlayerOrder::Stop {
            player_id,
        });
    }

    /// Wave 583: host attack-move residual (minimap/right-click fallback).
    #[inline]
    pub(super) fn host_command_attack_move(&mut self, player_id: u32, dest: glam::Vec3) {
        // Wave 583/871/917/927/929: host attack-move residual via direct-order boundary.
        self.host_issue_direct_player_order(crate::game_logic::DirectPlayerOrder::AttackMove {
            player_id,
            dest,
        });
    }

    /// Wave 583: host move residual (minimap/right-click fallback).
    #[inline]
    pub(super) fn host_command_move(&mut self, player_id: u32, dest: glam::Vec3) {
        // Wave 583/871/917/927/929: host move residual via direct-order boundary.
        self.host_issue_direct_player_order(crate::game_logic::DirectPlayerOrder::Move {
            player_id,
            dest,
        });
    }

    /// Wave 583: host legal-build probe residual (construct honesty path).
    #[inline]
    pub(super) fn host_legal_build_code_at_for_builder(
        &mut self,
        team: crate::game_logic::Team,
        loc: glam::Vec3,
        template: &str,
        builder: Option<crate::game_logic::ObjectId>,
    ) -> u32 {
        // Wave 583/911/924/929: host legal build-code residual with per-frame cache.
        // Placement cursor/UI + pad scans share this residual (no live dual-read on hit).
        let frame = self
            .host_match_logic_frame
            .or_else(|| self.last_presentation_frame.as_ref().map(|p| p.frame.0))
            .unwrap_or(0);
        if self.host_legal_build_cache_frame != Some(frame) {
            self.host_legal_build_cache_frame = Some(frame);
            self.host_legal_build_cache.clear();
        }
        let qx = (loc.x * 4.0).round() as i32;
        let qz = (loc.z * 4.0).round() as i32;
        let th = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            template.hash(&mut h);
            h.finish()
        };
        let bid = builder.map(|b| b.0).unwrap_or(0);
        let key = (team, qx, qz, th, bid);
        if let Some(code) = self.host_legal_build_cache.get(&key).copied() {
            return code;
        }
        let code = self.with_native_camera_input_effects(|logic, effects, _| {
            logic
                .legal_build_code_at_for_builder_with_effects(team, loc, template, builder, effects)
        });
        self.host_legal_build_cache.insert(key, code);
        code
    }

    /// Preview ghost: IGNORE_STEALTHED so unseen stealthed units do not redden.
    #[inline]
    pub(super) fn host_legal_build_code_at_for_preview(
        &mut self,
        team: crate::game_logic::Team,
        loc: glam::Vec3,
        template: &str,
        builder: Option<crate::game_logic::ObjectId>,
    ) -> u32 {
        let frame = self
            .host_match_logic_frame
            .or_else(|| self.last_presentation_frame.as_ref().map(|p| p.frame.0))
            .unwrap_or(0);
        if self.host_legal_build_cache_frame != Some(frame) {
            self.host_legal_build_cache_frame = Some(frame);
            self.host_legal_build_cache.clear();
        }
        let qx = (loc.x * 4.0).round() as i32;
        let qz = (loc.z * 4.0).round() as i32;
        let th = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            template.hash(&mut h);
            1u64.hash(&mut h); // preview vs confirm
            h.finish()
        };
        let bid = builder.map(|b| b.0).unwrap_or(0);
        let key = (team, qx, qz, th, bid);
        if let Some(code) = self.host_legal_build_cache.get(&key).copied() {
            return code;
        }
        #[cfg(feature = "game_client")]
        let terrain = self
            .render_pipeline
            .native_terrain_mut()
            .map(|terrain| terrain as &mut dyn crate::game_logic::NativeTerrainEffects);
        #[cfg(not(feature = "game_client"))]
        let terrain = None;
        let code = self
            .game_logic
            .legal_build_code_at_for_preview_with_effects(
                team,
                loc,
                template,
                builder,
                &mut crate::game_logic::ScriptPresentationEffects {
                    audio: None,
                    ui: None,
                    view: None,
                    terrain,
                },
            );
        self.host_legal_build_cache.insert(key, code);
        code
    }

    /// Wave 583: host legal-build location residual (construct honesty path).
    #[inline]
    pub(super) fn host_is_location_legal_to_build_for_builder(
        &mut self,
        team: crate::game_logic::Team,
        loc: glam::Vec3,
        template: &str,
        builder: Option<crate::game_logic::ObjectId>,
    ) -> bool {
        // Wave 583/910/911: route through cached legal-build residual.
        self.host_legal_build_code_at_for_builder(team, loc, template, builder)
            == crate::game_logic::host_production_buildable_command_residual::LBC_OK
    }

    /// Wave 583: host camera-follow write residual.
    #[inline]
    pub(super) fn host_set_camera_follow_object(
        &mut self,
        id: Option<crate::game_logic::ObjectId>,
    ) {
        // View::setCameraLock is an operation, even when the ID is equal:
        // authored tether may have changed the canonical lock style meanwhile.
        self.host_game_logic_mut()
            .apply_session_control_op(crate::game_logic::SessionControlOp::SetCameraFollow { id });
    }

    /// Input must observe the receiving live lock, including an action since
    /// the last frozen frame. A presentation position is not lock ownership.
    #[inline]
    pub(super) fn presentation_or_boot_camera_follow_active(&self) -> bool {
        self.game_logic.camera_follow_object_id().is_some()
    }

    /// Wave 583: boot residual EVA counter bundle (no presentation freeze).
    #[inline]
    pub(super) fn boot_eva_counter_bundle_from_host(&self) -> (u32, u32, u32, u32) {
        // Wave 583/898: prefer presentation EVA residual when freeze installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return (
                pres.eva_low_power_count,
                pres.eva_insufficient_funds_count,
                pres.eva_base_under_attack_count,
                pres.eva_ally_under_attack_count,
            );
        }
        // Wave 898: fail-closed boot default (no dual-read).
        (0, 0, 0, 0)
    }

    /// Wave 582: host production enqueue residual (train honesty path boundary).
    #[inline]
    pub(super) fn host_enqueue_production(
        &mut self,
        producer: crate::game_logic::ObjectId,
        template_name: String,
    ) -> bool {
        // Wave 582/868/919/931: enqueue residual via object-lifecycle authority.
        // Skip producer residual refresh under presentation freeze (freeze owns scan).
        let ok = matches!(
            self.with_native_camera_input_effects(|world, effects, _| world
                .apply_object_lifecycle_op_with_effects(
                    crate::game_logic::ObjectLifecycleOp::EnqueueProduction {
                        producer,
                        template_name,
                    },
                    effects
                )),
            crate::game_logic::ObjectLifecycleResult::Bool(true)
        );
        if ok && self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        ok
    }

    /// Wave 582: shell/menu frame process_commands residual (no Command SFX).
    /// Distinct from InGame `host_process_commands_with_command_sound`.
    #[inline]
    pub(super) fn host_process_shell_menu_commands(&mut self) {
        // Wave 582/871/914/918/932: shell/menu command drain via command-pipeline authority.
        // Empty queue skips process dual-write; stamp only when work ran without freeze.
        let processed = self
            .host_apply_command_pipeline_op(crate::game_logic::CommandPipelineOp::ProcessIfNeeded);
        if processed && self.last_presentation_frame.is_none() {
            self.host_stamp_sim_timing_residuals();
        }
    }

    /// Wave 581: host create_object residual (thin authority spawn boundary).
    #[inline]
    pub(super) fn host_create_object(
        &mut self,
        name: &str,
        team: crate::game_logic::Team,
        spawn_at: glam::Vec3,
    ) -> Option<crate::game_logic::ObjectId> {
        // Wave 581/867/919/931: host spawn residual via object-lifecycle authority.
        // Under presentation freeze, next finalize refreshes alive residuals.
        let res = self.with_native_camera_input_effects(|world, effects, _| {
            world.apply_object_lifecycle_op_with_effects(
                crate::game_logic::ObjectLifecycleOp::Create {
                    name: name.to_string(),
                    team,
                    spawn_at,
                },
                effects,
            )
        });
        let id = match res {
            crate::game_logic::ObjectLifecycleResult::Created(id) => id,
            _ => None,
        };
        if id.is_some() && self.last_presentation_frame.is_none() {
            self.host_refresh_local_train_producer_residuals();
        }
        id
    }

    /// Wave 555: presentation freeze owns local team residual when installed.
    /// Boot residual without freeze uses host player_team probe.
    #[inline]
    pub(super) fn presentation_or_boot_local_team(&self) -> crate::game_logic::Team {
        // Wave 555/845: presentation freeze owns local team residual when installed.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            return frame.local_team();
        }
        if let Some(team) = self.host_match_local_team {
            return team;
        }
        // Wave 898: fail-closed boot default.
        crate::game_logic::Team::USA
    }

    pub(super) fn presentation_or_boot_unlocked_sciences(&self, player_id: u32) -> Vec<String> {
        // Wave 555/846/859/894: multi-player residual map first (stamped from freeze).
        // Fail-closed empty on miss — no dual-read while residual/freeze is warm.
        if let Some(map) = self.host_match_unlocked_sciences.as_ref() {
            return map.get(&player_id).cloned().unwrap_or_default();
        }
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            if player_id == frame.local_player_id {
                return frame.local_unlocked_sciences.clone();
            }
            // Non-local while freeze live: fail-closed (no dual-read).
            return Vec::new();
        }
        // Wave 895: fail-closed boot default (no dual-read).
        let _ = player_id;
        Vec::new()
    }

    /// Wave 556: presentation freeze owns match-over / victory-label residual when
    /// installed (no live re-evaluate dual-read). Boot residual without freeze uses
    /// host `evaluate_victory_condition`.
    #[inline]

    /// Wave 855: single boot evaluate of victory condition for residual peels.
    /// Presentation freeze / host_match_over already own InGame outcomes; this only
    /// covers freeze-miss boot probes so match_over and winner share one evaluate.
    #[inline]
    pub(super) fn host_boot_victory_condition_residual(
        &mut self,
    ) -> Option<crate::game_logic::VictoryCondition> {
        if let Some(cached) = self.host_match_boot_victory_condition.as_ref() {
            return *cached;
        }
        // Wave 907/910: presentation freeze owns victory residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            let v = if !pres.match_over {
                None
            } else if let Some(id) = pres.victory_winner_id() {
                Some(crate::game_logic::VictoryCondition::Winner(id))
            } else {
                Some(crate::game_logic::VictoryCondition::Draw)
            };
            self.host_match_boot_victory_condition = Some(v);
            return v;
        }
        // Wave 910: cold boot residual fail-closed (no evaluate_victory dual-read).
        // Match-over/winner stamp only from presentation freeze or prior cache.
        self.host_match_boot_victory_condition = Some(None);
        None
    }

    pub(super) fn presentation_or_boot_match_over_label(&mut self) -> (bool, String) {
        // Prefer presentation victory residual when frame installed (no live
        // re-evaluate dual-read).
        // Wave 556/849: presentation freeze owns match-over / victory-label residual.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return (
                pres.match_over,
                pres.victory_label.clone().unwrap_or_default(),
            );
        }
        if let Some(over) = self.host_match_over {
            return (
                over,
                self.host_match_victory_label.clone().unwrap_or_default(),
            );
        }
        // Boot residual only — no presentation frame yet (single stamped evaluate).
        // Wave 855: boot residual via single stamped evaluate.
        if let Some(v) = self.host_boot_victory_condition_residual() {
            (true, format!("{v:?}"))
        } else {
            (false, String::new())
        }
    }

    /// Wave 556: InGame victory screen residual — presentation freeze owns match_over
    /// and winner id; boot residual evaluates host victory condition.
    /// Returns `Some(winner)` when a victory screen should show (`None` winner = draw).
    #[inline]
    pub(super) fn presentation_or_boot_victory_winner(&mut self) -> Option<Option<u32>> {
        // Wave 556/849: presentation freeze owns victory winner residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            if !pres.match_over {
                return None;
            }
            return Some(pres.victory_winner_id());
        }
        if let Some(over) = self.host_match_over {
            if !over {
                return None;
            }
            // Stamped winner residual: None draw / Some(id) winner.
            return Some(self.host_match_victory_winner.unwrap_or(None));
        }
        // Wave 855: boot residual via single stamped evaluate (shared with match_over).
        match self.host_boot_victory_condition_residual() {
            Some(crate::game_logic::VictoryCondition::Winner(id)) => Some(Some(id)),
            Some(crate::game_logic::VictoryCondition::Draw) => Some(None),
            None => None,
        }
    }

    /// Wave 552: Menu residual — only trust freeze when it *affirms* shell-map
    /// mode. Stale InGame frames (`fow_shell_bypass=false`) fall through to live
    /// `isInShellGame` so shell ticks are not suppressed after a match.
    #[inline]
    pub(super) fn presentation_affirms_shell_or_boot(&self) -> bool {
        // Wave 552: menu residual — freeze must affirm shell, else boot probe.
        match self.last_presentation_frame.as_ref() {
            Some(pres) if pres.fow_shell_bypass => true,
            _ => self.host_is_in_shell_game(),
        }
    }

    /// Wave 552: shell-bypass from an optional presentation frame (pipeline or
    /// last). Missing frame → boot `isInShellGame`.
    #[inline]
    pub(super) fn shell_bypass_from_presentation(
        &self,
        frame: Option<&crate::presentation_frame::PresentationFrame>,
    ) -> bool {
        // Wave 552: optional freeze owns shell-bypass; boot residual otherwise.
        match frame {
            Some(pres) => pres.fow_shell_bypass,
            None => self.host_is_in_shell_game(),
        }
    }

    pub(super) fn map_ai_difficulty_to_save(difficulty: crate::ai::AIDifficulty) -> GameDifficulty {
        match difficulty {
            crate::ai::AIDifficulty::Easy => GameDifficulty::Easy,
            crate::ai::AIDifficulty::Medium => GameDifficulty::Medium,
            crate::ai::AIDifficulty::Hard | crate::ai::AIDifficulty::Brutal => GameDifficulty::Hard,
        }
    }

    pub(super) fn build_save_info(
        &self,
        slot: &str,
        display_name: &str,
        description: &str,
        save_type: SaveFileType,
    ) -> SaveGameInfo {
        // Wave 545/554: presentation freeze owns save metadata residual when installed
        // (even if map_name empty — no host dual-read mid-frame). Boot residual only
        // without a freeze (via presentation_or_boot_* helpers).
        let map_name = self.presentation_or_boot_map_name();
        let difficulty = Self::map_ai_difficulty_to_save(self.presentation_or_boot_ai_difficulty());
        let play_time =
            std::time::Duration::from_secs_f32(self.presentation_or_boot_total_play_time());
        let (campaign_side, mission_number) =
            crate::save_load::campaign_header_from_campaign_manager();

        SaveGameInfo {
            pristine_map_name: self.host_game_logic().map_definition_source.clone(),
            filename: slot.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            map_name,
            campaign_side,
            mission_number,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time,
            difficulty,
            save_type,
        }
    }

    /// Wave 611: via `host_quick_save_from_hotkey`.
    pub(super) fn quick_save_from_hotkey(&mut self, source: &str) {
        // Wave 611: thin wrapper — residual via host helper.
        self.host_quick_save_from_hotkey(source)
    }

    /// Wave 935/936: intentional immutable GameLogic borrow boundary.
    /// Wave 936: sole-authority surface honesty lock (apply_* + split-borrow adapters only).
    #[inline]
    pub(super) fn host_game_logic(&self) -> &crate::game_logic::GameLogic {
        &self.game_logic
    }

    /// Wave 935: intentional mutable GameLogic borrow boundary.
    #[inline]
    pub(super) fn host_game_logic_mut(&mut self) -> &mut crate::game_logic::GameLogic {
        &mut self.game_logic
    }

    /// Admission is the sole native map-definition handoff. Candidates never
    /// reroll or reset the receiving Client's physical snow during parsing.
    fn host_admit_snow_for_world(&mut self) {
        #[cfg(feature = "game_client")]
        {
            self.game_client.admit_base_snow_definition();
            for definition in &self.game_logic.weather_definition_updates {
                self.game_client.admit_snow_definition(
                    &game_client::snow::WeatherSetting::from_common(definition),
                );
            }
            self.game_client
                .snow_mut()
                .set_visible(self.game_logic.weather_state().visible);
        }
    }

    /// Startup's worker owns admitted rules and script/filter results; the app
    /// retains its actual view pose/HAG owner until subsequent map bootstrap.
    fn admit_startup_candidate_view(
        previous: &mut crate::presentation_frame::SessionView,
        candidate: &mut crate::presentation_frame::SessionView,
    ) {
        let (candidate_visibility, candidate_visibility_delivered) =
            candidate.take_letterbox_visibility_output();
        let candidate_focus_previous = candidate.pending_focus_previous_target;
        let candidate_camera_settings = candidate.camera.settings;
        let candidate_camera_clip = candidate.camera.clip_policy;
        let candidate_camera_shake = candidate.camera.shake_policy;
        let candidate_camera_movement = candidate.camera.movement_policy;
        let candidate_camera_input = candidate.camera.input_policy;
        let candidate_scripted = std::mem::take(&mut candidate.scripted);
        // Map actions already applied to the candidate's receiving filter.
        #[cfg(feature = "game_client")]
        let candidate_filter = std::mem::take(&mut candidate.filter);
        *candidate = std::mem::take(previous);
        candidate.pending_letterbox_visibility = candidate_visibility;
        candidate.letterbox_visibility_delivered_since_present = candidate_visibility_delivered;
        candidate.pending_focus_previous_target = candidate_focus_previous;
        candidate.camera.settings = candidate_camera_settings;
        candidate.camera.clip_policy = candidate_camera_clip;
        candidate.camera.shake_policy = candidate_camera_shake;
        candidate.camera.movement_policy = candidate_camera_movement;
        candidate.camera.input_policy = candidate_camera_input;
        candidate.scripted = candidate_scripted;
        #[cfg(feature = "game_client")]
        {
            candidate.filter = candidate_filter;
        }
    }

    /// Wave 935: intentional full GameLogic replace boundary (map-load install).
    #[inline]
    pub(super) fn host_replace_game_logic(
        &mut self,
        mut logic: crate::game_logic::GameLogic,
        mut native_map: super::map_native_admission::NativeMapCandidate,
    ) -> Result<(), String> {
        if let Err(reason) = self.rejected_native_worlds.validate_admission() {
            self.rejected_native_worlds
                .retain(Some(Box::new(logic)), native_map, reason.clone());
            return Err(reason);
        }
        #[cfg(feature = "game_client")]
        let epoch = match native_map
            .epoch()
            .filter(|epoch| *epoch == self.next_native_world_epoch())
        {
            Some(epoch) => epoch,
            None => {
                let cleanup = self.reject_receiving_world_candidate(logic, native_map);
                return Err(format!(
                    "accepted native map epoch missing/stale; cleanup={cleanup:?}"
                ));
            }
        };
        if let Err(error) = self.with_native_camera_input_effects(|world, effects, _| {
            world.destroy_all_objects_immediate_with_effects_checked(effects)
        }) {
            #[cfg(feature = "game_client")]
            {
                let cleanup = self.reject_receiving_world_candidate(logic, native_map);
                return Err(format!(
                    "outgoing Object retirement failed: {error}; candidate cleanup={cleanup:?}"
                ));
            }
            #[cfg(not(feature = "game_client"))]
            {
                let cleanup = self.reject_receiving_world_candidate(logic, native_map);
                return Err(format!(
                    "outgoing Object retirement failed: {error}; candidate cleanup={cleanup:?}"
                ));
            }
        }
        // Retire old Client while old GameLogic and visual handle still exist.
        #[cfg(feature = "game_client")]
        let pending_map_audio = match native_map
            .install_owned_worlds(&mut self.game_client, &mut self.render_pipeline)
        {
            Ok(audio) => audio,
            Err((error, mut rejected)) => {
                let cleanup = self.reject_receiving_world_candidate(logic, rejected);
                return Err(match cleanup {
                    Ok(()) => error,
                    Err(cleanup) => format!("{error}; rejected candidate cleanup: {cleanup}"),
                });
            }
        };
        #[cfg(not(feature = "game_client"))]
        let epoch = self.next_native_world_epoch();
        #[cfg(not(feature = "game_client"))]
        let pending_map_audio = {
            self.render_pipeline.invalidate_world_visual_state();
            native_map.install_visuals(&mut self.render_pipeline)
        };
        if logic
            .session_view
            .camera
            .input_policy
            .map_parsed_game_data()
        {
            self.ui_manager.reload_options_preferences();
        }
        // Do this before replacing GameLogic so the old world's active token,
        // frozen residual, modal WND, and popup-owned pause all disappear as
        // one boundary. Process-lifetime popup tokens then reject any late
        // callback which arrives after the new world is installed.
        #[cfg(feature = "game_client")]
        self.host_invalidate_active_popup_for_world_boundary();

        self.pending_match_start = None;
        self.destroy_source_new_game_background();

        // The receiving application keeps its persistent view until map admission
        // resets it. A detached candidate never publishes or replaces this state.
        Self::admit_startup_candidate_view(
            &mut self.game_logic.session_view,
            &mut logic.session_view,
        );
        self.game_logic = logic;
        if let Some(local) = self.game_logic.local_player_id() {
            self.current_player_id = local;
            self.host_match_local_player_id = Some(local);
        }
        self.ui_manager
            .admit_camera_input_defaults(self.game_logic.camera_input_rules());
        self.host_admit_snow_for_world();
        if let Some(shadow) = self.gameworld_shadow.as_mut() {
            shadow.reset_for_world_boundary();
            // The replacement is already a complete host world.  Seed the
            // fresh GameWorld before any boot/menu probe can observe it; the
            // staged-restore and successful map-load boundaries use the same
            // reset-then-sync ordering.
            shadow.sync_from_host(&self.game_logic);
        }
        self.commit_native_world_epoch(epoch);
        #[cfg(feature = "game_client")]
        self.game_client
            .bind_visual_world(gamelogic::helpers::ClientVisualHandle::new(Arc::clone(
                &self.game_logic.world_services,
            )));

        self.invalidate_presentation_terrain_cache();
        self.draw_module_name_cache.borrow_mut().clear();
        self.kind_name_cache.borrow_mut().clear();
        #[cfg(feature = "game_client")]
        if matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        ) {
            self.set_owned_control_bar_scheme("Observer");
        }
        pending_map_audio.admit_to_live();
        Ok(())
    }

    /// Preserve the candidate's saved continuation after presentation commit
    /// work. The selected seed was restored from snapshot.random_seed; this
    /// only restores its capsule and never changes another match or fallback.
    /// The omitted pre-v22 capsule remains a no-op here.
    fn host_restore_staged_logic_rng(
        logic: &mut crate::game_logic::GameLogic,
        seed_words: [u32; 6],
    ) {
        if seed_words != [0; 6] {
            logic.logic_random.set_seed_words(seed_words);
        }
    }

    /// Install a fully staged save world and its GameClient companion.
    ///
    /// Unlike `host_replace_game_logic`, this also owns the legacy singleton
    /// bundle mutated by map loading.  The order is intentional: invalidate
    /// old UI/log ownership, install candidate globals, install matching host
    /// logic, then finish the preflight-checked callback-free native commit
    /// before rebuilding the shadow from host authority.
    fn host_replace_staged_restore_world(
        &mut self,
        staged: StagedRestoreWorld,
    ) -> Result<(), String> {
        // Reject unexplained Core callbacks before invalidating the old UI,
        // replacing its world, or admitting the candidate camera continuation.
        if let Err(reason) = self.rejected_native_worlds.validate_admission() {
            self.rejected_native_worlds.retain(
                Some(Box::new(staged.logic)),
                staged.native_map,
                reason.clone(),
            );
            return Err(reason);
        }
        let StagedRestoreWorld {
            mut native_map,
            mut logic,
            info: _,
            runtime_world,
            shroud: _,
            client_drawables,
            game_client_xfer_bytes,
            terrain_visual,
            map_water_configuration,
            client_persist,
            camera_continuation,
            owns_session_camera,
            mut control_groups,
            logic_rng_seed_words,
        } = staged;
        let runtime_world = match runtime_world.preflight_native() {
            Ok(world) => world,
            Err(error) => {
                #[cfg(feature = "game_client")]
                {
                    let cleanup = self.reject_receiving_world_candidate(logic, native_map);
                    return Err(format!(
                        "{error}; rejecting saved native preflight: {cleanup:?}"
                    ));
                }
                #[cfg(not(feature = "game_client"))]
                {
                    let cleanup = self.reject_receiving_world_candidate(logic, native_map);
                    return Err(format!(
                        "{error}; rejecting saved logical preflight: {cleanup:?}"
                    ));
                }
            }
        };
        #[cfg(feature = "game_client")]
        let epoch = match native_map
            .epoch()
            .filter(|epoch| *epoch == self.next_native_world_epoch())
        {
            Some(epoch) => epoch,
            None => {
                let cleanup = self.reject_receiving_world_candidate(logic, native_map);
                return Err(format!(
                    "saved native candidate epoch missing/stale; cleanup={cleanup:?}"
                ));
            }
        };
        #[cfg(not(feature = "game_client"))]
        let epoch = self.next_native_world_epoch();
        if epoch != self.next_native_world_epoch() {
            return Err("saved native candidate epoch is stale".into());
        }
        // Immutable application policy transfers; the old live reader never does.
        // This prepares only the detached candidate and cannot reset the receiver.
        let mut recorder_inputs = self
            .game_logic
            .replay_pending
            .recorder_owned()
            .owner_inputs()
            .expect("receiving native Recorder inputs")
            .clone();
        recorder_inputs.map = logic.get_current_map_name().to_owned();
        recorder_inputs.seed = logic.logic_base_seed;
        logic.replay_pending.reset_recorder_owned(recorder_inputs);
        // Failed staging/preflight leaves the current Engine untouched. An
        // admitted candidate starts a new game, then its saved Native state
        // (when present) restores the replay flag below, in source order.
        self.reset_replay_pacing_for_new_game();
        #[cfg(feature = "game_client")]
        self.host_invalidate_active_popup_for_world_boundary();

        if let Err(error) = self.with_native_camera_input_effects(|world, effects, _| {
            world.destroy_all_objects_immediate_with_effects_checked(effects)
        }) {
            #[cfg(feature = "game_client")]
            {
                let cleanup = self.reject_receiving_world_candidate(logic, native_map);
                return Err(format!(
                    "outgoing saved-world retirement failed: {error}; candidate cleanup={cleanup:?}"
                ));
            }
            #[cfg(not(feature = "game_client"))]
            {
                let cleanup = self.reject_receiving_world_candidate(logic, native_map);
                return Err(format!(
                    "outgoing saved-world retirement failed: {error}; candidate cleanup={cleanup:?}"
                ));
            }
        }
        // Destroy the previous client's Drawables while its own world and the
        // C++-style ambient module callbacks still refer to that old world.
        // Some DrawModule destructors consult TheGameClient during teardown.
        #[cfg(feature = "game_client")]
        let pending_map_audio = match native_map
            .install_owned_worlds(&mut self.game_client, &mut self.render_pipeline)
        {
            Ok(audio) => audio,
            Err((error, mut rejected)) => {
                let cleanup = self.reject_receiving_world_candidate(logic, rejected);
                return Err(match cleanup {
                    Ok(()) => error,
                    Err(cleanup) => format!("{error}; rejected candidate cleanup: {cleanup}"),
                });
            }
        };
        #[cfg(not(feature = "game_client"))]
        let pending_map_audio = native_map.install_visuals(&mut self.render_pipeline);

        let native_commit = runtime_world.install_globals();
        // Only admitted replacement cancels a source prepared start. Failed
        // staging/preflight above retains its live pending owner and windows.
        self.pending_match_start = None;
        self.destroy_source_new_game_background();
        admit_staged_session_view(
            &mut self.game_logic.session_view,
            &mut logic.session_view,
            owns_session_camera,
        );
        let saved_local = logic
            .initial_skirmish_admission
            .as_ref()
            .and_then(|initial| initial.local_slot);
        let old_logic = std::mem::replace(&mut self.game_logic, logic);
        self.commit_native_world_epoch(epoch);
        if let Some(local) = saved_local {
            self.current_player_id = local;
            self.host_match_local_player_id = Some(local);
        }
        self.host_admit_snow_for_world();
        #[cfg(feature = "game_client")]
        self.game_client
            .bind_visual_world(gamelogic::helpers::ClientVisualHandle::new(Arc::clone(
                &self.game_logic.world_services,
            )));
        #[cfg(feature = "game_client")]
        if let Some(bytes) = game_client_xfer_bytes.as_deref() {
            crate::save_load::snapshot::restore_game_client_from_xfer_bytes(
                &mut self.game_client,
                bytes,
            )
            .map_err(|err| {
                format!("failed to restore CHUNK_GameClient after world commit: {err}")
            })?;
        }
        #[cfg(feature = "game_client")]
        crate::save_load::snapshot::persist_v18::apply_drawable_xfer_to_client(
            &client_persist,
            &self.game_logic,
            &mut self.game_client,
        );
        // Restore GameClient/View before exposing the candidate world. No
        // Core callback is replayed: native map admission already executed its
        // owned actions before saved continuation replaced runtime state.
        let has_camera_continuation = camera_continuation.is_some() || owns_session_camera;
        let owned_camera = owns_session_camera.then(|| {
            (
                crate::presentation_frame::camera::continuation::SessionCameraState::capture(
                    &self.game_logic.session_view.camera,
                ),
                self.game_logic.session_view.camera.rotation,
                self.game_logic
                    .session_view
                    .camera
                    .rotation_registers
                    .capture(),
            )
        });
        if let Some(camera) = camera_continuation {
            self.admit_camera_continuation(camera);
            if let Some((owned, timing, registers)) = owned_camera {
                owned.admit(&mut self.game_logic.session_view.camera);
                self.game_logic.session_view.camera.rotation = timing;
                self.game_logic.session_view.camera.rotation_registers =
                    crate::presentation_frame::camera::registers::RotationRegisters::restore(
                        registers,
                    );
            }
        } else if !owns_session_camera && let Some(camera) = client_persist.camera() {
            self.game_logic.session_view.target = glam::Vec3::from_array(camera.target);
        } else if !owns_session_camera && let Some(camera) = client_persist.tactical_view_camera {
            self.game_logic.session_view.target =
                glam::Vec3::new(camera.position[0], camera.position[2], camera.position[1]);
        }
        // Native map actions already ran at definition admission and saved team
        // state replaced their runtime state. No Core script is replayed here.
        // Native effects follow client/view admission, on this renderer only.
        if let Some(visual) = self.render_pipeline.native_terrain_mut() {
            if let Some(terrain_visual) = terrain_visual {
                terrain_visual.apply_native(visual);
            } else if let Some(configured) = map_water_configuration {
                visual.admit_map_water_configuration(&configured);
            }
        }
        native_commit.finish_after_logic_commit();
        // Movement receipts belong to runtime output, not the save wire. Map
        // admission must not replay movement
        // cues when the newly installed world first publishes presentation.
        self.game_logic.movement_events.clear();
        drop(old_logic);
        self.reset_receiving_recorder_at_engine_boundary();
        // C++ RandomValue is process-static and no load path reseeds it
        // (GameState.cpp:628-741); only mission saves InitRandom(0) in their
        // message-loop follow-up.  Reseed after all staged map work, including
        // deferred team actions, so the first post-load draw continues the
        // save-point stream.
        Self::host_restore_staged_logic_rng(&mut self.game_logic, logic_rng_seed_words);
        self.invalidate_presentation_terrain_cache();
        if let Some(shadow) = self.gameworld_shadow.as_mut() {
            shadow.reset_for_world_boundary();
            shadow.sync_from_host(&self.game_logic);
        }
        // Recreate map defaults only for older pose-only saves. Full native
        // continuation already admitted its fixed basis, HAG goal, pending
        // movements and impulses; rebuilding them here would discard callback FX.
        self.last_presentation_frame = None;
        self.render_pipeline.set_presentation_frame(None);
        self.ensure_presentation_env_seeded()?;
        if !has_camera_continuation {
            self.game_logic.session_view.camera.invalidate_constraints();
            (
                self.game_logic.session_view.target,
                self.game_logic.session_view.camera.camera_position,
                self.game_logic.session_view.camera.camera_zoom,
                self.game_logic.session_view.camera.native_camera,
            ) = Self::bootstrap_native_camera_for_loaded_map(
                self.game_logic.isInShellGame(),
                self.current_player_id,
                Self::startup_camera_defaults_for(&self.game_logic),
                self.last_presentation_frame.as_deref(),
            );
            self.game_logic.session_view.camera.camera_pitch_radians = 0.0;
            self.game_logic.session_view.camera.camera_yaw_radians = 0.0;
            self.game_logic
                .session_view
                .camera
                .cancel_scalar_movements();
            if let Some(camera) = client_persist.camera() {
                self.game_logic.session_view.camera.camera_position =
                    glam::Vec3::from_array(camera.position);
                self.game_logic.session_view.target = glam::Vec3::from_array(camera.target);
                self.game_logic.session_view.camera.camera_zoom = camera.zoom;
                self.game_logic.session_view.camera.camera_yaw_radians = camera.angle;
                self.sync_orbit_from_camera_transform();
                if let Some(terrain) =
                    self.native_camera_terrain_height_around(self.game_logic.session_view.target)
                {
                    self.game_logic
                        .session_view
                        .camera
                        .native_camera
                        .admit_restored_pose_height(
                            self.game_logic.session_view.camera.camera_zoom,
                            terrain,
                        );
                }
            } else if let Some(camera) = client_persist.tactical_view_camera {
                self.game_logic.session_view.target =
                    glam::Vec3::new(camera.position[0], camera.position[2], camera.position[1]);
                self.game_logic.session_view.camera.camera_yaw_radians = camera.angle;
                self.apply_camera_orbit_transform();
            }
            let (_, render_look) = self
                .game_logic
                .session_view
                .camera
                .native_camera
                .basis()
                .pose(
                    self.game_logic.session_view.target,
                    self.game_logic.session_view.camera.camera_zoom,
                    self.game_logic.session_view.camera.camera_pitch_radians,
                    self.game_logic.session_view.camera.camera_yaw_radians,
                );
            self.game_logic.session_view.camera.view_matrix = glam::Mat4::look_at_rh(
                self.game_logic.session_view.camera.camera_position,
                render_look,
                glam::Vec3::Y,
            );
        }
        if owns_session_camera {
            // The saved matrix retains the completed camera pose, including
            // fast-view continuation, but projection belongs to this display's
            // current viewport. Loading must not restore another window's aspect.
            self.rebuild_tactical_projection();
        }
        #[cfg(feature = "game_client")]
        if matches!(
            self.game_logic.game_mode(),
            crate::game_logic::GameMode::Replay
        ) {
            self.set_owned_control_bar_scheme("Observer");
        }
        // Keep the client companion staged until the first complete frozen
        // frame after the successful world replacement. Actual collection
        // validates source identity against that receiving frame and removes
        // each candidate once, so a failed/missing W3D asset cannot replay a
        // stale timeline on a later frame.
        self.render_pipeline
            .queue_client_drawable_restore(client_drawables);
        if let Some(groups) = control_groups
            .as_mut()
            .and_then(|owners| owners.remove(&(self.current_player_id as i32)))
        {
            self.control_groups = groups
                .into_iter()
                .map(|(slot, ids)| {
                    (
                        slot,
                        ids.into_iter().map(crate::game_logic::ObjectId).collect(),
                    )
                })
                .collect();
        } else {
            self.control_groups.clear();
        }
        if self
            .game_logic
            .session_view
            .camera
            .input_policy
            .map_parsed_game_data()
        {
            self.ui_manager.reload_options_preferences();
        }
        self.ui_manager
            .admit_camera_input_defaults(self.game_logic.camera_input_rules());
        // Transaction adaptation: only the successful receiving commit may
        // update controls, after its restored/default camera is installed.
        self.initialize_receiving_replay_controls();
        // The receiving Recorder policy may have been replaced by this commit.
        // Application definitions remain owned by the existing admitted snapshot.
        self.configure_receiving_replay_menu();
        // No Common AR_Play output from this candidate can escape a rejected
        // client/camera/presentation admission. Emit once at the successful tail.
        self.game_logic.finish_saved_snapshot_admission()?;
        pending_map_audio.admit_to_live();
        Ok(())
    }

    pub(super) fn host_save_game_authority(
        &mut self,
        slot: &str,
        save_info: &SaveGameInfo,
    ) -> Result<(), String> {
        // DrawEvents marks chirps once before saving. Rendering sends immutable
        // receipts; admit them to this session before serializing soundPlayed.
        #[cfg(feature = "game_client")]
        if let Some(ui) = self.game_client.in_game_ui_subsystem() {
            use game_client::helpers::InGameUiHooks;
            let ui = game_client::core::subsystems::InGameUiHandle::new(ui);
            super::run_loop::admit_radar_presentations(
                &mut self.game_logic,
                ui.take_radar_acknowledgments(),
            );
        }
        // Wave 928: single save authority boundary.
        // Capture the renderer-owned Drawable companion at the same host
        // authority boundary as the logical snapshot.  The pipeline method
        // is renderer-local and immutable here; an empty/unresolved cache is
        // intentionally a valid fail-closed companion.
        let camera = crate::save_load::snapshot::CameraPersist {
            angle: self.game_logic.session_view.camera.camera_yaw_radians,
            position: self
                .game_logic
                .session_view
                .camera
                .camera_position
                .to_array(),
            target: self.game_logic.session_view.target.to_array(),
            zoom: self.game_logic.session_view.camera.camera_zoom,
        };
        let client_save = crate::save_load::snapshot::ClientSaveState {
            camera: Some(camera),
            camera_continuation: Some(self.capture_camera_continuation()?),
            control_groups: Some(crate::save_load::snapshot::ControlGroupSave {
                player_index: self.current_player_id as i32,
                groups: self
                    .control_groups
                    .iter()
                    .map(|(slot, ids)| (*slot, ids.iter().map(|id| id.0).collect()))
                    .collect(),
            }),
        };
        let visual_world = gamelogic::helpers::ClientVisualHandle::new(Arc::clone(
            &self.game_logic.world_services,
        ));
        let client_drawables = self
            .render_pipeline
            .capture_client_drawable_snapshot(&visual_world);
        let save_path = self.save_file_manager.get_save_path(slot);
        let native_terrain = self.render_pipeline.native_terrain();
        let result = self
            .save_file_manager
            .save_game_with_client_save_state_and_native_terrain(
                slot,
                &self.game_logic,
                client_drawables,
                &mut self.game_client,
                save_info,
                client_save,
                native_terrain,
            )
            .map_err(|e| format!("{e}"));
        // C++ GameState::saveGame (GameState.cpp:547-597): HUD
        // GUI:GameSaveComplete on success; MessageBoxOk(GUI:Error,
        // GUI:ErrorSavingGame) after a write failure.
        Self::surface_save_game_ui_feedback(&save_path, &result);
        result
    }

    /// C++ `GameState::saveGame` user feedback (`GameState.cpp:547-597`).
    fn surface_save_game_ui_feedback(save_path: &std::path::Path, result: &Result<(), String>) {
        #[cfg(feature = "game_client")]
        match result {
            Ok(()) => {
                game_client::helpers::TheInGameUI::message("GUI:GameSaveComplete");
            }
            Err(_) => {
                let filepath = save_path.display().to_string();
                let template = game_client::game_text::GameText::fetch("GUI:ErrorSavingGame");
                let body = if template.contains("%s") {
                    template.replacen("%s", &filepath, 1)
                } else {
                    format!("{template} {filepath}")
                };
                let title = game_client::game_text::GameText::fetch("GUI:Error");
                let _ = game_client::gui::message_box_ok(&title, &body, None);
            }
        }
        #[cfg(not(feature = "game_client"))]
        let _ = (save_path, result);
    }

    /// Select the only offline modes supported by the staged save restore.
    ///
    /// C++ `GameStateMap::xfer` v2 writes `TheGameLogic->getGameMode()` and
    /// load calls `setGameMode` before `startNewGame(TRUE)`. Prefer that saved
    /// mode so a skirmish save loaded from a campaign (or the reverse) rebuilds
    /// the matching player list. Network/replay modes remain rejected.
    fn offline_restore_mode_for_save(
        active_mode: crate::game_logic::GameMode,
        save_info: &SaveGameInfo,
        saved_game_mode: Option<i32>,
    ) -> Result<crate::game_logic::GameMode, String> {
        use crate::game_logic::GameMode;

        if matches!(
            active_mode,
            GameMode::Multiplayer | GameMode::Internet | GameMode::Lan | GameMode::Replay
        ) {
            return Err("network and replay save restore is deferred".to_string());
        }

        if let Some(saved) = saved_game_mode.and_then(crate::save_load::live_game_mode_from_cpp) {
            return match saved {
                GameMode::SinglePlayer | GameMode::Skirmish => Ok(saved),
                GameMode::Multiplayer | GameMode::Internet | GameMode::Lan | GameMode::Replay => {
                    Err("network and replay save restore is deferred".to_string())
                }
                GameMode::Shell | GameMode::None => Self::offline_restore_mode_fallback(save_info),
            };
        }

        match active_mode {
            GameMode::SinglePlayer | GameMode::Skirmish => Ok(active_mode),
            GameMode::Shell | GameMode::None => Self::offline_restore_mode_fallback(save_info),
            GameMode::Multiplayer | GameMode::Internet | GameMode::Lan | GameMode::Replay => {
                Err("network and replay save restore is deferred".to_string())
            }
        }
    }

    fn offline_restore_mode_fallback(
        save_info: &SaveGameInfo,
    ) -> Result<crate::game_logic::GameMode, String> {
        use crate::game_logic::GameMode;
        if matches!(save_info.save_type, SaveFileType::Mission)
            || save_info.mission_number.is_some()
        {
            Ok(GameMode::SinglePlayer)
        } else {
            Ok(GameMode::Skirmish)
        }
    }

    /// Decode, map-load, and restore a save into a disposable world.
    ///
    /// C++ restores a game in the context of its saved map.  The previous Rust
    /// path restored the snapshot directly into the live world, so it could
    /// enter `InGame` with the old map's terrain/scripts or after a failed map
    /// lookup.  This helper is transactional from the caller's perspective:
    /// it never receives the live world and only returns a fully restored,
    /// exact-map world on success.
    fn stage_saved_world_for_restore(
        save_file_manager: &mut SaveFileManager,
        slot: &str,
        active_mode: crate::game_logic::GameMode,
        template_catalog: RestoreTemplateCatalog<'_>,
        weapon_catalog: &Option<
            std::collections::HashMap<String, Arc<gamelogic::weapon::WeaponTemplate>>,
        >,
        projectile_definitions: &crate::game_logic::projectile_definitions::ProjectileDefinitions,
        ocl_application_base: &Arc<
            std::collections::HashMap<String, gamelogic::effects::ObjectCreationList>,
        >,
        ai_definition_base: &game_engine::common::ini::AIData,
        production_energy_base: game_engine::common::ini::ini_game_data::ProductionEnergyRules,
        physics_rules_base: game_engine::common::ini::ini_game_data::PhysicsContentRules,
        terrain_definition_seed: &Arc<game_engine::common::ini::ini_terrain::TerrainTypes>,
        armor_definition_seed: Option<&Arc<gamelogic::object::armor::ArmorDefinitions>>,
        camera_settings: crate::presentation_frame::camera::settings::CameraSettings,
        camera_clip_policy: crate::presentation_frame::camera::clip::CameraClipPolicy,
        camera_shake_policy: crate::presentation_frame::camera::shake_rules::CameraShakePolicy,
        camera_movement_policy: crate::presentation_frame::camera::movement_rules::CameraMovementPolicy,
        camera_input_policy: crate::presentation_frame::camera::input_rules::CameraInputPolicy,
        live_services: &Arc<gamelogic::system::engine_stores::WorldServices>,
    ) -> Result<StagedRestoreWorld, RestoreStagingFailure> {
        Self::stage_saved_world_for_restore_with_native(
            save_file_manager,
            slot,
            active_mode,
            template_catalog,
            weapon_catalog,
            projectile_definitions,
            ocl_application_base,
            ai_definition_base,
            production_energy_base,
            physics_rules_base,
            terrain_definition_seed,
            armor_definition_seed,
            camera_settings,
            camera_clip_policy,
            camera_shake_policy,
            camera_movement_policy,
            camera_input_policy,
            live_services,
            None,
        )
    }

    fn stage_saved_world_for_restore_with_native(
        save_file_manager: &mut SaveFileManager,
        slot: &str,
        active_mode: crate::game_logic::GameMode,
        template_catalog: RestoreTemplateCatalog<'_>,
        weapon_catalog: &Option<
            std::collections::HashMap<String, Arc<gamelogic::weapon::WeaponTemplate>>,
        >,
        projectile_definitions: &crate::game_logic::projectile_definitions::ProjectileDefinitions,
        ocl_application_base: &Arc<
            std::collections::HashMap<String, gamelogic::effects::ObjectCreationList>,
        >,
        ai_definition_base: &game_engine::common::ini::AIData,
        production_energy_base: game_engine::common::ini::ini_game_data::ProductionEnergyRules,
        physics_rules_base: game_engine::common::ini::ini_game_data::PhysicsContentRules,
        terrain_definition_seed: &Arc<game_engine::common::ini::ini_terrain::TerrainTypes>,
        armor_definition_seed: Option<&Arc<gamelogic::object::armor::ArmorDefinitions>>,
        camera_settings: crate::presentation_frame::camera::settings::CameraSettings,
        camera_clip_policy: crate::presentation_frame::camera::clip::CameraClipPolicy,
        camera_shake_policy: crate::presentation_frame::camera::shake_rules::CameraShakePolicy,
        camera_movement_policy: crate::presentation_frame::camera::movement_rules::CameraMovementPolicy,
        camera_input_policy: crate::presentation_frame::camera::input_rules::CameraInputPolicy,
        live_services: &Arc<gamelogic::system::engine_stores::WorldServices>,
        native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<StagedRestoreWorld, RestoreStagingFailure> {
        let crate::save_load::DecodedGameSave {
            snapshot,
            save_info,
            game_client_xfer: game_client_xfer_bytes,
            terrain_visual_xfer,
            saved_game_mode,
        } = save_file_manager
            .load_game_snapshot(slot)
            .map_err(|err| format!("{err}"))?;

        let mut staged = Self::stage_decoded_saved_world_for_restore_with_native(
            &snapshot,
            save_info,
            saved_game_mode,
            game_client_xfer_bytes,
            terrain_visual_xfer,
            slot,
            active_mode,
            template_catalog,
            weapon_catalog,
            projectile_definitions,
            ocl_application_base,
            ai_definition_base,
            production_energy_base,
            physics_rules_base,
            terrain_definition_seed,
            armor_definition_seed,
            camera_settings,
            camera_clip_policy,
            camera_shake_policy,
            camera_movement_policy,
            camera_input_policy,
            live_services,
            |snapshot, staged| {
                save_file_manager
                    .restore_game_snapshot(snapshot, staged)
                    .map_err(|err| format!("{err}"))
            },
            native,
        )?;
        Ok(staged)
    }

    /// Stage a decoded save behind a raw singleton/TLS transaction boundary.
    ///
    /// `restore_snapshot` is deliberately injected only at this private seam:
    /// the focused test can force an error *after* map loading has mutated all
    /// candidate globals, proving rollback preserves the active world.  It is
    /// not a production failpoint and does not alter save schema/versioning.
    /// Direct logical save admission uses the same exact-map candidate kernel
    /// as the native Engine, but has no native renderer/client receiver.
    pub(crate) fn restore_decoded_logical_save(
        manager: &mut SaveFileManager,
        slot: &str,
        decoded: crate::save_load::DecodedGameSave,
        live: &mut crate::game_logic::GameLogic,
    ) -> Result<
        (
            SaveGameInfo,
            Option<crate::save_load::snapshot::PreparedTerrainVisual>,
        ),
        String,
    > {
        manager.validate_logical_restore_admission()?;
        if let Some(error) = live.object_retirement_error() {
            return Err(error.to_string());
        }
        if live.requires_native_drawable_owner() && !live.host_objects().registered_is_empty() {
            return Err("logical-only restore cannot retire a live native Object owner".into());
        }
        if crate::game_logic::script_loader::find_map_file(&decoded.save_info.map_name).is_none() {
            return Self::restore_mapless_logical_save(manager, decoded, live);
        }
        let crate::save_load::DecodedGameSave {
            snapshot,
            save_info,
            saved_game_mode,
            game_client_xfer,
            terrain_visual_xfer,
        } = decoded;
        let staged = Self::stage_decoded_saved_world_for_restore(
            &snapshot,
            save_info,
            saved_game_mode,
            game_client_xfer,
            terrain_visual_xfer,
            slot,
            live.game_mode(),
            RestoreTemplateCatalog {
                templates: &live.templates,
                modules: &live.receiving_module_content,
                object_definitions: &live.receiving_object_definitions,
                baseline: &live.receiving_template_baseline,
                lighting_baseline: live.lighting_rules_baseline(),
                body_damage_baseline: live.body_damage_baseline(),
            },
            &live.weapon_definition_base,
            &live.projectile_definitions,
            &live.ocl_application_definitions(),
            live.ai_definitions.baseline(),
            live.production_energy_baseline(),
            live.physics_rules_baseline(),
            live.effective_terrain_definitions(),
            live.current_armor_definitions(),
            live.session_view.camera.settings,
            live.session_view.camera.clip_policy,
            live.session_view.camera.shake_policy,
            live.session_view.camera.movement_policy,
            live.session_view.camera.input_policy,
            &live.world_services,
            |snapshot, candidate| {
                manager
                    .restore_game_snapshot(snapshot, candidate)
                    .map_err(|error| error.to_string())
            },
        )
        .map_err(|failure| {
            if let Some(world) = failure.retained_logic {
                manager.retain_rejected_logical_world(world);
            }
            failure.reason
        })?;
        let StagedRestoreWorld {
            mut logic,
            info,
            runtime_world,
            terrain_visual,
            owns_session_camera,
            ..
        } = staged;
        if let Err(reason) = logic.finish_saved_snapshot_admission() {
            return Err(Self::reject_logical_restore_candidate(
                manager, logic, reason,
            ));
        }
        let checked = match runtime_world.preflight_native() {
            Ok(checked) => checked,
            Err(reason) => {
                return Err(Self::reject_logical_restore_candidate(
                    manager, logic, reason,
                ));
            }
        };
        // Nothing can fail beyond this boundary. No native effects are applied
        // by this logical-only API; caller owns the returned optional payload.
        if let Err(error) =
            live.destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
        {
            return Err(Self::reject_logical_restore_candidate(
                manager,
                logic,
                format!("outgoing logical Object retirement: {error}"),
            ));
        }
        let committed = checked.install_globals();
        admit_staged_session_view(
            &mut live.session_view,
            &mut logic.session_view,
            owns_session_camera,
        );
        let previous = std::mem::replace(live, logic);
        drop(previous);
        committed.finish_after_logic_commit();
        Ok((info, terrain_visual))
    }

    /// Existing logical-only archives may have no authored map. Their checked
    /// saved grid is legitimate logical data, not a guessed native water map.
    fn restore_mapless_logical_save(
        manager: &mut SaveFileManager,
        decoded: crate::save_load::DecodedGameSave,
        live: &mut crate::game_logic::GameLogic,
    ) -> Result<
        (
            SaveGameInfo,
            Option<crate::save_load::snapshot::PreparedTerrainVisual>,
        ),
        String,
    > {
        let chunks = crate::save_load::snapshot::player_team_chunks_from_world(&decoded.snapshot)
            .map_err(|error| error.to_string())?;
        let definition_names = chunks
            .teams
            .as_ref()
            .filter(|teams| teams.persist_roster)
            .map(|teams| {
                teams
                    .prototypes
                    .iter()
                    .map(|prototype| prototype.team_name.clone())
                    .collect::<Vec<_>>()
            });
        let definitions = live
            .team_factory
            .detached_definition_catalog(definition_names.as_deref())?;
        let transaction = gamelogic::runtime_world_transaction::HostRuntimeWorldStage::begin(
            &live.world_services,
        );
        let mut candidate = crate::game_logic::GameLogic::new();
        let preparation = (|| -> Result<_, String> {
            candidate.start_new_game_for_restore(live.game_mode());
            candidate.begin_saved_snapshot_admission()?;
            // The receiving loaded catalog owns name/mask allocation order; the
            // snapshot carries purchased/pending state, not UpgradeTemplate INI.
            // Clone the value-owned catalog before admitting that runtime state.
            let upgrade_definitions = live
                .world_services
                .upgrade_center()
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone();
            *candidate
                .world_services
                .upgrade_center()
                .write()
                .unwrap_or_else(|error| error.into_inner()) = upgrade_definitions;
            candidate.receiving_module_content = live.receiving_module_content.clone();
            candidate.receiving_object_definitions = live.receiving_object_definitions.clone();
            candidate.receiving_template_baseline = live.receiving_template_baseline.clone();
            candidate.templates.extend(live.templates.clone());
            candidate.weapon_definition_base = live.weapon_definition_base.clone();
            candidate.weapon_definitions = live.weapon_definition_base.clone();
            candidate.projectile_definitions = live.projectile_definitions.clone();
            candidate.seed_ocl_application_definitions(live.ocl_application_definitions());
            candidate.set_ai_definition_base(live.ai_definitions.baseline().clone());
            candidate.admit_production_energy_base(live.production_energy_baseline());
            candidate.admit_physics_rules_base(live.physics_rules_baseline())?;
            candidate.admit_body_damage_baseline(live.body_damage_baseline());
            candidate.admit_lighting_rules_base(live.lighting_rules_baseline().clone());
            candidate
                .admit_terrain_definitions(Arc::clone(live.effective_terrain_definitions()))?;
            if let Some(catalog) = live.current_armor_definitions() {
                candidate.admit_armor_definitions(Arc::clone(catalog));
            }
            candidate.session_view.camera.settings = live.session_view.camera.settings.reset_seed();
            candidate.session_view.camera.clip_policy =
                live.session_view.camera.clip_policy.reset_seed();
            candidate.session_view.camera.shake_policy =
                live.session_view.camera.shake_policy.reset_seed();
            candidate.session_view.camera.movement_policy =
                live.session_view.camera.movement_policy.reset_seed();
            candidate.session_view.camera.input_policy =
                live.session_view.camera.input_policy.reset_seed();
            // Definitions only: IDs/order/controllers retained, mutable script
            // caches and Team instances never shared with the running factory.
            drop(
                candidate
                    .team_factory
                    .replace_for_world_boundary(definitions),
            );
            let builder = crate::save_load::SnapshotBuilder::new();
            builder
                .prepare_logical_terrain_for_restore(&decoded.snapshot, &mut candidate)
                .map_err(|error| error.to_string())?;
            let terrain = crate::save_load::snapshot::prepare_receiving_terrain_visual(
                &candidate,
                decoded.terrain_visual_xfer.as_deref(),
            )
            .map_err(|error| error.to_string())?;
            // Validate the complete paired payload before object/script restore.
            manager
                .restore_game_snapshot(&decoded.snapshot, &mut candidate)
                .map_err(|error| error.to_string())?;
            candidate.select_admitted_replay_observer_local()?;
            if let Some(terrain) = &terrain {
                crate::save_load::snapshot::admit_receiving_terrain_visual(&mut candidate, terrain)
                    .map_err(|error| error.to_string())?;
            }
            let owns_camera = crate::save_load::snapshot::read_owned_session_camera(
                &decoded.snapshot.lifecycle_tail,
            )
            .map_err(|error| error.to_string())?;
            candidate.finish_saved_snapshot_admission()?;
            Ok((terrain, owns_camera))
        })();
        let (terrain, owns_camera) = match preparation {
            Ok(prepared) => prepared,
            Err(reason) => {
                return Err(Self::reject_logical_restore_candidate(
                    manager, candidate, reason,
                ));
            }
        };
        let staged = transaction.finish_and_restore_live(&candidate.world_services);
        let checked = match staged.preflight_native() {
            Ok(checked) => checked,
            Err(reason) => {
                return Err(Self::reject_logical_restore_candidate(
                    manager, candidate, reason,
                ));
            }
        };
        if let Err(error) =
            live.destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
        {
            return Err(Self::reject_logical_restore_candidate(
                manager,
                candidate,
                format!("outgoing logical Object retirement: {error}"),
            ));
        }
        let committed = checked.install_globals();
        admit_staged_session_view(
            &mut live.session_view,
            &mut candidate.session_view,
            owns_camera,
        );
        let previous = std::mem::replace(live, candidate);
        drop(previous);
        committed.finish_after_logic_commit();
        Ok((decoded.save_info, terrain))
    }

    fn stage_decoded_saved_world_for_restore<F>(
        snapshot: &crate::save_load::WorldSnapshot,
        save_info: SaveGameInfo,
        saved_game_mode: Option<i32>,
        game_client_xfer_bytes: Option<Vec<u8>>,
        terrain_visual_xfer_bytes: Option<Vec<u8>>,
        slot: &str,
        active_mode: crate::game_logic::GameMode,
        template_catalog: RestoreTemplateCatalog<'_>,
        weapon_catalog: &Option<
            std::collections::HashMap<String, Arc<gamelogic::weapon::WeaponTemplate>>,
        >,
        projectile_definitions: &crate::game_logic::projectile_definitions::ProjectileDefinitions,
        ocl_application_base: &Arc<
            std::collections::HashMap<String, gamelogic::effects::ObjectCreationList>,
        >,
        ai_definition_base: &game_engine::common::ini::AIData,
        production_energy_base: game_engine::common::ini::ini_game_data::ProductionEnergyRules,
        physics_rules_base: game_engine::common::ini::ini_game_data::PhysicsContentRules,
        terrain_definition_seed: &Arc<game_engine::common::ini::ini_terrain::TerrainTypes>,
        armor_definition_seed: Option<&Arc<gamelogic::object::armor::ArmorDefinitions>>,
        camera_settings: crate::presentation_frame::camera::settings::CameraSettings,
        camera_clip_policy: crate::presentation_frame::camera::clip::CameraClipPolicy,
        camera_shake_policy: crate::presentation_frame::camera::shake_rules::CameraShakePolicy,
        camera_movement_policy: crate::presentation_frame::camera::movement_rules::CameraMovementPolicy,
        camera_input_policy: crate::presentation_frame::camera::input_rules::CameraInputPolicy,
        live_services: &Arc<gamelogic::system::engine_stores::WorldServices>,
        restore_snapshot: F,
    ) -> Result<StagedRestoreWorld, RestoreStagingFailure>
    where
        F: FnOnce(
            &crate::save_load::WorldSnapshot,
            &mut crate::game_logic::GameLogic,
        ) -> Result<(), String>,
    {
        Self::stage_decoded_saved_world_for_restore_with_native(
            snapshot,
            save_info,
            saved_game_mode,
            game_client_xfer_bytes,
            terrain_visual_xfer_bytes,
            slot,
            active_mode,
            template_catalog,
            weapon_catalog,
            projectile_definitions,
            ocl_application_base,
            ai_definition_base,
            production_energy_base,
            physics_rules_base,
            terrain_definition_seed,
            armor_definition_seed,
            camera_settings,
            camera_clip_policy,
            camera_shake_policy,
            camera_movement_policy,
            camera_input_policy,
            live_services,
            restore_snapshot,
            None,
        )
    }

    fn stage_decoded_saved_world_for_restore_with_native<F>(
        snapshot: &crate::save_load::WorldSnapshot,
        save_info: SaveGameInfo,
        saved_game_mode: Option<i32>,
        game_client_xfer_bytes: Option<Vec<u8>>,
        terrain_visual_xfer_bytes: Option<Vec<u8>>,
        slot: &str,
        active_mode: crate::game_logic::GameMode,
        template_catalog: RestoreTemplateCatalog<'_>,
        weapon_catalog: &Option<
            std::collections::HashMap<String, Arc<gamelogic::weapon::WeaponTemplate>>,
        >,
        projectile_definitions: &crate::game_logic::projectile_definitions::ProjectileDefinitions,
        ocl_application_base: &Arc<
            std::collections::HashMap<String, gamelogic::effects::ObjectCreationList>,
        >,
        ai_definition_base: &game_engine::common::ini::AIData,
        production_energy_base: game_engine::common::ini::ini_game_data::ProductionEnergyRules,
        physics_rules_base: game_engine::common::ini::ini_game_data::PhysicsContentRules,
        terrain_definition_seed: &Arc<game_engine::common::ini::ini_terrain::TerrainTypes>,
        armor_definition_seed: Option<&Arc<gamelogic::object::armor::ArmorDefinitions>>,
        camera_settings: crate::presentation_frame::camera::settings::CameraSettings,
        camera_clip_policy: crate::presentation_frame::camera::clip::CameraClipPolicy,
        camera_shake_policy: crate::presentation_frame::camera::shake_rules::CameraShakePolicy,
        camera_movement_policy: crate::presentation_frame::camera::movement_rules::CameraMovementPolicy,
        camera_input_policy: crate::presentation_frame::camera::input_rules::CameraInputPolicy,
        live_services: &Arc<gamelogic::system::engine_stores::WorldServices>,
        restore_snapshot: F,
        mut native: Option<&mut dyn crate::game_logic::NativeMapAdmission>,
    ) -> Result<StagedRestoreWorld, RestoreStagingFailure>
    where
        F: FnOnce(
            &crate::save_load::WorldSnapshot,
            &mut crate::game_logic::GameLogic,
        ) -> Result<(), String>,
    {
        let mode = Self::offline_restore_mode_for_save(active_mode, &save_info, saved_game_mode)?;
        if mode != crate::game_logic::GameMode::Skirmish
            && (crate::save_load::snapshot::initial_skirmish::read(snapshot)
                .map_err(|error| error.to_string())?
                .is_some()
                || crate::save_load::snapshot::initial_skirmish::source(snapshot)
                    .map_err(|error| error.to_string())?
                    .is_some())
        {
            return Err("initial Skirmish admission supplied for another game mode".into());
        }
        let initial_skirmish =
            crate::save_load::snapshot::initial_skirmish::read_for_restore(snapshot)
                .map_err(|error| error.to_string())?;
        if mode == crate::game_logic::GameMode::Skirmish {
            let initial = initial_skirmish.as_ref().ok_or("Missing saved initial SkirmishGameInfo and native resolved admission; cannot reconstruct participant definitions")?;
            initial.validate_definitions()?;
        } else if initial_skirmish.is_some() {
            return Err("initial Skirmish admission supplied for another game mode".into());
        }
        // Decode all camera owner state before map staging can touch shared services.
        let camera_continuation =
            crate::save_load::snapshot::read_native_camera_continuation(&snapshot.lifecycle_tail)
                .map_err(|err| format!("{err}"))?;
        let owns_session_camera =
            crate::save_load::snapshot::read_owned_session_camera(&snapshot.lifecycle_tail)
                .map_err(|err| format!("{err}"))?;
        // Validate detached view data before map staging can touch shared services.
        if let Some(camera) = snapshot.persist_v18.camera() {
            camera.validate().map_err(|err| format!("{err}"))?;
        }
        if let Some(camera) = snapshot.persist_v18.tactical_view_camera {
            camera.validate().map_err(|err| format!("{err}"))?;
        }
        if let Some(bytes) = game_client_xfer_bytes.as_deref() {
            crate::save_load::snapshot::validate_game_client_xfer_bytes(bytes)
                .map_err(|err| format!("{err}"))?;
        }
        let control_groups =
            crate::save_load::snapshot::read_control_groups(&snapshot.lifecycle_tail)
                .map_err(|err| format!("{err}"))?;
        let saved_map = save_info.map_name.trim();
        if saved_map.is_empty() || saved_map == "-" || saved_map.eq_ignore_ascii_case("unknown") {
            return Err(format!(
                "save '{slot}' has no usable saved map identity ({:?})",
                save_info.map_name
            )
            .into());
        }

        // Resolve before touching a staging GameLogic.  `load_map` permits a
        // couple of development-only maps, but a player save must name an
        // on-disk retail map; accepting a fallback here would make the restored
        // snapshot look playable while its map-specific terrain/scripts differ.
        let resolved_map = crate::game_logic::script_loader::find_map_file(saved_map)
            .ok_or_else(|| format!("saved map '{saved_map}' is not available on disk"))?;
        let resolved_map = resolved_map.canonicalize().unwrap_or(resolved_map);
        let resolved_map_name = resolved_map
            .to_str()
            .ok_or_else(|| format!("saved map '{saved_map}' has a non-UTF-8 path"))?
            .to_string();

        // Map loading and snapshot restore still write shared metadata (sides,
        // players, teams, scripts and named/area trackers). Terrain, shroud,
        // visuals and Main AI belong to their world and are not swapped.
        // Move shared metadata out before constructing the candidate so
        // failure cannot poison the still-playable host match. Core/native
        // transaction state is restored before installing the candidate.
        let runtime_stage =
            gamelogic::runtime_world_transaction::HostRuntimeWorldStage::begin(live_services);
        let mut staged = crate::game_logic::GameLogic::new();
        staged.seed_ocl_application_definitions(Arc::clone(ocl_application_base));
        staged.set_ai_definition_base(ai_definition_base.clone());
        staged.admit_production_energy_base(production_energy_base);
        staged.admit_physics_rules_base(physics_rules_base)?;
        staged.admit_body_damage_baseline(template_catalog.body_damage_baseline);
        staged.admit_lighting_rules_base(template_catalog.lighting_baseline.clone());
        staged.admit_terrain_definitions(Arc::clone(terrain_definition_seed))?;
        if let Some(catalog) = armor_definition_seed {
            staged.admit_armor_definitions(Arc::clone(catalog));
        }
        staged.start_new_game_for_restore(mode);
        staged.begin_saved_snapshot_admission()?;
        if native.is_some() {
            staged.require_native_drawable_owner();
        }
        if let Some(initial) = &initial_skirmish {
            initial.admit(&mut staged)?;
        }
        // Detached receiving seed; no live settings change on failed load.
        staged.session_view.camera.settings = camera_settings.reset_seed();
        staged.session_view.camera.clip_policy = camera_clip_policy.reset_seed();
        staged.session_view.camera.shake_policy = camera_shake_policy.reset_seed();
        staged.session_view.camera.movement_policy = camera_movement_policy.reset_seed();
        staged.session_view.camera.input_policy = camera_input_policy.reset_seed();
        // Map object restoration needs the live INI/template catalog.  Keep
        // custom/mod templates from the source match while retaining the fresh
        // world's standard startup catalog.
        staged.receiving_module_content = template_catalog.modules.clone();
        staged.receiving_object_definitions = template_catalog.object_definitions.clone();
        staged.receiving_template_baseline = template_catalog.baseline.clone();
        staged.templates.extend(template_catalog.templates.clone());
        staged.weapon_definition_base = weapon_catalog.clone();
        staged.weapon_definitions = weapon_catalog.clone();
        staged.projectile_definitions = projectile_definitions.clone();

        // Preserve the saved logical identity in `GameLogic::map_name`; the
        // prior resolution above only proves that this exact identity maps to
        // a real on-disk retail file rather than to a development fallback.
        let definition_source = save_info.pristine_map_name.as_deref().unwrap_or(saved_map);
        let preparation = (|| -> Result<_, String> {
            let mut map_water_configuration = None;
            let mut capture_map_water = |state: &gamelogic::terrain_water::WaterGridState| {
                map_water_configuration = Some(state.clone());
            };
            if !staged.load_map_with_definition_source_and_water_delivery_and_native_checked(
                saved_map,
                definition_source,
                Some(&mut capture_map_water),
                native
                    .as_mut()
                    .map(|native| &mut **native as &mut dyn crate::game_logic::NativeMapAdmission),
            )? {
                return Err(format!(
                    "failed to load saved map '{saved_map}' from '{resolved_map_name}'"
                ));
            }
            if !staged.isInGame() || staged.get_current_map_name() != saved_map {
                return Err(format!(
                    "saved map '{saved_map}' did not become the active staged map"
                ));
            }

            // Validate the companion against actual map water layout before the
            // logical snapshot can replace it. No native state is touched here.
            let terrain_visual = crate::save_load::snapshot::prepare_receiving_terrain_visual(
                &staged,
                terrain_visual_xfer_bytes.as_deref(),
            )
            .map_err(|error| error.to_string())?;
            // The saved-map foundation already owns native constructor outputs.
            // Retire those exact Objects through the candidate loan before the
            // detached snapshot can replace frame/players/teams and saved IDs.
            if let Some(native) = native.as_mut() {
                staged
                    .destroy_all_objects_immediate_with_effects_checked(&mut native.effects())
                    .map_err(|error| format!("saved-map foundation retirement: {error}"))?;
            } else {
                staged
                    .destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
                    .map_err(|error| format!("saved-map foundation retirement: {error}"))?;
            }
            restore_snapshot(snapshot, &mut staged)?;
            if let Some(native) = native.as_mut() {
                native.admit_restored_object_drawables(&mut staged)?;
            }
            if let Some(prepared) = &terrain_visual {
                crate::save_load::snapshot::admit_receiving_terrain_visual(&mut staged, prepared)
                    .map_err(|error| error.to_string())?;
            }

            // Snapshot/companion admission may replace canonical raw heights after
            // map factories ran. Finish that SAME detached native owner now; saved
            // water and the optional scorch tail still apply at native commit.
            if let Some(native) = native.as_mut() {
                native.admit_restored_heights(&staged)?;
            }

            // Snapshot restoration must not turn a successful map load into a
            // false in-game claim.  It currently restores terrain/objects but not
            // GameMode/map identity, so these checks also guard future changes.
            if !staged.isInGame() || staged.get_current_map_name() != saved_map {
                return Err(format!(
                    "saved map '{saved_map}' was lost while restoring the snapshot"
                ));
            }

            Ok((terrain_visual, map_water_configuration))
        })();
        let (terrain_visual, map_water_configuration) = match preparation {
            Ok(prepared) => prepared,
            Err(error) => {
                let cleanup = if let Some(native) = native.as_mut() {
                    staged
                        .destroy_all_objects_immediate_with_effects_checked(&mut native.effects())
                        .map_err(|failure| failure.to_string())
                        .and_then(|()| native.discard_attempt())
                } else {
                    staged
                        .destroy_all_objects_immediate_with_effects_checked(&mut Default::default())
                        .map_err(|failure| failure.to_string())
                };
                let reason = format!("{error}; staged Object cleanup={cleanup:?}");
                return Err(RestoreStagingFailure {
                    reason,
                    retained_logic: cleanup.is_err().then(|| Box::new(staged)),
                });
            }
        };

        // Restore the active singleton/TLS state before exposing the candidate
        // to the caller.  The returned opaque bundle is installed only by the
        // no-fail combined commit below.
        let runtime_world = runtime_stage.finish_and_restore_live(&staged.world_services);
        Ok(StagedRestoreWorld {
            native_map: Default::default(),
            logic: staged,
            info: save_info,
            runtime_world,
            shroud: snapshot.shroud.clone(),
            client_drawables: snapshot.client_drawables.clone(),
            game_client_xfer_bytes,
            terrain_visual,
            map_water_configuration,
            client_persist: snapshot.persist_v18.clone(),
            camera_continuation,
            owns_session_camera,
            control_groups,
            logic_rng_seed_words: snapshot.logic_rng_seed_words,
        })
    }

    /// Wave 928: single load authority boundary.
    pub(super) fn host_load_game_authority(
        &mut self,
        slot: &str,
    ) -> Result<SaveGameInfo, HostLoadFailure> {
        self.host_try_load_game_authority(slot)
    }

    fn validate_staged_native_fade(
        logic: &crate::game_logic::GameLogic,
    ) -> Result<(), HostLoadFailure> {
        logic.owned_script_engine.as_ref().ok_or_else(|| HostLoadFailure::Staging(
            "Cannot freeze restored native camera fade: receiving interpreter is unavailable or lent".into(),
        ))?;
        Ok(())
    }

    fn host_try_load_game_authority(
        &mut self,
        slot: &str,
    ) -> Result<SaveGameInfo, HostLoadFailure> {
        let save_info = self
            .save_file_manager
            .get_save_info(slot)
            .map_err(|err| HostLoadFailure::Staging(format!("{err}")))?;
        if save_info.save_type == SaveFileType::Mission {
            return self
                .host_restart_mission_from_save(slot, save_info)
                .map_err(HostLoadFailure::Staging);
        }

        // Keep the current world untouched until the save metadata, exact map,
        // and snapshot all restore successfully in a staging world.
        // CHUNK_Campaign is stashed during decode; apply only after commit so
        // a failed load cannot leave half-applied campaign on the live match
        // (C++ GameState.cpp:695-712 clearGameData).
        self.rejected_native_worlds
            .validate_admission()
            .map_err(HostLoadFailure::Staging)?;
        let prior_campaign = crate::save_load::capture_live_campaign_state();
        let active_mode = self.game_logic.game_mode();
        let template_catalog = self.game_logic.templates.clone();

        #[cfg(feature = "game_client")]
        let staged_result = (|| -> Result<StagedRestoreWorld, String> {
            use crate::game_logic::NativeMapAdmission;
            let policy =
                super::native_view_effects::receiving_native_drawable_policy_for(&self.game_logic);
            let mut candidate = super::map_native_admission::NativeMapCandidate::with_epoch(
                self.next_native_world_epoch(),
            );
            let mut retirement = game_client::core::game_client::NativeParticleRetirementService;
            let mut attempt = candidate.attempt(
                self.render_pipeline.borrow_native_draw_services(),
                &mut self.graphics_system,
                self.current_player_id,
                policy,
                &mut retirement,
            )?;
            let result = Self::stage_saved_world_for_restore_with_native(
                &mut self.save_file_manager,
                slot,
                active_mode,
                RestoreTemplateCatalog {
                    templates: &template_catalog,
                    modules: &self.game_logic.receiving_module_content,
                    object_definitions: &self.game_logic.receiving_object_definitions,
                    baseline: &self.game_logic.receiving_template_baseline,
                    lighting_baseline: self.game_logic.lighting_rules_baseline(),
                    body_damage_baseline: self.game_logic.body_damage_baseline(),
                },
                &self.game_logic.weapon_definition_base,
                &self.game_logic.projectile_definitions,
                &self.game_logic.ocl_application_definitions(),
                self.game_logic.ai_definitions.baseline(),
                self.game_logic.production_energy_baseline(),
                self.game_logic.physics_rules_baseline(),
                self.game_logic.effective_terrain_definitions(),
                self.game_logic.current_armor_definitions(),
                self.game_logic.session_view.camera.settings,
                self.game_logic.session_view.camera.clip_policy,
                self.game_logic.session_view.camera.shake_policy,
                self.game_logic.session_view.camera.movement_policy,
                self.game_logic.session_view.camera.input_policy,
                &self.game_logic.world_services,
                Some(&mut attempt),
            );
            // Staging errors retire/discard before their logical owner drops;
            // do not replay an already failed candidate destructor here.
            drop(attempt);
            let mut staged = match result {
                Ok(staged) => staged,
                Err(failure) => return Err(self.retain_staging_failure(failure, candidate)),
            };
            staged.native_map = candidate;
            Ok(staged)
        })();
        #[cfg(not(feature = "game_client"))]
        let staged_result = Self::stage_saved_world_for_restore_with_native(
            &mut self.save_file_manager,
            slot,
            active_mode,
            RestoreTemplateCatalog {
                templates: &template_catalog,
                modules: &self.game_logic.receiving_module_content,
                object_definitions: &self.game_logic.receiving_object_definitions,
                baseline: &self.game_logic.receiving_template_baseline,
                lighting_baseline: self.game_logic.lighting_rules_baseline(),
                body_damage_baseline: self.game_logic.body_damage_baseline(),
            },
            &self.game_logic.weapon_definition_base,
            &self.game_logic.projectile_definitions,
            &self.game_logic.ocl_application_definitions(),
            self.game_logic.ai_definitions.baseline(),
            self.game_logic.production_energy_baseline(),
            self.game_logic.physics_rules_baseline(),
            self.game_logic.effective_terrain_definitions(),
            self.game_logic.current_armor_definitions(),
            self.game_logic.session_view.camera.settings,
            self.game_logic.session_view.camera.clip_policy,
            self.game_logic.session_view.camera.shake_policy,
            self.game_logic.session_view.camera.movement_policy,
            self.game_logic.session_view.camera.input_policy,
            &self.game_logic.world_services,
            None,
        )
        .map_err(|failure| self.retain_staging_failure(failure, Default::default()));
        let staged = match staged_result {
            Ok(staged) => staged,
            Err(err) => {
                crate::save_load::rollback_campaign_after_failed_load(prior_campaign);
                return Err(HostLoadFailure::Staging(err));
            }
        };

        let mut staged = staged;
        if let Err(failure) = Self::validate_staged_native_fade(&staged.logic) {
            if let Err(cleanup) =
                self.reject_receiving_world_candidate(staged.logic, staged.native_map)
            {
                crate::save_load::rollback_campaign_after_failed_load(prior_campaign);
                return Err(HostLoadFailure::Staging(format!(
                    "{failure:?}; rejecting saved native candidate: {cleanup}"
                )));
            }
            crate::save_load::rollback_campaign_after_failed_load(prior_campaign);
            return Err(failure);
        }
        let save_info = staged.info.clone();
        if let Err(err) = self.host_replace_staged_restore_world(staged) {
            crate::save_load::rollback_campaign_after_failed_load(prior_campaign);
            return Err(HostLoadFailure::Committed(err));
        }
        crate::save_load::commit_stashed_campaign_state();
        info!(
            "Game loaded successfully from slot '{}' on map '{}'",
            slot,
            self.game_logic.get_current_map_name()
        );
        Ok(save_info)
    }

    /// C++ `GameState::loadGame` user feedback (`GameState.cpp:695-712`).
    fn surface_load_game_ui_feedback(save_path: &std::path::Path) {
        #[cfg(feature = "game_client")]
        {
            let filepath = save_path.display().to_string();
            let (title, body) = crate::save_load::format_error_loading_game(&filepath);
            let _ = game_client::gui::message_box_ok(&title, &body, None);
        }
        #[cfg(not(feature = "game_client"))]
        let _ = save_path;
    }

    /// C++ `GameState::loadGame` (`GameState.cpp:706-742`) for
    /// `SAVE_FILE_TYPE_MISSION`: xfer CHUNK_Campaign, InitRandom(0),
    /// pendingFile = mission map, MSG_NEW_GAME(GAME_SINGLE_PLAYER,
    /// TheCampaignManager->getGameDifficulty(), getRankPoints()).
    fn host_restart_mission_from_save(
        &mut self,
        slot: &str,
        mut save_info: SaveGameInfo,
    ) -> Result<SaveGameInfo, String> {
        let mut rank_points = 0;
        if let Ok(state) = self.save_file_manager.read_campaign_state(slot) {
            save_info.difficulty = match state.difficulty {
                0 => GameDifficulty::Easy,
                2 => GameDifficulty::Hard,
                _ => GameDifficulty::Medium,
            };
            if !state.campaign.is_empty() {
                save_info.campaign_side = Some(state.campaign.clone());
            }
            rank_points = state.rank_points;
            game_engine::System::apply_campaign_manager_runtime(state.clone());
            game_client::gui::campaign_manager::get_campaign_manager()
                .apply_logic_chunk_state(state);
        }
        game_engine::common::random_value::init_client_audio_random_with_seed(0);
        self.host_game_logic_mut().set_logic_random_seed(0);
        {
            let mut global = game_engine::common::global_data::write();
            global.pending_file = save_info.map_name.clone();
        }
        let difficulty = match save_info.difficulty {
            GameDifficulty::Easy => 0,
            GameDifficulty::Medium => 1,
            GameDifficulty::Hard => 2,
        };
        if let Ok(mut stream) = game_engine::common::message_stream::get_message_stream().write() {
            let msg = stream
                .append_message(game_engine::common::message_stream::GameMessageType::NewGame);
            msg.append_integer_argument(0); // GAME_SINGLE_PLAYER
            msg.append_integer_argument(difficulty);
            msg.append_integer_argument(rank_points);
        }
        let faction = save_info
            .campaign_side
            .clone()
            .filter(|side| !side.trim().is_empty())
            .unwrap_or_else(|| "USA".to_string());
        self.start_game_from_ui(HostStartRequest::without_player_template(
            crate::game_logic::GameMode::SinglePlayer,
            faction,
            save_info.map_name.clone(),
            None,
        ));
        info!(
            "Mission save '{}' restarts map '{}' instead of restoring mid-world",
            save_info.filename, save_info.map_name
        );
        Ok(save_info)
    }

    /// Wave 928: single skirmish-config authority boundary.
    #[inline]
    pub(super) fn host_apply_skirmish_config_authority(
        &mut self,
        config: &crate::skirmish_config::SkirmishMatchConfig,
    ) -> Result<(), String> {
        // Wave 928: single skirmish-config authority boundary.

        crate::skirmish_config::apply_skirmish_config(&mut self.game_logic, config)?;
        // GameInfo's explicit local slot may be a later human. Commands and
        // presentation must use the player admitted by this receiving world.
        if let Some(local) = self.game_logic.local_player_id() {
            self.current_player_id = local;
        }
        Ok(())
    }

    #[cfg(all(any(test, feature = "internal"), feature = "game_client"))]
    pub(super) fn probe_skirmish_local_command_owner(&mut self) -> Result<(), String> {
        // Exercise the actual native admission boundary, not a second selector.
        game_engine::common::ini::ensure_player_templates_loaded();
        let store = game_engine::common::rts::player_template::get_player_template_store();
        let usa = store.find_template_index("FactionAmerica").unwrap() as i32;
        let gla = store.find_template_index("FactionGLA").unwrap() as i32;
        drop(store);
        {
            let mut setup = game_client::gui::get_skirmish_setup();
            setup.set_selected_map("Lone Eagle".into());
            let gi = setup.game_info_mut().game_info_mut();
            gi.reset();
            gi.set_map("Lone Eagle".into());
            gi.set_seed(713);
            gi.set_starting_cash(game_client::Money::new(19000));
            for (i, tpl, ip) in [(0, usa, 1), (1, gla, 2)] {
                let slot = gi.get_slot_mut(i).unwrap();
                slot.set_state(
                    game_client::SlotState::Player,
                    format!("Owner Human {i}"),
                    ip,
                );
                slot.set_player_template(tpl);
                slot.set_start_pos(i as i32);
                slot.set_team_number(i as i32);
                slot.set_color(-1);
                slot.save_off_original_info();
            }
            gi.set_local_ip(2);
            gi.set_in_game();
            assert_eq!(gi.get_local_slot_num(), 1);
        }
        let config = crate::skirmish_config::config_from_client_skirmish_setup(None)
            .ok_or("actual two-human Client setup did not produce a native configuration")?;
        self.current_player_id = 0;
        self.host_apply_skirmish_config_authority(&config)?;
        assert_eq!(self.game_logic.local_player_id(), Some(1));
        assert_eq!(
            self.current_player_id, 1,
            "Engine command owner follows admitted local slot"
        );
        let direct = crate::skirmish_config::golden_skirmish_config("Lone Eagle");
        self.host_apply_skirmish_config_authority(&direct)?;
        assert_eq!(self.game_logic.local_player_id(), Some(0));
        assert_eq!(
            self.current_player_id, 0,
            "direct native local admission remains unchanged"
        );
        Ok(())
    }

    /// Wave 928: runtime-host GameWorld authority probe boundary.
    #[inline]
    pub(super) fn host_simulate_gameworld_authority_probe(&mut self) -> bool {
        // Wave 928: runtime-host GameWorld authority probe boundary.

        let (_, probe) = crate::gameworld_shadow::probe_host_vs_gameworld(&self.game_logic);
        probe.counts_match
            && probe.economy_match
            && probe.health_match
            && probe.pose_match
            && probe.attack_target_match
            && probe.move_target_match
            && probe.weapon_match
            && probe.contain_match
            && probe.production_match
            && probe.destroy_visibility_match
    }

    pub(super) fn host_quick_save_from_hotkey(&mut self, source: &str) {
        // Wave 611: host residual helper.
        // Prefer presentation game_mode residual when installed.
        let mode = self.presentation_or_live_game_mode();
        if !matches!(mode, GameMode::SinglePlayer | GameMode::Skirmish) {
            info!(
                "{} ignored: quick save is only available in single-player or skirmish (mode={:?})",
                source, mode
            );
            return;
        }

        info!("{} requested quick save", source);
        let save_info = self.build_save_info(
            "quicksave",
            "Quick Save",
            "Quick Save",
            SaveFileType::QuickSave,
        );

        if let Err(err) = self.host_save_game_authority("quicksave", &save_info) {
            warn!("Quick save failed for 'quicksave': {}", err);
        } else {
            info!("Quick save stored in slot 'quicksave'");
        }
    }

    pub(super) fn quick_load_from_hotkey(&mut self, source: &str) {
        let restore_screen = match self.current_state {
            GameState::Paused => Some(Screen::PauseMenu),
            GameState::InGame => Some(Screen::GameHUD),
            _ => None,
        };
        // Prefer presentation game_mode residual when installed.
        let mode = self.presentation_or_live_game_mode();
        if !matches!(mode, GameMode::SinglePlayer | GameMode::Skirmish) {
            info!(
                "{} ignored: quick load is only available in single-player or skirmish (mode={:?})",
                source, mode
            );
            if self.ui_manager.current_screen() == Some(Screen::Loading) {
                if let Some(screen) = restore_screen {
                    self.ui_manager.transition_to_screen(screen);
                }
            }
            return;
        }

        if !self.save_file_manager.save_exists("quicksave") {
            warn!(
                "{} requested quick load, but no 'quicksave' slot exists",
                source
            );
            if self.ui_manager.current_screen() == Some(Screen::Loading) {
                if let Some(screen) = restore_screen {
                    self.ui_manager.transition_to_screen(screen);
                }
            }
            return;
        }

        info!("{} requested quick load from slot 'quicksave'", source);
        let _ = self.load_game_from_ui("quicksave");
    }

    /// Wave 611: via `host_save_game_from_ui`.
    pub(super) fn save_game_from_ui(&mut self, slot: &str, display_name: &str) {
        // Wave 611: thin wrapper — residual via host helper.
        self.host_save_game_from_ui(slot, display_name)
    }

    pub(super) fn host_save_game_from_ui(&mut self, slot: &str, display_name: &str) {
        // Wave 611: host residual helper.
        let slot = slot.trim();
        if slot.is_empty() {
            return;
        }

        // C++ GameState::missionSave uses GUI:MissionSave. PopupSaveLoad
        // setEditDescription never invents "Save {slot}".
        let description = if display_name.trim().is_empty() {
            Self::default_ui_save_description(&self.presentation_or_boot_map_name())
        } else {
            display_name.to_string()
        };
        let save_info =
            self.build_save_info(slot, &description, &description, SaveFileType::Normal);

        if let Err(err) = self.host_save_game_authority(slot, &save_info) {
            warn!("Save failed for '{}': {}", slot, err);
        } else {
            info!("Saved game to slot '{}'", slot);
        }
    }

    /// Empty UI names: `GUI:MissionSave` while a campaign is live, else
    /// C++ `setEditDescription` map-leaf / campaign+number.
    fn default_ui_save_description(map_name: &str) -> String {
        let mission = crate::save_load::current_mission_save_description();
        if !mission.is_empty() && !mission.contains("MISSING:") {
            return mission;
        }
        crate::save_load::default_save_edit_description(map_name)
    }

    /// Wave 611: via `host_load_game_from_ui`.
    pub(super) fn load_game_from_ui(&mut self, slot: &str) -> Result<(), String> {
        // Wave 611: thin wrapper — residual via host helper.
        self.host_load_game_from_ui(slot)
    }

    /// A staged load has not changed the live world until
    /// `host_replace_game_logic` succeeds.  Failed staging must therefore
    /// return an active match to its prior UI state rather than calling the
    /// match-reset/menu path, which would discard its renderer/presentation
    /// state despite the failed transaction.
    fn staged_load_failure_return_state(prior_state: GameState) -> GameState {
        match prior_state {
            GameState::InGame | GameState::Paused => prior_state,
            _ => GameState::Menu,
        }
    }

    // Shared by the actual UI load path and its native main-thread regression.
    fn host_handle_load_failure(
        &mut self,
        slot: &str,
        prior_state: GameState,
        failure: HostLoadFailure,
    ) -> Result<(), String> {
        let committed = failure.requires_match_reset();
        let err = failure.into_message();
        warn!("Load failed for '{}': {}", slot, err);
        if let Some(retirement) = self.game_logic.object_retirement_error() {
            // Retain the failed outgoing session/native owner pair. Earlier
            // destructor callbacks may already have completed; no reset/retry.
            error!("Load left an unrecoverable native owner: {retirement}");
            self.transition_to_state(GameState::Exiting);
            return Err(err);
        }
        if committed && self.rejected_native_worlds.validate_admission().is_err() {
            // Detached failed owners prohibit subsystem reset too. A staging
            // rejection may preserve its healthy old world; after committed
            // failure that rollback is no longer guaranteed.
            error!("Load cannot reset while a failed native candidate remains retained");
            self.transition_to_state(GameState::Exiting);
            return Err(err);
        }
        if committed {
            // C++ GameState::loadGame clears GameLogic data and resets
            // GameEngine on a post-process failure. The staged Rust
            // path can preserve an old match before commit, but once
            // a nested Drawable Xfer fails it must not expose the
            // partly installed world as a playable InGame state.
            self.return_to_main_menu_after_match();
        } else {
            self.transition_to_state(Self::staged_load_failure_return_state(prior_state));
        }
        // C++ GameState.cpp:695-712 resets the engine before showing
        // GUI:ErrorLoadingGame. A committed failure's menu teardown
        // would otherwise destroy a dialog created beforehand.
        let save_path = self.save_file_manager.get_save_path(slot);
        Self::surface_load_game_ui_feedback(&save_path);
        Err(err)
    }

    pub(super) fn host_load_game_from_ui(&mut self, slot: &str) -> Result<(), String> {
        // Wave 611: host residual helper.
        let slot = slot.trim();
        if slot.is_empty() {
            return Err("empty save slot".into());
        }

        // Capture this before load-screen preparation changes the state to
        // Loading. The staging helper below owns no live GameLogic/renderer
        // references, so this is the only state needed for rollback.
        let prior_state = self.current_state;

        // Headless host residual: skip load-screen SFX / GPU rebinds that block the
        // control loop for many seconds after snapshot restore.
        let headless_host = self.runtime_host_headless;

        if !headless_host {
            #[cfg(feature = "game_client")]
            // Prefer presentation game_mode residual when installed.
            self.prepare_cpp_load_screen_for_mode(self.presentation_or_live_game_mode(), true);
        }
        self.transition_to_state(GameState::Loading);
        match self.host_load_game_authority(slot) {
            Ok(save_info) => {
                if self.pending_match_start.is_some() {
                    // Mission save posted MSG_NEW_GAME and parked startNewGame.
                    // Stay on Loading so complete_parked_match_start loads the
                    // next map clean instead of installing a mid-world snapshot.
                    info!(
                        "Mission save '{}' queued a fresh start on '{}'",
                        slot, save_info.map_name
                    );
                    return Ok(());
                }
                info!(
                    "Loaded save '{}' (map={}, name={})",
                    slot, save_info.map_name, save_info.display_name
                );

                // The staged authority has installed a freshly decoded map.
                // Drop every prior presentation/residual snapshot before the
                // first post-load frame so UI, terrain, and WGPU cannot retain
                // the map that was active before the load attempt.
                let loaded_map_name = self.game_logic.get_current_map_name().to_string();
                // The staging authority has already validated this world. Keep
                // its authoritative mode so clearing stale presentation state
                // cannot turn a successfully restored offline match into an
                // unclassified session. Failed staged loads return before this
                // point and preserve the prior active world instead.
                let loaded_match_mode = self.game_logic.game_mode();
                self.host_clear_match_residuals();
                self.host_match_game_mode = Some(loaded_match_mode);
                self.host_match_map_name = Some(loaded_map_name.clone());
                self.last_presentation_frame = None;
                self.last_ui_state = None;
                self.render_pipeline.set_presentation_frame(None);

                self.host_set_paused(false);
                self.match_over = false;
                self.victory_summary = None;
                self.selected_objects.clear();

                if !headless_host {
                    // Wave 455: seed presentation env then apply presentation-only heightmap/skybox hints.
                    // Wave 455: seed presentation env then apply presentation-only hints.
                    self.ensure_presentation_env_seeded()?;
                    Self::apply_heightmap_hint(&mut self.render_pipeline);
                    Self::apply_skybox_hint(&mut self.render_pipeline);
                    self.ensure_presentation_env_seeded()?;
                    Self::sync_render_terrain_visual(
                        &mut self.game_logic,
                        &mut self.render_pipeline,
                        &self.graphics_system,
                        loaded_map_name.as_str(),
                    );
                    if let Err(err) = self.reinitialize_minimap_renderer() {
                        warn!(
                            "Failed to reinitialize minimap renderer after load: {}",
                            err
                        );
                    }
                    self.ensure_presentation_env_seeded()?;
                    Self::apply_map_lighting(&mut self.graphics_system, &mut self.render_pipeline);
                }

                // Seed presentation before first InGame render (units/HUD identity).
                self.seed_presentation_after_match_start()?;
                self.transition_to_state(GameState::InGame);
                Ok(())
            }
            Err(failure) => self.host_handle_load_failure(slot, prior_state, failure),
        }
    }
}

#[cfg(test)]
#[path = "host_authority/staged_restore_tests.rs"]
mod staged_restore_tests;

#[cfg(test)]
#[path = "fixed_step_boundary_tests.rs"]
mod fixed_step_boundary_tests;

#[cfg(test)]
#[path = "native_team_preflight_tests.rs"]
mod native_team_preflight_tests;

#[cfg(test)]
#[path = "camera_rule_stage_tests.rs"]
mod camera_rule_stage_tests;

#[cfg(test)]
#[path = "camera_startup_handoff_tests.rs"]
mod camera_startup_handoff_tests;

#[cfg(test)]
#[path = "host_authority/ocl_stage_tests.rs"]
mod ocl_stage_tests;

// Opt-in RuntimeHostBridge acceptance on the actual live Engine/window.
#[cfg(feature = "game_client")]
#[path = "host_authority/camera_window_gate.rs"]
mod camera_window_gate;

#[cfg(test)]
#[path = "host_authority/initial_skirmish_owner_tests.rs"]
mod initial_skirmish_owner_tests;

#[cfg(test)]
#[path = "host_authority/saved_mode_owner_tests.rs"]
mod saved_mode_owner_tests;

#[cfg(test)]
#[path = "host_authority/native_fade_owner_tests.rs"]
mod native_fade_owner_tests;

#[cfg(all(
    feature = "internal",
    feature = "game_client",
    not(target_arch = "wasm32")
))]
#[path = "host_authority/terminal_load_owner_probe.rs"]
pub(super) mod terminal_load_owner_probe;
