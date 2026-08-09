//! MapCacheBuilder Main Entry Point
//!
//! Corresponds to C++ file: Tools/MapCacheBuilder/Source/WinMain.cpp
//!
//! Default build is CLI. Optional chrome: `cargo run -p map_cache_builder --features ui -- --ui`.

use anyhow::Result;
use map_cache_builder::run_cli;

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("MapCacheBuilder starting...");
    log::info!("Command & Conquer Generals Zero Hour - Map Cache Builder");

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--ui" || a == "-ui") {
        #[cfg(feature = "ui")]
        {
            return map_cache_builder::ui::run_ui();
        }
        #[cfg(not(feature = "ui"))]
        {
            anyhow::bail!(
                "UI chrome requires rebuilding with `--features ui` (default remains CLI)"
            );
        }
    }

    run_cli(&args)
}
