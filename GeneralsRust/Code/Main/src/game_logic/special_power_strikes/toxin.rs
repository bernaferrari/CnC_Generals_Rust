//! Anthrax toxin residual constants.
use super::types::*;
// --- Anthrax toxin residual (retail AnthraxBombPoisonFieldWeapon / LifetimeUpdate) ---

/// Retail `AnthraxBombPoisonFieldWeapon` PrimaryDamage.
pub const ANTHRAX_TOXIN_DAMAGE_PER_TICK: f32 = 40.0;
/// Retail `AnthraxBombPoisonFieldWeapon` PrimaryDamageRadius.
pub const ANTHRAX_TOXIN_RADIUS: f32 = 300.0;
/// Retail DelayBetweenShots = 500 ms → 15 frames @ 30 FPS.
pub const ANTHRAX_TOXIN_TICK_INTERVAL_FRAMES: u32 = 15;
/// Retail PoisonFieldAnthraxBomb LifetimeUpdate Min/MaxLifetime = 60000 ms @ 30 FPS.
pub const ANTHRAX_TOXIN_DURATION_FRAMES: u32 = 1800;
/// Residual ambient cue for the anthrax pool (`PoisonFieldAnthraxBomb.SoundAmbient`).
pub const ANTHRAX_TOXIN_AUDIO: &str = "AnthraxPoolAmbientLoop";
/// Retail `AnthraxBombPoisonFieldWeapon` FireFX residual.
pub const ANTHRAX_TOXIN_FIRE_FX: &str = "WeaponFX_LargePoisonFieldWeaponUpgraded";
/// Retail `AnthraxBombPoisonFieldWeapon` DamageType residual.
pub const ANTHRAX_TOXIN_DAMAGE_TYPE: &str = "POISON";
/// Retail `AnthraxBombPoisonFieldWeapon` DeathType residual.
pub const ANTHRAX_TOXIN_DEATH_TYPE: &str = "POISONED_BETA";
/// Retail `AnthraxBombPoisonFieldWeapon` WeaponSpeed residual (dist/sec).
pub const ANTHRAX_TOXIN_WEAPON_SPEED: f32 = 600.0;
/// Retail AnthraxBombWeapon FireOCL residual.
pub const ANTHRAX_TOXIN_OCL: &str = "OCL_PoisonFieldAnthraxBomb";
/// Retail OCL_PoisonFieldAnthraxBomb CreateObject residual.
pub const ANTHRAX_TOXIN_OBJECT_NAME: &str = "PoisonFieldAnthraxBomb";
/// Retail PoisonFieldAnthraxBomb MaxHealth residual.
pub const ANTHRAX_TOXIN_FIELD_MAX_HEALTH: f32 = 120.0;
/// Retail AnthraxBombWeapon impact blast residual name.
pub const ANTHRAX_BOMB_WEAPON_NAME: &str = "AnthraxBombWeapon";
/// Retail AnthraxBombWeapon PrimaryDamage residual (impact blast only).
pub const ANTHRAX_BOMB_IMPACT_DAMAGE: f32 = 200.0;
/// Retail AnthraxBombWeapon PrimaryDamageRadius residual.
pub const ANTHRAX_BOMB_IMPACT_RADIUS: f32 = 100.0;
/// Retail AnthraxBombPoisonFieldWeapon DelayBetweenShots msec residual.
pub const ANTHRAX_TOXIN_DELAY_BETWEEN_SHOTS_MS: u32 = 500;
/// Retail PoisonFieldAnthraxBomb LifetimeUpdate Min/MaxLifetime msec.
pub const ANTHRAX_TOXIN_LIFETIME_MS: u32 = 60000;
/// Retail AnthraxBombPoisonFieldWeapon RadiusDamageAffects residual.
pub const ANTHRAX_TOXIN_RADIUS_DAMAGE_AFFECTS: &str = "ALLIES ENEMIES NEUTRALS NOT_AIRBORNE";
/// Retail AnthraxBombPoisonFieldWeapon AttackRange residual.
pub const ANTHRAX_TOXIN_ATTACK_RANGE: f32 = 15.0;
/// Retail AnthraxBombPoisonFieldWeapon MinimumAttackRange residual.
pub const ANTHRAX_TOXIN_MINIMUM_ATTACK_RANGE: f32 = 10.0;
