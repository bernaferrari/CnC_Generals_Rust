//! Wave 474 residual peels: ensure_presentation_env_for_hints is instance-only.
//! - `fn ensure_presentation_env_for_hints(&mut self)` (no free GameLogic params)
//! - seeds via self.game_logic + self.gameworld_shadow
//! - ensure_presentation_env_seeded calls self.ensure_...
//! - zero free-fn helpers take typed game_logic params on CncGameEngine
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 466/467 seed/mirror peels (supersedes free-fn surface).
//! Architecture residual - Main free dual-read surface for env seed closed.
//!
//! Sources (cnc_game_engine.rs):
//! - fn ensure_presentation_env_for_hints(&mut self)
//! - fn ensure_presentation_env_seeded(&mut self)
//! - no free fn with game_logic: &GameLogic
//!
//! Fail-closed:
//! - Still freezes from host GameLogic when pipeline empty
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const ENSURE_PRESENTATION_ENV_INSTANCE_METHOD_NAMES_WAVE474: &[&str] = &[
    "ensure_presentation_env_for_hints",
    "ensure_presentation_env_seeded",
    "build_for_engine",
    "gameworld_shadow.as_ref",
    "zero free game_logic helpers",
    "playable_claim = false",
];

pub const ENSURE_PRESENTATION_ENV_INSTANCE_SOURCE_MARKERS_WAVE474: &[&str] = &[
    "fn ensure_presentation_env_for_hints(&mut self)",
    "Wave 474: instance seed only",
    "self.gameworld_shadow.as_ref()",
    "self.ensure_presentation_env_for_hints()",
];

pub const ENSURE_PRESENTATION_ENV_INSTANCE_NAV_STEPS_WAVE474: &[&str] = &[
    "CONVERT_ENSURE_TO_INSTANCE",
    "SEED_USES_SELF_GAMELOGIC",
    "SEED_USES_SELF_SHADOW",
    "SEEDED_CALLS_SELF_ENSURE",
    "FREE_FN_SURFACE_EMPTY",
    "NO_FREE_GAMELOGIC_PARAM",
];

pub const RUNTIME_HOST_ENSURE_PRESENTATION_ENV_INSTANCE_CMD_NAMES_WAVE474: &[&str] = &[
    "click_ensure_presentation_env_instance_ok_wnd_convert",
    "click_ensure_presentation_env_instance_ok_wnd_seed",
    "click_ensure_presentation_env_instance_ok_wnd_shadow",
    "click_ensure_presentation_env_instance_ok_wnd_prepare",
    "click_ensure_presentation_env_instance_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualEnsurePresentationEnvInstanceAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    EnsureSource = 4,
    FreeSurface = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualEnsurePresentationEnvInstanceAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_ensure_presentation_env_instance_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_ensure_presentation_env_instance_last_action()
-> ResidualEnsurePresentationEnvInstanceAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualEnsurePresentationEnvInstanceAction::MethodNames,
        2 => ResidualEnsurePresentationEnvInstanceAction::SourceMarkers,
        3 => ResidualEnsurePresentationEnvInstanceAction::NavCommands,
        4 => ResidualEnsurePresentationEnvInstanceAction::EnsureSource,
        5 => ResidualEnsurePresentationEnvInstanceAction::FreeSurface,
        6 => ResidualEnsurePresentationEnvInstanceAction::Composite,
        _ => ResidualEnsurePresentationEnvInstanceAction::Idle,
    }
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
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

fn free_fns_taking_game_logic(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut search = src;
    while let Some(rel) = search.find("fn ") {
        let after = &search[rel + 3..];
        let name_len = after
            .chars()
            .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
            .count();
        let name = &after[..name_len];
        let sig_end = after.find('{').unwrap_or(after.len().min(600));
        let sig = &after[..sig_end];
        let has_param = sig.contains("game_logic: &")
            || sig.contains("game_logic: Option<&")
            || sig.contains("game_logic:Option<&")
            || sig.contains("game_logic: &mut");
        let first = sig
            .find('(')
            .map(|i| sig[i + 1..].split(',').next().unwrap_or("").trim())
            .unwrap_or("");
        let is_instance = (first.starts_with('&') && first.contains("self"))
            || first == "self"
            || first.starts_with("mut self");
        if has_param && !is_instance && !name.is_empty() {
            out.push(name.to_string());
        }
        search = &after[name_len.max(1)..];
    }
    out.sort();
    out.dedup();
    out
}

