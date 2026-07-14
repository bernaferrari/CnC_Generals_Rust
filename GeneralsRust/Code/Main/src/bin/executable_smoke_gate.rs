//! Gate: production `generals` binary via runtime host bridge.
//!
//! Prefer NewGame queue path (Menu drain). Falls back is internal to harness.
//! `playable_claim` must stay false.

use generals_main::executable_smoke::{format_executable_smoke_report, run_executable_smoke};
use std::time::Duration;

fn main() {
    let timeout_secs: u64 = std::env::var("EXECUTABLE_SMOKE_TIMEOUT_SECS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(120);
    // Default: direct start_game (proven). Opt into NewGame drain with EXECUTABLE_SMOKE_NEW_GAME=1.
    let use_new_game = std::env::var("EXECUTABLE_SMOKE_NEW_GAME")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    let r = run_executable_smoke(Duration::from_secs(timeout_secs), use_new_game);
    println!("{}", format_executable_smoke_report(&r));

    // Fail-closed retail claim.
    if r.playable_claim {
        eprintln!("executable_smoke_gate: FAIL playable_claim must stay false");
        std::process::exit(2);
    }

    // Soft environments without display/binary: non-zero but distinct for CI classification.
    match r.status.as_str() {
        "success" | "success_partial_exit" | "success_forced_exit" if r.executable_host_ok => {
            println!(
                "executable_smoke_gate: PASS (executable_host_ok=true playable_claim=false ingame={} menu={} frames={} new_game={})",
                r.reached_ingame, r.reached_menu, r.frames_observed, r.new_game_path
            );
            std::process::exit(0);
        }
        "binary_missing" => {
            eprintln!("executable_smoke_gate: SKIP binary_missing — {}", r.detail);
            // Treat missing binary as failure in this gate (caller should build first).
            std::process::exit(3);
        }
        "assets_or_display_unavailable" | "spawn_failed" | "no_menu" => {
            eprintln!(
                "executable_smoke_gate: FAIL env/display status={} — {}",
                r.status, r.detail
            );
            std::process::exit(4);
        }
        other => {
            eprintln!(
                "executable_smoke_gate: FAIL status={} host_ok={} — {}",
                other, r.executable_host_ok, r.detail
            );
            std::process::exit(1);
        }
    }
}
