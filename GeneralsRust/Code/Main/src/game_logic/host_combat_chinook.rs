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
//! - Not multi-door exit paths / ExitStart bone matrix
//! - Not full WeaponSet chooser / model condition icon matrix
//! - Not full passenger contact-weapon exclusion edge cases / nested contain
//! - Not full PointDefenseLaserUpdate velocity prediction (see `host_point_defense` residual)
//! - Host ChinookAI residual covers auto-land, KindOf attack, evac/HeadOffMap,
//!   MoveToBldg combat-drop, MOVE_TO_AND_LAND repair, RappelSpeed, pad search +
//!   supply dump + layer, and idle-only passenger follow. Not leftover dual-world ropes.

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
/// Retail WeaponBonusPassedToPassengers residual (Combat Chinook riders inherit
/// container hero / upgrade / frenzy flags via Weapon::computeBonus).
pub const COMBAT_CHINOOK_WEAPON_BONUS_PASSED_TO_PASSENGERS: bool = true;
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

/// Vanilla / Superweapon Chinook: ChinookAIUpdate + TransportContain, not Combat.
pub fn is_regular_chinook_template(template_name: &str) -> bool {
    let lower = template_name.to_ascii_lowercase();
    !lower.is_empty() && lower.contains("chinook") && !is_combat_chinook_template(template_name)
}

/// C++ `ActionManager::canEnterObject(..., COMBATDROP_INTO)` after common checks.
pub fn combat_drop_into_allowed(
    target_alive: bool,
    target_under_construction: bool,
    target_sold: bool,
    target_name: &str,
    has_contain_module: bool,
    target_is_heal_contain: bool,
    enterer_at_full_health: bool,
    target_is_faction_structure: bool,
) -> bool {
    if !target_alive || target_under_construction || target_sold {
        return false;
    }
    let n = target_name.to_ascii_lowercase();
    if n.contains("prison") || n.contains("powtruck") || n.contains("pow_truck") {
        return false;
    }
    if !has_contain_module {
        return false;
    }
    if target_is_heal_contain && enterer_at_full_health {
        return false;
    }
    if target_is_faction_structure {
        return false;
    }
    true
}

/// C++ `ThingTemplate::getPerUnitFX("CombatDropKillFX")` key (AIStates.cpp:563).
/// Slot name, not an FXList template — do not invent a fallback list.
pub const COMBAT_DROP_KILL_FX_KEY: &str = "CombatDropKillFX";

/// Authored UnitSpecificFX CombatDropKillFX list name for `template_name`.
///
/// C++ `obj->getTemplate()->getPerUnitFX("CombatDropKillFX")`. Missing
/// leftover template or `None` slot stays silent.
pub fn leftover_combat_drop_kill_fx_name(template_name: &str) -> Option<String> {
    leftover_combat_drop_kill_fx_from_factory(template_name)
        .or_else(|| leftover_combat_drop_kill_fx_from_assets(template_name))
}

fn leftover_combat_drop_kill_fx_from_factory(template_name: &str) -> Option<String> {
    let guard = game_engine::common::thing::thing_factory::try_get_thing_factory()?;
    let factory = guard.as_ref()?;
    let tmpl = factory.find_template(template_name, false)?;
    let key = COMBAT_DROP_KILL_FX_KEY.to_string();
    let fx = tmpl.get_per_unit_fx(&key)?;
    nonempty_fx_list_name(fx.name.as_str())
}

fn leftover_combat_drop_kill_fx_from_assets(template_name: &str) -> Option<String> {
    let manager = crate::assets::get_asset_manager()?;
    let manager = manager.lock().ok()?;
    let definition = manager.get_object_definition(template_name)?;
    let key = format!("UnitSpecificFX.{COMBAT_DROP_KILL_FX_KEY}");
    let raw = definition
        .attributes
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        .map(|(_, v)| v.as_str())
        .or_else(|| {
            definition
                .attributes
                .get(COMBAT_DROP_KILL_FX_KEY)
                .map(String::as_str)
        })?;
    nonempty_fx_list_name(raw)
}

fn nonempty_fx_list_name(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name.eq_ignore_ascii_case("None") {
        None
    } else {
        Some(name.to_string())
    }
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
        clip_size: 0,
        clip_reload_time: 0.0,
        // Retail AntiAirborneVehicle = Yes on ListeningOutpostUpgradedDummyWeapon.
        can_target_air: true,
        can_target_ground: true,
        projectile_speed: 0.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        ..Weapon::default()
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
        clip_size: 0,
        clip_reload_time: 0.0,
        // AntiSmallMissile residual honesty — air/missile residual target.
        can_target_air: true,
        can_target_ground: false,
        projectile_speed: 999_999.0,
        pre_attack_delay: 0.0,
        splash_radius: 0.0,
        suspend_fx_frame: 0,
        ..Weapon::default()
    }
}

