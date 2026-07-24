//! Host GLA Toxin Tractor residual (poison stream + contaminate spray field).
//!
//! Residual slice (playability):
//! - PRIMARY `ToxinTruckGun`: poison stream (dmg **10**, radius **10**, range **100**,
//!   Delay 40ms → 2 frames residual). Anthrax Beta → dmg **12.5**.
//!   Anthrax Gamma (`Chem_ToxinTruckGunGamma`) → dmg **20.5**.
//! - SECONDARY `ToxinTruckSprayer` contaminate residual (special attack only):
//!   SecondaryDamage **2** / radius **75**, range **15**. After residual spray,
//!   spawns MediumPoisonField DoT (2 dmg / radius 80 / 30s / 500ms ticks).
//!   Anthrax Beta/Gamma → spray **2.5** + upgraded MediumPoisonField (**2.5**/tick).
//! - Death residual: `ToxinShellWeapon` → SmallPoisonField (2 dmg / radius 12 /
//!   10s lifetime). Anthrax Beta/Gamma → **2.5**/tick / radius **7.5**.
//! - Chem General toxin trucks start at Anthrax Beta baseline (retail upgraded
//!   WeaponSet) until `Chem_Upgrade_GLAAnthraxGamma` is researched.
//! - Salvage PlusOne/PlusTwo residual: stream + spray damage matrix (retail Weapon.ini).
//!
//! Wave 55 residual pack (retail Weapon.ini / GLAVehicle / System.ini honesty):
//! - Contaminate puddle: Medium field dmg/radius/duration/tick + OCL names +
//!   FireOCLAfterWeaponCooldown MinShots **4**, ContinuousFireCoast **300**ms → **9**f,
//!   OCLLifetimePerSecond **10000**, OCLLifetimeMaxCap **180000**
//! - Stream residual: ClipSize **30**, ClipReload **40**ms, WeaponSpeed **600**,
//!   FireSoundLoopTime **80**ms, AllowAttackGarrisonedBldgs Yes
//! - Spray residual: salvage PlusOne/PlusTwo damage matrix (2/2.5/3 + anthrax tiers)
//! - Upgrade anthrax residual: Beta/Gamma stream salvage matrix + death types
//! - Clean-up interaction residual: KindOf CLEANUP_HAZARD / field HP / clear-in-radius
//!
//! Fail-closed honesty:
//! - FireOCLAfterWeaponCooldown: MinShots=4 + coast spawn residual (primary stream still live)
//! - Not full stream projectile drawing / spigot bone / turret pitch matrix
//! - Not full gamma particle bones / HazardousMaterialArmor damage stack
//! - Not network toxin replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Retail primary stream weapon.
pub const TOXIN_TRUCK_GUN: &str = "ToxinTruckGun";
/// Retail primary after Anthrax Beta.
pub const TOXIN_TRUCK_GUN_UPGRADED: &str = "ToxinTruckGunUpgraded";
/// Retail Chem Anthrax Gamma primary stream.
pub const TOXIN_TRUCK_GUN_GAMMA: &str = "Chem_ToxinTruckGunGamma";
/// Retail secondary contaminate spray.
pub const TOXIN_TRUCK_SPRAYER: &str = "ToxinTruckSprayer";
/// Retail secondary after Anthrax Beta.
pub const TOXIN_TRUCK_SPRAYER_UPGRADED: &str = "ToxinTruckSprayerUpgraded";
/// Retail Chem Anthrax Gamma spray.
pub const TOXIN_TRUCK_SPRAYER_GAMMA: &str = "Chem_ToxinTruckSprayerGamma";
/// Retail Upgrade_GLAAnthraxBeta.
pub const UPGRADE_GLA_ANTHRAX_BETA: &str = "Upgrade_GLAAnthraxBeta";
/// Retail Chem_Upgrade_GLAAnthraxGamma (Chemical General).
pub const UPGRADE_GLA_ANTHRAX_GAMMA: &str = "Chem_Upgrade_GLAAnthraxGamma";
/// Alias residual for shorthand tests / host unlock tags.
pub const UPGRADE_GLA_ANTHRAX_GAMMA_ALT: &str = "Upgrade_GLAAnthraxGamma";

/// Base primary damage / radius / range.
pub const TOXIN_STREAM_DAMAGE: f32 = 10.0;
pub const TOXIN_STREAM_DAMAGE_UPGRADED: f32 = 12.5;
/// Retail Chem_ToxinTruckGunGamma PrimaryDamage.
pub const TOXIN_STREAM_DAMAGE_GAMMA: f32 = 20.5;
pub const TOXIN_STREAM_RADIUS: f32 = 10.0;
pub const TOXIN_STREAM_RANGE: f32 = 100.0;
/// DelayBetweenShots 40ms → 2 frames @ 30 FPS (ceil).
pub const TOXIN_STREAM_DELAY_FRAMES: u32 = 2;

/// Contaminate spray residual (SecondaryDamage / radius / AttackRange).
pub const TOXIN_SPRAY_DAMAGE: f32 = 2.0;
pub const TOXIN_SPRAY_DAMAGE_UPGRADED: f32 = 2.5;
/// Retail ToxinTruckSprayerPlusOne SecondaryDamage.
pub const TOXIN_SPRAY_DAMAGE_PLUS_ONE: f32 = 2.5;
/// Retail ToxinTruckSprayerPlusTwo SecondaryDamage.
pub const TOXIN_SPRAY_DAMAGE_PLUS_TWO: f32 = 3.0;
/// Retail ToxinTruckSprayerUpgradedPlusOne SecondaryDamage.
pub const TOXIN_SPRAY_DAMAGE_UPGRADED_PLUS_ONE: f32 = 3.0;
/// Retail ToxinTruckSprayerUpgradedPlusTwo SecondaryDamage.
pub const TOXIN_SPRAY_DAMAGE_UPGRADED_PLUS_TWO: f32 = 4.0;
/// Retail Chem_ToxinTruckSprayerGammaPlusOne SecondaryDamage.
pub const TOXIN_SPRAY_DAMAGE_GAMMA_PLUS_ONE: f32 = 3.5;
/// Retail Chem_ToxinTruckSprayerGammaPlusTwo SecondaryDamage.
pub const TOXIN_SPRAY_DAMAGE_GAMMA_PLUS_TWO: f32 = 4.5;
pub const TOXIN_SPRAY_RADIUS: f32 = 75.0;
pub const TOXIN_SPRAY_RANGE: f32 = 15.0;
/// DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const TOXIN_SPRAY_DELAY_FRAMES: u32 = 6;
/// Retail ContinuousFireCoast 300ms → 9 frames (ceil) — spray field spawn coast residual.
pub const TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_MS: u32 = 300;
pub const TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES: u32 = 9;
/// Retail FireOCLAfterWeaponCooldown MinShotsToCreateOCL residual.
pub const TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL: u32 = 4;
/// Retail FireOCLAfterWeaponCooldown OCLLifetimePerSecond residual (msec).
pub const TOXIN_SPRAY_OCL_LIFETIME_PER_SECOND_MS: u32 = 10_000;
/// Retail FireOCLAfterWeaponCooldown OCLLifetimeMaxCap residual (msec).
pub const TOXIN_SPRAY_OCL_LIFETIME_MAX_CAP_MS: u32 = 180_000;
/// OCLLifetimeMaxCap 180000ms → 5400 frames @ 30 FPS.
pub const TOXIN_SPRAY_OCL_LIFETIME_MAX_CAP_FRAMES: u32 = 5_400;

