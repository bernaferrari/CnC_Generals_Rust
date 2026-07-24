//! Host China Frenzy ("Rage") special-power residual — temporary ally attack buff.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(Frenzy)` at a world location applies a temporary weapon-bonus
//!   enrage (retail SuperweaponFrenzy → Frenzy_InvisibleMarker_Level* →
//!   WeaponBonusUpdate → doTempWeaponBonus(FRENZY_ONE/TWO/THREE)).
//! - Nearby **same-team non-structure CAN_ATTACK residual** units receive a
//!   temporary damage multiplier (110% / 120% / 130%) for BonusDuration.
//! - Honesty counters/flags for residual gates and tests.
//!
//! Wave 52 residual pack (retail System.ini / GameData.ini / Science.ini):
//! - Frenzy_InvisibleMarker Level1/2/3 OCL template names
//! - BonusDuration 10000/20000/30000 ms → 300/600/900 frames
//! - BonusRange / RadiusCursorRadius 200 (all levels)
//! - WeaponBonus FRENZY_ONE/TWO/THREE DAMAGE 110%/120%/130%
//! - Science SCIENCE_Frenzy1/2/3 tier gate residual
//! - RequiredAffectKindOf=CAN_ATTACK / ForbiddenAffectKindOf=STRUCTURE
//! - DeletionUpdate Min/MaxLifetime = 1 msec → 1 frame (one pulse)
//! - ParticleSysBone FrenzyCloud residual name
//!
//! Fail-closed honesty:
//! - Frenzy_InvisibleMarker spawn + DeletionUpdate 1-frame residual closed
//! - Not full KindOf multi-mask engine beyond residual Required/Forbidden filters
//! - Not full ally relationship filter (uses same-team residual)
//! - Not full player science ownership matrix beyond residual name tier gate
//! - Not full FrenzyCloud particle / red TINT_STATUS_FRENZY drawable path
//! - Not network Frenzy replication (network deferred)
//!
//! Note: Retail Frenzy is a **China** Generals power (not GLA). Host residual
//! still applies to whatever team activates it (playability / tests).

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const FRENZY_LOGIC_FPS: f32 = 30.0;

/// Retail SuperweaponFrenzy RadiusCursorRadius residual (= 200).
/// Also matches Frenzy_InvisibleMarker WeaponBonusUpdate BonusRange = 200.
pub const HOST_FRENZY_RADIUS: f32 = 200.0;

/// Per-level BonusRange residual (System.ini — all levels use 200).
pub const FRENZY_LEVEL1_RADIUS: f32 = 200.0;
pub const FRENZY_LEVEL2_RADIUS: f32 = 200.0;
pub const FRENZY_LEVEL3_RADIUS: f32 = 200.0;

/// Retail Frenzy_InvisibleMarker_Level1 BonusDuration = 10000 ms → 300 frames.
pub const FRENZY_LEVEL1_DURATION_MS: u32 = 10_000;
/// Retail Frenzy_InvisibleMarker_Level2 BonusDuration = 20000 ms → 600 frames.
pub const FRENZY_LEVEL2_DURATION_MS: u32 = 20_000;
/// Retail Frenzy_InvisibleMarker_Level3 BonusDuration = 30000 ms → 900 frames.
pub const FRENZY_LEVEL3_DURATION_MS: u32 = 30_000;

/// Retail BonusDuration residual frames (parseDurationUnsignedInt ceil).
pub const FRENZY_LEVEL1_DURATION_FRAMES: u32 = 300;
pub const FRENZY_LEVEL2_DURATION_FRAMES: u32 = 600;
pub const FRENZY_LEVEL3_DURATION_FRAMES: u32 = 900;

/// Retail WeaponBonusUpdate BonusDelay residual msec (long; marker dies first).
pub const FRENZY_BONUS_DELAY_MS: u32 = 100_000;

