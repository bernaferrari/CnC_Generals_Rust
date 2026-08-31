// Lifecycle: tests for the executable smoke harness (source honesty,
// status parsing, claim gates). Both pre-split test mods, unchanged.
//
// The harness source is split across executable_smoke.rs (hub) and the
// executable_smoke/ lifecycle fragments concatenated in `include!` order.
// Source-text honesty assertions that previously read the single-file
// monolith now read EXECUTABLE_SMOKE_SRC — the identical text stream
// (hub first, then fragments, then these tests, matching the pre-split
// file order that satisfied self-referencing assertions such as
// `pause_ok:paused` / `pause_ok:resumed`).

/// Concatenated executable_smoke sources in pre-split monolith order — shared
/// with the game_logic residual packs via the private `executable_smoke_source`
/// module (single source of truth; do not fork the fragment list).
#[cfg(test)]
use crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC;

#[cfg(test)]
mod tests {
    use super::*;
    use super::EXECUTABLE_SMOKE_SRC;
    #[test]
    fn kill_stale_matches_runtime_host_underscore() {
        let src = EXECUTABLE_SMOKE_SRC;
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
        let src = EXECUTABLE_SMOKE_SRC;
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
        let src = EXECUTABLE_SMOKE_SRC;
        assert!(src.contains("ExecutableSmokeLaunch::Windowed => \"-runtime_host=windowed\""));
        assert!(src.contains("run_windowed_acceptance_smoke"));
        let launch = src
            .split("let runtime_host_arg = match launch")
            .nth(1)
            .expect("launch match");
        assert!(launch.contains("Windowed => \"-runtime_host=windowed\""));
        assert!(
            !include_str!("../bin/windowed_acceptance_gate.rs").contains("-runtime_host=headless")
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

        let src = include_str!("../bin/executable_smoke_gate.rs");
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
        let src = include_str!("../cnc_game_engine/dispatch.rs");
        assert!(
            src.contains("window_visible_from_winit_query")
                && src.contains("self.window.is_visible()"),
            "engine must call the shipped winit visibility helper"
        );
        let boot = include_str!("../cnc_game_engine/runtime.rs");
        assert!(
            boot.contains("fn publish_booting_from_winit_query")
                && boot.contains("window_visible_from_winit_query"),
            "boot status must use the shipped winit query, never a hardcoded true"
        );
        assert!(
            !boot.contains("window_visible: true"),
            "must not forge window_visible=true in boot residual"
        );
        let helper = include_str!("../cnc_game_engine/types.rs");
        assert!(
            helper.contains("fn apply_runtime_host_window_visibility")
                && helper.contains("set_visible(false)")
                && helper.contains("set_visible(true)")
                && helper.contains("window_visible_from_winit_query"),
            "windowed show / headless hide must go through the honest helper"
        );
        let win_main = include_str!("../win_main.rs");
        assert!(
            win_main.contains("ActivationPolicy::Regular")
                && win_main.contains("create_host_event_loop"),
            "windowed macOS EventLoop must request Regular activation so the OS window appears"
        );
        assert!(
            include_str!("../main.rs").contains("create_host_event_loop"),
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
        let runtime = include_str!("../cnc_game_engine/runtime.rs");
        assert!(
            runtime.contains("note_windowed_surface_presented")
                && runtime.contains("live_frame_ok_from_windowed_present"),
            "publish must use the shipped present/capture helper"
        );
        let loop_src = include_str!("../cnc_game_engine/run_loop.rs");
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
        let input = include_str!("../cnc_game_engine/input.rs");
        assert!(
            input.contains("note_skirmish_path_gadget")
                && input.contains("handle_mouse_button_input"),
            "skirmish path must latch inside handle_mouse_button_input"
        );
        assert!(
            !input.contains("click_skirmish_start") && !input.contains("main_menu_skirmish_wnd_ok"),
            "host click_skirmish_start residual is not the wnd_widget_tree_nav latch"
        );
        let snap = include_str!("../cnc_game_engine/dispatch.rs");
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
        let smoke = EXECUTABLE_SMOKE_SRC;
        let prod = smoke.split("#[cfg(test)]").next().expect("prod");
        assert!(
            prod.contains("reached_ingame_from_live_map"),
            "smoke must set reached_ingame from the shipped map latch"
        );
    }

    #[test]
    fn interactive_gameplay_latches_only_from_handle_mouse_button_input_rmb() {
        let input = include_str!("../cnc_game_engine/input.rs");
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
        let snap = include_str!("../cnc_game_engine/dispatch.rs");
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
        // SAFETY: serialized by the repo --test-threads=1 convention plus
        // env_compat module contract (GENERALS_* toggle read at defined
        // boundaries); no other thread reads env mid-test.
        unsafe {
            crate::env_compat::set_var("GENERALS_RUNTIME_HOST_WND", "1");
        }
        assert!(
            !executable_host_ok_from_residuals(true, false),
            "WND path must not claim host_ok without shell_wnd residual"
        );
        assert!(executable_host_ok_from_residuals(true, true));
        assert!(!executable_host_ok_from_residuals(false, true));
        // SAFETY: same serialized-env contract as above.
        unsafe {
            crate::env_compat::set_var("GENERALS_RUNTIME_HOST_WND", "0");
        }
        assert!(
            executable_host_ok_from_residuals(true, false),
            "WND-off path allows host_ok without shell residual"
        );
        // restore
        match _guard {
            // SAFETY: restore path under the same serialization contract.
            Ok(v) => unsafe { crate::env_compat::set_var("GENERALS_RUNTIME_HOST_WND", v) },
            // SAFETY: removal path under the same serialization contract.
            Err(_) => unsafe { crate::env_compat::remove_var("GENERALS_RUNTIME_HOST_WND") },
        }
    }

    #[test]
    fn smoke_defaults_wnd_enabled() {
        let src = EXECUTABLE_SMOKE_SRC;
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
        let smoke = EXECUTABLE_SMOKE_SRC;
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
    use super::EXECUTABLE_SMOKE_SRC;
    #[test]
    fn click_skirmish_start_prefers_wnd_gadget_when_enabled() {
        let dispatch = include_str!("../cnc_game_engine/runtime_host/mod.rs");
        assert!(
            dispatch.contains("\"click_skirmish_start\""),
            "click_skirmish_start command must be dispatched"
        );
        // Live impl is in skirmish.rs (ENGINE_SRC 9k window no longer reaches it).
        let window = include_str!("../cnc_game_engine/runtime_host/skirmish.rs");
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
        let src = EXECUTABLE_SMOKE_SRC;
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
        let src = EXECUTABLE_SMOKE_SRC;
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
            "../../../GameEngine/GameClient/src/gui/callbacks/skirmish_game_options_menu.rs"
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
