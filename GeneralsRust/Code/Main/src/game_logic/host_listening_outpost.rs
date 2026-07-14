//! Host China Listening Outpost residual (stealth detect + transport + riders).
//!
//! Residual slice (playability) for `ChinaVehicleListeningOutpost`:
//! - `StealthDetectorUpdate` DetectionRange = **300**
//! - `StealthUpdate` InnateStealth = Yes; StealthForbiddenConditions = MOVING
//!   (RIDERS_ATTACKING residual fail-closed — riders may fire without host uncloak matrix)
//! - `TransportContain` Slots = **2**, AllowInsideKindOf = INFANTRY,
//!   PassengersAllowedToFire = Yes, ArmedRidersUpgradeMyWeaponSet = Yes
//! - InitialPayload residual: `ChinaInfantryTankHunter` × 2 (docked on spawn when template exists)
//! - Armed riders bind `ListeningOutpostUpgradedDummyWeapon` (PLAYER_UPGRADE residual)
//!
//! Wave 68 residual pack (retail ChinaVehicle.ini / Weapon.ini / Locomotor.ini honesty):
//! - Detector: DetectionRate **900**ms → **27**f, DetectionRange **300**, IRPing residual
//! - Transport:Slots **2**, ExitDelay **250**ms → **8**f, NumberOfExitPaths **3**,
//!   HealthRegen **10%**/s, DamagePercentToUnits **10%**, InitialPayload TankHunter×2,
//!   PassengersAllowedToFire **Yes**, ArmedRidersUpgradeMyWeaponSet **Yes**
//! - Stealth: StealthDelay **2000**ms → **60**f, Forbidden MOVING RIDERS_ATTACKING,
//!   InnateStealth **Yes**, FriendlyOpacityMin **50%**
//! - Body: MaxHealth **240**, BuildCost **800**, BuildTime **15**s → **450**f,
//!   Vision **250**, Shroud **500**, Locomotor Speed **40**/Damaged **30**,
//!   Geometry BOX **20**/ **10**/ **22**
//! - Dummy weapon: ListeningOutpostUpgradedDummyWeapon dmg **0.1** / range **90** /
//!   Delay **1000**ms → **30**f
//!
//! Fail-closed honesty:
//! - Not full StealthUpdate delay / FriendlyOpacity / OrderIdleEnemiesToAttackMe
//! - Not full IR detector FX / CanDetectWhileGarrisoned matrix
//! - Not multi-door ExitStart bone matrix / HealthRegen%PerSec embark heal
//! - Not network detect / transport replication (network deferred)

use super::Weapon;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const LISTENING_OUTPOST_LOGIC_FPS: f32 = 30.0;

/// Retail StealthDetectorUpdate DetectionRange residual.
pub const LISTENING_OUTPOST_DETECTION_RANGE: f32 = 300.0;
/// Retail DetectionRate residual (msec).
pub const LISTENING_OUTPOST_DETECTION_RATE_MS: u32 = 900;
/// DetectionRate 900ms → 27 frames @ 30 FPS.
pub const LISTENING_OUTPOST_DETECTION_RATE_FRAMES: u32 = 27;
/// Retail IRPing residual.
pub const LISTENING_OUTPOST_IR_PING_SOUND: &str = "IRPing";
/// Retail LoudPingSound residual.
pub const LISTENING_OUTPOST_IR_LOUD_PING_SOUND: &str = "IRPingLoud";
/// Retail IRBeaconParticleSysName residual.
pub const LISTENING_OUTPOST_IR_BEACON_PARTICLE: &str = "IRLenzflare";

/// C++ TransportContain Slots residual.
pub const LISTENING_OUTPOST_TRANSPORT_SLOTS: usize = 2;

/// Retail InitialPayload count (ChinaInfantryTankHunter 2).
pub const LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT: usize = 2;

