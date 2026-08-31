//! A10 Thunderbolt science-tier residual constants and helpers.
use super::types::*;
// --- A10 Thunderbolt special-power residual pack (Wave 72) ---

/// Retail SuperweaponA10ThunderboltMissileStrike ReloadTime residual (msec).
pub const A10_STRIKE_RELOAD_MS: u32 = 240_000;
/// ReloadTime frames residual (240000 ms → 7200 @ 30 FPS).
pub const A10_STRIKE_RELOAD_FRAMES: u32 = 7_200;
/// Retail SuperweaponA10ThunderboltMissileStrike RadiusCursorRadius residual.
pub const A10_STRIKE_RADIUS_CURSOR: f32 = 50.0;
/// Retail RequiredScience residual (tier 1).
pub const A10_STRIKE_REQUIRED_SCIENCE: &str = "SCIENCE_A10ThunderboltMissileStrike1";
/// Retail SpecialPower template residual name.
pub const A10_STRIKE_SPECIAL_POWER: &str = "SuperweaponA10ThunderboltMissileStrike";
/// Retail ViewObjectDuration residual (msec).
pub const A10_STRIKE_VIEW_OBJECT_DURATION_MS: u32 = 30_000;
/// ViewObjectDuration frames residual (30000 ms → 900).
pub const A10_STRIKE_VIEW_OBJECT_DURATION_FRAMES: u32 = 900;
/// Retail ViewObjectRange residual.
pub const A10_STRIKE_VIEW_OBJECT_RANGE: f32 = 250.0;
/// Retail SharedSyncedTimer residual.
pub const A10_STRIKE_SHARED_SYNCED_TIMER: bool = true;
/// Retail PublicTimer residual.
pub const A10_STRIKE_PUBLIC_TIMER: bool = false;
/// Retail ShortcutPower residual.
pub const A10_STRIKE_SHORTCUT_POWER: bool = true;
/// Host residual aggregate max damage at epicenter (multi-missile residual path).
pub const A10_STRIKE_HOST_MAX_DAMAGE: f32 = 500.0;
/// Host residual outer damage radius for multi-missile residual path.
pub const A10_STRIKE_HOST_RADIUS: f32 = 100.0;
/// Host residual inner falloff radius.
pub const A10_STRIKE_HOST_INNER_RADIUS: f32 = 40.0;
/// A-10 flight/approach residual frames (shorter than full aircraft OCL).
pub const A10_STRIKE_IMPACT_DELAY_FRAMES: u32 = 60;
/// Retail A10ThunderboltMissileWeapon PrimaryDamage residual (per missile).
pub const A10_MISSILE_PRIMARY_DAMAGE: f32 = 200.0;
/// Retail A10ThunderboltMissileWeapon PrimaryDamageRadius residual.
pub const A10_MISSILE_PRIMARY_RADIUS: f32 = 50.0;
/// Retail A10ThunderboltMissileWeapon ClipReloadTime residual (msec).
pub const A10_MISSILE_CLIP_RELOAD_MS: u32 = 20_000;
/// ClipReloadTime frames residual (20000 ms → 600).
pub const A10_MISSILE_CLIP_RELOAD_FRAMES: u32 = 600;
/// Retail A10ThunderboltVulcan PrimaryDamage residual.
pub const A10_VULCAN_PRIMARY_DAMAGE: f32 = 10.0;
/// Retail A10ThunderboltVulcan PrimaryDamageRadius residual.
pub const A10_VULCAN_PRIMARY_RADIUS: f32 = 4.0;
/// Retail A10ThunderboltVulcan DelayBetweenShots residual (msec).
pub const A10_VULCAN_DELAY_BETWEEN_SHOTS_MS: u32 = 60;
/// C++ has no consolidated A10 impact cue (per-missile FX_A10ThunderboltMissileExplosion).
pub const A10_STRIKE_IMPACT_AUDIO: &str = "";

// --- Wave 76: A10 science-tier FormationSize residual pack ---