/// MediumPoisonField residual (spray contamination OCL).
pub const TOXIN_MED_FIELD_DAMAGE: f32 = 2.0;
/// Retail Chem_MediumPoisonFieldWeaponGamma / upgraded anthrax residual.
pub const TOXIN_MED_FIELD_DAMAGE_UPGRADED: f32 = 2.5;
pub const TOXIN_MED_FIELD_RADIUS: f32 = 80.0;
/// DelayBetweenShots 500ms → 15 frames.
pub const TOXIN_MED_FIELD_TICK_FRAMES: u32 = 15;
/// Lifetime 30000ms → 900 frames.
pub const TOXIN_MED_FIELD_DURATION_FRAMES: u32 = 900;
/// Retail PoisonFieldMedium LifetimeUpdate Min/MaxLifetime residual (msec).
pub const TOXIN_MED_FIELD_LIFETIME_MS: u32 = 30_000;
/// Retail PoisonFieldMedium Body MaxHealth residual.
pub const TOXIN_MED_FIELD_MAX_HEALTH: f32 = 100.0;
/// Retail PoisonFieldUpgradedMedium / GammaMedium MaxHealth residual.
pub const TOXIN_MED_FIELD_MAX_HEALTH_UPGRADED: f32 = 120.0;
/// Retail PoisonFieldMedium GeometryMajorRadius residual.
pub const TOXIN_MED_FIELD_GEOMETRY_RADIUS: f32 = 40.0;

/// SmallPoisonField residual (death ToxinShellWeapon OCL).
pub const TOXIN_SMALL_FIELD_DAMAGE: f32 = 2.0;
/// Retail Chem_SmallPoisonFieldWeaponGamma PrimaryDamage residual.
pub const TOXIN_SMALL_FIELD_DAMAGE_UPGRADED: f32 = 2.5;
/// Retail SmallPoisonFieldWeapon PrimaryDamageRadius residual.
pub const TOXIN_SMALL_FIELD_RADIUS: f32 = 12.0;
/// Retail SmallPoisonFieldWeaponUpgraded / Chem_SmallPoisonFieldWeaponGamma radius.
pub const TOXIN_SMALL_FIELD_RADIUS_UPGRADED: f32 = 7.5;
/// Lifetime 10000ms → 300 frames.
pub const TOXIN_SMALL_FIELD_DURATION_FRAMES: u32 = 300;
pub const TOXIN_SMALL_FIELD_TICK_FRAMES: u32 = 15;
/// Retail PoisonFieldSmall LifetimeUpdate residual (msec).
pub const TOXIN_SMALL_FIELD_LIFETIME_MS: u32 = 10_000;
/// Retail PoisonFieldSmall MaxHealth residual.
pub const TOXIN_SMALL_FIELD_MAX_HEALTH: f32 = 100.0;
/// Retail PoisonFieldUpgradedSmall / GammaSmall MaxHealth residual.
pub const TOXIN_SMALL_FIELD_MAX_HEALTH_UPGRADED: f32 = 120.0;
/// Retail PoisonFieldSmall GeometryMajorRadius residual.
pub const TOXIN_SMALL_FIELD_GEOMETRY_RADIUS: f32 = 6.0;
/// Retail PoisonFieldUpgradedSmall GeometryMajorRadius residual.
pub const TOXIN_SMALL_FIELD_GEOMETRY_RADIUS_UPGRADED: f32 = 4.0;

/// Salvage PlusOne / PlusTwo primary damage residual (non-anthrax path).
pub const TOXIN_STREAM_DAMAGE_PLUS_ONE: f32 = 12.5;
pub const TOXIN_STREAM_DAMAGE_PLUS_TWO: f32 = 15.0;
/// Retail ToxinTruckGunUpgradedPlusOne PrimaryDamage.
pub const TOXIN_STREAM_DAMAGE_UPGRADED_PLUS_ONE: f32 = 15.0;
/// Retail ToxinTruckGunUpgradedPlusTwo PrimaryDamage.
pub const TOXIN_STREAM_DAMAGE_UPGRADED_PLUS_TWO: f32 = 20.0;
/// Retail Chem_ToxinTruckGunGammaPlusOne PrimaryDamage.
pub const TOXIN_STREAM_DAMAGE_GAMMA_PLUS_ONE: f32 = 24.5;
/// Retail Chem_ToxinTruckGunGammaPlusTwo PrimaryDamage.
pub const TOXIN_STREAM_DAMAGE_GAMMA_PLUS_TWO: f32 = 28.5;

/// Stream weapon residual (ToxinTruckGun).
pub const TOXIN_STREAM_CLIP_SIZE: u32 = 30;
/// ClipReloadTime 40ms residual.
pub const TOXIN_STREAM_CLIP_RELOAD_MS: u32 = 40;
/// ClipReloadTime 40ms → 2 frames @ 30 FPS (ceil).
pub const TOXIN_STREAM_CLIP_RELOAD_FRAMES: u32 = 2;
/// DelayBetweenShots 40ms residual.
pub const TOXIN_STREAM_DELAY_MS: u32 = 40;
/// WeaponSpeed residual (dist/sec).
pub const TOXIN_STREAM_WEAPON_SPEED: f32 = 600.0;
/// FireSoundLoopTime 80ms residual.
pub const TOXIN_STREAM_FIRE_SOUND_LOOP_MS: u32 = 80;
/// AllowAttackGarrisonedBldgs residual.
pub const TOXIN_STREAM_ALLOW_ATTACK_GARRISONED: bool = true;
/// Retail upgraded stream MinimumAttackRange residual.
pub const TOXIN_STREAM_MIN_RANGE_UPGRADED: f32 = 10.0;

/// Spray weapon residual timing.
pub const TOXIN_SPRAY_DELAY_MS: u32 = 200;
pub const TOXIN_SPRAY_WEAPON_SPEED: f32 = 600.0;
pub const TOXIN_SPRAY_ACCEPTABLE_AIM_DELTA_DEG: f32 = 180.0;

/// Contaminate OCL residual names (FireOCLAfterWeaponCooldown).
pub const TOXIN_OCL_POISON_FIELD_MEDIUM: &str = "OCL_PoisonFieldMedium";
pub const TOXIN_OCL_POISON_FIELD_UPGRADED_MEDIUM: &str = "OCL_PoisonFieldUpgradedMedium";
pub const TOXIN_OCL_POISON_FIELD_GAMMA_MEDIUM: &str = "OCL_PoisonFieldGammaMedium";
pub const TOXIN_OCL_POISON_FIELD_SMALL: &str = "OCL_PoisonFieldSmall";
pub const TOXIN_OCL_POISON_FIELD_UPGRADED_SMALL: &str = "OCL_PoisonFieldUpgradedSmall";
pub const TOXIN_OCL_POISON_FIELD_GAMMA_SMALL: &str = "OCL_PoisonFieldGammaSmall";
/// Death weapon residual names.
pub const TOXIN_SHELL_WEAPON: &str = "ToxinShellWeapon";
pub const TOXIN_SHELL_WEAPON_UPGRADED: &str = "ToxinShellWeaponUpgraded";
pub const TOXIN_SHELL_WEAPON_GAMMA: &str = "Chem_ToxinShellWeaponGamma";

