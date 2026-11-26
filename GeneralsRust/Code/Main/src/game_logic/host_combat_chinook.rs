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
use generals_sim::chinook::{
    DEFAULT_MIN_DROP_HEIGHT, DEFAULT_PREFERRED_HEIGHT, DEFAULT_RAPPEL_SPEED,
    DEFAULT_ROPE_RELEASE_DELAY_FRAMES,
};
#[cfg(test)]
use generals_sim::chinook::{ChinookAi, ChinookAiState, ChinookFlightStatus, ChinookFreeToExit};
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

// --- ChinookAIUpdate residual ---

/// Retail MaxBoxes residual.
pub const COMBAT_CHINOOK_MAX_BOXES: u32 = 8;
/// Retail NumRopes residual.
pub const COMBAT_CHINOOK_NUM_ROPES: u32 = 4;
/// Retail PerRopeDelayMin residual (msec).
pub const COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_MS: u32 = 900;
/// Retail PerRopeDelayMax residual (msec).
pub const COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_MS: u32 = 1_500;
/// PerRopeDelayMax 1500ms → 45 frames @ 30 FPS.
pub const COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_FRAMES: u32 = 45;
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

/// Authored AirF Combat Chinook: a Chinook hull whose TransportContain has
/// `PassengersAllowedToFire = Yes` (retail `AirF_AmericaVehicleChinook`).
/// Vanilla `AmericaVehicleChinook` TransportContain leaves it No. Keys on the
/// parsed contain module, not the template name (hq-2qedt.269).
pub fn is_authored_combat_chinook(template: &crate::game_logic::ThingTemplate) -> bool {
    let contain = &template.contain_module;
    is_chinook_hull_template(&template.name)
        && contain.kind == crate::game_logic::ContainModuleKind::Transport
        && contain.passengers_allowed_to_fire
}

/// Chinook hull family (vanilla, Combat, Superweapon): name residual for the
/// ChinookAI install; Combat vs vanilla is split by `is_authored_combat_chinook`.
pub fn is_chinook_hull_template(template_name: &str) -> bool {
    template_name.to_ascii_lowercase().contains("chinook")
}

/// Test fixture: author retail Combat Chinook TransportContain on a template.
#[cfg(test)]
pub(crate) fn author_combat_chinook_contain(template: &mut crate::game_logic::ThingTemplate) {
    template.contain_module.kind =
        crate::game_logic::ContainModuleKind::Transport;
    template.contain_module.slots = Some(COMBAT_CHINOOK_TRANSPORT_SLOTS);
    template.contain_module.passengers_allowed_to_fire = true;
    // Retail AllowInsideKindOf = INFANTRY VEHICLE, ForbidInsideKindOf = AIRCRAFT HUGE_VEHICLE.
    template.contain_module.admission = crate::game_logic::ContainAdmission::InfantryOrVehicle;
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


/// Wave 58 residual honesty: ChinookAIUpdate / body residual.
pub fn honesty_combat_chinook_ai_body_residual_ok() -> bool {
    COMBAT_CHINOOK_MAX_BOXES == 8
        && COMBAT_CHINOOK_NUM_ROPES == 4
        && COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_MS == 900
        && DEFAULT_ROPE_RELEASE_DELAY_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_PER_ROPE_DELAY_MIN_MS)
        && COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_MS == 1_500
        && COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_FRAMES
            == combat_chinook_ms_to_frames(COMBAT_CHINOOK_PER_ROPE_DELAY_MAX_MS)
        && (DEFAULT_RAPPEL_SPEED - 30.0).abs() < 0.01
        && (DEFAULT_MIN_DROP_HEIGHT - 40.0).abs() < 0.01
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
        && (DEFAULT_PREFERRED_HEIGHT - 100.0).abs() < 0.01
        && COMBAT_CHINOOK_TRANSPORT_SLOT_COUNT == 0
}

