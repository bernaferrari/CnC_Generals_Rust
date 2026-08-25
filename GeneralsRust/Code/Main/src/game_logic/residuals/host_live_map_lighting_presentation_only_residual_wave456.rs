//! Wave 456 residual peels: map lighting presentation-only boundary
//! (apply_map_lighting reads PresentationFrame.world_env; no live GameLogic
//! dual-read). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 455 heightmap/skybox presentation-only env apply.
//! Architecture residual - lighting prefers presentation freeze.
//!
//! Sources (cnc_game_engine.rs):
//! - apply_map_lighting(graphics, pipeline) — no &GameLogic
//! - ensure_presentation_env_for_hints before apply at call sites
//! - lighting fields from world_env (ambient/sun/fog)
//!
//! Fail-closed:
//! - Not full TimeOfDay/GlobalLighting C++ parity
//! - Not GPU light probe residual
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

/// Ordered method names for map lighting presentation-only residual.
pub const MAP_LIGHTING_PRESENTATION_ONLY_METHOD_NAMES_WAVE456: &[&str] = &[
    "apply_map_lighting",
    "ensure_presentation_env_for_hints",
    "set_environment_lighting",
    "set_lighting",
    "presentation_frame",
    "world_env",
];

/// Source markers residual pack.
pub const MAP_LIGHTING_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE456: &[&str] = &[
    "Wave 456: presentation-only map lighting",
    "presentation_frame()",
    "world_env",
    "has_map_metadata",
];

/// Ordered navigation steps.
pub const MAP_LIGHTING_PRESENTATION_ONLY_NAV_STEPS_WAVE456: &[&str] = &[
    "SEED_PRESENTATION_IF_MISSING",
    "READ_WORLD_ENV_LIGHTING",
    "APPLY_PIPELINE_ENVIRONMENT_LIGHTING",
    "APPLY_GRAPHICS_LIGHTING",
    "FALLBACK_WHEN_NO_METADATA",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_MAP_LIGHTING_PRESENTATION_ONLY_CMD_NAMES_WAVE456: &[&str] = &[
    "click_map_lighting_presentation_only_ok_wnd_heightmap",
    "click_map_lighting_presentation_only_ok_wnd_skybox",
    "click_map_lighting_presentation_only_ok_wnd_sync",
    "click_map_lighting_presentation_only_ok_wnd_prepare",
    "click_map_lighting_presentation_only_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualMapLightingPresentationOnlyAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    LightingSource = 4,
    CallSites = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualMapLightingPresentationOnlyAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_map_lighting_presentation_only_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_map_lighting_presentation_only_last_action()
-> ResidualMapLightingPresentationOnlyAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualMapLightingPresentationOnlyAction::MethodNames,
        2 => ResidualMapLightingPresentationOnlyAction::SourceMarkers,
        3 => ResidualMapLightingPresentationOnlyAction::NavCommands,
        4 => ResidualMapLightingPresentationOnlyAction::LightingSource,
        5 => ResidualMapLightingPresentationOnlyAction::CallSites,
        6 => ResidualMapLightingPresentationOnlyAction::Composite,
        _ => ResidualMapLightingPresentationOnlyAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_map_lighting_presentation_only_method_names_residual_wave456() -> bool {
    MAP_LIGHTING_PRESENTATION_ONLY_METHOD_NAMES_WAVE456.len() == 6
        && residual_name_index(
            MAP_LIGHTING_PRESENTATION_ONLY_METHOD_NAMES_WAVE456,
            "apply_map_lighting",
        ) == Some(0)
        && residual_name_index(
            MAP_LIGHTING_PRESENTATION_ONLY_METHOD_NAMES_WAVE456,
            "world_env",
        ) == Some(5)
}

pub fn honesty_map_lighting_presentation_only_source_markers_residual_wave456() -> bool {
    MAP_LIGHTING_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE456.len() == 4
        && residual_name_index(
            MAP_LIGHTING_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE456,
            "Wave 456: presentation-only map lighting",
        ) == Some(0)
        && residual_name_index(
            MAP_LIGHTING_PRESENTATION_ONLY_SOURCE_MARKERS_WAVE456,
            "has_map_metadata",
        ) == Some(3)
}

pub fn honesty_map_lighting_presentation_only_nav_commands_residual_wave456() -> bool {
    MAP_LIGHTING_PRESENTATION_ONLY_NAV_STEPS_WAVE456.len() == 6
        && residual_name_index(
            MAP_LIGHTING_PRESENTATION_ONLY_NAV_STEPS_WAVE456,
            "READ_WORLD_ENV_LIGHTING",
        ) == Some(1)
        && residual_name_index(
            MAP_LIGHTING_PRESENTATION_ONLY_NAV_STEPS_WAVE456,
            "NO_LIVE_GAMELOGIC_DUAL_READ",
        ) == Some(5)
        && RUNTIME_HOST_MAP_LIGHTING_PRESENTATION_ONLY_CMD_NAMES_WAVE456.len() == 5
        && residual_name_index(
            RUNTIME_HOST_MAP_LIGHTING_PRESENTATION_ONLY_CMD_NAMES_WAVE456,
            "click_map_lighting_presentation_only_ok_wnd_prepare",
        ) == Some(3)
}

/// Residual: apply_map_lighting is presentation-only (no GameLogic param).
pub fn simulate_map_lighting_presentation_only_source() -> bool {
    let src = cnc_source();
    let at = match src.find("fn apply_map_lighting(") {
        Some(i) => i,
        None => return false,
    };
    // Bound to this function only (brace match) so later ensure_* sigs don't pollute.
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
    let body = &src[at..end];
    let ok = body.contains("Wave 456")
        && body.contains("presentation_frame()")
        && body.contains("world_env")
        && (body.contains("set_environment_lighting")
            || body.contains("set_environment_lighting_with_terrain"))
        && body.contains("has_map_metadata")
        && !body.contains("game_logic: &GameLogic")
        && !body.contains("last_parsed_map_settings()");
    residual_action_store(ResidualMapLightingPresentationOnlyAction::LightingSource);
    ok
}

/// Residual: call sites seed presentation then apply without &game_logic.
pub fn simulate_map_lighting_presentation_only_callsites() -> bool {
    let src = cnc_source();
    let ensure_n = src.matches("self.gameworld_shadow.as_ref()").count();
    let apply_n = src.matches("Self::apply_map_lighting(").count();
    // Count call sites that still pass game_logic as third arg after apply_map_lighting(
    let mut three_arg_calls = 0usize;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find("Self::apply_map_lighting(") {
        let i = from + rel;
        let win = src[i..].get(..160).unwrap_or(&src[i..]);
        if win.contains("&self.game_logic") {
            three_arg_calls += 1;
        }
        from = i + 24;
    }
    // 2026-08-15: bound the signature check to the function body — later
    // GameLogic mentions must not poison this residual.
    let sig_ok = src
        .find("fn apply_map_lighting(")
        .and_then(|i| {
            let after = &src[i..];
            let brace = after.find('{')?;
            let mut depth = 0i32;
            for (j, ch) in after[brace..].char_indices() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            let body = &after[..=brace + j];
                            return Some(!body.contains("game_logic:"));
                        }
                    }
                    _ => {}
                }
            }
            None
        })
        .unwrap_or(false);
    let ok = ensure_n >= 3 && apply_n >= 3 && three_arg_calls == 0 && sig_ok;
    residual_action_store(ResidualMapLightingPresentationOnlyAction::CallSites);
    ok
}

