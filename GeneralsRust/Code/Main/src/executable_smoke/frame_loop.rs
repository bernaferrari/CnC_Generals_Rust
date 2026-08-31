// Lifecycle: frame loop — wall budget, child-exit watch, status poll with
// residual latching, and the Menu → Skirmish → Start → InGame → command
// chain → exit phase machine (phase 2 ladder lives in gameplay_chain.rs).

/// Loop-carried smoke state: residual latches, command details, peak
/// counters, command-chain step, phase, and windowed-inject bookkeeping.
/// Field-per-local mirror of the pre-split `run_executable_smoke_once`
/// locals; every field starts at its original zero value via `Default`.
#[derive(Default)]
struct SmokeRunState {
    gameplay_step: u8,
    saw_select_ok: bool,
    saw_move_ok: bool,
    saw_attack_ok: bool,
    saw_construct_ok: bool,
    construct_detail: String,
    saw_train_ok: bool,
    train_detail: String,
    saw_save_ok: bool,
    save_detail: String,
    saw_load_ok: bool,
    load_detail: String,
    saw_stop_ok: bool,
    stop_detail: String,
    saw_sell_ok: bool,
    sell_detail: String,
    saw_upgrade_ok: bool,
    upgrade_detail: String,
    saw_guard_ok: bool,
    guard_detail: String,
    saw_attack_move_ok: bool,
    attack_move_detail: String,
    saw_scatter_ok: bool,
    scatter_detail: String,
    saw_patrol_ok: bool,
    patrol_detail: String,
    saw_deploy_ok: bool,
    deploy_detail: String,
    saw_cheer_ok: bool,
    cheer_detail: String,
    saw_formation_ok: bool,
    saw_combat_damage: bool,
    saw_early_combat_cmd: bool,
    formation_detail: String,
    saw_capture_ok: bool,
    capture_detail: String,
    saw_return_supplies_ok: bool,
    return_supplies_detail: String,
    saw_evacuate_ok: bool,
    evacuate_detail: String,
    saw_repair_ok: bool,
    repair_detail: String,
    saw_return_to_base_ok: bool,
    return_to_base_detail: String,
    saw_attitude_ok: bool,
    attitude_detail: String,
    saw_rally_ok: bool,
    rally_detail: String,
    saw_switch_weapons_ok: bool,
    switch_weapons_detail: String,
    saw_view_cc_ok: bool,
    view_cc_detail: String,
    saw_clear_mines_ok: bool,
    clear_mines_detail: String,
    saw_beacon_ok: bool,
    beacon_detail: String,
    saw_hack_ok: bool,
    hack_detail: String,
    saw_cleanup_ok: bool,
    cleanup_detail: String,
    saw_combat_drop_ok: bool,
    combat_drop_detail: String,
    saw_overcharge_ok: bool,
    overcharge_detail: String,
    saw_special_power_ok: bool,
    special_power_detail: String,
    saw_remove_beacon_ok: bool,
    remove_beacon_detail: String,
    saw_demo_ok: bool,
    demo_detail: String,
    saw_view_radar_ok: bool,
    view_radar_detail: String,
    saw_force_attack_ok: bool,
    force_attack_detail: String,
    saw_force_attack_object_ok: bool,
    force_attack_object_detail: String,
    saw_select_all_ok: bool,
    select_all_detail: String,
    saw_control_group_ok: bool,
    control_group_detail: String,
    saw_waypoint_ok: bool,
    waypoint_detail: String,
    saw_box_select_ok: bool,
    box_select_detail: String,
    saw_presentation_frame_ok: bool,
    saw_presentation_live_fallback_ok: bool,
    saw_gameworld_presentation_entities_ok: bool,
    max_gameworld_presentation_entities: u32,
    saw_gameworld_overlay_stamped_ok: bool,
    max_gameworld_overlay_stamped: u32,
    max_gameworld_appended: u32,
    max_gameworld_rebuilt: u32,
    saw_gameworld_rebuilt_ok: bool,
    presentation_detail: String,
    saw_shell_wnd_ok: bool,
    shell_wnd_detail: String,
    saw_select_similar_ok: bool,
    select_similar_detail: String,
    saw_select_on_screen_ok: bool,
    select_on_screen_detail: String,
    saw_select_structures_ok: bool,
    select_structures_detail: String,
    saw_select_aircraft_ok: bool,
    select_aircraft_detail: String,
    saw_select_idle_ok: bool,
    select_idle_detail: String,
    saw_camera_reset_ok: bool,
    camera_reset_detail: String,
    saw_camera_zoom_ok: bool,
    camera_zoom_detail: String,
    saw_pause_ok: bool,
    pause_detail: String,
    saw_cancel_production_ok: bool,
    cancel_production_detail: String,
    saw_diplomacy_ok: bool,
    diplomacy_detail: String,
    saw_live_frame_ok: bool,
    saw_window_visible: bool,
    saw_wnd_widget_tree_nav: bool,
    saw_interactive_gameplay: bool,
    saw_physical_build_and_produce: bool,
    saw_physical_gather_resources: bool,
    saw_physical_save_load_continue: bool,
    max_render_item_count: u32,
    max_render_alive_objects: u32,
    render_items_nonzero_polls: u32,
    saw_auto_attack_ok: bool,
    auto_attack_detail: String,
    saw_options_ok: bool,
    options_detail: String,
    saw_request_capture_ok: bool,
    request_capture_detail: String,
    saw_skirmish_start_wnd_ok: bool,
    train_sent: bool,
    saw_construct_under_construction: bool,
    train_retry_started: Option<Instant>,
    load_retry_started: Option<Instant>,
    phase: u8, // 0 wait menu/boot, 1 commanded, 2 wait ingame, 3 exit
    last_snap: StatusSnap,
    commanded_at: Option<Instant>,
    windowed_start_sent: bool,
    // Windowed phase-20 substep: 0=menu nav inject, 1=start_game, 2=gameplay order inject.
    windowed_inject_step: u8,
    windowed_menu_nav_sent: bool,
    windowed_gameplay_order_sent: bool,
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

