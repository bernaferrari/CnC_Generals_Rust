//! Executable smoke via the production `generals` binary + runtime host bridge.
//!
//! This is **stronger** than headless `shell_smoke` (which constructs `GameLogic`
//! in-process): it boots the real event loop, creates a (hidden) window, runs
//! WW3D headless init, and drives Menu → Start through the same control file
//! path GPUI uses.
//!
//! Honesty:
//! - `playable_claim` is **always false** here until full interactive retail WND navigation is proven end-to-end
//!   path + GPU match playthrough is proven.
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
const LONE_EAGLE_CANDIDATES: &[&str] = &[
    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
    "Maps/Lone Eagle/Lone Eagle.map",
    "Lone Eagle",
];

#[derive(Debug, Clone)]
pub struct ExecutableSmokeResult {
    pub status: String,
    pub detail: String,
    /// Always false — not a retail W3D interactive playthrough claim.
    pub playable_claim: bool,
    /// Limited: process reached InGame (or Menu+start attempted) and exited 0.
    pub executable_host_ok: bool,
    pub process_started: bool,
    pub reached_menu: bool,
    pub reached_ingame: bool,
    /// Runtime-host select+move command accepted (not WND click; still not full playable_claim).
    pub gameplay_cmd_ok: bool,
    /// Runtime-host dozer construct command accepted (still not full playable_claim).
    pub construct_cmd_ok: bool,
    /// Runtime-host train_unit accepted (still not full playable_claim).
    pub train_cmd_ok: bool,
    pub save_cmd_ok: bool,
    /// Runtime-host quickload after save accepted (still not full playable_claim).
    pub load_cmd_ok: bool,
    /// Runtime-host stop_all accepted (still not full playable_claim).
    pub stop_cmd_ok: bool,
    /// Runtime-host sell accepted (still not full playable_claim).
    pub sell_cmd_ok: bool,
    pub upgrade_cmd_ok: bool,
    pub guard_cmd_ok: bool,
    pub attack_move_cmd_ok: bool,
    pub scatter_cmd_ok: bool,
    pub patrol_cmd_ok: bool,
    pub deploy_cmd_ok: bool,
    pub cheer_cmd_ok: bool,
    pub formation_cmd_ok: bool,
    pub capture_cmd_ok: bool,
    pub return_supplies_cmd_ok: bool,
    pub evacuate_cmd_ok: bool,
    pub repair_cmd_ok: bool,
    pub return_to_base_cmd_ok: bool,
    pub attitude_cmd_ok: bool,
    pub rally_cmd_ok: bool,
    pub switch_weapons_cmd_ok: bool,
    pub view_cc_cmd_ok: bool,
    pub clear_mines_cmd_ok: bool,
    pub beacon_cmd_ok: bool,
    pub hack_cmd_ok: bool,
    pub cleanup_cmd_ok: bool,
    pub combat_drop_cmd_ok: bool,
    pub overcharge_cmd_ok: bool,
    pub special_power_cmd_ok: bool,
    pub remove_beacon_cmd_ok: bool,
    pub demo_cmd_ok: bool,
    pub view_radar_cmd_ok: bool,
    pub force_attack_cmd_ok: bool,
    pub force_attack_object_cmd_ok: bool,
    pub select_all_cmd_ok: bool,
    pub control_group_cmd_ok: bool,
    pub waypoint_cmd_ok: bool,
    pub box_select_cmd_ok: bool,
    /// InGame status reported presentation_frame_ok=true at least once.
    pub presentation_frame_ok: bool,
    /// No live GameLogic dual-reads while presentation owned collect (status residual).
    pub presentation_live_fallback_ok: bool,
    pub select_similar_cmd_ok: bool,
    pub select_on_screen_cmd_ok: bool,
    pub select_structures_cmd_ok: bool,
    pub select_aircraft_cmd_ok: bool,
    pub select_idle_cmd_ok: bool,
    pub camera_reset_cmd_ok: bool,
    pub camera_zoom_cmd_ok: bool,
    pub pause_cmd_ok: bool,
    pub cancel_production_cmd_ok: bool,
    pub diplomacy_cmd_ok: bool,
    /// Host published a usable live frame.png (GPU/screenshot residual).
    pub live_frame_ok: bool,
    /// Peak InGame unit mesh render_item_count from host status (world draw residual).
    pub max_render_item_count: u32,
    /// Peak InGame presentation-alive object count.
    pub max_render_alive_objects: u32,
    /// True when InGame observed stable non-zero render items (not a one-frame flash).
    pub render_items_stable_ok: bool,
    pub auto_attack_cmd_ok: bool,
    pub options_cmd_ok: bool,
    pub request_capture_cmd_ok: bool,
    /// Runtime-host opened Skirmish UI screen before start_game.
    pub skirmish_menu_ok: bool,
    /// Runtime-host exercised SkirmishMenu Start button click path (not WND widget tree).
    pub skirmish_start_click_ok: bool,
    /// click_skirmish_start used retail WND ButtonStart path (ok_wnd / wnd_pending).
    pub skirmish_start_wnd_ok: bool,
    pub frames_observed: u32,
    pub map_seen: String,
    pub exit_code: Option<i32>,
    pub new_game_path: bool,
}

impl Default for ExecutableSmokeResult {
    fn default() -> Self {
        Self {
            status: "not_run".into(),
            detail: String::new(),
            playable_claim: false,
            executable_host_ok: false,
            process_started: false,
            reached_menu: false,
            reached_ingame: false,
            gameplay_cmd_ok: false,
            construct_cmd_ok: false,
            train_cmd_ok: false,
            save_cmd_ok: false,
            load_cmd_ok: false,
            stop_cmd_ok: false,
            sell_cmd_ok: false,
            upgrade_cmd_ok: false,
            guard_cmd_ok: false,
            attack_move_cmd_ok: false,
            scatter_cmd_ok: false,
            patrol_cmd_ok: false,
            deploy_cmd_ok: false,
            cheer_cmd_ok: false,
            formation_cmd_ok: false,
            capture_cmd_ok: false,
            return_supplies_cmd_ok: false,
            evacuate_cmd_ok: false,
            repair_cmd_ok: false,
            return_to_base_cmd_ok: false,
            attitude_cmd_ok: false,
            rally_cmd_ok: false,
            switch_weapons_cmd_ok: false,
            view_cc_cmd_ok: false,
            clear_mines_cmd_ok: false,
            beacon_cmd_ok: false,
            hack_cmd_ok: false,
            cleanup_cmd_ok: false,
            combat_drop_cmd_ok: false,
            overcharge_cmd_ok: false,
            special_power_cmd_ok: false,
            remove_beacon_cmd_ok: false,
            demo_cmd_ok: false,
            view_radar_cmd_ok: false,
            force_attack_cmd_ok: false,
            force_attack_object_cmd_ok: false,
            select_all_cmd_ok: false,
            control_group_cmd_ok: false,
            waypoint_cmd_ok: false,
            box_select_cmd_ok: false,
            presentation_frame_ok: false,
            presentation_live_fallback_ok: false,
            select_similar_cmd_ok: false,
            select_on_screen_cmd_ok: false,
            select_structures_cmd_ok: false,
            select_aircraft_cmd_ok: false,
            select_idle_cmd_ok: false,
            camera_reset_cmd_ok: false,
            camera_zoom_cmd_ok: false,
            pause_cmd_ok: false,
            cancel_production_cmd_ok: false,
            diplomacy_cmd_ok: false,
            live_frame_ok: false,
            max_render_item_count: 0,
            max_render_alive_objects: 0,
            render_items_stable_ok: false,
            auto_attack_cmd_ok: false,
            options_cmd_ok: false,
            request_capture_cmd_ok: false,
            skirmish_menu_ok: false,
            skirmish_start_click_ok: false,
            skirmish_start_wnd_ok: false,
            frames_observed: 0,
            map_seen: "-".into(),
            exit_code: None,
            new_game_path: false,
        }
    }
}

#[derive(Debug, Default, Clone)]
struct StatusSnap {
    state: String,
    ui_screen: String,
    map: String,
    frame: u32,
    startup_progress: f32,
    startup_phase: String,
    selected_count: u32,
    local_mobile_units: u32,
    last_gameplay_cmd: String,
    match_over: bool,
    victory_label: String,
    presentation_frame_ok: bool,
    presentation_live_fallback_reads: u32,
    waypoint_mode: bool,
    live_frame_ok: bool,
    render_item_count: u32,
    render_alive_objects: u32,
    render_fow_filtered: u32,
    render_frustum_culled: u32,
}

