//! CarpetBomb faction-tier residual constants and helpers.
use super::types::*;
// --- Carpet Bomb line multi-strike residual (retail SUPERWEAPON_CarpetBomb) ---

/// Retail `SUPERWEAPON_CarpetBomb` Payload count (`Payload = CarpetBomb 15`).
pub const CARPET_BOMB_COUNT: u32 = 15;
/// Residual spacing between bomb epicenters along the drop line
/// (host residual; full DeliveryDistance flight path deferred).
pub const CARPET_BOMB_SPACING: f32 = 25.0;
/// Retail OCL CarpetBomb DeliverPayload DropVariance X (C++ horizontal X).
pub const CARPET_BOMB_DROP_VARIANCE_X: f32 = 30.0;
/// Retail OCL CarpetBomb DeliverPayload DropVariance Y (C++ horizontal Y → host Z).
pub const CARPET_BOMB_DROP_VARIANCE_Y: f32 = 40.0;
/// Retail OCL CarpetBomb DropVariance Z (vertical; unused when 0).
pub const CARPET_BOMB_DROP_VARIANCE_Z: f32 = 0.0;
/// Retail OCL CarpetBomb `DropDelay` = 300 ms → 9 frames @ 30 FPS
/// (parseDurationUnsignedInt: ms × 30 / 1000).
pub const CARPET_BOMB_DROP_DELAY_FRAMES: u32 = 9;
/// Retail `CarpetBombWeapon` PrimaryDamage.
pub const CARPET_BOMB_DAMAGE: f32 = 300.0;
/// Retail `CarpetBombWeapon` PrimaryDamageRadius.
pub const CARPET_BOMB_RADIUS: f32 = 50.0;
/// Bomber approach residual frames before first bomb DropDelay stagger starts
/// (fail-closed vs full edge-spawn + transit locomotor).
pub const CARPET_BOMB_IMPACT_DELAY_FRAMES: u32 = 90;
/// Retail SUPERWEAPON_CarpetBomb DropDelay msec residual.
pub const CARPET_BOMB_DROP_DELAY_MS: u32 = 300;
/// Retail AirF_SUPERWEAPON_CarpetBomb DropDelay = 130 ms → ceil 4 frames @ 30 FPS.
pub const CARPET_BOMB_DROP_DELAY_AIRF_MS: u32 = 130;
/// AirF DropDelay frames residual (ceil 130*30/1000 = 4).
pub const CARPET_BOMB_DROP_DELAY_AIRF_FRAMES: u32 = 4;
/// Retail AirF_SUPERWEAPON_CarpetBomb Payload count (`Payload = CarpetBomb 12`).
pub const CARPET_BOMB_COUNT_AIRF: u32 = 12;
/// Retail SUPERWEAPON_ChinaCarpetBomb / Nuke_ Payload count (`Payload = … 10`).
pub const CARPET_BOMB_COUNT_CHINA: u32 = 10;
/// Retail SUPERWEAPON_CarpetBomb DeliveryDistance residual.
pub const CARPET_BOMB_DELIVERY_DISTANCE: f32 = 400.0;
/// Retail AirF_SUPERWEAPON_CarpetBomb DeliveryDistance residual.
pub const CARPET_BOMB_DELIVERY_DISTANCE_AIRF: f32 = 500.0;
/// Retail SUPERWEAPON_ChinaCarpetBomb DeliveryDistance residual.
pub const CARPET_BOMB_DELIVERY_DISTANCE_CHINA: f32 = 350.0;
/// Retail AmericaJetB52 / B52Locomotor PreferredHeight residual.
pub const CARPET_BOMB_PREFERRED_HEIGHT: f32 = 100.0;
/// Retail SUPERWEAPON_CarpetBomb Transport residual.
pub const CARPET_BOMB_TRANSPORT: &str = "AmericaJetB52";
/// Retail AirF transport residual.
pub const CARPET_BOMB_TRANSPORT_AIRF: &str = "AirF_AmericaJetB3";
/// Retail China transport residual.
pub const CARPET_BOMB_TRANSPORT_CHINA: &str = "ChinaJetCarpetBomber";
/// Retail CarpetBomb payload object residual.
pub const CARPET_BOMB_PAYLOAD_OBJECT: &str = "CarpetBomb";
/// Retail CarpetBombWeapon residual name.
pub const CARPET_BOMB_WEAPON_NAME: &str = "CarpetBombWeapon";
/// Retail CarpetBomb FXListDie DeathFX residual.
pub const CARPET_BOMB_FIRE_FX: &str = "FX_CarpetBomb";
/// Retail impact audio residual (ExplosionCarpetBomb SoundEffects).
pub const CARPET_BOMB_EXPLOSION_AUDIO: &str = "ExplosionCarpetBomb";
/// Retail DeliverPayload DropOffset Z residual.
pub const CARPET_BOMB_DROP_OFFSET_Z: f32 = -2.0;
/// Retail SUPERWEAPON_CarpetBomb DeliveryDecalRadius residual.
pub const CARPET_BOMB_DECAL_RADIUS: f32 = 100.0;
/// Retail SuperweaponCarpetBomb ReloadTime residual (msec).
pub const CARPET_BOMB_RELOAD_MS: u32 = 150000;
/// ReloadTime frames residual (150000 ms → 4500 @ 30 FPS).
pub const CARPET_BOMB_RELOAD_FRAMES: u32 = 4500;
/// Retail SuperweaponCarpetBomb RadiusCursorRadius residual.
pub const CARPET_BOMB_RADIUS_CURSOR: f32 = 100.0;
/// Retail CarpetBomb SoundFallingFromPlane residual.
pub const CARPET_BOMB_FALLING_SOUND: &str = "DaisyCutterWeapon";
/// Retail CarpetBomb model residual.
pub const CARPET_BOMB_MODEL: &str = "EXCarptBmb";
/// Retail B52Locomotor template residual.
pub const CARPET_BOMB_LOCOMOTOR: &str = "B52Locomotor";
/// Retail B52Locomotor Speed residual (dist/sec).
pub const CARPET_BOMB_LOCOMOTOR_SPEED: f32 = 125.0;

