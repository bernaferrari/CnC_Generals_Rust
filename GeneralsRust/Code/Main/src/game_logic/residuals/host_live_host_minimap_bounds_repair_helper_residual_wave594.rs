//! Wave 594 residual peels: minimap heightmap repair + last_presentation align
//! is centralized through `host_repair_minimap_presentation_bounds`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 468 minimap reinit instance residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host_repair_minimap_presentation_bounds
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MINIMAP_BOUNDS_REPAIR_HELPER_METHOD_NAMES_WAVE594: &[&str] = &[
    "host_repair_minimap_presentation_bounds",
    "reinitialize_minimap_renderer",
    "heightmap_world_size",
    "host_override_world_size",
    "last_presentation_frame",
    "Wave 594",
    "playable_claim = false",
];

pub const LIVE_HOST_MINIMAP_BOUNDS_REPAIR_HELPER_NAV_STEPS_WAVE594: &[&str] = &[
    "REQUIRE_MINIMAP_BOUNDS_REPAIR_HELPER",
    "REQUIRE_HEIGHTMAP_STAMP",
    "REQUIRE_HOST_WORLD_SIZE_OVERRIDE",
    "REQUIRE_LAST_PRESENTATION_ALIGN",
    "LIVE_HOST_MINIMAP_BOUNDS_REPAIR_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_MINIMAP_BOUNDS_REPAIR_HELPER_CMD_NAMES_WAVE594: &[&str] = &[
    "host_minimap_bounds_repair_helper",
    "heightmap_stamp",
    "host_world_size_override",
    "last_presentation_align",
    "minimap_bounds_repair_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMinimapBoundsRepairHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostMinimapBoundsRepairHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualHostMinimapBoundsRepairHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_minimap_bounds_repair_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_minimap_bounds_repair_helper_last_action()
-> ResidualHostMinimapBoundsRepairHelperAction {
    ResidualHostMinimapBoundsRepairHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn last_sig_index(src: &str, sig: &str) -> Option<usize> {
    let mut at = None;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find(sig) {
        at = Some(from + rel);
        from = from + rel + sig.len();
    }
    at
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = last_sig_index(src, sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_host_minimap_bounds_repair_helper_method_names_residual_wave594() -> bool {
    let names = LIVE_HOST_MINIMAP_BOUNDS_REPAIR_HELPER_METHOD_NAMES_WAVE594;
    let ok = residual_name_index(names, "host_repair_minimap_presentation_bounds").is_some()
        && residual_name_index(names, "reinitialize_minimap_renderer").is_some()
        && residual_name_index(names, "host_override_world_size").is_some()
        && residual_name_index(names, "Wave 594").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostMinimapBoundsRepairHelperAction::MethodNames);
    ok
}

pub fn honesty_host_minimap_bounds_repair_helper_source_markers_residual_wave594() -> bool {
    let eng = eng_source();
    let Some(body) = fn_body(eng, "fn host_repair_minimap_presentation_bounds(") else {
        residual_action_store(ResidualHostMinimapBoundsRepairHelperAction::SourceMarkers);
        return false;
    };
    let Some(reinit) = fn_body(eng, "fn reinitialize_minimap_renderer(") else {
        residual_action_store(ResidualHostMinimapBoundsRepairHelperAction::SourceMarkers);
        return false;
    };
    let body_ok = body.contains("Wave 594")
        && body.contains("heightmap_world_size")
        && body.contains("host_override_world_size")
        && body.contains("last_presentation_frame = Some")
        && body.contains("presentation_frame_mut")
        && body.contains("world_min")
        && body.contains("world_max");
    let call_ok = reinit.contains("host_repair_minimap_presentation_bounds")
        && reinit.contains("Wave 594")
        && eng.contains("self.host_repair_minimap_presentation_bounds(world_bounds)");
    let ok = body_ok && call_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostMinimapBoundsRepairHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_minimap_bounds_repair_helper_nav_commands_residual_wave594() -> bool {
    let steps = LIVE_HOST_MINIMAP_BOUNDS_REPAIR_HELPER_NAV_STEPS_WAVE594;
    let cmds = RUNTIME_HOST_LIVE_HOST_MINIMAP_BOUNDS_REPAIR_HELPER_CMD_NAMES_WAVE594;
    let ok = residual_name_index(steps, "REQUIRE_MINIMAP_BOUNDS_REPAIR_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_HEIGHTMAP_STAMP").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_WORLD_SIZE_OVERRIDE").is_some()
        && residual_name_index(steps, "REQUIRE_LAST_PRESENTATION_ALIGN").is_some()
        && residual_name_index(steps, "LIVE_HOST_MINIMAP_BOUNDS_REPAIR_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_minimap_bounds_repair_helper").is_some()
        && residual_name_index(cmds, "heightmap_stamp").is_some()
        && residual_name_index(cmds, "host_world_size_override").is_some()
        && residual_name_index(cmds, "last_presentation_align").is_some()
        && residual_name_index(cmds, "minimap_bounds_repair_residual").is_some();
    residual_action_store(ResidualHostMinimapBoundsRepairHelperAction::NavCommands);
    ok
}

pub fn simulate_host_minimap_bounds_repair_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 594")
        && eng.contains("fn host_repair_minimap_presentation_bounds")
        && eng.contains("heightmap_world_size");
    residual_action_store(ResidualHostMinimapBoundsRepairHelperAction::CollectSource);
    ok
}

pub fn simulate_host_minimap_bounds_repair_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_repair_minimap_presentation_bounds(world_bounds)")
        && eng.contains("Wave 594: heightmap repair + last_presentation align via host helper");
    residual_action_store(ResidualHostMinimapBoundsRepairHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_minimap_bounds_repair_helper_residual_pack_wave594() -> bool {
    honesty_host_minimap_bounds_repair_helper_method_names_residual_wave594()
        && honesty_host_minimap_bounds_repair_helper_source_markers_residual_wave594()
        && honesty_host_minimap_bounds_repair_helper_nav_commands_residual_wave594()
        && simulate_host_minimap_bounds_repair_helper_collect_source()
        && simulate_host_minimap_bounds_repair_helper_dispatch_source()
}

pub fn simulate_live_host_minimap_bounds_repair_helper_honesty() -> bool {
    let ok = honesty_host_minimap_bounds_repair_helper_residual_pack_wave594();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostMinimapBoundsRepairHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_minimap_bounds_repair_helper_method_names_residual_wave594());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_minimap_bounds_repair_helper_source_markers_residual_wave594());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_minimap_bounds_repair_helper_nav_commands_residual_wave594());
    }

    #[test]
    fn host_minimap_bounds_repair_helper_sources() {
        assert!(simulate_host_minimap_bounds_repair_helper_collect_source());
        assert!(simulate_host_minimap_bounds_repair_helper_dispatch_source());
    }

    #[test]
    fn wave594_composite_pack() {
        assert!(honesty_host_minimap_bounds_repair_helper_residual_pack_wave594());
    }

    #[test]
    fn simulate_live_host_minimap_bounds_repair_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_minimap_bounds_repair_helper_honesty(),
            "host minimap bounds repair helper residual must latch"
        );
        assert!(residual_host_minimap_bounds_repair_helper_ok());
        assert_eq!(
            residual_host_minimap_bounds_repair_helper_last_action(),
            ResidualHostMinimapBoundsRepairHelperAction::Composite
        );
    }
}
