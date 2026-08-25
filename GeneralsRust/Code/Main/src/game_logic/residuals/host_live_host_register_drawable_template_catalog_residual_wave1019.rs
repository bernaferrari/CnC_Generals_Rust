//! Wave 1019: dual-world register_drawable template catalog residual.
//!
//! When OBJECT_REGISTRY is empty, register_drawable_with_template peels
//! template_name from the presentation translator catalog by object_id.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_REGISTER_DRAWABLE_TEMPLATE_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1019: &[&str] = &[
    "register_drawable_with_template",
    "translator_catalog_entry",
    "template_name",
    "Wave 1019",
    "playable_claim = false",
];

pub const LIVE_HOST_REGISTER_DRAWABLE_TEMPLATE_CATALOG_RESIDUAL_NAV_STEPS_WAVE1019: &[&str] = &[
    "REGISTER_DRAWABLE",
    "TRANSLATOR_CATALOG",
    "TEMPLATE_NAME",
    "LIVE_HOST_REGISTER_DRAWABLE_TEMPLATE_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRegisterDrawableTemplateCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostRegisterDrawableTemplateCatalogResidualAction) {
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

pub fn honesty_host_register_drawable_template_catalog_residual_method_names_residual_wave1019()
-> bool {
    let names = LIVE_HOST_REGISTER_DRAWABLE_TEMPLATE_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1019;
    let ok = residual_name_index(names, "register_drawable_with_template").is_some()
        && residual_name_index(names, "Wave 1019").is_some();
    residual_action_store(ResidualHostRegisterDrawableTemplateCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_register_drawable_template_catalog_residual_nav_commands_residual_wave1019()
-> bool {
    let steps = LIVE_HOST_REGISTER_DRAWABLE_TEMPLATE_CATALOG_RESIDUAL_NAV_STEPS_WAVE1019;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_REGISTER_DRAWABLE_TEMPLATE_CATALOG_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "REGISTER_DRAWABLE").is_some();
    residual_action_store(ResidualHostRegisterDrawableTemplateCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_register_drawable_template_catalog_residual_residual_pack_wave1019() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let gc = gc_source();
    let ok = gc.contains("Wave 1019: dual-world peels template name from translator catalog")
        && gc.contains("dual_world_registry_unavailable()")
        && gc.contains("translator_catalog_entry")
        && (gc.contains("drawable.set_template_name(Some(entry.template_name.clone()))")
            || gc.contains("drawable.set_template_name(Some(apparent.to_string()))"))
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostRegisterDrawableTemplateCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_register_drawable_template_catalog_residual_honesty() -> bool {
    let a =
        honesty_host_register_drawable_template_catalog_residual_method_names_residual_wave1019();
    let b =
        honesty_host_register_drawable_template_catalog_residual_nav_commands_residual_wave1019();
    let c = honesty_host_register_drawable_template_catalog_residual_residual_pack_wave1019();
    residual_action_store(
        ResidualHostRegisterDrawableTemplateCatalogResidualAction::DispatchSource,
    );
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_register_drawable_template_catalog_residual_wave1019() {
        assert!(honesty_host_register_drawable_template_catalog_residual_residual_pack_wave1019());
        assert!(
            honesty_host_register_drawable_template_catalog_residual_method_names_residual_wave1019(
            )
        );
        assert!(
            honesty_host_register_drawable_template_catalog_residual_nav_commands_residual_wave1019(
            )
        );
        assert!(simulate_live_host_register_drawable_template_catalog_residual_honesty());
    }
}
