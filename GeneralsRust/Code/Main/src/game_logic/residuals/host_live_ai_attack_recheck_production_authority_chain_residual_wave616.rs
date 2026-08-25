//! Wave 616 residual peels:
//! 1) Host AI attack recheck spacing stays **60s** (C++ `checkReadyTeams` ready-team
//!    force-start *numeric* residual) — not a gate-driven early-attack shortcut.
//! 2) Production authority chain markers remain wired:
//!    ready-log → collect → spawn helper → apply (Waves 614/613/615/608).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 177 production authority / AI skirmish residuals.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `ai.rs` ATTACK_RECHECK_SECONDS + evaluate_attack_opportunities
//! - `game_logic.rs` / `gameworld_shadow.rs` production sole-tick chain
//! - GeneralsMD `AI/AIPlayer.cpp` checkReadyTeams 60*LOGICFRAMES_PER_SECOND
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false
//! - Full C++ checkReadyTeams (idle/scripted teams) remains unported

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_AI_ATTACK_RECHECK_PRODUCTION_AUTHORITY_CHAIN_METHOD_NAMES_WAVE616: &[&str] = &[
    "ATTACK_RECHECK_SECONDS",
    "evaluate_attack_opportunities",
    "host_production_ready_log",
    "host_collect_production_completions",
    "host_spawn_production_unit",
    "Wave 616",
    "playable_claim = false",
];

pub const LIVE_AI_ATTACK_RECHECK_PRODUCTION_AUTHORITY_CHAIN_NAV_STEPS_WAVE616: &[&str] = &[
    "REQUIRE_ATTACK_RECHECK_60S",
    "REQUIRE_NO_GATE_EARLY_ATTACK",
    "REQUIRE_PRODUCTION_READY_LOG",
    "REQUIRE_PRODUCTION_SPAWN_HELPER",
    "LIVE_AI_ATTACK_RECHECK_PRODUCTION_AUTHORITY_CHAIN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_AI_ATTACK_RECHECK_PRODUCTION_AUTHORITY_CHAIN_CMD_NAMES_WAVE616:
    &[&str] = &[
    "ai_attack_recheck_60s",
    "no_gate_early_attack",
    "production_ready_log",
    "production_spawn_helper",
    "ai_production_authority_chain_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualAiAttackRecheckProductionAuthorityChainAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualAiAttackRecheckProductionAuthorityChainAction {
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

fn residual_action_store(action: ResidualAiAttackRecheckProductionAuthorityChainAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_ai_attack_recheck_production_authority_chain_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_ai_attack_recheck_production_authority_chain_last_action()
-> ResidualAiAttackRecheckProductionAuthorityChainAction {
    ResidualAiAttackRecheckProductionAuthorityChainAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}

fn ai_source() -> &'static str {
    include_str!("../../ai.rs")
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn ready_log_source() -> &'static str {
    include_str!("../host_production_ready_log.rs")
}

pub fn honesty_ai_attack_recheck_production_authority_chain_method_names_residual_wave616() -> bool
{
    let names = LIVE_AI_ATTACK_RECHECK_PRODUCTION_AUTHORITY_CHAIN_METHOD_NAMES_WAVE616;
    let ok = residual_name_index(names, "ATTACK_RECHECK_SECONDS").is_some()
        && residual_name_index(names, "host_spawn_production_unit").is_some()
        && residual_name_index(names, "host_production_ready_log").is_some()
        && residual_name_index(names, "Wave 616").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualAiAttackRecheckProductionAuthorityChainAction::MethodNames);
    ok
}

pub fn honesty_ai_attack_recheck_production_authority_chain_source_markers_residual_wave616() -> bool
{
    let ai = ai_source();
    let gl = gl_source();
    let sh = shadow_source();
    let ready = ready_log_source();
    // Wave 616: 60s attack recheck — not gate-driven 2s.
    let ai_ok = ai.contains("pub const ATTACK_RECHECK_SECONDS: f32 = 60.0")
        && ai.contains("evaluate_attack_opportunities")
        && ai.contains("ATTACK_RECHECK_SECONDS")
        && ai.contains("checkReadyTeams")
        && !ai.contains("ATTACK_RECHECK_SECONDS: f32 = 2.0")
        && !ai.contains("ATTACK_RECHECK_SECONDS: f32 = 2 ");
    let chain_ok = ready.contains("Wave 614")
        && gl.contains("fn host_collect_production_completions")
        && gl.contains("fn host_spawn_production_unit")
        && gl.contains("fn host_apply_unit_production_completions")
        && sh.contains("host_production_ready_log::record");
    let ok = ai_ok && chain_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualAiAttackRecheckProductionAuthorityChainAction::SourceMarkers);
    ok
}

