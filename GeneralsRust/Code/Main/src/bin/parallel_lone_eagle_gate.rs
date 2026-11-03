//! Gate: two Lone Eagle (+AI) skirmishes in parallel under Barrier sync.
//!
//! Exit codes:
//!   0 — match under same orders and diverge under different tokens
//!   1 — assertion failure / load failure
//!   2 — Lone Eagle assets unavailable (honest skip)

use generals_main::map_frame_scenario::run_parallel_lone_eagle_skirmishes;

fn main() {
    match run_parallel_lone_eagle_skirmishes() {
        Ok(()) => {
            println!("parallel_lone_eagle_gate: PASS");
            std::process::exit(0);
        }
        Err(msg) if msg.contains("assets missing") => {
            eprintln!("parallel_lone_eagle_gate: SKIP ({msg})");
            std::process::exit(2);
        }
        Err(msg) => {
            eprintln!("parallel_lone_eagle_gate: FAIL ({msg})");
            std::process::exit(1);
        }
    }
}
