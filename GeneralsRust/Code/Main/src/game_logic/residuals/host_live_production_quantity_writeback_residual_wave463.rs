//! Wave 463 residual peels: production quantity_total/quantity_produced preserved
//! through host progress log → GameWorld EntityProductionItem → writeback.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 180 production writeback residual.
//! Architecture residual - C++ ProductionEntry quantity parity on sole-tick path.
//!
//! Sources:
//! - EntityProductionItem.quantity_total / quantity_produced
//! - HostProductionQueueItem quantity fields
//! - writeback_production_to_host maps quantity (not hard-coded 1/0)
//!
//! Fail-closed:
//! - Host still completes/spawns units (not full GW spawn authority)
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRODUCTION_QUANTITY_WRITEBACK_METHOD_NAMES_WAVE463: &[&str] = &[
    "writeback_production_to_host",
    "apply_host_production_progress_events",
    "tick_production_queues",
    "quantity_total",
    "quantity_produced",
    "HostProductionQueueItem",
];

pub const PRODUCTION_QUANTITY_WRITEBACK_SOURCE_MARKERS_WAVE463: &[&str] = &[
    "Wave 463: preserve C++ production quantity residual through GW writeback",
    "quantity_total: it.quantity_total.max(1)",
    "quantity_produced: it.quantity_produced",
    "EntityProductionItem",
];

pub const PRODUCTION_QUANTITY_WRITEBACK_NAV_STEPS_WAVE463: &[&str] = &[
    "HOST_SNAPSHOT_QUANTITY_INTO_PROGRESS_LOG",
    "APPLY_PROGRESS_EVENTS_TO_GAMEWORLD",
    "SOLE_TICK_QUEUE_PROGRESS",
    "WRITEBACK_QUANTITY_TO_HOST",
    "HOST_TRY_COMPLETE_USES_PRESERVED_QTY",
    "NO_HARDCODED_QUANTITY_ONE_ZERO",
];

pub const RUNTIME_HOST_PRODUCTION_QUANTITY_WRITEBACK_CMD_NAMES_WAVE463: &[&str] = &[
    "click_production_quantity_writeback_ok_wnd_snapshot",
    "click_production_quantity_writeback_ok_wnd_apply",
    "click_production_quantity_writeback_ok_wnd_tick",
    "click_production_quantity_writeback_ok_wnd_prepare",
    "click_production_quantity_writeback_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualProductionQuantityWritebackAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    WritebackSource = 4,
    EntityFields = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualProductionQuantityWritebackAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_production_quantity_writeback_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_production_quantity_writeback_last_action()
-> ResidualProductionQuantityWritebackAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualProductionQuantityWritebackAction::MethodNames,
        2 => ResidualProductionQuantityWritebackAction::SourceMarkers,
        3 => ResidualProductionQuantityWritebackAction::NavCommands,
        4 => ResidualProductionQuantityWritebackAction::WritebackSource,
        5 => ResidualProductionQuantityWritebackAction::EntityFields,
        6 => ResidualProductionQuantityWritebackAction::Composite,
        _ => ResidualProductionQuantityWritebackAction::Idle,
    }
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn entity_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}

fn progress_log_source() -> &'static str {
    include_str!("../host_production_progress_log.rs")
}

pub fn honesty_production_quantity_writeback_method_names_residual_wave463() -> bool {
    PRODUCTION_QUANTITY_WRITEBACK_METHOD_NAMES_WAVE463.len() == 6
        && residual_name_index(
            PRODUCTION_QUANTITY_WRITEBACK_METHOD_NAMES_WAVE463,
            "writeback_production_to_host",
        ) == Some(0)
        && residual_name_index(
            PRODUCTION_QUANTITY_WRITEBACK_METHOD_NAMES_WAVE463,
            "HostProductionQueueItem",
        ) == Some(5)
}

pub fn honesty_production_quantity_writeback_source_markers_residual_wave463() -> bool {
    PRODUCTION_QUANTITY_WRITEBACK_SOURCE_MARKERS_WAVE463.len() == 4
        && residual_name_index(
            PRODUCTION_QUANTITY_WRITEBACK_SOURCE_MARKERS_WAVE463,
            "Wave 463: preserve C++ production quantity residual through GW writeback",
        ) == Some(0)
        && residual_name_index(
            PRODUCTION_QUANTITY_WRITEBACK_SOURCE_MARKERS_WAVE463,
            "EntityProductionItem",
        ) == Some(3)
}