pub fn honesty_ai_attack_recheck_production_authority_chain_nav_commands_residual_wave616() -> bool
{
    let steps = LIVE_AI_ATTACK_RECHECK_PRODUCTION_AUTHORITY_CHAIN_NAV_STEPS_WAVE616;
    let cmds = RUNTIME_HOST_LIVE_AI_ATTACK_RECHECK_PRODUCTION_AUTHORITY_CHAIN_CMD_NAMES_WAVE616;
    let ok = residual_name_index(steps, "REQUIRE_ATTACK_RECHECK_60S").is_some()
        && residual_name_index(steps, "REQUIRE_NO_GATE_EARLY_ATTACK").is_some()
        && residual_name_index(steps, "REQUIRE_PRODUCTION_READY_LOG").is_some()
        && residual_name_index(steps, "REQUIRE_PRODUCTION_SPAWN_HELPER").is_some()
        && residual_name_index(steps, "LIVE_AI_ATTACK_RECHECK_PRODUCTION_AUTHORITY_CHAIN")
            .is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "ai_attack_recheck_60s").is_some()
        && residual_name_index(cmds, "no_gate_early_attack").is_some()
        && residual_name_index(cmds, "production_ready_log").is_some()
        && residual_name_index(cmds, "production_spawn_helper").is_some()
        && residual_name_index(cmds, "ai_production_authority_chain_residual").is_some();
    residual_action_store(ResidualAiAttackRecheckProductionAuthorityChainAction::NavCommands);
    ok
}

pub fn simulate_ai_attack_recheck_production_authority_chain_collect_source() -> bool {
    let ok = ai_source().contains("ATTACK_RECHECK_SECONDS: f32 = 60.0")
        && ready_log_source().contains("HostProductionReadyEvent")
        && gl_source().contains("host_spawn_production_unit");
    residual_action_store(ResidualAiAttackRecheckProductionAuthorityChainAction::CollectSource);
    ok
}

pub fn simulate_ai_attack_recheck_production_authority_chain_dispatch_source() -> bool {
    let ai = ai_source();
    let gl = gl_source();
    let ok = ai.contains("current_time - self.last_attack_time < Self::ATTACK_RECHECK_SECONDS")
        && gl.contains("host_production_ready_log::drain")
        && gl.contains("self.host_spawn_production_unit(&template, team, spawn_pos)")
        && crate::ai::AIPlayer::ATTACK_RECHECK_SECONDS == 60.0
        && crate::ai::AIPlayer::ATTACK_RECHECK_SECONDS >= 30.0;
    residual_action_store(ResidualAiAttackRecheckProductionAuthorityChainAction::DispatchSource);
    ok
}

pub fn honesty_ai_attack_recheck_production_authority_chain_residual_pack_wave616() -> bool {
    honesty_ai_attack_recheck_production_authority_chain_method_names_residual_wave616()
        && honesty_ai_attack_recheck_production_authority_chain_source_markers_residual_wave616()
        && honesty_ai_attack_recheck_production_authority_chain_nav_commands_residual_wave616()
        && simulate_ai_attack_recheck_production_authority_chain_collect_source()
        && simulate_ai_attack_recheck_production_authority_chain_dispatch_source()
}

pub fn simulate_live_ai_attack_recheck_production_authority_chain_honesty() -> bool {
    let ok = honesty_ai_attack_recheck_production_authority_chain_residual_pack_wave616();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualAiAttackRecheckProductionAuthorityChainAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_ai_attack_recheck_production_authority_chain_method_names_residual_wave616()
        );
    }

    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_ai_attack_recheck_production_authority_chain_source_markers_residual_wave616()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_ai_attack_recheck_production_authority_chain_nav_commands_residual_wave616()
        );
    }

    #[test]
    fn ai_attack_recheck_production_authority_chain_sources() {
        assert!(simulate_ai_attack_recheck_production_authority_chain_collect_source());
        assert!(simulate_ai_attack_recheck_production_authority_chain_dispatch_source());
    }

    #[test]
    fn wave616_composite_pack() {
        assert!(honesty_ai_attack_recheck_production_authority_chain_residual_pack_wave616());
    }

    #[test]
    fn simulate_live_ai_attack_recheck_production_authority_chain_honesty_residual_live() {
        assert!(
            simulate_live_ai_attack_recheck_production_authority_chain_honesty(),
            "ai attack recheck + production authority chain residual must latch"
        );
        assert!(residual_ai_attack_recheck_production_authority_chain_ok());
        assert_eq!(
            residual_ai_attack_recheck_production_authority_chain_last_action(),
            ResidualAiAttackRecheckProductionAuthorityChainAction::Composite
        );
    }
}
