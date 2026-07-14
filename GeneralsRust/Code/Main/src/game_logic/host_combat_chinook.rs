//! Host Air Force Combat Chinook residual.
//!
//! Residual slice (playability) for `AirF_AmericaVehicleChinook`:
//! - `TransportContain` capacity (`Slots = 8`, infantry + vehicle)
//! - `PassengersAllowedToFire = Yes` — docked riders residual-fire from chinook origin
//! - `ArmedRidersUpgradeMyWeaponSet = Yes` — set WEAPONSET_PLAYER_UPGRADE residual
//!   when any armed rider is loaded (`ListeningOutpostUpgradedDummyWeapon` bind)
//! - `KindOf` residual includes `CAN_ATTACK` (Combat Chinook only; vanilla Chinook does not)
//!
//! Wave 58 residual pack (retail AirforceGeneral.ini / Weapon.ini / Locomotor.ini honesty):
//! - TransportContain: Slots **8**, ExitDelay **100**ms → **3**f, NumberOfExitPaths **1**,
//!   DamagePercentToUnits **100%**, AllowInsideKindOf INFANTRY VEHICLE,
//!   ForbidInsideKindOf AIRCRAFT HUGE_VEHICLE, GoAggressiveOnExit **Yes**,
//!   ArmedRidersUpgradeMyWeaponSet **Yes**, PassengersAllowedToFire **Yes**
//! - ListeningOutpostUpgradedDummyWeapon: dmg **0.1**, range **90**, Delay **1000**ms → **30**f,
//!   AntiAirborneVehicle **Yes** (passenger "minigun" enable residual)
//! - PointDefenseLaser residual: AirF_PointDefenseLaser PrimaryDamage **100**,
//!   AttackRange **65**, Delay **250**ms → **8**f, ScanRange **250**, ScanRate **33**ms → **1**f,
//!   PredictTargetVelocityFactor **1.0**
//! - ChinookAIUpdate residual: MaxBoxes **8**, NumRopes **4**,
//!   PerRopeDelayMin **900**ms → **27**f, PerRopeDelayMax **1500**ms → **45**f,
//!   RappelSpeed **30**, MinDropHeight **40**, RopeFinalHeight **10**,
//!   SupplyCenterActionDelay **3000**ms → **90**f, SupplyWarehouseActionDelay **1250**ms → **38**f,
//!   UpgradedSupplyBoost **60**
//! - Body MaxHealth **350**, VisionRange **300**, ShroudClearingRange **600**,
//!   BuildCost **1200**, ChinookLocomotor Speed **150**, PreferredHeight **100**
//!
//! Fail-closed honesty:
//! - Not full C++ ChinookAIUpdate ropes / supply boxes / rappel / combat drop clear
//! - Not multi-door exit paths / ExitStart bone matrix
//! - Not full WeaponSet chooser / model condition icon matrix
//! - Not full passenger contact-weapon exclusion edge cases / nested contain
//! - Not full PointDefenseLaserUpdate velocity prediction (see `host_point_defense` residual)

use super::Weapon;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const COMBAT_CHINOOK_LOGIC_FPS: f32 = 30.0;

