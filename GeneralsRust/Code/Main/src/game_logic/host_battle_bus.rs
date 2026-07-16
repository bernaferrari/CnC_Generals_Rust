//! Host GLA Battle Bus residual.
//!
//! Residual slice (playability):
//! - `TransportContain` capacity for GLA Battle Bus (`Slots = 8`, infantry only)
//! - `PassengersAllowedToFire = Yes` — docked riders residual-fire from bus origin
//! - `ArmedRidersUpgradeMyWeaponSet = Yes` — set WEAPONSET_PLAYER_UPGRADE residual
//!   when any armed infantry rider is loaded (BattleBusPassengerDummyWeapon bind)
//!
//! Wave 58 residual pack (retail GLAVehicle.ini / Weapon.ini honesty):
//! - TransportContain: Slots **8**, ExitDelay **250**ms → **8**f, NumberOfExitPaths **5**,
//!   DamagePercentToUnits **100%**, AllowInsideKindOf INFANTRY,
//!   PassengersAllowedToFire **Yes**, ArmedRidersUpgradeMyWeaponSet **Yes**,
//!   WeaponBonusPassedToPassengers **Yes**, DelayExitInAir **Yes**,
//!   GoAggressiveOnExit **Yes**
//! - BattleBusPassengerDummyWeapon: dmg **0.001**, range **90**, Delay **10000**ms → **300**f
//! - BattleBusDummyWeapon (SECONDARY AA residual): dmg **0.0001**, range **320**,
//!   Delay **500**ms → **15**f, AntiAirborneVehicle/Infantry **Yes**, AntiGround **No**
//! - BattleBusSlowDeathBehavior (suicide/detonate residual): ThrowForce **100**,
//!   PercentDamageToPassengers **50%**, EmptyHulkDestructionDelay **1000**ms → **30**f,
//!   ProbabilityModifier **5**, DestructionDelay **0**, DestructionDelayVariance **200**ms → **6**f
//! - UndeadBody: MaxHealth **400**, SecondLifeMaxHealth **650**
//! - FX/OCL residual: FX_BattleBusStartUndeath, OCL_BattleBusStartUndeath,
//!   FX_BattleBusHitGround, OCL_BattleBusHitGround, FX_BuggyNewDeathExplosion,
//!   OCL_BattleBusDeath
//! - VisionRange **150**, ShroudClearingRange **200**, BuildCost **1000**,
//!   BattleBusLocomotor Speed **70**
//!
//! Fail-closed honesty:
//! - Not full C++ BattleBusSlowDeathBehavior undeath / SECOND_LIFE structure hulk
//! - Not multi-door exit paths / ExitStart bone matrix
//! - Not full WeaponSet chooser / model condition icon matrix
//! - Not full passenger contact-weapon exclusion edge cases / nested contain

use super::Weapon;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const BATTLE_BUS_LOGIC_FPS: f32 = 30.0;

/// C++ `GLAVehicleBattleBus` TransportContain `Slots = 8`.
pub const BATTLE_BUS_TRANSPORT_SLOTS: usize = 8;
/// Retail PassengersAllowedToFire residual.
pub const BATTLE_BUS_PASSENGERS_ALLOWED_TO_FIRE: bool = true;
/// Retail ArmedRidersUpgradeMyWeaponSet residual.
pub const BATTLE_BUS_ARMED_RIDERS_UPGRADE_WEAPON_SET: bool = true;
/// Retail AllowInsideKindOf = INFANTRY residual.
pub const BATTLE_BUS_ALLOW_INFANTRY_ONLY: bool = true;
/// Retail DamagePercentToUnits residual (percent).
pub const BATTLE_BUS_DAMAGE_PERCENT_TO_UNITS: f32 = 100.0;
/// Retail ExitDelay residual (msec).
pub const BATTLE_BUS_EXIT_DELAY_MS: u32 = 250;
/// ExitDelay 250ms → 8 frames @ 30 FPS.
pub const BATTLE_BUS_EXIT_DELAY_FRAMES: u32 = 8;
/// Retail NumberOfExitPaths residual.
pub const BATTLE_BUS_NUMBER_OF_EXIT_PATHS: u32 = 5;
/// Retail GoAggressiveOnExit residual.
pub const BATTLE_BUS_GO_AGGRESSIVE_ON_EXIT: bool = true;
/// Retail WeaponBonusPassedToPassengers residual.
pub const BATTLE_BUS_WEAPON_BONUS_PASSED_TO_PASSENGERS: bool = true;
/// Retail DelayExitInAir residual.
pub const BATTLE_BUS_DELAY_EXIT_IN_AIR: bool = true;
/// Retail TransportSlotCount residual (slots this vehicle takes when carried).
pub const BATTLE_BUS_TRANSPORT_SLOT_COUNT: u32 = 8;

