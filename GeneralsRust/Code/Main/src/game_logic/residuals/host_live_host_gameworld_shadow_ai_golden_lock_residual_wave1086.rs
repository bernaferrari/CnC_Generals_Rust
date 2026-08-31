//! Wave 1086: GameWorld shadow + AI/golden honesty lock residual.
//!
//! After Waves 1084–1085, locks production honesty markers:
//! - host → GameWorldShadow writeback → presentation shell (no dual-tick default)
//! - AI ATTACK_RECHECK_SECONDS residual-locked at 60s (not gate-driven 2s)
//! - golden map combat forbids take_damage / re-team cheats
//! - playable_claim stays false (no full retail WND/GPU playthrough)

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GAMEWORLD_SHADOW_AI_GOLDEN_LOCK_METHOD_NAMES_WAVE1086: &[&str] = &[
    "host_run_gameworld_shadow_after_logic",
    "host_sync_shadow_and_build_presentation",
    "ATTACK_RECHECK_SECONDS",
    "no take_damage fallback",
    "Wave 1086",
    "playable_claim = false",
];

pub const LIVE_HOST_GAMEWORLD_SHADOW_AI_GOLDEN_LOCK_NAV_STEPS_WAVE1086: &[&str] = &[
    "GAMEWORLD_SHADOW",
    "AI_60S_RECHECK",
    "GOLDEN_NO_TAKE_DAMAGE_CHEAT",
    "LIVE_HOST_GAMEWORLD_SHADOW_AI_GOLDEN_LOCK",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGameworldShadowAiGoldenLockAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostGameworldShadowAiGoldenLockAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn aw_source() -> &'static str {
    include_str!("../../authoritative_world.rs")
}
fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn ai_source() -> &'static str {
    include_str!("../../ai.rs")
}
fn golden_source() -> &'static str {
    include_str!("../../golden_skirmish.rs")
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

pub fn honesty_host_gameworld_shadow_ai_golden_lock_method_names_residual_wave1086() -> bool {
    let names = LIVE_HOST_GAMEWORLD_SHADOW_AI_GOLDEN_LOCK_METHOD_NAMES_WAVE1086;
    let ok = residual_name_index(names, "host_run_gameworld_shadow_after_logic").is_some()
        && residual_name_index(names, "Wave 1086").is_some();
    residual_action_store(ResidualHostGameworldShadowAiGoldenLockAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_gameworld_shadow_ai_golden_lock_nav_commands_residual_wave1086() -> bool {
    let steps = LIVE_HOST_GAMEWORLD_SHADOW_AI_GOLDEN_LOCK_NAV_STEPS_WAVE1086;
    let ok = residual_name_index(steps, "LIVE_HOST_GAMEWORLD_SHADOW_AI_GOLDEN_LOCK").is_some()
        && residual_name_index(steps, "GAMEWORLD_SHADOW").is_some();
    residual_action_store(ResidualHostGameworldShadowAiGoldenLockAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_gameworld_shadow_ai_golden_lock_residual_pack_wave1086() -> bool {
    let aw = aw_source();
    let cnc = cnc_source();
    let shadow = shadow_source();
    let ai = ai_source();
    let golden = golden_source();
    let es = es_source();
    let ui = ui_source();
    let ok = cnc.contains("fn host_run_gameworld_shadow_after_logic")
        && cnc.contains("fn host_sync_shadow_and_build_presentation")
        && cnc.contains("host_tick_game_client_presentation_shell")
        && shadow.contains("pub struct GameWorldShadow")
        && shadow.contains("apply_host_writeback_op")
        && shadow.contains("shadow_session_after_host_tick")
        && aw.contains("fn dual_tick_policy")
        && aw.contains("DualTickPolicy::AuthorityOnly")
        && aw.contains("GENERALS_ALLOW_DUAL_TICK")
        && ai.contains("pub const ATTACK_RECHECK_SECONDS: f32 = 60.0")
        && ai.contains("Wave 616: residual-locked at 60s")
        && golden.contains("no take_damage fallback")
        && golden.contains("no take_damage / re-team cheat")
        // 2026-08-14: executable_smoke refactored to the five-flag
        // `retail_windowed_playable_claim` formula; the headless gate keeps the
        // claim false via that constructor instead of a literal assignment.
        && es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`")
        && !cnc.contains("playable_claim = true")
        && ui.contains(
            "Wave 1085: slaver/tip residual fail-closed on unusable/FOW/stealth non-local",
        );
    residual_action_store(ResidualHostGameworldShadowAiGoldenLockAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_gameworld_shadow_ai_golden_lock_honesty() -> bool {
    let a = honesty_host_gameworld_shadow_ai_golden_lock_method_names_residual_wave1086();
    let b = honesty_host_gameworld_shadow_ai_golden_lock_nav_commands_residual_wave1086();
    let c = honesty_host_gameworld_shadow_ai_golden_lock_residual_pack_wave1086();
    residual_action_store(ResidualHostGameworldShadowAiGoldenLockAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_gameworld_shadow_ai_golden_lock_wave1086() {
        assert!(honesty_host_gameworld_shadow_ai_golden_lock_residual_pack_wave1086());
        assert!(honesty_host_gameworld_shadow_ai_golden_lock_method_names_residual_wave1086());
        assert!(honesty_host_gameworld_shadow_ai_golden_lock_nav_commands_residual_wave1086());
        assert!(simulate_live_host_gameworld_shadow_ai_golden_lock_honesty());
    }
}
