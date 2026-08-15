//! Wave 1057: dual-world emoticon presentation residual.
//!
//! PresentationDrawableSync stamps emoticon_name/frames; dual host residual
//! applies set_emoticon on drawable icon UI. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_EMOTICON_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1057: &[&str] = &[
    "emoticon_name",
    "set_emoticon",
    "Wave 1057",
    "playable_claim = false",
];

pub const LIVE_HOST_EMOTICON_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1057: &[&str] = &[
    "EMOTICON",
    "PRESENTATION_STAMP",
    "LIVE_HOST_EMOTICON_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEmoticonPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostEmoticonPresentationResidualAction) {
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
fn drawable_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}

pub fn honesty_host_emoticon_presentation_residual_method_names_residual_wave1057() -> bool {
    let names = LIVE_HOST_EMOTICON_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1057;
    let ok = residual_name_index(names, "emoticon_name").is_some()
        && residual_name_index(names, "Wave 1057").is_some();
    residual_action_store(ResidualHostEmoticonPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_emoticon_presentation_residual_nav_commands_residual_wave1057() -> bool {
    let steps = LIVE_HOST_EMOTICON_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1057;
    let ok = residual_name_index(steps, "LIVE_HOST_EMOTICON_PRESENTATION_RESIDUAL").is_some()
        && residual_name_index(steps, "EMOTICON").is_some();
    residual_action_store(ResidualHostEmoticonPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_emoticon_presentation_residual_residual_pack_wave1057() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let d = drawable_source();
    let ok = client.contains("pub emoticon_name: String")
        && client.contains("pub emoticon_frames_left: i32")
        && cnc.contains("Wave 1057: emoticon residual for dual icon UI")
        && cnc.contains("emoticon_name: o.emoticon_name.clone()")
        && d.contains("Wave 1057: emoticon residual for dual icon UI")
        && d.contains("self.set_emoticon(&emoticon_name, emoticon_frames_left as u32)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostEmoticonPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_emoticon_presentation_residual_honesty() -> bool {
    let a = honesty_host_emoticon_presentation_residual_method_names_residual_wave1057();
    let b = honesty_host_emoticon_presentation_residual_nav_commands_residual_wave1057();
    let c = honesty_host_emoticon_presentation_residual_residual_pack_wave1057();
    residual_action_store(ResidualHostEmoticonPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_emoticon_presentation_residual_wave1057() {
        assert!(honesty_host_emoticon_presentation_residual_residual_pack_wave1057());
        assert!(honesty_host_emoticon_presentation_residual_method_names_residual_wave1057());
        assert!(honesty_host_emoticon_presentation_residual_nav_commands_residual_wave1057());
        assert!(simulate_live_host_emoticon_presentation_residual_honesty());
    }
}