// --- Wave 78: CarpetBomb faction-tier reload / cursor / OCL residual deepen ---
/// Retail AirF_SuperweaponCarpetBomb ReloadTime residual (msec).
pub const CARPET_BOMB_RELOAD_AIRF_MS: u32 = 240_000;
/// AirF CarpetBomb ReloadTime 240000ms → 7200 frames @ 30 FPS.
pub const CARPET_BOMB_RELOAD_AIRF_FRAMES: u32 = 7_200;
/// Retail Nuke_SuperweaponChinaCarpetBomb ReloadTime residual (msec).
pub const CARPET_BOMB_RELOAD_NUKE_MS: u32 = 180_000;
/// Retail Early_SuperweaponChinaCarpetBomb ReloadTime residual (msec).
pub const EARLY_CHINA_CARPET_RELOAD_MS: u32 = 150_000;
/// Early China Carpet ReloadTime frames residual.
pub const EARLY_CHINA_CARPET_RELOAD_FRAMES: u32 = 4_500;
/// Retail Early_SuperweaponChinaCarpetBomb RequiredScience residual.
pub const EARLY_CHINA_CARPET_REQUIRED_SCIENCE: &str = "Early_SCIENCE_ChinaCarpetBomb";
/// Retail Early_SuperweaponChinaCarpetBomb Enum residual.
pub const EARLY_CHINA_CARPET_SPECIAL_ENUM: &str = "EARLY_SPECIAL_CHINA_CARPET_BOMB";
/// Retail Early_SuperweaponChinaCarpetBomb name residual.
pub const EARLY_CHINA_CARPET_SPECIAL_POWER: &str = "Early_SuperweaponChinaCarpetBomb";
/// Retail AirF_SuperweaponCarpetBomb RequiredScience residual.
pub const AIRF_CARPET_REQUIRED_SCIENCE: &str = "SCIENCE_AirF_CarpetBomb";
/// Retail AirF_SuperweaponCarpetBomb Enum residual.
pub const AIRF_CARPET_SPECIAL_ENUM: &str = "AIRF_SPECIAL_CARPET_BOMB";
/// Retail AirF_SuperweaponCarpetBomb name residual.
pub const AIRF_CARPET_SPECIAL_POWER: &str = "AirF_SuperweaponCarpetBomb";
/// Retail AirF Carpet ReloadTime residual (msec).
pub const AIRF_CARPET_RELOAD_MS: u32 = 240_000;
/// AirF Carpet ReloadTime frames residual.
pub const AIRF_CARPET_RELOAD_FRAMES: u32 = 7_200;
/// Nuke China CarpetBomb ReloadTime 180000ms → 5400 frames @ 30 FPS.
pub const CARPET_BOMB_RELOAD_NUKE_FRAMES: u32 = 5_400;
/// Retail SuperweaponChinaCarpetBomb / AirF RadiusCursorRadius residual.
pub const CARPET_BOMB_RADIUS_CURSOR_CHINA: f32 = 180.0;
/// Alias: AirF RadiusCursorRadius residual (same 180 as China).
pub const CARPET_BOMB_RADIUS_CURSOR_AIRF: f32 = 180.0;
/// Retail SuperweaponCarpetBomb ViewObjectDuration residual (msec).
pub const CARPET_BOMB_VIEW_OBJECT_DURATION_MS: u32 = 40_000;
/// ViewObjectDuration 40000ms → 1200 frames @ 30 FPS.
pub const CARPET_BOMB_VIEW_OBJECT_DURATION_FRAMES: u32 = 1_200;
/// Retail SuperweaponCarpetBomb ViewObjectRange residual.
pub const CARPET_BOMB_VIEW_OBJECT_RANGE: f32 = 250.0;
/// Retail SUPERWEAPON_CarpetBomb OCL residual name.
pub const CARPET_BOMB_OCL_AMERICA: &str = "SUPERWEAPON_CarpetBomb";
/// Retail AirF_SUPERWEAPON_CarpetBomb OCL residual name.
pub const CARPET_BOMB_OCL_AIRF: &str = "AirF_SUPERWEAPON_CarpetBomb";
/// Retail SUPERWEAPON_ChinaCarpetBomb OCL residual name.
pub const CARPET_BOMB_OCL_CHINA: &str = "SUPERWEAPON_ChinaCarpetBomb";
/// Retail SCIENCE_CarpetBomb residual (commented RequiredScience on public timer).
pub const CARPET_BOMB_SCIENCE_AMERICA: &str = "SCIENCE_CarpetBomb";
/// Retail SCIENCE_AirF_CarpetBomb residual.
pub const CARPET_BOMB_SCIENCE_AIRF: &str = "SCIENCE_AirF_CarpetBomb";
/// Retail SCIENCE_ChinaCarpetBomb residual.
pub const CARPET_BOMB_SCIENCE_CHINA: &str = "SCIENCE_ChinaCarpetBomb";
/// Retail America DeliveryDecal Texture residual (OCL reuses SCCA10Strike_USA).
pub const CARPET_BOMB_DECAL_TEXTURE_AMERICA: &str = "SCCA10Strike_USA";
/// Retail AirF/China DeliveryDecal Texture residual.
pub const CARPET_BOMB_DECAL_TEXTURE_CHINA_AIRF: &str = "SCCCarpBomb";
/// Retail DeliveryDecal Style residual (all CarpetBomb OCLs).
pub const CARPET_BOMB_DECAL_STYLE: &str = "SHADOW_ALPHA_DECAL";
/// Retail DeliveryDecal OpacityMin residual (percent).
pub const CARPET_BOMB_DECAL_OPACITY_MIN_PCT: u32 = 25;
/// Retail DeliveryDecal OpacityMax residual (percent).
pub const CARPET_BOMB_DECAL_OPACITY_MAX_PCT: u32 = 50;
/// Retail DeliveryDecal OpacityThrobTime residual (msec).
pub const CARPET_BOMB_DECAL_THROB_MS: u32 = 500;
/// Retail America DeliveryDecal Color residual (R:255 G:156 B:0 A:255).
pub const CARPET_BOMB_DECAL_COLOR_AMERICA: (u8, u8, u8, u8) = (255, 156, 0, 255);
/// Retail AirF/China DeliveryDecal Color residual (R:255 G:0 B:0 A:255).
pub const CARPET_BOMB_DECAL_COLOR_CHINA_AIRF: (u8, u8, u8, u8) = (255, 0, 0, 255);
/// Retail AirF DeliveryDecalRadius residual.
pub const CARPET_BOMB_DECAL_RADIUS_AIRF: f32 = 180.0;
/// Retail China DeliveryDecalRadius residual.
pub const CARPET_BOMB_DECAL_RADIUS_CHINA: f32 = 180.0;