/// C++ `AirF_AmericaVehicleChinook` TransportContain `Slots = 8`.
pub const COMBAT_CHINOOK_TRANSPORT_SLOTS: usize = 8;
/// Retail PassengersAllowedToFire residual.
pub const COMBAT_CHINOOK_PASSENGERS_ALLOWED_TO_FIRE: bool = true;
/// Retail ArmedRidersUpgradeMyWeaponSet residual.
pub const COMBAT_CHINOOK_ARMED_RIDERS_UPGRADE_WEAPON_SET: bool = true;
/// Retail AllowInsideKindOf includes INFANTRY residual.
pub const COMBAT_CHINOOK_ALLOW_INFANTRY: bool = true;
/// Retail AllowInsideKindOf includes VEHICLE residual.
pub const COMBAT_CHINOOK_ALLOW_VEHICLE: bool = true;
/// Retail ForbidInsideKindOf AIRCRAFT residual.
pub const COMBAT_CHINOOK_FORBID_AIRCRAFT: bool = true;
/// Retail ForbidInsideKindOf HUGE_VEHICLE residual.
pub const COMBAT_CHINOOK_FORBID_HUGE_VEHICLE: bool = true;
/// Retail DamagePercentToUnits residual (percent).
pub const COMBAT_CHINOOK_DAMAGE_PERCENT_TO_UNITS: f32 = 100.0;
/// Retail ExitDelay residual (msec).
pub const COMBAT_CHINOOK_EXIT_DELAY_MS: u32 = 100;
/// ExitDelay 100ms → 3 frames @ 30 FPS.
pub const COMBAT_CHINOOK_EXIT_DELAY_FRAMES: u32 = 3;
/// Retail NumberOfExitPaths residual.
pub const COMBAT_CHINOOK_NUMBER_OF_EXIT_PATHS: u32 = 1;
/// Retail GoAggressiveOnExit residual.
pub const COMBAT_CHINOOK_GO_AGGRESSIVE_ON_EXIT: bool = true;
/// Retail KindOf CAN_ATTACK residual (Combat Chinook only).
pub const COMBAT_CHINOOK_CAN_ATTACK: bool = true;

/// Residual of Weapon.ini `ListeningOutpostUpgradedDummyWeapon` AttackRange.
pub const LISTENING_OUTPOST_DUMMY_RANGE: f32 = 90.0;
/// Residual of Weapon.ini `ListeningOutpostUpgradedDummyWeapon` PrimaryDamage.
pub const LISTENING_OUTPOST_DUMMY_DAMAGE: f32 = 0.1;
/// Residual of Weapon.ini `ListeningOutpostUpgradedDummyWeapon` DelayBetweenShots
/// (1000 msec → 1.0 sec).
pub const LISTENING_OUTPOST_DUMMY_RELOAD_SEC: f32 = 1.0;
/// Retail DelayBetweenShots residual (msec).
pub const LISTENING_OUTPOST_DUMMY_DELAY_MS: u32 = 1_000;
/// Delay 1000ms → 30 frames @ 30 FPS.
pub const LISTENING_OUTPOST_DUMMY_DELAY_FRAMES: u32 = 30;
/// Retail dummy weapon name residual.
pub const LISTENING_OUTPOST_DUMMY_WEAPON: &str = "ListeningOutpostUpgradedDummyWeapon";
/// Retail AcceptableAimDelta residual (degrees).
pub const LISTENING_OUTPOST_DUMMY_AIM_DELTA: f32 = 180.0;
/// Retail AntiAirborneVehicle residual on dummy.
pub const LISTENING_OUTPOST_DUMMY_ANTI_AIR: bool = true;

// --- PointDefenseLaser residual (Combat Chinook "minigun"/PDL residual) ---

/// Retail AirF_PointDefenseLaser weapon name residual.
pub const COMBAT_CHINOOK_PDL_WEAPON: &str = "AirF_PointDefenseLaser";
/// Retail PDL PrimaryDamage residual.
pub const COMBAT_CHINOOK_PDL_DAMAGE: f32 = 100.0;
/// Retail PDL AttackRange residual.
pub const COMBAT_CHINOOK_PDL_RANGE: f32 = 65.0;
/// Retail PDL DelayBetweenShots residual (msec).
pub const COMBAT_CHINOOK_PDL_DELAY_MS: u32 = 250;
/// Delay 250ms → 8 frames @ 30 FPS.
pub const COMBAT_CHINOOK_PDL_DELAY_FRAMES: u32 = 8;
/// Retail PointDefenseLaserUpdate ScanRange residual.
pub const COMBAT_CHINOOK_PDL_SCAN_RANGE: f32 = 250.0;
/// Retail ScanRate residual (msec).
pub const COMBAT_CHINOOK_PDL_SCAN_RATE_MS: u32 = 33;
/// ScanRate 33ms → 1 frame @ 30 FPS (round half-up).
pub const COMBAT_CHINOOK_PDL_SCAN_RATE_FRAMES: u32 = 1;
/// Retail PredictTargetVelocityFactor residual.
pub const COMBAT_CHINOOK_PDL_PREDICT_VELOCITY_FACTOR: f32 = 1.0;
/// Retail FireFX residual name.
pub const COMBAT_CHINOOK_PDL_FIRE_FX: &str = "WeaponFX_PaladinPointDefenseLaser";
/// Retail LaserName residual.
pub const COMBAT_CHINOOK_PDL_LASER_NAME: &str = "AirF_PointDefenseLaserBeam";