/// Retail SCIENCE_A10ThunderboltMissileStrike1 residual.
pub const A10_SCIENCE_TIER1: &str = "SCIENCE_A10ThunderboltMissileStrike1";
/// Retail SCIENCE_A10ThunderboltMissileStrike2 residual.
pub const A10_SCIENCE_TIER2: &str = "SCIENCE_A10ThunderboltMissileStrike2";
/// Retail SCIENCE_A10ThunderboltMissileStrike3 residual.
pub const A10_SCIENCE_TIER3: &str = "SCIENCE_A10ThunderboltMissileStrike3";
/// Retail SUPERWEAPON_A10ThunderboltMissileStrike1 OCL residual.
pub const A10_OCL_TIER1: &str = "SUPERWEAPON_A10ThunderboltMissileStrike1";
/// Retail SUPERWEAPON_A10ThunderboltMissileStrike2 OCL residual.
pub const A10_OCL_TIER2: &str = "SUPERWEAPON_A10ThunderboltMissileStrike2";
/// Retail SUPERWEAPON_A10ThunderboltMissileStrike3 OCL residual.
pub const A10_OCL_TIER3: &str = "SUPERWEAPON_A10ThunderboltMissileStrike3";
/// Retail DeliverPayload FormationSize L1 residual (jets).
pub const A10_FORMATIONION_SIZE_L1: u32 = 1;
/// Retail DeliverPayload FormationSize L2 residual (jets).
pub const A10_FORMATIONION_SIZE_L2: u32 = 2;
/// Retail DeliverPayload FormationSize L3 residual (jets).
pub const A10_FORMATIONION_SIZE_L3: u32 = 3;
/// Retail FormationSpacing residual (all tiers).
pub const A10_FORMATIONION_SPACING: f32 = 35.0;
/// Retail DeliveryDistance residual (all tiers).
pub const A10_DELIVERY_DISTANCE: f32 = 450.0;
/// Retail PreOpenDistance residual: SUPERWEAPON_A10ThunderboltMissileStrike1/2/3
/// do not set PreOpenDistance (defaults 0), so the inbound band expansion of
/// C++ isCloseEnoughToTarget is the identity here.
pub const A10_PRE_OPEN_DISTANCE: f32 = 0.0;
/// Retail DropDelay residual (msec between payload sets; all tiers).
pub const A10_DROP_DELAY_MS: u32 = 500;
/// DropDelay 500ms → 15 frames @ 30 FPS.
pub const A10_DROP_DELAY_FRAMES: u32 = 15;
/// Retail VisibleNumBones residual (missile bones on A10).
pub const A10_VISIBLE_NUM_BONES: u32 = 6;
/// Retail VisibleItemsDroppedPerInterval residual.
pub const A10_VISIBLE_ITEMS_DROPPED_PER_INTERVAL: u32 = 2;
/// Retail DiveStartDistance residual.
pub const A10_DIVE_START_DISTANCE: f32 = 500.0;
/// Retail DiveEndDistance residual.
pub const A10_DIVE_END_DISTANCE: f32 = 300.0;
/// Retail StrafeLength residual.
pub const A10_STRAFE_LENGTH: f32 = 450.0;
/// Retail DeliveryDecal Texture residual.
pub const A10_DELIVERY_DECAL_TEXTURE: &str = "SCCA10Strike_USA";
/// Retail DeliveryDecal Style residual.
pub const A10_DELIVERY_DECAL_STYLE: &str = "SHADOW_ALPHA_DECAL";
/// Retail DeliveryDecal OpacityMin residual (percent).
pub const A10_DELIVERY_DECAL_OPACITY_MIN_PCT: u32 = 25;
/// Retail DeliveryDecal OpacityMax residual (percent).
pub const A10_DELIVERY_DECAL_OPACITY_MAX_PCT: u32 = 50;
/// Retail DeliveryDecal OpacityThrobTime residual (msec).
pub const A10_DELIVERY_DECAL_THROB_MS: u32 = 500;
/// Retail DeliveryDecal Color residual (R:255 G:156 B:0 A:255).
pub const A10_DELIVERY_DECAL_COLOR: (u8, u8, u8, u8) = (255, 156, 0, 255);
/// Retail DeliveryDecalRadius residual (matches RadiusCursor).
pub const A10_DELIVERY_DECAL_RADIUS: f32 = 50.0;
/// Retail Transport residual.
pub const A10_TRANSPORT: &str = "AmericaJetA10Thunderbolt";
/// Retail VisiblePayloadTemplateName residual.
pub const A10_PAYLOAD_TEMPLATE: &str = "A10ThunderboltMissile";
/// Retail VisiblePayloadWeaponTemplate residual.
pub const A10_PAYLOAD_WEAPON: &str = "A10ThunderboltMissileWeapon";

