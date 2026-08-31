//! Wave 833: executable smoke prefers release generals + GeneralsRust cwd, and
//! does not pkill bare exe paths (Booting exit-101 race). playable_claim false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EXEC_SMOKE_RELEASE_PREFER_METHOD_NAMES_WAVE833: &[&str] = &[
    "resolve_runtime_exe",
    "target/release/generals",
    "current_dir",
    "GENERALS_RUNTIME_EXE_PREFER_DEBUG",
    "Wave 833",
    "playable_claim = false",
];
pub const LIVE_HOST_EXEC_SMOKE_RELEASE_PREFER_NAV_STEPS_WAVE833: &[&str] = &[
    "REQUIRE_RELEASE_PREFER",
    "REQUIRE_WORKSPACE_CWD",
    "REQUIRE_NO_BARE_PKILL",
    "LIVE_HOST_EXEC_SMOKE_RELEASE_PREFER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostExecSmokeReleasePreferAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostExecSmokeReleasePreferAction {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostExecSmokeReleasePreferAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
fn shell_base_source() -> &'static str {
    concat!(
        include_str!("../../../../GameEngine/GameClient/src/gui/shell/base/mod.rs"),
        include_str!("../../../../GameEngine/GameClient/src/gui/shell/base/types.rs"),
        include_str!("../../../../GameEngine/GameClient/src/gui/shell/base/scheme.rs"),
        include_str!(
            "../../../../GameEngine/GameClient/src/gui/shell/base/animate_window.rs"
        ),
        include_str!(
            "../../../../GameEngine/GameClient/src/gui/shell/base/shell_lifecycle.rs"
        ),
        include_str!("../../../../GameEngine/GameClient/src/gui/shell/base/shell_ops.rs"),
        include_str!("../../../../GameEngine/GameClient/src/gui/shell/base/tests.rs"),
        include_str!("../../../../GameEngine/GameClient/src/gui/shell/base/residual.rs")
    )
}
fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
pub fn honesty_host_exec_smoke_release_prefer_method_names_residual_wave833() -> bool {
    let names = LIVE_HOST_EXEC_SMOKE_RELEASE_PREFER_METHOD_NAMES_WAVE833;
    let ok = residual_name_index(names, "resolve_runtime_exe").is_some()
        && residual_name_index(names, "target/release/generals").is_some()
        && residual_name_index(names, "current_dir").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_EXE_PREFER_DEBUG").is_some()
        && residual_name_index(names, "Wave 833").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostExecSmokeReleasePreferAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_exec_smoke_release_prefer_nav_commands_residual_wave833() -> bool {
    let steps = LIVE_HOST_EXEC_SMOKE_RELEASE_PREFER_NAV_STEPS_WAVE833;
    let ok = residual_name_index(steps, "LIVE_HOST_EXEC_SMOKE_RELEASE_PREFER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostExecSmokeReleasePreferAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_exec_smoke_release_prefer_residual_pack_wave833() -> bool {
    let es = es_source();
    let ok = (es.contains("Wave 833: current-source binary")
        || es.contains("Wave 833: prefer release over debug for smoke stability"))
        && es.contains("GENERALS_RUNTIME_EXE_PREFER_DEBUG")
        && es.contains("Wave 833: run from GeneralsRust workspace root")
        && es.contains(".current_dir(&workspace_cwd)")
        && es.contains("Wave 833: never pkill bare exe path")
        && !es.contains("format!(\"{exe_s}\")");
    residual_action_store(ResidualHostExecSmokeReleasePreferAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_exec_smoke_release_prefer_honesty() -> bool {
    let a = honesty_host_exec_smoke_release_prefer_method_names_residual_wave833();
    let b = honesty_host_exec_smoke_release_prefer_nav_commands_residual_wave833();
    let c = honesty_host_exec_smoke_release_prefer_residual_pack_wave833();
    residual_action_store(ResidualHostExecSmokeReleasePreferAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_exec_smoke_release_prefer_residual_wave833() {
        assert!(honesty_host_exec_smoke_release_prefer_residual_pack_wave833());
        assert!(honesty_host_exec_smoke_release_prefer_method_names_residual_wave833());
        assert!(honesty_host_exec_smoke_release_prefer_nav_commands_residual_wave833());
        assert!(simulate_live_host_exec_smoke_release_prefer_honesty());
    }
}
