//! Gate: production `generals` boots to live MainMenu.wnd (hq-x1j4u.9.1).
//!
//! Skips cleanly when the retail install or runtime binary is absent.
//! Point assets with GENERALS_ASSETS_DIR / GENERALS_RETAIL_DIR, or keep
//! `windows_game/` beside the repo root (symlink is fine).

use generals_main::executable_smoke::{format_executable_smoke_report, run_boot_to_shell_menu, SmokeOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

fn retail_assets_present() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("GENERALS_RETAIL_DIR").or_else(|_| std::env::var("GENERALS_ASSETS_DIR"))
    {
        let p = PathBuf::from(dir);
        if retail_root_looks_valid(&p) {
            return Some(p);
        }
    }
    let candidates = [
        PathBuf::from("windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("windows_game"),
        PathBuf::from("../windows_game/Command & Conquer Generals Zero Hour"),
        PathBuf::from("../windows_game"),
    ];
    for c in candidates {
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
    // Prefer a real BIG archive from the install; also accept extracted WindowZH.
    data.join("EnglishZH.big").is_file()
        || data.join("WindowZH.big").is_file()
        || root
            .join("extracted_big_files/WindowZH/Window/Menus/MainMenu.wnd")
            .is_file()
        || root
            .parent()
            .map(|p| {
                p.join("extracted_big_files/WindowZH/Window/Menus/MainMenu.wnd")
                    .is_file()
            })
            .unwrap_or(false)
}

fn main() {
    let timeout_secs: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            std::env::var("BOOT_SHELL_MENU_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(180);

    let Some(retail) = retail_assets_present() else {
        eprintln!(
            "boot_shell_menu_gate: SKIP retail assets missing (set GENERALS_RETAIL_DIR or GENERALS_ASSETS_DIR)"
        );
        std::process::exit(0);
    };
    let options = SmokeOptions::for_retail(&retail);
    eprintln!(
        "boot_shell_menu_gate: retail={} timeout={}s",
        retail.display(),
        timeout_secs
    );

    let r = run_boot_to_shell_menu(Duration::from_secs(timeout_secs), &options);
    println!("{}", format_executable_smoke_report(&r));

    if r.status == "binary_missing" {
        eprintln!("boot_shell_menu_gate: SKIP binary_missing — {}", r.detail);
        std::process::exit(0);
    }

    let pass = r.executable_host_ok
        && r.reached_menu
        && r.shell_wnd_ok
        && (r.status == "success" || r.status == "success_forced_exit");
    if pass {
        println!(
            "boot_shell_menu_gate: PASS menu={} shell_wnd={} frames={} status={}",
            r.reached_menu, r.shell_wnd_ok, r.frames_observed, r.status
        );
        std::process::exit(0);
    }
    eprintln!(
        "boot_shell_menu_gate: FAIL status={} host_ok={} menu={} shell_wnd={} — {}",
        r.status, r.executable_host_ok, r.reached_menu, r.shell_wnd_ok, r.detail
    );
    std::process::exit(1);
}