/// DeathType residual names (Weapon.ini).
pub const TOXIN_DEATH_TYPE_POISONED: &str = "POISONED";
pub const TOXIN_DEATH_TYPE_POISONED_BETA: &str = "POISONED_BETA";
pub const TOXIN_DEATH_TYPE_POISONED_GAMMA: &str = "POISONED_GAMMA";

/// KindOf residual for poison fields (cleanup interaction).
pub const TOXIN_FIELD_KINDOF_CLEANUP_HAZARD: &str = "CLEANUP_HAZARD";
/// Armor residual on poison field objects.
pub const TOXIN_FIELD_ARMOR: &str = "HazardousMaterialArmor";
/// HazardFieldCoreWeapon residual (anti-stack blast at spawn).
pub const TOXIN_FIELD_CORE_WEAPON: &str = "HazardFieldCoreWeapon";
/// Anthrax pool ambient residual (upgraded/gamma fields).
pub const TOXIN_ANTHRAX_POOL_AUDIO: &str = "AnthraxPoolAmbientLoop";

/// Residual fire / ambient audio.
pub const TOXIN_STREAM_AUDIO: &str = "ToxinTractorWeaponLoop";
pub const TOXIN_SPRAY_AUDIO: &str = "ToxinTractorContaminate";
pub const TOXIN_POISON_AUDIO: &str = "ToxicPoolAmbientLoop";

/// Logic frames per second residual.
pub const TOXIN_LOGIC_FPS: f32 = 30.0;

/// Anthrax residual combat tier (Beta = stock upgrade; Gamma = Chem general).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum AnthraxResidualTier {
    #[default]
    None,
    /// Upgrade_GLAAnthraxBeta / Chem baseline upgraded weapons.
    Beta,
    /// Chem_Upgrade_GLAAnthraxGamma residual.
    Gamma,
}

impl AnthraxResidualTier {
    pub fn is_upgraded(self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn is_gamma(self) -> bool {
        matches!(self, Self::Gamma)
    }
}

/// Salvage residual tier for toxin tractor primary.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ToxinTractorSalvageTier {
    #[default]
    Base = 0,
    One = 1,
    Two = 2,
}

impl ToxinTractorSalvageTier {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::One,
            2 => Self::Two,
            _ => Self::Base,
        }
    }
}

/// Whether template is a residual Toxin Tractor / Toxin Truck vehicle.
///
/// Fail-closed: name residual (not full Salvage / W3D turret matrix).
/// Excludes weapons, projectiles, poison field system objects.
pub fn is_toxin_tractor_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("stream")
        || n.contains("poisonfield")
        || n.contains("shell")
        || n.starts_with("upgrade")
        || n.contains("sprayer")
        || n.ends_with("gun")
        || n.contains("gungun")
        || n.contains("truckgun")
        || n.contains("trucksprayer")
    {
        return false;
    }
    n.contains("toxintruck")
        || n.contains("toxintrac")
        || n.contains("toxin_truck")
        || n.contains("toxin_tractor")
        || n == "gla_toxintruck"
        || n == "gla_toxintraktor"
        || n == "testtoxintruck"
        || n == "testtoxintraktor"
        || (n.contains("vehicletoxin") && (n.contains("truck") || n.contains("tractor")))
}

/// Whether template is a Chemical General residual unit (Chem_ / GC_Chem_).
///
/// Chem toxin trucks start with upgraded (Anthrax Beta) weapons in retail INI.
pub fn is_chem_general_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.starts_with("chem_") || n.starts_with("gc_chem_") || n.contains("testchem")
}

/// True when upgrade name is Anthrax Gamma residual research.
pub fn is_anthrax_gamma_upgrade_name(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    n.contains("anthraxgamma")
}

/// True when upgrade name is Anthrax Beta residual research.
pub fn is_anthrax_beta_upgrade_name(name: &str) -> bool {
    let n: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect();
    n.contains("anthraxbeta") && !n.contains("anthraxgamma")
}

/// Resolve anthrax residual tier from upgrade tags + Chem template baseline.
///
/// Fail-closed: not full WeaponSet PLAYER_UPGRADE module matrix / science prereqs.
pub fn anthrax_tier_from_flags(
    has_gamma: bool,
    has_beta: bool,
    chem_template_baseline: bool,
) -> AnthraxResidualTier {
    if has_gamma {
        AnthraxResidualTier::Gamma
    } else if has_beta || chem_template_baseline {
        AnthraxResidualTier::Beta
    } else {
        AnthraxResidualTier::None
    }
}

/// Convert msec residual → logic frames @ 30 FPS (ceil for non-integer like Delay 40ms).
pub fn toxin_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    let frames = (ms as f32) * TOXIN_LOGIC_FPS / 1000.0;
    frames.ceil() as u32
}

/// Primary stream damage residual (salvage + anthrax tier).
///
/// Retail Weapon.ini matrix:
/// - Base 10 / PlusOne 12.5 / PlusTwo 15
/// - Beta 12.5 / 15 / 20
/// - Gamma 20.5 / 24.5 / 28.5
pub fn toxin_stream_damage(tier: ToxinTractorSalvageTier, anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::Gamma => match tier {
            ToxinTractorSalvageTier::Base => TOXIN_STREAM_DAMAGE_GAMMA,
            ToxinTractorSalvageTier::One => TOXIN_STREAM_DAMAGE_GAMMA_PLUS_ONE,
            ToxinTractorSalvageTier::Two => TOXIN_STREAM_DAMAGE_GAMMA_PLUS_TWO,
        },
        AnthraxResidualTier::Beta => match tier {
            ToxinTractorSalvageTier::Base => TOXIN_STREAM_DAMAGE_UPGRADED,
            ToxinTractorSalvageTier::One => TOXIN_STREAM_DAMAGE_UPGRADED_PLUS_ONE,
            ToxinTractorSalvageTier::Two => TOXIN_STREAM_DAMAGE_UPGRADED_PLUS_TWO,
        },
        AnthraxResidualTier::None => match tier {
            ToxinTractorSalvageTier::Base => TOXIN_STREAM_DAMAGE,
            ToxinTractorSalvageTier::One => TOXIN_STREAM_DAMAGE_PLUS_ONE,
            ToxinTractorSalvageTier::Two => TOXIN_STREAM_DAMAGE_PLUS_TWO,
        },
    }
}

/// Contaminate spray secondary damage residual (base salvage / no crate).
///
/// Compatibility wrapper — host combat path currently uses anthrax tier only.
pub fn toxin_spray_damage(anthrax: AnthraxResidualTier) -> f32 {
    toxin_spray_damage_with_salvage(ToxinTractorSalvageTier::Base, anthrax)
}