/// Residual of C++ TransportContain `letRidersUpgradeWeaponSet`
/// (TransportContain.cpp:226-229): only infantry with a non-contact damage
/// weapon count as armed. Vehicles may ride a Combat Chinook
/// (`AllowInsideKindOf = INFANTRY VEHICLE`) but never arm the rider weapon set.
pub fn combat_chinook_rider_has_viable_weapon(
    weapon: Option<&Weapon>,
    is_infantry: bool,
    is_vehicle: bool,
) -> bool {
    let _ = is_vehicle;
    if !gamelogic::object::contain::transport_contain_passenger_kind_allowed_to_fire(is_infantry) {
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

/// C++ `ChinookTakeoffOrLandingState` / `ChinookMoveToBldgState` 3-unit threshold.
pub const HOST_CHINOOK_ARRIVE_THRESH: f32 = 3.0;

/// C++ `while (ai->loseOneBox())` on transport landing / combat drop
/// (`ChinookAIUpdate.cpp:213-216`, `:473-475`).
///
/// Host collectors store cash in `Object.stored_resources.supplies` and crate
/// visuals in `drawable_supply_boxes`. Zeroing only `HostChinookAI.supply_boxes`
/// leaves diverted Chinooks carrying crates.
pub fn lose_all_chinook_object_boxes(obj: &mut crate::game_logic::Object) {
    if let Some(ai) = obj.chinook_ai.as_mut() {
        while ai.lose_one_box() {}
    }
    // C++ loseOneBox decrements m_numberBoxes then updateDrawableSupplyStatus.
    // Host cash + crate visual are the live equivalent of that counter.
    obj.set_stored_supplies(0);
    obj.drawable_supply_boxes = 0;
}

/// True when C++ `ChinookTakeoffOrLandingState` / `ChinookCombatDropState`
/// would have just run `while (loseOneBox())`.
pub fn chinook_flight_dumps_carried_boxes(status: HostChinookFlightStatus) -> bool {
    matches!(
        status,
        HostChinookFlightStatus::Landing | HostChinookFlightStatus::DoingCombatDrop
    )
}

/// C++ `ChinookFlightStatus` residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HostChinookFlightStatus {
    TakingOff = 0,
    #[default]
    Flying = 1,
    DoingCombatDrop = 2,
    Landing = 3,
    Landed = 4,
}

/// C++ `ChinookAIStateType` residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum HostChinookAIState {
    #[default]
    Idle,
    TakingOff,
    Landing,
    MoveToAndLand,
    MoveToAndEvac,
    LandAndEvac,
    EvacAndTakeoff,
    MoveToAndEvacAndExitInit,
    MoveToAndEvacAndExit,
    LandAndEvacAndExit,
    EvacAndExit,
    TakeoffAndExit,
    HeadOffMap,
    MoveToCombatDrop,
    DoCombatDrop,
}

/// C++ `AIFreeToExitType` residual for Chinook load/unload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostChinookFreeToExit {
    FreeToExit,
    WaitToExit,
}

/// Live-host ChinookAIUpdate residual (auto-land, evac, combat-drop, repair, rappel).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostChinookAI {
    pub flight_status: HostChinookFlightStatus,
    pub state: HostChinookAIState,
    pub pos: [f32; 3],
    pub dest: [f32; 3],
    pub original_pos: [f32; 3],
    pub preferred_height: f32,
    pub min_drop_height: f32,
    pub rappel_speed: f32,
    pub supply_boxes: u32,
    pub layer_is_ground: bool,
    pub pad_search_applied: bool,
    pub rider8: bool,
    pub destroyed: bool,
    pub healee: bool,
    pub airfield_id: Option<u32>,
    pub parent_idle: bool,
    pub wanting_enter_or_exit: bool,
    /// KindOf CAN_ATTACK (Combat Chinook). Not OBJECT_STATUS_CAN_ATTACK.
    pub kind_of_can_attack: bool,
    pub passengers_allowed_to_fire: bool,
    pub combat_drop_dest_z: f32,
    pub move_to_bldg_old_preferred: f32,
    pub contained_count: u32,
    pub map_lo: [f32; 2],
    pub map_hi: [f32; 2],
    /// C++ `RopeInfo::nextDropTime` residual (logic frames).
    #[serde(default)]
    pub combat_drop_next_release_frame: u32,
    /// Rappellers released this drop (rope stagger).
    #[serde(default)]
    pub combat_drop_releases: u32,
    /// C++ `privateCombatDrop` goal object (garrison / structure).
    #[serde(default)]
    pub combat_drop_target: Option<u32>,
    /// C++ `m_pendingCommand` evac dest (landed takeoff reconstitution).
    #[serde(default)]
    pub pending_evac_dest: Option<[f32; 3]>,
    #[serde(default)]
    pub pending_evac_and_exit: bool,
    /// In-flight `aiRappelInto` jobs (rappeller, building, dest Y).
    #[serde(default)]
    pub rappel_into_jobs: Vec<HostRappelJob>,
}

/// Live residual of C++ `AIRappelState` goal (roof dest + garrison).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct HostRappelJob {
    pub rappeller: u32,
    pub building: Option<u32>,
    pub dest_y: f32,
}

fn host_chinook_dist_sqr(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    let dz = a[2] - b[2];
    dx * dx + dy * dy + dz * dz
}

impl HostChinookAI {
    /// Combat Chinook residual (KindOf CAN_ATTACK).
    pub fn new_combat(pos: [f32; 3]) -> Self {
        Self::new(pos, true)
    }

    /// Vanilla Chinook residual (no KindOf CAN_ATTACK).
    pub fn new_vanilla(pos: [f32; 3]) -> Self {
        Self::new(pos, false)
    }

    fn new(pos: [f32; 3], kind_of_can_attack: bool) -> Self {
        Self {
            flight_status: HostChinookFlightStatus::Flying,
            state: HostChinookAIState::Idle,
            pos,
            dest: pos,
            original_pos: pos,
            preferred_height: COMBAT_CHINOOK_PREFERRED_HEIGHT,
            min_drop_height: COMBAT_CHINOOK_MIN_DROP_HEIGHT,
            rappel_speed: COMBAT_CHINOOK_RAPPEL_SPEED,
            supply_boxes: COMBAT_CHINOOK_MAX_BOXES,
            layer_is_ground: true,
            pad_search_applied: false,
            rider8: false,
            destroyed: false,
            healee: false,
            airfield_id: None,
            parent_idle: true,
            wanting_enter_or_exit: false,
            kind_of_can_attack,
            combat_drop_next_release_frame: 0,
            combat_drop_releases: 0,
            passengers_allowed_to_fire: kind_of_can_attack,
            combat_drop_dest_z: pos[2],
            move_to_bldg_old_preferred: COMBAT_CHINOOK_PREFERRED_HEIGHT,
            contained_count: 0,
            map_lo: [0.0, 0.0],
            map_hi: [500.0, 500.0],
            combat_drop_target: None,
            pending_evac_dest: None,
            pending_evac_and_exit: false,
            rappel_into_jobs: Vec::new(),
        }
    }