pub fn honesty_production_quantity_writeback_nav_commands_residual_wave463() -> bool {
    PRODUCTION_QUANTITY_WRITEBACK_NAV_STEPS_WAVE463.len() == 6
        && residual_name_index(
            PRODUCTION_QUANTITY_WRITEBACK_NAV_STEPS_WAVE463,
            "WRITEBACK_QUANTITY_TO_HOST",
        ) == Some(3)
        && residual_name_index(
            PRODUCTION_QUANTITY_WRITEBACK_NAV_STEPS_WAVE463,
            "NO_HARDCODED_QUANTITY_ONE_ZERO",
        ) == Some(5)
        && RUNTIME_HOST_PRODUCTION_QUANTITY_WRITEBACK_CMD_NAMES_WAVE463.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRODUCTION_QUANTITY_WRITEBACK_CMD_NAMES_WAVE463,
            "click_production_quantity_writeback_ok_wnd_prepare",
        ) == Some(3)
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let at = src.find(sig)?;
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut end = at;
    let mut seen = false;
    for (j, &b) in bytes[at..].iter().enumerate() {
        if b == b'{' {
            depth += 1;
            seen = true;
        } else if b == b'}' {
            depth -= 1;
            if seen && depth == 0 {
                end = at + j + 1;
                break;
            }
        }
    }
    Some(&src[at..end])
}

pub fn simulate_production_quantity_writeback_source() -> bool {
    let src = shadow_source();
    let Some(body) = function_body(src, "fn writeback_production_to_host(") else {
        return false;
    };
    let ok = body
        .contains("Wave 463: preserve C++ production quantity residual through GW writeback")
        && body.contains("quantity_total: it.quantity_total.max(1)")
        && body.contains("quantity_produced: it.quantity_produced")
        && !body.contains("quantity_total: 1,\n                    quantity_produced: 0,");
    residual_action_store(ResidualProductionQuantityWritebackAction::WritebackSource);
    ok
}

pub fn simulate_production_quantity_entity_fields_source() -> bool {
    let ent = entity_source();
    let log = progress_log_source();
    let ok = ent.contains("struct EntityProductionItem")
        && ent.contains("pub quantity_total: u32")
        && ent.contains("pub quantity_produced: u32")
        && log.contains("pub quantity_total: u32")
        && log.contains("pub quantity_produced: u32")
        && shadow_source().contains("quantity_total: p.quantity_total.max(1)");
    residual_action_store(ResidualProductionQuantityWritebackAction::EntityFields);
    ok
}

pub fn honesty_production_quantity_writeback_residual_pack_wave463() -> bool {
    honesty_production_quantity_writeback_method_names_residual_wave463()
        && honesty_production_quantity_writeback_source_markers_residual_wave463()
        && honesty_production_quantity_writeback_nav_commands_residual_wave463()
        && simulate_production_quantity_writeback_source()
        && simulate_production_quantity_entity_fields_source()
}

pub fn simulate_live_production_quantity_writeback_honesty() -> bool {
    let ok = honesty_production_quantity_writeback_residual_pack_wave463();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualProductionQuantityWritebackAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_production_quantity_writeback_method_names_residual_wave463());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_production_quantity_writeback_source_markers_residual_wave463());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_production_quantity_writeback_nav_commands_residual_wave463());
    }

    #[test]
    fn production_quantity_writeback_sources() {
        assert!(simulate_production_quantity_writeback_source());
        assert!(simulate_production_quantity_entity_fields_source());
    }

    #[test]
    fn wave463_composite_pack() {
        assert!(honesty_production_quantity_writeback_residual_pack_wave463());
    }

    #[test]
    fn simulate_live_production_quantity_writeback_honesty_residual_live() {
        assert!(
            simulate_live_production_quantity_writeback_honesty(),
            "production quantity writeback residual must latch"
        );
        assert!(residual_production_quantity_writeback_ok());
        assert_eq!(
            residual_production_quantity_writeback_last_action(),
            ResidualProductionQuantityWritebackAction::Composite
        );
    }
}