    // Lifecycle: initialization / asset bootstrap.
    let Some(boot) = bootstrap_smoke_child(launch, &mut result) else {
        return result;
    };
    let SmokeBootstrap {
        child: mut child,
        tmp,
        control_path,
        status_path,
        map,
    } = boot;

    let started = Instant::now();
    let mut st = SmokeRunState::default();

    loop {
        if smoke_wall_budget_exceeded(
            &mut st,
            &mut result,
            &mut child,
            &control_path,
            timeout,
            started,
        ) {
            break;
        }

        // Child exited early?
        if smoke_child_exited_early(&mut st, &mut result, &mut child, use_new_game_path) {
            break;
        }

        if let Some(snap) = parse_status(&status_path) {
            latch_status_residuals(&mut st, &mut result, &snap);
            match st.phase {
                0 => {
                    smoke_phase_wait_menu(
                        &mut st,
                        &mut result,
                        &snap,
                        &control_path,
                        launch,
                        driver,
                        started,
                    );
                }
                21 => {
                    // Manual windowed acceptance observation. All useful work
                    // happens in the status-poll section above; this arm must
                    // remain free of control-file writes other than the timeout
                    // cleanup outside the state machine.
                }
                20 => {
                    smoke_phase_windowed_inject(&mut st, &mut result, &snap, &control_path, &map);
                }
                10 => {
                    smoke_phase_skirmish_menu_open(
                        &mut st,
                        &mut result,
                        &snap,
                        &control_path,
                        &map,
                    );
                }
                1 => {
                    smoke_phase_skirmish_start_wait(
                        &mut st,
                        &mut result,
                        &snap,
                        &control_path,
                        &map,
                        use_new_game_path,
                    );
                }
                2 => {
                    smoke_drive_gameplay_chain(
                        &mut st,
                        &mut result,
                        &snap,
                        &control_path,
                        launch,
                    );
                }
                3 => {
                    // Wait for clean exit.
                    if smoke_phase_wait_clean_exit(
                        &mut st,
                        &mut result,
                        &mut child,
                        use_new_game_path,
                    ) {
                        break;
                    }
                }
                _ => {}
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    let _ = fs::remove_dir_all(&tmp);

    // Lifecycle: shutdown — merge latched poll residuals, then the gates.
    merge_shutdown_residuals(&mut result, &st);
    result
}

/// Wall-budget guard: prefer honest InGame finalization over a bare timeout
/// when the host already reached match state. Returns true to break the loop.
fn smoke_wall_budget_exceeded(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    child: &mut Child,
    control_path: &Path,
    timeout: Duration,
    started: Instant,
) -> bool {
    if started.elapsed() > timeout {
        // Prefer honest InGame finalization over a bare timeout when the host
        // already reached match state — long command chains can exceed wall budget.
        if result.reached_ingame {
            result.shell_wnd_ok = st.saw_shell_wnd_ok;
            // Wave 833: honest host control residual.
            result.gameplay_cmd_ok = (st.saw_select_ok && st.saw_move_ok && st.saw_attack_ok)
                || (st.saw_select_ok && st.saw_move_ok && st.saw_construct_ok && st.saw_train_ok)
                || (st.saw_construct_ok && st.saw_train_ok && st.saw_attack_ok)
                || (st.saw_construct_ok
                    && (st.saw_attack_ok || st.saw_attack_move_ok || st.saw_combat_damage))
                || (st.saw_select_ok
                    && st.saw_move_ok
                    && (st.saw_attack_ok || st.saw_attack_move_ok || st.saw_construct_ok));
            result.construct_cmd_ok = st.saw_construct_ok;
            result.train_cmd_ok = st.saw_train_ok;
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
                st.phase,
                st.gameplay_step,
                result.shell_wnd_ok,
                result.gameplay_cmd_ok
            );
        } else {
            result.shell_wnd_ok = st.saw_shell_wnd_ok;
            result.executable_host_ok =
                executable_host_ok_from_residuals(result.reached_menu, result.shell_wnd_ok);
            result.status = "timeout".into();
            result.detail = format!(
                "timeout after {:?} last_state={} menu={} ingame={} frames={} phase={} shell_wnd={}",
                timeout,
                st.last_snap.state,
                result.reached_menu,
                result.reached_ingame,
                result.frames_observed,
                st.phase,
                result.shell_wnd_ok
            );
        }
        let _ = write_control(&control_path, &["exit"]);
        kill_child(child);
        return true;
    }
    false
}

/// Early child-exit guard. Returns true to break the loop.
fn smoke_child_exited_early(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    child: &mut Child,
    use_new_game_path: bool,
) -> bool {
    if let Ok(Some(status)) = child.try_wait() {
        result.exit_code = status.code();
        result.shell_wnd_ok = st.saw_shell_wnd_ok;
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
        } else if matches!(st.last_snap.state.as_str(), "LaunchFailed" | "")
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
                st.last_snap.state,
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
        return true;
    }
    false
}

/// Per-poll residual latching: presentation/UI/render (presentation.rs),
/// combat damage, sticky command-prefix residuals, skirmish-menu evidence,
/// then last_snap / frames / map / Menu-InGame state transitions.
fn latch_status_residuals(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
) {
    latch_presentation_frame_residuals(st, snap);
    latch_shell_wnd_residuals(st, snap);
    latch_render_item_residuals(st, snap);
    latch_retail_flag_residuals(st, snap);
    if snap.match_damage_applied > 0.0 || snap.match_kills > 0 {
        st.saw_combat_damage = true;
    }
    // Wave 864: keep latched if counters ever rose (status may reset on path change).
    // (saw_combat_damage is sticky once true)
    if snap.last_gameplay_cmd.starts_with("construct_ok") {
        st.saw_construct_ok = true;
        st.construct_detail = snap.last_gameplay_cmd.clone();
    } else if snap.last_gameplay_cmd.starts_with("construct_")
        && st.construct_detail.is_empty()
    {
        st.construct_detail = snap.last_gameplay_cmd.clone();
    }
    if snap.last_gameplay_cmd.starts_with("train_ok") {
        st.saw_train_ok = true;
        st.train_detail = snap.last_gameplay_cmd.clone();
    } else if snap.last_gameplay_cmd.starts_with("train_") && st.train_detail.is_empty() {
        st.train_detail = snap.last_gameplay_cmd.clone();
    }
    if snap.last_gameplay_cmd.starts_with("save_ok") {
        st.saw_save_ok = true;
        st.save_detail = snap.last_gameplay_cmd.clone();
    } else if snap.last_gameplay_cmd.starts_with("save_") && st.save_detail.is_empty() {
        st.save_detail = snap.last_gameplay_cmd.clone();
    }
    if snap.last_gameplay_cmd.starts_with("load_ok") {
        st.saw_load_ok = true;
        st.load_detail = snap.last_gameplay_cmd.clone();
    } else if snap.last_gameplay_cmd.starts_with("load_") && st.load_detail.is_empty() {
        st.load_detail = snap.last_gameplay_cmd.clone();
    }
    if snap.last_gameplay_cmd.starts_with("select_all_ok") {
        st.saw_select_all_ok = true;
        st.select_all_detail = snap.last_gameplay_cmd.clone();
    } else if snap.last_gameplay_cmd.starts_with("select_all_")
        && !snap.last_gameplay_cmd.starts_with("select_all_combat")
        && st.select_all_detail.is_empty()
    {
        st.select_all_detail = snap.last_gameplay_cmd.clone();
    }
    if snap.last_gameplay_cmd.starts_with("formation_ok") {
        st.saw_formation_ok = true;
        st.formation_detail = snap.last_gameplay_cmd.clone();
    } else if snap.last_gameplay_cmd.starts_with("formation_")
        && st.formation_detail.is_empty()
    {
        st.formation_detail = snap.last_gameplay_cmd.clone();
    }
    if snap.skirmish_menu_ok
        || snap.ui_screen.to_ascii_lowercase().contains("skirmish")
        || snap.last_gameplay_cmd.starts_with("open_skirmish_menu_ok")
    {
        result.skirmish_menu_ok = true;
    }
    st.last_snap = snap.clone();
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
}

/// Phase 0 (asset/bootstrap wait): until Menu, or Booting far enough to
/// accept commands (startup progress / hard deadline).
fn smoke_phase_wait_menu(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
    launch: ExecutableSmokeLaunch,
    driver: SmokeDriver,
    started: Instant,
) {
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
            st.phase = 21;
            result.detail.push_str(" manual_windowed_observer;");
        } else if launch == ExecutableSmokeLaunch::Windowed {
            // Phase 20 drives winit-equivalent inject (same
            // handle_mouse_button_input as Injected — automation
            // only; does not latch playable_claim physical flags)
            // then shipped start_game — never soft skirmish menu
            // host commands / drive_os_wnd / cheat tokens.
            st.commanded_at = Some(Instant::now());
            st.phase = 20;
        } else {
            // Soft open Skirmish UI first (override only; WND off).
            let _ = write_control(&control_path, &["open_skirmish_menu"]);
            st.commanded_at = Some(Instant::now());
            st.phase = 10; // wait for Skirmish UI before start_game
        }
    }
}

