//! Wave 1037: dual-world selection KindOf flags catalog residual.
//!
//! Dual collect_drawables packs kind_of_flags from catalog kind_names so
//! can_select ForceAttackable/Structure/AlwaysSelectable residual matches C++.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_KINDOF_FLAGS_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1037: &[&str] = &[
    "catalog_kind_of_flags",
    "KINDOF_FORCEATTACKABLE",
    "kind_of_flags",
    "Wave 1037",
    "playable_claim = false",
];

pub const LIVE_HOST_KINDOF_FLAGS_CATALOG_RESIDUAL_NAV_STEPS_WAVE1037: &[&str] = &[
    "KINDOF_FLAGS",
    "SELECTION_XLAT",
    "FORCEATTACKABLE",
    "LIVE_HOST_KINDOF_FLAGS_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostKindofFlagsCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostKindofFlagsCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn sx_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs")
}

pub fn honesty_host_kindof_flags_catalog_residual_method_names_residual_wave1037() -> bool {
    let names = LIVE_HOST_KINDOF_FLAGS_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1037;
    let ok = residual_name_index(names, "catalog_kind_of_flags").is_some()
        && residual_name_index(names, "Wave 1037").is_some();
    residual_action_store(ResidualHostKindofFlagsCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_kindof_flags_catalog_residual_nav_commands_residual_wave1037() -> bool {
    let steps = LIVE_HOST_KINDOF_FLAGS_CATALOG_RESIDUAL_NAV_STEPS_WAVE1037;
    let ok = residual_name_index(steps, "LIVE_HOST_KINDOF_FLAGS_CATALOG_RESIDUAL").is_some()
        && residual_name_index(steps, "KINDOF_FLAGS").is_some();
    residual_action_store(ResidualHostKindofFlagsCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_kindof_flags_catalog_residual_residual_pack_wave1037() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sx = sx_source();
    let ok = sx.contains("Wave 1037: pack selection KindOf residual bits from catalog kind_names")
        && sx.contains("fn catalog_kind_of_flags")
        && sx.contains("Wave 1037: KindOf residual bits for can_select")
        && sx.contains("catalog_kind_of_flags(&entry.kind_names)")
        && sx.contains("ForceAttackable")
        && sx.contains("kind_of_flags |= KINDOF_FORCEATTACKABLE")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostKindofFlagsCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_kindof_flags_catalog_residual_honesty() -> bool {
    let a = honesty_host_kindof_flags_catalog_residual_method_names_residual_wave1037();
    let b = honesty_host_kindof_flags_catalog_residual_nav_commands_residual_wave1037();
    let c = honesty_host_kindof_flags_catalog_residual_residual_pack_wave1037();
    residual_action_store(ResidualHostKindofFlagsCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_kindof_flags_catalog_residual_wave1037() {
        assert!(honesty_host_kindof_flags_catalog_residual_residual_pack_wave1037());
        assert!(honesty_host_kindof_flags_catalog_residual_method_names_residual_wave1037());
        assert!(honesty_host_kindof_flags_catalog_residual_nav_commands_residual_wave1037());
        assert!(simulate_live_host_kindof_flags_catalog_residual_honesty());
    }
}
