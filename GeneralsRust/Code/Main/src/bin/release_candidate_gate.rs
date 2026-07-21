use generals_main::release_candidate::{format_rc_report, run_release_candidate_package};

fn main() {
    let report = run_release_candidate_package(2, 5);
    println!("{}", format_rc_report(&report));
    if report.soak_ok
        && report.deterministic_match
        && report.dual_run_hash_match
        && report.missing_asset_policy_ok
        && report.presentation_ok
        && report.campaign_soak_ok
        && report.campaign_runtime_ok
        && report.golden_status == "success"
    {
        println!(
            "release_candidate_gate: PASS campaign_runtime_ok=true retail_campaign_map_loaded={}",
            report.retail_campaign_map_loaded
        );
        std::process::exit(0);
    }
    eprintln!(
        "release_candidate_gate: FAIL presentation={} campaign={} campaign_runtime={} golden={}",
        report.presentation_ok,
        report.campaign_soak_ok,
        report.campaign_runtime_ok,
        report.golden_status
    );
    std::process::exit(1);
}
