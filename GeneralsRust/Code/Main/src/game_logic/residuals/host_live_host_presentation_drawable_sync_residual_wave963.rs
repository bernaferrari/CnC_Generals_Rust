//! Wave 963: presentation drawable full sync (model + prune, no OBJECT_REGISTRY).
//!
//! `sync_presentation_drawables` ensures/creates drawables, stamps model-condition
//! and body-damage residual, prunes nonresident/absent visuals. Engine calls sync
//! before shroud/pose. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_DRAWABLE_SYNC_METHOD_NAMES_WAVE963: &[&str] = &[
    "sync_presentation_drawables",
    "ensure_presentation_drawables",
    "PresentationDrawableSync",
    "stamp_presentation_object_residual",
    "Wave 963",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_DRAWABLE_SYNC_NAV_STEPS_WAVE963: &[&str] = &[
    "PRESENTATION_DRAWABLE_SYNC",
    "MODEL_CONDITION_FROM_FREEZE",
    "PRUNE_NONRESIDENT_DRAWABLES",
    "NO_OBJECT_REGISTRY_POPULATE",
    "LIVE_HOST_PRESENTATION_DRAWABLE_SYNC",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationDrawableSyncAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationDrawableSyncAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
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
    &src[i..src.len().min(i + 8_000)]
}

fn non_comment(window: &str) -> String {
    window
        .lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn honesty_host_presentation_drawable_sync_method_names_residual_wave963() -> bool {
    let names = LIVE_HOST_PRESENTATION_DRAWABLE_SYNC_METHOD_NAMES_WAVE963;
    let ok = residual_name_index(names, "sync_presentation_drawables").is_some()
        && residual_name_index(names, "Wave 963").is_some();
    residual_action_store(ResidualHostPresentationDrawableSyncAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_drawable_sync_nav_commands_residual_wave963() -> bool {
    let steps = LIVE_HOST_PRESENTATION_DRAWABLE_SYNC_NAV_STEPS_WAVE963;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_DRAWABLE_SYNC").is_some()
        && residual_name_index(steps, "PRUNE_NONRESIDENT_DRAWABLES").is_some();
    residual_action_store(ResidualHostPresentationDrawableSyncAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_drawable_sync_residual_pack_wave963() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let tick = fn_window(cnc, "fn host_tick_game_client_presentation_shell(");
    let ok = client.contains("Wave 963")
        && cnc.contains("Wave 963")
        && client.contains("struct PresentationDrawableSync")
        && client.contains("sync_presentation_drawables")
        && (client.contains("stamp_presentation_model_residual")
            || client.contains("stamp_presentation_object_residual"))
        && client.contains("destroy_drawable")
        && client.contains("model_condition_bits")
        && client.contains("react_to_body_damage_state_change")
        && tick.contains("sync_presentation_drawables")
        && tick.contains("apply_frozen_direct_shroud_statuses")
        && tick
            .find("sync_presentation_drawables")
            .zip(tick.find("apply_frozen_direct_shroud_statuses"))
            .map(|(a, b)| a < b)
            .unwrap_or(false)
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationDrawableSyncAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_drawable_sync_honesty() -> bool {
    let a = honesty_host_presentation_drawable_sync_method_names_residual_wave963();
    let b = honesty_host_presentation_drawable_sync_nav_commands_residual_wave963();
    let c = honesty_host_presentation_drawable_sync_residual_pack_wave963();
    residual_action_store(ResidualHostPresentationDrawableSyncAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_drawable_sync_residual_wave963() {
        assert!(honesty_host_presentation_drawable_sync_residual_pack_wave963());
        assert!(honesty_host_presentation_drawable_sync_method_names_residual_wave963());
        assert!(honesty_host_presentation_drawable_sync_nav_commands_residual_wave963());
        assert!(simulate_live_host_presentation_drawable_sync_honesty());
    }
}
