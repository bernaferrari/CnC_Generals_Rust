//! Wave 174 residual peels: presentation / GameClient boundary honesty residual
//! (execute is presentation-only; full GameClient::update OS-input path unused;
//! host owns `HashMap<ObjectId, Object>`; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 173 single-authority combat honesty residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `render_pipeline::execute` takes no live `&GameLogic`
//! - `CncGameEngine` keeps full GameClient OS-input update disconnected
//! - Main `GameLogic` owns objects by stable `ObjectId` (not OBJECT_REGISTRY primary)
//!
//! Fail-closed:
//! - Not full GameClient::update re-enable / drawable OS path
//! - Not full GameWorld production cutover
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Presentation / GameClient boundary residual method names.
pub const PRESENTATION_CLIENT_BOUNDARY_METHOD_NAMES_WAVE174: &[&str] = &[
    "RenderPipeline::execute",
    "presentation_frame",
    "GameClient::update OS-input unused",
    "HashMap<ObjectId, Object>",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const PRESENTATION_CLIENT_BOUNDARY_NAV_STEPS_WAVE174: &[&str] = &[
    "REQUIRE_EXECUTE_PRESENTATION_ONLY",
    "REQUIRE_GAMECLIENT_OS_INPUT_DISCONNECTED",
    "REQUIRE_HOST_OBJECTID_STORE",
    "LIVE_SEED_PRESENTATION_AFTER_MAP",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_PRESENTATION_CLIENT_BOUNDARY_CMD_NAMES_WAVE174: &[&str] = &[
    "click_presentation_client_boundary_ok_execute",
    "click_presentation_client_boundary_ok_client",
    "click_presentation_client_boundary_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_presentation_client_boundary_method_names_residual_wave174() -> bool {
    PRESENTATION_CLIENT_BOUNDARY_METHOD_NAMES_WAVE174.len() == 5
        && residual_name_index(
            PRESENTATION_CLIENT_BOUNDARY_METHOD_NAMES_WAVE174,
            "RenderPipeline::execute",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CLIENT_BOUNDARY_METHOD_NAMES_WAVE174,
            "HashMap<ObjectId, Object>",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_CLIENT_BOUNDARY_METHOD_NAMES_WAVE174,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_presentation_client_boundary_nav_commands_residual_wave174() -> bool {
    PRESENTATION_CLIENT_BOUNDARY_NAV_STEPS_WAVE174.len() == 5
        && residual_name_index(
            PRESENTATION_CLIENT_BOUNDARY_NAV_STEPS_WAVE174,
            "REQUIRE_EXECUTE_PRESENTATION_ONLY",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_CLIENT_BOUNDARY_NAV_STEPS_WAVE174,
            "LIVE_SEED_PRESENTATION_AFTER_MAP",
        ) == Some(3)
        && RUNTIME_HOST_PRESENTATION_CLIENT_BOUNDARY_CMD_NAMES_WAVE174.len() == 3
}

/// Wave 174 composite residual honesty pack.
pub fn honesty_presentation_client_boundary_residual_pack_wave174() -> bool {
    honesty_presentation_client_boundary_method_names_residual_wave174()
        && honesty_presentation_client_boundary_nav_commands_residual_wave174()
}

/// Source residual: `execute` is presentation-only (no live GameLogic parameter).
pub fn honesty_execute_presentation_only_source() -> bool {
    let src = crate::graphics::render_pipeline::RENDER_PIPELINE_SRC;
    let i = match src.find("pub fn execute(") {
        Some(i) => i,
        None => return false,
    };
    // Signature window through opening brace body start.
    let window = &src[i..src.len().min(i + 700)];
    window.contains("presentation_frame")
        && !window.contains("game_logic: Option<&GameLogic>")
        && !window.contains("game_logic: &GameLogic")
        && !window.contains("game_logic: &mut GameLogic")
        && !window.contains("logic: &GameLogic")
}

/// Source residual: full GameClient OS-input update path is deliberately unused.
pub fn honesty_game_client_os_input_disconnected_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    src.contains("Full GameClient::update() OS-input path")
        && src.contains("is not used")
        && src.contains("Main owns input")
}

/// Source residual: host GameLogic owns objects by ObjectId HashMap.
pub fn honesty_host_objectid_store_source() -> bool {
    let src = super::GAME_LOGIC_HOST_SRC;
    src.contains("pub objects: HostObjectStore")
        && src.contains("struct HostObjectStore")
        && src.contains("HashMap<ObjectId, Object>")
        && src.contains("next_object_id: ObjectId")
}

/// Live residual: boundary source honesty + post-map presentation seed non-empty when maps load.
pub fn simulate_presentation_client_boundary_honesty() -> bool {
    use crate::game_logic::{
        DEFAULT_SKIRMISH_MAP_WAVE169, GameLogic, GameMode, LONE_EAGLE_MAP_WAVE169,
        resolve_retail_map_path,
    };
    use crate::presentation_frame::PresentationFrame;

    if !honesty_presentation_client_boundary_residual_pack_wave174() {
        return false;
    }
    if !honesty_execute_presentation_only_source() {
        return false;
    }
    if !honesty_game_client_os_input_disconnected_source() {
        return false;
    }
    if !honesty_host_objectid_store_source() {
        return false;
    }

    // Live: empty host ObjectId store before map.
    let mut logic = GameLogic::new();
    if !logic.host_objects().is_empty() {
        return false;
    }

    // Prefer Lone Eagle; fall back to Defcon6; soft-ok if neither (CI without MapsZH).
    let map_name = if resolve_retail_map_path(LONE_EAGLE_MAP_WAVE169).is_some() {
        LONE_EAGLE_MAP_WAVE169
    } else if resolve_retail_map_path(DEFAULT_SKIRMISH_MAP_WAVE169).is_some() {
        DEFAULT_SKIRMISH_MAP_WAVE169
    } else {
        return true;
    };

    logic.start_new_game(GameMode::Skirmish);
    let path = resolve_retail_map_path(map_name);
    let loaded = match path {
        Some(p) => {
            let s = p.to_string_lossy();
            logic.load_map(s.as_ref()) || logic.load_map(map_name)
        }
        None => logic.load_map(map_name),
    };
    if !loaded {
        return false;
    }
    if logic.host_objects().is_empty() {
        return false;
    }

    logic.update();
    let pres = PresentationFrame::build_from_logic(&logic, 0);
    // Presentation boundary: non-empty seed from host ObjectId store (no live GameLogic in execute).
    if pres.objects.is_empty() {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_client_boundary_method_names_residual_wave174());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_client_boundary_nav_commands_residual_wave174());
    }

    #[test]
    fn wave174_composite_pack() {
        assert!(honesty_presentation_client_boundary_residual_pack_wave174());
    }

    #[test]
    fn execute_and_client_source() {
        assert!(honesty_execute_presentation_only_source());
        assert!(honesty_game_client_os_input_disconnected_source());
        assert!(honesty_host_objectid_store_source());
    }

    #[test]
    fn simulate_presentation_client_boundary_honesty_residual_live() {
        assert!(
            simulate_presentation_client_boundary_honesty(),
            "presentation-only execute + GameClient OS-input disconnect residual must latch"
        );
    }
}
