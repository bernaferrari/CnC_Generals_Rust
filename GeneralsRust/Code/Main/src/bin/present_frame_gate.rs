//! Gate: windowed Lone Eagle present sit-through (hq-x1j4u.9.6).
use generals_main::executable_smoke::{
    format_executable_smoke_report, lone_eagle_map_on_disk, run_present_skirmish_frame,
    ExecutableSmokeResult, SmokeOptions,
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
// A crash or an observed window is not evidence that the display is unavailable.
fn no_window(r: &ExecutableSmokeResult) -> bool {
    !r.window_visible
        && r.exit_code.is_none()
        && (r.status == "assets_or_display_unavailable"
            || (r.windowed_launch && !r.reached_ingame && r.status == "timeout"))
}


fn main() {
    let timeout: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .or_else(|| {
            std::env::var("PRESENT_FRAME_TIMEOUT_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(900);
    let Some(retail) = retail_root() else {
        eprintln!("present_frame_gate: SKIP retail missing");
        std::process::exit(0);
    };
    let mut options = SmokeOptions::for_retail(&retail);
    if !lone_eagle_map_on_disk(&options) {
        eprintln!("present_frame_gate: SKIP Lone Eagle missing");
        std::process::exit(0);
    }
    let shot = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/present_frame_lone_eagle.png");
    if let Some(parent) = shot.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if options.screenshot_path.is_none() {
        options.screenshot_path = Some(shot);
    }
    eprintln!(
        "present_frame_gate: retail={} timeout={}s screenshot={}",
        retail.display(),
        timeout,
        options.screenshot_path.as_ref().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()
    );
    let r = run_present_skirmish_frame(Duration::from_secs(timeout), &options);
    println!("{}", format_executable_smoke_report(&r));
    if r.status == "binary_missing" {
        eprintln!("present_frame_gate: SKIP binary_missing — {}", r.detail);
        std::process::exit(0);
    }
    if no_window(&r) {
        eprintln!(
            "present_frame_gate: NO_WINDOW — no window can be opened from this non-interactive shell; status={} detail={}",
            r.status, r.detail
        );
        std::process::exit(4);
    }
    let detail = r.detail.clone();
    let categories_ok = detail.contains("terrain=")
        && !detail.contains("present_frame_fail")
        && detail.contains("present_frame_pass");
    let pass = r.executable_host_ok
        && r.reached_ingame
        && r.gameplay_cmd_ok
        && r.render_items_stable_ok
        && r.max_render_item_count > 0
        && r.live_frame_ok
        && categories_ok
        && (r.status == "success" || r.status == "success_forced_exit");
    if pass {
        println!(
            "present_frame_gate: PASS map={} window_visible={} render_items={} screenshot detail={}",
            r.map_seen, r.window_visible, r.max_render_item_count, r.detail
        );
        std::process::exit(0);
    }
    eprintln!(
        "present_frame_gate: FAIL status={} host_ok={} ingame={} gameplay={} render_stable={} items={} window_visible={} detail={}",
        r.status,
        r.executable_host_ok,
        r.reached_ingame,
        r.gameplay_cmd_ok,
        r.render_items_stable_ok,
        r.max_render_item_count,
        r.window_visible,
        r.detail
    );
    std::process::exit(1);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_panic_is_not_classified_as_unavailable_display() {
        let mut result = ExecutableSmokeResult::default();
        result.status = "process_exited".into();
        result.windowed_launch = true;
        result.window_visible = true;
        result.exit_code = Some(101);
        result.detail = "display initialized; child panicked".into();
        assert!(!no_window(&result));

        result.window_visible = false;
        result.status = "assets_or_display_unavailable".into();
        assert!(!no_window(&result), "a child failure is not proof of display failure");

        result.exit_code = None;
        result.status = "timeout".into();
        assert!(no_window(&result), "a windowed launch with no observed window timed out");
    }
}