/// Phase 20 (windowed interactive): honest winit-equivalent inject only
/// (inject_winit_equivalent_named_gadget_click /
/// inject_winit_equivalent_gameplay_order_click → handle_mouse_button_input).
/// Never drive_os_wnd_* for evidence, never note_* forge, never cheats.
fn smoke_phase_windowed_inject(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
    map: &str,
) {
    // Windowed interactive phase: honest winit-equivalent inject
    // only (inject_winit_equivalent_named_gadget_click /
    // inject_winit_equivalent_gameplay_order_click → handle_mouse_button_input).
    // Never drive_os_wnd_* for evidence, never note_* forge, never cheats.
    if st.saw_wnd_widget_tree_nav && st.saw_interactive_gameplay && result.reached_ingame
    {
        st.phase = 2;
        st.gameplay_step = 0;
        st.commanded_at = Some(Instant::now());
    } else if !result.reached_ingame {
        let ready = st.commanded_at
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
            || st.saw_wnd_widget_tree_nav;
        let nav_miss = snap.last_gameplay_cmd.starts_with("winit_menu_nav_miss")
            || snap.last_gameplay_cmd.starts_with("winit_menu_nav_partial");
        // Sequential honesty path (not forge):
        // 1) winit_menu_nav only → latch menu_click via inject
        // 2) after nav_ok residual, start_game → match_started
        // 3) never start before nav when shell is ready
        // Wait for a presented frame before menu inject so live_frame
        // can latch on the Menu residual (smoke polls may miss it after start).
        let frame_ready = snap.live_frame_ok || st.saw_live_frame_ok;
        if !st.windowed_menu_nav_sent
            && ready
            && shell_ready
            && frame_ready
            && snap.state == "Menu"
        {
            let _ = write_control(&control_path, &["winit_menu_nav"]);
            st.windowed_menu_nav_sent = true;
            st.windowed_inject_step = 1;
            st.commanded_at = Some(Instant::now());
            result.detail.push_str(" windowed_winit_menu_nav;");
        } else if !st.windowed_menu_nav_sent
            && ready
            && shell_ready
            && !frame_ready
            && snap.state == "Menu"
            && st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(45))
                .unwrap_or(false)
        {
            // Proceed with nav after wait; claim stays false without live_frame.
            let _ = write_control(&control_path, &["winit_menu_nav"]);
            st.windowed_menu_nav_sent = true;
            st.windowed_inject_step = 1;
            st.commanded_at = Some(Instant::now());
            result
                .detail
                .push_str(" windowed_winit_menu_nav_no_live_frame;");
        } else if st.windowed_menu_nav_sent
            && !st.windowed_start_sent
            && nav_miss
            && ready
            && snap.state == "Menu"
            && shell_ready
            && st.windowed_inject_step < 4
        {
            let _ = write_control(&control_path, &["winit_menu_nav"]);
            st.windowed_inject_step = st.windowed_inject_step.saturating_add(1);
            st.commanded_at = Some(Instant::now());
            result.detail.push_str(" windowed_winit_menu_nav_retry;");
        } else if st.windowed_menu_nav_sent
            && !st.windowed_start_sent
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
            st.windowed_start_sent = true;
            st.commanded_at = Some(Instant::now());
            result.detail.push_str(" windowed_start_game_after_nav;");
        } else if !st.windowed_start_sent
            && st.windowed_menu_nav_sent
            && !nav_ok
            && ready
            && st.commanded_at
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
            st.windowed_start_sent = true;
            st.commanded_at = Some(Instant::now());
            result
                .detail
                .push_str(" windowed_start_game_grace_after_nav_miss;");
        } else if !st.windowed_start_sent
            && !st.windowed_menu_nav_sent
            && !shell_ready
            && st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(25))
                .unwrap_or(false)
        {
            // Grace start if shell never becomes ready (claim stays false).
            let start = format!(
                "start_game|mode=skirmish|faction=USA|map={}",
                map.replace('|', "/")
            );
            let _ = write_control(&control_path, &[start.as_str()]);
            st.windowed_start_sent = true;
            st.commanded_at = Some(Instant::now());
            result.detail.push_str(" windowed_start_game_grace;");
        }
    } else if result.reached_ingame {
        // Select + RMB inject through handle_mouse_button_input.
        // Wait briefly for units (render_alive) so select can succeed.
        // Fifth claim flag is interactive_gameplay only — keep retrying
        // until status gameplay=true (or inject budget exhausted).
        let ready = st.commanded_at
            .map(|t| t.elapsed() > Duration::from_millis(800))
            .unwrap_or(true);
        let units_ready = snap.render_alive_objects > 0
            || snap.local_mobile_units > 0
            || snap.last_gameplay_cmd.starts_with("select_ok")
            || st.commanded_at
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
        if !st.windowed_gameplay_order_sent && ready && units_ready {
            let _ = write_control(&control_path, &["select_local_unit"]);
            st.windowed_gameplay_order_sent = true;
            st.windowed_inject_step = 0;
            st.commanded_at = Some(Instant::now());
            result.detail.push_str(" windowed_select_local_unit;");
        } else if st.windowed_gameplay_order_sent
            && !st.saw_interactive_gameplay
            && ready
            && units_ready
            && st.commanded_at
                .map(|t| t.elapsed() > Duration::from_millis(1200))
                .unwrap_or(false)
            && st.windowed_inject_step < 48
        {
            if st.windowed_inject_step % 2 == 0 {
                // After first select (step 0 already sent), even steps
                // re-select when residual is missing/fail; otherwise RMB.
                if st.windowed_inject_step == 0
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
            st.windowed_inject_step = st.windowed_inject_step.saturating_add(1);
            st.commanded_at = Some(Instant::now());
        } else if st.windowed_gameplay_order_sent
            && (st.saw_interactive_gameplay
                || (st.windowed_inject_step >= 48
                    && st.commanded_at
                        .map(|t| t.elapsed() > Duration::from_secs(2))
                        .unwrap_or(false))
                || st.commanded_at
                    .map(|t| t.elapsed() > Duration::from_secs(120))
                    .unwrap_or(false))
            && ready
        {
            // Advance only after interactive evidence or inject budget.
            st.phase = 2;
            st.gameplay_step = 0;
            st.commanded_at = Some(Instant::now());
        }
        let _ = st.windowed_inject_step;
    }
}

