//! Wave 420 residual peels: ExperienceTracker dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), owner-template
//! experience helpers fail-closed without dual-world factory walks.
//! Local XP mutators stay ungated (they work without registry when sink is unset).
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 419 WeaponSet dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/experience/tracker.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// ExperienceTracker dual-world empty-gate residual method names.
pub const LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE420: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_owner_template_experience_required",
    "get_owner_template_experience_value",
    "owner_is_trainable",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE420: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_EXPERIENCE_TRACKER_EMPTY_GATES",
    "LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE420: &[&str] = &[
    "click_live_experience_tracker_dual_world_empty_gate_ok_prepare",
    "click_live_experience_tracker_dual_world_empty_gate_ok_live",
    "click_live_experience_tracker_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_experience_tracker_dual_world_empty_gate_method_names_residual_wave420() -> bool
{
    LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE420.len() == 5
        && residual_name_index(
            LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE420,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE420,
            "owner_is_trainable",
        ) == Some(3)
        && residual_name_index(
            LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE420,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_experience_tracker_dual_world_empty_gate_nav_commands_residual_wave420() -> bool
{
    LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE420.len() == 4
        && residual_name_index(
            LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE420,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE420,
            "LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_EXPERIENCE_TRACKER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE420.len() == 3
}

/// Wave 420 composite residual honesty pack.
pub fn honesty_live_experience_tracker_dual_world_empty_gate_residual_pack_wave420() -> bool {
    honesty_live_experience_tracker_dual_world_empty_gate_method_names_residual_wave420()
        && honesty_live_experience_tracker_dual_world_empty_gate_nav_commands_residual_wave420()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(name) {
        let i = search_from + rel;
        let Some(b) = src[i..].find('{') else {
            search_from = i + name.len();
            continue;
        };
        let brace = i + b;
        let mut depth = 0usize;
        for (off, ch) in src[brace..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let body = &src[i..brace + off + 1];
                        if body.contains("dual_world_registry_unavailable") {
                            return Some(body);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        search_from = i + name.len();
    }
    None
}

/// Source residual: ExperienceTracker empty dual-world short-circuits.
pub fn honesty_experience_tracker_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/experience/tracker.rs");
    if !(g.contains("Wave 420")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(req) = fn_body(g, "fn get_owner_template_experience_required(") else {
        return false;
    };
    let Some(val) = fn_body(g, "fn get_owner_template_experience_value(") else {
        return false;
    };
    let Some(train) = fn_body(g, "fn owner_is_trainable(") else {
        return false;
    };
    // Local mutators must remain ungated.
    let add_ungated = !g[g.find("fn add_experience_points(").unwrap_or(0)..]
        .get(..350)
        .unwrap_or("")
        .contains("dual_world_registry_unavailable");
    helper_ok
        && req.contains("return None")
        && val.contains("return None")
        && train.contains("return None")
        && add_ungated
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_experience_tracker_dual_world_empty_gate_honesty() -> bool {
    honesty_live_experience_tracker_dual_world_empty_gate_residual_pack_wave420()
        && honesty_experience_tracker_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_experience_tracker_dual_world_empty_gate_method_names_residual_wave420()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_experience_tracker_dual_world_empty_gate_nav_commands_residual_wave420()
        );
    }

    #[test]
    fn wave420_composite_pack() {
        assert!(honesty_live_experience_tracker_dual_world_empty_gate_residual_pack_wave420());
    }

    #[test]
    fn experience_tracker_dual_world_empty_gate_sources() {
        assert!(honesty_experience_tracker_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_experience_tracker_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_experience_tracker_dual_world_empty_gate_honesty(),
            "experience tracker dual-world empty gate residual must latch"
        );
    }
}
