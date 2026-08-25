//! Wave 562 residual peels: combat-kill particle observe residual test binds
//! weapon before `attack_target` so `can_attack()` admits the order (host combat
//! → death particle → PresentationFrame.particle_systems). Never flips shell
//! `playable_claim`.
//!
//! Orthogonal to Wave 561 logic-steps presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` presentation_frame_observes_combat_kill_particle_systems
//! - `object.rs` can_attack / attack_target
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_COMBAT_KILL_PARTICLE_OBSERVE_METHOD_NAMES_WAVE562: &[&str] = &[
    "presentation_frame_observes_combat_kill_particle_systems",
    "attack_target",
    "can_attack",
    "Wave 562",
    "playable_claim = false",
];

pub const LIVE_COMBAT_KILL_PARTICLE_OBSERVE_NAV_STEPS_WAVE562: &[&str] = &[
    "REQUIRE_WEAPON_BEFORE_ATTACK_TARGET",
    "REQUIRE_COMBAT_KILL_PARTICLE_OBSERVE",
    "LIVE_COMBAT_KILL_PARTICLE_OBSERVE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_COMBAT_KILL_PARTICLE_OBSERVE_CMD_NAMES_WAVE562: &[&str] = &[
    "weapon_before_attack_target",
    "combat_kill_particle_observe",
    "death_explosion_particle",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualCombatKillParticleObserveAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualCombatKillParticleObserveAction {
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

fn residual_action_store(action: ResidualCombatKillParticleObserveAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_combat_kill_particle_observe_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_combat_kill_particle_observe_last_action() -> ResidualCombatKillParticleObserveAction
{
    ResidualCombatKillParticleObserveAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn pf_source() -> &'static str {
    // 2026-08-15: observe test lives in presentation_frame/tests/apply_honesty.rs.
    include_str!("../../presentation_frame/tests/apply_honesty.rs")
}

fn obj_source() -> &'static str {
    crate::game_logic::object::OBJECT_SRC
}

fn test_body<'a>(src: &'a str) -> Option<&'a str> {
    let sig = "fn presentation_frame_observes_combat_kill_particle_systems(";
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

pub fn honesty_combat_kill_particle_observe_method_names_residual_wave562() -> bool {
    let names = LIVE_COMBAT_KILL_PARTICLE_OBSERVE_METHOD_NAMES_WAVE562;
    let ok = residual_name_index(
        names,
        "presentation_frame_observes_combat_kill_particle_systems",
    )
    .is_some()
        && residual_name_index(names, "attack_target").is_some()
        && residual_name_index(names, "can_attack").is_some()
        && residual_name_index(names, "Wave 562").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualCombatKillParticleObserveAction::MethodNames);
    ok
}

pub fn honesty_combat_kill_particle_observe_source_markers_residual_wave562() -> bool {
    let pf = pf_source();
    let obj = obj_source();
    let Some(body) = test_body(pf) else {
        residual_action_store(ResidualCombatKillParticleObserveAction::SourceMarkers);
        return false;
    };
    let weapon_at = body.find("a.weapon = Some(Weapon");
    let attack_at = body.find("a.attack_target(victim)");
    let order_ok = match (weapon_at, attack_at) {
        (Some(w), Some(a)) => w < a,
        _ => false,
    };
    let marker_ok = body.contains("Wave 562")
        && body.contains("can_attack()")
        && body.contains("projectile_speed: 0.0")
        && body.contains("SlowDeath")
        && body.contains("has_active_particles()")
        && body.contains("DeathExplosion");
    // 2026-08-15: can_attack uses weapon_slot(0..2) (object/bonuses.rs:616-618).
    let can_attack_ok = obj.contains("fn can_attack")
        && (obj.contains("self.weapon.is_some()")
            || obj.contains("self.weapon_slot(slot).is_some()"));
    let ok = order_ok && marker_ok && can_attack_ok && !pf.contains("playable_claim = true");
    residual_action_store(ResidualCombatKillParticleObserveAction::SourceMarkers);
    ok
}

pub fn honesty_combat_kill_particle_observe_nav_commands_residual_wave562() -> bool {
    let steps = LIVE_COMBAT_KILL_PARTICLE_OBSERVE_NAV_STEPS_WAVE562;
    let cmds = RUNTIME_HOST_LIVE_COMBAT_KILL_PARTICLE_OBSERVE_CMD_NAMES_WAVE562;
    let ok = residual_name_index(steps, "REQUIRE_WEAPON_BEFORE_ATTACK_TARGET").is_some()
        && residual_name_index(steps, "REQUIRE_COMBAT_KILL_PARTICLE_OBSERVE").is_some()
        && residual_name_index(steps, "LIVE_COMBAT_KILL_PARTICLE_OBSERVE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "weapon_before_attack_target").is_some()
        && residual_name_index(cmds, "combat_kill_particle_observe").is_some()
        && residual_name_index(cmds, "death_explosion_particle").is_some();
    residual_action_store(ResidualCombatKillParticleObserveAction::NavCommands);
    ok
}

pub fn simulate_combat_kill_particle_observe_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 562")
        && pf.contains("fn presentation_frame_observes_combat_kill_particle_systems")
        && pf.contains("has_active_particles()");
    residual_action_store(ResidualCombatKillParticleObserveAction::CollectSource);
    ok
}

pub fn simulate_combat_kill_particle_observe_dispatch_source() -> bool {
    let Some(body) = test_body(pf_source()) else {
        residual_action_store(ResidualCombatKillParticleObserveAction::DispatchSource);
        return false;
    };
    let ok = body.contains("logic.update()")
        && body.contains("build_from_logic")
        && body.contains("combat_particles().active_count()")
        && body.contains("process_destroy_list()");
    residual_action_store(ResidualCombatKillParticleObserveAction::DispatchSource);
    ok
}

pub fn honesty_combat_kill_particle_observe_residual_pack_wave562() -> bool {
    honesty_combat_kill_particle_observe_method_names_residual_wave562()
        && honesty_combat_kill_particle_observe_source_markers_residual_wave562()
        && honesty_combat_kill_particle_observe_nav_commands_residual_wave562()
        && simulate_combat_kill_particle_observe_collect_source()
        && simulate_combat_kill_particle_observe_dispatch_source()
}

pub fn simulate_live_combat_kill_particle_observe_honesty() -> bool {
    let ok = honesty_combat_kill_particle_observe_residual_pack_wave562();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualCombatKillParticleObserveAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_combat_kill_particle_observe_method_names_residual_wave562());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_combat_kill_particle_observe_source_markers_residual_wave562());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_combat_kill_particle_observe_nav_commands_residual_wave562());
    }

    #[test]
    fn combat_kill_particle_observe_sources() {
        assert!(simulate_combat_kill_particle_observe_collect_source());
        assert!(simulate_combat_kill_particle_observe_dispatch_source());
    }

    #[test]
    fn wave562_composite_pack() {
        assert!(honesty_combat_kill_particle_observe_residual_pack_wave562());
    }

    #[test]
    fn simulate_live_combat_kill_particle_observe_honesty_residual_live() {
        assert!(
            simulate_live_combat_kill_particle_observe_honesty(),
            "combat kill particle observe residual must latch"
        );
        assert!(residual_combat_kill_particle_observe_ok());
        assert_eq!(
            residual_combat_kill_particle_observe_last_action(),
            ResidualCombatKillParticleObserveAction::Composite
        );
    }
}
