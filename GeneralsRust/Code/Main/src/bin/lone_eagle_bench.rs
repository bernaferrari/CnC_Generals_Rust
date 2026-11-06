//! Lone Eagle performance baseline: headless N-tick sim + windowed N-frame present.
//!
//! Writes `GeneralsRust/docs/lone_eagle_perf_baseline.md` and prints a summary.
//! Optional macOS `sample` capture lands beside the docs file when available.

use generals_main::executable_smoke::lone_eagle_map_on_disk;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Default, Clone)]
struct Snap {
    state: String,
    frame: u32,
    logic_frame: u32,
    present_ms: f32,
    present_collect_ms: f32,
    present_terrain_ms: f32,
    present_forward_ms: f32,
    radar_pixels: u32,
    render_terrain_chunks: u32,
    last_gameplay_cmd: String,
    startup_phase: String,
}

fn retail_root() -> Option<PathBuf> {
    for key in ["GENERALS_RETAIL_DIR", "GENERALS_ASSETS_DIR"] {
        if let Ok(dir) = std::env::var(key) {
            let p = PathBuf::from(dir);
            if p.join("Data/EnglishZH.big").is_file() {
                return Some(p.canonicalize().unwrap_or(p));
            }
        }
    }
    for c in [
        PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from(
            "/Users/bernardoferrari/Downloads/CnC_Generals_Zero_Hour-main/windows_game/Command & Conquer Generals Zero Hour",
        ),
    ] {
        if c.join("Data/EnglishZH.big").is_file() {
            return Some(c.canonicalize().unwrap_or(c));
        }
    }
    None
}

