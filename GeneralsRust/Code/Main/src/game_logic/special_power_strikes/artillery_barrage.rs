//! ArtilleryBarrage science-tier residual constants and helpers.
use super::types::*;
// --- Artillery Barrage scatter multi-shell residual (retail SUPERWEAPON_ArtilleryBarrage1) ---

/// Retail `SUPERWEAPON_ArtilleryBarrage1` FormationSize (Level1).
pub const ARTILLERY_BARRAGE_SHELL_COUNT: u32 = 12;
/// Retail `SUPERWEAPON_ArtilleryBarrage2` FormationSize.
pub const ARTILLERY_BARRAGE_SHELL_COUNT_L2: u32 = 24;
/// Retail `SUPERWEAPON_ArtilleryBarrage3` FormationSize.
pub const ARTILLERY_BARRAGE_SHELL_COUNT_L3: u32 = 36;

// --- Wave 78: ArtilleryBarrage science-tier name / OCL / decal residual deepen ---
/// Retail SCIENCE_ArtilleryBarrage1 residual.
pub const ARTILLERY_SCIENCE_TIER1: &str = "SCIENCE_ArtilleryBarrage1";
/// Retail SCIENCE_ArtilleryBarrage2 residual.
pub const ARTILLERY_SCIENCE_TIER2: &str = "SCIENCE_ArtilleryBarrage2";
/// Retail SCIENCE_ArtilleryBarrage3 residual.
pub const ARTILLERY_SCIENCE_TIER3: &str = "SCIENCE_ArtilleryBarrage3";
/// Retail SUPERWEAPON_ArtilleryBarrage1 OCL residual.
pub const ARTILLERY_OCL_TIER1: &str = "SUPERWEAPON_ArtilleryBarrage1";
/// Retail SUPERWEAPON_ArtilleryBarrage2 OCL residual.
pub const ARTILLERY_OCL_TIER2: &str = "SUPERWEAPON_ArtilleryBarrage2";
/// Retail SUPERWEAPON_ArtilleryBarrage3 OCL residual.
pub const ARTILLERY_OCL_TIER3: &str = "SUPERWEAPON_ArtilleryBarrage3";
/// Retail SciencePurchasePointCost residual (all ArtilleryBarrage tiers).
pub const ARTILLERY_SCIENCE_POINT_COST: u32 = 1;
/// Retail SCIENCE_ArtilleryBarrage1 PrerequisiteSciences residual tokens.
pub const ARTILLERY_SCIENCE1_PREREQ: [&str; 2] = ["SCIENCE_CHINA", "SCIENCE_Rank3"];
/// Retail SCIENCE_ArtilleryBarrage2 PrerequisiteSciences residual tokens.
pub const ARTILLERY_SCIENCE2_PREREQ: [&str; 2] = ["SCIENCE_ArtilleryBarrage1", "SCIENCE_Rank3"];
/// Retail SCIENCE_ArtilleryBarrage3 PrerequisiteSciences residual tokens.
pub const ARTILLERY_SCIENCE3_PREREQ: [&str; 2] = ["SCIENCE_ArtilleryBarrage2", "SCIENCE_Rank3"];
/// Retail DeliveryDecal Texture residual (all Artillery OCL tiers).
pub const ARTILLERY_DELIVERY_DECAL_TEXTURE: &str = "SCCArtilleryBarrage_China";
/// Retail DeliveryDecal Style residual.
pub const ARTILLERY_DELIVERY_DECAL_STYLE: &str = "SHADOW_ALPHA_DECAL";
/// Retail DeliveryDecal OpacityMin residual (percent).
pub const ARTILLERY_DELIVERY_DECAL_OPACITY_MIN_PCT: u32 = 25;
/// Retail DeliveryDecal OpacityMax residual (percent).
pub const ARTILLERY_DELIVERY_DECAL_OPACITY_MAX_PCT: u32 = 50;
/// Retail DeliveryDecal OpacityThrobTime residual (msec).
pub const ARTILLERY_DELIVERY_DECAL_THROB_MS: u32 = 500;
/// Retail DeliveryDecal Color residual (R:255 G:156 B:0 A:255).
pub const ARTILLERY_DELIVERY_DECAL_COLOR: (u8, u8, u8, u8) = (255, 156, 0, 255);
/// Retail VisibleNumBones residual (all Artillery OCL tiers).
pub const ARTILLERY_VISIBLE_NUM_BONES: u32 = 1;
/// Retail VisibleItemsDroppedPerInterval residual.
pub const ARTILLERY_VISIBLE_ITEMS_DROPPED_PER_INTERVAL: u32 = 1;
/// Retail SuperweaponArtilleryBarrage ViewObjectDuration residual (msec).
pub const ARTILLERY_VIEW_OBJECT_DURATION_MS: u32 = 30_000;
/// ViewObjectDuration 30000ms → 900 frames @ 30 FPS.
pub const ARTILLERY_VIEW_OBJECT_DURATION_FRAMES: u32 = 900;
/// Retail SuperweaponArtilleryBarrage ViewObjectRange residual.
pub const ARTILLERY_VIEW_OBJECT_RANGE: f32 = 250.0;

