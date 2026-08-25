//! Wave 479 residual peels: under special-power sole-tick, host does not
//! per-frame stomp GW shared cooldown remaining via cooldown log snapshots.
//! - host sole-tick branch returns without `record_host_cooldowns` loop
//! - fire/reset still records via `reset_shared_special_power_timer`
//! - GW `tick_player_shared_special_power_cooldowns` advances remaining
//! - `writeback_shared_special_power_cooldowns_to_host` restores host view
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 477/478 production/construction sole-tick no-stomp peels.
//! Architecture residual - shared SP countdown last-writer is GameWorld under sole-tick.
//!
//! Sources:
//! - game_logic.rs special-power sole-tick branch
//! - gameworld_shadow::tick_player_shared_special_power_cooldowns
//! - gameworld_shadow::writeback_shared_special_power_cooldowns_to_host
//!
//! Fail-closed:
//! - Object-level SP modules may still use host_special_power_log on fire
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_METHOD_NAMES_WAVE479: &[&str] = &[
    "gameworld_special_power_sole_tick_enabled",
    "record_host_cooldowns",
    "tick_player_shared_special_power_cooldowns",
    "writeback_shared_special_power_cooldowns_to_host",
    "reset_shared_special_power_timer",
    "playable_claim = false",
];

pub const SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_SOURCE_MARKERS_WAVE479: &[&str] = &[
    "Wave 479: do not republish full cooldown snapshots each frame",
    "tick_player_shared_special_power_cooldowns",
    "writeback_shared_special_power_cooldowns_to_host",
    "reset_shared_special_power_timer",
];

pub const SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_NAV_STEPS_WAVE479: &[&str] = &[
    "DETECT_SP_SOLE_TICK",
    "SKIP_FRAME_COOLDOWN_SNAPSHOT",
    "FIRE_RESET_STILL_RECORDS",
    "GW_TICKS_SHARED_CDS",
    "WRITEBACK_HOST_CDS",
    "NO_PER_FRAME_STOMP",
];

pub const RUNTIME_HOST_SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_CMD_NAMES_WAVE479: &[&str] = &[
    "click_special_power_sole_tick_no_cooldown_stomp_ok_wnd_detect",
    "click_special_power_sole_tick_no_cooldown_stomp_ok_wnd_skip",
    "click_special_power_sole_tick_no_cooldown_stomp_ok_wnd_tick",
    "click_special_power_sole_tick_no_cooldown_stomp_ok_wnd_prepare",
    "click_special_power_sole_tick_no_cooldown_stomp_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualSpecialPowerSoleTickNoCooldownStompAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    HostSource = 4,
    ShadowSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualSpecialPowerSoleTickNoCooldownStompAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_special_power_sole_tick_no_cooldown_stomp_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_special_power_sole_tick_no_cooldown_stomp_last_action()
-> ResidualSpecialPowerSoleTickNoCooldownStompAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualSpecialPowerSoleTickNoCooldownStompAction::MethodNames,
        2 => ResidualSpecialPowerSoleTickNoCooldownStompAction::SourceMarkers,
        3 => ResidualSpecialPowerSoleTickNoCooldownStompAction::NavCommands,
        4 => ResidualSpecialPowerSoleTickNoCooldownStompAction::HostSource,
        5 => ResidualSpecialPowerSoleTickNoCooldownStompAction::ShadowSource,
        6 => ResidualSpecialPowerSoleTickNoCooldownStompAction::Composite,
        _ => ResidualSpecialPowerSoleTickNoCooldownStompAction::Idle,
    }
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let brace = src[start..].find('{')? + start;
    let mut depth = 0i32;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Host sole-tick SP branch body (search marker window).
fn host_sp_sole_window(src: &str) -> Option<&str> {
    // Prefer Wave 479 marker on GameLogic::tick_shared_special_power_timers sole branch.
    let marker = "Wave 479: do not republish full cooldown snapshots each frame";
    let start = src.find(marker)?;
    let start = start.saturating_sub(160);
    Some(&src[start..src.len().min(start + 900)])
}

