//! GameWorld authority gates (enabled / live / sole-tick) and gate helpers.
//!
//! Authority decisions are GameLogic context fields (`GameWorldAuthority`,
//! hq-e84zk) — the retired `GENERALS_GAMEWORLD_*_AUTHORITY` env flags are
//! process-global by nature and let tests re-author another instance. Deep
//! readers resolve through the current instance's thread-local snapshot.

use super::*;

use crate::game_logic::game_logic::gameworld_authority::current_gameworld_authority;

static SHADOW_ENABLED_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static DEFERRED_DESTROY_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static ENTITY_MODULES_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

pub(super) fn reset_authority_env_caches() {
    for c in [
        &SHADOW_ENABLED_CACHE,
        &DEFERRED_DESTROY_CACHE,
        &ENTITY_MODULES_CACHE,
    ] {
        c.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Serializes tests (and residual harnesses) that mutate GENERALS_GAMEWORLD_* env.
#[cfg(test)]
pub(crate) fn authority_env_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn gameworld_shadow_enabled() -> bool {
    env_flag_cached(&SHADOW_ENABLED_CACHE, "GENERALS_GAMEWORLD_SHADOW", true)
}

/// When enabled, GameWorld shadow mutations are the **last writer** for HP each tick.
/// Host combat still runs mid-frame; end-of-tick reapplies drained damage events
/// on the shadow and writebacks health/destroyed onto host objects.
///
/// C++ has one `TheGameLogic` store. Default is **off** so host
/// `GameLogic` is the sole writer. Opt in with
/// `GameLogic::set_damage_authority(true)`.
pub fn gameworld_damage_authority_enabled() -> bool {
    current_gameworld_authority().damage
}

/// Damage HP defer only while shadow can writeback (alias of enabled&&shadow).
#[inline]
pub fn gameworld_damage_authority_live() -> bool {
    // Fail-open to host HP when no coupled engine shadow session is active
    // (unit tests, host-only gates). Matches construction/production sole-tick.
    gameworld_damage_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Economy last-writer (player supplies/power). Default = **off** (host sole writer).
/// Opt in with `GameLogic::set_economy_authority(true)`.
pub fn gameworld_economy_authority_enabled() -> bool {
    current_gameworld_authority().economy
}

/// Economy last-writer is only meaningful while a shadow session can write back cash.
/// Host-only matches must mutate supplies immediately (same coupling as damage/fire-spawn).
#[inline]
pub fn gameworld_economy_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_economy_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// When enabled, GameWorld integrates path/move targets after the host tick and
/// writebacks pose/movement as last-writer. Host `update_movement` skips integrate.
///
/// C++ Locomotor writes one pose on TheGameLogic. Default **off**.
/// Opt in with `GameLogic::set_movement_authority(true)`.
pub fn gameworld_movement_authority_enabled() -> bool {
    current_gameworld_authority().movement
}

/// Movement last-writer only while shadow can step/writeback poses.
#[inline]
pub fn gameworld_movement_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_movement_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld last-writer for attack target / fire-intent residual.
/// Default **off** (host sole writer). `GameLogic::set_ai_attack_authority(true)`.
pub fn gameworld_ai_attack_authority_enabled() -> bool {
    current_gameworld_authority().ai_attack
}

/// AI attack/fire-intent channel only while shadow can writeback.
#[inline]
pub fn gameworld_ai_attack_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_ai_attack_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld projectile flight last-writer. Default **off**.
pub fn gameworld_projectile_authority_enabled() -> bool {
    current_gameworld_authority().projectile
}

/// Projectile integrate defer only while shadow session steps flight.
#[inline]
pub fn gameworld_projectile_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_projectile_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld AI decision last-writer. Default **off**.
pub fn gameworld_ai_decision_authority_enabled() -> bool {
    current_gameworld_authority().ai_decision
}

/// AI decision last-writer only while shadow can apply/writeback decisions.
#[inline]
pub fn gameworld_ai_decision_authority_live() -> bool {
    // Fail-open to host AI state when no coupled engine shadow writeback frame.
    gameworld_ai_decision_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld fire-spawn last-writer. Default **off**.
pub fn gameworld_fire_spawn_authority_enabled() -> bool {
    current_gameworld_authority().fire_spawn
}

/// Fire-spawn defer only while shadow can drain spawn log.
#[inline]
pub fn gameworld_fire_spawn_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_fire_spawn_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld construction last-writer. Default **off**.
pub fn gameworld_construction_authority_enabled() -> bool {
    current_gameworld_authority().construction
}

/// Construction progress last-writer only while shadow can sole-tick percent.
#[inline]
pub fn gameworld_construction_authority_live() -> bool {
    gameworld_construction_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Host skips construction percent advance only when authority AND shadow session run.
pub fn gameworld_construction_sole_tick_enabled() -> bool {
    gameworld_construction_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld special-power last-writer. Default **off**.
pub fn gameworld_special_power_authority_enabled() -> bool {
    current_gameworld_authority().special_power
}

/// Host skips SP countdown advance only when authority AND shadow session run.
pub fn gameworld_special_power_sole_tick_enabled() -> bool {
    gameworld_special_power_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld production-queue last-writer. Default **off**.
/// Wave 464: GameWorld sole-ticks queue progress + exit delay when this is on.
pub fn gameworld_production_authority_enabled() -> bool {
    current_gameworld_authority().production
}

/// Production queue last-writer only while shadow can sole-tick progress.
#[inline]
pub fn gameworld_production_authority_live() -> bool {
    gameworld_production_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Host skips progress advance only when production authority AND shadow session run.
pub fn gameworld_production_sole_tick_enabled() -> bool {
    gameworld_production_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld weapon-slot last-writer. Default **off**.
pub fn gameworld_weapon_authority_enabled() -> bool {
    current_gameworld_authority().weapon
}

/// Opt-in GameWorld entity-module attach. Production default **off**.
pub fn gameworld_entity_modules_enabled() -> bool {
    env_flag_cached(
        &ENTITY_MODULES_CACHE,
        "GENERALS_GAMEWORLD_ENTITY_MODULES",
        false,
    )
}

#[inline]
pub fn gameworld_entity_modules_live() -> bool {
    gameworld_entity_modules_enabled() && gameworld_shadow_enabled() && shadow_coupled_tick_active()
}

#[inline]
pub fn gameworld_weapon_authority_live() -> bool {
    gameworld_weapon_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Opt-in GameWorld deferred-destroy lockstep. Production default **off**.
pub fn gameworld_deferred_destroy_enabled() -> bool {
    env_flag_cached(
        &DEFERRED_DESTROY_CACHE,
        "GENERALS_GAMEWORLD_DEFERRED_DESTROY",
        false,
    )
}

#[inline]
pub fn gameworld_deferred_destroy_live() -> bool {
    gameworld_deferred_destroy_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Refresh the default-on shadow gate cache for smoke/gate entry points.
///
/// Authority channels are GameLogic context fields (no env, hq-e84zk). The
/// remaining `GENERALS_GAMEWORLD_*` env flags (shadow / entity-modules /
/// deferred-destroy) keep process-stable caches; entry points refresh so an
/// explicit opt-out (`=0|false`) written before startup is honored.
pub fn ensure_gate_damage_authority() {
    ensure_gate_economy_authority();
    ensure_gate_production_authority();
    // Caches may have been primed before a caller changed an explicit gate.
    refresh_gameworld_authority_env_caches();
}

/// Refresh the default-on economy authority cache without mutating process env.
pub fn ensure_gate_economy_authority() {
    refresh_gameworld_authority_env_caches();
}

/// Refresh the default-on production authority cache without mutating process env.
pub fn ensure_gate_production_authority() {
    refresh_gameworld_authority_env_caches();
}
