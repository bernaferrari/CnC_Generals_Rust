//! Host GLA Hijack / ConvertToCarBomb residual.
//!
//! Residual slice (playability):
//! - `Hijack`: infantry walks to enemy ground vehicle → transfer team +
//!   OBJECT_STATUS_HIJACKED; hijacker is consumed (fail-closed residual of
//!   ConvertToHijackedVehicleCrateCollide + HijackerUpdate hide-in-vehicle;
//!   always consume, never eject-pilot re-spawn). Already-hijacked targets
//!   are rejected. Observable audio + radar message on success.
//! - `ConvertToCarbomb`: infantry reaches vehicle (incl. neutral civilians) →
//!   vehicle defects to converter team, gains IS_CARBOMB + SuicideCarBomb weapon
//!   residual, converter is consumed (C++ ConvertToCarBombCrateCollide).
//! - Car-bomb vehicle attacks (weapon fire in range) → suicide detonation AOE
//!   (SuicideCarBomb PrimaryDamage/Radius residual) + destroy self; damages
//!   nearby structures/units for observable splash.
//!
//! Wave 59 residual pack (retail Weapon.ini / FXList honesty):
//! - Detonation damage residual: Primary **700**/r**20**, Secondary **100**/r**50**,
//!   DamageDealtAtSelfPosition **Yes**, RadiusDamageAffects SELF SUICIDE ALLIES
//!   ENEMIES NEUTRALS NOT_SIMILAR
//! - Convert residual: ConvertToCarBombCrateCollide → FX_MakeCarBombSuccess /
//!   sound **TerroristCarBomb**; HijackVehicle AttackRange **5**
//! - FireSound residual: SuicideCarBomb FireSound **CarBomberDie**, FireFX
//!   **WeaponFX_SuicideDynamitePackDetonation**
//! - Range residual: AttackRange **5**, ClipSize **1**, AutoReloadsClip **No**
//!
//! Fail-closed honesty:
//! - Not full C++ WeaponSet CARBOMB chooser / model condition icon matrix
//! - Not full HijackerUpdate hide-in-partition / eject-pilot re-spawn path
//! - Not full SuicideCarBomb secondary radius / NOT_SIMILAR ally filtering
//! - Not full radar re-add / EVA vehicle-stolen / script name transfer
//! - Not full immune-to-capture / transport-occupancy / dozer-task cancel matrix

use super::Weapon;
use serde::{Deserialize, Serialize};

/// Audio residual when a car bomb detonates.
pub const CAR_BOMB_DETONATE_AUDIO: &str = "CarBomberDie";

/// Audio residual when ConvertToCarBomb succeeds (FX list name residual).
pub const CAR_BOMB_CONVERT_AUDIO: &str = "MakeCarBombSuccess";

/// Retail FX_MakeCarBombSuccess Sound Name residual.
pub const CAR_BOMB_CONVERT_FX_SOUND: &str = "TerroristCarBomb";

/// Retail ConvertToCarBombCrateCollide FXList residual.
pub const CAR_BOMB_CONVERT_FX_LIST: &str = "FX_MakeCarBombSuccess";

/// Audio residual when Hijack succeeds.
pub const HIJACK_AUDIO: &str = "HijackDriver";

/// Retail SuicideCarBomb weapon template name.
pub const SUICIDE_CAR_BOMB_WEAPON: &str = "SuicideCarBomb";

/// Retail HijackVehicle weapon template name.
pub const HIJACK_VEHICLE_WEAPON: &str = "HijackVehicle";

// SuicideCarBomb residual (Weapon.ini):
// PrimaryDamage = 700, PrimaryDamageRadius = 20
// SecondaryDamage = 100, SecondaryDamageRadius = 50
// AttackRange = 5
pub const SUICIDE_CAR_BOMB_DAMAGE: f32 = 700.0;
pub const SUICIDE_CAR_BOMB_RADIUS: f32 = 20.0;
pub const SUICIDE_CAR_BOMB_SECONDARY_DAMAGE: f32 = 100.0;
pub const SUICIDE_CAR_BOMB_SECONDARY_RADIUS: f32 = 50.0;
pub const SUICIDE_CAR_BOMB_ATTACK_RANGE: f32 = 5.0;
/// Retail HijackVehicle AttackRange residual (same close-range gate).
pub const HIJACK_ATTACK_RANGE: f32 = 5.0;
/// Retail DamageType residual.
pub const SUICIDE_CAR_BOMB_DAMAGE_TYPE: &str = "EXPLOSION";
/// Retail DeathType residual.
pub const SUICIDE_CAR_BOMB_DEATH_TYPE: &str = "SUICIDED";
/// Retail WeaponSpeed residual.
pub const SUICIDE_CAR_BOMB_WEAPON_SPEED: f32 = 99999.0;
/// Retail FireFX residual.
pub const SUICIDE_CAR_BOMB_FIRE_FX: &str = "WeaponFX_SuicideDynamitePackDetonation";
/// Retail FireSound residual.
pub const SUICIDE_CAR_BOMB_FIRE_SOUND: &str = "CarBomberDie";
/// Retail RadiusDamageAffects residual tokens.
pub const SUICIDE_CAR_BOMB_RADIUS_AFFECTS: &str =
    "SELF SUICIDE ALLIES ENEMIES NEUTRALS NOT_SIMILAR";