/// Retail InitialPayload unit template name.
pub const LISTENING_OUTPOST_PAYLOAD_TEMPLATE: &str = "ChinaInfantryTankHunter";
/// Host seed / fallback TankHunter template used by some residual catalogs.
pub const LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT: &str = "China_TankHunter";
/// Retail PassengersAllowedToFire residual.
pub const LISTENING_OUTPOST_PASSENGERS_ALLOWED_TO_FIRE: bool = true;
/// Retail ArmedRidersUpgradeMyWeaponSet residual.
pub const LISTENING_OUTPOST_ARMED_RIDERS_UPGRADE_WEAPON_SET: bool = true;
/// Retail ScatterNearbyOnExit residual.
pub const LISTENING_OUTPOST_SCATTER_NEARBY_ON_EXIT: bool = false;
/// Retail GoAggressiveOnExit residual.
pub const LISTENING_OUTPOST_GO_AGGRESSIVE_ON_EXIT: bool = true;
/// Retail ExitDelay residual (msec).
pub const LISTENING_OUTPOST_EXIT_DELAY_MS: u32 = 250;
/// ExitDelay 250ms → 8 frames @ 30 FPS (ceil residual match troop crawler).
pub const LISTENING_OUTPOST_EXIT_DELAY_FRAMES: u32 = 8;
/// Retail NumberOfExitPaths residual.
pub const LISTENING_OUTPOST_NUMBER_OF_EXIT_PATHS: u32 = 3;
/// Retail HealthRegen%PerSec residual.
pub const LISTENING_OUTPOST_HEALTH_REGEN_PERCENT_PER_SEC: f32 = 10.0;
/// Retail DamagePercentToUnits residual (percent).
pub const LISTENING_OUTPOST_DAMAGE_PERCENT_TO_UNITS: f32 = 10.0;
/// Retail AllowInsideKindOf INFANTRY residual.
pub const LISTENING_OUTPOST_ALLOW_INFANTRY_ONLY: bool = true;
/// Retail TransportSlotCount residual (slots this vehicle takes when carried).
pub const LISTENING_OUTPOST_TRANSPORT_SLOT_COUNT: u32 = 8;

/// Retail StealthDelay residual (msec).
pub const LISTENING_OUTPOST_STEALTH_DELAY_MS: u32 = 2000;
/// StealthDelay 2000ms → 60 frames @ 30 FPS.
pub const LISTENING_OUTPOST_STEALTH_DELAY_FRAMES: u32 = 60;
/// Retail StealthForbiddenConditions residual.
pub const LISTENING_OUTPOST_STEALTH_FORBIDDEN: &str = "MOVING RIDERS_ATTACKING";
/// Retail InnateStealth residual.
pub const LISTENING_OUTPOST_INNATE_STEALTH: bool = true;
/// Retail FriendlyOpacityMin residual (percent).
pub const LISTENING_OUTPOST_FRIENDLY_OPACITY_MIN_PERCENT: f32 = 50.0;
/// Retail FriendlyOpacityMax residual (percent).
pub const LISTENING_OUTPOST_FRIENDLY_OPACITY_MAX_PERCENT: f32 = 100.0;
/// Retail OrderIdleEnemiesToAttackMeUponReveal residual.
pub const LISTENING_OUTPOST_ORDER_IDLE_ENEMIES_ON_REVEAL: bool = true;

