//! Host hero special-ability residual (Burton / Jarmen Kell / Black Lotus).
//!
//! Residual slice (playability):
//! - Jarmen Kell `SnipeVehicle`: DAMAGE_KILLPILOT residual — vehicle becomes
//!   unmanned + Neutral (no HP damage), so infantry can later recrew/capture.
//! - Colonel Burton `PlantTimedDemoCharge`: walk to structure/vehicle → plant
//!   sticky timed charge (reuses host_mines TimedDemoCharge residual).
//! - Colonel Burton `PlantRemoteDemoCharge` + `DetonateRemoteDemoCharges`:
//!   plant sticky remote charge (no auto-timer) then remote-detonate all charges
//!   planted by that producer (SPECIAL_REMOTE_CHARGES residual).
//! - Black Lotus `CaptureBuilding`: hero capture residual without infantry
//!   Capture research; StartAbilityRange **150** (vs infantry melee pad);
//!   reuses Capturing AI ownership-transfer residual.
//! - Black Lotus `StealCashHack`: walk to enemy cash generator (supply /
//!   black market / drop zone) within range **150** → steal residual cash.
//! - Black Lotus `DisableVehicleHack`: walk to enemy ground vehicle within
//!   range **150** → DISABLED_HACKED for EffectDuration residual (30s / 900
//!   logic frames); vehicle cannot move or attack until the timer expires.
//!
//! Fail-closed honesty:
//! - Not full SpecialAbilityUpdate preparation timers / packing / flee-after-plant
//! - Not full StickyBombUpdate attach bones / geometry splash / max-charge list UI
//! - Not full CashHackSpecialPower science upgrade money matrix (1000/2000/4000)
//! - Not combat-bike rider-eject / academy sniped-vehicle stats
//! - Not full laser attach / disable FX particle interleave / VoiceDisableVehicleComplete
//! - Not full ActionManager canCapture edge matrix (stealth / garrison / shroud)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const HERO_ABILITY_LOGIC_FPS: f32 = 30.0;

/// Retail StartAbilityRange for all three Black Lotus specials
/// (CaptureBuilding / DisableVehicleHack / StealCashHack).
pub const BLACK_LOTUS_START_ABILITY_RANGE: f32 = 150.0;

/// Retail-inspired residual cash steal amount (SpecialAbilityBlackLotusStealCashHack
/// default MoneyAmount residual; fail-closed vs SCIENCE_CashHack2/3 matrix).
pub const STEAL_CASH_DEFAULT_AMOUNT: u32 = 1000;

/// C++ SpecialAbilityUpdate EffectDuration residual for
/// SpecialAbilityBlackLotusDisableVehicleHack.
/// Host residual locks 30s / 900 frames (playability; INI EffectDuration=15000
/// comment claims 30s — fail-closed vs full SpecialAbilityUpdate timer).
pub const DISABLE_VEHICLE_HACK_DURATION_MS: u32 = 30_000;

/// Logic-frame residual of EffectDuration (ms * 30 / 1000).
pub const DISABLE_VEHICLE_HACK_DURATION_FRAMES: u32 =
    (DISABLE_VEHICLE_HACK_DURATION_MS * 30) / 1000;

/// Audio residual when a vehicle pilot is sniped (host-side cue name).
pub const SNIPE_VEHICLE_AUDIO: &str = "UnitSniped";

/// Audio residual when Black Lotus completes cash steal.
pub const STEAL_CASH_AUDIO: &str = "BlackLotusStealCash";

/// Audio residual when Black Lotus completes vehicle disable hack.
pub const DISABLE_VEHICLE_HACK_AUDIO: &str = "BlackLotusDisableVehicle";

/// Audio residual when Black Lotus completes building capture.
pub const CAPTURE_BUILDING_AUDIO: &str = "BlackLotusCaptureBuilding";

