//! Windowed sit-through acceptance — a real 8-step API, not a headless lie.
//!
//! # What this is
//!
//! `executable_smoke_gate`, `shell_smoke`, and `golden_skirmish` are **headless
//! host** gates. They may report `executable_host_ok` / `shell_host_playable_ok`
//! / `map_host_playable_ok`, but they **require `playable_claim=false`**. This
//! module is the separate windowed acceptance evaluator: it scores a completed
//! [`ExecutableSmokeResult`] against eight observed sit-through steps and
//! requires retail `windows_game` assets (Lone Eagle or MapsZH).
//!
//! `playable_claim` is **reported only**. [`evaluate_windowed_acceptance`] never
//! requires it to be true and never writes it. Headless `host_ok` is **never**
//! treated as a pass.
//!
//! # How to run
//!
//! Needs a real display / GPU **and** `windows_game` assets (Lone Eagle map or
//! MapsZH extract / `.big`). This is not a CI-green headless residual.
//!
//! Probe first with [`display_or_wgpu_adapter_available`]. If that is false the
//! gate must **not** fake PASS — it reports `assets_or_display_unavailable`
//! (exit 4). When a display or wgpu adapter can present, run the 900s windowed
//! sit-through **with a human operating the visible game**. The runner is an
//! observer: it does not write menu, game, production, gather, or save/load
//! commands into the runtime-host control file.
//!
//! ```text
//! # Machine with a GPU / window server (default 900s):
//! cargo build -p generals_main --bin generals --release
//! cargo run -p generals_main --bin windowed_acceptance_gate
//! cargo run -p generals_main --bin windowed_acceptance_gate -- 900
//! WINDOWED_ACCEPTANCE_TIMEOUT_SECS=900 cargo run -p generals_main --bin windowed_acceptance_gate
//!
//! # Override the probe (tests / CI only — never a green lie):
//! WINDOWED_ACCEPTANCE_FORCE_DISPLAY=0  # treat as no display/adapter
//! WINDOWED_ACCEPTANCE_FORCE_DISPLAY=1  # treat as display/adapter present
//! ```
//!
//! Build the runtime first (`cargo build -p generals_main --bin generals --release`)
//! or set `GENERALS_RUNTIME_EXE`. Timeout CLI arg overrides
//! `WINDOWED_ACCEPTANCE_TIMEOUT_SECS` (default 900s).
//!
//! # Windowed intent (no parent `set_var`)
//!
//! [`crate::executable_smoke::run_windowed_acceptance_smoke`] launches the child
//! with `-runtime_host=windowed` (never `-runtime_host=headless`) plus
//! `-windowed`. This module does **not** call `std::env::set_var` — that
//! races parallel tests in the parent process. The host does not currently read
//! `GENERALS_WINDOWED`. If a future host path honours it, pass it on the child
//! `Command` env only; do not poison the parent.
//!
//! # The eight steps (all required to PASS)
//!
//! 1. `visible_window` — real OS window (`window_visible`)
//! 2. `physical_main_menu_skirmish_nav` — physical winit menu→match
//!    (`wnd_widget_tree_nav`). Host `main_menu_skirmish_wnd_ok` is **not**
//!    sufficient and is not required.
//! 3. `wgpu_presented_frame` — real WGPU-presented frame (`live_frame_ok`)
//! 4. `non_shell_map` — `reached_ingame` and map is not shellmap / empty / `-`
//! 5. `select_move_attack` — physical winit command evidence
//!    (`interactive_gameplay`). Host-command `gameplay_cmd_ok` /
//!    `combat_damage_ok` are **not** sufficient.
//! 6. `build_and_produce` — physical UI evidence
//!    (`physical_build_and_produce`), never the runtime-host
//!    `construct_cmd_ok` / `train_cmd_ok` diagnostics
//! 7. `gather_resources` — physical UI evidence
//!    (`physical_gather_resources`), never `return_supplies_cmd_ok`
//! 8. `save_load_continue` — physical PopupSaveLoad evidence
//!    (`physical_save_load_continue`), never host `save_cmd_ok` / `load_cmd_ok`
//!
//! The live status publisher does not yet emit steps 6–8's physical evidence
//! fields. Until it does, these steps intentionally remain false even when a
//! runtime-host control command succeeded. That is a real remaining
//! instrumentation gap, not permission to turn host diagnostics into a PASS.
//!
//! Plus `windows_game_assets` (Lone Eagle `.map` or MapsZH). Distinct exits:
//! - `0` PASS all 8 + assets
//! - `3` binary missing
//! - `4` assets / display unavailable (`spawn_failed` / `no_menu` included)
//! - `1` sit-through incomplete (some steps false)
//!
//! Unit tests in this module are filesystem / GPU free except for optional
//! injected fake asset roots.

use crate::executable_smoke::{
    ExecutableSmokeResult, LONE_EAGLE_CANDIDATES, lone_eagle_map_on_disk,
};
use std::path::{Path, PathBuf};

/// Number of sit-through steps that must all be true to PASS.
pub const WINDOWED_ACCEPTANCE_STEP_COUNT: usize = 8;

/// Stable step names, in evaluation order (1–8).
pub const WINDOWED_ACCEPTANCE_STEP_NAMES: &[&str] = &[
    "visible_window",
    "physical_main_menu_skirmish_nav",
    "wgpu_presented_frame",
    "non_shell_map",
    "select_move_attack",
    "build_and_produce",
    "gather_resources",
    "save_load_continue",
];

