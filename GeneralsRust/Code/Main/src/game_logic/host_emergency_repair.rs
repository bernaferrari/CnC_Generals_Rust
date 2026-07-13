//! Host Emergency Repair special-power residual — single-burst ally vehicle heal.
//!
//! Residual slice (playability):
//! - `DoSpecialPower(EmergencyRepair)` at a world location heals damaged same-team
//!   **VEHICLE** units in radius (retail SuperweaponEmergencyRepair →
//!   SUPERWEAPON_RepairVehicles* → RepairVehiclesInArea_InvisibleMarker_Level*
//!   AutoHealBehavior SingleBurst).
//! - HealingAmount residual 100 / 200 / 300 by science tier (Level1/2/3).
//! - Radius residual 100 (RadiusCursorRadius / AutoHealBehavior Radius).
//! - Honesty counters/flags for residual gates and tests.
//!
//! Fail-closed honesty:
//! - Not full OCL RepairVehicles invisible marker / RepairCloud particle path
//! - Not full ally relationship filter (uses same-team residual)
//! - Not full science tier upgrade matrix (default Level 1; optional level param)
//! - Not KindOf aircraft-as-vehicle edge cases beyond residual Vehicle KindOf
//! - Not network EmergencyRepair replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

/// Logic frames per second (host fixed step).
pub const EMERGENCY_REPAIR_LOGIC_FPS: f32 = 30.0;

/// Retail SuperweaponEmergencyRepair RadiusCursorRadius residual (= 100).
/// Also matches RepairVehiclesInArea_InvisibleMarker AutoHealBehavior Radius.
pub const HOST_EMERGENCY_REPAIR_RADIUS: f32 = 100.0;

/// Retail RepairVehiclesInArea_InvisibleMarker_Level1 HealingAmount.
pub const EMERGENCY_REPAIR_LEVEL1_HEAL: f32 = 100.0;
/// Retail RepairVehiclesInArea_InvisibleMarker_Level2 HealingAmount.
pub const EMERGENCY_REPAIR_LEVEL2_HEAL: f32 = 200.0;
/// Retail RepairVehiclesInArea_InvisibleMarker_Level3 HealingAmount.
pub const EMERGENCY_REPAIR_LEVEL3_HEAL: f32 = 300.0;

/// Activate audio residual (SpecialPower.ini InitiateAtLocationSound).
pub const EMERGENCY_REPAIR_ACTIVATE_AUDIO: &str = "EmergencyRepairActivate";

/// Residual Emergency Repair science tier → HealingAmount.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostEmergencyRepairLevel {
    /// SCIENCE_EmergencyRepair1 → Level1 (100 HP).
    One = 1,
    /// SCIENCE_EmergencyRepair2 → Level2 (200 HP).
    Two = 2,
    /// SCIENCE_EmergencyRepair3 → Level3 (300 HP).
    Three = 3,
}

impl HostEmergencyRepairLevel {
    /// Parse residual level from 1..=3 (fail-closed: unknown → One).
    pub fn from_u8(level: u8) -> Self {
        match level {
            2 => HostEmergencyRepairLevel::Two,
            3 => HostEmergencyRepairLevel::Three,
            _ => HostEmergencyRepairLevel::One,
        }
    }

    pub fn as_u8(self) -> u8 {
        self as u8
    }

    /// Retail AutoHealBehavior HealingAmount for this tier.
    pub fn heal_amount(self) -> f32 {
        match self {
            HostEmergencyRepairLevel::One => EMERGENCY_REPAIR_LEVEL1_HEAL,
            HostEmergencyRepairLevel::Two => EMERGENCY_REPAIR_LEVEL2_HEAL,
            HostEmergencyRepairLevel::Three => EMERGENCY_REPAIR_LEVEL3_HEAL,
        }
    }
}

/// Whether residual target can receive Emergency Repair heal burst.
///
/// Retail AutoHealBehavior KindOf = VEHICLE, SingleBurst, StartsActive:
/// - same-team residual (allies)
/// - alive
/// - VEHICLE KindOf
/// - not under construction residual
/// - damaged (current < max) so heal is observable
pub fn is_legal_emergency_repair_target(
    is_vehicle: bool,
    is_alive: bool,
    same_team: bool,
    under_construction: bool,
    is_damaged: bool,
) -> bool {
    is_vehicle && is_alive && same_team && !under_construction && is_damaged
}

/// 2D distance check residual (C++ FROM_CENTER_2D / AutoHeal Radius).
pub fn in_emergency_repair_radius_2d(center: (f32, f32), target: (f32, f32), radius: f32) -> bool {
    let dx = center.0 - target.0;
    let dz = center.1 - target.1;
    dx * dx + dz * dz <= radius * radius
}

/// One active residual Emergency Repair activation bookkeeping entry.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostEmergencyRepair {
    pub id: u32,
    pub player_id: u32,
    pub location: Vec3,
    pub radius: f32,
    pub level: HostEmergencyRepairLevel,
    pub activate_frame: u32,
    pub caster_id: Option<ObjectId>,
    /// Ally vehicles that received the SingleBurst heal this activation.
    pub heals: u32,
    /// Total HP restored this activation (honesty / debug).
    pub heal_amount_total: f32,
}