/// Residual of Weapon.ini `BattleBusPassengerDummyWeapon` AttackRange.
pub const BATTLE_BUS_PASSENGER_DUMMY_RANGE: f32 = 90.0;
/// Residual of Weapon.ini `BattleBusPassengerDummyWeapon` PrimaryDamage (negligible).
pub const BATTLE_BUS_PASSENGER_DUMMY_DAMAGE: f32 = 0.001;
/// Residual of Weapon.ini `BattleBusPassengerDummyWeapon` DelayBetweenShots (msec → sec).
pub const BATTLE_BUS_PASSENGER_DUMMY_RELOAD_SEC: f32 = 10.0;
/// Retail DelayBetweenShots residual (msec).
pub const BATTLE_BUS_PASSENGER_DUMMY_DELAY_MS: u32 = 10_000;
/// Delay 10000ms → 300 frames @ 30 FPS.
pub const BATTLE_BUS_PASSENGER_DUMMY_DELAY_FRAMES: u32 = 300;
/// Retail passenger dummy weapon name residual.
pub const BATTLE_BUS_PASSENGER_DUMMY_WEAPON: &str = "BattleBusPassengerDummyWeapon";

/// Retail BattleBusDummyWeapon (SECONDARY AA residual) name.
pub const BATTLE_BUS_DUMMY_WEAPON: &str = "BattleBusDummyWeapon";
/// Retail BattleBusDummyWeapon PrimaryDamage residual.
pub const BATTLE_BUS_DUMMY_DAMAGE: f32 = 0.0001;
/// Retail BattleBusDummyWeapon AttackRange residual.
pub const BATTLE_BUS_DUMMY_RANGE: f32 = 320.0;
/// Retail BattleBusDummyWeapon DelayBetweenShots residual (msec).
pub const BATTLE_BUS_DUMMY_DELAY_MS: u32 = 500;
/// Delay 500ms → 15 frames @ 30 FPS.
pub const BATTLE_BUS_DUMMY_DELAY_FRAMES: u32 = 15;
/// Retail AntiAirborne residual on dummy.
pub const BATTLE_BUS_DUMMY_ANTI_AIR: bool = true;
/// Retail AntiGround residual on dummy.
pub const BATTLE_BUS_DUMMY_ANTI_GROUND: bool = false;

// --- BattleBusSlowDeathBehavior suicide/detonate residual ---

/// Retail ThrowForce residual.
pub const BATTLE_BUS_THROW_FORCE: f32 = 100.0;
/// Retail PercentDamageToPassengers residual (percent).
pub const BATTLE_BUS_PERCENT_DAMAGE_TO_PASSENGERS: f32 = 50.0;
/// Retail EmptyHulkDestructionDelay residual (msec).
pub const BATTLE_BUS_EMPTY_HULK_DESTRUCTION_DELAY_MS: u32 = 1_000;
/// EmptyHulkDestructionDelay 1000ms → 30 frames @ 30 FPS.
pub const BATTLE_BUS_EMPTY_HULK_DESTRUCTION_DELAY_FRAMES: u32 = 30;
/// Retail ProbabilityModifier residual.
pub const BATTLE_BUS_PROBABILITY_MODIFIER: u32 = 5;
/// Retail DestructionDelay residual (msec) — second-life final detonate.
pub const BATTLE_BUS_DESTRUCTION_DELAY_MS: u32 = 0;
/// Retail DestructionDelayVariance residual (msec).
pub const BATTLE_BUS_DESTRUCTION_DELAY_VARIANCE_MS: u32 = 200;
/// DestructionDelayVariance 200ms → 6 frames @ 30 FPS.
pub const BATTLE_BUS_DESTRUCTION_DELAY_VARIANCE_FRAMES: u32 = 6;
/// Retail FXStartUndeath residual.
pub const BATTLE_BUS_FX_START_UNDEATH: &str = "FX_BattleBusStartUndeath";
/// Retail OCLStartUndeath residual.
pub const BATTLE_BUS_OCL_START_UNDEATH: &str = "OCL_BattleBusStartUndeath";
/// Retail FXHitGround residual.
pub const BATTLE_BUS_FX_HIT_GROUND: &str = "FX_BattleBusHitGround";
/// Retail OCLHitGround residual.
pub const BATTLE_BUS_OCL_HIT_GROUND: &str = "OCL_BattleBusHitGround";
/// Retail final detonate FX residual.
pub const BATTLE_BUS_FX_FINAL_DETONATE: &str = "FX_BuggyNewDeathExplosion";
/// Retail final detonate OCL residual.
pub const BATTLE_BUS_OCL_FINAL_DEATH: &str = "OCL_BattleBusDeath";

