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
                    &["construct|template=USA_Barracks|auto_target=1"],
                );
                st.gameplay_step = 3;
                st.commanded_at = Some(Instant::now());
            }
        } else {
            let _ = write_control(
                &control_path,
                &[
                    "construct|template=USA_Barracks|spawn_dozer=1|alias_fallback=1|auto_target=1",
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
            && st.commanded_at
                .map(|t| t.elapsed() > Duration::from_secs(3))
                .unwrap_or(false)
        {
            let _ = write_control(
                &control_path,
                &["construct|template=USA_Barracks|auto_target=1"],
            );
            st.commanded_at = Some(Instant::now());
        }
        if launch == ExecutableSmokeLaunch::Windowed {
            // construct_ok is "DozerConstruct issued". Honest train
            // waits until we observe under_construction>0 then 0
            // (barracks finished). Immediate UC==0 is a stale frame.
            if snap.under_construction > 0 {
                st.saw_construct_under_construction = true;
            }
            let elapsed = st.commanded_at.map(|t| t.elapsed()).unwrap_or_default();
            let min_wait = elapsed > Duration::from_secs(8);
            let build_done = st.saw_construct_ok
                && st.saw_construct_under_construction
                && snap.under_construction == 0
                && min_wait;
            let build_timeout = elapsed > Duration::from_secs(90);
            if !build_done && !build_timeout {
                // keep polling; do not issue train yet
            } else if st.saw_construct_ok || build_timeout {
                // One template only — a second train_unit overwrites
                // train_ok with train_fail_enqueue on the CC.
                let _ = write_control(
                    &control_path,
                    &["train_unit|template=AmericaInfantryRanger|auto_target=1"],
                );
                st.train_sent = true;
                st.train_retry_started = Some(Instant::now());
                st.gameplay_step = 4;
                st.commanded_at = Some(Instant::now());
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
                && snap.last_gameplay_cmd.starts_with("load_fail")
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
        if snap.last_gameplay_cmd.starts_with("attack_ok")
            || snap.last_gameplay_cmd.starts_with("attack_fail")
            || snap.last_gameplay_cmd.starts_with("attack_begin")
        {
            st.saw_attack_ok = true;
        }
        if snap.last_gameplay_cmd.starts_with("select_ok") {
            st.saw_select_ok = true;
        }
        if launch == ExecutableSmokeLaunch::Windowed {
            // Re-issue only while the producer is complete and train
            // has not succeeded. train_fail must not block a retry.
            if st.train_sent
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
        if want_exit
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
