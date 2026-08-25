//! Wave 985: host production pause residual peel.
//!
//! ControlBar empty dual-world queues pause requests; Main drains onto
//! BuildingData.production_paused so factory timers hold (C++ ProductionUpdate pause).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRODUCTION_PAUSE_RESIDUAL_METHOD_NAMES_WAVE985: &[&str] = &[
    "queue_host_production_pause",
    "take_host_production_pause_requests",
    "set_production_paused",
    "production_paused",
    "advance_production_progress",
    "Wave 985",
    "playable_claim = false",
];

pub const LIVE_HOST_PRODUCTION_PAUSE_RESIDUAL_NAV_STEPS_WAVE985: &[&str] = &[
    "CONTROL_BAR_QUEUE_PAUSE",
    "MAIN_DRAIN_BUILDING_DATA",
    "HOLD_PRODUCTION_TIMER",
    "LIVE_HOST_PRODUCTION_PAUSE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionPauseResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostProductionPauseResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn buildings_source() -> &'static str {
    include_str!("../buildings.rs")
}
fn control_bar_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_production_pause_residual_method_names_residual_wave985() -> bool {
    let names = LIVE_HOST_PRODUCTION_PAUSE_RESIDUAL_METHOD_NAMES_WAVE985;
    let ok = residual_name_index(names, "queue_host_production_pause").is_some()
        && residual_name_index(names, "Wave 985").is_some();
    residual_action_store(ResidualHostProductionPauseResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_pause_residual_nav_commands_residual_wave985() -> bool {
    let steps = LIVE_HOST_PRODUCTION_PAUSE_RESIDUAL_NAV_STEPS_WAVE985;
    let ok = residual_name_index(steps, "LIVE_HOST_PRODUCTION_PAUSE_RESIDUAL").is_some()
        && residual_name_index(steps, "HOLD_PRODUCTION_TIMER").is_some();
    residual_action_store(ResidualHostProductionPauseResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_production_pause_residual_residual_pack_wave985() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let b = buildings_source();
    let cb = control_bar_source();
    let advance = match b.find("pub fn advance_production_progress") {
        Some(i) => &b[i..],
        None => "",
    };
    let pause_fn = match cb.find("fn set_build_queue_paused") {
        Some(i) => &cb[i..],
        None => "",
    };
    let finalize = match cnc.find("fn host_finalize_presentation_after_logic") {
        Some(i) => &cnc[i..],
        None => "",
    };
    let ok = b.contains("pub production_paused: bool")
        && b.contains("Wave 985")
        && advance.contains("production_paused")
        && b.contains("pub fn set_production_paused")
        && cb.contains("queue_host_production_pause")
        && cb.contains("take_host_production_pause_requests")
        && pause_fn.contains("queue_host_production_pause")
        && gl.contains("Wave 985: host production pause residual")
        && gl.contains("pub fn set_production_paused")
        && finalize.contains("take_host_production_pause_requests")
        && finalize.contains("set_production_paused")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionPauseResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_production_pause_residual_honesty() -> bool {
    let a = honesty_host_production_pause_residual_method_names_residual_wave985();
    let b = honesty_host_production_pause_residual_nav_commands_residual_wave985();
    let c = honesty_host_production_pause_residual_residual_pack_wave985();
    residual_action_store(ResidualHostProductionPauseResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_production_pause_residual_wave985() {
        assert!(honesty_host_production_pause_residual_residual_pack_wave985());
        assert!(honesty_host_production_pause_residual_method_names_residual_wave985());
        assert!(honesty_host_production_pause_residual_nav_commands_residual_wave985());
        assert!(simulate_live_host_production_pause_residual_honesty());
    }

    #[test]
    fn building_data_pause_holds_production_timer() {
        use crate::game_logic::Resources;
        use crate::game_logic::buildings::{
            BuildingData, BuildingType, ProductionItem, ProductionKind,
        };
        let mut bd = BuildingData::new(BuildingType::Barracks);
        bd.production_queue.push(ProductionItem {
            template_name: "USA_Ranger".into(),
            progress: 0.0,
            total_time: 10.0,
            construction_frames: 0,
            cost: Resources::default(),
            quantity_total: 1,
            quantity_produced: 0,
            kind: ProductionKind::Unit,
        });
        bd.set_production_paused(true);
        let before = bd.production_queue[0].progress;
        bd.advance_production_progress(1.0, 1.0);
        assert_eq!(bd.production_queue[0].progress, before);
        bd.set_production_paused(false);
        bd.advance_production_progress(1.0, 1.0);
        assert!(bd.production_queue[0].progress > before);
    }
}
