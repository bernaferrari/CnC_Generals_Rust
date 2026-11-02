//! Gate: retail skirmish select → move → attack (hq-x1j4u.9.3).
use generals_main::executable_smoke::{
    format_executable_smoke_report, lone_eagle_map_on_disk, run_issue_player_orders,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn retail_root() -> Option<PathBuf> {
    for key in ["GENERALS_RETAIL_DIR", "GENERALS_ASSETS_DIR"] {
        if let Ok(dir) = std::env::var(key) {
            let p = PathBuf::from(dir);
            if looks_valid(&p) { return Some(p); }
        }
    }
    for c in [
        PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("windows_game"),
    ] {
        if looks_valid(&c) { return Some(c.canonicalize().unwrap_or(c)); }
    }
    None
}
fn looks_valid(root: &Path) -> bool {
    root.join("Data/EnglishZH.big").is_file()
        || Path::new("windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map").is_file()
}

fn main() {
    let timeout: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok())
        .or_else(|| std::env::var("PLAYER_ORDERS_TIMEOUT_SECS").ok().and_then(|s| s.parse().ok()))
        .unwrap_or(720);
    let Some(retail) = retail_root() else {
        eprintln!("player_orders_gate: SKIP retail missing");
        std::process::exit(0);
    };
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        unsafe { std::env::set_var("GENERALS_ASSETS_DIR", &retail); }
    }
    if !lone_eagle_map_on_disk() {
        eprintln!("player_orders_gate: SKIP Lone Eagle missing");
        std::process::exit(0);
    }
    eprintln!("player_orders_gate: retail={} timeout={}s", retail.display(), timeout);
    let r = run_issue_player_orders(Duration::from_secs(timeout));
    println!("{}", format_executable_smoke_report(&r));
    if r.status == "binary_missing" {
        eprintln!("player_orders_gate: SKIP binary_missing — {}", r.detail);
        std::process::exit(0);
    }
    let pass = r.executable_host_ok && r.reached_ingame && r.gameplay_cmd_ok
        && (r.status == "success" || r.status == "success_forced_exit");
    if pass {
        println!("player_orders_gate: PASS map={} gameplay={} frames={} status={}",
            r.map_seen, r.gameplay_cmd_ok, r.frames_observed, r.status);
        std::process::exit(0);
    }
    eprintln!("player_orders_gate: FAIL status={} host_ok={} ingame={} gameplay={} detail={}",
        r.status, r.executable_host_ok, r.reached_ingame, r.gameplay_cmd_ok, r.detail);
    std::process::exit(1);
}