/// Residual Artillery Barrage science tier (FormationSize 12/24/36).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ArtilleryBarrageScienceTier {
    #[default]
    Level1,
    Level2,
    Level3,
}

impl ArtilleryBarrageScienceTier {
    /// Retail FormationSize for this science tier.
    pub fn formation_size(self) -> u32 {
        match self {
            ArtilleryBarrageScienceTier::Level1 => ARTILLERY_BARRAGE_SHELL_COUNT,
            ArtilleryBarrageScienceTier::Level2 => ARTILLERY_BARRAGE_SHELL_COUNT_L2,
            ArtilleryBarrageScienceTier::Level3 => ARTILLERY_BARRAGE_SHELL_COUNT_L3,
        }
    }

    /// Retail science residual name for this tier.
    pub fn science_name(self) -> &'static str {
        match self {
            ArtilleryBarrageScienceTier::Level1 => ARTILLERY_SCIENCE_TIER1,
            ArtilleryBarrageScienceTier::Level2 => ARTILLERY_SCIENCE_TIER2,
            ArtilleryBarrageScienceTier::Level3 => ARTILLERY_SCIENCE_TIER3,
        }
    }

    /// Retail SUPERWEAPON_ArtilleryBarrageN OCL residual name.
    pub fn ocl_name(self) -> &'static str {
        match self {
            ArtilleryBarrageScienceTier::Level1 => ARTILLERY_OCL_TIER1,
            ArtilleryBarrageScienceTier::Level2 => ARTILLERY_OCL_TIER2,
            ArtilleryBarrageScienceTier::Level3 => ARTILLERY_OCL_TIER3,
        }
    }

    /// Map SCIENCE_ArtilleryBarrage1/2/3 (or generic name residual) to tier.
    /// Higher tiers win when multiple sciences are present (caller should pass highest).
    pub fn from_science_name(name: &str) -> Option<Self> {
        let n: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        if n.contains("artillerybarrage3") {
            Some(ArtilleryBarrageScienceTier::Level3)
        } else if n.contains("artillerybarrage2") {
            Some(ArtilleryBarrageScienceTier::Level2)
        } else if n.contains("artillerybarrage1") || n.contains("artillerybarrage") {
            Some(ArtilleryBarrageScienceTier::Level1)
        } else {
            None
        }
    }

    /// Select highest unlocked ArtilleryBarrage science tier from a science name list.
    pub fn highest_from_sciences<'a, I>(sciences: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best = ArtilleryBarrageScienceTier::Level1;
        for s in sciences {
            if let Some(t) = Self::from_science_name(s) {
                best = match (best, t) {
                    (_, ArtilleryBarrageScienceTier::Level3)
                    | (ArtilleryBarrageScienceTier::Level3, _) => {
                        ArtilleryBarrageScienceTier::Level3
                    }
                    (_, ArtilleryBarrageScienceTier::Level2)
                    | (ArtilleryBarrageScienceTier::Level2, _) => {
                        ArtilleryBarrageScienceTier::Level2
                    }
                    _ => ArtilleryBarrageScienceTier::Level1,
                };
            }
        }
        best
    }
}
/// Retail `ArtilleryBarrageDamageWeapon` PrimaryDamage.
pub const ARTILLERY_BARRAGE_DAMAGE: f32 = 105.0;
/// Retail `ArtilleryBarrageDamageWeapon` PrimaryDamageRadius.
pub const ARTILLERY_BARRAGE_RADIUS: f32 = 50.0;
/// Retail DeliverPayload `WeaponErrorRadius` (shell scatter radius around target).
pub const ARTILLERY_BARRAGE_ERROR_RADIUS: f32 = 100.0;
/// Retail DeliverPayload `DelayDeliveryMax` = 3000 ms → 90 frames @ 30 FPS.
/// Used as: (1) base reaction/approach residual before first shell, and
/// (2) max additional per-shell DelayDelivery stagger after that base.
pub const ARTILLERY_BARRAGE_IMPACT_DELAY_FRAMES: u32 = 90;
/// Legacy ring radius used by older residual placement (pre WeaponErrorRadius draw).
/// Kept for honesty/tests that still reference the constant name.
pub const ARTILLERY_BARRAGE_RING_RADIUS: f32 = 75.0;
/// Retail DeliverPayload DelayDeliveryMax msec residual.
pub const ARTILLERY_BARRAGE_DELAY_DELIVERY_MAX_MS: u32 = 3000;
/// Retail DelayDeliveryMin residual (not set in INI; C++ only exposes Max; host 0).
pub const ARTILLERY_BARRAGE_DELAY_DELIVERY_MIN_FRAMES: u32 = 0;
/// Retail SUPERWEAPON_ArtilleryBarrage Transport residual.
pub const ARTILLERY_BARRAGE_TRANSPORT: &str = "ChinaArtilleryCannon";
/// Retail VisiblePayloadTemplateName residual.
pub const ARTILLERY_BARRAGE_SHELL_OBJECT: &str = "ChinaArtilleryBarrageShell";
/// Retail VisiblePayloadWeaponTemplate residual.
pub const ARTILLERY_BARRAGE_WEAPON_NAME: &str = "ArtilleryBarrageDamageWeapon";
/// Retail ChinaArtilleryBarrageCannonLocomotor PreferredHeight residual.
pub const ARTILLERY_BARRAGE_PREFERRED_HEIGHT: f32 = 500.0;
/// Retail DeliverPayload DeliveryDistance residual.
pub const ARTILLERY_BARRAGE_DELIVERY_DISTANCE: f32 = 250.0;
/// Retail PreOpenDistance residual: SUPERWEAPON_ArtilleryBarrage1/2/3 do not
/// set PreOpenDistance (defaults 0), so the inbound band expansion of C++
/// isCloseEnoughToTarget is the identity here.
pub const ARTILLERY_BARRAGE_PRE_OPEN_DISTANCE: f32 = 0.0;
/// Retail DeliveryDecalRadius residual.
pub const ARTILLERY_BARRAGE_DECAL_RADIUS: f32 = 125.0;
/// Retail FormationSpacing residual.
pub const ARTILLERY_BARRAGE_FORMATION_SPACING: f32 = 1.0;
/// Retail ExitPitchRate residual.
pub const ARTILLERY_BARRAGE_EXIT_PITCH_RATE: f32 = 30.0;
/// Retail ChinaArtilleryBarrageCannonLocomotor template residual.
pub const ARTILLERY_BARRAGE_LOCOMOTOR: &str = "ChinaArtilleryBarrageCannonLocomotor";
/// Retail ChinaArtilleryBarrageCannonLocomotor Speed residual.
pub const ARTILLERY_BARRAGE_LOCOMOTOR_SPEED: f32 = 150.0;
/// Retail ProjectileDetonationFX residual.
pub const ARTILLERY_BARRAGE_FIRE_FX: &str = "FX_ArtilleryBarrage";
/// Retail SuperweaponArtilleryBarrage InitiateSound residual.
pub const ARTILLERY_BARRAGE_INITIATE_SOUND: &str = "FireArtilleryCannonSound";
/// Wave 77: ArtilleryBarrage has no InitiateAtLocationSound residual in SpecialPower.ini.
pub const ARTILLERY_BARRAGE_INITIATE_AT_LOCATION_SOUND: &str = "";
/// Retail SuperweaponArtilleryBarrage ReloadTime residual (msec).
pub const ARTILLERY_BARRAGE_RELOAD_MS: u32 = 300000;
/// ReloadTime frames residual (300000 ms → 9000 @ 30 FPS).
pub const ARTILLERY_BARRAGE_RELOAD_FRAMES: u32 = 9000;
/// Retail SuperweaponArtilleryBarrage RadiusCursorRadius residual.
pub const ARTILLERY_BARRAGE_RADIUS_CURSOR: f32 = 125.0;
/// Retail ChinaArtilleryCannon MaxHealth residual.
pub const ARTILLERY_BARRAGE_CANNON_MAX_HEALTH: f32 = 200.0;
/// Retail ChinaArtilleryCannon KindOf residual (honesty substring).
pub const ARTILLERY_BARRAGE_CANNON_KIND_OF: &str =
    "PRELOAD CAN_ATTACK VEHICLE AIRCRAFT UNATTACKABLE IGNORED_IN_GUI EMP_HARDENED";
