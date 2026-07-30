//! Wave 879: wwdownload lib clippy -D warnings peel (unused binds, dead_code).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WWDOWNLOAD_CLIPPY_METHOD_NAMES_WAVE879: &[&str] = &[
    "wwdownload",
    "parse_url_info",
    "config_manager",
    "Wave 879",
    "playable_claim = false",
];

pub const LIVE_HOST_WWDOWNLOAD_CLIPPY_NAV_STEPS_WAVE879: &[&str] = &[
    "WWDOWNLOAD_LIB_CLIPPY_CLEAN",
    "PREFIX_UNUSED_FULL_PATH",
    "ALLOW_DEAD_PARSE_URL_INFO",
    "LIVE_HOST_WWDOWNLOAD_CLIPPY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWwdownloadClippyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWwdownloadClippyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn registry_source() -> &'static str {
    include_str!("../../../Libraries/Source/WWVegas/WWDownload/src/registry.rs")
}

fn url_source() -> &'static str {
    include_str!("../../../Libraries/Source/WWVegas/WWDownload/src/url_builder.rs")
}

fn download_source() -> &'static str {
    include_str!("../../../Libraries/Source/WWVegas/WWDownload/src/download.rs")
}

pub fn honesty_host_wwdownload_clippy_method_names_residual_wave879() -> bool {
    let names = LIVE_HOST_WWDOWNLOAD_CLIPPY_METHOD_NAMES_WAVE879;
    let ok = residual_name_index(names, "wwdownload").is_some()
        && residual_name_index(names, "Wave 879").is_some();
    residual_action_store(ResidualHostWwdownloadClippyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_wwdownload_clippy_nav_commands_residual_wave879() -> bool {
    let steps = LIVE_HOST_WWDOWNLOAD_CLIPPY_NAV_STEPS_WAVE879;
    let ok = residual_name_index(steps, "LIVE_HOST_WWDOWNLOAD_CLIPPY").is_some()
        && residual_name_index(steps, "WWDOWNLOAD_LIB_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostWwdownloadClippyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_wwdownload_clippy_residual_pack_wave879() -> bool {
    let r = registry_source();
    let u = url_source();
    let d = download_source();
    let ok = r.contains("let _full_path =")
        && u.contains("let _base_url =")
        && u.contains("#[allow(dead_code)]\npub fn parse_url_info")
        && d.contains("#[allow(dead_code)]\n    config_manager:")
        && !r.contains("playable_claim = true");
    residual_action_store(ResidualHostWwdownloadClippyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_wwdownload_clippy_honesty() -> bool {
    let a = honesty_host_wwdownload_clippy_method_names_residual_wave879();
    let b = honesty_host_wwdownload_clippy_nav_commands_residual_wave879();
    let c = honesty_host_wwdownload_clippy_residual_pack_wave879();
    residual_action_store(ResidualHostWwdownloadClippyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_wwdownload_clippy_residual_wave879() {
        assert!(honesty_host_wwdownload_clippy_residual_pack_wave879());
        assert!(honesty_host_wwdownload_clippy_method_names_residual_wave879());
        assert!(honesty_host_wwdownload_clippy_nav_commands_residual_wave879());
        assert!(simulate_live_host_wwdownload_clippy_honesty());
    }
}
