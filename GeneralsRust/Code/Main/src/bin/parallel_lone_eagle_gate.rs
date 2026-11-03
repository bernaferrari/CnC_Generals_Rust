//! Gate: two Lone Eagle (+AI) skirmishes in parallel under Barrier sync.
//!
//! Reuses the retail asset root (`GENERALS_ASSETS_DIR` →
//! `windows_game/Command & Conquer Generals Zero Hour`) and the same
//! in-process GameData + golden skirmish admit path as the playable Lone Eagle
//! hosts. Exit codes:
//!   0 — both skirmishes load, match under same orders, diverge under tokens
//!   1 — load or ownership assertion failure
//!   2 — retail assets unavailable (honest skip)

use generals_main::map_frame_scenario::run_parallel_lone_eagle_skirmishes;

fn main() {
    match run_parallel_lone_eagle_skirmishes() {
        Ok(()) => {
            println!("parallel_lone_eagle_gate: PASS");
            std::process::exit(0);
        }
        Err(msg) if msg.contains("retail assets missing") || msg.contains("not found after retail") => {
            eprintln!("parallel_lone_eagle_gate: SKIP ({msg})");
            std::process::exit(2);
        }
        Err(msg) => {
            eprintln!("parallel_lone_eagle_gate: FAIL ({msg})");
            std::process::exit(1);
        }
    }
}