/// Whether template is a residual Black Lotus hero.
///
/// Fail-closed: name residual. Excludes weapons / science / debris tokens.
pub fn is_black_lotus_template(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    if n.is_empty() {
        return false;
    }
    if n.contains("weapon")
        || n.contains("projectile")
        || n.contains("missile")
        || n.contains("debris")
        || n.contains("hulk")
        || n.contains("dead")
        || n.starts_with("upgrade")
        || n.contains("science")
        || n.contains("crate")
        || n.contains("locomotor")
        || n.contains("voice")
        || n.contains("command")
        || n.contains("button")
        || n.contains("portrait")
        || n.contains("hack")
        || n.contains("disable")
        || n.contains("steal")
        || n.contains("capture")
    {
        return false;
    }
    // Explicit residual test / shorthand names.
    if n == "testblacklotus"
        || n == "testlotus"
        || n == "black_lotus"
        || n == "china_blacklotus"
        || n == "china_lotus"
    {
        return true;
    }
    n.contains("blacklotus") || n.contains("black_lotus")
}

/// Whether residual unit can issue Black Lotus specials (alive + template).
pub fn can_activate_black_lotus_ability(is_lotus: bool, is_alive: bool) -> bool {
    is_lotus && is_alive
}

/// Whether residual unit may use CaptureBuilding without infantry Capture research.
///
/// Heroes (KindOf::Hero / name) and Black Lotus template residual.
pub fn can_capture_without_upgrade(is_hero: bool, is_lotus: bool) -> bool {
    is_hero || is_lotus
}

/// Whether unit is within Black Lotus StartAbilityRange residual.
pub fn black_lotus_in_start_range(distance: f32) -> bool {
    distance <= BLACK_LOTUS_START_ABILITY_RANGE
}

/// Legal residual StealCashHack target (enemy cash generator structure).
pub fn is_legal_steal_cash_target(
    is_alive: bool,
    is_structure: bool,
    under_construction: bool,
    is_enemy: bool,
    is_cash_generator: bool,
) -> bool {
    is_alive && is_structure && !under_construction && is_enemy && is_cash_generator
}

/// Legal residual DisableVehicleHack target (enemy manned ground vehicle).
pub fn is_legal_disable_vehicle_target(
    is_alive: bool,
    is_vehicle: bool,
    is_airborne: bool,
    is_enemy: bool,
    already_hacked: bool,
    unmanned: bool,
) -> bool {
    is_alive
        && is_vehicle
        && !is_airborne
        && is_enemy
        && !already_hacked
        && !unmanned
}

/// Legal residual Black Lotus CaptureBuilding target (enemy structure).
pub fn is_legal_lotus_capture_target(
    is_alive: bool,
    is_structure: bool,
    under_construction: bool,
    is_enemy: bool,
) -> bool {
    is_alive && is_structure && !under_construction && is_enemy
}

/// Absolute expiry frame for residual vehicle disable.
pub fn disable_vehicle_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(DISABLE_VEHICLE_HACK_DURATION_FRAMES)
}

/// True when a template/building is a residual cash-hack target (C++ KINDOF_CASH_GENERATOR).
///
/// Fail-closed name residual for supply centers, black markets, supply drop zones.
pub fn is_cash_hack_target_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("supplycenter")
        || n.contains("supply_center")
        || n.contains("blackmarket")
        || n.contains("black_market")
        || n.contains("supplydropzone")
        || n.contains("supply_drop")
        || n == "testsupplycenter"
        || n == "testbuilding"
        || n == "testcashgenerator"
}

/// Whether object kinds residual-match a cash generator (SupplyCenter / BlackMarket).
pub fn is_cash_generator_kind(
    is_supply_center: bool,
    is_fs_supply_center: bool,
    is_black_market: bool,
    is_supply_dropzone: bool,
) -> bool {
    is_supply_center || is_fs_supply_center || is_black_market || is_supply_dropzone
}

/// Combined residual cash-generator check (template name OR kind flags).
pub fn is_cash_hack_target(
    template_name: &str,
    is_supply_center: bool,
    is_fs_supply_center: bool,
    is_black_market: bool,
    is_supply_dropzone: bool,
) -> bool {
    is_cash_hack_target_template(template_name)
        || is_cash_generator_kind(
            is_supply_center,
            is_fs_supply_center,
            is_black_market,
            is_supply_dropzone,
        )
}

