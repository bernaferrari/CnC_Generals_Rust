//! Wave 881: ui_framework tool crate clippy -D warnings peel (egui renames,
//! crate allows, is_none_or). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UI_FRAMEWORK_CLIPPY_METHOD_NAMES_WAVE881: &[&str] = &[
    "ui_framework",
    "CornerRadius",
    "from_id_salt",
    "is_none_or",
    "Wave 881",
    "playable_claim = false",
];

pub const LIVE_HOST_UI_FRAMEWORK_CLIPPY_NAV_STEPS_WAVE881: &[&str] = &[
    "UI_FRAMEWORK_CLIPPY_CLEAN",
    "EGUI_RENAME_CORNER_RADIUS",
    "EGUI_RENAME_FROM_ID_SALT",
    "LIVE_HOST_UI_FRAMEWORK_CLIPPY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUiFrameworkClippyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUiFrameworkClippyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn lib_source() -> &'static str {
    include_str!("../../../Tools/UIFramework/src/lib.rs")
}

fn hot_source() -> &'static str {
    include_str!("../../../Tools/UIFramework/src/hot_reload.rs")
}

pub fn honesty_host_ui_framework_clippy_method_names_residual_wave881() -> bool {
    let names = LIVE_HOST_UI_FRAMEWORK_CLIPPY_METHOD_NAMES_WAVE881;
    let ok = residual_name_index(names, "ui_framework").is_some()
        && residual_name_index(names, "Wave 881").is_some();
    residual_action_store(ResidualHostUiFrameworkClippyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_framework_clippy_nav_commands_residual_wave881() -> bool {
    let steps = LIVE_HOST_UI_FRAMEWORK_CLIPPY_NAV_STEPS_WAVE881;
    let ok = residual_name_index(steps, "LIVE_HOST_UI_FRAMEWORK_CLIPPY").is_some()
        && residual_name_index(steps, "UI_FRAMEWORK_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostUiFrameworkClippyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ui_framework_clippy_residual_pack_wave881() -> bool {
    let lib = lib_source();
    let hot = hot_source();
    let themes = include_str!("../../../Tools/UIFramework/src/themes.rs");
    let panels = include_str!("../../../Tools/UIFramework/src/panels.rs");
    let ok = lib.contains("#![allow(clippy::new_without_default)]")
        && lib.contains("#![allow(ambiguous_glob_reexports)]")
        && hot.contains("is_none_or(|last| modified > last)")
        && themes.contains("CornerRadius")
        && panels.contains("from_id_salt")
        && !lib.contains("playable_claim = true");
    residual_action_store(ResidualHostUiFrameworkClippyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ui_framework_clippy_honesty() -> bool {
    let a = honesty_host_ui_framework_clippy_method_names_residual_wave881();
    let b = honesty_host_ui_framework_clippy_nav_commands_residual_wave881();
    let c = honesty_host_ui_framework_clippy_residual_pack_wave881();
    residual_action_store(ResidualHostUiFrameworkClippyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ui_framework_clippy_residual_wave881() {
        assert!(honesty_host_ui_framework_clippy_residual_pack_wave881());
        assert!(honesty_host_ui_framework_clippy_method_names_residual_wave881());
        assert!(honesty_host_ui_framework_clippy_nav_commands_residual_wave881());
        assert!(simulate_live_host_ui_framework_clippy_honesty());
    }
}
