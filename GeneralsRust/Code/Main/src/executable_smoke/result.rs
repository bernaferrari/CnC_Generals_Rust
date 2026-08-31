// Lifecycle: result type — ExecutableSmokeResult fields, claim gates,
// and Default (field-for-field copy of the pre-split monolith).

#[derive(Debug, Clone)]
pub struct ExecutableSmokeResult {
    pub status: String,
    pub detail: String,
    /// True only via `retail_windowed_playable_claim` (all five flags).
    /// Headless host smoke never sets `window_visible` / `wnd_widget_tree_nav`.
    /// Residual-pack honesty must not OR into this flag (`host_wave_inflation`).
    pub playable_claim: bool,
    /// Honest headless vertical slice: shell WND + skirmish latch chain
    /// (map/slots/rules/start) + InGame + select/move + construct + train +
    /// presentation-owned frame with non-zero stable render items
    /// (no live GameLogic dual-read). Still not full retail WND playable_claim.
    pub host_vertical_slice_ok: bool,
    /// Limited: process reached InGame (or Menu+start attempted) and exited 0.
    pub executable_host_ok: bool,
    pub process_started: bool,
    pub reached_menu: bool,
    /// Retail shell WND residual: shell active with MainMenu/Skirmish layout on stack.
    pub shell_wnd_ok: bool,
    pub reached_ingame: bool,
    /// Runtime-host select+move command accepted (not WND click; still not full playable_claim).
    pub gameplay_cmd_ok: bool,
    /// Runtime-host dozer construct command accepted (still not full playable_claim).
    pub construct_cmd_ok: bool,
    /// Runtime-host train_unit accepted (still not full playable_claim).
    pub train_cmd_ok: bool,
    /// Physical ControlBar/UI interaction both began construction and queued a
    /// unit in the same windowed session. This is deliberately separate from
    /// the runtime-host `construct_cmd_ok` / `train_cmd_ok` diagnostics: a
    /// control-file command must never satisfy the manual acceptance gate.
    ///
    /// The windowed status protocol does not publish this evidence yet, so it
    /// remains fail-closed until the live input path emits
    /// `physical_build_and_produce=true`.
    pub physical_build_and_produce: bool,
    pub save_cmd_ok: bool,
    /// Runtime-host quickload after save accepted (still not full playable_claim).
    pub load_cmd_ok: bool,
    /// Runtime-host stop_all accepted (still not full playable_claim).
    pub stop_cmd_ok: bool,
    /// Runtime-host sell accepted (still not full playable_claim).
    pub sell_cmd_ok: bool,
    pub upgrade_cmd_ok: bool,
    pub guard_cmd_ok: bool,
    pub attack_move_cmd_ok: bool,
    /// Host attack applied observable HP damage (combat residual).
    pub combat_damage_ok: bool,
    pub scatter_cmd_ok: bool,
    pub patrol_cmd_ok: bool,
    pub deploy_cmd_ok: bool,
    pub cheer_cmd_ok: bool,
    pub formation_cmd_ok: bool,
    pub capture_cmd_ok: bool,
    pub return_supplies_cmd_ok: bool,
    /// Physical UI/world input observed a completed gather/return-resources
    /// workflow. Unlike `return_supplies_cmd_ok`, this cannot be inferred from
    /// a runtime-host command result.
    pub physical_gather_resources: bool,
    /// Physical PopupSaveLoad UI input completed a save, load, and continued
    /// in the same windowed session. Unlike `save_cmd_ok` / `load_cmd_ok`,
    /// this is not set by runtime-host automation.
    pub physical_save_load_continue: bool,
    pub evacuate_cmd_ok: bool,
    pub repair_cmd_ok: bool,
    pub return_to_base_cmd_ok: bool,
    pub attitude_cmd_ok: bool,
    pub rally_cmd_ok: bool,
    pub switch_weapons_cmd_ok: bool,
    pub view_cc_cmd_ok: bool,
    pub clear_mines_cmd_ok: bool,
    pub beacon_cmd_ok: bool,
    pub hack_cmd_ok: bool,
    pub cleanup_cmd_ok: bool,
    pub combat_drop_cmd_ok: bool,
    pub overcharge_cmd_ok: bool,
    pub special_power_cmd_ok: bool,
    pub remove_beacon_cmd_ok: bool,
    pub demo_cmd_ok: bool,
    pub view_radar_cmd_ok: bool,
    pub force_attack_cmd_ok: bool,
    pub force_attack_object_cmd_ok: bool,
    pub select_all_cmd_ok: bool,
    pub control_group_cmd_ok: bool,
    pub waypoint_cmd_ok: bool,
    pub box_select_cmd_ok: bool,
    /// InGame status reported presentation_frame_ok=true at least once.
    pub presentation_frame_ok: bool,
    /// No live GameLogic dual-reads while presentation owned collect (status residual).
    pub presentation_live_fallback_ok: bool,
    /// InGame observed gameworld_presentation_entities>0 at least once (shadow observe-path).
    pub gameworld_presentation_entities_ok: bool,
    /// Peak InGame gameworld_presentation_entities from runtime-host status.
    pub max_gameworld_presentation_entities: u32,
    /// InGame observed gameworld_overlay_stamped>0 at least once.
    pub gameworld_overlay_stamped_ok: bool,
    /// Peak InGame gameworld_overlay_stamped from runtime-host status.
    pub max_gameworld_overlay_stamped: u32,
    pub max_gameworld_appended: u32,
    /// Peak gameworld_rebuilt (Wave 194 default GW roster).
    pub max_gameworld_rebuilt: u32,
    /// InGame observed gameworld_rebuilt>0 at least once (Wave 196).
    pub gameworld_rebuilt_ok: bool,
    pub select_similar_cmd_ok: bool,
    pub select_on_screen_cmd_ok: bool,
    pub select_structures_cmd_ok: bool,
    pub select_aircraft_cmd_ok: bool,
    pub select_idle_cmd_ok: bool,
    pub camera_reset_cmd_ok: bool,
    pub camera_zoom_cmd_ok: bool,
    pub pause_cmd_ok: bool,
    pub cancel_production_cmd_ok: bool,
    pub diplomacy_cmd_ok: bool,
    /// Host published a usable live frame.png (GPU/screenshot residual).
    pub live_frame_ok: bool,
    /// Host published a visible (non-headless) OS window.
    pub window_visible: bool,
    /// Host published a hit-verified WND widget-tree LeftDown/Up click.
    pub wnd_widget_tree_nav: bool,
    /// Physical winit command after a physical WND menu→match transition.
    /// Never set by runtime-host control-file commands.
    pub interactive_gameplay: bool,
    /// Comma-separated names of the five retail sit-through flags that are still
    /// false (`window_visible`, `wnd_widget_tree_nav`, `live_frame_ok`, `ingame`,
    /// `gameplay`). Empty only when `playable_claim` is true.
    pub retail_sit_through_missing: String,
    /// Peak InGame unit mesh render_item_count from host status (world draw residual).
    pub max_render_item_count: u32,
    /// Peak InGame presentation-alive object count.
    pub max_render_alive_objects: u32,
    /// True when InGame observed stable non-zero render items (not a one-frame flash).
    pub render_items_stable_ok: bool,
    pub auto_attack_cmd_ok: bool,
    pub options_cmd_ok: bool,
    pub request_capture_cmd_ok: bool,
    /// Runtime-host opened Skirmish UI screen before start_game.
    pub skirmish_menu_ok: bool,
    /// Runtime-host exercised SkirmishMenu Start button click path (not WND widget tree).
    pub skirmish_start_click_ok: bool,
    /// click_skirmish_start used retail WND ButtonStart path (ok_wnd / wnd_pending).
    pub skirmish_start_wnd_ok: bool,
    /// open_skirmish_menu used retail MainMenu.wnd:ButtonSkirmish GBM_SELECTED residual.
    pub main_menu_skirmish_wnd_ok: bool,
    /// click_skirmish_start used retail map-select overlay OK residual before Start.
    pub skirmish_map_select_wnd_ok: bool,
    /// click_skirmish_start applied human+AI slot residual before Start.
    pub skirmish_slot_config_wnd_ok: bool,
    /// click_skirmish_start applied cash/SW/speed rules residual before Start.
    pub skirmish_rules_wnd_ok: bool,
    pub frames_observed: u32,
    pub map_seen: String,
    pub exit_code: Option<i32>,
    pub new_game_path: bool,
    /// True when this result came from [`ExecutableSmokeLaunch::Windowed`].
    /// Headless host smoke stays false so `playable_claim` cannot go true.
    pub windowed_launch: bool,
}