    /// C++ `SupplyTruckAIUpdate::loseOneBox` (ChinookAIUpdate inherits).
    pub fn lose_one_box(&mut self) -> bool {
        if self.supply_boxes == 0 {
            return false;
        }
        self.supply_boxes -= 1;
        true
    }

    /// C++ `while (ai->loseOneBox())` residual box counter.
    fn lose_all_boxes(&mut self) {
        while self.lose_one_box() {}
    }

    /// C++ `getAiFreeToExit`: landed, or combat-drop + `KINDOF_CAN_RAPPEL`.
    pub fn ai_free_to_exit(&self, exiter_can_rappel: bool) -> HostChinookFreeToExit {
        if self.flight_status == HostChinookFlightStatus::Landed
            || (self.flight_status == HostChinookFlightStatus::DoingCombatDrop && exiter_can_rappel)
        {
            HostChinookFreeToExit::FreeToExit
        } else {
            HostChinookFreeToExit::WaitToExit
        }
    }

    /// C++ `isKindOf(KINDOF_CAN_ATTACK)` gate for privateAttack*.
    pub fn can_issue_attack(&self) -> bool {
        self.kind_of_can_attack
    }

    /// C++ passenger follow: only riders with `getCurrentVictim()==NULL`.
    pub fn passenger_should_follow_attack(&self, passenger_has_victim: bool) -> bool {
        self.passengers_allowed_to_fire && !passenger_has_victim
    }

    /// C++ `ChinookCombatDropState` `setDesiredSpeed(m_rappelSpeed)`.
    pub fn apply_rappel_speed(&self) -> f32 {
        self.rappel_speed
    }
    /// C++ `KINDOF_CAN_RAPPEL` residual: host rappellers are infantry.
    pub fn passenger_kind_can_rappel(is_infantry: bool) -> bool {
        is_infantry
    }

    /// Hover arrived: enter `DO_COMBAT_DROP` (C++ ChinookMoveToBldg → CombatDrop).
    pub fn arrive_for_combat_drop(&mut self) {
        if self.state == HostChinookAIState::MoveToCombatDrop {
            self.enter_state(HostChinookAIState::DoCombatDrop);
        }
    }

    /// C++ `now >= nextDropTime` on a rope.
    pub fn can_release_rappeller(&self, now: u32) -> bool {
        self.flight_status == HostChinookFlightStatus::DoingCombatDrop
            && now >= self.combat_drop_next_release_frame
    }

    /// C++ `nextDropTime = now + GameLogicRandomValue(perRopeDelayMin, max)`.
    pub fn on_rappeller_released(&mut self, now: u32) {
        self.combat_drop_releases = self.combat_drop_releases.saturating_add(1);
        self.combat_drop_next_release_frame =
            now.saturating_add(COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_FRAMES);
    }

    /// C++ idle + `hasObjectsWantingToEnterOrExit` + not landed → `LANDING`.
    /// Combat-drop hover must not auto-land (`MOVE_TO_COMBAT_DROP` / `DO_COMBAT_DROP`).
    pub fn tick_idle_auto_land(&mut self) {
        if matches!(
            self.state,
            HostChinookAIState::MoveToCombatDrop | HostChinookAIState::DoCombatDrop
        ) {
            return;
        }
        if !self.parent_idle {
            return;
        }
        if self.wanting_enter_or_exit && self.flight_status != HostChinookFlightStatus::Landed {
            self.enter_state(HostChinookAIState::Landing);
        } else if !self.wanting_enter_or_exit
            && self.flight_status == HostChinookFlightStatus::Landed
            && self.airfield_id.is_none()
        {
            self.enter_state(HostChinookAIState::TakingOff);
        }
    }

    /// C++ `AICMD_MOVE_TO_POSITION_AND_EVACUATE[_AND_EXIT]`.
    pub fn command_evac(&mut self, dest: [f32; 3], and_exit: bool) {
        if host_chinook_dist_sqr(self.pos, dest)
            > HOST_CHINOOK_ARRIVE_THRESH * HOST_CHINOOK_ARRIVE_THRESH
            && self.flight_status == HostChinookFlightStatus::Landed
        {
            // C++ stores `m_pendingCommand` then TAKING_OFF; dest is not dropped.
            self.pending_evac_dest = Some(dest);
            self.pending_evac_and_exit = and_exit;
            self.enter_state(HostChinookAIState::TakingOff);
            return;
        }
        self.dest = dest;
        self.enter_state(if and_exit {
            HostChinookAIState::MoveToAndEvacAndExitInit
        } else {
            HostChinookAIState::MoveToAndEvac
        });
    }

    /// C++ `ChinookCombatDropState` success: clear Held, `CHINOOK_FLYING`, idle.
    pub fn finish_combat_drop(&mut self) {
        if self.state == HostChinookAIState::DoCombatDrop
            || self.flight_status == HostChinookFlightStatus::DoingCombatDrop
        {
            self.succeed();
        }
    }

    /// After a real dump, continue evac-and-exit without re-recording spawn at LZ.
    pub fn begin_takeoff_and_exit(&mut self) {
        self.contained_count = 0;
        self.wanting_enter_or_exit = false;
        self.enter_state(HostChinookAIState::TakeoffAndExit);
    }

