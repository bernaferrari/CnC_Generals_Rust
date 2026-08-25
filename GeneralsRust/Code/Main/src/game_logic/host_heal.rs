//! Host AutoHeal leftovers: DefaultAutoHealBehavior + ambulance radius pulse.
//!
//! Residual slice (playability):
//! - Trainable units: C++ `ModuleTag_DefaultAutoHealBehavior` — StartsActive=Yes,
//!   HealingAmount=**2**, HealingDelay=**1000**ms, StartHealingDelay=**5000**ms,
//!   radius 0. Ctor `giveSelfUpgrade`; `update()` self-heals then sleeps forever
//!   at max health; `onDamage` restarts after StartHealingDelay. HELD still heals.
//! - AmericaVehicleMedic / Ambulance: C++ `AutoHealBehavior` radius **pulse** —
//!   - ModuleTag_22 infantry: HealingAmount=**4**, HealingDelay=**1000**ms,
//!     Radius=**100**, KindOf=INFANTRY, StartsActive=Yes.
//!   - ModuleTag_23 vehicle: HealingAmount=**5**, HealingDelay=**1000**ms,
//!     Radius=**100**, KindOf=VEHICLE, ForbiddenKindOf=AIRCRAFT,
//!     SkipSelfForHealing=Yes, StartsActive=Yes.
//! - TransportContain residual: Slots=**3**, HealthRegen%PerSec=**25** while embarked
//!   (AllowInsideKindOf=INFANTRY).
//! - Sole-benefactor: Object `attemptHealingFromSoleBenefactor` for HealingDelay frames.
//! - HealPad `GetHealed` residual honesty counters live in `GameLogic`.
//!
//! Fail-closed honesty:
//! - Not full particle / world-anim heal pulse FX
//! - Not full TransportContain embark/exit door matrix / DamagePercentToUnits combat
//! - HealthRegen%PerSec embark heal is applied by leftover host tick
//! - Not network heal replication (network deferred)

use super::ObjectId;
use crate::game_logic::GameLogic;
use std::collections::HashMap;

/// Retail ambulance infantry pulse amount residual (AutoHealBehavior ModuleTag_22 HealingAmount).
pub const AMBULANCE_INFANTRY_HEAL_AMOUNT: f32 = 4.0;

/// Retail ambulance vehicle pulse amount residual (AutoHealBehavior ModuleTag_23 HealingAmount).
pub const AMBULANCE_VEHICLE_HEAL_AMOUNT: f32 = 5.0;

/// Retail ambulance heal delay residual in seconds (HealingDelay = 1000 ms).
pub const AMBULANCE_HEAL_DELAY_SEC: f32 = 1.0;

/// Retail HealingDelay msec residual (shared ModuleTag_22 / ModuleTag_23).
pub const AMBULANCE_HEAL_DELAY_MS: u32 = 1000;
/// Retail HealingDelay in logic frames (30 fps).
pub const AMBULANCE_HEAL_DELAY_FRAMES: u32 = 30;

/// Continuous residual rate equivalent to infantry amount/delay pulse average.
pub const HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC: f32 =
    AMBULANCE_INFANTRY_HEAL_AMOUNT / AMBULANCE_HEAL_DELAY_SEC;

/// Continuous residual rate equivalent to vehicle amount/delay pulse average.
pub const HOST_AMBULANCE_VEHICLE_HEAL_HP_PER_SEC: f32 =
    AMBULANCE_VEHICLE_HEAL_AMOUNT / AMBULANCE_HEAL_DELAY_SEC;

/// Retail ambulance AutoHeal radius residual (AmericaVehicleMedic Radius = 100).
pub const HOST_AMBULANCE_HEAL_RADIUS: f32 = 100.0;

/// Retail ModuleTag_23 SkipSelfForHealing residual.
pub const AMBULANCE_VEHICLE_SKIP_SELF_FOR_HEALING: bool = true;