/// Retail GameData.ini WeaponBonus FRENZY_ONE DAMAGE 110%.
pub const FRENZY_LEVEL1_DAMAGE_MULT: f32 = 1.10;
/// Retail GameData.ini WeaponBonus FRENZY_TWO DAMAGE 120%.
pub const FRENZY_LEVEL2_DAMAGE_MULT: f32 = 1.20;
/// Retail GameData.ini WeaponBonus FRENZY_THREE DAMAGE 130%.
pub const FRENZY_LEVEL3_DAMAGE_MULT: f32 = 1.30;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound = FrenzyActivate).
pub const FRENZY_ACTIVATE_AUDIO: &str = "FrenzyActivate";

/// Retail SuperweaponFrenzy ReloadTime residual (msec).
pub const FRENZY_RELOAD_TIME_MS: u32 = 240_000;
/// ReloadTime 240000 ms → 7200 frames @ 30 FPS.
pub const FRENZY_RELOAD_TIME_FRAMES: u32 = 7_200;

/// Retail OCL / System.ini Frenzy invisible-marker templates.
pub const FRENZY_MARKER_LEVEL1: &str = "Frenzy_InvisibleMarker_Level1";
pub const FRENZY_MARKER_LEVEL2: &str = "Frenzy_InvisibleMarker_Level2";
pub const FRENZY_MARKER_LEVEL3: &str = "Frenzy_InvisibleMarker_Level3";

/// Retail SpecialPower template residual.
pub const SUPERWEAPON_FRENZY: &str = "SuperweaponFrenzy";

/// Retail science tier residual names (Science.ini).
pub const SCIENCE_FRENZY1: &str = "SCIENCE_Frenzy1";
pub const SCIENCE_FRENZY2: &str = "SCIENCE_Frenzy2";
pub const SCIENCE_FRENZY3: &str = "SCIENCE_Frenzy3";

/// Retail ParticleSysBone residual on Frenzy_InvisibleMarker draw.
pub const FRENZY_CLOUD_PARTICLE: &str = "FrenzyCloud";

/// Retail WeaponBonusUpdate RequiredAffectKindOf residual name.
pub const FRENZY_REQUIRED_AFFECT_KINDOF: &str = "CAN_ATTACK";
/// Retail WeaponBonusUpdate ForbiddenAffectKindOf residual name.
pub const FRENZY_FORBIDDEN_AFFECT_KINDOF: &str = "STRUCTURE";

/// Retail BonusConditionType residual names (GameData / WeaponBonusUpdate).
pub const FRENZY_CONDITION_ONE: &str = "FRENZY_ONE";
pub const FRENZY_CONDITION_TWO: &str = "FRENZY_TWO";
pub const FRENZY_CONDITION_THREE: &str = "FRENZY_THREE";

/// C++ `WeaponBonusConditionType` residual discriminants (ALLOW_DEMORALIZE off:
/// DEMORALIZED_OBSOLETE still occupies slot 7).
/// WEAPONBONUSCONDITION_FRENZY_ONE / TWO / THREE.
pub const WEAPON_BONUS_FRENZY_ONE: u8 = 24;
pub const WEAPON_BONUS_FRENZY_TWO: u8 = 25;
pub const WEAPON_BONUS_FRENZY_THREE: u8 = 26;

/// Retail DeletionUpdate MinLifetime residual (msec INI = 1 → ceil frames = 1).
/// Marker is a one-pulse object ("one pulse" comment in System.ini).
pub const FRENZY_MARKER_DELETION_LIFETIME_MS: u32 = 1;
/// DeletionUpdate lifetime residual frames (ceil(1 * 30/1000) = 1).
pub const FRENZY_MARKER_DELETION_LIFETIME_FRAMES: u32 = 1;

/// Convert msec residual → logic frames @ 30 FPS (C++ parseDurationUnsignedInt ceil).
pub fn frenzy_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * (FRENZY_LOGIC_FPS / 1000.0)).ceil() as u32
}

