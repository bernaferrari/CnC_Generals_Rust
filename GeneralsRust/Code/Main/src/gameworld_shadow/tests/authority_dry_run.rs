//! Guarded authority-flag dry-run. Does not change production defaults.

use super::*;
use crate::gameworld_shadow::{
    GAMEWORLD_AUTHORITY_ENV_NAMES, probe_host_vs_gameworld, shadow_session_after_host_tick,
};

#[test]
fn authority_env_guard_dry_run_keeps_probes_true() {
    let mut env = AuthorityEnvGuard::lock();
    for name in GAMEWORLD_AUTHORITY_ENV_NAMES {
        env = env.set(name, "1");
    }
    let _env = env.couple();

    let mut logic = GameLogic::new();
    let cfg = golden_skirmish_config("AuthDry");
    apply_skirmish_config(&mut logic, &cfg).expect("cfg");
    ensure_template(&mut logic, "AuthDryRanger", 80.0);
    let _id = logic
        .create_object("AuthDryRanger", Team::USA, Vec3::new(6.0, 0.0, 6.0))
        .expect("id");

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let mut blockers = Vec::new();
    for frame in 0..4 {
        logic.update_with_dt(1.0 / 30.0);
        let _ = shadow_session_after_host_tick(&mut shadow, &mut logic);
        let (_world, probe) = probe_host_vs_gameworld(&mut logic);
        if !probe.counts_match {
            blockers.push(format!("frame {frame}: counts_match"));
        }
        if !probe.economy_match {
            blockers.push(format!("frame {frame}: economy_match"));
        }
        if !probe.health_match {
            blockers.push(format!("frame {frame}: health_match"));
        }
        if !probe.pose_match {
            blockers.push(format!("frame {frame}: pose_match"));
        }
        if !probe.attack_target_match {
            blockers.push(format!("frame {frame}: attack_target_match"));
        }
        if !probe.move_target_match {
            blockers.push(format!("frame {frame}: move_target_match"));
        }
        if !probe.weapon_match {
            blockers.push(format!("frame {frame}: weapon_match"));
        }
        if !probe.contain_match {
            blockers.push(format!("frame {frame}: contain_match"));
        }
        if !probe.destroy_visibility_match {
            blockers.push(format!("frame {frame}: destroy_visibility_match"));
        }
        if !probe.production_match {
            blockers.push(format!("frame {frame}: production_match"));
        }
    }
    assert!(
        blockers.is_empty(),
        "authority dry-run flip blockers: {blockers:?}"
    );
}