/// Retail TransportContain Slots residual (AmericaVehicleMedic).
pub const AMBULANCE_TRANSPORT_SLOTS: u32 = 3;

/// Retail TransportContain HealthRegen%PerSec residual while embarked.
pub const AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC: f32 = 25.0;

/// Retail TransportContain DamagePercentToUnits residual (honesty pack).
pub const AMBULANCE_TRANSPORT_DAMAGE_PERCENT_TO_UNITS: f32 = 0.10;

/// Retail `ModuleTag_DefaultAutoHealBehavior` HealingAmount residual.
pub const DEFAULT_AUTO_HEAL_AMOUNT: f32 = 2.0;
/// Retail DefaultAutoHeal HealingDelay = 1000 ms.
pub const DEFAULT_AUTO_HEAL_DELAY_MS: u32 = 1000;
/// Retail DefaultAutoHeal HealingDelay in logic frames (30 fps).
pub const DEFAULT_AUTO_HEAL_DELAY_FRAMES: u32 = 30;
/// Retail DefaultAutoHeal StartHealingDelay = 5000 ms.
pub const DEFAULT_AUTO_HEAL_START_DELAY_MS: u32 = 5000;
/// Retail DefaultAutoHeal StartHealingDelay in logic frames (30 fps).
pub const DEFAULT_AUTO_HEAL_START_DELAY_FRAMES: u32 = 150;

/// C++ PartitionFilterRelationship ALLOW_ALLIES: same player or allied player.
/// Leftover fallback: same non-neutral team when player ids are missing.
pub fn ambulance_heal_is_ally(same_team: bool, player_allies: Option<bool>) -> bool {
    match player_allies {
        Some(allies) => allies,
        None => same_team,
    }
}

/// Accumulate `dt` toward a 1-second AutoHeal pulse. Returns true when a pulse is due.
pub fn ambulance_pulse_ready(accum: &mut f32, dt: f32) -> bool {
    if dt <= 0.0 {
        return false;
    }
    *accum += dt;

    if *accum + f32::EPSILON >= AMBULANCE_HEAL_DELAY_SEC {
        *accum -= AMBULANCE_HEAL_DELAY_SEC;
        if *accum < 0.0 {
            *accum = 0.0;
        }
        true
    } else {
        false
    }
}

/// Per-object leftover `ModuleTag_DefaultAutoHealBehavior` residual.
///
/// C++: StartsActive=Yes, HealingAmount=2, HealingDelay=1000ms, StartHealingDelay=5000ms,
/// radius 0. Ctor `giveSelfUpgrade`; `update()` self-heals then sleeps forever at max
/// health; `onDamage` restarts after StartHealingDelay. HELD (garrisoned) still heals.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HostDefaultAutoHealData {
    /// Frame when the next self-heal pulse may run.
    pub wake_frame: u32,
    /// Guard so onDamage cannot force a heal before HealingDelay after a pulse.
    pub soonest_heal_frame: u32,
    pub stopped: bool,
    /// C++ `giveSelfUpgrade` from StartsActive=Yes.
    pub upgrade_active: bool,
    /// Set by leftover damage path; consumed on next tick.
    pub pending_damage: bool,
}

impl Default for HostDefaultAutoHealData {
    fn default() -> Self {
        Self {
            wake_frame: 0,
            soonest_heal_frame: 0,
            stopped: false,
            upgrade_active: true,
            pending_damage: false,
        }
    }
}

impl HostDefaultAutoHealData {
    pub fn new() -> Self {
        Self::default()
    }

    /// Trainable units inherit DefaultAutoHeal; C++ drops the module when `!isTrainable`.
    pub fn for_trainable_template(_template_name: &str, is_trainable: bool) -> Option<Self> {
        if is_trainable {
            Some(Self::new())
        } else {
            None
        }
    }

