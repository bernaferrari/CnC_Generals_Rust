//! Host GLA Battle Bus residual.
//!
//! Residual slice (playability):
//! - `TransportContain` capacity for GLA Battle Bus (`Slots = 8`, infantry only)
//! - `PassengersAllowedToFire = Yes` — docked riders residual-fire from bus origin
//! - `ArmedRidersUpgradeMyWeaponSet = Yes` — set WEAPONSET_PLAYER_UPGRADE residual
//!   when any armed infantry rider is loaded (BattleBusPassengerDummyWeapon bind)
//!
//! Fail-closed honesty:
//! - Not full C++ BattleBusSlowDeathBehavior undeath / SECOND_LIFE structure hulk
//! - Not multi-door exit paths / ExitStart bone matrix
//! - Not full WeaponSet chooser / model condition icon matrix
//! - Not full passenger contact-weapon exclusion edge cases / nested contain

use super::Weapon;
use serde::{Deserialize, Serialize};

/// C++ `GLAVehicleBattleBus` TransportContain `Slots = 8`.
pub const BATTLE_BUS_TRANSPORT_SLOTS: usize = 8;

/// Residual of Weapon.ini `BattleBusPassengerDummyWeapon` AttackRange.
pub const BATTLE_BUS_PASSENGER_DUMMY_RANGE: f32 = 90.0;

/// Residual of Weapon.ini `BattleBusPassengerDummyWeapon` PrimaryDamage (negligible).
pub const BATTLE_BUS_PASSENGER_DUMMY_DAMAGE: f32 = 0.001;

/// Residual of Weapon.ini `BattleBusPassengerDummyWeapon` DelayBetweenShots (msec → sec).
pub const BATTLE_BUS_PASSENGER_DUMMY_RELOAD_SEC: f32 = 10.0;

/// Host residual honesty counters for Battle Bus load / unload / passenger fire /
/// armed-riders weapon-set upgrade.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostBattleBusRegistry {
    /// Successful infantry loads into a Battle Bus residual transport.
    pub loads: u32,
    /// Successful unload/evacuate from a Battle Bus residual transport.
    pub unloads: u32,
    /// Residual fire-from-bus passenger shots applied.
    pub passenger_fires: u32,
    /// Times armed-riders upgraded the bus weapon set residual.
    pub weapon_set_upgrades: u32,
}

impl HostBattleBusRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_load(&mut self) {
        self.loads = self.loads.saturating_add(1);
    }

    pub fn record_unload(&mut self) {
        self.unloads = self.unloads.saturating_add(1);
    }

    pub fn record_passenger_fire(&mut self) {
        self.passenger_fires = self.passenger_fires.saturating_add(1);
    }

    pub fn record_weapon_set_upgrade(&mut self) {
        self.weapon_set_upgrades = self.weapon_set_upgrades.saturating_add(1);
    }

    /// Residual honesty: load → docked → unload path exercised.
    pub fn honesty_load_unload_ok(&self) -> bool {
        self.loads > 0 && self.unloads > 0
    }

    /// Residual honesty: at least one passenger residual fire-from-bus shot.
    pub fn honesty_passenger_fire_ok(&self) -> bool {
        self.passenger_fires > 0
    }

    /// Residual honesty: armed riders upgraded the bus weapon set at least once.
    pub fn honesty_weapon_set_upgrade_ok(&self) -> bool {
        self.weapon_set_upgrades > 0
    }

    /// Combined residual path honesty (load/unload and/or combat).
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_load_unload_ok()
            || self.honesty_passenger_fire_ok()
            || self.honesty_weapon_set_upgrade_ok()
    }
}

/// True when template name is a GLA (or general) Battle Bus residual template.
/// Matches `GLAVehicleBattleBus`, `Chem_GLAVehicleBattleBus`, etc.
pub fn is_battle_bus_template(template_name: &str) -> bool {
    let lower = template_name.to_ascii_lowercase();
    lower.contains("battlebus") || lower.contains("battle_bus")
}

/// Residual BattleBusPassengerDummyWeapon bound when armed riders upgrade weapon set.
/// Negligible damage — passengers deal real residual fire; this enables attack range.
pub fn battle_bus_passenger_dummy_weapon() -> Weapon {
    Weapon {
        damage: BATTLE_BUS_PASSENGER_DUMMY_DAMAGE,
        range: BATTLE_BUS_PASSENGER_DUMMY_RANGE,
        min_range: 0.0,
        reload_time: BATTLE_BUS_PASSENGER_DUMMY_RELOAD_SEC,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 0.0,
        pre_attack_delay: 0.0,
    }
}

/// Residual of C++ TransportContain armed-rider check:
/// infantry with a non-contact damage weapon counts as "armed".
pub fn rider_has_viable_weapon(weapon: Option<&Weapon>, is_infantry: bool) -> bool {
    if !is_infantry {
        return false;
    }
    let Some(w) = weapon else {
        return false;
    };
    // Contact residual: very short range treated as contact (melee).
    // C++ isContactWeapon() — residual uses range <= 5 as contact-like.
    w.damage > 0.0 && w.range > 5.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_detection_matches_gla_and_variants() {
        assert!(is_battle_bus_template("GLAVehicleBattleBus"));
        assert!(is_battle_bus_template("Chem_GLAVehicleBattleBus"));
        assert!(is_battle_bus_template("Demo_GLAVehicleBattleBus"));
        assert!(!is_battle_bus_template("AmericaVehicleHumvee"));
        assert!(!is_battle_bus_template("ChinaTankOverlord"));
    }

    #[test]
    fn honesty_tracks_load_unload_and_fire() {
        let mut reg = HostBattleBusRegistry::new();
        assert!(!reg.honesty_any_ok());
        reg.record_load();
        reg.record_unload();
        assert!(reg.honesty_load_unload_ok());
        reg.record_passenger_fire();
        assert!(reg.honesty_passenger_fire_ok());
        reg.record_weapon_set_upgrade();
        assert!(reg.honesty_weapon_set_upgrade_ok());
    }

    #[test]
    fn passenger_dummy_weapon_is_long_range_negligible_damage() {
        let w = battle_bus_passenger_dummy_weapon();
        assert!((w.range - BATTLE_BUS_PASSENGER_DUMMY_RANGE).abs() < f32::EPSILON);
        assert!(w.damage < 0.01);
        assert!(w.can_target_ground);
    }

    #[test]
    fn armed_rider_requires_infantry_damage_weapon() {
        let rifle = Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        };
        assert!(rider_has_viable_weapon(Some(&rifle), true));
        assert!(!rider_has_viable_weapon(Some(&rifle), false));
        let melee = Weapon {
            damage: 20.0,
            range: 3.0,
            ..Weapon::default()
        };
        assert!(!rider_has_viable_weapon(Some(&melee), true));
        assert!(!rider_has_viable_weapon(None, true));
    }
}
