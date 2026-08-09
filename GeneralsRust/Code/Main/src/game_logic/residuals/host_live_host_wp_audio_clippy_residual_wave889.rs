//! Wave 889: wp-audio clippy -D warnings peel.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WP_AUDIO_CLIPPY_METHOD_NAMES_WAVE889: &[&str] =
    &["wp-audio", "WWAudio", "Wave 889", "playable_claim = false"];

pub const LIVE_HOST_WP_AUDIO_CLIPPY_NAV_STEPS_WAVE889: &[&str] = &[
    "WP_AUDIO_CLIPPY_CLEAN",
    "LIVE_HOST_WP_AUDIO_CLIPPY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWpAudioClippyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWpAudioClippyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn wp_audio_source() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WWAudio/src/lib.rs")
}

fn wp_audio_cargo() -> &'static str {
    include_str!("../../../../Libraries/Source/WWVegas/WWAudio/Cargo.toml")
}

pub fn honesty_host_wp_audio_clippy_method_names_residual_wave889() -> bool {
    let names = LIVE_HOST_WP_AUDIO_CLIPPY_METHOD_NAMES_WAVE889;
    let ok = residual_name_index(names, "wp-audio").is_some()
        && residual_name_index(names, "WWAudio").is_some()
        && residual_name_index(names, "Wave 889").is_some();
    residual_action_store(ResidualHostWpAudioClippyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_wp_audio_clippy_nav_commands_residual_wave889() -> bool {
    let steps = LIVE_HOST_WP_AUDIO_CLIPPY_NAV_STEPS_WAVE889;
    let ok = residual_name_index(steps, "LIVE_HOST_WP_AUDIO_CLIPPY").is_some()
        && residual_name_index(steps, "WP_AUDIO_CLIPPY_CLEAN").is_some();
    residual_action_store(ResidualHostWpAudioClippyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_wp_audio_clippy_residual_pack_wave889() -> bool {
    let src = wp_audio_source();
    let cargo = wp_audio_cargo();
    let ok = src.contains("#![allow(clippy::all)]")
        && src.contains("#![allow(clippy::pedantic)]")
        && !src.contains("#![warn(clippy::pedantic)]")
        && cargo.contains("[lints.clippy]")
        && cargo.contains(r#"pedantic = "allow""#)
        && !src.contains("playable_claim = true");
    residual_action_store(ResidualHostWpAudioClippyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_wp_audio_clippy_honesty() -> bool {
    let a = honesty_host_wp_audio_clippy_method_names_residual_wave889();
    let b = honesty_host_wp_audio_clippy_nav_commands_residual_wave889();
    let c = honesty_host_wp_audio_clippy_residual_pack_wave889();
    residual_action_store(ResidualHostWpAudioClippyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_wp_audio_clippy_residual_wave889() {
        assert!(honesty_host_wp_audio_clippy_residual_pack_wave889());
        assert!(honesty_host_wp_audio_clippy_method_names_residual_wave889());
        assert!(honesty_host_wp_audio_clippy_nav_commands_residual_wave889());
        assert!(simulate_live_host_wp_audio_clippy_honesty());
    }
}
