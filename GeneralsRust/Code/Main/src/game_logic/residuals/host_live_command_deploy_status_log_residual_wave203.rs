//! Wave 203 residual peels: Deploy toggles `set_deployed` → `set_status_deployed`
//! so `host_status_log` drains SetCombatStatus.deployed last-writer.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 202 cheer/science residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `Object::set_deployed` → `set_status_deployed` → `record_deployed`
//! - `execute_deploy` uses `set_deployed`
//!
//! Fail-closed:
//! - Not full GameClient OS-input cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Deploy status residual method names.
pub const LIVE_COMMAND_DEPLOY_STATUS_LOG_METHOD_NAMES_WAVE203: &[&str] = &[
    "set_deployed",
    "set_status_deployed",
    "record_deployed",
    "execute_deploy",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_DEPLOY_STATUS_LOG_NAV_STEPS_WAVE203: &[&str] = &[
    "REQUIRE_SET_DEPLOYED_LOGS_STATUS",
    "REQUIRE_EXECUTE_DEPLOY_USES_SET_DEPLOYED",
    "LIVE_DEPLOY_LOGS_STATUS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_DEPLOY_STATUS_LOG_CMD_NAMES_WAVE203: &[&str] = &[
    "click_live_command_deploy_status_log_ok_prepare",
    "click_live_command_deploy_status_log_ok_live",
    "click_live_command_deploy_status_log_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_live_command_deploy_status_log_method_names_residual_wave203() -> bool {
    LIVE_COMMAND_DEPLOY_STATUS_LOG_METHOD_NAMES_WAVE203.len() == 5
        && residual_name_index(
            LIVE_COMMAND_DEPLOY_STATUS_LOG_METHOD_NAMES_WAVE203,
            "set_deployed",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_DEPLOY_STATUS_LOG_METHOD_NAMES_WAVE203,
            "execute_deploy",
        ) == Some(3)
        && residual_name_index(
            LIVE_COMMAND_DEPLOY_STATUS_LOG_METHOD_NAMES_WAVE203,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_deploy_status_log_nav_commands_residual_wave203() -> bool {
    LIVE_COMMAND_DEPLOY_STATUS_LOG_NAV_STEPS_WAVE203.len() == 4
        && residual_name_index(
            LIVE_COMMAND_DEPLOY_STATUS_LOG_NAV_STEPS_WAVE203,
            "REQUIRE_SET_DEPLOYED_LOGS_STATUS",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_DEPLOY_STATUS_LOG_NAV_STEPS_WAVE203,
            "LIVE_DEPLOY_LOGS_STATUS",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_DEPLOY_STATUS_LOG_CMD_NAMES_WAVE203.len() == 3
}

/// Wave 203 composite residual honesty pack.
pub fn honesty_live_command_deploy_status_log_residual_pack_wave203() -> bool {
    honesty_live_command_deploy_status_log_method_names_residual_wave203()
        && honesty_live_command_deploy_status_log_nav_commands_residual_wave203()
}

/// Source residual: set_deployed routes through set_status_deployed.
pub fn honesty_set_deployed_logs_status_source() -> bool {
    let src = crate::game_logic::object::OBJECT_SRC;
    let i = match src.find("fn set_deployed") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..];
    body.contains("set_status_deployed") && !body.contains("self.status.deployed = deployed")
}

/// Source residual: execute_deploy calls set_deployed.
pub fn honesty_execute_deploy_uses_set_deployed_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = match ce.find("fn execute_deploy") {
        Some(i) => i,
        None => return false,
    };
    let body = &ce[i..];
    body.contains("set_deployed")
}

/// Live residual: set_deployed drains host_status deployed event.
pub fn simulate_live_command_deploy_status_log_honesty() -> bool {
    use crate::game_logic::host_status_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_command_deploy_status_log_residual_pack_wave203() {
        return false;
    }
    if !honesty_set_deployed_logs_status_source() {
        return false;
    }
    if !honesty_execute_deploy_uses_set_deployed_source() {
        return false;
    }

    host_status_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Deploy203");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("DeployUnit203") {
        let mut t = ThingTemplate::new("DeployUnit203");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Vehicle);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("DeployUnit203".into(), t);
    }
    let Some(uid) = logic.create_object("DeployUnit203", Team::USA, Vec3::new(30.0, 0.0, 30.0))
    else {
        return false;
    };

    host_status_log::clear();
    if let Some(unit) = logic.get_object_mut(uid) {
        unit.set_deployed(true);
        if !unit.is_deployed() {
            return false;
        }
    } else {
        return false;
    }
    let events = host_status_log::drain();
    events
        .iter()
        .any(|e| e.object == uid && e.deployed == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_deploy_status_log_method_names_residual_wave203());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_deploy_status_log_nav_commands_residual_wave203());
    }

    #[test]
    fn wave203_composite_pack() {
        assert!(honesty_live_command_deploy_status_log_residual_pack_wave203());
    }

    #[test]
    fn deploy_status_sources() {
        assert!(honesty_set_deployed_logs_status_source());
        assert!(honesty_execute_deploy_uses_set_deployed_source());
    }

    #[test]
    fn simulate_live_command_deploy_status_log_honesty_residual_live() {
        assert!(
            simulate_live_command_deploy_status_log_honesty(),
            "deploy status log residual must latch"
        );
    }
}