    /// C++ AutoHealBehavior::onDamage for radius==0: reset StartHealingDelay.
    pub fn on_damage(&mut self, current_frame: u32) {
        if self.stopped || !self.upgrade_active {
            return;
        }
        self.pending_damage = false;
        if DEFAULT_AUTO_HEAL_START_DELAY_FRAMES > 0 {
            self.wake_frame = current_frame.saturating_add(DEFAULT_AUTO_HEAL_START_DELAY_FRAMES);
        } else if current_frame > self.soonest_heal_frame {
            self.wake_frame = current_frame;
        }
    }

    pub fn mark_damaged(&mut self) {
        if self.upgrade_active && !self.stopped {
            self.pending_damage = true;
        }
    }

    /// C++ AutoHealBehavior::undoUpgrade — unmanned enter drops veteran self-heal.
    pub fn undo_upgrade(&mut self) {
        self.upgrade_active = false;
        self.soonest_heal_frame = 0;
        self.wake_frame = 0;
        self.pending_damage = false;
    }

    /// Pulse amount this frame, or 0 if sleeping / full health / stopped.
    /// Garrisoned / HELD units still heal (C++ DISABLED_HELD).
    pub fn tick_heal_amount(
        &mut self,
        current_frame: u32,
        current_health: f32,
        max_health: f32,
    ) -> f32 {
        if self.stopped || !self.upgrade_active {
            return 0.0;
        }
        if self.pending_damage {
            self.on_damage(current_frame);
        }
        if max_health <= 0.0 || current_health >= max_health - f32::EPSILON {
            // C++ update() at max health: UPDATE_SLEEP_FOREVER until onDamage.
            return 0.0;
        }
        if current_frame < self.wake_frame {
            return 0.0;
        }
        self.soonest_heal_frame = current_frame.saturating_add(DEFAULT_AUTO_HEAL_DELAY_FRAMES);
        self.wake_frame = self.soonest_heal_frame;
        DEFAULT_AUTO_HEAL_AMOUNT
    }
}

/// Whether template is a residual ambulance / medic healer unit.
///
/// Fail-closed: name-based residual (not full INI AutoHealBehavior module matrix).
pub fn is_ambulance_healer(template_name: &str) -> bool {
    let n = template_name.to_ascii_lowercase();
    n.contains("ambulance") || n.contains("vehiclemedic") || n.ends_with("medic")
}

/// Whether residual target can receive ambulance infantry AutoHeal (ModuleTag_22).
pub fn is_legal_ambulance_infantry_heal_target(
    is_infantry: bool,
    is_alive: bool,
    is_damaged: bool,
    same_team: bool,
    is_self: bool,
) -> bool {
    is_infantry && is_alive && is_damaged && same_team && !is_self
}

/// Whether residual target can receive ambulance vehicle AutoHeal (ModuleTag_23).
///
/// Retail: KindOf=VEHICLE, ForbiddenKindOf=AIRCRAFT, SkipSelfForHealing=Yes,
/// same controlling player residual (host: same_team), damaged, alive.
pub fn is_legal_ambulance_vehicle_heal_target(
    is_vehicle: bool,
    is_aircraft: bool,
    is_alive: bool,
    is_damaged: bool,
    same_team: bool,
    is_self: bool,
) -> bool {
    if !is_vehicle || is_aircraft {
        return false;
    }
    if !is_alive || !is_damaged || !same_team {
        return false;
    }
    if AMBULANCE_VEHICLE_SKIP_SELF_FOR_HEALING && is_self {
        return false;
    }
    true
}

/// 2D distance check residual (C++ FROM_CENTER_2D).
pub fn in_heal_radius_2d(healer_pos: (f32, f32), target_pos: (f32, f32), radius: f32) -> bool {
    let dx = healer_pos.0 - target_pos.0;
    let dy = healer_pos.1 - target_pos.1;
    dx * dx + dy * dy <= radius * radius
}

