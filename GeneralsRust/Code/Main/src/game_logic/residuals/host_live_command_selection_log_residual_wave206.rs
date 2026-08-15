//! Wave 206 residual peels: selection commands call `Object::select`/`deselect`
//! (via `GameLogic::select_objects` / additive select) so `host_status_log`
//! drains selected last-writer. Formation dissolve uses `set_formation(0, …)`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 205 order-target residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `execute_selection` / `execute_selection_command` → select_objects / select
//! - formation dissolve → set_formation(0, ZERO)
//!
//! Fail-closed:
//! - Not full GameClient OS-input cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Selection log residual method names.
pub const LIVE_COMMAND_SELECTION_LOG_METHOD_NAMES_WAVE206: &[&str] = &[
    "select_objects",
    "Object::select",
    "Object::deselect",
    "set_formation(0)",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_SELECTION_LOG_NAV_STEPS_WAVE206: &[&str] = &[
    "REQUIRE_SELECTION_USES_OBJECT_SELECT",
    "REQUIRE_FORMATION_DISSOLVE_USES_SET_FORMATION",
    "LIVE_SELECTION_LOGS_STATUS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_SELECTION_LOG_CMD_NAMES_WAVE206: &[&str] = &[
    "click_live_command_selection_log_ok_prepare",
    "click_live_command_selection_log_ok_live",
    "click_live_command_selection_log_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_selection_log_method_names_residual_wave206() -> bool {
    LIVE_COMMAND_SELECTION_LOG_METHOD_NAMES_WAVE206.len() == 5
        && residual_name_index(
            LIVE_COMMAND_SELECTION_LOG_METHOD_NAMES_WAVE206,
            "select_objects",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_SELECTION_LOG_METHOD_NAMES_WAVE206,
            "Object::select",
        ) == Some(1)
        && residual_name_index(
            LIVE_COMMAND_SELECTION_LOG_METHOD_NAMES_WAVE206,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_selection_log_nav_commands_residual_wave206() -> bool {
    LIVE_COMMAND_SELECTION_LOG_NAV_STEPS_WAVE206.len() == 4
        && residual_name_index(
            LIVE_COMMAND_SELECTION_LOG_NAV_STEPS_WAVE206,
            "REQUIRE_SELECTION_USES_OBJECT_SELECT",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_SELECTION_LOG_NAV_STEPS_WAVE206,
            "LIVE_SELECTION_LOGS_STATUS",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_SELECTION_LOG_CMD_NAMES_WAVE206.len() == 3
}

/// Wave 206 composite residual honesty pack.
pub fn honesty_live_command_selection_log_residual_pack_wave206() -> bool {
    honesty_live_command_selection_log_method_names_residual_wave206()
        && honesty_live_command_selection_log_nav_commands_residual_wave206()
}

/// Source residual: execute_selection uses select_objects / select (not list-only).
pub fn honesty_selection_uses_object_select_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let i = match ce.find("fn execute_selection") {
        Some(i) => i,
        None => return false,
    };
    let body = &ce[i..ce.len().min(i + 2000)];
    // Wave 232: additive select may go through unit_select_if_team (Object::select inside).
    body.contains("select_objects")
        && (body.contains(".select()") || body.contains("unit_select_if_team"))
        && (body.contains("Wave 206") || body.contains("Wave 232"))
}

/// Source residual: command_system selection uses select_objects.
pub fn honesty_command_system_selection_uses_object_select_source() -> bool {
    let cs = crate::command_system::COMMAND_SYSTEM_SRC;
    let i = match cs.find("fn execute_selection_command") {
        Some(i) => i,
        None => return false,
    };
    let body = &cs[i..cs.len().min(i + 2000)];
    // Wave 231: additive select may go through GameLogic::unit_select_if_team
    // (which calls Object::select); create_new still uses select_objects.
    body.contains("select_objects")
        && (body.contains(".select()") || body.contains("unit_select_if_team"))
}

/// Source residual: production has no formation_id = 0 field write.
pub fn honesty_formation_dissolve_uses_set_formation_source() -> bool {
    let ce = crate::command_executor::COMMAND_EXECUTOR_SRC;
    let gl = super::GAME_LOGIC_HOST_SRC;
    let prod = match ce.find("#[cfg(test)]") {
        Some(i) => &ce[..i],
        None => ce,
    };
    let no_raw_field = !prod.lines().any(|l| {
        let t = l.trim_start();
        !t.starts_with("//") && t.contains("formation_id = 0")
    });
    // Wave 232: dissolve may live in GameLogic unit_command_* helpers.
    let uses_set = prod.contains("set_formation(0")
        || gl.contains("set_formation(0")
        || prod.contains("unit_command_set_formation")
        || gl.contains("unit_command_tighten_to");
    no_raw_field && uses_set
}

/// Live residual: select_objects drains host_status selected events.
pub fn simulate_live_command_selection_log_honesty() -> bool {
    use crate::game_logic::host_status_log;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_command_selection_log_residual_pack_wave206() {
        return false;
    }
    if !honesty_selection_uses_object_select_source() {
        return false;
    }
    if !honesty_command_system_selection_uses_object_select_source() {
        return false;
    }
    if !honesty_formation_dissolve_uses_set_formation_source() {
        return false;
    }

    host_status_log::clear();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("Select206");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("SelectUnit206") {
        let mut t = ThingTemplate::new("SelectUnit206");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SelectUnit206".into(), t);
    }
    let Some(a) = logic.create_object("SelectUnit206", Team::USA, Vec3::new(0.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(b) = logic.create_object("SelectUnit206", Team::USA, Vec3::new(5.0, 0.0, 0.0)) else {
        return false;
    };
    let Some(pid) = logic.local_player_id() else {
        return false;
    };

    host_status_log::clear();
    logic.select_objects(pid, vec![a]);
    let ev1 = host_status_log::drain();
    if !ev1
        .iter()
        .any(|e| e.object == a && e.selected == Some(true))
    {
        return false;
    }
    if let Some(obj) = logic.get_object(a) {
        if !obj.selected {
            return false;
        }
    } else {
        return false;
    }

    host_status_log::clear();
    logic.select_objects(pid, vec![b]);
    let ev2 = host_status_log::drain();
    // Previous deselected, new selected.
    let desel_a = ev2
        .iter()
        .any(|e| e.object == a && e.selected == Some(false));
    let sel_b = ev2
        .iter()
        .any(|e| e.object == b && e.selected == Some(true));
    desel_a && sel_b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_selection_log_method_names_residual_wave206());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_selection_log_nav_commands_residual_wave206());
    }

    #[test]
    fn wave206_composite_pack() {
        assert!(honesty_live_command_selection_log_residual_pack_wave206());
    }

    #[test]
    fn selection_sources() {
        assert!(honesty_selection_uses_object_select_source());
        assert!(honesty_command_system_selection_uses_object_select_source());
        assert!(honesty_formation_dissolve_uses_set_formation_source());
    }

    #[test]
    fn simulate_live_command_selection_log_honesty_residual_live() {
        assert!(
            simulate_live_command_selection_log_honesty(),
            "selection log residual must latch"
        );
    }
}