/// Contaminate spray SecondaryDamage residual (salvage + anthrax tier).
///
/// Retail Weapon.ini matrix:
/// - Base 2 / PlusOne 2.5 / PlusTwo 3
/// - Beta 2.5 / 3 / 4
/// - Gamma 2.5 / 3.5 / 4.5
pub fn toxin_spray_damage_with_salvage(
    tier: ToxinTractorSalvageTier,
    anthrax: AnthraxResidualTier,
) -> f32 {
    match anthrax {
        AnthraxResidualTier::None => match tier {
            ToxinTractorSalvageTier::Base => TOXIN_SPRAY_DAMAGE,
            ToxinTractorSalvageTier::One => TOXIN_SPRAY_DAMAGE_PLUS_ONE,
            ToxinTractorSalvageTier::Two => TOXIN_SPRAY_DAMAGE_PLUS_TWO,
        },
        AnthraxResidualTier::Beta => match tier {
            ToxinTractorSalvageTier::Base => TOXIN_SPRAY_DAMAGE_UPGRADED,
            ToxinTractorSalvageTier::One => TOXIN_SPRAY_DAMAGE_UPGRADED_PLUS_ONE,
            ToxinTractorSalvageTier::Two => TOXIN_SPRAY_DAMAGE_UPGRADED_PLUS_TWO,
        },
        AnthraxResidualTier::Gamma => match tier {
            ToxinTractorSalvageTier::Base => TOXIN_SPRAY_DAMAGE_UPGRADED,
            ToxinTractorSalvageTier::One => TOXIN_SPRAY_DAMAGE_GAMMA_PLUS_ONE,
            ToxinTractorSalvageTier::Two => TOXIN_SPRAY_DAMAGE_GAMMA_PLUS_TWO,
        },
    }
}

/// MediumPoisonField damage-per-tick residual.
pub fn toxin_med_field_damage(anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_MED_FIELD_DAMAGE,
        AnthraxResidualTier::Beta | AnthraxResidualTier::Gamma => TOXIN_MED_FIELD_DAMAGE_UPGRADED,
    }
}

/// SmallPoisonField (death) damage-per-tick residual.
pub fn toxin_small_field_damage(anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_SMALL_FIELD_DAMAGE,
        AnthraxResidualTier::Beta | AnthraxResidualTier::Gamma => TOXIN_SMALL_FIELD_DAMAGE_UPGRADED,
    }
}

/// SmallPoisonField PrimaryDamageRadius residual (base 12 / upgraded+gamma 7.5).
pub fn toxin_small_field_radius(anthrax: AnthraxResidualTier) -> f32 {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_SMALL_FIELD_RADIUS,
        AnthraxResidualTier::Beta | AnthraxResidualTier::Gamma => TOXIN_SMALL_FIELD_RADIUS_UPGRADED,
    }
}

/// Medium field MaxHealth residual.
pub fn toxin_med_field_max_health(anthrax: AnthraxResidualTier) -> f32 {
    if anthrax.is_upgraded() {
        TOXIN_MED_FIELD_MAX_HEALTH_UPGRADED
    } else {
        TOXIN_MED_FIELD_MAX_HEALTH
    }
}

/// Small field MaxHealth residual.
pub fn toxin_small_field_max_health(anthrax: AnthraxResidualTier) -> f32 {
    if anthrax.is_upgraded() {
        TOXIN_SMALL_FIELD_MAX_HEALTH_UPGRADED
    } else {
        TOXIN_SMALL_FIELD_MAX_HEALTH
    }
}

/// DeathType residual name for stream/spray/field.
pub fn toxin_death_type_name(anthrax: AnthraxResidualTier) -> &'static str {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_DEATH_TYPE_POISONED,
        AnthraxResidualTier::Beta => TOXIN_DEATH_TYPE_POISONED_BETA,
        AnthraxResidualTier::Gamma => TOXIN_DEATH_TYPE_POISONED_GAMMA,
    }
}

/// FireOCL residual for spray cooldown medium field.
pub fn toxin_spray_ocl_name(anthrax: AnthraxResidualTier) -> &'static str {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_OCL_POISON_FIELD_MEDIUM,
        AnthraxResidualTier::Beta => TOXIN_OCL_POISON_FIELD_UPGRADED_MEDIUM,
        AnthraxResidualTier::Gamma => TOXIN_OCL_POISON_FIELD_GAMMA_MEDIUM,
    }
}

/// FireOCL residual for death small field.
pub fn toxin_death_ocl_name(anthrax: AnthraxResidualTier) -> &'static str {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_OCL_POISON_FIELD_SMALL,
        AnthraxResidualTier::Beta => TOXIN_OCL_POISON_FIELD_UPGRADED_SMALL,
        AnthraxResidualTier::Gamma => TOXIN_OCL_POISON_FIELD_GAMMA_SMALL,
    }
}

/// Death weapon residual name (FireWeaponWhenDead).
pub fn toxin_death_weapon_name(anthrax: AnthraxResidualTier) -> &'static str {
    match anthrax {
        AnthraxResidualTier::None => TOXIN_SHELL_WEAPON,
        AnthraxResidualTier::Beta => TOXIN_SHELL_WEAPON_UPGRADED,
        AnthraxResidualTier::Gamma => TOXIN_SHELL_WEAPON_GAMMA,
    }
}

/// Poison-field ambient audio residual.
pub fn toxin_field_ambient_audio(anthrax: AnthraxResidualTier) -> &'static str {
    if anthrax.is_upgraded() {
        TOXIN_ANTHRAX_POOL_AUDIO
    } else {
        TOXIN_POISON_AUDIO
    }
}

/// Whether residual spray has fired enough shots to create contaminate OCL.
pub fn toxin_spray_ready_for_ocl(continuous_shots: u32) -> bool {
    continuous_shots >= TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL
}

/// Clean-up interaction residual: field epicenter within cleanup radius is clearable.
///
/// Retail poison fields carry KindOf CLEANUP_HAZARD + HazardousMaterialArmor;
/// AmbulanceCleanHazardWeapon / CleanupArea clears them in PrimaryDamageRadius.
pub fn toxin_field_clearable_by_cleanup(
    field_pos: (f32, f32),
    cleanup_center: (f32, f32),
    cleanup_radius: f32,
) -> bool {
    in_radius_2d(cleanup_center, field_pos, cleanup_radius)
}

/// Name residual for cleanup-hazard kindof (fail-closed vs full KindOf mask).
pub fn is_toxin_cleanup_hazard_name(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("poisonfield")
        || n.contains("poison_field")
        || n.contains("cleanup_hazard")
        || n.contains("cleanuphazard")
        || n.contains("toxicpool")
        || n.contains("anthraxpool")
}

/// Whether residual secondary is contaminate spray path (spawn medium field).
pub fn should_apply_toxin_spray(is_toxin_tractor: bool, fired_slot: u8) -> bool {
    is_toxin_tractor && fired_slot == 1
}

/// Whether residual primary stream should apply small splash radius residual.
pub fn should_apply_toxin_stream(is_toxin_tractor: bool, fired_slot: u8) -> bool {
    is_toxin_tractor && fired_slot == 0
}

/// Stream residual damage at distance (primary radius ring).
pub fn toxin_stream_damage_at(distance: f32, base_damage: f32) -> f32 {
    if distance <= TOXIN_STREAM_RADIUS {
        base_damage
    } else {
        0.0
    }
}

/// Spray residual damage at distance from tractor (SecondaryDamageRadius).
pub fn toxin_spray_damage_at(distance: f32, spray_damage: f32) -> f32 {
    if distance <= TOXIN_SPRAY_RADIUS {
        spray_damage
    } else {
        0.0
    }
}

/// Legal residual toxin splash / field target (not airborne residual).
pub fn is_legal_toxin_target(
    is_alive: bool,
    is_self: bool,
    under_construction: bool,
    is_combat_kind: bool,
    is_airborne: bool,
) -> bool {
    is_alive && !is_self && !under_construction && is_combat_kind && !is_airborne
}