/// Horizontal distance helper for residual attach placement.
pub fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

/// Bookkeeping id for residual plant (producer → charge object).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroAbilityPlant {
    pub producer_id: ObjectId,
    pub charge_id: ObjectId,
    pub target_id: ObjectId,
}

/// Host residual honesty counters for hero special abilities.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostHeroAbilityRegistry {
    /// Jarmen Kell snipe resolved (vehicle unmanned).
    pub snipe_kills: u32,
    /// Burton timed demo charge planted via special ability.
    pub timed_charges_planted: u32,
    /// Burton remote demo charge planted via special ability.
    pub remote_charges_planted: u32,
    /// Remote demo charge detonations resolved (count of charges blown).
    pub remote_charges_detonated: u32,
    /// Black Lotus cash-hack steals completed.
    pub cash_steals: u32,
    /// Total cash transferred via residual cash-hack.
    pub cash_stolen_total: u32,
    /// Black Lotus disable-vehicle hacks completed.
    pub vehicle_disables: u32,
    /// Black Lotus / hero CaptureBuilding residual completes.
    pub building_captures: u32,
}

impl HostHeroAbilityRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn record_snipe(&mut self) {
        self.snipe_kills = self.snipe_kills.saturating_add(1);
    }

    pub fn record_timed_charge_plant(&mut self) {
        self.timed_charges_planted = self.timed_charges_planted.saturating_add(1);
    }

    pub fn record_remote_charge_plant(&mut self) {
        self.remote_charges_planted = self.remote_charges_planted.saturating_add(1);
    }

    pub fn record_remote_charge_detonate(&mut self, count: u32) {
        self.remote_charges_detonated = self.remote_charges_detonated.saturating_add(count);
    }

    pub fn record_cash_steal(&mut self, amount: u32) {
        self.cash_steals = self.cash_steals.saturating_add(1);
        self.cash_stolen_total = self.cash_stolen_total.saturating_add(amount);
    }

    pub fn record_vehicle_disable(&mut self) {
        self.vehicle_disables = self.vehicle_disables.saturating_add(1);
    }

    pub fn record_building_capture(&mut self) {
        self.building_captures = self.building_captures.saturating_add(1);
    }

    /// Residual honesty: at least one snipe unmanned a vehicle.
    pub fn honesty_snipe_ok(&self) -> bool {
        self.snipe_kills > 0
    }

    /// Residual honesty: at least one timed charge planted by hero ability.
    pub fn honesty_timed_charge_plant_ok(&self) -> bool {
        self.timed_charges_planted > 0
    }

    /// Residual honesty: at least one remote charge planted by hero ability.
    pub fn honesty_remote_charge_plant_ok(&self) -> bool {
        self.remote_charges_planted > 0
    }

    /// Residual honesty: plant → remote detonate path exercised.
    pub fn honesty_remote_charge_detonate_ok(&self) -> bool {
        self.remote_charges_planted > 0 && self.remote_charges_detonated > 0
    }

    /// Residual honesty: at least one cash steal completed.
    pub fn honesty_cash_steal_ok(&self) -> bool {
        self.cash_steals > 0 && self.cash_stolen_total > 0
    }

    /// Residual honesty: at least one vehicle disable hack completed.
    pub fn honesty_vehicle_disable_ok(&self) -> bool {
        self.vehicle_disables > 0
    }

    /// Residual honesty: at least one Black Lotus / hero building capture completed.
    pub fn honesty_building_capture_ok(&self) -> bool {
        self.building_captures > 0
    }

    /// Combined hero residual path honesty (any hero ability observed).
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_snipe_ok()
            || self.honesty_timed_charge_plant_ok()
            || self.honesty_remote_charge_plant_ok()
            || self.honesty_remote_charge_detonate_ok()
            || self.honesty_cash_steal_ok()
            || self.honesty_vehicle_disable_ok()
            || self.honesty_building_capture_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_flags_track_snipe_cash_and_capture() {
        let mut reg = HostHeroAbilityRegistry::new();
        assert!(!reg.honesty_any_ok());
        reg.record_snipe();
        assert!(reg.honesty_snipe_ok());
        assert!(reg.honesty_any_ok());
        reg.record_cash_steal(500);
        assert!(reg.honesty_cash_steal_ok());
        assert_eq!(reg.cash_stolen_total, 500);
        reg.record_timed_charge_plant();
        assert!(reg.honesty_timed_charge_plant_ok());
        reg.record_remote_charge_plant();
        reg.record_remote_charge_detonate(2);
        assert!(reg.honesty_remote_charge_detonate_ok());
        assert_eq!(reg.remote_charges_detonated, 2);
        reg.record_vehicle_disable();
        assert!(reg.honesty_vehicle_disable_ok());
        reg.record_building_capture();
        assert!(reg.honesty_building_capture_ok());
        assert_eq!(DISABLE_VEHICLE_HACK_DURATION_FRAMES, 900);
        assert_eq!(BLACK_LOTUS_START_ABILITY_RANGE, 150.0);
    }

    #[test]
    fn cash_hack_template_names() {
        assert!(is_cash_hack_target_template("AmericaSupplyCenter"));
        assert!(is_cash_hack_target_template("GLABlackMarket"));
        assert!(is_cash_hack_target_template("TestSupplyCenter"));
        assert!(is_cash_hack_target_template("TestBuilding"));
        assert!(!is_cash_hack_target_template("AmericaRanger"));
        assert!(!is_cash_hack_target_template("AmericaWarFactory"));
    }

    #[test]
    fn black_lotus_template_names() {
        assert!(is_black_lotus_template("ChinaInfantryBlackLotus"));
        assert!(is_black_lotus_template("Infa_ChinaInfantryBlackLotus"));
        assert!(is_black_lotus_template("Nuke_ChinaInfantryBlackLotus"));
        assert!(is_black_lotus_template("TestBlackLotus"));
        assert!(is_black_lotus_template("TestLotus"));
        assert!(!is_black_lotus_template("ChinaInfantryHacker"));
        assert!(!is_black_lotus_template("ChinaInfantryRedguard"));
        assert!(!is_black_lotus_template("BlackLotusVoiceHackCash"));
        assert!(!is_black_lotus_template("SpecialAbilityBlackLotusStealCashHack"));
        assert!(!is_black_lotus_template("TestTank"));
        assert!(can_activate_black_lotus_ability(true, true));
        assert!(!can_activate_black_lotus_ability(true, false));
        assert!(!can_activate_black_lotus_ability(false, true));
        assert!(can_capture_without_upgrade(true, false));
        assert!(can_capture_without_upgrade(false, true));
        assert!(!can_capture_without_upgrade(false, false));
    }

    #[test]
    fn legal_target_matrices() {
        assert!(is_legal_steal_cash_target(true, true, false, true, true));
        assert!(!is_legal_steal_cash_target(true, true, false, true, false));
        assert!(!is_legal_steal_cash_target(true, true, true, true, true));
        assert!(!is_legal_steal_cash_target(true, false, false, true, true));
        assert!(is_legal_disable_vehicle_target(
            true, true, false, true, false, false
        ));
        assert!(!is_legal_disable_vehicle_target(
            true, true, true, true, false, false
        ));
        assert!(!is_legal_disable_vehicle_target(
            true, true, false, true, true, false
        ));
        assert!(!is_legal_disable_vehicle_target(
            true, true, false, true, false, true
        ));
        assert!(is_legal_lotus_capture_target(true, true, false, true));
        assert!(!is_legal_lotus_capture_target(true, true, true, true));
        assert!(black_lotus_in_start_range(150.0));
        assert!(black_lotus_in_start_range(0.0));
        assert!(!black_lotus_in_start_range(150.1));
        assert_eq!(disable_vehicle_until_frame(100), 1000);
    }
}