    /// C++ `privateGetRepaired` → `MOVE_TO_AND_LAND` (not immediate LANDING).
    pub fn command_repair(&mut self, depot: [f32; 3], depot_id: u32) {
        if matches!(
            self.flight_status,
            HostChinookFlightStatus::Landing | HostChinookFlightStatus::Landed
        ) {
            return;
        }
        self.airfield_id = Some(depot_id);
        self.dest = depot;
        self.enter_state(HostChinookAIState::MoveToAndLand);
    }

    /// C++ `privateCombatDrop` → `MOVE_TO_COMBAT_DROP` (MoveToBldg height).
    pub fn command_combat_drop(&mut self, dest: [f32; 3], building_height: Option<f32>) {
        self.dest = dest;
        self.move_to_bldg_old_preferred = self.preferred_height;
        let mut new_pref = self.preferred_height;
        if let Some(bldg_h) = building_height {
            new_pref = bldg_h + self.min_drop_height;
            if new_pref < self.preferred_height {
                new_pref = self.preferred_height;
            }
        }
        self.combat_drop_dest_z = dest[2] + new_pref;
        self.preferred_height = new_pref;
        self.enter_state(HostChinookAIState::MoveToCombatDrop);
    }

    fn enter_state(&mut self, state: HostChinookAIState) {
        self.state = state;
        match state {
            HostChinookAIState::Idle => {}
            HostChinookAIState::TakingOff | HostChinookAIState::TakeoffAndExit => {
                self.enter_takeoff_or_landing(false);
            }
            HostChinookAIState::Landing
            | HostChinookAIState::LandAndEvac
            | HostChinookAIState::LandAndEvacAndExit => {
                self.enter_takeoff_or_landing(true);
            }
            HostChinookAIState::MoveToAndLand
            | HostChinookAIState::MoveToAndEvac
            | HostChinookAIState::MoveToAndEvacAndExit => {}
            HostChinookAIState::MoveToAndEvacAndExitInit => {
                self.original_pos = self.pos;
                self.enter_state(HostChinookAIState::MoveToAndEvacAndExit);
            }
            HostChinookAIState::EvacAndTakeoff | HostChinookAIState::EvacAndExit => {
                self.contained_count = 0;
                self.enter_state(if state == HostChinookAIState::EvacAndTakeoff {
                    HostChinookAIState::TakingOff
                } else {
                    HostChinookAIState::TakeoffAndExit
                });
            }
            HostChinookAIState::HeadOffMap => {
                self.rider8 = true;
                self.dest = self.original_pos;
            }
            HostChinookAIState::MoveToCombatDrop => {}
            HostChinookAIState::DoCombatDrop => {
                self.flight_status = HostChinookFlightStatus::DoingCombatDrop;
                self.preferred_height = self.move_to_bldg_old_preferred;
                // C++ ChinookCombatDropState::onEnter while(loseOneBox()).
                // Object stored_resources / drawable crates: lose_all_chinook_object_boxes.
                self.lose_all_boxes();
                self.combat_drop_next_release_frame = 0;
                self.combat_drop_releases = 0;
            }
        }
    }

    /// C++ `ChinookTakeoffOrLandingState::onEnter`: dump boxes, pad search, layer.
    fn enter_takeoff_or_landing(&mut self, landing: bool) {
        self.flight_status = if landing {
            HostChinookFlightStatus::Landing
        } else {
            HostChinookFlightStatus::TakingOff
        };
        if landing {
            // C++ ChinookTakeoffOrLandingState::onEnter while(loseOneBox()).
            // Object stored_resources / drawable crates: lose_all_chinook_object_boxes.
            self.lose_all_boxes();
            self.pad_search_applied = true;
            self.dest = [self.pos[0], self.pos[1], 0.0];
        } else {
            self.dest = [
                self.pos[0],
                self.pos[1],
                self.pos[2].max(0.0) + self.preferred_height,
            ];
            self.layer_is_ground = true;
        }
    }

    fn arrived_3d(&self, dest: [f32; 3]) -> bool {
        host_chinook_dist_sqr(self.pos, dest)
            <= HOST_CHINOOK_ARRIVE_THRESH * HOST_CHINOOK_ARRIVE_THRESH
    }

    fn arrived_2d(&self, dest: [f32; 3]) -> bool {
        let dx = self.pos[0] - dest[0];
        let dy = self.pos[1] - dest[1];
        dx * dx + dy * dy <= HOST_CHINOOK_ARRIVE_THRESH * HOST_CHINOOK_ARRIVE_THRESH
    }

    fn step_toward(&mut self, dest: [f32; 3], step: f32) {
        let dx = dest[0] - self.pos[0];
        let dy = dest[1] - self.pos[1];
        let dz = dest[2] - self.pos[2];
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        if len <= step || len < 0.0001 {
            self.pos = dest;
            return;
        }
        self.pos = [
            self.pos[0] + dx / len * step,
            self.pos[1] + dy / len * step,
            self.pos[2] + dz / len * step,
        ];
    }