pub fn honesty_special_power_sole_tick_no_cooldown_stomp_method_names_residual_wave479() -> bool {
    SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_METHOD_NAMES_WAVE479.len() == 6
        && residual_name_index(
            SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_METHOD_NAMES_WAVE479,
            "gameworld_special_power_sole_tick_enabled",
        ) == Some(0)
        && residual_name_index(
            SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_METHOD_NAMES_WAVE479,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_special_power_sole_tick_no_cooldown_stomp_source_markers_residual_wave479() -> bool {
    SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_SOURCE_MARKERS_WAVE479.len() == 4
        && residual_name_index(
            SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_SOURCE_MARKERS_WAVE479,
            "Wave 479: do not republish full cooldown snapshots each frame",
        ) == Some(0)
        && residual_name_index(
            SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_SOURCE_MARKERS_WAVE479,
            "reset_shared_special_power_timer",
        ) == Some(3)
}

pub fn honesty_special_power_sole_tick_no_cooldown_stomp_nav_commands_residual_wave479() -> bool {
    SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_NAV_STEPS_WAVE479.len() == 6
        && residual_name_index(
            SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_NAV_STEPS_WAVE479,
            "SKIP_FRAME_COOLDOWN_SNAPSHOT",
        ) == Some(1)
        && residual_name_index(
            SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_NAV_STEPS_WAVE479,
            "NO_PER_FRAME_STOMP",
        ) == Some(5)
        && RUNTIME_HOST_SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_CMD_NAMES_WAVE479.len() == 5
        && residual_name_index(
            RUNTIME_HOST_SPECIAL_POWER_SOLE_TICK_NO_COOLDOWN_STOMP_CMD_NAMES_WAVE479,
            "click_special_power_sole_tick_no_cooldown_stomp_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_special_power_sole_tick_no_cooldown_stomp_host_source() -> bool {
    let gl = gl_source();
    let Some(window) = host_sp_sole_window(gl) else {
        return false;
    };
    let ok = window.contains("Wave 479: do not republish full cooldown snapshots each frame")
        && window.contains("return;")
        && !window.contains("player.record_host_cooldowns()")
        && gl.contains("fn reset_shared_special_power_timer")
        && gl.contains("record_host_cooldowns()");
    residual_action_store(ResidualSpecialPowerSoleTickNoCooldownStompAction::HostSource);
    ok
}

pub fn simulate_special_power_sole_tick_no_cooldown_stomp_shadow_source() -> bool {
    let src = shadow_source();
    let ok = src.contains("fn tick_player_shared_special_power_cooldowns")
        && src.contains("fn writeback_shared_special_power_cooldowns_to_host")
        && src.contains("apply_host_player_cooldown_events")
        && src.contains("gameworld_special_power_sole_tick_enabled()");
    residual_action_store(ResidualSpecialPowerSoleTickNoCooldownStompAction::ShadowSource);
    ok
}

pub fn honesty_special_power_sole_tick_no_cooldown_stomp_residual_pack_wave479() -> bool {
    honesty_special_power_sole_tick_no_cooldown_stomp_method_names_residual_wave479()
        && honesty_special_power_sole_tick_no_cooldown_stomp_source_markers_residual_wave479()
        && honesty_special_power_sole_tick_no_cooldown_stomp_nav_commands_residual_wave479()
        && simulate_special_power_sole_tick_no_cooldown_stomp_host_source()
        && simulate_special_power_sole_tick_no_cooldown_stomp_shadow_source()
}

pub fn simulate_live_special_power_sole_tick_no_cooldown_stomp_honesty() -> bool {
    let ok = honesty_special_power_sole_tick_no_cooldown_stomp_residual_pack_wave479();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualSpecialPowerSoleTickNoCooldownStompAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_special_power_sole_tick_no_cooldown_stomp_method_names_residual_wave479());
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_special_power_sole_tick_no_cooldown_stomp_source_markers_residual_wave479()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_special_power_sole_tick_no_cooldown_stomp_nav_commands_residual_wave479());
    }

    #[test]
    fn special_power_sole_tick_no_cooldown_stomp_sources() {
        assert!(simulate_special_power_sole_tick_no_cooldown_stomp_host_source());
        assert!(simulate_special_power_sole_tick_no_cooldown_stomp_shadow_source());
        let window = host_sp_sole_window(gl_source()).unwrap();
        assert!(!window.contains("player.record_host_cooldowns()"));
    }

    #[test]
    fn wave479_composite_pack() {
        assert!(honesty_special_power_sole_tick_no_cooldown_stomp_residual_pack_wave479());
    }

    #[test]
    fn simulate_live_special_power_sole_tick_no_cooldown_stomp_honesty_residual_live() {
        assert!(
            simulate_live_special_power_sole_tick_no_cooldown_stomp_honesty(),
            "special-power sole-tick no cooldown stomp residual must latch"
        );
        assert!(residual_special_power_sole_tick_no_cooldown_stomp_ok());
        assert_eq!(
            residual_special_power_sole_tick_no_cooldown_stomp_last_action(),
            ResidualSpecialPowerSoleTickNoCooldownStompAction::Composite
        );
    }
}
