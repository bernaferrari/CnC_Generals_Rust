//! Multi-instance viewer: N independent Zero Hour engines in one process,
//! composited into one labelled grid (one tile per e2e scenario).
//!
//! ```text
//! generals_multi [--instances N] [--scenario S]... [--columns C]
//!                [--headless] [--shots DIR] [--shot-frame F] [--shot-timeout SECS]
//!                [--run-for SECS] [--present-fps FPS] [--boot-parallel K]
//!                [--instance-size WxH] [--cell-size WxH] [--audio] [-- <engine args>]
//! ```
//! Scenarios: `golden` (Lone Eagle skirmish, default), `shell`,
//! `skirmish:<map>[:<faction>]`, `cmds:<runtime-host cmd>;<cmd>`; they are
//! cycled across instances. Retail assets come from `GENERALS_ASSETS_DIR`.
//! `--headless` composites off-screen only and exits after the dump.
//! Exit codes: 0 all instances ran, 1 an instance failed, 3 usage.

use generals_main::cnc_game_engine::multi_instance::{
    InstanceScenario, MultiInstanceOptions, run_multi_instance,
};
use std::path::PathBuf;
use std::time::Duration;

fn usage() -> ! {
    eprintln!(
        "usage: generals_multi [--instances N] [--scenario golden|shell|skirmish:<map>[:<faction>]|cmds:<a>;<b>]... \
         [--columns C] [--headless] [--shots DIR] [--shot-frame F] [--shot-timeout SECS] [--run-for SECS] \
         [--present-fps FPS] [--boot-parallel K] [--instance-size WxH] [--cell-size WxH] [--audio] [-- <engine args>]"
    );
    std::process::exit(3);
}

fn size(value: &str) -> (u32, u32) {
    value
        .split_once(['x', 'X'])
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or_else(|| usage())
}

fn parse() -> MultiInstanceOptions {
    let mut options = MultiInstanceOptions::default();
    let mut scenarios = Vec::new();
    let mut cell_size = None;
    let mut audio = false;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        let mut value = || args.next().unwrap_or_else(|| usage());
        match flag.as_str() {
            "--instances" | "-n" => options.instances = value().parse().unwrap_or_else(|_| usage()),
            "--scenario" | "-s" => {
                scenarios.push(value().parse::<InstanceScenario>().unwrap_or_else(|e| {
                    eprintln!("generals_multi: {e}");
                    usage()
                }))
            }
            "--columns" => options.columns = Some(value().parse().unwrap_or_else(|_| usage())),
            "--headless" => options.windowed = false,
            "--shots" => options.shots_dir = Some(PathBuf::from(value())),
            "--shot-frame" => {
                options.shot_logic_frame = value().parse().unwrap_or_else(|_| usage())
            }
            "--shot-timeout" => {
                options.shot_timeout =
                    Duration::from_secs(value().parse().unwrap_or_else(|_| usage()))
            }
            "--run-for" => {
                options.run_for = Some(Duration::from_secs(
                    value().parse().unwrap_or_else(|_| usage()),
                ))
            }
            "--present-fps" => options.present_fps = value().parse().unwrap_or_else(|_| usage()),
            "--boot-parallel" => {
                options.boot_parallelism = value().parse().unwrap_or_else(|_| usage())
            }
            "--instance-size" => options.instance_size = size(&value()),
            "--cell-size" => cell_size = Some(size(&value())),
            "--userdata" => options.userdata_root = PathBuf::from(value()),
            "--audio" => audio = true,
            "--" => {
                options.extra_args.extend(args.by_ref());
                break;
            }
            "-h" | "--help" => usage(),
            _ => usage(),
        }
    }
    if !scenarios.is_empty() {
        options.scenarios = scenarios;
    }
    if !audio {
        // N audio devices add nothing to a visual e2e grid.
        options.extra_args.insert(0, "-noaudio".to_string());
    }
    options.cell_size = cell_size.unwrap_or(if options.instances > 8 {
        (360, 270)
    } else {
        (400, 300)
    });
    if options.headless_without_shots() {
        eprintln!("generals_multi: --headless needs --shots DIR");
        usage();
    }
    options
}

trait HeadlessCheck {
    fn headless_without_shots(&self) -> bool;
}

impl HeadlessCheck for MultiInstanceOptions {
    fn headless_without_shots(&self) -> bool {
        !self.windowed && self.shots_dir.is_none() && self.run_for.is_none()
    }
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .filter_module(
            "generals_main::cnc_game_engine::multi_instance",
            log::LevelFilter::Info,
        )
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        .init();
    let options = parse();
    // Same working directory `generals` normalizes to (WinMain parity).
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
    {
        let _ = std::env::set_current_dir(dir);
    }
    // SAFETY: set before any instance thread exists; process configuration,
    // identical for every instance (runtime-host WND shell, as golden_capture).
    unsafe { std::env::set_var("GENERALS_RUNTIME_HOST_WND", "1") };
    let event_loop = generals_main::win_main::create_host_event_loop(!options.windowed)
        .unwrap_or_else(|e| {
            eprintln!("generals_multi: event loop: {e}");
            std::process::exit(1);
        });
    let started = std::time::Instant::now();
    // The winit loop owns this thread until every instance has stopped.
    let report = run_multi_instance(event_loop, options);
    match report {
        Ok(report) => {
            println!(
                "generals_multi: ran {} instances for {:.0}s",
                report.instances.len(),
                started.elapsed().as_secs_f32()
            );
            print!("{}", report.summary());
            let failed = report
                .instances
                .iter()
                .any(|r| r.status.phase_name() == "FAILED");
            std::process::exit(i32::from(failed));
        }
        Err(e) => {
            eprintln!("generals_multi: {e:#}");
            std::process::exit(1);
        }
    }
}