/// Reload time seconds residual for delay frames @ 30 FPS.
pub fn delay_frames_to_reload_secs(delay_frames: u32) -> f32 {
    (delay_frames.max(1) as f32) / 30.0
}

/// One active residual poison field (medium spray or small death).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostToxinTractorPoisonZone {
    pub id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub position: Vec3,
    pub radius: f32,
    pub damage_per_tick: f32,
    pub activate_frame: u32,
    pub expires_frame: u32,
    pub next_tick_frame: u32,
    /// Anthrax residual tier for this field (Beta/Gamma use upgraded DoT).
    pub anthrax_tier: AnthraxResidualTier,
    /// True when spawned by death residual (small field).
    pub from_death: bool,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
}

impl HostToxinTractorPoisonZone {
    /// Backward-compat residual flag (any anthrax upgrade).
    pub fn anthrax_upgraded(&self) -> bool {
        self.anthrax_tier.is_upgraded()
    }

    pub fn is_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.expires_frame
    }

    pub fn is_due_tick(&self, current_frame: u32) -> bool {
        !self.is_expired(current_frame) && current_frame >= self.next_tick_frame
    }
}

/// Damage application plan for a single victim this tick.
#[derive(Debug, Clone, Copy)]
pub struct HostToxinTractorPoisonHit {
    pub target_id: ObjectId,
    pub damage: f32,
    pub zone_id: u32,
}

/// Result of resolving one poison zone's damage tick.
#[derive(Debug, Clone)]
pub struct HostToxinTractorPoisonTickPlan {
    pub zone_id: u32,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostToxinTractorPoisonHit>,
}

/// C++ FireOCLAfterWeaponCooldownUpdate residual state (toxin spray secondary).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HostFireOclAfterCooldownData {
    pub consecutive_shots: u32,
    pub start_frame: u32,
    pub last_shot_frame: u32,
    pub valid: bool,
    pub ocl_spawns: u32,
}

impl HostFireOclAfterCooldownData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a secondary spray shot this frame.
    pub fn record_shot(&mut self, current_frame: u32) {
        if self.consecutive_shots == 0 {
            self.start_frame = current_frame;
        }
        self.consecutive_shots = self.consecutive_shots.saturating_add(1);
        self.last_shot_frame = current_frame;
        self.valid = true;
    }

    /// Called when secondary is idle past coast; returns Some(lifetime_frames) if OCL should fire.
    pub fn try_fire_ocl_on_cooldown(&mut self, current_frame: u32) -> Option<u32> {
        if !self.valid {
            return None;
        }
        if self.consecutive_shots < TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL {
            self.reset();
            return None;
        }
        let lifetime = ocl_lifetime_frames(self.start_frame, current_frame);
        self.reset();
        Some(lifetime.max(1))
    }

    pub fn reset(&mut self) {
        self.consecutive_shots = 0;
        self.start_frame = 0;
        self.last_shot_frame = 0;
        self.valid = false;
    }
}