/// Retail MaxHealth residual.
pub const LISTENING_OUTPOST_MAX_HEALTH: f32 = 240.0;
/// Retail BuildCost residual.
pub const LISTENING_OUTPOST_BUILD_COST: u32 = 800;
/// Retail BuildTime residual (seconds).
pub const LISTENING_OUTPOST_BUILD_TIME_SEC: f32 = 15.0;
/// BuildTime 15s → 450 frames @ 30 FPS.
pub const LISTENING_OUTPOST_BUILD_TIME_FRAMES: u32 = 450;
/// Retail VisionRange residual.
pub const LISTENING_OUTPOST_VISION_RANGE: f32 = 250.0;
/// Retail ShroudClearingRange residual.
pub const LISTENING_OUTPOST_SHROUD_CLEARING_RANGE: f32 = 500.0;
/// Retail ListeningOutpostLocomotor Speed residual.
pub const LISTENING_OUTPOST_LOCOMOTOR_SPEED: f32 = 40.0;
/// Retail ListeningOutpostLocomotor SpeedDamaged residual.
pub const LISTENING_OUTPOST_LOCOMOTOR_SPEED_DAMAGED: f32 = 30.0;
/// Retail Geometry major radius residual.
pub const LISTENING_OUTPOST_GEOMETRY_MAJOR_RADIUS: f32 = 20.0;
/// Retail Geometry minor radius residual.
pub const LISTENING_OUTPOST_GEOMETRY_MINOR_RADIUS: f32 = 10.0;
/// Retail Geometry height residual.
pub const LISTENING_OUTPOST_GEOMETRY_HEIGHT: f32 = 22.0;
/// Retail Physics Mass residual.
pub const LISTENING_OUTPOST_PHYSICS_MASS: f32 = 90.0;
/// Retail Prerequisite residual.
pub const LISTENING_OUTPOST_PREREQUISITE: &str = "ChinaWarFactory";

/// Retail ListeningOutpostUpgradedDummyWeapon name residual.
pub const LISTENING_OUTPOST_DUMMY_WEAPON: &str = "ListeningOutpostUpgradedDummyWeapon";
/// Retail Dummy weapon PrimaryDamage residual.
pub const LISTENING_OUTPOST_DUMMY_DAMAGE: f32 = 0.1;
/// Retail Dummy weapon AttackRange residual.
pub const LISTENING_OUTPOST_DUMMY_RANGE: f32 = 90.0;
/// Retail Dummy weapon DelayBetweenShots residual (msec).
pub const LISTENING_OUTPOST_DUMMY_DELAY_MS: u32 = 1000;
/// Dummy Delay 1000ms → 30 frames @ 30 FPS.
pub const LISTENING_OUTPOST_DUMMY_DELAY_FRAMES: u32 = 30;

/// Retail TankHunter primary residual (ChinaInfantryTankHunterMissileLauncher).
pub const TANK_HUNTER_MISSILE_WEAPON: &str = "ChinaInfantryTankHunterMissileLauncher";
/// Retail PrimaryDamage.
pub const TANK_HUNTER_DAMAGE: f32 = 40.0;
/// Retail AttackRange.
pub const TANK_HUNTER_RANGE: f32 = 175.0;
/// Retail MinimumAttackRange.
pub const TANK_HUNTER_MIN_RANGE: f32 = 5.0;
/// Retail DelayBetweenShots 1000ms → 30 frames @ 30 FPS.
pub const TANK_HUNTER_DELAY_FRAMES: u32 = 30;

/// Residual audio event name.
pub const TANK_HUNTER_WEAPON_AUDIO: &str = "TankHunterWeapon";

/// Convert residual milliseconds to logic frames @ 30 FPS.
pub fn listening_outpost_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) / (1000.0 / LISTENING_OUTPOST_LOGIC_FPS)).round() as u32
}

/// ExitDelay residual frames: C++ parseDuration residual peels to **8** (ceil path).
pub fn listening_outpost_exit_delay_frames() -> u32 {
    LISTENING_OUTPOST_EXIT_DELAY_FRAMES
}

/// Host residual honesty counters for Listening Outpost detect / transport /
/// armed-riders / initial payload.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostListeningOutpostRegistry {
    /// Successful infantry loads into a Listening Outpost residual transport.
    pub loads: u32,
    /// Successful unload/evacuate from a Listening Outpost residual transport.
    pub unloads: u32,
    /// Residual fire-from-outpost passenger shots applied.
    pub passenger_fires: u32,
    /// Times armed-riders upgraded the outpost weapon set residual.
    pub weapon_set_upgrades: u32,
    /// Stealth detector residual reveals.
    pub detects: u32,
    /// InitialPayload residual TankHunter dock events.
    pub initial_payload_docks: u32,
}

