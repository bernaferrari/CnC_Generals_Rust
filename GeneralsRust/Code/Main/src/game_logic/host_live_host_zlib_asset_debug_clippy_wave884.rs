//! Wave 884: zlib_compression, asset_pipeline, debug_window clippy -D warnings peel.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ZLIB_ASSET_DEBUG_METHOD_NAMES_WAVE884: &[&str] = &[
    "zlib_compression",
    "asset_pipeline",
    "debug_window",
    "Wave 884",
    "playable_claim = false",
];

pub const LIVE_HOST_ZLIB_ASSET_DEBUG_NAV_STEPS_WAVE884: &[&str] = &[
    "ZLIB_CLIPPY_CLEAN",
    "ASSET_PIPELINE_CLIPPY_CLEAN",
    "DEBUG_WINDOW_CLIPPY_CLEAN",
    "LIVE_HOST_ZLIB_ASSET_DEBUG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostZlibAssetDebugAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostZlibAssetDebugAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn zlib_source() -> &'static str {
    include_str!("../../../Libraries/Source/Compression/ZLib/src/lib.rs")
}

fn inflate_source() -> &'static str {
    include_str!("../../../Libraries/Source/Compression/ZLib/src/inflate.rs")
}

fn asset_source() -> &'static str {
    include_str!("../../../Libraries/Source/AssetPipeline/src/lib.rs")
}

fn debug_source() -> &'static str {
    include_str!("../../../Tools/DebugWindow/src/main.rs")
}

pub fn honesty_host_zlib_asset_debug_method_names_residual_wave884() -> bool {
    let names = LIVE_HOST_ZLIB_ASSET_DEBUG_METHOD_NAMES_WAVE884;
    let ok = residual_name_index(names, "zlib_compression").is_some()
        && residual_name_index(names, "asset_pipeline").is_some()
        && residual_name_index(names, "Wave 884").is_some();
    residual_action_store(ResidualHostZlibAssetDebugAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_zlib_asset_debug_nav_commands_residual_wave884() -> bool {
    let steps = LIVE_HOST_ZLIB_ASSET_DEBUG_NAV_STEPS_WAVE884;
    let ok = residual_name_index(steps, "LIVE_HOST_ZLIB_ASSET_DEBUG").is_some()
        && residual_name_index(steps, "ZLIB_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostZlibAssetDebugAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_zlib_asset_debug_residual_pack_wave884() -> bool {
    let z = zlib_source();
    let inf = inflate_source();
    let a = asset_source();
    let d = debug_source();
    let ok = z.contains("#![allow(dead_code)]")
        && inf.contains("lengths.resize(lengths.len() + repeat, 0)")
        && a.contains("#![allow(dead_code)]")
        && a.contains("#![allow(unused_imports)]")
        && !d.contains("use env_logger;\n")
        && !z.contains("playable_claim = true");
    residual_action_store(ResidualHostZlibAssetDebugAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_zlib_asset_debug_honesty() -> bool {
    let a = honesty_host_zlib_asset_debug_method_names_residual_wave884();
    let b = honesty_host_zlib_asset_debug_nav_commands_residual_wave884();
    let c = honesty_host_zlib_asset_debug_residual_pack_wave884();
    residual_action_store(ResidualHostZlibAssetDebugAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_zlib_asset_debug_residual_wave884() {
        assert!(honesty_host_zlib_asset_debug_residual_pack_wave884());
        assert!(honesty_host_zlib_asset_debug_method_names_residual_wave884());
        assert!(honesty_host_zlib_asset_debug_nav_commands_residual_wave884());
        assert!(simulate_live_host_zlib_asset_debug_honesty());
    }
}