/// Extra missing-list name when retail `windows_game` assets are absent.
pub const WINDOWS_GAME_ASSETS_MISSING: &str = "windows_game_assets";

/// The result must come from the separate non-headless windowed launcher.
/// Otherwise a hand-built or headless `ExecutableSmokeResult` could satisfy
/// booleans without proving an actual WGPU windowed session.
pub const WINDOWED_LAUNCH_MISSING: &str = "windowed_launch";

/// First-class status when the OS has no presentable display and wgpu finds
/// no adapter. Distinct from sit-through failure; never a green PASS.
pub const ASSETS_OR_DISPLAY_UNAVAILABLE: &str = "assets_or_display_unavailable";

/// Env override for [`display_or_wgpu_adapter_available`] (tests / CI only).
const FORCE_DISPLAY_ENV: &str = "WINDOWED_ACCEPTANCE_FORCE_DISPLAY";

/// Injected display / adapter probe so unit tests do not need a GPU.
pub trait DisplayAvailabilityProbe {
    fn force_unavailable(&self) -> bool {
        false
    }
    fn force_available(&self) -> bool {
        false
    }
    fn has_os_display(&self) -> bool;
    fn has_wgpu_adapter(&self) -> bool;
}

/// Production probe: env override, then OS display hint, then wgpu enumerate.
pub struct OsDisplayProbe;

impl DisplayAvailabilityProbe for OsDisplayProbe {
    fn force_unavailable(&self) -> bool {
        matches!(
            std::env::var(FORCE_DISPLAY_ENV)
                .ok()
                .as_deref()
                .map(str::trim),
            Some("0") | Some("false") | Some("FALSE") | Some("no")
        )
    }

    fn force_available(&self) -> bool {
        matches!(
            std::env::var(FORCE_DISPLAY_ENV)
                .ok()
                .as_deref()
                .map(str::trim),
            Some("1") | Some("true") | Some("TRUE") | Some("yes")
        )
    }

    fn has_os_display(&self) -> bool {
        os_session_has_display()
    }

    fn has_wgpu_adapter(&self) -> bool {
        wgpu_adapter_can_present()
    }
}

/// True when a real display or a wgpu adapter can present.
///
/// Fail-closed: missing display **and** missing adapter is not a PASS. Override
/// with `WINDOWED_ACCEPTANCE_FORCE_DISPLAY=0|1` for tests only.
pub fn display_or_wgpu_adapter_available() -> bool {
    display_or_wgpu_adapter_available_with(&OsDisplayProbe)
}

/// Same rules as [`display_or_wgpu_adapter_available`], with an injected probe.
pub fn display_or_wgpu_adapter_available_with(probe: &impl DisplayAvailabilityProbe) -> bool {
    if probe.force_unavailable() {
        return false;
    }
    if probe.force_available() {
        return true;
    }
    probe.has_os_display() || probe.has_wgpu_adapter()
}

/// Status string for a failed display/adapter probe (exit 4).
pub fn display_unavailable_status() -> &'static str {
    ASSETS_OR_DISPLAY_UNAVAILABLE
}

fn os_session_has_display() -> bool {
    if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return true;
    }
    #[cfg(target_os = "macos")]
    {
        // Aqua / WindowServer is present on a local Mac session. SSH-only
        // boxes without a console still fall through to the wgpu probe.
        if std::path::Path::new("/System/Library/CoreServices/WindowServer").exists()
            && std::env::var_os("SSH_CONNECTION").is_none()
            && std::env::var_os("CI").is_none()
        {
            return true;
        }
    }
    #[cfg(target_os = "windows")]
    {
        if std::env::var_os("SESSIONNAME").is_some() && std::env::var_os("CI").is_none() {
            return true;
        }
    }
    false
}

fn wgpu_adapter_can_present() -> bool {
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..Default::default()
    });
    let adapters = instance.enumerate_adapters(wgpu::Backends::all());
    adapters
        .into_iter()
        .any(|adapter| !matches!(adapter.get_info().device_type, wgpu::DeviceType::Other))
}

/// Relative MapsZH extract / archive locations (workspace-relative).
const MAPSZH_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH",
    "windows_game/extracted_big_files_v2/MapsZH",
    "../windows_game/extracted_big_files/MapsZH",
    "../windows_game/extracted_big_files_v2/MapsZH",
    "windows_game/Command & Conquer Generals Zero Hour/MapsZH.big",
    "../windows_game/Command & Conquer Generals Zero Hour/MapsZH.big",
    "MapsZH",
    "MapsZH.big",
];

