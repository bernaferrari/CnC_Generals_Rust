//! Host Air Force Combat Chinook residual.
//!
//! Residual slice (playability) for `AirF_AmericaVehicleChinook`:
//! - `TransportContain` capacity (`Slots = 8`, infantry + vehicle)
//! - `PassengersAllowedToFire = Yes` — docked riders residual-fire from chinook origin
//! - `ArmedRidersUpgradeMyWeaponSet = Yes` — set WEAPONSET_PLAYER_UPGRADE residual
//!   when any armed rider is loaded (`ListeningOutpostUpgradedDummyWeapon` bind)
//! - `KindOf` residual includes `CAN_ATTACK` (Combat Chinook only; vanilla Chinook does not)
//!
//! Fail-closed honesty:
//! - Not full C++ ChinookAIUpdate ropes / supply boxes / rappel / combat drop clear
//! - Not multi-door exit paths / ExitStart bone matrix
//! - Not full WeaponSet chooser / model condition icon matrix
//! - Not full passenger contact-weapon exclusion edge cases / nested contain
//! - Not full PointDefenseLaserUpdate velocity prediction (see `host_point_defense` residual)

use super::Weapon;
use serde::{Deserialize, Serialize};

/// C++ `AirF_AmericaVehicleChinook` TransportContain `Slots = 8`.
pub const COMBAT_CHINOOK_TRANSPORT_SLOTS: usize = 8;

/// Residual of Weapon.ini `ListeningOutpostUpgradedDummyWeapon` AttackRange.
pub const LISTENING_OUTPOST_DUMMY_RANGE: f32 = 90.0;

/// Residual of Weapon.ini `ListeningOutpostUpgradedDummyWeapon` PrimaryDamage.
pub const LISTENING_OUTPOST_DUMMY_DAMAGE: f32 = 0.1;

/// Residual of Weapon.ini `ListeningOutpostUpgradedDummyWeapon` DelayBetweenShots
/// (1000 msec → 1.0 sec).
pub const LISTENING_OUTPOST_DUMMY_RELOAD_SEC: f32 = 1.0;

/// Host residual honesty counters for Combat Chinook load / unload / passenger
/// fire / armed-riders weapon-set upgrade.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostCombatChinookRegistry {
    /// Successful infantry/vehicle loads into a Combat Chinook residual transport.
    pub loads: u32,
    /// Successful unload/evacuate from a Combat Chinook residual transport.
    pub unloads: u32,
    /// Residual fire-from-chinook passenger shots applied.
    pub passenger_fires: u32,
    /// Times armed-riders upgraded the chinook weapon set residual.
    pub weapon_set_upgrades: u32,
}

impl HostCombatChinookRegistry {
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

    /// Residual honesty: at least one passenger residual fire-from-chinook shot.
    pub fn honesty_passenger_fire_ok(&self) -> bool {
        self.passenger_fires > 0
    }

    /// Residual honesty: armed riders upgraded the chinook weapon set at least once.
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

/// True when template name is Air Force Combat Chinook residual template.
/// Matches `AirF_AmericaVehicleChinook`, `TestCombatChinook`, etc.
/// Fail-closed: vanilla `AmericaVehicleChinook` (no passenger fire / armed riders).
pub fn is_combat_chinook_template(template_name: &str) -> bool {
    let lower = template_name.to_ascii_lowercase();
    if lower.is_empty() {
        return false;
    }
    if lower == "testcombatchinook"
        || lower.contains("combatchinook")
        || lower.contains("combat_chinook")
    {
        return true;
    }
    // Air Force General Combat Chinook only — requires AirF_ prefix + chinook.
    if lower.starts_with("airf_") && lower.contains("chinook") {
        return true;
    }
    false
}

/// Residual `ListeningOutpostUpgradedDummyWeapon` bound when armed riders
/// upgrade weapon set (PLAYER_UPGRADE set). Negligible damage — passengers
/// deal real residual fire; this enables attack range / CAN_ATTACK residual.
pub fn listening_outpost_upgraded_dummy_weapon() -> Weapon {
    Weapon {
        damage: LISTENING_OUTPOST_DUMMY_DAMAGE,
        range: LISTENING_OUTPOST_DUMMY_RANGE,
        min_range: 0.0,
        reload_time: LISTENING_OUTPOST_DUMMY_RELOAD_SEC,
        last_fire_time: 0.0,
        ammo: None,
        // Retail AntiAirborneVehicle = Yes on ListeningOutpostUpgradedDummyWeapon.
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: 0.0,
        pre_attack_delay: 0.0,
    }
}

/// Residual of C++ TransportContain armed-rider check for Combat Chinook:
/// infantry or vehicle with a non-contact damage weapon counts as "armed".
/// (AllowInsideKindOf = INFANTRY VEHICLE residual.)
pub fn combat_chinook_rider_has_viable_weapon(
    weapon: Option<&Weapon>,
    is_infantry: bool,
    is_vehicle: bool,
) -> bool {
    if !is_infantry && !is_vehicle {
        return false;
    }
    let Some(w) = weapon else {
        return false;
    };
    // Contact residual: very short range treated as contact (melee).
    // C++ isContactWeapon() — residual uses range <= 5 as contact-like.
    w.damage > 0.0 && w.range > 5.0
}

/// True when weapon looks like a residual passenger dummy
/// (BattleBusPassengerDummyWeapon damage 0.001 or ListeningOutpost 0.1).
pub fn is_passenger_dummy_weapon(weapon: &Weapon) -> bool {
    weapon.damage > 0.0 && weapon.damage < 0.15 && weapon.range >= 80.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn template_detection_matches_airf_only() {
        assert!(is_combat_chinook_template("AirF_AmericaVehicleChinook"));
        assert!(is_combat_chinook_template("TestCombatChinook"));
        assert!(is_combat_chinook_template("CombatChinook"));
        // Vanilla USA Chinook has no PassengersAllowedToFire / ArmedRiders residual.
        assert!(!is_combat_chinook_template("AmericaVehicleChinook"));
        assert!(!is_combat_chinook_template("USA_Chinook"));
        assert!(!is_combat_chinook_template("GLAVehicleBattleBus"));
        assert!(!is_combat_chinook_template("AirF_AmericaJetRaptor"));
    }

    #[test]
    fn honesty_tracks_load_unload_and_fire() {
        let mut reg = HostCombatChinookRegistry::new();
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
    fn listening_outpost_dummy_is_long_range_low_damage_anti_air() {
        let w = listening_outpost_upgraded_dummy_weapon();
        assert!((w.range - LISTENING_OUTPOST_DUMMY_RANGE).abs() < f32::EPSILON);
        assert!((w.damage - LISTENING_OUTPOST_DUMMY_DAMAGE).abs() < f32::EPSILON);
        assert!(w.can_target_ground);
        assert!(w.can_target_air);
        assert!(is_passenger_dummy_weapon(&w));
    }

    #[test]
    fn armed_rider_allows_infantry_and_vehicle() {
        let rifle = Weapon {
            damage: 10.0,
            range: 100.0,
            ..Weapon::default()
        };
        assert!(combat_chinook_rider_has_viable_weapon(Some(&rifle), true, false));
        assert!(combat_chinook_rider_has_viable_weapon(Some(&rifle), false, true));
        assert!(!combat_chinook_rider_has_viable_weapon(Some(&rifle), false, false));
        let melee = Weapon {
            damage: 20.0,
            range: 3.0,
            ..Weapon::default()
        };
        assert!(!combat_chinook_rider_has_viable_weapon(Some(&melee), true, false));
        assert!(!combat_chinook_rider_has_viable_weapon(None, true, false));
    }
}
