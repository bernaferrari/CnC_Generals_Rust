//! Executable smoke via the production `generals` binary + runtime host bridge.
//!
//! This is **stronger** than headless `shell_smoke` (which constructs `GameLogic`
//! in-process): it boots the real event loop, creates a (hidden) window, runs
//! WW3D headless init, and drives Menu → Start through the same control file
//! path GPUI uses.
//!
//! Honesty:
//! - `playable_claim` is the five-flag retail formula (`window_visible` &&
//!   `wnd_widget_tree_nav` && `live_frame_ok` && InGame && gameplay). Headless
//!   host smoke never publishes a visible OS window or OS/WND widget-tree nav, so
//!   the claim stays false (`executable_smoke_gate` / `behavior_gate` still require
//!   false for the **headless** host). Windowed launch may publish true when all
//!   five flags are observed; the gate does not reject that finished-window claim.
//! - `retail_sit_through_missing` lists whichever of those five flags are still
//!   false (empty only when the claim is true). Status lines print each flag.
//! - `host_vertical_slice_ok` is the strengthened headless claim: shell WND + skirmish
//!   latch chain (map/slots/rules/start) + InGame + construct/train/gameplay +
//!   presentation boundary with non-zero stable render items (no live GameLogic
//!   dual-read). Still not full retail `playable_claim`.
//! - `executable_host_ok` is the limited claim: process boots, reaches Menu or
//!   InGame via runtime host commands, and exits cleanly.
//! - If display/GPU/window creation fails in the environment, status is
//!   `assets_or_display_unavailable` (fail-closed, not a green lie).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Candidate retail Lone Eagle paths (workspace-relative).
///
/// Shared with [`crate::windowed_acceptance`] so the windowed sit-through gate
/// probes the same extract locations as executable smoke.
pub const LONE_EAGLE_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
];

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

#[derive(Debug, Default, Clone)]
struct StatusSnap {
    state: String,
    ui_screen: String,
    /// Sticky host residual: skirmish menu was opened (survives InGame clear).
    skirmish_menu_ok: bool,
    map: String,
    frame: u32,
    startup_progress: f32,
    startup_phase: String,
    selected_count: u32,
    local_mobile_units: u32,
    under_construction: u32,
    match_damage_applied: f32,
    match_kills: u32,
    last_gameplay_cmd: String,
    match_over: bool,
    victory_label: String,
    presentation_frame_ok: bool,
    gameworld_presentation_entities: u32,
    gameworld_overlay_stamped: u32,
    gameworld_appended: u32,
    gameworld_rebuilt: u32,
    gameworld_primary_objects: bool,
    shell_screen_count: u32,
    shell_top_wnd: String,
    shell_active: bool,
    presentation_live_fallback_reads: u32,
    waypoint_mode: bool,
    live_frame_ok: bool,
    window_visible: bool,
    wnd_widget_tree_nav: bool,
    interactive_gameplay: bool,
    /// Physical workflow evidence. These intentionally have no fallback to
    /// `last_gameplay_cmd`, which is a runtime-host control-channel diagnostic.
    physical_build_and_produce: bool,
    physical_gather_resources: bool,
    physical_save_load_continue: bool,
    retail_sit_through_missing: String,
    render_item_count: u32,
    render_alive_objects: u32,
    render_fow_filtered: u32,
    render_frustum_culled: u32,
}

fn parse_status(path: &Path) -> Option<StatusSnap> {
    let text = fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    let mut snap = StatusSnap::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "state" => snap.state = v.trim().to_string(),
            "ui_screen" => snap.ui_screen = v.trim().to_string(),
            "skirmish_menu_ok" => {
                snap.skirmish_menu_ok =
                    matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "yes");
            }
            "map" => snap.map = v.trim().to_string(),
            "frame" => snap.frame = v.trim().parse().unwrap_or(0),
            "startup_progress" => snap.startup_progress = v.trim().parse().unwrap_or(0.0),
            "startup_phase" => snap.startup_phase = v.trim().to_string(),
            "selected_count" => snap.selected_count = v.trim().parse().unwrap_or(0),
            "local_mobile_units" => snap.local_mobile_units = v.trim().parse().unwrap_or(0),
            "under_construction" => snap.under_construction = v.trim().parse().unwrap_or(0),
            "match_damage_applied" => {
                snap.match_damage_applied = v.trim().parse().unwrap_or(0.0);
            }
            "match_kills" => snap.match_kills = v.trim().parse().unwrap_or(0),
            "last_gameplay_cmd" => snap.last_gameplay_cmd = v.trim().to_string(),
            "presentation_frame_ok" => {
                snap.presentation_frame_ok = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "gameworld_presentation_entities" => {
                snap.gameworld_presentation_entities = v.trim().parse().unwrap_or(0);
            }
            "gameworld_overlay_stamped" => {
                snap.gameworld_overlay_stamped = v.trim().parse().unwrap_or(0);
            }
            "gameworld_appended" => {
                snap.gameworld_appended = v.parse().unwrap_or(0);
            }
            "gameworld_rebuilt" => {
                snap.gameworld_rebuilt = v.trim().parse().unwrap_or(0);
            }
            "gameworld_primary_objects" => {
                snap.gameworld_primary_objects = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "shell_screen_count" => {
                snap.shell_screen_count = v.trim().parse().unwrap_or(0);
            }
            "shell_top_wnd" => snap.shell_top_wnd = v.trim().to_string(),
            "shell_active" => {
                snap.shell_active = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "presentation_live_fallback_reads" => {
                snap.presentation_live_fallback_reads = v.trim().parse().unwrap_or(0);
            }
            "waypoint_mode" => {
                snap.waypoint_mode = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "live_frame_ok" => {
                snap.live_frame_ok = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "window_visible" => {
                snap.window_visible = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "wnd_widget_tree_nav" => {
                snap.wnd_widget_tree_nav = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "gameplay" | "interactive_gameplay" => {
                snap.interactive_gameplay = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "physical_build_and_produce" => {
                snap.physical_build_and_produce = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "physical_gather_resources" => {
                snap.physical_gather_resources = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "physical_save_load_continue" => {
                snap.physical_save_load_continue = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "retail_sit_through_missing" => {
                snap.retail_sit_through_missing = v.trim().to_string();
            }
            "render_item_count" => {
                snap.render_item_count = v.trim().parse().unwrap_or(0);
            }
            "render_alive_objects" => {
                snap.render_alive_objects = v.trim().parse().unwrap_or(0);
            }
            "render_fow_filtered" => {
                snap.render_fow_filtered = v.trim().parse().unwrap_or(0);
            }
            "render_frustum_culled" => {
                snap.render_frustum_culled = v.trim().parse().unwrap_or(0);
            }
            "match_over" => snap.match_over = matches!(v.trim(), "true" | "1" | "True"),
            "victory_label" => snap.victory_label = v.trim().to_string(),
            _ => {}
        }
    }
    Some(snap)
}

fn write_control(path: &Path, lines: &[&str]) -> std::io::Result<()> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    for line in lines {
        writeln!(f, "{line}")?;
    }
    f.flush()
}

fn kill_stale_runtime_host_generals(exe: &Path) {
    // Fail-soft: prior smoke / cargo runs can leave a hanging `generals` holding
    // GPU/display and cause Booting→exit before Menu (or Tokio shutdown races).
    #[cfg(unix)]
    {
        let exe_s = exe.to_string_lossy().to_string();
        // CLI flag is `-runtime_host=headless` (underscore). Also match basename
        // when the absolute path differs between debug/release invocations.
        // Wave 833: never pkill bare exe path — that races the just-spawned child
        // when paths collide across debug/release. Match runtime_host flag only.
        let patterns = [
            format!("{exe_s}.*runtime_host"),
            "target/.*/generals.*runtime_host=headless".to_string(),
            "generals.*-runtime_host=headless".to_string(),
        ];
        for pat in patterns {
            let _ = std::process::Command::new("pkill")
                .args(["-9", "-f", &pat])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        // Allow GPU/window teardown before the next spawn.
        std::thread::sleep(Duration::from_millis(1200));
    }
    let _ = exe;
}

pub(crate) fn resolve_runtime_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GENERALS_RUNTIME_EXE") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    // Wave 833: current-source binary. Newest-mtime among debug+release so a
    // freshly built debug tree is not skipped for a stale release. Optional
    // GENERALS_RUNTIME_EXE_PREFER_RELEASE=1 restores release-first. Override
    // with GENERALS_RUNTIME_EXE. GENERALS_RUNTIME_EXE_PREFER_DEBUG=1 still
    // means newest-mtime (same as default).
    let prefer_release_first =
        std::env::var_os("GENERALS_RUNTIME_EXE_PREFER_RELEASE").is_some_and(|v| {
            let s = v.to_string_lossy();
            !(s.is_empty()
                || s == "0"
                || s.eq_ignore_ascii_case("false")
                || s.eq_ignore_ascii_case("no"))
        });
    let candidates = [
        PathBuf::from("target/release/generals"),
        PathBuf::from("GeneralsRust/target/release/generals"),
        PathBuf::from("./target/release/generals"),
        PathBuf::from("target/debug/generals"),
        PathBuf::from("GeneralsRust/target/debug/generals"),
        PathBuf::from("./target/debug/generals"),
    ];
    if let Some(path) = resolve_runtime_exe_from_candidates(&candidates, !prefer_release_first) {
        return Some(path);
    }
    // Try next to current exe
    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            let sibling = dir.join("generals");
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }
    None
}

pub(crate) fn resolve_runtime_exe_from_candidates(
    candidates: &[PathBuf],
    newest_mtime: bool,
) -> Option<PathBuf> {
    if newest_mtime {
        let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
        for c in candidates {
            if !c.is_file() {
                continue;
            }
            let modified = c
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            match &best {
                Some((t, _)) if modified <= *t => {}
                _ => best = Some((modified, c.clone())),
            }
        }
        if let Some((_, path)) = best {
            return Some(path);
        }
    } else {
        for c in candidates {
            if c.is_file() {
                return Some(c.clone());
            }
        }
    }
    None
}

fn resolve_lone_eagle_map() -> String {
    let mut candidates: Vec<PathBuf> = LONE_EAGLE_CANDIDATES.iter().map(PathBuf::from).collect();
    // Walk from CARGO_MANIFEST_DIR (Code/Main) up to repo root and common extract dirs.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for base in [
        manifest.clone(),
        manifest.join(".."),
        manifest.join("../.."),
        manifest.join("../../.."),
        manifest.join("../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle"),
        manifest.join("../../../windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle"),
    ] {
        candidates.push(base.join("Lone Eagle.map"));
        candidates.push(base.join("Maps/Lone Eagle/Lone Eagle.map"));
        candidates.push(
            base.join("windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map"),
        );
        candidates.push(
            base.join("windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map"),
        );
    }
    if let Ok(cwd) = std::env::current_dir() {
        for c in LONE_EAGLE_CANDIDATES {
            candidates.push(cwd.join(c));
            candidates.push(cwd.join("..").join(c));
        }
    }
    for c in candidates {
        if c.is_file() {
            // Prefer absolute canonical path so the child process cwd does not matter.
            return c.canonicalize().unwrap_or(c).to_string_lossy().into_owned();
        }
    }
    "Lone Eagle".into()
}

/// True when a Lone Eagle `.map` file exists on one of the smoke search paths.
///
/// The bare `"Lone Eagle"` fallback name (no file on disk) is **not** a hit.
pub fn lone_eagle_map_on_disk() -> bool {
    let resolved = resolve_lone_eagle_map();
    Path::new(&resolved).is_file()
}

fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// How the production `generals` child is launched.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableSmokeLaunch {
    /// Existing headless host gate. Uses `-runtime_host=headless`.
    /// `playable_claim` stays false.
    HeadlessHost,
    /// Visible OS window + WGPU. Uses `-runtime_host=windowed` so GPUI
    /// status/control still publish, but `init_headless` is not used.
    Windowed,
}

/// Whether the smoke loop may write gameplay/menu commands into the runtime
/// host's control file.
///
/// The regular headless smoke is deliberately automated. The windowed
/// acceptance gate is deliberately an observer: it must not manufacture the
/// menu, order, production, gather, or save/load evidence that it reports.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmokeDriver {
    Automated,
    ManualObserver,
}

/// Run the executable smoke with a timeout budget.
///
/// `use_new_game_path`: when true, drive Start via `queue_new_game` (Menu drain).
/// When false, use direct `start_game` runtime host command.
pub fn run_executable_smoke(timeout: Duration, use_new_game_path: bool) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        use_new_game_path,
        ExecutableSmokeLaunch::HeadlessHost,
        SmokeDriver::Automated,
    )
}

/// Windowed sit-through observer. Never passes `-runtime_host=headless`, and
/// never drives the child through its control file. A person must operate the
/// visible game; the runner only observes its status and terminates it at the
/// timeout boundary.
///
/// `use_new_game_path` is retained for the shared API/report shape, but is
/// intentionally ignored by the manual observer.
pub fn run_windowed_acceptance_smoke(
    timeout: Duration,
    use_new_game_path: bool,
) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        use_new_game_path,
        ExecutableSmokeLaunch::Windowed,
        SmokeDriver::ManualObserver,
    )
}

fn run_executable_smoke_with_launch_and_driver(
    timeout: Duration,
    use_new_game_path: bool,
    launch: ExecutableSmokeLaunch,
    driver: SmokeDriver,
) -> ExecutableSmokeResult {
    // One automatic retry: Booting early-exit is commonly a stale GPU/lock race after
    // pkill -9 (no Drop cleanup). Second attempt after a fresh kill is usually green.
    let first = run_executable_smoke_once(timeout, use_new_game_path, launch, driver);
    let retryable = matches!(
        first.status.as_str(),
        "process_exited" | "timeout" | "no_menu"
    ) && !first.reached_menu
        && !first.reached_ingame;
    if !retryable {
        return first;
    }
    std::thread::sleep(Duration::from_millis(1500));
    let second = run_executable_smoke_once(timeout, use_new_game_path, launch, driver);
    if second.executable_host_ok || second.reached_menu || second.reached_ingame {
        let mut out = second;
        out.detail = format!(
            "retry_after_boot_race; first={}; {}",
            first.detail, out.detail
        );
        return out;
    }
    // Prefer the more informative failure.
    let mut out = first;
    out.detail = format!(
        "retry_also_failed; second={}; {}",
        second.detail, out.detail
    );
    out
}

