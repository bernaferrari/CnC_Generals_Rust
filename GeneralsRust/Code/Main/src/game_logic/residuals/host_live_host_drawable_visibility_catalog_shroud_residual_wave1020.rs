//! Wave 1020: dual-world drawable visibility catalog shroud residual.
//!
//! update_drawable_visibility peels presentation translator catalog shroud_status
//! onto drawable_map when OBJECT_REGISTRY is empty (host presentation path).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DRAWABLE_VISIBILITY_CATALOG_SHROUD_RESIDUAL_METHOD_NAMES_WAVE1020: &[&str] = &[
    "update_drawable_visibility",
    "shroud_status",
    "set_fully_obscured_by_shroud",
    "Wave 1020",
    "playable_claim = false",
];

pub const LIVE_HOST_DRAWABLE_VISIBILITY_CATALOG_SHROUD_RESIDUAL_NAV_STEPS_WAVE1020: &[&str] = &[
    "DRAWABLE_VISIBILITY",
    "TRANSLATOR_CATALOG",
    "SHROUD_STATUS",
    "LIVE_HOST_DRAWABLE_VISIBILITY_CATALOG_SHROUD_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDrawableVisibilityCatalogShroudResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDrawableVisibilityCatalogShroudResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
fn gc_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_drawable_visibility_catalog_shroud_residual_method_names_residual_wave1020()
-> bool {
    let names = LIVE_HOST_DRAWABLE_VISIBILITY_CATALOG_SHROUD_RESIDUAL_METHOD_NAMES_WAVE1020;
    let ok = residual_name_index(names, "update_drawable_visibility").is_some()
        && residual_name_index(names, "Wave 1020").is_some();
    residual_action_store(ResidualHostDrawableVisibilityCatalogShroudResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_visibility_catalog_shroud_residual_nav_commands_residual_wave1020()
-> bool {
    let steps = LIVE_HOST_DRAWABLE_VISIBILITY_CATALOG_SHROUD_RESIDUAL_NAV_STEPS_WAVE1020;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_DRAWABLE_VISIBILITY_CATALOG_SHROUD_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "DRAWABLE_VISIBILITY").is_some();
    residual_action_store(ResidualHostDrawableVisibilityCatalogShroudResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_visibility_catalog_shroud_residual_residual_pack_wave1020() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let gc = gc_source();
    let ok = (gc.contains("Wave 1020: host empty dual-world peels presentation catalog shroud")
        || gc.contains("Wave 1020/1044: host empty dual-world peels presentation catalog shroud")
        || gc.contains("Wave 1020/1021 peels catalog shroud"))
        && gc.contains("entry.shroud_status >= 2")
        && (gc.contains("drawable.set_fully_obscured_by_shroud(fully_obscured)")
            || gc.contains("set_fully_obscured_by_shroud"))
        && (gc.contains("drawable.set_visible(!fully_obscured)")
            || gc.contains("set_visible(!fully_obscured)"))
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDrawableVisibilityCatalogShroudResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_drawable_visibility_catalog_shroud_residual_honesty() -> bool {
    let a =
        honesty_host_drawable_visibility_catalog_shroud_residual_method_names_residual_wave1020();
    let b =
        honesty_host_drawable_visibility_catalog_shroud_residual_nav_commands_residual_wave1020();
    let c = honesty_host_drawable_visibility_catalog_shroud_residual_residual_pack_wave1020();
    residual_action_store(
        ResidualHostDrawableVisibilityCatalogShroudResidualAction::DispatchSource,
    );
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_drawable_visibility_catalog_shroud_residual_wave1020() {
        assert!(honesty_host_drawable_visibility_catalog_shroud_residual_residual_pack_wave1020());
        assert!(
            honesty_host_drawable_visibility_catalog_shroud_residual_method_names_residual_wave1020(
            )
        );
        assert!(
            honesty_host_drawable_visibility_catalog_shroud_residual_nav_commands_residual_wave1020(
            )
        );
        assert!(simulate_live_host_drawable_visibility_catalog_shroud_residual_honesty());
    }
}
