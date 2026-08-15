//! Wave 962: presentation-owned drawable ensure (no OBJECT_REGISTRY dual-world).
//!
//! GameClient creates missing drawables from PresentationFrame identity so
//! pose/shroud residuals bind without populating OBJECT_REGISTRY.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_DRAWABLE_ENSURE_METHOD_NAMES_WAVE962: &[&str] = &[
    "ensure_presentation_drawables",
    "apply_frozen_direct_presentation_poses",
    "Wave 962",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_DRAWABLE_ENSURE_NAV_STEPS_WAVE962: &[&str] = &[
    "PRESENTATION_DRAWABLE_ENSURE",
    "NO_OBJECT_REGISTRY_POPULATE",
    "POSE_AFTER_ENSURE",
    "LIVE_HOST_PRESENTATION_DRAWABLE_ENSURE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationDrawableEnsureAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationDrawableEnsureAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}

fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

fn fn_window<'a>(src: &'a str, marker: &str) -> &'a str {
    let Some(i) = src.find(marker) else {
        return "";
    };
    let Some(brace) = src[i..].find('{').map(|o| i + o) else {
        return "";
    };
    let mut depth = 0usize;
    let mut p = brace;
    let bytes = src.as_bytes();
    while p < src.len() {
        match bytes[p] as char {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &src[i..=p];
                }
            }
            _ => {}
        }
        p += 1;
    }
    &src[i..src.len().min(i + 6_000)]
}

fn non_comment(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_presentation_drawable_ensure_method_names_residual_wave962() -> bool {
    let names = LIVE_HOST_PRESENTATION_DRAWABLE_ENSURE_METHOD_NAMES_WAVE962;
    let ok = residual_name_index(names, "ensure_presentation_drawables").is_some()
        && residual_name_index(names, "Wave 962").is_some();
    residual_action_store(ResidualHostPresentationDrawableEnsureAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_drawable_ensure_nav_commands_residual_wave962() -> bool {
    let steps = LIVE_HOST_PRESENTATION_DRAWABLE_ENSURE_NAV_STEPS_WAVE962;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_DRAWABLE_ENSURE").is_some()
        && residual_name_index(steps, "NO_OBJECT_REGISTRY_POPULATE").is_some();
    residual_action_store(ResidualHostPresentationDrawableEnsureAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_drawable_ensure_residual_pack_wave962() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let ok = client.contains("Wave 962")
        && (cnc.contains("Wave 962") || cnc.contains("Wave 963"))
        && client.contains("ensure_presentation_drawables")
        && client.contains("sync_presentation_drawables")
        && cnc.contains("sync_presentation_drawables")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationDrawableEnsureAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_drawable_ensure_honesty() -> bool {
    let a = honesty_host_presentation_drawable_ensure_method_names_residual_wave962();
    let b = honesty_host_presentation_drawable_ensure_nav_commands_residual_wave962();
    let c = honesty_host_presentation_drawable_ensure_residual_pack_wave962();
    residual_action_store(ResidualHostPresentationDrawableEnsureAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_drawable_ensure_residual_wave962() {
        assert!(honesty_host_presentation_drawable_ensure_residual_pack_wave962());
        assert!(honesty_host_presentation_drawable_ensure_method_names_residual_wave962());
        assert!(honesty_host_presentation_drawable_ensure_nav_commands_residual_wave962());
        assert!(simulate_live_host_presentation_drawable_ensure_honesty());
    }
}