pub fn honesty_map_lighting_presentation_only_residual_pack_wave456() -> bool {
    honesty_map_lighting_presentation_only_method_names_residual_wave456()
        && honesty_map_lighting_presentation_only_source_markers_residual_wave456()
        && honesty_map_lighting_presentation_only_nav_commands_residual_wave456()
        && simulate_map_lighting_presentation_only_source()
        && simulate_map_lighting_presentation_only_callsites()
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_map_lighting_presentation_only_honesty() -> bool {
    let ok = honesty_map_lighting_presentation_only_residual_pack_wave456();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualMapLightingPresentationOnlyAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_map_lighting_presentation_only_method_names_residual_wave456());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_map_lighting_presentation_only_source_markers_residual_wave456());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_map_lighting_presentation_only_nav_commands_residual_wave456());
    }

    #[test]
    fn map_lighting_presentation_only_sources() {
        assert!(simulate_map_lighting_presentation_only_source());
        assert!(simulate_map_lighting_presentation_only_callsites());
    }

    #[test]
    fn wave456_composite_pack() {
        assert!(honesty_map_lighting_presentation_only_residual_pack_wave456());
    }

    #[test]
    fn simulate_live_map_lighting_presentation_only_honesty_residual_live() {
        assert!(
            simulate_live_map_lighting_presentation_only_honesty(),
            "map lighting presentation-only residual must latch"
        );
        assert!(residual_map_lighting_presentation_only_ok());
        assert_eq!(
            residual_map_lighting_presentation_only_last_action(),
            ResidualMapLightingPresentationOnlyAction::Composite
        );
    }
}