/// Per-step windowed sit-through report.
///
/// `playable_claim` and `executable_host_ok` are copied from smoke for
/// display only. They do not gate [`Self::passed`]; `windowed_launch` does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowedAcceptanceReport {
    /// Provenance guard: only a `-runtime_host=windowed` result may satisfy
    /// acceptance, never a headless/synthetic result with copied flags.
    pub windowed_launch: bool,
    pub visible_window: bool,
    pub physical_main_menu_skirmish_nav: bool,
    pub wgpu_presented_frame: bool,
    pub non_shell_map: bool,
    pub select_move_attack: bool,
    pub build_and_produce: bool,
    pub gather_resources: bool,
    pub save_load_continue: bool,
    /// Lone Eagle `.map` or MapsZH extract / `.big` found on disk.
    pub assets_present: bool,
    /// Copied from smoke. **Reported only** — not required, never written.
    pub playable_claim: bool,
    /// Copied from smoke. Headless `host_ok` is never sufficient for PASS.
    pub executable_host_ok: bool,
    /// Runtime-host workflow diagnostics. Kept visible to make it clear why
    /// the corresponding physical acceptance step did *not* go green.
    pub host_build_and_produce_diagnostic: bool,
    pub host_gather_resources_diagnostic: bool,
    pub host_save_load_continue_diagnostic: bool,
    /// Failed sit-through step names (and `windows_game_assets` when absent).
    pub missing: Vec<String>,
    /// `pass` / `binary_missing` / `assets_or_display_unavailable` /
    /// `sit_through_incomplete`.
    pub status: String,
    /// 0 / 1 / 3 / 4 — see module docs.
    pub exit_code: i32,
    /// True only when this is a windowed launch, all 8 steps hold, and assets
    /// are present.
    pub passed: bool,
    /// How many of the 8 sit-through steps are true (`0..=8`).
    pub steps_ok: u8,
    pub smoke_status: String,
    pub smoke_detail: String,
    pub map_seen: String,
}

impl Default for WindowedAcceptanceReport {
    fn default() -> Self {
        evaluate_windowed_acceptance_with_assets(&ExecutableSmokeResult::default(), false)
    }
}

/// True when `map` is a loaded non-shell map (not `shellmap`, empty, or `-`).
pub fn is_non_shell_map(map: &str) -> bool {
    let map = map.to_ascii_lowercase();
    !map.contains("shellmap") && !map.trim().is_empty() && map != "-"
}

/// True when retail `windows_game` assets are on disk (Lone Eagle or MapsZH).
///
/// Uses the same Lone Eagle candidates / search walk as executable smoke, plus
/// MapsZH extract directories and `MapsZH.big`.
pub fn windows_game_assets_present() -> bool {
    if lone_eagle_map_on_disk() {
        return true;
    }
    windows_game_assets_present_in(&default_asset_search_roots())
}

/// Asset probe over caller-supplied roots (unit tests inject a temp dir).
///
/// A root counts when it contains a Lone Eagle candidate file **or** a MapsZH
/// directory / `MapsZH.big` (directly or under the usual `windows_game/...`
/// extract paths).
pub fn windows_game_assets_present_in(roots: &[impl AsRef<Path>]) -> bool {
    for root in roots {
        let root = root.as_ref();
        for rel in LONE_EAGLE_CANDIDATES {
            let candidate = root.join(rel);
            if candidate.is_file() {
                return true;
            }
        }
        for rel in MAPSZH_CANDIDATES {
            let candidate = root.join(rel);
            if candidate.is_file() || candidate.is_dir() {
                return true;
            }
        }
    }
    false
}

fn default_asset_search_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(cwd.clone());
        roots.push(cwd.join(".."));
        roots.push(cwd.join("../.."));
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    roots.push(manifest.clone());
    roots.push(manifest.join(".."));
    roots.push(manifest.join("../.."));
    roots.push(manifest.join("../../.."));
    roots
}

/// Evaluate the 8-step windowed sit-through against a completed smoke result.
///
/// Probes real disk for `windows_game` assets. Prefer
/// [`evaluate_windowed_acceptance_with_assets`] in unit tests so the result
/// does not depend on the developer tree.
pub fn evaluate_windowed_acceptance(r: &ExecutableSmokeResult) -> WindowedAcceptanceReport {
    evaluate_windowed_acceptance_with_assets(r, windows_game_assets_present())
}