/// C++ fireOCL lifetime: (now-start)*seconds * (lifetimePerSecond_ms/1000) → frames, capped.
pub fn ocl_lifetime_frames(start_frame: u32, now_frame: u32) -> u32 {
    let elapsed = now_frame.saturating_sub(start_frame).max(1) as f32;
    let seconds = elapsed / 30.0;
    let life_sec = seconds * (TOXIN_SPRAY_OCL_LIFETIME_PER_SECOND_MS as f32) * 0.001;
    let frames = (life_sec * 30.0).round() as u32;
    let max_frames = ((TOXIN_SPRAY_OCL_LIFETIME_MAX_CAP_MS as f32) * 30.0 / 1000.0).round() as u32;
    frames.min(max_frames.max(1)).max(1)
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostToxinTractorRegistry {
    next_id: u32,
    active: Vec<HostToxinTractorPoisonZone>,
    pub zones_spawned: u32,
    pub death_fields_spawned: u32,
    pub expirations: u32,
    pub total_damage_applied: f32,
    pub damage_applications: u32,
    pub objects_destroyed: u32,
    /// Primary stream residual fires.
    pub stream_fires: u32,
    /// Units hit by stream residual (including intended).
    pub stream_units_hit: u32,
    /// FireOCLAfterWeaponCooldown residual medium-field spawns.
    pub fire_ocl_spawns: u32,
    /// Contaminate spray residual fires.
    pub spray_fires: u32,
    /// Units hit by spray residual splash.
    pub spray_units_hit: u32,
    /// Salvage tier apply count.
    pub salvage_upgrades: u32,
    /// Anthrax Gamma residual stream fires (observable honesty).
    pub gamma_stream_fires: u32,
    /// Anthrax Gamma residual field spawns (spray or death).
    pub gamma_fields_spawned: u32,
}

impl HostToxinTractorRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub fn active_zones(&self) -> &[HostToxinTractorPoisonZone] {
        &self.active
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    pub fn record_stream_fire(&mut self, units_hit: u32) {
        self.stream_fires = self.stream_fires.saturating_add(1);
        self.stream_units_hit = self.stream_units_hit.saturating_add(units_hit);
    }

    pub fn record_fire_ocl_spawn(&mut self) {
        self.fire_ocl_spawns = self.fire_ocl_spawns.saturating_add(1);
    }

    pub fn record_spray_fire(&mut self, units_hit: u32) {
        self.spray_fires = self.spray_fires.saturating_add(1);
        self.spray_units_hit = self.spray_units_hit.saturating_add(units_hit);
    }

    pub fn record_salvage_upgrade(&mut self) {
        self.salvage_upgrades = self.salvage_upgrades.saturating_add(1);
    }

    /// Spawn residual MediumPoisonField at contaminate spray location.
    pub fn spawn_medium_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        impact_pos: Vec3,
        activate_frame: u32,
        anthrax: AnthraxResidualTier,
    ) -> u32 {
        self.spawn_medium_field_lifetime(
            source_object,
            source_team,
            impact_pos,
            activate_frame,
            anthrax,
            TOXIN_MED_FIELD_DURATION_FRAMES,
        )
    }

    /// FireOCL residual: MediumPoisonField with computed OCL lifetime frames.
    pub fn spawn_medium_field_lifetime(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        impact_pos: Vec3,
        activate_frame: u32,
        anthrax: AnthraxResidualTier,
        lifetime_frames: u32,
    ) -> u32 {
        let id = self.alloc_id();
        let life = lifetime_frames.max(1);
        let zone = HostToxinTractorPoisonZone {
            id,
            source_object,
            source_team,
            position: impact_pos,
            radius: TOXIN_MED_FIELD_RADIUS,
            damage_per_tick: toxin_med_field_damage(anthrax),
            activate_frame,
            expires_frame: activate_frame.saturating_add(life),
            next_tick_frame: activate_frame,
            anthrax_tier: anthrax,
            from_death: false,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(zone);
        self.zones_spawned = self.zones_spawned.saturating_add(1);
        if anthrax.is_gamma() {
            self.gamma_fields_spawned = self.gamma_fields_spawned.saturating_add(1);
        }
        id
    }

    /// Spawn residual SmallPoisonField on toxin tractor death.
    pub fn spawn_death_field(
        &mut self,
        source_object: ObjectId,
        source_team: super::Team,
        death_pos: Vec3,
        activate_frame: u32,
        anthrax: AnthraxResidualTier,
    ) -> u32 {
        let id = self.alloc_id();
        let zone = HostToxinTractorPoisonZone {
            id,
            source_object,
            source_team,
            position: death_pos,
            radius: toxin_small_field_radius(anthrax),
            damage_per_tick: toxin_small_field_damage(anthrax),
            activate_frame,
            expires_frame: activate_frame.saturating_add(TOXIN_SMALL_FIELD_DURATION_FRAMES),
            next_tick_frame: activate_frame,
            anthrax_tier: anthrax,
            from_death: true,
            total_damage_applied: 0.0,
            damage_applications: 0,
            objects_destroyed: 0,
        };
        self.active.push(zone);
        self.zones_spawned = self.zones_spawned.saturating_add(1);
        self.death_fields_spawned = self.death_fields_spawned.saturating_add(1);
        if anthrax.is_gamma() {
            self.gamma_fields_spawned = self.gamma_fields_spawned.saturating_add(1);
        }
        id
    }

    pub fn record_gamma_stream_fire(&mut self) {
        self.gamma_stream_fires = self.gamma_stream_fires.saturating_add(1);
    }

    pub fn honesty_gamma_ok(&self) -> bool {
        self.gamma_stream_fires > 0 || self.gamma_fields_spawned > 0
    }

    pub fn plan_due_ticks(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool, bool)],
    ) -> Vec<HostToxinTractorPoisonTickPlan> {
        // object_positions: (id, pos, team, alive, airborne)
        let mut plans = Vec::new();
        for zone in &self.active {
            if !zone.is_due_tick(current_frame) {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, _team, alive, airborne) in object_positions {
                if !alive || id == zone.source_object || airborne {
                    continue;
                }
                let dx = zone.position.x - pos.x;
                let dz = zone.position.z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                if dist <= zone.radius {
                    hits.push(HostToxinTractorPoisonHit {
                        target_id: id,
                        damage: zone.damage_per_tick,
                        zone_id: zone.id,
                    });
                }
            }
            plans.push(HostToxinTractorPoisonTickPlan {
                zone_id: zone.id,
                source_object: zone.source_object,
                source_team: zone.source_team,
                hits,
            });
        }
        plans.sort_by_key(|p| p.zone_id);
        plans
    }

    pub fn record_tick_complete(
        &mut self,
        zone_id: u32,
        total_damage: f32,
        applications: u32,
        objects_destroyed: u32,
        current_frame: u32,
    ) {
        if let Some(zone) = self.active.iter_mut().find(|z| z.id == zone_id) {
            zone.total_damage_applied += total_damage;
            zone.damage_applications += applications;
            zone.objects_destroyed += objects_destroyed;
            let interval = if zone.from_death {
                TOXIN_SMALL_FIELD_TICK_FRAMES
            } else {
                TOXIN_MED_FIELD_TICK_FRAMES
            };
            zone.next_tick_frame = current_frame.saturating_add(interval);
            self.total_damage_applied += total_damage;
            self.damage_applications = self.damage_applications.saturating_add(applications);
            self.objects_destroyed = self.objects_destroyed.saturating_add(objects_destroyed);
        }
    }

    pub fn prune_expired(&mut self, current_frame: u32) {
        let before = self.active.len();
        self.active.retain(|z| !z.is_expired(current_frame));
        self.expirations = self
            .expirations
            .saturating_add((before.saturating_sub(self.active.len())) as u32);
    }

    pub fn honesty_stream_ok(&self) -> bool {
        self.stream_fires > 0
    }

    pub fn honesty_spray_ok(&self) -> bool {
        // Spray fire residual: units hit and/or FireOCL medium field after MinShots+coast.
        self.spray_fires > 0
            && (self.spray_units_hit > 0
                || self.zones_spawned > 0
                || self.fire_ocl_spawns > 0)
    }

    pub fn honesty_death_field_ok(&self) -> bool {
        self.death_fields_spawned > 0
    }

    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_stream_ok() || self.honesty_spray_ok() || self.honesty_death_field_ok()
    }

    /// Clear residual poison fields whose epicenters fall within cleanup radius.
    ///
    /// Wave 55 clean-up interaction residual (Ambulance / CleanupArea path).
    pub fn clear_fields_in_radius(&mut self, center: (f32, f32), cleanup_radius: f32) -> u32 {
        let before = self.active.len();
        self.active.retain(|z| {
            !toxin_field_clearable_by_cleanup((z.position.x, z.position.z), center, cleanup_radius)
        });
        let cleared = (before.saturating_sub(self.active.len())) as u32;
        self.expirations = self.expirations.saturating_add(cleared);
        cleared
    }
}

/// 2D distance residual.
pub fn in_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

// --- Wave 55 residual honesty packs (retail INI constants) ---

/// Contaminate puddle poison field residual (amount/radius/duration/tick/OCL).
pub fn honesty_toxin_contaminate_puddle_residual_ok() -> bool {
    (TOXIN_MED_FIELD_DAMAGE - 2.0).abs() < 0.01
        && (TOXIN_MED_FIELD_DAMAGE_UPGRADED - 2.5).abs() < 0.01
        && (TOXIN_MED_FIELD_RADIUS - 80.0).abs() < 0.01
        && TOXIN_MED_FIELD_TICK_FRAMES == 15
        && TOXIN_MED_FIELD_DURATION_FRAMES == 900
        && TOXIN_MED_FIELD_LIFETIME_MS == 30_000
        && TOXIN_MED_FIELD_DURATION_FRAMES == toxin_ms_to_frames(TOXIN_MED_FIELD_LIFETIME_MS)
        && (TOXIN_MED_FIELD_MAX_HEALTH - 100.0).abs() < 0.01
        && (TOXIN_MED_FIELD_MAX_HEALTH_UPGRADED - 120.0).abs() < 0.01
        && (TOXIN_MED_FIELD_GEOMETRY_RADIUS - 40.0).abs() < 0.01
        && (TOXIN_SMALL_FIELD_DAMAGE - 2.0).abs() < 0.01
        && (TOXIN_SMALL_FIELD_DAMAGE_UPGRADED - 2.5).abs() < 0.01
        && (TOXIN_SMALL_FIELD_RADIUS - 12.0).abs() < 0.01
        && (TOXIN_SMALL_FIELD_RADIUS_UPGRADED - 7.5).abs() < 0.01
        && TOXIN_SMALL_FIELD_DURATION_FRAMES == 300
        && TOXIN_SMALL_FIELD_LIFETIME_MS == 10_000
        && TOXIN_SMALL_FIELD_DURATION_FRAMES == toxin_ms_to_frames(TOXIN_SMALL_FIELD_LIFETIME_MS)
        && (toxin_small_field_radius(AnthraxResidualTier::None) - 12.0).abs() < 0.01
        && (toxin_small_field_radius(AnthraxResidualTier::Beta) - 7.5).abs() < 0.01
        && (toxin_small_field_radius(AnthraxResidualTier::Gamma) - 7.5).abs() < 0.01
        && TOXIN_OCL_POISON_FIELD_MEDIUM == "OCL_PoisonFieldMedium"
        && TOXIN_OCL_POISON_FIELD_UPGRADED_MEDIUM == "OCL_PoisonFieldUpgradedMedium"
        && TOXIN_OCL_POISON_FIELD_GAMMA_MEDIUM == "OCL_PoisonFieldGammaMedium"
        && toxin_spray_ocl_name(AnthraxResidualTier::None) == TOXIN_OCL_POISON_FIELD_MEDIUM
        && toxin_spray_ocl_name(AnthraxResidualTier::Beta) == TOXIN_OCL_POISON_FIELD_UPGRADED_MEDIUM
        && toxin_spray_ocl_name(AnthraxResidualTier::Gamma) == TOXIN_OCL_POISON_FIELD_GAMMA_MEDIUM
}

