//! Gate: retail skirmish AI activity + concurrent human move (hq-x1j4u.9.5).
use generals_main::executable_smoke::{
    format_executable_smoke_report, lone_eagle_map_on_disk, run_ai_opponent_skirmish,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn retail_root() -> Option<PathBuf> {
    for key in ["GENERALS_RETAIL_DIR", "GENERALS_ASSETS_DIR"] {
        if let Ok(dir) = std::env::var(key) {
            let p = PathBuf::from(dir);
            if p.join("Data/EnglishZH.big").is_file()
                || Path::new(
                    "windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map",
                )
                .is_file()
            {
                return Some(p);
            }
        }
    }
    let c = PathBuf::from("windows_game/Command & Conquer Generals Zero Hour");
    if c.join("Data/EnglishZH.big").is_file() {
        return Some(c.canonicalize().unwrap_or(c));
    }
    None
}

fn main() {
    let timeout: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            std::env::var("AI_RETAIL_SKIRMISH_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(900);
    let Some(retail) = retail_root() else {
        eprintln!("ai_retail_skirmish_gate: SKIP retail missing");
        std::process::exit(0);
    };
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        unsafe {
            std::env::set_var("GENERALS_ASSETS_DIR", &retail);
        }
    }
    if !lone_eagle_map_on_disk() {
        eprintln!("ai_retail_skirmish_gate: SKIP Lone Eagle missing");
        std::process::exit(0);
    }
    eprintln!(
        "ai_retail_skirmish_gate: retail={} timeout={}s",
        retail.display(),
        timeout
    );
    let r = run_ai_opponent_skirmish(Duration::from_secs(timeout));
    println!("{}", format_executable_smoke_report(&r));
    if r.status == "binary_missing" {
        eprintln!("ai_retail_skirmish_gate: SKIP binary_missing — {}", r.detail);
        std::process::exit(0);
    }
    let pass = r.executable_host_ok
        && r.reached_ingame
        && r.gameplay_cmd_ok
        && (r.status == "success" || r.status == "success_forced_exit");
    if pass {
        println!(
            "ai_retail_skirmish_gate: PASS map={} gameplay={} detail={}",
            r.map_seen, r.gameplay_cmd_ok, r.detail
        );
        std::process::exit(0);
    }
    eprintln!(
        "ai_retail_skirmish_gate: FAIL status={} host_ok={} ingame={} gameplay={} detail={}",
        r.status, r.executable_host_ok, r.reached_ingame, r.gameplay_cmd_ok, r.detail
    );
    std::process::exit(1);
}