// --- ChinookAIUpdate residual ---

/// Retail MaxBoxes residual.
pub const COMBAT_CHINOOK_MAX_BOXES: u32 = 8;
/// Retail NumRopes residual.
pub const COMBAT_CHINOOK_NUM_ROPES: u32 = 4;
/// Retail PerRopeDelayMin residual (msec).
pub const COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_MS: u32 = 900;
/// PerRopeDelayMin 900ms → 27 frames @ 30 FPS.
pub const COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_FRAMES: u32 = 27;
/// Retail PerRopeDelayMax residual (msec).
pub const COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_MS: u32 = 1_500;
/// PerRopeDelayMax 1500ms → 45 frames @ 30 FPS.
pub const COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_FRAMES: u32 = 45;
/// Retail RappelSpeed residual.
pub const COMBAT_CHINOOK_RAPPEL_SPEED: f32 = 30.0;
/// Retail MinDropHeight residual.
pub const COMBAT_CHINOOK_MIN_DROP_HEIGHT: f32 = 40.0;
/// Retail RopeFinalHeight residual.
pub const COMBAT_CHINOOK_ROPE_FINAL_HEIGHT: f32 = 10.0;
/// Retail SupplyCenterActionDelay residual (msec).
pub const COMBAT_CHINOOK_SUPPLY_CENTER_DELAY_MS: u32 = 3_000;
/// SupplyCenterActionDelay 3000ms → 90 frames @ 30 FPS.
pub const COMBAT_CHINOOK_SUPPLY_CENTER_DELAY_FRAMES: u32 = 90;
/// Retail SupplyWarehouseActionDelay residual (msec).
pub const COMBAT_CHINOOK_SUPPLY_WAREHOUSE_DELAY_MS: u32 = 1_250;
/// SupplyWarehouseActionDelay 1250ms → 38 frames @ 30 FPS.
pub const COMBAT_CHINOOK_SUPPLY_WAREHOUSE_DELAY_FRAMES: u32 = 38;
/// Retail SupplyWarehouseScanDistance residual.
pub const COMBAT_CHINOOK_SUPPLY_WAREHOUSE_SCAN_DISTANCE: f32 = 700.0;
/// Retail UpgradedSupplyBoost residual.
pub const COMBAT_CHINOOK_UPGRADED_SUPPLY_BOOST: u32 = 60;

// --- Body / locomotor residual ---

