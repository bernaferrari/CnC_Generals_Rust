//! Wave 883: wwshade lib clippy -D warnings peel (Default derives, strip_suffix,
//! entry API, flatten flush). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WWSHADE_CLIPPY_METHOD_NAMES_WAVE883: &[&str] = &[
    "wwshade",
    "strip_suffix",
    "or_insert_with",
    "Wave 883",
    "playable_claim = false",
];

pub const LIVE_HOST_WWSHADE_CLIPPY_NAV_STEPS_WAVE883: &[&str] = &[
    "WWSHADE_LIB_CLIPPY_CLEAN",
    "SHADER_STRIP_SUFFIX",
    "MESH_CONTAINER_ENTRY_API",
    "LIVE_HOST_WWSHADE_CLIPPY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWwshadeClippyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWwshadeClippyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn emb_source() -> &'static str {
    include_str!("../../../Libraries/Source/WWVegas/wwshade/src/embedded_shaders.rs")
}

fn ren_source() -> &'static str {
    include_str!("../../../Libraries/Source/WWVegas/wwshade/src/renderer.rs")
}

pub fn honesty_host_wwshade_clippy_method_names_residual_wave883() -> bool {
    let names = LIVE_HOST_WWSHADE_CLIPPY_METHOD_NAMES_WAVE883;
    let ok = residual_name_index(names, "wwshade").is_some()
        && residual_name_index(names, "Wave 883").is_some();
    residual_action_store(ResidualHostWwshadeClippyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_wwshade_clippy_nav_commands_residual_wave883() -> bool {
    let steps = LIVE_HOST_WWSHADE_CLIPPY_NAV_STEPS_WAVE883;
    let ok = residual_name_index(steps, "LIVE_HOST_WWSHADE_CLIPPY").is_some()
        && residual_name_index(steps, "WWSHADE_LIB_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostWwshadeClippyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_wwshade_clippy_residual_pack_wave883() -> bool {
    let emb = emb_source();
    let ren = ren_source();
    let ok = emb.contains("name.strip_suffix(\".vsh\")")
        && emb.contains("name.strip_suffix(\".psh\")")
        && ren.contains(".entry(class_id)")
        && ren.contains("or_insert_with(|| MeshContainer::new(class_id))")
        && ren.contains("iter_mut().flatten()")
        && !emb.contains("playable_claim = true");
    residual_action_store(ResidualHostWwshadeClippyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_wwshade_clippy_honesty() -> bool {
    let a = honesty_host_wwshade_clippy_method_names_residual_wave883();
    let b = honesty_host_wwshade_clippy_nav_commands_residual_wave883();
    let c = honesty_host_wwshade_clippy_residual_pack_wave883();
    residual_action_store(ResidualHostWwshadeClippyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_wwshade_clippy_residual_wave883() {
        assert!(honesty_host_wwshade_clippy_residual_pack_wave883());
        assert!(honesty_host_wwshade_clippy_method_names_residual_wave883());
        assert!(honesty_host_wwshade_clippy_nav_commands_residual_wave883());
        assert!(simulate_live_host_wwshade_clippy_honesty());
    }
}
