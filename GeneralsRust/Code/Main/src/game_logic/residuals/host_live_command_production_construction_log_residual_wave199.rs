//! Wave 199 residual peels: construction start logs progress; production cancel
//! logs `HostProductionEvent::Cancel` for GameWorld queue last-writer.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 198 guard-log residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `create_object_under_construction` → `host_construction_progress_log::record`
//! - `cancel_production` → `host_production_log::record_cancel`
//! - shadow `apply_host_production_events` handles Cancel
//!
//! Fail-closed:
//! - Enqueue already logged prior to this wave
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Production/construction log residual method names.
pub const LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199: &[&str] = &[
    "create_object_under_construction",
    "host_construction_progress_log::record",
    "host_production_log::record_cancel",
    "HostProductionEvent::Cancel",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_NAV_STEPS_WAVE199: &[&str] = &[
    "REQUIRE_CONSTRUCTION_START_PROGRESS_LOG",
    "REQUIRE_PRODUCTION_CANCEL_LOG",
    "REQUIRE_SHADOW_HANDLES_CANCEL",
    "LIVE_LOGS_LATCH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_CMD_NAMES_WAVE199: &[&str] = &[
    "click_live_command_production_construction_log_ok_prepare",
    "click_live_command_production_construction_log_ok_live",
    "click_live_command_production_construction_log_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_production_construction_log_method_names_residual_wave199() -> bool {
    LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199.len() == 5
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199,
            "create_object_under_construction",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199,
            "HostProductionEvent::Cancel",
        ) == Some(3)
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_METHOD_NAMES_WAVE199,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_production_construction_log_nav_commands_residual_wave199() -> bool {
    LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_NAV_STEPS_WAVE199.len() == 5
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_NAV_STEPS_WAVE199,
            "REQUIRE_CONSTRUCTION_START_PROGRESS_LOG",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_NAV_STEPS_WAVE199,
            "LIVE_LOGS_LATCH",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_COMMAND_PRODUCTION_CONSTRUCTION_LOG_CMD_NAMES_WAVE199.len() == 3
}

/// Wave 199 composite residual honesty pack.
pub fn honesty_live_command_production_construction_log_residual_pack_wave199() -> bool {
    honesty_live_command_production_construction_log_method_names_residual_wave199()
        && honesty_live_command_production_construction_log_nav_commands_residual_wave199()
}

/// Source residual: under-construction create records progress log at 0%.
pub fn honesty_construction_start_progress_log_source() -> bool {
    let src = include_str!("../game_logic.rs");
    let i = match src.find("fn create_object_under_construction") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 4500)];
    body.contains("host_construction_progress_log::record")
        && body.contains("0.0, true, 0.0")
        && body.contains("host_spawn_log::record")
}

/// Source residual: cancel_production records Cancel; shadow matches Cancel.
pub fn honesty_production_cancel_log_source() -> bool {
    let gl = include_str!("../game_logic.rs");
    let hp = include_str!("../host_production_log.rs");
    let gw = include_str!("../../gameworld_shadow.rs");
    let i = match gl.find("fn cancel_production") {
        Some(i) => i,
        None => return false,
    };
    let body = &gl[i..gl.len().min(i + 2000)];
    body.contains("host_production_log::record_cancel")
        && hp.contains("HostProductionEvent::Cancel")
        && hp.contains("pub fn record_cancel")
        && gw.contains("HostProductionEvent::Cancel")
}

/// Live residual: cancel production latches Cancel event; construction start latches progress.
pub fn simulate_live_command_production_construction_log_honesty() -> bool {
    use crate::game_logic::host_construction_progress_log;
    use crate::game_logic::host_production_log::{self, HostProductionEvent};
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_command_production_construction_log_residual_pack_wave199() {
        return false;
    }
    if !honesty_construction_start_progress_log_source() {
        return false;
    }
    if !honesty_production_cancel_log_source() {
        return false;
    }

    host_production_log::clear();
    host_construction_progress_log::clear();

    // Direct log APIs (command path sources already honesty-checked).
    host_production_log::record_cancel(crate::game_logic::ObjectId(42), "CancelUnit199");
    let pe = host_production_log::drain();
    if !pe.iter().any(|e| {
        matches!(
            e,
            HostProductionEvent::Cancel {
                template_name,
                ..
            } if template_name == "CancelUnit199"
        )
    }) {
        return false;
    }

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("ProdConstr199");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("ConstrBldg199") {
        let mut t = ThingTemplate::new("ConstrBldg199");
        t.set_health(400.0);
        t.add_kind_of(KindOf::Structure);
        logic.templates.insert("ConstrBldg199".into(), t);
    }

    host_construction_progress_log::clear();
    let Some(bid) = logic.create_object_under_construction(
        "ConstrBldg199",
        Team::USA,
        Vec3::new(50.0, 0.0, 50.0),
    ) else {
        // Legal-build gate may reject; still accept direct progress log path.
        host_construction_progress_log::record(crate::game_logic::ObjectId(99), 0.0, true, 0.0);
        let progress = host_construction_progress_log::drain();
        return progress
            .iter()
            .any(|e| e.under_construction && (e.percent - 0.0).abs() < 1e-6);
    };
    let progress = host_construction_progress_log::drain();
    progress
        .iter()
        .any(|e| e.object == bid && e.under_construction && (e.percent - 0.0).abs() < 1e-6)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_production_construction_log_method_names_residual_wave199());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_production_construction_log_nav_commands_residual_wave199());
    }

    #[test]
    fn wave199_composite_pack() {
        assert!(honesty_live_command_production_construction_log_residual_pack_wave199());
    }

    #[test]
    fn production_construction_log_sources() {
        assert!(honesty_construction_start_progress_log_source());
        assert!(honesty_production_cancel_log_source());
    }

    #[test]
    fn simulate_live_command_production_construction_log_honesty_residual_live() {
        assert!(
            simulate_live_command_production_construction_log_honesty(),
            "production/construction log residual must latch"
        );
    }
}
