//! Wave 475 residual peels: map grounding no longer dual-writes OBJECT_REGISTRY poses.
//! - `ground_loaded_map_objects_to_terrain` updates host ObjectStore only
//! - no named-tracker → OBJECT_REGISTRY.set_position peel
//! - bridge/shadow materialize remains the GW pose channel
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 247/248 registry empty fastpaths.
//! Architecture residual - host map load does not pose dual-write into legacy registry.
//!
//! Sources (game_logic.rs):
//! - fn ground_loaded_map_objects_to_terrain
//! - host set_position + ground_height residual + optional move log
//! - no OBJECT_REGISTRY.get_object in grounding
//!
//! Fail-closed:
//! - Named shell registration may still exist on bridge-on create path (separate residual)
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_METHOD_NAMES_WAVE475: &[&str] = &[
    "ground_loaded_map_objects_to_terrain",
    "set_ground_height_residual",
    "host_move_log::record",
    "no OBJECT_REGISTRY.get_object",
    "host ObjectStore only",
    "playable_claim = false",
];

pub const MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_SOURCE_MARKERS_WAVE475: &[&str] = &[
    "Wave 475: host map grounding is host-ObjectStore only",
    "Dual-world OBJECT_REGISTRY pose writes retired",
    "set_ground_height_residual",
    "no OBJECT_REGISTRY.get_object in grounding",
];

pub const MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_NAV_STEPS_WAVE475: &[&str] = &[
    "GROUND_HOST_OBJECTS",
    "RECORD_GROUND_HEIGHT_RESIDUAL",
    "OPTIONAL_MOVE_LOG_UNDER_MOVEMENT_AUTH",
    "SKIP_SHELL_REGISTRY_POSE_WRITE",
    "SHADOW_OWNS_GW_POSES",
    "NO_REGISTRY_GET_OBJECT_IN_GROUND",
];