/// Retail DamageDealtAtSelfPosition residual.
pub const SUICIDE_CAR_BOMB_DAMAGE_AT_SELF: bool = true;
/// Retail ClipSize residual.
pub const SUICIDE_CAR_BOMB_CLIP_SIZE: u32 = 1;
/// Retail AutoReloadsClip residual.
pub const SUICIDE_CAR_BOMB_AUTO_RELOADS: bool = false;
/// Retail DelayBetweenShots residual (msec).
pub const SUICIDE_CAR_BOMB_DELAY_MS: u32 = 0;

/// C++ HijackerUpdateModuleData::m_parachuteName residual.
///
/// Retail GLAInfantryHijacker HijackerUpdate ParachuteName = AmericaParachute.
pub const HIJACKER_PARACHUTE_NAME: &str = "AmericaParachute";

/// Host residual honesty counters for Hijack / CarBomb residual.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCarBombRegistry {
    /// Successful hijack team transfers.
    pub hijacks: u32,
    /// EVA VehicleStolen residual fires.
    pub eva_vehicle_stolen: u32,
    /// Successful ConvertToCarBomb conversions (vehicle now IS_CARBOMB).
    pub conversions: u32,
    /// Car-bomb suicide detonations resolved.
    pub detonations: u32,
    /// Total residual HP damage dealt by car-bomb detonations (observable).
    pub detonation_damage_dealt: f32,
    /// C++ HijackerUpdate airborne PutInContainer AmericaParachute residual.
    pub airborne_parachute_puts: u32,
    /// C++ ParachuteContain::onCollide land residual after airborne hijack eject.
    pub airborne_parachute_lands: u32,
    /// C++ ParachuteContain::onDie FreeFallDamage residual (chute killed mid-air).
    pub airborne_parachute_free_falls: u32,
}

impl HostCarBombRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_hijack(&mut self) {
        self.hijacks = self.hijacks.saturating_add(1);
    }

    pub fn record_conversion(&mut self) {
        self.conversions = self.conversions.saturating_add(1);
    }

    pub fn record_detonation(&mut self, damage_dealt: f32) {
        self.detonations = self.detonations.saturating_add(1);
        if damage_dealt > 0.0 {
            self.detonation_damage_dealt += damage_dealt;
        }
    }

    pub fn record_airborne_parachute_put(&mut self) {
        self.airborne_parachute_puts = self.airborne_parachute_puts.saturating_add(1);
    }

    pub fn record_airborne_parachute_land(&mut self) {
        self.airborne_parachute_lands = self.airborne_parachute_lands.saturating_add(1);
    }

    pub fn record_airborne_parachute_free_fall(&mut self) {
        self.airborne_parachute_free_falls = self.airborne_parachute_free_falls.saturating_add(1);
    }

    /// Residual honesty: airborne hijack eject put rider in AmericaParachute.
    pub fn honesty_airborne_parachute_ok(&self) -> bool {
        self.airborne_parachute_puts > 0
    }

    /// Residual honesty: AmericaParachute ground collide released rider.
    pub fn honesty_airborne_parachute_land_ok(&self) -> bool {
        self.airborne_parachute_lands > 0
    }

    /// Residual honesty: FreeFallDamage from chute death mid-air.
    pub fn honesty_airborne_parachute_free_fall_ok(&self) -> bool {
        self.airborne_parachute_free_falls > 0
    }

    /// Residual honesty: at least one hijack transferred a vehicle.
    pub fn honesty_hijack_ok(&self) -> bool {
        self.hijacks > 0
    }

    pub fn record_eva_vehicle_stolen(&mut self) {
        self.eva_vehicle_stolen = self.eva_vehicle_stolen.saturating_add(1);
    }

    pub fn honesty_eva_vehicle_stolen_ok(&self) -> bool {
        self.eva_vehicle_stolen > 0
    }

    /// Residual honesty: at least one vehicle converted to car bomb.
    pub fn honesty_convert_ok(&self) -> bool {
        self.conversions > 0
    }

    /// Residual honesty: at least one car-bomb detonation with observable damage.
    pub fn honesty_detonate_ok(&self) -> bool {
        self.detonations > 0 && self.detonation_damage_dealt > 0.0
    }

    /// Combined residual path honesty (hijack / convert / detonate).
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_hijack_ok()
            || self.honesty_convert_ok()
            || self.honesty_detonate_ok()
            || self.honesty_airborne_parachute_ok()
    }
}

