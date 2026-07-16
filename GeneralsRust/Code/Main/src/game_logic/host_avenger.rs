//! Host America Avenger residual (Target Designator FAERIE_FIRE + air lasers).
//!
//! Residual slice (playability):
//! - AmericaTankAvenger / *Avenger* residual sources paint enemy targets with
//!   `OBJECT_STATUS_FAERIE_FIRE` via primary `AvengerTargetDesignator`
//!   (DamageType=STATUS, DamageStatusType=FAERIE_FIRE, PrimaryDamage = duration ms).
//! - Units shooting a FAERIE_FIRE target gain residual TARGET_FAERIE_FIRE
//!   rate-of-fire **150%** (GameData.ini WeaponBonus).
//! - Air laser residual secondary: dual-turret collapse into one AA stream
//!   (`AvengerAirLaserOne` stats: 10 dmg / 300 range / 200ms delay / air only).
//! - PointDefenseLaser intercept remains in `host_point_defense` (already closed).
//!
//! Wave 66 residual pack (retail AmericaVehicle.ini / Weapon.ini / Locomotor.ini):
//! - Designator residual: DamageType STATUS / DamageStatusType FAERIE_FIRE,
//!   PrimaryDamage **200**ms duration, range **200**, Delay **200**ms → **6**f.
//! - Air laser residual: dmg **10** / range **300** / Delay **200**ms → **6**f,
//!   AntiGround **No**, AntiAirborneVehicle **Yes**, AntiAirborneInfantry **No**,
//!   DamageType SMALL_ARMS, LaserName AvengerLaserBeam.
//! - Body residual: MaxHealth **300**, Vision **150**, Shroud **300**, BuildCost **2000**,
//!   BuildTime **10**s → **300**f, TransportSlotCount **3**, Speed **30**/Damaged **20**.
//! - PDL residual: AvengerPointDefenseLaserOne/Two ScanRange **200**.
//!
//! Fail-closed honesty:
//! - Not full portable AmericaTankAvengerLaserTurret OverlordContain passenger
//! - Not full dual independent AirLaser streams / laser drawable bone attach
//! - Not full StatusDamageHelper Xfer / multi-status exclusivity matrix
//! - Not network FAERIE_FIRE / Avenger replication (network deferred)

use super::Weapon;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const AVENGER_LOGIC_FPS: f32 = 30.0;

/// Retail AvengerTargetDesignator weapon name.
pub const AVENGER_TARGET_DESIGNATOR: &str = "AvengerTargetDesignator";
/// Retail AvengerAirLaserOne (turret PRIMARY; residual collapses dual lasers).
pub const AVENGER_AIR_LASER: &str = "AvengerAirLaserOne";
/// Retail AvengerAirLaserTwo (second turret residual name).
pub const AVENGER_AIR_LASER_TWO: &str = "AvengerAirLaserTwo";
/// Retail AvengerAirLaserDummy SECONDARY on chassis WeaponSet.
pub const AVENGER_AIR_LASER_DUMMY: &str = "AvengerAirLaserDummy";

/// Retail AvengerTargetDesignator AttackRange residual.
pub const AVENGER_DESIGNATOR_RANGE: f32 = 200.0;
/// Retail designator DelayBetweenShots residual (msec).
pub const AVENGER_DESIGNATOR_DELAY_MS: u32 = 200;
/// Retail AvengerTargetDesignator DelayBetweenShots 200ms → 6 frames @ 30 FPS.
pub const AVENGER_DESIGNATOR_DELAY_FRAMES: u32 = 6;
/// Retail PrimaryDamage is status duration in msec (ActiveBody DAMAGE_STATUS path).
/// 200 ms → ConvertDurationFromMsecsToFrames ≈ 6 frames @ 30 FPS.
pub const AVENGER_FAERIE_FIRE_DURATION_MS: u32 = 200;
/// Duration in logic frames (ceil(ms * 30 / 1000)).
pub const AVENGER_FAERIE_FIRE_DURATION_FRAMES: u32 = 6;
/// Retail designator DamageType residual.
pub const AVENGER_DESIGNATOR_DAMAGE_TYPE: &str = "STATUS";
/// Retail designator DamageStatusType residual.
pub const AVENGER_DAMAGE_STATUS_TYPE: &str = "FAERIE_FIRE";
/// Retail designator FireFX residual.
pub const AVENGER_DESIGNATOR_FIRE_FX: &str = "WeaponFX_AvengerTargetDesignator";