/// Residual A10 science tier (FormationSize 1/2/3 jets).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum A10StrikeScienceTier {
    /// SCIENCE_A10ThunderboltMissileStrike1 → FormationSize **1**.
    #[default]
    Level1,
    /// SCIENCE_A10ThunderboltMissileStrike2 → FormationSize **2**.
    Level2,
    /// SCIENCE_A10ThunderboltMissileStrike3 → FormationSize **3**.
    Level3,
}

impl A10StrikeScienceTier {
    /// Retail DeliverPayload FormationSize for this science tier.
    pub fn formation_size(self) -> u32 {
        match self {
            A10StrikeScienceTier::Level1 => A10_FORMATIONION_SIZE_L1,
            A10StrikeScienceTier::Level2 => A10_FORMATIONION_SIZE_L2,
            A10StrikeScienceTier::Level3 => A10_FORMATIONION_SIZE_L3,
        }
    }

    /// Retail science residual name for this tier.
    pub fn science_name(self) -> &'static str {
        match self {
            A10StrikeScienceTier::Level1 => A10_SCIENCE_TIER1,
            A10StrikeScienceTier::Level2 => A10_SCIENCE_TIER2,
            A10StrikeScienceTier::Level3 => A10_SCIENCE_TIER3,
        }
    }

    /// Retail SUPERWEAPON_A10ThunderboltMissileStrikeN OCL residual name.
    pub fn ocl_name(self) -> &'static str {
        match self {
            A10StrikeScienceTier::Level1 => A10_OCL_TIER1,
            A10StrikeScienceTier::Level2 => A10_OCL_TIER2,
            A10StrikeScienceTier::Level3 => A10_OCL_TIER3,
        }
    }

    /// Map SCIENCE_A10ThunderboltMissileStrike1/2/3 (or AirF residual) to tier.
    pub fn from_science_name(name: &str) -> Option<Self> {
        let n = name.to_ascii_lowercase();
        if n.contains("a10thunderboltmissilestrike3") || n.ends_with("strike3") {
            Some(A10StrikeScienceTier::Level3)
        } else if n.contains("a10thunderboltmissilestrike2") || n.ends_with("strike2") {
            Some(A10StrikeScienceTier::Level2)
        } else if n.contains("a10thunderboltmissilestrike1")
            || n.contains("a10thunderboltmissilestrike")
            || n.ends_with("strike1")
        {
            Some(A10StrikeScienceTier::Level1)
        } else {
            None
        }
    }

    /// Select highest unlocked A10 science tier from a science name list.
    pub fn highest_from_sciences<'a, I>(sciences: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best = A10StrikeScienceTier::Level1;
        for s in sciences {
            if let Some(tier) = Self::from_science_name(s) {
                best = match (best, tier) {
                    (_, A10StrikeScienceTier::Level3) | (A10StrikeScienceTier::Level3, _) => {
                        A10StrikeScienceTier::Level3
                    }
                    (_, A10StrikeScienceTier::Level2) | (A10StrikeScienceTier::Level2, _) => {
                        A10StrikeScienceTier::Level2
                    }
                    _ => A10StrikeScienceTier::Level1,
                };
            }
        }
        best
    }
}
