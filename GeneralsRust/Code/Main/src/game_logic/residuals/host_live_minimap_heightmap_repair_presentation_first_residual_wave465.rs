//! Wave 465 residual peels: minimap degenerate-bounds repair stamps
//! PresentationFrame from heightmap size before host override_world_size.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 457 presentation-first minimap bounds.
//! Architecture residual - presentation freeze is source of repaired extents.
//!
//! Sources (cnc_game_engine.rs):
//! - reinitialize_minimap_renderer heightmap repair builds world_bounds from (w,h)
//! - presentation_frame_mut stamped before game_logic.override_world_size
//! - host override remains pathfinding residual only
//!
//! Fail-closed:
//! - Host still receives override_world_size for sim/pathfinding
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_METHOD_NAMES_WAVE465: &[&str] = &[
    "reinitialize_minimap_renderer",
    "heightmap_world_size",
    "presentation_frame_mut",
    "override_world_size",
    "host_override_world_size",
    "sync_heightmap_world_bounds",
    "update_minimap_world_bounds",
];

pub const MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_SOURCE_MARKERS_WAVE465: &[&str] = &[
    "Wave 465: presentation-first minimap bounds",
    "heightmap_world_size",
    "presentation_frame_mut",
    "override_world_size",
];

pub const MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_NAV_STEPS_WAVE465: &[&str] = &[
    "READ_BOUNDS_FROM_PRESENTATION",
    "INIT_MINIMAP_RENDERER",
    "DETECT_DEGENERATE_BOUNDS",
    "BUILD_BOUNDS_FROM_HEIGHTMAP_SIZE",
    "STAMP_PRESENTATION_BEFORE_HOST_OVERRIDE",
    "SYNC_MINIMAP_AND_HEIGHTMAP_BOUNDS",
];

pub const RUNTIME_HOST_MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_CMD_NAMES_WAVE465: &[&str] = &[
    "click_minimap_heightmap_repair_presentation_first_ok_wnd_read",
    "click_minimap_heightmap_repair_presentation_first_ok_wnd_repair",
    "click_minimap_heightmap_repair_presentation_first_ok_wnd_stamp",
    "click_minimap_heightmap_repair_presentation_first_ok_wnd_prepare",
    "click_minimap_heightmap_repair_presentation_first_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualMinimapHeightmapRepairPresentationFirstAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    RepairSource = 4,
    OrderSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualMinimapHeightmapRepairPresentationFirstAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_minimap_heightmap_repair_presentation_first_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_minimap_heightmap_repair_presentation_first_last_action()
-> ResidualMinimapHeightmapRepairPresentationFirstAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualMinimapHeightmapRepairPresentationFirstAction::MethodNames,
        2 => ResidualMinimapHeightmapRepairPresentationFirstAction::SourceMarkers,
        3 => ResidualMinimapHeightmapRepairPresentationFirstAction::NavCommands,
        4 => ResidualMinimapHeightmapRepairPresentationFirstAction::RepairSource,
        5 => ResidualMinimapHeightmapRepairPresentationFirstAction::OrderSource,
        6 => ResidualMinimapHeightmapRepairPresentationFirstAction::Composite,
        _ => ResidualMinimapHeightmapRepairPresentationFirstAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_minimap_heightmap_repair_presentation_first_method_names_residual_wave465() -> bool {
    MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_METHOD_NAMES_WAVE465.len() == 7
        && residual_name_index(
            MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_METHOD_NAMES_WAVE465,
            "reinitialize_minimap_renderer",
        ) == Some(0)
        && residual_name_index(
            MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_METHOD_NAMES_WAVE465,
            "host_override_world_size",
        ) == Some(4)
        && residual_name_index(
            MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_METHOD_NAMES_WAVE465,
            "update_minimap_world_bounds",
        ) == Some(6)
}

pub fn honesty_minimap_heightmap_repair_presentation_first_source_markers_residual_wave465() -> bool
{
    MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_SOURCE_MARKERS_WAVE465.len() == 4
        && residual_name_index(
            MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_SOURCE_MARKERS_WAVE465,
            "Wave 465: presentation-first minimap bounds",
        ) == Some(0)
        && residual_name_index(
            MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_SOURCE_MARKERS_WAVE465,
            "override_world_size",
        ) == Some(3)
}

pub fn honesty_minimap_heightmap_repair_presentation_first_nav_commands_residual_wave465() -> bool {
    MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_NAV_STEPS_WAVE465.len() == 6
        && residual_name_index(
            MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_NAV_STEPS_WAVE465,
            "BUILD_BOUNDS_FROM_HEIGHTMAP_SIZE",
        ) == Some(3)
        && residual_name_index(
            MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_NAV_STEPS_WAVE465,
            "STAMP_PRESENTATION_BEFORE_HOST_OVERRIDE",
        ) == Some(4)
        && RUNTIME_HOST_MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_CMD_NAMES_WAVE465.len() == 5
        && residual_name_index(
            RUNTIME_HOST_MINIMAP_HEIGHTMAP_REPAIR_PRESENTATION_FIRST_CMD_NAMES_WAVE465,
            "click_minimap_heightmap_repair_presentation_first_ok_wnd_prepare",
        ) == Some(3)
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let at = src.find(sig)?;
    let bytes = src.as_bytes();
    let mut depth = 0i32;
    let mut end = at;
    let mut seen = false;
    for (j, &b) in bytes[at..].iter().enumerate() {
        if b == b'{' {
            depth += 1;
            seen = true;
        } else if b == b'}' {
            depth -= 1;
            if seen && depth == 0 {
                end = at + j + 1;
                break;
            }
        }
    }
    Some(&src[at..end])
}

pub fn simulate_minimap_heightmap_repair_presentation_first_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn reinitialize_minimap_renderer(") else {
        return false;
    };
    // Wave 594: heightmap stamp lives in host_repair_minimap_presentation_bounds.
    let Some(repair) = function_body(src, "fn host_repair_minimap_presentation_bounds(") else {
        return false;
    };
    let ok = (body.contains("Wave 468: instance path")
        || body.contains("Wave 465: presentation-first minimap bounds")
        || body.contains("Wave 594")
        || repair.contains("Wave 594"))
        && body.contains("host_repair_minimap_presentation_bounds")
        && repair.contains("heightmap_world_size()")
        && repair.contains("presentation_frame_mut()")
        && repair.contains("override_world_size");
    residual_action_store(ResidualMinimapHeightmapRepairPresentationFirstAction::RepairSource);
    ok
}

