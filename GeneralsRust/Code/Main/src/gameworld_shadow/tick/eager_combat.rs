//! Post-logic combat residuals: fire-spawn, move/attack, damage/heal, projectiles, destroy.

use super::*;
use crate::game_logic::GameLogic;
use crate::gameworld_shadow::GameWorldShadow;

pub fn eager_apply_host_fire_spawns_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &mut GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_fire_spawn_authority_enabled()
    {
        return 0;
    }
    // Wave 682/712: post-logic fire-spawn materialize (exclusive shadow+logic borrows).
    let spawns = crate::game_logic::host_fire_spawn_log::drain();
    if spawns.is_empty() {
        EARLY_FIRE_SPAWN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_fire_spawn_events(logic, spawns.clone());
    EARLY_FIRE_SPAWN_BATCH.with(|c| *c.borrow_mut() = Some((spawns, true)));
    n
}

/// Wave 683: immediately after the host logic frame on a coupled tick, drain
/// `host_attack_log` / `host_move_log` into GameWorld attack/move targets.
///
/// Runs before `shadow_session_after_host_tick` so AI/path residuals see current
/// orders the same frame. Session drains stay idempotent (logs already empty).
///
/// Safe exclusive borrows: caller passes `&mut GameWorldShadow` + `&GameLogic`.
pub fn eager_apply_host_move_attack_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> (usize, usize) {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return (0, 0);
    }
    // Wave 683/712: post-logic move/attack materialize (exclusive shadow borrow).
    let attack_events = crate::game_logic::host_attack_log::drain();
    let move_events = crate::game_logic::host_move_log::drain();
    let mut attacks = 0usize;
    let mut moves = 0usize;
    for ev in &attack_events {
        if shadow.queue_set_attack_target_for_host(ev.attacker, ev.target) {
            attacks = attacks.saturating_add(1);
        }
    }
    for ev in &move_events {
        if shadow.queue_set_move_target_for_host(ev.unit, ev.destination) {
            moves = moves.saturating_add(1);
        }
    }
    // Host movement.target residual (path follow destinations not always logged).
    if gameworld_movement_authority_enabled() {
        let _ = shadow.apply_host_move_targets(logic);
    }
    if attacks > 0 || moves > 0 {
        let _ = shadow.apply_pending();
    }
    // Wave 712: handoff batches so session does not re-queue.
    if attack_events.is_empty() {
        EARLY_ATTACK_BATCH.with(|c| *c.borrow_mut() = None);
    } else {
        EARLY_ATTACK_BATCH.with(|c| *c.borrow_mut() = Some((attack_events, true)));
    }
    if move_events.is_empty() {
        EARLY_MOVE_BATCH.with(|c| *c.borrow_mut() = None);
    } else {
        EARLY_MOVE_BATCH.with(|c| *c.borrow_mut() = Some((move_events, true)));
    }
    (attacks, moves)
}

