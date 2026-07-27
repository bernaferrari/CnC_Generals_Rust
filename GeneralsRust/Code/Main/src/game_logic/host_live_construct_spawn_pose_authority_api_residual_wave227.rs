//! Wave 227 residual peels: construct dozer spawn remembers pose (no live
//! `get_object(builder)` dual-read); upgrade honesty uses
//! `GameLogic::object_is_alive` (Wave 584: via `presentation_or_boot_object_alive`).
//! Engine production path has zero `.get_object(` dual-reads. Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 226 hotkey selection/camera residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` construct spawn / upgrade honesty
//! - `game_logic.rs` object_is_alive / object_position
//!
//! Fail-closed:
//! - Not full C++ dozer construct pad scan parity
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Construct-spawn pose / alive-probe residual method names.
pub const LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_METHOD_NAMES_WAVE227: &[&str] = &[
    "spawned_builder_pose",
    "object_is_alive",
    "presentation_or_boot_object_alive",
    "host_object_is_alive",
    "object_position",
    "construct",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_NAV_STEPS_WAVE227: &[&str] = &[
    "REQUIRE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API",
    "REQUIRE_NO_ENGINE_GET_OBJECT",
    "LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_CMD_NAMES_WAVE227: &[&str] = &[
    "click_live_construct_spawn_pose_authority_api_ok_prepare",
    "click_live_construct_spawn_pose_authority_api_ok_live",
    "click_live_construct_spawn_pose_authority_api_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_construct_spawn_pose_authority_api_method_names_residual_wave227() -> bool {
    LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_METHOD_NAMES_WAVE227.len() == 7
        && residual_name_index(
            LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_METHOD_NAMES_WAVE227,
            "spawned_builder_pose",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_METHOD_NAMES_WAVE227,
            "object_is_alive",
        ) == Some(1)
        && residual_name_index(
            LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_METHOD_NAMES_WAVE227,
            "presentation_or_boot_object_alive",
        ) == Some(2)
        && residual_name_index(
            LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_METHOD_NAMES_WAVE227,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_construct_spawn_pose_authority_api_nav_commands_residual_wave227() -> bool {
    LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_NAV_STEPS_WAVE227.len() == 4
        && residual_name_index(
            LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_NAV_STEPS_WAVE227,
            "REQUIRE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API",
        ) == Some(0)
        && residual_name_index(
            LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_NAV_STEPS_WAVE227,
            "LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CONSTRUCT_SPAWN_POSE_AUTHORITY_API_CMD_NAMES_WAVE227.len() == 3
}

/// Wave 227 composite residual honesty pack.
pub fn honesty_live_construct_spawn_pose_authority_api_residual_pack_wave227() -> bool {
    honesty_live_construct_spawn_pose_authority_api_method_names_residual_wave227()
        && honesty_live_construct_spawn_pose_authority_api_nav_commands_residual_wave227()
}

/// Source residual: spawn pose + object_is_alive; engine has no `.get_object(`.
pub fn honesty_construct_spawn_pose_authority_api_source() -> bool {
    let gl = include_str!("game_logic.rs");
    let eng = include_str!("../cnc_game_engine.rs");
    gl.contains("pub fn object_is_alive")
        && gl.contains("pub fn object_position")
        && gl.contains("Wave 227")
        && eng.contains("spawned_builder_pose")
        && eng.contains("Wave 227")
        && (eng.contains("object_is_alive(pid)")
            || eng.contains("presentation_or_boot_object_alive(pid)"))
        && eng.contains("fn presentation_or_boot_object_alive")
        && eng.contains("fn host_object_is_alive")
        // Production engine must not dual-read via `.get_object(`.
        && !eng.contains(".get_object(")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_construct_spawn_pose_authority_api_honesty() -> bool {
    honesty_live_construct_spawn_pose_authority_api_residual_pack_wave227()
        && honesty_construct_spawn_pose_authority_api_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_construct_spawn_pose_authority_api_method_names_residual_wave227());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_construct_spawn_pose_authority_api_nav_commands_residual_wave227());
    }

    #[test]
    fn wave227_composite_pack() {
        assert!(honesty_live_construct_spawn_pose_authority_api_residual_pack_wave227());
    }

    #[test]
    fn construct_spawn_pose_sources() {
        assert!(honesty_construct_spawn_pose_authority_api_source());
    }

    #[test]
    fn simulate_live_construct_spawn_pose_authority_api_honesty_residual_live() {
        assert!(
            simulate_live_construct_spawn_pose_authority_api_honesty(),
            "construct spawn pose authority API residual must latch"
        );
    }
}