/// Retail MaxHealth residual.
pub const COMBAT_CHINOOK_MAX_HEALTH: f32 = 350.0;
/// Retail VisionRange residual.
pub const COMBAT_CHINOOK_VISION_RANGE: f32 = 300.0;
/// Retail ShroudClearingRange residual.
pub const COMBAT_CHINOOK_SHROUD_CLEARING_RANGE: f32 = 600.0;
/// Retail BuildCost residual.
pub const COMBAT_CHINOOK_BUILD_COST: u32 = 1_200;
/// Retail BuildTime residual (seconds).
pub const COMBAT_CHINOOK_BUILD_TIME_SEC: f32 = 25.0;
/// Retail ChinookLocomotor Speed residual.
pub const COMBAT_CHINOOK_LOCOMOTOR_SPEED: f32 = 150.0;
/// Retail PreferredHeight residual.
pub const COMBAT_CHINOOK_PREFERRED_HEIGHT: f32 = 100.0;
/// Retail TransportSlotCount residual (not transportable as cargo).
pub const COMBAT_CHINOOK_TRANSPORT_SLOT_COUNT: u32 = 0;

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
    /// Wave 58: residual point-defense laser shots booked.
    pub pdl_fires: u32,
    /// Wave 58: residual passenger "minigun" enable events (dummy weapon bind).
    pub minigun_enables: u32,
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
        // Armed riders bind ListeningOutpost dummy — residual "minigun enable".
        self.minigun_enables = self.minigun_enables.saturating_add(1);
    }

    pub fn record_pdl_fire(&mut self) {
        self.pdl_fires = self.pdl_fires.saturating_add(1);
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

    /// Wave 58 residual honesty: minigun enable (dummy weapon bind) booked.
    pub fn honesty_minigun_enable_ok(&self) -> bool {
        self.minigun_enables > 0
    }

    /// Wave 58 residual honesty: PDL fire booked.
    pub fn honesty_pdl_fire_ok(&self) -> bool {
        self.pdl_fires > 0
    }

    /// Combined residual path honesty (load/unload and/or combat).
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_load_unload_ok()
            || self.honesty_passenger_fire_ok()
            || self.honesty_weapon_set_upgrade_ok()
            || self.honesty_pdl_fire_ok()
    }
}

/// Convert msec residual → logic frames @ 30 FPS (round half-up).
pub fn combat_chinook_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / COMBAT_CHINOOK_LOGIC_FPS)).round() as u32
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

