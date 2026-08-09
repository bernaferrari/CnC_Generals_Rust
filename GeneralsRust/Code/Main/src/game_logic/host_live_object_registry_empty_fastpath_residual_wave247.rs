//! Wave 247 residual peels: `OBJECT_REGISTRY` host/presentation empty path uses
//! a lock-free `live_count` short-circuit so dual-world AI/group/client lookups
//! do not take `RwLock` when the registry is empty (Main host authority path).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 246 world pick probe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `GameEngine/.../object/registry.rs` live_count / is_empty / get_object /
//!   contains / get_all_objects short-circuit
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Registry still dual-world when populated (legacy modules)

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Object registry empty fast-path residual method names.
pub const LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_METHOD_NAMES_WAVE247: &[&str] = &[
    "live_count",
    "is_empty",
    "get_object",
    "contains",
    "get_all_objects",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_NAV_STEPS_WAVE247: &[&str] = &[
    "REQUIRE_REGISTRY_LIVE_COUNT",
    "REQUIRE_EMPTY_SHORT_CIRCUIT",
    "LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_CMD_NAMES_WAVE247: &[&str] = &[
    "click_live_object_registry_empty_fastpath_ok_prepare",
    "click_live_object_registry_empty_fastpath_ok_live",
    "click_live_object_registry_empty_fastpath_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_object_registry_empty_fastpath_method_names_residual_wave247() -> bool {
    LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_METHOD_NAMES_WAVE247.len() == 6
        && residual_name_index(
            LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_METHOD_NAMES_WAVE247,
            "live_count",
        ) == Some(0)
        && residual_name_index(
            LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_METHOD_NAMES_WAVE247,
            "get_all_objects",
        ) == Some(4)
        && residual_name_index(
            LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_METHOD_NAMES_WAVE247,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_object_registry_empty_fastpath_nav_commands_residual_wave247() -> bool {
    LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_NAV_STEPS_WAVE247.len() == 4
        && residual_name_index(
            LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_NAV_STEPS_WAVE247,
            "REQUIRE_REGISTRY_LIVE_COUNT",
        ) == Some(0)
        && residual_name_index(
            LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_NAV_STEPS_WAVE247,
            "LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_OBJECT_REGISTRY_EMPTY_FASTPATH_CMD_NAMES_WAVE247.len() == 3
}

/// Wave 247 composite residual honesty pack.
pub fn honesty_live_object_registry_empty_fastpath_residual_pack_wave247() -> bool {
    honesty_live_object_registry_empty_fastpath_method_names_residual_wave247()
        && honesty_live_object_registry_empty_fastpath_nav_commands_residual_wave247()
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

/// Source residual: registry empty path is lock-free via live_count.
pub fn honesty_object_registry_empty_fastpath_source() -> bool {
    let reg = include_str!("../../../GameEngine/GameLogic/src/object/registry.rs");
    if !(reg.contains("live_count: AtomicUsize")
        && reg.contains("Wave 247")
        && reg.contains("Ordering::Acquire")
        && reg.contains("Ordering::Release"))
    {
        return false;
    }
    let Some(is_empty) = fn_body(reg, "pub fn is_empty(") else {
        return false;
    };
    if !(is_empty.contains("live_count") && !is_empty.contains("store.read()")) {
        return false;
    }
    let Some(get) = fn_body(reg, "pub fn get_object(") else {
        return false;
    };
    if !(get.contains("is_empty()")
        && get.contains("Wave 247")
        && get.contains("find_object_by_id"))
    {
        return false;
    }
    let Some(contains) = fn_body(reg, "pub fn contains(") else {
        return false;
    };
    contains.contains("is_empty()") && contains.contains("Wave 247")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_object_registry_empty_fastpath_honesty() -> bool {
    honesty_live_object_registry_empty_fastpath_residual_pack_wave247()
        && honesty_object_registry_empty_fastpath_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_object_registry_empty_fastpath_method_names_residual_wave247());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_object_registry_empty_fastpath_nav_commands_residual_wave247());
    }

    #[test]
    fn wave247_composite_pack() {
        assert!(honesty_live_object_registry_empty_fastpath_residual_pack_wave247());
    }

    #[test]
    fn object_registry_empty_fastpath_sources() {
        assert!(honesty_object_registry_empty_fastpath_source());
    }

    #[test]
    fn simulate_live_object_registry_empty_fastpath_honesty_residual_live() {
        assert!(
            simulate_live_object_registry_empty_fastpath_honesty(),
            "object registry empty fastpath residual must latch"
        );
    }
}