/// Embarked infantry heal residual HP/sec from TransportContain HealthRegen%PerSec.
///
/// `percent_per_sec` of max health per second (25 → 0.25 * max_health / sec).
pub fn ambulance_embarked_heal_hp_per_sec(max_health: f32) -> f32 {
    leftover_transport_embarked_heal_hp_per_sec(
        max_health,
        AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC,
    )
}

/// C++ TransportContain HealthRegen%PerSec residual for leftover host templates.
pub fn leftover_transport_health_regen_percent(template_name: &str) -> f32 {
    if is_ambulance_healer(template_name) {
        return AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC;
    }
    if crate::game_logic::host_troop_crawler::is_troop_crawler_template(template_name) {
        return crate::game_logic::host_troop_crawler::TROOP_CRAWLER_HEALTH_REGEN_PERCENT_PER_SEC;
    }
    if crate::game_logic::host_listening_outpost::is_listening_outpost_template(template_name) {
        return crate::game_logic::host_listening_outpost::LISTENING_OUTPOST_HEALTH_REGEN_PERCENT_PER_SEC;
    }
    0.0
}

/// HP/sec from leftover TransportContain HealthRegen%PerSec.
pub fn leftover_transport_embarked_heal_hp_per_sec(max_health: f32, percent_per_sec: f32) -> f32 {
    if max_health <= 0.0 || percent_per_sec == 0.0 {
        return 0.0;
    }
    max_health * (percent_per_sec / 100.0)
}

/// C++ TransportContain::update embark heal for leftover dt:
/// `maxHealth * (percent / 100) * dt`.
pub fn leftover_transport_embarked_heal_amount(
    max_health: f32,
    percent_per_sec: f32,
    dt: f32,
) -> f32 {
    leftover_transport_embarked_heal_hp_per_sec(max_health, percent_per_sec) * dt.max(0.0)
}

/// Residual sole-benefactor exclusivity map (ObjectId → healer_id).
///
/// Host residual: first-healer-wins per pulse/frame — a target accepts heal only
/// from the first ambulance that claims it. Clears each pulse for next cycle.
#[derive(Debug, Clone, Default)]
pub struct HostAmbulanceHealExclusivity {
    /// Target ObjectId → claiming healer ObjectId.
    beneficiaries: HashMap<ObjectId, ObjectId>,
    /// Claims that won (first healer for a target).
    pub claims_granted: u32,
    /// Later healers rejected because target already claimed.
    pub claims_rejected: u32,
}

impl HostAmbulanceHealExclusivity {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.beneficiaries.clear();
        // Keep historical honesty counters (do not zero).
    }

    pub fn clear_pulse(&mut self) {
        self.beneficiaries.clear();
    }

    pub fn reset_honesty(&mut self) {
        *self = Self::default();
    }

    /// First-healer-wins claim. Returns true if `healer` may heal `target`.
    pub fn try_claim(&mut self, target: ObjectId, healer: ObjectId) -> bool {
        match self.beneficiaries.get(&target) {
            Some(existing) if *existing == healer => true,
            Some(_) => {
                self.claims_rejected = self.claims_rejected.saturating_add(1);
                false
            }
            None => {
                self.beneficiaries.insert(target, healer);
                self.claims_granted = self.claims_granted.saturating_add(1);
                true
            }
        }
    }

    pub fn claimed_healer(&self, target: ObjectId) -> Option<ObjectId> {
        self.beneficiaries.get(&target).copied()
    }

    pub fn honesty_exclusivity_ok(&self) -> bool {
        self.claims_granted > 0
    }

    pub fn honesty_reject_ok(&self) -> bool {
        self.claims_rejected > 0
    }
}