// Wave 684: post-logic damage batch handoff to shadow session (avoid double-apply).
thread_local! {
    static EARLY_DAMAGE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_damage_log::HostDamageEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

/// Take post-logic damage batch if Wave 684 already drained+applied it.
/// Returns `(events, already_applied_to_shadow)`.
pub fn take_early_damage_batch() -> Option<(
    Vec<crate::game_logic::host_damage_log::HostDamageEvent>,
    bool,
)> {
    EARLY_DAMAGE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 684: immediately after the host logic frame on a coupled tick, drain
/// `host_damage_log` into GameWorld Damage/Destroy mutations.
///
/// Stashes the batch for `shadow_session_after_host_tick` so `write_health` still
/// sees non-empty events and does not double-apply mutations.
pub fn eager_apply_host_damage_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_damage_authority_enabled()
    {
        return 0;
    }
    // Wave 684: post-logic damage materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_damage_log::drain();
    if events.is_empty() {
        EARLY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let (queued, applied) = shadow.apply_host_damage_events(&events);
    EARLY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    queued.saturating_add(applied)
}

// Wave 685: post-logic heal batch handoff to shadow session (avoid double-apply).
thread_local! {
    static EARLY_HEAL_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_heal_log::HostHealEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_heal_batch()
-> Option<(Vec<crate::game_logic::host_heal_log::HostHealEvent>, bool)> {
    EARLY_HEAL_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 685: immediately after the host logic frame on a coupled tick, drain
/// `host_heal_log` into GameWorld SetHealth mutations.
///
/// Stashes the batch for `shadow_session_after_host_tick` so `write_health` still
/// sees non-empty heals and does not double-apply mutations.
pub fn eager_apply_host_heal_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_damage_authority_enabled()
    {
        return 0;
    }
    // Wave 685: post-logic heal materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_heal_log::drain();
    if events.is_empty() {
        EARLY_HEAL_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_heal_events(&events);
    EARLY_HEAL_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 686: post-logic max-health / experience batch handoff (avoid double-apply).
thread_local! {
    static EARLY_MAX_HEALTH_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_max_health_log::HostMaxHealthEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 686: post-logic max-health / experience batch handoff (avoid double-apply).
thread_local! {
    static EARLY_EXPERIENCE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_experience_log::HostExperienceEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_max_health_batch() -> Option<(
    Vec<crate::game_logic::host_max_health_log::HostMaxHealthEvent>,
    bool,
)> {
    EARLY_MAX_HEALTH_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_experience_batch() -> Option<(
    Vec<crate::game_logic::host_experience_log::HostExperienceEvent>,
    bool,
)> {
    EARLY_EXPERIENCE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 686: post-logic drain `host_max_health_log` into GameWorld SetMaxHealth.
pub fn eager_apply_host_max_health_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_damage_authority_enabled()
    {
        return 0;
    }
    // Wave 686: post-logic max-health materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_max_health_log::drain();
    if events.is_empty() {
        EARLY_MAX_HEALTH_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_max_health_events(&events);
    EARLY_MAX_HEALTH_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 686: post-logic drain `host_experience_log` into GameWorld SetExperience.
pub fn eager_apply_host_experience_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_damage_authority_enabled()
    {
        return 0;
    }
    // Wave 686: post-logic experience materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_experience_log::drain();
    if events.is_empty() {
        EARLY_EXPERIENCE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_experience_events(&events);
    EARLY_EXPERIENCE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 710: post-logic combat-attack / projectile batch handoff.
thread_local! {
    static EARLY_COMBAT_ATTACK_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_combat_attack_log::HostCombatAttackEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 710: post-logic combat-attack / projectile batch handoff.
thread_local! {
    static EARLY_PROJECTILE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_projectile_log::HostProjectileEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_combat_attack_batch() -> Option<(
    Vec<crate::game_logic::host_combat_attack_log::HostCombatAttackEvent>,
    bool,
)> {
    EARLY_COMBAT_ATTACK_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_projectile_batch() -> Option<(
    Vec<crate::game_logic::host_projectile_log::HostProjectileEvent>,
    bool,
)> {
    EARLY_PROJECTILE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 710: post-logic drain `host_combat_attack_log` into GameWorld combat-attack state.
pub fn eager_apply_host_combat_attack_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 710: post-logic combat-attack materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_combat_attack_log::drain();
    if events.is_empty() {
        EARLY_COMBAT_ATTACK_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_combat_attack_events(&events);
    EARLY_COMBAT_ATTACK_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 710: post-logic drain `host_projectile_log` into GameWorld projectile flight.
pub fn eager_apply_host_projectile_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 710: post-logic projectile materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_projectile_log::drain();
    if events.is_empty() {
        EARLY_PROJECTILE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_projectile_events(&events);
    EARLY_PROJECTILE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

// Wave 711: post-logic destroy / contain / AI-decision batch handoff.
thread_local! {
    static EARLY_DESTROY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_destroy_log::HostDestroyEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_destroy_batch() -> Option<(
    Vec<crate::game_logic::host_destroy_log::HostDestroyEvent>,
    bool,
)> {
    EARLY_DESTROY_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 711: post-logic drain `host_destroy_log` into GameWorld destroy queue.
pub fn eager_apply_host_destroy_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 711: post-logic destroy materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_destroy_log::drain();
    if events.is_empty() {
        EARLY_DESTROY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let (queued, _) = shadow.apply_host_destroy_events(&events);
    EARLY_DESTROY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    queued
}

// Wave 712: post-logic spawn batch handoff (complements mid-frame eager_map).
thread_local! {
    static EARLY_ATTACK_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_attack_log::HostAttackEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 712: post-logic spawn batch handoff (complements mid-frame eager_map).
thread_local! {
    static EARLY_MOVE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_move_log::HostMoveEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

// Wave 712: post-logic spawn batch handoff (complements mid-frame eager_map).
thread_local! {
    static EARLY_FIRE_SPAWN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::combat::PendingProjectile>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_attack_batch() -> Option<(
    Vec<crate::game_logic::host_attack_log::HostAttackEvent>,
    bool,
)> {
    EARLY_ATTACK_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_move_batch()
-> Option<(Vec<crate::game_logic::host_move_log::HostMoveEvent>, bool)> {
    EARLY_MOVE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_fire_spawn_batch()
-> Option<(Vec<crate::game_logic::combat::PendingProjectile>, bool)> {
    EARLY_FIRE_SPAWN_BATCH.with(|c| c.borrow_mut().take())
}

/// Drop unused post-logic handoff batches when the outermost couple ends.
pub(super) fn clear_early_combat_batches() {
    EARLY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_HEAL_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_MAX_HEALTH_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_EXPERIENCE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_COMBAT_ATTACK_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_PROJECTILE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_DESTROY_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_ATTACK_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_MOVE_BATCH.with(|c| *c.borrow_mut() = None);
    EARLY_FIRE_SPAWN_BATCH.with(|c| *c.borrow_mut() = None);
}