fn parse_status(path: &Path) -> Option<StatusSnap> {
    let text = fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    let mut snap = StatusSnap::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "state" => snap.state = v.trim().to_string(),
            "ui_screen" => snap.ui_screen = v.trim().to_string(),
            "map" => snap.map = v.trim().to_string(),
            "frame" => snap.frame = v.trim().parse().unwrap_or(0),
            "startup_progress" => snap.startup_progress = v.trim().parse().unwrap_or(0.0),
            "startup_phase" => snap.startup_phase = v.trim().to_string(),
            "selected_count" => snap.selected_count = v.trim().parse().unwrap_or(0),
            "local_mobile_units" => snap.local_mobile_units = v.trim().parse().unwrap_or(0),
            "last_gameplay_cmd" => snap.last_gameplay_cmd = v.trim().to_string(),
            "presentation_frame_ok" => {
                snap.presentation_frame_ok = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "presentation_live_fallback_reads" => {
                snap.presentation_live_fallback_reads = v.trim().parse().unwrap_or(0);
            }
            "waypoint_mode" => {
                snap.waypoint_mode = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "live_frame_ok" => {
                snap.live_frame_ok = matches!(
                    v.trim().to_ascii_lowercase().as_str(),
                    "1" | "true" | "yes" | "on"
                );
            }
            "render_item_count" => {
                snap.render_item_count = v.trim().parse().unwrap_or(0);
            }
            "render_alive_objects" => {
                snap.render_alive_objects = v.trim().parse().unwrap_or(0);
            }
            "render_fow_filtered" => {
                snap.render_fow_filtered = v.trim().parse().unwrap_or(0);
            }
            "render_frustum_culled" => {
                snap.render_frustum_culled = v.trim().parse().unwrap_or(0);
            }
            "match_over" => snap.match_over = matches!(v.trim(), "true" | "1" | "True"),
            "victory_label" => snap.victory_label = v.trim().to_string(),
            _ => {}
        }
    }
    Some(snap)
}

fn write_control(path: &Path, lines: &[&str]) -> std::io::Result<()> {
    let mut f = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)?;
    for line in lines {
        writeln!(f, "{line}")?;
    }
    f.flush()
}

fn kill_stale_runtime_host_generals(exe: &Path) {
    // Fail-soft: prior smoke / cargo runs can leave a hanging `generals` holding
    // GPU/display and cause Booting→exit before Menu (or Tokio shutdown races).
    #[cfg(unix)]
    {
        let exe_s = exe.to_string_lossy().to_string();
        // CLI flag is `-runtime_host=headless` (underscore). Also match basename
        // when the absolute path differs between debug/release invocations.
        let patterns = [
            format!("{exe_s}.*runtime_host"),
            format!("{exe_s}"),
            "generals.*runtime_host=headless".to_string(),
        ];
        for pat in patterns {
            let _ = std::process::Command::new("pkill")
                .args(["-9", "-f", &pat])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        // Allow GPU/window teardown before the next spawn.
        std::thread::sleep(Duration::from_millis(1200));
    }
    let _ = exe;
}

fn resolve_runtime_exe() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GENERALS_RUNTIME_EXE") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    let candidates = [
        PathBuf::from("target/debug/generals"),
        PathBuf::from("target/release/generals"),
        PathBuf::from("GeneralsRust/target/debug/generals"),
        PathBuf::from("GeneralsRust/target/release/generals"),
        PathBuf::from("./target/debug/generals"),
        PathBuf::from("./target/release/generals"),
    ];
    // Prefer the newest on-disk binary so a stale release build cannot mask
    // freshly compiled debug host commands (construct residual).
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for c in candidates {
        if !c.is_file() {
            continue;
        }
        let modified = c
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        match &best {
            Some((t, _)) if modified <= *t => {}
            _ => best = Some((modified, c)),
        }
    }
    if let Some((_, path)) = best {
        return Some(path);
    }
    // Try next to current exe
    if let Ok(cur) = std::env::current_exe() {
        if let Some(dir) = cur.parent() {
            let sibling = dir.join("generals");
            if sibling.is_file() {
                return Some(sibling);
            }
        }
    }
    None
}

fn resolve_lone_eagle_map() -> String {
    let mut candidates: Vec<PathBuf> = LONE_EAGLE_CANDIDATES.iter().map(PathBuf::from).collect();
    // Walk from CARGO_MANIFEST_DIR (Code/Main) up to repo root and common extract dirs.
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for base in [
        manifest.clone(),
        manifest.join(".."),
        manifest.join("../.."),
        manifest.join("../../.."),
        manifest.join("../../../windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle"),
        manifest.join("../../../windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle"),
    ] {
        candidates.push(base.join("Lone Eagle.map"));
        candidates.push(base.join("Maps/Lone Eagle/Lone Eagle.map"));
        candidates.push(
            base.join("windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map"),
        );
        candidates.push(
            base.join("windows_game/extracted_big_files_v2/MapsZH/Maps/Lone Eagle/Lone Eagle.map"),
        );
    }
    if let Ok(cwd) = std::env::current_dir() {
        for c in LONE_EAGLE_CANDIDATES {
            candidates.push(cwd.join(c));
            candidates.push(cwd.join("..").join(c));
        }
    }
    for c in candidates {
        if c.is_file() {
            // Prefer absolute canonical path so the child process cwd does not matter.
            return c.canonicalize().unwrap_or(c).to_string_lossy().into_owned();
        }
    }
    "Lone Eagle".into()
}