impl ExecutableSmokeResult {
    /// Shell / empty / "-" maps cannot satisfy `reached_ingame`.
    pub fn map_is_shell_for_claim(map: &str) -> bool {
        let m = map.trim().to_ascii_lowercase();
        m.is_empty() || m == "-" || m.contains("shellmap")
    }

    /// `reached_ingame` from the live status snapshot: InGame/Paused and a
    /// non-shell map. Empty / `-` / shellmap stay false.
    pub fn reached_ingame_from_live_map(state: &str, map: &str) -> bool {
        matches!(state, "InGame" | "Paused") && !Self::map_is_shell_for_claim(map)
    }

    /// `window_visible` from the shipped winit query. Headless is always false.
    /// `None` (platform cannot query) is visible only when not headless.
    pub fn window_visible_from_winit_query(headless: bool, is_visible: Option<bool>) -> bool {
        !headless && is_visible.unwrap_or(!headless)
    }

    /// `live_frame_ok` from a promoted capture PNG **or** a windowed wgpu
    /// surface present. Headless never calls the present latch.
    pub fn live_frame_ok_from_windowed_present(
        capture_promoted: bool,
        windowed_surface_presented: bool,
    ) -> bool {
        capture_promoted || windowed_surface_presented
    }

    /// Named gadgets that count as MainMenu → Skirmish (not Parent/Ruler/Options).
    pub fn wnd_nav_gadget_is_skirmish_path(name: &str) -> bool {
        let n = name.to_ascii_lowercase();
        n.contains("buttonskirmish")
            || n.contains("buttonstart")
            || n.contains("skirmishgameoptions")
    }

