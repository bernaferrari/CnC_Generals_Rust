use super::*;

impl CnCGameEngine {
    /// Prefer presentation-frozen game mode when a frame is installed.
    /// Wave 609: via `host_presentation_or_live_game_mode`.
    pub(in crate::cnc_game_engine) fn presentation_or_live_game_mode(&self) -> GameMode {
        // Wave 609: thin wrapper — UI/presentation residual via host helper.
        self.host_presentation_or_live_game_mode()
    }

    /// Prefer presentation-frozen game mode when a frame is installed.
    pub(in crate::cnc_game_engine) fn host_presentation_or_live_game_mode(&self) -> GameMode {
        // Wave 609/842: host UI/presentation residual helper.
        // Prefer freeze, then host-owned match mode residual, then boot GameLogic.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.game_mode;
        }
        if let Some(mode) = self.host_match_game_mode {
            return mode;
        }
        // Wave 898: fail-closed boot default (menu/shell).
        GameMode::Shell
    }

    /// Wave 550: presentation freeze owns visual speed residual when installed.
    /// Boot residual without freeze uses host GameLogic probe.
    #[inline]

    /// Wave 844: refresh host-owned sim timing residuals from Main GameLogic.
    /// Called after match load and each presentation finalize (stamp only — consumers
    /// prefer freeze, then these residuals, then boot live probes).
    #[inline]

    /// Wave 848/853/857: single stamp-phase object scan for alive-set, local train
    /// producers, and special-power-ready ids. Prefer presentation freeze when
    /// installed (no host dual-read); otherwise one `get_objects` pass fills all
    /// residual families.
    pub(in crate::cnc_game_engine) fn host_refresh_local_train_producer_residuals(&mut self) {
        let team = self
            .host_match_local_team
            .or_else(|| {
                self.last_presentation_frame
                    .as_ref()
                    .map(|f| f.local_team())
            })
            .unwrap_or(crate::game_logic::Team::USA);
        let mut barracks = Vec::new();
        let mut any = Vec::new();
        let mut unfinished = Vec::new();
        let mut sample = None;
        let mut alive = std::collections::HashSet::new();
        let mut special_ready = std::collections::HashSet::new();

        if let Some(pres) = self.last_presentation_frame.as_ref() {
            for o in &pres.objects {
                if !o.destroyed && o.health_current > 0.0 {
                    alive.insert(o.id.0);
                    if o.special_power_ready {
                        special_ready.insert(o.id.0);
                    }
                }
                if o.team != team || o.destroyed || o.health_current <= 0.0 {
                    continue;
                }
                if sample.is_none() {
                    let p = o.position;
                    sample = Some([p.x, p.y, p.z]);
                }
                let name = o.template_name.to_ascii_lowercase();
                let is_barracks = name.contains("barracks")
                    || o.building_type
                        == Some(crate::presentation_frame::PresentationBuildingType::Barracks);
                let is_producer = o.can_produce
                    || is_barracks
                    || name.contains("warfactory")
                    || name.contains("airfield")
                    || name.contains("helipad");
                if !is_producer {
                    continue;
                }
                if o.under_construction {
                    unfinished.push(o.id);
                    continue;
                }
                if is_barracks {
                    barracks.push(o.id);
                } else {
                    any.push(o.id);
                }
            }
        } else {
            // Wave 853/857/902: cold residual fail-closed (no get_objects dual-read).
            // InGame stamps via presentation freeze path above.
            let _ = team;
        }
        barracks.sort_by_key(|id| id.0);
        any.sort_by_key(|id| id.0);
        unfinished.sort_by_key(|id| id.0);
        self.host_match_local_barracks_ids = Some(barracks);
        self.host_match_local_producer_ids = Some(any);
        self.host_match_local_unfinished_producer_ids = Some(unfinished);
        self.host_match_local_team_sample_pos = sample;
        self.host_match_alive_object_ids = Some(alive);
        self.host_match_special_power_ready_ids = Some(special_ready);
    }

    pub(in crate::cnc_game_engine) fn host_refresh_match_sim_residuals_from_logic(&mut self) {
        // Wave 893: prefer presentation freeze for sim timing + replay/team when live.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_visual_speed = Some(pres.visual_speed_multiplier);
            self.host_match_time_frozen = Some(pres.time_frozen_for_simulation);
            self.host_match_total_play_time = Some(pres.total_play_time_seconds);
            self.host_match_logic_frame = Some(pres.frame.0);
            self.host_match_logic_steps = Some((
                pres.logic_steps_run,
                pres.logic_steps_budget_hit,
                pres.logic_steps_accumulated_seconds,
            ));
            self.host_match_in_replay = Some(pres.in_replay_game);
            self.host_match_local_player_id = Some(self.current_player_id);
            self.host_match_in_shell = Some(false);
            self.host_match_local_team = Some(pres.local_team);
            // Wave 894: AI difficulty residual from freeze (no get_difficulty dual-read).
            self.host_match_ai_difficulty = Some(pres.ai_difficulty);
        } else {
            // Wave 902/908/909: cold residual after match-start — keep prior tick stamp or
            // fail-closed zeros until presentation seed / next update snapshot. No live probe.
            self.host_match_visual_speed = self.host_match_visual_speed.or(Some(1.0));
            self.host_match_time_frozen = Some(self.game_paused);
            self.host_match_total_play_time = self.host_match_total_play_time.or(Some(0.0));
            self.host_match_logic_frame = self.host_match_logic_frame.or(Some(0));
            self.host_match_logic_steps = self.host_match_logic_steps.or(Some((0, false, 0.0)));
            self.host_match_in_replay = Some(false);
            self.host_match_local_player_id = Some(self.current_player_id);
            self.host_match_in_shell = Some(false);
            self.host_match_local_team = Some(crate::game_logic::Team::USA);
        }
        // Wave 846: diplomacy / template / sciences host residuals.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_diplomacy_players = Some(pres.players.clone());
            self.host_match_known_template_names = Some(pres.known_template_names.clone());
            // Wave 901: sciences residual from freeze only (no player_unlocked dual-read).
            let mut sciences = std::collections::HashMap::new();
            sciences.insert(pres.local_player_id, pres.local_unlocked_sciences.clone());
            for p in &pres.players {
                sciences.entry(p.id).or_insert_with(Vec::new);
            }
            self.host_match_unlocked_sciences = Some(sciences);
        } else {
            // Wave 901: cold residual fail-closed (boot_player_info already residual-only).
            self.host_match_diplomacy_players = Some(Vec::new());
            self.host_match_known_template_names = Some(Vec::new());
            self.host_match_unlocked_sciences = Some(std::collections::HashMap::new());
        }
        // Wave 848: stamp local train producers after other match residuals.
        self.host_refresh_local_train_producer_residuals();
        // Wave 861/901: skirmish host is never multiplayer (no dual-read).
        self.host_match_in_multiplayer = Some(false);
        // Wave 862: stamp world bounds residual (freeze first).
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_world_bounds = Some(pres.world_env.world_bounds_vec3());
        } else if let Some(b) = self.host_match_world_bounds {
            let _ = b; // keep prior stamp
        } else {
            // Wave 901: cold residual fail-closed.
            self.host_match_world_bounds = Some((glam::Vec3::ZERO, glam::Vec3::ZERO));
        }
        // Wave 863/901: stamp first-opponent residual from diplomacy residual only.
        let local = self
            .host_match_local_player_id
            .unwrap_or(self.current_player_id);
        if let Some(players) = self.host_match_diplomacy_players.as_ref() {
            self.host_match_first_opponent_id =
                Some(players.iter().find(|p| p.id != local).map(|p| p.id));
        } else if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_first_opponent_id =
                Some(pres.players.iter().find(|p| p.id != local).map(|p| p.id));
        } else {
            // Wave 901: cold residual fail-closed.
            self.host_match_first_opponent_id = Some(None);
        }
        // Wave 901: removed Wave 855 clear that wiped camera/mp/bounds/opponent stamps.
        // Wave 849: stamp match outcome residuals from freeze (or clear when none).
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            self.host_match_over = Some(pres.match_over);
            self.host_match_victory_label = pres.victory_label.clone();
            self.host_match_victory_summary = pres.victory_summary.clone();
            if pres.match_over {
                self.host_match_victory_winner = Some(pres.victory_winner_id());
            } else {
                self.host_match_victory_winner = None;
            }
        }
        // An installed freeze owns the match selection residual, including
        // an empty deselect. The engine list is only the boot residual.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            if !pres.selected.is_empty() {
                self.host_match_selected_ids = Some(pres.selected.clone());
            } else {
                let from_objs: Vec<_> = pres
                    .objects
                    .iter()
                    .filter(|o| o.selected && !o.destroyed && o.health_current > 0.0)
                    .map(|o| o.id)
                    .collect();
                self.host_match_selected_ids = Some(from_objs);
            }
        } else if !self.selected_objects.is_empty() {
            self.host_match_selected_ids = Some(self.selected_objects.clone());
        }
        // Wave 851/853: alive residual stamped inside host_refresh_local_train_producer_residuals
        // (single freeze walk or single host get_objects scan).
        // Wave 852/901: purchasable science residual fail-closed (no can_purchase dual-read).
        {
            let mut map = std::collections::HashMap::new();
            if let Some(players) = self.host_match_diplomacy_players.as_ref() {
                for p in players {
                    map.insert(p.id, std::collections::HashSet::new());
                }
            }
            self.host_match_purchasable_sciences = Some(map);
        }
        // Wave 868/901: local science purchase points from freeze only.
        self.host_match_local_science_purchase_points =
            Some(if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.local_science_purchase_points()
            } else {
                0
            });
        // Wave 921: local supplies residual from freeze only.
        self.host_match_local_supplies =
            Some(if let Some(frame) = self.last_presentation_frame.as_ref() {
                frame.local_supplies as u32
            } else {
                0
            });
        // Wave 854/857: special-power-ready residual stamped inside
        // host_refresh_local_train_producer_residuals (single freeze/host scan).
    }

    pub(in crate::cnc_game_engine) fn presentation_or_boot_visual_speed(&self) -> f32 {
        self.game_logic.visual_speed_multiplier()
    }

    /// Wave 551: presentation freeze owns time-frozen residual when installed.
    /// Boot residual without freeze uses host GameLogic probe.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_time_frozen(&self) -> bool {
        // Wave 551/844: presentation freeze owns time-frozen residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.time_frozen_for_simulation;
        }
        if let Some(v) = self.host_match_time_frozen {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        false
    }

    /// Wave 552: Prefer presentation shell bypass when a frame is installed
    /// (`fow_shell_bypass`, even if false). Boot residual without freeze uses
    /// host `isInShellGame`.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_shell_bypass(&self) -> bool {
        // Wave 552/845: presentation freeze owns shell-bypass residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.fow_shell_bypass;
        }
        if let Some(v) = self.host_match_in_shell {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read). Menu shell residual
        // is stamped into host_match_in_shell on match transitions.
        true
    }

    /// Wave 553: presentation freeze owns total play-time residual when installed
    /// (pipeline freeze preferred, then last frame). Boot residual without freeze
    /// uses host `get_total_play_time`.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_total_play_time(&self) -> f32 {
        // Wave 553/844: presentation freeze owns total play-time residual when installed.
        if let Some(pres) = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_deref())
        {
            return pres.total_play_time_seconds;
        }
        if let Some(v) = self.host_match_total_play_time {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        0.0
    }

    /// Wave 553: presentation freeze owns local player id residual when installed.
    /// Boot residual without freeze uses host `local_player_id` probe.
    #[inline]

    /// Wave 569: defeat residual — prefer presentation freeze `defeated_player_ids`,
    /// drain live queue when freeze installed; boot residual takes live.

    /// Wave 570: script message residual — prefer pipeline/last presentation freeze
    /// `new_script_messages`, drain live queue when freeze installed; boot residual takes live.
    /// Wave 607: via `host_take_presentation_or_boot_new_script_messages`.
    pub(in crate::cnc_game_engine) fn take_presentation_or_boot_new_script_messages(
        &mut self,
    ) -> Vec<String> {
        // Wave 607: thin wrapper — presentation/boot drain via host helper.
        self.host_take_presentation_or_boot_new_script_messages()
    }

    /// Wave 570: script message residual — prefer pipeline/last presentation freeze
    /// `new_script_messages`, drain live queue when freeze installed; boot residual takes live.
    pub(in crate::cnc_game_engine) fn host_take_presentation_or_boot_new_script_messages(
        &mut self,
    ) -> Vec<String> {
        // Wave 607/900: presentation freeze owns script message residual when installed.
        // No live take drain dual-read; boot fail-closed empty.
        if let Some(pres) = self
            .render_pipeline
            .presentation_frame()
            .or(self.last_presentation_frame.as_deref())
        {
            return pres.new_script_messages.clone();
        }
        // Wave 900: fail-closed boot default.
        Vec::new()
    }

    /// Wave 607: via `host_take_presentation_or_boot_defeat_events`.
    pub(in crate::cnc_game_engine) fn take_presentation_or_boot_defeat_events(
        &mut self,
    ) -> Vec<u32> {
        // Wave 607: thin wrapper — presentation/boot drain via host helper.
        self.host_take_presentation_or_boot_defeat_events()
    }

    pub(in crate::cnc_game_engine) fn host_take_presentation_or_boot_defeat_events(
        &mut self,
    ) -> Vec<u32> {
        // Wave 607/900: presentation freeze owns defeat residual when installed.
        // No live take drain dual-read; boot fail-closed empty.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.defeated_player_ids.clone();
        }
        // Wave 900: fail-closed boot default.
        Vec::new()
    }

    /// Wave 569: alliance residual — prefer presentation freeze `alliance_events`,
    /// drain live queue when freeze installed; boot residual takes live.
    /// Wave 607: via `host_take_presentation_or_boot_alliance_events`.
    pub(in crate::cnc_game_engine) fn take_presentation_or_boot_alliance_events(
        &mut self,
    ) -> Vec<crate::game_logic::AllianceNotification> {
        // Wave 607: thin wrapper — presentation/boot drain via host helper.
        self.host_take_presentation_or_boot_alliance_events()
    }

    /// Wave 569: alliance residual — prefer presentation freeze `alliance_events`,
    /// drain live queue when freeze installed; boot residual takes live.
    pub(in crate::cnc_game_engine) fn host_take_presentation_or_boot_alliance_events(
        &mut self,
    ) -> Vec<crate::game_logic::AllianceNotification> {
        // Wave 607/900: presentation freeze owns alliance residual when installed.
        // No live take drain dual-read; boot fail-closed empty.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.alliance_events.clone();
        }
        // Wave 900: fail-closed boot default.
        Vec::new()
    }

    pub(in crate::cnc_game_engine) fn presentation_or_boot_local_player_id(&self) -> Option<u32> {
        // Wave 553/843: presentation freeze owns local player id residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return Some(pres.local_player_id);
        }
        if let Some(id) = self.host_match_local_player_id {
            return Some(id);
        }
        // Wave 895: host current_player_id residual (no GameLogic dual-read).
        Some(self.current_player_id)
    }

    /// Wave 554: presentation freeze owns map-name residual when installed
    /// (even if empty). Boot residual without freeze uses host map probe.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_map_name(&self) -> String {
        // Wave 554/860: presentation freeze owns map-name residual when installed.
        // Wave 840/843/860: if freeze still holds shell residual after match load, prefer
        // host_match_map_name (no live GameLogic dual-read). Live probe only when residual cold.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            let freeze = pres.world_env.map_name.clone();
            if matches!(
                self.current_state,
                GameState::InGame | GameState::Paused | GameState::Loading
            ) && Self::map_name_is_shell_residual(&freeze)
            {
                if let Some(host) = self.host_match_map_name.as_ref() {
                    if !Self::map_name_is_shell_residual(host) {
                        return host.clone();
                    }
                    // Wave 860: warm residual still shell — fail-closed to residual, no live dual-read.
                    return host.clone();
                }
                // Wave 896: residual cold under InGame shell freeze — fail-closed to freeze
                // (no get_current_map_name dual-read). Match-start stamps host_match_map_name.
            }
            return freeze;
        }
        if let Some(host) = self.host_match_map_name.as_ref() {
            return host.clone();
        }
        // Wave 896: fail-closed boot default (no dual-read).
        String::new()
    }

    /// Wave 554: presentation freeze owns AI difficulty residual when installed.
    /// Boot residual without freeze uses host difficulty probe.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_ai_difficulty(
        &self,
    ) -> crate::ai::AIDifficulty {
        // Wave 554/843: presentation freeze owns AI difficulty residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.ai_difficulty;
        }
        if let Some(d) = self.host_match_ai_difficulty {
            return d;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        crate::ai::AIDifficulty::Medium
    }

    /// Wave 557: presentation freeze owns replay-mode residual when installed.
    /// Boot residual without freeze uses host `isInReplayGame`.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_in_replay_game(&self) -> bool {
        // Wave 557/844: presentation freeze owns replay-mode residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.in_replay_game;
        }
        if let Some(v) = self.host_match_in_replay {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        false
    }

    /// Wave 558: presentation freeze owns diplomacy roster residual when installed.
    /// Boot residual without freeze uses host player_* probes (no get_players dual-read).
    /// Wave 573: boot roster build via `boot_player_info_from_host`.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_diplomacy_players(
        &self,
    ) -> Vec<crate::presentation_frame::PresentationPlayerInfo> {
        // Wave 558/846: presentation freeze owns diplomacy roster residual when installed.
        if let Some(frame) = self.last_presentation_frame.as_ref() {
            return frame.players.clone();
        }
        if let Some(players) = self.host_match_diplomacy_players.as_ref() {
            return players.clone();
        }
        // Wave 895: fail-closed boot default (no dual-read). Diplomacy residual is
        // stamped into host_match_diplomacy_players on match sim refresh.
        Vec::new()
    }

    /// Wave 560: presentation freeze owns logic-frame residual when installed.
    /// Boot residual without freeze uses host `get_frame`.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_logic_frame(&self) -> u32 {
        // Wave 560/844: presentation freeze owns logic-frame residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.frame.0;
        }
        if let Some(v) = self.host_match_logic_frame {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        0
    }

    /// Wave 561: presentation freeze owns fixed-step catch-up residual when installed.
    /// Boot residual without freeze uses host `fixed_step_diagnostics().steps_run`.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_logic_steps(&self) -> u32 {
        self.presentation_or_boot_fixed_step_diagnostics().0
    }

    /// Wave 564: presentation freeze owns full fixed-step diagnostics residual when
    /// installed (`steps_run`, `budget_hit`, `accumulated_time_seconds`). Boot residual
    /// without freeze uses host `fixed_step_diagnostics`.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_fixed_step_diagnostics(
        &self,
    ) -> (u32, bool, f32) {
        // Wave 564/844: presentation freeze owns fixed-step diagnostics residual when installed.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return (
                pres.logic_steps_run,
                pres.logic_steps_budget_hit,
                pres.logic_steps_accumulated_seconds,
            );
        }
        if let Some(v) = self.host_match_logic_steps {
            return v;
        }
        // Wave 895: fail-closed boot default (no dual-read).
        (0, false, 0.0)
    }

    /// Wave 563/565: presentation freeze owns template-name residual when installed
    /// (train + construct). Boot residual without freeze uses host `templates.contains_key`.
    #[inline]
    pub(in crate::cnc_game_engine) fn presentation_or_boot_has_template(&self, name: &str) -> bool {
        // Wave 563/846/859: presentation freeze owns template-name residual when installed.
        // Wave 565: construct residual shares this helper with train.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            return pres.has_template_name(name);
        }
        // Wave 859: warm host residual is fail-closed (no live dual-read on miss).
        if let Some(names) = self.host_match_known_template_names.as_ref() {
            return names.binary_search(&name.to_string()).is_ok()
                || names.iter().any(|n| n.eq_ignore_ascii_case(name));
        }
        // Wave 895: fail-closed boot default (no dual-read).
        false
    }

    /// Wave 581: ensure GoldenRanger host template residual for train honesty path.
    pub(in crate::cnc_game_engine) fn host_ensure_golden_ranger_template(&mut self) {
        // Wave 581/722/872/915/934: mid-command host insert residual (callers must opt in).
        // Prefer residual template table when warm (no live dual-read on hit).
        if let Some(names) = self.host_match_known_template_names.as_ref() {
            if names.binary_search(&"GoldenRanger".to_string()).is_ok()
                || names.iter().any(|n| n.eq_ignore_ascii_case("GoldenRanger"))
            {
                return;
            }
        } else if self.last_presentation_frame.is_some() {
            // Wave 915: presentation freeze without residual table — fail-closed skip
            // mid-match template dual-write (boot residual still inserts below).
            return;
        }
        // Wave 900: residual miss / boot → insert host template (or_insert via insert).
        // No contains_key dual-read probe before write.
        let mut tpl = crate::game_logic::ThingTemplate::new("GoldenRanger");
        tpl.set_health(120.0);
        tpl.set_cost(100, 0);
        tpl.build_time = 0.05;
        tpl.add_kind_of(crate::game_logic::KindOf::Infantry);
        tpl.add_kind_of(crate::game_logic::KindOf::Selectable);
        tpl.add_kind_of(crate::game_logic::KindOf::Attackable);
        let _ = self.host_game_logic_mut().apply_host_support_op(
            crate::game_logic::HostSupportOp::InsertThingTemplate {
                name: "GoldenRanger".into(),
                template: tpl,
            },
        );
        self.host_stamp_known_template_name("GoldenRanger");
    }

    /// Wave 872: keep host_match_known_template_names residual warm after inserts.
    #[inline]
    pub(in crate::cnc_game_engine) fn host_stamp_known_template_name(&mut self, name: &str) {
        let key = name.to_string();
        let entry = self
            .host_match_known_template_names
            .get_or_insert_with(Vec::new);
        if entry.binary_search(&key).is_err() && !entry.iter().any(|n| n.eq_ignore_ascii_case(name))
        {
            match entry.binary_search(&key) {
                Ok(_) => {}
                Err(i) => entry.insert(i, key),
            }
        }
    }

    /// Wave 581: template residual for train/construct mid-command host inserts.
    /// Prefer freeze known names; if freeze misses, allow live host `templates`
    /// (inserts after last PresentationFrame). Boot without freeze uses host only.
    #[inline]
    /// Wave 610: via `host_presentation_or_live_has_template`.
    pub(in crate::cnc_game_engine) fn presentation_or_live_has_template(&self, name: &str) -> bool {
        // Wave 610: thin wrapper — residual via host helper.
        self.host_presentation_or_live_has_template(name)
    }

    /// Wave 581: template residual for train/construct mid-command host inserts.
    /// Prefer freeze known names; if freeze misses, allow live host `templates`
    /// (inserts after last PresentationFrame). Boot without freeze uses host only.
    #[inline]
    pub(in crate::cnc_game_engine) fn host_presentation_or_live_has_template(
        &self,
        name: &str,
    ) -> bool {
        // Wave 610/846/859: host residual helper.
        // Wave 581: freeze known names OR host residual OR live host insert residual.
        if let Some(pres) = self.last_presentation_frame.as_ref() {
            if pres.has_template_name(name) {
                return true;
            }
            // Wave 898: freeze lag miss uses host residual only (no dual-read).
            if let Some(names) = self.host_match_known_template_names.as_ref() {
                return names.binary_search(&name.to_string()).is_ok()
                    || names.iter().any(|n| n.eq_ignore_ascii_case(name));
            }
            return false;
        }
        if let Some(names) = self.host_match_known_template_names.as_ref() {
            return names.binary_search(&name.to_string()).is_ok()
                || names.iter().any(|n| n.eq_ignore_ascii_case(name));
        }
        // Wave 898: fail-closed boot default (no dual-read).
        false
    }
}
