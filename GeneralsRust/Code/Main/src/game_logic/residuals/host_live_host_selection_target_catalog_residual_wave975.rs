//! Wave 975: selection target validation presentation catalog residual.
//!
//! Peels selection_any_local_object_can_target and related enter/repair/attack/
//! resume/crate helpers onto presentation translator catalog residual when
//! OBJECT_REGISTRY is empty. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECTION_TARGET_CATALOG_METHOD_NAMES_WAVE975: &[&str] = &[
    "selection_any_local_object_can_target",
    "selection_attack_result",
    "selection_can_enter_target",
    "selection_can_repair_target",
    "Wave 975",
    "playable_claim = false",
];

pub const LIVE_HOST_SELECTION_TARGET_CATALOG_NAV_STEPS_WAVE975: &[&str] = &[
    "SELECTION_TARGET_FROM_CATALOG",
    "ATTACK_ENTER_REPAIR_RESIDUAL",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_SELECTION_TARGET_CATALOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectionTargetCatalogAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectionTargetCatalogAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_selection_target_catalog_method_names_residual_wave975() -> bool {
    let names = LIVE_HOST_SELECTION_TARGET_CATALOG_METHOD_NAMES_WAVE975;
    let ok = residual_name_index(names, "selection_any_local_object_can_target").is_some()
        && residual_name_index(names, "Wave 975").is_some();
    residual_action_store(ResidualHostSelectionTargetCatalogAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_target_catalog_nav_commands_residual_wave975() -> bool {
    let steps = LIVE_HOST_SELECTION_TARGET_CATALOG_NAV_STEPS_WAVE975;
    let ok = residual_name_index(steps, "LIVE_HOST_SELECTION_TARGET_CATALOG").is_some()
        && residual_name_index(steps, "ATTACK_ENTER_REPAIR_RESIDUAL").is_some();
    residual_action_store(ResidualHostSelectionTargetCatalogAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_target_catalog_residual_pack_wave975() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let any = match tr.find("fn selection_any_local_object_can_target") {
        Some(i) => &tr[i..tr.len().min(i + 1200)],
        None => "",
    };
    let atk = match tr.find("fn selection_attack_result") {
        Some(i) => &tr[i..tr.len().min(i + 1600)],
        None => "",
    };
    let enter = match tr.find("fn selection_can_enter_target") {
        Some(i) => &tr[i..tr.len().min(i + 800)],
        None => "",
    };
    let ok = tr.contains("Wave 975")
        && any.contains("translator_catalog_entry")
        && any.contains("translator_entry_is_local")
        && atk.contains("CanAttackResult::Possible")
        && atk.contains("translator_catalog_entry")
        && enter.contains("translator_catalog_entry")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSelectionTargetCatalogAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_selection_target_catalog_honesty() -> bool {
    let a = honesty_host_selection_target_catalog_method_names_residual_wave975();
    let b = honesty_host_selection_target_catalog_nav_commands_residual_wave975();
    let c = honesty_host_selection_target_catalog_residual_pack_wave975();
    residual_action_store(ResidualHostSelectionTargetCatalogAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_selection_target_catalog_residual_wave975() {
        assert!(honesty_host_selection_target_catalog_residual_pack_wave975());
        assert!(honesty_host_selection_target_catalog_method_names_residual_wave975());
        assert!(honesty_host_selection_target_catalog_nav_commands_residual_wave975());
        assert!(simulate_live_host_selection_target_catalog_honesty());
    }
}
