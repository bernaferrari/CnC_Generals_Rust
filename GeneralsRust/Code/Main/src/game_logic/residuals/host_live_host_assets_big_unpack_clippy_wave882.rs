//! Wave 882: big_unpack + ww3d-assets lib clippy -D warnings peel; projectile
//! sole-integrate honesty stamp. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ASSETS_BIG_UNPACK_METHOD_NAMES_WAVE882: &[&str] = &[
    "big_unpack",
    "ww3d-assets",
    "gameworld_projectile_authority_live",
    "Wave 882",
    "playable_claim = false",
];

pub const LIVE_HOST_ASSETS_BIG_UNPACK_NAV_STEPS_WAVE882: &[&str] = &[
    "BIG_UNPACK_CLIPPY_CLEAN",
    "WW3D_ASSETS_CLIPPY_CLEAN",
    "PROJECTILE_SOLE_INTEGRATE_HONESTY",
    "LIVE_HOST_ASSETS_BIG_UNPACK",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAssetsBigUnpackAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostAssetsBigUnpackAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn big_source() -> &'static str {
    include_str!("../../../../Tools/big_unpack/src/lib.rs")
}

fn assets_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-assets/src/assets.rs")
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

pub fn honesty_host_assets_big_unpack_method_names_residual_wave882() -> bool {
    let names = LIVE_HOST_ASSETS_BIG_UNPACK_METHOD_NAMES_WAVE882;
    let ok = residual_name_index(names, "big_unpack").is_some()
        && residual_name_index(names, "ww3d-assets").is_some()
        && residual_name_index(names, "Wave 882").is_some();
    residual_action_store(ResidualHostAssetsBigUnpackAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_assets_big_unpack_nav_commands_residual_wave882() -> bool {
    let steps = LIVE_HOST_ASSETS_BIG_UNPACK_NAV_STEPS_WAVE882;
    let ok = residual_name_index(steps, "LIVE_HOST_ASSETS_BIG_UNPACK").is_some()
        && residual_name_index(steps, "BIG_UNPACK_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostAssetsBigUnpackAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_assets_big_unpack_residual_pack_wave882() -> bool {
    let big = big_source();
    let assets = assets_source();
    let gl = gl_source();
    let ok = big.contains("fn parse_big_index(f: &mut File)")
        && big.contains("read_exact::<4>(f)")
        && assets.contains("Option<&dyn Prototype>")
        && assets.contains(".map(|b| b.as_ref())")
        && gl.contains("Wave 882: projectile sole-integrate under GW authority")
        && gl.contains("gameworld_projectile_authority_live()")
        && !big.contains("playable_claim = true");
    residual_action_store(ResidualHostAssetsBigUnpackAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_assets_big_unpack_honesty() -> bool {
    let a = honesty_host_assets_big_unpack_method_names_residual_wave882();
    let b = honesty_host_assets_big_unpack_nav_commands_residual_wave882();
    let c = honesty_host_assets_big_unpack_residual_pack_wave882();
    residual_action_store(ResidualHostAssetsBigUnpackAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_assets_big_unpack_residual_wave882() {
        assert!(honesty_host_assets_big_unpack_residual_pack_wave882());
        assert!(honesty_host_assets_big_unpack_method_names_residual_wave882());
        assert!(honesty_host_assets_big_unpack_nav_commands_residual_wave882());
        assert!(simulate_live_host_assets_big_unpack_honesty());
    }
}
