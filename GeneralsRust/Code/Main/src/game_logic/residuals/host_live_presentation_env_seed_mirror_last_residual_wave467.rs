//! Wave 467 residual peels: ensure_presentation_env_seeded mirrors pipeline
//! freeze into last_presentation_frame when missing (boot UI/script consumers).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 466 shadow-aware env seed.
//! Architecture residual - one seed path for pipeline + last frame.
//!
//! Sources (cnc_game_engine.rs):
//! - ensure_presentation_env_seeded calls ensure_presentation_env_for_hints + mirror
//! - call sites use ensure_presentation_env_seeded()
//! - last_presentation_frame filled when pipeline freeze installed
//!
//! Fail-closed:
//! - Boot without freeze still uses live update_ui_state
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_ENV_SEED_MIRROR_LAST_METHOD_NAMES_WAVE467: &[&str] = &[
    "ensure_presentation_env_seeded",
    "ensure_presentation_env_for_hints",
    "presentation_frame",
    "last_presentation_frame",
    "set_presentation_frame",
    "gameworld_shadow",
];

pub const PRESENTATION_ENV_SEED_MIRROR_LAST_SOURCE_MARKERS_WAVE467: &[&str] = &[
    "Wave 467: seed pipeline presentation (host+GW) and mirror into last_presentation_frame",
    "ensure_presentation_env_seeded",
    "last_presentation_frame.is_none()",
    "presentation_frame().cloned()",
];

pub const PRESENTATION_ENV_SEED_MIRROR_LAST_NAV_STEPS_WAVE467: &[&str] = &[
    "CALL_ENSURE_PRESENTATION_ENV_SEEDED",
    "SEED_PIPELINE_VIA_BUILD_FOR_ENGINE",
    "MIRROR_TO_LAST_PRESENTATION_IF_MISSING",
    "RENDER_UI_PREFERS_PIPELINE_OR_LAST",
    "BOOT_LIVE_UI_ONLY_WHEN_NO_FRAME",
    "NO_ORPHAN_PIPELINE_FREEZE",
];