// --- UndeadBody residual ---

/// Retail MaxHealth residual (first life).
pub const BATTLE_BUS_MAX_HEALTH: f32 = 400.0;
/// Retail SecondLifeMaxHealth residual (hulk / SECOND_LIFE).
pub const BATTLE_BUS_SECOND_LIFE_MAX_HEALTH: f32 = 650.0;
/// Retail VisionRange residual.
pub const BATTLE_BUS_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const BATTLE_BUS_SHROUD_CLEARING_RANGE: f32 = 200.0;
/// Retail BuildCost residual.
pub const BATTLE_BUS_BUILD_COST: u32 = 1_000;
/// Retail BuildTime residual (seconds).
pub const BATTLE_BUS_BUILD_TIME_SEC: f32 = 15.0;
/// Retail BattleBusLocomotor Speed residual.
pub const BATTLE_BUS_LOCOMOTOR_SPEED: f32 = 70.0;

/// Host residual honesty counters for Battle Bus load / unload / passenger fire /
/// armed-riders weapon-set upgrade / undeath detonate residual.
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
    /// Wave 58: residual undeath / first-life detonate transitions booked.
    pub undeath_detonates: u32,
    /// Wave 58: residual empty-hulk destruction residual bookings.
    pub empty_hulk_destructions: u32,
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

    pub fn record_undeath_detonate(&mut self) {
        self.undeath_detonates = self.undeath_detonates.saturating_add(1);
    }

    pub fn record_empty_hulk_destruction(&mut self) {
        self.empty_hulk_destructions = self.empty_hulk_destructions.saturating_add(1);
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

    /// Wave 58 residual honesty: undeath/detonate residual booked.
    pub fn honesty_undeath_detonate_ok(&self) -> bool {
        self.undeath_detonates > 0
    }

    /// Wave 58 residual honesty: empty hulk destruction residual booked.
    pub fn honesty_empty_hulk_destruction_ok(&self) -> bool {
        self.empty_hulk_destructions > 0
    }

    /// Combined residual path honesty (load/unload and/or combat).
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_load_unload_ok()
            || self.honesty_passenger_fire_ok()
            || self.honesty_weapon_set_upgrade_ok()
            || self.honesty_undeath_detonate_ok()
    }
}

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn battle_bus_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / BATTLE_BUS_LOGIC_FPS)).round() as u32
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
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: false,
        can_target_ground: true,
        projectile_speed: 0.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Residual BattleBusDummyWeapon (SECONDARY AA residual enable).