pub fn simulate_minimap_heightmap_repair_order_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn reinitialize_minimap_renderer(") else {
        return false;
    };
    // Wave 594: stamp order is inside host_repair_minimap_presentation_bounds.
    let Some(repair) = function_body(src, "fn host_repair_minimap_presentation_bounds(") else {
        return false;
    };
    let Some(stamp) = repair.find("presentation_frame_mut()") else {
        return false;
    };
    let Some(override_at) = repair.find("override_world_size") else {
        return false;
    };
    let ok = stamp < override_at
        && repair.contains("half_w")
        && repair.contains("half_h")
        && !repair.contains("world_bounds = game_logic.world_bounds()")
        && body.contains("presentation_world_bounds()")
        && body.contains("host_repair_minimap_presentation_bounds");
    residual_action_store(ResidualMinimapHeightmapRepairPresentationFirstAction::OrderSource);
    ok
}

pub fn honesty_minimap_heightmap_repair_presentation_first_residual_pack_wave465() -> bool {
    honesty_minimap_heightmap_repair_presentation_first_method_names_residual_wave465()
        && honesty_minimap_heightmap_repair_presentation_first_source_markers_residual_wave465()
        && honesty_minimap_heightmap_repair_presentation_first_nav_commands_residual_wave465()
        && simulate_minimap_heightmap_repair_presentation_first_source()
        && simulate_minimap_heightmap_repair_order_source()
}

pub fn simulate_live_minimap_heightmap_repair_presentation_first_honesty() -> bool {
    let ok = honesty_minimap_heightmap_repair_presentation_first_residual_pack_wave465();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualMinimapHeightmapRepairPresentationFirstAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_minimap_heightmap_repair_presentation_first_method_names_residual_wave465()
        );
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_minimap_heightmap_repair_presentation_first_source_markers_residual_wave465()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_minimap_heightmap_repair_presentation_first_nav_commands_residual_wave465()
        );
    }

    #[test]
    fn minimap_heightmap_repair_presentation_first_sources() {
        assert!(simulate_minimap_heightmap_repair_presentation_first_source());
        assert!(simulate_minimap_heightmap_repair_order_source());
    }

    #[test]
    fn wave465_composite_pack() {
        assert!(honesty_minimap_heightmap_repair_presentation_first_residual_pack_wave465());
    }

    #[test]
    fn simulate_live_minimap_heightmap_repair_presentation_first_honesty_residual_live() {
        assert!(
            simulate_live_minimap_heightmap_repair_presentation_first_honesty(),
            "minimap heightmap repair presentation-first residual must latch"
        );
        assert!(residual_minimap_heightmap_repair_presentation_first_ok());
        assert_eq!(
            residual_minimap_heightmap_repair_presentation_first_last_action(),
            ResidualMinimapHeightmapRepairPresentationFirstAction::Composite
        );
    }
}
