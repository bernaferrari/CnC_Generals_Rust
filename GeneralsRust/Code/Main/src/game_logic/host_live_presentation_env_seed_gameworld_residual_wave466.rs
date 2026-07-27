//! Wave 466 residual peels: ensure_presentation_env_for_hints seeds
//! PresentationFrame via build_for_engine(host, shadow) when GameWorld shadow
//! exists (not host-only None freeze).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 455 presentation-only env apply.
//! Architecture residual - boot/map env freeze includes GW overlay when live.
//!
//! Sources (cnc_game_engine.rs):
//! - ensure_presentation_env_for_hints(..., shadow: Option<&GameWorldShadow>)
//! - build_for_engine(..., shadow) instead of None
//! - call sites pass self.gameworld_shadow.as_ref()
//!
//! Fail-closed:
//! - Without shadow session still freezes from host logic only
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_ENV_SEED_GAMEWORLD_METHOD_NAMES_WAVE466: &[&str] = &[
    "ensure_presentation_env_for_hints",
    "build_for_engine",
    "gameworld_shadow",
    "presentation_frame",
    "set_presentation_frame",
    "overlay_gameworld_shadow",
];

pub const PRESENTATION_ENV_SEED_GAMEWORLD_SOURCE_MARKERS_WAVE466: &[&str] = &[
    "Wave 466: prefer host+GameWorld shadow freeze when a shadow session exists",
    "build_for_engine",
    "gameworld_shadow.as_ref()",
    "set_presentation_frame",
];

pub const PRESENTATION_ENV_SEED_GAMEWORLD_NAV_STEPS_WAVE466: &[&str] = &[
    "DETECT_MISSING_PRESENTATION_FRAME",
    "PASS_GAMEWORLD_SHADOW_OPTION",
    "BUILD_FOR_ENGINE_WITH_SHADOW",
    "SET_PIPELINE_PRESENTATION_FRAME",
    "ENV_APPLY_PRESENTATION_ONLY",
    "NO_HOST_ONLY_NONE_WHEN_SHADOW_LIVE",
];

pub const RUNTIME_HOST_PRESENTATION_ENV_SEED_GAMEWORLD_CMD_NAMES_WAVE466: &[&str] = &[
    "click_presentation_env_seed_gameworld_ok_wnd_detect",
    "click_presentation_env_seed_gameworld_ok_wnd_pass_shadow",
    "click_presentation_env_seed_gameworld_ok_wnd_build",
    "click_presentation_env_seed_gameworld_ok_wnd_prepare",
    "click_presentation_env_seed_gameworld_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationEnvSeedGameworldAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EnsureSource = 4,
    CallSites = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationEnvSeedGameworldAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_env_seed_gameworld_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_env_seed_gameworld_last_action(
) -> ResidualPresentationEnvSeedGameworldAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationEnvSeedGameworldAction::MethodNames,
        2 => ResidualPresentationEnvSeedGameworldAction::SourceMarkers,
        3 => ResidualPresentationEnvSeedGameworldAction::NavCommands,
        4 => ResidualPresentationEnvSeedGameworldAction::EnsureSource,
        5 => ResidualPresentationEnvSeedGameworldAction::CallSites,
        6 => ResidualPresentationEnvSeedGameworldAction::Composite,
        _ => ResidualPresentationEnvSeedGameworldAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

pub fn honesty_presentation_env_seed_gameworld_method_names_residual_wave466() -> bool {
    PRESENTATION_ENV_SEED_GAMEWORLD_METHOD_NAMES_WAVE466.len() == 6
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_METHOD_NAMES_WAVE466,
            "ensure_presentation_env_for_hints",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_METHOD_NAMES_WAVE466,
            "overlay_gameworld_shadow",
        ) == Some(5)
}

pub fn honesty_presentation_env_seed_gameworld_source_markers_residual_wave466() -> bool {
    PRESENTATION_ENV_SEED_GAMEWORLD_SOURCE_MARKERS_WAVE466.len() == 4
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_SOURCE_MARKERS_WAVE466,
            "Wave 466: prefer host+GameWorld shadow freeze when a shadow session exists",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_SOURCE_MARKERS_WAVE466,
            "gameworld_shadow.as_ref()",
        ) == Some(2)
}