/// Combined Wave 58 Combat Chinook residual honesty pack.
pub fn honesty_combat_chinook_residual_pack_ok() -> bool {
    honesty_combat_chinook_transport_residual_ok()
        && honesty_combat_chinook_minigun_dummy_residual_ok()
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
        let mut ai = ChinookAi::new_combat([0.0, 0.0, 100.0]);
        ai.wanting_enter_or_exit = true;
        assert_eq!(ai.ai_free_to_exit(false), ChinookFreeToExit::WaitToExit);
        ai.tick_idle_auto_land();
        assert_eq!(ai.flight_status, ChinookFlightStatus::Landing);
        assert!(ai.pad_search_applied);
        ai.pos = [0.0, 0.0, 0.0];
        ai.dest = [0.0, 0.0, 0.0];
        assert!(ai.update_from_observed_pose().is_none());
        assert_eq!(ai.flight_status, ChinookFlightStatus::Landed);
        assert_eq!(ai.ai_free_to_exit(false), ChinookFreeToExit::FreeToExit);
    }

    #[test]
    fn host_chinook_attack_uses_kind_of_not_status() {
        assert!(ChinookAi::new_combat([0.0, 0.0, 0.0]).can_issue_attack());
        assert!(!ChinookAi::new_vanilla([0.0, 0.0, 0.0]).can_issue_attack());
    }

    #[test]
    fn host_chinook_evac_lands_dumps_takeoff_and_heads_off_map() {
        let mut ai = ChinookAi::new_combat([0.0, 0.0, 80.0]);
        ai.contained_count = 3;
        ai.command_evac([30.0, 0.0, 0.0], true);
        assert_eq!(ai.state, ChinookAiState::MoveToAndEvacAndExit);
        ai.pos = [30.0, 0.0, 80.0];
        let mut guard = 0u32;
        while ai.state != ChinookAiState::HeadOffMap && !ai.destroyed && guard < 32 {
            ai.pos = ai.dest;
            if let Some(effect) = ai.update_from_observed_pose() {
                // This fixture models a successful contain-domain operation.
                assert!(ai.complete_evacuation(effect, 0));
            }
            guard += 1;
        }
        assert_eq!(ai.contained_count, 0);
        assert_eq!(ai.state, ChinookAiState::HeadOffMap);
        assert!(ai.rider8);
        ai.pos = [-1.0, 0.0, 80.0];
        assert!(ai.update_from_observed_pose().is_none());
        assert!(ai.destroyed);
    }

    #[test]
    fn host_chinook_combat_drop_waits_for_move_to_bldg_height() {
        let mut ai = ChinookAi::new_combat([0.0, 0.0, 100.0]);
        ai.command_combat_drop([15.0, 0.0, 0.0], Some(20.0));
        assert_eq!(ai.state, ChinookAiState::MoveToCombatDrop);
        assert_ne!(ai.flight_status, ChinookFlightStatus::DoingCombatDrop);
        assert!((ai.combat_drop_dest_z - 100.0).abs() < 0.01 || ai.combat_drop_dest_z >= 60.0);
        ai.pos = [15.0, 0.0, ai.combat_drop_dest_z];
        assert!(ai.update_from_observed_pose().is_none());
        assert_eq!(ai.state, ChinookAiState::DoCombatDrop);
        assert_eq!(ai.flight_status, ChinookFlightStatus::DoingCombatDrop);
        assert!((ai.apply_rappel_speed() - 30.0).abs() < 0.01);
    }

    #[test]
    fn host_chinook_repair_moves_to_pad_before_landing() {
        let mut ai = ChinookAi::new_combat([0.0, 0.0, 8.0]);
        ai.command_repair([90.0, 0.0, 0.0], 4);
        assert_eq!(ai.state, ChinookAiState::MoveToAndLand);
        assert_ne!(ai.flight_status, ChinookFlightStatus::Landing);
        ai.pos = [90.0, 0.0, 8.0];
        assert!(ai.update_from_observed_pose().is_none());
        assert_eq!(ai.state, ChinookAiState::Landing);
        ai.pos = [90.0, 0.0, 0.0];
        assert!(ai.update_from_observed_pose().is_none());
        assert_eq!(ai.flight_status, ChinookFlightStatus::Landed);
        assert!(ai.update_from_observed_pose().is_none());
        assert!(ai.healee);
    }

    #[test]
    fn host_chinook_passenger_follow_is_idle_only() {
        let ai = ChinookAi::new_combat([0.0, 0.0, 0.0]);
        assert!(ai.passenger_should_follow_attack(false));
        assert!(!ai.passenger_should_follow_attack(true));
    }

    #[test]
    fn combat_drop_free_to_exit_only_when_can_rappel() {
        let mut ai = ChinookAi::new_combat([0.0, 0.0, 100.0]);
        ai.command_combat_drop([10.0, 0.0, 0.0], None);
        ai.arrive_for_combat_drop();
        assert_eq!(ai.flight_status, ChinookFlightStatus::DoingCombatDrop);
        assert_eq!(ai.ai_free_to_exit(false), ChinookFreeToExit::WaitToExit);
        assert_eq!(ai.ai_free_to_exit(true), ChinookFreeToExit::FreeToExit);
        assert!(ChinookAi::passenger_kind_can_rappel(true));
        assert!(!ChinookAi::passenger_kind_can_rappel(false));
        ai.wanting_enter_or_exit = true;
        ai.parent_idle = true;
        ai.tick_idle_auto_land();
        assert_eq!(ai.flight_status, ChinookFlightStatus::DoingCombatDrop);
        ai.on_rappeller_released(0);
        assert!(!ai.can_release_rappeller(1));
        assert!(ai.can_release_rappeller(DEFAULT_ROPE_RELEASE_DELAY_FRAMES));
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
            let frame = logic.get_current_frame() as u32;
            let obj = logic.host_object_mut(id).expect("obj");
            if !obj.is_combat_chinook_style_container() {
                obj.install_combat_chinook_transport(frame);
            }
            let ai = obj.chinook_ai.as_ref().expect("chinook_ai installed");
            assert_eq!(ai.flight_status, ChinookFlightStatus::Flying);
            assert_eq!(ai.ai_free_to_exit(false), ChinookFreeToExit::WaitToExit);
            obj.pending_evacuate_on_stop = true;
        }
        // One logic frame (C++ ChinookAIUpdate::update runs per 1/30s tick);
        // 150 u/s speed moves 5u per tick, so an 80u descent stays Landing.
        logic
            .tick_chinook_ai(1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        let obj = logic.host_object(id).expect("obj");
        let ai = obj.chinook_ai.as_ref().expect("chinook_ai");
        assert_eq!(ai.flight_status, ChinookFlightStatus::Landing);
        assert!(obj.precise_z_pos, "Chinook landing PRECISE_Z_POS");
        assert!(obj.ultra_accurate, "Chinook landing ULTRA_ACCURATE");
        assert_eq!(ai.ai_free_to_exit(false), ChinookFreeToExit::WaitToExit);
    }

    #[test]
    fn landed_evac_keeps_dest_across_takeoff() {
        let mut ai = ChinookAi::new_combat([0.0, 0.0, 0.0]);
        ai.flight_status = ChinookFlightStatus::Landed;
        ai.command_evac([80.0, 0.0, 0.0], false);
        assert_eq!(ai.state, ChinookAiState::TakingOff);
        assert_eq!(ai.pending_evac_dest, Some([80.0, 0.0, 0.0]));
        ai.pos = ai.dest;
        assert!(ai.update_from_observed_pose().is_none());
        assert_eq!(ai.state, ChinookAiState::Idle);
        assert_eq!(ai.pending_evac_dest, Some([80.0, 0.0, 0.0]));
        assert!(ai.update_from_observed_pose().is_none());
        assert_eq!(ai.state, ChinookAiState::MoveToAndEvac);
        assert!((ai.dest[0] - 80.0).abs() < 0.01);
        assert!(ai.pending_evac_dest.is_none());
    }

    #[test]
    fn finish_combat_drop_leaves_doing_combat_drop() {
        let mut ai = ChinookAi::new_combat([0.0, 0.0, 100.0]);
        ai.command_combat_drop([10.0, 0.0, 0.0], None);
        ai.arrive_for_combat_drop();
        assert_eq!(ai.flight_status, ChinookFlightStatus::DoingCombatDrop);
        ai.finish_combat_drop();
        assert_eq!(ai.flight_status, ChinookFlightStatus::Flying);
        assert_eq!(ai.state, ChinookAiState::Idle);
    }

    #[test]
    fn headoffmap_uses_recorded_spawn_not_lz() {
        let spawn = [-40.0, 0.0, 80.0];
        let mut ai = ChinookAi::new_combat(spawn);
        ai.command_evac([40.0, 0.0, 0.0], true);
        assert_eq!(ai.original_pos, spawn);
        ai.pos = [40.0, 0.0, 80.0];
        ai.begin_takeoff_and_exit();
        ai.pos = ai.dest;
        assert!(ai.update_from_observed_pose().is_none());
        assert_eq!(ai.state, ChinookAiState::HeadOffMap);
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
            let frame = logic.get_current_frame() as u32;
            let obj = logic.host_object_mut(id).expect("obj");
            if !obj.is_combat_chinook_style_container() {
                obj.install_combat_chinook_transport(frame);
            }
            obj.set_carried_supply_boxes(8);
            obj.pending_evacuate_on_stop = true;
        }
        assert_eq!(
            logic.host_object(id).unwrap().carried_supply_boxes(),
            8
        );
        assert_eq!(logic.host_object(id).unwrap().drawable_supply_boxes, 8);
        logic
            .tick_chinook_ai(1.0 / 30.0)
            .expect("receiving AI operation succeeds");
        let obj = logic.host_object(id).expect("obj");
        assert_eq!(
            obj.chinook_ai.as_ref().unwrap().flight_status,
            ChinookFlightStatus::Landing
        );
        assert_eq!(
            obj.carried_supply_boxes(), 0,
            "diverted landing must dump cash crates"
        );
        assert_eq!(
            obj.drawable_supply_boxes, 0,
            "diverted landing must dump crate visuals"
        );

        {
            let obj = logic.host_object_mut(id).expect("obj");
            obj.set_carried_supply_boxes(4);
            if let Some(ai) = obj.chinook_ai.as_mut() {
                ai.flight_status = ChinookFlightStatus::Flying;
                ai.state = ChinookAiState::MoveToCombatDrop;
                ai.dest = ai.pos;
                ai.combat_drop_dest_z = ai.pos[2];
            }
        }
        logic
            .tick_chinook_ai(1.0)
            .expect("receiving AI operation succeeds");
        let obj = logic.host_object(id).expect("obj");
        assert_eq!(
            obj.chinook_ai.as_ref().unwrap().flight_status,
            ChinookFlightStatus::DoingCombatDrop
        );
        assert_eq!(
            obj.carried_supply_boxes(), 0,
            "combat drop must dump cash crates"
        );
        assert_eq!(
            obj.drawable_supply_boxes, 0,
            "combat drop must dump crate visuals"
        );
    }


    /// hq-2qedt.269
    #[test]
    fn combat_chinook_install_keys_on_authored_contain_not_template_name() {
        use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
        let mut logic = GameLogic::new();
        for (name, authored) in [("AirF_FixtureChinook269", false), ("Fixture269Chinook", true)] {
            let mut tpl = ThingTemplate::new(name);
            tpl.add_kind_of(KindOf::Vehicle)
                .add_kind_of(KindOf::Aircraft)
                .set_health(350.0);
            if authored {
                author_combat_chinook_contain(&mut tpl);
            }
            logic.templates.insert(name.to_string(), tpl);
        }
        let name_only = logic
            .create_object("AirF_FixtureChinook269", Team::USA, glam::Vec3::ZERO)
            .expect("name-only");
        let authored = logic
            .create_object("Fixture269Chinook", Team::USA, glam::Vec3::new(50.0, 0.0, 0.0))
            .expect("authored");
        let n = logic.host_object(name_only).unwrap();
        assert!(!n.is_combat_chinook_style_container());
        assert!(!n.passengers_allowed_to_fire);
        assert!(n.chinook_ai.is_some(), "still a vanilla Chinook hull");
        let a = logic.host_object(authored).unwrap();
        assert!(a.is_combat_chinook_style_container());
        assert!(a.passengers_allowed_to_fire);
    }
}

#[cfg(test)]
#[path = "host_combat_chinook/live_flight_tests.rs"]
mod live_flight_tests;

#[cfg(test)]
#[path = "host_combat_chinook/flight_fixture.rs"]
mod flight_fixture;