    fn succeed(&mut self) {
        let next = match self.state {
            HostChinookAIState::TakingOff => {
                self.flight_status = HostChinookFlightStatus::Flying;
                HostChinookAIState::Idle
            }
            HostChinookAIState::Landing => {
                self.flight_status = HostChinookFlightStatus::Landed;
                HostChinookAIState::Idle
            }
            HostChinookAIState::MoveToAndLand => HostChinookAIState::Landing,
            HostChinookAIState::MoveToAndEvac => HostChinookAIState::LandAndEvac,
            HostChinookAIState::LandAndEvac => {
                self.flight_status = HostChinookFlightStatus::Landed;
                HostChinookAIState::EvacAndTakeoff
            }
            HostChinookAIState::EvacAndTakeoff => HostChinookAIState::TakingOff,
            HostChinookAIState::MoveToAndEvacAndExit => HostChinookAIState::LandAndEvacAndExit,
            HostChinookAIState::LandAndEvacAndExit => {
                self.flight_status = HostChinookFlightStatus::Landed;
                HostChinookAIState::EvacAndExit
            }
            HostChinookAIState::EvacAndExit => HostChinookAIState::TakeoffAndExit,
            HostChinookAIState::TakeoffAndExit => {
                self.flight_status = HostChinookFlightStatus::Flying;
                HostChinookAIState::HeadOffMap
            }
            HostChinookAIState::HeadOffMap => {
                self.rider8 = false;
                HostChinookAIState::Idle
            }
            HostChinookAIState::MoveToCombatDrop => HostChinookAIState::DoCombatDrop,
            HostChinookAIState::DoCombatDrop => {
                self.flight_status = HostChinookFlightStatus::Flying;
                HostChinookAIState::Idle
            }
            HostChinookAIState::MoveToAndEvacAndExitInit | HostChinookAIState::Idle => {
                HostChinookAIState::Idle
            }
        };
        if next != self.state {
            self.enter_state(next);
        } else {
            self.state = HostChinookAIState::Idle;
        }
        // C++ `update`: parent idle reconstitutes `m_pendingCommand`.
        if self.state == HostChinookAIState::Idle {
            if let Some(dest) = self.pending_evac_dest.take() {
                let and_exit = self.pending_evac_and_exit;
                self.pending_evac_and_exit = false;
                self.command_evac(dest, and_exit);
            }
        }
    }

    /// Advance leftover-equivalent flight residual one step.
    pub fn tick(&mut self, step: f32) {
        if self.destroyed {
            return;
        }
        self.tick_idle_auto_land();
        if self.flight_status == HostChinookFlightStatus::Landed && self.airfield_id.is_some() {
            self.healee = true;
        }
        match self.state {
            HostChinookAIState::Idle | HostChinookAIState::DoCombatDrop => {}
            HostChinookAIState::TakingOff
            | HostChinookAIState::Landing
            | HostChinookAIState::LandAndEvac
            | HostChinookAIState::LandAndEvacAndExit
            | HostChinookAIState::TakeoffAndExit => {
                self.step_toward(self.dest, step);
                if self.arrived_3d(self.dest) {
                    self.succeed();
                }
            }
            HostChinookAIState::MoveToAndLand
            | HostChinookAIState::MoveToAndEvac
            | HostChinookAIState::MoveToAndEvacAndExit => {
                self.step_toward(self.dest, step);
                if self.arrived_2d(self.dest) {
                    self.succeed();
                }
            }
            HostChinookAIState::MoveToCombatDrop => {
                let hover = [self.dest[0], self.dest[1], self.combat_drop_dest_z];
                self.step_toward(hover, step);
                if self.arrived_2d(hover)
                    && (self.pos[2] - self.combat_drop_dest_z).abs() <= HOST_CHINOOK_ARRIVE_THRESH
                {
                    self.succeed();
                }
            }
            HostChinookAIState::HeadOffMap => {
                // C++ ChinookHeadOffMapState::update (ChinookAIUpdate.cpp:152-162)
                // destroys the owner the moment its CURRENT position leaves
                // getExtentIncludingBorder — the bounds check observes the
                // pre-move position each frame; an already-outside owner never
                // gets another locomotor step back toward the map.
                if self.pos[0] < self.map_lo[0]
                    || self.pos[0] > self.map_hi[0]
                    || self.pos[1] < self.map_lo[1]
                    || self.pos[1] > self.map_hi[1]
                {
                    self.destroyed = true;
                    self.succeed();
                } else {
                    self.step_toward(self.dest, step);
                }
            }
            HostChinookAIState::EvacAndTakeoff
            | HostChinookAIState::EvacAndExit
            | HostChinookAIState::MoveToAndEvacAndExitInit => {}
        }
    }
}