pub fn battle_bus_dummy_weapon() -> Weapon {
    Weapon {
        damage: BATTLE_BUS_DUMMY_DAMAGE,
        range: BATTLE_BUS_DUMMY_RANGE,
        min_range: 0.0,
        reload_time: (BATTLE_BUS_DUMMY_DELAY_FRAMES.max(1) as f32) / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        clip_size: 0,
        clip_reload_time: 0.0,
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
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

/// Residual passenger damage applied on first-life undeath detonate.
///
/// `PercentDamageToPassengers = 50%` of passenger max health residual.
pub fn battle_bus_undeath_passenger_damage(passenger_max_health: f32) -> f32 {
    passenger_max_health * (BATTLE_BUS_PERCENT_DAMAGE_TO_PASSENGERS / 100.0)
}

/// Residual: whether empty hulk should self-destruct after EmptyHulkDestructionDelay.
pub fn battle_bus_empty_hulk_should_destroy(
    is_second_life: bool,
    passenger_count: usize,
    frames_empty: u32,
) -> bool {
    is_second_life
        && passenger_count == 0
        && frames_empty >= BATTLE_BUS_EMPTY_HULK_DESTRUCTION_DELAY_FRAMES
}

// --- Wave 58 residual honesty packs ---

/// Wave 58 residual honesty: transport residual.
pub fn honesty_battle_bus_transport_residual_ok() -> bool {
    BATTLE_BUS_TRANSPORT_SLOTS == 8
        && BATTLE_BUS_PASSENGERS_ALLOWED_TO_FIRE
        && BATTLE_BUS_ARMED_RIDERS_UPGRADE_WEAPON_SET
        && BATTLE_BUS_ALLOW_INFANTRY_ONLY
        && (BATTLE_BUS_DAMAGE_PERCENT_TO_UNITS - 100.0).abs() < 0.01
        && BATTLE_BUS_EXIT_DELAY_MS == 250
        && BATTLE_BUS_EXIT_DELAY_FRAMES == battle_bus_ms_to_frames(BATTLE_BUS_EXIT_DELAY_MS)
        && BATTLE_BUS_NUMBER_OF_EXIT_PATHS == 5
        && BATTLE_BUS_GO_AGGRESSIVE_ON_EXIT
        && BATTLE_BUS_WEAPON_BONUS_PASSED_TO_PASSENGERS
        && BATTLE_BUS_DELAY_EXIT_IN_AIR
        && BATTLE_BUS_TRANSPORT_SLOT_COUNT == 8
}

/// Wave 58 residual honesty: passenger dummy + AA dummy residual.
pub fn honesty_battle_bus_weapon_dummy_residual_ok() -> bool {
    BATTLE_BUS_PASSENGER_DUMMY_WEAPON == "BattleBusPassengerDummyWeapon"
        && (BATTLE_BUS_PASSENGER_DUMMY_DAMAGE - 0.001).abs() < 0.0001
        && (BATTLE_BUS_PASSENGER_DUMMY_RANGE - 90.0).abs() < 0.01
        && BATTLE_BUS_PASSENGER_DUMMY_DELAY_MS == 10_000
        && BATTLE_BUS_PASSENGER_DUMMY_DELAY_FRAMES
            == battle_bus_ms_to_frames(BATTLE_BUS_PASSENGER_DUMMY_DELAY_MS)
        && (BATTLE_BUS_PASSENGER_DUMMY_RELOAD_SEC - 10.0).abs() < 0.01
        && BATTLE_BUS_DUMMY_WEAPON == "BattleBusDummyWeapon"
        && (BATTLE_BUS_DUMMY_DAMAGE - 0.0001).abs() < 0.00001
        && (BATTLE_BUS_DUMMY_RANGE - 320.0).abs() < 0.01
        && BATTLE_BUS_DUMMY_DELAY_MS == 500
        && BATTLE_BUS_DUMMY_DELAY_FRAMES == battle_bus_ms_to_frames(BATTLE_BUS_DUMMY_DELAY_MS)
        && BATTLE_BUS_DUMMY_ANTI_AIR
        && !BATTLE_BUS_DUMMY_ANTI_GROUND
}

/// Wave 58 residual honesty: suicide/detonate / SlowDeath residual.
pub fn honesty_battle_bus_suicide_detonate_residual_ok() -> bool {
    (BATTLE_BUS_THROW_FORCE - 100.0).abs() < 0.01
        && (BATTLE_BUS_PERCENT_DAMAGE_TO_PASSENGERS - 50.0).abs() < 0.01
        && BATTLE_BUS_EMPTY_HULK_DESTRUCTION_DELAY_MS == 1_000
        && BATTLE_BUS_EMPTY_HULK_DESTRUCTION_DELAY_FRAMES
            == battle_bus_ms_to_frames(BATTLE_BUS_EMPTY_HULK_DESTRUCTION_DELAY_MS)
        && BATTLE_BUS_PROBABILITY_MODIFIER == 5
        && BATTLE_BUS_DESTRUCTION_DELAY_MS == 0
        && BATTLE_BUS_DESTRUCTION_DELAY_VARIANCE_MS == 200
        && BATTLE_BUS_DESTRUCTION_DELAY_VARIANCE_FRAMES
            == battle_bus_ms_to_frames(BATTLE_BUS_DESTRUCTION_DELAY_VARIANCE_MS)
        && BATTLE_BUS_FX_START_UNDEATH == "FX_BattleBusStartUndeath"
        && BATTLE_BUS_OCL_START_UNDEATH == "OCL_BattleBusStartUndeath"
        && BATTLE_BUS_FX_HIT_GROUND == "FX_BattleBusHitGround"
        && BATTLE_BUS_OCL_HIT_GROUND == "OCL_BattleBusHitGround"
        && BATTLE_BUS_FX_FINAL_DETONATE == "FX_BuggyNewDeathExplosion"
        && BATTLE_BUS_OCL_FINAL_DEATH == "OCL_BattleBusDeath"
        && (battle_bus_undeath_passenger_damage(100.0) - 50.0).abs() < 0.01
        && battle_bus_empty_hulk_should_destroy(true, 0, 30)
        && !battle_bus_empty_hulk_should_destroy(true, 0, 29)
        && !battle_bus_empty_hulk_should_destroy(true, 1, 30)
        && !battle_bus_empty_hulk_should_destroy(false, 0, 30)
}

/// Wave 58 residual honesty: body / vision residual.
pub fn honesty_battle_bus_body_residual_ok() -> bool {
    (BATTLE_BUS_MAX_HEALTH - 400.0).abs() < 0.01
        && (BATTLE_BUS_SECOND_LIFE_MAX_HEALTH - 650.0).abs() < 0.01
        && (BATTLE_BUS_VISION_RANGE - 150.0).abs() < 0.01
        && (BATTLE_BUS_SHROUD_CLEARING_RANGE - 200.0).abs() < 0.01
        && BATTLE_BUS_BUILD_COST == 1_000
        && (BATTLE_BUS_BUILD_TIME_SEC - 15.0).abs() < 0.01
        && (BATTLE_BUS_LOCOMOTOR_SPEED - 70.0).abs() < 0.01
}

/// Combined Wave 58 Battle Bus residual honesty pack.
pub fn honesty_battle_bus_residual_pack_ok() -> bool {
    honesty_battle_bus_transport_residual_ok()
        && honesty_battle_bus_weapon_dummy_residual_ok()
        && honesty_battle_bus_suicide_detonate_residual_ok()
        && honesty_battle_bus_body_residual_ok()
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
        reg.record_undeath_detonate();
        assert!(reg.honesty_undeath_detonate_ok());
        reg.record_empty_hulk_destruction();
        assert!(reg.honesty_empty_hulk_destruction_ok());
    }

    #[test]
    fn passenger_dummy_weapon_is_long_range_negligible_damage() {
        let w = battle_bus_passenger_dummy_weapon();
        assert!((w.range - BATTLE_BUS_PASSENGER_DUMMY_RANGE).abs() < f32::EPSILON);
        assert!(w.damage < 0.01);
        assert!(w.can_target_ground);
    }

    #[test]
    fn aa_dummy_weapon_is_air_only_negligible() {
        let w = battle_bus_dummy_weapon();
        assert!((w.range - 320.0).abs() < 0.01);
        assert!(w.can_target_air && !w.can_target_ground);
        assert!(w.damage < 0.001);
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

    #[test]
    fn battle_bus_residual_pack_honesty_wave58() {
        assert!(honesty_battle_bus_residual_pack_ok());
        assert_eq!(battle_bus_ms_to_frames(250), 8);
        assert_eq!(battle_bus_ms_to_frames(500), 15);
        assert_eq!(battle_bus_ms_to_frames(1_000), 30);
        assert_eq!(battle_bus_ms_to_frames(10_000), 300);
        assert_eq!(battle_bus_ms_to_frames(200), 6);
        assert_eq!(BATTLE_BUS_TRANSPORT_SLOTS, 8);
        assert!((battle_bus_undeath_passenger_damage(200.0) - 100.0).abs() < 0.01);
    }
}