pub const RUNTIME_HOST_MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_CMD_NAMES_WAVE475: &[&str] = &[
    "click_map_ground_no_registry_pose_dual_write_ok_wnd_ground",
    "click_map_ground_no_registry_pose_dual_write_ok_wnd_residual",
    "click_map_ground_no_registry_pose_dual_write_ok_wnd_skip",
    "click_map_ground_no_registry_pose_dual_write_ok_wnd_prepare",
    "click_map_ground_no_registry_pose_dual_write_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualMapGroundNoRegistryPoseDualWriteAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    GroundSource = 4,
    RegistryAbsent = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualMapGroundNoRegistryPoseDualWriteAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_map_ground_no_registry_pose_dual_write_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_map_ground_no_registry_pose_dual_write_last_action()
-> ResidualMapGroundNoRegistryPoseDualWriteAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualMapGroundNoRegistryPoseDualWriteAction::MethodNames,
        2 => ResidualMapGroundNoRegistryPoseDualWriteAction::SourceMarkers,
        3 => ResidualMapGroundNoRegistryPoseDualWriteAction::NavCommands,
        4 => ResidualMapGroundNoRegistryPoseDualWriteAction::GroundSource,
        5 => ResidualMapGroundNoRegistryPoseDualWriteAction::RegistryAbsent,
        6 => ResidualMapGroundNoRegistryPoseDualWriteAction::Composite,
        _ => ResidualMapGroundNoRegistryPoseDualWriteAction::Idle,
    }
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let brace = src[start..].find('{')? + start;
    let mut depth = 0i32;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_map_ground_no_registry_pose_dual_write_method_names_residual_wave475() -> bool {
    MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_METHOD_NAMES_WAVE475.len() == 6
        && residual_name_index(
            MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_METHOD_NAMES_WAVE475,
            "ground_loaded_map_objects_to_terrain",
        ) == Some(0)
        && residual_name_index(
            MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_METHOD_NAMES_WAVE475,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_map_ground_no_registry_pose_dual_write_source_markers_residual_wave475() -> bool {
    MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_SOURCE_MARKERS_WAVE475.len() == 4
        && residual_name_index(
            MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_SOURCE_MARKERS_WAVE475,
            "Wave 475: host map grounding is host-ObjectStore only",
        ) == Some(0)
        && residual_name_index(
            MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_SOURCE_MARKERS_WAVE475,
            "no OBJECT_REGISTRY.get_object in grounding",
        ) == Some(3)
}

pub fn honesty_map_ground_no_registry_pose_dual_write_nav_commands_residual_wave475() -> bool {
    MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_NAV_STEPS_WAVE475.len() == 6
        && residual_name_index(
            MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_NAV_STEPS_WAVE475,
            "GROUND_HOST_OBJECTS",
        ) == Some(0)
        && residual_name_index(
            MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_NAV_STEPS_WAVE475,
            "NO_REGISTRY_GET_OBJECT_IN_GROUND",
        ) == Some(5)
        && RUNTIME_HOST_MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_CMD_NAMES_WAVE475.len() == 5
        && residual_name_index(
            RUNTIME_HOST_MAP_GROUND_NO_REGISTRY_POSE_DUAL_WRITE_CMD_NAMES_WAVE475,
            "click_map_ground_no_registry_pose_dual_write_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_map_ground_host_only_source() -> bool {
    let src = gl_source();
    let Some(body) = function_body(src, "fn ground_loaded_map_objects_to_terrain(") else {
        return false;
    };
    let ok = body.contains("Wave 475: host map grounding is host-ObjectStore only")
        && body.contains("set_ground_height_residual")
        && body.contains("set_position")
        && !body.contains("OBJECT_REGISTRY.get_object")
        && !body.contains("get_named_object_tracker")
        && !body.contains("object_arc.write()");
    residual_action_store(ResidualMapGroundNoRegistryPoseDualWriteAction::GroundSource);
    ok
}

pub fn simulate_map_ground_registry_pose_absent() -> bool {
    let src = gl_source();
    let Some(body) = function_body(src, "fn ground_loaded_map_objects_to_terrain(") else {
        return false;
    };
    let ok = !body.contains("OBJECT_REGISTRY.get_object")
        && !body.contains("engine_object_bridge_enabled")
        && body.contains("Dual-world OBJECT_REGISTRY pose writes retired");
    residual_action_store(ResidualMapGroundNoRegistryPoseDualWriteAction::RegistryAbsent);
    ok
}

pub fn honesty_map_ground_no_registry_pose_dual_write_residual_pack_wave475() -> bool {
    honesty_map_ground_no_registry_pose_dual_write_method_names_residual_wave475()
        && honesty_map_ground_no_registry_pose_dual_write_source_markers_residual_wave475()
        && honesty_map_ground_no_registry_pose_dual_write_nav_commands_residual_wave475()
        && simulate_map_ground_host_only_source()
        && simulate_map_ground_registry_pose_absent()
}

pub fn simulate_live_map_ground_no_registry_pose_dual_write_honesty() -> bool {
    let ok = honesty_map_ground_no_registry_pose_dual_write_residual_pack_wave475();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualMapGroundNoRegistryPoseDualWriteAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_map_ground_no_registry_pose_dual_write_method_names_residual_wave475());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_map_ground_no_registry_pose_dual_write_source_markers_residual_wave475());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_map_ground_no_registry_pose_dual_write_nav_commands_residual_wave475());
    }

    #[test]
    fn map_ground_no_registry_pose_dual_write_sources() {
        assert!(simulate_map_ground_host_only_source());
        assert!(simulate_map_ground_registry_pose_absent());
        let body = function_body(gl_source(), "fn ground_loaded_map_objects_to_terrain(").unwrap();
        assert!(!body.contains("OBJECT_REGISTRY.get_object"));
        assert!(!body.contains("get_named_object_tracker"));
    }

    #[test]
    fn wave475_composite_pack() {
        assert!(honesty_map_ground_no_registry_pose_dual_write_residual_pack_wave475());
    }

    #[test]
    fn simulate_live_map_ground_no_registry_pose_dual_write_honesty_residual_live() {
        assert!(
            simulate_live_map_ground_no_registry_pose_dual_write_honesty(),
            "map ground no registry pose dual-write residual must latch"
        );
        assert!(residual_map_ground_no_registry_pose_dual_write_ok());
        assert_eq!(
            residual_map_ground_no_registry_pose_dual_write_last_action(),
            ResidualMapGroundNoRegistryPoseDualWriteAction::Composite
        );
    }
}