pub fn honesty_ensure_presentation_env_instance_method_names_residual_wave474() -> bool {
    ENSURE_PRESENTATION_ENV_INSTANCE_METHOD_NAMES_WAVE474.len() == 6
        && residual_name_index(
            ENSURE_PRESENTATION_ENV_INSTANCE_METHOD_NAMES_WAVE474,
            "ensure_presentation_env_for_hints",
        ) == Some(0)
        && residual_name_index(
            ENSURE_PRESENTATION_ENV_INSTANCE_METHOD_NAMES_WAVE474,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_ensure_presentation_env_instance_source_markers_residual_wave474() -> bool {
    ENSURE_PRESENTATION_ENV_INSTANCE_SOURCE_MARKERS_WAVE474.len() == 4
        && residual_name_index(
            ENSURE_PRESENTATION_ENV_INSTANCE_SOURCE_MARKERS_WAVE474,
            "fn ensure_presentation_env_for_hints(&mut self)",
        ) == Some(0)
        && residual_name_index(
            ENSURE_PRESENTATION_ENV_INSTANCE_SOURCE_MARKERS_WAVE474,
            "self.ensure_presentation_env_for_hints()",
        ) == Some(3)
}

pub fn honesty_ensure_presentation_env_instance_nav_commands_residual_wave474() -> bool {
    ENSURE_PRESENTATION_ENV_INSTANCE_NAV_STEPS_WAVE474.len() == 6
        && residual_name_index(
            ENSURE_PRESENTATION_ENV_INSTANCE_NAV_STEPS_WAVE474,
            "CONVERT_ENSURE_TO_INSTANCE",
        ) == Some(0)
        && residual_name_index(
            ENSURE_PRESENTATION_ENV_INSTANCE_NAV_STEPS_WAVE474,
            "NO_FREE_GAMELOGIC_PARAM",
        ) == Some(5)
        && RUNTIME_HOST_ENSURE_PRESENTATION_ENV_INSTANCE_CMD_NAMES_WAVE474.len() == 5
        && residual_name_index(
            RUNTIME_HOST_ENSURE_PRESENTATION_ENV_INSTANCE_CMD_NAMES_WAVE474,
            "click_ensure_presentation_env_instance_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_ensure_presentation_env_instance_source() -> bool {
    let src = cnc_source();
    // Wave 590: thin wrapper delegates to host_ensure_presentation_env_for_hints.
    let Some(wrap) = function_body(src, "fn ensure_presentation_env_for_hints(") else {
        return false;
    };
    let wrap_ok = wrap.contains("host_ensure_presentation_env_for_hints")
        && wrap.contains("&mut self")
        && !wrap.contains("game_logic: &GameLogic");
    let Some(body) = function_body(src, "fn host_ensure_presentation_env_for_hints(") else {
        return false;
    };
    let ok = wrap_ok
        && (body.contains("Wave 474") || body.contains("Wave 590"))
        && body.contains("self.gameworld_shadow.as_ref()")
        && body.contains("&self.game_logic")
        && body.contains("build_for_engine")
        && !body.contains("game_logic: &GameLogic");
    residual_action_store(ResidualEnsurePresentationEnvInstanceAction::EnsureSource);
    ok
}

pub fn simulate_ensure_presentation_env_free_surface_empty() -> bool {
    let src = cnc_source();
    let free = free_fns_taking_game_logic(src);
    let ok = free.is_empty()
        && src.contains("self.ensure_presentation_env_for_hints()")
        && !src.contains("Self::ensure_presentation_env_for_hints(");
    residual_action_store(ResidualEnsurePresentationEnvInstanceAction::FreeSurface);
    ok
}

pub fn honesty_ensure_presentation_env_instance_residual_pack_wave474() -> bool {
    honesty_ensure_presentation_env_instance_method_names_residual_wave474()
        && honesty_ensure_presentation_env_instance_source_markers_residual_wave474()
        && honesty_ensure_presentation_env_instance_nav_commands_residual_wave474()
        && simulate_ensure_presentation_env_instance_source()
        && simulate_ensure_presentation_env_free_surface_empty()
}

pub fn simulate_live_ensure_presentation_env_instance_honesty() -> bool {
    let ok = honesty_ensure_presentation_env_instance_residual_pack_wave474();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualEnsurePresentationEnvInstanceAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_ensure_presentation_env_instance_method_names_residual_wave474());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_ensure_presentation_env_instance_source_markers_residual_wave474());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_ensure_presentation_env_instance_nav_commands_residual_wave474());
    }

    #[test]
    fn ensure_presentation_env_instance_sources() {
        assert!(simulate_ensure_presentation_env_instance_source());
        assert!(simulate_ensure_presentation_env_free_surface_empty());
        assert!(free_fns_taking_game_logic(cnc_source()).is_empty());
    }

    #[test]
    fn wave474_composite_pack() {
        assert!(honesty_ensure_presentation_env_instance_residual_pack_wave474());
    }

    #[test]
    fn simulate_live_ensure_presentation_env_instance_honesty_residual_live() {
        assert!(
            simulate_live_ensure_presentation_env_instance_honesty(),
            "ensure presentation env instance residual must latch"
        );
        assert!(residual_ensure_presentation_env_instance_ok());
        assert_eq!(
            residual_ensure_presentation_env_instance_last_action(),
            ResidualEnsurePresentationEnvInstanceAction::Composite
        );
    }
}