/// Retail AvengerAirLaserOne PrimaryDamage residual.
pub const AVENGER_AIR_LASER_DAMAGE: f32 = 10.0;
/// Retail AvengerAirLaserOne AttackRange residual.
pub const AVENGER_AIR_LASER_RANGE: f32 = 300.0;
/// Retail air laser DelayBetweenShots residual (msec).
pub const AVENGER_AIR_LASER_DELAY_MS: u32 = 200;
/// Retail AvengerAirLaserOne DelayBetweenShots 200ms → 6 frames.
pub const AVENGER_AIR_LASER_DELAY_FRAMES: u32 = 6;
/// Retail air laser DamageType residual.
pub const AVENGER_AIR_LASER_DAMAGE_TYPE: &str = "SMALL_ARMS";
/// Retail air laser AntiGround residual.
pub const AVENGER_AIR_LASER_ANTI_GROUND: bool = false;
/// Retail air laser AntiAirborneVehicle residual.
pub const AVENGER_AIR_LASER_ANTI_AIRBORNE_VEHICLE: bool = true;
/// Retail air laser AntiAirborneInfantry residual.
pub const AVENGER_AIR_LASER_ANTI_AIRBORNE_INFANTRY: bool = false;
/// Retail LaserName residual.
pub const AVENGER_LASER_NAME: &str = "AvengerLaserBeam";

/// Retail GameData.ini WeaponBonus TARGET_FAERIE_FIRE RATE_OF_FIRE 150%.
pub const FAERIE_FIRE_ROF_MULTIPLIER: f32 = 1.50;

/// Paint / designator audio residual.
pub const AVENGER_PAINT_AUDIO: &str = "AvengerPaintWeaponLoop";
/// Air laser fire audio residual.
pub const AVENGER_AIR_LASER_AUDIO: &str = "AvengerAirLaserWeapon";

// --- Body residual (AmericaTankAvenger) ---

/// Retail ActiveBody MaxHealth residual.
pub const AVENGER_MAX_HEALTH: f32 = 300.0;
/// Retail VisionRange residual.
pub const AVENGER_VISION_RANGE: f32 = 150.0;
/// Retail ShroudClearingRange residual.
pub const AVENGER_SHROUD_CLEARING_RANGE: f32 = 300.0;
/// Retail BuildCost residual.
pub const AVENGER_BUILD_COST: u32 = 2000;
/// Retail BuildTime residual (seconds).
pub const AVENGER_BUILD_TIME_SEC: f32 = 10.0;
/// BuildTime 10s → 300 frames @ 30 FPS.
pub const AVENGER_BUILD_TIME_FRAMES: u32 = 300;
/// Retail TransportSlotCount residual.
pub const AVENGER_TRANSPORT_SLOT_COUNT: u32 = 3;
/// Retail AvengerLocomotor Speed residual.
pub const AVENGER_LOCOMOTOR_SPEED: f32 = 30.0;
/// Retail AvengerLocomotor SpeedDamaged residual.
pub const AVENGER_LOCOMOTOR_SPEED_DAMAGED: f32 = 20.0;
/// Retail PointDefenseLaser ScanRange residual.
pub const AVENGER_PDL_SCAN_RANGE: f32 = 200.0;
/// Retail PointDefenseLaser weapon residual names.
pub const AVENGER_PDL_ONE: &str = "AvengerPointDefenseLaserOne";
pub const AVENGER_PDL_TWO: &str = "AvengerPointDefenseLaserTwo";

/// Convert residual milliseconds to logic frames @ 30 FPS (round half-up).
pub fn avenger_ms_to_frames(ms: u32) -> u32 {
    if ms == 0 {
        return 0;
    }
    ((ms as f32) * AVENGER_LOGIC_FPS / 1000.0).round() as u32
}

/// Host residual honesty counters for Avenger paint / air laser / ROF grant.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostAvengerRegistry {
    pub paints: u32,
    pub air_laser_fires: u32,
    pub rof_grants: u32,
}

