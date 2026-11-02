//! P0 hq-x1j4u.9.5: AI activity on retail Lone Eagle while human issues move orders.
use generals_main::executable_smoke::{lone_eagle_map_on_disk, run_ai_opponent_skirmish};
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
    if c.join("Data/EnglishZH.big").is_file()
        || Path::new("windows_game/extracted_big_files/MapsZH/Maps/Lone Eagle/Lone Eagle.map")
            .is_file()
    {
        return Some(c.canonicalize().unwrap_or(c));
    }
    None
}

#[test]
fn production_host_ai_runs_with_human_orders_on_retail_map() {
    let Some(retail) = retail_root() else {
        eprintln!("skip: retail missing");
        return;
    };
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        unsafe {
            std::env::set_var("GENERALS_ASSETS_DIR", &retail);
        }
    }
    if !lone_eagle_map_on_disk() {
        eprintln!("skip: Lone Eagle missing");
        return;
    }
    let timeout = Duration::from_secs(
        std::env::var("AI_RETAIL_SKIRMISH_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(900),
    );
    let r = run_ai_opponent_skirmish(timeout);
    if r.status == "binary_missing" {
        eprintln!("skip: {}", r.detail);
        return;
    }
    assert!(
        r.executable_host_ok
            && r.reached_ingame
            && r.gameplay_cmd_ok
            && (r.status == "success" || r.status == "success_forced_exit"),
        "expected AI activity + human move; status={} host_ok={} ingame={} gameplay={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_ingame,
        r.gameplay_cmd_ok,
        r.detail
    );
}
