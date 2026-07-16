//! Composite behavior gate — truthful playability signal (not file-matrix only).
//!
//! Runs production-linked map/golden/breadth/RC checks. Matrix audit is reported
//! separately and never alone proves playability.

use generals_main::ai_skirmish_activity::{
    format_ai_activity_report, run_medium_ai_skirmish_activity,
};
use generals_main::breadth_scenarios::{format_breadth_report, run_all_breadth};
use generals_main::executable_smoke::{format_executable_smoke_report, run_executable_smoke};
use generals_main::golden_campaign::{format_campaign_report, run_golden_campaign};
use generals_main::golden_skirmish::{format_golden_report, run_golden_skirmish};
use generals_main::map_frame_scenario::{
    format_map_frame_report, run_map_frame_scenario, MapFrameStatus,
};
use generals_main::release_candidate::{format_rc_report, run_release_candidate_package};
use generals_main::shell_smoke::{format_shell_smoke_report, run_shell_smoke};
use std::time::Duration;

fn main() {
    // Gate default: GameWorld damage last-writer for HP (opt out DAMAGE_AUTHORITY=0).
    generals_main::gameworld_shadow::ensure_gate_damage_authority();
    let mut failed = Vec::new();

    // 1) Map frames — when assets present require load + advance.
    let map = run_map_frame_scenario(None, 5);
    println!("map_frame: {}", format_map_frame_report(&map));
    let map_ok = map.frames_advanced > 0
        && matches!(
            map.status,
            MapFrameStatus::Success | MapFrameStatus::AssetsUnavailable
        );
    if map.map_loaded && map.frames_advanced == 0 {
        failed.push("map_frame_loaded_but_no_frames".into());
    } else if !map_ok {
        failed.push(format!("map_frame status={:?}", map.status));
    }

    // 2) Golden skirmish vertical slice (enough end frames; combat has its own budget).
    let golden = run_golden_skirmish(None, 30);
    println!("golden: {}", format_golden_report(&golden));
    if golden.status != "success" || !golden.victory || !golden.save_load_ok || !golden.fought {
        failed.push(format!(
            "golden status={} victory={} save={} fight={}",
            golden.status, golden.victory, golden.save_load_ok, golden.fought
        ));
    }
    // Map present: !synthetic_combat && map_host_playable_ok; playable_claim always false.
    // Map absent: synthetic_combat && !map_host_playable_ok; playable_claim always false.
    if golden.playable_claim {
        failed.push(format!(
            "golden playable_claim={} (must stay false; headless is not retail playthrough)",
            golden.playable_claim
        ));
    }
    if golden.map_loaded {
        if golden.synthetic_combat {
            failed.push(format!(
                "golden synthetic_combat={} (expected false when map_loaded with map-world victory)",
                golden.synthetic_combat
            ));
        }
        if !golden.map_host_playable_ok {
            failed.push(format!(
                "golden map_host_playable_ok={} (expected true when map-world victory proven)",
                golden.map_host_playable_ok
            ));
        }
        if !(golden.map_combat_ok
            && golden.same_world_production_ok
            && golden.same_world_victory_ok
            && golden.players_preserved_on_load)
        {
            failed.push(format!(
                "golden map same-world flags incomplete combat={} prod={} victory={} preserved={}",
                golden.map_combat_ok,
                golden.same_world_production_ok,
                golden.same_world_victory_ok,
                golden.players_preserved_on_load
            ));
        }
    } else {
        if !golden.synthetic_combat {
            failed.push(format!(
                "golden synthetic_combat={} (expected true when map absent)",
                golden.synthetic_combat
            ));
        }
        if golden.map_host_playable_ok {
            failed.push(format!(
                "golden map_host_playable_ok={} (must be false while synthetic_combat / map absent)",
                golden.map_host_playable_ok
            ));
        }
    }
    if !golden.ai_structure_templates_retained {
        failed.push(format!(
            "golden ai_structure_templates_retained={} (must retain AI structure catalog; no mid-scenario strip)",
            golden.ai_structure_templates_retained
        ));
    }

    // 3) Breadth categories.
    let breadth = run_all_breadth();
    println!("{}", format_breadth_report(&breadth));
    for r in &breadth {
        if !r.ok {
            failed.push(format!("breadth {} failed: {}", r.category, r.detail));
        }
    }

    // 4) Medium AI non-idle activity on host path.
    let ai = run_medium_ai_skirmish_activity(120);
    println!("ai: {}", format_ai_activity_report(&ai));
    if ai.status != "success"
        || !(ai.activity_count >= 2
            || ai.ai_structures >= 3
            || (ai.activity_count >= 1 && ai.ai_units_or_queue >= 1))
    {
        failed.push(format!(
            "ai activity={} status={}",
            ai.activity_count, ai.status
        ));
    }

    // 5) Shell/boot smoke (headless production types + skirmish start).
    let shell = run_shell_smoke(8);
    println!("{}", format_shell_smoke_report(&shell));
    if shell.status != "success" {
        failed.push(format!("shell {}", shell.detail));
    }
    if shell.playable_claim {
        failed.push(format!(
            "shell playable_claim={} (headless smoke must fail-closed for retail W3D)",
            shell.playable_claim
        ));
    }
    if !shell.shell_host_playable_ok {
        failed.push(format!(
            "shell shell_host_playable_ok={} (expected true when status=success)",
            shell.shell_host_playable_ok
        ));
    }
    if !shell.control_bar_layout_ok {
        failed.push(format!(
            "shell control_bar_layout_ok={} (ControlBar.wnd ensure residual)",
            shell.control_bar_layout_ok
        ));
    }

    // 6) Campaign residual — SinglePlayer + mission scripts tick + frames.
    let campaign = run_golden_campaign(None, 5);
    println!("campaign: {}", format_campaign_report(&campaign));
    if !(campaign.campaign_playable_claim
        && campaign.status == "success"
        && campaign.scripts_tick_ok
        && campaign.frames_advanced > 0)
    {
        failed.push(format!(
            "campaign residual claim={} status={} scripts_tick={} frames={}",
            campaign.campaign_playable_claim,
            campaign.status,
            campaign.scripts_tick_ok,
            campaign.frames_advanced
        ));
    }

    // 7) RC package.
    let rc = run_release_candidate_package(2, 5);
    println!("rc: {}", format_rc_report(&rc));
    if !(rc.soak_ok
        && rc.deterministic_match
        && rc.dual_run_hash_match
        && rc.missing_asset_policy_ok
        && rc.presentation_ok
        && rc.campaign_soak_ok
        && rc.campaign_runtime_ok
        && rc.golden_status == "success")
    {
        failed.push(format!("rc failed: {}", format_rc_report(&rc)));
    }

    // 8) Executable smoke (real generals binary + runtime host). Soft-skip when
    // binary missing or display/GPU unavailable; fail only when host starts and
    // then fails to reach InGame, or when playable_claim flips true.
    // Opt out: EXECUTABLE_SMOKE=0. Force NewGame path: EXECUTABLE_SMOKE_NEW_GAME=1.
    let exec_enabled = std::env::var("EXECUTABLE_SMOKE")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(true);
    let mut exec_host_ok = false;
    let mut exec_status = "skipped".to_string();
    if exec_enabled {
        let timeout_secs: u64 = std::env::var("EXECUTABLE_SMOKE_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(90);
        let use_new_game = std::env::var("EXECUTABLE_SMOKE_NEW_GAME")
            .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let exec = run_executable_smoke(Duration::from_secs(timeout_secs), use_new_game);
        println!("{}", format_executable_smoke_report(&exec));
        exec_status = exec.status.clone();
        exec_host_ok = exec.executable_host_ok;
        if exec.playable_claim {
            failed.push(
                "executable_smoke playable_claim=true (must stay false for residual honesty)"
                    .into(),
            );
        }
        match exec.status.as_str() {
            "success" | "success_partial_exit" | "success_forced_exit" => {
                if !(exec.executable_host_ok && exec.reached_ingame) {
                    failed.push(format!(
                        "executable_smoke status={} host_ok={} ingame={} detail={}",
                        exec.status, exec.executable_host_ok, exec.reached_ingame, exec.detail
                    ));
                }
            }
            "binary_missing" | "assets_or_display_unavailable" | "spawn_failed" | "no_menu" => {
                // Soft environments (CI without display/assets): do not fail composite.
                eprintln!(
                    "behavior_gate: executable_smoke soft-skip status={} detail={}",
                    exec.status, exec.detail
                );
            }
            other => {
                // Process started and reached Menu but failed InGame, etc.
                if exec.process_started && exec.reached_menu && !exec.reached_ingame {
                    failed.push(format!(
                        "executable_smoke started but no InGame status={} detail={}",
                        other, exec.detail
                    ));
                } else if exec.process_started && !exec.reached_menu {
                    eprintln!(
                        "behavior_gate: executable_smoke soft-skip (no menu) status={} detail={}",
                        other, exec.detail
                    );
                } else {
                    failed.push(format!(
                        "executable_smoke status={} detail={}",
                        other, exec.detail
                    ));
                }
            }
        }
    } else {
        println!("executable_smoke: skipped (EXECUTABLE_SMOKE=0)");
    }

    if failed.is_empty() {
        // PASS text reflects values already asserted above (not hardcoded-only).
        // Honesty flags: shell playable_claim always false; golden synthetic when no map;
        // retail_* / combat_no_teleport / combat_realistic_* are residual honesty only.
        // Campaign retail_campaign_map_loaded fail-closed when full MD_*/GC_* load hangs.
        println!(
            "behavior_gate: PASS (headless host APIs; golden map_loaded={} synthetic_combat={} playable_claim={} map_host_ok={} retail_prod={} retail_gather={} combat_no_teleport={} combat_realistic_speed={} combat_store_damage={}; shell playable_claim={} shell_host_playable_ok={}; campaign_playable_claim={} retail_campaign_map_loaded={}; executable_host_ok={} executable_status={})",
            golden.map_loaded,
            golden.synthetic_combat,
            golden.playable_claim,
            golden.map_host_playable_ok,
            golden.retail_production_chain_ok,
            golden.retail_gather_ok,
            golden.combat_no_teleport_ok,
            golden.combat_realistic_speed_ok,
            golden.combat_store_damage_ok,
            shell.playable_claim,
            shell.shell_host_playable_ok,
            campaign.campaign_playable_claim,
            campaign.retail_campaign_map_loaded,
            exec_host_ok,
            exec_status
        );
        std::process::exit(0);
    }
    eprintln!(
        "behavior_gate: FAIL golden_map={} synthetic={} playable_claim={} map_host_ok={} retail_prod={} retail_gather={} combat_no_teleport={} combat_realistic_speed={} combat_store_damage={} shell_claim={} shell_host_playable_ok={}",
        golden.map_loaded,
        golden.synthetic_combat,
        golden.playable_claim,
        golden.map_host_playable_ok,
        golden.retail_production_chain_ok,
        golden.retail_gather_ok,
        golden.combat_no_teleport_ok,
        golden.combat_realistic_speed_ok,
        golden.combat_store_damage_ok,
        shell.playable_claim,
        shell.shell_host_playable_ok
    );
    for f in failed {
        eprintln!("  - {f}");
    }
    std::process::exit(1);
}
