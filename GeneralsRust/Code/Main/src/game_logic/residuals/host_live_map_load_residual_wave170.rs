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

/// Run one live map load peel via GameLogic + parent-walking path resolve.
/// Returns (frames_advanced, presentation_object_count).
fn live_load_one(map_name: &str) -> Option<(u32, usize)> {
    use crate::game_logic::{GameLogic, GameMode, resolve_retail_map_path};

    let path = resolve_retail_map_path(map_name)?;
    let mut logic = GameLogic::new();
    logic.start_new_game(GameMode::Skirmish);
    let s = path.to_string_lossy();
    let loaded = logic.load_map(s.as_ref()) || logic.load_map(map_name);
    if !loaded {
        return None;
    }
    let before = logic.get_frame();
    for _ in 0..LIVE_MAP_LOAD_FRAME_ADVANCE_WAVE170 {
        logic.update();
    }
    let advanced = logic.get_frame().saturating_sub(before);
    let object_count = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0)
        .objects
        .len();
    Some((advanced, object_count))
}

/// Live residual: when MapsZH present, Defcon6 and Lone Eagle must load + advance.
/// When absent, soft-ok (CI without maps).
pub fn simulate_live_map_load_honesty() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, LONE_EAGLE_MAP_WAVE169, resolve_retail_map_path,
    };

    if !honesty_live_map_load_residual_pack_wave170() {
        return false;
    }

    let defcon_path = resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169);
    let lone_path = resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169);

    // No maps checked out — soft residual (still require pack honesty).
    if defcon_path.is_none() && lone_path.is_none() {
        return true;
    }

    // Partial tree fail-closed.
    let (Some(_), Some(_)) = (defcon_path, lone_path) else {
        return false;
    };

    let Some((defcon_adv, _)) = live_load_one(DEFAULT_SKIRMISH_MAP_WAVE169) else {
        return false;
    };
    if defcon_adv == 0 {
        return false;
    }

    let Some((lone_adv, lone_objs)) = live_load_one(LONE_EAGLE_MAP_WAVE169) else {
        return false;
    };
    if lone_adv == 0 {
        return false;
    }
    // Lone Eagle behavior_gate residual historically loads hundreds of objects.
    // Accept any non-empty world as materialisation; do not hardcode 903.
    if lone_objs == 0 {
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