/// Spray weapon residual (coast / min shots / range / salvage matrix).
pub fn honesty_toxin_spray_weapon_residual_ok() -> bool {
    (TOXIN_SPRAY_DAMAGE - 2.0).abs() < 0.01
        && (TOXIN_SPRAY_DAMAGE_UPGRADED - 2.5).abs() < 0.01
        && (TOXIN_SPRAY_RADIUS - 75.0).abs() < 0.01
        && (TOXIN_SPRAY_RANGE - 15.0).abs() < 0.01
        && TOXIN_SPRAY_DELAY_MS == 200
        && TOXIN_SPRAY_DELAY_FRAMES == toxin_ms_to_frames(TOXIN_SPRAY_DELAY_MS)
        && TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_MS == 300
        && TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_FRAMES
            == toxin_ms_to_frames(TOXIN_SPRAY_CONTINUOUS_FIRE_COAST_MS)
        && TOXIN_SPRAY_MIN_SHOTS_TO_CREATE_OCL == 4
        && toxin_spray_ready_for_ocl(4)
        && !toxin_spray_ready_for_ocl(3)
        && TOXIN_SPRAY_OCL_LIFETIME_PER_SECOND_MS == 10_000
        && TOXIN_SPRAY_OCL_LIFETIME_MAX_CAP_MS == 180_000
        && TOXIN_SPRAY_OCL_LIFETIME_MAX_CAP_FRAMES
            == toxin_ms_to_frames(TOXIN_SPRAY_OCL_LIFETIME_MAX_CAP_MS)
        && (toxin_spray_damage_with_salvage(
            ToxinTractorSalvageTier::Two,
            AnthraxResidualTier::None,
        ) - 3.0)
            .abs()
            < 0.01
        && (toxin_spray_damage_with_salvage(
            ToxinTractorSalvageTier::Two,
            AnthraxResidualTier::Beta,
        ) - 4.0)
            .abs()
            < 0.01
        && (toxin_spray_damage_with_salvage(
            ToxinTractorSalvageTier::Two,
            AnthraxResidualTier::Gamma,
        ) - 4.5)
            .abs()
            < 0.01
        && (TOXIN_SPRAY_ACCEPTABLE_AIM_DELTA_DEG - 180.0).abs() < 0.01
}

/// Upgrade anthrax residual (stream salvage matrix + death types + OCL).
pub fn honesty_toxin_anthrax_upgrade_residual_ok() -> bool {
    (toxin_stream_damage(ToxinTractorSalvageTier::Base, AnthraxResidualTier::Beta) - 12.5).abs()
        < 0.01
        && (toxin_stream_damage(ToxinTractorSalvageTier::One, AnthraxResidualTier::Beta) - 15.0)
            .abs()
            < 0.01
        && (toxin_stream_damage(ToxinTractorSalvageTier::Two, AnthraxResidualTier::Beta) - 20.0)
            .abs()
            < 0.01
        && (toxin_stream_damage(ToxinTractorSalvageTier::Base, AnthraxResidualTier::Gamma) - 20.5)
            .abs()
            < 0.01
        && (toxin_stream_damage(ToxinTractorSalvageTier::One, AnthraxResidualTier::Gamma) - 24.5)
            .abs()
            < 0.01
        && (toxin_stream_damage(ToxinTractorSalvageTier::Two, AnthraxResidualTier::Gamma) - 28.5)
            .abs()
            < 0.01
        && toxin_death_type_name(AnthraxResidualTier::None) == TOXIN_DEATH_TYPE_POISONED
        && toxin_death_type_name(AnthraxResidualTier::Beta) == TOXIN_DEATH_TYPE_POISONED_BETA
        && toxin_death_type_name(AnthraxResidualTier::Gamma) == TOXIN_DEATH_TYPE_POISONED_GAMMA
        && toxin_death_weapon_name(AnthraxResidualTier::None) == TOXIN_SHELL_WEAPON
        && toxin_death_weapon_name(AnthraxResidualTier::Beta) == TOXIN_SHELL_WEAPON_UPGRADED
        && toxin_death_weapon_name(AnthraxResidualTier::Gamma) == TOXIN_SHELL_WEAPON_GAMMA
        && toxin_death_ocl_name(AnthraxResidualTier::Gamma) == TOXIN_OCL_POISON_FIELD_GAMMA_SMALL
        && toxin_field_ambient_audio(AnthraxResidualTier::Beta) == TOXIN_ANTHRAX_POOL_AUDIO
        && toxin_field_ambient_audio(AnthraxResidualTier::None) == TOXIN_POISON_AUDIO
        && TOXIN_STREAM_CLIP_SIZE == 30
        && TOXIN_STREAM_DELAY_MS == 40
        && TOXIN_STREAM_DELAY_FRAMES == toxin_ms_to_frames(TOXIN_STREAM_DELAY_MS)
        && TOXIN_STREAM_CLIP_RELOAD_FRAMES == toxin_ms_to_frames(TOXIN_STREAM_CLIP_RELOAD_MS)
        && (TOXIN_STREAM_WEAPON_SPEED - 600.0).abs() < 0.01
        && TOXIN_STREAM_ALLOW_ATTACK_GARRISONED
        && (TOXIN_STREAM_MIN_RANGE_UPGRADED - 10.0).abs() < 0.01
}

/// Clean-up interaction residual (CLEANUP_HAZARD name + clear-in-radius).
pub fn honesty_toxin_cleanup_interaction_residual_ok() -> bool {
    TOXIN_FIELD_KINDOF_CLEANUP_HAZARD == "CLEANUP_HAZARD"
        && TOXIN_FIELD_ARMOR == "HazardousMaterialArmor"
        && TOXIN_FIELD_CORE_WEAPON == "HazardFieldCoreWeapon"
        && is_toxin_cleanup_hazard_name("PoisonFieldMedium")
        && is_toxin_cleanup_hazard_name("PoisonFieldUpgradedSmall")
        && is_toxin_cleanup_hazard_name("Chem_PoisonFieldGammaMedium")
        && !is_toxin_cleanup_hazard_name("GLAVehicleToxinTruck")
        && !is_toxin_cleanup_hazard_name("USA_Ranger")
        && toxin_field_clearable_by_cleanup((0.0, 0.0), (0.0, 0.0), 50.0)
        && !toxin_field_clearable_by_cleanup((100.0, 0.0), (0.0, 0.0), 50.0)
}

/// Combined Wave 55 toxin residual honesty pack.
pub fn honesty_toxin_tractor_residual_pack_ok() -> bool {
    honesty_toxin_contaminate_puddle_residual_ok()
        && honesty_toxin_spray_weapon_residual_ok()
        && honesty_toxin_anthrax_upgrade_residual_ok()
        && honesty_toxin_cleanup_interaction_residual_ok()
}