/// Residual Frenzy science tier → FRENZY_ONE / TWO / THREE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostFrenzyLevel {
    /// SCIENCE_Frenzy1 → Frenzy_InvisibleMarker_Level1 (FRENZY_ONE).
    One = 1,
    /// SCIENCE_Frenzy2 → Frenzy_InvisibleMarker_Level2 (FRENZY_TWO).
    Two = 2,
    /// SCIENCE_Frenzy3 → Frenzy_InvisibleMarker_Level3 (FRENZY_THREE).
    Three = 3,
}

impl HostFrenzyLevel {
    /// Parse residual level from 1..=3 (fail-closed: unknown → One).
    pub fn from_u8(level: u8) -> Self {
        match level {
            2 => HostFrenzyLevel::Two,
            3 => HostFrenzyLevel::Three,
            _ => HostFrenzyLevel::One,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Retail science residual name for this tier.
    pub fn science_name(self) -> &'static str {
        match self {
            HostFrenzyLevel::One => SCIENCE_FRENZY1,
            HostFrenzyLevel::Two => SCIENCE_FRENZY2,
            HostFrenzyLevel::Three => SCIENCE_FRENZY3,
        }
    }

    /// Retail OCL / System.ini invisible-marker template for this tier.
    pub fn marker_template(self) -> &'static str {
        match self {
            HostFrenzyLevel::One => FRENZY_MARKER_LEVEL1,
            HostFrenzyLevel::Two => FRENZY_MARKER_LEVEL2,
            HostFrenzyLevel::Three => FRENZY_MARKER_LEVEL3,
        }
    }

    /// Retail BonusConditionType residual name.
    pub fn condition_name(self) -> &'static str {
        match self {
            HostFrenzyLevel::One => FRENZY_CONDITION_ONE,
            HostFrenzyLevel::Two => FRENZY_CONDITION_TWO,
            HostFrenzyLevel::Three => FRENZY_CONDITION_THREE,
        }
    }

    /// C++ WeaponBonusConditionType residual discriminant.
    pub fn weapon_bonus_discriminant(self) -> u8 {
        match self {
            HostFrenzyLevel::One => WEAPON_BONUS_FRENZY_ONE,
            HostFrenzyLevel::Two => WEAPON_BONUS_FRENZY_TWO,
            HostFrenzyLevel::Three => WEAPON_BONUS_FRENZY_THREE,
        }
    }

    /// Retail BonusRange residual (all levels 200).
    pub fn radius(self) -> f32 {
        match self {
            HostFrenzyLevel::One => FRENZY_LEVEL1_RADIUS,
            HostFrenzyLevel::Two => FRENZY_LEVEL2_RADIUS,
            HostFrenzyLevel::Three => FRENZY_LEVEL3_RADIUS,
        }
    }

    /// Retail BonusDuration in logic frames.
    pub fn duration_frames(self) -> u32 {
        let ms = match self {
            HostFrenzyLevel::One => FRENZY_LEVEL1_DURATION_MS,
            HostFrenzyLevel::Two => FRENZY_LEVEL2_DURATION_MS,
            HostFrenzyLevel::Three => FRENZY_LEVEL3_DURATION_MS,
        };
        frenzy_ms_to_frames(ms)
    }

    /// Retail BonusDuration residual msec.
    pub fn duration_ms(self) -> u32 {
        match self {
            HostFrenzyLevel::One => FRENZY_LEVEL1_DURATION_MS,
            HostFrenzyLevel::Two => FRENZY_LEVEL2_DURATION_MS,
            HostFrenzyLevel::Three => FRENZY_LEVEL3_DURATION_MS,
        }
    }

    /// Retail DAMAGE weapon-bonus multiplier.
    pub fn damage_multiplier(self) -> f32 {
        match self {
            HostFrenzyLevel::One => FRENZY_LEVEL1_DAMAGE_MULT,
            HostFrenzyLevel::Two => FRENZY_LEVEL2_DAMAGE_MULT,
            HostFrenzyLevel::Three => FRENZY_LEVEL3_DAMAGE_MULT,
        }
    }
}

