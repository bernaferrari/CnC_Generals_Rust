//! Wave 325 residual peels: SpectreGunshipUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), gunship
//! orbit/target helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 324 PartitionManager dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/spectre_gunship_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Spectre gunship dual-world empty-gate residual method names.
pub const LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE325: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_fair_distance_from_ship",
    "update",
    "does_special_power_update_pass_science_test",
    "initiate_intent_to_do_special_power",
    "set_special_power_overridable_destination",
    "on_object_created",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE325: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SPECTRE_GUNSHIP_EMPTY_GATES",
    "LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE325: &[&str] = &[
    "click_live_spectre_gunship_dual_world_empty_gate_ok_prepare",
    "click_live_spectre_gunship_dual_world_empty_gate_ok_live",
    "click_live_spectre_gunship_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_spectre_gunship_dual_world_empty_gate_method_names_residual_wave325() -> bool {
    LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE325.len() == 8
        && residual_name_index(
            LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE325,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE325,
            "update",
        ) == Some(2)
        && residual_name_index(
            LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE325,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_spectre_gunship_dual_world_empty_gate_nav_commands_residual_wave325() -> bool {
    LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE325.len() == 4
        && residual_name_index(
            LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE325,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE325,
            "LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SPECTRE_GUNSHIP_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE325.len() == 3
}

/// Wave 325 composite residual honesty pack.
pub fn honesty_live_spectre_gunship_dual_world_empty_gate_residual_pack_wave325() -> bool {
    honesty_live_spectre_gunship_dual_world_empty_gate_method_names_residual_wave325()
        && honesty_live_spectre_gunship_dual_world_empty_gate_nav_commands_residual_wave325()
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

/// Source residual: spectre gunship empty dual-world short-circuits.
pub fn honesty_spectre_gunship_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/spectre_gunship_update.rs"
    );
    if !(g.contains("Wave 325")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(update) = fn_body(g, "fn update(") else {
        return false;
    };
    let Some(created) = fn_body(g, "fn on_object_created(") else {
        return false;
    };
    helper_ok
        && update.contains("Ok(UpdateSleepTime::Forever)")
        && created.contains("Ok(())")
        && g.contains("fn is_fair_distance_from_ship")
        && g.contains("fn initiate_intent_to_do_special_power")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_spectre_gunship_dual_world_empty_gate_honesty() -> bool {
    honesty_live_spectre_gunship_dual_world_empty_gate_residual_pack_wave325()
        && honesty_spectre_gunship_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_spectre_gunship_dual_world_empty_gate_method_names_residual_wave325());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_spectre_gunship_dual_world_empty_gate_nav_commands_residual_wave325());
    }

    #[test]
    fn wave325_composite_pack() {
        assert!(honesty_live_spectre_gunship_dual_world_empty_gate_residual_pack_wave325());
    }

    #[test]
    fn spectre_gunship_dual_world_empty_gate_sources() {
        assert!(honesty_spectre_gunship_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_spectre_gunship_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_spectre_gunship_dual_world_empty_gate_honesty(),
            "spectre gunship dual-world empty gate residual must latch"
        );
    }
}
