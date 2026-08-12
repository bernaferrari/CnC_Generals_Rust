#![allow(unused_imports, unused_variables, dead_code, non_snake_case)]
use super::*;
impl CnCGameEngine {
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
    pub(super) fn host_update_logic_frame(&mut self, dt: f32, budget: Option<usize>) {
        // Wave 584/870/908/919/923/929: single tick_logic_frame authority boundary + stamp snapshot.
        // Skip authority tick dual-write when host residual is paused (GameLogic also
        // no-ops is_paused; avoid the call entirely).
        if self.game_paused {
            let _ = (dt, budget);
            return;
        }
        let snap = self
            .game_logic
            .tick_logic_frame(dt, self.last_frame_timing.as_ref(), budget);
        self.host_stamp_sim_timing_from_snapshot(snap);
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
        let team_name = if let Some(pres) = self.last_presentation_frame.as_ref() {
            pres.local_team.get_name().to_string()
        } else {
            self.ui_local_player_team_name()
                .unwrap_or_else(|| "Neutral".to_string())
        };

        SaveGameInfo {
            filename: slot.to_string(),
            display_name: display_name.to_string(),
            description: description.to_string(),
            map_name,
            campaign_side: Some(team_name),
            mission_number: None,
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
        self.game_logic = logic;
    }

    pub(super) fn host_save_game_authority(
        &mut self,
        slot: &str,
        save_info: &SaveGameInfo,
    ) -> Result<(), String> {
        // Wave 928: single save authority boundary.
        self.save_file_manager
            .save_game(slot, &self.game_logic, save_info)
            .map_err(|e| format!("{e}"))
    }

    /// Wave 928: single load authority boundary.
    #[inline]
    pub(super) fn host_load_game_authority(&mut self, slot: &str) -> Result<SaveGameInfo, String> {
        // Wave 928: single load authority boundary.

        self.save_file_manager
            .load_game(slot, &mut self.game_logic)
            .map_err(|e| format!("{e}"))
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

        let save_info =
            self.build_save_info(slot, display_name, display_name, SaveFileType::Normal);

        if let Err(err) = self.host_save_game_authority(slot, &save_info) {
            warn!("Save failed for '{}': {}", slot, err);
        } else {
            info!("Saved game to slot '{}'", slot);
        }
    }

    /// Wave 611: via `host_load_game_from_ui`.
    pub(super) fn load_game_from_ui(&mut self, slot: &str) -> Result<(), String> {
        // Wave 611: thin wrapper — residual via host helper.
        self.host_load_game_from_ui(slot)
    }

    pub(super) fn host_load_game_from_ui(&mut self, slot: &str) -> Result<(), String> {
        // Wave 611: host residual helper.
        let slot = slot.trim();
        if slot.is_empty() {
            return Err("empty save slot".into());
        }

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
                info!(
                    "Loaded save '{}' (map={}, name={})",
                    slot, save_info.map_name, save_info.display_name
                );

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
                        save_info.map_name.as_str(),
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
                self.return_to_main_menu_after_match();
                Err(err.to_string())
            }
        }
    }
}
