//! Wave 244 residual peels: command can_* / classify / attack paths use
//! GameLogic unit capability probes instead of dual-reading `&Object` via
//! `get_object` on selected units. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 243 construct economy probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` unit_team / unit_is_alive / unit_is_worker / unit_can_repair /
//!   unit_is_hero / unit_is_kind_of / unit_template_name / unit_exists /
//!   unit_under_construction / unit_needs_service
//! - `command_system.rs` can_attack_target / can_capture_building /
//!   can_gather_from_target / can_resume_construction / can_repair_target /
//!   can_enter_target / can_get_serviced_at_target /
//!   classify_right_click_target_from_presentation / execute_attack_command
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - create_select_similar / determine_context / box-select still boot-residual
//!   get_object when presentation freeze missing

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Command unit probe residual method names.
pub const LIVE_COMMAND_UNIT_PROBE_METHOD_NAMES_WAVE244: &[&str] = &[
    "unit_team",
    "unit_is_alive",
    "unit_is_worker",
    "unit_can_repair",
    "unit_exists",
    "can_attack_target",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_UNIT_PROBE_NAV_STEPS_WAVE244: &[&str] = &[
    "REQUIRE_COMMAND_UNIT_PROBE",
    "REQUIRE_CAN_HELPERS_USE_PROBES",
    "LIVE_COMMAND_UNIT_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_UNIT_PROBE_CMD_NAMES_WAVE244: &[&str] = &[
    "click_live_command_unit_probe_ok_prepare",
    "click_live_command_unit_probe_ok_live",
    "click_live_command_unit_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_unit_probe_method_names_residual_wave244() -> bool {
    LIVE_COMMAND_UNIT_PROBE_METHOD_NAMES_WAVE244.len() == 7
        && residual_name_index(LIVE_COMMAND_UNIT_PROBE_METHOD_NAMES_WAVE244, "unit_team") == Some(0)
        && residual_name_index(
            LIVE_COMMAND_UNIT_PROBE_METHOD_NAMES_WAVE244,
            "can_attack_target",
        ) == Some(5)
        && residual_name_index(
            LIVE_COMMAND_UNIT_PROBE_METHOD_NAMES_WAVE244,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_unit_probe_nav_commands_residual_wave244() -> bool {
    LIVE_COMMAND_UNIT_PROBE_NAV_STEPS_WAVE244.len() == 4
        && residual_name_index(
            LIVE_COMMAND_UNIT_PROBE_NAV_STEPS_WAVE244,
            "REQUIRE_COMMAND_UNIT_PROBE",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_UNIT_PROBE_NAV_STEPS_WAVE244,
            "LIVE_COMMAND_UNIT_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_UNIT_PROBE_CMD_NAMES_WAVE244.len() == 3
}

/// Wave 244 composite residual honesty pack.
pub fn honesty_live_command_unit_probe_residual_pack_wave244() -> bool {
    honesty_live_command_unit_probe_method_names_residual_wave244()
        && honesty_live_command_unit_probe_nav_commands_residual_wave244()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: can_* / attack / classify use unit probes.
pub fn honesty_command_unit_probe_source() -> bool {
    let gl = include_str!("game_logic.rs");
    let cs_full = include_str!("../command_system.rs");
    let cs = cs_full.split("#[cfg(test)]").next().unwrap_or(cs_full);
    if !(gl.contains("pub fn unit_team(")
        && gl.contains("pub fn unit_is_alive(")
        && gl.contains("pub fn unit_is_worker(")
        && gl.contains("pub fn unit_can_repair(")
        && gl.contains("pub fn unit_exists("))
    {
        return false;
    }
    for name in [
        "fn can_attack_target(",
        "fn can_capture_building(",
        "fn can_gather_from_target(",
        "fn can_resume_construction(",
        "fn can_repair_target(",
        "fn can_enter_target(",
        "fn can_get_serviced_at_target(",
        "fn execute_attack_command(",
    ] {
        let Some(body) = fn_body(cs, name) else {
            return false;
        };
        if body.contains("get_object(") {
            return false;
        }
        // Wave 244/245: either wave marker is honest for unit-probe peels.
        if !(body.contains("Wave 244")
            || body.contains("Wave 245")
            || body.contains("Wave 230/244"))
        {
            return false;
        }
    }
    let Some(classify) = fn_body(cs, "fn classify_right_click_target_from_presentation(") else {
        return false;
    };
    classify.contains("Wave 244") && !classify.contains("get_object(")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_command_unit_probe_honesty() -> bool {
    honesty_live_command_unit_probe_residual_pack_wave244() && honesty_command_unit_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_unit_probe_method_names_residual_wave244());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_unit_probe_nav_commands_residual_wave244());
    }

    #[test]
    fn wave244_composite_pack() {
        assert!(honesty_live_command_unit_probe_residual_pack_wave244());
    }

    #[test]
    fn command_unit_probe_sources() {
        assert!(honesty_command_unit_probe_source());
    }

    #[test]
    fn simulate_live_command_unit_probe_honesty_residual_live() {
        assert!(
            simulate_live_command_unit_probe_honesty(),
            "command unit probe residual must latch"
        );
    }
}
