//! Multi-instance viewer: N independent Zero Hour engines in one process,
//! composited into one labelled grid (one tile per e2e scenario).
//!
//! ```text
//! generals_multi [--instances N] [--scenario S]... [--columns C]
//!                [--headless] [--shots DIR] [--shot-frame F] [--shot-timeout SECS]
//!                [--run-for SECS] [--present-fps FPS] [--update-hz HZ (60; 0 uncapped)]
//!                [--boot-parallel K]
//!                [--instance-size WxH] [--cell-size WxH] [--audio]
//!                [--suite e2e] [--gate-timeout SECS] [--pass-frame F]
//!                [--verdict-timeout SECS] [--until-verdicts] [-- <engine args>]
//! ```
//! Scenarios: `golden` (Lone Eagle skirmish, default), `shell`,
//! `skirmish:<map>[:<faction>]`, `cmds:<runtime-host cmd>;<cmd>`, and
//! `gate:<menu|map_load|orders|ai|save_load>[:<map>]` (a retail executable
//! gate's smoke driver attached to the tile); they are cycled across
//! instances. `--suite e2e` fills the grid with every retail gate, the
//! golden skirmish, the shell and skirmishes on other maps/factions. Each
//! tile shows a PASS/FAIL verdict. Retail assets come from
//! `GENERALS_ASSETS_DIR`; extra maps from `GENERALS_MAPS_DIR`.
//! `--headless` composites off-screen only and exits after the dump.
//! Exit codes: 0 every tile passed (or ran, if undecided), 1 a tile failed,
//! 3 usage.

#[cfg(not(target_arch = "wasm32"))]
use generals_main::cnc_game_engine::multi_instance::{
    InstanceScenario, MultiInstanceOptions, e2e_suite, run_multi_instance,
};
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
fn usage() -> ! {
    eprintln!(
        "usage: generals_multi [--instances N] [--scenario golden|shell|skirmish:<map>[:<faction>]|cmds:<a>;<b>]... \
         [--columns C] [--headless] [--shots DIR] [--shot-frame F] [--shot-timeout SECS] [--run-for SECS] \
         [--present-fps FPS] [--update-hz HZ] [--boot-parallel K] [--instance-size WxH] [--cell-size WxH] [--audio] [--suite e2e] [--gate-timeout SECS] \
         [--pass-frame F] [--verdict-timeout SECS] [--until-verdicts] [-- <engine args>]\n\
         scenarios also: gate:<menu|map_load|orders|ai|save_load>[:<map>]"
    );
    std::process::exit(3);
}

#[cfg(not(target_arch = "wasm32"))]
fn size(value: &str) -> (u32, u32) {
    value
        .split_once(['x', 'X'])
        .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
        .unwrap_or_else(|| usage())
}

#[cfg(not(target_arch = "wasm32"))]
fn parse() -> MultiInstanceOptions {
    let mut options = MultiInstanceOptions::default();
    let mut scenarios = Vec::new();
    let mut cell_size = None;
    let mut audio = false;
    let mut suite = false;
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
            "--shots" => options.shots_dir = Some(absolute(value())),
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
            "--update-hz" => options.update_hz = value().parse().unwrap_or_else(|_| usage()),
            "--boot-parallel" => {
                options.boot_parallelism = value().parse().unwrap_or_else(|_| usage())
            }
            "--instance-size" => options.instance_size = size(&value()),
            "--cell-size" => cell_size = Some(size(&value())),
            "--userdata" => options.userdata_root = absolute(value()),
            "--audio" => audio = true,
            "--suite" => match value().as_str() {
                "e2e" => suite = true,
                _ => usage(),
            },
            "--gate-timeout" => {
                options.gate_timeout =
                    Duration::from_secs(value().parse().unwrap_or_else(|_| usage()))
            }
            "--pass-frame" => {
                options.assertions.pass_logic_frame = value().parse().unwrap_or_else(|_| usage())
            }
            "--verdict-timeout" => {
                options.assertions.timeout =
                    Duration::from_secs(value().parse().unwrap_or_else(|_| usage()))
            }
            "--until-verdicts" => options.stop_when_decided = true,
            "--" => {
                options.extra_args.extend(args.by_ref());
                break;
            }
            "-h" | "--help" => usage(),
            _ => usage(),
        }
    }
    if suite {
        options.scenarios = e2e_suite(options.instances);
        options.stop_when_decided = true;
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

/// Resolve against the launch directory: the process later moves to the exe dir.
#[cfg(not(target_arch = "wasm32"))]
fn absolute(path: String) -> PathBuf {
    std::path::absolute(&path).unwrap_or_else(|_| PathBuf::from(path))
}

#[cfg(not(target_arch = "wasm32"))]
trait HeadlessCheck {
    fn headless_without_shots(&self) -> bool;
}

#[cfg(not(target_arch = "wasm32"))]
impl HeadlessCheck for MultiInstanceOptions {
    fn headless_without_shots(&self) -> bool {
        !self.windowed && self.shots_dir.is_none() && self.run_for.is_none()
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .filter_module(
            "generals_main::cnc_game_engine::multi_instance",
            log::LevelFilter::Info,
        )
        .filter_module("wgpu_core", log::LevelFilter::Error)
        .filter_module("wgpu_hal", log::LevelFilter::Error)
        // Instance threads are named `generals-instance-N`: tag every line.
        .format(|buf, record| {
            use std::io::Write;
            let thread = std::thread::current();
            writeln!(
                buf,
                "[{} {} {}] {}",
                thread.name().unwrap_or("?"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();
    let options = parse();
    // Same working directory `generals` normalizes to (WinMain parity).
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
    {
        let _ = std::env::set_current_dir(dir);
    }
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
            let (_, verdict_failures, _) = report.verdict_counts();
            let failed = verdict_failures > 0
                || report
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

#[cfg(target_arch = "wasm32")]
fn main() {
    panic!("generals_multi requires a native host");
}