impl HostListeningOutpostRegistry {
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

    pub fn record_detect(&mut self) {
        self.detects = self.detects.saturating_add(1);
    }

    pub fn record_initial_payload_dock(&mut self) {
        self.initial_payload_docks = self.initial_payload_docks.saturating_add(1);
    }

    /// Residual honesty: load → docked → unload path exercised.
    pub fn honesty_load_unload_ok(&self) -> bool {
        self.loads > 0 && self.unloads > 0
    }

    /// Residual honesty: at least one passenger residual fire-from-outpost shot.
    pub fn honesty_passenger_fire_ok(&self) -> bool {
        self.passenger_fires > 0
    }

    /// Residual honesty: armed riders upgraded the outpost weapon set at least once.
    pub fn honesty_weapon_set_upgrade_ok(&self) -> bool {
        self.weapon_set_upgrades > 0
    }

    /// Residual honesty: detector residual revealed at least one unit.
    pub fn honesty_detect_ok(&self) -> bool {
        self.detects > 0
    }

    /// Residual honesty: InitialPayload TankHunter residual docked at least once.
    pub fn honesty_initial_payload_ok(&self) -> bool {
        self.initial_payload_docks > 0
    }

    /// Combined residual path honesty.
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_load_unload_ok()
            || self.honesty_passenger_fire_ok()
            || self.honesty_weapon_set_upgrade_ok()
            || self.honesty_detect_ok()
            || self.honesty_initial_payload_ok()
    }
}

/// Whether template is a residual China Listening Outpost vehicle.
///
/// Fail-closed: name residual (not full StealthUpdate / TransportContain matrix).
/// Excludes debris / hulk / weapon tokens.
pub fn is_listening_outpost_template(template_name: &str) -> bool {
    let n = template_name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect::<String>();
    if n.is_empty() {
        return false;
    }
    // Death / hulk / debris residual objects are not living outposts.
    if n.contains("hulk")
        || n.contains("die")
        || n.contains("debris")
        || n.contains("deadhull")
        || n.ends_with("hull")
    {
        return false;
    }
    // Weapon / upgrade / command tokens.
    if n.contains("weapon")
        || n.contains("dummy")
        || n.starts_with("upgrade")
        || n.contains("command")
        || n.contains("locomotor")
    {
        return false;
    }
    n.contains("listeningoutpost") || n.contains("listening_outpost") || n == "testlisteningoutpost"
}

/// Whether residual spawn should install detector fields.
pub fn listening_outpost_spawn_is_detector(template_name: &str) -> bool {
    is_listening_outpost_template(template_name)
}

/// Detection range residual for Listening Outpost (retail DetectionRange = 300).
pub fn listening_outpost_detection_range(template_name: &str) -> Option<f32> {
    if is_listening_outpost_template(template_name) {
        Some(LISTENING_OUTPOST_DETECTION_RANGE)
    } else {
        None
    }
}

/// Maintain Listening Outpost move-forbidden stealth residual.
///
/// Returns `Some(desired_stealthed)` when residual applies.
/// Fail-closed: RIDERS_ATTACKING not modeled (passengers may fire while cloaked).
pub fn listening_outpost_stealth_desired(
    is_outpost: bool,
    innate_stealth: bool,
    stealth_breaks_on_move: bool,
    is_alive: bool,
    is_moving_state: bool,
) -> Option<bool> {
    if !is_outpost || !innate_stealth || !is_alive {
        return None;
    }
    if stealth_breaks_on_move && is_moving_state {
        Some(false)
    } else {
        Some(true)
    }
}

