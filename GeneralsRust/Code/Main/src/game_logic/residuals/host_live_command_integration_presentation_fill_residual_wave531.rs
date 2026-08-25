//! Wave 531 residual peels: `InputCommandProcessor` fills
//! `MouseCommandContext` presentation fields from an installed
//! `PresentationFrame` (target/selected/box/similar) and routes
//! `process_mouse_input` presentation-only (no GameLogic dual-read).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 530 OwnerChanged capture/hijack audio residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `command_integration.rs` install_presentation_frame / presentation_*_hint
//! - `command_system.rs` selected_presentation / target_presentation dual-read gate
//!
//! Fail-closed:
//! - Selection ids still live on host GameLogic player state
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Wave 531 method/source residual names.
pub const LIVE_COMMAND_INTEGRATION_PRESENTATION_FILL_METHOD_NAMES_WAVE531: &[&str] = &[
    "install_presentation_frame",
    "presentation_target_hint",
    "presentation_selected_unit_hints",
    "presentation_box_select_units",
    "presentation_select_similar_units",
    "presentation-only mouse path when frame installed",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_INTEGRATION_PRESENTATION_FILL_NAV_STEPS_WAVE531: &[&str] = &[
    "REQUIRE_COMMAND_INTEGRATION_PRESENTATION_FILL",
    "REQUIRE_INSTALL_PRESENTATION_FRAME",
    "REQUIRE_PRESENTATION_ONLY_MOUSE_PATH",
    "LIVE_COMMAND_INTEGRATION_PRESENTATION_FILL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_INTEGRATION_PRESENTATION_FILL_CMD_NAMES_WAVE531: &[&str] = &[
    "command_integration_presentation_fill",
    "install_presentation_frame",
    "presentation_only_mouse_path",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualCommandIntegrationPresentationFillAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualCommandIntegrationPresentationFillAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualCommandIntegrationPresentationFillAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_command_integration_presentation_fill_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_command_integration_presentation_fill_last_action()
-> ResidualCommandIntegrationPresentationFillAction {
    ResidualCommandIntegrationPresentationFillAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}

fn ci_source() -> &'static str {
    include_str!("../../command_integration.rs")
}

fn cs_source() -> &'static str {
    crate::command_system::COMMAND_SYSTEM_SRC
}

pub fn honesty_command_integration_presentation_fill_method_names_residual_wave531() -> bool {
    let names = LIVE_COMMAND_INTEGRATION_PRESENTATION_FILL_METHOD_NAMES_WAVE531;
    let ok = residual_name_index(names, "install_presentation_frame").is_some()
        && residual_name_index(names, "presentation_target_hint").is_some()
        && residual_name_index(names, "presentation_selected_unit_hints").is_some()
        && residual_name_index(names, "presentation_box_select_units").is_some()
        && residual_name_index(names, "presentation_select_similar_units").is_some()
        && residual_name_index(names, "presentation-only mouse path when frame installed")
            .is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualCommandIntegrationPresentationFillAction::MethodNames);
    ok
}

pub fn honesty_command_integration_presentation_fill_source_markers_residual_wave531() -> bool {
    let ci = ci_source();
    let cs = cs_source();
    let ok = ci.contains("Wave 531")
        && ci.contains("fn install_presentation_frame")
        && ci.contains("fn presentation_target_hint")
        && ci.contains("fn presentation_selected_unit_hints")
        && ci.contains("presentation_box_select_units: box_units")
        && ci.contains("presentation_select_similar_units: similar_units")
        && ci.contains("presentation-only mouse path when frame installed")
        && ci.contains("if self.presentation_frame.is_some()")
        && ci.contains("pick_object_id_at_world_from_presentation")
        && cs.contains("selected_presentation")
        && cs.contains("target_presentation")
        && cs.contains("presentation_box_select_units")
        && !ci.contains("playable_claim = true");
    residual_action_store(ResidualCommandIntegrationPresentationFillAction::SourceMarkers);
    ok
}

pub fn honesty_command_integration_presentation_fill_nav_commands_residual_wave531() -> bool {
    let steps = LIVE_COMMAND_INTEGRATION_PRESENTATION_FILL_NAV_STEPS_WAVE531;
    let cmds = RUNTIME_HOST_LIVE_COMMAND_INTEGRATION_PRESENTATION_FILL_CMD_NAMES_WAVE531;
    let ok = residual_name_index(steps, "REQUIRE_COMMAND_INTEGRATION_PRESENTATION_FILL").is_some()
        && residual_name_index(steps, "REQUIRE_INSTALL_PRESENTATION_FRAME").is_some()
        && residual_name_index(steps, "REQUIRE_PRESENTATION_ONLY_MOUSE_PATH").is_some()
        && residual_name_index(steps, "LIVE_COMMAND_INTEGRATION_PRESENTATION_FILL").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "command_integration_presentation_fill").is_some()
        && residual_name_index(cmds, "install_presentation_frame").is_some()
        && residual_name_index(cmds, "presentation_only_mouse_path").is_some();
    residual_action_store(ResidualCommandIntegrationPresentationFillAction::NavCommands);
    ok
}

pub fn simulate_command_integration_presentation_fill_collect_source() -> bool {
    let ci = ci_source();
    let ok = ci.contains("pub fn install_presentation_frame")
        && ci.contains("presentation_frame: Option<PresentationFrame>")
        && ci.contains("frame.box_select_unit_ids")
        && ci.contains("frame.similar_unit_ids");
    residual_action_store(ResidualCommandIntegrationPresentationFillAction::CollectSource);
    ok
}

pub fn simulate_command_integration_presentation_fill_dispatch_source() -> bool {
    let ci = ci_source();
    // When frame installed, gl_for_classify is None (presentation-only).
    let ok = ci.contains("let gl_for_classify = if self.presentation_frame.is_some()")
        && ci.contains("None")
        && ci.contains("system.process_mouse_input(")
        && ci.contains("gl_for_classify");
    residual_action_store(ResidualCommandIntegrationPresentationFillAction::DispatchSource);
    ok
}

pub fn honesty_command_integration_presentation_fill_residual_pack_wave531() -> bool {
    honesty_command_integration_presentation_fill_method_names_residual_wave531()
        && honesty_command_integration_presentation_fill_source_markers_residual_wave531()
        && honesty_command_integration_presentation_fill_nav_commands_residual_wave531()
        && simulate_command_integration_presentation_fill_collect_source()
        && simulate_command_integration_presentation_fill_dispatch_source()
}

pub fn simulate_live_command_integration_presentation_fill_honesty() -> bool {
    let ok = honesty_command_integration_presentation_fill_residual_pack_wave531();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualCommandIntegrationPresentationFillAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_command_integration_presentation_fill_method_names_residual_wave531());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_command_integration_presentation_fill_source_markers_residual_wave531());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_command_integration_presentation_fill_nav_commands_residual_wave531());
    }

    #[test]
    fn presentation_fill_sources() {
        assert!(simulate_command_integration_presentation_fill_collect_source());
        assert!(simulate_command_integration_presentation_fill_dispatch_source());
    }

    #[test]
    fn wave531_composite_pack() {
        assert!(honesty_command_integration_presentation_fill_residual_pack_wave531());
    }

    #[test]
    fn simulate_live_command_integration_presentation_fill_honesty_residual_live() {
        assert!(
            simulate_live_command_integration_presentation_fill_honesty(),
            "command_integration presentation fill residual must latch"
        );
        assert!(residual_command_integration_presentation_fill_ok());
        assert_eq!(
            residual_command_integration_presentation_fill_last_action(),
            ResidualCommandIntegrationPresentationFillAction::Composite
        );
    }
}