/// Residual honesty pack for ambulance AutoHeal ModuleTag_22 + ModuleTag_23 + TransportContain.
pub fn honesty_ambulance_auto_heal_constants_ok() -> bool {
    (AMBULANCE_INFANTRY_HEAL_AMOUNT - 4.0).abs() < 0.001
        && (AMBULANCE_VEHICLE_HEAL_AMOUNT - 5.0).abs() < 0.001
        && (HOST_AMBULANCE_HEAL_RADIUS - 100.0).abs() < 0.001
        && AMBULANCE_HEAL_DELAY_MS == 1000
        && (HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC - 4.0).abs() < 0.001
        && (HOST_AMBULANCE_VEHICLE_HEAL_HP_PER_SEC - 5.0).abs() < 0.001
        && AMBULANCE_VEHICLE_SKIP_SELF_FOR_HEALING
        && AMBULANCE_TRANSPORT_SLOTS == 3
        && (AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC - 25.0).abs() < 0.001
        && (AMBULANCE_TRANSPORT_DAMAGE_PERCENT_TO_UNITS - 0.10).abs() < 0.001
}
/// Combined residual honesty pack (Wave 71 + DefaultAutoHeal).
pub fn honesty_heal_residual_pack_ok() -> bool {
    honesty_ambulance_auto_heal_constants_ok()
        && (DEFAULT_AUTO_HEAL_AMOUNT - 2.0).abs() < 0.001
        && DEFAULT_AUTO_HEAL_DELAY_MS == 1000
        && DEFAULT_AUTO_HEAL_DELAY_FRAMES == 30
        && DEFAULT_AUTO_HEAL_START_DELAY_MS == 5000
        && DEFAULT_AUTO_HEAL_START_DELAY_FRAMES == 150
}