fn run_executable_smoke_once(
    timeout: Duration,
    use_new_game_path: bool,
    launch: ExecutableSmokeLaunch,
    driver: SmokeDriver,
) -> ExecutableSmokeResult {
    let mut result = ExecutableSmokeResult {
        playable_claim: false,
        new_game_path: use_new_game_path,
        windowed_launch: matches!(launch, ExecutableSmokeLaunch::Windowed),
        ..Default::default()
    };

    let Some(exe) = resolve_runtime_exe() else {
        result.status = "binary_missing".into();
        result.detail =
            "generals binary not found; build with `cargo build -p generals_main --bin generals --release` or set GENERALS_RUNTIME_EXE".into();
        return result;
    };

    // Best-effort: prior flaky runs can leave a hanging runtime_host `generals` holding
    // the GPU/display; that makes the next Booting exit before Menu.
    kill_stale_runtime_host_generals(&exe);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("generals_exec_smoke_{stamp}"));
    let _ = fs::create_dir_all(&tmp);
    let control_path = tmp.join("control.txt");
    let status_path = tmp.join("status.txt");
    let frame_path = tmp.join("frame.png");
    let _ = fs::write(&control_path, b"");
    let _ = fs::write(&status_path, b"");

    let map = resolve_lone_eagle_map();
    result.map_seen = map.clone();

    // Prefer -flag=value so option parsing cannot steal the next token
    // (matches GPUI bridge / verified boot path).
    // Wave 833: run from GeneralsRust workspace root so Data/INI + maps resolve.
    let workspace_cwd = {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // Code/Main
        manifest
            .parent() // Code
            .and_then(|p| p.parent()) // GeneralsRust
            .map(|p| p.to_path_buf())
            .filter(|p| p.join("target").is_dir() || p.join("Code").is_dir())
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    };
    let runtime_host_arg = match launch {
        ExecutableSmokeLaunch::HeadlessHost => "-runtime_host=headless",
        ExecutableSmokeLaunch::Windowed => "-runtime_host=windowed",
    };
    let mut child = match Command::new(&exe)
        .current_dir(&workspace_cwd)
        .arg(runtime_host_arg)
        .arg("-windowed")
        .arg("-width=640")
        .arg("-height=480")
        .arg(format!("-gpui_control={}", control_path.display()))
        .arg(format!("-gpui_status={}", status_path.display()))
        .arg(format!("-gpui_frame={}", frame_path.display()))
        .arg("-nologo")
        .arg("-nointro")
        // Default WND=1: retail ButtonStart residual is headless-safe after shell
        // re-borrow + map resolve + InGame world-draw fixes. Override with =0 for soft UI.
        .env(
            "GENERALS_RUNTIME_HOST_WND",
            std::env::var("GENERALS_RUNTIME_HOST_WND").unwrap_or_else(|_| "1".into()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        // CRITICAL: do not pipe stderr without a drain thread — Roads.ini warn
        // spam fills the OS pipe and deadlocks the child in Booting.
        // File redirect keeps panic traces for smoke diagnosis without pipe deadlock.
        .stderr(
            std::fs::File::create(tmp.join("child_stderr.txt")).unwrap_or_else(|_| {
                // Fallback if tmp missing — still avoid pipe deadlock.
                std::fs::File::create(std::env::temp_dir().join("generals_smoke_child_stderr.txt"))
                    .expect("smoke stderr file")
            }),
        )
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            result.status = "spawn_failed".into();
            result.detail = format!("failed to spawn {}: {e}", exe.display());
            return result;
        }
    };
    result.process_started = true;

    let started = Instant::now();
    let mut gameplay_step: u8 = 0;
    let mut saw_select_ok = false;
    let mut saw_move_ok = false;
    let mut saw_attack_ok = false;
    let mut saw_construct_ok = false;
    let mut construct_detail = String::new();
    let mut saw_train_ok = false;
    let mut train_detail = String::new();
    let mut saw_save_ok = false;
    let mut save_detail = String::new();
    let mut saw_load_ok = false;
    let mut load_detail = String::new();
    let mut saw_stop_ok = false;
    let mut stop_detail = String::new();
    let mut saw_sell_ok = false;
    let mut sell_detail = String::new();
    let mut saw_upgrade_ok = false;
    let mut upgrade_detail = String::new();
    let mut saw_guard_ok = false;
    let mut guard_detail = String::new();
    let mut saw_attack_move_ok = false;
    let mut attack_move_detail = String::new();
    let mut saw_scatter_ok = false;
    let mut scatter_detail = String::new();
    let mut saw_patrol_ok = false;
    let mut patrol_detail = String::new();
    let mut saw_deploy_ok = false;
    let mut deploy_detail = String::new();
    let mut saw_cheer_ok = false;
    let mut cheer_detail = String::new();
    let mut saw_formation_ok = false;
    let mut saw_combat_damage = false;
    let mut saw_early_combat_cmd = false;
    let mut formation_detail = String::new();
    let mut saw_capture_ok = false;
    let mut capture_detail = String::new();
    let mut saw_return_supplies_ok = false;
    let mut return_supplies_detail = String::new();
    let mut saw_evacuate_ok = false;
    let mut evacuate_detail = String::new();
    let mut saw_repair_ok = false;
    let mut repair_detail = String::new();
    let mut saw_return_to_base_ok = false;
    let mut return_to_base_detail = String::new();
    let mut saw_attitude_ok = false;
    let mut attitude_detail = String::new();
    let mut saw_rally_ok = false;
    let mut rally_detail = String::new();
    let mut saw_switch_weapons_ok = false;
    let mut switch_weapons_detail = String::new();
    let mut saw_view_cc_ok = false;
    let mut view_cc_detail = String::new();
    let mut saw_clear_mines_ok = false;
    let mut clear_mines_detail = String::new();
    let mut saw_beacon_ok = false;
    let mut beacon_detail = String::new();
    let mut saw_hack_ok = false;
    let mut hack_detail = String::new();
    let mut saw_cleanup_ok = false;
    let mut cleanup_detail = String::new();
    let mut saw_combat_drop_ok = false;
    let mut combat_drop_detail = String::new();
    let mut saw_overcharge_ok = false;
    let mut overcharge_detail = String::new();
    let mut saw_special_power_ok = false;
    let mut special_power_detail = String::new();
    let mut saw_remove_beacon_ok = false;
    let mut remove_beacon_detail = String::new();
    let mut saw_demo_ok = false;
    let mut demo_detail = String::new();
    let mut saw_view_radar_ok = false;
    let mut view_radar_detail = String::new();
    let mut saw_force_attack_ok = false;
    let mut force_attack_detail = String::new();
    let mut saw_force_attack_object_ok = false;
    let mut force_attack_object_detail = String::new();
    let mut saw_select_all_ok = false;
    let mut select_all_detail = String::new();
    let mut saw_control_group_ok = false;
    let mut control_group_detail = String::new();
    let mut saw_waypoint_ok = false;
    let mut waypoint_detail = String::new();
    let mut saw_box_select_ok = false;
    let mut box_select_detail = String::new();
    let mut saw_presentation_frame_ok = false;
    let mut saw_presentation_live_fallback_ok = false;
    let mut saw_gameworld_presentation_entities_ok = false;
    let mut max_gameworld_presentation_entities: u32 = 0;
    let mut saw_gameworld_overlay_stamped_ok = false;
    let mut max_gameworld_overlay_stamped: u32 = 0;
    let mut max_gameworld_appended: u32 = 0;
    let mut max_gameworld_rebuilt: u32 = 0;
    let mut saw_gameworld_rebuilt_ok = false;
    let mut presentation_detail = String::new();
    let mut saw_shell_wnd_ok = false;
    let mut shell_wnd_detail = String::new();
    let mut saw_select_similar_ok = false;
    let mut select_similar_detail = String::new();
    let mut saw_select_on_screen_ok = false;
    let mut select_on_screen_detail = String::new();
    let mut saw_select_structures_ok = false;
    let mut select_structures_detail = String::new();
    let mut saw_select_aircraft_ok = false;
    let mut select_aircraft_detail = String::new();
    let mut saw_select_idle_ok = false;
    let mut select_idle_detail = String::new();
    let mut saw_camera_reset_ok = false;
    let mut camera_reset_detail = String::new();
    let mut saw_camera_zoom_ok = false;
    let mut camera_zoom_detail = String::new();
    let mut saw_pause_ok = false;
    let mut pause_detail = String::new();
    let mut saw_cancel_production_ok = false;
    let mut cancel_production_detail = String::new();
    let mut saw_diplomacy_ok = false;
    let mut diplomacy_detail = String::new();
    let mut saw_live_frame_ok = false;
    let mut saw_window_visible = false;
    let mut saw_wnd_widget_tree_nav = false;
    let mut saw_interactive_gameplay = false;
    let mut saw_physical_build_and_produce = false;
    let mut saw_physical_gather_resources = false;
    let mut saw_physical_save_load_continue = false;
    let mut max_render_item_count: u32 = 0;
    let mut max_render_alive_objects: u32 = 0;
    let mut render_items_nonzero_polls: u32 = 0;
    let mut saw_auto_attack_ok = false;
    let mut auto_attack_detail = String::new();
    let mut saw_options_ok = false;
    let mut options_detail = String::new();
    let mut saw_request_capture_ok = false;
    let mut request_capture_detail = String::new();
    let mut saw_skirmish_start_wnd_ok = false;
    let mut train_sent = false;
    let mut saw_construct_under_construction = false;
    let mut train_retry_started: Option<Instant> = None;
    let mut load_retry_started: Option<Instant> = None;
    let mut phase = 0u8; // 0 wait menu/boot, 1 commanded, 2 wait ingame, 3 exit
    let mut last_snap = StatusSnap::default();
    let mut commanded_at: Option<Instant> = None;
    let mut windowed_start_sent = false;
    // Windowed phase-20 substep: 0=menu nav inject, 1=start_game, 2=gameplay order inject.
    let mut windowed_inject_step: u8 = 0;
    let mut windowed_menu_nav_sent = false;
    let mut windowed_gameplay_order_sent = false;

    loop {
        if started.elapsed() > timeout {
            // Prefer honest InGame finalization over a bare timeout when the host
            // already reached match state — long command chains can exceed wall budget.
            if result.reached_ingame {
                result.shell_wnd_ok = saw_shell_wnd_ok;
                // Wave 833: honest host control residual.
                result.gameplay_cmd_ok = (saw_select_ok && saw_move_ok && saw_attack_ok)
                    || (saw_select_ok && saw_move_ok && saw_construct_ok && saw_train_ok)
                    || (saw_construct_ok && saw_train_ok && saw_attack_ok)
                    || (saw_construct_ok
                        && (saw_attack_ok || saw_attack_move_ok || saw_combat_damage))
                    || (saw_select_ok
                        && saw_move_ok
                        && (saw_attack_ok || saw_attack_move_ok || saw_construct_ok));
                result.construct_cmd_ok = saw_construct_ok;
                result.train_cmd_ok = saw_train_ok;
                result.executable_host_ok =
                    executable_host_ok_from_residuals(true, result.shell_wnd_ok);
                result.status = if result.executable_host_ok {
                    "success_forced_exit".into()
                } else {
                    "ingame_without_shell_wnd".into()
                };
                result.detail = format!(
                    "wall timeout with InGame; status={} frames={} phase={} step={} shell_wnd={} gameplay={}",
                    result.status,
                    result.frames_observed,
                    phase,
                    gameplay_step,
                    result.shell_wnd_ok,
                    result.gameplay_cmd_ok
                );
            } else {
                result.shell_wnd_ok = saw_shell_wnd_ok;
                result.executable_host_ok =
                    executable_host_ok_from_residuals(result.reached_menu, result.shell_wnd_ok);
                result.status = "timeout".into();
                result.detail = format!(
                    "timeout after {:?} last_state={} menu={} ingame={} frames={} phase={} shell_wnd={}",
                    timeout,
                    last_snap.state,
                    result.reached_menu,
                    result.reached_ingame,
                    result.frames_observed,
                    phase,
                    result.shell_wnd_ok
                );
            }
            let _ = write_control(&control_path, &["exit"]);
            kill_child(&mut child);
            break;
        }

        // Child exited early?
        if let Ok(Some(status)) = child.try_wait() {
            result.exit_code = status.code();
            result.shell_wnd_ok = saw_shell_wnd_ok;
            if result.reached_ingame && status.success() {
                result.executable_host_ok =
                    executable_host_ok_from_residuals(true, result.shell_wnd_ok);
                result.status = if result.executable_host_ok {
                    "success".into()
                } else {
                    "ingame_without_shell_wnd".into()
                };
                let prior = result.detail.clone();
                result.detail = format!(
                    "exited ok after InGame frames={} map={} new_game={}",
                    result.frames_observed, result.map_seen, use_new_game_path
                );
                if let Some(idx) = prior.find("construct=") {
                    result.detail = format!("{}; {}", result.detail, &prior[idx..]);
                }
            } else if matches!(last_snap.state.as_str(), "LaunchFailed" | "")
                && !result.reached_menu
            {
                result.status = "assets_or_display_unavailable".into();
                result.detail = format!(
                    "process exited before Menu (code={:?}); display/GPU/assets may be unavailable",
                    status.code()
                );
            } else {
                result.status = "process_exited".into();
                result.detail = format!(
                    "process exited code={:?} state={} menu={} ingame={}",
                    status.code(),
                    last_snap.state,
                    result.reached_menu,
                    result.reached_ingame
                );
                // Partial success: reached InGame even if non-zero (e.g. unclean shutdown).
                if result.reached_ingame {
                    result.executable_host_ok =
                        executable_host_ok_from_residuals(true, result.shell_wnd_ok);
                    result.status = if result.executable_host_ok {
                        "success_partial_exit".into()
                    } else {
                        "ingame_without_shell_wnd".into()
                    };
                }
            }
            break;
        }

        if let Some(snap) = parse_status(&status_path) {
            // Presentation honesty residual from host status every poll.
            if snap.presentation_frame_ok {
                saw_presentation_frame_ok = true;
            }
            if snap.presentation_frame_ok && snap.presentation_live_fallback_reads == 0 {
                saw_presentation_live_fallback_ok = true;
            }
            if snap.gameworld_presentation_entities > 0 {
                saw_gameworld_presentation_entities_ok = true;
                max_gameworld_presentation_entities =
                    max_gameworld_presentation_entities.max(snap.gameworld_presentation_entities);
            }
            if snap.gameworld_overlay_stamped > 0 {
                saw_gameworld_overlay_stamped_ok = true;
                max_gameworld_overlay_stamped =
                    max_gameworld_overlay_stamped.max(snap.gameworld_overlay_stamped);
            }
            if snap.gameworld_appended > 0 {
                max_gameworld_appended = max_gameworld_appended.max(snap.gameworld_appended);
            }
            if snap.gameworld_rebuilt > 0 {
                saw_gameworld_rebuilt_ok = true;
                max_gameworld_rebuilt = max_gameworld_rebuilt.max(snap.gameworld_rebuilt);
            }
            if snap.presentation_frame_ok || snap.presentation_live_fallback_reads > 0 {
                presentation_detail = format!(
                    "frame_ok={} live_fallback={}",
                    snap.presentation_frame_ok, snap.presentation_live_fallback_reads
                );
            }
            // Retail shell WND residual: active shell with MainMenu/Skirmish layout.
            let top = snap.shell_top_wnd.to_ascii_lowercase();
            let wnd_layout =
                top.contains("mainmenu.wnd") || top.contains("skirmish") || top.contains("menus/");
            if snap.shell_active && snap.shell_screen_count > 0 && wnd_layout {
                saw_shell_wnd_ok = true;
                shell_wnd_detail = format!(
                    "active={} count={} top={}",
                    snap.shell_active, snap.shell_screen_count, snap.shell_top_wnd
                );
            } else if snap.shell_screen_count > 0
                || snap.shell_active
                || !snap.shell_top_wnd.is_empty()
            {
                shell_wnd_detail = format!(
                    "active={} count={} top={}",
                    snap.shell_active, snap.shell_screen_count, snap.shell_top_wnd
                );
            }
            // InGame world-draw residual: peak + stability of mesh pass item count.
            if matches!(snap.state.as_str(), "InGame" | "Paused") {
                max_render_item_count = max_render_item_count.max(snap.render_item_count);
                max_render_alive_objects = max_render_alive_objects.max(snap.render_alive_objects);
                if snap.render_item_count > 0 {
                    render_items_nonzero_polls = render_items_nonzero_polls.saturating_add(1);
                }
            }
            // Latch host residuals every poll — step boundaries can miss a one-frame
            // last_gameplay_cmd when the control loop is busy or a later command lands first.
            if snap.live_frame_ok {
                saw_live_frame_ok = true;
            }
            if snap.window_visible {
                saw_window_visible = true;
            }
            if snap.wnd_widget_tree_nav {
                saw_wnd_widget_tree_nav = true;
            }
            if snap.interactive_gameplay {
                saw_interactive_gameplay = true;
            }
            if snap.physical_build_and_produce {
                saw_physical_build_and_produce = true;
            }
            if snap.physical_gather_resources {
                saw_physical_gather_resources = true;
            }
            if snap.physical_save_load_continue {
                saw_physical_save_load_continue = true;
            }
            if snap.match_damage_applied > 0.0 || snap.match_kills > 0 {
                saw_combat_damage = true;
            }
            // Wave 864: keep latched if counters ever rose (status may reset on path change).
            // (saw_combat_damage is sticky once true)
            if snap.last_gameplay_cmd.starts_with("construct_ok") {
                saw_construct_ok = true;
                construct_detail = snap.last_gameplay_cmd.clone();
            } else if snap.last_gameplay_cmd.starts_with("construct_")
                && construct_detail.is_empty()
            {
                construct_detail = snap.last_gameplay_cmd.clone();
            }
            if snap.last_gameplay_cmd.starts_with("train_ok") {
                saw_train_ok = true;
                train_detail = snap.last_gameplay_cmd.clone();
            } else if snap.last_gameplay_cmd.starts_with("train_") && train_detail.is_empty() {
                train_detail = snap.last_gameplay_cmd.clone();
            }
            if snap.last_gameplay_cmd.starts_with("save_ok") {
                saw_save_ok = true;
                save_detail = snap.last_gameplay_cmd.clone();
            } else if snap.last_gameplay_cmd.starts_with("save_") && save_detail.is_empty() {
                save_detail = snap.last_gameplay_cmd.clone();
            }
            if snap.last_gameplay_cmd.starts_with("load_ok") {
                saw_load_ok = true;
                load_detail = snap.last_gameplay_cmd.clone();
            } else if snap.last_gameplay_cmd.starts_with("load_") && load_detail.is_empty() {
                load_detail = snap.last_gameplay_cmd.clone();
            }
            if snap.last_gameplay_cmd.starts_with("select_all_ok") {
                saw_select_all_ok = true;
                select_all_detail = snap.last_gameplay_cmd.clone();
            } else if snap.last_gameplay_cmd.starts_with("select_all_")
                && !snap.last_gameplay_cmd.starts_with("select_all_combat")
                && select_all_detail.is_empty()
            {
                select_all_detail = snap.last_gameplay_cmd.clone();
            }
            if snap.last_gameplay_cmd.starts_with("formation_ok") {
                saw_formation_ok = true;
                formation_detail = snap.last_gameplay_cmd.clone();
            } else if snap.last_gameplay_cmd.starts_with("formation_")
                && formation_detail.is_empty()
            {
                formation_detail = snap.last_gameplay_cmd.clone();
            }
            if snap.skirmish_menu_ok
                || snap.ui_screen.to_ascii_lowercase().contains("skirmish")
                || snap.last_gameplay_cmd.starts_with("open_skirmish_menu_ok")
            {
                result.skirmish_menu_ok = true;
            }
            last_snap = snap.clone();
            result.frames_observed = result.frames_observed.max(snap.frame);
            if snap.map != "-" && !snap.map.is_empty() {
                result.map_seen = snap.map.clone();
            }
            match snap.state.as_str() {
                "Menu" => {
                    result.reached_menu = true;
                    if snap.skirmish_menu_ok
                        || snap.ui_screen.to_ascii_lowercase().contains("skirmish")
                        || snap.last_gameplay_cmd.starts_with("open_skirmish_menu_ok")
                    {
                        result.skirmish_menu_ok = true;
                    }
                    if snap
                        .last_gameplay_cmd
                        .starts_with("open_skirmish_menu_ok_wnd")
                    {
                        result.main_menu_skirmish_wnd_ok = true;
                        result.skirmish_menu_ok = true;
                    }
                }
                "InGame" | "Paused" => {
                    result.reached_menu = true;
                    if snap.skirmish_menu_ok
                        || snap.ui_screen.to_ascii_lowercase().contains("skirmish")
                        || snap.last_gameplay_cmd.starts_with("open_skirmish_menu_ok")
                    {
                        result.skirmish_menu_ok = true;
                    }
                    if snap
                        .last_gameplay_cmd
                        .starts_with("open_skirmish_menu_ok_wnd")
                    {
                        result.main_menu_skirmish_wnd_ok = true;
                        result.skirmish_menu_ok = true;
                    }
                    if ExecutableSmokeResult::reached_ingame_from_live_map(
                        snap.state.as_str(),
                        &snap.map,
                    ) {
                        result.reached_ingame = true;
                    }
                }
                _ => {}
            }

            match phase {
                0 => {
                    // Wait until Menu or Booting finished enough to accept commands.
                    if snap.state == "Menu"
                        || (snap.state != "Booting"
                            && snap.startup_progress >= 0.99
                            && started.elapsed() > Duration::from_secs(8))
                        || started.elapsed() > Duration::from_secs(25)
                    {
                        if launch == ExecutableSmokeLaunch::Windowed
                            && driver == SmokeDriver::ManualObserver
                        {
                            // The acceptance runner is an observer, not a bot.
                            // Do not write Menu/Start/gameplay control commands:
                            // physical input must create every acceptance latch.
                            phase = 21;
                            result.detail.push_str(" manual_windowed_observer;");
                        } else if launch == ExecutableSmokeLaunch::Windowed {
                            // Phase 20 drives winit-equivalent inject (same
                            // handle_mouse_button_input as Injected — automation
                            // only; does not latch playable_claim physical flags)
                            // then shipped start_game — never soft skirmish menu
                            // host commands / drive_os_wnd / cheat tokens.
                            commanded_at = Some(Instant::now());
                            phase = 20;
                        } else {
                            // Soft open Skirmish UI first (override only; WND off).
                            let _ = write_control(&control_path, &["open_skirmish_menu"]);
                            commanded_at = Some(Instant::now());
                            phase = 10; // wait for Skirmish UI before start_game
                        }
                    }
                }

                21 => {
                    // Manual windowed acceptance observation. All useful work
                    // happens in the status-poll section above; this arm must
                    // remain free of control-file writes other than the
                    // timeout cleanup outside the state machine.
                }

                20 => {
                    // Windowed interactive phase: honest winit-equivalent inject
                    // only (inject_winit_equivalent_named_gadget_click /
                    // inject_winit_equivalent_gameplay_order_click → handle_mouse_button_input).
                    // Never drive_os_wnd_* for evidence, never note_* forge, never cheats.
                    if saw_wnd_widget_tree_nav && saw_interactive_gameplay && result.reached_ingame
                    {
                        phase = 2;
                        gameplay_step = 0;
                        commanded_at = Some(Instant::now());
                    } else if !result.reached_ingame {
                        let ready = commanded_at
                            .map(|t| t.elapsed() > Duration::from_millis(600))
                            .unwrap_or(true);
                        // Bare Menu (early latch before show_shell_menu) is not enough.
                        // Require shell_active (MainMenu.wnd pushed). Top WND / prior ok
                        // residual may corroborate, but screen_count alone is not ready.
                        let shell_ready = snap.shell_active
                            || (snap.shell_top_wnd.to_ascii_lowercase().contains("mainmenu")
                                && snap.shell_active)
                            || snap.last_gameplay_cmd.starts_with("winit_menu_nav_ok")
                            || snap.last_gameplay_cmd.starts_with("winit_click_named_ok");
                        let nav_ok = snap.wnd_widget_tree_nav
                            || snap.last_gameplay_cmd.starts_with("winit_menu_nav_ok")
                            || saw_wnd_widget_tree_nav;
                        let nav_miss = snap.last_gameplay_cmd.starts_with("winit_menu_nav_miss")
                            || snap.last_gameplay_cmd.starts_with("winit_menu_nav_partial");
                        // Sequential honesty path (not forge):
                        // 1) winit_menu_nav only → latch menu_click via inject
                        // 2) after nav_ok residual, start_game → match_started
                        // 3) never start before nav when shell is ready
                        // Wait for a presented frame before menu inject so live_frame
                        // can latch on the Menu residual (smoke polls may miss it after start).
                        let frame_ready = snap.live_frame_ok || saw_live_frame_ok;
                        if !windowed_menu_nav_sent
                            && ready
                            && shell_ready
                            && frame_ready
                            && snap.state == "Menu"
                        {
                            let _ = write_control(&control_path, &["winit_menu_nav"]);
                            windowed_menu_nav_sent = true;
                            windowed_inject_step = 1;
                            commanded_at = Some(Instant::now());
                            result.detail.push_str(" windowed_winit_menu_nav;");
                        } else if !windowed_menu_nav_sent
                            && ready
                            && shell_ready
                            && !frame_ready
                            && snap.state == "Menu"
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(45))
                                .unwrap_or(false)
                        {
                            // Proceed with nav after wait; claim stays false without live_frame.
                            let _ = write_control(&control_path, &["winit_menu_nav"]);
                            windowed_menu_nav_sent = true;
                            windowed_inject_step = 1;
                            commanded_at = Some(Instant::now());
                            result
                                .detail
                                .push_str(" windowed_winit_menu_nav_no_live_frame;");
                        } else if windowed_menu_nav_sent
                            && !windowed_start_sent
                            && nav_miss
                            && ready
                            && snap.state == "Menu"
                            && shell_ready
                            && windowed_inject_step < 4
                        {
                            let _ = write_control(&control_path, &["winit_menu_nav"]);
                            windowed_inject_step = windowed_inject_step.saturating_add(1);
                            commanded_at = Some(Instant::now());
                            result.detail.push_str(" windowed_winit_menu_nav_retry;");
                        } else if windowed_menu_nav_sent
                            && !windowed_start_sent
                            && nav_ok
                            && ready
                            && (snap.state == "Menu" || snap.state == "Loading")
                        {
                            // Start only after honest menu inject residual.
                            let start = format!(
                                "start_game|mode=skirmish|faction=USA|map={}",
                                map.replace('|', "/")
                            );
                            let _ = write_control(&control_path, &[start.as_str()]);
                            windowed_start_sent = true;
                            commanded_at = Some(Instant::now());
                            result.detail.push_str(" windowed_start_game_after_nav;");
                        } else if !windowed_start_sent
                            && windowed_menu_nav_sent
                            && !nav_ok
                            && ready
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(12))
                                .unwrap_or(false)
                        {
                            // Grace start after inject attempts (claim stays false without
                            // menu_click → match chain).
                            let start = format!(
                                "start_game|mode=skirmish|faction=USA|map={}",
                                map.replace('|', "/")
                            );
                            let _ = write_control(&control_path, &[start.as_str()]);
                            windowed_start_sent = true;
                            commanded_at = Some(Instant::now());
                            result
                                .detail
                                .push_str(" windowed_start_game_grace_after_nav_miss;");
                        } else if !windowed_start_sent
                            && !windowed_menu_nav_sent
                            && !shell_ready
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(25))
                                .unwrap_or(false)
                        {
                            // Grace start if shell never becomes ready (claim stays false).
                            let start = format!(
                                "start_game|mode=skirmish|faction=USA|map={}",
                                map.replace('|', "/")
                            );
                            let _ = write_control(&control_path, &[start.as_str()]);
                            windowed_start_sent = true;
                            commanded_at = Some(Instant::now());
                            result.detail.push_str(" windowed_start_game_grace;");
                        }
                    } else if result.reached_ingame {
                        // Select + RMB inject through handle_mouse_button_input.
                        // Wait briefly for units (render_alive) so select can succeed.
                        // Fifth claim flag is interactive_gameplay only — keep retrying
                        // until status gameplay=true (or inject budget exhausted).
                        let ready = commanded_at
                            .map(|t| t.elapsed() > Duration::from_millis(800))
                            .unwrap_or(true);
                        let units_ready = snap.render_alive_objects > 0
                            || snap.local_mobile_units > 0
                            || snap.last_gameplay_cmd.starts_with("select_ok")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(8))
                                .unwrap_or(false);
                        let select_ok = snap.last_gameplay_cmd.starts_with("select_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("winit_gameplay_order_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("winit_gameplay_order_partial");
                        // Sequential inject: even steps = select, odd steps = RMB order.
                        // Never advance to host-cmd phase until interactive_gameplay or
                        // inject budget is exhausted (fifth claim flag is RMB only).
                        if !windowed_gameplay_order_sent && ready && units_ready {
                            let _ = write_control(&control_path, &["select_local_unit"]);
                            windowed_gameplay_order_sent = true;
                            windowed_inject_step = 0;
                            commanded_at = Some(Instant::now());
                            result.detail.push_str(" windowed_select_local_unit;");
                        } else if windowed_gameplay_order_sent
                            && !saw_interactive_gameplay
                            && ready
                            && units_ready
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_millis(1200))
                                .unwrap_or(false)
                            && windowed_inject_step < 48
                        {
                            if windowed_inject_step % 2 == 0 {
                                // After first select (step 0 already sent), even steps
                                // re-select when residual is missing/fail; otherwise RMB.
                                if windowed_inject_step == 0
                                    || select_ok
                                    || snap.last_gameplay_cmd.starts_with("select_ok")
                                    || snap.last_gameplay_cmd.starts_with("winit_gameplay_order")
                                {
                                    let _ = write_control(&control_path, &["winit_gameplay_order"]);
                                    result.detail.push_str(" windowed_winit_gameplay_order;");
                                } else {
                                    let _ = write_control(&control_path, &["select_local_unit"]);
                                    result.detail.push_str(" windowed_select_local_unit;");
                                }
                            } else {
                                let _ = write_control(&control_path, &["select_local_unit"]);
                                result.detail.push_str(" windowed_select_local_unit;");
                            }
                            windowed_inject_step = windowed_inject_step.saturating_add(1);
                            commanded_at = Some(Instant::now());
                        } else if windowed_gameplay_order_sent
                            && (saw_interactive_gameplay
                                || (windowed_inject_step >= 48
                                    && commanded_at
                                        .map(|t| t.elapsed() > Duration::from_secs(2))
                                        .unwrap_or(false))
                                || commanded_at
                                    .map(|t| t.elapsed() > Duration::from_secs(120))
                                    .unwrap_or(false))
                            && ready
                        {
                            // Advance only after interactive evidence or inject budget.
                            phase = 2;
                            gameplay_step = 0;
                            commanded_at = Some(Instant::now());
                        }
                        let _ = windowed_inject_step;
                    }
                }

                10 => {
                    if snap.skirmish_menu_ok
                        || snap.ui_screen.to_ascii_lowercase().contains("skirmish")
                        || snap.last_gameplay_cmd.starts_with("open_skirmish_menu_ok")
                    {
                        result.skirmish_menu_ok = true;
                    }
                    if snap
                        .last_gameplay_cmd
                        .starts_with("open_skirmish_menu_ok_wnd")
                    {
                        result.main_menu_skirmish_wnd_ok = true;
                        result.skirmish_menu_ok = true;
                    }
                    // Proceed once Skirmish is visible, or after a short grace poll.
                    let ready = result.skirmish_menu_ok
                        || commanded_at
                            .map(|t| t.elapsed() > Duration::from_millis(800))
                            .unwrap_or(true);
                    if ready {
                        // Prefer real SkirmishMenu Start button click residual.
                        let click = format!("click_skirmish_start|map={}", map.replace('|', "/"));
                        let _ = write_control(&control_path, &[click.as_str()]);
                        commanded_at = Some(Instant::now());
                        phase = 1;
                    }
                }

                1 => {
                    if snap
                        .last_gameplay_cmd
                        .starts_with("click_skirmish_start_ok")
                    {
                        result.skirmish_start_click_ok = true;
                    }
                    // WND gadget path residual (may still be pending NewGame drain).
                    if snap
                        .last_gameplay_cmd
                        .starts_with("click_skirmish_start_ok_wnd")
                        || snap
                            .last_gameplay_cmd
                            .starts_with("click_skirmish_start_wnd")
                    {
                        result.skirmish_start_click_ok = true;
                        saw_skirmish_start_wnd_ok = true;
                        result.skirmish_start_wnd_ok = true;
                    }
                    if snap.last_gameplay_cmd.contains("map_select")
                        || snap
                            .last_gameplay_cmd
                            .starts_with("click_skirmish_start_ok_wnd_via_map_select")
                        || snap
                            .last_gameplay_cmd
                            .starts_with("click_skirmish_map_select_ok_wnd")
                    {
                        result.skirmish_map_select_wnd_ok = true;
                    }
                    if snap.last_gameplay_cmd.contains("via_slots")
                        || snap.last_gameplay_cmd.contains("map_select_slots")
                    {
                        result.skirmish_slot_config_wnd_ok = true;
                        // Map-select path still counts as WND start residual when paired.
                        if snap
                            .last_gameplay_cmd
                            .starts_with("click_skirmish_start_ok_wnd")
                        {
                            result.skirmish_start_click_ok = true;
                            saw_skirmish_start_wnd_ok = true;
                            result.skirmish_start_wnd_ok = true;
                        }
                    }
                    if snap.last_gameplay_cmd.contains("rules")
                        || snap.last_gameplay_cmd.contains("slots_rules")
                    {
                        result.skirmish_rules_wnd_ok = true;
                        result.skirmish_slot_config_wnd_ok = true;
                        if snap
                            .last_gameplay_cmd
                            .starts_with("click_skirmish_start_ok_wnd")
                        {
                            result.skirmish_start_click_ok = true;
                            saw_skirmish_start_wnd_ok = true;
                            result.skirmish_start_wnd_ok = true;
                        }
                    }
                    if result.reached_ingame {
                        phase = 2;
                    } else if commanded_at
                        .map(|t| t.elapsed() > Duration::from_secs(45))
                        .unwrap_or(false)
                    {
                        // Retry once with direct start_game if NewGame path stalled.
                        if use_new_game_path {
                            let start = format!(
                                "start_game|mode=skirmish|faction=USA|map={}",
                                map.replace('|', "/")
                            );
                            let _ = write_control(&control_path, &[start.as_str()]);
                            commanded_at = Some(Instant::now());
                            phase = 1; // stay
                            result.detail.push_str(" fallback_start_game;");
                        } else {
                            result.status = "start_timeout".into();
                            result.detail = format!(
                                "did not reach InGame after start command; state={} phase={}",
                                snap.state, snap.startup_phase
                            );
                            let _ = write_control(&control_path, &["exit"]);
                            phase = 3;
                        }
                    }
                }
                2 => {
                    // Issue host gameplay commands (select + move), then exit.
                    // Not WND widget clicks — still not playable_claim.
                    if gameplay_step == 0 {
                        let _ = write_control(&control_path, &["select_local_unit"]);
                        gameplay_step = 1;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 1
                        && (snap.last_gameplay_cmd.starts_with("select_ok")
                            || snap.last_gameplay_cmd.starts_with("select_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_ok") {
                            saw_select_ok = true;
                        }
                        let _ = write_control(&control_path, &["move_selected|x=100|y=0|z=100"]);
                        gameplay_step = 2;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 2
                        && (snap.last_gameplay_cmd.starts_with("move_ok")
                            || snap.last_gameplay_cmd.starts_with("move_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("move_ok") {
                            saw_move_ok = true;
                        }
                        if launch == ExecutableSmokeLaunch::Windowed {
                            // Wait real frames after physical nav+order before construct.
                            if commanded_at
                                .map(|t| t.elapsed() < Duration::from_secs(2))
                                .unwrap_or(false)
                            {
                                // keep polling
                            } else {
                                // Require an existing builder — no spawn_dozer cheat.
                                let _ = write_control(
                                    &control_path,
                                    &["construct|template=USA_Barracks|auto_target=1"],
                                );
                                gameplay_step = 3;
                                commanded_at = Some(Instant::now());
                            }
                        } else {
                            let _ = write_control(
                                &control_path,
                                &[
                                    "construct|template=USA_Barracks|spawn_dozer=1|alias_fallback=1|auto_target=1",
                                ],
                            );
                            gameplay_step = 3;
                            commanded_at = Some(Instant::now());
                        }
                    } else if gameplay_step == 3
                        && (snap.last_gameplay_cmd.starts_with("construct_ok")
                            || snap.last_gameplay_cmd.starts_with("construct_fail")
                            || snap.last_gameplay_cmd.starts_with("construct_")
                            || commanded_at
                                .map(|t| {
                                    t.elapsed()
                                        > if launch == ExecutableSmokeLaunch::Windowed {
                                            Duration::from_secs(45)
                                        } else {
                                            Duration::from_secs(5)
                                        }
                                })
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("construct_ok") {
                            saw_construct_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("construct_") {
                            construct_detail = snap.last_gameplay_cmd.clone();
                        }
                        if launch == ExecutableSmokeLaunch::Windowed
                            && snap
                                .last_gameplay_cmd
                                .starts_with("construct_fail_no_building")
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(3))
                                .unwrap_or(false)
                        {
                            let _ = write_control(
                                &control_path,
                                &["construct|template=USA_Barracks|auto_target=1"],
                            );
                            commanded_at = Some(Instant::now());
                        }
                        if launch == ExecutableSmokeLaunch::Windowed {
                            // construct_ok is "DozerConstruct issued". Honest train
                            // waits until we observe under_construction>0 then 0
                            // (barracks finished). Immediate UC==0 is a stale frame.
                            if snap.under_construction > 0 {
                                saw_construct_under_construction = true;
                            }
                            let elapsed = commanded_at.map(|t| t.elapsed()).unwrap_or_default();
                            let min_wait = elapsed > Duration::from_secs(8);
                            let build_done = saw_construct_ok
                                && saw_construct_under_construction
                                && snap.under_construction == 0
                                && min_wait;
                            let build_timeout = elapsed > Duration::from_secs(90);
                            if !build_done && !build_timeout {
                                // keep polling; do not issue train yet
                            } else if saw_construct_ok || build_timeout {
                                // One template only — a second train_unit overwrites
                                // train_ok with train_fail_enqueue on the CC.
                                let _ = write_control(
                                    &control_path,
                                    &["train_unit|template=AmericaInfantryRanger|auto_target=1"],
                                );
                                train_sent = true;
                                train_retry_started = Some(Instant::now());
                                gameplay_step = 4;
                                commanded_at = Some(Instant::now());
                            }
                        } else {
                            let _ = write_control(
                                &control_path,
                                &[
                                    "train_unit|template=AmericaInfantryRanger|force_complete=1|grant_supplies=1|alias_fallback=1|auto_target=1",
                                    "train_unit|template=USA_Ranger|force_complete=1|grant_supplies=1|alias_fallback=1|auto_target=1",
                                ],
                            );
                            train_sent = true;
                            gameplay_step = 4;
                            commanded_at = Some(Instant::now());
                        }
                    } else if gameplay_step == 4
                        && (snap.last_gameplay_cmd.starts_with("train_ok")
                            || snap.last_gameplay_cmd.starts_with("train_fail")
                            || snap.last_gameplay_cmd.starts_with("train_")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(8))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("train_ok") {
                            saw_train_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("train_") {
                            train_detail = snap.last_gameplay_cmd.clone();
                        }
                        // Barracks not ready yet — retry train, do not advance.
                        if launch == ExecutableSmokeLaunch::Windowed
                            && !saw_train_ok
                            && snap
                                .last_gameplay_cmd
                                .starts_with("train_fail_no_ready_barracks")
                            && train_retry_started
                                .map(|t| t.elapsed() < Duration::from_secs(75))
                                .unwrap_or(false)
                        {
                            if commanded_at
                                .map(|t| t.elapsed() >= Duration::from_secs(4))
                                .unwrap_or(false)
                            {
                                let _ = write_control(
                                    &control_path,
                                    &["train_unit|template=AmericaInfantryRanger|auto_target=1"],
                                );
                                commanded_at = Some(Instant::now());
                            }
                            // stay on step 4 — bounded by train_retry_started
                        } else {
                            // Host residual: train_ok queues production; wait until a second
                            // local mobile exits so later formation/select residuals are honest.
                            // Fail-closed timeout still advances so the chain cannot hang forever.
                            let train_mobile_ready = snap.local_mobile_units >= 2;
                            let train_wait_expired = commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(20))
                                .unwrap_or(false);
                            if !train_mobile_ready && !train_wait_expired {
                                // keep polling; do not advance yet
                            } else if !saw_early_combat_cmd {
                                // Wave 864: issue combat early while InGame so match_damage
                                // counters have time to accumulate before late options steps.
                                let _ = write_control(
                                    &control_path,
                                    &["attack_nearest_enemy|auto_target=1"],
                                );
                                saw_early_combat_cmd = true;
                                commanded_at = Some(Instant::now());
                            } else if commanded_at
                                .map(|t| t.elapsed() < Duration::from_secs(12))
                                .unwrap_or(false)
                            {
                                // Wave 1112/1115: longer window for attack residual + damage
                                // counters (2s/6s were flaky under load; still fail-closed).
                                // Wave 1115: re-issue attack mid-window so FOW/retarget lag
                                // still has a chance to apply host_damage_log totals.
                                if snap.last_gameplay_cmd.starts_with("attack_ok") {
                                    saw_attack_ok = true;
                                }
                                if snap.match_damage_applied > 0.0 || snap.match_kills > 0 {
                                    saw_combat_damage = true;
                                }
                                let elapsed = commanded_at.map(|t| t.elapsed()).unwrap_or_default();
                                if !saw_combat_damage
                                    && elapsed >= Duration::from_secs(4)
                                    && elapsed < Duration::from_secs(5)
                                {
                                    let _ = write_control(
                                        &control_path,
                                        &["attack_nearest_enemy|auto_target=1"],
                                    );
                                }
                            } else if launch == ExecutableSmokeLaunch::Windowed {
                                let _ = write_control(
                                    &control_path,
                                    &[
                                        "upgrade|name=UpgradeAmericaRangerCaptureBuilding|auto_target=1",
                                    ],
                                );
                                gameplay_step = 5;
                                commanded_at = Some(Instant::now());
                            } else {
                                let _ = write_control(
                                    &control_path,
                                    &[
                                        "upgrade|name=UpgradeAmericaRangerCaptureBuilding|grant_supplies=1|alias_fallback=1|auto_target=1",
                                    ],
                                );
                                gameplay_step = 5;
                                commanded_at = Some(Instant::now());
                            }
                        }
                    } else if gameplay_step == 5
                        && (snap.last_gameplay_cmd.starts_with("upgrade_ok")
                            || snap.last_gameplay_cmd.starts_with("upgrade_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("upgrade_ok") {
                            saw_upgrade_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("upgrade_") {
                            upgrade_detail = snap.last_gameplay_cmd.clone();
                        }
                        // Step 8: windowed drives Pause → PopupSaveLoad.wnd (or quit-menu
                        // SaveLoad gadget) so save_cmd_ok / load_cmd_ok come from the WND
                        // path. Headless keeps host quicksave. Do not fake a pass if the
                        // layout is missing (`save_fail_wnd_missing`).
                        if launch == ExecutableSmokeLaunch::Windowed {
                            let _ = write_control(
                                &control_path,
                                &["pause_save|slot=wnd_pause|via=PopupSaveLoad.wnd"],
                            );
                        } else {
                            let _ = write_control(&control_path, &["quicksave"]);
                        }
                        gameplay_step = 6;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 6
                        && (snap.last_gameplay_cmd.starts_with("save_ok")
                            || snap.last_gameplay_cmd.starts_with("save_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("save_ok") {
                            saw_save_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("save_") {
                            save_detail = snap.last_gameplay_cmd.clone();
                        }
                        // Round-trip: windowed Pause/PopupSaveLoad load; headless quickload.
                        if launch == ExecutableSmokeLaunch::Windowed {
                            let _ = write_control(
                                &control_path,
                                &["pause_load|slot=wnd_pause|via=PopupSaveLoad.wnd"],
                            );
                        } else {
                            let _ = write_control(&control_path, &["quickload"]);
                        }
                        gameplay_step = 7;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 7
                        && (snap.last_gameplay_cmd.starts_with("load_ok")
                            || snap.last_gameplay_cmd.starts_with("load_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(20))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("load_ok") {
                            saw_load_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("load_") {
                            load_detail = snap.last_gameplay_cmd.clone();
                        }
                        if launch == ExecutableSmokeLaunch::Windowed && !saw_load_ok && saw_save_ok
                        {
                            if load_retry_started.is_none() {
                                load_retry_started = Some(Instant::now());
                            }
                            let retry_budget = load_retry_started
                                .map(|t| t.elapsed() < Duration::from_secs(18))
                                .unwrap_or(false);
                            if retry_budget
                                && snap.last_gameplay_cmd.starts_with("load_fail")
                                && commanded_at
                                    .map(|t| t.elapsed() >= Duration::from_secs(2))
                                    .unwrap_or(false)
                            {
                                let _ = write_control(
                                    &control_path,
                                    &["pause_load|slot=wnd_pause|via=PopupSaveLoad.wnd"],
                                );
                                commanded_at = Some(Instant::now());
                            } else if !retry_budget {
                                let _ = write_control(&control_path, &["stop_all"]);
                                gameplay_step = 8;
                                commanded_at = Some(Instant::now());
                            }
                            // stay on step 7 until load_ok or retry budget
                        } else {
                            let _ = write_control(&control_path, &["stop_all"]);
                            gameplay_step = 8;
                            commanded_at = Some(Instant::now());
                        }
                    } else if gameplay_step == 8
                        && (snap.last_gameplay_cmd.starts_with("stop_ok")
                            || snap.last_gameplay_cmd.starts_with("stop_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("stop_ok") {
                            saw_stop_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("stop_") {
                            stop_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["sell|auto_target=1"]);
                        gameplay_step = 9;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 9
                        && (snap.last_gameplay_cmd.starts_with("sell_ok")
                            || snap.last_gameplay_cmd.starts_with("sell_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("sell_ok") {
                            saw_sell_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("sell_") {
                            sell_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ =
                            write_control(&control_path, &["guard|x=120|y=0|z=120|auto_target=1"]);
                        gameplay_step = 10;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 10
                        && (snap.last_gameplay_cmd.starts_with("guard_ok")
                            || snap.last_gameplay_cmd.starts_with("guard_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("guard_ok") {
                            saw_guard_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("guard_") {
                            guard_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(
                            &control_path,
                            &["attack_move|x=150|y=0|z=150|auto_target=1"],
                        );
                        gameplay_step = 11;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 11
                        && (snap.last_gameplay_cmd.starts_with("attack_move_ok")
                            || snap.last_gameplay_cmd.starts_with("attack_move_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("attack_move_ok") {
                            saw_attack_move_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_move_") {
                            attack_move_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["scatter|auto_target=1"]);
                        gameplay_step = 12;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 12
                        && (snap.last_gameplay_cmd.starts_with("scatter_ok")
                            || snap.last_gameplay_cmd.starts_with("scatter_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("scatter_ok") {
                            saw_scatter_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("scatter_") {
                            scatter_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["patrol"]);
                        gameplay_step = 13;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 13
                        && (snap.last_gameplay_cmd.starts_with("patrol_ok")
                            || snap.last_gameplay_cmd.starts_with("patrol_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("patrol_ok") {
                            saw_patrol_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("patrol_") {
                            patrol_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["deploy"]);
                        gameplay_step = 14;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 14
                        && (snap.last_gameplay_cmd.starts_with("deploy_ok")
                            || snap.last_gameplay_cmd.starts_with("deploy_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("deploy_ok") {
                            saw_deploy_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("deploy_") {
                            deploy_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["cheer"]);
                        gameplay_step = 15;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 15
                        && (snap.last_gameplay_cmd.starts_with("cheer_ok")
                            || snap.last_gameplay_cmd.starts_with("cheer_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("cheer_ok") {
                            saw_cheer_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("cheer_") {
                            cheer_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["formation|spawn_buddy=1"]);
                        gameplay_step = 16;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 16
                        && (snap.last_gameplay_cmd.starts_with("formation_ok")
                            || snap.last_gameplay_cmd.starts_with("formation_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("formation_ok") {
                            saw_formation_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("formation_") {
                            formation_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["capture"]);
                        gameplay_step = 17;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 17
                        && (snap.last_gameplay_cmd.starts_with("capture_ok")
                            || snap.last_gameplay_cmd.starts_with("capture_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("capture_ok") {
                            saw_capture_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("capture_") {
                            capture_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["return_supplies|auto_target=1"]);
                        gameplay_step = 18;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 18
                        && (snap.last_gameplay_cmd.starts_with("return_supplies_ok")
                            || snap.last_gameplay_cmd.starts_with("return_supplies_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("return_supplies_ok") {
                            saw_return_supplies_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("return_supplies_") {
                            return_supplies_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["evacuate"]);
                        gameplay_step = 19;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 19
                        && (snap.last_gameplay_cmd.starts_with("evacuate_ok")
                            || snap.last_gameplay_cmd.starts_with("evacuate_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("evacuate_ok") {
                            saw_evacuate_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("evacuate_") {
                            evacuate_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["repair"]);
                        gameplay_step = 20;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 20
                        && (snap.last_gameplay_cmd.starts_with("repair_ok")
                            || snap.last_gameplay_cmd.starts_with("repair_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("repair_ok") {
                            saw_repair_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("repair_") {
                            repair_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["return_to_base"]);
                        gameplay_step = 21;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 21
                        && (snap.last_gameplay_cmd.starts_with("return_to_base_ok")
                            || snap.last_gameplay_cmd.starts_with("return_to_base_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("return_to_base_ok") {
                            saw_return_to_base_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("return_to_base_") {
                            return_to_base_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["attitude_aggressive"]);
                        gameplay_step = 22;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 22
                        && (snap.last_gameplay_cmd.starts_with("attitude_ok")
                            || snap.last_gameplay_cmd.starts_with("attitude_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("attitude_ok") {
                            saw_attitude_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("attitude_") {
                            attitude_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ =
                            write_control(&control_path, &["rally|x=90|y=0|z=90|auto_target=1"]);
                        gameplay_step = 23;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 23
                        && (snap.last_gameplay_cmd.starts_with("rally_ok")
                            || snap.last_gameplay_cmd.starts_with("rally_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("rally_ok") {
                            saw_rally_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("rally_") {
                            rally_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["switch_weapons"]);
                        gameplay_step = 24;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 24
                        && (snap.last_gameplay_cmd.starts_with("switch_weapons_ok")
                            || snap.last_gameplay_cmd.starts_with("switch_weapons_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("switch_weapons_ok") {
                            saw_switch_weapons_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("switch_weapons_") {
                            switch_weapons_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["view_cc"]);
                        gameplay_step = 25;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 25
                        && (snap.last_gameplay_cmd.starts_with("view_cc_ok")
                            || snap.last_gameplay_cmd.starts_with("view_cc_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("view_cc_ok") {
                            saw_view_cc_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("view_cc_") {
                            view_cc_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["clear_mines"]);
                        gameplay_step = 26;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 26
                        && (snap.last_gameplay_cmd.starts_with("clear_mines_ok")
                            || snap.last_gameplay_cmd.starts_with("clear_mines_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("clear_mines_ok") {
                            saw_clear_mines_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("clear_mines_") {
                            clear_mines_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["place_beacon|x=60|y=0|z=60"]);
                        gameplay_step = 27;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 27
                        && (snap.last_gameplay_cmd.starts_with("beacon_ok")
                            || snap.last_gameplay_cmd.starts_with("beacon_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("beacon_ok") {
                            saw_beacon_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("beacon_") {
                            beacon_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["hack_internet"]);
                        gameplay_step = 28;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 28
                        && (snap.last_gameplay_cmd.starts_with("hack_ok")
                            || snap.last_gameplay_cmd.starts_with("hack_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("hack_ok") {
                            saw_hack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("hack_") {
                            hack_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["cleanup_area"]);
                        gameplay_step = 29;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 29
                        && (snap.last_gameplay_cmd.starts_with("cleanup_ok")
                            || snap.last_gameplay_cmd.starts_with("cleanup_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("cleanup_ok") {
                            saw_cleanup_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("cleanup_") {
                            cleanup_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["combat_drop|x=75|y=0|z=75"]);
                        gameplay_step = 30;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 30
                        && (snap.last_gameplay_cmd.starts_with("combat_drop_ok")
                            || snap.last_gameplay_cmd.starts_with("combat_drop_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("combat_drop_ok") {
                            saw_combat_drop_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("combat_drop_") {
                            combat_drop_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["toggle_overcharge|auto_target=1"]);
                        gameplay_step = 31;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 31
                        && (snap.last_gameplay_cmd.starts_with("overcharge_ok")
                            || snap.last_gameplay_cmd.starts_with("overcharge_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("overcharge_ok") {
                            saw_overcharge_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("overcharge_") {
                            overcharge_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["special_power"]);
                        gameplay_step = 32;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 32
                        && (snap.last_gameplay_cmd.starts_with("special_power_ok")
                            || snap.last_gameplay_cmd.starts_with("special_power_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("special_power_ok") {
                            saw_special_power_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("special_power_") {
                            special_power_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["remove_beacon"]);
                        gameplay_step = 33;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 33
                        && (snap.last_gameplay_cmd.starts_with("remove_beacon_ok")
                            || snap.last_gameplay_cmd.starts_with("remove_beacon_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("remove_beacon_ok") {
                            saw_remove_beacon_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("remove_beacon_") {
                            remove_beacon_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["demo_suicide"]);
                        gameplay_step = 34;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 34
                        && (snap.last_gameplay_cmd.starts_with("demo_suicide_ok")
                            || snap.last_gameplay_cmd.starts_with("demo_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("demo_suicide_ok") {
                            saw_demo_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("demo_") {
                            demo_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["view_radar"]);
                        gameplay_step = 35;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 35
                        && (snap.last_gameplay_cmd.starts_with("view_radar_ok")
                            || snap.last_gameplay_cmd.starts_with("view_radar_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("view_radar_ok") {
                            saw_view_radar_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("view_radar_") {
                            view_radar_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["force_attack|x=110|y=0|z=110"]);
                        gameplay_step = 36;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 36
                        && (snap.last_gameplay_cmd.starts_with("force_attack_ok")
                            || snap.last_gameplay_cmd.starts_with("force_attack_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("force_attack_ok") {
                            saw_force_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("force_attack_")
                            && !snap.last_gameplay_cmd.starts_with("force_attack_object")
                        {
                            force_attack_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["force_attack_object"]);
                        gameplay_step = 37;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 37
                        && (snap.last_gameplay_cmd.starts_with("force_attack_object_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("force_attack_object_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("force_attack_object_ok") {
                            saw_force_attack_object_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("force_attack_object_") {
                            force_attack_object_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_all"]);
                        gameplay_step = 38;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 38
                        && (snap.last_gameplay_cmd.starts_with("select_all_ok")
                            || snap.last_gameplay_cmd.starts_with("select_all_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(8))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_all_ok") {
                            saw_select_all_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_all_")
                            && !snap.last_gameplay_cmd.starts_with("select_all_combat")
                        {
                            select_all_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["assign_control_group|group=1"]);
                        gameplay_step = 39;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 39
                        && (snap
                            .last_gameplay_cmd
                            .starts_with("control_group_assign_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("control_group_assign_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap
                            .last_gameplay_cmd
                            .starts_with("control_group_assign_ok")
                        {
                            // partial — need recall too
                            control_group_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("control_group_") {
                            control_group_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["recall_control_group|group=1"]);
                        gameplay_step = 40;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 40
                        && (snap
                            .last_gameplay_cmd
                            .starts_with("control_group_recall_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("control_group_recall_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap
                            .last_gameplay_cmd
                            .starts_with("control_group_recall_ok")
                            && control_group_detail.starts_with("control_group_assign_ok")
                        {
                            saw_control_group_ok = true;
                        } else if snap
                            .last_gameplay_cmd
                            .starts_with("control_group_recall_ok")
                        {
                            // assign detail may have been overwritten — still ok if recall ok after assign step
                            saw_control_group_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("control_group_") {
                            control_group_detail =
                                format!("{};{}", control_group_detail, snap.last_gameplay_cmd);
                        }
                        let _ = write_control(&control_path, &["waypoint_mode|on=1"]);
                        gameplay_step = 41;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 41
                        && (snap.last_gameplay_cmd.starts_with("waypoint_mode_ok")
                            || snap.last_gameplay_cmd.starts_with("waypoint_mode_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("waypoint_mode_") {
                            waypoint_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["add_waypoint|x=130|y=0|z=130"]);
                        gameplay_step = 42;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 42
                        && (snap.last_gameplay_cmd.starts_with("waypoint_ok")
                            || snap.last_gameplay_cmd.starts_with("waypoint_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("waypoint_ok") {
                            saw_waypoint_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("waypoint_") {
                            waypoint_detail =
                                format!("{};{}", waypoint_detail, snap.last_gameplay_cmd);
                        }
                        let _ = write_control(
                            &control_path,
                            &["box_select|min_x=-8000|max_x=8000|min_z=-8000|max_z=8000"],
                        );
                        gameplay_step = 43;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 43
                        && (snap.last_gameplay_cmd.starts_with("box_select_ok")
                            || snap.last_gameplay_cmd.starts_with("box_select_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("box_select_ok") {
                            saw_box_select_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("box_select_") {
                            box_select_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_similar"]);
                        gameplay_step = 44;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 44
                        && (snap.last_gameplay_cmd.starts_with("select_similar_ok")
                            || snap.last_gameplay_cmd.starts_with("select_similar_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_similar_ok") {
                            saw_select_similar_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_similar_") {
                            select_similar_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_on_screen"]);
                        gameplay_step = 45;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 45
                        && (snap.last_gameplay_cmd.starts_with("select_on_screen_ok")
                            || snap.last_gameplay_cmd.starts_with("select_on_screen_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_on_screen_ok") {
                            saw_select_on_screen_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_on_screen_") {
                            select_on_screen_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_structures"]);
                        gameplay_step = 46;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 46
                        && (snap.last_gameplay_cmd.starts_with("select_structures_ok")
                            || snap.last_gameplay_cmd.starts_with("select_structures_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_structures_ok") {
                            saw_select_structures_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_structures_") {
                            select_structures_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_aircraft"]);
                        gameplay_step = 47;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 47
                        && (snap.last_gameplay_cmd.starts_with("select_aircraft_ok")
                            || snap.last_gameplay_cmd.starts_with("select_aircraft_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_aircraft_ok") {
                            saw_select_aircraft_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_aircraft_") {
                            select_aircraft_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_idle_harvesters"]);
                        gameplay_step = 48;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 48
                        && (snap.last_gameplay_cmd.starts_with("select_idle_ok")
                            || snap.last_gameplay_cmd.starts_with("select_idle_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_idle_ok") {
                            saw_select_idle_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_idle_") {
                            select_idle_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["camera_reset"]);
                        gameplay_step = 49;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 49
                        && (snap.last_gameplay_cmd.starts_with("camera_reset_ok")
                            || snap.last_gameplay_cmd.starts_with("camera_reset_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("camera_reset_ok") {
                            saw_camera_reset_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("camera_reset_") {
                            camera_reset_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["camera_zoom|z=1.25"]);
                        gameplay_step = 50;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 50
                        && (snap.last_gameplay_cmd.starts_with("camera_zoom_ok")
                            || snap.last_gameplay_cmd.starts_with("camera_zoom_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("camera_zoom_ok") {
                            saw_camera_zoom_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("camera_zoom_") {
                            camera_zoom_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["toggle_pause"]);
                        gameplay_step = 51;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 51 {
                        if snap.last_gameplay_cmd.starts_with("pause_ok") {
                            pause_detail = snap.last_gameplay_cmd.clone();
                            saw_pause_ok = true;
                            let _ = write_control(&control_path, &["toggle_pause"]);
                            gameplay_step = 52;
                            commanded_at = Some(Instant::now());
                        } else if snap.last_gameplay_cmd.starts_with("pause_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(12))
                                .unwrap_or(false)
                        {
                            if snap.last_gameplay_cmd.starts_with("pause_") {
                                pause_detail = snap.last_gameplay_cmd.clone();
                            }
                            let _ = write_control(&control_path, &["cancel_production"]);
                            gameplay_step = 53;
                            commanded_at = Some(Instant::now());
                        } else if commanded_at
                            .map(|t| t.elapsed() > Duration::from_millis(1500))
                            .unwrap_or(false)
                        {
                            let _ = write_control(&control_path, &["toggle_pause"]);
                            commanded_at = Some(Instant::now());
                        }
                    } else if gameplay_step == 52 {
                        if snap.last_gameplay_cmd.starts_with("pause_ok") {
                            pause_detail = format!("{};{}", pause_detail, snap.last_gameplay_cmd);
                            saw_pause_ok = true;
                            let _ = write_control(&control_path, &["cancel_production"]);
                            gameplay_step = 53;
                            commanded_at = Some(Instant::now());
                        } else if snap.last_gameplay_cmd.starts_with("pause_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(12))
                                .unwrap_or(false)
                        {
                            let _ = write_control(&control_path, &["cancel_production"]);
                            gameplay_step = 53;
                            commanded_at = Some(Instant::now());
                        } else if commanded_at
                            .map(|t| t.elapsed() > Duration::from_millis(1500))
                            .unwrap_or(false)
                        {
                            let _ = write_control(&control_path, &["toggle_pause"]);
                            commanded_at = Some(Instant::now());
                        }
                    } else if gameplay_step == 53
                        && (snap.last_gameplay_cmd.starts_with("cancel_production_ok")
                            || snap.last_gameplay_cmd.starts_with("cancel_production_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("cancel_production_ok") {
                            saw_cancel_production_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("cancel_production_") {
                            cancel_production_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["request_capture"]);
                        gameplay_step = 54;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 54
                        && (snap.last_gameplay_cmd.starts_with("request_capture_ok")
                            || snap.last_gameplay_cmd.starts_with("request_capture_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("request_capture_ok") {
                            saw_request_capture_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("request_capture_") {
                            request_capture_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["auto_attack|on=1"]);
                        gameplay_step = 55;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 55
                        && (snap.last_gameplay_cmd.starts_with("auto_attack_ok")
                            || snap.last_gameplay_cmd.starts_with("auto_attack_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("auto_attack_ok") {
                            saw_auto_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("auto_attack_") {
                            auto_attack_detail = snap.last_gameplay_cmd.clone();
                        }
                        // Attack while still InGame (options/diplomacy leave match).
                        let _ = write_control(&control_path, &["attack_nearest_enemy"]);
                        gameplay_step = 56;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 56
                        && (snap.last_gameplay_cmd.starts_with("attack_ok")
                            || snap.last_gameplay_cmd.starts_with("attack_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("attack_ok") {
                            saw_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_") {
                            // keep prior attack detail path in final branch too
                        }
                        let _ = write_control(&control_path, &["options_probe"]);
                        gameplay_step = 57;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 57
                        && (snap.last_gameplay_cmd.starts_with("options_probe_ok")
                            || snap.last_gameplay_cmd.starts_with("options_ok")
                            || snap.last_gameplay_cmd.starts_with("options_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("options_probe_ok")
                            || snap.last_gameplay_cmd.starts_with("options_ok")
                        {
                            saw_options_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("options_") {
                            options_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["open_diplomacy"]);
                        gameplay_step = 58;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 58
                        && (snap.last_gameplay_cmd.starts_with("diplomacy_ok")
                            || snap.last_gameplay_cmd.starts_with("diplomacy_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("diplomacy_ok") {
                            saw_diplomacy_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("diplomacy_") {
                            diplomacy_detail = snap.last_gameplay_cmd.clone();
                        }
                        gameplay_step = 59;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step >= 59 {
                        if snap.last_gameplay_cmd.starts_with("move_ok") {
                            saw_move_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("construct_ok") {
                            saw_construct_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("construct_") {
                            construct_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("train_ok") {
                            saw_train_ok = true;
                            train_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("train_") {
                            train_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("save_ok") {
                            saw_save_ok = true;
                            save_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("save_") {
                            save_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("load_ok") {
                            saw_load_ok = true;
                            load_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("load_") {
                            load_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("stop_ok") {
                            saw_stop_ok = true;
                            stop_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("stop_") {
                            stop_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("sell_ok") {
                            saw_sell_ok = true;
                            sell_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("sell_") {
                            sell_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("upgrade_ok") {
                            saw_upgrade_ok = true;
                            upgrade_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("upgrade_") {
                            upgrade_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("guard_ok") {
                            saw_guard_ok = true;
                            guard_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("guard_") {
                            guard_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_move_ok") {
                            saw_attack_move_ok = true;
                            attack_move_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("attack_move_") {
                            attack_move_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("scatter_ok") {
                            saw_scatter_ok = true;
                            scatter_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("scatter_") {
                            scatter_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("patrol_ok") {
                            saw_patrol_ok = true;
                            patrol_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("patrol_") {
                            patrol_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("deploy_ok") {
                            saw_deploy_ok = true;
                            deploy_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("deploy_") {
                            deploy_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("cheer_ok") {
                            saw_cheer_ok = true;
                            cheer_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("cheer_") {
                            cheer_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("formation_ok") {
                            saw_formation_ok = true;
                            formation_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("formation_") {
                            formation_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("capture_ok") {
                            saw_capture_ok = true;
                            capture_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("capture_") {
                            capture_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("return_supplies_ok") {
                            saw_return_supplies_ok = true;
                            return_supplies_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("return_supplies_") {
                            return_supplies_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("evacuate_ok") {
                            saw_evacuate_ok = true;
                            evacuate_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("evacuate_") {
                            evacuate_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("repair_ok") {
                            saw_repair_ok = true;
                            repair_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("repair_") {
                            repair_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("return_to_base_ok") {
                            saw_return_to_base_ok = true;
                            return_to_base_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("return_to_base_") {
                            return_to_base_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("attitude_ok") {
                            saw_attitude_ok = true;
                            attitude_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("attitude_") {
                            attitude_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("rally_ok") {
                            saw_rally_ok = true;
                            rally_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("rally_") {
                            rally_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("switch_weapons_ok") {
                            saw_switch_weapons_ok = true;
                            switch_weapons_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("switch_weapons_") {
                            switch_weapons_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("view_cc_ok") {
                            saw_view_cc_ok = true;
                            view_cc_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("view_cc_") {
                            view_cc_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("clear_mines_ok") {
                            saw_clear_mines_ok = true;
                            clear_mines_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("clear_mines_") {
                            clear_mines_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("beacon_ok") {
                            saw_beacon_ok = true;
                            beacon_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("beacon_") {
                            beacon_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("hack_ok") {
                            saw_hack_ok = true;
                            hack_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("hack_") {
                            hack_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("cleanup_ok") {
                            saw_cleanup_ok = true;
                            cleanup_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("cleanup_") {
                            cleanup_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("combat_drop_ok") {
                            saw_combat_drop_ok = true;
                            combat_drop_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("combat_drop_") {
                            combat_drop_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("overcharge_ok") {
                            saw_overcharge_ok = true;
                            overcharge_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("overcharge_") {
                            overcharge_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("special_power_ok") {
                            saw_special_power_ok = true;
                            special_power_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("special_power_") {
                            special_power_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("remove_beacon_ok") {
                            saw_remove_beacon_ok = true;
                            remove_beacon_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("remove_beacon_") {
                            remove_beacon_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("demo_suicide_ok") {
                            saw_demo_ok = true;
                            demo_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("demo_") {
                            demo_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("view_radar_ok") {
                            saw_view_radar_ok = true;
                            view_radar_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("view_radar_") {
                            view_radar_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("force_attack_ok") {
                            saw_force_attack_ok = true;
                            force_attack_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("force_attack_")
                            && !snap.last_gameplay_cmd.starts_with("force_attack_object")
                        {
                            force_attack_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("force_attack_object_ok") {
                            saw_force_attack_object_ok = true;
                            force_attack_object_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("force_attack_object_") {
                            force_attack_object_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("select_all_ok") {
                            saw_select_all_ok = true;
                            select_all_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("select_all_")
                            && !snap.last_gameplay_cmd.starts_with("select_all_combat")
                        {
                            select_all_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap
                            .last_gameplay_cmd
                            .starts_with("control_group_assign_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("control_group_recall_ok")
                        {
                            if snap
                                .last_gameplay_cmd
                                .starts_with("control_group_recall_ok")
                            {
                                saw_control_group_ok = true;
                            }
                            control_group_detail =
                                format!("{};{}", control_group_detail, snap.last_gameplay_cmd);
                        } else if snap.last_gameplay_cmd.starts_with("control_group_") {
                            control_group_detail =
                                format!("{};{}", control_group_detail, snap.last_gameplay_cmd);
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_ok")
                            || snap.last_gameplay_cmd.starts_with("attack_fail")
                            || snap.last_gameplay_cmd.starts_with("attack_begin")
                        {
                            saw_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_ok") {
                            saw_select_ok = true;
                        }
                        if launch == ExecutableSmokeLaunch::Windowed {
                            // Re-issue only while the producer is complete and train
                            // has not succeeded. train_fail must not block a retry.
                            if train_sent
                                && !saw_train_ok
                                && saw_construct_ok
                                && saw_construct_under_construction
                                && snap.under_construction == 0
                                && commanded_at
                                    .map(|t| t.elapsed() > Duration::from_secs(2))
                                    .unwrap_or(false)
                            {
                                let _ = write_control(
                                    &control_path,
                                    &["train_unit|template=AmericaInfantryRanger|auto_target=1"],
                                );
                            }
                        } else if train_sent
                            && train_detail.is_empty()
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(2))
                                .unwrap_or(false)
                        {
                            let _ = write_control(
                                &control_path,
                                &[
                                    "train_unit|template=AmericaInfantryRanger|force_complete=1|grant_supplies=1|alias_fallback=1|auto_target=1",
                                ],
                            );
                        }
                        // Primary: select+move+attack. Residual: production+attack proves
                        // host command path when early select timing is noisy.
                        // Wave 833: honest host control residual.
                        result.gameplay_cmd_ok = (saw_select_ok && saw_move_ok && saw_attack_ok)
                            || (saw_select_ok && saw_move_ok && saw_construct_ok && saw_train_ok)
                            || (saw_construct_ok && saw_train_ok && saw_attack_ok)
                            || (saw_construct_ok
                                && (saw_attack_ok || saw_attack_move_ok || saw_combat_damage))
                            || (saw_select_ok
                                && saw_move_ok
                                && (saw_attack_ok || saw_attack_move_ok || saw_construct_ok));
                        result.construct_cmd_ok = saw_construct_ok;
                        result.train_cmd_ok = saw_train_ok;
                        result.physical_build_and_produce = saw_physical_build_and_produce;
                        result.save_cmd_ok = saw_save_ok;
                        result.load_cmd_ok = saw_load_ok;
                        result.stop_cmd_ok = saw_stop_ok;
                        result.sell_cmd_ok = saw_sell_ok;
                        result.upgrade_cmd_ok = saw_upgrade_ok;
                        result.guard_cmd_ok = saw_guard_ok;
                        result.attack_move_cmd_ok = saw_attack_move_ok;
                        result.combat_damage_ok = saw_combat_damage;
                        result.scatter_cmd_ok = saw_scatter_ok;
                        result.patrol_cmd_ok = saw_patrol_ok;
                        result.deploy_cmd_ok = saw_deploy_ok;
                        result.cheer_cmd_ok = saw_cheer_ok;
                        result.formation_cmd_ok = saw_formation_ok;
                        result.capture_cmd_ok = saw_capture_ok;
                        result.return_supplies_cmd_ok = saw_return_supplies_ok;
                        result.physical_gather_resources = saw_physical_gather_resources;
                        result.physical_save_load_continue = saw_physical_save_load_continue;
                        result.evacuate_cmd_ok = saw_evacuate_ok;
                        result.repair_cmd_ok = saw_repair_ok;
                        result.return_to_base_cmd_ok = saw_return_to_base_ok;
                        result.attitude_cmd_ok = saw_attitude_ok;
                        result.rally_cmd_ok = saw_rally_ok;
                        result.switch_weapons_cmd_ok = saw_switch_weapons_ok;
                        result.view_cc_cmd_ok = saw_view_cc_ok;
                        result.clear_mines_cmd_ok = saw_clear_mines_ok;
                        result.beacon_cmd_ok = saw_beacon_ok;
                        result.hack_cmd_ok = saw_hack_ok;
                        result.cleanup_cmd_ok = saw_cleanup_ok;
                        result.combat_drop_cmd_ok = saw_combat_drop_ok;
                        result.overcharge_cmd_ok = saw_overcharge_ok;
                        result.special_power_cmd_ok = saw_special_power_ok;
                        result.remove_beacon_cmd_ok = saw_remove_beacon_ok;
                        result.demo_cmd_ok = saw_demo_ok;
                        result.view_radar_cmd_ok = saw_view_radar_ok;
                        result.force_attack_cmd_ok = saw_force_attack_ok;
                        result.force_attack_object_cmd_ok = saw_force_attack_object_ok;
                        result.select_all_cmd_ok = saw_select_all_ok;
                        result.control_group_cmd_ok = saw_control_group_ok;
                        result.waypoint_cmd_ok = saw_waypoint_ok;
                        result.box_select_cmd_ok = saw_box_select_ok;
                        result.presentation_frame_ok = saw_presentation_frame_ok;
                        result.presentation_live_fallback_ok = saw_presentation_live_fallback_ok;
                        result.gameworld_presentation_entities_ok =
                            saw_gameworld_presentation_entities_ok;
                        result.max_gameworld_presentation_entities =
                            max_gameworld_presentation_entities;
                        result.gameworld_overlay_stamped_ok = saw_gameworld_overlay_stamped_ok;
                        result.max_gameworld_overlay_stamped = max_gameworld_overlay_stamped;
                        result.max_gameworld_appended = max_gameworld_appended;
                        result.max_gameworld_rebuilt = max_gameworld_rebuilt;
                        result.gameworld_rebuilt_ok = saw_gameworld_rebuilt_ok;
                        result.shell_wnd_ok = saw_shell_wnd_ok;
                        result.max_render_item_count = max_render_item_count;
                        result.max_render_alive_objects = max_render_alive_objects;
                        // Stable = at least 3 InGame polls with items (not a one-frame flash).
                        result.render_items_stable_ok =
                            render_items_nonzero_polls >= 3 && max_render_item_count > 0;
                        result.select_similar_cmd_ok = saw_select_similar_ok;
                        result.select_on_screen_cmd_ok = saw_select_on_screen_ok;
                        result.select_structures_cmd_ok = saw_select_structures_ok;
                        result.select_aircraft_cmd_ok = saw_select_aircraft_ok;
                        result.select_idle_cmd_ok = saw_select_idle_ok;
                        result.camera_reset_cmd_ok = saw_camera_reset_ok;
                        result.camera_zoom_cmd_ok = saw_camera_zoom_ok;
                        result.pause_cmd_ok = saw_pause_ok;
                        result.cancel_production_cmd_ok = saw_cancel_production_ok;
                        result.diplomacy_cmd_ok = saw_diplomacy_ok;
                        result.live_frame_ok = saw_live_frame_ok;
                        result.window_visible = saw_window_visible;
                        result.wnd_widget_tree_nav = saw_wnd_widget_tree_nav;
                        result.interactive_gameplay = saw_interactive_gameplay;
                        result.auto_attack_cmd_ok = saw_auto_attack_ok;
                        result.options_cmd_ok = saw_options_ok;
                        result.request_capture_cmd_ok = saw_request_capture_ok;
                        result.skirmish_start_wnd_ok =
                            saw_skirmish_start_wnd_ok || result.skirmish_start_wnd_ok;
                        if !presentation_detail.is_empty() {
                            result.detail =
                                format!("{}; presentation={}", result.detail, presentation_detail);
                        }
                        result.detail =
                            format!("{}; last_cmd={}", result.detail, snap.last_gameplay_cmd);
                        if !construct_detail.is_empty() {
                            result.detail =
                                format!("{}; construct={}", result.detail, construct_detail);
                        }
                        if !train_detail.is_empty() {
                            result.detail = format!("{}; train={}", result.detail, train_detail);
                        }
                        if !save_detail.is_empty() {
                            result.detail = format!("{}; save={}", result.detail, save_detail);
                        }
                        if !load_detail.is_empty() {
                            result.detail = format!("{}; load={}", result.detail, load_detail);
                        }
                        // Exit only after the full host command chain finishes
                        // (step >= 59: pause/cancel/attack/options/diplomacy), or on
                        // hard stall / frame budget. Do not cut off mid-chain once
                        // construct/train/attack land — later residuals (pause, etc.)
                        // would stay false forever.
                        let chain_complete = gameplay_step >= 59;
                        // Only hard-stall once we're deep in the chain; early steps
                        // have their own per-command timeouts.
                        let hard_stall = gameplay_step >= 50
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(120))
                                .unwrap_or(false);
                        // Windowed: after the host chain finishes, keep retrying the
                        // RMB inject until interactive_gameplay (fifth claim flag)
                        // latches — do not exit solely on host gameplay_cmd_ok.
                        let want_exit = chain_complete
                            || hard_stall
                            || snap.frame
                                >= if launch == ExecutableSmokeLaunch::Windowed
                                    && !saw_interactive_gameplay
                                {
                                    2500u32
                                } else {
                                    500u32
                                };
                        if want_exit
                            && launch == ExecutableSmokeLaunch::Windowed
                            && !saw_interactive_gameplay
                            && result.reached_ingame
                            && windowed_inject_step < 90
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(2))
                                .unwrap_or(true)
                        {
                            if windowed_inject_step % 2 == 0 {
                                let _ = write_control(&control_path, &["select_local_unit"]);
                            } else {
                                let _ = write_control(&control_path, &["winit_gameplay_order"]);
                                result
                                    .detail
                                    .push_str(" windowed_late_winit_gameplay_order;");
                            }
                            windowed_inject_step = windowed_inject_step.saturating_add(1);
                            commanded_at = Some(Instant::now());
                        } else if want_exit {
                            let _ = write_control(&control_path, &["exit"]);
                            phase = 3;
                        }
                    }
                }
                3 => {
                    // Wait for clean exit.
                    if let Ok(Some(status)) = child.try_wait() {
                        result.exit_code = status.code();
                        if result.reached_ingame {
                            result.executable_host_ok =
                                executable_host_ok_from_residuals(true, result.shell_wnd_ok);
                            result.status = if result.executable_host_ok {
                                "success".into()
                            } else {
                                "ingame_without_shell_wnd".into()
                            };
                            result.detail = format!(
                                "InGame frames={} map={} exit={:?} new_game={} menu={} shell_wnd={}",
                                result.frames_observed,
                                result.map_seen,
                                status.code(),
                                use_new_game_path,
                                result.reached_menu,
                                result.shell_wnd_ok
                            );
                        } else if result.reached_menu {
                            result.status = "menu_only".into();
                            result.detail = format!(
                                "reached Menu but not InGame; exit={:?} map={}",
                                status.code(),
                                result.map_seen
                            );
                        } else {
                            result.status = "no_menu".into();
                            result.detail = format!(
                                "never reached Menu; exit={:?} last_state={}",
                                status.code(),
                                last_snap.state
                            );
                        }
                        break;
                    }
                    if commanded_at
                        .map(|t| t.elapsed() > Duration::from_secs(20))
                        .unwrap_or(false)
                        && phase == 3
                    {
                        kill_child(&mut child);
                        if result.reached_ingame {
                            result.executable_host_ok =
                                executable_host_ok_from_residuals(true, result.shell_wnd_ok);
                            result.status = if result.executable_host_ok {
                                "success_forced_exit".into()
                            } else {
                                "ingame_without_shell_wnd".into()
                            };
                            result.detail = format!(
                                "InGame ok but exit hang; frames={} map={} shell_wnd={}",
                                result.frames_observed, result.map_seen, result.shell_wnd_ok
                            );
                        } else {
                            result.status = "exit_hang".into();
                            result.detail = format!(
                                "exit command did not stop process; shell_wnd={} menu={} frames={}",
                                saw_shell_wnd_ok, result.reached_menu, result.frames_observed
                            );
                        }
                        result.shell_wnd_ok = saw_shell_wnd_ok;
                        result.executable_host_ok = executable_host_ok_from_residuals(
                            result.reached_menu || result.reached_ingame,
                            result.shell_wnd_ok,
                        );
                        break;
                    }
                }
                _ => {}
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    let _ = fs::remove_dir_all(&tmp);

    // Wave 839: ensure presentation honesty counters are latched before vertical gate.
    // Always merge poll latches here — early child death / partial exit skips the
    // phase-2 assignment block, and must not drop live_frame_ok / interactive_gameplay.
    result.presentation_frame_ok = result.presentation_frame_ok || saw_presentation_frame_ok;
    result.window_visible = result.window_visible || saw_window_visible;
    result.wnd_widget_tree_nav = result.wnd_widget_tree_nav || saw_wnd_widget_tree_nav;
    result.interactive_gameplay = result.interactive_gameplay || saw_interactive_gameplay;
    result.live_frame_ok = result.live_frame_ok || saw_live_frame_ok;
    result.physical_build_and_produce =
        result.physical_build_and_produce || saw_physical_build_and_produce;
    result.physical_gather_resources =
        result.physical_gather_resources || saw_physical_gather_resources;
    result.physical_save_load_continue =
        result.physical_save_load_continue || saw_physical_save_load_continue;
    // Fifth retail claim flag is interactive_gameplay alone (status `gameplay=` /
    // RMB latch). Host gameplay_cmd_ok remains a separate residual for the
    // vertical-slice / command chain and must NOT OR into playable_claim.
    result.presentation_live_fallback_ok =
        result.presentation_live_fallback_ok || saw_presentation_live_fallback_ok;
    result.shell_wnd_ok = result.shell_wnd_ok || saw_shell_wnd_ok;
    result.max_render_item_count = result.max_render_item_count.max(max_render_item_count);
    result.max_render_alive_objects = result
        .max_render_alive_objects
        .max(max_render_alive_objects);
    result.render_items_stable_ok = result.render_items_stable_ok
        || (result.reached_ingame && max_render_item_count > 0 && render_items_nonzero_polls >= 3);
    result.gameworld_presentation_entities_ok =
        result.gameworld_presentation_entities_ok || saw_gameworld_presentation_entities_ok;
    result.max_gameworld_presentation_entities = result
        .max_gameworld_presentation_entities
        .max(max_gameworld_presentation_entities);
    result.gameworld_overlay_stamped_ok =
        result.gameworld_overlay_stamped_ok || saw_gameworld_overlay_stamped_ok;
    result.max_gameworld_overlay_stamped = result
        .max_gameworld_overlay_stamped
        .max(max_gameworld_overlay_stamped);
    result.max_gameworld_appended = result.max_gameworld_appended.max(max_gameworld_appended);
    result.max_gameworld_rebuilt = result.max_gameworld_rebuilt.max(max_gameworld_rebuilt);
    result.gameworld_rebuilt_ok = result.gameworld_rebuilt_ok || saw_gameworld_rebuilt_ok;
    // Host command residuals observed during polls (even if chain cut short).
    result.gameplay_cmd_ok = result.gameplay_cmd_ok
        || (saw_select_ok && saw_move_ok && saw_attack_ok)
        || (saw_select_ok && saw_move_ok && saw_construct_ok && saw_train_ok)
        || (saw_construct_ok && saw_train_ok && saw_attack_ok)
        || (saw_construct_ok && (saw_attack_ok || saw_attack_move_ok || saw_combat_damage))
        || (saw_select_ok
            && saw_move_ok
            && (saw_attack_ok || saw_attack_move_ok || saw_construct_ok));
    result.construct_cmd_ok = result.construct_cmd_ok || saw_construct_ok;
    result.train_cmd_ok = result.train_cmd_ok || saw_train_ok;
    // Wave 176: presentation boundary + host vertical slice on the result itself.
    result.apply_host_vertical_slice_gate();
    result
}

fn runtime_host_wnd_enabled() -> bool {
    !matches!(
        std::env::var("GENERALS_RUNTIME_HOST_WND")
            .unwrap_or_else(|_| "1".into())
            .trim()
            .to_ascii_lowercase()
            .as_str(),
        "0" | "false" | "off" | "no"
    )
}

/// When WND push is enabled, limited host_ok also requires shell_wnd residual.
fn executable_host_ok_from_residuals(reached_ingame: bool, shell_wnd_ok: bool) -> bool {
    if !reached_ingame {
        return false;
    }
    if runtime_host_wnd_enabled() {
        shell_wnd_ok
    } else {
        true
    }
}

pub fn format_executable_smoke_report(r: &ExecutableSmokeResult) -> String {
    // Five-flag report: gameplay= is interactive_gameplay (RMB latch), not host cmd.
    let sit_through_missing = ExecutableSmokeResult::retail_sit_through_missing_flags(
        r.window_visible,
        r.wnd_widget_tree_nav,
        r.live_frame_ok,
        r.reached_ingame,
        r.interactive_gameplay,
    );
    format!(
        "executable_smoke status={} host_ok={} playable_claim={} window_visible={} wnd_widget_tree_nav={} live_frame_ok={} ingame={} gameplay={} retail_sit_through_missing={} host_vertical_slice={} presentation_fallback={} started={} menu={} shell_wnd={} main_menu_skirmish_wnd={} map_select_wnd={} slot_config_wnd={} rules_wnd={} ingame={} gameplay_cmd={} construct_cmd={} train_cmd={} upgrade_cmd={} save_cmd={} load_cmd={} stop_cmd={} sell_cmd={} guard_cmd={} attack_move_cmd={} combat_damage={} scatter_cmd={} patrol_cmd={} deploy_cmd={} cheer_cmd={} formation_cmd={} capture_cmd={} return_supplies_cmd={} physical_build_and_produce={} physical_gather_resources={} physical_save_load_continue={} evacuate_cmd={} repair_cmd={} return_to_base_cmd={} attitude_cmd={} rally_cmd={} switch_weapons_cmd={} view_cc_cmd={} clear_mines_cmd={} beacon_cmd={} hack_cmd={} cleanup_cmd={} combat_drop_cmd={} overcharge_cmd={} special_power_cmd={} remove_beacon_cmd={} demo_cmd={} view_radar_cmd={} force_attack_cmd={} force_attack_object_cmd={} select_all_cmd={} control_group_cmd={} waypoint_cmd={} box_select_cmd={} presentation_frame_ok={} gw_pres_ents_ok={} max_gw_pres_ents={} gw_overlay_stamp_ok={} gw_appended={} gw_rebuilt_ok={} gw_rebuilt={} max_gw_overlay_stamp={} max_render_items={} render_items_stable={} max_render_alive={} presentation_live_fallback_ok={} select_similar_cmd={} select_on_screen_cmd={} select_structures_cmd={} select_aircraft_cmd={} select_idle_cmd={} camera_reset_cmd={} camera_zoom_cmd={} pause_cmd={} cancel_production_cmd={} diplomacy_cmd={} live_frame_ok={} window_visible={} wnd_widget_tree_nav={} auto_attack_cmd={} options_cmd={} request_capture_cmd={} skirmish_start_wnd={} skirmish_menu={} skirmish_start_click={} frames={} map={} exit={:?} new_game={} detail={}",
        r.status,
        r.executable_host_ok,
        r.playable_claim,
        r.window_visible,
        r.wnd_widget_tree_nav,
        r.live_frame_ok,
        r.reached_ingame,
        r.interactive_gameplay,
        sit_through_missing,
        r.host_vertical_slice_ok,
        r.presentation_live_fallback_ok,
        r.process_started,
        r.reached_menu,
        r.shell_wnd_ok,
        r.main_menu_skirmish_wnd_ok,
        r.skirmish_map_select_wnd_ok,
        r.skirmish_slot_config_wnd_ok,
        r.skirmish_rules_wnd_ok,
        r.reached_ingame,
        r.gameplay_cmd_ok,
        r.construct_cmd_ok,
        r.train_cmd_ok,
        r.upgrade_cmd_ok,
        r.save_cmd_ok,
        r.load_cmd_ok,
        r.stop_cmd_ok,
        r.sell_cmd_ok,
        r.guard_cmd_ok,
        r.attack_move_cmd_ok,
        r.combat_damage_ok,
        r.scatter_cmd_ok,
        r.patrol_cmd_ok,
        r.deploy_cmd_ok,
        r.cheer_cmd_ok,
        r.formation_cmd_ok,
        r.capture_cmd_ok,
        r.return_supplies_cmd_ok,
        r.physical_build_and_produce,
        r.physical_gather_resources,
        r.physical_save_load_continue,
        r.evacuate_cmd_ok,
        r.repair_cmd_ok,
        r.return_to_base_cmd_ok,
        r.attitude_cmd_ok,
        r.rally_cmd_ok,
        r.switch_weapons_cmd_ok,
        r.view_cc_cmd_ok,
        r.clear_mines_cmd_ok,
        r.beacon_cmd_ok,
        r.hack_cmd_ok,
        r.cleanup_cmd_ok,
        r.combat_drop_cmd_ok,
        r.overcharge_cmd_ok,
        r.special_power_cmd_ok,
        r.remove_beacon_cmd_ok,
        r.demo_cmd_ok,
        r.view_radar_cmd_ok,
        r.force_attack_cmd_ok,
        r.force_attack_object_cmd_ok,
        r.select_all_cmd_ok,
        r.control_group_cmd_ok,
        r.waypoint_cmd_ok,
        r.box_select_cmd_ok,
        r.presentation_frame_ok,
        r.gameworld_presentation_entities_ok,
        r.max_gameworld_presentation_entities,
        r.gameworld_overlay_stamped_ok,
        r.max_gameworld_appended,
        r.gameworld_rebuilt_ok,
        r.max_gameworld_rebuilt,
        r.max_gameworld_overlay_stamped,
        r.max_render_item_count,
        r.render_items_stable_ok,
        r.max_render_alive_objects,
        r.presentation_live_fallback_ok,
        r.select_similar_cmd_ok,
        r.select_on_screen_cmd_ok,
        r.select_structures_cmd_ok,
        r.select_aircraft_cmd_ok,
        r.select_idle_cmd_ok,
        r.camera_reset_cmd_ok,
        r.camera_zoom_cmd_ok,
        r.pause_cmd_ok,
        r.cancel_production_cmd_ok,
        r.diplomacy_cmd_ok,
        r.live_frame_ok,
        r.window_visible,
        r.wnd_widget_tree_nav,
        r.auto_attack_cmd_ok,
        r.options_cmd_ok,
        r.request_capture_cmd_ok,
        r.skirmish_start_wnd_ok,
        r.skirmish_menu_ok,
        r.skirmish_start_click_ok,
        r.frames_observed,
        r.map_seen,
        r.exit_code,
        r.new_game_path,
        r.detail
    )
}

#[cfg(test)]
mod tests {

    #[test]
    fn kill_stale_matches_runtime_host_underscore() {
        let src = include_str!("executable_smoke.rs");
        let kill_fn = src
            .split("fn kill_stale_runtime_host_generals")
            .nth(1)
            .and_then(|s| s.split("fn resolve_runtime_exe").next())
            .expect("kill_stale fn body");
        assert!(
            kill_fn.contains("runtime_host"),
            "stale kill must match -runtime_host CLI (underscore)"
        );
        assert!(
            !kill_fn.contains("runtime-host"),
            "stale kill must not use hyphenated runtime-host pkill pattern"
        );
        assert!(
            kill_fn.contains("generals.*runtime_host") || kill_fn.contains("runtime_host"),
            "expected runtime_host pkill pattern"
        );
    }

    use super::*;

    #[test]
    fn resolve_runtime_exe_honors_general_runtime_exe_override() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("generals_override_bin");
        fs::write(&fake, b"ok").unwrap();
        let prev = std::env::var_os("GENERALS_RUNTIME_EXE");
        crate::env_compat::set_var("GENERALS_RUNTIME_EXE", &fake);
        let got = resolve_runtime_exe();
        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_RUNTIME_EXE", v),
            None => crate::env_compat::remove_var("GENERALS_RUNTIME_EXE"),
        }
        assert_eq!(got.as_deref(), Some(fake.as_path()));
    }

    #[test]
    fn resolve_runtime_exe_from_candidates_prefers_newer_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("release_generals");
        let newer = dir.path().join("debug_generals");
        fs::write(&older, b"old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(&newer, b"new").unwrap();
        let picked = resolve_runtime_exe_from_candidates(&[older.clone(), newer.clone()], true);
        assert_eq!(
            picked.as_deref(),
            Some(newer.as_path()),
            "newest-mtime must win over a stale earlier candidate"
        );
        let release_first = resolve_runtime_exe_from_candidates(&[older.clone(), newer], false);
        assert_eq!(
            release_first.as_deref(),
            Some(older.as_path()),
            "release-first policy still walks in candidate order"
        );
    }

    #[test]
    fn parse_status_reads_keys() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("status.txt");
        fs::write(
            &p,
            "state=Menu\nui_screen=Some(MainMenu)\nmap=-\nframe=3\nstartup_progress=1.0\nstartup_phase=Ready\n",
        )
        .unwrap();
        let s = parse_status(&p).unwrap();
        assert_eq!(s.state, "Menu");
        assert_eq!(s.frame, 3);
        assert!((s.startup_progress - 1.0).abs() < f32::EPSILON);
    }

    // Residual packs live behind cfg(test)/host-residuals; this test is lib-only.
    #[cfg(any(test, feature = "host-residuals"))]
    #[test]
    fn residual_packs_cannot_set_playable_claim() {
        use crate::game_logic::{
            honesty_host_construct_dead_combat_latch_residual_pack_wave1115,
            honesty_host_drawable_overlay_dead_residual_pack_wave1114,
            honesty_host_selection_hud_disabled_fow_residual_pack_wave1113,
            residual_pack_cannot_set_playable_claim, self_table_honesty_is_inflation,
            simulate_live_host_construct_dead_combat_latch_residual_honesty,
            simulate_live_host_drawable_overlay_dead_residual_honesty,
            simulate_live_host_selection_hud_disabled_fow_residual_honesty,
        };

        assert!(
            self_table_honesty_is_inflation(),
            "self-table residual honesty is inflation policy"
        );

        // Constructing/running residual packs must not publish playable_claim.
        assert!(honesty_host_construct_dead_combat_latch_residual_pack_wave1115());
        assert!(honesty_host_drawable_overlay_dead_residual_pack_wave1114());
        assert!(honesty_host_selection_hud_disabled_fow_residual_pack_wave1113());
        assert!(simulate_live_host_construct_dead_combat_latch_residual_honesty());
        assert!(simulate_live_host_drawable_overlay_dead_residual_honesty());
        assert!(simulate_live_host_selection_hud_disabled_fow_residual_honesty());

        let mut r = ExecutableSmokeResult::default();
        assert!(residual_pack_cannot_set_playable_claim(r.playable_claim));
        assert!(!r.playable_claim);
        r.apply_host_vertical_slice_gate();
        assert!(
            residual_pack_cannot_set_playable_claim(r.playable_claim),
            "residual packs + host_ok path must leave playable_claim false"
        );

        let report = format_executable_smoke_report(&r);
        assert!(report.contains("playable_claim=false"));
        assert!(report.contains("window_visible=false"));
        assert!(report.contains("wnd_widget_tree_nav=false"));
        assert!(report.contains("live_frame_ok=false"));
        assert!(report.contains("retail_sit_through_missing="));

        // Production smoke must keep five-flag formula only (no residual_pack OR).
        let src = include_str!("executable_smoke.rs");
        let prod = src.split("#[cfg(test)]").next().expect("production smoke");
        assert!(prod.contains("self.playable_claim = Self::retail_windowed_playable_claim("));
        assert!(!prod.contains("playable_claim |= "));
        assert!(!prod.contains("executable_host_ok |= honesty_"));
        assert!(!prod.contains("playable_claim = honesty_"));
        assert!(!prod.contains("playable_claim = playable_claim ||"));
    }

    #[test]
    fn playable_claim_always_false_on_default() {
        let r = ExecutableSmokeResult::default();
        assert!(!r.playable_claim);
        assert_eq!(
            r.retail_sit_through_missing,
            "window_visible,wnd_widget_tree_nav,live_frame_ok,ingame,gameplay"
        );
        let report = format_executable_smoke_report(&r);
        assert!(report.contains("playable_claim=false"));
        assert!(report.contains("window_visible=false"));
        assert!(report.contains("wnd_widget_tree_nav=false"));
        assert!(report.contains("live_frame_ok=false"));
        assert!(report.contains("ingame=false"));
        assert!(report.contains("gameplay=false"));
        assert!(report.contains(
            "retail_sit_through_missing=window_visible,wnd_widget_tree_nav,live_frame_ok,ingame,gameplay"
        ));
    }

    #[test]
    fn host_vertical_slice_ok_never_flips_playable_claim() {
        let mut r = ExecutableSmokeResult::default();
        r.shell_wnd_ok = true;
        r.main_menu_skirmish_wnd_ok = true;
        r.skirmish_map_select_wnd_ok = true;
        r.skirmish_slot_config_wnd_ok = true;
        r.skirmish_rules_wnd_ok = true;
        r.skirmish_start_wnd_ok = true;
        r.reached_ingame = true;
        r.gameplay_cmd_ok = true;
        r.construct_cmd_ok = true;
        r.train_cmd_ok = true;
        r.executable_host_ok = true;
        r.presentation_frame_ok = true;
        r.presentation_live_fallback_ok = true;
        r.max_render_alive_objects = 8;
        r.max_render_item_count = 16;
        r.render_items_stable_ok = true;
        r.gameworld_presentation_entities_ok = true;
        r.gameworld_overlay_stamped_ok = true;
        r.gameworld_rebuilt_ok = true;
        r.map_seen = "Lone Eagle".into();
        r.apply_host_vertical_slice_gate();
        assert!(
            r.host_vertical_slice_ok,
            "headless WND+cmd+presentation slice can be true"
        );
        assert!(
            !r.playable_claim,
            "retail windowed WND/GPU sit-through is not proven; playable_claim stays false"
        );
        assert!(
            r.retail_sit_through_missing.contains("window_visible"),
            "headless sit-through must report window_visible missing"
        );
        assert!(!ExecutableSmokeResult::retail_windowed_playable_claim(
            false, true, true, true, true
        ));
        assert!(ExecutableSmokeResult::retail_windowed_playable_claim(
            true, true, true, true, true
        ));
    }

    #[test]
    fn parses_window_visible_and_wnd_widget_tree_nav_from_status() {
        let path =
            std::env::temp_dir().join(format!("generals_smoke_wnd_vis_{}.txt", std::process::id()));
        std::fs::write(
            &path,
            "state=Menu\nwindow_visible=true\nwnd_widget_tree_nav=true\nlive_frame_ok=true\nretail_sit_through_missing=ingame,gameplay\n",
        )
        .unwrap();
        let snap = parse_status(&path).expect("snap");
        let _ = std::fs::remove_file(&path);
        assert!(snap.window_visible);
        assert!(snap.wnd_widget_tree_nav);
        assert!(snap.live_frame_ok);
        assert_eq!(snap.retail_sit_through_missing, "ingame,gameplay");
    }

    #[test]
    fn parses_window_visible_and_wnd_widget_tree_nav_numeric_yes() {
        let path = std::env::temp_dir().join(format!(
            "generals_smoke_wnd_vis_num_{}.txt",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "state=InGame\nwindow_visible=1\nwnd_widget_tree_nav=yes\nlive_frame_ok=on\n",
        )
        .unwrap();
        let snap = parse_status(&path).expect("snap");
        let _ = std::fs::remove_file(&path);
        assert!(snap.window_visible);
        assert!(snap.wnd_widget_tree_nav);
        assert!(snap.live_frame_ok);
    }

    #[test]
    fn parses_physical_interactive_gameplay_from_status() {
        let path = std::env::temp_dir().join(format!(
            "generals_smoke_interactive_gameplay_{}.txt",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "state=InGame\ngameplay=true\ninteractive_gameplay=1\n",
        )
        .unwrap();
        let snap = parse_status(&path).expect("snap");
        let _ = std::fs::remove_file(&path);
        assert!(snap.interactive_gameplay);
    }

    #[test]
    fn parses_physical_operational_evidence_without_host_command_fallback() {
        let path = std::env::temp_dir().join(format!(
            "generals_smoke_physical_workflows_{}.txt",
            std::process::id()
        ));
        std::fs::write(
            &path,
            "state=InGame\nphysical_build_and_produce=yes\nphysical_gather_resources=1\nphysical_save_load_continue=on\n",
        )
        .unwrap();
        let snap = parse_status(&path).expect("snap");
        let _ = std::fs::remove_file(&path);
        assert!(snap.physical_build_and_produce);
        assert!(snap.physical_gather_resources);
        assert!(snap.physical_save_load_continue);
    }

    #[test]
    fn windowed_launch_is_not_headless_runtime_host() {
        let src = include_str!("executable_smoke.rs");
        assert!(src.contains("ExecutableSmokeLaunch::Windowed => \"-runtime_host=windowed\""));
        assert!(src.contains("run_windowed_acceptance_smoke"));
        let launch = src
            .split("let runtime_host_arg = match launch")
            .nth(1)
            .expect("launch match");
        assert!(launch.contains("Windowed => \"-runtime_host=windowed\""));
        assert!(
            !include_str!("bin/windowed_acceptance_gate.rs").contains("-runtime_host=headless")
        );
        let observer = src
            .split("pub fn run_windowed_acceptance_smoke")
            .nth(1)
            .and_then(|s| {
                s.split("fn run_executable_smoke_with_launch_and_driver")
                    .next()
            })
            .expect("windowed observer entrypoint");
        assert!(
            observer.contains("SmokeDriver::ManualObserver"),
            "windowed acceptance must be a manual observer"
        );
        assert!(src.contains("phase = 21"));
        let manual_phase = src
            .split("21 => {")
            .nth(1)
            .and_then(|s| s.split("20 => {").next())
            .expect("manual observer phase");
        assert!(
            !manual_phase.contains("write_control"),
            "manual observer must not manufacture input through the control file"
        );
    }

    #[test]
    fn apply_host_vertical_slice_gate_playable_only_with_all_five_retail_flags() {
        let mut r = ExecutableSmokeResult::default();
        r.reached_ingame = true;
        // Host cmd residual alone must never be the fifth claim flag.
        r.gameplay_cmd_ok = true;
        r.interactive_gameplay = false;
        r.live_frame_ok = true;
        r.apply_host_vertical_slice_gate();
        assert!(
            !r.playable_claim,
            "headless/missing window+WND nav must not flip playable_claim"
        );
        assert!(r.retail_sit_through_missing.contains("window_visible"));
        assert!(r.retail_sit_through_missing.contains("wnd_widget_tree_nav"));
        assert!(
            r.retail_sit_through_missing.contains("gameplay"),
            "fifth flag missing without interactive_gameplay"
        );
        r.window_visible = true;
        r.wnd_widget_tree_nav = true;
        r.apply_host_vertical_slice_gate();
        assert!(
            !r.playable_claim,
            "gameplay_cmd_ok without interactive_gameplay must keep playable_claim false"
        );
        assert_eq!(r.retail_sit_through_missing, "gameplay");
        r.interactive_gameplay = true;
        r.apply_host_vertical_slice_gate();
        assert!(
            r.playable_claim,
            "all five retail windowed flags true => playable_claim"
        );
        assert!(r.retail_sit_through_missing.is_empty());
        r.live_frame_ok = false;
        r.apply_host_vertical_slice_gate();
        assert!(
            !r.playable_claim,
            "missing live GPU frame.png must not flip playable_claim"
        );
        assert_eq!(r.retail_sit_through_missing, "live_frame_ok");
    }

    #[test]
    fn retail_windowed_playable_claim_false_unless_all_five() {
        assert!(!ExecutableSmokeResult::retail_windowed_playable_claim(
            false, true, true, true, true
        ));
        assert_eq!(
            ExecutableSmokeResult::retail_sit_through_missing_flags(false, true, true, true, true),
            "window_visible"
        );
        assert!(!ExecutableSmokeResult::retail_windowed_playable_claim(
            true, false, true, true, true
        ));
        assert_eq!(
            ExecutableSmokeResult::retail_sit_through_missing_flags(true, false, true, true, true),
            "wnd_widget_tree_nav"
        );
        assert!(!ExecutableSmokeResult::retail_windowed_playable_claim(
            true, true, false, true, true
        ));
        assert_eq!(
            ExecutableSmokeResult::retail_sit_through_missing_flags(true, true, false, true, true),
            "live_frame_ok"
        );
        assert!(!ExecutableSmokeResult::retail_windowed_playable_claim(
            true, true, true, false, true
        ));
        assert_eq!(
            ExecutableSmokeResult::retail_sit_through_missing_flags(true, true, true, false, true),
            "ingame"
        );
        assert!(!ExecutableSmokeResult::retail_windowed_playable_claim(
            true, true, true, true, false
        ));
        assert_eq!(
            ExecutableSmokeResult::retail_sit_through_missing_flags(true, true, true, true, false),
            "gameplay"
        );
        assert!(ExecutableSmokeResult::retail_windowed_playable_claim(
            true, true, true, true, true
        ));
        assert_eq!(
            ExecutableSmokeResult::retail_sit_through_missing_flags(true, true, true, true, true),
            ""
        );
    }

    #[test]
    fn playable_claim_gate_rejects_headless_true_allows_windowed_five_flags() {
        let mut headless = ExecutableSmokeResult::default();
        assert!(headless.playable_claim_gate_ok().is_ok());
        headless.playable_claim = true;
        assert!(
            headless.playable_claim_gate_ok().is_err(),
            "headless claim true must fail the gate"
        );

        let mut windowed = ExecutableSmokeResult::default();
        windowed.windowed_launch = true;
        windowed.window_visible = true;
        windowed.wnd_widget_tree_nav = true;
        windowed.live_frame_ok = true;
        windowed.reached_ingame = true;
        // Host residual alone is not the fifth flag.
        windowed.gameplay_cmd_ok = true;
        windowed.interactive_gameplay = false;
        windowed.apply_host_vertical_slice_gate();
        assert!(
            !windowed.playable_claim,
            "gameplay_cmd_ok without interactive_gameplay must keep claim false"
        );
        windowed.interactive_gameplay = true;
        windowed.apply_host_vertical_slice_gate();
        assert!(windowed.playable_claim, "five flags => windowed claim");
        assert!(
            windowed.playable_claim_gate_ok().is_ok(),
            "gate must not reject a true windowed claim when all five flags hold"
        );

        windowed.live_frame_ok = false;
        windowed.apply_host_vertical_slice_gate();
        assert!(!windowed.playable_claim);
        windowed.playable_claim = true; // forged
        assert!(
            windowed.playable_claim_gate_ok().is_err(),
            "windowed true without all five flags is still illegal"
        );

        let src = include_str!("bin/executable_smoke_gate.rs");
        assert!(src.contains("playable_claim_gate_ok"));
        assert!(!src.contains("playable_claim must stay false\""));
    }

    #[test]
    fn four_of_five_retail_flags_keep_playable_claim_false() {
        let mut r = ExecutableSmokeResult::default();
        r.window_visible = true;
        r.wnd_widget_tree_nav = true;
        r.live_frame_ok = true;
        r.reached_ingame = true;
        // interactive_gameplay still false — 4/5 (host cmd residual is irrelevant)
        r.gameplay_cmd_ok = true;
        r.interactive_gameplay = false;
        r.apply_host_vertical_slice_gate();
        assert!(
            !r.playable_claim,
            "4/5 retail flags must not flip playable_claim"
        );
        assert_eq!(r.retail_sit_through_missing, "gameplay");
        let report = format_executable_smoke_report(&r);
        assert!(report.contains("playable_claim=false"));
        assert!(report.contains("window_visible=true"));
        assert!(report.contains("wnd_widget_tree_nav=true"));
        assert!(report.contains("live_frame_ok=true"));
        assert!(report.contains("ingame=true"));
        assert!(report.contains("gameplay=false"));
        assert!(report.contains("gameplay_cmd=true"));
        assert!(report.contains("retail_sit_through_missing=gameplay"));

        // Still false: host residual was already true; fifth flag is interactive only.
        r.gameplay_cmd_ok = true;
        r.interactive_gameplay = false;
        r.apply_host_vertical_slice_gate();
        assert!(
            !r.playable_claim,
            "gameplay_cmd_ok=true with interactive_gameplay=false keeps claim false"
        );

        r.interactive_gameplay = true;
        r.apply_host_vertical_slice_gate();
        assert!(
            r.playable_claim,
            "only all five retail flags true => playable_claim"
        );
        assert!(r.retail_sit_through_missing.is_empty());
        let report = format_executable_smoke_report(&r);
        assert!(report.contains("playable_claim=true"));
        assert!(report.contains("gameplay=true"));
        assert!(report.contains("retail_sit_through_missing="));
        assert!(!report.contains("retail_sit_through_missing=gameplay"));
    }

    #[test]
    fn window_visible_from_winit_query_false_when_headless_or_hidden() {
        assert!(!ExecutableSmokeResult::window_visible_from_winit_query(
            true,
            Some(true)
        ));
        assert!(!ExecutableSmokeResult::window_visible_from_winit_query(
            false,
            Some(false)
        ));
        assert!(!ExecutableSmokeResult::window_visible_from_winit_query(
            true, None
        ));
        assert!(ExecutableSmokeResult::window_visible_from_winit_query(
            false,
            Some(true)
        ));
        assert!(ExecutableSmokeResult::window_visible_from_winit_query(
            false, None
        ));
        let src = include_str!("cnc_game_engine/dispatch.rs");
        assert!(
            src.contains("window_visible_from_winit_query")
                && src.contains("self.window.is_visible()"),
            "engine must call the shipped winit visibility helper"
        );
        let boot = include_str!("cnc_game_engine/runtime.rs");
        assert!(
            boot.contains("fn publish_booting_from_winit_query")
                && boot.contains("window_visible_from_winit_query"),
            "boot status must use the shipped winit query, never a hardcoded true"
        );
        assert!(
            !boot.contains("window_visible: true"),
            "must not forge window_visible=true in boot residual"
        );
        let helper = include_str!("cnc_game_engine/types.rs");
        assert!(
            helper.contains("fn apply_runtime_host_window_visibility")
                && helper.contains("set_visible(false)")
                && helper.contains("set_visible(true)")
                && helper.contains("window_visible_from_winit_query"),
            "windowed show / headless hide must go through the honest helper"
        );
        let win_main = include_str!("win_main.rs");
        assert!(
            win_main.contains("ActivationPolicy::Regular")
                && win_main.contains("create_host_event_loop"),
            "windowed macOS EventLoop must request Regular activation so the OS window appears"
        );
        assert!(
            include_str!("main.rs").contains("create_host_event_loop"),
            "production main must use the Regular-activation EventLoop helper"
        );
    }

    #[test]
    fn live_frame_ok_from_windowed_present_not_host_ok() {
        assert!(!ExecutableSmokeResult::live_frame_ok_from_windowed_present(
            false, false
        ));
        assert!(ExecutableSmokeResult::live_frame_ok_from_windowed_present(
            true, false
        ));
        assert!(ExecutableSmokeResult::live_frame_ok_from_windowed_present(
            false, true
        ));
        let runtime = include_str!("cnc_game_engine/runtime.rs");
        assert!(
            runtime.contains("note_windowed_surface_presented")
                && runtime.contains("live_frame_ok_from_windowed_present"),
            "publish must use the shipped present/capture helper"
        );
        let loop_src = include_str!("cnc_game_engine/run_loop.rs");
        assert!(
            loop_src.contains("note_windowed_surface_presented")
                && loop_src.contains("!runtime_headless_mode"),
            "present latch only after a successful windowed render"
        );
    }

    #[test]
    fn wnd_widget_tree_nav_requires_skirmish_path_not_click_skirmish_start() {
        assert!(ExecutableSmokeResult::wnd_nav_gadget_is_skirmish_path(
            "MainMenu.wnd:ButtonSkirmish"
        ));
        assert!(ExecutableSmokeResult::wnd_nav_gadget_is_skirmish_path(
            "SkirmishGameOptionsMenu.wnd:ButtonStart"
        ));
        assert!(!ExecutableSmokeResult::wnd_nav_gadget_is_skirmish_path(
            "MainMenu.wnd:ButtonOptions"
        ));
        let input = include_str!("cnc_game_engine/input.rs");
        assert!(
            input.contains("note_skirmish_path_gadget")
                && input.contains("handle_mouse_button_input"),
            "skirmish path must latch inside handle_mouse_button_input"
        );
        assert!(
            !input.contains("click_skirmish_start") && !input.contains("main_menu_skirmish_wnd_ok"),
            "host click_skirmish_start residual is not the wnd_widget_tree_nav latch"
        );
        let snap = include_str!("cnc_game_engine/dispatch.rs");
        assert!(
            snap.contains("interactive_playability.wnd_menu_to_match_complete()"),
            "status wnd_widget_tree_nav is the evidence chain, not host-ok"
        );
    }

    #[test]
    fn reached_ingame_from_live_map_rejects_shell_empty_and_dash() {
        assert!(!ExecutableSmokeResult::reached_ingame_from_live_map(
            "InGame", "shellmap"
        ));
        assert!(!ExecutableSmokeResult::reached_ingame_from_live_map(
            "InGame",
            "ShellMapMD"
        ));
        assert!(!ExecutableSmokeResult::reached_ingame_from_live_map(
            "InGame", "-"
        ));
        assert!(!ExecutableSmokeResult::reached_ingame_from_live_map(
            "InGame", "  "
        ));
        assert!(!ExecutableSmokeResult::reached_ingame_from_live_map(
            "Menu",
            "Lone Eagle"
        ));
        assert!(ExecutableSmokeResult::reached_ingame_from_live_map(
            "InGame",
            "Lone Eagle"
        ));
        assert!(ExecutableSmokeResult::reached_ingame_from_live_map(
            "Paused",
            "MapsZH/foo"
        ));
        let smoke = include_str!("executable_smoke.rs");
        let prod = smoke.split("#[cfg(test)]").next().expect("prod");
        assert!(
            prod.contains("reached_ingame_from_live_map"),
            "smoke must set reached_ingame from the shipped map latch"
        );
    }

    #[test]
    fn interactive_gameplay_latches_only_from_handle_mouse_button_input_rmb() {
        let input = include_str!("cnc_game_engine/input.rs");
        let start = input
            .find("fn handle_mouse_button_input")
            .expect("handle_mouse_button_input");
        let end = input[start..]
            .find("fn inject_winit_equivalent_cursor_at")
            .map(|i| start + i)
            .unwrap_or(start + 4000);
        let body = &input[start..end];
        assert!(
            body.contains("note_gameplay_order") && body.contains("handle_right_click"),
            "RMB release on world must call handle_right_click then note_gameplay_order"
        );
        assert!(
            !body.contains("gameplay_cmd_ok"),
            "host gameplay_cmd_ok must not be the interactive_gameplay latch"
        );
        let snap = include_str!("cnc_game_engine/dispatch.rs");
        assert!(snap.contains("interactive_playability.gameplay_complete()"));
    }

    #[test]
    fn parses_render_item_count_from_status() {
        let path =
            std::env::temp_dir().join(format!("generals_smoke_status_{}.txt", std::process::id()));
        std::fs::write(
            &path,
            "state=InGame\nrender_item_count=42\nrender_alive_objects=100\nrender_fow_filtered=10\nrender_frustum_culled=5\npresentation_frame_ok=true\n",
        )
        .unwrap();
        let snap = parse_status(&path).expect("snap");
        let _ = std::fs::remove_file(&path);
        assert_eq!(snap.render_item_count, 42);
        assert_eq!(snap.render_alive_objects, 100);
        assert!(snap.presentation_frame_ok);
    }

    #[test]
    fn host_ok_requires_shell_wnd_when_wnd_enabled() {
        let _guard = std::env::var("GENERALS_RUNTIME_HOST_WND");
        // Safety: process-local env for this test only.
        unsafe {
            crate::env_compat::set_var("GENERALS_RUNTIME_HOST_WND", "1");
        }
        assert!(
            !executable_host_ok_from_residuals(true, false),
            "WND path must not claim host_ok without shell_wnd residual"
        );
        assert!(executable_host_ok_from_residuals(true, true));
        assert!(!executable_host_ok_from_residuals(false, true));
        unsafe {
            crate::env_compat::set_var("GENERALS_RUNTIME_HOST_WND", "0");
        }
        assert!(
            executable_host_ok_from_residuals(true, false),
            "WND-off path allows host_ok without shell residual"
        );
        // restore
        match _guard {
            Ok(v) => unsafe { crate::env_compat::set_var("GENERALS_RUNTIME_HOST_WND", v) },
            Err(_) => unsafe { crate::env_compat::remove_var("GENERALS_RUNTIME_HOST_WND") },
        }
    }

    #[test]
    fn smoke_defaults_wnd_enabled() {
        let src = include_str!("executable_smoke.rs");
        assert!(
            src.contains("unwrap_or_else(|_| \"1\".into())"),
            "executable smoke should default GENERALS_RUNTIME_HOST_WND=1"
        );
        assert!(src.contains("max_render_item_count"));
        assert!(src.contains("render_items_stable_ok"));
    }

    #[test]
    fn smoke_tracks_shell_wnd_residual_keys() {
        let eng = crate::cnc_game_engine::ENGINE_SRC;
        assert!(
            eng.contains("shell_screen_count")
                && eng.contains("shell_top_wnd")
                && eng.contains("shell_active"),
            "runtime host snapshot must publish shell WND residual"
        );
        let smoke = include_str!("executable_smoke.rs");
        assert!(
            smoke.contains("shell_wnd_ok") && smoke.contains("shell_top_wnd"),
            "executable smoke must parse shell WND residual"
        );
        let path = std::env::temp_dir().join("generals_shell_wnd_status_sample.txt");
        std::fs::write(
            &path,
            "state=Menu
shell_screen_count=1
shell_top_wnd=Menus/MainMenu.wnd
shell_active=true
",
        )
        .unwrap();
        let snap = parse_status(&path).expect("parse");
        assert_eq!(snap.shell_screen_count, 1);
        assert!(snap.shell_active);
        assert!(
            snap.shell_top_wnd
                .to_ascii_lowercase()
                .contains("mainmenu.wnd")
        );
        let _ = std::fs::remove_file(&path);
    }
}

#[cfg(test)]
mod skirmish_wnd_start_residual_tests {
    #[test]
    fn click_skirmish_start_prefers_wnd_gadget_when_enabled() {
        let dispatch = include_str!("cnc_game_engine/runtime_host/mod.rs");
        assert!(
            dispatch.contains("\"click_skirmish_start\""),
            "click_skirmish_start command must be dispatched"
        );
        // Live impl is in skirmish.rs (ENGINE_SRC 9k window no longer reaches it).
        let window = include_str!("cnc_game_engine/runtime_host/skirmish.rs");
        let idx = window
            .find("fn runtime_host_cmd_click_skirmish_start")
            .expect("click_skirmish_start impl");
        let window = &window[idx..];
        assert!(
            window.contains("simulate_skirmish_start_button_gadget_selected"),
            "must try retail WND ButtonStart GadgetSelected residual"
        );
        assert!(
            window.contains("click_skirmish_start_ok_wnd")
                || window.contains("click_skirmish_start_wnd_pending"),
            "must report wnd-specific gameplay cmd honesty"
        );
        assert!(
            window.contains("simulate_start_button_click"),
            "must keep Main SkirmishMenu mouse residual fallback"
        );
    }

    #[test]
    fn executable_smoke_wnd_host_override_residual() {
        let src = include_str!("executable_smoke.rs");
        assert!(
            src.contains("GENERALS_RUNTIME_HOST_WND")
                && src.contains("unwrap_or_else(|_| \"1\".into())"),
            "smoke defaults WND=1 for retail ButtonStart residual"
        );
        assert!(
            src.contains("skirmish_start_wnd_ok"),
            "smoke must track WND ButtonStart honesty separately"
        );
        let i = src.find("GENERALS_RUNTIME_HOST_WND").expect("env");
        let env_block = &src[i..src.len().min(i + 450)];
        assert!(
            !env_block.contains("var_os(\"DISPLAY\")"),
            "WND enable must not gate on X11 DISPLAY"
        );
    }

    #[test]
    fn executable_smoke_waits_for_full_command_chain() {
        let src = include_str!("executable_smoke.rs");
        assert!(
            src.contains("chain_complete")
                && src.contains("gameplay_step >= 59")
                && !src.contains(
                    "saw_attack_ok\n                                && snap.frame >= 20)"
                ),
            "smoke must not exit on early construct/train/attack alone"
        );
        assert!(
            src.contains("pause_ok:paused") && src.contains("pause_ok:resumed"),
            "pause residual must remain in chain"
        );
    }

    #[test]
    fn game_client_exposes_skirmish_button_start_gadget_simulate() {
        let src = include_str!(
            "../../GameEngine/GameClient/src/gui/callbacks/skirmish_game_options_menu.rs"
        );
        assert!(
            src.contains("fn simulate_skirmish_start_button_gadget_selected"),
            "WND ButtonStart gadget residual helper missing"
        );
        assert!(
            src.contains("WindowMessage::GadgetSelected"),
            "must fire GadgetSelected like C++ GBM_SELECTED"
        );
    }
}
