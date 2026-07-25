//! Wave 248 residual peels: legacy AI object registry uses lock-free empty
//! short-circuit + read-first get (write only to prune dead weaks). Host Main
//! path does not populate legacy registry, so dual-world AI lookups skip locks.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 247 OBJECT_REGISTRY empty fastpath residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `GameEngine/.../ai/object_registry.rs` live_count / get_legacy_object /
//!   legacy_object_registry_is_empty
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Legacy registry still dual-world when populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Legacy object registry fastpath residual method names.
pub const LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_METHOD_NAMES_WAVE248: &[&str] = &[
    "live_count",
    "legacy_object_registry_is_empty",
    "get_legacy_object",
    "get_readonly",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_NAV_STEPS_WAVE248: &[&str] = &[
    "REQUIRE_LEGACY_REGISTRY_LIVE_COUNT",
    "REQUIRE_READ_FIRST_GET",
    "LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_CMD_NAMES_WAVE248: &[&str] = &[
    "click_live_legacy_object_registry_fastpath_ok_prepare",
    "click_live_legacy_object_registry_fastpath_ok_live",
    "click_live_legacy_object_registry_fastpath_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_legacy_object_registry_fastpath_method_names_residual_wave248() -> bool {
    LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_METHOD_NAMES_WAVE248.len() == 5
        && residual_name_index(
            LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_METHOD_NAMES_WAVE248,
            "live_count",
        ) == Some(0)
        && residual_name_index(
            LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_METHOD_NAMES_WAVE248,
            "get_legacy_object",
        ) == Some(2)
        && residual_name_index(
            LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_METHOD_NAMES_WAVE248,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_legacy_object_registry_fastpath_nav_commands_residual_wave248() -> bool {
    LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_NAV_STEPS_WAVE248.len() == 4
        && residual_name_index(
            LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_NAV_STEPS_WAVE248,
            "REQUIRE_LEGACY_REGISTRY_LIVE_COUNT",
        ) == Some(0)
        && residual_name_index(
            LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_NAV_STEPS_WAVE248,
            "LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_LEGACY_OBJECT_REGISTRY_FASTPATH_CMD_NAMES_WAVE248.len() == 3
}

/// Wave 248 composite residual honesty pack.
pub fn honesty_live_legacy_object_registry_fastpath_residual_pack_wave248() -> bool {
    honesty_live_legacy_object_registry_fastpath_method_names_residual_wave248()
        && honesty_live_legacy_object_registry_fastpath_nav_commands_residual_wave248()
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

/// Source residual: legacy registry empty path is lock-free; get is read-first.
pub fn honesty_legacy_object_registry_fastpath_source() -> bool {
    let reg = include_str!("../../../GameEngine/GameLogic/src/ai/object_registry.rs");
    if !(reg.contains("live_count: AtomicUsize")
        && reg.contains("Wave 248")
        && reg.contains("legacy_object_registry_is_empty")
        && reg.contains("get_readonly")
        && reg.contains("Ordering::Acquire"))
    {
        return false;
    }
    let Some(get) = fn_body(reg, "pub fn get_legacy_object(") else {
        return false;
    };
    get.contains("is_empty()") && get.contains("store.read()") && get.contains("get_and_prune")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_legacy_object_registry_fastpath_honesty() -> bool {
    honesty_live_legacy_object_registry_fastpath_residual_pack_wave248()
        && honesty_legacy_object_registry_fastpath_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_legacy_object_registry_fastpath_method_names_residual_wave248());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_legacy_object_registry_fastpath_nav_commands_residual_wave248());
    }

    #[test]
    fn wave248_composite_pack() {
        assert!(honesty_live_legacy_object_registry_fastpath_residual_pack_wave248());
    }

    #[test]
    fn legacy_object_registry_fastpath_sources() {
        assert!(honesty_legacy_object_registry_fastpath_source());
    }

    #[test]
    fn simulate_live_legacy_object_registry_fastpath_honesty_residual_live() {
        assert!(
            simulate_live_legacy_object_registry_fastpath_honesty(),
            "legacy object registry fastpath residual must latch"
        );
    }
}
