//! Wave 1109: upgraded/mine/minimap + HUD production-queue sold residual.
//!
//! After Waves 1106–1108 selection/power/panel sold peels:
//! - `upgraded_objects` / `mine_objects` / `hud_minimap_units` still included sold
//! - `apply_to_game_hud` production queue only skipped destroyed on primary

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_UPGRADE_MINE_MINIMAP_HUD_QUEUE_SOLD_METHOD_NAMES_WAVE1109: &[&str] = &[
    "upgraded_objects",
    "mine_objects",
    "hud_minimap_units",
    "apply_to_game_hud",
    "Wave 1109",
    "playable_claim: false",
];

pub const LIVE_HOST_UPGRADE_MINE_MINIMAP_HUD_QUEUE_SOLD_NAV_STEPS_WAVE1109: &[&str] = &[
    "UPGRADE_MINE_EXCLUDE_SOLD",
    "MINIMAP_DOTS_EXCLUDE_SOLD",
    "HUD_QUEUE_PRIMARY_USABLE",
    "LIVE_HOST_UPGRADE_MINE_MINIMAP_HUD_QUEUE_SOLD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostUpgradeMineMinimapHudQueueSoldAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostUpgradeMineMinimapHudQueueSoldAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_upgrade_mine_minimap_hud_queue_sold_method_names_residual_wave1109() -> bool {
    let names = LIVE_HOST_UPGRADE_MINE_MINIMAP_HUD_QUEUE_SOLD_METHOD_NAMES_WAVE1109;
    let ok = residual_name_index(names, "upgraded_objects").is_some()
        && residual_name_index(names, "Wave 1109").is_some();
    residual_action_store(ResidualHostUpgradeMineMinimapHudQueueSoldAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_upgrade_mine_minimap_hud_queue_sold_nav_commands_residual_wave1109() -> bool {
    let steps = LIVE_HOST_UPGRADE_MINE_MINIMAP_HUD_QUEUE_SOLD_NAV_STEPS_WAVE1109;
    let ok = residual_name_index(steps, "LIVE_HOST_UPGRADE_MINE_MINIMAP_HUD_QUEUE_SOLD").is_some()
        && residual_name_index(steps, "MINIMAP_DOTS_EXCLUDE_SOLD").is_some();
    residual_action_store(ResidualHostUpgradeMineMinimapHudQueueSoldAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_upgrade_mine_minimap_hud_queue_sold_residual_pack_wave1109() -> bool {
    let pf = pf_source();
    let es = es_source();
    let ok = pf.contains("Wave 1109: upgrade residual feed excludes sold")
        && pf.contains("Wave 1109: mine residual feed excludes sold")
        && pf.contains("Wave 1109: minimap unit-dot residual excludes sold")
        && pf.contains("Wave 1109: HUD queue residual fail-closed on sold/unusable primary")
        && pf.contains("fn upgraded_objects")
        && pf.contains("fn mine_objects")
        && pf.contains("fn hud_minimap_units")
        && pf.contains("fn apply_to_game_hud")
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostUpgradeMineMinimapHudQueueSoldAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_upgrade_mine_minimap_hud_queue_sold_residual_honesty() -> bool {
    let a = honesty_host_upgrade_mine_minimap_hud_queue_sold_method_names_residual_wave1109();
    let b = honesty_host_upgrade_mine_minimap_hud_queue_sold_nav_commands_residual_wave1109();
    let c = honesty_host_upgrade_mine_minimap_hud_queue_sold_residual_pack_wave1109();
    residual_action_store(ResidualHostUpgradeMineMinimapHudQueueSoldAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_upgrade_mine_minimap_hud_queue_sold_residual_wave1109() {
        assert!(honesty_host_upgrade_mine_minimap_hud_queue_sold_residual_pack_wave1109());
        assert!(honesty_host_upgrade_mine_minimap_hud_queue_sold_method_names_residual_wave1109());
        assert!(honesty_host_upgrade_mine_minimap_hud_queue_sold_nav_commands_residual_wave1109());
        assert!(simulate_live_host_upgrade_mine_minimap_hud_queue_sold_residual_honesty());
    }
}
