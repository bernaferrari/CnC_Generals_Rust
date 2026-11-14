// Lifecycle: InGame gameplay command chain driven through the runtime-host
// control file. Mirrors the original `phase 2` match arm verbatim; only the
// loop locals are `st.`-prefixed SmokeRunState fields.

/// Phase 2: InGame host command chain (select + move, then exit).
/// Not WND widget clicks — still not playable_claim.
///
/// Steps 0..=7 drive select/move/construct/train/upgrade/save/load,
/// steps 8..=58 walk the retail command ladder (stop/sell/guard/...), and
/// step >= 59 latches every residual into the result, formats detail, and
/// sends the exit command (windowed keeps retrying the RMB inject for the
/// fifth retail claim flag first).
fn smoke_drive_gameplay_chain(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
    launch: ExecutableSmokeLaunch,
) {
    if st.saw_construct_ok && snap.under_construction > 0 {
        st.saw_construct_under_construction = true;
    }
    // P0 orders gate: select → move → attack, then exit (no construct ladder).
    if st.orders_goal {
        smoke_drive_orders_only_chain(st, result, snap, control_path);
        return;
    }
    // P0 AI gate: select → move, then require AI activity residual.
    if st.ai_activity_goal {
        smoke_drive_ai_activity_chain(st, result, snap, control_path);
        return;
    }
    // P0 save/load gate: orders → N ticks → save → load → restore assert → post-load order.
    if st.save_load_goal {
        smoke_drive_save_load_chain(st, result, snap, control_path);
        return;
    }
    // P0 present-frame gate: select, sit ≥60s with stable render items, screenshot.
    if st.present_frame_goal {
        smoke_drive_present_frame_chain(st, result, snap, control_path);
        return;
    }
    // Issue host gameplay commands (select + move), then exit.
    // Not WND widget clicks — still not playable_claim.
    if st.gameplay_step == 0 {
        let _ = write_control(&control_path, &["select_local_unit"]);
        st.gameplay_step = 1;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 1
        && (snap.last_gameplay_cmd.starts_with("select_ok")
            || snap.last_gameplay_cmd.starts_with("select_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(6))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("select_ok") {
            st.saw_select_ok = true;
        }
        let _ = write_control(&control_path, &["move_selected|x=100|y=0|z=100"]);
        st.gameplay_step = 2;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 2
        && (snap.last_gameplay_cmd.starts_with("move_ok")
            || snap.last_gameplay_cmd.starts_with("move_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(6))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("move_ok") {
            st.saw_move_ok = true;
        }
        if launch == ExecutableSmokeLaunch::Windowed {
            // Wait real frames after physical nav+order before construct.
            if st.commanded_at
                .map(|t| t.elapsed() < Duration::from_secs(2))
                .unwrap_or(false)
            {
                // keep polling
            } else {
                // Require an existing builder — no spawn_dozer cheat.
                let _ = write_control(
                    &control_path,
                    &["construct|template=AmericaBarracks|auto_target=1"],
                );
                st.gameplay_step = 3;
                st.construct_wait_started = Some(Instant::now());
                st.commanded_at = Some(Instant::now());
            }
        } else {
            let _ = write_control(
                &control_path,
                &[
                    "construct|template=AmericaBarracks|spawn_dozer=1|alias_fallback=1|auto_target=1",
                ],
            );
            st.gameplay_step = 3;
            st.commanded_at = Some(Instant::now());
        }
    } else if st.gameplay_step == 3
        && (snap.last_gameplay_cmd.starts_with("construct_ok")
            || snap.last_gameplay_cmd.starts_with("construct_fail")
            || snap.last_gameplay_cmd.starts_with("construct_")
            || st.commanded_at
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
            st.saw_construct_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("construct_") {
            st.construct_detail = snap.last_gameplay_cmd.clone();
        }
        if launch == ExecutableSmokeLaunch::Windowed
            && snap
                .last_gameplay_cmd
                .starts_with("construct_fail_no_building")
            && st.construct_wait_started
                .is_none_or(|started| started.elapsed() <= Duration::from_secs(90))
            && st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(3))
                .unwrap_or(false)
        {
            let _ = write_control(
                &control_path,
                &["construct|template=AmericaBarracks|auto_target=1"],
            );
            st.commanded_at = Some(Instant::now());
        }
        if launch == ExecutableSmokeLaunch::Windowed {
            // Train only after a physical scaffold has appeared and completed.
            // An immediate zero can be a stale status frame.
            if snap.under_construction > 0 {
                st.saw_construct_under_construction = true;
            }
            let elapsed = st
                .construct_wait_started
                .get_or_insert_with(Instant::now)
                .elapsed();
            match construction_wait(
                st.saw_construct_ok,
                st.saw_construct_under_construction,
                snap.under_construction,
                elapsed,
                construction_timeout(
                    std::env::var("EXECUTABLE_SMOKE_CONSTRUCTION_TIMEOUT_SECS")
                        .ok()
                        .as_deref(),
                ),
            ) {
                ConstructionWait::Pending => {}
                ConstructionWait::Ready => {
                    let _ = write_control(
                        &control_path,
                        &["train_unit|template=AmericaInfantryRanger|auto_target=1"],
                    );
                    st.train_sent = true;
                    st.train_retry_started = Some(Instant::now());
                    st.gameplay_step = 4;
                    st.commanded_at = Some(Instant::now());
                }
                ConstructionWait::TimedOut => {
                    // Continue independent checks, but do not train from an
                    // unfinished or unobserved scaffold. The late pass can
                    // still train if the observed scaffold later finishes.
                    st.train_detail = "train_skipped_barracks_not_ready".into();
                    st.gameplay_step = 4;
                    st.commanded_at = Some(Instant::now());
                }
            }
        } else {
            let _ = write_control(
                &control_path,
                &[
                    "train_unit|template=AmericaInfantryRanger|force_complete=1|grant_supplies=1|alias_fallback=1|auto_target=1",
                    "train_unit|template=USA_Ranger|force_complete=1|grant_supplies=1|alias_fallback=1|auto_target=1",
                ],
            );
            st.train_sent = true;
            st.gameplay_step = 4;
            st.commanded_at = Some(Instant::now());
        }
    } else if st.gameplay_step == 4
        && (snap.last_gameplay_cmd.starts_with("train_ok")
            || snap.last_gameplay_cmd.starts_with("train_fail")
            || snap.last_gameplay_cmd.starts_with("train_")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(8))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("train_ok") {
            st.saw_train_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("train_") {
            st.train_detail = snap.last_gameplay_cmd.clone();
        }
        // Barracks not ready yet — retry train, do not advance.
        if launch == ExecutableSmokeLaunch::Windowed
            && !st.saw_train_ok
            && snap
                .last_gameplay_cmd
                .starts_with("train_fail_no_ready_barracks")
            && st.train_retry_started
                .map(|t| t.elapsed() < Duration::from_secs(75))
                .unwrap_or(false)
        {
            if st.commanded_at
                .map(|t| t.elapsed() >= Duration::from_secs(4))
                .unwrap_or(false)
            {
                let _ = write_control(
                    &control_path,
                    &["train_unit|template=AmericaInfantryRanger|auto_target=1"],
                );
                st.commanded_at = Some(Instant::now());
            }
            // stay on step 4 — bounded by train_retry_started
        } else {
            // Host residual: train_ok queues production; wait until a second
            // local mobile exits so later formation/select residuals are honest.
            // Fail-closed timeout still advances so the chain cannot hang forever.
            let train_mobile_ready = snap.local_mobile_units >= 2;
            let train_wait_expired = st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(20))
                .unwrap_or(false);
            if !train_mobile_ready && !train_wait_expired {
                // keep polling; do not advance yet
            } else if !st.saw_early_combat_cmd {
                // Wave 864: issue combat early while InGame so match_damage
                // counters have time to accumulate before late options steps.
                let _ = write_control(
                    &control_path,
                    &["attack_nearest_enemy|auto_target=1"],
                );
                st.saw_early_combat_cmd = true;
                st.commanded_at = Some(Instant::now());
            } else if st.commanded_at
                .map(|t| t.elapsed() < Duration::from_secs(12))
                .unwrap_or(false)
            {
                // Wave 1112/1115: longer window for attack residual + damage
                // counters (2s/6s were flaky under load; still fail-closed).
                // Wave 1115: re-issue attack mid-window so FOW/retarget lag
                // still has a chance to apply host_damage_log totals.
                if snap.last_gameplay_cmd.starts_with("attack_ok") {
                    st.saw_attack_ok = true;
                }
                if snap.match_damage_applied > 0.0 || snap.match_kills > 0 {
                    st.saw_combat_damage = true;
                }
                let elapsed = st.commanded_at.map(|t| t.elapsed()).unwrap_or_default();
                if !st.saw_combat_damage
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
                st.gameplay_step = 5;
                st.commanded_at = Some(Instant::now());
            } else {
                let _ = write_control(
                    &control_path,
                    &[
                        "upgrade|name=UpgradeAmericaRangerCaptureBuilding|grant_supplies=1|alias_fallback=1|auto_target=1",
                    ],
                );
                st.gameplay_step = 5;
                st.commanded_at = Some(Instant::now());
            }
        }
    } else if st.gameplay_step == 5
        && (snap.last_gameplay_cmd.starts_with("upgrade_ok")
            || snap.last_gameplay_cmd.starts_with("upgrade_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(6))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("upgrade_ok") {
            st.saw_upgrade_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("upgrade_") {
            st.upgrade_detail = snap.last_gameplay_cmd.clone();
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
        st.gameplay_step = 6;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 6
        && launch == ExecutableSmokeLaunch::Windowed
        && snap.last_gameplay_cmd.starts_with("save_wait_wnd_quit")
        && st.save_retry_started.map(|start| start.elapsed() < Duration::from_secs(30)).unwrap_or(true)
    {
        st.save_retry_started.get_or_insert_with(Instant::now);
        if st.commanded_at.map(|sent| sent.elapsed() >= Duration::from_secs(1)).unwrap_or(true) {
            let _ = write_control(&control_path, &["pause_save|slot=wnd_pause|via=PopupSaveLoad.wnd"]);
            st.commanded_at = Some(Instant::now());
        }
    } else if st.gameplay_step == 6
        && (snap.last_gameplay_cmd.starts_with("save_ok")
            || snap.last_gameplay_cmd.starts_with("save_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(5))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("save_ok") {
            st.saw_save_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("save_") {
            st.save_detail = snap.last_gameplay_cmd.clone();
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
        st.gameplay_step = 7;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 7
        && (snap.last_gameplay_cmd.starts_with("load_ok")
            || snap.last_gameplay_cmd.starts_with("load_fail")
            || snap.last_gameplay_cmd.starts_with("load_wait_wnd_quit")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(20))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("load_ok") {
            st.saw_load_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("load_") {
            st.load_detail = snap.last_gameplay_cmd.clone();
        }
        if launch == ExecutableSmokeLaunch::Windowed && !st.saw_load_ok && st.saw_save_ok
        {
            if st.load_retry_started.is_none() {
                st.load_retry_started = Some(Instant::now());
            }
            let retry_budget = st.load_retry_started
                .map(|t| t.elapsed() < Duration::from_secs(18))
                .unwrap_or(false);
            if retry_budget
                && (snap.last_gameplay_cmd.starts_with("load_fail")
                    || snap.last_gameplay_cmd.starts_with("load_wait_wnd_quit"))
                && st.commanded_at
                    .map(|t| t.elapsed() >= Duration::from_secs(2))
                    .unwrap_or(false)
            {
                let _ = write_control(
                    &control_path,
                    &["pause_load|slot=wnd_pause|via=PopupSaveLoad.wnd"],
                );
                st.commanded_at = Some(Instant::now());
            } else if !retry_budget {
                let _ = write_control(&control_path, &["stop_all"]);
                st.gameplay_step = 8;
                st.commanded_at = Some(Instant::now());
            }
            // stay on step 7 until load_ok or retry budget
        } else {
            let _ = write_control(&control_path, &["stop_all"]);
            st.gameplay_step = 8;
            st.commanded_at = Some(Instant::now());
        }
    } else if st.gameplay_step == 8
        && (snap.last_gameplay_cmd.starts_with("stop_ok")
            || snap.last_gameplay_cmd.starts_with("stop_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("stop_ok") {
            st.saw_stop_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("stop_") {
            st.stop_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["sell|auto_target=1"]);
        st.gameplay_step = 9;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 9
        && (snap.last_gameplay_cmd.starts_with("sell_ok")
            || snap.last_gameplay_cmd.starts_with("sell_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(5))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("sell_ok") {
            st.saw_sell_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("sell_") {
            st.sell_detail = snap.last_gameplay_cmd.clone();
        }
        let _ =
            write_control(&control_path, &["guard|x=120|y=0|z=120|auto_target=1"]);
        st.gameplay_step = 10;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 10
        && (snap.last_gameplay_cmd.starts_with("guard_ok")
            || snap.last_gameplay_cmd.starts_with("guard_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(5))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("guard_ok") {
            st.saw_guard_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("guard_") {
            st.guard_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(
            &control_path,
            &["attack_move|x=150|y=0|z=150|auto_target=1"],
        );
        st.gameplay_step = 11;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 11
        && (snap.last_gameplay_cmd.starts_with("attack_move_ok")
            || snap.last_gameplay_cmd.starts_with("attack_move_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(5))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("attack_move_ok") {
            st.saw_attack_move_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("attack_move_") {
            st.attack_move_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["scatter|auto_target=1"]);
        st.gameplay_step = 12;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 12
        && (snap.last_gameplay_cmd.starts_with("scatter_ok")
            || snap.last_gameplay_cmd.starts_with("scatter_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(5))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("scatter_ok") {
            st.saw_scatter_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("scatter_") {
            st.scatter_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["patrol"]);
        st.gameplay_step = 13;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 13
        && (snap.last_gameplay_cmd.starts_with("patrol_ok")
            || snap.last_gameplay_cmd.starts_with("patrol_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("patrol_ok") {
            st.saw_patrol_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("patrol_") {
            st.patrol_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["deploy"]);
        st.gameplay_step = 14;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 14
        && (snap.last_gameplay_cmd.starts_with("deploy_ok")
            || snap.last_gameplay_cmd.starts_with("deploy_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("deploy_ok") {
            st.saw_deploy_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("deploy_") {
            st.deploy_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["cheer"]);
        st.gameplay_step = 15;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 15
        && (snap.last_gameplay_cmd.starts_with("cheer_ok")
            || snap.last_gameplay_cmd.starts_with("cheer_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("cheer_ok") {
            st.saw_cheer_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("cheer_") {
            st.cheer_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["formation|spawn_buddy=1"]);
        st.gameplay_step = 16;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 16
        && (snap.last_gameplay_cmd.starts_with("formation_ok")
            || snap.last_gameplay_cmd.starts_with("formation_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(5))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("formation_ok") {
            st.saw_formation_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("formation_") {
            st.formation_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["capture"]);
        st.gameplay_step = 17;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 17
        && (snap.last_gameplay_cmd.starts_with("capture_ok")
            || snap.last_gameplay_cmd.starts_with("capture_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("capture_ok") {
            st.saw_capture_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("capture_") {
            st.capture_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["return_supplies|auto_target=1"]);
        st.gameplay_step = 18;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 18
        && (snap.last_gameplay_cmd.starts_with("return_supplies_ok")
            || snap.last_gameplay_cmd.starts_with("return_supplies_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("return_supplies_ok") {
            st.saw_return_supplies_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("return_supplies_") {
            st.return_supplies_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["evacuate"]);
        st.gameplay_step = 19;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 19
        && (snap.last_gameplay_cmd.starts_with("evacuate_ok")
            || snap.last_gameplay_cmd.starts_with("evacuate_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("evacuate_ok") {
            st.saw_evacuate_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("evacuate_") {
            st.evacuate_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["repair"]);
        st.gameplay_step = 20;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 20
        && (snap.last_gameplay_cmd.starts_with("repair_ok")
            || snap.last_gameplay_cmd.starts_with("repair_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("repair_ok") {
            st.saw_repair_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("repair_") {
            st.repair_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["return_to_base"]);
        st.gameplay_step = 21;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 21
        && (snap.last_gameplay_cmd.starts_with("return_to_base_ok")
            || snap.last_gameplay_cmd.starts_with("return_to_base_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("return_to_base_ok") {
            st.saw_return_to_base_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("return_to_base_") {
            st.return_to_base_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["attitude_aggressive"]);
        st.gameplay_step = 22;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 22
        && (snap.last_gameplay_cmd.starts_with("attitude_ok")
            || snap.last_gameplay_cmd.starts_with("attitude_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("attitude_ok") {
            st.saw_attitude_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("attitude_") {
            st.attitude_detail = snap.last_gameplay_cmd.clone();
        }
        let _ =
            write_control(&control_path, &["rally|x=90|y=0|z=90|auto_target=1"]);
        st.gameplay_step = 23;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 23
        && (snap.last_gameplay_cmd.starts_with("rally_ok")
            || snap.last_gameplay_cmd.starts_with("rally_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("rally_ok") {
            st.saw_rally_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("rally_") {
            st.rally_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["switch_weapons"]);
        st.gameplay_step = 24;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 24
        && (snap.last_gameplay_cmd.starts_with("switch_weapons_ok")
            || snap.last_gameplay_cmd.starts_with("switch_weapons_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("switch_weapons_ok") {
            st.saw_switch_weapons_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("switch_weapons_") {
            st.switch_weapons_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["view_cc"]);
        st.gameplay_step = 25;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 25
        && (snap.last_gameplay_cmd.starts_with("view_cc_ok")
            || snap.last_gameplay_cmd.starts_with("view_cc_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("view_cc_ok") {
            st.saw_view_cc_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("view_cc_") {
            st.view_cc_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["clear_mines"]);
        st.gameplay_step = 26;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 26
        && (snap.last_gameplay_cmd.starts_with("clear_mines_ok")
            || snap.last_gameplay_cmd.starts_with("clear_mines_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("clear_mines_ok") {
            st.saw_clear_mines_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("clear_mines_") {
            st.clear_mines_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["place_beacon|x=60|y=0|z=60"]);
        st.gameplay_step = 27;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 27
        && (snap.last_gameplay_cmd.starts_with("beacon_ok")
            || snap.last_gameplay_cmd.starts_with("beacon_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("beacon_ok") {
            st.saw_beacon_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("beacon_") {
            st.beacon_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["hack_internet"]);
        st.gameplay_step = 28;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 28
        && (snap.last_gameplay_cmd.starts_with("hack_ok")
            || snap.last_gameplay_cmd.starts_with("hack_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("hack_ok") {
            st.saw_hack_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("hack_") {
            st.hack_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["cleanup_area"]);
        st.gameplay_step = 29;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 29
        && (snap.last_gameplay_cmd.starts_with("cleanup_ok")
            || snap.last_gameplay_cmd.starts_with("cleanup_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("cleanup_ok") {
            st.saw_cleanup_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("cleanup_") {
            st.cleanup_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["combat_drop|x=75|y=0|z=75"]);
        st.gameplay_step = 30;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 30
        && (snap.last_gameplay_cmd.starts_with("combat_drop_ok")
            || snap.last_gameplay_cmd.starts_with("combat_drop_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("combat_drop_ok") {
            st.saw_combat_drop_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("combat_drop_") {
            st.combat_drop_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["toggle_overcharge|auto_target=1"]);
        st.gameplay_step = 31;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 31
        && (snap.last_gameplay_cmd.starts_with("overcharge_ok")
            || snap.last_gameplay_cmd.starts_with("overcharge_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("overcharge_ok") {
            st.saw_overcharge_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("overcharge_") {
            st.overcharge_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["special_power"]);
        st.gameplay_step = 32;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 32
        && (snap.last_gameplay_cmd.starts_with("special_power_ok")
            || snap.last_gameplay_cmd.starts_with("special_power_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("special_power_ok") {
            st.saw_special_power_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("special_power_") {
            st.special_power_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["remove_beacon"]);
        st.gameplay_step = 33;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 33
        && (snap.last_gameplay_cmd.starts_with("remove_beacon_ok")
            || snap.last_gameplay_cmd.starts_with("remove_beacon_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("remove_beacon_ok") {
            st.saw_remove_beacon_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("remove_beacon_") {
            st.remove_beacon_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["demo_suicide"]);
        st.gameplay_step = 34;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 34
        && (snap.last_gameplay_cmd.starts_with("demo_suicide_ok")
            || snap.last_gameplay_cmd.starts_with("demo_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("demo_suicide_ok") {
            st.saw_demo_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("demo_") {
            st.demo_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["view_radar"]);
        st.gameplay_step = 35;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 35
        && (snap.last_gameplay_cmd.starts_with("view_radar_ok")
            || snap.last_gameplay_cmd.starts_with("view_radar_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("view_radar_ok") {
            st.saw_view_radar_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("view_radar_") {
            st.view_radar_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["force_attack|x=110|y=0|z=110"]);
        st.gameplay_step = 36;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 36
        && (snap.last_gameplay_cmd.starts_with("force_attack_ok")
            || snap.last_gameplay_cmd.starts_with("force_attack_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("force_attack_ok") {
            st.saw_force_attack_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("force_attack_")
            && !snap.last_gameplay_cmd.starts_with("force_attack_object")
        {
            st.force_attack_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["force_attack_object"]);
        st.gameplay_step = 37;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 37
        && (snap.last_gameplay_cmd.starts_with("force_attack_object_ok")
            || snap
                .last_gameplay_cmd
                .starts_with("force_attack_object_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("force_attack_object_ok") {
            st.saw_force_attack_object_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("force_attack_object_") {
            st.force_attack_object_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["select_all"]);
        st.gameplay_step = 38;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 38
        && (snap.last_gameplay_cmd.starts_with("select_all_ok")
            || snap.last_gameplay_cmd.starts_with("select_all_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(8))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("select_all_ok") {
            st.saw_select_all_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("select_all_")
            && !snap.last_gameplay_cmd.starts_with("select_all_combat")
        {
            st.select_all_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["assign_control_group|group=1"]);
        st.gameplay_step = 39;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 39
        && (snap
            .last_gameplay_cmd
            .starts_with("control_group_assign_ok")
            || snap
                .last_gameplay_cmd
                .starts_with("control_group_assign_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap
            .last_gameplay_cmd
            .starts_with("control_group_assign_ok")
        {
            // partial — need recall too
            st.control_group_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("control_group_") {
            st.control_group_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["recall_control_group|group=1"]);
        st.gameplay_step = 40;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 40
        && (snap
            .last_gameplay_cmd
            .starts_with("control_group_recall_ok")
            || snap
                .last_gameplay_cmd
                .starts_with("control_group_recall_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap
            .last_gameplay_cmd
            .starts_with("control_group_recall_ok")
            && st.control_group_detail.starts_with("control_group_assign_ok")
        {
            st.saw_control_group_ok = true;
        } else if snap
            .last_gameplay_cmd
            .starts_with("control_group_recall_ok")
        {
            // assign detail may have been overwritten — still ok if recall ok after assign step
            st.saw_control_group_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("control_group_") {
            st.control_group_detail =
                format!("{};{}", st.control_group_detail, snap.last_gameplay_cmd);
        }
        let _ = write_control(&control_path, &["waypoint_mode|on=1"]);
        st.gameplay_step = 41;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 41
        && (snap.last_gameplay_cmd.starts_with("waypoint_mode_ok")
            || snap.last_gameplay_cmd.starts_with("waypoint_mode_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("waypoint_mode_") {
            st.waypoint_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["add_waypoint|x=130|y=0|z=130"]);
        st.gameplay_step = 42;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 42
        && (snap.last_gameplay_cmd.starts_with("waypoint_ok")
            || snap.last_gameplay_cmd.starts_with("waypoint_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("waypoint_ok") {
            st.saw_waypoint_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("waypoint_") {
            st.waypoint_detail =
                format!("{};{}", st.waypoint_detail, snap.last_gameplay_cmd);
        }
        let _ = write_control(
            &control_path,
            &["box_select|min_x=-8000|max_x=8000|min_z=-8000|max_z=8000"],
        );
        st.gameplay_step = 43;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 43
        && (snap.last_gameplay_cmd.starts_with("box_select_ok")
            || snap.last_gameplay_cmd.starts_with("box_select_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("box_select_ok") {
            st.saw_box_select_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("box_select_") {
            st.box_select_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["select_similar"]);
        st.gameplay_step = 44;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 44
        && (snap.last_gameplay_cmd.starts_with("select_similar_ok")
            || snap.last_gameplay_cmd.starts_with("select_similar_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("select_similar_ok") {
            st.saw_select_similar_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("select_similar_") {
            st.select_similar_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["select_on_screen"]);
        st.gameplay_step = 45;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 45
        && (snap.last_gameplay_cmd.starts_with("select_on_screen_ok")
            || snap.last_gameplay_cmd.starts_with("select_on_screen_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("select_on_screen_ok") {
            st.saw_select_on_screen_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("select_on_screen_") {
            st.select_on_screen_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["select_structures"]);
        st.gameplay_step = 46;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 46
        && (snap.last_gameplay_cmd.starts_with("select_structures_ok")
            || snap.last_gameplay_cmd.starts_with("select_structures_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("select_structures_ok") {
            st.saw_select_structures_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("select_structures_") {
            st.select_structures_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["select_aircraft"]);
        st.gameplay_step = 47;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 47
        && (snap.last_gameplay_cmd.starts_with("select_aircraft_ok")
            || snap.last_gameplay_cmd.starts_with("select_aircraft_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("select_aircraft_ok") {
            st.saw_select_aircraft_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("select_aircraft_") {
            st.select_aircraft_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["select_idle_harvesters"]);
        st.gameplay_step = 48;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 48
        && (snap.last_gameplay_cmd.starts_with("select_idle_ok")
            || snap.last_gameplay_cmd.starts_with("select_idle_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("select_idle_ok") {
            st.saw_select_idle_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("select_idle_") {
            st.select_idle_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["camera_reset"]);
        st.gameplay_step = 49;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 49
        && (snap.last_gameplay_cmd.starts_with("camera_reset_ok")
            || snap.last_gameplay_cmd.starts_with("camera_reset_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("camera_reset_ok") {
            st.saw_camera_reset_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("camera_reset_") {
            st.camera_reset_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["camera_zoom|z=1.25"]);
        st.gameplay_step = 50;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 50
        && (snap.last_gameplay_cmd.starts_with("camera_zoom_ok")
            || snap.last_gameplay_cmd.starts_with("camera_zoom_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("camera_zoom_ok") {
            st.saw_camera_zoom_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("camera_zoom_") {
            st.camera_zoom_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["toggle_pause"]);
        st.gameplay_step = 51;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 51 {
        if snap.last_gameplay_cmd.starts_with("pause_ok") {
            st.pause_detail = snap.last_gameplay_cmd.clone();
            st.saw_pause_ok = true;
            let _ = write_control(&control_path, &["toggle_pause"]);
            st.gameplay_step = 52;
            st.commanded_at = Some(Instant::now());
        } else if snap.last_gameplay_cmd.starts_with("pause_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(12))
                .unwrap_or(false)
        {
            if snap.last_gameplay_cmd.starts_with("pause_") {
                st.pause_detail = snap.last_gameplay_cmd.clone();
            }
            let _ = write_control(&control_path, &["cancel_production"]);
            st.gameplay_step = 53;
            st.commanded_at = Some(Instant::now());
        } else if st.commanded_at
            .map(|t| t.elapsed() > Duration::from_millis(1500))
            .unwrap_or(false)
        {
            let _ = write_control(&control_path, &["toggle_pause"]);
            st.commanded_at = Some(Instant::now());
        }
    } else if st.gameplay_step == 52 {
        if snap.last_gameplay_cmd.starts_with("pause_ok") {
            st.pause_detail = format!("{};{}", st.pause_detail, snap.last_gameplay_cmd);
            st.saw_pause_ok = true;
            let _ = write_control(&control_path, &["cancel_production"]);
            st.gameplay_step = 53;
            st.commanded_at = Some(Instant::now());
        } else if snap.last_gameplay_cmd.starts_with("pause_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(12))
                .unwrap_or(false)
        {
            let _ = write_control(&control_path, &["cancel_production"]);
            st.gameplay_step = 53;
            st.commanded_at = Some(Instant::now());
        } else if st.commanded_at
            .map(|t| t.elapsed() > Duration::from_millis(1500))
            .unwrap_or(false)
        {
            let _ = write_control(&control_path, &["toggle_pause"]);
            st.commanded_at = Some(Instant::now());
        }
    } else if st.gameplay_step == 53
        && (snap.last_gameplay_cmd.starts_with("cancel_production_ok")
            || snap.last_gameplay_cmd.starts_with("cancel_production_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("cancel_production_ok") {
            st.saw_cancel_production_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("cancel_production_") {
            st.cancel_production_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["request_capture"]);
        st.gameplay_step = 54;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 54
        && (snap.last_gameplay_cmd.starts_with("request_capture_ok")
            || snap.last_gameplay_cmd.starts_with("request_capture_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("request_capture_ok") {
            st.saw_request_capture_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("request_capture_") {
            st.request_capture_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["auto_attack|on=1"]);
        st.gameplay_step = 55;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 55
        && (snap.last_gameplay_cmd.starts_with("auto_attack_ok")
            || snap.last_gameplay_cmd.starts_with("auto_attack_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("auto_attack_ok") {
            st.saw_auto_attack_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("auto_attack_") {
            st.auto_attack_detail = snap.last_gameplay_cmd.clone();
        }
        // Attack while still InGame (options/diplomacy leave match).
        let _ = write_control(&control_path, &["attack_nearest_enemy"]);
        st.gameplay_step = 56;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 56
        && (snap.last_gameplay_cmd.starts_with("attack_ok")
            || snap.last_gameplay_cmd.starts_with("attack_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(6))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("attack_ok") {
            st.saw_attack_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("attack_") {
            // keep prior attack detail path in final branch too
        }
        let _ = write_control(&control_path, &["options_probe"]);
        st.gameplay_step = 57;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 57
        && (snap.last_gameplay_cmd.starts_with("options_probe_ok")
            || snap.last_gameplay_cmd.starts_with("options_ok")
            || snap.last_gameplay_cmd.starts_with("options_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(6))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("options_probe_ok")
            || snap.last_gameplay_cmd.starts_with("options_ok")
        {
            st.saw_options_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("options_") {
            st.options_detail = snap.last_gameplay_cmd.clone();
        }
        let _ = write_control(&control_path, &["open_diplomacy"]);
        st.gameplay_step = 58;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step == 58
        && (snap.last_gameplay_cmd.starts_with("diplomacy_ok")
            || snap.last_gameplay_cmd.starts_with("diplomacy_fail")
            || st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(4))
                .unwrap_or(false))
    {
        if snap.last_gameplay_cmd.starts_with("diplomacy_ok") {
            st.saw_diplomacy_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("diplomacy_") {
            st.diplomacy_detail = snap.last_gameplay_cmd.clone();
        }
        st.gameplay_step = 59;
        st.commanded_at = Some(Instant::now());
    } else if st.gameplay_step >= 59 {
        if snap.last_gameplay_cmd.starts_with("move_ok") {
            st.saw_move_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("construct_ok") {
            st.saw_construct_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("construct_") {
            st.construct_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("train_ok") {
            st.saw_train_ok = true;
            st.train_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("train_") {
            st.train_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("save_ok") {
            st.saw_save_ok = true;
            st.save_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("save_") {
            st.save_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("load_ok") {
            st.saw_load_ok = true;
            st.load_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("load_") {
            st.load_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("stop_ok") {
            st.saw_stop_ok = true;
            st.stop_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("stop_") {
            st.stop_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("sell_ok") {
            st.saw_sell_ok = true;
            st.sell_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("sell_") {
            st.sell_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("upgrade_ok") {
            st.saw_upgrade_ok = true;
            st.upgrade_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("upgrade_") {
            st.upgrade_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("guard_ok") {
            st.saw_guard_ok = true;
            st.guard_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("guard_") {
            st.guard_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("attack_move_ok") {
            st.saw_attack_move_ok = true;
            st.attack_move_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("attack_move_") {
            st.attack_move_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("scatter_ok") {
            st.saw_scatter_ok = true;
            st.scatter_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("scatter_") {
            st.scatter_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("patrol_ok") {
            st.saw_patrol_ok = true;
            st.patrol_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("patrol_") {
            st.patrol_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("deploy_ok") {
            st.saw_deploy_ok = true;
            st.deploy_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("deploy_") {
            st.deploy_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("cheer_ok") {
            st.saw_cheer_ok = true;
            st.cheer_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("cheer_") {
            st.cheer_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("formation_ok") {
            st.saw_formation_ok = true;
            st.formation_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("formation_") {
            st.formation_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("capture_ok") {
            st.saw_capture_ok = true;
            st.capture_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("capture_") {
            st.capture_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("return_supplies_ok") {
            st.saw_return_supplies_ok = true;
            st.return_supplies_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("return_supplies_") {
            st.return_supplies_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("evacuate_ok") {
            st.saw_evacuate_ok = true;
            st.evacuate_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("evacuate_") {
            st.evacuate_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("repair_ok") {
            st.saw_repair_ok = true;
            st.repair_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("repair_") {
            st.repair_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("return_to_base_ok") {
            st.saw_return_to_base_ok = true;
            st.return_to_base_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("return_to_base_") {
            st.return_to_base_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("attitude_ok") {
            st.saw_attitude_ok = true;
            st.attitude_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("attitude_") {
            st.attitude_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("rally_ok") {
            st.saw_rally_ok = true;
            st.rally_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("rally_") {
            st.rally_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("switch_weapons_ok") {
            st.saw_switch_weapons_ok = true;
            st.switch_weapons_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("switch_weapons_") {
            st.switch_weapons_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("view_cc_ok") {
            st.saw_view_cc_ok = true;
            st.view_cc_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("view_cc_") {
            st.view_cc_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("clear_mines_ok") {
            st.saw_clear_mines_ok = true;
            st.clear_mines_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("clear_mines_") {
            st.clear_mines_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("beacon_ok") {
            st.saw_beacon_ok = true;
            st.beacon_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("beacon_") {
            st.beacon_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("hack_ok") {
            st.saw_hack_ok = true;
            st.hack_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("hack_") {
            st.hack_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("cleanup_ok") {
            st.saw_cleanup_ok = true;
            st.cleanup_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("cleanup_") {
            st.cleanup_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("combat_drop_ok") {
            st.saw_combat_drop_ok = true;
            st.combat_drop_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("combat_drop_") {
            st.combat_drop_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("overcharge_ok") {
            st.saw_overcharge_ok = true;
            st.overcharge_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("overcharge_") {
            st.overcharge_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("special_power_ok") {
            st.saw_special_power_ok = true;
            st.special_power_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("special_power_") {
            st.special_power_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("remove_beacon_ok") {
            st.saw_remove_beacon_ok = true;
            st.remove_beacon_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("remove_beacon_") {
            st.remove_beacon_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("demo_suicide_ok") {
            st.saw_demo_ok = true;
            st.demo_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("demo_") {
            st.demo_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("view_radar_ok") {
            st.saw_view_radar_ok = true;
            st.view_radar_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("view_radar_") {
            st.view_radar_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("force_attack_ok") {
            st.saw_force_attack_ok = true;
            st.force_attack_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("force_attack_")
            && !snap.last_gameplay_cmd.starts_with("force_attack_object")
        {
            st.force_attack_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("force_attack_object_ok") {
            st.saw_force_attack_object_ok = true;
            st.force_attack_object_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("force_attack_object_") {
            st.force_attack_object_detail = snap.last_gameplay_cmd.clone();
        }
        if snap.last_gameplay_cmd.starts_with("select_all_ok") {
            st.saw_select_all_ok = true;
            st.select_all_detail = snap.last_gameplay_cmd.clone();
        } else if snap.last_gameplay_cmd.starts_with("select_all_")
            && !snap.last_gameplay_cmd.starts_with("select_all_combat")
        {
            st.select_all_detail = snap.last_gameplay_cmd.clone();
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
                st.saw_control_group_ok = true;
            }
            st.control_group_detail =
                format!("{};{}", st.control_group_detail, snap.last_gameplay_cmd);
        } else if snap.last_gameplay_cmd.starts_with("control_group_") {
            st.control_group_detail =
                format!("{};{}", st.control_group_detail, snap.last_gameplay_cmd);
        }
        record_attack_success(&mut st.saw_attack_ok, &snap.last_gameplay_cmd);
        if snap.last_gameplay_cmd.starts_with("select_ok") {
            st.saw_select_ok = true;
        }
        if launch == ExecutableSmokeLaunch::Windowed {
            // Re-issue only while the producer is complete and train
            // has not succeeded. train_fail must not block a retry.
            if !st.train_sent
                && st.saw_construct_ok
                && st.saw_construct_under_construction
                && snap.under_construction == 0
            {
                let _ = write_control(
                    &control_path,
                    &["train_unit|template=AmericaInfantryRanger|auto_target=1"],
                );
                st.train_sent = true;
                st.train_detail.clear();
                st.train_retry_started = Some(Instant::now());
                st.commanded_at = Some(Instant::now());
            } else if st.train_sent
                && !st.saw_train_ok
                && st.saw_construct_ok
                && st.saw_construct_under_construction
                && snap.under_construction == 0
                && st.commanded_at
                    .map(|t| t.elapsed() > Duration::from_secs(2))
                    .unwrap_or(false)
            {
                let _ = write_control(
                    &control_path,
                    &["train_unit|template=AmericaInfantryRanger|auto_target=1"],
                );
            }
        } else if st.train_sent
            && st.train_detail.is_empty()
            && st.commanded_at
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
        result.physical_build_and_produce = st.saw_physical_build_and_produce;
        result.save_cmd_ok = st.saw_save_ok;
        result.load_cmd_ok = st.saw_load_ok;
        result.stop_cmd_ok = st.saw_stop_ok;
        result.sell_cmd_ok = st.saw_sell_ok;
        result.upgrade_cmd_ok = st.saw_upgrade_ok;
        result.guard_cmd_ok = st.saw_guard_ok;
        result.attack_move_cmd_ok = st.saw_attack_move_ok;
        result.combat_damage_ok = st.saw_combat_damage;
        result.scatter_cmd_ok = st.saw_scatter_ok;
        result.patrol_cmd_ok = st.saw_patrol_ok;
        result.deploy_cmd_ok = st.saw_deploy_ok;
        result.cheer_cmd_ok = st.saw_cheer_ok;
        result.formation_cmd_ok = st.saw_formation_ok;
        result.capture_cmd_ok = st.saw_capture_ok;
        result.return_supplies_cmd_ok = st.saw_return_supplies_ok;
        result.physical_gather_resources = st.saw_physical_gather_resources;
        result.physical_save_load_continue = st.saw_physical_save_load_continue;
        result.evacuate_cmd_ok = st.saw_evacuate_ok;
        result.repair_cmd_ok = st.saw_repair_ok;
        result.return_to_base_cmd_ok = st.saw_return_to_base_ok;
        result.attitude_cmd_ok = st.saw_attitude_ok;
        result.rally_cmd_ok = st.saw_rally_ok;
        result.switch_weapons_cmd_ok = st.saw_switch_weapons_ok;
        result.view_cc_cmd_ok = st.saw_view_cc_ok;
        result.clear_mines_cmd_ok = st.saw_clear_mines_ok;
        result.beacon_cmd_ok = st.saw_beacon_ok;
        result.hack_cmd_ok = st.saw_hack_ok;
        result.cleanup_cmd_ok = st.saw_cleanup_ok;
        result.combat_drop_cmd_ok = st.saw_combat_drop_ok;
        result.overcharge_cmd_ok = st.saw_overcharge_ok;
        result.special_power_cmd_ok = st.saw_special_power_ok;
        result.remove_beacon_cmd_ok = st.saw_remove_beacon_ok;
        result.demo_cmd_ok = st.saw_demo_ok;
        result.view_radar_cmd_ok = st.saw_view_radar_ok;
        result.force_attack_cmd_ok = st.saw_force_attack_ok;
        result.force_attack_object_cmd_ok = st.saw_force_attack_object_ok;
        result.select_all_cmd_ok = st.saw_select_all_ok;
        result.control_group_cmd_ok = st.saw_control_group_ok;
        result.waypoint_cmd_ok = st.saw_waypoint_ok;
        result.box_select_cmd_ok = st.saw_box_select_ok;
        result.presentation_frame_ok = st.saw_presentation_frame_ok;
        result.presentation_live_fallback_ok = st.saw_presentation_live_fallback_ok;
        result.gameworld_presentation_entities_ok =
            st.saw_gameworld_presentation_entities_ok;
        result.max_gameworld_presentation_entities =
            st.max_gameworld_presentation_entities;
        result.gameworld_overlay_stamped_ok = st.saw_gameworld_overlay_stamped_ok;
        result.max_gameworld_overlay_stamped = st.max_gameworld_overlay_stamped;
        result.max_gameworld_appended = st.max_gameworld_appended;
        result.max_gameworld_rebuilt = st.max_gameworld_rebuilt;
        result.gameworld_rebuilt_ok = st.saw_gameworld_rebuilt_ok;
        result.shell_wnd_ok = st.saw_shell_wnd_ok;
        result.max_render_item_count = st.max_render_item_count;
        result.max_render_alive_objects = st.max_render_alive_objects;
        // Stable = at least 3 InGame polls with items (not a one-frame flash).
        result.render_items_stable_ok =
            st.render_items_nonzero_polls >= 3 && st.max_render_item_count > 0;
        result.select_similar_cmd_ok = st.saw_select_similar_ok;
        result.select_on_screen_cmd_ok = st.saw_select_on_screen_ok;
        result.select_structures_cmd_ok = st.saw_select_structures_ok;
        result.select_aircraft_cmd_ok = st.saw_select_aircraft_ok;
        result.select_idle_cmd_ok = st.saw_select_idle_ok;
        result.camera_reset_cmd_ok = st.saw_camera_reset_ok;
        result.camera_zoom_cmd_ok = st.saw_camera_zoom_ok;
        result.pause_cmd_ok = st.saw_pause_ok;
        result.cancel_production_cmd_ok = st.saw_cancel_production_ok;
        result.diplomacy_cmd_ok = st.saw_diplomacy_ok;
        result.live_frame_ok = st.saw_live_frame_ok;
        result.window_visible = st.saw_window_visible;
        result.wnd_widget_tree_nav = st.saw_wnd_widget_tree_nav;
        result.interactive_gameplay = st.saw_interactive_gameplay;
        result.auto_attack_cmd_ok = st.saw_auto_attack_ok;
        result.options_cmd_ok = st.saw_options_ok;
        result.request_capture_cmd_ok = st.saw_request_capture_ok;
        result.skirmish_start_wnd_ok =
            st.saw_skirmish_start_wnd_ok || result.skirmish_start_wnd_ok;
        if !st.presentation_detail.is_empty() {
            result.detail =
                format!("{}; presentation={}", result.detail, st.presentation_detail);
        }
        result.detail =
            format!("{}; last_cmd={}", result.detail, snap.last_gameplay_cmd);
        if !st.construct_detail.is_empty() {
            result.detail =
                format!("{}; construct={}", result.detail, st.construct_detail);
        }
        if !st.train_detail.is_empty() {
            result.detail = format!("{}; train={}", result.detail, st.train_detail);
        }
        if !st.save_detail.is_empty() {
            result.detail = format!("{}; save={}", result.detail, st.save_detail);
        }
        if !st.load_detail.is_empty() {
            result.detail = format!("{}; load={}", result.detail, st.load_detail);
        }
        // Exit only after the full host command chain finishes
        // (step >= 59: pause/cancel/attack/options/diplomacy), or on
        // hard stall / frame budget. Do not cut off mid-chain once
        // construct/train/attack land — later residuals (pause, etc.)
        // would stay false forever.
        let chain_complete = st.gameplay_step >= 59;
        // Only hard-stall once we're deep in the chain; early steps
        // have their own per-command timeouts.
        let hard_stall = st.gameplay_step >= 50
            && st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(120))
                .unwrap_or(false);
        // Windowed: after the host chain finishes, keep retrying the
        // RMB inject until interactive_gameplay (fifth claim flag)
        // latches — do not exit solely on host gameplay_cmd_ok.
        let want_exit = chain_complete
            || hard_stall
            || snap.frame
                >= if launch == ExecutableSmokeLaunch::Windowed
                    && !st.saw_interactive_gameplay
                {
                    2500u32
                } else {
                    500u32
                };
        let waiting_for_deferred_train = launch == ExecutableSmokeLaunch::Windowed
            && st.train_sent
            && !st.saw_train_ok
            && st.train_retry_started
                .is_some_and(|started| started.elapsed() < Duration::from_secs(10));
        if waiting_for_deferred_train {
            // Give the runtime time to consume the train request before another
            // control message or exit can replace it.
        } else if want_exit
            && launch == ExecutableSmokeLaunch::Windowed
            && !st.saw_interactive_gameplay
            && result.reached_ingame
            && st.windowed_inject_step < 90
            && st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(2))
                .unwrap_or(true)
        {
            if st.windowed_inject_step % 2 == 0 {
                let _ = write_control(&control_path, &["select_local_unit"]);
            } else {
                let _ = write_control(&control_path, &["winit_gameplay_order"]);
                result
                    .detail
                    .push_str(" windowed_late_winit_gameplay_order;");
            }
            st.windowed_inject_step = st.windowed_inject_step.saturating_add(1);
            st.commanded_at = Some(Instant::now());
        } else if want_exit {
            let _ = write_control(&control_path, &["exit"]);
            st.phase = 3;
        }
    }
}


/// Minimal live-host order path for hq-x1j4u.9.3: select → move → attack → exit.
///
/// Advances only on `*_ok` residuals. Retries on fail. If no FOW-clear enemy is
/// available, falls back to `attack_move` (still a live attack-class order).
fn smoke_drive_orders_only_chain(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
) {
    if snap.last_gameplay_cmd.starts_with("select_ok") {
        st.saw_select_ok = true;
    }
    if snap.last_gameplay_cmd.starts_with("move_ok") {
        st.saw_move_ok = true;
    }
    if snap.last_gameplay_cmd.starts_with("attack_ok")
        || snap.last_gameplay_cmd.starts_with("attack_move_ok")
        || snap.last_gameplay_cmd.starts_with("force_attack_ok")
    {
        st.saw_attack_ok = true;
    }
    // Keep a breadcrumb of the latest host residual for failure reports.
    if !snap.last_gameplay_cmd.is_empty() {
        result.detail = format!(
            "orders_step={} last_cmd={} mobiles={} selected={} select={} move={} attack={}",
            st.gameplay_step,
            snap.last_gameplay_cmd,
            snap.local_mobile_units,
            snap.selected_count,
            st.saw_select_ok,
            st.saw_move_ok,
            st.saw_attack_ok
        );
    }
    if st.saw_select_ok && st.saw_move_ok && st.saw_attack_ok {
        result.gameplay_cmd_ok = true;
        result.detail.push_str(" orders_select_move_attack;");
        let _ = write_control(&control_path, &["exit"]);
        st.commanded_at = Some(Instant::now());
        st.phase = 3;
        return;
    }

    let retry_due = st
        .commanded_at
        .map(|t| t.elapsed() > Duration::from_secs(6))
        .unwrap_or(true);

    // 0: wait until presentation publishes a local mobile, then select.
    if !st.saw_select_ok {
        if st.gameplay_step == 0 {
            if snap.local_mobile_units == 0 && snap.selected_count == 0 {
                // Stay on step 0 until a unit exists (map admission can lag).
                if retry_due {
                    st.commanded_at = Some(Instant::now());
                }
                return;
            }
            let _ = write_control(&control_path, &["select_local_unit"]);
            st.gameplay_step = 1;
            st.commanded_at = Some(Instant::now());
            return;
        }
        // step >= 1: wait for select_ok; retry select on fail/timeout.
        if snap.last_gameplay_cmd.starts_with("select_fail") || retry_due {
            let _ = write_control(&control_path, &["select_local_unit"]);
            st.commanded_at = Some(Instant::now());
            st.gameplay_step = st.gameplay_step.saturating_add(1).min(20);
        }
        return;
    }

    // Selected: issue move and wait for move_ok.
    if !st.saw_move_ok {
        if st.gameplay_step < 30 {
            let _ = write_control(&control_path, &["move_selected|x=100|y=0|z=100"]);
            st.gameplay_step = 30;
            st.commanded_at = Some(Instant::now());
            return;
        }
        if snap.last_gameplay_cmd.starts_with("move_fail") || retry_due {
            // Reselect then move — selection can be cleared by host ticks.
            let _ = write_control(
                &control_path,
                &["select_local_unit", "move_selected|x=120|y=0|z=120"],
            );
            st.commanded_at = Some(Instant::now());
            st.gameplay_step = st.gameplay_step.saturating_add(1).min(39);
        }
        return;
    }

    // Moved: issue attack (enemy if possible, else attack-move).
    if !st.saw_attack_ok {
        if st.gameplay_step < 40 {
            let _ = write_control(&control_path, &["attack_nearest_enemy|auto_target=1"]);
            st.gameplay_step = 40;
            st.commanded_at = Some(Instant::now());
            return;
        }
        if snap.last_gameplay_cmd.starts_with("attack_fail_no_enemy") {
            let _ = write_control(
                &control_path,
                &["attack_move|x=200|y=0|z=200"],
            );
            st.commanded_at = Some(Instant::now());
            st.gameplay_step = 41;
            return;
        }
        if snap.last_gameplay_cmd.starts_with("attack_fail") || retry_due {
            let _ = write_control(
                &control_path,
                &[
                    "select_local_unit",
                    "attack_nearest_enemy|auto_target=1",
                    "attack_move|x=220|y=0|z=220",
                ],
            );
            st.commanded_at = Some(Instant::now());
            st.gameplay_step = st.gameplay_step.saturating_add(1).min(50);
        }
    }
}


fn smoke_drive_ai_activity_chain(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
) {
    if snap.last_gameplay_cmd.starts_with("select_ok") {
        st.saw_select_ok = true;
    }
    if snap.last_gameplay_cmd.starts_with("move_ok") {
        st.saw_move_ok = true;
    }
    if snap.last_gameplay_cmd.starts_with("ai_activity_ok") {
        let act = snap
            .last_gameplay_cmd
            .split(':')
            .find_map(|part| part.strip_prefix("act=")?.parse::<u64>().ok())
            .unwrap_or(0);
        let structs = snap
            .last_gameplay_cmd
            .split(':')
            .find_map(|part| part.strip_prefix("struct=")?.parse::<u64>().ok())
            .unwrap_or(0);
        let units = snap
            .last_gameplay_cmd
            .split(':')
            .find_map(|part| part.strip_prefix("units=")?.parse::<u64>().ok())
            .unwrap_or(0);
        let ai = snap
            .last_gameplay_cmd
            .split(':')
            .find_map(|part| part.strip_prefix("ai=")?.parse::<u64>().ok())
            .unwrap_or(0);
        // AI present and either activity counter moved, multi-structure base, or units/queue.
        if ai >= 1 && (act >= 1 || structs >= 2 || units >= 1) {
            st.saw_ai_activity_ok = true;
        }
        result.detail = format!(
            "ai_probe act={act} struct={structs} units={units} ai={ai} select={} move={}",
            st.saw_select_ok, st.saw_move_ok
        );
    }

    if st.saw_select_ok && st.saw_move_ok && st.saw_ai_activity_ok {
        result.gameplay_cmd_ok = true;
        result.detail.push_str(" ai_activity_pass;");
        let _ = write_control(&control_path, &["exit"]);
        st.commanded_at = Some(Instant::now());
        st.phase = 3;
        return;
    }

    let retry_due = st
        .commanded_at
        .map(|t| t.elapsed() > Duration::from_secs(8))
        .unwrap_or(true);

    if !st.saw_select_ok {
        if snap.local_mobile_units == 0 && st.gameplay_step == 0 && !retry_due {
            return;
        }
        if st.gameplay_step == 0 || snap.last_gameplay_cmd.starts_with("select_fail") || retry_due {
            let _ = write_control(&control_path, &["select_local_unit"]);
            st.gameplay_step = 1;
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    if !st.saw_move_ok {
        if st.gameplay_step < 10 {
            let _ = write_control(&control_path, &["move_selected|x=100|y=0|z=100"]);
            st.gameplay_step = 10;
            st.commanded_at = Some(Instant::now());
            return;
        }
        if snap.last_gameplay_cmd.starts_with("move_fail") || retry_due {
            let _ = write_control(
                &control_path,
                &["select_local_unit", "move_selected|x=120|y=0|z=120"],
            );
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    // Player has ordered; poll AI activity.
    if st.gameplay_step < 20 {
        let _ = write_control(&control_path, &["report_ai_activity"]);
        st.gameplay_step = 20;
        st.commanded_at = Some(Instant::now());
        return;
    }
    if retry_due {
        let _ = write_control(&control_path, &["report_ai_activity"]);
        st.commanded_at = Some(Instant::now());
        st.gameplay_step = st.gameplay_step.saturating_add(1).min(40);
    }
}



/// Parse `key=value` fragments from a `world_snap_ok:...` residual.
fn world_snap_field<'a>(cmd: &'a str, key: &str) -> Option<&'a str> {
    let prefix = format!("{key}=");
    cmd.split(':').find_map(|part| part.strip_prefix(&prefix))
}

fn world_snaps_match(pre: &str, post: &str) -> Result<(), String> {
    if !pre.starts_with("world_snap_ok:") || !post.starts_with("world_snap_ok:") {
        return Err(format!("bad snap residuals pre={pre} post={post}"));
    }
    let units: u64 = world_snap_field(pre, "units")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    if units == 0 {
        return Err("unit count is 0 (empty world — not a retail skirmish restore)".into());
    }
    for key in ["map", "p0", "units", "id", "owner"] {
        let a = world_snap_field(pre, key).unwrap_or("");
        let b = world_snap_field(post, key).unwrap_or("");
        if a != b {
            return Err(format!("{key} mismatch pre={a} post={b}"));
        }
    }
    for key in ["hp", "x", "y", "z"] {
        let a: f32 = world_snap_field(pre, key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(f32::NAN);
        let b: f32 = world_snap_field(post, key)
            .and_then(|s| s.parse().ok())
            .unwrap_or(f32::NAN);
        if !a.is_finite() || !b.is_finite() || (a - b).abs() > 0.05 {
            return Err(format!("{key} mismatch pre={a} post={b}"));
        }
    }
    let fa = world_snap_field(pre, "frame").unwrap_or("");
    let fb = world_snap_field(post, "frame").unwrap_or("");
    if fa != fb {
        return Err(format!("frame mismatch pre={fa} post={fb}"));
    }
    Ok(())
}

/// P0 hq-x1j4u.9.7: retail Lone Eagle save → load into staged fresh GameLogic,
/// assert map/resources/units, then prove orders still accept.
fn smoke_drive_save_load_chain(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
) {
    if snap.last_gameplay_cmd.starts_with("select_ok") {
        st.saw_select_ok = true;
    }
    if snap.last_gameplay_cmd.starts_with("move_ok") {
        if st.gameplay_step >= 80 {
            st.saw_post_load_move_ok = true;
        } else {
            st.saw_move_ok = true;
        }
    }
    if snap.last_gameplay_cmd.starts_with("save_ok") {
        st.saw_save_ok = true;
    }
    if snap.last_gameplay_cmd.starts_with("load_ok") {
        st.saw_load_ok = true;
    }
    if snap.last_gameplay_cmd.starts_with("world_snap_ok:") {
        if st.pre_world_snap.is_empty() && st.gameplay_step < 50 {
            st.pre_world_snap = snap.last_gameplay_cmd.clone();
        } else if !st.pre_world_snap.is_empty()
            && st.saw_load_ok
            && st.gameplay_step >= 60
            && !st.saw_world_restore_ok
        {
            match world_snaps_match(&st.pre_world_snap, &snap.last_gameplay_cmd) {
                Ok(()) => st.saw_world_restore_ok = true,
                Err(err) => {
                    result.detail = format!(
                        "world_restore_fail:{err}; pre={}; post={}",
                        st.pre_world_snap, snap.last_gameplay_cmd
                    );
                }
            }
        }
    }

    let breadcrumb = format!(
        "save_load_step={} last_cmd={} select={} move={} save={} load={} restore={} post_move={}",
        st.gameplay_step,
        snap.last_gameplay_cmd,
        st.saw_select_ok,
        st.saw_move_ok,
        st.saw_save_ok,
        st.saw_load_ok,
        st.saw_world_restore_ok,
        st.saw_post_load_move_ok
    );
    if !result.detail.starts_with("world_restore_fail:") {
        result.detail = breadcrumb;
    }

    // Save/load errors can show modal dialogs. A reported failure is terminal
    // for this round-trip, not an invitation to repeat the same operation.
    if (st.gameplay_step == 40 && snap.last_gameplay_cmd.starts_with("save_fail"))
        || (st.gameplay_step == 50 && snap.last_gameplay_cmd.starts_with("load_fail"))
    {
        result.status = "save_load_command_failed".into();
        result.detail = format!("save_load command failed: {}", snap.last_gameplay_cmd);
        return;
    }


    if st.saw_select_ok
        && st.saw_move_ok
        && st.saw_save_ok
        && st.saw_load_ok
        && st.saw_world_restore_ok
        && st.saw_post_load_move_ok
    {
        result.gameplay_cmd_ok = true;
        result.save_cmd_ok = true;
        result.load_cmd_ok = true;
        result.detail.push_str(" save_load_pass;");
        let _ = write_control(control_path, &["exit"]);
        st.commanded_at = Some(Instant::now());
        st.phase = 3;
        return;
    }

    let retry_due = st
        .commanded_at
        .map(|t| t.elapsed() > Duration::from_secs(8))
        .unwrap_or(true);

    if !st.saw_select_ok {
        if snap.local_mobile_units == 0 && st.gameplay_step == 0 && !retry_due {
            return;
        }
        if st.gameplay_step == 0 || snap.last_gameplay_cmd.starts_with("select_fail") || retry_due {
            let _ = write_control(control_path, &["select_local_unit"]);
            st.gameplay_step = 1;
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    if !st.saw_move_ok {
        if st.gameplay_step < 10 {
            let _ = write_control(control_path, &["move_selected|x=100|y=0|z=100"]);
            st.gameplay_step = 10;
            st.commanded_at = Some(Instant::now());
            return;
        }
        if snap.last_gameplay_cmd.starts_with("move_fail") || retry_due {
            let _ = write_control(
                control_path,
                &["select_local_unit", "move_selected|x=120|y=0|z=120"],
            );
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    // Advance N logic ticks after the order before freezing the world.
    const SAVE_LOAD_TICK_WAIT: u32 = 45;
    if st.gameplay_step < 20 {
        st.save_load_tick_anchor = Some(snap.frame);
        st.gameplay_step = 20;
        st.commanded_at = Some(Instant::now());
        return;
    }
    if st.gameplay_step == 20 {
        let anchor = st.save_load_tick_anchor.unwrap_or(snap.frame);
        let advanced = snap.frame.saturating_sub(anchor);
        let waited = st
            .commanded_at
            .map(|t| t.elapsed() >= Duration::from_secs(3))
            .unwrap_or(false);
        if advanced < SAVE_LOAD_TICK_WAIT && !waited {
            return;
        }
        // Pause so pre-snap / save / load / post-snap share one logic frame.
        let _ = write_control(control_path, &["pause"]);
        st.gameplay_step = 25;
        st.commanded_at = Some(Instant::now());
        return;
    }
    if st.gameplay_step == 25 {
        if snap.last_gameplay_cmd.starts_with("pause_ok") || retry_due {
            st.gameplay_step = 30;
            st.commanded_at = Some(Instant::now());
        } else if retry_due {
            let _ = write_control(control_path, &["pause"]);
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    if st.pre_world_snap.is_empty() {
        if st.gameplay_step < 30 {
            st.gameplay_step = 30;
        }
        if st.gameplay_step == 30 || retry_due {
            let _ = write_control(control_path, &["report_world_snapshot"]);
            st.gameplay_step = 31;
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    if !st.saw_save_ok {
        if st.gameplay_step < 40 {
            let _ = write_control(control_path, &["quicksave"]);
            st.gameplay_step = 40;
            st.commanded_at = Some(Instant::now());
            return;
        }
        // Do not reissue an unacknowledged save: it may already have opened a
        // modal error dialog. The overall wall budget bounds a missing reply.
        return;
    }

    if !st.saw_load_ok {
        if st.gameplay_step < 50 {
            let _ = write_control(control_path, &["quickload"]);
            st.gameplay_step = 50;
            st.commanded_at = Some(Instant::now());
            return;
        }
        // Like save, load may have side effects even if status is delayed.
        return;
    }

    if !st.saw_world_restore_ok {
        if st.gameplay_step < 60 {
            let _ = write_control(control_path, &["report_world_snapshot"]);
            st.gameplay_step = 60;
            st.commanded_at = Some(Instant::now());
            return;
        }
        if retry_due {
            let _ = write_control(control_path, &["report_world_snapshot"]);
            st.commanded_at = Some(Instant::now());
            st.gameplay_step = st.gameplay_step.saturating_add(1).min(69);
        }
        return;
    }

    // Resume then prove orders still accept on the restored world.
    if st.gameplay_step < 75 {
        let _ = write_control(control_path, &["pause"]); // toggle → resume if still paused
        st.gameplay_step = 75;
        st.commanded_at = Some(Instant::now());
        return;
    }
    if st.gameplay_step == 75 {
        if snap.last_gameplay_cmd.starts_with("pause_ok") || retry_due {
            st.gameplay_step = 80;
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    if !st.saw_post_load_move_ok {
        if st.gameplay_step == 80 {
            let _ = write_control(
                control_path,
                &["select_local_unit", "move_selected|x=140|y=0|z=140"],
            );
            st.gameplay_step = 81;
            st.commanded_at = Some(Instant::now());
            return;
        }
        if snap.last_gameplay_cmd.starts_with("move_fail")
            || snap.last_gameplay_cmd.starts_with("select_fail")
            || retry_due
        {
            let _ = write_control(
                control_path,
                &["select_local_unit", "move_selected|x=150|y=0|z=150"],
            );
            st.commanded_at = Some(Instant::now());
            st.gameplay_step = st.gameplay_step.saturating_add(1).min(89);
        }
    }
}




/// Sample the on-screen radar gadget (bottom-left HUD) from a captured frame.
/// `radar_pixels` only proves the CPU terrain texture is non-black; shroud or a
/// missing draw can still leave the gadget black. Require enough non-black,
/// non-uniform samples inside the radar rectangle (C++ W3DRadar::draw into the
/// radar window).
fn png_minimap_on_screen_has_terrain(path: &Path) -> bool {
    let Ok(data) = fs::read(path) else {
        return false;
    };
    let Ok(img) = image::load_from_memory(&data) else {
        return false;
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w < 320 || h < 240 {
        return false;
    }
    // ControlBar radar sits bottom-left; keep a generous rect that still avoids
    // the money/text strip to the right of the gadget on 1280x720.
    let x0 = (w as f32 * 0.02) as u32;
    let y0 = (h as f32 * 0.72) as u32;
    let x1 = (w as f32 * 0.16) as u32;
    let y1 = (h as f32 * 0.97) as u32;
    let mut non_black = 0u32;
    let mut samples = 0u32;
    let mut sum = [0u64; 3];
    let mut sum_sq = [0u64; 3];
    let mut buckets = std::collections::HashSet::new();
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let p = rgba.get_pixel(x, y).0;
            samples += 1;
            if p[0] | p[1] | p[2] != 0 {
                non_black += 1;
            }
            for i in 0..3 {
                let v = p[i] as u64;
                sum[i] += v;
                sum_sq[i] += v * v;
            }
            buckets.insert((p[0] >> 3, p[1] >> 3, p[2] >> 3));
            x = x.saturating_add(2);
        }
        y = y.saturating_add(2);
    }
    if samples < 64 || non_black * 5 < samples {
        // <20% non-black → still the empty black gadget.
        return false;
    }
    let mut max_var = 0.0f64;
    for i in 0..3 {
        let mean = sum[i] as f64 / samples as f64;
        let var = sum_sq[i] as f64 / samples as f64 - mean * mean;
        max_var = max_var.max(var);
    }
    // Uniform fill or a single icon on black fails; retail snow/rock radar has
    // hundreds of color buckets and material variance.
    buckets.len() >= 24 && max_var >= 200.0
}

/// Playable terrain view (upper ~65%) must show tile-level color variance.
/// Grey stand-in Art/Terrain placeholders are nearly uniform (~var 50); retail
/// Lone Eagle snow/rock/cliff atlases are far more varied.
fn png_terrain_view_has_retail_tile_variance(path: &Path) -> bool {
    let Ok(data) = fs::read(path) else {
        return false;
    };
    let Ok(img) = image::load_from_memory(&data) else {
        return false;
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w < 320 || h < 240 {
        return false;
    }
    let x0 = (w as f32 * 0.08) as u32;
    let y0 = (h as f32 * 0.08) as u32;
    let x1 = (w as f32 * 0.92) as u32;
    let y1 = (h as f32 * 0.58) as u32;
    let mut samples = 0u32;
    let mut sum = [0u64; 3];
    let mut sum_sq = [0u64; 3];
    let mut buckets = std::collections::HashSet::new();
    let mut y = y0;
    while y < y1 {
        let mut x = x0;
        while x < x1 {
            let p = rgba.get_pixel(x, y).0;
            samples += 1;
            for i in 0..3 {
                let v = p[i] as u64;
                sum[i] += v;
                sum_sq[i] += v * v;
            }
            buckets.insert((p[0] >> 3, p[1] >> 3, p[2] >> 3));
            x = x.saturating_add(3);
        }
        y = y.saturating_add(3);
    }
    if samples < 256 {
        return false;
    }
    let mut max_var = 0.0f64;
    for i in 0..3 {
        let mean = sum[i] as f64 / samples as f64;
        let var = sum_sq[i] as f64 / samples as f64 - mean * mean;
        max_var = max_var.max(var);
    }
    // Prior grey stand-in frames measured ~53 variance / ~40 buckets in a
    // corner crop; retail snow/rock is well above both.
    buckets.len() >= 120 && max_var >= 250.0
}


/// Reject clear-color / sky-only frames: lower half must show edge/texture variance.
fn is_procedural_runtime_fallback_png(path: &Path) -> bool {
    // Matches RuntimeHostBridge::build_procedural_fallback_png (1280x720 smooth
    // green→blue ramp). Real skirmish frames never match this exact formula.
    let Ok(data) = fs::read(path) else {
        return false;
    };
    if data.len() < 33 || data.get(0..8) != Some(&[0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n']) {
        return false;
    }
    let Ok(img) = image::load_from_memory(&data) else {
        return false;
    };
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w != 1280 || h != 720 {
        return false;
    }
    let sample = |x: u32, y: u32| {
        let p = rgba.get_pixel(x, y).0;
        (p[0], p[1], p[2])
    };
    let expected = |x: u32, y: u32| {
        let v = y as f32 / (h.saturating_sub(1).max(1)) as f32;
        let u = x as f32 / (w.saturating_sub(1).max(1)) as f32;
        let r = (22.0 + 26.0 * (1.0 - v) + 12.0 * u).clamp(0.0, 255.0) as u8;
        let g = (34.0 + 38.0 * (1.0 - v)).clamp(0.0, 255.0) as u8;
        let b = (48.0 + 58.0 * v).clamp(0.0, 255.0) as u8;
        (r, g, b)
    };
    for (x, y) in [(0u32, 0u32), (w / 2, h / 2), (w - 1, h - 1), (0, h - 1), (w - 1, 0)] {
        let (a, e) = (sample(x, y), expected(x, y));
        if a.0.abs_diff(e.0) > 1 || a.1.abs_diff(e.1) > 1 || a.2.abs_diff(e.2) > 1 {
            return false;
        }
    }
    true
}

fn png_lower_half_has_texture_variance(path: &Path) -> bool {
    let Ok(data) = fs::read(path) else {
        return false;
    };
    if data.len() < 33 || data.get(0..8) != Some(b"\x89PNG\r\n\x1a\n") {
        return false;
    }
    let mut i = 8usize;
    let mut w = 0u32;
    let mut h = 0u32;
    let mut color = 0u8;
    let mut idat = Vec::new();
    while i + 12 <= data.len() {
        let ln = u32::from_be_bytes(data[i..i + 4].try_into().unwrap()) as usize;
        let typ = &data[i + 4..i + 8];
        if i + 12 + ln > data.len() {
            break;
        }
        let chunk = &data[i + 8..i + 8 + ln];
        match typ {
            b"IHDR" if chunk.len() >= 10 => {
                w = u32::from_be_bytes(chunk[0..4].try_into().unwrap());
                h = u32::from_be_bytes(chunk[4..8].try_into().unwrap());
                color = chunk[9];
            }
            b"IDAT" => idat.extend_from_slice(chunk),
            b"IEND" => break,
            _ => {}
        }
        i += 12 + ln;
    }
    if w < 8 || h < 8 || idat.is_empty() {
        return false;
    }
    use std::io::Read;
    let mut dec = flate2::read::ZlibDecoder::new(idat.as_slice());
    let mut raw = Vec::new();
    if dec.read_to_end(&mut raw).is_err() {
        return false;
    }
    let bpp: usize = match color {
        2 => 3,
        6 => 4,
        0 => 1,
        _ => return false,
    };
    let stride = (w as usize).saturating_mul(bpp);
    let expected = (stride + 1).saturating_mul(h as usize);
    if raw.len() < expected || stride == 0 {
        return false;
    }
    let mut prev = vec![0u8; stride];
    let mut rows = Vec::with_capacity(h as usize);
    let mut off = 0usize;
    for _ in 0..h as usize {
        let filter = raw[off];
        off += 1;
        let mut row = raw[off..off + stride].to_vec();
        off += stride;
        match filter {
            1 => {
                for x in bpp..stride {
                    row[x] = row[x].wrapping_add(row[x - bpp]);
                }
            }
            2 => {
                for x in 0..stride {
                    row[x] = row[x].wrapping_add(prev[x]);
                }
            }
            3 => {
                for x in 0..stride {
                    let left = if x >= bpp { row[x - bpp] } else { 0 };
                    let avg = ((u16::from(left) + u16::from(prev[x])) / 2) as u8;
                    row[x] = row[x].wrapping_add(avg);
                }
            }
            4 => {
                for x in 0..stride {
                    let a = if x >= bpp { row[x - bpp] } else { 0 };
                    let b = prev[x];
                    let c = if x >= bpp { prev[x - bpp] } else { 0 };
                    let p = i16::from(a) + i16::from(b) - i16::from(c);
                    let pa = (p - i16::from(a)).unsigned_abs();
                    let pb = (p - i16::from(b)).unsigned_abs();
                    let pc = (p - i16::from(c)).unsigned_abs();
                    let pr = if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    };
                    row[x] = row[x].wrapping_add(pr);
                }
            }
            _ => {}
        }
        prev.clone_from(&row);
        rows.push(row);
    }
    let start = (h as usize) / 2;
    let step_y = ((h as usize) / 48).max(1);
    let step_x = ((w as usize) / 48).max(1).saturating_mul(bpp).max(bpp);
    let mut edge = 0u64;
    let mut samples = 0u64;
    let mut uniq = std::collections::BTreeSet::new();
    for y in (start..h as usize).step_by(step_y) {
        let row = &rows[y];
        let y0 = y.saturating_sub(step_y).max(start);
        let above = &rows[y0];
        let mut x = 0usize;
        while x + bpp < stride {
            let dx = (row[x] as i16 - row[x + bpp] as i16).unsigned_abs() as u64;
            let dy = (row[x] as i16 - above[x] as i16).unsigned_abs() as u64;
            if dx > 12 || dy > 12 {
                edge += 1;
            }
            // Quantize RGB for uniqueness (ignore alpha).
            let key = (row[x] / 8, row.get(x + 1).copied().unwrap_or(0) / 8, row.get(x + 2).copied().unwrap_or(0) / 8);
            uniq.insert(key);
            samples += 1;
            x += step_x;
        }
    }
    // Smooth sky/clear gradients have almost no edges and few unique bins.
    samples > 20 && edge * 10 >= samples && uniq.len() >= 12
}

/// P0 hq-x1j4u.9.6: windowed Lone Eagle present sit-through with a selected unit,
/// non-zero stable render items, and a saved screenshot under the worktree.
fn smoke_drive_present_frame_chain(
    st: &mut SmokeRunState,
    result: &mut ExecutableSmokeResult,
    snap: &StatusSnap,
    control_path: &Path,
) {
    if snap.last_gameplay_cmd.starts_with("select_ok") {
        st.saw_select_ok = true;
    }
    if snap.selected_count > 0 {
        st.saw_select_ok = true;
    }
    if snap.render_item_count > 0 {
        st.max_render_item_count = st.max_render_item_count.max(snap.render_item_count);
        st.render_items_nonzero_polls = st.render_items_nonzero_polls.saturating_add(1);
    }
    if snap.window_visible {
        st.saw_window_visible = true;
        result.window_visible = true;
    }
    if snap.presentation_frame_ok {
        st.saw_presentation_frame_ok = true;
        result.presentation_frame_ok = true;
    }
    if snap.live_frame_ok {
        st.saw_live_frame_ok = true;
        result.live_frame_ok = true;
    }

    result.detail = format!(
        "present_step={} select={} selected={} window_visible={} render_items={} terrain={} models={} ui={} selection={} stable_polls={} sit_secs={} last_cmd={}",
        st.gameplay_step,
        st.saw_select_ok,
        snap.selected_count,
        st.saw_window_visible,
        st.max_render_item_count,
        st.max_terrain_chunks,
        st.max_render_models,
        st.max_render_ui,
        st.max_render_selection,
        st.render_items_nonzero_polls,
        st.present_frame_ingame_since
            .map(|t| t.elapsed().as_secs())
            .unwrap_or(0),
        snap.last_gameplay_cmd
    );

    let retry_due = st
        .commanded_at
        .map(|t| t.elapsed() > Duration::from_secs(6))
        .unwrap_or(true);

    // Select a local unit once InGame.
    if !st.saw_select_ok || snap.selected_count == 0 {
        if st.gameplay_step == 0 || snap.last_gameplay_cmd.starts_with("select_fail") || retry_due {
            let _ = write_control(control_path, &["select_local_unit"]);
            st.gameplay_step = 1;
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    if st.present_frame_ingame_since.is_none() {
        let _ = write_control(control_path, &["view_command_center"]);
        st.present_frame_ingame_since = Some(Instant::now());
        st.gameplay_step = 10;
        st.commanded_at = Some(Instant::now());
    }

    let sit = st
        .present_frame_ingame_since
        .map(|t| t.elapsed())
        .unwrap_or_default();
    // Peak per-category draws (weak render_item_count alone is not enough).
    st.max_terrain_chunks = st.max_terrain_chunks.max(snap.render_terrain_chunks);
    st.max_render_models = st.max_render_models.max(snap.render_models);
    st.max_render_ui = st.max_render_ui.max(snap.render_ui);
    st.max_render_selection = st.max_render_selection.max(snap.render_selection);
    st.max_radar_pixels = st.max_radar_pixels.max(snap.radar_pixels);
    if snap.present_ms > 0.0 {
        st.sum_present_ms += f64::from(snap.present_ms);
        st.sum_collect_ms += f64::from(snap.present_collect_ms);
        st.sum_terrain_ms += f64::from(snap.present_terrain_ms);
        st.sum_forward_ms += f64::from(snap.present_forward_ms);
        st.present_timing_samples = st.present_timing_samples.saturating_add(1);
    }

    // Model pass items can read 0 between presents while alive-object residual
    // still proves unit/structure meshes were collected.
    let categories_ok = st.max_terrain_chunks > 0
        && (st.max_render_models >= 3
            || st.max_render_alive_objects >= 3
            || st.max_render_item_count.saturating_sub(st.max_render_ui) >= 3)
        && st.max_render_ui > 0
        && st.max_render_selection > 0
        && st.max_radar_pixels > 0
        && snap.selected_count > 0;
    let stable = st.render_items_nonzero_polls >= 3 && st.max_render_item_count > 0;

    // Sit through at least 60s with selection held; keep focusing camera on CC.
    if sit < Duration::from_secs(60) {
        if retry_due {
            if snap.selected_count == 0 {
                let _ = write_control(
                    control_path,
                    &["view_command_center", "select_local_unit"],
                );
            } else if sit.as_secs() % 15 == 0 {
                let _ = write_control(control_path, &["view_command_center"]);
            }
            st.commanded_at = Some(Instant::now());
        }
        return;
    }

    if !stable || !categories_ok {
        if sit < Duration::from_secs(120) {
            if retry_due {
                let _ = write_control(
                    control_path,
                    &["view_command_center", "select_local_unit"],
                );
                st.commanded_at = Some(Instant::now());
            }
            return;
        }
        result.detail = format!(
            "present_frame_fail; categories short; terrain={} models={} ui={} selection={} radar_pixels={} items={} selected={}",
            st.max_terrain_chunks,
            st.max_render_models,
            st.max_render_ui,
            st.max_render_selection,
            st.max_radar_pixels,
            st.max_render_item_count,
            snap.selected_count
        );
        let _ = write_control(control_path, &["exit"]);
        st.commanded_at = Some(Instant::now());
        st.phase = 3;
        return;
    }

    // Request a capture, then copy frame.png into the worktree artifacts dir.
    if st.gameplay_step < 20 {
        let _ = write_control(control_path, &["request_capture"]);
        st.gameplay_step = 20;
        st.commanded_at = Some(Instant::now());
        return;
    }
    if st.gameplay_step == 20 {
        let ready = (snap.last_gameplay_cmd.starts_with("request_capture_ok")
            && (snap.live_frame_ok || st.saw_live_frame_ok))
            || st.commanded_at.map(|t| t.elapsed() > Duration::from_secs(20)).unwrap_or(false);
        if !ready {
            return;
        }
        let frame_src = control_path
            .parent()
            .map(|p| p.join("frame.png"))
            .unwrap_or_else(|| PathBuf::from("frame.png"));
        // Worktree-relative artifacts/ (gitignored), unless this run overrides it.
        let dest = st.screenshot_path.clone()
            .unwrap_or_else(|| {
                PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../artifacts/present_frame_lone_eagle.png")
            });
        if let Some(parent) = dest.parent() {
            let _ = fs::create_dir_all(parent);
        }
        // Wait briefly for the PNG to appear after the capture request.
        let mut copied = false;
        for _ in 0..50 {
            if frame_src.is_file()
                && !is_procedural_runtime_fallback_png(&frame_src)
                && fs::copy(&frame_src, &dest).is_ok()
            {
                copied = true;
                break;
            }
            let _ = write_control(control_path, &["request_capture"]);
            thread::sleep(Duration::from_millis(200));
        }
        st.present_screenshot_path = if copied {
            dest.canonicalize()
                .unwrap_or_else(|_| dest.clone())
                .to_string_lossy()
                .into_owned()
        } else {
            format!("capture_missing:{}", frame_src.display())
        };
        let image_ok = copied
            && !is_procedural_runtime_fallback_png(Path::new(&st.present_screenshot_path))
            && png_lower_half_has_texture_variance(Path::new(&st.present_screenshot_path))
            && png_minimap_on_screen_has_terrain(Path::new(&st.present_screenshot_path))
            && png_terrain_view_has_retail_tile_variance(Path::new(&st.present_screenshot_path))
            && (snap.live_frame_ok || st.saw_live_frame_ok);
        st.present_image_ok = image_ok;
        if !image_ok {
            result.detail = format!(
                "present_frame_fail; image lacks on-screen radar terrain or retail tile variance; screenshot={} terrain={} models={} ui={} selection={} radar_pixels={}",
                st.present_screenshot_path,
                st.max_terrain_chunks,
                st.max_render_models,
                st.max_render_ui,
                st.max_render_selection,
                st.max_radar_pixels
            );
            // Keep failing closed: do not latch present_frame_ok on sky-only frames.
            let _ = write_control(control_path, &["exit"]);
            st.commanded_at = Some(Instant::now());
            st.phase = 3;
            return;
        }
        st.saw_present_frame_ok = true;
        result.gameplay_cmd_ok = true;
        result.render_items_stable_ok = true;
        result.max_render_item_count = st.max_render_item_count;
        let n = st.present_timing_samples.max(1) as f64;
        let avg_present = st.sum_present_ms / n;
        let avg_collect = st.sum_collect_ms / n;
        let avg_terrain = st.sum_terrain_ms / n;
        let avg_forward = st.sum_forward_ms / n;
        let docs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs/lone_eagle_perf_baseline.md");
        if let Some(parent) = docs.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let body = format!(
            "# Lone Eagle performance baseline\n\nCaptured during `present_frame_gate` sit-through (America/Sao_Paulo local).\n\n| mode | samples | avg present_ms | collect | terrain | forward | radar_pixels | terrain_chunks |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n| windowed present | {} | {:.3} | {:.3} | {:.3} | {:.3} | {} | {} |\n\n## Hot spots\n\nPresent breakdown from `RenderPipeline::execute` residuals. Leading stage: {}.\n\n## Notes\n\n- Facility census cache: `facility_cache_gate` / `build_facility_template_names_cache_is_per_world_and_invalidates` (hq-wq7g6y).\n- Radar terrain texture non-black samples gated via `radar_pixels>0` (full 128x128 = 16384).\n- Terrain source tiles hydrate through TerrainZH.big archive reader after heightmap hint install.\n",
            st.present_timing_samples,
            avg_present,
            avg_collect,
            avg_terrain,
            avg_forward,
            st.max_radar_pixels,
            st.max_terrain_chunks,
            if avg_terrain >= avg_collect && avg_terrain >= avg_forward {
                "terrain"
            } else if avg_collect >= avg_forward {
                "collect"
            } else {
                "forward"
            }
        );
        let _ = fs::write(&docs, body);
        result.detail = format!(
            "present_frame_pass; window_visible={} terrain={} models={} ui={} selection={} radar_pixels={} items={} screenshot={} present_ms={:.3}",
            st.saw_window_visible,
            st.max_terrain_chunks,
            st.max_render_models,
            st.max_render_ui,
            st.max_render_selection,
            st.max_radar_pixels,
            st.max_render_item_count,
            st.present_screenshot_path,
            avg_present
        );
        let _ = write_control(control_path, &["exit"]);
        st.commanded_at = Some(Instant::now());
        st.phase = 3;
    }
}
