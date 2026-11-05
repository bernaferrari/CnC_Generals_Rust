//! Golden-R harness (renderer plan R0): deterministic headless capture of one
//! frozen frame, optionally compared pixel-for-pixel with a reference PNG.
//!
//! ```text
//! golden_capture <out.png> [--scene shell|lone-eagle] [--frame N]
//!                [--compare ref.png] [--timeout SECS]
//! ```
//! The child `generals` runs headless (`-runtime_host=headless`, 800x600) with
//! `GENERALS_GOLDEN_CAPTURE`; see `cnc_game_engine::golden_capture` for the
//! freeze rules. Retail assets come from `GENERALS_ASSETS_DIR` /
//! `GENERALS_RETAIL_DIR`; without them the harness skips (exit 0).
//! `golden_capture --diff a.png b.png` only compares two existing captures.
//! Exit codes: 0 pass/skip, 1 capture failed, 2 image differs, 3 usage.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Scene {
    /// ShellMapMD behind the main menu.
    Shell,
    /// Lone Eagle skirmish started through `start_game`.
    LoneEagle,
}

struct Args {
    out: PathBuf,
    scene: Scene,
    frame: u32,
    compare: Option<PathBuf>,
    timeout: Duration,
}

fn parse_args() -> Option<Args> {
    let mut it = std::env::args().skip(1);
    let out = PathBuf::from(it.next()?);
    let mut args = Args {
        out,
        scene: Scene::Shell,
        frame: 60,
        compare: None,
        timeout: Duration::from_secs(900),
    };
    while let Some(flag) = it.next() {
        let value = it.next()?;
        match flag.as_str() {
            "--scene" => {
                args.scene = match value.as_str() {
                    "shell" => Scene::Shell,
                    "lone-eagle" => Scene::LoneEagle,
                    _ => return None,
                }
            }
            "--frame" => args.frame = value.parse().ok()?,
            "--compare" => args.compare = Some(PathBuf::from(value)),
            "--timeout" => args.timeout = Duration::from_secs(value.parse().ok()?),
            _ => return None,
        }
    }
    Some(args)
}

fn retail_root() -> Option<PathBuf> {
    ["GENERALS_ASSETS_DIR", "GENERALS_RETAIL_DIR"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .find(|root| root.join("Data/EnglishZH.big").is_file())
}

fn lone_eagle_map(retail: &Path) -> Option<PathBuf> {
    let relative = Path::new("Maps/Lone Eagle/Lone Eagle.map");
    let mut candidates = vec![retail.join(relative)];
    if let Some(parent) = retail.parent() {
        for extract in ["extracted_big_files", "extracted_big_files_v2"] {
            candidates.push(parent.join(extract).join("MapsZH").join(relative));
        }
    }
    candidates.into_iter().find(|path| path.is_file())
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn generals_exe() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GENERALS_RUNTIME_EXE") {
        return Some(PathBuf::from(path));
    }
    let root = workspace_root();
    ["target/release/generals", "target/debug/generals"]
        .into_iter()
        .map(|rel| root.join(rel))
        .find(|path| path.is_file())
}

fn status_state(status: &Path) -> Option<String> {
    let text = std::fs::read_to_string(status).ok()?;
    text.lines()
        .find_map(|line| line.strip_prefix("state="))
        .map(|state| state.trim().to_string())
}

/// Pixel comparison of two captures.
struct Diff {
    differing: u64,
    max_delta: u8,
    /// Bounding box of differing pixels: (min_x, min_y, max_x, max_y).
    bounds: Option<(u32, u32, u32, u32)>,
}