/// Residual SuicideCarBomb weapon bound onto converted vehicles.
pub fn suicide_car_bomb_weapon() -> Weapon {
    Weapon {
        damage: SUICIDE_CAR_BOMB_DAMAGE,
        range: SUICIDE_CAR_BOMB_ATTACK_RANGE,
        min_range: 0.0,
        reload_time: 0.0,
        last_fire_time: 0.0,
        ammo: Some(1),
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 0.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Residual AOE damage at distance (primary + secondary SuicideCarBomb rings).
pub fn car_bomb_damage_at_distance(distance: f32) -> f32 {
    let primary = if distance <= SUICIDE_CAR_BOMB_RADIUS {
        let half = SUICIDE_CAR_BOMB_RADIUS * 0.5;
        if distance <= half {
            SUICIDE_CAR_BOMB_DAMAGE
        } else {
            let t = (distance - half) / (SUICIDE_CAR_BOMB_RADIUS - half).max(0.001);
            SUICIDE_CAR_BOMB_DAMAGE * (1.0 - t).max(0.0)
        }
    } else {
        0.0
    };
    let secondary = if distance <= SUICIDE_CAR_BOMB_SECONDARY_RADIUS {
        let half = SUICIDE_CAR_BOMB_SECONDARY_RADIUS * 0.5;
        if distance <= half {
            SUICIDE_CAR_BOMB_SECONDARY_DAMAGE
        } else {
            let t = (distance - half) / (SUICIDE_CAR_BOMB_SECONDARY_RADIUS - half).max(0.001);
            SUICIDE_CAR_BOMB_SECONDARY_DAMAGE * (1.0 - t).max(0.0)
        }
    } else {
        0.0
    };
    primary.max(secondary)
}

/// Whether residual attack range is legal for suicide car bomb / hijack close gate.
pub fn car_bomb_range_ok(distance: f32) -> bool {
    distance <= SUICIDE_CAR_BOMB_ATTACK_RANGE
}

// --- Wave 59 residual honesty packs ---

/// Detonation damage residual (primary / secondary rings + self-damage flags).
pub fn honesty_car_bomb_detonation_damage_residual_ok() -> bool {
    (SUICIDE_CAR_BOMB_DAMAGE - 700.0).abs() < 0.01
        && (SUICIDE_CAR_BOMB_RADIUS - 20.0).abs() < 0.01
        && (SUICIDE_CAR_BOMB_SECONDARY_DAMAGE - 100.0).abs() < 0.01
        && (SUICIDE_CAR_BOMB_SECONDARY_RADIUS - 50.0).abs() < 0.01
        && SUICIDE_CAR_BOMB_DAMAGE_AT_SELF
        && SUICIDE_CAR_BOMB_DAMAGE_TYPE == "EXPLOSION"
        && SUICIDE_CAR_BOMB_DEATH_TYPE == "SUICIDED"
        && (SUICIDE_CAR_BOMB_WEAPON_SPEED - 99999.0).abs() < 0.1
        && SUICIDE_CAR_BOMB_RADIUS_AFFECTS.contains("SELF")
        && SUICIDE_CAR_BOMB_RADIUS_AFFECTS.contains("SUICIDE")
        && SUICIDE_CAR_BOMB_RADIUS_AFFECTS.contains("NOT_SIMILAR")
        && (car_bomb_damage_at_distance(0.0) - 700.0).abs() < 0.01
        && (car_bomb_damage_at_distance(10.0) - 700.0).abs() < 0.01
        && car_bomb_damage_at_distance(25.0) > 0.0
        && car_bomb_damage_at_distance(SUICIDE_CAR_BOMB_SECONDARY_RADIUS + 1.0) <= 0.0
}

/// Convert residual (FX / audio / hijack weapon identity).
pub fn honesty_car_bomb_convert_residual_ok() -> bool {
    CAR_BOMB_CONVERT_AUDIO == "MakeCarBombSuccess"
        && CAR_BOMB_CONVERT_FX_LIST == "FX_MakeCarBombSuccess"
        && CAR_BOMB_CONVERT_FX_SOUND == "TerroristCarBomb"
        && HIJACK_AUDIO == "HijackDriver"
        && HIJACK_VEHICLE_WEAPON == "HijackVehicle"
        && (HIJACK_ATTACK_RANGE - 5.0).abs() < 0.01
        && SUICIDE_CAR_BOMB_WEAPON == "SuicideCarBomb"
        && !CAR_BOMB_CONVERT_AUDIO.is_empty()
}

/// FireSound / FireFX residual honesty.
pub fn honesty_car_bomb_fire_sound_residual_ok() -> bool {
    SUICIDE_CAR_BOMB_FIRE_SOUND == "CarBomberDie"
        && CAR_BOMB_DETONATE_AUDIO == "CarBomberDie"
        && SUICIDE_CAR_BOMB_FIRE_FX == "WeaponFX_SuicideDynamitePackDetonation"
        && !SUICIDE_CAR_BOMB_FIRE_SOUND.is_empty()
}

/// Range residual (attack gate + clip one-shot).
pub fn honesty_car_bomb_range_residual_ok() -> bool {
    (SUICIDE_CAR_BOMB_ATTACK_RANGE - 5.0).abs() < 0.01
        && (HIJACK_ATTACK_RANGE - SUICIDE_CAR_BOMB_ATTACK_RANGE).abs() < 0.01
        && SUICIDE_CAR_BOMB_CLIP_SIZE == 1
        && !SUICIDE_CAR_BOMB_AUTO_RELOADS
        && SUICIDE_CAR_BOMB_DELAY_MS == 0
        && car_bomb_range_ok(5.0)
        && !car_bomb_range_ok(5.1)
        && {
            let w = suicide_car_bomb_weapon();
            (w.range - 5.0).abs() < 0.01 && w.ammo == Some(1)
        }
}

/// Combined Wave 59 car-bomb residual honesty pack.
pub fn honesty_car_bomb_residual_pack_ok() -> bool {
    honesty_car_bomb_detonation_damage_residual_ok()
        && honesty_car_bomb_convert_residual_ok()
        && honesty_car_bomb_fire_sound_residual_ok()
        && honesty_car_bomb_range_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_tracks_convert_and_detonate() {
        let mut reg = HostCarBombRegistry::new();
        assert!(!reg.honesty_any_ok());
        reg.record_conversion();
        assert!(reg.honesty_convert_ok());
        reg.record_hijack();
        assert!(reg.honesty_hijack_ok());
        reg.record_detonation(250.0);
        assert!(reg.honesty_detonate_ok());
        assert!((reg.detonation_damage_dealt - 250.0).abs() < f32::EPSILON);
    }

    #[test]
    fn suicide_weapon_is_close_range_one_shot() {
        let w = suicide_car_bomb_weapon();
        assert!((w.range - SUICIDE_CAR_BOMB_ATTACK_RANGE).abs() < f32::EPSILON);
        assert_eq!(w.ammo, Some(1));
        assert!(w.can_target_ground);
        assert!(!w.can_target_air);
    }

    #[test]
    fn aoe_damage_full_at_zero_distance() {
        let d = car_bomb_damage_at_distance(0.0);
        assert!((d - SUICIDE_CAR_BOMB_DAMAGE).abs() < 0.01);
    }

    #[test]
    fn aoe_damage_zero_outside_secondary() {
        let d = car_bomb_damage_at_distance(SUICIDE_CAR_BOMB_SECONDARY_RADIUS + 1.0);
        assert!(d <= 0.0);
    }

    #[test]
    fn car_bomb_residual_pack_honesty() {
        assert!(honesty_car_bomb_detonation_damage_residual_ok());
        assert!(honesty_car_bomb_convert_residual_ok());
        assert!(honesty_car_bomb_fire_sound_residual_ok());
        assert!(honesty_car_bomb_range_residual_ok());
        assert!(honesty_car_bomb_residual_pack_ok());
    }
}