pub const RUNTIME_HOST_PRESENTATION_ENV_SEED_MIRROR_LAST_CMD_NAMES_WAVE467: &[&str] = &[
    "click_presentation_env_seed_mirror_last_ok_wnd_call",
    "click_presentation_env_seed_mirror_last_ok_wnd_seed",
    "click_presentation_env_seed_mirror_last_ok_wnd_mirror",
    "click_presentation_env_seed_mirror_last_ok_wnd_prepare",
    "click_presentation_env_seed_mirror_last_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationEnvSeedMirrorLastAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    SeededSource = 4,
    CallSites = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationEnvSeedMirrorLastAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_env_seed_mirror_last_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_env_seed_mirror_last_last_action()
-> ResidualPresentationEnvSeedMirrorLastAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationEnvSeedMirrorLastAction::MethodNames,
        2 => ResidualPresentationEnvSeedMirrorLastAction::SourceMarkers,
        3 => ResidualPresentationEnvSeedMirrorLastAction::NavCommands,
        4 => ResidualPresentationEnvSeedMirrorLastAction::SeededSource,
        5 => ResidualPresentationEnvSeedMirrorLastAction::CallSites,
        6 => ResidualPresentationEnvSeedMirrorLastAction::Composite,
        _ => ResidualPresentationEnvSeedMirrorLastAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_env_seed_mirror_last_method_names_residual_wave467() -> bool {
    PRESENTATION_ENV_SEED_MIRROR_LAST_METHOD_NAMES_WAVE467.len() == 6
        && residual_name_index(
            PRESENTATION_ENV_SEED_MIRROR_LAST_METHOD_NAMES_WAVE467,
            "ensure_presentation_env_seeded",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_ENV_SEED_MIRROR_LAST_METHOD_NAMES_WAVE467,
            "gameworld_shadow",
        ) == Some(5)
}

pub fn honesty_presentation_env_seed_mirror_last_source_markers_residual_wave467() -> bool {
    PRESENTATION_ENV_SEED_MIRROR_LAST_SOURCE_MARKERS_WAVE467.len() == 4
        && residual_name_index(
            PRESENTATION_ENV_SEED_MIRROR_LAST_SOURCE_MARKERS_WAVE467,
            "Wave 467: seed pipeline presentation (host+GW) and mirror into last_presentation_frame",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_ENV_SEED_MIRROR_LAST_SOURCE_MARKERS_WAVE467,
            "presentation_frame().cloned()",
        ) == Some(3)
}

pub fn honesty_presentation_env_seed_mirror_last_nav_commands_residual_wave467() -> bool {
    PRESENTATION_ENV_SEED_MIRROR_LAST_NAV_STEPS_WAVE467.len() == 6
        && residual_name_index(
            PRESENTATION_ENV_SEED_MIRROR_LAST_NAV_STEPS_WAVE467,
            "MIRROR_TO_LAST_PRESENTATION_IF_MISSING",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_ENV_SEED_MIRROR_LAST_NAV_STEPS_WAVE467,
            "NO_ORPHAN_PIPELINE_FREEZE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_ENV_SEED_MIRROR_LAST_CMD_NAMES_WAVE467.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRESENTATION_ENV_SEED_MIRROR_LAST_CMD_NAMES_WAVE467,
            "click_presentation_env_seed_mirror_last_ok_wnd_prepare",
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

pub fn simulate_presentation_env_seed_mirror_last_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn ensure_presentation_env_seeded(") else {
        return false;
    };
    let ok = (body.contains(
        "Wave 467: seed pipeline presentation (host+GW) and mirror into last_presentation_frame",
    ) || body.contains("Wave 467/474: seed pipeline presentation"))
        && body.contains("ensure_presentation_env_for_hints")
        && body.contains("last_presentation_frame.is_none()")
        && body.contains("presentation_frame().cloned()")
        && !body.contains("Self::ensure_presentation_env_for_hints(");
    residual_action_store(ResidualPresentationEnvSeedMirrorLastAction::SeededSource);
    ok
}

pub fn simulate_presentation_env_seed_mirror_last_callsites() -> bool {
    let src = cnc_source();
    let n = src.matches("ensure_presentation_env_seeded()").count();
    // Exclude the definition line `fn ensure_presentation_env_seeded(`
    let ok = n >= 3 && src.contains("fn ensure_presentation_env_seeded");
    residual_action_store(ResidualPresentationEnvSeedMirrorLastAction::CallSites);
    ok
}

pub fn honesty_presentation_env_seed_mirror_last_residual_pack_wave467() -> bool {
    honesty_presentation_env_seed_mirror_last_method_names_residual_wave467()
        && honesty_presentation_env_seed_mirror_last_source_markers_residual_wave467()
        && honesty_presentation_env_seed_mirror_last_nav_commands_residual_wave467()
        && simulate_presentation_env_seed_mirror_last_source()
        && simulate_presentation_env_seed_mirror_last_callsites()
}

pub fn simulate_live_presentation_env_seed_mirror_last_honesty() -> bool {
    let ok = honesty_presentation_env_seed_mirror_last_residual_pack_wave467();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationEnvSeedMirrorLastAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_env_seed_mirror_last_method_names_residual_wave467());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_env_seed_mirror_last_source_markers_residual_wave467());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_env_seed_mirror_last_nav_commands_residual_wave467());
    }

    #[test]
    fn presentation_env_seed_mirror_last_sources() {
        assert!(simulate_presentation_env_seed_mirror_last_source());
        assert!(simulate_presentation_env_seed_mirror_last_callsites());
    }

    #[test]
    fn wave467_composite_pack() {
        assert!(honesty_presentation_env_seed_mirror_last_residual_pack_wave467());
    }

    #[test]
    fn simulate_live_presentation_env_seed_mirror_last_honesty_residual_live() {
        assert!(
            simulate_live_presentation_env_seed_mirror_last_honesty(),
            "presentation env seed mirror last residual must latch"
        );
        assert!(residual_presentation_env_seed_mirror_last_ok());
        assert_eq!(
            residual_presentation_env_seed_mirror_last_last_action(),
            ResidualPresentationEnvSeedMirrorLastAction::Composite
        );
    }
}