impl HostAvengerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::new();
    }

    pub fn record_paint(&mut self) {
        self.paints = self.paints.saturating_add(1);
    }

    pub fn record_air_laser_fire(&mut self) {
        self.air_laser_fires = self.air_laser_fires.saturating_add(1);
    }

    pub fn record_rof_grant(&mut self) {
        self.rof_grants = self.rof_grants.saturating_add(1);
    }

    pub fn honesty_paint_ok(&self) -> bool {
        self.paints > 0
    }

    pub fn honesty_air_laser_ok(&self) -> bool {
        self.air_laser_fires > 0
    }

    pub fn honesty_rof_ok(&self) -> bool {
        self.rof_grants > 0
    }

    pub fn honesty_ok(&self) -> bool {
        self.honesty_paint_ok() || self.honesty_air_laser_ok() || self.honesty_rof_ok()
    }
}

/// Whether template is a residual America Avenger AA vehicle.
///
/// Fail-closed: name residual (not full OverlordContain turret matrix).
/// Excludes laser beams, designator FX, and pure weapon names.
pub fn is_avenger_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n == "testavenger" || n == "usa_avenger" {
        return true;
    }
    // Weapon / laser / turret subsystem objects are not the chassis residual.
    if n.contains("weapon")
        || n.contains("laserbeam")
        || n.contains("laserturret")
        || n.contains("pointdefense")
        || n.contains("designator")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("shell")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
    {
        return false;
    }
    // AmericaTankAvenger / USA_Avenger / SupW_AmericaVehicleAvenger / …
    // Avoid AmericaTankAvengerLaserTurret (portable turret).
    if n.contains("laserturret") {
        return false;
    }
    n.contains("avenger")
}

/// Whether residual Avenger shot should paint FAERIE_FIRE (primary designator path).
///
/// Retail: PRIMARY AvengerTargetDesignator applies status; secondary is air dummy.
/// Residual: primary slot paints; secondary air laser deals HP damage vs aircraft.
pub fn should_apply_faerie_fire_paint(
    is_avenger: bool,
    weapon_slot: u8,
    target_alive: bool,
    enemy_or_forced: bool,
) -> bool {
    is_avenger && weapon_slot == 0 && target_alive && enemy_or_forced
}

/// Whether residual Avenger air laser should deal HP damage (secondary slot).
pub fn should_apply_avenger_air_laser(
    is_avenger: bool,
    weapon_slot: u8,
    target_is_air: bool,
    target_alive: bool,
    enemy_or_forced: bool,
) -> bool {
    is_avenger && weapon_slot == 1 && target_is_air && target_alive && enemy_or_forced
}

/// Effective weapon reload seconds under TARGET_FAERIE_FIRE ROF residual.
///
/// `reload_time / 1.5` when target is painted; identity otherwise.
pub fn effective_reload_vs_target(reload_time: f32, target_has_faerie_fire: bool) -> f32 {
    if target_has_faerie_fire && reload_time > 0.0 {
        reload_time / FAERIE_FIRE_ROF_MULTIPLIER
    } else {
        reload_time
    }
}

/// Weapon ready check with FAERIE_FIRE ROF residual.
pub fn weapon_ready_vs_faerie(
    last_fire_time: f32,
    reload_time: f32,
    current_time: f32,
    target_has_faerie_fire: bool,
) -> bool {
    let effective = effective_reload_vs_target(reload_time, target_has_faerie_fire);
    current_time - last_fire_time >= effective
}

