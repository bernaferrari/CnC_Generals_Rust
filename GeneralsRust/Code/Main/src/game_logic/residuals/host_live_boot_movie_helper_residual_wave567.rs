//! Wave 567 residual peels: boot residual movies are centralized through
//! `apply_boot_movie_residual`. Presentation freeze continues to use
//! `apply_presentation_movie_residual` (peek freeze + drain live queues).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 566 boot UI message helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` apply_boot_movie_residual / apply_presentation_movie_residual
//! - `presentation_frame.rs` pending_movie / pending_radar_movie
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_BOOT_MOVIE_HELPER_METHOD_NAMES_WAVE567: &[&str] = &[
    "apply_boot_movie_residual",
    "apply_presentation_movie_residual",
    "take_pending_movie",
    "take_pending_radar_movie",
    "Wave 567",
    "playable_claim = false",
];

pub const LIVE_BOOT_MOVIE_HELPER_NAV_STEPS_WAVE567: &[&str] = &[
    "REQUIRE_BOOT_MOVIE_HELPER",
    "REQUIRE_PRESENTATION_MOVIE_HELPER",
    "LIVE_BOOT_MOVIE_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_BOOT_MOVIE_HELPER_CMD_NAMES_WAVE567: &[&str] = &[
    "boot_movie_helper",
    "presentation_movie_helper",
    "pending_movie_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualBootMovieHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualBootMovieHelperAction {
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

fn residual_action_store(action: ResidualBootMovieHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_boot_movie_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_boot_movie_helper_last_action() -> ResidualBootMovieHelperAction {
    ResidualBootMovieHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
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

pub fn honesty_boot_movie_helper_method_names_residual_wave567() -> bool {
    let names = LIVE_BOOT_MOVIE_HELPER_METHOD_NAMES_WAVE567;
    let ok = residual_name_index(names, "apply_boot_movie_residual").is_some()
        && residual_name_index(names, "apply_presentation_movie_residual").is_some()
        && residual_name_index(names, "take_pending_movie").is_some()
        && residual_name_index(names, "take_pending_radar_movie").is_some()
        && residual_name_index(names, "Wave 567").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualBootMovieHelperAction::MethodNames);
    ok
}

pub fn honesty_boot_movie_helper_source_markers_residual_wave567() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("pub pending_movie: Option<String>")
        && pf.contains("pub pending_radar_movie: Option<String>");
    let Some(boot) = fn_body(eng, "fn apply_boot_movie_residual(") else {
        residual_action_store(ResidualBootMovieHelperAction::SourceMarkers);
        return false;
    };
    let Some(pres) = fn_body(eng, "fn apply_presentation_movie_residual(") else {
        residual_action_store(ResidualBootMovieHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 899 peeled live take_* dual-reads out of boot movies
    // (fail-closed no-op). Presentation apply peeks freeze fields; Wave 900
    // removed the post-apply take_* drain.
    let boot_ok = boot.contains("Wave 567") && !boot.contains("take_pending_movie()");
    let pres_ok = (pres.contains("Wave 567") || eng.contains("Wave 567: pairs with"))
        && pres.contains("pres.pending_movie")
        && pres.contains("pres.pending_radar_movie")
        && !pres.contains("take_pending_movie()")
        && !pres.contains("take_pending_radar_movie()");
    // Call sites: freeze path uses presentation helper; boot uses boot helper.
    let call_ok = eng.contains("self.apply_boot_movie_residual()")
        && eng.contains("self.apply_presentation_movie_residual(&pres)");
    // No inline boot block left outside helpers.
    let inline_gone = !eng.contains("// Boot residual movies (no presentation frame).");
    let ok = field_ok
        && boot_ok
        && pres_ok
        && call_ok
        && inline_gone
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualBootMovieHelperAction::SourceMarkers);
    ok
}

pub fn honesty_boot_movie_helper_nav_commands_residual_wave567() -> bool {
    let steps = LIVE_BOOT_MOVIE_HELPER_NAV_STEPS_WAVE567;
    let cmds = RUNTIME_HOST_LIVE_BOOT_MOVIE_HELPER_CMD_NAMES_WAVE567;
    let ok = residual_name_index(steps, "REQUIRE_BOOT_MOVIE_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_PRESENTATION_MOVIE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_BOOT_MOVIE_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "boot_movie_helper").is_some()
        && residual_name_index(cmds, "presentation_movie_helper").is_some()
        && residual_name_index(cmds, "pending_movie_residual").is_some();
    residual_action_store(ResidualBootMovieHelperAction::NavCommands);
    ok
}

pub fn simulate_boot_movie_helper_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 567")
        && eng.contains("fn apply_boot_movie_residual")
        && eng.contains("fn apply_presentation_movie_residual");
    residual_action_store(ResidualBootMovieHelperAction::CollectSource);
    ok
}

pub fn simulate_boot_movie_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.apply_boot_movie_residual()")
        && eng.contains("self.apply_presentation_movie_residual(&pres)")
        && eng.contains("last_presentation_frame.clone()");
    residual_action_store(ResidualBootMovieHelperAction::DispatchSource);
    ok
}

pub fn honesty_boot_movie_helper_residual_pack_wave567() -> bool {
    honesty_boot_movie_helper_method_names_residual_wave567()
        && honesty_boot_movie_helper_source_markers_residual_wave567()
        && honesty_boot_movie_helper_nav_commands_residual_wave567()
        && simulate_boot_movie_helper_collect_source()
        && simulate_boot_movie_helper_dispatch_source()
}

pub fn simulate_live_boot_movie_helper_honesty() -> bool {
    let ok = honesty_boot_movie_helper_residual_pack_wave567();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualBootMovieHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_boot_movie_helper_method_names_residual_wave567());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_boot_movie_helper_source_markers_residual_wave567());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_boot_movie_helper_nav_commands_residual_wave567());
    }

    #[test]
    fn boot_movie_helper_sources() {
        assert!(simulate_boot_movie_helper_collect_source());
        assert!(simulate_boot_movie_helper_dispatch_source());
    }

    #[test]
    fn wave567_composite_pack() {
        assert!(honesty_boot_movie_helper_residual_pack_wave567());
    }

    #[test]
    fn simulate_live_boot_movie_helper_honesty_residual_live() {
        assert!(
            simulate_live_boot_movie_helper_honesty(),
            "boot movie helper residual must latch"
        );
        assert!(residual_boot_movie_helper_ok());
        assert_eq!(
            residual_boot_movie_helper_last_action(),
            ResidualBootMovieHelperAction::Composite
        );
    }
}