/// Same rules as [`evaluate_windowed_acceptance`], with an injected asset flag.
///
/// Does **not** require or mutate `r.playable_claim`. Does **not** treat
/// `r.executable_host_ok` as a pass. It does require `r.windowed_launch` so
/// copied/headless smoke booleans cannot produce a green acceptance result.
pub fn evaluate_windowed_acceptance_with_assets(
    r: &ExecutableSmokeResult,
    assets_present: bool,
) -> WindowedAcceptanceReport {
    let windowed_launch = r.windowed_launch;
    let visible_window = r.window_visible;
    let physical_main_menu_skirmish_nav = r.wnd_widget_tree_nav;
    let wgpu_presented_frame = r.live_frame_ok;
    let non_shell_map = r.reached_ingame && is_non_shell_map(&r.map_seen);
    let select_move_attack = r.interactive_gameplay;
    // Runtime-host command results are diagnostics only. They have no physical
    // input provenance, so accepting them here would let the smoke driver
    // certify its own scripted activity as a playable session.
    let host_build_and_produce_diagnostic = r.construct_cmd_ok && r.train_cmd_ok;
    let host_gather_resources_diagnostic = r.return_supplies_cmd_ok;
    let host_save_load_continue_diagnostic = r.save_cmd_ok && r.load_cmd_ok;
    let build_and_produce = r.physical_build_and_produce;
    let gather_resources = r.physical_gather_resources;
    let save_load_continue = r.physical_save_load_continue;

    let steps = [
        (WINDOWED_ACCEPTANCE_STEP_NAMES[0], visible_window),
        (
            WINDOWED_ACCEPTANCE_STEP_NAMES[1],
            physical_main_menu_skirmish_nav,
        ),
        (WINDOWED_ACCEPTANCE_STEP_NAMES[2], wgpu_presented_frame),
        (WINDOWED_ACCEPTANCE_STEP_NAMES[3], non_shell_map),
        (WINDOWED_ACCEPTANCE_STEP_NAMES[4], select_move_attack),
        (WINDOWED_ACCEPTANCE_STEP_NAMES[5], build_and_produce),
        (WINDOWED_ACCEPTANCE_STEP_NAMES[6], gather_resources),
        (WINDOWED_ACCEPTANCE_STEP_NAMES[7], save_load_continue),
    ];

    let steps_ok = steps.iter().filter(|(_, ok)| *ok).count() as u8;
    let all_eight = steps_ok as usize == WINDOWED_ACCEPTANCE_STEP_COUNT;

    let mut missing: Vec<String> = steps
        .iter()
        .filter(|(_, ok)| !*ok)
        .map(|(name, _)| (*name).to_string())
        .collect();
    if !windowed_launch {
        missing.push(WINDOWED_LAUNCH_MISSING.to_string());
    }
    if !assets_present {
        missing.push(WINDOWS_GAME_ASSETS_MISSING.to_string());
    }

    let (status, exit_code, passed) = classify_windowed_acceptance(
        r.status.as_str(),
        windowed_launch,
        all_eight,
        assets_present,
    );

    WindowedAcceptanceReport {
        windowed_launch,
        visible_window,
        physical_main_menu_skirmish_nav,
        wgpu_presented_frame,
        non_shell_map,
        select_move_attack,
        build_and_produce,
        gather_resources,
        save_load_continue,
        assets_present,
        playable_claim: r.playable_claim,
        executable_host_ok: r.executable_host_ok,
        host_build_and_produce_diagnostic,
        host_gather_resources_diagnostic,
        host_save_load_continue_diagnostic,
        missing,
        status,
        exit_code,
        passed,
        steps_ok,
        smoke_status: r.status.clone(),
        smoke_detail: r.detail.clone(),
        map_seen: r.map_seen.clone(),
    }
}

/// Map smoke status + step/asset truth onto the distinct gate exits.
///
/// A proven *windowed* 8/8 + assets always wins (exit 0), even if smoke's
/// status string is stale. `executable_host_ok` is intentionally unused.
fn classify_windowed_acceptance(
    smoke_status: &str,
    windowed_launch: bool,
    all_eight: bool,
    assets_present: bool,
) -> (String, i32, bool) {
    if windowed_launch && all_eight && assets_present {
        return ("pass".into(), 0, true);
    }
    if smoke_status == "binary_missing" {
        return ("binary_missing".into(), 3, false);
    }
    let assets_or_display = !assets_present
        || matches!(
            smoke_status,
            "assets_or_display_unavailable" | "spawn_failed" | "no_menu"
        );
    if assets_or_display {
        return ("assets_or_display_unavailable".into(), 4, false);
    }
    ("sit_through_incomplete".into(), 1, false)
}

