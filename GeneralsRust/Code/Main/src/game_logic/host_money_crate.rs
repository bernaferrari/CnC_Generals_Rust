//! Host MoneyCrateCollide residual (unit + BuildingPickup).
//!
//! Residual slice (playability):
//! - Models retail `MoneyCrateCollide` / `CrateCollide` pickup without full
//!   CollideModule partition pair events or Anim2D ExecuteAnimation path.
//! - SupplyDropZoneCrate residual: MoneyProvided **250**, BuildingPickup **Yes**,
//!   UpgradedBoost Upgrade_AmericaSupplyLines **+25**.
//! - Unit residual: non-structure, non-neutral colliders within residual radius
//!   credit money and destroy the crate (host API).
//! - BuildingPickup residual: STRUCTURE colliders may pick up when
//!   `building_pickup` is set (Supply Drop Zone path).
//!
//! Fail-closed honesty:
//! - Not full CrateCollide kindof multi / ForbiddenKindOf / science gate matrix
//! - Not full above-terrain reject / Anim2D MoneyPickUp / EVA floating text
//! - Not network crate replication (network deferred)

use super::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Retail SupplyDropZoneCrate MoneyProvided.
pub const SUPPLY_DROP_CRATE_MONEY_PROVIDED: u32 = 250;

/// Retail UpgradedBoost for Upgrade_AmericaSupplyLines.
pub const SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST: u32 = 25;

/// Residual unit pickup radius (crate GeometryMajorRadius 12 + unit reach).
pub const MONEY_CRATE_UNIT_PICKUP_RADIUS: f32 = 20.0;

/// Residual BuildingPickup radius (zone / structure collect residual).
/// Large enough to cover supply-drop line formation (±50 at spacing 20).
pub const MONEY_CRATE_BUILDING_PICKUP_RADIUS: f32 = 80.0;

/// Audio residual when money crate is collected.
pub const MONEY_CRATE_PICKUP_AUDIO: &str = "CrateMoney";

/// One residual money crate registered after DeliverPayload spawn / test seed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostMoneyCrateEntry {
    pub object_id: ObjectId,
    pub money_provided: u32,
    /// BuildingPickup residual (SupplyDropZoneCrate = Yes).
    pub building_pickup: bool,
    /// SupplyLines boost residual amount when upgrade present.
    pub supply_lines_boost: u32,
    /// When true, bulk BuildingPickup residual already paid for this crate
    /// (unit pickup must not double-credit).
    pub building_pickup_residual_paid: bool,
}

/// Result of a residual crate pickup.
#[derive(Debug, Clone, PartialEq)]
pub struct HostMoneyCratePickup {
    pub crate_id: ObjectId,
    pub picker_id: ObjectId,
    pub team: super::Team,
    pub amount: u32,
    pub supply_lines_boost: u32,
    pub via_building_pickup: bool,
}

/// Host registry of residual money crates + honesty counters.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostMoneyCrateRegistry {
    crates: HashMap<ObjectId, HostMoneyCrateEntry>,
    /// Successful residual pickups (unit or building).
    pub pickups: u32,
    /// Cash credited via residual MoneyCrateCollide path.
    pub cash_total: u32,
    /// Unit (non-structure) pickups.
    pub unit_pickups: u32,
    /// BuildingPickup residual pickups.
    pub building_pickups: u32,
    /// SupplyLines boost cash portion observed.
    pub supply_lines_boost_cash_total: u32,
}

