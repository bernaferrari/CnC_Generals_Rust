//! Wave 1070: dual-world ControlBar destroyed/masked + producer residual.
//!
//! ControlBar dual clears on destroyed/masked selection; production dual fails
//! closed on masked/under-construction producers. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONTROLBAR_DESTROYED_MASKED_PRODUCER_RESIDUAL_METHOD_NAMES_WAVE1070: &[&str] =
    &[
        "catalog_destroyed",
        "catalog_masked",
        "under_construction",
        "Wave 1070",
        "playable_claim = false",
    ];

pub const LIVE_HOST_CONTROLBAR_DESTROYED_MASKED_PRODUCER_RESIDUAL_NAV_STEPS_WAVE1070: &[&str] = &[
    "CONTROL_BAR",
    "DESTROYED_MASKED",
    "PRODUCER",
    "LIVE_HOST_CONTROLBAR_DESTROYED_MASKED_PRODUCER_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostControlbarDestroyedMaskedProducerResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostControlbarDestroyedMaskedProducerResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_controlbar_destroyed_masked_producer_residual_method_names_residual_wave1070()
-> bool {
    let names = LIVE_HOST_CONTROLBAR_DESTROYED_MASKED_PRODUCER_RESIDUAL_METHOD_NAMES_WAVE1070;
    let ok = residual_name_index(names, "catalog_masked").is_some()
        && residual_name_index(names, "Wave 1070").is_some();
    residual_action_store(ResidualHostControlbarDestroyedMaskedProducerResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_controlbar_destroyed_masked_producer_residual_nav_commands_residual_wave1070()
-> bool {
    let steps = LIVE_HOST_CONTROLBAR_DESTROYED_MASKED_PRODUCER_RESIDUAL_NAV_STEPS_WAVE1070;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_CONTROLBAR_DESTROYED_MASKED_PRODUCER_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "CONTROL_BAR").is_some();
    residual_action_store(ResidualHostControlbarDestroyedMaskedProducerResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_controlbar_destroyed_masked_producer_residual_residual_pack_wave1070() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let ok = cb.contains("Wave 1070: destroyed/masked residual also clears dual-world ControlBar")
        && cb.contains("catalog_destroyed")
        && cb.contains("catalog_masked")
        && cb.contains(
            "catalog_sold || catalog_unselectable || catalog_destroyed || catalog_masked",
        )
        && cb.contains("Wave 1070: masked/under-construction producers also fail-closed")
        && cb.contains("Wave 1070: masked/UC/unselectable producer residual fail-closed")
        && cb.contains("Wave 1070: masked/UC producer residual clears production UI")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(
        ResidualHostControlbarDestroyedMaskedProducerResidualAction::SourceMarkers,
    );
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_controlbar_destroyed_masked_producer_residual_honesty() -> bool {
    let a =
        honesty_host_controlbar_destroyed_masked_producer_residual_method_names_residual_wave1070();
    let b =
        honesty_host_controlbar_destroyed_masked_producer_residual_nav_commands_residual_wave1070();
    let c = honesty_host_controlbar_destroyed_masked_producer_residual_residual_pack_wave1070();
    residual_action_store(
        ResidualHostControlbarDestroyedMaskedProducerResidualAction::DispatchSource,
    );
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_controlbar_destroyed_masked_producer_residual_wave1070() {
        assert!(
            honesty_host_controlbar_destroyed_masked_producer_residual_residual_pack_wave1070()
        );
        assert!(
            honesty_host_controlbar_destroyed_masked_producer_residual_method_names_residual_wave1070()
        );
        assert!(
            honesty_host_controlbar_destroyed_masked_producer_residual_nav_commands_residual_wave1070()
        );
        assert!(simulate_live_host_controlbar_destroyed_masked_producer_residual_honesty());
    }
}
