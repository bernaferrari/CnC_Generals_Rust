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

/// Audio residual when ConvertToCarBomb succeeds.
pub const CAR_BOMB_CONVERT_AUDIO: &str = "MakeCarBombSuccess";

/// Audio residual when Hijack succeeds.
pub const HIJACK_AUDIO: &str = "HijackDriver";

// SuicideCarBomb residual (Weapon.ini):
// PrimaryDamage = 700, PrimaryDamageRadius = 20
// SecondaryDamage = 100, SecondaryDamageRadius = 50
// AttackRange = 5
pub const SUICIDE_CAR_BOMB_DAMAGE: f32 = 700.0;
pub const SUICIDE_CAR_BOMB_RADIUS: f32 = 20.0;
pub const SUICIDE_CAR_BOMB_SECONDARY_DAMAGE: f32 = 100.0;
pub const SUICIDE_CAR_BOMB_SECONDARY_RADIUS: f32 = 50.0;
pub const SUICIDE_CAR_BOMB_ATTACK_RANGE: f32 = 5.0;

/// Host residual honesty counters for Hijack / CarBomb residual.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCarBombRegistry {
    /// Successful hijack team transfers.
    pub hijacks: u32,
    /// Successful ConvertToCarBomb conversions (vehicle now IS_CARBOMB).
    pub conversions: u32,
    /// Car-bomb suicide detonations resolved.
    pub detonations: u32,
    /// Total residual HP damage dealt by car-bomb detonations (observable).
    pub detonation_damage_dealt: f32,
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

    /// Residual honesty: at least one hijack transferred a vehicle.
    pub fn honesty_hijack_ok(&self) -> bool {
        self.hijacks > 0
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
        self.honesty_hijack_ok() || self.honesty_convert_ok() || self.honesty_detonate_ok()
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
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 0.0,
        pre_attack_delay: 0.0,
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
            let t =
                (distance - half) / (SUICIDE_CAR_BOMB_SECONDARY_RADIUS - half).max(0.001);
            SUICIDE_CAR_BOMB_SECONDARY_DAMAGE * (1.0 - t).max(0.0)
        }
    } else {
        0.0
    };
    primary.max(secondary)
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
}