/// Host residual registry for Emergency Repair special power activations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostEmergencyRepairRegistry {
    next_id: u32,
    /// Recent activations (bookkeeping).
    activations: Vec<HostEmergencyRepair>,
    /// Total activations (honesty).
    pub activation_count: u32,
    /// Total SingleBurst heal grants applied.
    pub heal_count: u32,
    /// Cumulative HP restored.
    pub heal_amount_total: f32,
}

impl HostEmergencyRepairRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn activation_count(&self) -> u32 {
        self.activation_count
    }

    pub fn heal_count(&self) -> u32 {
        self.heal_count
    }

    pub fn heal_amount_total(&self) -> f32 {
        self.heal_amount_total
    }

    pub fn activations(&self) -> &[HostEmergencyRepair] {
        &self.activations
    }

    pub fn alloc_id(&mut self) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1);
        id
    }

    /// Record a successful residual Emergency Repair activation.
    pub fn record_activation(&mut self, entry: HostEmergencyRepair) {
        self.activation_count = self.activation_count.saturating_add(1);
        self.heal_count = self.heal_count.saturating_add(entry.heals);
        self.heal_amount_total += entry.heal_amount_total;
        self.activations.push(entry);
        // Keep bookkeeping bounded (residual, not full history Xfer).
        if self.activations.len() > 32 {
            let drain = self.activations.len() - 32;
            self.activations.drain(0..drain);
        }
    }

    /// Residual honesty: at least one Emergency Repair activated.
    pub fn honesty_activate_ok(&self) -> bool {
        self.activation_count > 0
    }

    /// Residual honesty: at least one vehicle received heal.
    pub fn honesty_heal_ok(&self) -> bool {
        self.heal_count > 0
    }

    /// Combined host path: activated and healed at least one vehicle.
    pub fn honesty_host_path_ok(&self) -> bool {
        self.honesty_activate_ok() && self.honesty_heal_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emergency_repair_constants_match_retail_residual() {
        assert!((HOST_EMERGENCY_REPAIR_RADIUS - 100.0).abs() < 0.01);
        assert!((HostEmergencyRepairLevel::One.heal_amount() - 100.0).abs() < 0.01);
        assert!((HostEmergencyRepairLevel::Two.heal_amount() - 200.0).abs() < 0.01);
        assert!((HostEmergencyRepairLevel::Three.heal_amount() - 300.0).abs() < 0.01);
        assert!(!EMERGENCY_REPAIR_ACTIVATE_AUDIO.is_empty());
    }

    #[test]
    fn legal_emergency_repair_target_matrix() {
        // is_vehicle, alive, same_team, under_construction, is_damaged
        assert!(is_legal_emergency_repair_target(true, true, true, false, true));
        assert!(!is_legal_emergency_repair_target(false, true, true, false, true)); // infantry
        assert!(!is_legal_emergency_repair_target(true, false, true, false, true)); // dead
        assert!(!is_legal_emergency_repair_target(true, true, false, false, true)); // enemy
        assert!(!is_legal_emergency_repair_target(true, true, true, true, true)); // constructing
        assert!(!is_legal_emergency_repair_target(true, true, true, false, false)); // full HP
    }

    #[test]
    fn emergency_repair_radius_and_level_parse() {
        assert!(in_emergency_repair_radius_2d((0.0, 0.0), (50.0, 0.0), 100.0));
        assert!(!in_emergency_repair_radius_2d((0.0, 0.0), (150.0, 0.0), 100.0));
        assert_eq!(HostEmergencyRepairLevel::from_u8(1), HostEmergencyRepairLevel::One);
        assert_eq!(HostEmergencyRepairLevel::from_u8(2), HostEmergencyRepairLevel::Two);
        assert_eq!(HostEmergencyRepairLevel::from_u8(3), HostEmergencyRepairLevel::Three);
        assert_eq!(
            HostEmergencyRepairLevel::from_u8(99),
            HostEmergencyRepairLevel::One
        ); // fail-closed
    }

    #[test]
    fn honesty_registry_records_heals() {
        let mut reg = HostEmergencyRepairRegistry::new();
        assert!(!reg.honesty_host_path_ok());
        let id = reg.alloc_id();
        reg.record_activation(HostEmergencyRepair {
            id,
            player_id: 1,
            location: Vec3::ZERO,
            radius: HOST_EMERGENCY_REPAIR_RADIUS,
            level: HostEmergencyRepairLevel::One,
            activate_frame: 0,
            caster_id: None,
            heals: 2,
            heal_amount_total: 200.0,
        });
        assert!(reg.honesty_activate_ok());
        assert!(reg.honesty_heal_ok());
        assert!(reg.honesty_host_path_ok());
        assert_eq!(reg.activation_count(), 1);
        assert_eq!(reg.heal_count(), 2);
        assert!((reg.heal_amount_total() - 200.0).abs() < 0.01);
    }
}
