//! Host-sole gate parity under a coupled GameWorld shadow session.
//!
//! Live default: shadow ON + coupled tick, every authority channel OFF
//! (C++ parity: one store, one tick). Before the host-sole gate fix,
//! `update_simulation` skipped these host residuals whenever the shadow was
//! coupled, while the GameWorld side of each channel drained only under
//! movement authority (off) — so the channel silently stalled live.
//!
//! These tests drive the FULL host tick inside the coupled window, mirroring
//! the engine frame (host logic inside the scoped authority window → eager
//! host→GW residual batch → `shadow_session_after_host_tick`), and assert the
//! host-side observable still happens with all authorities off.

use super::*;
use crate::gameworld_shadow::{
    begin_shadow_coupled_tick, clear_active_shadow_for_coupled_tick,
    eager_apply_all_host_residuals_after_logic, end_shadow_coupled_tick,
    install_active_shadow_for_coupled_tick, shadow_session_after_host_tick,
    with_gameworld_authority, GameWorldShadow,
};

/// Serialize against tests that mutate `GENERALS_GAMEWORLD_*` env and pin the
/// shadow gate on for the duration (prior value restored on drop).
struct ShadowEnvPin {
    prior: Option<String>,
    _lock: std::sync::MutexGuard<'static, ()>,
}

impl ShadowEnvPin {
    fn new() -> Self {
        let lock = crate::gameworld_shadow::authority_env_lock();
        let prior = std::env::var("GENERALS_GAMEWORLD_SHADOW").ok();
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
        assert!(
            crate::gameworld_shadow::gameworld_shadow_enabled(),
            "shadow gate must be on for the coupled-parity tests"
        );
        Self { prior, _lock: lock }
    }
}

