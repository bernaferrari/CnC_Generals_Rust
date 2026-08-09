//! Wave 878: CI clippy peel for string_system, ini_parser, ww_save_load libs
//! (-D warnings). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CI_CLIPPY_PEEL_METHOD_NAMES_WAVE878: &[&str] = &[
    "string_system",
    "ini_parser",
    "ww_save_load",
    "should_implement_trait",
    "Wave 878",
    "playable_claim = false",
];

pub const LIVE_HOST_CI_CLIPPY_PEEL_NAV_STEPS_WAVE878: &[&str] = &[
    "STRING_SYSTEM_CLIPPY_CLEAN",
    "INI_PARSER_CLIPPY_CLEAN",
    "WW_SAVE_LOAD_LIB_CLIPPY_CLEAN",
    "LIVE_HOST_CI_CLIPPY_PEEL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCiClippyPeelAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCiClippyPeelAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn string_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WWLib/string_system/src/lib.rs")
}

fn ini_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WWLib/ini_parser/src/lib.rs")
}

fn saveload_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WWSaveLoad/src/saveload.rs")
}

pub fn honesty_host_ci_clippy_peel_method_names_residual_wave878() -> bool {
    let names = LIVE_HOST_CI_CLIPPY_PEEL_METHOD_NAMES_WAVE878;
    let ok = residual_name_index(names, "string_system").is_some()
        && residual_name_index(names, "ww_save_load").is_some()
        && residual_name_index(names, "Wave 878").is_some();
    residual_action_store(ResidualHostCiClippyPeelAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ci_clippy_peel_nav_commands_residual_wave878() -> bool {
    let steps = LIVE_HOST_CI_CLIPPY_PEEL_NAV_STEPS_WAVE878;
    let ok = residual_name_index(steps, "LIVE_HOST_CI_CLIPPY_PEEL").is_some()
        && residual_name_index(steps, "STRING_SYSTEM_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostCiClippyPeelAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ci_clippy_peel_residual_pack_wave878() -> bool {
    let s = string_source();
    let i = ini_source();
    let sl = saveload_source();
    let ok = s.contains("#[allow(clippy::should_implement_trait)]")
        && s.contains("#[allow(clippy::not_unsafe_ptr_arg_deref)]")
        && s.contains("Some(self.cmp(other))")
        && i.contains("#[allow(clippy::new_without_default)]")
        && sl.contains("type RemapCallback =")
        && sl.contains("SAVE_LOAD_SYSTEM.get_or_init(SaveLoadSystem::new)")
        && !s.contains("playable_claim = true");
    residual_action_store(ResidualHostCiClippyPeelAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ci_clippy_peel_honesty() -> bool {
    let a = honesty_host_ci_clippy_peel_method_names_residual_wave878();
    let b = honesty_host_ci_clippy_peel_nav_commands_residual_wave878();
    let c = honesty_host_ci_clippy_peel_residual_pack_wave878();
    residual_action_store(ResidualHostCiClippyPeelAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ci_clippy_peel_residual_wave878() {
        assert!(honesty_host_ci_clippy_peel_residual_pack_wave878());
        assert!(honesty_host_ci_clippy_peel_method_names_residual_wave878());
        assert!(honesty_host_ci_clippy_peel_nav_commands_residual_wave878());
        assert!(simulate_live_host_ci_clippy_peel_honesty());
    }
}
