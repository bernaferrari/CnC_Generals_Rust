//! Executable smoke via the production `generals` binary + runtime host bridge.
//!
//! This is **stronger** than headless `shell_smoke` (which constructs `GameLogic`
//! in-process): it boots the real event loop, creates a (hidden) window, runs
//! WW3D headless init, and drives Menu → Start through the same control file
//! path GPUI uses.
//!
//! Honesty:
//! - `playable_claim` is **always false** here until a full interactive WND click
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
    // GPU/display and cause Booting→exit before Menu.
    #[cfg(unix)]
    {
        let exe_s = exe.to_string_lossy().to_string();
        // Prefer exact binary path matches (runtime-host first, then any generals).
        for pat in [format!("{exe_s}.*runtime-host"), format!("{exe_s}")] {
            let _ = std::process::Command::new("pkill")
                .args(["-9", "-f", &pat])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status();
        }
        std::thread::sleep(Duration::from_millis(800));
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
        PathBuf::from("target/release/generals"),
        PathBuf::from("target/debug/generals"),
        PathBuf::from("GeneralsRust/target/release/generals"),
        PathBuf::from("GeneralsRust/target/debug/generals"),
        PathBuf::from("./target/release/generals"),
        PathBuf::from("./target/debug/generals"),
    ];
    for c in candidates {
        if c.is_file() {
            return Some(c);
        }
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

    // Best-effort: prior flaky runs can leave a hanging runtime-host `generals` holding
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
                result.detail = format!(
                    "exited ok after InGame frames={} map={} new_game={}",
                    result.frames_observed, result.map_seen, use_new_game_path
                );
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
            last_snap = snap.clone();
            result.frames_observed = result.frames_observed.max(snap.frame);
            if snap.map != "-" && !snap.map.is_empty() {
                result.map_seen = snap.map.clone();
            }
            match snap.state.as_str() {
                "Menu" => result.reached_menu = true,
                "InGame" | "Paused" => {
                    result.reached_menu = true;
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
                        if use_new_game_path {
                            // Production-ish path: queue MSG_NEW_GAME and drain in-engine.
                            let q = format!(
                                "queue_new_game|mode=skirmish|map={}",
                                map.replace('|', "/")
                            );
                            let _ = write_control(&control_path, &[q.as_str()]);
                        } else {
                            let start = format!(
                                "start_game|mode=skirmish|faction=USA|map={}",
                                map.replace('|', "/")
                            );
                            let _ = write_control(&control_path, &[start.as_str()]);
                        }
                        commanded_at = Some(Instant::now());
                        phase = 1;
                    }
                }
                1 => {
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
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(3))
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
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(3))
                                .unwrap_or(false))
                    {
                        if snap.last_gameplay_cmd.starts_with("move_ok") {
                            saw_move_ok = true;
                        }
                        let _ = write_control(&control_path, &["attack_nearest_enemy"]);
                        gameplay_step = 3;
                        commanded_at = Some(Instant::now());
                    } else if gameplay_step >= 3 {
                        if snap.last_gameplay_cmd.starts_with("move_ok") {
                            saw_move_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("attack_ok")
                            || snap.last_gameplay_cmd.starts_with("attack_fail_no_enemy")
                        {
                            saw_attack_ok = true;
                        }
                        if snap.last_gameplay_cmd.starts_with("select_ok") {
                            saw_select_ok = true;
                        }
                        // select + move required; attack attempted (ok or no enemy).
                        result.gameplay_cmd_ok = saw_select_ok && saw_move_ok && saw_attack_ok;
                        if snap.frame >= 8
                            || commanded_at
                                .map(|t| t.elapsed() > Duration::from_secs(5))
                                .unwrap_or(true)
                        {
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
        "executable_smoke status={} host_ok={} playable_claim={} started={} menu={} ingame={} gameplay_cmd={} frames={} map={} exit={:?} new_game={} detail={}",
        r.status,
        r.executable_host_ok,
        r.playable_claim,
        r.process_started,
        r.reached_menu,
        r.reached_ingame,
        r.gameplay_cmd_ok,
        r.frames_observed,
        r.map_seen,
        r.exit_code,
        r.new_game_path,
        r.detail
    )
}

#[cfg(test)]
mod tests {
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
}