/// Map science residual name → Frenzy level (fail-closed: unknown → One).
pub fn frenzy_level_from_science(science: &str) -> HostFrenzyLevel {
    match science {
        SCIENCE_FRENZY2 | "Early_SCIENCE_Frenzy2" => HostFrenzyLevel::Two,
        SCIENCE_FRENZY3 | "Early_SCIENCE_Frenzy3" => HostFrenzyLevel::Three,
        SCIENCE_FRENZY1 | "Early_SCIENCE_Frenzy1" | _ => HostFrenzyLevel::One,
    }
}
/// Select highest unlocked Frenzy science tier (fail-closed → Level1).
pub fn highest_frenzy_level_from_sciences<'a, I>(sciences: I) -> HostFrenzyLevel
where
    I: IntoIterator<Item = &'a str>,
{
    let mut best = HostFrenzyLevel::One;
    for s in sciences {
        let n = s.to_ascii_lowercase().replace('_', "").replace('-', "");
        if n.contains("frenzy3") {
            return HostFrenzyLevel::Three;
        }
        if n.contains("frenzy2") {
            best = HostFrenzyLevel::Two;
        }
    }
    best
}

/// Whether residual target can receive Frenzy / Rage attack buff.
///
/// Retail WeaponBonusUpdate filters:
/// - allies (host residual: same-team)
/// - alive
/// - RequiredAffectKindOf = CAN_ATTACK
/// - ForbiddenAffectKindOf = STRUCTURE
pub fn is_legal_frenzy_target(
    is_structure: bool,
    is_alive: bool,
    same_team: bool,
    can_attack: bool,
    under_construction: bool,
) -> bool {
    !is_structure && is_alive && same_team && can_attack && !under_construction
}