impl HostMoneyCrateRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    pub fn crate_count(&self) -> usize {
        self.crates.len()
    }

    pub fn get(&self, id: ObjectId) -> Option<&HostMoneyCrateEntry> {
        self.crates.get(&id)
    }

    pub fn contains(&self, id: ObjectId) -> bool {
        self.crates.contains_key(&id)
    }

    /// Register a residual money crate (SupplyDropZoneCrate defaults).
    pub fn register_supply_drop_crate(&mut self, object_id: ObjectId) {
        self.register(
            object_id,
            SUPPLY_DROP_CRATE_MONEY_PROVIDED,
            true,
            SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST,
        );
    }

    pub fn register(
        &mut self,
        object_id: ObjectId,
        money_provided: u32,
        building_pickup: bool,
        supply_lines_boost: u32,
    ) {
        self.crates.insert(
            object_id,
            HostMoneyCrateEntry {
                object_id,
                money_provided,
                building_pickup,
                supply_lines_boost,
                building_pickup_residual_paid: false,
            },
        );
    }

    pub fn forget(&mut self, object_id: ObjectId) {
        self.crates.remove(&object_id);
    }

    pub fn ids(&self) -> Vec<ObjectId> {
        self.crates.keys().copied().collect()
    }

    /// Money amount for a pickup (base + optional SupplyLines boost).
    pub fn cash_for_pickup(entry: &HostMoneyCrateEntry, has_supply_lines: bool) -> (u32, u32) {
        if entry.money_provided == 0 || entry.building_pickup_residual_paid {
            return (0, 0);
        }
        let boost = if has_supply_lines {
            entry.supply_lines_boost
        } else {
            0
        };
        (entry.money_provided.saturating_add(boost), boost)
    }

    /// Horizontal XZ distance residual (crate collide proximity).
    pub fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
        let dx = a.x - b.x;
        let dz = a.z - b.z;
        (dx * dx + dz * dz).sqrt()
    }

    /// Whether residual unit collider may pick up (CrateCollide isValidToExecute subset).
    pub fn is_legal_unit_picker(
        is_alive: bool,
        is_neutral: bool,
        is_structure: bool,
        is_projectile: bool,
    ) -> bool {
        is_alive && !is_neutral && !is_structure && !is_projectile
    }

    /// Whether residual structure collider may pick up (BuildingPickup).
    pub fn is_legal_building_picker(
        is_alive: bool,
        is_neutral: bool,
        is_structure: bool,
        is_constructed: bool,
        building_pickup: bool,
    ) -> bool {
        building_pickup && is_alive && !is_neutral && is_structure && is_constructed
    }

    /// Apply a successful residual pickup: remove crate entry and update honesty.
    pub fn record_pickup(
        &mut self,
        crate_id: ObjectId,
        amount: u32,
        supply_lines_boost: u32,
        via_building_pickup: bool,
    ) -> bool {
        if amount == 0 {
            return false;
        }
        if self.crates.remove(&crate_id).is_none() {
            return false;
        }
        self.pickups = self.pickups.saturating_add(1);
        self.cash_total = self.cash_total.saturating_add(amount);
        self.supply_lines_boost_cash_total = self
            .supply_lines_boost_cash_total
            .saturating_add(supply_lines_boost.min(amount));
        if via_building_pickup {
            self.building_pickups = self.building_pickups.saturating_add(1);
        } else {
            self.unit_pickups = self.unit_pickups.saturating_add(1);
        }
        true
    }

    /// Mark crates as BuildingPickup residual bulk-paid (unit path disabled).
    pub fn mark_building_pickup_residual_paid(&mut self, crate_ids: &[ObjectId]) {
        for id in crate_ids {
            if let Some(entry) = self.crates.get_mut(id) {
                entry.building_pickup_residual_paid = true;
            }
        }
    }

    // --- Honesty ---

    pub fn honesty_unit_pickup_ok(&self) -> bool {
        self.unit_pickups > 0 && self.cash_total > 0
    }

    pub fn honesty_building_pickup_ok(&self) -> bool {
        self.building_pickups > 0 && self.cash_total > 0
    }

    pub fn honesty_money_crate_collide_ok(&self) -> bool {
        self.pickups > 0 && self.cash_total > 0
    }

    pub fn honesty_supply_lines_boost_ok(&self) -> bool {
        self.supply_lines_boost_cash_total > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn supply_drop_crate_money_constants() {
        assert_eq!(SUPPLY_DROP_CRATE_MONEY_PROVIDED, 250);
        assert_eq!(SUPPLY_DROP_CRATE_SUPPLY_LINES_BOOST, 25);
        assert!(MONEY_CRATE_BUILDING_PICKUP_RADIUS >= 50.0);
    }

    #[test]
    fn unit_pickup_credits_and_forgets_crate() {
        let mut reg = HostMoneyCrateRegistry::new();
        let crate_id = ObjectId(10);
        reg.register_supply_drop_crate(crate_id);
        assert_eq!(reg.crate_count(), 1);
        let (amount, boost) =
            HostMoneyCrateRegistry::cash_for_pickup(reg.get(crate_id).unwrap(), false);
        assert_eq!(amount, 250);
        assert_eq!(boost, 0);
        assert!(reg.record_pickup(crate_id, amount, boost, false));
        assert!(reg.honesty_unit_pickup_ok());
        assert!(!reg.contains(crate_id));
        assert_eq!(reg.cash_total, 250);
    }

    #[test]
    fn supply_lines_boost_residual() {
        let mut reg = HostMoneyCrateRegistry::new();
        let crate_id = ObjectId(11);
        reg.register_supply_drop_crate(crate_id);
        let (amount, boost) =
            HostMoneyCrateRegistry::cash_for_pickup(reg.get(crate_id).unwrap(), true);
        assert_eq!(amount, 275);
        assert_eq!(boost, 25);
        assert!(reg.record_pickup(crate_id, amount, boost, true));
        assert!(reg.honesty_building_pickup_ok());
        assert!(reg.honesty_supply_lines_boost_ok());
    }

    #[test]
    fn legal_picker_gates() {
        assert!(HostMoneyCrateRegistry::is_legal_unit_picker(
            true, false, false, false
        ));
        assert!(!HostMoneyCrateRegistry::is_legal_unit_picker(
            true, true, false, false
        ));
        assert!(!HostMoneyCrateRegistry::is_legal_unit_picker(
            true, false, true, false
        ));
        assert!(HostMoneyCrateRegistry::is_legal_building_picker(
            true, false, true, true, true
        ));
        assert!(!HostMoneyCrateRegistry::is_legal_building_picker(
            true, false, true, true, false
        ));
        let _ = Team::USA;
    }
}
