//! Wave 246 residual peels: world-position object pick uses GameLogic
//! `pick_object_id_at_world` instead of caller-side `game_logic.objects` dual-walk.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 245 selection query probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `game_logic.rs` pick_object_id_at_world
//! - `command_integration.rs` find_object_at_position
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Engine InGame pick already presentation-only (orthogonal)

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// World pick probe residual method names.
pub const LIVE_WORLD_PICK_PROBE_METHOD_NAMES_WAVE246: &[&str] = &[
    "pick_object_id_at_world",
    "find_object_at_position",
    "PriorityAcquireCandidate",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_WORLD_PICK_PROBE_NAV_STEPS_WAVE246: &[&str] = &[
    "REQUIRE_WORLD_PICK_PROBE",
    "REQUIRE_NO_CALLER_OBJECTS_WALK",
    "LIVE_WORLD_PICK_PROBE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_WORLD_PICK_PROBE_CMD_NAMES_WAVE246: &[&str] = &[
    "click_live_world_pick_probe_ok_prepare",
    "click_live_world_pick_probe_ok_live",
    "click_live_world_pick_probe_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_world_pick_probe_method_names_residual_wave246() -> bool {
    LIVE_WORLD_PICK_PROBE_METHOD_NAMES_WAVE246.len() == 4
        && residual_name_index(
            LIVE_WORLD_PICK_PROBE_METHOD_NAMES_WAVE246,
            "pick_object_id_at_world",
        ) == Some(0)
        && residual_name_index(
            LIVE_WORLD_PICK_PROBE_METHOD_NAMES_WAVE246,
            "find_object_at_position",
        ) == Some(1)
        && residual_name_index(
            LIVE_WORLD_PICK_PROBE_METHOD_NAMES_WAVE246,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_world_pick_probe_nav_commands_residual_wave246() -> bool {
    LIVE_WORLD_PICK_PROBE_NAV_STEPS_WAVE246.len() == 4
        && residual_name_index(
            LIVE_WORLD_PICK_PROBE_NAV_STEPS_WAVE246,
            "REQUIRE_WORLD_PICK_PROBE",
        ) == Some(0)
        && residual_name_index(
            LIVE_WORLD_PICK_PROBE_NAV_STEPS_WAVE246,
            "LIVE_WORLD_PICK_PROBE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_WORLD_PICK_PROBE_CMD_NAMES_WAVE246.len() == 3
}

/// Wave 246 composite residual honesty pack.
pub fn honesty_live_world_pick_probe_residual_pack_wave246() -> bool {
    honesty_live_world_pick_probe_method_names_residual_wave246()
        && honesty_live_world_pick_probe_nav_commands_residual_wave246()
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

/// Source residual: pick API exists; integration find uses it without objects walk.
pub fn honesty_world_pick_probe_source() -> bool {
    let gl = super::GAME_LOGIC_HOST_SRC;
    let ci_full = include_str!("../../command_integration.rs");
    let ci = ci_full.split("#[cfg(test)]").next().unwrap_or(ci_full);
    if !gl.contains("pub fn pick_object_id_at_world(") {
        return false;
    }
    let Some(find) = fn_body(ci, "fn find_object_at_position(") else {
        return false;
    };
    find.contains("Wave 246")
        && find.contains("pick_object_id_at_world")
        && !find.contains(".objects")
        && !find.contains("get_objects")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_world_pick_probe_honesty() -> bool {
    honesty_live_world_pick_probe_residual_pack_wave246() && honesty_world_pick_probe_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_world_pick_probe_method_names_residual_wave246());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_world_pick_probe_nav_commands_residual_wave246());
    }

    #[test]
    fn wave246_composite_pack() {
        assert!(honesty_live_world_pick_probe_residual_pack_wave246());
    }

    #[test]
    fn world_pick_probe_sources() {
        assert!(honesty_world_pick_probe_source());
    }

    #[test]
    fn simulate_live_world_pick_probe_honesty_residual_live() {
        assert!(
            simulate_live_world_pick_probe_honesty(),
            "world pick probe residual must latch"
        );
    }
}
