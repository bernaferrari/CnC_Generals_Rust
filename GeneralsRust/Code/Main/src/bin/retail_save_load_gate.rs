//! Gate: retail Lone Eagle save/load round-trip + post-load orders (hq-x1j4u.9.7).
use generals_main::executable_smoke::{
    format_executable_smoke_report, lone_eagle_map_on_disk, run_retail_skirmish_save_load,
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
            std::env::var("RETAIL_SAVE_LOAD_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(900);
    let Some(retail) = retail_root() else {
        eprintln!("retail_save_load_gate: SKIP retail missing");
        std::process::exit(0);
    };
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        unsafe {
            std::env::set_var("GENERALS_ASSETS_DIR", &retail);
        }
    }
    if !lone_eagle_map_on_disk() {
        eprintln!("retail_save_load_gate: SKIP Lone Eagle missing");
        std::process::exit(0);
    }
    eprintln!(
        "retail_save_load_gate: retail={} timeout={}s",
        retail.display(),
        timeout
    );
    let r = run_retail_skirmish_save_load(Duration::from_secs(timeout));
    println!("{}", format_executable_smoke_report(&r));
    if r.status == "binary_missing" {
        eprintln!("retail_save_load_gate: SKIP binary_missing — {}", r.detail);
        std::process::exit(0);
    }
    let empty_units = r.detail.contains("units=0")
        || r.detail.contains("unit count is 0")
        || r.detail.contains("world_restore_fail:unit count");
    let pass = r.executable_host_ok
        && r.reached_ingame
        && r.gameplay_cmd_ok
        && r.save_cmd_ok
        && r.load_cmd_ok
        && !empty_units
        && (r.status == "success" || r.status == "success_forced_exit");
    if pass {
        println!(
            "retail_save_load_gate: PASS map={} save={} load={} gameplay={} detail={}",
            r.map_seen, r.save_cmd_ok, r.load_cmd_ok, r.gameplay_cmd_ok, r.detail
        );
        std::process::exit(0);
    }
    eprintln!(
        "retail_save_load_gate: FAIL status={} host_ok={} ingame={} save={} load={} gameplay={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_ingame,
        r.save_cmd_ok,
        r.load_cmd_ok,
        r.gameplay_cmd_ok,
        r.detail
    );
    std::process::exit(1);
}
