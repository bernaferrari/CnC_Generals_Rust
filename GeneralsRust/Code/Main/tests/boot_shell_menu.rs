//! P0 hq-x1j4u.9.1: production host boots to live MainMenu.wnd with retail assets.
//!
//! Skips when retail data or the `generals` binary is unavailable so CI without
//! the install stays green. Local Mac worktrees with `windows_game/` must pass.

use generals_main::executable_smoke::{lone_eagle_map_on_disk, run_boot_to_shell_menu};
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
    data.join("EnglishZH.big").is_file()
        || data.join("WindowZH.big").is_file()
        || root
            .parent()
            .map(|p| {
                p.join("extracted_big_files/WindowZH/Window/Menus/MainMenu.wnd")
                    .is_file()
            })
            .unwrap_or(false)
        || Path::new("windows_game/extracted_big_files/WindowZH/Window/Menus/MainMenu.wnd")
            .is_file()
}

#[test]
fn production_host_boots_to_main_menu_wnd() {
    let Some(retail) = retail_root() else {
        eprintln!("skip: retail assets missing");
        return;
    };
    if std::env::var_os("GENERALS_ASSETS_DIR").is_none() {
        // SAFETY: test process before spawning the runtime host child.
        unsafe {
            std::env::set_var("GENERALS_ASSETS_DIR", &retail);
        }
    }
    // Prefer a real map on disk as a secondary retail signal (not required for menu).
    let _ = lone_eagle_map_on_disk();

    let timeout = Duration::from_secs(
        std::env::var("BOOT_SHELL_MENU_TIMEOUT_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(180),
    );
    let r = run_boot_to_shell_menu(timeout);
    if r.status == "binary_missing" {
        eprintln!("skip: generals binary missing — {}", r.detail);
        return;
    }
    assert!(
        r.executable_host_ok && r.reached_menu && r.shell_wnd_ok,
        "expected boot to MainMenu.wnd; status={} host_ok={} menu={} shell={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_menu,
        r.shell_wnd_ok,
        r.detail
    );
    assert!(
        r.status == "success" || r.status == "success_forced_exit",
        "unexpected status {}",
        r.status
    );
}
