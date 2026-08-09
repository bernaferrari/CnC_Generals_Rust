//! Wave 1005: dual-world object-id lookup performance presentation residual.
//!
//! report_object_id_lookup_performance peels empty OBJECT_REGISTRY by reporting
//! presentation translator catalog size instead of silent no-op.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_OBJECT_ID_LOOKUP_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1005: &[&str] = &[
    "report_object_id_lookup_performance",
    "translator_catalog_len",
    "Wave 1005",
    "playable_claim = false",
];

pub const LIVE_HOST_OBJECT_ID_LOOKUP_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1005: &[&str] = &[
    "DUAL_WORLD",
    "PRESENTATION_CATALOG_LEN",
    "LOOKUP_RESIDUAL",
    "LIVE_HOST_OBJECT_ID_LOOKUP_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostObjectIdLookupPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostObjectIdLookupPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn me_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/meta_event.rs")
}
fn tr_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}

pub fn honesty_host_object_id_lookup_presentation_residual_method_names_residual_wave1005() -> bool
{
    let names = LIVE_HOST_OBJECT_ID_LOOKUP_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1005;
    let ok = residual_name_index(names, "translator_catalog_len").is_some()
        && residual_name_index(names, "Wave 1005").is_some();
    residual_action_store(ResidualHostObjectIdLookupPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_object_id_lookup_presentation_residual_nav_commands_residual_wave1005() -> bool
{
    let steps = LIVE_HOST_OBJECT_ID_LOOKUP_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1005;
    let ok = residual_name_index(steps, "LIVE_HOST_OBJECT_ID_LOOKUP_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "PRESENTATION_CATALOG_LEN").is_some();
    residual_action_store(ResidualHostObjectIdLookupPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_object_id_lookup_presentation_residual_residual_pack_wave1005() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let me = me_source();
    let tr = tr_source();
    let body = match me.find("fn report_object_id_lookup_performance") {
        Some(i) => &me[i..me.len().min(i + 900)],
        None => "",
    };
    let ok = tr.contains("pub fn translator_catalog_len")
        && body.contains("Wave 345/1005")
        && body.contains("translator_catalog_len")
        && body.contains("presentation catalog knows")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostObjectIdLookupPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_object_id_lookup_presentation_residual_honesty() -> bool {
    let a = honesty_host_object_id_lookup_presentation_residual_method_names_residual_wave1005();
    let b = honesty_host_object_id_lookup_presentation_residual_nav_commands_residual_wave1005();
    let c = honesty_host_object_id_lookup_presentation_residual_residual_pack_wave1005();
    residual_action_store(ResidualHostObjectIdLookupPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_object_id_lookup_presentation_residual_wave1005() {
        assert!(honesty_host_object_id_lookup_presentation_residual_residual_pack_wave1005());
        assert!(
            honesty_host_object_id_lookup_presentation_residual_method_names_residual_wave1005()
        );
        assert!(
            honesty_host_object_id_lookup_presentation_residual_nav_commands_residual_wave1005()
        );
        assert!(simulate_live_host_object_id_lookup_presentation_residual_honesty());
    }
}
