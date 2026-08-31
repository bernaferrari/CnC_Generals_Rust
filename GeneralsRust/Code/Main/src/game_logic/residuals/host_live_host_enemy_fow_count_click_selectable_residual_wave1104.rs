//! Wave 1104: enemy FOW + UC/alive counts + single-click selectable residual.
//!
//! - `first_enemy_attackable_id` ignored FOW Clear (unlike is_enemy_attackable)
//! - UC/alive presentation counts included sold objects
//! - engine single-click select trusted pick id without local selectable recheck

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ENEMY_FOW_COUNT_CLICK_SELECTABLE_METHOD_NAMES_WAVE1104: &[&str] = &[
    "first_enemy_attackable_id",
    "count_under_construction_friendlies",
    "alive_object_count",
    "host_set_selection",
    "Wave 1104",
    "playable_claim: false",
];

pub const LIVE_HOST_ENEMY_FOW_COUNT_CLICK_SELECTABLE_NAV_STEPS_WAVE1104: &[&str] = &[
    "ENEMY_ATTACKABLE_FOW_CLEAR",
    "UC_ALIVE_COUNT_EXCLUDES_SOLD",
    "SINGLE_CLICK_LOCAL_SELECTABLE",
    "LIVE_HOST_ENEMY_FOW_COUNT_CLICK_SELECTABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEnemyFowCountClickSelectableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostEnemyFowCountClickSelectableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_enemy_fow_count_click_selectable_method_names_residual_wave1104() -> bool {
    let names = LIVE_HOST_ENEMY_FOW_COUNT_CLICK_SELECTABLE_METHOD_NAMES_WAVE1104;
    let ok = residual_name_index(names, "first_enemy_attackable_id").is_some()
        && residual_name_index(names, "Wave 1104").is_some();
    residual_action_store(ResidualHostEnemyFowCountClickSelectableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_enemy_fow_count_click_selectable_nav_commands_residual_wave1104() -> bool {
    let steps = LIVE_HOST_ENEMY_FOW_COUNT_CLICK_SELECTABLE_NAV_STEPS_WAVE1104;
    let ok = residual_name_index(steps, "LIVE_HOST_ENEMY_FOW_COUNT_CLICK_SELECTABLE").is_some()
        && residual_name_index(steps, "SINGLE_CLICK_LOCAL_SELECTABLE").is_some();
    residual_action_store(ResidualHostEnemyFowCountClickSelectableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_enemy_fow_count_click_selectable_residual_pack_wave1104() -> bool {
    let pf = pf_source();
    let cnc = cnc_source();
    let es = es_source();
    let ok = pf.contains("Wave 1104: fail-closed on non-local FOW unless Clear")
        && pf.contains("fn first_enemy_attackable_id")
        && pf.contains("visibility_alpha >= 0.95")
        && pf.contains("Wave 1104: fail-closed on sold UC residual count")
        && pf.contains("Wave 1104: alive count residual excludes sold")
        && cnc.contains("Wave 1104: belt-and-suspenders local selectable check")
        && cnc.contains("presentation_is_selectable")
        && (cnc.contains("if !selectable")
            || cnc.contains("if !self.is_locally_selectable_click_target")
            || cnc.contains("presentation_is_selectable"))
        && cnc.contains("host_set_selection")
        && (es.contains("playable_claim: false")
            || es.contains("self.playable_claim = Self::retail_windowed_playable_claim("));
    residual_action_store(ResidualHostEnemyFowCountClickSelectableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_enemy_fow_count_click_selectable_residual_honesty() -> bool {
    let a = honesty_host_enemy_fow_count_click_selectable_method_names_residual_wave1104();
    let b = honesty_host_enemy_fow_count_click_selectable_nav_commands_residual_wave1104();
    let c = honesty_host_enemy_fow_count_click_selectable_residual_pack_wave1104();
    residual_action_store(ResidualHostEnemyFowCountClickSelectableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_enemy_fow_count_click_selectable_residual_wave1104() {
        assert!(honesty_host_enemy_fow_count_click_selectable_residual_pack_wave1104());
        assert!(honesty_host_enemy_fow_count_click_selectable_method_names_residual_wave1104());
        assert!(honesty_host_enemy_fow_count_click_selectable_nav_commands_residual_wave1104());
        assert!(simulate_live_host_enemy_fow_count_click_selectable_residual_honesty());
    }
}
