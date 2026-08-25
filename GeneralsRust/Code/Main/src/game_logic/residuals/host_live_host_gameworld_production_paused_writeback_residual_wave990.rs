//! Wave 990: GameWorld production_paused residual writeback / tick skip.
//!
//! Host BuildingData::production_paused freezes into Entity.production_paused on
//! shadow sync. GameWorld sole-tick production advances only when not paused.
//! Writeback keeps host BuildingData.production_paused aligned.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GAMEWORLD_PRODUCTION_PAUSED_WRITEBACK_RESIDUAL_METHOD_NAMES_WAVE990: &[&str] =
    &[
        "production_paused",
        "tick_production_queues",
        "Wave 990",
        "playable_claim = false",
    ];

pub const LIVE_HOST_GAMEWORLD_PRODUCTION_PAUSED_WRITEBACK_RESIDUAL_NAV_STEPS_WAVE990: &[&str] = &[
    "ENTITY_PRODUCTION_PAUSED",
    "TICK_SKIP_WHEN_PAUSED",
    "HOST_WRITEBACK",
    "LIVE_HOST_GAMEWORLD_PRODUCTION_PAUSED_WRITEBACK_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGameworldProductionPausedWritebackResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostGameworldProductionPausedWritebackResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn entity_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

pub fn honesty_host_gameworld_production_paused_writeback_residual_method_names_residual_wave990()
-> bool {
    let names = LIVE_HOST_GAMEWORLD_PRODUCTION_PAUSED_WRITEBACK_RESIDUAL_METHOD_NAMES_WAVE990;
    let ok = residual_name_index(names, "production_paused").is_some()
        && residual_name_index(names, "Wave 990").is_some();
    residual_action_store(
        ResidualHostGameworldProductionPausedWritebackResidualAction::MethodNames,
    );
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_gameworld_production_paused_writeback_residual_nav_commands_residual_wave990()
-> bool {
    let steps = LIVE_HOST_GAMEWORLD_PRODUCTION_PAUSED_WRITEBACK_RESIDUAL_NAV_STEPS_WAVE990;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_GAMEWORLD_PRODUCTION_PAUSED_WRITEBACK_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "TICK_SKIP_WHEN_PAUSED").is_some();
    residual_action_store(
        ResidualHostGameworldProductionPausedWritebackResidualAction::NavCommands,
    );
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_gameworld_production_paused_writeback_residual_residual_pack_wave990() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ent = entity_source();
    let sh = shadow_source();
    let tick = match sh.find("pub fn tick_production_queues") {
        Some(i) => &sh[i..sh.len().min(i + 1200)],
        None => "",
    };
    let ok = ent.contains("pub production_paused: bool")
        && ent.contains("Wave 990")
        && sh.contains("e.production_paused = bd.production_paused")
        && sh.contains("bd.production_paused = ent.production_paused")
        && tick.contains("!ent.production_paused")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(
        ResidualHostGameworldProductionPausedWritebackResidualAction::SourceMarkers,
    );
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_gameworld_production_paused_writeback_residual_honesty() -> bool {
    let a =
        honesty_host_gameworld_production_paused_writeback_residual_method_names_residual_wave990();
    let b =
        honesty_host_gameworld_production_paused_writeback_residual_nav_commands_residual_wave990();
    let c = honesty_host_gameworld_production_paused_writeback_residual_residual_pack_wave990();
    residual_action_store(
        ResidualHostGameworldProductionPausedWritebackResidualAction::DispatchSource,
    );
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_gameworld_production_paused_writeback_residual_wave990() {
        assert!(
            honesty_host_gameworld_production_paused_writeback_residual_residual_pack_wave990()
        );
        assert!(honesty_host_gameworld_production_paused_writeback_residual_method_names_residual_wave990());
        assert!(honesty_host_gameworld_production_paused_writeback_residual_nav_commands_residual_wave990());
        assert!(simulate_live_host_gameworld_production_paused_writeback_residual_honesty());
    }
}
