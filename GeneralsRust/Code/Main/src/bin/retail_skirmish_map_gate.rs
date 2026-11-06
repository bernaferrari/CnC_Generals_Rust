//! Gate: shell → skirmish start → retail non-shell map InGame (hq-x1j4u.9.2).
//!
//! Skips cleanly when retail assets or the `generals` binary are absent.

use generals_main::executable_smoke::{
    format_executable_smoke_report, lone_eagle_map_on_disk, run_load_retail_skirmish_map,
    ExecutableSmokeResult, SmokeOptions,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn retail_assets_present() -> Option<PathBuf> {
    for key in ["GENERALS_RETAIL_DIR", "GENERALS_ASSETS_DIR"] {
        if let Ok(dir) = std::env::var(key) {
            let p = PathBuf::from(dir);
            if retail_root_looks_valid(&p) {
                return Some(p);
            }
        }
    }
    for c in [
        PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("windows_game"),
        PathBuf::from("../windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("../windows_game"),
    ] {
        if retail_root_looks_valid(&c) {
            return Some(c.canonicalize().unwrap_or(c));
        }
    }
    None
}

fn retail_root_looks_valid(root: &Path) -> bool {
    let data = if root
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("Data"))
    {
        root.to_path_buf()
    } else {
        root.join("Data")
    };
    data.join("MapsZH.big").is_file()
        || data.join("EnglishZH.big").is_file()
        || root
            .parent()
            .map(|p| {
                p.join("extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map")
                    .is_file()
            })
            .unwrap_or(false)
        || Path::new("windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map")
            .is_file()
}

fn main() {
    let timeout_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            std::env::var("RETAIL_SKIRMISH_MAP_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(600);

    let Some(retail) = retail_assets_present() else {
        eprintln!(
            "retail_skirmish_map_gate: SKIP retail assets missing (set GENERALS_RETAIL_DIR)"
        );
        std::process::exit(0);
    };
    let options = SmokeOptions::for_retail(&retail);
    if !lone_eagle_map_on_disk(&options) {
        eprintln!(
            "retail_skirmish_map_gate: SKIP Lone Eagle.map not found on smoke search paths"
        );
        std::process::exit(0);
    }
    eprintln!(
        "retail_skirmish_map_gate: retail={} timeout={}s",
        retail.display(),
        timeout_secs
    );

    let r = run_load_retail_skirmish_map(Duration::from_secs(timeout_secs), &options);
    println!("{}", format_executable_smoke_report(&r));

    if r.status == "binary_missing" {
        eprintln!("retail_skirmish_map_gate: SKIP binary_missing — {}", r.detail);
        std::process::exit(0);
    }

    let retail_map = !ExecutableSmokeResult::map_is_shell_for_claim(&r.map_seen);
    let pass = r.executable_host_ok
        && r.reached_menu
        && r.shell_wnd_ok
        && r.reached_ingame
        && retail_map
        && (r.status == "success" || r.status == "success_forced_exit");
    if pass {
        println!(
            "retail_skirmish_map_gate: PASS map={} menu={} ingame={} frames={} status={}",
            r.map_seen, r.reached_menu, r.reached_ingame, r.frames_observed, r.status
        );
        std::process::exit(0);
    }
    eprintln!(
        "retail_skirmish_map_gate: FAIL status={} host_ok={} menu={} shell={} ingame={} map={} — {}",
        r.status, r.executable_host_ok, r.reached_menu, r.shell_wnd_ok, r.reached_ingame, r.map_seen, r.detail
    );
    std::process::exit(1);
}
