//! Composite behavior gate — truthful playability signal (not file-matrix only).
//!
//! Runs production-linked map/golden/breadth/RC checks. Matrix audit is reported
//! separately and never alone proves playability.

use generals_main::breadth_scenarios::{format_breadth_report, run_all_breadth};
use generals_main::golden_skirmish::{format_golden_report, run_golden_skirmish};
use generals_main::map_frame_scenario::{
    format_map_frame_report, run_map_frame_scenario, MapFrameStatus,
};
use generals_main::release_candidate::{format_rc_report, run_release_candidate_package};

fn main() {
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

    // 2) Golden skirmish vertical slice.
    let golden = run_golden_skirmish(None, 12);
    println!("golden: {}", format_golden_report(&golden));
    if golden.status != "success"
        || !golden.victory
        || !golden.save_load_ok
        || !golden.fought
    {
        failed.push(format!(
            "golden status={} victory={} save={} fight={}",
            golden.status, golden.victory, golden.save_load_ok, golden.fought
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

    // 4) RC package.
    let rc = run_release_candidate_package(2, 5);
    println!("rc: {}", format_rc_report(&rc));
    if !(rc.soak_ok
        && rc.deterministic_match
        && rc.dual_run_hash_match
        && rc.missing_asset_policy_ok
        && rc.presentation_ok
        && rc.campaign_soak_ok
        && rc.golden_status == "success")
    {
        failed.push(format!("rc failed: {}", format_rc_report(&rc)));
    }

    if failed.is_empty() {
        println!("behavior_gate: PASS (map+golden+breadth+rc)");
        std::process::exit(0);
    }
    eprintln!("behavior_gate: FAIL");
    for f in failed {
        eprintln!("  - {f}");
    }
    std::process::exit(1);
}