/// Build residual designator primary weapon (status paint; no HP damage residual).
pub fn avenger_designator_weapon() -> Weapon {
    Weapon {
        damage: 0.0, // STATUS residual — no hitpoint damage
        range: AVENGER_DESIGNATOR_RANGE,
        min_range: 0.0,
        reload_time: AVENGER_DESIGNATOR_DELAY_FRAMES as f32 / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true, // can paint air targets residual
        can_target_ground: true,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Build residual dual-laser-collapse AA secondary weapon.
pub fn avenger_air_laser_weapon() -> Weapon {
    Weapon {
        damage: AVENGER_AIR_LASER_DAMAGE,
        range: AVENGER_AIR_LASER_RANGE,
        min_range: 0.0,
        reload_time: AVENGER_AIR_LASER_DELAY_FRAMES as f32 / 30.0,
        last_fire_time: 0.0,
        ammo: None,
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
    }
}

/// Prefer air laser secondary when Avenger targets aircraft.
pub fn avenger_prefer_air_laser(is_avenger: bool, target_is_air: bool) -> bool {
    is_avenger && target_is_air
}

// --- Wave 66 residual honesty packs ---

/// Wave 66 residual honesty: designator / FAERIE_FIRE residual peel.
pub fn honesty_avenger_designator_residual_ok() -> bool {
    AVENGER_TARGET_DESIGNATOR == "AvengerTargetDesignator"
        && (AVENGER_DESIGNATOR_RANGE - 200.0).abs() < 0.01
        && AVENGER_DESIGNATOR_DELAY_MS == 200
        && AVENGER_DESIGNATOR_DELAY_FRAMES == avenger_ms_to_frames(AVENGER_DESIGNATOR_DELAY_MS)
        && AVENGER_DESIGNATOR_DELAY_FRAMES == 6
        && AVENGER_FAERIE_FIRE_DURATION_MS == 200
        && AVENGER_FAERIE_FIRE_DURATION_FRAMES
            == avenger_ms_to_frames(AVENGER_FAERIE_FIRE_DURATION_MS)
        && AVENGER_FAERIE_FIRE_DURATION_FRAMES == 6
        && AVENGER_DESIGNATOR_DAMAGE_TYPE == "STATUS"
        && AVENGER_DAMAGE_STATUS_TYPE == "FAERIE_FIRE"
        && AVENGER_DESIGNATOR_FIRE_FX == "WeaponFX_AvengerTargetDesignator"
        && AVENGER_PAINT_AUDIO == "AvengerPaintWeaponLoop"
        && (FAERIE_FIRE_ROF_MULTIPLIER - 1.50).abs() < 0.001
        && {
            let d = avenger_designator_weapon();
            d.damage.abs() < 0.001
                && (d.range - 200.0).abs() < 0.01
                && d.can_target_ground
                && d.can_target_air
        }
}

/// Wave 66 residual honesty: air laser residual peel.
pub fn honesty_avenger_air_laser_residual_ok() -> bool {
    AVENGER_AIR_LASER == "AvengerAirLaserOne"
        && AVENGER_AIR_LASER_TWO == "AvengerAirLaserTwo"
        && AVENGER_AIR_LASER_DUMMY == "AvengerAirLaserDummy"
        && (AVENGER_AIR_LASER_DAMAGE - 10.0).abs() < 0.01
        && (AVENGER_AIR_LASER_RANGE - 300.0).abs() < 0.01
        && AVENGER_AIR_LASER_DELAY_MS == 200
        && AVENGER_AIR_LASER_DELAY_FRAMES == avenger_ms_to_frames(AVENGER_AIR_LASER_DELAY_MS)
        && AVENGER_AIR_LASER_DELAY_FRAMES == 6
        && AVENGER_AIR_LASER_DAMAGE_TYPE == "SMALL_ARMS"
        && !AVENGER_AIR_LASER_ANTI_GROUND
        && AVENGER_AIR_LASER_ANTI_AIRBORNE_VEHICLE
        && !AVENGER_AIR_LASER_ANTI_AIRBORNE_INFANTRY
        && AVENGER_LASER_NAME == "AvengerLaserBeam"
        && AVENGER_AIR_LASER_AUDIO == "AvengerAirLaserWeapon"
        && {
            let a = avenger_air_laser_weapon();
            (a.damage - 10.0).abs() < 0.01
                && (a.range - 300.0).abs() < 0.01
                && a.can_target_air
                && !a.can_target_ground
        }
}

/// Wave 66 residual honesty: body / PDL residual peel.
pub fn honesty_avenger_body_residual_ok() -> bool {
    (AVENGER_MAX_HEALTH - 300.0).abs() < 0.01
        && (AVENGER_VISION_RANGE - 150.0).abs() < 0.01
        && (AVENGER_SHROUD_CLEARING_RANGE - 300.0).abs() < 0.01
        && AVENGER_BUILD_COST == 2000
        && (AVENGER_BUILD_TIME_SEC - 10.0).abs() < 0.01
        && AVENGER_BUILD_TIME_FRAMES
            == ((AVENGER_BUILD_TIME_SEC * AVENGER_LOGIC_FPS).round() as u32)
        && AVENGER_BUILD_TIME_FRAMES == 300
        && AVENGER_TRANSPORT_SLOT_COUNT == 3
        && (AVENGER_LOCOMOTOR_SPEED - 30.0).abs() < 0.01
        && (AVENGER_LOCOMOTOR_SPEED_DAMAGED - 20.0).abs() < 0.01
        && (AVENGER_PDL_SCAN_RANGE - 200.0).abs() < 0.01
        && AVENGER_PDL_ONE == "AvengerPointDefenseLaserOne"
        && AVENGER_PDL_TWO == "AvengerPointDefenseLaserTwo"
        && is_avenger_template("AmericaTankAvenger")
        && !is_avenger_template("AmericaTankAvengerLaserTurret")
}

/// Combined Wave 66 Avenger residual honesty pack.
pub fn honesty_avenger_residual_pack_ok() -> bool {
    honesty_avenger_designator_residual_ok()
        && honesty_avenger_air_laser_residual_ok()
        && honesty_avenger_body_residual_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn avenger_template_name_matrix() {
        assert!(is_avenger_template("AmericaTankAvenger"));
        assert!(is_avenger_template("USA_Avenger"));
        assert!(is_avenger_template("TestAvenger"));
        assert!(is_avenger_template("SupW_AmericaVehicleAvenger"));
        assert!(!is_avenger_template("AmericaTankAvengerLaserTurret"));
        assert!(!is_avenger_template("AvengerPointDefenseLaserOne"));
        assert!(!is_avenger_template("AmericaTankPaladin"));
        assert!(!is_avenger_template("AmericaTankCrusader"));
        assert!(!is_avenger_template(""));
    }

    #[test]
    fn paint_and_air_laser_gates() {
        assert!(should_apply_faerie_fire_paint(true, 0, true, true));
        assert!(!should_apply_faerie_fire_paint(true, 1, true, true));
        assert!(!should_apply_faerie_fire_paint(false, 0, true, true));
        assert!(should_apply_avenger_air_laser(true, 1, true, true, true));
        assert!(!should_apply_avenger_air_laser(true, 0, true, true, true));
        assert!(!should_apply_avenger_air_laser(true, 1, false, true, true));
    }

    #[test]
    fn faerie_rof_speeds_reload() {
        let base = 1.0_f32;
        let effective = effective_reload_vs_target(base, true);
        assert!((effective - base / 1.5).abs() < 0.001);
        assert!((effective_reload_vs_target(base, false) - base).abs() < 0.001);
        // With 150% ROF, a shot at t=0 is ready again by t≈0.667.
        assert!(weapon_ready_vs_faerie(0.0, 1.0, 0.67, true));
        assert!(!weapon_ready_vs_faerie(0.0, 1.0, 0.50, true));
        assert!(!weapon_ready_vs_faerie(0.0, 1.0, 0.67, false));
    }

    #[test]
    fn honesty_counters() {
        let mut reg = HostAvengerRegistry::new();
        assert!(!reg.honesty_ok());
        reg.record_paint();
        assert!(reg.honesty_paint_ok());
        reg.record_air_laser_fire();
        assert!(reg.honesty_air_laser_ok());
        reg.record_rof_grant();
        assert!(reg.honesty_rof_ok());
        assert!(reg.honesty_ok());
    }

    #[test]
    fn designator_and_air_laser_weapons() {
        let d = avenger_designator_weapon();
        assert!((d.range - AVENGER_DESIGNATOR_RANGE).abs() < 0.01);
        assert!(d.damage.abs() < 0.001);
        assert!(d.can_target_ground && d.can_target_air);

        let a = avenger_air_laser_weapon();
        assert!((a.damage - AVENGER_AIR_LASER_DAMAGE).abs() < 0.01);
        assert!((a.range - AVENGER_AIR_LASER_RANGE).abs() < 0.01);
        assert!(a.can_target_air && !a.can_target_ground);
    }

    #[test]
    fn avenger_residual_pack_honesty_wave66() {
        assert_eq!(avenger_ms_to_frames(200), 6);
        assert!(honesty_avenger_designator_residual_ok());
        assert!(honesty_avenger_air_laser_residual_ok());
        assert!(honesty_avenger_body_residual_ok());
        assert!(honesty_avenger_residual_pack_ok());
        assert!(!AVENGER_AIR_LASER_ANTI_GROUND);
        assert!(AVENGER_AIR_LASER_ANTI_AIRBORNE_VEHICLE);
        assert_eq!(AVENGER_BUILD_TIME_FRAMES, 300);
        assert_eq!(AVENGER_DAMAGE_STATUS_TYPE, "FAERIE_FIRE");
    }
}