#[cfg(test)]
mod tests {
    #[test]
    fn fire_ocl_min_shots_and_lifetime() {
        let mut d = super::HostFireOclAfterCooldownData::new();
        assert!(d.try_fire_ocl_on_cooldown(10).is_none());
        for f in 0..3u32 {
            d.record_shot(f);
        }
        // Only 3 shots < 4.
        assert!(d.try_fire_ocl_on_cooldown(20).is_none());
        for f in 0..4u32 {
            d.record_shot(100 + f);
        }
        let life = d.try_fire_ocl_on_cooldown(110).expect("ocl");
        assert!(life >= 1);
        // 10 frames = 1/3 sec * 10 lifetime_per_sec = ~3.33 sec → ~100 frames, capped.
        assert!(life <= 5400); // max cap 180s
    }

    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn toxin_tractor_name_matrix() {
        assert!(is_toxin_tractor_template("GLAVehicleToxinTruck"));
        assert!(is_toxin_tractor_template("GLA_ToxinTruck"));
        assert!(is_toxin_tractor_template("TestToxinTruck"));
        assert!(is_toxin_tractor_template("Chem_GLAVehicleToxinTruck"));
        assert!(is_toxin_tractor_template("Demo_GLAVehicleToxinTruck"));
        assert!(is_toxin_tractor_template("Slth_GLAVehicleToxinTruck"));
        assert!(!is_toxin_tractor_template("ToxinTruckGun"));
        assert!(!is_toxin_tractor_template("ToxinTruckSprayer"));
        assert!(!is_toxin_tractor_template("PoisonFieldMedium"));
        assert!(!is_toxin_tractor_template("ToxinShellWeapon"));
        assert!(!is_toxin_tractor_template("GLAVehicleScudLauncher"));
        assert!(!is_toxin_tractor_template("USA_Ranger"));
    }

    #[test]
    fn stream_and_spray_stats() {
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Base, AnthraxResidualTier::None) - 10.0)
                .abs()
                < 0.01
        );
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Base, AnthraxResidualTier::Beta) - 12.5)
                .abs()
                < 0.01
        );
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Base, AnthraxResidualTier::Gamma) - 20.5)
                .abs()
                < 0.01
        );
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Two, AnthraxResidualTier::None) - 15.0)
                .abs()
                < 0.01
        );
        assert!((toxin_spray_damage(AnthraxResidualTier::None) - 2.0).abs() < 0.01);
        assert!((toxin_spray_damage(AnthraxResidualTier::Beta) - 2.5).abs() < 0.01);
        assert!((toxin_spray_damage(AnthraxResidualTier::Gamma) - 2.5).abs() < 0.01);
        assert!((toxin_med_field_damage(AnthraxResidualTier::None) - 2.0).abs() < 0.01);
        assert!((toxin_med_field_damage(AnthraxResidualTier::Gamma) - 2.5).abs() < 0.01);
        assert!((toxin_stream_damage_at(5.0, 10.0) - 10.0).abs() < 0.01);
        assert!((toxin_stream_damage_at(15.0, 10.0)).abs() < 0.01);
        assert!((toxin_spray_damage_at(50.0, 2.0) - 2.0).abs() < 0.01);
        assert!((toxin_spray_damage_at(80.0, 2.0)).abs() < 0.01);
        assert!(should_apply_toxin_spray(true, 1));
        assert!(!should_apply_toxin_spray(true, 0));
        assert!(should_apply_toxin_stream(true, 0));
        assert!(is_chem_general_template("Chem_GLAVehicleToxinTruck"));
        assert!(is_anthrax_gamma_upgrade_name(
            "Chem_Upgrade_GLAAnthraxGamma"
        ));
        assert!(is_anthrax_gamma_upgrade_name("Upgrade_GLAAnthraxGamma"));
        assert!(!is_anthrax_gamma_upgrade_name("Upgrade_GLAAnthraxBeta"));
        assert_eq!(
            anthrax_tier_from_flags(false, false, true),
            AnthraxResidualTier::Beta
        );
        assert_eq!(
            anthrax_tier_from_flags(true, true, true),
            AnthraxResidualTier::Gamma
        );
    }

    #[test]
    fn registry_spawn_and_honesty() {
        let mut reg = HostToxinTractorRegistry::new();
        let id = reg.spawn_medium_field(
            ObjectId(1),
            Team::GLA,
            Vec3::ZERO,
            0,
            AnthraxResidualTier::Gamma,
        );
        assert_eq!(id, 0);
        assert!((reg.active_zones()[0].damage_per_tick - 2.5).abs() < 0.01);
        assert!(reg.active_zones()[0].anthrax_upgraded());
        assert!(reg.honesty_gamma_ok());
        reg.record_stream_fire(1);
        assert!(reg.honesty_stream_ok());
        let _ = reg.spawn_medium_field(
            ObjectId(1),
            Team::GLA,
            Vec3::ZERO,
            0,
            AnthraxResidualTier::None,
        );
        reg.record_spray_fire(2);
        assert!(reg.honesty_spray_ok());
        let _ = reg.spawn_death_field(
            ObjectId(1),
            Team::GLA,
            Vec3::ZERO,
            0,
            AnthraxResidualTier::None,
        );
        assert!(reg.honesty_death_field_ok());
        assert!(reg.honesty_host_path_ok());
        // gamma medium + base medium + death field
        assert_eq!(reg.active_count(), 3);
    }

    #[test]
    fn toxin_residual_pack_honesty() {
        assert!(honesty_toxin_contaminate_puddle_residual_ok());
        assert!(honesty_toxin_spray_weapon_residual_ok());
        assert!(honesty_toxin_anthrax_upgrade_residual_ok());
        assert!(honesty_toxin_cleanup_interaction_residual_ok());
        assert!(honesty_toxin_tractor_residual_pack_ok());
    }

    #[test]
    fn toxin_salvage_anthrax_matrix_and_small_radius() {
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Two, AnthraxResidualTier::Beta) - 20.0)
                .abs()
                < 0.01
        );
        assert!(
            (toxin_stream_damage(ToxinTractorSalvageTier::Two, AnthraxResidualTier::Gamma) - 28.5)
                .abs()
                < 0.01
        );
        assert!(
            (toxin_spray_damage_with_salvage(
                ToxinTractorSalvageTier::One,
                AnthraxResidualTier::Gamma
            ) - 3.5)
                .abs()
                < 0.01
        );
        let mut reg = HostToxinTractorRegistry::new();
        let _ = reg.spawn_death_field(
            ObjectId(1),
            Team::GLA,
            Vec3::new(0.0, 0.0, 0.0),
            0,
            AnthraxResidualTier::Beta,
        );
        assert!((reg.active_zones()[0].radius - 7.5).abs() < 0.01);
        let cleared = reg.clear_fields_in_radius((0.0, 0.0), 50.0);
        assert_eq!(cleared, 1);
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn toxin_ms_to_frames_matches_retail_delays() {
        assert_eq!(toxin_ms_to_frames(40), 2);
        assert_eq!(toxin_ms_to_frames(200), 6);
        assert_eq!(toxin_ms_to_frames(300), 9);
        assert_eq!(toxin_ms_to_frames(500), 15);
        assert_eq!(toxin_ms_to_frames(10_000), 300);
        assert_eq!(toxin_ms_to_frames(30_000), 900);
    }
}
