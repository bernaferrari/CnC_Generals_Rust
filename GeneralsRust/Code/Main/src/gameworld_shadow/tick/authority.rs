//! GameWorld authority gates (enabled / live / sole-tick) and gate helpers.
//!
//! Authority decisions are GameLogic context fields (`GameWorldAuthority`,
//! hq-e84zk) — the retired `GENERALS_GAMEWORLD_*_AUTHORITY` env flags are
//! process-global by nature and let tests re-author another instance. Deep
//! readers resolve through the current instance's thread-local snapshot,
//! published for the duration of the operation that needs it via
//! [`with_gameworld_authority`] (save/restore, unwind-safe).

use super::*;

use crate::game_logic::game_logic::gameworld_authority::{
    current_gameworld_authority, publish_gameworld_authority, GameWorldAuthority,
};

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

/// Publish `authority` as this thread's GameLogic authority context for the
/// duration of `f`, restoring the previously published snapshot afterwards
/// (normal return or unwind).
///
/// Scoped publication is the sanctioned seam between the per-instance
/// `GameLogic` authority fields and the deep thread-local readers below: a
/// tick (or any authority-consulting operation) opens a window for exactly the
/// instance that drives it. A nested window — another instance being
/// constructed or configured inside `f` — cannot leak out, and nothing
/// published here outlives `f`. The restore guard lives in this frame and is
/// private, so callers cannot drop or `mem::forget` their way past it.
#[inline]
pub(crate) fn with_gameworld_authority<R>(
    authority: GameWorldAuthority,
    f: impl FnOnce() -> R,
) -> R {
    /// Captured previous snapshot; restores exactly once, even on unwind.
    struct RestorePrevious(GameWorldAuthority);

    impl Drop for RestorePrevious {
        fn drop(&mut self) {
            publish_gameworld_authority(self.0);
        }
    }

    let _restore = RestorePrevious(current_gameworld_authority());
    publish_gameworld_authority(authority);
    f()
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

#[cfg(test)]
mod scoped_publication_tests {
    use super::*;

    use crate::game_logic::ObjectId;
    use crate::game_logic::GameLogic;
    use crate::game_logic::combat::{self, DamageType, PendingProjectile};
    use crate::game_logic::host_fire_spawn_log;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::gameworld_shadow::GameWorldShadow;

    fn authority_with(damage: bool, production: bool) -> GameWorldAuthority {
        GameWorldAuthority {
            damage,
            production,
            ..GameWorldAuthority::DEFAULT_OFF
        }
    }

    /// (a) Save/restore discipline: a scoped tick observes its own instance
    /// even after a foreign instance published over the ambient snapshot, and
    /// a nested publication (another instance's construction/configuration)
    /// cannot leak out of its own operation.
    #[test]
    fn nested_publication_does_not_leak_and_scoped_read_observes_own_instance() {
        let base = current_gameworld_authority();
        let a = authority_with(true, false);
        let b = authority_with(false, true);

        // Legacy writer shape (still in place outside this module): setters
        // and constructors publish globally. The operations that consult the
        // gates — the host logic frame (cnc_game_engine
        // host_update_logic_frame) and the post-logic eager batch — now open
        // their own scoped windows, so a later writer cannot re-author them.
        // Simulate A authoring itself, then B — constructed/configured later
        // — overwriting the thread snapshot.
        publish_gameworld_authority(a);
        publish_gameworld_authority(b);
        assert!(gameworld_production_authority_enabled());
        assert!(!gameworld_damage_authority_enabled());

        // A's tick entry opens a scoped window: its deep readers observe A,
        // not B's leftover snapshot.
        with_gameworld_authority(a, || {
            assert!(gameworld_damage_authority_enabled());
            assert!(!gameworld_production_authority_enabled());

            // B's construction/configuration nested inside A's operation:
            // within B's own window B observes itself...
            with_gameworld_authority(b, || {
                assert!(gameworld_production_authority_enabled());
                assert!(!gameworld_damage_authority_enabled());
            });

            // ...but A's readers observe A again once B's window closed.
            assert!(gameworld_damage_authority_enabled());
            assert!(!gameworld_production_authority_enabled());
        });

        // Nothing leaked past A's window either: restore puts back exactly
        // what was published before the window opened.
        assert_eq!(current_gameworld_authority(), b);
        publish_gameworld_authority(base);
    }

    /// (b) Unwinding inside a scoped publication (including a nested one)
    /// restores the previous snapshot.
    #[test]
    fn unwind_inside_scoped_publication_restores_previous_snapshot() {
        let base = current_gameworld_authority();
        let a = authority_with(true, false);
        let b = authority_with(false, true);

        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_gameworld_authority(a, || {
                with_gameworld_authority(b, || {
                    panic!("scoped authority unwind probe");
                });
            });
        }));

        assert!(panicked.is_err());
        // Both windows unwound through: the thread snapshot is the pre-test one.
        assert_eq!(current_gameworld_authority(), base);
    }

    /// (c) Two sequential scoped ticks with different authority values each
    /// observe their own value, and neither outlives its tick.
    #[test]
    fn sequential_scoped_ticks_each_observe_their_own_authority() {
        let base = current_gameworld_authority();

        let mut observed = Vec::new();
        for authority in [authority_with(true, false), authority_with(false, true)] {
            with_gameworld_authority(authority, || {
                observed.push((
                    gameworld_damage_authority_enabled(),
                    gameworld_production_authority_enabled(),
                ));
            });
        }

        assert_eq!(observed, vec![(true, false), (false, true)]);
        assert_eq!(current_gameworld_authority(), base);
    }

    /// The engine's post-logic residual batch (the in-scope tick seam) resolves
    /// deep-reader gates against the DRIVING instance's authority — not the
    /// ambient snapshot a foreign instance's construction/configuration left
    /// behind — and the batch's publication does not outlive the batch.
    #[test]
    fn eager_residual_batch_is_scoped_to_the_driving_instance() {
        let _env_guard = authority_env_lock();
        let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

        // The driving instance opts into fire-spawn authority; a foreign
        // instance's later construction/config wipes the ambient snapshot.
        let mut logic = GameLogic::new();
        logic.set_fire_spawn_authority(true);
        publish_gameworld_authority(GameWorldAuthority::DEFAULT_OFF);
        assert!(!gameworld_fire_spawn_authority_enabled());

        host_fire_spawn_log::clear();
        combat::clear_pending_projectile_queue_for_test();
        host_fire_spawn_log::record(PendingProjectile {
            shooter_id: ObjectId(1),
            shooter_pos: glam::Vec3::ZERO,
            source_context: None,
            target_id: Some(ObjectId(2)),
            target_pos: Some(glam::Vec3::new(50.0, 0.0, 0.0)),
            damage: 12.0,
            speed: 100.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Bullet,
            death_type: HostDeathType::Normal,
            projectile_object_name: "TestMissile".to_string(),
            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: 0,
            projectile_collides: 0,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        });

        let mut shadow = GameWorldShadow::new(64);
        begin_shadow_coupled_tick();
        eager_apply_all_host_residuals_after_logic(&mut shadow, &mut logic);
        end_shadow_coupled_tick();

        // The batch consulted the driving instance's scoped snapshot
        // (fire-spawn gate open) and drained the residual; without scoped
        // publication the gate would read the ambient off snapshot and leave
        // the log intact.
        assert_eq!(
            host_fire_spawn_log::len(),
            0,
            "eager batch must resolve gates against the driving instance's scoped authority"
        );
        // The batch's publication did not leak past its operation.
        assert!(!gameworld_fire_spawn_authority_enabled());

        host_fire_spawn_log::clear();
        combat::clear_pending_projectile_queue_for_test();
        match prev_shadow {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
    }

    /// The host logic frame opens the same scoped window around its
    /// GameLogic tick (cnc_game_engine `host_update_logic_frame`): a foreign
    /// instance constructed mid-session publishes its DEFAULT_OFF
    /// fresh-instance barrier over the thread snapshot (game_logic
    /// construct.rs), yet the driving world's deep readers resolve its own
    /// authority inside the frame, and the foreign snapshot is restored after.
    #[test]
    fn host_frame_window_resolves_driving_instance_after_foreign_construction() {
        let _env_guard = authority_env_lock();
        let prev_shadow = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

        let base = current_gameworld_authority();

        // World A opts into damage authority (the setter still publishes
        // globally — legacy writer shape).
        let mut a = GameLogic::new();
        a.set_damage_authority(true);
        assert!(a.gameworld_authority().damage);

        // World B constructed later overwrites the thread snapshot with its
        // default-off fresh-instance barrier.
        let _b = GameLogic::new();
        assert_eq!(current_gameworld_authority(), GameWorldAuthority::DEFAULT_OFF);
        assert!(!gameworld_damage_authority_enabled());

        // A's host frame window: deep readers resolve A, not B's leftover —
        // including the coupled live gate while a shadow session runs.
        begin_shadow_coupled_tick();
        with_gameworld_authority(*a.gameworld_authority(), || {
            assert_eq!(current_gameworld_authority(), *a.gameworld_authority());
            assert!(gameworld_damage_authority_enabled());
            assert!(gameworld_damage_authority_live());
        });
        end_shadow_coupled_tick();

        // Window closed: the foreign world's published snapshot is back.
        assert_eq!(current_gameworld_authority(), GameWorldAuthority::DEFAULT_OFF);
        assert!(!gameworld_damage_authority_enabled());

        publish_gameworld_authority(base);
        match prev_shadow {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
    }
}