/// Residual AirF_PointDefenseLaser weapon (Combat Chinook PDL residual).
pub fn combat_chinook_pdl_weapon() -> Weapon {
    Weapon {
        damage: COMBAT_CHINOOK_PDL_DAMAGE,
        range: COMBAT_CHINOOK_PDL_RANGE,
        min_range: 0.0,
        reload_time: (COMBAT_CHINOOK_PDL_DELAY_FRAMES.max(1) as f32) / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        // AntiSmallMissile residual honesty — air/missile residual target.
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 999_999.0,
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

/// Residual: whether rider kind is allowed inside Combat Chinook.
pub fn combat_chinook_allows_rider(
    is_infantry: bool,
    is_vehicle: bool,
    is_aircraft: bool,
    is_huge_vehicle: bool,
) -> bool {
    if is_aircraft && COMBAT_CHINOOK_FORBID_AIRCRAFT {
        return false;
    }
    if is_huge_vehicle && COMBAT_CHINOOK_FORBID_HUGE_VEHICLE {
        return false;
    }
    (is_infantry && COMBAT_CHINOOK_ALLOW_INFANTRY) || (is_vehicle && COMBAT_CHINOOK_ALLOW_VEHICLE)
}

// --- Wave 58 residual honesty packs ---

/// Wave 58 residual honesty: transport residual.
pub fn honesty_combat_chinook_transport_residual_ok() -> bool {
    COMBAT_CHINOOK_TRANSPORT_SLOTS == 8
        && COMBAT_CHINOOK_PASSENGERS_ALLOWED_TO_FIRE
        && COMBAT_CHINOOK_ARMED_RIDERS_UPGRADE_WEAPON_SET
        && COMBAT_CHINOOK_ALLOW_INFANTRY
        && COMBAT_CHINOOK_ALLOW_VEHICLE
        && COMBAT_CHINOOK_FORBID_AIRCRAFT
        && COMBAT_CHINOOK_FORBID_HUGE_VEHICLE
        && (COMBAT_CHINOOK_DAMAGE_PERCENT_TO_UNITS - 100.0).abs() < 0.01
        && COMBAT_CHINOOK_EXIT_DELAY_MS == 100
        && COMBAT_CHINOOK_EXIT_DELAY_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_EXIT_DELAY_MS)
        && COMBAT_CHINOOK_NUMBER_OF_EXIT_PATHS == 1
        && COMBAT_CHINOOK_GO_AGGRESSIVE_ON_EXIT
        && COMBAT_CHINOOK_CAN_ATTACK
        && combat_chinook_allows_rider(true, false, false, false)
        && combat_chinook_allows_rider(false, true, false, false)
        && !combat_chinook_allows_rider(false, false, true, false)
        && !combat_chinook_allows_rider(false, true, false, true)
}

/// Wave 58 residual honesty: passenger dummy / minigun-enable residual.
pub fn honesty_combat_chinook_minigun_dummy_residual_ok() -> bool {
    LISTENING_OUTPOST_DUMMY_WEAPON == "ListeningOutpostUpgradedDummyWeapon"
        && (LISTENING_OUTPOST_DUMMY_DAMAGE - 0.1).abs() < 0.01
        && (LISTENING_OUTPOST_DUMMY_RANGE - 90.0).abs() < 0.01
        && LISTENING_OUTPOST_DUMMY_DELAY_MS == 1_000
        && LISTENING_OUTPOST_DUMMY_DELAY_FRAMES
            == combat_chinook_ms_to_frames(LISTENING_OUTPOST_DUMMY_DELAY_MS)
        && (LISTENING_OUTPOST_DUMMY_RELOAD_SEC - 1.0).abs() < 0.01
        && LISTENING_OUTPOST_DUMMY_ANTI_AIR
        && (LISTENING_OUTPOST_DUMMY_AIM_DELTA - 180.0).abs() < 0.01
        && is_passenger_dummy_weapon(&listening_outpost_upgraded_dummy_weapon())
}

/// Wave 58 residual honesty: PointDefenseLaser residual.
pub fn honesty_combat_chinook_pdl_residual_ok() -> bool {
    COMBAT_CHINOOK_PDL_WEAPON == "AirF_PointDefenseLaser"
        && (COMBAT_CHINOOK_PDL_DAMAGE - 100.0).abs() < 0.01
        && (COMBAT_CHINOOK_PDL_RANGE - 65.0).abs() < 0.01
        && COMBAT_CHINOOK_PDL_DELAY_MS == 250
        && COMBAT_CHINOOK_PDL_DELAY_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_PDL_DELAY_MS)
        && (COMBAT_CHINOOK_PDL_SCAN_RANGE - 250.0).abs() < 0.01
        && COMBAT_CHINOOK_PDL_SCAN_RATE_MS == 33
        && COMBAT_CHINOOK_PDL_SCAN_RATE_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_PDL_SCAN_RATE_MS)
        && (COMBAT_CHINOOK_PDL_PREDICT_VELOCITY_FACTOR - 1.0).abs() < 0.01
        && COMBAT_CHINOOK_PDL_FIRE_FX == "WeaponFX_PaladinPointDefenseLaser"
        && COMBAT_CHINOOK_PDL_LASER_NAME == "AirF_PointDefenseLaserBeam"
}