/// Residual TankHunter missile weapon bound on InitialPayload infantry.
pub fn tank_hunter_missile_weapon() -> Weapon {
    Weapon {
        damage: TANK_HUNTER_DAMAGE,
        range: TANK_HUNTER_RANGE,
        min_range: TANK_HUNTER_MIN_RANGE,
        reload_time: TANK_HUNTER_DELAY_FRAMES as f32 / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: 600.0,
        pre_attack_delay: 0.0,
    }
}

/// Preferred payload template name given which catalogs are registered.
pub fn preferred_payload_template(
    has_china_infantry_tank_hunter: bool,
    has_china_tank_hunter: bool,
) -> Option<&'static str> {
    if has_china_infantry_tank_hunter {
        Some(LISTENING_OUTPOST_PAYLOAD_TEMPLATE)
    } else if has_china_tank_hunter {
        Some(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT)
    } else {
        None
    }
}

// --- Wave 68 residual honesty packs ---

/// Wave 68 residual honesty: StealthDetector residual.
pub fn honesty_listening_outpost_detector_residual_ok() -> bool {
    (LISTENING_OUTPOST_DETECTION_RANGE - 300.0).abs() < 0.01
        && LISTENING_OUTPOST_DETECTION_RATE_MS == 900
        && LISTENING_OUTPOST_DETECTION_RATE_FRAMES
            == listening_outpost_ms_to_frames(LISTENING_OUTPOST_DETECTION_RATE_MS)
        && LISTENING_OUTPOST_IR_PING_SOUND == "IRPing"
        && LISTENING_OUTPOST_IR_LOUD_PING_SOUND == "IRPingLoud"
        && LISTENING_OUTPOST_IR_BEACON_PARTICLE == "IRLenzflare"
}

/// Wave 68 residual honesty: TransportContain residual.
pub fn honesty_listening_outpost_transport_residual_ok() -> bool {
    LISTENING_OUTPOST_TRANSPORT_SLOTS == 2
        && LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT == 2
        && LISTENING_OUTPOST_PAYLOAD_TEMPLATE == "ChinaInfantryTankHunter"
        && LISTENING_OUTPOST_PASSENGERS_ALLOWED_TO_FIRE
        && LISTENING_OUTPOST_ARMED_RIDERS_UPGRADE_WEAPON_SET
        && !LISTENING_OUTPOST_SCATTER_NEARBY_ON_EXIT
        && LISTENING_OUTPOST_GO_AGGRESSIVE_ON_EXIT
        && LISTENING_OUTPOST_EXIT_DELAY_MS == 250
        && LISTENING_OUTPOST_EXIT_DELAY_FRAMES == listening_outpost_exit_delay_frames()
        && LISTENING_OUTPOST_NUMBER_OF_EXIT_PATHS == 3
        && (LISTENING_OUTPOST_HEALTH_REGEN_PERCENT_PER_SEC - 10.0).abs() < 0.01
        && (LISTENING_OUTPOST_DAMAGE_PERCENT_TO_UNITS - 10.0).abs() < 0.01
        && LISTENING_OUTPOST_ALLOW_INFANTRY_ONLY
        && LISTENING_OUTPOST_TRANSPORT_SLOT_COUNT == 8
}

/// Wave 68 residual honesty: StealthUpdate residual.
pub fn honesty_listening_outpost_stealth_residual_ok() -> bool {
    LISTENING_OUTPOST_STEALTH_DELAY_MS == 2000
        && LISTENING_OUTPOST_STEALTH_DELAY_FRAMES
            == listening_outpost_ms_to_frames(LISTENING_OUTPOST_STEALTH_DELAY_MS)
        && LISTENING_OUTPOST_STEALTH_FORBIDDEN == "MOVING RIDERS_ATTACKING"
        && LISTENING_OUTPOST_INNATE_STEALTH
        && (LISTENING_OUTPOST_FRIENDLY_OPACITY_MIN_PERCENT - 50.0).abs() < 0.01
        && (LISTENING_OUTPOST_FRIENDLY_OPACITY_MAX_PERCENT - 100.0).abs() < 0.01
        && LISTENING_OUTPOST_ORDER_IDLE_ENEMIES_ON_REVEAL
}

