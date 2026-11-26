// Tests for the executable smoke harness, including status parsing and claim gates.

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn save_load_failure_stops_without_retrying_dialog_producing_commands() {
        let dir = tempfile::tempdir().unwrap();
        let control = dir.path().join("control");
        for (step, failure, saved) in [
            (40, "save_fail:quicksave:serialization rejected", false),
            (50, "load_fail:quicksave:bad snapshot", true),
        ] {
            let mut st = SmokeRunState {
                gameplay_step: step,
                saw_select_ok: true,
                saw_move_ok: true,
                saw_save_ok: saved,
                pre_world_snap: "world_snap_ok:fixture".into(),
                commanded_at: Some(Instant::now() - Duration::from_secs(9)),
                ..Default::default()
            };
            let mut result = ExecutableSmokeResult::default();
            let snap = StatusSnap {
                last_gameplay_cmd: failure.into(),
                ..Default::default()
            };
            smoke_drive_save_load_chain(&mut st, &mut result, &snap, &control);
            assert_eq!(result.status, "save_load_command_failed");
            assert!(result.detail.contains(failure));
            assert!(!control.exists(), "failed command must not be retried");
        }
    }

    #[test]
    fn save_load_waits_for_unacknowledged_command_without_reissuing_it() {
        let dir = tempfile::tempdir().unwrap();
        let control = dir.path().join("control");
        for (step, saved) in [(40, false), (50, true)] {
            let mut st = SmokeRunState {
                gameplay_step: step,
                saw_select_ok: true,
                saw_move_ok: true,
                saw_save_ok: saved,
                pre_world_snap: "world_snap_ok:fixture".into(),
                commanded_at: Some(Instant::now() - Duration::from_secs(9)),
                ..Default::default()
            };
            let mut result = ExecutableSmokeResult::default();
            let snap = StatusSnap::default();
            smoke_drive_save_load_chain(&mut st, &mut result, &snap, &control);
            assert!(!control.exists(), "unacknowledged command must not be reissued");
        }
    }

    // Overrides are owned by Command and installed only in the fresh child.
    // libtest and native libraries may have threads even with --test-threads=1;
    // never mutate the parent process environment to configure these cases.
    #[cfg(not(target_arch = "wasm32"))]
    fn isolated_environment_case(
        test: &str,
        scenario: &str,
        key: &str,
        value: &std::ffi::OsStr,
        run: impl FnOnce(),
    ) {
        const CHILD_TEST: &str = "GENERALS_SMOKE_ENV_CHILD_TEST";
        const CHILD_CASE: &str = "GENERALS_SMOKE_ENV_CHILD_CASE";
        let module = module_path!()
            .strip_prefix("generals_main::")
            .unwrap_or(module_path!());
        let exact = format!("{module}::{test}");
        if std::env::var(CHILD_TEST).ok().as_deref() == Some(exact.as_str()) {
            if std::env::var(CHILD_CASE).ok().as_deref() == Some(scenario) {
                run();
            }
            return;
        }

        use std::io::Read;
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &exact, "--nocapture", "--test-threads=1"])
            .env(CHILD_TEST, &exact)
            .env(CHILD_CASE, scenario)
            .env(key, value)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let pipes: [Box<dyn Read + Send>; 2] = [
            Box::new(child.stdout.take().unwrap()),
            Box::new(child.stderr.take().unwrap()),
        ];
        let readers = pipes.map(|mut pipe| {
            std::thread::spawn(move || {
                let mut bytes = Vec::new();
                pipe.read_to_end(&mut bytes).unwrap();
                bytes
            })
        });
        let deadline = Instant::now() + Duration::from_secs(30);
        let (status, timed_out) = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break (status, false);
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                break (child.wait().unwrap(), true);
            }
            std::thread::sleep(Duration::from_millis(20));
        };
        let output =
            readers.map(|reader| String::from_utf8_lossy(&reader.join().unwrap()).into_owned());
        assert!(!timed_out, "environment child timed out: {output:?}");
        assert!(
            status.success(),
            "environment child failed: {status}: {output:?}"
        );
        assert!(
            output[0].contains("running 1 test") && output[0].contains("1 passed; 0 failed"),
            "exact environment child must run one passing test: {output:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn smoke_timeout_does_not_restart_or_leave_a_stuck_child() {
        let dir = tempfile::tempdir().unwrap();
        let timeout = Duration::ZERO;
        let launch = ExecutableSmokeLaunch::HeadlessHost;
        let mut spawned_pid = None;
        let result = run_smoke_with_boot_retry(timeout, |started| {
            assert!(spawned_pid.is_none(), "an expired run must not retry");
            let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
            // The OS reports the pid at spawn, whether or not the child has
            // received CPU time by the time the harness reaches its deadline.
            spawned_pid = Some(child.id());
            let boot = SmokeBootstrap {
                child: SmokeHost::Spawned(SmokeChild(child)),
                tmp: dir.path().to_owned(),
                owns_tmp: false,
                control_path: dir.path().join("control"),
                status_path: dir.path().join("status"),
                map: "Lone Eagle".into(),
                screenshot_path: None,
            };
            run_smoke_session(
                boot,
                ExecutableSmokeResult::default(),
                timeout,
                started,
                false,
                launch,
                SmokeDriver::MapLoadOnly,
            )
        });
        assert_eq!(result.status, "timeout", "{}", result.detail);
        let pid = spawned_pid.expect("the initial attempt spawned a child").to_string();
        let alive = Command::new("/bin/kill")
            .args(["-0", &pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success();
        assert!(!alive, "timed out child {pid} remains alive");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn parallel_smoke_children_resolve_their_own_retail_map() {
        const CHILD: &str = "GENERALS_SMOKE_ASSET_ISOLATION_CASE";
        if let Ok(case) = std::env::var(CHILD) {
            let expected: &[u8] = match case.as_str() {
                "first" => b"first map",
                "second" => b"second map",
                other => panic!("unexpected smoke case {other}"),
            };
            let options = SmokeOptions::from_environment();
            let map = resolve_lone_eagle_map(options.assets_dir.as_deref());
            assert_eq!(fs::read(map).unwrap(), expected);
            return;
        }

        let tmp = tempfile::tempdir().unwrap();
        let module = module_path!().strip_prefix("generals_main::").unwrap_or(module_path!());
        let exact = format!("{module}::parallel_smoke_children_resolve_their_own_retail_map");
        let mut children = Vec::new();
        for (case, contents) in [("first", &b"first map"[..]), ("second", &b"second map"[..])] {
            let assets = tmp.path().join(case);
            let map = assets.join("Maps/Lone Eagle/Lone Eagle.map");
            fs::create_dir_all(map.parent().unwrap()).unwrap();
            fs::write(map, contents).unwrap();
            let options = SmokeOptions { assets_dir: Some(assets), screenshot_path: None };
            let mut command = Command::new(std::env::current_exe().unwrap());
            configure_smoke_child_assets(&mut command, &options);
            children.push(command.args(["--exact", &exact, "--nocapture"])
                .env(CHILD, case)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn().unwrap());
        }
        for child in children {
            let output = child.wait_with_output().unwrap();
            assert!(
                output.status.success(),
                "smoke child failed: stdout={}\nstderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                String::from_utf8_lossy(&output.stdout).contains("1 passed; 0 failed"),
                "smoke child did not run the requested test"
            );
        }
    }

    #[test]
    fn failed_smoke_keeps_bounded_child_error_and_shell_state() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join("child_stderr.txt"),
            format!(
                "{}\nthread 'main' panicked at game.rs:42\n",
                "x".repeat(8192)
            ),
        )
        .unwrap();
        let mut result = ExecutableSmokeResult::default();
        result.status = "process_exited".into();
        let st = SmokeRunState {
            shell_wnd_detail: "active=true count=0 top=".into(),
            ..Default::default()
        };

        append_smoke_failure_diagnostics(&mut result, &st, dir.path());

        assert!(
            result
                .detail
                .contains("shell_last=active=true count=0 top=")
        );
        assert!(result.detail.contains("panicked at game.rs:42"));
        assert!(!result.detail.contains(&"x".repeat(4097)));
    }

    #[cfg(unix)]
    #[test]
    fn child_crash_after_ingame_is_not_a_host_pass() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "exit 101"])
            .spawn()
            .unwrap();
        child.wait().unwrap();
        let mut child = SmokeHost::Spawned(SmokeChild(child));
        let mut result = ExecutableSmokeResult {
            reached_ingame: true,
            reached_menu: true,
            ..Default::default()
        };
        let mut st = SmokeRunState {
            saw_shell_wnd_ok: true,
            ..Default::default()
        };
        st.last_snap.state = "InGame".into();

        assert!(smoke_child_exited_early(
            &mut st,
            &mut result,
            &mut child,
            true
        ));
        assert_eq!(result.exit_code, Some(101));
        assert_eq!(result.status, "process_exited");
        assert!(!result.executable_host_ok);

        let mut shutdown_child = std::process::Command::new("sh")
            .args(["-c", "exit 101"])
            .spawn()
            .unwrap();
        shutdown_child.wait().unwrap();
        let mut shutdown_child = SmokeHost::Spawned(SmokeChild(shutdown_child));
        let mut shutdown_result = ExecutableSmokeResult {
            reached_ingame: true,
            reached_menu: true,
            shell_wnd_ok: true,
            ..Default::default()
        };
        assert!(smoke_phase_wait_clean_exit(
            &mut st,
            &mut shutdown_result,
            &mut shutdown_child,
            true,
        ));
        assert_eq!(shutdown_result.status, "process_exited");
        assert!(!shutdown_result.executable_host_ok);
    }

    #[test]
    fn attached_host_reports_its_frame_loop_exit_and_receives_exit_on_kill() {
        let dir = tempfile::tempdir().unwrap();
        let host = AttachedSmokeHost {
            control_path: dir.path().join("control.txt"),
            status_path: dir.path().join("status.txt"),
            exit: std::sync::Arc::new(std::sync::Mutex::new(None)),
            map: None,
        };
        let exit = std::sync::Arc::clone(&host.exit);
        let mut smoke_host = SmokeHost::Attached(host.clone());
        assert!(smoke_host.try_wait().unwrap().is_none());
        smoke_host.kill();
        assert_eq!(fs::read_to_string(&host.control_path).unwrap().trim(), "exit");
        *exit.lock().unwrap() = Some(0);
        let ended = smoke_host.try_wait().unwrap().expect("ended");
        assert!(ended.success());
        assert_eq!(ended.code(), Some(0));
    }

    #[test]
    fn smoke_gates_parse_and_apply_their_gate_binary_predicates() {
        for gate in SmokeGate::ALL {
            assert_eq!(gate.name().parse::<SmokeGate>().unwrap(), gate);
        }
        assert!("nope".parse::<SmokeGate>().is_err());
        let menu = ExecutableSmokeResult {
            status: "success".into(),
            executable_host_ok: true,
            reached_menu: true,
            shell_wnd_ok: true,
            ..Default::default()
        };
        assert!(SmokeGate::BootShellMenu.passed(&menu));
        assert!(!SmokeGate::IssueOrders.passed(&menu));
        let orders = ExecutableSmokeResult {
            reached_ingame: true,
            gameplay_cmd_ok: true,
            ..menu.clone()
        };
        assert!(SmokeGate::IssueOrders.passed(&orders));
        assert!(!SmokeGate::SaveLoad.passed(&orders));
        let timed_out = ExecutableSmokeResult {
            status: "timeout".into(),
            ..orders
        };
        assert!(!SmokeGate::IssueOrders.passed(&timed_out));
    }

    #[test]
    fn ingame_frame_progress_expires_stalled_smoke_evidence() {
        let mut st = SmokeRunState::default();
        let mut result = ExecutableSmokeResult::default();
        let mut snap = StatusSnap {
            state: "InGame".into(),
            frame: 210,
            ..Default::default()
        };
        latch_status_residuals(&mut st, &mut result, &snap);
        assert!(!ingame_frame_stalled(&st, Duration::from_secs(30)));

        st.last_ingame_frame_progress_at = Some(Instant::now() - Duration::from_secs(31));
        latch_status_residuals(&mut st, &mut result, &snap);
        assert!(ingame_frame_stalled(&st, Duration::from_secs(30)));

        snap.frame += 1;
        latch_status_residuals(&mut st, &mut result, &snap);
        assert!(!ingame_frame_stalled(&st, Duration::from_secs(30)));
    }

    #[cfg(unix)]
    #[test]
    fn wall_timeout_rejects_stalled_ingame_host() {
        let dir = tempfile::tempdir().unwrap();
        let control = dir.path().join("control.txt");
        let mut child = SmokeHost::Spawned(SmokeChild(
            std::process::Command::new("sleep")
                .arg("10")
                .spawn()
                .unwrap(),
        ));
        let mut st = SmokeRunState {
            saw_shell_wnd_ok: true,
            last_ingame_frame: Some(210),
            last_ingame_frame_progress_at: Some(Instant::now() - Duration::from_secs(31)),
            ..Default::default()
        };
        let mut result = ExecutableSmokeResult {
            reached_ingame: true,
            ..Default::default()
        };

        assert!(smoke_wall_budget_exceeded(
            &mut st,
            &mut result,
            &mut child,
            &control,
            Duration::from_secs(1),
            Instant::now() - Duration::from_secs(2),
        ));
        assert_eq!(result.status, "ingame_stalled");
        assert!(!result.executable_host_ok);
        assert!(result.detail.contains("last_ingame_frame=Some(210)"));
    }

    #[test]
    fn automated_smoke_waits_for_live_shell_wnd_before_skirmish_command() {
        let dir = tempfile::tempdir().unwrap();
        let control = dir.path().join("control.txt");
        fs::write(&control, "").unwrap();
        let mut st = SmokeRunState::default();
        let mut result = ExecutableSmokeResult::default();
        let mut snap = StatusSnap {
            state: "Loading".into(),
            startup_progress: 1.0,
            ..Default::default()
        };
        let started = Instant::now() - Duration::from_secs(26);

        smoke_phase_wait_menu(
            &mut st,
            &mut result,
            &snap,
            &control,
            ExecutableSmokeLaunch::HeadlessHost,
            SmokeDriver::Automated,
            started,
        );
        assert_eq!(st.phase, 0);
        assert!(fs::read_to_string(&control).unwrap().is_empty());

        snap.state = "Menu".into();
        snap.shell_active = true;
        smoke_phase_wait_menu(
            &mut st,
            &mut result,
            &snap,
            &control,
            ExecutableSmokeLaunch::HeadlessHost,
            SmokeDriver::Automated,
            started,
        );
        assert_eq!(st.phase, 0, "shell_active alone is not a WND tree");

        snap.shell_screen_count = 1;
        snap.shell_top_wnd = "Menus/MainMenu.wnd".into();
        latch_shell_wnd_residuals(&mut st, &snap);
        smoke_phase_wait_menu(
            &mut st,
            &mut result,
            &snap,
            &control,
            ExecutableSmokeLaunch::HeadlessHost,
            SmokeDriver::Automated,
            started,
        );
        assert_eq!(st.phase, 10);
        assert!(
            fs::read_to_string(&control)
                .unwrap()
                .contains("open_skirmish_menu")
        );
    }

    #[test]
    fn windowed_presentation_uses_live_screens_without_physical_input_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let control = dir.path().join("control.txt");
        let mut st = SmokeRunState {
            phase: 20,
            present_frame_goal: true,
            ..Default::default()
        };
        let mut result = ExecutableSmokeResult::default();
        let mut snap = StatusSnap {
            state: "Menu".into(),
            shell_active: true,
            shell_top_wnd: "Menus/MainMenu.wnd".into(),
            shell_screen_count: 1,
            live_frame_ok: true,
            ..Default::default()
        };
        let map = "Maps/Lone Eagle/Lone Eagle.map";

        smoke_phase_windowed_inject(&mut st, &mut result, &snap, &control, map);
        assert_eq!(fs::read_to_string(&control).unwrap().trim(), "winit_menu_nav");

        snap.shell_top_wnd = "Menus/SkirmishGameOptionsMenu.wnd".into();
        snap.shell_screen_count = 2;
        snap.last_gameplay_cmd = "winit_menu_nav_partial".into();
        st.commanded_at = Some(Instant::now() - Duration::from_secs(1));
        smoke_phase_windowed_inject(&mut st, &mut result, &snap, &control, map);
        assert!(fs::read_to_string(&control).unwrap().starts_with("start_game|"));

        snap.state = "InGame".into();
        result.reached_ingame = true;
        smoke_phase_windowed_inject(&mut st, &mut result, &snap, &control, map);
        assert_eq!(st.phase, 2);
        assert_eq!(st.gameplay_step, 0);
        assert!(!st.saw_wnd_widget_tree_nav);
        assert!(!st.saw_interactive_gameplay);
        assert!(!st.windowed_gameplay_order_sent);
        assert!(!result.playable_claim);
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn resolve_runtime_exe_honors_general_runtime_exe_override() {
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("generals_override_bin");
        fs::write(&fake, b"ok").unwrap();
        let before = std::env::var_os("GENERALS_RUNTIME_EXE");
        isolated_environment_case(
            "resolve_runtime_exe_honors_general_runtime_exe_override",
            "override",
            "GENERALS_RUNTIME_EXE",
            fake.as_os_str(),
            || {
                let expected = PathBuf::from(std::env::var_os("GENERALS_RUNTIME_EXE").unwrap());
                assert!(expected.is_file());
                let got = resolve_runtime_exe();
                assert_eq!(got, Some(expected.canonicalize().unwrap()));
            },
        );
        assert_eq!(
            std::env::var_os("GENERALS_RUNTIME_EXE"),
            before,
            "the child override must not mutate the parent environment"
        );
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
        r.map_seen = "Lone Eagle".into();
        r.apply_host_vertical_slice_gate();
        assert!(!r.host_vertical_slice_ok, "the renderer needs a shared object roster");
        r.max_shared_presentation_objects = 8;
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
    fn radar_capture_compares_clear_terrain_not_black_or_lone_marker() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("radar.png");
        let reference = "0,0,128,128;20,20,140,150,160;30,20,140,150,160;20,30,140,150,160;30,30,140,150,160;h,50,50;h,60,50;h,50,60;h,60,60";
        let mut image = image::RgbImage::new(128, 128);
        image.put_pixel(20, 107, image::Rgb([140, 150, 160]));
        image.save(&path).unwrap();
        assert!(!png_minimap_on_screen_has_terrain(&path, reference));
        for (x, y) in [(30, 107), (20, 97), (30, 97)] {
            image.put_pixel(x, y, image::Rgb([140, 150, 160]));
        }
        image.save(&path).unwrap();
        assert!(!png_minimap_on_screen_has_terrain(&path, reference), "uniform terrain has no image identity");
        let solid = image::RgbImage::from_pixel(128, 128, image::Rgb([140, 150, 160]));
        solid.save(&path).unwrap();
        assert!(!png_minimap_on_screen_has_terrain(&path, reference), "flat fill is not the shrouded map");
        let dark = "0,0,128,128;20,20,32,32,32;30,20,32,32,32;20,30,32,32,32;30,30,32,32,32;h,50,50;h,60,50;h,50,60;h,60,60";
        let mut black = image::RgbImage::new(128, 128);
        black.save(&path).unwrap();
        assert!(!png_minimap_on_screen_has_terrain(&path, dark), "black is not dark terrain");
        black.put_pixel(20, 107, image::Rgb([32, 32, 32]));
        black.save(&path).unwrap();
        assert!(!png_minimap_on_screen_has_terrain(&path, dark), "one marker is not terrain");
        for (x, y) in [(30, 107), (20, 97), (30, 97)] {
            black.put_pixel(x, y, image::Rgb([32, 32, 32]));
        }
        black.save(&path).unwrap();
        assert!(!png_minimap_on_screen_has_terrain(&path, dark), "uniform dark pixels are unverifiable");
        assert!(!png_minimap_on_screen_has_terrain(&path, ""));
    }

    #[test]
    fn radar_capture_requires_observed_terrain_detail_even_inside_shroud() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("radar.png");
        // The captured retail snow radar varies by only a few RGB levels.
        let reference = "0,0,128,128;20,20,98,98,98;30,20,99,99,99;40,20,100,100,100;50,20,101,101,101;20,30,102,102,102;30,30,103,103,103;40,30,104,104,104;50,30,105,105,105;h,70,50;h,80,50;h,70,60;h,80,60";
        let mut image = image::RgbImage::new(128, 128);
        for y in 94..=110 {
            for x in 18..=52 {
                image.put_pixel(x, y, image::Rgb([101, 101, 101]));
            }
        }
        image.save(&path).unwrap();
        assert!(!png_minimap_on_screen_has_terrain(&path, reference), "matching flat clear patch is not terrain evidence");
        for (x, y, color) in [(20, 107, 98), (30, 107, 99), (40, 107, 100), (50, 107, 101), (20, 97, 102), (30, 97, 103), (40, 97, 104), (50, 97, 105)] {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    image.put_pixel((x + dx) as u32, (y + dy) as u32, image::Rgb([color, color, color]));
                }
            }
        }
        image.save(&path).unwrap();
        assert!(png_minimap_on_screen_has_terrain(&path, reference));
        let clear_reference = reference.split(";h,").next().unwrap();
        for n in 1..=3 {
            let mut sparse = clear_reference.to_owned();
            for (u, v) in [(70, 50), (80, 50), (70, 60)].into_iter().take(n) {
                sparse.push_str(&format!(";h,{u},{v}"));
            }
            assert!(png_minimap_on_screen_has_terrain(&path, &sparse), "sparse hidden probes are not a shroud mismatch");
        }
        // Every probe has the same 3x3 neighborhood. Selecting whichever
        // neighboring pixel best matches its expected color would manufacture
        // a convincing gradient although the fixed sample centers are flat.
        for (x, y) in [(20, 107), (30, 107), (40, 107), (50, 107), (20, 97), (30, 97), (40, 97), (50, 97)] {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let color = if dx == 0 && dy == 0 { 101 }
                        else if (dx + dy) % 2 == 0 { 98 } else { 105 };
                    image.put_pixel((x + dx) as u32, (y + dy) as u32, image::Rgb([color, color, color]));
                }
            }
        }
        image.save(&path).unwrap();
        assert!(!png_minimap_on_screen_has_terrain(&path, reference), "checkerboard neighborhoods must not manufacture terrain correlation");
    }

    #[test]
    fn radar_capture_ignores_marker_edge_pixels_when_proving_snow_detail() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("radar-marker-overlay.png");
        let mut reference = String::from("0,0,128,128");
        let mut image = image::RgbImage::new(128, 128);
        for index in 0..16 {
            let x = 20 + index * 5;
            let y = 24;
            let color = 98 + index % 8;
            reference.push_str(&format!(";{x},{y},{color},{color},{color}"));
            // Eight terrain samples remain native; the other eight fall on
            // antialiased radar marker edges and stay inside the broad visual
            // tolerance while destroying the snow's subtle correlation.
            let observed = if index < 8 {
                color
            } else if index % 2 == 0 {
                color + 15
            } else {
                color - 15
            };
            let pixel_y = 127 - y;
            image.put_pixel(x as u32, pixel_y as u32, image::Rgb([observed; 3]));
        }
        for (x, y) in [(70, 50), (80, 50), (70, 60), (80, 60)] {
            reference.push_str(&format!(";h,{x},{y}"));
        }
        image.save(&path).unwrap();
        assert!(png_minimap_on_screen_has_terrain(&path, &reference));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn host_ok_requires_shell_wnd_when_wnd_enabled() {
        let before = std::env::var_os("GENERALS_RUNTIME_HOST_WND");
        isolated_environment_case(
            "host_ok_requires_shell_wnd_when_wnd_enabled",
            "wnd-on",
            "GENERALS_RUNTIME_HOST_WND",
            std::ffi::OsStr::new("1"),
            || {
                assert!(
                    !executable_host_ok_from_residuals(runtime_host_wnd_enabled(), true, false),
                    "WND path must not claim host_ok without shell_wnd residual"
                );
                assert!(executable_host_ok_from_residuals(runtime_host_wnd_enabled(), true, true));
                assert!(!executable_host_ok_from_residuals(runtime_host_wnd_enabled(), false, true));
            },
        );
        isolated_environment_case(
            "host_ok_requires_shell_wnd_when_wnd_enabled",
            "wnd-off",
            "GENERALS_RUNTIME_HOST_WND",
            std::ffi::OsStr::new("0"),
            || {
                assert!(
                    executable_host_ok_from_residuals(runtime_host_wnd_enabled(), true, false),
                    "WND-off path allows host_ok without shell residual"
                );
                let ordinary = crate::command_line::CommandLineArgs::parse_from_args(vec!["generals".into()])
                    .expect("regular startup");
                let mut attached = ordinary.clone();
                attached.enable_runtime_host_wnd();
                assert!(attached.use_runtime_host_wnd());
                assert!(!ordinary.use_runtime_host_wnd());
                assert_eq!(std::env::var("GENERALS_RUNTIME_HOST_WND").unwrap(), "0");
            },
        );
        assert_eq!(
            std::env::var_os("GENERALS_RUNTIME_HOST_WND"),
            before,
            "child WND choices must not mutate the parent environment"
        );
    }

    #[test]
    fn smoke_tracks_shell_wnd_residual_keys() {
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
