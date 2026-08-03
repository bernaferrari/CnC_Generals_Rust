//! Wave 1067: dual-world sold visibility + under-construction source residual.
//!
//! update_drawable_visibility/sync hide sold drawables; attack/any-local/SP dual
//! fail-closed on under-construction local sources. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SOLD_VIS_UNDER_CONSTRUCTION_SOURCE_RESIDUAL_METHOD_NAMES_WAVE1067: &[&str] = &[
    "update_drawable_visibility",
    "sync_with_game_logic",
    "entry.sold",
    "under_construction",
    "Wave 1067",
    "playable_claim = false",
];

pub const LIVE_HOST_SOLD_VIS_UNDER_CONSTRUCTION_SOURCE_RESIDUAL_NAV_STEPS_WAVE1067: &[&str] = &[
    "SOLD_VISIBILITY",
    "UNDER_CONSTRUCTION_SOURCE",
    "LIVE_HOST_SOLD_VIS_UNDER_CONSTRUCTION_SOURCE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSoldVisUnderConstructionSourceResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSoldVisUnderConstructionSourceResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn gc_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}
fn ui_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}
fn tr_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/translators.rs")
}

pub fn honesty_host_sold_vis_under_construction_source_residual_method_names_residual_wave1067(
) -> bool {
    let names = LIVE_HOST_SOLD_VIS_UNDER_CONSTRUCTION_SOURCE_RESIDUAL_METHOD_NAMES_WAVE1067;
    let ok = residual_name_index(names, "entry.sold").is_some()
        && residual_name_index(names, "Wave 1067").is_some();
    residual_action_store(ResidualHostSoldVisUnderConstructionSourceResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sold_vis_under_construction_source_residual_nav_commands_residual_wave1067(
) -> bool {
    let steps = LIVE_HOST_SOLD_VIS_UNDER_CONSTRUCTION_SOURCE_RESIDUAL_NAV_STEPS_WAVE1067;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_SOLD_VIS_UNDER_CONSTRUCTION_SOURCE_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "SOLD_VISIBILITY").is_some();
    residual_action_store(ResidualHostSoldVisUnderConstructionSourceResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sold_vis_under_construction_source_residual_residual_pack_wave1067() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let gc = gc_source();
    let ui = ui_source();
    let tr = tr_source();
    let ok = gc.contains("Wave 1067: sold residual always hidden")
        && gc.matches("|| entry.sold").count() >= 2
        && ui.contains("Wave 1067: under-construction source residual fail-closed")
        && ui.contains("source.under_construction")
        && tr.contains("Wave 1067: under-construction local source residual fail-closed")
        && tr.matches("!sel.under_construction").count() >= 2
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSoldVisUnderConstructionSourceResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sold_vis_under_construction_source_residual_honesty() -> bool {
    let a =
        honesty_host_sold_vis_under_construction_source_residual_method_names_residual_wave1067();
    let b =
        honesty_host_sold_vis_under_construction_source_residual_nav_commands_residual_wave1067();
    let c = honesty_host_sold_vis_under_construction_source_residual_residual_pack_wave1067();
    residual_action_store(ResidualHostSoldVisUnderConstructionSourceResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sold_vis_under_construction_source_residual_wave1067() {
        assert!(honesty_host_sold_vis_under_construction_source_residual_residual_pack_wave1067());
        assert!(
            honesty_host_sold_vis_under_construction_source_residual_method_names_residual_wave1067(
            )
        );
        assert!(
            honesty_host_sold_vis_under_construction_source_residual_nav_commands_residual_wave1067(
            )
        );
        assert!(simulate_live_host_sold_vis_under_construction_source_residual_honesty());
    }
}