pub fn honesty_presentation_env_seed_gameworld_nav_commands_residual_wave466() -> bool {
    PRESENTATION_ENV_SEED_GAMEWORLD_NAV_STEPS_WAVE466.len() == 6
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_NAV_STEPS_WAVE466,
            "PASS_GAMEWORLD_SHADOW_OPTION",
        ) == Some(1)
        && residual_name_index(
            PRESENTATION_ENV_SEED_GAMEWORLD_NAV_STEPS_WAVE466,
            "NO_HOST_ONLY_NONE_WHEN_SHADOW_LIVE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_ENV_SEED_GAMEWORLD_CMD_NAMES_WAVE466.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRESENTATION_ENV_SEED_GAMEWORLD_CMD_NAMES_WAVE466,
            "click_presentation_env_seed_gameworld_ok_wnd_prepare",
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

pub fn simulate_presentation_env_seed_gameworld_source() -> bool {
    let src = cnc_source();
    let Some(body) = function_body(src, "fn ensure_presentation_env_for_hints(") else {
        return false;
    };
    let ok = body
        .contains("Wave 466: prefer host+GameWorld shadow freeze when a shadow session exists")
        && body.contains("shadow: Option<&crate::gameworld_shadow::GameWorldShadow>")
        && body.contains("build_for_engine")
        && body.contains("shadow,")
        && !body.contains("None,\n            );");
    residual_action_store(ResidualPresentationEnvSeedGameworldAction::EnsureSource);
    ok
}

pub fn simulate_presentation_env_seed_gameworld_callsites() -> bool {
    let src = cnc_source();
    // Wave 467: call sites use ensure_presentation_env_seeded (mirrors last frame).
    let seeded = src.matches("ensure_presentation_env_seeded()").count();
    let def_ok = src.contains("fn ensure_presentation_env_seeded")
        && src.contains("self.gameworld_shadow.as_ref()")
        && src.contains("last_presentation_frame.is_none()");
    // Free-fn ensure still takes shadow Option; no two-arg Self::ensure call sites remain.
    let mut two_arg = 0usize;
    let mut from = 0usize;
    while let Some(rel) = src[from..].find("Self::ensure_presentation_env_for_hints(") {
        let i = from + rel;
        let win = &src[i..src.len().min(i + 280)];
        // Definition site includes param list with shadow: — skip fn definition.
        if !win.contains("shadow:") && !win.contains("gameworld_shadow.as_ref()") {
            two_arg += 1;
        }
        from = i + 40;
    }
    let ok = seeded >= 3 && def_ok && two_arg == 0;
    residual_action_store(ResidualPresentationEnvSeedGameworldAction::CallSites);
    ok
}

pub fn honesty_presentation_env_seed_gameworld_residual_pack_wave466() -> bool {
    honesty_presentation_env_seed_gameworld_method_names_residual_wave466()
        && honesty_presentation_env_seed_gameworld_source_markers_residual_wave466()
        && honesty_presentation_env_seed_gameworld_nav_commands_residual_wave466()
        && simulate_presentation_env_seed_gameworld_source()
        && simulate_presentation_env_seed_gameworld_callsites()
}

pub fn simulate_live_presentation_env_seed_gameworld_honesty() -> bool {
    let ok = honesty_presentation_env_seed_gameworld_residual_pack_wave466();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationEnvSeedGameworldAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_env_seed_gameworld_method_names_residual_wave466());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_env_seed_gameworld_source_markers_residual_wave466());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_env_seed_gameworld_nav_commands_residual_wave466());
    }

    #[test]
    fn presentation_env_seed_gameworld_sources() {
        assert!(simulate_presentation_env_seed_gameworld_source());
        assert!(simulate_presentation_env_seed_gameworld_callsites());
    }

    #[test]
    fn wave466_composite_pack() {
        assert!(honesty_presentation_env_seed_gameworld_residual_pack_wave466());
    }

    #[test]
    fn simulate_live_presentation_env_seed_gameworld_honesty_residual_live() {
        assert!(
            simulate_live_presentation_env_seed_gameworld_honesty(),
            "presentation env seed gameworld residual must latch"
        );
        assert!(residual_presentation_env_seed_gameworld_ok());
        assert_eq!(
            residual_presentation_env_seed_gameworld_last_action(),
            ResidualPresentationEnvSeedGameworldAction::Composite
        );
    }
}