fn compare_png(a: &Path, b: &Path) -> Result<Diff, String> {
    let a = image::open(a).map_err(|e| format!("{}: {e}", a.display()))?.to_rgba8();
    let b = image::open(b).map_err(|e| format!("{}: {e}", b.display()))?.to_rgba8();
    if a.dimensions() != b.dimensions() {
        return Err(format!("size {:?} vs {:?}", a.dimensions(), b.dimensions()));
    }
    let mut diff = Diff {
        differing: 0,
        max_delta: 0,
        bounds: None,
    };
    for ((x, y, pa), pb) in a.enumerate_pixels().zip(b.pixels()) {
        let delta = pa.0.iter().zip(pb.0.iter()).map(|(p, q)| p.abs_diff(*q)).max();
        if let Some(delta) = delta.filter(|delta| *delta > 0) {
            diff.differing += 1;
            diff.max_delta = diff.max_delta.max(delta);
            diff.bounds = Some(match diff.bounds {
                None => (x, y, x, y),
                Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
            });
        }
    }
    Ok(diff)
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.len() == 4 && argv[1] == "--diff" {
        report_compare(Path::new(&argv[2]), Path::new(&argv[3]));
        return;
    }
    let Some(args) = parse_args() else {
        eprintln!(
            "usage: golden_capture <out.png> [--scene shell|lone-eagle] [--frame N] [--compare ref.png] [--timeout SECS]"
        );
        std::process::exit(3);
    };
    let Some(retail) = retail_root() else {
        eprintln!("golden_capture: SKIP retail assets missing (set GENERALS_ASSETS_DIR)");
        return;
    };
    let Some(exe) = generals_exe() else {
        eprintln!("golden_capture: SKIP generals binary missing");
        return;
    };
    let map = match args.scene {
        Scene::Shell => None,
        Scene::LoneEagle => match lone_eagle_map(&retail) {
            Some(map) => Some(map),
            None => {
                eprintln!("golden_capture: SKIP Lone Eagle map missing");
                return;
            }
        },
    };

    let work = std::env::temp_dir().join(format!("golden_capture_{}", std::process::id()));
    let profile = work.join("profile");
    if let Err(err) = std::fs::create_dir_all(&profile) {
        eprintln!("golden_capture: cannot create {}: {err}", profile.display());
        std::process::exit(1);
    }
    let control = work.join("control.txt");
    let status = work.join("status.txt");
    let _ = std::fs::write(&control, b"");
    let _ = std::fs::write(&status, b"");
    let _ = std::fs::remove_file(&args.out);
    let stderr = std::fs::File::create(work.join("stderr.txt")).ok();

    let mut child = match Command::new(&exe)
        .current_dir(workspace_root())
        .args([
            "-runtime_host=headless",
            "-runtime_host_private_profile=1",
            "-windowed",
            "-width=800",
            "-height=600",
            "-nologo",
            "-nointro",
        ])
        .arg(format!("-userdata={}", profile.display()))
        .arg(format!("-gpui_control={}", control.display()))
        .arg(format!("-gpui_status={}", status.display()))
        .arg(format!("-gpui_frame={}", work.join("frame.png").display()))
        .env("GENERALS_ASSETS_DIR", &retail)
        .env("GENERALS_RUNTIME_HOST_WND", "1")
        .env("GENERALS_GOLDEN_CAPTURE", &args.out)
        .env(
            "GENERALS_GOLDEN_STATE",
            if map.is_some() { "ingame" } else { "menu" },
        )
        .env("GENERALS_GOLDEN_LOGIC_FRAME", args.frame.to_string())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr.map_or_else(Stdio::null, Stdio::from))
        .spawn()
    {
        Ok(child) => child,
        Err(err) => {
            eprintln!("golden_capture: spawn {} failed: {err}", exe.display());
            std::process::exit(1);
        }
    };

    let started = Instant::now();
    let mut start_sent = false;
    let exited = loop {
        if let Ok(Some(_)) = child.try_wait() {
            break true;
        }
        if started.elapsed() > args.timeout {
            let _ = child.kill();
            let _ = child.wait();
            break false;
        }
        if let (Some(map), false) = (&map, start_sent) {
            if status_state(&status).as_deref() == Some("Menu") {
                let command = format!("start_game|mode=skirmish|map={}\n", map.display());
                start_sent = std::fs::write(&control, command).is_ok();
            }
        }
        std::thread::sleep(Duration::from_millis(500));
    };
    if !exited || !args.out.is_file() {
        eprintln!(
            "golden_capture: FAIL no capture after {:?} (exited={exited}); logs in {}",
            started.elapsed(),
            work.display()
        );
        std::process::exit(1);
    }
    let stats = args.out.with_extension("stats.txt");
    println!(
        "golden_capture: wrote {} in {:?}; {}",
        args.out.display(),
        started.elapsed(),
        std::fs::read_to_string(&stats).unwrap_or_default().trim()
    );
    let _ = std::fs::remove_dir_all(&work);

    if let Some(reference) = args.compare {
        report_compare(&reference, &args.out);
    }
}

fn report_compare(reference: &Path, capture: &Path) {
    match compare_png(reference, capture) {
        Ok(diff) if diff.differing == 0 => {
            println!("golden_capture: IDENTICAL to {}", reference.display());
        }
        Ok(diff) => {
            println!(
                "golden_capture: DIFFERS from {}: {} pixels, max channel delta {}, bounds {:?}",
                reference.display(),
                diff.differing,
                diff.max_delta,
                diff.bounds
            );
            std::process::exit(2);
        }
        Err(err) => {
            eprintln!("golden_capture: compare failed: {err}");
            std::process::exit(1);
        }
    }
}
