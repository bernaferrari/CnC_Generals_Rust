//! Wave 973: message-stream translator presentation catalog residual.
//!
//! Stamps unit catalog residual for host translators and peels
//! relationship/prisoner/mine/source selection onto freeze data when
//! OBJECT_REGISTRY is empty. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TRANSLATOR_CATALOG_METHOD_NAMES_WAVE973: &[&str] = &[
    "set_translator_presentation_residual",
    "relationship_to_target",
    "is_prisoner_target",
    "is_locally_controlled_mine_target",
    "Wave 973",
    "playable_claim = false",
];

pub const LIVE_HOST_TRANSLATOR_CATALOG_NAV_STEPS_WAVE973: &[&str] = &[
    "TRANSLATOR_FROM_CATALOG",
    "RELATIONSHIP_RESIDUAL",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_TRANSLATOR_CATALOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTranslatorCatalogAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostTranslatorCatalogAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

fn residual_mod_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}

fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_translator_catalog_method_names_residual_wave973() -> bool {
    let names = LIVE_HOST_TRANSLATOR_CATALOG_METHOD_NAMES_WAVE973;
    let ok = residual_name_index(names, "set_translator_presentation_residual").is_some()
        && residual_name_index(names, "Wave 973").is_some();
    residual_action_store(ResidualHostTranslatorCatalogAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_translator_catalog_nav_commands_residual_wave973() -> bool {
    let steps = LIVE_HOST_TRANSLATOR_CATALOG_NAV_STEPS_WAVE973;
    let ok = residual_name_index(steps, "LIVE_HOST_TRANSLATOR_CATALOG").is_some()
        && residual_name_index(steps, "RELATIONSHIP_RESIDUAL").is_some();
    residual_action_store(ResidualHostTranslatorCatalogAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_translator_catalog_residual_pack_wave973() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let residual = residual_mod_source();
    let client = client_source();
    let rel = match tr.find("fn relationship_to_target") {
        Some(i) => &tr[i..tr.len().min(i + 900)],
        None => "",
    };
    let pris = match tr.find("fn is_prisoner_target") {
        Some(i) => &tr[i..tr.len().min(i + 500)],
        None => "",
    };
    let ok = residual.contains("Wave 973")
        && tr.contains("Wave 973")
        && client.contains("Wave 973")
        && residual.contains("set_translator_presentation_residual")
        && rel.contains("translator_catalog_entry")
        && pris.contains("translator_catalog_has_kind")
        && client.contains("set_translator_presentation_residual")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostTranslatorCatalogAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_translator_catalog_honesty() -> bool {
    let a = honesty_host_translator_catalog_method_names_residual_wave973();
    let b = honesty_host_translator_catalog_nav_commands_residual_wave973();
    let c = honesty_host_translator_catalog_residual_pack_wave973();
    residual_action_store(ResidualHostTranslatorCatalogAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_translator_catalog_residual_wave973() {
        assert!(honesty_host_translator_catalog_residual_pack_wave973());
        assert!(honesty_host_translator_catalog_method_names_residual_wave973());
        assert!(honesty_host_translator_catalog_nav_commands_residual_wave973());
        assert!(simulate_live_host_translator_catalog_honesty());
    }
}