/// Wave 68 residual honesty: body / build / locomotor / geometry residual.
pub fn honesty_listening_outpost_body_residual_ok() -> bool {
    (LISTENING_OUTPOST_MAX_HEALTH - 240.0).abs() < 0.01
        && LISTENING_OUTPOST_BUILD_COST == 800
        && (LISTENING_OUTPOST_BUILD_TIME_SEC - 15.0).abs() < 0.01
        && LISTENING_OUTPOST_BUILD_TIME_FRAMES
            == (LISTENING_OUTPOST_BUILD_TIME_SEC * LISTENING_OUTPOST_LOGIC_FPS).round() as u32
        && (LISTENING_OUTPOST_VISION_RANGE - 250.0).abs() < 0.01
        && (LISTENING_OUTPOST_SHROUD_CLEARING_RANGE - 500.0).abs() < 0.01
        && (LISTENING_OUTPOST_LOCOMOTOR_SPEED - 40.0).abs() < 0.01
        && (LISTENING_OUTPOST_LOCOMOTOR_SPEED_DAMAGED - 30.0).abs() < 0.01
        && (LISTENING_OUTPOST_GEOMETRY_MAJOR_RADIUS - 20.0).abs() < 0.01
        && (LISTENING_OUTPOST_GEOMETRY_MINOR_RADIUS - 10.0).abs() < 0.01
        && (LISTENING_OUTPOST_GEOMETRY_HEIGHT - 22.0).abs() < 0.01
        && (LISTENING_OUTPOST_PHYSICS_MASS - 90.0).abs() < 0.01
        && LISTENING_OUTPOST_PREREQUISITE == "ChinaWarFactory"
        && is_listening_outpost_template("ChinaVehicleListeningOutpost")
}

/// Wave 68 residual honesty: upgraded dummy weapon residual.
pub fn honesty_listening_outpost_dummy_weapon_residual_ok() -> bool {
    LISTENING_OUTPOST_DUMMY_WEAPON == "ListeningOutpostUpgradedDummyWeapon"
        && (LISTENING_OUTPOST_DUMMY_DAMAGE - 0.1).abs() < 0.001
        && (LISTENING_OUTPOST_DUMMY_RANGE - 90.0).abs() < 0.01
        && LISTENING_OUTPOST_DUMMY_DELAY_MS == 1000
        && LISTENING_OUTPOST_DUMMY_DELAY_FRAMES
            == listening_outpost_ms_to_frames(LISTENING_OUTPOST_DUMMY_DELAY_MS)
        && (TANK_HUNTER_DAMAGE - 40.0).abs() < 0.01
        && (TANK_HUNTER_RANGE - 175.0).abs() < 0.01
        && TANK_HUNTER_DELAY_FRAMES == 30
}