/// 2D distance check residual (C++ FROM_CENTER_2D / BonusRange).
pub fn in_frenzy_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// Wave 52 residual honesty: Frenzy Level1/2/3 damage / duration / radius /
/// science / marker lifetime / KindOf / FrenzyCloud pack.
pub fn honesty_frenzy_residual_ok() -> bool {
    (HOST_FRENZY_RADIUS - 200.0).abs() < 0.01
        && (FRENZY_LEVEL1_RADIUS - 200.0).abs() < 0.01
        && (FRENZY_LEVEL2_RADIUS - 200.0).abs() < 0.01
        && (FRENZY_LEVEL3_RADIUS - 200.0).abs() < 0.01
        && FRENZY_LEVEL1_DURATION_MS == 10_000
        && FRENZY_LEVEL2_DURATION_MS == 20_000
        && FRENZY_LEVEL3_DURATION_MS == 30_000
        && FRENZY_LEVEL1_DURATION_FRAMES == frenzy_ms_to_frames(FRENZY_LEVEL1_DURATION_MS)
        && FRENZY_LEVEL2_DURATION_FRAMES == frenzy_ms_to_frames(FRENZY_LEVEL2_DURATION_MS)
        && FRENZY_LEVEL3_DURATION_FRAMES == frenzy_ms_to_frames(FRENZY_LEVEL3_DURATION_MS)
        && HostFrenzyLevel::One.duration_frames() == 300
        && HostFrenzyLevel::Two.duration_frames() == 600
        && HostFrenzyLevel::Three.duration_frames() == 900
        && (HostFrenzyLevel::One.damage_multiplier() - 1.10).abs() < 0.001
        && (HostFrenzyLevel::Two.damage_multiplier() - 1.20).abs() < 0.001
        && (HostFrenzyLevel::Three.damage_multiplier() - 1.30).abs() < 0.001
        && (HostFrenzyLevel::One.radius() - 200.0).abs() < 0.01
        && (HostFrenzyLevel::Two.radius() - 200.0).abs() < 0.01
        && (HostFrenzyLevel::Three.radius() - 200.0).abs() < 0.01
        && HostFrenzyLevel::One.science_name() == SCIENCE_FRENZY1
        && HostFrenzyLevel::Two.science_name() == SCIENCE_FRENZY2
        && HostFrenzyLevel::Three.science_name() == SCIENCE_FRENZY3
        && HostFrenzyLevel::One.marker_template() == FRENZY_MARKER_LEVEL1
        && HostFrenzyLevel::Two.marker_template() == FRENZY_MARKER_LEVEL2
        && HostFrenzyLevel::Three.marker_template() == FRENZY_MARKER_LEVEL3
        && HostFrenzyLevel::One.condition_name() == FRENZY_CONDITION_ONE
        && HostFrenzyLevel::Two.condition_name() == FRENZY_CONDITION_TWO
        && HostFrenzyLevel::Three.condition_name() == FRENZY_CONDITION_THREE
        && HostFrenzyLevel::One.weapon_bonus_discriminant() == WEAPON_BONUS_FRENZY_ONE
        && HostFrenzyLevel::Two.weapon_bonus_discriminant() == WEAPON_BONUS_FRENZY_TWO
        && HostFrenzyLevel::Three.weapon_bonus_discriminant() == WEAPON_BONUS_FRENZY_THREE
        && FRENZY_REQUIRED_AFFECT_KINDOF == "CAN_ATTACK"
        && FRENZY_FORBIDDEN_AFFECT_KINDOF == "STRUCTURE"
        && FRENZY_CLOUD_PARTICLE == "FrenzyCloud"
        && FRENZY_MARKER_DELETION_LIFETIME_MS == 1
        && FRENZY_MARKER_DELETION_LIFETIME_FRAMES
            == frenzy_ms_to_frames(FRENZY_MARKER_DELETION_LIFETIME_MS)
        && FRENZY_BONUS_DELAY_MS == 100_000
        && FRENZY_RELOAD_TIME_MS == 240_000
        && FRENZY_RELOAD_TIME_FRAMES == frenzy_ms_to_frames(FRENZY_RELOAD_TIME_MS)
        && SUPERWEAPON_FRENZY == "SuperweaponFrenzy"
        && !FRENZY_ACTIVATE_AUDIO.is_empty()
}
/// Combined residual honesty pack (Wave 71).
pub fn honesty_frenzy_residual_pack_ok() -> bool {
    honesty_frenzy_residual_ok()
}

/// One active residual Frenzy activation bookkeeping entry (honesty / debug).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostFrenzy {
    pub id: u32,
    pub player_id: u32,
    pub location: Vec3,
    pub radius: f32,
    pub level: HostFrenzyLevel,
    pub activate_frame: u32,
    pub expire_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// Ally units that received FRENZY weapon-bonus residual this activation.
    pub buffs: u32,
}

/// Host residual registry for Frenzy / Rage special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostFrenzyRegistry {
    next_id: u32,
    /// Recent activations (bookkeeping; buff timers live on objects).
    activations: Vec<HostFrenzy>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Total FRENZY weapon-bonus grants applied.
    pub buff_count: u32,
    /// C++ Frenzy_InvisibleMarker OCL spawn residual.
    pub markers_spawned: u32,
    /// Marker ids spawned this frame (DeletionUpdate next frame).
    pub markers_this_frame: Vec<super::ObjectId>,
    /// Marker ids due for DeletionUpdate residual this update.
    pub pending_marker_deletes: Vec<super::ObjectId>,
}

