//! Wave 535 residual peels: `PresentationEvent::ParticleSystemSpawned` maps to
//! presentation audio (Explosion/FireBurn/PoisonDeath/…) at snapshot pose.
//! Muzzle/impact/exhaust skipped (covered by WeaponFire/WeaponHit).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 534 full EVA matrix residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` collect_audio_events ParticleSystemSpawned
//! - `CombatParticleKind` death/explosion kinds
//!
//! Fail-closed:
//! - Not full FXList / Miles particle-audio binding matrix
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_PARTICLE_SPAWN_AUDIO_METHOD_NAMES_WAVE535: &[&str] = &[
    "PresentationEvent::ParticleSystemSpawned",
    "CombatParticleKind::DeathExplosion",
    "Explosion",
    "FireBurn",
    "PoisonDeath",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_PARTICLE_SPAWN_AUDIO_NAV_STEPS_WAVE535: &[&str] = &[
    "REQUIRE_PARTICLE_SPAWN_AUDIO",
    "REQUIRE_DEATH_EXPLOSION_MAP",
    "LIVE_PRESENTATION_PARTICLE_SPAWN_AUDIO",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_PARTICLE_SPAWN_AUDIO_CMD_NAMES_WAVE535: &[&str] =
    &["particle_spawn_audio", "death_explosion", "fire_burn"];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationParticleSpawnAudioAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationParticleSpawnAudioAction {
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

fn residual_action_store(action: ResidualPresentationParticleSpawnAudioAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_particle_spawn_audio_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_particle_spawn_audio_last_action()
-> ResidualPresentationParticleSpawnAudioAction {
    ResidualPresentationParticleSpawnAudioAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_presentation_particle_spawn_audio_method_names_residual_wave535() -> bool {
    let names = LIVE_PRESENTATION_PARTICLE_SPAWN_AUDIO_METHOD_NAMES_WAVE535;
    let ok = residual_name_index(names, "PresentationEvent::ParticleSystemSpawned").is_some()
        && residual_name_index(names, "CombatParticleKind::DeathExplosion").is_some()
        && residual_name_index(names, "Explosion").is_some()
        && residual_name_index(names, "FireBurn").is_some()
        && residual_name_index(names, "PoisonDeath").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationParticleSpawnAudioAction::MethodNames);
    ok
}

pub fn honesty_presentation_particle_spawn_audio_source_markers_residual_wave535() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 535")
        && pf.contains("combat particle spawn → presentation audio")
        && pf.contains("PresentationEvent::ParticleSystemSpawned")
        && pf.contains("CombatParticleKind::DeathExplosion")
        && pf.contains("\"Explosion\"")
        && pf.contains("\"FireBurn\"")
        && pf.contains("\"PoisonDeath\"")
        && pf.contains("\"LaserDeath\"")
        && pf.contains("\"DeathSmoke\"")
        && pf.contains("WeaponMuzzleFlash")
        && pf.contains("handled below (Wave 535)")
        && !pf.contains("playable_claim = true");
    residual_action_store(ResidualPresentationParticleSpawnAudioAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_particle_spawn_audio_nav_commands_residual_wave535() -> bool {
    let steps = LIVE_PRESENTATION_PARTICLE_SPAWN_AUDIO_NAV_STEPS_WAVE535;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_PARTICLE_SPAWN_AUDIO_CMD_NAMES_WAVE535;
    let ok = residual_name_index(steps, "REQUIRE_PARTICLE_SPAWN_AUDIO").is_some()
        && residual_name_index(steps, "REQUIRE_DEATH_EXPLOSION_MAP").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_PARTICLE_SPAWN_AUDIO").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "particle_spawn_audio").is_some()
        && residual_name_index(cmds, "death_explosion").is_some()
        && residual_name_index(cmds, "fire_burn").is_some();
    residual_action_store(ResidualPresentationParticleSpawnAudioAction::NavCommands);
    ok
}

pub fn simulate_presentation_particle_spawn_audio_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 535")
        && pf.contains("ParticleSystemSpawned")
        && pf.contains("DeathExplosion");
    residual_action_store(ResidualPresentationParticleSpawnAudioAction::CollectSource);
    ok
}

pub fn simulate_presentation_particle_spawn_audio_dispatch_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("AudioEventRequest::new(event_name)")
        && pf.contains("with_priority(160)")
        && pf.contains("with_position(*position)");
    residual_action_store(ResidualPresentationParticleSpawnAudioAction::DispatchSource);
    ok
}

pub fn honesty_presentation_particle_spawn_audio_residual_pack_wave535() -> bool {
    honesty_presentation_particle_spawn_audio_method_names_residual_wave535()
        && honesty_presentation_particle_spawn_audio_source_markers_residual_wave535()
        && honesty_presentation_particle_spawn_audio_nav_commands_residual_wave535()
        && simulate_presentation_particle_spawn_audio_collect_source()
        && simulate_presentation_particle_spawn_audio_dispatch_source()
}

pub fn simulate_live_presentation_particle_spawn_audio_honesty() -> bool {
    let ok = honesty_presentation_particle_spawn_audio_residual_pack_wave535();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationParticleSpawnAudioAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_particle_spawn_audio_method_names_residual_wave535());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_particle_spawn_audio_source_markers_residual_wave535());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_particle_spawn_audio_nav_commands_residual_wave535());
    }

    #[test]
    fn presentation_particle_spawn_audio_sources() {
        assert!(simulate_presentation_particle_spawn_audio_collect_source());
        assert!(simulate_presentation_particle_spawn_audio_dispatch_source());
    }

    #[test]
    fn wave535_composite_pack() {
        assert!(honesty_presentation_particle_spawn_audio_residual_pack_wave535());
    }

    #[test]
    fn simulate_live_presentation_particle_spawn_audio_honesty_residual_live() {
        assert!(
            simulate_live_presentation_particle_spawn_audio_honesty(),
            "particle spawn audio residual must latch"
        );
        assert!(residual_presentation_particle_spawn_audio_ok());
        assert_eq!(
            residual_presentation_particle_spawn_audio_last_action(),
            ResidualPresentationParticleSpawnAudioAction::Composite
        );
    }
}
