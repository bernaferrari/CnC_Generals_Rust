//! Gate: production `generals` binary via runtime host bridge.
//!
//! Prefer NewGame queue path (Menu drain). Falls back is internal to harness.
//! `playable_claim` must stay false.

use generals_main::executable_smoke::{format_executable_smoke_report, run_executable_smoke};
use std::time::Duration;

fn main() {
    // CLI seconds (e.g. `executable_smoke_gate 900`) > env > default 480.
    // Smoke command chain exceeds 120s after map load on debug builds.
    let timeout_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            std::env::var("EXECUTABLE_SMOKE_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(480);
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

    // InGame world-draw residual: require presentation snapshot + stable mesh items.
    // Fail-closed: Menu→InGame without unit draw is not a playable host residual.
    if r.executable_host_ok && r.reached_ingame {
        if !r.presentation_frame_ok {
            eprintln!(
                "executable_smoke_gate: FAIL presentation_frame_ok=false (InGame without PresentationFrame)"
            );
            std::process::exit(5);
        }
        if !r.render_items_stable_ok || r.max_render_item_count == 0 {
            eprintln!(
                "executable_smoke_gate: FAIL render_items_stable={} max_render_items={} (InGame world mesh pass empty/unstable)",
                r.render_items_stable_ok, r.max_render_item_count
            );
            std::process::exit(6);
        }
    }

    // Soft environments without display/binary: non-zero but distinct for CI classification.
    match r.status.as_str() {
        "success" | "success_partial_exit" | "success_forced_exit" if r.executable_host_ok => {
            println!(
                "executable_smoke_gate: PASS (executable_host_ok=true playable_claim=false ingame={} menu={} gameplay_cmd={} skirmish_menu={} frames={} new_game={} presentation_ok={} render_items={} render_stable={})",
                r.reached_ingame, r.reached_menu, r.gameplay_cmd_ok, r.skirmish_menu_ok, r.frames_observed, r.new_game_path,
                r.presentation_frame_ok, r.max_render_item_count, r.render_items_stable_ok
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