/// Live residual honesty: auto-land + KindOf + evac + combat-drop + repair + rappel + follow.
pub fn honesty_host_chinook_ai_cpp_residual_ok() -> bool {
    let mut flying = HostChinookAI::new_combat([10.0, 10.0, 100.0]);
    flying.wanting_enter_or_exit = true;
    flying.contained_count = 1;
    flying.tick(0.0);
    let auto_lands = flying.state == HostChinookAIState::Landing
        && flying.flight_status == HostChinookFlightStatus::Landing
        && flying.ai_free_to_exit(false) == HostChinookFreeToExit::WaitToExit
        && flying.supply_boxes == 0
        && flying.pad_search_applied;

    let combat = HostChinookAI::new_combat([0.0, 0.0, 0.0]);
    let vanilla = HostChinookAI::new_vanilla([0.0, 0.0, 0.0]);
    let kind_of = combat.can_issue_attack() && !vanilla.can_issue_attack();

    let mut evac = HostChinookAI::new_combat([0.0, 0.0, 80.0]);
    evac.contained_count = 2;
    evac.command_evac([40.0, 0.0, 0.0], true);
    let evac_starts = evac.state == HostChinookAIState::MoveToAndEvacAndExit;
    evac.pos = [40.0, 0.0, 80.0];
    evac.tick(200.0);
    // After 2D arrive → land → dump → takeoff → HeadOffMap.
    let mut guard = 0u32;
    while evac.state != HostChinookAIState::HeadOffMap && !evac.destroyed && guard < 32 {
        evac.tick(200.0);
        guard += 1;
    }
    evac.pos = [-10.0, 0.0, 80.0];
    evac.tick(1.0);
    let evac_ok = evac_starts && evac.contained_count == 0 && evac.destroyed;

    let mut drop = HostChinookAI::new_combat([0.0, 0.0, 100.0]);
    drop.command_combat_drop([20.0, 0.0, 0.0], Some(80.0));
    let drop_moves = drop.state == HostChinookAIState::MoveToCombatDrop
        && drop.flight_status != HostChinookFlightStatus::DoingCombatDrop
        && (drop.combat_drop_dest_z - 120.0).abs() < 0.01;
    drop.pos = [20.0, 0.0, 120.0];
    drop.tick(1.0);
    let drop_ok = drop_moves
        && drop.state == HostChinookAIState::DoCombatDrop
        && drop.flight_status == HostChinookFlightStatus::DoingCombatDrop
        && (drop.apply_rappel_speed() - COMBAT_CHINOOK_RAPPEL_SPEED).abs() < 0.01;
    let mut repair = HostChinookAI::new_combat([0.0, 0.0, 5.0]);
    repair.command_repair([80.0, 0.0, 0.0], 7);
    let repair_moves = repair.state == HostChinookAIState::MoveToAndLand
        && repair.flight_status != HostChinookFlightStatus::Landing
        && repair.flight_status != HostChinookFlightStatus::Landed;
    repair.pos = [80.0, 0.0, 5.0];
    repair.tick(1.0);
    let repair_lands = repair.state == HostChinookAIState::Landing;
    repair.pos = [80.0, 0.0, 0.0];
    repair.tick(1.0);
    repair.tick(1.0);
    let repair_ok = repair_moves
        && repair_lands
        && repair.flight_status == HostChinookFlightStatus::Landed
        && repair.healee;
    let follow = combat.passenger_should_follow_attack(false)
        && !combat.passenger_should_follow_attack(true);

    auto_lands && kind_of && evac_ok && drop_ok && repair_ok && follow
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
        assert!(is_regular_chinook_template("AmericaVehicleChinook"));
        assert!(is_regular_chinook_template("USA_Chinook"));
        assert!(!is_regular_chinook_template("AirF_AmericaVehicleChinook"));
        assert!(combat_drop_into_allowed(
            true,
            false,
            false,
            "AmericaCivilianBunker",
            true,
            false,
            false,
            false
        ));
        assert!(!combat_drop_into_allowed(
            true,
            false,
            false,
            "AmericaCommandCenter",
            true,
            false,
            false,
            true
        ));
        assert!(!combat_drop_into_allowed(
            true, false, false, "Tree", false, false, false, false
        ));
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
    fn armed_rider_allows_infantry_not_vehicle() {
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
        assert!(
            !combat_chinook_rider_has_viable_weapon(Some(&rifle), false, true),
            "C++ letRidersUpgradeWeaponSet skips non-infantry riders"
        );
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

    #[test]
    fn host_chinook_ai_auto_lands_for_load_unload() {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 100.0]);
        ai.wanting_enter_or_exit = true;
        assert_eq!(ai.ai_free_to_exit(false), HostChinookFreeToExit::WaitToExit);
        ai.tick_idle_auto_land();
        assert_eq!(ai.flight_status, HostChinookFlightStatus::Landing);
        assert!(ai.pad_search_applied);
        assert_eq!(ai.supply_boxes, 0);
        ai.pos = [0.0, 0.0, 0.0];
        ai.dest = [0.0, 0.0, 0.0];
        ai.tick(1.0);
        assert_eq!(ai.flight_status, HostChinookFlightStatus::Landed);
        assert_eq!(ai.ai_free_to_exit(false), HostChinookFreeToExit::FreeToExit);
    }

    #[test]
    fn host_chinook_attack_uses_kind_of_not_status() {
        assert!(HostChinookAI::new_combat([0.0, 0.0, 0.0]).can_issue_attack());
        assert!(!HostChinookAI::new_vanilla([0.0, 0.0, 0.0]).can_issue_attack());
    }

    #[test]
    fn host_chinook_evac_lands_dumps_takeoff_and_heads_off_map() {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 80.0]);
        ai.contained_count = 3;
        ai.command_evac([30.0, 0.0, 0.0], true);
        assert_eq!(ai.state, HostChinookAIState::MoveToAndEvacAndExit);
        ai.pos = [30.0, 0.0, 80.0];
        let mut guard = 0u32;
        while ai.state != HostChinookAIState::HeadOffMap && !ai.destroyed && guard < 32 {
            ai.tick(200.0);
            guard += 1;
        }
        assert_eq!(ai.contained_count, 0);
        assert_eq!(ai.state, HostChinookAIState::HeadOffMap);
        assert!(ai.rider8);
        ai.pos = [-1.0, 0.0, 80.0];
        ai.tick(1.0);
        assert!(ai.destroyed);
    }

    #[test]
    fn host_chinook_combat_drop_waits_for_move_to_bldg_height() {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 100.0]);
        ai.command_combat_drop([15.0, 0.0, 0.0], Some(20.0));
        assert_eq!(ai.state, HostChinookAIState::MoveToCombatDrop);
        assert_ne!(ai.flight_status, HostChinookFlightStatus::DoingCombatDrop);
        assert!((ai.combat_drop_dest_z - 100.0).abs() < 0.01 || ai.combat_drop_dest_z >= 60.0);
        ai.pos = [15.0, 0.0, ai.combat_drop_dest_z];
        ai.tick(1.0);
        assert_eq!(ai.state, HostChinookAIState::DoCombatDrop);
        assert_eq!(ai.flight_status, HostChinookFlightStatus::DoingCombatDrop);
        assert!((ai.apply_rappel_speed() - 30.0).abs() < 0.01);
    }

    #[test]
    fn host_chinook_repair_moves_to_pad_before_landing() {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 8.0]);
        ai.command_repair([90.0, 0.0, 0.0], 4);
        assert_eq!(ai.state, HostChinookAIState::MoveToAndLand);
        assert_ne!(ai.flight_status, HostChinookFlightStatus::Landing);
        ai.pos = [90.0, 0.0, 8.0];
        ai.tick(1.0);
        assert_eq!(ai.state, HostChinookAIState::Landing);
        ai.pos = [90.0, 0.0, 0.0];
        ai.tick(1.0);
        assert_eq!(ai.flight_status, HostChinookFlightStatus::Landed);
        ai.tick(1.0);
        assert!(ai.healee);
    }

    #[test]
    fn host_chinook_passenger_follow_is_idle_only() {
        let ai = HostChinookAI::new_combat([0.0, 0.0, 0.0]);
        assert!(ai.passenger_should_follow_attack(false));
        assert!(!ai.passenger_should_follow_attack(true));
    }

    #[test]
    fn combat_drop_free_to_exit_only_when_can_rappel() {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 100.0]);
        ai.command_combat_drop([10.0, 0.0, 0.0], None);
        ai.arrive_for_combat_drop();
        assert_eq!(ai.flight_status, HostChinookFlightStatus::DoingCombatDrop);
        assert_eq!(ai.ai_free_to_exit(false), HostChinookFreeToExit::WaitToExit);
        assert_eq!(ai.ai_free_to_exit(true), HostChinookFreeToExit::FreeToExit);
        assert!(HostChinookAI::passenger_kind_can_rappel(true));
        assert!(!HostChinookAI::passenger_kind_can_rappel(false));
        ai.wanting_enter_or_exit = true;
        ai.parent_idle = true;
        ai.tick_idle_auto_land();
        assert_eq!(ai.flight_status, HostChinookFlightStatus::DoingCombatDrop);
        ai.on_rappeller_released(0);
        assert!(!ai.can_release_rappeller(1));
        assert!(ai.can_release_rappeller(COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_FRAMES));
    }

    #[test]
    fn host_chinook_ai_cpp_residual_honesty_pack() {
        assert!(honesty_host_chinook_ai_cpp_residual_ok());
    }

    /// C++ ChinookAIUpdate.cpp:1067-1087 live host: idle + want enter/exit auto-lands.
    #[test]
    fn live_host_chinook_auto_lands_when_idle_and_wanting() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("AirF_AmericaVehicleChinook");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Attackable);
        tpl.set_health(350.0);
        logic
            .templates
            .insert("AirF_AmericaVehicleChinook".into(), tpl);
        let id = logic
            .create_object(
                "AirF_AmericaVehicleChinook",
                Team::USA,
                Vec3::new(0.0, 100.0, 0.0),
            )
            .expect("chinook");
        {
            let obj = logic.host_object_mut(id).expect("obj");
            if !obj.is_combat_chinook_style_container() {
                obj.install_combat_chinook_transport();
            }
            let ai = obj.chinook_ai.as_ref().expect("chinook_ai installed");
            assert_eq!(ai.flight_status, HostChinookFlightStatus::Flying);
            assert_eq!(ai.ai_free_to_exit(false), HostChinookFreeToExit::WaitToExit);
            obj.pending_evacuate_on_stop = true;
        }
        // One logic frame (C++ ChinookAIUpdate::update runs per 1/30s tick);
        // 150 u/s speed moves 5u per tick, so an 80u descent stays Landing.
        logic.tick_chinook_ai(1.0 / 30.0);
        let obj = logic.host_object(id).expect("obj");
        let ai = obj.chinook_ai.as_ref().expect("chinook_ai");
        assert_eq!(ai.flight_status, HostChinookFlightStatus::Landing);
        assert!(obj.precise_z_pos, "Chinook landing PRECISE_Z_POS");
        assert!(obj.ultra_accurate, "Chinook landing ULTRA_ACCURATE");
        assert_eq!(ai.ai_free_to_exit(false), HostChinookFreeToExit::WaitToExit);
    }

    /// hq-0xpfm: two chinooks landing at the same XY must not share one LZ.
    #[test]
    fn live_host_chinooks_unstack_landing_dest() {
        use crate::game_logic::{GameLogic, KindOf, ObjectType, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("AmericaVehicleChinook");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Aircraft);
        tpl.set_health(350.0);
        logic.templates.insert("AmericaVehicleChinook".into(), tpl);
        let pos = Vec3::new(0.0, 80.0, 0.0);
        let a = logic
            .create_object("AmericaVehicleChinook", Team::USA, pos)
            .expect("chinook a");
        let b = logic
            .create_object("AmericaVehicleChinook", Team::USA, pos)
            .expect("chinook b");
        for id in [a, b] {
            let obj = logic.host_object_mut(id).expect("obj");
            obj.install_chinook_transport();
            obj.object_type = ObjectType::Aircraft;
            obj.loco_appearance = crate::game_logic::LocomotorAppearance::Hover;
            obj.status.airborne_target = true;
            obj.pending_evacuate_on_stop = true;
        }
        logic.tick_chinook_ai(1.0 / 30.0);
        let da = logic
            .host_object(a)
            .and_then(|o| o.chinook_ai.as_ref())
            .expect("ai a")
            .dest;
        let db = logic
            .host_object(b)
            .and_then(|o| o.chinook_ai.as_ref())
            .expect("ai b")
            .dest;
        assert_eq!(
            logic
                .host_object(a)
                .and_then(|o| o.chinook_ai.as_ref())
                .map(|ai| ai.flight_status),
            Some(HostChinookFlightStatus::Landing)
        );
        let stacked = (da[0] - db[0]).abs() < 1.0 && (da[1] - db[1]).abs() < 1.0;
        assert!(
            !stacked,
            "chinooks must not share one LZ da={da:?} db={db:?}"
        );
    }
    #[test]
    fn landed_evac_keeps_dest_across_takeoff() {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 0.0]);
        ai.flight_status = HostChinookFlightStatus::Landed;
        ai.command_evac([80.0, 0.0, 0.0], false);
        assert_eq!(ai.state, HostChinookAIState::TakingOff);
        assert_eq!(ai.pending_evac_dest, Some([80.0, 0.0, 0.0]));
        ai.pos = ai.dest;
        ai.tick(1.0);
        assert_eq!(ai.state, HostChinookAIState::MoveToAndEvac);
        assert!((ai.dest[0] - 80.0).abs() < 0.01);
        assert!(ai.pending_evac_dest.is_none());
    }

    #[test]
    fn finish_combat_drop_leaves_doing_combat_drop() {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 100.0]);
        ai.command_combat_drop([10.0, 0.0, 0.0], None);
        ai.arrive_for_combat_drop();
        assert_eq!(ai.flight_status, HostChinookFlightStatus::DoingCombatDrop);
        ai.finish_combat_drop();
        assert_eq!(ai.flight_status, HostChinookFlightStatus::Flying);
        assert_eq!(ai.state, HostChinookAIState::Idle);
    }

    #[test]
    fn headoffmap_uses_recorded_spawn_not_lz() {
        let spawn = [-40.0, 0.0, 80.0];
        let mut ai = HostChinookAI::new_combat(spawn);
        ai.command_evac([40.0, 0.0, 0.0], true);
        assert_eq!(ai.original_pos, spawn);
        ai.pos = [40.0, 0.0, 80.0];
        ai.begin_takeoff_and_exit();
        ai.pos = ai.dest;
        ai.tick(1.0);
        assert_eq!(ai.state, HostChinookAIState::HeadOffMap);
        assert_eq!(ai.dest, spawn);
    }

    /// hq-o3j2d: landing / combat-drop dump Object crates, not just residual counter.
    #[test]
    fn diverted_chinook_drops_object_crates() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        use glam::Vec3;

        let mut logic = GameLogic::new();
        let mut tpl = ThingTemplate::new("AirF_AmericaVehicleChinook");
        tpl.add_kind_of(KindOf::Vehicle);
        tpl.add_kind_of(KindOf::Aircraft);
        tpl.set_health(350.0);
        tpl.supply_truck_metadata = Some(crate::game_logic::SupplyTruckMetadata {
            max_boxes: 8,
            warehouse_scan_distance: 700.0,
            warehouse_delay_frames: 0,
            center_delay_frames: 0,
            upgraded_supply_boost: 60,
        });
        logic
            .templates
            .insert("AirF_AmericaVehicleChinook".into(), tpl);
        let id = logic
            .create_object(
                "AirF_AmericaVehicleChinook",
                Team::USA,
                Vec3::new(0.0, 100.0, 0.0),
            )
            .expect("chinook");
        {
            let obj = logic.host_object_mut(id).expect("obj");
            if !obj.is_combat_chinook_style_container() {
                obj.install_combat_chinook_transport();
            }
            obj.set_stored_supplies(8 * 75);
            obj.drawable_supply_boxes = 8;
            obj.pending_evacuate_on_stop = true;
        }
        assert_eq!(
            logic.host_object(id).unwrap().stored_resources.supplies,
            600
        );
        assert_eq!(logic.host_object(id).unwrap().drawable_supply_boxes, 8);
        logic.tick_chinook_ai(1.0 / 30.0);
        let obj = logic.host_object(id).expect("obj");
        assert_eq!(
            obj.chinook_ai.as_ref().unwrap().flight_status,
            HostChinookFlightStatus::Landing
        );
        assert_eq!(
            obj.stored_resources.supplies, 0,
            "diverted landing must dump cash crates"
        );
        assert_eq!(
            obj.drawable_supply_boxes, 0,
            "diverted landing must dump crate visuals"
        );
        assert_eq!(obj.chinook_ai.as_ref().unwrap().supply_boxes, 0);

        {
            let obj = logic.host_object_mut(id).expect("obj");
            obj.set_stored_supplies(4 * 75);
            obj.drawable_supply_boxes = 4;
            if let Some(ai) = obj.chinook_ai.as_mut() {
                ai.flight_status = HostChinookFlightStatus::Flying;
                ai.state = HostChinookAIState::MoveToCombatDrop;
                ai.supply_boxes = 4;
                ai.arrive_for_combat_drop();
            }
        }
        logic.tick_chinook_ai(1.0);
        let obj = logic.host_object(id).expect("obj");
        assert_eq!(
            obj.chinook_ai.as_ref().unwrap().flight_status,
            HostChinookFlightStatus::DoingCombatDrop
        );
        assert_eq!(
            obj.stored_resources.supplies, 0,
            "combat drop must dump cash crates"
        );
        assert_eq!(
            obj.drawable_supply_boxes, 0,
            "combat drop must dump crate visuals"
        );
    }

    #[test]
    fn lose_one_box_decrements_until_empty() {
        let mut ai = HostChinookAI::new_combat([0.0, 0.0, 100.0]);
        ai.supply_boxes = 3;
        assert!(ai.lose_one_box());
        assert_eq!(ai.supply_boxes, 2);
        while ai.lose_one_box() {}
        assert_eq!(ai.supply_boxes, 0);
        assert!(!ai.lose_one_box());
    }
}