/// Residual CarpetBomb faction/science tier (bomb count / DropDelay / DeliveryDistance).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CarpetBombFactionTier {
    /// USA SUPERWEAPON_CarpetBomb residual (Payload 15 / DropDelay 300 / Dist 400).
    #[default]
    America,
    /// Air Force AirF_SUPERWEAPON_CarpetBomb residual (12 / 130ms / 500).
    AirForce,
    /// China SUPERWEAPON_ChinaCarpetBomb residual (10 / 300ms / 350).
    China,
}

impl CarpetBombFactionTier {
    /// Retail Payload bomb count for this faction residual.
    pub fn bomb_count(self) -> u32 {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_COUNT,
            CarpetBombFactionTier::AirForce => CARPET_BOMB_COUNT_AIRF,
            CarpetBombFactionTier::China => CARPET_BOMB_COUNT_CHINA,
        }
    }

    /// Retail DropDelay frames residual for this faction.
    pub fn drop_delay_frames(self) -> u32 {
        match self {
            CarpetBombFactionTier::America | CarpetBombFactionTier::China => {
                CARPET_BOMB_DROP_DELAY_FRAMES
            }
            CarpetBombFactionTier::AirForce => CARPET_BOMB_DROP_DELAY_AIRF_FRAMES,
        }
    }

    /// Retail DeliveryDistance residual for this faction.
    pub fn delivery_distance(self) -> f32 {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_DELIVERY_DISTANCE,
            CarpetBombFactionTier::AirForce => CARPET_BOMB_DELIVERY_DISTANCE_AIRF,
            CarpetBombFactionTier::China => CARPET_BOMB_DELIVERY_DISTANCE_CHINA,
        }
    }

    /// Retail OCL Transport residual name.
    pub fn transport(self) -> &'static str {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_TRANSPORT,
            CarpetBombFactionTier::AirForce => CARPET_BOMB_TRANSPORT_AIRF,
            CarpetBombFactionTier::China => CARPET_BOMB_TRANSPORT_CHINA,
        }
    }

    /// Host residual line length ≈ (count-1)*spacing (DeliveryDistance flight deferred).
    pub fn line_length(self) -> f32 {
        let n = self.bomb_count().max(1);
        (n as f32 - 1.0) * CARPET_BOMB_SPACING
    }

    /// Retail Superweapon*CarpetBomb ReloadTime residual (msec) for this faction.
    ///
    /// America/China baseline **150000**; AirF **240000**. Nuke_ China variant
    /// (**180000**) is a separate SpecialPower residual (see CARPET_BOMB_RELOAD_NUKE_MS).
    pub fn reload_ms(self) -> u32 {
        match self {
            CarpetBombFactionTier::America | CarpetBombFactionTier::China => CARPET_BOMB_RELOAD_MS,
            CarpetBombFactionTier::AirForce => CARPET_BOMB_RELOAD_AIRF_MS,
        }
    }

    /// ReloadTime frames residual for this faction (@ 30 FPS ceil).
    pub fn reload_frames(self) -> u32 {
        duration_ms_to_logic_frames(self.reload_ms())
    }

    /// Retail RadiusCursorRadius residual for this faction.
    pub fn radius_cursor(self) -> f32 {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_RADIUS_CURSOR,
            CarpetBombFactionTier::AirForce => CARPET_BOMB_RADIUS_CURSOR_AIRF,
            CarpetBombFactionTier::China => CARPET_BOMB_RADIUS_CURSOR_CHINA,
        }
    }

    /// Retail DeliveryDecalRadius residual for this faction.
    pub fn delivery_decal_radius(self) -> f32 {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_DECAL_RADIUS,
            CarpetBombFactionTier::AirForce => CARPET_BOMB_DECAL_RADIUS_AIRF,
            CarpetBombFactionTier::China => CARPET_BOMB_DECAL_RADIUS_CHINA,
        }
    }

    /// Retail OCL residual name for this faction.
    pub fn ocl_name(self) -> &'static str {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_OCL_AMERICA,
            CarpetBombFactionTier::AirForce => CARPET_BOMB_OCL_AIRF,
            CarpetBombFactionTier::China => CARPET_BOMB_OCL_CHINA,
        }
    }

    /// Retail science residual name for this faction.
    pub fn science_name(self) -> &'static str {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_SCIENCE_AMERICA,
            CarpetBombFactionTier::AirForce => CARPET_BOMB_SCIENCE_AIRF,
            CarpetBombFactionTier::China => CARPET_BOMB_SCIENCE_CHINA,
        }
    }

    /// Retail DeliveryDecal Texture residual for this faction.
    pub fn delivery_decal_texture(self) -> &'static str {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_DECAL_TEXTURE_AMERICA,
            CarpetBombFactionTier::AirForce | CarpetBombFactionTier::China => {
                CARPET_BOMB_DECAL_TEXTURE_CHINA_AIRF
            }
        }
    }

    /// Retail DeliveryDecal Color residual for this faction.
    pub fn delivery_decal_color(self) -> (u8, u8, u8, u8) {
        match self {
            CarpetBombFactionTier::America => CARPET_BOMB_DECAL_COLOR_AMERICA,
            CarpetBombFactionTier::AirForce | CarpetBombFactionTier::China => {
                CARPET_BOMB_DECAL_COLOR_CHINA_AIRF
            }
        }
    }

    /// Map science/OCL residual name to faction tier.
    pub fn from_science_or_ocl_name(name: &str) -> Option<Self> {
        let n: String = name
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        if n.contains("airf") {
            Some(CarpetBombFactionTier::AirForce)
        } else if n.contains("china") || n.contains("nuke_chinacarpet") {
            Some(CarpetBombFactionTier::China)
        } else if n.contains("carpetbomb") {
            Some(CarpetBombFactionTier::America)
        } else {
            None
        }
    }

    /// Residual faction carpet tier from host team (fail-closed America).
    pub fn from_team(team: crate::game_logic::Team) -> Self {
        match team {
            crate::game_logic::Team::China => CarpetBombFactionTier::China,
            _ => CarpetBombFactionTier::America,
        }
    }

    /// Team baseline + AirForce override from unlocked AirF carpet science/OCL names.
    pub fn highest_from_team_and_sciences<'a, I>(team: crate::game_logic::Team, sciences: I) -> Self
    where
        I: IntoIterator<Item = &'a str>,
    {
        let mut best = Self::from_team(team);
        for s in sciences {
            if let Some(t) = Self::from_science_or_ocl_name(s) {
                if matches!(t, CarpetBombFactionTier::AirForce) {
                    return CarpetBombFactionTier::AirForce;
                }
                if matches!(t, CarpetBombFactionTier::China) {
                    best = CarpetBombFactionTier::China;
                }
            }
        }
        best
    }
}