fn parse_status(path: &Path) -> Option<Snap> {
    let text = fs::read_to_string(path).ok()?;
    if text.trim().is_empty() {
        return None;
    }
    let mut s = Snap::default();
    for line in text.lines() {
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        match k.trim() {
            "state" => s.state = v.trim().to_string(),
            "frame" => s.frame = v.trim().parse().unwrap_or(0),
            "logic_frame" => s.logic_frame = v.trim().parse().unwrap_or(0),
            "present_ms" => s.present_ms = v.trim().parse().unwrap_or(0.0),
            "present_collect_ms" => s.present_collect_ms = v.trim().parse().unwrap_or(0.0),
            "present_terrain_ms" => s.present_terrain_ms = v.trim().parse().unwrap_or(0.0),
            "present_forward_ms" => s.present_forward_ms = v.trim().parse().unwrap_or(0.0),
            "radar_pixels" => s.radar_pixels = v.trim().parse().unwrap_or(0),
            "render_terrain_chunks" => s.render_terrain_chunks = v.trim().parse().unwrap_or(0),
            "last_gameplay_cmd" => s.last_gameplay_cmd = v.trim().to_string(),
            "startup_phase" => s.startup_phase = v.trim().to_string(),
            _ => {}
        }
    }
    Some(s)
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

fn find_generals_exe() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("GENERALS_RUNTIME_EXE") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace = manifest.parent()?.parent()?;
    // Prefer release for honest ms/tick / boot numbers.
    let candidates = [
        workspace.join("target/release/generals"),
        workspace.join("target/debug/generals"),
        PathBuf::from("target/release/generals"),
        PathBuf::from("target/debug/generals"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

struct BenchNumbers {
    mode: &'static str,
    ticks: u32,
    frames: u32,
    ms_per_tick: f64,
    ms_per_frame: f64,
    peak_radar_pixels: u32,
    peak_terrain_chunks: u32,
    avg_present_ms: f64,
    avg_collect_ms: f64,
    avg_terrain_ms: f64,
    avg_forward_ms: f64,
    sample_path: Option<PathBuf>,
}

fn mean(xs: &[f32]) -> f64 {
    if xs.is_empty() {
        return 0.0;
    }
    xs.iter().map(|v| f64::from(*v)).sum::<f64>() / xs.len() as f64
}

fn run_mode(
    launch: &str,
    width: u32,
    height: u32,
    n_ticks: u32,
    n_frames: u32,
    timeout: Duration,
    map: &str,
    docs_dir: &Path,
) -> Result<BenchNumbers, String> {
    let exe = find_generals_exe().ok_or_else(|| "generals binary missing".to_string())?;
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let tmp = std::env::temp_dir().join(format!(
        "lone_eagle_bench_{}_{}",
        launch,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&tmp);
    fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let control_path = tmp.join("control.txt");
    let status_path = tmp.join("status.txt");
    let frame_path = tmp.join("frame.png");
    let user_data = tmp.join("userdata");
    fs::create_dir_all(&user_data).map_err(|e| e.to_string())?;
    let _ = fs::File::create(&control_path);
    let _ = fs::File::create(&status_path);

    let runtime_host_arg = if launch == "headless" {
        "-runtime_host=headless"
    } else {
        "-runtime_host=windowed"
    };

    let mut child: Child = Command::new(&exe)
        .current_dir(&workspace)
        .arg(runtime_host_arg)
        .arg("-runtime_host_private_profile=1")
        .arg(format!("-userdata={}", user_data.display()))
        .arg("-windowed")
        .arg(format!("-width={width}"))
        .arg(format!("-height={height}"))
        .arg(format!("-gpui_control={}", control_path.display()))
        .arg(format!("-gpui_status={}", status_path.display()))
        .arg(format!("-gpui_frame={}", frame_path.display()))
        // Keep present_ms honest: no periodic PNG readback during the bench.
        .arg("-gpui_capture_interval_ms=0")
        .arg("-nologo")
        .arg("-nointro")
        .env(
            "GENERALS_RUNTIME_HOST_WND",
            std::env::var("GENERALS_RUNTIME_HOST_WND").unwrap_or_else(|_| "1".into()),
        )
        .env(
            "RUST_LOG",
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        )
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(
            fs::File::create(tmp.join("child_stderr.txt")).map_err(|e| e.to_string())?,
        )
        .spawn()
        .map_err(|e| format!("spawn failed: {e}"))?;

    let started = Instant::now();
    let mut phase = 0u8;
    let mut commanded_at = Instant::now();
    let mut measure_start: Option<Instant> = None;
    let mut start_logic = 0u32;
    let mut start_frame = 0u32;
    let mut present_samples: Vec<f32> = Vec::new();
    let mut collect_samples: Vec<f32> = Vec::new();
    let mut terrain_samples: Vec<f32> = Vec::new();
    let mut forward_samples: Vec<f32> = Vec::new();
    let mut peak_radar = 0u32;
    let mut peak_chunks = 0u32;
    let mut done = false;
    let mut sample_path = None;
    let mut boot_to_ingame: Option<f32> = None;

    while started.elapsed() < timeout && !done {
        if let Some(snap) = parse_status(&status_path) {
            peak_radar = peak_radar.max(snap.radar_pixels);
            peak_chunks = peak_chunks.max(snap.render_terrain_chunks);
            match phase {
                0 => {
                    // Do not drive menus while the host is still Loading catalogs;
                    // early start_game races Object module admission and rejects the map.
                    let ready = matches!(snap.state.as_str(), "MainMenu" | "Shell" | "Menu")
                        && !snap.startup_phase.to_ascii_lowercase().contains("load");
                    if ready {
                        let _ = write_control(&control_path, &["open_skirmish_menu"]);
                        phase = 1;
                        commanded_at = Instant::now();
                    }
                }
                1 => {
                    if snap.last_gameplay_cmd.starts_with("open_skirmish_menu_ok")
                        || commanded_at.elapsed() > Duration::from_secs(8)
                    {
                        // Match present_frame_gate: queue_new_game after Object
                        // catalogs finish admitting (direct start_game can race).
                        if commanded_at.elapsed() < Duration::from_secs(3)
                            && snap.last_gameplay_cmd.starts_with("open_skirmish_menu_ok")
                        {
                            // settle
                        } else {
                            let cmd = format!(
                                "queue_new_game|mode=skirmish|faction=USA|map={map}"
                            );
                            let _ = write_control(&control_path, &[&cmd]);
                            phase = 2;
                            commanded_at = Instant::now();
                        }
                    } else if commanded_at.elapsed() > Duration::from_secs(2) {
                        let _ = write_control(&control_path, &["open_skirmish_menu"]);
                        commanded_at = Instant::now();
                    }
                }
                2 => {
                    if snap.state != "InGame"
                        && commanded_at.elapsed() > Duration::from_secs(25)
                    {
                        let cmd = format!(
                            "start_game|mode=skirmish|faction=USA|map={map}"
                        );
                        let _ = write_control(&control_path, &[&cmd]);
                        commanded_at = Instant::now();
                    }
                    if snap.state == "InGame" && snap.logic_frame > 0 {
                        if boot_to_ingame.is_none() {
                            boot_to_ingame = Some(started.elapsed().as_secs_f32());
                            eprintln!(
                                "lone_eagle_bench: boot_to_InGame={:.2}s ({launch})",
                                boot_to_ingame.unwrap()
                            );
                        }
                        // Let AI settle a few frames, then measure.
                        if measure_start.is_none() {
                            if snap.logic_frame > 30 {
                                measure_start = Some(Instant::now());
                                start_logic = snap.logic_frame;
                                start_frame = snap.frame;
                                // Fire a macOS sample while measuring when available.
                                if launch == "windowed" {
                                    if let Ok(sample) = which_sample() {
                                        let out = docs_dir.join(format!(
                                            "lone_eagle_bench_{launch}_sample.txt"
                                        ));
                                        let _ = Command::new(sample)
                                            .args([
                                                &format!("{}", child.id()),
                                                "3",
                                                "-file",
                                                out.to_str().unwrap_or("/tmp/sample.txt"),
                                            ])
                                            .stdout(Stdio::null())
                                            .stderr(Stdio::null())
                                            .spawn();
                                        sample_path = Some(out);
                                    }
                                }
                            }
                        } else {
                            if snap.present_ms > 0.0 {
                                present_samples.push(snap.present_ms);
                                collect_samples.push(snap.present_collect_ms);
                                terrain_samples.push(snap.present_terrain_ms);
                                forward_samples.push(snap.present_forward_ms);
                            }
                            let logic_delta = snap.logic_frame.saturating_sub(start_logic);
                            let frame_delta = snap.frame.saturating_sub(start_frame);
                            if logic_delta >= n_ticks && frame_delta >= n_frames {
                                done = true;
                            }
                        }
                    } else if commanded_at.elapsed() > Duration::from_secs(20) {
                        let cmd = format!(
                            "start_game|mode=skirmish|faction=USA|map={map}"
                        );
                        let _ = write_control(&control_path, &[&cmd]);
                        commanded_at = Instant::now();
                    }
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    let measure_elapsed = measure_start
        .map(|t| t.elapsed())
        .unwrap_or_default()
        .as_secs_f64();
    let final_snap = parse_status(&status_path).unwrap_or_default();
    let logic_delta = final_snap.logic_frame.saturating_sub(start_logic).max(1);
    let frame_delta = final_snap.frame.saturating_sub(start_frame).max(1);
    let ms_per_tick = (measure_elapsed * 1000.0) / f64::from(logic_delta);
    let ms_per_frame = (measure_elapsed * 1000.0) / f64::from(frame_delta);

    // Surface sync stage lines from child stderr (RUST_LOG=info).
    if let Ok(err_txt) = fs::read_to_string(tmp.join("child_stderr.txt")) {
        for line in err_txt.lines() {
            if line.contains("Fast legacy")
                || line.contains("settings parse finished")
                || line.contains("heightmap ready")
                || line.contains("legacy runtime sync finished")
                || line.contains("scripts admitted")
            {
                eprintln!("lone_eagle_bench: stage| {line}");
            }
        }
    }
    if let Some(secs) = boot_to_ingame {
        eprintln!("lone_eagle_bench: summary boot_to_InGame={secs:.2}s launch={launch}");
    } else {
        eprintln!("lone_eagle_bench: summary boot_to_InGame=NOT_REACHED launch={launch}");
    }

    // One on-demand capture for artifacts / PNG gates (not during the timed window).
    if launch == "windowed" {
        let _ = write_control(&control_path, &["request_capture"]);
        let wait_deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < wait_deadline {
            if frame_path.is_file() {
                if let Ok(meta) = fs::metadata(&frame_path) {
                    if meta.len() > 1024 {
                        break;
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    let _ = write_control(&control_path, &["exit"]);
    thread::sleep(Duration::from_millis(500));
    let _ = child.kill();
    let _ = child.wait();

    if !done && measure_start.is_none() {
        return Err(format!(
            "{launch}: never reached measurable InGame (state={} logic={} cmd={})",
            final_snap.state, final_snap.logic_frame, final_snap.last_gameplay_cmd
        ));
    }

    Ok(BenchNumbers {
        mode: if launch == "headless" {
            "headless"
        } else {
            "windowed"
        },
        ticks: logic_delta,
        frames: frame_delta,
        ms_per_tick,
        ms_per_frame,
        peak_radar_pixels: peak_radar,
        peak_terrain_chunks: peak_chunks,
        avg_present_ms: mean(&present_samples),
        avg_collect_ms: mean(&collect_samples),
        avg_terrain_ms: mean(&terrain_samples),
        avg_forward_ms: mean(&forward_samples),
        sample_path,
    })
}

fn which_sample() -> Result<PathBuf, ()> {
    for candidate in ["/opt/homebrew/bin/sample", "/usr/bin/sample"] {
        let p = PathBuf::from(candidate);
        if p.is_file() {
            return Ok(p);
        }
    }
    Err(())
}

fn write_docs(path: &Path, rows: &[BenchNumbers], notes: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::File::create(path)?;
    writeln!(f, "# Lone Eagle performance baseline")?;
    writeln!(f)?;
    writeln!(
        f,
        "Captured by `lone_eagle_bench` on the Mac playable worktree. Times are wall-clock over observed logic/present deltas after InGame settle."
    )?;
    writeln!(f)?;
    writeln!(
        f,
        "| mode | ticks | frames | ms/tick | ms/frame | avg present_ms | collect | terrain | forward | radar_pixels | terrain_chunks |"
    )?;
    writeln!(
        f,
        "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"
    )?;
    for r in rows {
        writeln!(
            f,
            "| {} | {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} | {} |",
            r.mode,
            r.ticks,
            r.frames,
            r.ms_per_tick,
            r.ms_per_frame,
            r.avg_present_ms,
            r.avg_collect_ms,
            r.avg_terrain_ms,
            r.avg_forward_ms,
            r.peak_radar_pixels,
            r.peak_terrain_chunks
        )?;
    }
    writeln!(f)?;
    writeln!(f, "## Hot spots")?;
    writeln!(f)?;
    writeln!(
        f,
        "Present breakdown comes from `RenderPipeline::execute` residuals (`present_collect_ms` / `present_terrain_ms` / `present_forward_ms`). Larger of the three is the leading GPU/CPU present cost."
    )?;
    for r in rows {
        let mut parts = BTreeMap::new();
        parts.insert("collect", r.avg_collect_ms);
        parts.insert("terrain", r.avg_terrain_ms);
        parts.insert("forward", r.avg_forward_ms);
        let mut ordered: Vec<_> = parts.into_iter().collect();
        ordered.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        writeln!(
            f,
            "- **{}**: top present stages {:?}{}",
            r.mode,
            ordered,
            r.sample_path
                .as_ref()
                .map(|p| format!("; sample={}", p.display()))
                .unwrap_or_default()
        )?;
    }
    writeln!(f)?;
    writeln!(f, "## Notes")?;
    writeln!(f)?;
    writeln!(f, "{notes}")?;
    Ok(())
}

fn main() {
    let n_ticks: u32 = std::env::var("LONE_EAGLE_BENCH_TICKS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let n_frames: u32 = std::env::var("LONE_EAGLE_BENCH_FRAMES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(180);
    let timeout = Duration::from_secs(
        std::env::var("LONE_EAGLE_BENCH_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(600),
    );
    let Some(retail) = retail_root() else {
        eprintln!("lone_eagle_bench: SKIP retail missing");
        std::process::exit(0);
    };
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        // SAFETY: set in main() before any benchmark thread or child process is spawned, so nothing reads the environment concurrently.
        unsafe {
            std::env::set_var("GENERALS_ASSETS_DIR", &retail);
        }
    }
    if !lone_eagle_map_on_disk() {
        eprintln!("lone_eagle_bench: SKIP Lone Eagle missing");
        std::process::exit(0);
    }
    // Prefer an absolute Lone Eagle path so the child (cwd=GeneralsRust) does
    // not resolve a relative ../windows_game path through a different root.
    let map = generals_main::executable_smoke::LONE_EAGLE_CANDIDATES
        .iter()
        .map(PathBuf::from)
        .find(|p| p.is_file())
        .and_then(|p| p.canonicalize().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Lone Eagle".to_string());
    eprintln!("lone_eagle_bench: map={map}");
    let docs_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../docs");
    let docs_path = docs_dir.join("lone_eagle_perf_baseline.md");
    let mut rows = Vec::new();
    let mut notes = String::new();

    // WINDOWED_FIRST
    match run_mode(
        "windowed",
        1280,
        720,
        n_ticks,
        n_frames,
        timeout,
        &map,
        &docs_dir,
    ) {
        Ok(r) => {
            eprintln!(
                "lone_eagle_bench: windowed ms/frame={:.3} present_ms={:.3} (collect={:.3} terrain={:.3} forward={:.3}) radar_pixels={}",
                r.ms_per_frame,
                r.avg_present_ms,
                r.avg_collect_ms,
                r.avg_terrain_ms,
                r.avg_forward_ms,
                r.peak_radar_pixels
            );
            rows.push(r);
        }
        Err(e) => {
            eprintln!("lone_eagle_bench: windowed FAIL {e}");
            notes.push_str(&format!("- windowed failed: {e}\n"));
        }
    }

    match run_mode(
        "headless",
        640,
        480,
        n_ticks,
        1,
        timeout,
        &map,
        &docs_dir,
    ) {
        Ok(r) => {
            eprintln!(
                "lone_eagle_bench: headless ms/tick={:.3} ticks={} radar_pixels={}",
                r.ms_per_tick, r.ticks, r.peak_radar_pixels
            );
            rows.push(r);
        }
        Err(e) => {
            eprintln!("lone_eagle_bench: headless FAIL {e}");
            notes.push_str(&format!("- headless failed: {e}\n"));
        }
    }


    if notes.is_empty() {
        notes.push_str("- Baseline captured successfully.\n");
    }
    if let Err(e) = write_docs(&docs_path, &rows, &notes) {
        eprintln!("lone_eagle_bench: docs write failed: {e}");
        std::process::exit(1);
    }
    println!("lone_eagle_bench: wrote {}", docs_path.display());
    if rows.is_empty() {
        std::process::exit(1);
    }
}
