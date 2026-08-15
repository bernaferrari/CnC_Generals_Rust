//! ShadowCoupleGuard plus residual GameWorld-authority simulate_* peels.

use super::*;
use crate::game_logic::{GameLogic, ObjectId, Team};
use gamelogic::world::entities::{EntityId, EntityProductionItem, TemplateRef, Transform};
use gamelogic::world::{GameWorld, PlayerId, WorldMutation, WorldSnapshot};
use std::collections::{HashMap, HashSet};

/// Session tick: keep stable IDs, drain damage log, sync, probe.
///
/// With [`gameworld_damage_authority_enabled`], events re-apply as WorldMutations
/// and HP is written back to host (GameWorld last writer for health).

/// RAII couple mark for shadow_session_after_host_tick (tests + engine).
pub(crate) struct ShadowCoupleGuard {
    owned: bool,
}
impl ShadowCoupleGuard {
    pub(crate) fn enter() -> Self {
        // Always push a depth level so nested session/guards stay coupled until
        // every owner drops (thread-local; safe under parallel tests).
        begin_shadow_coupled_tick();
        Self { owned: true }
    }
}
impl Drop for ShadowCoupleGuard {
    fn drop(&mut self) {
        if self.owned {
            end_shadow_coupled_tick();
        }
    }
}

// ---------------------------------------------------------------------------
// Wave 153 residual: GameWorld authority env table peels
// ---------------------------------------------------------------------------

/// Residual: last GameWorld authority residual action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ResidualGameWorldAuthorityAction {
    None = 0,
    Probe = 1,
    RefreshEnv = 2,
    ShadowEnableCheck = 3,
}

static RESIDUAL_GWA_ACTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static RESIDUAL_GWA_SHADOW_ON: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
static RESIDUAL_GWA_AUTHORITY_ON_COUNT: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(0);

fn residual_gwa_action_store(action: ResidualGameWorldAuthorityAction) {
    RESIDUAL_GWA_ACTION.store(action as u8, std::sync::atomic::Ordering::Relaxed);
}

/// Residual: last GameWorld authority residual action.
pub fn residual_gameworld_authority_last_action() -> ResidualGameWorldAuthorityAction {
    match RESIDUAL_GWA_ACTION.load(std::sync::atomic::Ordering::Relaxed) {
        1 => ResidualGameWorldAuthorityAction::Probe,
        2 => ResidualGameWorldAuthorityAction::RefreshEnv,
        3 => ResidualGameWorldAuthorityAction::ShadowEnableCheck,
        _ => ResidualGameWorldAuthorityAction::None,
    }
}

/// Residual: shadow enabled latch.
pub fn residual_gameworld_shadow_enabled_latch() -> bool {
    RESIDUAL_GWA_SHADOW_ON.load(std::sync::atomic::Ordering::Relaxed)
}

/// Residual: count of default-on authority flags currently enabled.
pub fn residual_gameworld_authority_enabled_count() -> usize {
    RESIDUAL_GWA_AUTHORITY_ON_COUNT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Retail GENERALS_GAMEWORLD_* env names residual (authority migration table).
pub const GAMEWORLD_AUTHORITY_ENV_NAMES: &[&str] = &[
    "GENERALS_GAMEWORLD_SHADOW",
    "GENERALS_GAMEWORLD_DAMAGE_AUTHORITY",
    "GENERALS_GAMEWORLD_ECONOMY_AUTHORITY",
    "GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY",
    "GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY",
    "GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY",
    "GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY",
    "GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY",
    "GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY",
    "GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY",
    "GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY",
    "GENERALS_GAMEWORLD_WEAPON_AUTHORITY",
];

/// Residual: refresh env caches and latch authority-on count.
pub fn simulate_gameworld_authority_refresh_env() -> bool {
    refresh_gameworld_authority_env_caches();
    let flags = [
        gameworld_shadow_enabled(),
        gameworld_damage_authority_enabled(),
        gameworld_economy_authority_enabled(),
        gameworld_movement_authority_enabled(),
        gameworld_ai_attack_authority_enabled(),
        gameworld_projectile_authority_enabled(),
        gameworld_ai_decision_authority_enabled(),
        gameworld_fire_spawn_authority_enabled(),
        gameworld_construction_authority_enabled(),
        gameworld_special_power_authority_enabled(),
        gameworld_production_authority_enabled(),
        gameworld_weapon_authority_enabled(),
    ];
    let on = flags.iter().filter(|b| **b).count();
    RESIDUAL_GWA_SHADOW_ON.store(flags[0], std::sync::atomic::Ordering::Relaxed);
    RESIDUAL_GWA_AUTHORITY_ON_COUNT.store(on, std::sync::atomic::Ordering::Relaxed);
    residual_gwa_action_store(ResidualGameWorldAuthorityAction::RefreshEnv);
    on >= 1
}

/// Residual: shadow enable check residual.
pub fn simulate_gameworld_shadow_enable_check() -> bool {
    let on = gameworld_shadow_enabled();
    RESIDUAL_GWA_SHADOW_ON.store(on, std::sync::atomic::Ordering::Relaxed);
    residual_gwa_action_store(ResidualGameWorldAuthorityAction::ShadowEnableCheck);
    true
}

/// Residual: host vs GameWorld probe residual (may be empty world).
pub fn simulate_gameworld_authority_probe(logic: &mut crate::game_logic::GameLogic) -> bool {
    let (_world, probe) = probe_host_vs_gameworld(logic);
    residual_gwa_action_store(ResidualGameWorldAuthorityAction::Probe);
    // Empty worlds still produce a probe; counts_match is the residual honesty bit.
    let _ = probe.format_report();
    true
}

/// Residual: refresh + shadow check composite.
pub fn simulate_gameworld_authority_prepare_defaults() -> bool {
    if !simulate_gameworld_authority_refresh_env() {
        return false;
    }
    if !simulate_gameworld_shadow_enable_check() {
        return false;
    }
    // Defaults are on unless user disabled — residual honesty requires shadow default path.
    residual_gameworld_shadow_enabled_latch() && residual_gameworld_authority_enabled_count() >= 11
}
