//! Wave 449 residual peels: contain module factory missing-owner fail-closed.
//! When the dual-world/host owner object cannot be resolved, factory helpers
//! install a no-op `MissingOwnerModule` instead of panicking.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 448 object upgrade batch dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/contain_module_overrides.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Populated dual-world still constructs real modules

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Contain module overrides fail-closed residual method names.
pub const LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_METHOD_NAMES_WAVE449: &[&str] = &[
    "MissingOwnerModule",
    "missing_owner_module",
    "active_behavior_module",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_NAV_STEPS_WAVE449: &[&str] = &[
    "REQUIRE_MISSING_OWNER_MODULE",
    "REQUIRE_FACTORY_FAIL_CLOSED",
    "LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_CMD_NAMES_WAVE449: &[&str] = &[
    "click_live_contain_module_overrides_fail_closed_ok_prepare",
    "click_live_contain_module_overrides_fail_closed_ok_live",
    "click_live_contain_module_overrides_fail_closed_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_contain_module_overrides_fail_closed_method_names_residual_wave449() -> bool {
    LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_METHOD_NAMES_WAVE449.len() == 4
        && residual_name_index(
            LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_METHOD_NAMES_WAVE449,
            "MissingOwnerModule",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_METHOD_NAMES_WAVE449,
            "active_behavior_module",
        ) == Some(2)
        && residual_name_index(
            LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_METHOD_NAMES_WAVE449,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_contain_module_overrides_fail_closed_nav_commands_residual_wave449() -> bool {
    LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_NAV_STEPS_WAVE449.len() == 4
        && residual_name_index(
            LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_NAV_STEPS_WAVE449,
            "REQUIRE_MISSING_OWNER_MODULE",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_NAV_STEPS_WAVE449,
            "LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CONTAIN_MODULE_OVERRIDES_FAIL_CLOSED_CMD_NAMES_WAVE449.len() == 3
}

/// Wave 449 composite residual honesty pack.
pub fn honesty_live_contain_module_overrides_fail_closed_residual_pack_wave449() -> bool {
    honesty_live_contain_module_overrides_fail_closed_method_names_residual_wave449()
        && honesty_live_contain_module_overrides_fail_closed_nav_commands_residual_wave449()
}

/// Source residual: contain factory missing-owner fail-closed.
pub fn honesty_contain_module_overrides_fail_closed_source() -> bool {
    let g = gamelogic::contain_module_overrides::CONTAIN_OVERRIDES_SRC;
    if !(g.contains("Wave 449")
        && g.contains("struct MissingOwnerModule")
        && g.contains("fn missing_owner_module(")
        && g.contains("installing no-op module"))
    {
        return false;
    }
    // Object-missing factory paths must not panic.
    let object_panics =
        g.matches("requires a valid object").count() + g.matches("requires owning object").count();
    let panic_object = g.contains("panic!(\"{module_name} requires a valid object\")")
        || g.contains("panic!(\"{module_name} requires owning object {object_id}\")")
        || g.contains("requires a valid object\"))")
        || g.contains("requires owning object {object_id}\"));")
        || g.contains("requires owning object {object_id}\");");
    let active_ok = g.contains("fn active_behavior_module")
        && g.contains("return missing_owner_module(module_name, engine_data)");
    object_panics == 0 && !panic_object && active_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_contain_module_overrides_fail_closed_honesty() -> bool {
    honesty_live_contain_module_overrides_fail_closed_residual_pack_wave449()
        && honesty_contain_module_overrides_fail_closed_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_contain_module_overrides_fail_closed_method_names_residual_wave449());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_contain_module_overrides_fail_closed_nav_commands_residual_wave449());
    }

    #[test]
    fn wave449_composite_pack() {
        assert!(honesty_live_contain_module_overrides_fail_closed_residual_pack_wave449());
    }

    #[test]
    fn contain_module_overrides_fail_closed_sources() {
        assert!(honesty_contain_module_overrides_fail_closed_source());
    }

    #[test]
    fn simulate_live_contain_module_overrides_fail_closed_honesty_residual_live() {
        assert!(
            simulate_live_contain_module_overrides_fail_closed_honesty(),
            "contain module overrides fail-closed residual must latch"
        );
    }
}