/// Wave 58 residual honesty: ChinookAIUpdate / body residual.
pub fn honesty_combat_chinook_ai_body_residual_ok() -> bool {
    COMBAT_CHINOOK_MAX_BOXES == 8
        && COMBAT_CHINOOK_NUM_ROPES == 4
        && COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_MS == 900
        && COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_MS)
        && COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_MS == 1_500
        && COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_MS)
        && (COMBAT_CHINOOK_RAPPEL_SPEED - 30.0).abs() < 0.01
        && (COMBAT_CHINOOK_MIN_DROP_HEIGHT - 40.0).abs() < 0.01
        && (COMBAT_CHINOOK_ROPE_FINAL_HEIGHT - 10.0).abs() < 0.01
        && COMBAT_CHINOOK_SUPPLY_CENTER_DELAY_MS == 3_000
        && COMBAT_CHINOOK_SUPPLY_CENTER_DELAY_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_SUPPLY_CENTER_DELAY_MS)
        && COMBAT_CHINOOK_SUPPLY_WAREHOUSE_DELAY_MS == 1_250
        && COMBAT_CHINOOK_SUPPLY_WAREHOUSE_DELAY_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_SUPPLY_WAREHOUSE_DELAY_MS)
        && (COMBAT_CHINOOK_SUPPLY_WAREHOUSE_SCAN_DISTANCE - 700.0).abs() < 0.01
        && COMBAT_CHINOOK_UPGRADED_SUPPLY_BOOST == 60
        && (COMBAT_CHINOOK_MAX_HEALTH - 350.0).abs() < 0.01
        && (COMBAT_CHINOOK_VISION_RANGE - 300.0).abs() < 0.01
        && (COMBAT_CHINOOK_SHROUD_CLEARING_RANGE - 600.0).abs() < 0.01
        && COMBAT_CHINOOK_BUILD_COST == 1_200
        && (COMBAT_CHINOOK_BUILD_TIME_SEC - 25.0).abs() < 0.01
        && (COMBAT_CHINOOK_LOCOMOTOR_SPEED - 150.0).abs() < 0.01
        && (COMBAT_CHINOOK_PREFERRED_HEIGHT - 100.0).abs() < 0.01
        && COMBAT_CHINOOK_TRANSPORT_SLOT_COUNT == 0
}

/// Combined Wave 58 Combat Chinook residual honesty pack.
pub fn honesty_combat_chinook_residual_pack_ok() -> bool {
    honesty_combat_chinook_transport_residual_ok()
        && honesty_combat_chinook_minigun_dummy_residual_ok()
        && honesty_combat_chinook_pdl_residual_ok()
        && honesty_combat_chinook_ai_body_residual_ok()
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
        assert!(reg.honesty_minigun_enable_ok());
        reg.record_pdl_fire();
        assert!(reg.honesty_pdl_fire_ok());
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
        assert!(combat_chinook_rider_has_viable_weapon(
            Some(&rifle),
            true,
            false
        ));
        assert!(combat_chinook_rider_has_viable_weapon(
            Some(&rifle),
            false,
            true
        ));
        assert!(!combat_chinook_rider_has_viable_weapon(
            Some(&rifle),
            false,
            false
        ));
        let melee = Weapon {
            damage: 20.0,
            range: 3.0,
            ..Weapon::default()
        };
        assert!(!combat_chinook_rider_has_viable_weapon(
            Some(&melee),
            true,
            false
        ));
        assert!(!combat_chinook_rider_has_viable_weapon(None, true, false));
    }

    #[test]
    fn pdl_weapon_stats() {
        let w = combat_chinook_pdl_weapon();
        assert!((w.damage - 100.0).abs() < 0.01);
        assert!((w.range - 65.0).abs() < 0.01);
        assert!(w.can_target_air && !w.can_target_ground);
        assert!((w.reload_time - (8.0 / 30.0)).abs() < 0.001);
    }

    #[test]
    fn combat_chinook_residual_pack_honesty_wave58() {
        assert!(honesty_combat_chinook_residual_pack_ok());
        assert_eq!(combat_chinook_ms_to_frames(100), 3);
        assert_eq!(combat_chinook_ms_to_frames(250), 8);
        assert_eq!(combat_chinook_ms_to_frames(900), 27);
        assert_eq!(combat_chinook_ms_to_frames(1_500), 45);
        assert_eq!(combat_chinook_ms_to_frames(1_250), 38);
        assert_eq!(combat_chinook_ms_to_frames(33), 1);
        assert_eq!(COMBAT_CHINOOK_TRANSPORT_SLOTS, 8);
    }
}
