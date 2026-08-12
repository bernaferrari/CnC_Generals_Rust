//! Wave 212 residual peels: `start_sell_object` deselects via `Object::deselect`
//! so `host_status_log` drains selected=false (no bare `status.selected = false`).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 211 host beacon presentation residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `GameLogic::start_sell_object` → `deselect` + `set_status_sold`
//! - `execute_sell` → `start_sell_object`
//!
//! Fail-closed:
//! - Not full C++ BuildAssistant multi-frame sell scaffold refund matrix
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Sell deselect residual method names.
pub const LIVE_COMMAND_SELL_DESELECT_LOG_METHOD_NAMES_WAVE212: &[&str] = &[
    "start_sell_object",
    "deselect",
    "set_status_sold",
    "execute_sell",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_SELL_DESELECT_LOG_NAV_STEPS_WAVE212: &[&str] = &[
    "REQUIRE_START_SELL_USES_DESELECT",
    "REQUIRE_EXECUTE_SELL_STARTS_SELL",
    "LIVE_SELL_DESELECTS_LOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_SELL_DESELECT_LOG_CMD_NAMES_WAVE212: &[&str] = &[
    "click_live_command_sell_deselect_log_ok_prepare",
    "click_live_command_sell_deselect_log_ok_live",
    "click_live_command_sell_deselect_log_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_sell_deselect_log_method_names_residual_wave212() -> bool {
    LIVE_COMMAND_SELL_DESELECT_LOG_METHOD_NAMES_WAVE212.len() == 5
        && residual_name_index(
            LIVE_COMMAND_SELL_DESELECT_LOG_METHOD_NAMES_WAVE212,
            "start_sell_object",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_SELL_DESELECT_LOG_METHOD_NAMES_WAVE212,
            "deselect",
        ) == Some(1)
        && residual_name_index(
            LIVE_COMMAND_SELL_DESELECT_LOG_METHOD_NAMES_WAVE212,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_sell_deselect_log_nav_commands_residual_wave212() -> bool {
    LIVE_COMMAND_SELL_DESELECT_LOG_NAV_STEPS_WAVE212.len() == 4
        && residual_name_index(
            LIVE_COMMAND_SELL_DESELECT_LOG_NAV_STEPS_WAVE212,
            "REQUIRE_START_SELL_USES_DESELECT",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_SELL_DESELECT_LOG_NAV_STEPS_WAVE212,
            "LIVE_SELL_DESELECTS_LOG",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_SELL_DESELECT_LOG_CMD_NAMES_WAVE212.len() == 3
}

/// Wave 212 composite residual honesty pack.
pub fn honesty_live_command_sell_deselect_log_residual_pack_wave212() -> bool {
    honesty_live_command_sell_deselect_log_method_names_residual_wave212()
        && honesty_live_command_sell_deselect_log_nav_commands_residual_wave212()
}

/// Source residual: start_sell_object uses deselect, not bare selected=false.
pub fn honesty_start_sell_uses_deselect_source() -> bool {
    let src = include_str!("../game_logic.rs");
    let i = match src.find("fn start_sell_object") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 2000)];
    body.contains("deselect()")
        && body.contains("set_status_sold(true)")
        && !body.contains("status.selected = false")
}

/// Source residual: execute_sell calls start_sell_object.
pub fn honesty_execute_sell_starts_sell_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = match ce.find("fn execute_sell") {
        Some(i) => i,
        None => return false,
    };
    let body = &ce[i..ce.len().min(i + 1200)];
    body.contains("start_sell_object")
}

/// Live residual: start_sell_object drains selected=false status event.
pub fn simulate_live_command_sell_deselect_log_honesty() -> bool {
    use crate::game_logic::host_status_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_command_sell_deselect_log_residual_pack_wave212() {
        return false;
    }
    if !honesty_start_sell_uses_deselect_source() {
        return false;
    }
    if !honesty_execute_sell_starts_sell_source() {
        return false;
    }

    host_status_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Sell212");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("SellBldg212") {
        let mut t = ThingTemplate::new("SellBldg212");
        t.set_health(500.0);
        t.add_kind_of(KindOf::Structure);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SellBldg212".into(), t);
    }
    let Some(bid) = logic.create_object("SellBldg212", Team::USA, Vec3::new(15.0, 0.0, 15.0))
    else {
        return false;
    };
    // Select then sell.
    if let Some(obj) = logic.get_object_mut(bid) {
        obj.select();
    }
    host_status_log::clear();
    if !logic.start_sell_object(bid) {
        return false;
    }
    let events = host_status_log::drain();
    let sold = events
        .iter()
        .any(|e| e.object == bid && e.sold == Some(true));
    let desel = events
        .iter()
        .any(|e| e.object == bid && e.selected == Some(false));
    let still_selected = logic.get_object(bid).map(|o| o.selected).unwrap_or(true);
    sold && desel && !still_selected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_sell_deselect_log_method_names_residual_wave212());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_sell_deselect_log_nav_commands_residual_wave212());
    }

    #[test]
    fn wave212_composite_pack() {
        assert!(honesty_live_command_sell_deselect_log_residual_pack_wave212());
    }

    #[test]
    fn sell_deselect_sources() {
        assert!(honesty_start_sell_uses_deselect_source());
        assert!(honesty_execute_sell_starts_sell_source());
    }

    #[test]
    fn simulate_live_command_sell_deselect_log_honesty_residual_live() {
        assert!(
            simulate_live_command_sell_deselect_log_honesty(),
            "sell deselect log residual must latch"
        );
    }
}