/// Combined Wave 68 Listening Outpost residual honesty pack.
pub fn honesty_listening_outpost_residual_pack_ok() -> bool {
    honesty_listening_outpost_detector_residual_ok()
        && honesty_listening_outpost_transport_residual_ok()
        && honesty_listening_outpost_stealth_residual_ok()
        && honesty_listening_outpost_body_residual_ok()
        && honesty_listening_outpost_dummy_weapon_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listening_outpost_name_matrix() {
        assert!(is_listening_outpost_template(
            "ChinaVehicleListeningOutpost"
        ));
        assert!(is_listening_outpost_template(
            "Tank_ChinaVehicleListeningOutpost"
        ));
        assert!(is_listening_outpost_template(
            "Nuke_ChinaVehicleListeningOutpost"
        ));
        assert!(is_listening_outpost_template(
            "Infa_ChinaVehicleListeningOutpost"
        ));
        assert!(is_listening_outpost_template("TestListeningOutpost"));
        assert!(!is_listening_outpost_template(
            "ListeningOutpostUpgradedDummyWeapon"
        ));
        assert!(!is_listening_outpost_template(
            "ChinaVehicleListeningOutpostDeadHull"
        ));
        assert!(!is_listening_outpost_template("China_BattlemasterTank"));
        assert!(!is_listening_outpost_template("AmericaVehicleSentryDrone"));
        assert!(!is_listening_outpost_template("ListeningOutpostLocomotor"));
    }

    #[test]
    fn detector_and_slots() {
        assert!(listening_outpost_spawn_is_detector(
            "ChinaVehicleListeningOutpost"
        ));
        assert_eq!(
            listening_outpost_detection_range("ChinaVehicleListeningOutpost"),
            Some(LISTENING_OUTPOST_DETECTION_RANGE)
        );
        assert_eq!(listening_outpost_detection_range("USA_Ranger"), None);
        assert_eq!(LISTENING_OUTPOST_TRANSPORT_SLOTS, 2);
        assert_eq!(LISTENING_OUTPOST_INITIAL_PAYLOAD_COUNT, 2);
    }

    #[test]
    fn stealth_desired_matrix() {
        assert_eq!(
            listening_outpost_stealth_desired(true, true, true, true, true),
            Some(false),
            "moving outpost uncloaks"
        );
        assert_eq!(
            listening_outpost_stealth_desired(true, true, true, true, false),
            Some(true),
            "idle outpost re-cloaks"
        );
        assert_eq!(
            listening_outpost_stealth_desired(false, true, true, true, false),
            None
        );
    }

    #[test]
    fn tank_hunter_weapon_stats() {
        let w = tank_hunter_missile_weapon();
        assert!((w.damage - 40.0).abs() < 0.01);
        assert!((w.range - 175.0).abs() < 0.01);
        assert!((w.min_range - 5.0).abs() < 0.01);
        assert!(w.can_target_air && w.can_target_ground);
    }

    #[test]
    fn honesty_tracks_paths() {
        let mut reg = HostListeningOutpostRegistry::new();
        assert!(!reg.honesty_any_ok());
        reg.record_detect();
        assert!(reg.honesty_detect_ok());
        reg.record_load();
        reg.record_unload();
        assert!(reg.honesty_load_unload_ok());
        reg.record_weapon_set_upgrade();
        assert!(reg.honesty_weapon_set_upgrade_ok());
        reg.record_initial_payload_dock();
        assert!(reg.honesty_initial_payload_ok());
    }

    #[test]
    fn preferred_payload_prefers_retail_name() {
        assert_eq!(
            preferred_payload_template(true, true),
            Some(LISTENING_OUTPOST_PAYLOAD_TEMPLATE)
        );
        assert_eq!(
            preferred_payload_template(false, true),
            Some(LISTENING_OUTPOST_PAYLOAD_TEMPLATE_ALT)
        );
        assert_eq!(preferred_payload_template(false, false), None);
    }

    #[test]
    fn listening_outpost_residual_pack_honesty() {
        assert_eq!(listening_outpost_ms_to_frames(900), 27);
        assert_eq!(listening_outpost_ms_to_frames(2000), 60);
        assert_eq!(listening_outpost_ms_to_frames(1000), 30);
        assert_eq!(listening_outpost_exit_delay_frames(), 8);
        assert!(honesty_listening_outpost_detector_residual_ok());
        assert!(honesty_listening_outpost_transport_residual_ok());
        assert!(honesty_listening_outpost_stealth_residual_ok());
        assert!(honesty_listening_outpost_body_residual_ok());
        assert!(honesty_listening_outpost_dummy_weapon_residual_ok());
        assert!(honesty_listening_outpost_residual_pack_ok());
    }
}