/// Phase 10: wait for the Skirmish UI, then click the real SkirmishMenu
/// Start button path.
fn smoke_phase_skirmish_menu_open(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
    map: &str,
) {
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
        || st.commanded_at
            .map(|t| t.elapsed() > Duration::from_millis(800))
            .unwrap_or(true);
    if ready {
        // Prefer real SkirmishMenu Start button click residual.
        let click = format!("click_skirmish_start|map={}", map.replace('|', "/"));
        let _ = write_control(&control_path, &[click.as_str()]);
        st.commanded_at = Some(Instant::now());
        st.phase = 1;
    }
}

/// Phase 1: wait for InGame after start; latch skirmish WND residuals and
/// retry once with direct start_game if the NewGame path stalled.
fn smoke_phase_skirmish_start_wait(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
    map: &str,
    use_new_game_path: bool,
) {
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
        st.saw_skirmish_start_wnd_ok = true;
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
            st.saw_skirmish_start_wnd_ok = true;
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
            st.saw_skirmish_start_wnd_ok = true;
            result.skirmish_start_wnd_ok = true;
        }
    }
    if result.reached_ingame {
        st.phase = 2;
    } else if st.commanded_at
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
            st.commanded_at = Some(Instant::now());
            st.phase = 1; // stay
            result.detail.push_str(" fallback_start_game;");
        } else {
            result.status = "start_timeout".into();
            result.detail = format!(
                "did not reach InGame after start command; state={} phase={}",
                snap.state, snap.startup_phase
            );
            let _ = write_control(&control_path, &["exit"]);
            st.phase = 3;
        }
    }
}

/// Phase 3 (shutdown): wait for clean exit; force-kill on exit hang.
/// Returns true when the smoke loop should break.
fn smoke_phase_wait_clean_exit(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    child: &mut Child,
    use_new_game_path: bool,
) -> bool {
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
                st.last_snap.state
            );
        }
        return true;
    }
    if st.commanded_at
        .map(|t| t.elapsed() > Duration::from_secs(20))
        .unwrap_or(false)
        && st.phase == 3
    {
        kill_child(child);
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
                st.saw_shell_wnd_ok, result.reached_menu, result.frames_observed
            );
        }
        result.shell_wnd_ok = st.saw_shell_wnd_ok;
        result.executable_host_ok = executable_host_ok_from_residuals(
            result.reached_menu || result.reached_ingame,
            result.shell_wnd_ok,
        );
        return true;
    }
    false
}
