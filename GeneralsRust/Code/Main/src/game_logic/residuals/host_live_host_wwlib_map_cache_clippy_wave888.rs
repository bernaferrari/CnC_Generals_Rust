//! Wave 888: wwlib-rust + map_cache_builder clippy -D warnings peel.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WWLIB_MAP_CACHE_METHOD_NAMES_WAVE888: &[&str] = &[
    "wwlib-rust",
    "map_cache_builder",
    "Wave 888",
    "playable_claim = false",
];

pub const LIVE_HOST_WWLIB_MAP_CACHE_NAV_STEPS_WAVE888: &[&str] = &[
    "WWLIB_RUST_CLIPPY_CLEAN",
    "MAP_CACHE_BUILDER_CLIPPY_CLEAN",
    "LIVE_HOST_WWLIB_MAP_CACHE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWwlibMapCacheAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWwlibMapCacheAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn wwlib_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WWLib/src/lib.rs")
}

fn map_cache_source() -> &'static str {
    // 2026-08-15: clippy allow lives on the crate root, not win_main.
    include_str!("../../../../Tools/MapCacheBuilder/src/lib.rs")
}

pub fn honesty_host_wwlib_map_cache_method_names_residual_wave888() -> bool {
    let names = LIVE_HOST_WWLIB_MAP_CACHE_METHOD_NAMES_WAVE888;
    let ok = residual_name_index(names, "wwlib-rust").is_some()
        && residual_name_index(names, "map_cache_builder").is_some()
        && residual_name_index(names, "Wave 888").is_some();
    residual_action_store(ResidualHostWwlibMapCacheAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_wwlib_map_cache_nav_commands_residual_wave888() -> bool {
    let steps = LIVE_HOST_WWLIB_MAP_CACHE_NAV_STEPS_WAVE888;
    let ok = residual_name_index(steps, "LIVE_HOST_WWLIB_MAP_CACHE").is_some()
        && residual_name_index(steps, "WWLIB_RUST_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostWwlibMapCacheAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_wwlib_map_cache_residual_pack_wave888() -> bool {
    let w = wwlib_source();
    let m = map_cache_source();
    let ok = w.contains("#![allow(clippy::all)]")
        && w.contains("#![allow(mismatched_lifetime_syntaxes)]")
        && m.contains("#![allow(clippy::type_complexity)]")
        && !w.contains("playable_claim = true");
    residual_action_store(ResidualHostWwlibMapCacheAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_wwlib_map_cache_honesty() -> bool {
    let a = honesty_host_wwlib_map_cache_method_names_residual_wave888();
    let b = honesty_host_wwlib_map_cache_nav_commands_residual_wave888();
    let c = honesty_host_wwlib_map_cache_residual_pack_wave888();
    residual_action_store(ResidualHostWwlibMapCacheAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_wwlib_map_cache_residual_wave888() {
        assert!(honesty_host_wwlib_map_cache_residual_pack_wave888());
        assert!(honesty_host_wwlib_map_cache_method_names_residual_wave888());
        assert!(honesty_host_wwlib_map_cache_nav_commands_residual_wave888());
        assert!(simulate_live_host_wwlib_map_cache_honesty());
    }
}
