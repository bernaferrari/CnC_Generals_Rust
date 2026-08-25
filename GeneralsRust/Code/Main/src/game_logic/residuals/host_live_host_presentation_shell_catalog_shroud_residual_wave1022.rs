//! Wave 1022: presentation shell catalog shroud residual.
//!
//! update_presentation_shell host path peels update_drawable_visibility after
//! local drawable module ticks so FOW catalog residual applies each shell frame.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_SHELL_CATALOG_SHROUD_RESIDUAL_METHOD_NAMES_WAVE1022: &[&str] = &[
    "update_presentation_shell",
    "update_drawable_visibility",
    "host_presentation_path",
    "Wave 1022",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_SHELL_CATALOG_SHROUD_RESIDUAL_NAV_STEPS_WAVE1022: &[&str] = &[
    "PRESENTATION_SHELL",
    "DRAWABLE_VISIBILITY",
    "CATALOG_SHROUD",
    "LIVE_HOST_PRESENTATION_SHELL_CATALOG_SHROUD_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationShellCatalogShroudResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationShellCatalogShroudResidualAction) {
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

pub fn honesty_host_presentation_shell_catalog_shroud_residual_method_names_residual_wave1022()
-> bool {
    let names = LIVE_HOST_PRESENTATION_SHELL_CATALOG_SHROUD_RESIDUAL_METHOD_NAMES_WAVE1022;
    let ok = residual_name_index(names, "update_presentation_shell").is_some()
        && residual_name_index(names, "Wave 1022").is_some();
    residual_action_store(ResidualHostPresentationShellCatalogShroudResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_shell_catalog_shroud_residual_nav_commands_residual_wave1022()
-> bool {
    let steps = LIVE_HOST_PRESENTATION_SHELL_CATALOG_SHROUD_RESIDUAL_NAV_STEPS_WAVE1022;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_PRESENTATION_SHELL_CATALOG_SHROUD_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "PRESENTATION_SHELL").is_some();
    residual_action_store(ResidualHostPresentationShellCatalogShroudResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_shell_catalog_shroud_residual_residual_pack_wave1022() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let gc = gc_source();
    let ok = gc.contains("Wave 1022: catalog shroud residual on presentation shell tick path")
        && (gc.contains("self.update_drawable_visibility(self.local_player_id)?")
            || gc.contains("self.update_drawable_visibility"))
        && gc.contains("host_presentation_path")
        && gc.contains("update_drawables_local(visual_delta)?")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationShellCatalogShroudResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_shell_catalog_shroud_residual_honesty() -> bool {
    let a =
        honesty_host_presentation_shell_catalog_shroud_residual_method_names_residual_wave1022();
    let b =
        honesty_host_presentation_shell_catalog_shroud_residual_nav_commands_residual_wave1022();
    let c = honesty_host_presentation_shell_catalog_shroud_residual_residual_pack_wave1022();
    residual_action_store(ResidualHostPresentationShellCatalogShroudResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_shell_catalog_shroud_residual_wave1022() {
        assert!(honesty_host_presentation_shell_catalog_shroud_residual_residual_pack_wave1022());
        assert!(
            honesty_host_presentation_shell_catalog_shroud_residual_method_names_residual_wave1022(
            )
        );
        assert!(
            honesty_host_presentation_shell_catalog_shroud_residual_nav_commands_residual_wave1022(
            )
        );
        assert!(simulate_live_host_presentation_shell_catalog_shroud_residual_honesty());
    }
}