impl Drop for ShadowEnvPin {
    fn drop(&mut self) {
        match &self.prior {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    }
}

/// One engine-parity logic frame with the shadow coupled: the host tick runs
/// inside `begin/end_shadow_coupled_tick` with the instance's authority
/// context published (all channels off in these tests), then the post-logic
/// eager residual batch and the shadow session boundary run exactly like
/// `host_run_coupled_fast_forward_loop`.
fn coupled_frame(shadow: &mut GameWorldShadow, logic: &mut GameLogic) {
    begin_shadow_coupled_tick();
    install_active_shadow_for_coupled_tick(shadow);
    let _ = with_gameworld_authority(*logic.gameworld_authority(), || {
        logic.tick_logic_frame(LOGIC_FRAME_TIMESTEP, None, None);
    });
    eager_apply_all_host_residuals_after_logic(shadow, logic);
    let _probe = shadow_session_after_host_tick(shadow, logic);
    clear_active_shadow_for_coupled_tick();
    end_shadow_coupled_tick();
}

/// Wave 780 / Fix A: a damaged BaseRegenerateUpdate structure must self-repair
/// on the HOST tick while the shadow is coupled and every authority is off,
/// and the shadow health probe must stay green (no GW dual-peel heal re-apply).
#[test]
fn coupled_shadow_base_regen_heals_on_host_and_probe_stays_green() {
    let _env = ShadowEnvPin::new();

    let mut logic = GameLogic::new();
    let mut tpl = ThingTemplate::new("AmericaCommandCenter");
    tpl.add_kind_of(KindOf::Structure).set_health(1000.0);
    logic
        .templates
        .insert("AmericaCommandCenter".to_string(), tpl);
    let id = logic
        .create_object("AmericaCommandCenter", Team::USA, glam::Vec3::ZERO)
        .expect("cc");
    assert!(logic.host_object(id).unwrap().base_regenerate.is_some());

    {
        let o = logic.host_object_mut(id).unwrap();
        o.health.current = 500.0;
        o.status.under_construction = false;
    }
    logic.set_current_frame(0);
    logic.notify_base_regenerate_damage(id, false);

    let mut shadow = GameWorldShadow::new(64);
    // 95 coupled frames cross the 90-frame damage-delay wake (heal rate: 3).
    for _ in 0..95 {
        coupled_frame(&mut shadow, &mut logic);
    }
    let hp = logic.host_object(id).unwrap().health.current;
    assert!(
        hp > 500.0,
        "structure must self-repair on the host tick while the shadow is coupled (hp={hp})"
    );
    assert!(logic.base_regenerate_reg.heal_ticks >= 1);
    let probe = shadow.probe(&mut logic);
    assert!(
        probe.health_match,
        "shadow health probe must stay green after host-sole heal: {}",
        probe.detail
    );
}

/// Wave 817 (lifetime expiry representative): a crate DeletionUpdate lifetime
/// must expire on the HOST tick while the shadow is coupled.
#[test]
fn coupled_shadow_crate_lifetime_expires_on_host() {
    let _env = ShadowEnvPin::new();

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("SalvageCrate");
    t.set_health(50.0);
    logic.templates.insert("SalvageCrate".into(), t);
    let cid = logic
        .create_object("SalvageCrate", Team::Neutral, glam::Vec3::ZERO)
        .expect("crate");
    logic.host_money_crates.register_salvage_crate(cid, 40);
    // Force expires_frame via arm with min=max=1 from frame 0.
    logic.frame = 0;
    logic.host_money_crates.arm_deletion_update(cid, 0, 1, 1, 0);
    assert_eq!(logic.host_money_crates.get(cid).unwrap().expires_frame, 1);

    let mut shadow = GameWorldShadow::new(64);
    for _ in 0..4 {
        coupled_frame(&mut shadow, &mut logic);
    }
    assert!(
        !logic.host_money_crates.contains(cid),
        "crate DeletionUpdate lifetime must expire on the host tick while coupled"
    );
    assert!(
        logic.objects.get(&cid).is_none(),
        "expired crate must be destroyed on the host while coupled"
    );
}

/// Wave 820 (fire spread representative): an ignited flammable must run the
/// fire-spread pass on the HOST tick while the shadow is coupled.
#[test]
fn coupled_shadow_fire_spread_runs_on_host() {
    let _env = ShadowEnvPin::new();

    let mut logic = GameLogic::new();
    for name in ["DogwoodTreeA", "DogwoodTreeB"] {
        let mut tpl = ThingTemplate::new(name);
        tpl.set_health(50.0);
        logic.templates.insert(name.to_string(), tpl);
    }
    let a = logic
        .create_object("DogwoodTreeA", Team::Neutral, glam::Vec3::ZERO)
        .expect("tree a");
    let b = logic
        .create_object(
            "DogwoodTreeB",
            Team::Neutral,
            glam::Vec3::new(
                crate::game_logic::host_fire_spread::TREE_SPREAD_TRY_RANGE * 0.5,
                0.0,
                0.0,
            ),
        )
        .expect("tree b");
    assert!(logic.host_object(a).unwrap().has_fire_spread());
    assert!(logic.host_object(b).unwrap().has_fire_spread());

    assert!(logic.ignite_object_fire_spread(a));
    {
        // Force the first spread attempt due immediately.
        let o = logic.host_object_mut(a).unwrap();
        if let Some(fs) = o.fire_spread.as_mut() {
            fs.next_spread_frame = 0;
        }
    }

    let mut shadow = GameWorldShadow::new(64);
    // Cross frame 30 (the direct-call precedent's spread-due frame).
    for _ in 0..35 {
        coupled_frame(&mut shadow, &mut logic);
    }
    assert!(
        logic.fire_spread_reg.spreads > 0,
        "aflame tree must attempt spread on the host tick while coupled"
    );
    assert!(
        logic
            .host_object(b)
            .and_then(|o| o.fire_spread.as_ref().map(|f| f.is_aflame()))
            .unwrap_or(false)
            || logic.fire_spread_reg.ignitions >= 2,
        "neighbor within range must ignite on the host tick while coupled"
    );
}

/// Wave 825 (scud poison representative): a MediumPoisonField zone spawned by
/// a toxin SCUD warhead must tick its DoT on the HOST while the shadow is
/// coupled.
#[test]
fn coupled_shadow_scud_poison_dot_ticks_on_host() {
    let _env = ShadowEnvPin::new();

    let mut logic = GameLogic::new();
    let mut t = ThingTemplate::new("PoisonSponge");
    t.set_health(5000.0);
    t.add_kind_of(KindOf::Infantry);
    logic.templates.insert("PoisonSponge".into(), t);
    let inf = logic
        .create_object("PoisonSponge", Team::USA, glam::Vec3::ZERO)
        .expect("infantry");

    // Detonate a toxin SCUD warhead on the infantry: spawns the
    // MediumPoisonField DoT zone (host residual registry).
    let _ = logic.apply_scud_area_at(glam::Vec3::ZERO, None, Team::GLA, true);
    assert!(
        logic.scud_poison_zones().active_count() >= 1,
        "toxin blast must spawn the poison field zone"
    );
    let after_blast = logic.host_object(inf).unwrap().health.current;
    assert!(
        (after_blast - 5000.0).abs() > 0.01,
        "blast itself must land (hp={after_blast})"
    );

    let mut shadow = GameWorldShadow::new(64);
    // Zone ticks roughly every 15 frames after activation.
    for _ in 0..50 {
        coupled_frame(&mut shadow, &mut logic);
    }
    let after = logic.host_object(inf).unwrap().health.current;
    assert!(
        after < after_blast,
        "poison field DoT must tick on the host while coupled (blast={after_blast} after={after})"
    );
}
