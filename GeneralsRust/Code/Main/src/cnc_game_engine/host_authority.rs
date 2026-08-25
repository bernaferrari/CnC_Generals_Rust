#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;

/// A decoded save restored into a private host `GameLogic` plus the exact
/// process-global runtime bundle that map loading touched while staging it.
/// The bundle stays opaque until the single combined commit boundary below.
struct StagedRestoreWorld {
    logic: crate::game_logic::GameLogic,
    info: SaveGameInfo,
    runtime_world: gamelogic::runtime_world_transaction::StagedRuntimeWorld,
    shroud: gamelogic::system::shroud_manager::ShroudSnapshot,
    /// Renderer-owned Drawable state decoded with the logical world.  It is
    /// queued only after the staged world has committed, then validated
    /// against the first fresh presentation topology by RenderPipeline.
    client_drawables: crate::save_load::snapshot::ClientDrawableWorldSnapshot,
}

impl CnCGameEngine {
    /// Begin a new transient direct-visual world identity.
    ///
    /// This is deliberately separate from durable world/save identity.  A
    /// successful world replacement reconstructs client Drawables, while a
    /// failed staged/load attempt must leave the active associations valid.
    #[inline]
    pub(super) fn host_advance_direct_visual_world_epoch(&mut self) {
        let next = self.host_direct_visual_world_epoch.wrapping_add(1);
        self.host_direct_visual_world_epoch = if next == 0 { 1 } else { next };
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
            self.host_game_logic_mut().apply_session_control_op(
                crate::game_logic::SessionControlOp::OverrideWorldSize { width, height },
            );
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
    pub(super) fn host_update_shell_with_budget(&mut self, dt: f32, budget: usize) {
        // Wave 584/872/908/920/934: shell update via host-support authority.
        // Skip empty-dt authority shell tick dual-write.
        if dt <= 0.0 {
            let _ = budget;
            return;
        }
        let snap = match self.host_game_logic_mut().apply_host_support_op(
            crate::game_logic::HostSupportOp::UpdateShellWithBudget { dt, budget },
        ) {
            crate::game_logic::HostSupportResult::Snapshot(s) => s,
            _ => return,
        };
        self.host_stamp_sim_timing_from_snapshot(snap);
    }

    /// Wave 584: host logic-frame tick residual (timing/dt + optional headless budget).
    #[inline]
    pub(super) fn host_update_logic_frame(
        &mut self,
        dt: f32,
        budget: Option<usize>,
    ) -> crate::game_logic::SimTimingSnapshot {
        // Wave 584/870/908/919/923/929: single tick_logic_frame authority boundary + stamp snapshot.
        // Skip authority tick dual-write when host residual is paused (GameLogic also
        // no-ops is_paused; avoid the call entirely).
        if self.game_paused {
            let _ = (dt, budget);
            return crate::game_logic::SimTimingSnapshot::default();
        }
        // C++ GameEngine.cpp:749 reads TheGameLogic->isGamePaused() as the only
        // pause flag. A leftover GameLogic.is_paused=true with engine unpaused
        // pins logic_frame at 0 (hq-fx1z).
        if self.game_logic.is_paused() {
            self.game_logic.set_paused(false);
        }
        let snap = self
            .game_logic
            .tick_logic_frame(dt, self.last_frame_timing.as_ref(), budget);

        self.host_stamp_sim_timing_from_snapshot(snap);
        snap
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
        let _ = self.host_game_logic_mut().apply_host_support_op(
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
        self.host_match_in_replay = None;
        self.host_match_in_shell = None;
        self.host_match_local_team = None;
        self.host_match_diplomacy_players = None;
        self.host_match_known_template_names = None;
        self.host_match_unlocked_sciences = None;
        self.host_match_camera_follow_active = None;
        self.host_match_camera_follow_position = None;
        self.host_match_camera_follow_id = None;
        self.camera_follow_factor = -1.0;
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
        self.host_match_script_camera_max_height = None;
        self.host_match_script_camera_pitch = None;
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
        // Boot residual only when match outcome residual was never stamped.
        // Prefer host_match_over stamp path: if match not over and residual cold, empty.
        // Wave 895: fail-closed boot default (no dual-read). Match-over residual
        // and freeze already covered above; cold residual yields empty summary.
        let _ = winner;
        crate::game_logic::VictorySummary::default()
    }

    /// Wave 584: host match reset residual (GameLogic::reset boundary).
    #[inline]
    pub(super) fn host_reset_game_logic(&mut self) {
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
        self.invalidate_presentation_terrain_cache();
        self.host_clear_match_residuals();
        self.selected_objects.clear();
        self.last_presentation_frame = None;
        self.last_ui_state = None;
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
            let _ = self
                .game_logic
                .apply_object_lifecycle_op(crate::game_logic::ObjectLifecycleOp::Destroy { id });
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
            self.host_game_logic_mut().apply_object_lifecycle_op(
                crate::game_logic::ObjectLifecycleOp::ClearMovementPath { id },
            ),
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
        let r = match self.host_game_logic_mut().apply_object_lifecycle_op(
            crate::game_logic::ObjectLifecycleOp::AdjustGuardRadius { id, delta },
        ) {
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
            self.host_game_logic_mut().apply_object_lifecycle_op(
                crate::game_logic::ObjectLifecycleOp::ForceCompleteConstruction { id },
            ),
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
        self.host_game_logic_mut().apply_direct_player_order(order);
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
        let code = self
            .game_logic
            .legal_build_code_at_for_builder(team, loc, template, builder);
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
        let code = self
            .game_logic
            .legal_build_code_at_for_preview(team, loc, template, builder);
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
        // Wave 583/847/891/903/904/913/933: camera follow via session-control authority.
        // (no get_object dual-read). Skip authority write when residual already matches.
        let stamped_pos = id.and_then(|oid| {
            self.last_presentation_frame.as_ref().and_then(|pres| {
                pres.objects
                    .iter()
                    .find(|o| o.id == oid)
                    .map(|o| [o.position.x, o.position.y, o.position.z])
            })
        });
        if self.host_match_camera_follow_id != Some(id) {
            self.host_game_logic_mut().apply_session_control_op(
                crate::game_logic::SessionControlOp::SetCameraFollow { id },
            );
        }
        self.host_match_camera_follow_id = Some(id);
        self.host_match_camera_follow_active = Some(id.is_some());
        self.host_match_camera_follow_position = stamped_pos;
    }

    /// Wave 583: presentation freeze owns follow-active residual when installed.
    /// Boot residual without freeze uses host camera_follow_object_id probe.
    #[inline]
    pub(super) fn presentation_or_boot_camera_follow_active(&self) -> bool {
        // Wave 583/847: presentation freeze owns follow-active residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.camera_follow_position.is_some();
        }
        if let Some(v) = self.host_match_camera_follow_active {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        false
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
            self.host_game_logic_mut().apply_object_lifecycle_op(
                crate::game_logic::ObjectLifecycleOp::EnqueueProduction {
                    producer,
                    template_name,
                },
            ),
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
            .game_logic
            .apply_command_pipeline_op(crate::game_logic::CommandPipelineOp::ProcessIfNeeded);
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
        let res = self.host_game_logic_mut().apply_object_lifecycle_op(
            crate::game_logic::ObjectLifecycleOp::Create {
                name: name.to_string(),
                team,
                spawn_at,
            },
        );
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

    /// Wave 935: intentional full GameLogic replace boundary (map-load install).
    #[inline]
    pub(super) fn host_replace_game_logic(&mut self, logic: crate::game_logic::GameLogic) {
        // Do this before replacing GameLogic so the old world's active token,
        // frozen residual, modal WND, and popup-owned pause all disappear as
        // one boundary. Process-lifetime popup tokens then reject any late
        // callback which arrives after the new world is installed.
        #[cfg(feature = "game_client")]
        self.host_invalidate_active_popup_for_world_boundary();

        self.game_logic = logic;
        if let Some(shadow) = self.gameworld_shadow.as_mut() {
            shadow.reset_for_world_boundary();
            // The replacement is already a complete host world.  Seed the
            // fresh GameWorld before any boot/menu probe can observe it; the
            // staged-restore and successful map-load boundaries use the same
            // reset-then-sync ordering.
            shadow.sync_from_host(&self.game_logic);
        }
        self.host_advance_direct_visual_world_epoch();
        #[cfg(feature = "game_client")]
        self.game_client.invalidate_presentation_drawable_world();
        self.render_pipeline.invalidate_world_visual_state();
        self.invalidate_presentation_terrain_cache();
    }

    /// Install a fully staged save world as one no-fail host boundary.
    ///
    /// Unlike `host_replace_game_logic`, this also owns the legacy singleton
    /// bundle mutated by map loading.  The order is intentional: invalidate
    /// old UI/log ownership, install candidate globals, install matching host
    /// logic, then run TeamFactory post-unlock callbacks against that complete
    /// candidate world before rebuilding the shadow from host authority.
    fn host_replace_staged_restore_world(&mut self, staged: StagedRestoreWorld) {
        #[cfg(feature = "game_client")]
        self.host_invalidate_active_popup_for_world_boundary();

        crate::game_logic::staged_world_effects::discard_live_for_world_replace();

        let StagedRestoreWorld {
            logic,
            info: _,
            runtime_world,
            shroud: _,
            client_drawables,
        } = staged;
        let deferred_effects = runtime_world.install_globals();
        let old_logic = std::mem::replace(&mut self.game_logic, logic);

        // C++ `createInactiveTeam` executes `ExecuteActionsOnCreate`
        // synchronously.  Staging deferred the guard-drop callbacks solely to
        // avoid targeting the old world; now that both halves are committed,
        // execute them in original queue order before shadow reconstruction.
        deferred_effects.execute_after_logic_commit();
        drop(old_logic);

        self.host_advance_direct_visual_world_epoch();
        #[cfg(feature = "game_client")]
        self.game_client.invalidate_presentation_drawable_world();
        self.render_pipeline.invalidate_world_visual_state();
        // Keep the client companion staged until the first complete frozen
        // frame after the successful world replacement.  `set_presentation_frame`
        // performs source-identity validation before collection and removes
        // each candidate once, so a failed/missing W3D asset cannot replay a
        // stale timeline on a later frame.
        self.render_pipeline
            .queue_client_drawable_restore(client_drawables);
        self.invalidate_presentation_terrain_cache();
        if let Some(camera) = crate::save_load::snapshot::take_pending_camera() {
            self.camera_position =
                glam::Vec3::new(camera.position[0], camera.position[1], camera.position[2]);
            self.camera_target =
                glam::Vec3::new(camera.target[0], camera.target[1], camera.target[2]);
            if camera.zoom.is_finite() && camera.zoom > 0.05 {
                self.camera_zoom = camera.zoom;
            }
            self.view_matrix =
                glam::Mat4::look_at_rh(self.camera_position, self.camera_target, glam::Vec3::Y);
            self.sync_orbit_from_camera_transform();
        }
        if let Some(groups) = crate::save_load::snapshot::take_pending_control_groups() {
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
        if let Some(shadow) = self.gameworld_shadow.as_mut() {
            shadow.reset_for_world_boundary();
            shadow.sync_from_host(&self.game_logic);
        }
    }

    pub(super) fn host_save_game_authority(
        &mut self,
        slot: &str,
        save_info: &SaveGameInfo,
    ) -> Result<(), String> {
        // Wave 928: single save authority boundary.
        // Capture the renderer-owned Drawable companion at the same host
        // authority boundary as the logical snapshot.  The pipeline method
        // is renderer-local and immutable here; an empty/unresolved cache is
        // intentionally a valid fail-closed companion.
        let offset = self.camera_position - self.camera_target;
        crate::save_load::snapshot::set_pending_camera(crate::save_load::snapshot::CameraPersist {
            angle: offset.x.atan2(offset.z),
            position: [
                self.camera_position.x,
                self.camera_position.y,
                self.camera_position.z,
            ],
            target: [
                self.camera_target.x,
                self.camera_target.y,
                self.camera_target.z,
            ],
            zoom: self.camera_zoom,
        });
        crate::save_load::snapshot::set_pending_control_groups(
            self.control_groups
                .iter()
                .map(|(slot, ids)| (*slot, ids.iter().map(|id| id.0).collect()))
                .collect(),
        );
        let client_drawables = self.render_pipeline.capture_client_drawable_snapshot();
        let save_path = self.save_file_manager.get_save_path(slot);
        let result = self
            .save_file_manager
            .save_game_with_client_drawable_snapshot(
                slot,
                &self.game_logic,
                client_drawables,
                save_info,
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
    ) -> Result<crate::game_logic::GameMode, String> {
        use crate::game_logic::GameMode;

        if matches!(
            active_mode,
            GameMode::Multiplayer | GameMode::Internet | GameMode::Lan | GameMode::Replay
        ) {
            return Err("network and replay save restore is deferred".to_string());
        }

        if let Some(saved) = crate::save_load::take_loaded_game_state_map_mode()
            .and_then(crate::save_load::live_game_mode_from_cpp)
        {
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
        template_catalog: &std::collections::HashMap<String, crate::game_logic::ThingTemplate>,
    ) -> Result<StagedRestoreWorld, String> {
        let (snapshot, save_info) = save_file_manager
            .load_game_snapshot(slot)
            .map_err(|err| format!("{err}"))?;

        Self::stage_decoded_saved_world_for_restore(
            &snapshot,
            save_info,
            slot,
            active_mode,
            template_catalog,
            |snapshot, staged| {
                save_file_manager
                    .restore_game_snapshot(snapshot, staged)
                    .map_err(|err| format!("{err}"))
            },
        )
    }

    /// Stage a decoded save behind a raw singleton/TLS transaction boundary.
    ///
    /// `restore_snapshot` is deliberately injected only at this private seam:
    /// the focused test can force an error *after* map loading has mutated all
    /// candidate globals, proving rollback preserves the active world.  It is
    /// not a production failpoint and does not alter save schema/versioning.
    fn stage_decoded_saved_world_for_restore<F>(
        snapshot: &crate::save_load::WorldSnapshot,
        save_info: SaveGameInfo,
        slot: &str,
        active_mode: crate::game_logic::GameMode,
        template_catalog: &std::collections::HashMap<String, crate::game_logic::ThingTemplate>,
        restore_snapshot: F,
    ) -> Result<StagedRestoreWorld, String>
    where
        F: FnOnce(
            &crate::save_load::WorldSnapshot,
            &mut crate::game_logic::GameLogic,
        ) -> Result<(), String>,
    {
        let saved_map = save_info.map_name.trim();
        if saved_map.is_empty() || saved_map == "-" || saved_map.eq_ignore_ascii_case("unknown") {
            return Err(format!(
                "save '{slot}' has no usable saved map identity ({:?})",
                save_info.map_name
            ));
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

        let mode = Self::offline_restore_mode_for_save(active_mode, &save_info)?;

        // Map loading and snapshot restore write legacy singleton state (AI,
        // terrain, sides, players, teams, script engine, shroud, named/area
        // trackers).  Move that state out before constructing the candidate so
        // failure cannot poison the still-playable host match.  Main's TLS
        // logs/shadow bind path need the matching take/restore scope around
        // the same work.
        let staged_effects = crate::game_logic::staged_world_effects::StagedWorldEffects::enter();
        let runtime_stage = gamelogic::runtime_world_transaction::RuntimeWorldStage::begin();
        let mut staged = crate::game_logic::GameLogic::new();
        staged.start_new_game(mode);
        // Map object restoration needs the live INI/template catalog.  Keep
        // custom/mod templates from the source match while retaining the fresh
        // world's standard startup catalog.
        staged.templates.extend(template_catalog.clone());

        // Preserve the saved logical identity in `GameLogic::map_name`; the
        // prior resolution above only proves that this exact identity maps to
        // a real on-disk retail file rather than to a development fallback.
        if !staged.load_map(saved_map) {
            return Err(format!(
                "failed to load saved map '{saved_map}' from '{resolved_map_name}'"
            ));
        }
        if !staged.isInGame() || staged.get_current_map_name() != saved_map {
            return Err(format!(
                "saved map '{saved_map}' did not become the active staged map"
            ));
        }

        restore_snapshot(snapshot, &mut staged)?;

        // Snapshot restoration must not turn a successful map load into a
        // false in-game claim.  It currently restores terrain/objects but not
        // GameMode/map identity, so these checks also guard future changes.
        if !staged.isInGame() || staged.get_current_map_name() != saved_map {
            return Err(format!(
                "saved map '{saved_map}' was lost while restoring the snapshot"
            ));
        }

        // Restore the active singleton/TLS state before exposing the candidate
        // to the caller.  The returned opaque bundle is installed only by the
        // no-fail combined commit below.
        let runtime_world = runtime_stage.finish_and_restore_live();
        staged_effects.finish_and_restore_live();
        Ok(StagedRestoreWorld {
            logic: staged,
            info: save_info,
            runtime_world,
            shroud: snapshot.shroud.clone(),
            client_drawables: snapshot.client_drawables.clone(),
        })
    }

    /// Wave 928: single load authority boundary.
    pub(super) fn host_load_game_authority(&mut self, slot: &str) -> Result<SaveGameInfo, String> {
        let save_path = self.save_file_manager.get_save_path(slot);
        let result = self.host_try_load_game_authority(slot);
        // C++ GameState::loadGame (GameState.cpp:695-712): MessageBoxOk
        // GUI:Error / GUI:ErrorLoadingGame with the filepath on xfer or
        // loadPostProcess failure. Missing, truncated, and non-host
        // CHUNK_GameLogic saves must not fail silently.
        Self::surface_load_game_ui_feedback(&save_path, &result);
        result
    }

    fn host_try_load_game_authority(&mut self, slot: &str) -> Result<SaveGameInfo, String> {
        let save_info = self
            .save_file_manager
            .get_save_info(slot)
            .map_err(|err| format!("{err}"))?;
        if save_info.save_type == SaveFileType::Mission {
            return self.host_restart_mission_from_save(slot, save_info);
        }

        // Keep the current world untouched until the save metadata, exact map,
        // and snapshot all restore successfully in a staging world.
        // CHUNK_Campaign is stashed during decode; apply only after commit so
        // a failed load cannot leave half-applied campaign on the live match
        // (C++ GameState.cpp:695-712 clearGameData).
        let prior_campaign = crate::save_load::capture_live_campaign_state();
        let active_mode = self.game_logic.game_mode();
        let template_catalog = self.game_logic.templates.clone();
        let staged = match Self::stage_saved_world_for_restore(
            &mut self.save_file_manager,
            slot,
            active_mode,
            &template_catalog,
        ) {
            Ok(staged) => staged,
            Err(err) => {
                crate::save_load::rollback_campaign_after_failed_load(prior_campaign);
                return Err(err);
            }
        };

        crate::save_load::commit_stashed_campaign_state();
        let save_info = staged.info.clone();
        self.host_replace_staged_restore_world(staged);
        info!(
            "Game loaded successfully from slot '{}' on map '{}'",
            slot,
            self.game_logic.get_current_map_name()
        );
        Ok(save_info)
    }

    /// C++ `GameState::loadGame` user feedback (`GameState.cpp:695-712`).
    fn surface_load_game_ui_feedback(
        save_path: &std::path::Path,
        result: &Result<SaveGameInfo, String>,
    ) {
        #[cfg(feature = "game_client")]
        if result.is_err() {
            let filepath = save_path.display().to_string();
            let (title, body) = crate::save_load::format_error_loading_game(&filepath);
            let _ = game_client::gui::message_box_ok(&title, &body, None);
        }
        #[cfg(not(feature = "game_client"))]
        let _ = (save_path, result);
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
        game_engine::common::random_value::init_random_with_seed(0);
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

        crate::skirmish_config::apply_skirmish_config(&mut self.game_logic, config)
    }

    /// Wave 928: runtime-host GameWorld authority probe boundary.
    #[inline]
    pub(super) fn host_simulate_gameworld_authority_probe(&mut self) -> bool {
        // Wave 928: runtime-host GameWorld authority probe boundary.

        crate::gameworld_shadow::simulate_gameworld_authority_probe(&mut self.game_logic)
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
                    self.ensure_presentation_env_seeded();
                    Self::apply_heightmap_hint(&mut self.render_pipeline);
                    Self::apply_skybox_hint(&mut self.render_pipeline);
                    self.ensure_presentation_env_seeded();
                    Self::sync_render_terrain_visual(
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
                    self.ensure_presentation_env_seeded();
                    Self::apply_map_lighting(&mut self.graphics_system, &mut self.render_pipeline);
                }

                // Seed presentation before first InGame render (units/HUD identity).
                self.seed_presentation_after_match_start();
                self.transition_to_state(GameState::InGame);
                Ok(())
            }
            Err(err) => {
                warn!("Load failed for '{}': {}", slot, err);
                // Do not reset GameLogic, presentation, or renderer state:
                // `stage_saved_world_for_restore` has not installed anything
                // on error. Transitioning out of Loading only hides its shell
                // overlay; it does not invalidate the still-playable match.
                self.transition_to_state(Self::staged_load_failure_return_state(prior_state));
                Err(err.to_string())
            }
        }
    }
}

#[cfg(test)]
mod staged_restore_tests {
    use super::*;
    use crate::ai::AIDifficulty;
    use crate::game_logic::{GameLogic, GameMode, Player, Team};
    use crate::save_load::{GameDifficulty, SaveFileManager, SaveFileType};
    use std::time::SystemTime;

    /// Keeps this test's thread-local presentation queues independent from
    /// whatever a neighboring test had recorded before it began.
    struct WorldStageLogRestore {
        state: Option<crate::game_logic::staged_world_effects::WorldStageEffectsState>,
    }

    impl WorldStageLogRestore {
        fn take() -> Self {
            Self {
                state: Some(
                    crate::game_logic::staged_world_effects::WorldStageEffectsState::take_all_for_test(),
                ),
            }
        }
    }

    impl Drop for WorldStageLogRestore {
        fn drop(&mut self) {
            if let Some(state) = self.state.take() {
                drop(
                    crate::game_logic::staged_world_effects::WorldStageEffectsState::replace_all_for_test(state),
                );
            }
        }
    }

    /// Seed the full load-map/snapshot-restore queue closure.  The staged
    /// candidate uses a distinct object/label, so equality after its forced
    /// failure proves both pending queues and presentation last-drain state
    /// stayed attached to the live world.
    fn record_all_stage_queue_events(object: crate::game_logic::ObjectId, label: &str) {
        crate::game_logic::host_spawn_log::record(object, label.to_string(), 7, [1.0, 2.0, 3.0]);
        crate::game_logic::host_move_log::record(object, Some([4.0, 5.0, 6.0]));
        let _ = crate::game_logic::host_move_log::drain();
        crate::game_logic::host_move_log::record(object, None);
        crate::game_logic::host_ground_height_log::record(object, 7.0, true);
        crate::game_logic::host_model_mesh_log::record(object, label, 1.25);
        crate::game_logic::host_kind_of_log::record(object, 0x55aa_1234);
        crate::game_logic::host_identity_log::record(
            object,
            label.to_string(),
            [0.25, 0.5, 0.75, 1.0],
        );
        crate::game_logic::host_movement_log::record(
            object,
            glam::Vec3::new(1.0, 2.0, 3.0),
            4.0,
            0,
            &[glam::Vec3::new(5.0, 6.0, 7.0)],
            false,
            0x11,
            false,
            false,
            false,
            true,
            8,
            9,
            10.0,
            11,
            false,
            Some(12),
            Some(13),
        );
        crate::game_logic::host_demo_mine_cheer_log::record(object, true, true, 14.0);
        crate::game_logic::host_detector_log::record(object, true, 15.0, 16);
        crate::game_logic::host_overlord_log::record(object, true, false, 17, false);
        crate::game_logic::host_stealth_flags_log::record(
            crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent {
                object,
                innate_stealth: true,
                stealth_breaks_on_attack: false,
                stealth_breaks_on_move: true,
                is_tunnel_network: false,
                passengers_allowed_to_fire: true,
            },
        );
        crate::game_logic::host_hive_log::record(object, 18, 19.0);
        crate::game_logic::host_weapon_set_log::record(object, true, false);
        crate::game_logic::host_contain_capacity_log::record(object, 20, 21);
        crate::game_logic::host_status_log::record_selected(object, true);
        crate::game_logic::host_ai_attitude_log::record(object, 2);
        crate::game_logic::host_special_power_log::record(object, false, 22.0, 23.0, true);
        crate::game_logic::host_player_cooldown_log::record(
            24,
            vec![(format!("{label}-cooldown"), 25.0)],
        );
        crate::game_logic::host_stored_supplies_log::record(object, 26);
        crate::game_logic::host_contain_log::record_contained_by(
            object,
            Some(crate::game_logic::ObjectId(0x00ff_ee13)),
        );
        crate::game_logic::host_ai_state_log::record(object, 12);
        crate::game_logic::host_ai_mood_log::record(object, 27, 28, true, label.to_string());
        crate::game_logic::host_locomotor_log::record(
            object, false, false, true, false, true, false, 29.0, 30.0, true, false, 31.0, 32.0,
            33.0, 34, 35, 36.0, -1,
        );
        crate::game_logic::host_combat_attack_log::record(
            object,
            37,
            38.0,
            39,
            40,
            41,
            42,
            43,
            true,
            Some([44.0, 45.0, 46.0]),
            47,
            48.0,
        );
        crate::game_logic::host_attack_log::record(
            object,
            Some(crate::game_logic::ObjectId(0x00ff_ee14)),
        );
        let _ = crate::game_logic::host_attack_log::drain();
        crate::game_logic::host_attack_log::record(object, None);
        crate::game_logic::host_target_location_log::record(object, Some([49.0, 50.0, 51.0]));
        crate::game_logic::host_ai_decision_log::record_set_state(object, 14);
        crate::game_logic::host_command_set_log::record(
            object,
            Some(format!("{label}-command-set")),
        );
        crate::game_logic::host_continuous_fire_log::record(object, 52, 53, 54);
        crate::game_logic::host_player_meta_log::record_sciences(55, [format!("{label}-science")]);
        crate::game_logic::host_player_progress_log::record(56, 57, 58, 59, 60.0);
        crate::game_logic::host_veterancy_log::record(object, 3);
        crate::game_logic::host_max_health_log::record(object, 61.0);
        crate::game_logic::host_experience_log::record(object, 62.0);
        crate::game_logic::host_building_type_log::record(object, true, 12);
        crate::game_logic::host_physics_motive_log::record(
            object,
            63,
            64.0,
            [65.0, 66.0, 67.0],
            68.0,
            69.0,
            70.0,
            true,
            71,
            false,
            72,
            73.0,
            74.0,
            true,
            75.0,
            76.0,
            Some([77.0, 78.0, 79.0]),
            Some(80),
            true,
            Some(81),
            Some(82),
        );
    }

    fn retail_map_path_for_test() -> Option<String> {
        // Keep this test portable for source-only CI while exercising a real
        // extracted retail map whenever `windows_game` is available.  Return
        // the original logical name so the test also proves the map resolver
        // can load the saved identity rather than only an absolute test path.
        for candidate in ["Lone Eagle", "ForgottenForestZH", "GC_ChinaBoss"] {
            if crate::game_logic::script_loader::find_map_file(candidate).is_some() {
                return Some(candidate.to_string());
            }
        }
        None
    }

    fn save_info(slot: &str, map_name: String) -> SaveGameInfo {
        SaveGameInfo {
            filename: slot.to_string(),
            display_name: slot.to_string(),
            description: "staged restore test".to_string(),
            map_name,
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: std::time::Duration::ZERO,
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        }
    }

    #[test]
    fn staged_load_failure_keeps_only_an_active_match_state() {
        assert_eq!(
            CnCGameEngine::staged_load_failure_return_state(GameState::InGame),
            GameState::InGame
        );
        assert_eq!(
            CnCGameEngine::staged_load_failure_return_state(GameState::Paused),
            GameState::Paused
        );
        for shell_state in [
            GameState::Initializing,
            GameState::Menu,
            GameState::Loading,
            GameState::Victory,
            GameState::Defeat,
            GameState::Exiting,
        ] {
            assert_eq!(
                CnCGameEngine::staged_load_failure_return_state(shell_state),
                GameState::Menu
            );
        }
    }

    #[test]
    fn staged_load_error_branch_preserves_live_world_contract() {
        let source = include_str!("host_authority.rs");
        let start = source
            .find("pub(super) fn host_load_game_from_ui")
            .expect("load UI authority");
        let body = &source[start..];
        let error = &body[body.find("Err(err) =>").expect("load error branch")..];
        let error = &error[..error
            .find("\n            }\n        }\n    }\n}")
            .unwrap_or(error.len())];
        assert!(error.contains("Self::staged_load_failure_return_state(prior_state)"));
        assert!(!error.contains("return_to_main_menu_after_match"));
        assert!(!error.contains("host_clear_match_residuals"));
        assert!(!error.contains("invalidate_world_visual_state"));
        assert!(!error.contains("invalidate_presentation_drawable_world"));
    }

    #[test]
    fn load_failure_surfaces_gui_error_loading_game() {
        let source = include_str!("host_authority.rs");
        let start = source
            .find("fn surface_load_game_ui_feedback")
            .expect("load error UI");
        let body = &source[start..];
        assert!(
            body.contains("GUI:ErrorLoadingGame") || body.contains("format_error_loading_game")
        );
        assert!(body.contains("message_box_ok"));
        let authority = source
            .find("pub(super) fn host_load_game_authority")
            .expect("load authority");
        let authority_body = &source[authority..];
        assert!(authority_body.contains("surface_load_game_ui_feedback"));
    }

    #[test]
    fn offline_restore_prefers_saved_skirmish_over_live_campaign() {
        crate::save_load::store_loaded_game_state_map_mode_for_test(Some(2));
        let mode = CnCGameEngine::offline_restore_mode_for_save(
            GameMode::SinglePlayer,
            &save_info("slot", "Maps\\Alpine.map".into()),
        )
        .expect("saved skirmish must restore");
        assert_eq!(mode, GameMode::Skirmish);
        crate::save_load::store_loaded_game_state_map_mode_for_test(Some(0));
        let mode = CnCGameEngine::offline_restore_mode_for_save(
            GameMode::Skirmish,
            &save_info("slot", "Maps\\Alpine.map".into()),
        )
        .expect("saved campaign must restore");
        assert_eq!(mode, GameMode::SinglePlayer);
        crate::save_load::store_loaded_game_state_map_mode_for_test(None);
    }

    #[test]
    fn staged_restore_rejects_unavailable_map_before_snapshot_restore() {
        let temp = tempfile::tempdir().expect("temporary save directory");
        let mut saves = SaveFileManager::with_save_directory(temp.path());
        saves.init().expect("initialize temporary save directory");

        // This deliberately needs no retail assets: map identity is validated
        // before a staging world is started or any snapshot state is restored.
        let source = GameLogic::new();
        let catalog = source.templates.clone();
        saves
            .save_game(
                "missing_map",
                &source,
                &save_info("missing_map", "Not A Retail Map".to_string()),
            )
            .expect("write invalid-map save metadata");

        let err = match CnCGameEngine::stage_saved_world_for_restore(
            &mut saves,
            "missing_map",
            GameMode::Shell,
            &catalog,
        ) {
            Ok(_) => panic!("missing saved map must reject before a false InGame restore"),
            Err(err) => err,
        };
        assert!(
            err.contains("not available on disk"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn staged_restore_requires_the_saved_retail_map_before_snapshot_restore() {
        let Some(map_name) = retail_map_path_for_test() else {
            eprintln!("retail maps unavailable — skip staged restore map test");
            return;
        };

        let temp = tempfile::tempdir().expect("temporary save directory");
        let mut saves = SaveFileManager::with_save_directory(temp.path());
        saves.init().expect("initialize temporary save directory");

        let mut source = GameLogic::new();
        source.start_new_game(GameMode::Skirmish);
        assert!(source.load_map(&map_name), "load source retail map");
        // Seed a save-time FOW state that differs from map-start reveals. The
        // staged candidate must carry these raw counters and the pending
        // expiry queue rather than retaining its map-load singleton state.
        let expected_shroud = {
            let mut shroud = gamelogic::system::shroud_manager::get_shroud_manager()
                .lock()
                .expect("source shroud lock");
            shroud.do_shroud_reveal(&glam::Vec3::ZERO, 75.0, 1);
            shroud.queue_undo_shroud_reveal(&glam::Vec3::ZERO, 75.0, 1, 41, 321);
            shroud.snapshot_state()
        };
        // Make the AI fixture explicit: map files do not all provide a
        // configured skirmish roster, while save/load must preserve one when
        // the source match has it.
        source.add_player(Player::new(70, Team::USA, "Human", true));
        source.add_player(Player::new(71, Team::China, "Computer", false));
        source.add_ai_opponent(71, Team::China, AIDifficulty::Hard);
        source.set_ai_active(71, false);
        source.relocate_host_ai_base(71, glam::Vec3::new(47.0, 0.0, -31.0));
        source.set_current_frame(321);
        let catalog = source.templates.clone();
        assert_eq!(source.host_ai_difficulty(71), Some(AIDifficulty::Hard));
        assert!(!source.is_host_ai_active(71));
        let client_drawables = crate::save_load::ClientDrawableWorldSnapshot {
            drawables: vec![crate::save_load::ClientDrawableStateSnapshot {
                object_id: 71,
                draw_module_index: 2,
                source_template_name: "SavedDrawableSource".to_string(),
                model_key: "SavedDrawableModel".to_string(),
                selected_condition_state_index: 5,
                last_seen_weapon_discharge_sequence: 19,
                ..Default::default()
            }],
        };
        saves
            .save_game_with_client_drawable_snapshot(
                "valid_map",
                &source,
                client_drawables.clone(),
                &save_info("valid_map", map_name.clone()),
            )
            .expect("write valid staged restore save");

        let restored = CnCGameEngine::stage_saved_world_for_restore(
            &mut saves,
            "valid_map",
            GameMode::Shell,
            &catalog,
        )
        .expect("saved map should load before restore");
        assert_eq!(restored.info.map_name, map_name);
        assert_eq!(
            restored.shroud, expected_shroud,
            "staged restore must carry exact saved FOW rather than map-start reveals"
        );
        assert_eq!(
            restored.client_drawables, client_drawables,
            "staged restore must carry the renderer companion to the commit boundary"
        );
        assert!(restored.logic.isInGame());
        assert_eq!(restored.logic.get_current_map_name(), map_name);
        assert_eq!(restored.logic.get_current_frame(), 321);
        assert_eq!(restored.logic.host_ai_player_count(), 1);
        assert_eq!(
            restored.logic.host_ai_difficulty(71),
            Some(AIDifficulty::Hard)
        );
        assert!(!restored.logic.is_host_ai_active(71));
        let restored_ai = restored
            .logic
            .snapshot_host_ai_players_for_save()
            .into_iter()
            .find(|ai| ai.player_id == 71)
            .expect("registered host AI must survive the on-disk restore");
        assert_eq!(
            restored_ai.base_center,
            Some(glam::Vec3::new(47.0, 0.0, -31.0))
        );
    }

    #[test]
    fn forced_post_restore_failure_restores_globals_and_tls_effect_queues() {
        let Some(map_name) = retail_map_path_for_test() else {
            eprintln!("retail maps unavailable — skip staged rollback transaction test");
            return;
        };

        let temp = tempfile::tempdir().expect("temporary save directory");
        let mut saves = SaveFileManager::with_save_directory(temp.path());
        saves.init().expect("initialize temporary save directory");

        let mut source = GameLogic::new();
        source.start_new_game(GameMode::Skirmish);
        assert!(source.load_map(&map_name), "load source retail map");
        // The forced error runs after snapshot restoration, which initializes
        // both legacy AI singletons.  Seed distinct live allocator/manager
        // contents first: rollback must recover them rather than merely leave
        // a valid-looking empty AI system behind.
        source.add_player(Player::new(0, Team::USA, "Human", true));
        source.add_player(Player::new(1, Team::China, "Computer", false));
        source.setup_skirmish_ai(0);
        let (first_live_ai_group_id, second_live_ai_group_id) = {
            let mut ai = gamelogic::ai::THE_AI.write().expect("lock live legacy AI");
            let first = ai.create_group();
            let first = first.read().expect("read first live AI group").get_id();
            let second = ai.create_group();
            let second = second.read().expect("read second live AI group").get_id();
            (first, second)
        };
        let live_integration_group_count =
            gamelogic::ai::integration::with_ai_integration_mut(|manager| {
                manager
                    .create_unit_group("stage-rollback-sentinel".to_string(), 1)
                    .expect("create live integration AI group");
                manager.get_unit_group_count()
            })
            .expect("live AI integration manager initialized");
        let catalog = source.templates.clone();
        saves
            .save_game(
                "forced_stage_failure",
                &source,
                &save_info("forced_stage_failure", map_name.clone()),
            )
            .expect("write forced-failure staged restore save");
        let (snapshot, info) = saves
            .load_game_snapshot("forced_stage_failure")
            .expect("decode forced-failure staged restore save");

        // Capture every singleton family this transaction owns.  The candidate
        // map load is deliberately allowed to mutate them before the injected
        // error; equality below proves the active values were restored, not
        // merely cleared to defaults.
        let global_probe = || {
            let terrain = gamelogic::terrain::get_terrain_logic()
                .read()
                .map(|terrain| terrain.get_source_filename().to_string())
                .unwrap_or_default();
            let players = gamelogic::player::ThePlayerList()
                .read()
                .map(|players| (players.get_player_count(), players.get_local_player_index()))
                .unwrap_or_default();
            let teams = gamelogic::team::get_team_factory()
                .lock()
                .map(|teams| {
                    (
                        teams.get_all_teams().len(),
                        teams.get_next_team_id(),
                        teams.get_next_team_prototype_id(),
                    )
                })
                .unwrap_or_default();
            let sides = gamelogic::sides_list::get_sides_list()
                .read()
                .map(|sides| (sides.get_num_sides(), sides.get_num_teams()))
                .unwrap_or_default();
            let shroud = gamelogic::system::shroud_manager::get_shroud_manager()
                .lock()
                .map(|shroud| shroud.snapshot_state())
                .unwrap_or_default();
            let mut named = gamelogic::scripting::engine::get_named_object_tracker()
                .get_all_named_objects()
                .unwrap_or_default();
            named.sort();
            let mut areas = gamelogic::scripting::engine::get_area_tracker().all_area_aabbs();
            areas.sort_by(|left, right| left.0.cmp(&right.0));
            let script_engine_present = gamelogic::scripting::engine::get_script_engine()
                .read()
                .map(|engine| engine.is_some())
                .unwrap_or(false);
            let legacy_ai_groups = gamelogic::ai::THE_AI
                .read()
                .map(|ai| {
                    (
                        ai.get_group_by_id(first_live_ai_group_id).is_some(),
                        ai.get_group_by_id(second_live_ai_group_id).is_some(),
                    )
                })
                .unwrap_or_default();
            let integration_group_count =
                gamelogic::ai::integration::with_ai_integration(|manager| {
                    manager.get_unit_group_count()
                })
                .unwrap_or_default();
            (
                terrain,
                players,
                teams,
                sides,
                shroud,
                named,
                areas,
                script_engine_present,
                legacy_ai_groups,
                integration_group_count,
            )
        };
        let before = global_probe();

        let _log_restore = WorldStageLogRestore::take();
        let sentinel = crate::game_logic::ObjectId(0x00ff_ee11);
        record_all_stage_queue_events(sentinel, "stage-rollback-sentinel");
        let expected =
            crate::game_logic::staged_world_effects::WorldStageEffectsState::take_all_for_test();
        drop(
            crate::game_logic::staged_world_effects::WorldStageEffectsState::replace_all_for_test(
                expected.clone(),
            ),
        );

        let err = match CnCGameEngine::stage_decoded_saved_world_for_restore(
            &snapshot,
            info,
            "forced_stage_failure",
            GameMode::Skirmish,
            &catalog,
            |_snapshot, staged| {
                assert!(staged.isInGame(), "failure is injected after map load");
                assert!(gamelogic::runtime_world_transaction::world_runtime_staging_active());
                assert!(crate::game_logic::staged_world_effects::world_stage_effects_active());
                // The map candidate has already emitted its normal object
                // creation events.  Emit every conditional residual family as
                // well, so rollback proves the raw boundary discards both.
                record_all_stage_queue_events(
                    crate::game_logic::ObjectId(0x00ff_ee12),
                    "staged-candidate-only",
                );
                Err("forced failure after staged snapshot restore".to_string())
            },
        ) {
            Ok(_) => panic!("injected post-restore failure must reject the candidate world"),
            Err(err) => err,
        };
        assert!(err.contains("forced failure after staged snapshot restore"));

        assert_eq!(global_probe(), before, "rollback must restore live globals");
        let resumed_ai_group_id = {
            let mut ai = gamelogic::ai::THE_AI
                .write()
                .expect("lock restored legacy AI");
            let resumed = ai.create_group();
            let resumed_id = resumed
                .read()
                .expect("read post-rollback legacy AI group")
                .get_id();
            resumed_id
        };
        assert_eq!(
            resumed_ai_group_id,
            second_live_ai_group_id.wrapping_add(1),
            "rollback must preserve the legacy AI group-ID allocator"
        );
        assert_eq!(
            gamelogic::ai::integration::with_ai_integration(|manager| {
                manager.get_unit_group_count()
            }),
            Some(live_integration_group_count),
            "rollback must preserve the live AI integration manager"
        );
        let actual =
            crate::game_logic::staged_world_effects::WorldStageEffectsState::take_all_for_test();
        assert_eq!(
            actual, expected,
            "all map/snapshot-reachable TLS queues and last-drain state must survive rollback"
        );
    }

    #[test]
    fn staged_commit_runs_team_effects_only_after_logic_install() {
        let source = include_str!("host_authority.rs");
        let start = source
            .find("fn host_replace_staged_restore_world")
            .expect("combined staged commit boundary");
        let body = &source[start
            ..source[start..]
                .find("\n    pub(super) fn host_save_game_authority")
                .map(|end| start + end)
                .expect("end of combined staged commit boundary")];
        let globals = body
            .find("runtime_world.install_globals()")
            .expect("global install");
        let logic = body
            .find("std::mem::replace(&mut self.game_logic, logic)")
            .expect("host logic install");
        let effects = body
            .find("deferred_effects.execute_after_logic_commit()")
            .expect("deferred team effects");
        let shadow = body.find("shadow.sync_from_host").expect("shadow rebuild");
        assert!(globals < logic && logic < effects && effects < shadow);
    }
}
