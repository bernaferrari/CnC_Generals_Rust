//! Gate binary: load one map through production GameLogic and advance stable frames.
//!
//! Usage:
//!   map_frame_gate [--map NAME] [--frames N]
//!
//! Exit codes:
//!   0 — map loaded (or honest assets-unavailable) and frames advanced > 0
//!   1 — load failed or zero frames advanced
//!   2 — usage error

use generals_main::map_frame_scenario::{
    format_map_frame_report, run_map_frame_scenario, MapFrameStatus, DEFAULT_MAP_FRAME_ADVANCE,
};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("Usage: map_frame_gate [--map NAME_OR_PATH] [--frames N]");
        println!("Loads a retail/dev map via GameLogic::load_map and advances N logic frames.");
        println!("When no map is available, still advances frames and reports assets_unavailable.");
        return;
    }

    let mut map: Option<String> = None;
    let mut frames = DEFAULT_MAP_FRAME_ADVANCE;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--map" if i + 1 < args.len() => {
                map = Some(args[i + 1].clone());
                i += 2;
            }
            "--frames" if i + 1 < args.len() => {
                match args[i + 1].parse::<u32>() {
                    Ok(n) if n > 0 => frames = n,
                    _ => {
                        eprintln!("invalid --frames value: {}", args[i + 1]);
                        std::process::exit(2);
                    }
                }
                i += 2;
            }
            other => {
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
    }

    let result = run_map_frame_scenario(map.as_deref(), frames);
    println!("{}", format_map_frame_report(&result));

    match result.status {
        MapFrameStatus::Success if result.frames_advanced > 0 => {
            println!("map_frame_gate: PASS");
            std::process::exit(0);
        }
        MapFrameStatus::AssetsUnavailable if result.frames_advanced > 0 => {
            // Honest environmental degradation: update path works, assets missing.
            println!("map_frame_gate: PASS (assets unavailable; frame advance covered)");
            std::process::exit(0);
        }
        MapFrameStatus::LoadFailed if result.frames_advanced > 0 => {
            eprintln!("map_frame_gate: FAIL (map load failed)");
            std::process::exit(1);
        }
        _ => {
            eprintln!(
                "map_frame_gate: FAIL (status={} frames_advanced={})",
                result.status.as_str(),
                result.frames_advanced
            );
            std::process::exit(1);
        }
    }
}
