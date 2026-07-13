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
//! - Black Lotus `StealCashHack`: walk to enemy supply/cash building → steal
//!   a fixed residual cash amount into the hero's player resources.
//! - Black Lotus `DisableVehicleHack`: walk to enemy ground vehicle → apply
//!   DISABLED_HACKED for EffectDuration residual (30s / 900 logic frames);
//!   vehicle cannot move or attack until the timer expires.
//!
//! Fail-closed honesty:
//! - Not full SpecialAbilityUpdate preparation timers / packing / flee-after-plant
//! - Not full StickyBombUpdate attach bones / geometry splash / max-charge list UI
//! - Not full CashHackSpecialPower science upgrade money matrix
//! - Not combat-bike rider-eject / academy sniped-vehicle stats
//! - Not full laser attach / disable FX particle interleave / VoiceDisableVehicleComplete

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const HERO_ABILITY_LOGIC_FPS: f32 = 30.0;

/// Retail-inspired residual cash steal amount (SpecialAbilityBlackLotusStealCashHack
/// default MoneyAmount residual; fail-closed vs upgrade matrix).
pub const STEAL_CASH_DEFAULT_AMOUNT: u32 = 1000;

/// C++ SpecialAbilityUpdate EffectDuration = 30000 ms for
/// SpecialAbilityBlackLotusDisableVehicleHack (30 seconds at 30 FPS logic).
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

    /// Combined hero residual path honesty (any hero ability observed).
    pub fn honesty_any_ok(&self) -> bool {
        self.honesty_snipe_ok()
            || self.honesty_timed_charge_plant_ok()
            || self.honesty_remote_charge_plant_ok()
            || self.honesty_remote_charge_detonate_ok()
            || self.honesty_cash_steal_ok()
            || self.honesty_vehicle_disable_ok()
    }
}

/// True when a template/building is a residual cash-hack target (supply / black market).
pub fn is_cash_hack_target_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("supplycenter")
        || n.contains("supply_center")
        || n.contains("blackmarket")
        || n.contains("black_market")
        || n.contains("supplydropzone")
        || n == "testsupplycenter"
        || n == "testbuilding"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn honesty_flags_track_snipe_and_cash() {
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
        assert_eq!(DISABLE_VEHICLE_HACK_DURATION_FRAMES, 900);
    }

    #[test]
    fn cash_hack_template_names() {
        assert!(is_cash_hack_target_template("AmericaSupplyCenter"));
        assert!(is_cash_hack_target_template("GLABlackMarket"));
        assert!(is_cash_hack_target_template("TestSupplyCenter"));
        assert!(!is_cash_hack_target_template("AmericaRanger"));
    }
}