impl GameLogic {
    /// C++ `TransportContain::update` HealthRegen%PerSec on embarked riders.
    ///
    /// Leftover tick uses `dt` seconds: `maxHealth * percent / 100 * dt`.
    pub fn update_transport_health_regen(&mut self, dt: f32) {
        if dt <= 0.0 {
            return;
        }

        let carriers: Vec<(ObjectId, Vec<ObjectId>, f32)> = self
            .objects
            .iter()
            .filter_map(|(id, obj)| {
                if !obj.is_alive() {
                    return None;
                }
                let percent = leftover_transport_health_regen_percent(&obj.template_name);
                if percent == 0.0 {
                    return None;
                }
                let riders = obj.contained_units();
                if riders.is_empty() {
                    return None;
                }
                Some((*id, riders, percent))
            })
            .collect();

        for (_carrier_id, riders, percent) in carriers {
            for rider_id in riders {
                let Some(rider) = self.objects.get_mut(&rider_id) else {
                    continue;
                };
                if !rider.is_alive() {
                    continue;
                }
                let max_health = rider.health.maximum.max(rider.max_health);
                if rider.health.current + 0.01 >= max_health {
                    continue;
                }
                let regen = leftover_transport_embarked_heal_amount(max_health, percent, dt);
                if regen > 0.0 {
                    rider.heal(regen);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ambulance_healer_name_matrix() {
        assert!(is_ambulance_healer("AmericaVehicleMedic"));
        assert!(is_ambulance_healer("USA_Ambulance"));
        assert!(is_ambulance_healer("SupW_AmericaVehicleMedic"));
        assert!(is_ambulance_healer("Lazr_AmericaVehicleMedic"));
        assert!(is_ambulance_healer("AirF_AmericaVehicleMedic"));
        assert!(!is_ambulance_healer("USA_Ranger"));
        assert!(!is_ambulance_healer("TestInfantry"));
        assert!(!is_ambulance_healer("USA_Dozer"));
        assert!(!is_ambulance_healer("ChinaInfantryRedGuard"));
    }

    #[test]
    fn legal_infantry_heal_target_matrix() {
        assert!(is_legal_ambulance_infantry_heal_target(
            true, true, true, true, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            false, true, true, true, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            true, false, true, true, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            true, true, false, true, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            true, true, true, false, false
        ));
        assert!(!is_legal_ambulance_infantry_heal_target(
            true, true, true, true, true
        ));
    }

    #[test]
    fn legal_vehicle_heal_target_matrix_vehicle_vs_infantry_vs_aircraft() {
        // Damaged ally ground vehicle: legal.
        assert!(is_legal_ambulance_vehicle_heal_target(
            true, false, true, true, true, false
        ));
        // Infantry is not a vehicle target (ModuleTag_23 KindOf=VEHICLE).
        assert!(!is_legal_ambulance_vehicle_heal_target(
            false, false, true, true, true, false
        ));
        // Aircraft forbidden even when tagged vehicle+aircraft residual.
        assert!(!is_legal_ambulance_vehicle_heal_target(
            true, true, true, true, true, false
        ));
        // Pure aircraft reject.
        assert!(!is_legal_ambulance_vehicle_heal_target(
            false, true, true, true, true, false
        ));
        // Dead / full HP / enemy / self reject.
        assert!(!is_legal_ambulance_vehicle_heal_target(
            true, false, false, true, true, false
        ));
        assert!(!is_legal_ambulance_vehicle_heal_target(
            true, false, true, false, true, false
        ));
        assert!(!is_legal_ambulance_vehicle_heal_target(
            true, false, true, true, false, false
        ));
        assert!(!is_legal_ambulance_vehicle_heal_target(
            true, false, true, true, true, true
        ));
    }

    #[test]
    fn heal_radius_and_rate_positive() {
        assert!(HOST_AMBULANCE_HEAL_RADIUS > 0.0);
        assert!(HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC > 0.0);
        assert!(HOST_AMBULANCE_VEHICLE_HEAL_HP_PER_SEC > 0.0);
        assert!(HOST_AMBULANCE_VEHICLE_HEAL_HP_PER_SEC > HOST_AMBULANCE_INFANTRY_HEAL_HP_PER_SEC);
        assert!(in_heal_radius_2d((0.0, 0.0), (50.0, 0.0), 100.0));
        assert!(!in_heal_radius_2d((0.0, 0.0), (150.0, 0.0), 100.0));
    }

    #[test]
    fn ambulance_vehicle_auto_heal_constants_residual_honesty() {
        assert!(honesty_ambulance_auto_heal_constants_ok());
        assert_eq!(AMBULANCE_VEHICLE_HEAL_AMOUNT, 5.0);
        assert_eq!(AMBULANCE_INFANTRY_HEAL_AMOUNT, 4.0);
        assert_eq!(HOST_AMBULANCE_HEAL_RADIUS, 100.0);
        assert_eq!(AMBULANCE_HEAL_DELAY_MS, 1000);
        assert!(AMBULANCE_VEHICLE_SKIP_SELF_FOR_HEALING);
    }

    #[test]
    fn ambulance_transport_health_regen_residual_honesty() {
        assert_eq!(AMBULANCE_TRANSPORT_SLOTS, 3);
        assert_eq!(AMBULANCE_TRANSPORT_HEALTH_REGEN_PERCENT_PER_SEC, 25.0);
        // 25% of 100 max HP / sec → 25 HP/sec residual.
        assert!((ambulance_embarked_heal_hp_per_sec(100.0) - 25.0).abs() < 0.001);
        // 25% of 240 (ambulance max health residual) → 60 HP/sec.
        assert!((ambulance_embarked_heal_hp_per_sec(240.0) - 60.0).abs() < 0.001);
        assert_eq!(ambulance_embarked_heal_hp_per_sec(0.0), 0.0);
        assert!((AMBULANCE_TRANSPORT_DAMAGE_PERCENT_TO_UNITS - 0.10).abs() < 0.001);
        assert!(
            (leftover_transport_health_regen_percent("AmericaVehicleAmbulance") - 25.0).abs()
                < 0.001
        );
        assert!(
            (leftover_transport_health_regen_percent("ChinaVehicleTroopCrawler") - 10.0).abs()
                < 0.001
        );
        assert!(
            (leftover_transport_health_regen_percent("ChinaVehicleListeningOutpost") - 10.0).abs()
                < 0.001
        );
        assert_eq!(
            leftover_transport_health_regen_percent("AmericaVehicleHumvee"),
            0.0
        );
        // 10% of 100 max HP * 1/30s leftover frame → C++ TransportContain::update.
        assert!(
            (leftover_transport_embarked_heal_amount(100.0, 10.0, 1.0 / 30.0)
                - (100.0 * 0.10 / 30.0))
                .abs()
                < 0.0001
        );
    }

    #[test]
    fn sole_benefactor_first_healer_wins_residual_honesty() {
        let mut excl = HostAmbulanceHealExclusivity::new();
        assert!(!excl.honesty_exclusivity_ok());
        let target = ObjectId(10);
        let healer_a = ObjectId(1);
        let healer_b = ObjectId(2);
        assert!(excl.try_claim(target, healer_a));
        assert!(excl.honesty_exclusivity_ok());
        assert_eq!(excl.claimed_healer(target), Some(healer_a));
        // Second ambulance rejected for same target.
        assert!(!excl.try_claim(target, healer_b));
        assert!(excl.honesty_reject_ok());
        // Same healer re-claims ok.
        assert!(excl.try_claim(target, healer_a));
        assert_eq!(excl.claims_granted, 1);
        assert_eq!(excl.claims_rejected, 1);
        excl.clear_pulse();
        assert!(excl.try_claim(target, healer_b));
        assert_eq!(excl.claimed_healer(target), Some(healer_b));
    }
    /// Wave 71 residual pack honesty gate.
    #[test]
    fn heal_residual_pack_honesty_wave71() {
        assert!(honesty_heal_residual_pack_ok());
        assert_eq!(AMBULANCE_INFANTRY_HEAL_AMOUNT, 4.0);
        assert_eq!(AMBULANCE_VEHICLE_HEAL_AMOUNT, 5.0);
        assert_eq!(HOST_AMBULANCE_HEAL_RADIUS, 100.0);
        assert_eq!(AMBULANCE_TRANSPORT_SLOTS, 3);
    }

    #[test]
    fn default_auto_heal_on_damage_then_pulse() {
        let mut d = HostDefaultAutoHealData::new();
        assert!(HostDefaultAutoHealData::for_trainable_template("USA_Ranger", true).is_some());
        assert!(
            HostDefaultAutoHealData::for_trainable_template("AmericaCommandCenter", false)
                .is_none()
        );

        d.on_damage(10);
        assert_eq!(d.wake_frame, 10 + DEFAULT_AUTO_HEAL_START_DELAY_FRAMES);
        assert_eq!(
            d.tick_heal_amount(10 + DEFAULT_AUTO_HEAL_START_DELAY_FRAMES - 1, 50.0, 100.0),
            0.0
        );
        assert_eq!(
            d.tick_heal_amount(10 + DEFAULT_AUTO_HEAL_START_DELAY_FRAMES, 50.0, 100.0),
            DEFAULT_AUTO_HEAL_AMOUNT
        );
        // At max health: sleep forever until damaged again.
        assert_eq!(
            d.tick_heal_amount(10 + DEFAULT_AUTO_HEAL_START_DELAY_FRAMES + 30, 100.0, 100.0),
            0.0
        );
    }

    #[test]
    fn ambulance_pulse_and_ally_filter() {
        let mut accum = 0.0;
        assert!(!ambulance_pulse_ready(&mut accum, 1.0 / 30.0));
        for _ in 0..28 {
            assert!(!ambulance_pulse_ready(&mut accum, 1.0 / 30.0));
        }
        assert!(ambulance_pulse_ready(&mut accum, 1.0 / 30.0));

        assert!(ambulance_heal_is_ally(true, None));
        assert!(!ambulance_heal_is_ally(false, None));
        assert!(ambulance_heal_is_ally(false, Some(true)));
        assert!(!ambulance_heal_is_ally(true, Some(false)));
    }
}
