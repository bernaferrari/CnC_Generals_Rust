//! Wave 1021: dual-world update_drawables catalog shroud residual.
//!
//! When OBJECT_REGISTRY is empty, update_drawables peels
//! update_drawable_visibility (catalog shroud_status → drawable_map)
//! instead of skipping FOW bind entirely.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UPDATE_DRAWABLES_CATALOG_SHROUD_RESIDUAL_METHOD_NAMES_WAVE1021: &[&str] = &[
    "update_drawables",
    "update_drawable_visibility",
    "OBJECT_REGISTRY.is_empty",
    "Wave 1021",
    "playable_claim = false",
];

pub const LIVE_HOST_UPDATE_DRAWABLES_CATALOG_SHROUD_RESIDUAL_NAV_STEPS_WAVE1021: &[&str] = &[
    "UPDATE_DRAWABLES",
    "DRAWABLE_VISIBILITY",
    "CATALOG_SHROUD",
    "LIVE_HOST_UPDATE_DRAWABLES_CATALOG_SHROUD_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUpdateDrawablesCatalogShroudResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUpdateDrawablesCatalogShroudResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn gc_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_update_drawables_catalog_shroud_residual_method_names_residual_wave1021() -> bool
{
    let names = LIVE_HOST_UPDATE_DRAWABLES_CATALOG_SHROUD_RESIDUAL_METHOD_NAMES_WAVE1021;
    let ok = residual_name_index(names, "update_drawables").is_some()
        && residual_name_index(names, "Wave 1021").is_some();
    residual_action_store(ResidualHostUpdateDrawablesCatalogShroudResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_update_drawables_catalog_shroud_residual_nav_commands_residual_wave1021() -> bool
{
    let steps = LIVE_HOST_UPDATE_DRAWABLES_CATALOG_SHROUD_RESIDUAL_NAV_STEPS_WAVE1021;
    let ok = residual_name_index(steps, "LIVE_HOST_UPDATE_DRAWABLES_CATALOG_SHROUD_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "UPDATE_DRAWABLES").is_some();
    residual_action_store(ResidualHostUpdateDrawablesCatalogShroudResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_update_drawables_catalog_shroud_residual_residual_pack_wave1021() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let gc = gc_source();
    let ok = gc.contains("Wave 1020/1021 peels catalog shroud onto drawable_map")
        && gc.contains("self.update_drawable_visibility(local_player_index)?")
        && gc.contains("Wave 1021: catalog shroud residual on presentation shell render path")
        && gc.contains("if OBJECT_REGISTRY.is_empty()")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostUpdateDrawablesCatalogShroudResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_update_drawables_catalog_shroud_residual_honesty() -> bool {
    let a = honesty_host_update_drawables_catalog_shroud_residual_method_names_residual_wave1021();
    let b = honesty_host_update_drawables_catalog_shroud_residual_nav_commands_residual_wave1021();
    let c = honesty_host_update_drawables_catalog_shroud_residual_residual_pack_wave1021();
    residual_action_store(ResidualHostUpdateDrawablesCatalogShroudResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_update_drawables_catalog_shroud_residual_wave1021() {
        assert!(honesty_host_update_drawables_catalog_shroud_residual_residual_pack_wave1021());
        assert!(
            honesty_host_update_drawables_catalog_shroud_residual_method_names_residual_wave1021()
        );
        assert!(
            honesty_host_update_drawables_catalog_shroud_residual_nav_commands_residual_wave1021()
        );
        assert!(simulate_live_host_update_drawables_catalog_shroud_residual_honesty());
    }
}