impl HostFrenzyRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            activations: Vec::new(),
            activation_count: 0,
            buff_count: 0,
            markers_spawned: 0,
            markers_this_frame: Vec::new(),
            pending_marker_deletes: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn record_marker_spawn(&mut self, marker_id: super::ObjectId) {
        self.markers_spawned = self.markers_spawned.saturating_add(1);
        self.markers_this_frame.push(marker_id);
    }

    pub fn take_due_marker_deletes(&mut self) -> Vec<super::ObjectId> {
        // Retail DeletionUpdate Min/MaxLifetime = 1ms → 1 frame residual:
        // promote last-frame spawns to due, then drain due.
        let due = std::mem::take(&mut self.pending_marker_deletes);
        self.pending_marker_deletes = std::mem::take(&mut self.markers_this_frame);
        due
    }

    pub fn honesty_marker_ok(&self) -> bool {
        self.markers_spawned > 0
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn buff_count(&self) -> u32 {
        self.buff_count
    }

    pub fn activations(&self) -> &[HostFrenzy] {
        &self.activations
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a successful residual Frenzy activation.
    pub fn record_activation(&mut self, frenzy: HostFrenzy) {
        self.activation_count = self.activation_count.saturating_add(1);
        self.buff_count = self.buff_count.saturating_add(frenzy.buffs);
        self.activations.push(frenzy);
        // Keep bookkeeping bounded (residual, not full history Xfer).
        if self.activations.len() > 32 {
            let drain = self.activations.len() - 32;
            self.activations.drain(0..drain);
        }
    }

    /// Residual honesty: at least one Frenzy activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one unit received FRENZY attack buff.
    pub fn honesty_buff_ok(&self) -> bool {
        self.buff_count > 0
    }

    /// Combined host path: activated and applied at least one buff.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_buff_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frenzy_constants_match_retail_residual() {
        assert!((HOST_FRENZY_RADIUS - 200.0).abs() < 0.01);
        assert_eq!(HostFrenzyLevel::One.duration_frames(), 300);
        assert_eq!(HostFrenzyLevel::Two.duration_frames(), 600);
        assert_eq!(HostFrenzyLevel::Three.duration_frames(), 900);
        assert!((HostFrenzyLevel::One.damage_multiplier() - 1.10).abs() < 0.001);
        assert!((HostFrenzyLevel::Two.damage_multiplier() - 1.20).abs() < 0.001);
        assert!((HostFrenzyLevel::Three.damage_multiplier() - 1.30).abs() < 0.001);
        assert!(!FRENZY_ACTIVATE_AUDIO.is_empty());
    }

    #[test]
    fn frenzy_residual_pack_honesty() {
        assert!(honesty_frenzy_residual_ok());
        // Damage mult residual honesty 110/120/130%.
        assert_eq!(FRENZY_LEVEL1_DAMAGE_MULT, 1.10);
        assert_eq!(FRENZY_LEVEL2_DAMAGE_MULT, 1.20);
        assert_eq!(FRENZY_LEVEL3_DAMAGE_MULT, 1.30);
        // BonusDuration residual per level.
        assert_eq!(FRENZY_LEVEL1_DURATION_MS, 10_000);
        assert_eq!(FRENZY_LEVEL2_DURATION_MS, 20_000);
        assert_eq!(FRENZY_LEVEL3_DURATION_MS, 30_000);
        assert_eq!(FRENZY_LEVEL1_DURATION_FRAMES, 300);
        assert_eq!(FRENZY_LEVEL2_DURATION_FRAMES, 600);
        assert_eq!(FRENZY_LEVEL3_DURATION_FRAMES, 900);
        // Radius residual per level (all 200).
        assert_eq!(FRENZY_LEVEL1_RADIUS, 200.0);
        assert_eq!(FRENZY_LEVEL2_RADIUS, 200.0);
        assert_eq!(FRENZY_LEVEL3_RADIUS, 200.0);
        // Science tier gate residual.
        assert_eq!(SCIENCE_FRENZY1, "SCIENCE_Frenzy1");
        assert_eq!(SCIENCE_FRENZY2, "SCIENCE_Frenzy2");
        assert_eq!(SCIENCE_FRENZY3, "SCIENCE_Frenzy3");
        assert_eq!(
            frenzy_level_from_science(SCIENCE_FRENZY1),
            HostFrenzyLevel::One
        );
        assert_eq!(
            frenzy_level_from_science(SCIENCE_FRENZY2),
            HostFrenzyLevel::Two
        );
        assert_eq!(
            frenzy_level_from_science(SCIENCE_FRENZY3),
            HostFrenzyLevel::Three
        );
        assert_eq!(
            frenzy_level_from_science("Early_SCIENCE_Frenzy2"),
            HostFrenzyLevel::Two
        );
        // Marker DeletionUpdate lifetime residual.
        assert_eq!(FRENZY_MARKER_DELETION_LIFETIME_MS, 1);
        assert_eq!(FRENZY_MARKER_DELETION_LIFETIME_FRAMES, 1);
        assert_eq!(frenzy_ms_to_frames(1), 1);
        // KindOf multi-mask residual names.
        assert_eq!(FRENZY_REQUIRED_AFFECT_KINDOF, "CAN_ATTACK");
        assert_eq!(FRENZY_FORBIDDEN_AFFECT_KINDOF, "STRUCTURE");
        // OCL + FrenzyCloud residual.
        assert_eq!(FRENZY_MARKER_LEVEL1, "Frenzy_InvisibleMarker_Level1");
        assert_eq!(FRENZY_CLOUD_PARTICLE, "FrenzyCloud");
        // Discriminants residual.
        assert_eq!(WEAPON_BONUS_FRENZY_ONE, 24);
        assert_eq!(WEAPON_BONUS_FRENZY_TWO, 25);
        assert_eq!(WEAPON_BONUS_FRENZY_THREE, 26);
    }

    #[test]
    fn legal_frenzy_target_matrix() {
        // structure, alive, same_team, can_attack, under_construction
        assert!(is_legal_frenzy_target(false, true, true, true, false));
        assert!(!is_legal_frenzy_target(true, true, true, true, false)); // structure
        assert!(!is_legal_frenzy_target(false, false, true, true, false)); // dead
        assert!(!is_legal_frenzy_target(false, true, false, true, false)); // enemy
        assert!(!is_legal_frenzy_target(false, true, true, false, false)); // cannot attack
        assert!(!is_legal_frenzy_target(false, true, true, true, true)); // under construction
    }

    #[test]
    fn frenzy_radius_and_level_parse() {
        assert!(in_frenzy_radius_2d((0.0, 0.0), (100.0, 0.0), 200.0));
        assert!(!in_frenzy_radius_2d((0.0, 0.0), (250.0, 0.0), 200.0));
        assert_eq!(HostFrenzyLevel::from_u8(1), HostFrenzyLevel::One);
        assert_eq!(HostFrenzyLevel::from_u8(2), HostFrenzyLevel::Two);
        assert_eq!(HostFrenzyLevel::from_u8(3), HostFrenzyLevel::Three);
        assert_eq!(HostFrenzyLevel::from_u8(99), HostFrenzyLevel::One); // fail-closed
        assert_eq!(HostFrenzyLevel::One.marker_template(), FRENZY_MARKER_LEVEL1);
        assert_eq!(HostFrenzyLevel::Two.science_name(), SCIENCE_FRENZY2);
    }

    #[test]
    fn honesty_registry_records_buffs() {
        let mut reg = HostFrenzyRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostFrenzy {
            id,
            player_id: 1,
            location: Vec3::ZERO,
            radius: HOST_FRENZY_RADIUS,
            level: HostFrenzyLevel::One,
            activate_frame: 0,
            expire_frame: 300,
            caster_id: None,
            buffs: 2,
        });
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_buff_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.activation_count(), 1);
        assert_eq!(reg.buff_count(), 2);
    }
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn frenzy_residual_pack_honesty_wave71() {
        assert!(honesty_frenzy_residual_pack_ok());
        assert_eq!(FRENZY_LEVEL1_DURATION_FRAMES, 300);
        assert_eq!(FRENZY_LEVEL3_DURATION_FRAMES, 900);
        assert!((FRENZY_LEVEL2_DAMAGE_MULT - 1.20).abs() < 0.001);
    }
}