    /// Honest retail `playable_claim`: all five must be true. Headless host smoke
    /// never passes `window_visible` / `wnd_widget_tree_nav`.
    pub fn retail_windowed_playable_claim(
        window_visible: bool,
        wnd_widget_tree_nav: bool,
        gpu_present: bool,
        ingame: bool,
        gameplay: bool,
    ) -> bool {
        window_visible && wnd_widget_tree_nav && gpu_present && ingame && gameplay
    }

    /// Headless smoke must keep `playable_claim == false`. Windowed smoke may
    /// publish true **only** when all five retail flags are true.
    ///
    /// The fifth flag is `interactive_gameplay` (RMB order latch via
    /// `handle_mouse_button_input` / status `gameplay=`), **not** host
    /// `gameplay_cmd_ok` (select/move/construct residual).
    ///
    /// This is the gate policy used by `executable_smoke_gate`: a finished
    /// windowed sit-through is allowed to claim playable; a headless host
    /// result never is.
    pub fn playable_claim_gate_ok(&self) -> Result<(), &'static str> {
        if !self.playable_claim {
            return Ok(());
        }
        if self.windowed_launch {
            if Self::retail_windowed_playable_claim(
                self.window_visible,
                self.wnd_widget_tree_nav,
                self.live_frame_ok,
                self.reached_ingame,
                self.interactive_gameplay,
            ) {
                return Ok(());
            }
            return Err(
                "windowed playable_claim=true is only legal when all five retail flags are true",
            );
        }
        Err("headless playable_claim must stay false")
    }

    /// Comma-separated list of the five retail sit-through flags that are false.
    /// Empty iff `retail_windowed_playable_claim` would be true.
    pub fn retail_sit_through_missing_flags(
        window_visible: bool,
        wnd_widget_tree_nav: bool,
        live_frame_ok: bool,
        ingame: bool,
        gameplay: bool,
    ) -> String {
        let mut missing = Vec::new();
        if !window_visible {
            missing.push("window_visible");
        }
        if !wnd_widget_tree_nav {
            missing.push("wnd_widget_tree_nav");
        }
        if !live_frame_ok {
            missing.push("live_frame_ok");
        }
        if !ingame {
            missing.push("ingame");
        }
        if !gameplay {
            missing.push("gameplay");
        }
        missing.join(",")
    }

    /// Wave 176: latch `host_vertical_slice_ok` from presentation boundary + WND/cmd residuals.
    ///
    /// InGame requires a presentation-owned frame with zero live GameLogic dual-reads.
    /// Soft when the display never reached InGame (assets/GPU unavailable).
    /// `playable_claim` follows the five-flag retail formula (false in headless).
    /// Fifth retail flag is `interactive_gameplay` only — never host `gameplay_cmd_ok`.
    pub fn apply_host_vertical_slice_gate(&mut self) {
        // Headless latch peels / GenericTracer INI do not flip this claim.
        // Windowed interactive can prove it only when all five residuals are true.
        // Fifth flag: physical/inject RMB order latch, not host select/move residual.
        self.retail_sit_through_missing = Self::retail_sit_through_missing_flags(
            self.window_visible,
            self.wnd_widget_tree_nav,
            self.live_frame_ok,
            self.reached_ingame,
            self.interactive_gameplay,
        );
        self.playable_claim = Self::retail_windowed_playable_claim(
            self.window_visible,
            self.wnd_widget_tree_nav,
            self.live_frame_ok,
            self.reached_ingame,
            self.interactive_gameplay,
        );
        let map_is_shell_residual = Self::map_is_shell_for_claim(&self.map_seen);
        let skirmish_map_boundary_ok = !self.reached_ingame || !map_is_shell_residual;
        let gameworld_presentation_boundary_ok = !self.reached_ingame
            || !self.presentation_frame_ok
            || self.max_render_alive_objects == 0
            || self.gameworld_presentation_entities_ok;
        let gameworld_overlay_boundary_ok =
            !self.gameworld_presentation_entities_ok || self.gameworld_overlay_stamped_ok;
        let gameworld_rebuilt_boundary_ok =
            !self.gameworld_presentation_entities_ok || self.gameworld_rebuilt_ok;
        let render_mesh_boundary_ok = !self.reached_ingame
            || (self.max_render_alive_objects > 0
                && self.max_render_item_count > 0
                && self.render_items_stable_ok
                && self.presentation_live_fallback_ok);
        let presentation_boundary_ok = !self.reached_ingame
            || (self.presentation_frame_ok && self.presentation_live_fallback_ok);
        self.host_vertical_slice_ok = self.shell_wnd_ok
            && self.main_menu_skirmish_wnd_ok
            && self.skirmish_map_select_wnd_ok
            && self.skirmish_slot_config_wnd_ok
            && self.skirmish_rules_wnd_ok
            && self.skirmish_start_wnd_ok
            && self.reached_ingame
            && self.gameplay_cmd_ok
            && self.construct_cmd_ok
            && self.train_cmd_ok
            && self.executable_host_ok
            && presentation_boundary_ok
            && gameworld_presentation_boundary_ok
            && gameworld_overlay_boundary_ok
            && gameworld_rebuilt_boundary_ok
            && render_mesh_boundary_ok
            && skirmish_map_boundary_ok;
    }
}

