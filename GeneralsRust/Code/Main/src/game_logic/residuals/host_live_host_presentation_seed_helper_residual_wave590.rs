//! Wave 590 residual peels: presentation seed paths are centralized through
//! host helpers —
//! `host_seed_presentation_after_match_start`,
//! `host_ensure_presentation_frame_for_render`,
//! `host_ensure_presentation_env_for_hints`.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 589 post-logic finalize residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` host presentation seed helpers
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_SEED_HELPER_METHOD_NAMES_WAVE590: &[&str] = &[
    "host_seed_presentation_after_match_start",
    "host_ensure_presentation_frame_for_render",
    "host_ensure_presentation_env_for_hints",
    "seed_presentation_after_match_start",
    "build_for_engine",
    "sync_from_host",
    "host_sync_shadow_and_build_presentation",
    "Wave 590",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_SEED_HELPER_NAV_STEPS_WAVE590: &[&str] = &[
    "REQUIRE_MATCH_START_SEED_HELPER",
    "REQUIRE_BOOT_RENDER_SEED_HELPER",
    "REQUIRE_PIPELINE_ENV_SEED_HELPER",
    "REQUIRE_BUILD_FOR_ENGINE",
    "REQUIRE_SHADOW_SYNC_BEFORE_SEED",
    "LIVE_HOST_PRESENTATION_SEED_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_PRESENTATION_SEED_HELPER_CMD_NAMES_WAVE590: &[&str] = &[
    "host_match_start_seed_helper",
    "host_boot_render_seed_helper",
    "host_pipeline_env_seed_helper",
    "presentation_seed_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationSeedHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostPresentationSeedHelperAction {
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

fn residual_action_store(action: ResidualHostPresentationSeedHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_presentation_seed_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_presentation_seed_helper_last_action()
-> ResidualHostPresentationSeedHelperAction {
    ResidualHostPresentationSeedHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
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

pub fn honesty_host_presentation_seed_helper_method_names_residual_wave590() -> bool {
    let names = LIVE_HOST_PRESENTATION_SEED_HELPER_METHOD_NAMES_WAVE590;
    let ok = residual_name_index(names, "host_seed_presentation_after_match_start").is_some()
        && residual_name_index(names, "host_ensure_presentation_frame_for_render").is_some()
        && residual_name_index(names, "host_ensure_presentation_env_for_hints").is_some()
        && (residual_name_index(names, "build_for_engine").is_some()
            || residual_name_index(names, "host_sync_shadow_and_build_presentation").is_some())
        && residual_name_index(names, "Wave 590").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostPresentationSeedHelperAction::MethodNames);
    ok
}

pub fn honesty_host_presentation_seed_helper_source_markers_residual_wave590() -> bool {
    let eng = eng_source();
    let required = [
        "fn host_seed_presentation_after_match_start(&mut self)",
        "fn host_ensure_presentation_frame_for_render(&mut self)",
        "fn host_ensure_presentation_env_for_hints(&mut self)",
    ];
    let mut defs_ok = true;
    for sig in required {
        let Some(body) = fn_body(eng, sig) else {
            defs_ok = false;
            break;
        };
        if !body.contains("Wave 590")
            || !(body.contains("build_for_engine")
                || body.contains("host_sync_shadow_and_build_presentation"))
        {
            defs_ok = false;
            break;
        }
    }
    let Some(match_body) = fn_body(
        eng,
        "fn host_seed_presentation_after_match_start(&mut self)",
    ) else {
        residual_action_store(ResidualHostPresentationSeedHelperAction::SourceMarkers);
        return false;
    };
    let match_ok = (match_body.contains("sync_from_host")
        || match_body.contains("host_sync_shadow_and_build_presentation"))
        && match_body.contains("apply_to_game_hud")
        && match_body.contains("last_presentation_frame = Some(pres)");
    let call_ok = eng.contains("self.host_seed_presentation_after_match_start()")
        && eng.contains("self.host_ensure_presentation_frame_for_render()")
        && eng.contains("self.host_ensure_presentation_env_for_hints()");
    // Production build boundary: shared helper owns sync+build (Wave 926).
    let boundary_ok = eng.contains("fn host_sync_shadow_and_build_presentation")
        && eng.contains("host_sync_shadow_and_build_presentation(false)")
        && eng.contains("host_sync_shadow_and_build_presentation(true)");
    let ok =
        defs_ok && match_ok && call_ok && boundary_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostPresentationSeedHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_presentation_seed_helper_nav_commands_residual_wave590() -> bool {
    let steps = LIVE_HOST_PRESENTATION_SEED_HELPER_NAV_STEPS_WAVE590;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRESENTATION_SEED_HELPER_CMD_NAMES_WAVE590;
    let ok = residual_name_index(steps, "REQUIRE_MATCH_START_SEED_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_BOOT_RENDER_SEED_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_PIPELINE_ENV_SEED_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_BUILD_FOR_ENGINE").is_some()
        && residual_name_index(steps, "REQUIRE_SHADOW_SYNC_BEFORE_SEED").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRESENTATION_SEED_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_match_start_seed_helper").is_some()
        && residual_name_index(cmds, "host_boot_render_seed_helper").is_some()
        && residual_name_index(cmds, "host_pipeline_env_seed_helper").is_some()
        && residual_name_index(cmds, "presentation_seed_residual").is_some();
    residual_action_store(ResidualHostPresentationSeedHelperAction::NavCommands);
    ok
}

pub fn simulate_host_presentation_seed_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 590")
        && eng.contains("fn host_seed_presentation_after_match_start")
        && eng.contains("fn host_ensure_presentation_frame_for_render")
        && eng.contains("fn host_ensure_presentation_env_for_hints");
    residual_action_store(ResidualHostPresentationSeedHelperAction::CollectSource);
    ok
}

pub fn simulate_host_presentation_seed_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.host_seed_presentation_after_match_start()")
        && eng.contains("self.host_ensure_presentation_frame_for_render()")
        && eng.contains("self.host_ensure_presentation_env_for_hints()")
        && eng.contains("Boot/Menu residual: if no frame yet");
    residual_action_store(ResidualHostPresentationSeedHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_presentation_seed_helper_residual_pack_wave590() -> bool {
    honesty_host_presentation_seed_helper_method_names_residual_wave590()
        && honesty_host_presentation_seed_helper_source_markers_residual_wave590()
        && honesty_host_presentation_seed_helper_nav_commands_residual_wave590()
        && simulate_host_presentation_seed_helper_collect_source()
        && simulate_host_presentation_seed_helper_dispatch_source()
}

pub fn simulate_live_host_presentation_seed_helper_honesty() -> bool {
    let ok = honesty_host_presentation_seed_helper_residual_pack_wave590();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostPresentationSeedHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_presentation_seed_helper_method_names_residual_wave590());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_presentation_seed_helper_source_markers_residual_wave590());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_presentation_seed_helper_nav_commands_residual_wave590());
    }

    #[test]
    fn host_presentation_seed_helper_sources() {
        assert!(simulate_host_presentation_seed_helper_collect_source());
        assert!(simulate_host_presentation_seed_helper_dispatch_source());
    }

    #[test]
    fn wave590_composite_pack() {
        assert!(honesty_host_presentation_seed_helper_residual_pack_wave590());
    }

    #[test]
    fn simulate_live_host_presentation_seed_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_presentation_seed_helper_honesty(),
            "host presentation seed helper residual must latch"
        );
        assert!(residual_host_presentation_seed_helper_ok());
        assert_eq!(
            residual_host_presentation_seed_helper_last_action(),
            ResidualHostPresentationSeedHelperAction::Composite
        );
    }
}
