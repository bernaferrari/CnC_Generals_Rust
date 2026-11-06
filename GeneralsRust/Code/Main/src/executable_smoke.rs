//! Executable smoke via the production `generals` binary + runtime host bridge.
//!
//! This is **stronger** than headless `shell_smoke` (which constructs `GameLogic`
//! in-process): it boots the real event loop, creates a (hidden) window, runs
//! WW3D headless init, and drives Menu → Start through the same control file
//! path GPUI uses.
//!
//! Retail gates pass a per-run [`SmokeOptions`] snapshot. Asset overrides are
//! installed on the spawned host only, and screenshot destinations remain
//! owned by the invoking run; parallel runs do not rewrite process variables.
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

/// Per-run asset and capture configuration, passed to the smoke child only.
#[derive(Clone, Debug, Default)]
pub struct SmokeOptions {
    pub assets_dir: Option<PathBuf>,
    pub screenshot_path: Option<PathBuf>,
}

impl SmokeOptions {
    /// Snapshot the caller's inherited settings without changing the process environment.
    pub fn from_environment() -> Self {
        Self {
            assets_dir: std::env::var_os("GENERALS_ASSETS_DIR").map(PathBuf::from),
            screenshot_path: std::env::var_os("GENERALS_PRESENT_SCREENSHOT").map(PathBuf::from),
        }
    }

    /// Use the discovered retail installation only when no asset override exists.
    pub fn for_retail(retail: &Path) -> Self {
        let mut options = Self::from_environment();
        if options.assets_dir.is_none() {
            options.assets_dir = Some(retail.to_path_buf());
        }
        options
    }
}

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
        &SmokeOptions::from_environment(),
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
        &SmokeOptions::from_environment(),
    )
}

/// Windowed **automated** smoke: `-runtime_host=windowed -windowed` child
/// (a real, visible OS window) driven through the same runtime-host control
/// file as the headless automated smoke. The child-side phase-20 ladder uses
/// winit-equivalent injects (`Injected` origin), so the physical
/// `wnd_widget_tree_nav` / `interactive_gameplay` latches stay false unless a
/// real OS mouse input (human or OS clicker) re-enters
/// `handle_mouse_button_input` — exactly the retail sit-through contract.
/// Boot the production `generals` binary to the shell MainMenu.wnd, then exit.
///
/// This is the P0 hq-x1j4u.9.1 gate: real event loop + retail assets + live
/// shell WND. It does not continue into skirmish/InGame.
pub fn run_boot_to_shell_menu(timeout: Duration, options: &SmokeOptions) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        false,
        ExecutableSmokeLaunch::HeadlessHost,
        SmokeDriver::MenuOnly,
        options,
    )
}

/// Boot to shell, start a skirmish, and stop once a retail (non-shell) map is InGame.
///
/// P0 hq-x1j4u.9.2 gate. Skips the gameplay command chain so debug builds are not
/// held in the post-load update hot path longer than needed for the latch.
pub fn run_load_retail_skirmish_map(timeout: Duration, options: &SmokeOptions) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        true, // NewGame / skirmish menu drain path
        ExecutableSmokeLaunch::HeadlessHost,
        SmokeDriver::MapLoadOnly,
        options,
    )
}

/// Load a retail skirmish map, then issue select/move/attack on the live host.
///
/// P0 hq-x1j4u.9.3 gate. Stops after the three order residuals latch (does not
/// walk the full retail command ladder).
pub fn run_issue_player_orders(timeout: Duration, options: &SmokeOptions) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        true,
        ExecutableSmokeLaunch::HeadlessHost,
        SmokeDriver::OrdersOnly,
        options,
    )
}

/// Retail skirmish with concurrent human move orders and AI activity residual.
///
/// P0 hq-x1j4u.9.5 gate.
pub fn run_ai_opponent_skirmish(timeout: Duration, options: &SmokeOptions) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        true,
        ExecutableSmokeLaunch::HeadlessHost,
        SmokeDriver::AiActivity,
        options,
    )
}

/// Retail skirmish save → load (fresh staged GameLogic) → restore assert → post-load order.
///
/// P0 hq-x1j4u.9.7 gate. Uses production-host quicksave/quickload so load goes
/// through the same staged-candidate path as the UI load.
pub fn run_retail_skirmish_save_load(timeout: Duration, options: &SmokeOptions) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        true,
        ExecutableSmokeLaunch::HeadlessHost,
        SmokeDriver::SaveLoadOnly,
        options,
    )
}

/// Windowed Lone Eagle present sit-through (≥60s) with selected unit + stable render items.
///
/// P0 hq-x1j4u.9.6 gate. Uses `-runtime_host=windowed`. Saves a screenshot under
/// `GeneralsRust/artifacts/` (or `GENERALS_PRESENT_SCREENSHOT`) when capture works.
pub fn run_present_skirmish_frame(timeout: Duration, options: &SmokeOptions) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        true,
        ExecutableSmokeLaunch::Windowed,
        SmokeDriver::PresentFrameOnly,
        options,
    )
}

pub fn run_executable_smoke_windowed(
    timeout: Duration,
    use_new_game_path: bool,
) -> ExecutableSmokeResult {
    run_executable_smoke_with_launch_and_driver(
        timeout,
        use_new_game_path,
        ExecutableSmokeLaunch::Windowed,
        SmokeDriver::Automated,
        &SmokeOptions::from_environment(),
    )
}

fn run_executable_smoke_with_launch_and_driver(
    timeout: Duration,
    use_new_game_path: bool,
    launch: ExecutableSmokeLaunch,
    driver: SmokeDriver,
    options: &SmokeOptions,
) -> ExecutableSmokeResult {
    // Retry one early boot failure after this invocation's child is cleaned up.
    let first = run_executable_smoke_once(timeout, use_new_game_path, launch, driver, options);
    let retryable = matches!(
        first.status.as_str(),
        "process_exited" | "timeout" | "no_menu"
    ) && !first.reached_menu
        && !first.reached_ingame;
    if !retryable {
        return first;
    }
    std::thread::sleep(Duration::from_millis(1500));
    let second = run_executable_smoke_once(timeout, use_new_game_path, launch, driver, options);
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
#[path = "executable_smoke/retail_map.rs"]
mod retail_map;
include!("executable_smoke/process.rs");
include!("executable_smoke/bootstrap.rs");
include!("executable_smoke/frame_loop.rs");
include!("executable_smoke/construction_wait.rs");
include!("executable_smoke/windowed_navigation.rs");
include!("executable_smoke/command_status.rs");
include!("executable_smoke/gameplay_chain.rs");
include!("executable_smoke/presentation.rs");
include!("executable_smoke/shutdown.rs");
include!("executable_smoke/report.rs");

include!("executable_smoke/tests.rs");
