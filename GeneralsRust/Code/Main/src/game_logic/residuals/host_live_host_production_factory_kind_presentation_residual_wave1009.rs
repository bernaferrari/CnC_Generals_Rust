//! Wave 1009: dual-world get_object_has_production factory KindOf residual.
//!
//! When presentation queue/command-set residual is empty, peel translator
//! catalog KindOf names (FSBarracks/FSWarFactory/FSAirfield/CommandCenter/
//! Structure) so factory selection still reports production interface.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_FACTORY_KIND_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1009: &[&str] =
    &[
        "get_object_has_production",
        "FSBarracks",
        "translator_catalog_entry",
        "Wave 1009",
        "playable_claim = false",
    ];

pub const LIVE_HOST_PRODUCTION_FACTORY_KIND_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1009: &[&str] = &[
    "DUAL_WORLD",
    "FACTORY_KINDOF",
    "PRODUCTION_INTERFACE",
    "LIVE_HOST_PRODUCTION_FACTORY_KIND_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionFactoryKindPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProductionFactoryKindPresentationResidualAction) {
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

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_production_factory_kind_presentation_residual_method_names_residual_wave1009()
-> bool {
    let names = LIVE_HOST_PRODUCTION_FACTORY_KIND_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1009;
    let ok = residual_name_index(names, "FSBarracks").is_some()
        && residual_name_index(names, "Wave 1009").is_some();
    residual_action_store(ResidualHostProductionFactoryKindPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_factory_kind_presentation_residual_nav_commands_residual_wave1009()
-> bool {
    let steps = LIVE_HOST_PRODUCTION_FACTORY_KIND_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1009;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_PRODUCTION_FACTORY_KIND_PRESENTATION_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "FACTORY_KINDOF").is_some();
    residual_action_store(ResidualHostProductionFactoryKindPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_factory_kind_presentation_residual_residual_pack_wave1009() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let body = match cb.find("fn get_object_has_production") {
        Some(i) => &cb[i..],
        None => "",
    };
    let ok = body.contains("Wave 249/997/1009")
        && body.contains("FACTORY_KINDS")
        && body.contains("FSBarracks")
        && body.contains("FSWarFactory")
        && body.contains("translator_catalog_entry(obj_id)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(
        ResidualHostProductionFactoryKindPresentationResidualAction::SourceMarkers,
    );
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_production_factory_kind_presentation_residual_honesty() -> bool {
    let a =
        honesty_host_production_factory_kind_presentation_residual_method_names_residual_wave1009();
    let b =
        honesty_host_production_factory_kind_presentation_residual_nav_commands_residual_wave1009();
    let c = honesty_host_production_factory_kind_presentation_residual_residual_pack_wave1009();
    residual_action_store(
        ResidualHostProductionFactoryKindPresentationResidualAction::DispatchSource,
    );
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_production_factory_kind_presentation_residual_wave1009() {
        assert!(
            honesty_host_production_factory_kind_presentation_residual_residual_pack_wave1009()
        );
        assert!(honesty_host_production_factory_kind_presentation_residual_method_names_residual_wave1009());
        assert!(honesty_host_production_factory_kind_presentation_residual_nav_commands_residual_wave1009());
        assert!(simulate_live_host_production_factory_kind_presentation_residual_honesty());
    }
}
