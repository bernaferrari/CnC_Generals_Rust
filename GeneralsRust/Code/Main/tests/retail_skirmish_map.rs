//! P0 hq-x1j4u.9.2: shell → skirmish → retail non-shell map reaches InGame.
//!
//! Skips when retail data or the `generals` binary is unavailable.

use generals_main::executable_smoke::{
    lone_eagle_map_on_disk, run_load_retail_skirmish_map, ExecutableSmokeResult,
};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn retail_root() -> Option<PathBuf> {
    for key in ["GENERALS_RETAIL_DIR", "GENERALS_ASSETS_DIR"] {
        if let Ok(dir) = std::env::var(key) {
            let p = PathBuf::from(dir);
            if retail_looks_valid(&p) {
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
        if retail_looks_valid(&c) {
            return Some(c.canonicalize().unwrap_or(c));
        }
    }
    None
}

fn retail_looks_valid(root: &Path) -> bool {
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
        || Path::new("windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map")
            .is_file()
}

#[test]
fn production_host_loads_retail_map_into_skirmish() {
    let Some(retail) = retail_root() else {
        eprintln!("skip: retail assets missing");
        return;
    };
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        unsafe {
            std::env::set_var("GENERALS_ASSETS_DIR", &retail);
        }
    }
    if !lone_eagle_map_on_disk() {
        eprintln!("skip: Lone Eagle.map missing");
        return;
    }

    let timeout = Duration::from_secs(
        std::env::var("RETAIL_SKIRMISH_MAP_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(600),
    );
    let r = run_load_retail_skirmish_map(timeout);
    if r.status == "binary_missing" {
        eprintln!("skip: generals binary missing — {}", r.detail);
        return;
    }
    assert!(
        r.executable_host_ok
            && r.reached_menu
            && r.shell_wnd_ok
            && r.reached_ingame
            && !ExecutableSmokeResult::map_is_shell_for_claim(&r.map_seen),
        "expected retail skirmish map InGame; status={} host_ok={} menu={} shell={} ingame={} map={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_menu,
        r.shell_wnd_ok,
        r.reached_ingame,
        r.map_seen,
        r.detail
    );
    assert!(
        r.status == "success" || r.status == "success_forced_exit",
        "unexpected status {}",
        r.status
    );
}