impl Default for ExecutableSmokeResult {
    fn default() -> Self {
        Self {
            status: "not_run".into(),
            detail: String::new(),
            playable_claim: false,
            host_vertical_slice_ok: false,
            executable_host_ok: false,
            process_started: false,
            reached_menu: false,
            shell_wnd_ok: false,
            reached_ingame: false,
            gameplay_cmd_ok: false,
            construct_cmd_ok: false,
            train_cmd_ok: false,
            physical_build_and_produce: false,
            save_cmd_ok: false,
            load_cmd_ok: false,
            stop_cmd_ok: false,
            sell_cmd_ok: false,
            upgrade_cmd_ok: false,
            guard_cmd_ok: false,
            attack_move_cmd_ok: false,
            combat_damage_ok: false,
            scatter_cmd_ok: false,
            patrol_cmd_ok: false,
            deploy_cmd_ok: false,
            cheer_cmd_ok: false,
            formation_cmd_ok: false,
            capture_cmd_ok: false,
            return_supplies_cmd_ok: false,
            physical_gather_resources: false,
            physical_save_load_continue: false,
            evacuate_cmd_ok: false,
            repair_cmd_ok: false,
            return_to_base_cmd_ok: false,
            attitude_cmd_ok: false,
            rally_cmd_ok: false,
            switch_weapons_cmd_ok: false,
            view_cc_cmd_ok: false,
            clear_mines_cmd_ok: false,
            beacon_cmd_ok: false,
            hack_cmd_ok: false,
            cleanup_cmd_ok: false,
            combat_drop_cmd_ok: false,
            overcharge_cmd_ok: false,
            special_power_cmd_ok: false,
            remove_beacon_cmd_ok: false,
            demo_cmd_ok: false,
            view_radar_cmd_ok: false,
            force_attack_cmd_ok: false,
            force_attack_object_cmd_ok: false,
            select_all_cmd_ok: false,
            control_group_cmd_ok: false,
            waypoint_cmd_ok: false,
            box_select_cmd_ok: false,
            presentation_frame_ok: false,
            presentation_live_fallback_ok: false,
            gameworld_presentation_entities_ok: false,
            max_gameworld_presentation_entities: 0,
            gameworld_overlay_stamped_ok: false,
            max_gameworld_overlay_stamped: 0,
            max_gameworld_appended: 0,
            max_gameworld_rebuilt: 0,
            gameworld_rebuilt_ok: false,
            select_similar_cmd_ok: false,
            select_on_screen_cmd_ok: false,
            select_structures_cmd_ok: false,
            select_aircraft_cmd_ok: false,
            select_idle_cmd_ok: false,
            camera_reset_cmd_ok: false,
            camera_zoom_cmd_ok: false,
            pause_cmd_ok: false,
            cancel_production_cmd_ok: false,
            diplomacy_cmd_ok: false,
            live_frame_ok: false,
            window_visible: false,
            wnd_widget_tree_nav: false,
            interactive_gameplay: false,
            retail_sit_through_missing: Self::retail_sit_through_missing_flags(
                false, false, false, false, false,
            ),
            max_render_item_count: 0,
            max_render_alive_objects: 0,
            render_items_stable_ok: false,
            auto_attack_cmd_ok: false,
            options_cmd_ok: false,
            request_capture_cmd_ok: false,
            skirmish_menu_ok: false,
            skirmish_start_click_ok: false,
            skirmish_start_wnd_ok: false,
            main_menu_skirmish_wnd_ok: false,
            skirmish_map_select_wnd_ok: false,
            skirmish_slot_config_wnd_ok: false,
            skirmish_rules_wnd_ok: false,
            frames_observed: 0,
            map_seen: "-".into(),
            exit_code: None,
            new_game_path: false,
            windowed_launch: false,
        }
    }
}
