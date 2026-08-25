//! Windowed sit-through acceptance runner (thin).
//!
//! Evaluation lives in [`generals_main::windowed_acceptance`]. This binary only
//! runs executable smoke, evaluates the 8-step report, prints it, and exits.
//!
//! ```text
//! cargo run -p generals_main --bin windowed_acceptance_gate
//! cargo run -p generals_main --bin windowed_acceptance_gate -- 900
//! ```
//!
//! Requires a **real display + GPU**, retail `windows_game` assets (Lone Eagle
//! or MapsZH), and a person operating the visible game during the timeout.
//! This is **not** `executable_smoke_gate`: the windowed runner observes
//! physical input and does not drive the menu/game through its control file.
//! It does not flip or require `playable_claim`. Headless
//! `executable_host_ok` is never a pass.
//!
//! Windowed intent is the child flag `-runtime_host=windowed` (never headless)
//! plus `-windowed`. This process does not mutate parent environment.
//!
//! Exit codes:
//! - `0` all 8 sit-through steps + assets
//! - `3` `generals` binary missing
//! - `4` assets / display unavailable
//! - `1` sit-through incomplete

use generals_main::executable_smoke::{
    ExecutableSmokeResult, format_executable_smoke_report, run_windowed_acceptance_smoke,
};
use generals_main::windowed_acceptance::{
    display_or_wgpu_adapter_available, display_unavailable_status, evaluate_windowed_acceptance,
    format_windowed_acceptance_report,
};
use std::time::Duration;

fn main() {
    let timeout_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            std::env::var("WINDOWED_ACCEPTANCE_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(900);

    let use_new_game = std::env::var("EXECUTABLE_SMOKE_NEW_GAME")
        .map(|v| !(v == "0" || v.eq_ignore_ascii_case("false")))
        .unwrap_or(true);

    // First-class probe: no display and no wgpu adapter is
    // `assets_or_display_unavailable` (exit 4), never PASS.
    if !display_or_wgpu_adapter_available() {
        let mut smoke = ExecutableSmokeResult::default();
        smoke.status = display_unavailable_status().into();
        smoke.detail = "display_or_wgpu_adapter_available()=false; run windowed_acceptance_gate on a machine with a GPU/window server (WINDOWED_ACCEPTANCE_TIMEOUT_SECS=900)".into();
        println!("{}", format_executable_smoke_report(&smoke));
        let report = evaluate_windowed_acceptance(&smoke);
        println!("{}", format_windowed_acceptance_report(&report));
        eprintln!(
            "windowed_acceptance_gate: FAIL missing={} playable_claim={} (reported only) host_ok={} status={} smoke={} — {}",
            if report.missing.is_empty() {
                "-".to_string()
            } else {
                report.missing.join(",")
            },
            report.playable_claim,
            report.executable_host_ok,
            report.status,
            report.smoke_status,
            report.smoke_detail
        );
        std::process::exit(4);
    }

    let smoke = run_windowed_acceptance_smoke(Duration::from_secs(timeout_secs), use_new_game);
    println!("{}", format_executable_smoke_report(&smoke));

    let report = evaluate_windowed_acceptance(&smoke);
    println!("{}", format_windowed_acceptance_report(&report));

    if report.passed {
        println!(
            "windowed_acceptance_gate: PASS (8/8 sit-through; playable_claim={} is reported only; host_ok={} is never sufficient)",
            report.playable_claim, report.executable_host_ok
        );
    } else {
        eprintln!(
            "windowed_acceptance_gate: FAIL missing={} playable_claim={} (reported only) host_ok={} status={} smoke={} — {}",
            if report.missing.is_empty() {
                "-".to_string()
            } else {
                report.missing.join(",")
            },
            report.playable_claim,
            report.executable_host_ok,
            report.status,
            report.smoke_status,
            report.smoke_detail
        );
    }
    std::process::exit(report.exit_code);
}
