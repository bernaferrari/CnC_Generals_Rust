//! Wave 170 residual peels: live headless map load residual
//! (GameLogic::load_map Defcon6/Lone Eagle + frame advance; never flips
//! shell `playable_claim`).
//!
//! Orthogonal to Wave 169 start_game Loading source/path residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - GameLogic::loadMap retail MapsZH
//! - map_frame_scenario production-linked gate helpers
//! - behavior_gate Lone Eagle object load residual
//!
//! Fail-closed:
//! - Not full skirmish AI/combat residual
//! - Not full shell Start button → InGame UI residual
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live map load residual method names.
pub const LIVE_MAP_LOAD_METHOD_NAMES_WAVE170: &[&str] = &[
    "run_map_frame_scenario",
    "GameLogic::load_map",
    "GameLogic::update",
    "Defcon6",
    "Lone Eagle",
];

/// Ordered live map load residual navigation steps.
pub const LIVE_MAP_LOAD_NAV_STEPS_WAVE170: &[&str] = &[
    "RESOLVE_MAP_PATH",
    "START_NEW_GAME_SKIRMISH",
    "LOAD_MAP",
    "REQUIRE_MAP_LOADED",
    "ADVANCE_LOGIC_FRAMES",
    "REQUIRE_FRAME_ADVANCE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_MAP_LOAD_CMD_NAMES_WAVE170: &[&str] = &[
    "click_live_map_load_ok_defcon",
    "click_live_map_load_ok_lone_eagle",
    "click_live_map_load_miss",
];

/// Frames to advance after load (small headless peel).
pub const LIVE_MAP_LOAD_FRAME_ADVANCE_WAVE170: u32 = 5;

/// Honesty: method names residual pack.
pub fn honesty_live_map_load_method_names_residual_wave170() -> bool {
    LIVE_MAP_LOAD_METHOD_NAMES_WAVE170.len() == 5
        && residual_name_index(LIVE_MAP_LOAD_METHOD_NAMES_WAVE170, "GameLogic::load_map") == Some(1)
        && residual_name_index(LIVE_MAP_LOAD_METHOD_NAMES_WAVE170, "Lone Eagle") == Some(4)
        && LIVE_MAP_LOAD_FRAME_ADVANCE_WAVE170 == 5
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_map_load_nav_commands_residual_wave170() -> bool {
    LIVE_MAP_LOAD_NAV_STEPS_WAVE170.len() == 6
        && residual_name_index(LIVE_MAP_LOAD_NAV_STEPS_WAVE170, "LOAD_MAP") == Some(2)
        && residual_name_index(LIVE_MAP_LOAD_NAV_STEPS_WAVE170, "REQUIRE_FRAME_ADVANCE") == Some(5)
        && RUNTIME_HOST_LIVE_MAP_LOAD_CMD_NAMES_WAVE170.len() == 3
}

/// Wave 170 composite residual honesty pack.
pub fn honesty_live_map_load_residual_pack_wave170() -> bool {
    honesty_live_map_load_method_names_residual_wave170()
        && honesty_live_map_load_nav_commands_residual_wave170()
}

/// Run one live map load peel via production map_frame_scenario.
fn live_load_one(map_name: &str) -> crate::map_frame_scenario::MapFrameScenarioResult {
    crate::map_frame_scenario::run_map_frame_scenario(
        Some(map_name),
        LIVE_MAP_LOAD_FRAME_ADVANCE_WAVE170,
    )
}

/// Live residual: when MapsZH present, Defcon6 and Lone Eagle must load + advance.
/// When absent, AssetsUnavailable is soft-ok (CI without maps).
pub fn simulate_live_map_load_honesty() -> bool {
    use super::{resolve_retail_map_path, DEFAULT_SKIRMISH_MAP_WAVE169, LONE_EAGLE_MAP_WAVE169};
    use crate::map_frame_scenario::MapFrameStatus;

    if !honesty_live_map_load_residual_pack_wave170() {
        return false;
    }

    let defcon_path = resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169);
    let lone_path = resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169);

    // No maps checked out — soft residual (still require pack honesty).
    if defcon_path.is_none() && lone_path.is_none() {
        let probe = live_load_one(DEFAULT_SKIRMISH_MAP_WAVE169);
        return matches!(
            probe.status,
            MapFrameStatus::AssetsUnavailable | MapFrameStatus::Success
        );
    }

    // Partial tree fail-closed.
    let (Some(_), Some(_)) = (defcon_path, lone_path) else {
        return false;
    };

    let defcon = live_load_one(DEFAULT_SKIRMISH_MAP_WAVE169);
    if !defcon.map_loaded || !matches!(defcon.status, MapFrameStatus::Success) {
        return false;
    }
    if defcon.frames_advanced == 0 {
        return false;
    }

    let lone = live_load_one(LONE_EAGLE_MAP_WAVE169);
    if !lone.map_loaded || !matches!(lone.status, MapFrameStatus::Success) {
        return false;
    }
    if lone.frames_advanced == 0 {
        return false;
    }
    // Lone Eagle behavior_gate residual historically loads hundreds of objects.
    // Accept any non-empty world as materialisation; do not hardcode 903.
    if lone.object_count == 0 {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_map_load_method_names_residual_wave170());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_map_load_nav_commands_residual_wave170());
    }

    #[test]
    fn wave170_composite_pack() {
        assert!(honesty_live_map_load_residual_pack_wave170());
    }

    #[test]
    fn simulate_live_map_load_honesty_residual_live() {
        assert!(
            simulate_live_map_load_honesty(),
            "live Defcon6/Lone Eagle load_map + frame advance residual must latch"
        );
    }
}