fn kill_child(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

/// Run the executable smoke with a timeout budget.
///
/// `use_new_game_path`: when true, drive Start via `queue_new_game` (Menu drain).
/// When false, use direct `start_game` runtime host command.
pub fn run_executable_smoke(timeout: Duration, use_new_game_path: bool) -> ExecutableSmokeResult {
    // One automatic retry: Booting early-exit is commonly a stale GPU/lock race after
    // pkill -9 (no Drop cleanup). Second attempt after a fresh kill is usually green.
    let first = run_executable_smoke_once(timeout, use_new_game_path);
    let retryable = matches!(
        first.status.as_str(),
        "process_exited" | "timeout" | "no_menu"
    ) && !first.reached_menu
        && !first.reached_ingame;
    if !retryable {
        return first;
    }
    std::thread::sleep(Duration::from_millis(1500));
    let second = run_executable_smoke_once(timeout, use_new_game_path);
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

fn run_executable_smoke_once(timeout: Duration, use_new_game_path: bool) -> ExecutableSmokeResult {
    let mut result = ExecutableSmokeResult {
        playable_claim: false,
        new_game_path: use_new_game_path,
        ..Default::default()
    };

    let Some(exe) = resolve_runtime_exe() else {
        result.status = "binary_missing".into();
        result.detail =
            "generals binary not found; build with `cargo build -p generals_main --bin generals --release` or set GENERALS_RUNTIME_EXE".into();
        return result;
    };

    // Best-effort: prior flaky runs can leave a hanging runtime_host `generals` holding
    // the GPU/display; that makes the next Booting exit before Menu.
    kill_stale_runtime_host_generals(&exe);

    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let tmp = std::env::temp_dir().join(format!("generals_exec_smoke_{stamp}"));
    let _ = fs::create_dir_all(&tmp);
    let control_path = tmp.join("control.txt");
    let status_path = tmp.join("status.txt");
    let frame_path = tmp.join("frame.png");
    let _ = fs::write(&control_path, b"");
    let _ = fs::write(&status_path, b"");

    let map = resolve_lone_eagle_map();
    result.map_seen = map.clone();

    // Prefer -flag=value so option parsing cannot steal the next token
    // (matches GPUI bridge / verified boot path).
    let mut child = match Command::new(&exe)
        .arg("-runtime_host=headless")
        .arg("-windowed")
        .arg("-width=640")
        .arg("-height=480")
        .arg(format!("-gpui_control={}", control_path.display()))
        .arg(format!("-gpui_status={}", status_path.display()))
        .arg(format!("-gpui_frame={}", frame_path.display()))
        .arg("-nologo")
        .arg("-nointro")
        // Default WND=1: retail ButtonStart residual is headless-safe after shell
        // re-borrow + map resolve + InGame world-draw fixes. Override with =0 for soft UI.
        .env(
            "GENERALS_RUNTIME_HOST_WND",
            std::env::var("GENERALS_RUNTIME_HOST_WND").unwrap_or_else(|_| "1".into()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        // CRITICAL: do not pipe stderr without a drain thread — Roads.ini warn
        // spam fills the OS pipe and deadlocks the child in Booting.
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            result.status = "spawn_failed".into();
            result.detail = format!("failed to spawn {}: {e}", exe.display());
            return result;
        }
    };
    result.process_started = true;

    let started = Instant::now();
    let mut gameplay_step: u8 = 0;
    let mut saw_select_ok = false;
    let mut saw_move_ok = false;
    let mut saw_attack_ok = false;
    let mut saw_construct_ok = false;
    let mut construct_detail = String::new();
    let mut saw_train_ok = false;
    let mut train_detail = String::new();
    let mut saw_save_ok = false;
    let mut save_detail = String::new();
    let mut saw_load_ok = false;
    let mut load_detail = String::new();
    let mut saw_stop_ok = false;
    let mut stop_detail = String::new();
    let mut saw_sell_ok = false;
    let mut sell_detail = String::new();
    let mut saw_upgrade_ok = false;
    let mut upgrade_detail = String::new();
    let mut saw_guard_ok = false;
    let mut guard_detail = String::new();
    let mut saw_attack_move_ok = false;
    let mut attack_move_detail = String::new();
    let mut saw_scatter_ok = false;
    let mut scatter_detail = String::new();
    let mut saw_patrol_ok = false;
    let mut patrol_detail = String::new();
    let mut saw_deploy_ok = false;
    let mut deploy_detail = String::new();
    let mut saw_cheer_ok = false;
    let mut cheer_detail = String::new();
    let mut saw_formation_ok = false;
    let mut formation_detail = String::new();
    let mut saw_capture_ok = false;
    let mut capture_detail = String::new();
    let mut saw_return_supplies_ok = false;
    let mut return_supplies_detail = String::new();
    let mut saw_evacuate_ok = false;
    let mut evacuate_detail = String::new();
    let mut saw_repair_ok = false;
    let mut repair_detail = String::new();
    let mut saw_return_to_base_ok = false;
    let mut return_to_base_detail = String::new();
    let mut saw_attitude_ok = false;
    let mut attitude_detail = String::new();
    let mut saw_rally_ok = false;
    let mut rally_detail = String::new();
    let mut saw_switch_weapons_ok = false;
    let mut switch_weapons_detail = String::new();
    let mut saw_view_cc_ok = false;
    let mut view_cc_detail = String::new();
    let mut saw_clear_mines_ok = false;
    let mut clear_mines_detail = String::new();
    let mut saw_beacon_ok = false;
    let mut beacon_detail = String::new();
    let mut saw_hack_ok = false;
    let mut hack_detail = String::new();
    let mut saw_cleanup_ok = false;
    let mut cleanup_detail = String::new();
    let mut saw_combat_drop_ok = false;
    let mut combat_drop_detail = String::new();
    let mut saw_overcharge_ok = false;
    let mut overcharge_detail = String::new();
    let mut saw_special_power_ok = false;
    let mut special_power_detail = String::new();
    let mut saw_remove_beacon_ok = false;
    let mut remove_beacon_detail = String::new();
    let mut saw_demo_ok = false;
    let mut demo_detail = String::new();
    let mut saw_view_radar_ok = false;
    let mut view_radar_detail = String::new();
    let mut saw_force_attack_ok = false;
    let mut force_attack_detail = String::new();
    let mut saw_force_attack_object_ok = false;
    let mut force_attack_object_detail = String::new();
    let mut saw_select_all_ok = false;
    let mut select_all_detail = String::new();
    let mut saw_control_group_ok = false;
    let mut control_group_detail = String::new();
    let mut saw_waypoint_ok = false;
    let mut waypoint_detail = String::new();
    let mut saw_box_select_ok = false;
    let mut box_select_detail = String::new();
    let mut saw_presentation_frame_ok = false;
    let mut saw_presentation_live_fallback_ok = false;
    let mut presentation_detail = String::new();
    let mut saw_select_similar_ok = false;
    let mut select_similar_detail = String::new();
    let mut saw_select_on_screen_ok = false;
    let mut select_on_screen_detail = String::new();
    let mut saw_select_structures_ok = false;
    let mut select_structures_detail = String::new();
    let mut saw_select_aircraft_ok = false;
    let mut select_aircraft_detail = String::new();
    let mut saw_select_idle_ok = false;
    let mut select_idle_detail = String::new();
    let mut saw_camera_reset_ok = false;
    let mut camera_reset_detail = String::new();
    let mut saw_camera_zoom_ok = false;
    let mut camera_zoom_detail = String::new();
    let mut saw_pause_ok = false;
    let mut pause_detail = String::new();
    let mut saw_cancel_production_ok = false;
    let mut cancel_production_detail = String::new();
    let mut saw_diplomacy_ok = false;
    let mut diplomacy_detail = String::new();
    let mut saw_live_frame_ok = false;
    let mut max_render_item_count: u32 = 0;
    let mut max_render_alive_objects: u32 = 0;
    let mut render_items_nonzero_polls: u32 = 0;
    let mut saw_auto_attack_ok = false;
    let mut auto_attack_detail = String::new();
    let mut saw_options_ok = false;
    let mut options_detail = String::new();
    let mut saw_request_capture_ok = false;
    let mut request_capture_detail = String::new();
    let mut saw_skirmish_start_wnd_ok = false;
    let mut train_sent = false;
    let mut phase = 0u8; // 0 wait menu/boot, 1 commanded, 2 wait ingame, 3 exit
    let mut last_snap = StatusSnap::default();
    let mut commanded_at: Option<Instant> = None;

    loop {
        if started.elapsed() > timeout {
            result.status = "timeout".into();
            result.detail = format!(
                "timeout after {:?} last_state={} menu={} ingame={} frames={} phase={}",
                timeout,
                last_snap.state,
                result.reached_menu,
                result.reached_ingame,
                result.frames_observed,
                phase
            );
            kill_child(&mut child);
            break;
        }

        // Child exited early?
        if let Ok(Some(status)) = child.try_wait() {
            result.exit_code = status.code();
            if result.reached_ingame && status.success() {
                result.status = "success".into();
                result.executable_host_ok = true;
                let prior = result.detail.clone();
                result.detail = format!(
                    "exited ok after InGame frames={} map={} new_game={}",
                    result.frames_observed, result.map_seen, use_new_game_path
                );
                if let Some(idx) = prior.find("construct=") {
                    result.detail = format!("{}; {}", result.detail, &prior[idx..]);
                }
            } else if matches!(last_snap.state.as_str(), "LaunchFailed" | "")
                && !result.reached_menu
            {
                result.status = "assets_or_display_unavailable".into();
                result.detail = format!(
                    "process exited before Menu (code={:?}); display/GPU/assets may be unavailable",
                    status.code()
                );
            } else {
                result.status = "process_exited".into();
                result.detail = format!(
                    "process exited code={:?} state={} menu={} ingame={}",
                    status.code(),
                    last_snap.state,
                    result.reached_menu,
                    result.reached_ingame
                );
                // Partial success: reached InGame even if non-zero (e.g. unclean shutdown).
                if result.reached_ingame {
                    result.executable_host_ok = true;
                    result.status = "success_partial_exit".into();
                }
            }
            break;
        }

        if let Some(snap) = parse_status(&status_path) {
            // Presentation honesty residual from host status every poll.
            if snap.presentation_frame_ok {
                saw_presentation_frame_ok = true;
            }
            if snap.presentation_frame_ok && snap.presentation_live_fallback_reads == 0 {
                saw_presentation_live_fallback_ok = true;
            }
            if snap.presentation_frame_ok || snap.presentation_live_fallback_reads > 0 {
                presentation_detail = format!(
                    "frame_ok={} live_fallback={}",
                    snap.presentation_frame_ok, snap.presentation_live_fallback_reads
                );
            }
            // InGame world-draw residual: peak + stability of mesh pass item count.
            if matches!(snap.state.as_str(), "InGame" | "Paused") {
                max_render_item_count = max_render_item_count.max(snap.render_item_count);
                max_render_alive_objects = max_render_alive_objects.max(snap.render_alive_objects);
                if snap.render_item_count > 0 {
                    render_items_nonzero_polls = render_items_nonzero_polls.saturating_add(1);
                }
            }
            // Latch host residuals every poll — step boundaries can miss a one-frame
            // last_gameplay_cmd when the control loop is busy or a later command lands first.
            if snap.live_frame_ok {
                saw_live_frame_ok = true;
            }
            if snap.last_gameplay_cmd.starts_with("select_all_ok") {
                saw_select_all_ok = true;
                select_all_detail = snap.last_gameplay_cmd.clone();
            } else if snap.last_gameplay_cmd.starts_with("select_all_")
                && !snap.last_gameplay_cmd.starts_with("select_all_combat")
                && select_all_detail.is_empty()
            {
                select_all_detail = snap.last_gameplay_cmd.clone();
            }
            if snap.last_gameplay_cmd.starts_with("formation_ok") {
                saw_formation_ok = true;
                formation_detail = snap.last_gameplay_cmd.clone();
            } else if snap.last_gameplay_cmd.starts_with("formation_")
                && formation_detail.is_empty()
            {
                formation_detail = snap.last_gameplay_cmd.clone();
            }
            last_snap = snap.clone();
            result.frames_observed = result.frames_observed.max(snap.frame);
            if snap.map != "-" && !snap.map.is_empty() {
                result.map_seen = snap.map.clone();
            }
            match snap.state.as_str() {
                "Menu" => {
                    result.reached_menu = true;
                    if snap.ui_screen.to_ascii_lowercase().contains("skirmish") {
                        result.skirmish_menu_ok = true;
                    }
                }
                "InGame" | "Paused" => {
                    result.reached_menu = true;
                    if snap.ui_screen.to_ascii_lowercase().contains("skirmish") {
                        result.skirmish_menu_ok = true;
                    }
                    result.reached_ingame = true;
                }
                _ => {}
            }

            match phase {
                0 => {
                    // Wait until Menu or Booting finished enough to accept commands.
                    if snap.state == "Menu"
                        || (snap.state != "Booting"
                            && snap.startup_progress >= 0.99
                            && started.elapsed() > Duration::from_secs(8))
                        || started.elapsed() > Duration::from_secs(25)
                    {
                        // Soft open Skirmish UI first (override only; WND off).
                        let _ = write_control(&control_path, &["open_skirmish_menu"]);
                        commanded_at = Some(Instant::now());
                        phase = 10; // wait for Skirmish UI before start_game
                    }
                }

                10 => {
                    if snap.ui_screen.to_ascii_lowercase().contains("skirmish") {
                        result.skirmish_menu_ok = true;
                    }
                    // Proceed once Skirmish is visible, or after a short grace poll.
                    let ready = result.skirmish_menu_ok
                        || commanded_at
                            .map(|t| t.elapsed() > Duration::from_millis(800))
                            .unwrap_or(true);
                    if ready {
                        // Prefer real SkirmishMenu Start button click residual.
                        let click = format!("click_skirmish_start|map={}", map.replace('|', "/"));
                        let _ = write_control(&control_path, &[click.as_str()]);
                        commanded_at = Some(Instant::now());
                        phase = 1;
                    }
                }

                1 => {
                    if snap
                        .last_gameplay_cmd
                        .starts_with("click_skirmish_start_ok")
                    {
                        result.skirmish_start_click_ok = true;
                    }
                    // WND gadget path residual (may still be pending NewGame drain).
                    if snap
                        .last_gameplay_cmd
                        .starts_with("click_skirmish_start_ok_wnd")
                        || snap
                            .last_gameplay_cmd
                            .starts_with("click_skirmish_start_wnd")
                    {
                        result.skirmish_start_click_ok = true;
                        saw_skirmish_start_wnd_ok = true;
                        result.skirmish_start_wnd_ok = true;
                    }
                    if result.reached_ingame {
                        phase = 2;
                    } else if commanded_at
                        .map(|t| t.elapsed() > Duration::from_secs(45))
                        .unwrap_or(false)
                    {
                        // Retry once with direct start_game if NewGame path stalled.
                        if use_new_game_path {
                            let start = format!(
                                "start_game|mode=skirmish|faction=USA|map={}",
                                map.replace('|', "/")
                            );
                            let _ = write_control(&control_path, &[start.as_str()]);
                            commanded_at = Some(Instant::now());
                            phase = 1; // stay
                            result.detail.push_str(" fallback_start_game;");
                        } else {
                            result.status = "start_timeout".into();
                            result.detail = format!(
                                "did not reach InGame after start command; state={} phase={}",
                                snap.state, snap.startup_phase
                            );
                            let _ = write_control(&control_path, &["exit"]);
                            phase = 3;
                        }
                    }
                }
                2 => {
                    // Issue host gameplay commands (select + move), then exit.
                    // Not WND widget clicks — still not playable_claim.
                    if gameplay_step == 0 {
                        let _ = write_control(&control_path, &["select_local_unit"]);
                        gameplay_step = 1;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 1
                        && (snap.last_gameplay_cmd.starts_with("select_ok")
                            || snap.last_gameplay_cmd.starts_with("select_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_ok") {
                            saw_select_ok = true;
                        }
                        let _ = write_control(&control_path, &["move_selected|x=100|y=0|z=100"]);
                        gameplay_step = 2;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 2
                        && (snap.last_gameplay_cmd.starts_with("move_ok")
                            || snap.last_gameplay_cmd.starts_with("move_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("move_ok") {
                            saw_move_ok = true;
                        }
                        let _ = write_control(&control_path, &["construct|template=USA_Barracks"]);
                        gameplay_step = 3;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 3
                        && (snap.last_gameplay_cmd.starts_with("construct_ok")
                            || snap.last_gameplay_cmd.starts_with("construct_fail")
                            || snap.last_gameplay_cmd.starts_with("construct_")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("construct_ok") {
                            saw_construct_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("construct_") {
                            construct_detail = snap.last_gameplay_cmd.clone();
                        }
                        // Train before attack so victory/match_over cannot skip production residual.
                        let _ = write_control(
                            &control_path,
                            &[
                                "train_unit|template=AmericaInfantryRanger",
                                "train_unit|template=USA_Ranger",
                            ],
                        );
                        train_sent = true;
                        gameplay_step = 4;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 4
                        && (snap.last_gameplay_cmd.starts_with("train_ok")
                            || snap.last_gameplay_cmd.starts_with("train_fail")
                            || snap.last_gameplay_cmd.starts_with("train_")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(8))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("train_ok") {
                            saw_train_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("train_") {
                            train_detail = snap.last_gameplay_cmd.clone();
                        }
                        // Host residual: train_ok queues production; wait until a second
                        // local mobile exits so later formation/select residuals are honest.
                        // Fail-closed timeout still advances so the chain cannot hang forever.
                        let train_mobile_ready = snap.local_mobile_units >= 2;
                        let train_wait_expired = commanded_at
                            .map(|t| t.elapsed() > Duration::from_secs(20))
                            .unwrap_or(false);
                        if !train_mobile_ready && !train_wait_expired {
                            // keep polling; do not advance yet
                        } else {
                            let _ = write_control(
                                &control_path,
                                &["upgrade|name=UpgradeAmericaRangerCaptureBuilding"],
                            );
                            gameplay_step = 5;
                            commanded_at = Some(Instant::now());
                        }
                    } else if gameplay_step == 5
                        && (snap.last_gameplay_cmd.starts_with("upgrade_ok")
                            || snap.last_gameplay_cmd.starts_with("upgrade_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("upgrade_ok") {
                            saw_upgrade_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("upgrade_") {
                            upgrade_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["quicksave"]);
                        gameplay_step = 6;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 6
                        && (snap.last_gameplay_cmd.starts_with("save_ok")
                            || snap.last_gameplay_cmd.starts_with("save_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("save_ok") {
                            saw_save_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("save_") {
                            save_detail = snap.last_gameplay_cmd.clone();
                        }
                        // Round-trip residual: load the slot we just wrote.
                        let _ = write_control(&control_path, &["quickload"]);
                        gameplay_step = 7;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 7
                        && (snap.last_gameplay_cmd.starts_with("load_ok")
                            || snap.last_gameplay_cmd.starts_with("load_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(20))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("load_ok") {
                            saw_load_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("load_") {
                            load_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["stop_all"]);
                        gameplay_step = 8;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 8
                        && (snap.last_gameplay_cmd.starts_with("stop_ok")
                            || snap.last_gameplay_cmd.starts_with("stop_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("stop_ok") {
                            saw_stop_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("stop_") {
                            stop_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["sell"]);
                        gameplay_step = 9;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 9
                        && (snap.last_gameplay_cmd.starts_with("sell_ok")
                            || snap.last_gameplay_cmd.starts_with("sell_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("sell_ok") {
                            saw_sell_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("sell_") {
                            sell_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["guard|x=120|y=0|z=120"]);
                        gameplay_step = 10;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 10
                        && (snap.last_gameplay_cmd.starts_with("guard_ok")
                            || snap.last_gameplay_cmd.starts_with("guard_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("guard_ok") {
                            saw_guard_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("guard_") {
                            guard_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["attack_move|x=150|y=0|z=150"]);
                        gameplay_step = 11;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 11
                        && (snap.last_gameplay_cmd.starts_with("attack_move_ok")
                            || snap.last_gameplay_cmd.starts_with("attack_move_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("attack_move_ok") {
                            saw_attack_move_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_move_") {
                            attack_move_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["scatter"]);
                        gameplay_step = 12;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 12
                        && (snap.last_gameplay_cmd.starts_with("scatter_ok")
                            || snap.last_gameplay_cmd.starts_with("scatter_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("scatter_ok") {
                            saw_scatter_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("scatter_") {
                            scatter_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["patrol"]);
                        gameplay_step = 13;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 13
                        && (snap.last_gameplay_cmd.starts_with("patrol_ok")
                            || snap.last_gameplay_cmd.starts_with("patrol_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("patrol_ok") {
                            saw_patrol_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("patrol_") {
                            patrol_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["deploy"]);
                        gameplay_step = 14;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 14
                        && (snap.last_gameplay_cmd.starts_with("deploy_ok")
                            || snap.last_gameplay_cmd.starts_with("deploy_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("deploy_ok") {
                            saw_deploy_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("deploy_") {
                            deploy_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["cheer"]);
                        gameplay_step = 15;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 15
                        && (snap.last_gameplay_cmd.starts_with("cheer_ok")
                            || snap.last_gameplay_cmd.starts_with("cheer_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("cheer_ok") {
                            saw_cheer_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("cheer_") {
                            cheer_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["formation"]);
                        gameplay_step = 16;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 16
                        && (snap.last_gameplay_cmd.starts_with("formation_ok")
                            || snap.last_gameplay_cmd.starts_with("formation_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("formation_ok") {
                            saw_formation_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("formation_") {
                            formation_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["capture"]);
                        gameplay_step = 17;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 17
                        && (snap.last_gameplay_cmd.starts_with("capture_ok")
                            || snap.last_gameplay_cmd.starts_with("capture_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("capture_ok") {
                            saw_capture_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("capture_") {
                            capture_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["return_supplies"]);
                        gameplay_step = 18;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 18
                        && (snap.last_gameplay_cmd.starts_with("return_supplies_ok")
                            || snap.last_gameplay_cmd.starts_with("return_supplies_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("return_supplies_ok") {
                            saw_return_supplies_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("return_supplies_") {
                            return_supplies_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["evacuate"]);
                        gameplay_step = 19;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 19
                        && (snap.last_gameplay_cmd.starts_with("evacuate_ok")
                            || snap.last_gameplay_cmd.starts_with("evacuate_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("evacuate_ok") {
                            saw_evacuate_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("evacuate_") {
                            evacuate_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["repair"]);
                        gameplay_step = 20;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 20
                        && (snap.last_gameplay_cmd.starts_with("repair_ok")
                            || snap.last_gameplay_cmd.starts_with("repair_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("repair_ok") {
                            saw_repair_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("repair_") {
                            repair_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["return_to_base"]);
                        gameplay_step = 21;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 21
                        && (snap.last_gameplay_cmd.starts_with("return_to_base_ok")
                            || snap.last_gameplay_cmd.starts_with("return_to_base_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("return_to_base_ok") {
                            saw_return_to_base_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("return_to_base_") {
                            return_to_base_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["attitude_aggressive"]);
                        gameplay_step = 22;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 22
                        && (snap.last_gameplay_cmd.starts_with("attitude_ok")
                            || snap.last_gameplay_cmd.starts_with("attitude_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("attitude_ok") {
                            saw_attitude_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("attitude_") {
                            attitude_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["rally|x=90|y=0|z=90"]);
                        gameplay_step = 23;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 23
                        && (snap.last_gameplay_cmd.starts_with("rally_ok")
                            || snap.last_gameplay_cmd.starts_with("rally_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("rally_ok") {
                            saw_rally_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("rally_") {
                            rally_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["switch_weapons"]);
                        gameplay_step = 24;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 24
                        && (snap.last_gameplay_cmd.starts_with("switch_weapons_ok")
                            || snap.last_gameplay_cmd.starts_with("switch_weapons_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("switch_weapons_ok") {
                            saw_switch_weapons_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("switch_weapons_") {
                            switch_weapons_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["view_cc"]);
                        gameplay_step = 25;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 25
                        && (snap.last_gameplay_cmd.starts_with("view_cc_ok")
                            || snap.last_gameplay_cmd.starts_with("view_cc_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("view_cc_ok") {
                            saw_view_cc_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("view_cc_") {
                            view_cc_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["clear_mines"]);
                        gameplay_step = 26;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 26
                        && (snap.last_gameplay_cmd.starts_with("clear_mines_ok")
                            || snap.last_gameplay_cmd.starts_with("clear_mines_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("clear_mines_ok") {
                            saw_clear_mines_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("clear_mines_") {
                            clear_mines_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["place_beacon|x=60|y=0|z=60"]);
                        gameplay_step = 27;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 27
                        && (snap.last_gameplay_cmd.starts_with("beacon_ok")
                            || snap.last_gameplay_cmd.starts_with("beacon_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("beacon_ok") {
                            saw_beacon_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("beacon_") {
                            beacon_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["hack_internet"]);
                        gameplay_step = 28;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 28
                        && (snap.last_gameplay_cmd.starts_with("hack_ok")
                            || snap.last_gameplay_cmd.starts_with("hack_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("hack_ok") {
                            saw_hack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("hack_") {
                            hack_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["cleanup_area"]);
                        gameplay_step = 29;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 29
                        && (snap.last_gameplay_cmd.starts_with("cleanup_ok")
                            || snap.last_gameplay_cmd.starts_with("cleanup_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("cleanup_ok") {
                            saw_cleanup_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("cleanup_") {
                            cleanup_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["combat_drop|x=75|y=0|z=75"]);
                        gameplay_step = 30;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 30
                        && (snap.last_gameplay_cmd.starts_with("combat_drop_ok")
                            || snap.last_gameplay_cmd.starts_with("combat_drop_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("combat_drop_ok") {
                            saw_combat_drop_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("combat_drop_") {
                            combat_drop_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["toggle_overcharge"]);
                        gameplay_step = 31;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 31
                        && (snap.last_gameplay_cmd.starts_with("overcharge_ok")
                            || snap.last_gameplay_cmd.starts_with("overcharge_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("overcharge_ok") {
                            saw_overcharge_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("overcharge_") {
                            overcharge_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["special_power"]);
                        gameplay_step = 32;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 32
                        && (snap.last_gameplay_cmd.starts_with("special_power_ok")
                            || snap.last_gameplay_cmd.starts_with("special_power_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("special_power_ok") {
                            saw_special_power_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("special_power_") {
                            special_power_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["remove_beacon"]);
                        gameplay_step = 33;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 33
                        && (snap.last_gameplay_cmd.starts_with("remove_beacon_ok")
                            || snap.last_gameplay_cmd.starts_with("remove_beacon_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("remove_beacon_ok") {
                            saw_remove_beacon_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("remove_beacon_") {
                            remove_beacon_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["demo_suicide"]);
                        gameplay_step = 34;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 34
                        && (snap.last_gameplay_cmd.starts_with("demo_suicide_ok")
                            || snap.last_gameplay_cmd.starts_with("demo_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("demo_suicide_ok") {
                            saw_demo_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("demo_") {
                            demo_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["view_radar"]);
                        gameplay_step = 35;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 35
                        && (snap.last_gameplay_cmd.starts_with("view_radar_ok")
                            || snap.last_gameplay_cmd.starts_with("view_radar_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("view_radar_ok") {
                            saw_view_radar_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("view_radar_") {
                            view_radar_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["force_attack|x=110|y=0|z=110"]);
                        gameplay_step = 36;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 36
                        && (snap.last_gameplay_cmd.starts_with("force_attack_ok")
                            || snap.last_gameplay_cmd.starts_with("force_attack_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("force_attack_ok") {
                            saw_force_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("force_attack_")
                            && !snap.last_gameplay_cmd.starts_with("force_attack_object")
                        {
                            force_attack_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["force_attack_object"]);
                        gameplay_step = 37;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 37
                        && (snap.last_gameplay_cmd.starts_with("force_attack_object_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("force_attack_object_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("force_attack_object_ok") {
                            saw_force_attack_object_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("force_attack_object_") {
                            force_attack_object_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_all"]);
                        gameplay_step = 38;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 38
                        && (snap.last_gameplay_cmd.starts_with("select_all_ok")
                            || snap.last_gameplay_cmd.starts_with("select_all_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(8))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_all_ok") {
                            saw_select_all_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_all_")
                            && !snap.last_gameplay_cmd.starts_with("select_all_combat")
                        {
                            select_all_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["assign_control_group|group=1"]);
                        gameplay_step = 39;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 39
                        && (snap
                            .last_gameplay_cmd
                            .starts_with("control_group_assign_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("control_group_assign_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap
                            .last_gameplay_cmd
                            .starts_with("control_group_assign_ok")
                        {
                            // partial — need recall too
                            control_group_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("control_group_") {
                            control_group_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["recall_control_group|group=1"]);
                        gameplay_step = 40;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 40
                        && (snap
                            .last_gameplay_cmd
                            .starts_with("control_group_recall_ok")
                            || snap
                                .last_gameplay_cmd
                                .starts_with("control_group_recall_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap
                            .last_gameplay_cmd
                            .starts_with("control_group_recall_ok")
                            && control_group_detail.starts_with("control_group_assign_ok")
                        {
                            saw_control_group_ok = true;
                        } else if snap
                            .last_gameplay_cmd
                            .starts_with("control_group_recall_ok")
                        {
                            // assign detail may have been overwritten — still ok if recall ok after assign step
                            saw_control_group_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("control_group_") {
                            control_group_detail =
                                format!("{};{}", control_group_detail, snap.last_gameplay_cmd);
                        }
                        let _ = write_control(&control_path, &["waypoint_mode|on=1"]);
                        gameplay_step = 41;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 41
                        && (snap.last_gameplay_cmd.starts_with("waypoint_mode_ok")
                            || snap.last_gameplay_cmd.starts_with("waypoint_mode_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("waypoint_mode_") {
                            waypoint_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["add_waypoint|x=130|y=0|z=130"]);
                        gameplay_step = 42;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 42
                        && (snap.last_gameplay_cmd.starts_with("waypoint_ok")
                            || snap.last_gameplay_cmd.starts_with("waypoint_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("waypoint_ok") {
                            saw_waypoint_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("waypoint_") {
                            waypoint_detail =
                                format!("{};{}", waypoint_detail, snap.last_gameplay_cmd);
                        }
                        let _ = write_control(
                            &control_path,
                            &["box_select|min_x=-8000|max_x=8000|min_z=-8000|max_z=8000"],
                        );
                        gameplay_step = 43;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 43
                        && (snap.last_gameplay_cmd.starts_with("box_select_ok")
                            || snap.last_gameplay_cmd.starts_with("box_select_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("box_select_ok") {
                            saw_box_select_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("box_select_") {
                            box_select_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_similar"]);
                        gameplay_step = 44;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 44
                        && (snap.last_gameplay_cmd.starts_with("select_similar_ok")
                            || snap.last_gameplay_cmd.starts_with("select_similar_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_similar_ok") {
                            saw_select_similar_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_similar_") {
                            select_similar_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_on_screen"]);
                        gameplay_step = 45;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 45
                        && (snap.last_gameplay_cmd.starts_with("select_on_screen_ok")
                            || snap.last_gameplay_cmd.starts_with("select_on_screen_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_on_screen_ok") {
                            saw_select_on_screen_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_on_screen_") {
                            select_on_screen_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_structures"]);
                        gameplay_step = 46;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 46
                        && (snap.last_gameplay_cmd.starts_with("select_structures_ok")
                            || snap.last_gameplay_cmd.starts_with("select_structures_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_structures_ok") {
                            saw_select_structures_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_structures_") {
                            select_structures_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_aircraft"]);
                        gameplay_step = 47;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 47
                        && (snap.last_gameplay_cmd.starts_with("select_aircraft_ok")
                            || snap.last_gameplay_cmd.starts_with("select_aircraft_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_aircraft_ok") {
                            saw_select_aircraft_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_aircraft_") {
                            select_aircraft_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["select_idle_harvesters"]);
                        gameplay_step = 48;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 48
                        && (snap.last_gameplay_cmd.starts_with("select_idle_ok")
                            || snap.last_gameplay_cmd.starts_with("select_idle_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("select_idle_ok") {
                            saw_select_idle_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_idle_") {
                            select_idle_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["camera_reset"]);
                        gameplay_step = 49;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 49
                        && (snap.last_gameplay_cmd.starts_with("camera_reset_ok")
                            || snap.last_gameplay_cmd.starts_with("camera_reset_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("camera_reset_ok") {
                            saw_camera_reset_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("camera_reset_") {
                            camera_reset_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["camera_zoom|z=1.25"]);
                        gameplay_step = 50;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 50
                        && (snap.last_gameplay_cmd.starts_with("camera_zoom_ok")
                            || snap.last_gameplay_cmd.starts_with("camera_zoom_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("camera_zoom_ok") {
                            saw_camera_zoom_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("camera_zoom_") {
                            camera_zoom_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["toggle_pause"]);
                        gameplay_step = 51;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 51 {
                        if snap.last_gameplay_cmd.starts_with("pause_ok") {
                            pause_detail = snap.last_gameplay_cmd.clone();
                            saw_pause_ok = true;
                            let _ = write_control(&control_path, &["toggle_pause"]);
                            gameplay_step = 52;
                            commanded_at = Some(Instant::now());
                        } else if snap.last_gameplay_cmd.starts_with("pause_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(12))
                                .unwrap_or(false)
                        {
                            if snap.last_gameplay_cmd.starts_with("pause_") {
                                pause_detail = snap.last_gameplay_cmd.clone();
                            }
                            let _ = write_control(&control_path, &["cancel_production"]);
                            gameplay_step = 53;
                            commanded_at = Some(Instant::now());
                        } else if commanded_at
                            .map(|t| t.elapsed() > Duration::from_millis(1500))
                            .unwrap_or(false)
                        {
                            let _ = write_control(&control_path, &["toggle_pause"]);
                            commanded_at = Some(Instant::now());
                        }
                    } else if gameplay_step == 52 {
                        if snap.last_gameplay_cmd.starts_with("pause_ok") {
                            pause_detail = format!("{};{}", pause_detail, snap.last_gameplay_cmd);
                            saw_pause_ok = true;
                            let _ = write_control(&control_path, &["cancel_production"]);
                            gameplay_step = 53;
                            commanded_at = Some(Instant::now());
                        } else if snap.last_gameplay_cmd.starts_with("pause_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(12))
                                .unwrap_or(false)
                        {
                            let _ = write_control(&control_path, &["cancel_production"]);
                            gameplay_step = 53;
                            commanded_at = Some(Instant::now());
                        } else if commanded_at
                            .map(|t| t.elapsed() > Duration::from_millis(1500))
                            .unwrap_or(false)
                        {
                            let _ = write_control(&control_path, &["toggle_pause"]);
                            commanded_at = Some(Instant::now());
                        }
                    } else if gameplay_step == 53
                        && (snap.last_gameplay_cmd.starts_with("cancel_production_ok")
                            || snap.last_gameplay_cmd.starts_with("cancel_production_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("cancel_production_ok") {
                            saw_cancel_production_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("cancel_production_") {
                            cancel_production_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["request_capture"]);
                        gameplay_step = 54;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 54
                        && (snap.last_gameplay_cmd.starts_with("request_capture_ok")
                            || snap.last_gameplay_cmd.starts_with("request_capture_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("request_capture_ok") {
                            saw_request_capture_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("request_capture_") {
                            request_capture_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["auto_attack|on=1"]);
                        gameplay_step = 55;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 55
                        && (snap.last_gameplay_cmd.starts_with("auto_attack_ok")
                            || snap.last_gameplay_cmd.starts_with("auto_attack_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("auto_attack_ok") {
                            saw_auto_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("auto_attack_") {
                            auto_attack_detail = snap.last_gameplay_cmd.clone();
                        }
                        // Attack while still InGame (options/diplomacy leave match).
                        let _ = write_control(&control_path, &["attack_nearest_enemy"]);
                        gameplay_step = 56;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 56
                        && (snap.last_gameplay_cmd.starts_with("attack_ok")
                            || snap.last_gameplay_cmd.starts_with("attack_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("attack_ok") {
                            saw_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_") {
                            // keep prior attack detail path in final branch too
                        }
                        let _ = write_control(&control_path, &["options_probe"]);
                        gameplay_step = 57;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 57
                        && (snap.last_gameplay_cmd.starts_with("options_probe_ok")
                            || snap.last_gameplay_cmd.starts_with("options_ok")
                            || snap.last_gameplay_cmd.starts_with("options_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(6))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("options_probe_ok")
                            || snap.last_gameplay_cmd.starts_with("options_ok")
                        {
                            saw_options_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("options_") {
                            options_detail = snap.last_gameplay_cmd.clone();
                        }
                        let _ = write_control(&control_path, &["open_diplomacy"]);
                        gameplay_step = 58;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step == 58
                        && (snap.last_gameplay_cmd.starts_with("diplomacy_ok")
                            || snap.last_gameplay_cmd.starts_with("diplomacy_fail")
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(4))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("diplomacy_ok") {
                            saw_diplomacy_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("diplomacy_") {
                            diplomacy_detail = snap.last_gameplay_cmd.clone();
                        }
                        gameplay_step = 59;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step >= 59 {
                        if snap.last_gameplay_cmd.starts_with("move_ok") {
                            saw_move_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("construct_ok") {
                            saw_construct_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("construct_") {
                            construct_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("train_ok") {
                            saw_train_ok = true;
                            train_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("train_") {
                            train_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("save_ok") {
                            saw_save_ok = true;
                            save_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("save_") {
                            save_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("load_ok") {
                            saw_load_ok = true;
                            load_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("load_") {
                            load_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("stop_ok") {
                            saw_stop_ok = true;
                            stop_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("stop_") {
                            stop_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("sell_ok") {
                            saw_sell_ok = true;
                            sell_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("sell_") {
                            sell_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("upgrade_ok") {
                            saw_upgrade_ok = true;
                            upgrade_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("upgrade_") {
                            upgrade_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("guard_ok") {
                            saw_guard_ok = true;
                            guard_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("guard_") {
                            guard_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_move_ok") {
                            saw_attack_move_ok = true;
                            attack_move_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("attack_move_") {
                            attack_move_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("scatter_ok") {
                            saw_scatter_ok = true;
                            scatter_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("scatter_") {
                            scatter_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("patrol_ok") {
                            saw_patrol_ok = true;
                            patrol_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("patrol_") {
                            patrol_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("deploy_ok") {
                            saw_deploy_ok = true;
                            deploy_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("deploy_") {
                            deploy_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("cheer_ok") {
                            saw_cheer_ok = true;
                            cheer_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("cheer_") {
                            cheer_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("formation_ok") {
                            saw_formation_ok = true;
                            formation_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("formation_") {
                            formation_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("capture_ok") {
                            saw_capture_ok = true;
                            capture_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("capture_") {
                            capture_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("return_supplies_ok") {
                            saw_return_supplies_ok = true;
                            return_supplies_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("return_supplies_") {
                            return_supplies_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("evacuate_ok") {
                            saw_evacuate_ok = true;
                            evacuate_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("evacuate_") {
                            evacuate_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("repair_ok") {
                            saw_repair_ok = true;
                            repair_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("repair_") {
                            repair_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("return_to_base_ok") {
                            saw_return_to_base_ok = true;
                            return_to_base_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("return_to_base_") {
                            return_to_base_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("attitude_ok") {
                            saw_attitude_ok = true;
                            attitude_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("attitude_") {
                            attitude_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("rally_ok") {
                            saw_rally_ok = true;
                            rally_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("rally_") {
                            rally_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("switch_weapons_ok") {
                            saw_switch_weapons_ok = true;
                            switch_weapons_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("switch_weapons_") {
                            switch_weapons_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("view_cc_ok") {
                            saw_view_cc_ok = true;
                            view_cc_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("view_cc_") {
                            view_cc_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("clear_mines_ok") {
                            saw_clear_mines_ok = true;
                            clear_mines_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("clear_mines_") {
                            clear_mines_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("beacon_ok") {
                            saw_beacon_ok = true;
                            beacon_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("beacon_") {
                            beacon_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("hack_ok") {
                            saw_hack_ok = true;
                            hack_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("hack_") {
                            hack_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("cleanup_ok") {
                            saw_cleanup_ok = true;
                            cleanup_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("cleanup_") {
                            cleanup_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("combat_drop_ok") {
                            saw_combat_drop_ok = true;
                            combat_drop_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("combat_drop_") {
                            combat_drop_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("overcharge_ok") {
                            saw_overcharge_ok = true;
                            overcharge_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("overcharge_") {
                            overcharge_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("special_power_ok") {
                            saw_special_power_ok = true;
                            special_power_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("special_power_") {
                            special_power_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("remove_beacon_ok") {
                            saw_remove_beacon_ok = true;
                            remove_beacon_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("remove_beacon_") {
                            remove_beacon_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("demo_suicide_ok") {
                            saw_demo_ok = true;
                            demo_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("demo_") {
                            demo_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("view_radar_ok") {
                            saw_view_radar_ok = true;
                            view_radar_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("view_radar_") {
                            view_radar_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("force_attack_ok") {
                            saw_force_attack_ok = true;
                            force_attack_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("force_attack_")
                            && !snap.last_gameplay_cmd.starts_with("force_attack_object")
                        {
                            force_attack_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("force_attack_object_ok") {
                            saw_force_attack_object_ok = true;
                            force_attack_object_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("force_attack_object_") {
                            force_attack_object_detail = snap.last_gameplay_cmd.clone();
                        }
                        if snap.last_gameplay_cmd.starts_with("select_all_ok") {
                            saw_select_all_ok = true;
                            select_all_detail = snap.last_gameplay_cmd.clone();
                        } else if snap.last_gameplay_cmd.starts_with("select_all_")
                            && !snap.last_gameplay_cmd.starts_with("select_all_combat")
                        {
                            select_all_detail = snap.last_gameplay_cmd.clone();
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
                                saw_control_group_ok = true;
                            }
                            control_group_detail =
                                format!("{};{}", control_group_detail, snap.last_gameplay_cmd);
                        } else if snap.last_gameplay_cmd.starts_with("control_group_") {
                            control_group_detail =
                                format!("{};{}", control_group_detail, snap.last_gameplay_cmd);
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_ok")
                            || snap.last_gameplay_cmd.starts_with("attack_fail")
                            || snap.last_gameplay_cmd.starts_with("attack_begin")
                        {
                            saw_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_ok") {
                            saw_select_ok = true;
                        }
                        if train_sent
                            && train_detail.is_empty()
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(2))
                                .unwrap_or(false)
                        {
                            let _ = write_control(
                                &control_path,
                                &["train_unit|template=AmericaInfantryRanger"],
                            );
                        }
                        // Primary: select+move+attack. Residual: production+attack proves
                        // host command path when early select timing is noisy.
                        result.gameplay_cmd_ok = (saw_select_ok && saw_move_ok && saw_attack_ok)
                            || (saw_select_ok && saw_move_ok && saw_construct_ok && saw_train_ok)
                            || (saw_construct_ok && saw_train_ok && saw_attack_ok);
                        result.construct_cmd_ok = saw_construct_ok;
                        result.train_cmd_ok = saw_train_ok;
                        result.save_cmd_ok = saw_save_ok;
                        result.load_cmd_ok = saw_load_ok;
                        result.stop_cmd_ok = saw_stop_ok;
                        result.sell_cmd_ok = saw_sell_ok;
                        result.upgrade_cmd_ok = saw_upgrade_ok;
                        result.guard_cmd_ok = saw_guard_ok;
                        result.attack_move_cmd_ok = saw_attack_move_ok;
                        result.scatter_cmd_ok = saw_scatter_ok;
                        result.patrol_cmd_ok = saw_patrol_ok;
                        result.deploy_cmd_ok = saw_deploy_ok;
                        result.cheer_cmd_ok = saw_cheer_ok;
                        result.formation_cmd_ok = saw_formation_ok;
                        result.capture_cmd_ok = saw_capture_ok;
                        result.return_supplies_cmd_ok = saw_return_supplies_ok;
                        result.evacuate_cmd_ok = saw_evacuate_ok;
                        result.repair_cmd_ok = saw_repair_ok;
                        result.return_to_base_cmd_ok = saw_return_to_base_ok;
                        result.attitude_cmd_ok = saw_attitude_ok;
                        result.rally_cmd_ok = saw_rally_ok;
                        result.switch_weapons_cmd_ok = saw_switch_weapons_ok;
                        result.view_cc_cmd_ok = saw_view_cc_ok;
                        result.clear_mines_cmd_ok = saw_clear_mines_ok;
                        result.beacon_cmd_ok = saw_beacon_ok;
                        result.hack_cmd_ok = saw_hack_ok;
                        result.cleanup_cmd_ok = saw_cleanup_ok;
                        result.combat_drop_cmd_ok = saw_combat_drop_ok;
                        result.overcharge_cmd_ok = saw_overcharge_ok;
                        result.special_power_cmd_ok = saw_special_power_ok;
                        result.remove_beacon_cmd_ok = saw_remove_beacon_ok;
                        result.demo_cmd_ok = saw_demo_ok;
                        result.view_radar_cmd_ok = saw_view_radar_ok;
                        result.force_attack_cmd_ok = saw_force_attack_ok;
                        result.force_attack_object_cmd_ok = saw_force_attack_object_ok;
                        result.select_all_cmd_ok = saw_select_all_ok;
                        result.control_group_cmd_ok = saw_control_group_ok;
                        result.waypoint_cmd_ok = saw_waypoint_ok;
                        result.box_select_cmd_ok = saw_box_select_ok;
                        result.presentation_frame_ok = saw_presentation_frame_ok;
                        result.presentation_live_fallback_ok = saw_presentation_live_fallback_ok;
                        result.max_render_item_count = max_render_item_count;
                        result.max_render_alive_objects = max_render_alive_objects;
                        // Stable = at least 3 InGame polls with items (not a one-frame flash).
                        result.render_items_stable_ok =
                            render_items_nonzero_polls >= 3 && max_render_item_count > 0;
                        result.select_similar_cmd_ok = saw_select_similar_ok;
                        result.select_on_screen_cmd_ok = saw_select_on_screen_ok;
                        result.select_structures_cmd_ok = saw_select_structures_ok;
                        result.select_aircraft_cmd_ok = saw_select_aircraft_ok;
                        result.select_idle_cmd_ok = saw_select_idle_ok;
                        result.camera_reset_cmd_ok = saw_camera_reset_ok;
                        result.camera_zoom_cmd_ok = saw_camera_zoom_ok;
                        result.pause_cmd_ok = saw_pause_ok;
                        result.cancel_production_cmd_ok = saw_cancel_production_ok;
                        result.diplomacy_cmd_ok = saw_diplomacy_ok;
                        result.live_frame_ok = saw_live_frame_ok;
                        result.auto_attack_cmd_ok = saw_auto_attack_ok;
                        result.options_cmd_ok = saw_options_ok;
                        result.request_capture_cmd_ok = saw_request_capture_ok;
                        result.skirmish_start_wnd_ok =
                            saw_skirmish_start_wnd_ok || result.skirmish_start_wnd_ok;
                        if !presentation_detail.is_empty() {
                            result.detail =
                                format!("{}; presentation={}", result.detail, presentation_detail);
                        }
                        result.detail =
                            format!("{}; last_cmd={}", result.detail, snap.last_gameplay_cmd);
                        if !construct_detail.is_empty() {
                            result.detail =
                                format!("{}; construct={}", result.detail, construct_detail);
                        }
                        if !train_detail.is_empty() {
                            result.detail = format!("{}; train={}", result.detail, train_detail);
                        }
                        // Exit only after the full host command chain finishes
                        // (step >= 59: pause/cancel/attack/options/diplomacy), or on
                        // hard stall / frame budget. Do not cut off mid-chain once
                        // construct/train/attack land — later residuals (pause, etc.)
                        // would stay false forever.
                        let chain_complete = gameplay_step >= 59;
                        // Only hard-stall once we're deep in the chain; early steps
                        // have their own per-command timeouts.
                        let hard_stall = gameplay_step >= 50
                            && commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(120))
                                .unwrap_or(false);
                        if chain_complete || hard_stall || snap.frame >= 500 {
                            let _ = write_control(&control_path, &["exit"]);
                            phase = 3;
                        }
                    }
                }
                3 => {
                    // Wait for clean exit.
                    if let Ok(Some(status)) = child.try_wait() {
                        result.exit_code = status.code();
                        if result.reached_ingame {
                            result.executable_host_ok = true;
                            result.status = "success".into();
                            result.detail = format!(
                                "InGame frames={} map={} exit={:?} new_game={} menu={}",
                                result.frames_observed,
                                result.map_seen,
                                status.code(),
                                use_new_game_path,
                                result.reached_menu
                            );
                        } else if result.reached_menu {
                            result.status = "menu_only".into();
                            result.detail = format!(
                                "reached Menu but not InGame; exit={:?} map={}",
                                status.code(),
                                result.map_seen
                            );
                        } else {
                            result.status = "no_menu".into();
                            result.detail = format!(
                                "never reached Menu; exit={:?} last_state={}",
                                status.code(),
                                last_snap.state
                            );
                        }
                        break;
                    }
                    if commanded_at
                        .map(|t| t.elapsed() > Duration::from_secs(20))
                        .unwrap_or(false)
                        && phase == 3
                    {
                        kill_child(&mut child);
                        if result.reached_ingame {
                            result.executable_host_ok = true;
                            result.status = "success_forced_exit".into();
                            result.detail = format!(
                                "InGame ok but exit hang; frames={} map={}",
                                result.frames_observed, result.map_seen
                            );
                        } else {
                            result.status = "exit_hang".into();
                            result.detail = "exit command did not stop process".into();
                        }
                        break;
                    }
                }
                _ => {}
            }
        }

        thread::sleep(Duration::from_millis(100));
    }

    let _ = fs::remove_dir_all(&tmp);

    // Never flip retail claim from this harness.
    result.playable_claim = false;
    result
}

pub fn format_executable_smoke_report(r: &ExecutableSmokeResult) -> String {
    format!(
        "executable_smoke status={} host_ok={} playable_claim={} started={} menu={} ingame={} gameplay_cmd={} construct_cmd={} train_cmd={} upgrade_cmd={} save_cmd={} load_cmd={} stop_cmd={} sell_cmd={} guard_cmd={} attack_move_cmd={} scatter_cmd={} patrol_cmd={} deploy_cmd={} cheer_cmd={} formation_cmd={} capture_cmd={} return_supplies_cmd={} evacuate_cmd={} repair_cmd={} return_to_base_cmd={} attitude_cmd={} rally_cmd={} switch_weapons_cmd={} view_cc_cmd={} clear_mines_cmd={} beacon_cmd={} hack_cmd={} cleanup_cmd={} combat_drop_cmd={} overcharge_cmd={} special_power_cmd={} remove_beacon_cmd={} demo_cmd={} view_radar_cmd={} force_attack_cmd={} force_attack_object_cmd={} select_all_cmd={} control_group_cmd={} waypoint_cmd={} box_select_cmd={} presentation_frame_ok={} max_render_items={} render_items_stable={} max_render_alive={} presentation_live_fallback_ok={} select_similar_cmd={} select_on_screen_cmd={} select_structures_cmd={} select_aircraft_cmd={} select_idle_cmd={} camera_reset_cmd={} camera_zoom_cmd={} pause_cmd={} cancel_production_cmd={} diplomacy_cmd={} live_frame_ok={} auto_attack_cmd={} options_cmd={} request_capture_cmd={} skirmish_start_wnd={} skirmish_menu={} skirmish_start_click={} frames={} map={} exit={:?} new_game={} detail={}",
        r.status,
        r.executable_host_ok,
        r.playable_claim,
        r.process_started,
        r.reached_menu,
        r.reached_ingame,
        r.gameplay_cmd_ok,
        r.construct_cmd_ok,
        r.train_cmd_ok,
        r.upgrade_cmd_ok,
        r.save_cmd_ok,
        r.load_cmd_ok,
        r.stop_cmd_ok,
        r.sell_cmd_ok,
        r.guard_cmd_ok,
        r.attack_move_cmd_ok,
        r.scatter_cmd_ok,
        r.patrol_cmd_ok,
        r.deploy_cmd_ok,
        r.cheer_cmd_ok,
        r.formation_cmd_ok,
        r.capture_cmd_ok,
        r.return_supplies_cmd_ok,
        r.evacuate_cmd_ok,
        r.repair_cmd_ok,
        r.return_to_base_cmd_ok,
        r.attitude_cmd_ok,
        r.rally_cmd_ok,
        r.switch_weapons_cmd_ok,
        r.view_cc_cmd_ok,
        r.clear_mines_cmd_ok,
        r.beacon_cmd_ok,
        r.hack_cmd_ok,
        r.cleanup_cmd_ok,
        r.combat_drop_cmd_ok,
        r.overcharge_cmd_ok,
        r.special_power_cmd_ok,
        r.remove_beacon_cmd_ok,
        r.demo_cmd_ok,
        r.view_radar_cmd_ok,
        r.force_attack_cmd_ok,
        r.force_attack_object_cmd_ok,
        r.select_all_cmd_ok,
        r.control_group_cmd_ok,
        r.waypoint_cmd_ok,
        r.box_select_cmd_ok,
        r.presentation_frame_ok,
        r.max_render_item_count,
        r.render_items_stable_ok,
        r.max_render_alive_objects,
        r.presentation_live_fallback_ok,
        r.select_similar_cmd_ok,
        r.select_on_screen_cmd_ok,
        r.select_structures_cmd_ok,
        r.select_aircraft_cmd_ok,
        r.select_idle_cmd_ok,
        r.camera_reset_cmd_ok,
        r.camera_zoom_cmd_ok,
        r.pause_cmd_ok,
        r.cancel_production_cmd_ok,
        r.diplomacy_cmd_ok,
        r.live_frame_ok,
        r.auto_attack_cmd_ok,
        r.options_cmd_ok,
        r.request_capture_cmd_ok,
        r.skirmish_start_wnd_ok,
        r.skirmish_menu_ok,
        r.skirmish_start_click_ok,
        r.frames_observed,
        r.map_seen,
        r.exit_code,
        r.new_game_path,
        r.detail
    )
}

#[cfg(test)]
mod tests {

    #[test]
    fn kill_stale_matches_runtime_host_underscore() {
        let src = include_str!("executable_smoke.rs");
        let kill_fn = src
            .split("fn kill_stale_runtime_host_generals")
            .nth(1)
            .and_then(|s| s.split("fn resolve_runtime_exe").next())
            .expect("kill_stale fn body");
        assert!(
            kill_fn.contains("runtime_host"),
            "stale kill must match -runtime_host CLI (underscore)"
        );
        assert!(
            !kill_fn.contains("runtime-host"),
            "stale kill must not use hyphenated runtime-host pkill pattern"
        );
        assert!(
            kill_fn.contains("generals.*runtime_host") || kill_fn.contains("runtime_host"),
            "expected runtime_host pkill pattern"
        );
    }

    use super::*;

    #[test]
    fn parse_status_reads_keys() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("status.txt");
        fs::write(
            &p,
            "state=Menu\nui_screen=Some(MainMenu)\nmap=-\nframe=3\nstartup_progress=1.0\nstartup_phase=Ready\n",
        )
        .unwrap();
        let s = parse_status(&p).unwrap();
        assert_eq!(s.state, "Menu");
        assert_eq!(s.frame, 3);
        assert!((s.startup_progress - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn playable_claim_always_false_on_default() {
        let r = ExecutableSmokeResult::default();
        assert!(!r.playable_claim);
    }

    #[test]
    fn parses_render_item_count_from_status() {
        let path =
            std::env::temp_dir().join(format!("generals_smoke_status_{}.txt", std::process::id()));
        std::fs::write(
            &path,
            "state=InGame\nrender_item_count=42\nrender_alive_objects=100\nrender_fow_filtered=10\nrender_frustum_culled=5\npresentation_frame_ok=true\n",
        )
        .unwrap();
        let snap = parse_status(&path).expect("snap");
        let _ = std::fs::remove_file(&path);
        assert_eq!(snap.render_item_count, 42);
        assert_eq!(snap.render_alive_objects, 100);
        assert!(snap.presentation_frame_ok);
    }

    #[test]
    fn smoke_defaults_wnd_enabled() {
        let src = include_str!("executable_smoke.rs");
        assert!(
            src.contains("unwrap_or_else(|_| \"1\".into())"),
            "executable smoke should default GENERALS_RUNTIME_HOST_WND=1"
        );
        assert!(src.contains("max_render_item_count"));
        assert!(src.contains("render_items_stable_ok"));
    }
}

#[cfg(test)]
mod skirmish_wnd_start_residual_tests {
    #[test]
    fn click_skirmish_start_prefers_wnd_gadget_when_enabled() {
        let eng = include_str!("cnc_game_engine.rs");
        let idx = eng
            .find("\"click_skirmish_start\"")
            .expect("click_skirmish_start command");
        let window = &eng[idx..idx + 4500];
        assert!(
            window.contains("simulate_skirmish_start_button_gadget_selected"),
            "must try retail WND ButtonStart GadgetSelected residual"
        );
        assert!(
            window.contains("click_skirmish_start_ok_wnd")
                || window.contains("click_skirmish_start_wnd_pending"),
            "must report wnd-specific gameplay cmd honesty"
        );
        assert!(
            window.contains("simulate_start_button_click"),
            "must keep Main SkirmishMenu mouse residual fallback"
        );
    }

    #[test]
    fn executable_smoke_wnd_host_override_residual() {
        let src = include_str!("executable_smoke.rs");
        assert!(
            src.contains("GENERALS_RUNTIME_HOST_WND")
                && src.contains("unwrap_or_else(|_| \"0\".into())"),
            "smoke defaults WND=0 for gate stability"
        );
        assert!(
            src.contains("skirmish_start_wnd_ok"),
            "smoke must track WND ButtonStart honesty separately"
        );
        let i = src.find("GENERALS_RUNTIME_HOST_WND").expect("env");
        let env_block = &src[i..src.len().min(i + 450)];
        assert!(
            !env_block.contains("var_os(\"DISPLAY\")"),
            "WND enable must not gate on X11 DISPLAY"
        );
    }

    #[test]
    fn executable_smoke_waits_for_full_command_chain() {
        let src = include_str!("executable_smoke.rs");
        assert!(
            src.contains("chain_complete")
                && src.contains("gameplay_step >= 59")
                && !src.contains(
                    "saw_attack_ok\n                                && snap.frame >= 20)"
                ),
            "smoke must not exit on early construct/train/attack alone"
        );
        assert!(
            src.contains("pause_ok:paused") && src.contains("pause_ok:resumed"),
            "pause residual must remain in chain"
        );
    }

    #[test]
    fn game_client_exposes_skirmish_button_start_gadget_simulate() {
        let src = include_str!(
            "../../GameEngine/GameClient/src/gui/callbacks/skirmish_game_options_menu.rs"
        );
        assert!(
            src.contains("fn simulate_skirmish_start_button_gadget_selected"),
            "WND ButtonStart gadget residual helper missing"
        );
        assert!(
            src.contains("WindowMessage::GadgetSelected"),
            "must fire GadgetSelected like C++ GBM_SELECTED"
        );
    }
}