/// Human + grep-friendly report. `playable_claim` and `host_ok` are labeled
/// as reported-only so a green `host_ok` cannot be mistaken for PASS.
pub fn format_windowed_acceptance_report(report: &WindowedAcceptanceReport) -> String {
    let missing = if report.missing.is_empty() {
        "-".to_string()
    } else {
        report.missing.join(",")
    };
    format!(
        "windowed_acceptance status={} passed={} exit={} steps={}/{} missing={} \
         windowed_launch={} (required; headless/synthetic results cannot pass) \
         playable_claim={} (reported only; not required; evaluate never sets it) \
         host_ok={} (never sufficient for PASS) assets={} map={} smoke_status={} detail={}\n\
         host_workflow_diagnostic build_and_produce={} gather_resources={} save_load_continue={} (not acceptance evidence)\n\
         1 visible_window={}\n\
         2 physical_main_menu_skirmish_nav={}\n\
         3 wgpu_presented_frame={}\n\
         4 non_shell_map={}\n\
         5 select_move_attack={}\n\
         6 build_and_produce={}\n\
         7 gather_resources={}\n\
         8 save_load_continue={}",
        report.status,
        report.passed,
        report.exit_code,
        report.steps_ok,
        WINDOWED_ACCEPTANCE_STEP_COUNT,
        missing,
        report.windowed_launch,
        report.playable_claim,
        report.executable_host_ok,
        report.assets_present,
        report.map_seen,
        report.smoke_status,
        report.smoke_detail,
        report.host_build_and_produce_diagnostic,
        report.host_gather_resources_diagnostic,
        report.host_save_load_continue_diagnostic,
        report.visible_window,
        report.physical_main_menu_skirmish_nav,
        report.wgpu_presented_frame,
        report.non_shell_map,
        report.select_move_attack,
        report.build_and_produce,
        report.gather_resources,
        report.save_load_continue,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executable_smoke::ExecutableSmokeResult;
    use std::fs;

    fn passing_smoke() -> ExecutableSmokeResult {
        let mut r = ExecutableSmokeResult::default();
        r.status = "success".into();
        r.detail = "sit-through fixture".into();
        r.window_visible = true;
        r.wnd_widget_tree_nav = true;
        r.main_menu_skirmish_wnd_ok = true;
        r.live_frame_ok = true;
        r.reached_ingame = true;
        r.map_seen = "Lone Eagle".into();
        r.gameplay_cmd_ok = true;
        r.combat_damage_ok = true;
        r.interactive_gameplay = true;
        r.construct_cmd_ok = true;
        r.train_cmd_ok = true;
        r.physical_build_and_produce = true;
        r.return_supplies_cmd_ok = true;
        r.physical_gather_resources = true;
        r.save_cmd_ok = true;
        r.load_cmd_ok = true;
        r.physical_save_load_continue = true;
        r.executable_host_ok = true;
        r.playable_claim = false;
        r.windowed_launch = true;
        r
    }

    #[test]
    fn default_smoke_is_not_pass_and_lists_all_eight() {
        let r = ExecutableSmokeResult::default();
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(!report.passed);
        assert_ne!(report.exit_code, 0);
        assert_eq!(report.steps_ok, 0);
        assert_eq!(report.exit_code, 1);
        assert_eq!(report.status, "sit_through_incomplete");
        assert!(!report.playable_claim);
        for name in WINDOWED_ACCEPTANCE_STEP_NAMES {
            assert!(
                report.missing.iter().any(|m| m == name),
                "default must miss {name}: {:?}",
                report.missing
            );
        }
        assert!(
            !report
                .missing
                .iter()
                .any(|m| m == WINDOWS_GAME_ASSETS_MISSING)
        );
    }

    #[test]
    fn default_evaluate_without_injection_is_never_pass() {
        let r = ExecutableSmokeResult::default();
        let report = evaluate_windowed_acceptance(&r);
        assert!(!report.passed);
        assert_ne!(report.exit_code, 0);
        assert_eq!(report.steps_ok, 0);
        for name in WINDOWED_ACCEPTANCE_STEP_NAMES {
            assert!(
                report.missing.iter().any(|m| m == name),
                "evaluate() default must miss {name}"
            );
        }
    }

    #[test]
    fn each_step_independently_required() {
        for (idx, name) in WINDOWED_ACCEPTANCE_STEP_NAMES.iter().enumerate() {
            let mut r = passing_smoke();
            match idx {
                0 => r.window_visible = false,
                1 => r.wnd_widget_tree_nav = false,
                2 => r.live_frame_ok = false,
                3 => r.reached_ingame = false,
                4 => r.interactive_gameplay = false,
                5 => r.physical_build_and_produce = false,
                6 => r.physical_gather_resources = false,
                7 => r.physical_save_load_continue = false,
                _ => unreachable!(),
            }
            let report = evaluate_windowed_acceptance_with_assets(&r, true);
            assert!(!report.passed, "flipping {name} must not pass");
            assert_eq!(
                report.exit_code, 1,
                "flipping {name} is sit-through incomplete"
            );
            assert_eq!(
                report.missing,
                vec![(*name).to_string()],
                "flipping {name} must list only that step"
            );
            assert_eq!(report.steps_ok, 7);
        }
    }

    #[test]
    fn nav_step_is_physical_wnd_widget_tree_nav() {
        let mut r = passing_smoke();
        r.wnd_widget_tree_nav = false;
        r.main_menu_skirmish_wnd_ok = true;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert_eq!(
            report.missing,
            vec!["physical_main_menu_skirmish_nav".to_string()]
        );
        assert!(!report.physical_main_menu_skirmish_nav);
    }

    #[test]
    fn nav_step_ignores_host_skirmish_wnd_residual() {
        let mut r = passing_smoke();
        r.main_menu_skirmish_wnd_ok = false;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(
            report.physical_main_menu_skirmish_nav,
            "host ButtonSkirmish residual must not gate physical nav"
        );
        assert!(report.passed);
    }

    #[test]
    fn combat_step_requires_physical_interactive_gameplay() {
        let mut r = passing_smoke();
        r.interactive_gameplay = false;
        r.gameplay_cmd_ok = true;
        r.combat_damage_ok = true;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert_eq!(report.missing, vec!["select_move_attack".to_string()]);
        assert!(
            !report.select_move_attack,
            "host-command combat must not satisfy windowed select_move_attack"
        );
    }

    #[test]
    fn produce_step_requires_physical_control_bar_evidence() {
        let mut r = passing_smoke();
        r.physical_build_and_produce = false;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert_eq!(report.missing, vec!["build_and_produce".to_string()]);
    }

    #[test]
    fn persist_step_requires_physical_popup_save_load_evidence() {
        let mut r = passing_smoke();
        r.physical_save_load_continue = false;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert_eq!(report.missing, vec!["save_load_continue".to_string()]);
    }

    #[test]
    fn host_workflow_diagnostics_never_satisfy_physical_steps() {
        let mut r = passing_smoke();
        r.physical_build_and_produce = false;
        r.physical_gather_resources = false;
        r.physical_save_load_continue = false;
        // Leave all runtime-host command results green to prove they are
        // report-only diagnostics, not a backdoor into the acceptance result.
        r.construct_cmd_ok = true;
        r.train_cmd_ok = true;
        r.return_supplies_cmd_ok = true;
        r.save_cmd_ok = true;
        r.load_cmd_ok = true;

        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(!report.passed);
        assert_eq!(report.steps_ok, 5);
        assert_eq!(
            report.missing,
            vec![
                "build_and_produce".to_string(),
                "gather_resources".to_string(),
                "save_load_continue".to_string(),
            ]
        );
        assert!(report.host_build_and_produce_diagnostic);
        assert!(report.host_gather_resources_diagnostic);
        assert!(report.host_save_load_continue_diagnostic);
    }

    #[test]
    fn non_shell_map_rejects_shellmap_dash_and_empty() {
        for bad in [
            "shellmap",
            "Maps/ShellMap.map",
            "SHELLMAP_USA",
            "-",
            "",
            "   ",
        ] {
            let mut r = passing_smoke();
            r.map_seen = bad.into();
            r.reached_ingame = true;
            let report = evaluate_windowed_acceptance_with_assets(&r, true);
            assert!(
                report.missing.iter().any(|m| m == "non_shell_map"),
                "map {bad:?} must fail step 4: {:?}",
                report.missing
            );
            assert!(!report.non_shell_map);
            assert!(!report.passed);
        }
    }

    #[test]
    fn non_shell_map_requires_reached_ingame() {
        let mut r = passing_smoke();
        r.reached_ingame = false;
        r.map_seen = "Lone Eagle".into();
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(report.missing.iter().any(|m| m == "non_shell_map"));
        assert!(!is_non_shell_map("-"));
        assert!(!is_non_shell_map(""));
        assert!(!is_non_shell_map("shellmap"));
        assert!(is_non_shell_map("Lone Eagle"));
        assert!(is_non_shell_map(
            "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map"
        ));
    }

    #[test]
    fn all_eight_plus_assets_passes_with_playable_claim_false() {
        let r = passing_smoke();
        assert!(!r.playable_claim);
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(report.passed);
        assert_eq!(report.exit_code, 0);
        assert_eq!(report.status, "pass");
        assert_eq!(report.steps_ok, 8);
        assert!(report.missing.is_empty());
        assert!(
            !report.playable_claim,
            "evaluate must not set playable_claim"
        );
        assert_eq!(report.playable_claim, r.playable_claim);
    }

    #[test]
    fn playable_claim_true_is_reported_but_not_required() {
        let mut r = passing_smoke();
        r.playable_claim = true;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(report.passed);
        assert!(report.playable_claim);
        assert_eq!(report.exit_code, 0);

        r.window_visible = false;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(
            report.playable_claim,
            "evaluate copies playable_claim; does not clear it"
        );
        assert!(
            !report.passed,
            "playable_claim must not substitute a missing step"
        );
        assert_eq!(report.missing, vec!["visible_window".to_string()]);
    }

    #[test]
    fn host_ok_is_never_sufficient_for_pass() {
        let mut r = ExecutableSmokeResult::default();
        r.status = "success".into();
        r.executable_host_ok = true;
        r.playable_claim = false;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(report.executable_host_ok);
        assert!(!report.passed);
        assert_eq!(report.exit_code, 1);
        assert_eq!(report.status, "sit_through_incomplete");
        assert_eq!(report.steps_ok, 0);
    }

    #[test]
    fn binary_missing_is_exit_3() {
        let mut r = ExecutableSmokeResult::default();
        r.status = "binary_missing".into();
        r.detail = "generals binary not found".into();
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(!report.passed);
        assert_eq!(report.exit_code, 3);
        assert_eq!(report.status, "binary_missing");
    }

    #[test]
    fn assets_or_display_unavailable_is_exit_4() {
        for status in ["assets_or_display_unavailable", "spawn_failed", "no_menu"] {
            let mut r = ExecutableSmokeResult::default();
            r.status = status.into();
            let report = evaluate_windowed_acceptance_with_assets(&r, true);
            assert!(!report.passed, "{status}");
            assert_eq!(report.exit_code, 4, "{status}");
            assert_eq!(report.status, "assets_or_display_unavailable", "{status}");
        }
    }

    #[test]
    fn missing_assets_is_exit_4_even_when_steps_are_green() {
        let r = passing_smoke();
        let report = evaluate_windowed_acceptance_with_assets(&r, false);
        assert!(!report.passed);
        assert_eq!(report.exit_code, 4);
        assert_eq!(report.status, "assets_or_display_unavailable");
        assert_eq!(report.steps_ok, 8);
        assert_eq!(
            report.missing,
            vec![WINDOWS_GAME_ASSETS_MISSING.to_string()]
        );
        assert!(!report.assets_present);
    }

    #[test]
    fn proven_sit_through_overrides_stale_smoke_status() {
        let mut r = passing_smoke();
        r.status = "assets_or_display_unavailable".into();
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert!(
            report.passed,
            "8/8 + assets is PASS even if smoke status is stale"
        );
        assert_eq!(report.exit_code, 0);
    }

    #[test]
    fn copied_headless_or_synthetic_flags_can_never_pass() {
        let mut r = passing_smoke();
        r.windowed_launch = false;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert_eq!(report.steps_ok, 8, "the individual flags may be copied");
        assert!(!report.passed, "provenance is a separate mandatory guard");
        assert_eq!(report.exit_code, 1);
        assert_eq!(report.status, "sit_through_incomplete");
        assert_eq!(
            report.missing,
            vec![WINDOWED_LAUNCH_MISSING.to_string()],
            "a synthetic/headless result must disclose missing windowed provenance"
        );
    }

    #[test]
    fn evaluate_does_not_mutate_or_require_playable_claim() {
        let mut r = passing_smoke();
        r.playable_claim = false;
        let before = r.playable_claim;
        let report = evaluate_windowed_acceptance_with_assets(&r, true);
        assert_eq!(r.playable_claim, before);
        assert_eq!(report.playable_claim, before);
        assert!(report.passed);
        let src = include_str!("windowed_acceptance.rs");
        let prod = src.split("#[cfg(test)]").next().expect("prod");
        assert!(
            !prod.contains("playable_claim = true"),
            "evaluate must not assign playable_claim = true"
        );
        assert!(!prod.contains("r.playable_claim ="));
        assert!(!prod.contains("playable_claim |= "));
    }

    #[test]
    fn format_report_labels_reported_only_fields() {
        let report = evaluate_windowed_acceptance_with_assets(&passing_smoke(), true);
        let text = format_windowed_acceptance_report(&report);
        assert!(text.contains("windowed_acceptance status=pass"));
        assert!(text.contains("passed=true"));
        assert!(text.contains("exit=0"));
        assert!(text.contains("steps=8/8"));
        assert!(text.contains("windowed_launch=true (required; headless/synthetic"));
        assert!(text.contains("playable_claim=false (reported only; not required"));
        assert!(text.contains("host_ok=true (never sufficient for PASS)"));
        assert!(text.contains("host_workflow_diagnostic build_and_produce=true"));
        assert!(text.contains("1 visible_window=true"));
        assert!(text.contains("8 save_load_continue=true"));
        assert!(text.contains("missing=-"));
    }

    #[test]
    fn assets_present_false_on_empty_or_blank_roots() {
        let empty: &[&Path] = &[];
        assert!(!windows_game_assets_present_in(empty));
        let tmp = tempfile::tempdir().unwrap();
        assert!(!windows_game_assets_present_in(&[tmp.path()]));
    }

    #[test]
    fn assets_present_true_on_injected_lone_eagle_file() {
        let tmp = tempfile::tempdir().unwrap();
        let rel = "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle";
        let dir = tmp.path().join(rel);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("Lone Eagle.map"), b"fake-map").unwrap();
        assert!(windows_game_assets_present_in(&[tmp.path()]));
    }

    #[test]
    fn assets_present_true_on_injected_mapszh_dir() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("windows_game/extracted_big_files/MapsZH")).unwrap();
        assert!(windows_game_assets_present_in(&[tmp.path()]));
    }

    #[test]
    fn assets_present_true_on_injected_mapszh_big() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp
            .path()
            .join("windows_game/Command & Conquer Generals Zero Hour");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("MapsZH.big"), b"fake-big").unwrap();
        assert!(windows_game_assets_present_in(&[tmp.path()]));
    }

    #[test]
    fn assets_helper_uses_same_lone_eagle_candidates_as_smoke() {
        let src = include_str!("windowed_acceptance.rs");
        assert!(src.contains("LONE_EAGLE_CANDIDATES"));
        assert!(src.contains("lone_eagle_map_on_disk"));
        assert_eq!(
            LONE_EAGLE_CANDIDATES[0],
            "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map"
        );
        // Smoke const is what we walk; do not fork a second list.
        let smoke = include_str!("executable_smoke.rs");
        assert!(smoke.contains("pub const LONE_EAGLE_CANDIDATES"));
    }

    #[test]
    fn windows_game_assets_present_does_not_panic() {
        let _ = windows_game_assets_present();
    }

    #[test]
    fn windowed_load_to_ingame_is_not_a_noop() {
        // The shipped windowed host must actually call start_game_from_ui and
        // finish Loading → InGame. A comment is not a transition.
        let start = include_str!("cnc_game_engine/start_game.rs");
        assert!(
            start.contains("fn host_start_game_from_ui"),
            "match start lives on host_start_game_from_ui"
        );
        assert!(
            start.contains("self.abandon_startup_load_worker()"),
            "start_game must abandon a stuck boot worker"
        );
        assert!(
            start.contains("self.transition_to_state(GameState::Loading)")
                && start.contains("self.transition_to_state(GameState::InGame)"),
            "start_game_from_ui must leave Loading and reach InGame"
        );
        assert!(
            start.contains("self.seed_presentation_after_match_start()"),
            "finish_load / presentation seed must run before InGame"
        );

        let skirmish = include_str!("cnc_game_engine/runtime_host/skirmish.rs");
        let start_cmd = skirmish
            .split("fn runtime_host_cmd_start_game")
            .nth(1)
            .expect("runtime_host_cmd_start_game");
        assert!(
            start_cmd.contains("self.start_game_from_ui("),
            "windowed host start_game command must call start_game_from_ui, not a comment"
        );
        assert!(
            !start_cmd.contains("spawn_dozer")
                && !start_cmd.contains("force_complete")
                && !start_cmd.contains("grant_supplies"),
            "windowed start_game must not cheat"
        );

        let loading = include_str!("cnc_game_engine/camera_drain.rs");
        let loading_fn = loading
            .split("fn host_tick_loading_client_residuals")
            .nth(1)
            .and_then(|s| s.split("pub(super) fn host_tick_paused").next())
            .expect("loading tick bounded to next helper");
        assert!(
            loading_fn.contains("take_pending_new_game_start_request")
                && loading_fn.contains("self.start_game_from_ui("),
            "Loading must drain NewGame into start_game_from_ui"
        );
        assert!(
            loading_fn.contains("startup_load_should_release_to_menu")
                && loading_fn.contains("GameState::Menu"),
            "stuck boot Loading must release to Menu"
        );
        assert!(
            !loading_fn.contains("fn host_tick_paused_client_residuals"),
            "Loading window must not include later helpers"
        );

        let smoke = include_str!("executable_smoke.rs");
        let observer = smoke
            .split("pub fn run_windowed_acceptance_smoke")
            .nth(1)
            .and_then(|s| {
                s.split("fn run_executable_smoke_with_launch_and_driver")
                    .next()
            })
            .expect("windowed acceptance observer entrypoint");
        assert!(
            observer.contains("SmokeDriver::ManualObserver"),
            "the acceptance runner must observe physical play, not drive it"
        );
        let manual_phase = smoke
            .split("21 => {")
            .nth(1)
            .and_then(|s| s.split("20 => {").next())
            .expect("manual observation phase");
        assert!(
            !manual_phase.contains("write_control"),
            "manual observation must not send synthetic gameplay/control commands: {manual_phase}"
        );

        let abandon = include_str!("cnc_game_engine/shell.rs");
        assert!(
            abandon.contains("bump_startup_worker_generation()")
                && abandon.contains("startup_worker_owns(worker_gen)"),
            "abandon must invalidate the boot worker generation"
        );
        let drain = include_str!("cnc_game_engine/dispatch.rs");
        let take = drain
            .split("fn take_startup_messages_from_stream")
            .nth(1)
            .and_then(|s| {
                s.split("pub(super) fn apply_startup_new_game_dispatch")
                    .next()
            })
            .expect("take_startup bounded");
        assert!(
            take.contains("startup_worker_owns(owner_gen)") && take.contains("skip stream clear"),
            "abandoned worker must not clear NewGame from the stream"
        );
    }

    #[test]
    fn gate_bin_does_not_poison_parent_env() {
        let src = include_str!("bin/windowed_acceptance_gate.rs");
        assert!(
            !src.contains("std::env::set_var"),
            "windowed gate must not mutate parent env (races tests)"
        );
        assert!(
            src.contains("evaluate_windowed_acceptance"),
            "bin must call library evaluate"
        );
        assert!(
            src.contains("format_windowed_acceptance_report"),
            "bin must print library report"
        );
        assert!(
            src.contains("run_windowed_acceptance_smoke"),
            "bin must use the windowed launcher, not headless executable smoke"
        );
        assert!(
            !src.contains("run_executable_smoke("),
            "bin must not call the headless host smoke entry"
        );
        assert!(
            !src.contains("-runtime_host=headless"),
            "windowed gate must never request headless host"
        );
        assert!(
            src.contains("display_or_wgpu_adapter_available"),
            "bin must probe display/wgpu before claiming a sit-through"
        );
        assert!(
            src.contains("assets_or_display_unavailable"),
            "bin must name the first-class unavailable status"
        );
    }

    struct FakeDisplayProbe {
        force_off: bool,
        force_on: bool,
        display: bool,
        adapter: bool,
    }

    impl DisplayAvailabilityProbe for FakeDisplayProbe {
        fn force_unavailable(&self) -> bool {
            self.force_off
        }
        fn force_available(&self) -> bool {
            self.force_on
        }
        fn has_os_display(&self) -> bool {
            self.display
        }
        fn has_wgpu_adapter(&self) -> bool {
            self.adapter
        }
    }

    #[test]
    fn display_or_wgpu_adapter_available_probe_is_unit_tested() {
        let none = FakeDisplayProbe {
            force_off: false,
            force_on: false,
            display: false,
            adapter: false,
        };
        assert!(!display_or_wgpu_adapter_available_with(&none));
        assert_eq!(display_unavailable_status(), ASSETS_OR_DISPLAY_UNAVAILABLE);

        let display_only = FakeDisplayProbe {
            force_off: false,
            force_on: false,
            display: true,
            adapter: false,
        };
        assert!(display_or_wgpu_adapter_available_with(&display_only));

        let adapter_only = FakeDisplayProbe {
            force_off: false,
            force_on: false,
            display: false,
            adapter: true,
        };
        assert!(display_or_wgpu_adapter_available_with(&adapter_only));

        let forced_off = FakeDisplayProbe {
            force_off: true,
            display: true,
            adapter: true,
            force_on: false,
        };
        assert!(
            !display_or_wgpu_adapter_available_with(&forced_off),
            "force-unavailable must not fake a presentable adapter"
        );

        let forced_on = FakeDisplayProbe {
            force_on: true,
            force_off: false,
            display: false,
            adapter: false,
        };
        assert!(display_or_wgpu_adapter_available_with(&forced_on));
    }

    #[test]
    fn display_probe_does_not_panic() {
        let _ = display_or_wgpu_adapter_available();
    }

    #[test]
    fn missing_display_is_not_a_pass() {
        let r = ExecutableSmokeResult::default();
        let report = evaluate_windowed_acceptance_with_assets(&r, false);
        assert!(!report.passed);
        assert_eq!(report.exit_code, 4);
        assert_eq!(report.status, ASSETS_OR_DISPLAY_UNAVAILABLE);
    }
}
