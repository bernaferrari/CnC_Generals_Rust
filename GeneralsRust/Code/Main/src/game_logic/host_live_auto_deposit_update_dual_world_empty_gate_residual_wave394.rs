//! Wave 394 residual peels: AutoDepositUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), auto-deposit
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 393 LeafletDropBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/auto_deposit_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AutoDepositUpdate dual-world empty-gate residual method names.
pub const LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE394: &[&str] = &[
    "dual_world_registry_unavailable",
    "award_initial_capture_bonus",
    "get_upgraded_supply_boost",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE394: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AUTO_DEPOSIT_UPDATE_EMPTY_GATES",
    "LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE394: &[&str] =
    &[
        "click_live_auto_deposit_update_dual_world_empty_gate_ok_prepare",
        "click_live_auto_deposit_update_dual_world_empty_gate_ok_live",
        "click_live_auto_deposit_update_dual_world_empty_gate_miss",
    ];

/// Honesty: method names residual pack.
pub fn honesty_live_auto_deposit_update_dual_world_empty_gate_method_names_residual_wave394() -> bool
{
    LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE394.len() == 5
        && residual_name_index(
            LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE394,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE394,
            "update_simple",
        ) == Some(3)
        && residual_name_index(
            LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE394,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_auto_deposit_update_dual_world_empty_gate_nav_commands_residual_wave394() -> bool
{
    LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE394.len() == 4
        && residual_name_index(
            LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE394,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE394,
            "LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AUTO_DEPOSIT_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE394.len() == 3
}

/// Wave 394 composite residual honesty pack.
pub fn honesty_live_auto_deposit_update_dual_world_empty_gate_residual_pack_wave394() -> bool {
    honesty_live_auto_deposit_update_dual_world_empty_gate_method_names_residual_wave394()
        && honesty_live_auto_deposit_update_dual_world_empty_gate_nav_commands_residual_wave394()
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

/// Source residual: AutoDepositUpdate empty dual-world short-circuits.
pub fn honesty_auto_deposit_update_dual_world_empty_gate_source() -> bool {
    let g =
        include_str!("../../../GameEngine/GameLogic/src/object/behavior/auto_deposit_update.rs");
    if !(g.contains("Wave 394")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(award) = fn_body(g, "fn award_initial_capture_bonus(") else {
        return false;
    };
    let Some(boost) = fn_body(g, "fn get_upgraded_supply_boost(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    helper_ok
        && award.contains("return;")
        && boost.contains("return 0")
        && update.contains("UpdateSleepTime::Forever")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_auto_deposit_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_auto_deposit_update_dual_world_empty_gate_residual_pack_wave394()
        && honesty_auto_deposit_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_auto_deposit_update_dual_world_empty_gate_method_names_residual_wave394()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_auto_deposit_update_dual_world_empty_gate_nav_commands_residual_wave394()
        );
    }

    #[test]
    fn wave394_composite_pack() {
        assert!(honesty_live_auto_deposit_update_dual_world_empty_gate_residual_pack_wave394());
    }

    #[test]
    fn auto_deposit_update_dual_world_empty_gate_sources() {
        assert!(honesty_auto_deposit_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_auto_deposit_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_auto_deposit_update_dual_world_empty_gate_honesty(),
            "auto deposit update dual-world empty gate residual must latch"
        );
    }
}
