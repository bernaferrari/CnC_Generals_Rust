//! Executable smoke via the production `generals` binary + runtime host bridge.
//!
//! This is **stronger** than headless `shell_smoke` (which constructs `GameLogic`
//! in-process): it boots the real event loop, creates a (hidden) window, runs
//! WW3D headless init, and drives Menu → Start through the same control file
//! path GPUI uses.
//!
//! Honesty:
//! - `playable_claim` is the five-flag retail formula (`window_visible` &&
//!   `wnd_widget_tree_nav` && `live_frame_ok` && InGame && gameplay). Headless
//!   host smoke never publishes a visible OS window or OS/WND widget-tree nav, so
//!   the claim stays false (`executable_smoke_gate` / `behavior_gate` still require
//!   false for the **headless** host). Windowed launch may publish true when all
//!   five flags are observed; the gate does not reject that finished-window claim.
//! - `retail_sit_through_missing` lists whichever of those five flags are still
//!   false (empty only when the claim is true). Status lines print each flag.
//! - `host_vertical_slice_ok` is the strengthened headless claim: shell WND + skirmish
//!   latch chain (map/slots/rules/start) + InGame + construct/train/gameplay +
//!   presentation boundary with non-zero stable render items (no live GameLogic
//!   dual-read). Still not full retail `playable_claim`.
//! - `executable_host_ok` is the limited claim: process boots, reaches Menu or
//!   InGame via runtime host commands, and exits cleanly.
//! - If display/GPU/window creation fails in the environment, status is
//!   `assets_or_display_unavailable` (fail-closed, not a green lie).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Candidate retail Lone Eagle paths (workspace-relative).
///
/// Shared with [`crate::windowed_acceptance`] so the windowed sit-through gate
/// probes the same extract locations as executable smoke.
pub const LONE_EAGLE_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
];

/// Run the executable smoke with a timeout budget.
///
/// `use_new_game_path`: when true, drive Start via `queue_new_game` (Menu drain).
/// When false, use direct `start_game` runtime host command.
pub fn run_executable_smoke(timeout: Duration, use_new_game_path: bool) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        use_new_game_path,
        ExecutableSmokeLaunch::HeadlessHost,
        SmokeDriver::Automated,
    )
}

/// Windowed sit-through observer. Never passes `-runtime_host=headless`, and
/// never drives the child through its control file. A person must operate the
/// visible game; the runner only observes its status and terminates it at the
/// timeout boundary.
///
/// `use_new_game_path` is retained for the shared API/report shape, but is
/// intentionally ignored by the manual observer.
pub fn run_windowed_acceptance_smoke(
    timeout: Duration,
    use_new_game_path: bool,
) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        use_new_game_path,
        ExecutableSmokeLaunch::Windowed,
        SmokeDriver::ManualObserver,
    )
}

fn run_executable_smoke_with_launch_and_driver(
    timeout: Duration,
    use_new_game_path: bool,
    launch: ExecutableSmokeLaunch,
    driver: SmokeDriver,
) -> ExecutableSmokeResult {
    // One automatic retry: Booting early-exit is commonly a stale GPU/lock race after
    // pkill -9 (no Drop cleanup). Second attempt after a fresh kill is usually green.
    let first = run_executable_smoke_once(timeout, use_new_game_path, launch, driver);
    let retryable = matches!(
        first.status.as_str(),
        "process_exited" | "timeout" | "no_menu"
    ) && !first.reached_menu
        && !first.reached_ingame;
    if !retryable {
        return first;
    }
    std::thread::sleep(Duration::from_millis(1500));
    let second = run_executable_smoke_once(timeout, use_new_game_path, launch, driver);
    if second.executable_host_ok || second.reached_menu || second.reached_ingame {
        let mut out = second;
        out.detail = format!(
            "retry_after_boot_race; first={}; {}",
            first.detail, out.detail
        );
        return out;
    }
    // Prefer the more informative failure.
    let mut out = first;
    out.detail = format!(
        "retry_also_failed; second={}; {}",
        second.detail, out.detail
    );
    out
}

// Ordered lifecycle split (same-module `include!` convention as
// game_logic/combat and shell_smoke/result):
//   result → status → process → bootstrap → frame_loop → gameplay_chain →
//   presentation → shutdown → report → tests.
// EXECUTABLE_SMOKE_SRC in executable_smoke/tests.rs concatenates these in
// the same order for the source-text honesty assertions.
include!("executable_smoke/result.rs");
include!("executable_smoke/status.rs");
include!("executable_smoke/process.rs");
include!("executable_smoke/bootstrap.rs");
include!("executable_smoke/frame_loop.rs");
include!("executable_smoke/gameplay_chain.rs");
include!("executable_smoke/presentation.rs");
include!("executable_smoke/shutdown.rs");
include!("executable_smoke/report.rs");

include!("executable_smoke/tests.rs");
