//! Resource Gathering Manager
//!
//! The part of a Player's brain that keeps track of all Resource type Objects and makes
//! gathering type decisions based on them.

use std::collections::VecDeque;

use crate::common::system::{Snapshotable, Xfer, XferMode, XferVersion};

/// Object ID type for tracking game objects
pub type ObjectId = u32;

/// Provides access to the wider game world so the resource manager can
/// evaluate destinations deterministically while remaining testable.
pub trait ResourceWorld {
    /// Returns true if the object is still present in the world.
    fn object_exists(&self, id: ObjectId) -> bool;

    /// Returns true if the object has an AI update (matches C++ queryObject->getAI()).
    fn has_ai(&self, _id: ObjectId) -> bool {
        true
    }

    /// Tests whether supplies can be exchanged between `query_id` and `dest_id`.
    fn can_transfer_supplies_at(&self, query_id: ObjectId, dest_id: ObjectId) -> bool;

    /// Checks whether the dock associated with `dest_id` is clear for `query_id` to approach.
    fn is_clear_to_approach(&self, dest_id: ObjectId, query_id: ObjectId) -> bool;

    /// Returns the squared distance between query and destination or `None` if unreachable.
    fn distance_squared(&self, query_id: ObjectId, dest_id: ObjectId) -> Option<f32>;

    /// Returns true if the object is a supply warehouse dock.
    fn is_supply_warehouse_dock(&self, _dock_id: ObjectId) -> bool {
        true
    }

    /// Returns true if the object is a supply center dock.
    fn is_supply_center_dock(&self, _dock_id: ObjectId) -> bool {
        true
    }

    /// Optional user / AI override for docking at a specific object.
    fn preferred_dock(&self, _query_id: ObjectId) -> Option<ObjectId> {
        None
    }

    /// Optional per-unit scan radius for warehouses.
    fn warehouse_scan_distance(&self, _query_id: ObjectId) -> Option<f32> {
        None
    }
}

/// Resource gathering manager
///
/// Manages supply centers and warehouses for efficient resource collection.
/// Helps harvesters find the best places to collect and deposit resources.
pub struct ResourceGatheringManager {
    /// List of supply warehouse object IDs
    supply_warehouses: VecDeque<ObjectId>,
    /// List of supply center object IDs  
    supply_centers: VecDeque<ObjectId>,
}

impl ResourceGatheringManager {
    /// Create a new ResourceGatheringManager
    pub fn new() -> Self {
        Self {
            supply_warehouses: VecDeque::new(),
            supply_centers: VecDeque::new(),
        }
    }

    /// Add a supply center to the manager
    pub fn add_supply_center(&mut self, center_id: ObjectId) {
        self.supply_centers.push_back(center_id);
    }

    /// Remove a supply center from the manager
    pub fn remove_supply_center(&mut self, center_id: ObjectId) {
        self.supply_centers.retain(|&id| id != center_id);
    }

    /// Add a supply warehouse to the manager
    pub fn add_supply_warehouse(&mut self, warehouse_id: ObjectId) {
        self.supply_warehouses.push_back(warehouse_id);
    }

    /// Remove a supply warehouse from the manager
    pub fn remove_supply_warehouse(&mut self, warehouse_id: ObjectId) {
        self.supply_warehouses.retain(|&id| id != warehouse_id);
    }

    /// Find the best supply warehouse for a query object
    ///
    /// This considers distance, availability, and other factors to determine
    /// the optimal warehouse for a harvester to gather resources from.
    pub fn find_best_supply_warehouse<W: ResourceWorld>(
        &mut self,
        query_object_id: ObjectId,
        world: &W,
    ) -> Option<ObjectId> {
        if !world.object_exists(query_object_id) {
            return None;
        }

        if !world.has_ai(query_object_id) {
            return None;
        }

        Self::clean_invalid_objects(world, &mut self.supply_warehouses);

        if self.supply_warehouses.is_empty() {
            return None;
        }

        if let Some(preferred) = world.preferred_dock(query_object_id) {
            if world.object_exists(preferred) && world.is_supply_warehouse_dock(preferred) {
                if let Some((cost, _)) = compute_relative_cost(world, query_object_id, preferred) {
                    if cost.is_finite() {
                        return Some(preferred);
                    }
                }
            }
        }

        let max_distance_sq = world
            .warehouse_scan_distance(query_object_id)
            .map(|d| d.max(0.0).powi(2))
            .unwrap_or(100000.0);

        let mut best: Option<(ObjectId, f32)> = None;

        for &warehouse_id in &self.supply_warehouses {
            if let Some((cost, distance_sq)) =
                compute_relative_cost(world, query_object_id, warehouse_id)
            {
                if distance_sq <= max_distance_sq
                    && cost
                        < best
                            .map(|(_, best_cost)| best_cost)
                            .unwrap_or(f32::INFINITY)
                {
                    best = Some((warehouse_id, cost));
                }
            }
        }

        best.map(|(id, _)| id)
    }

    /// Find the best supply center for a query object
    ///
    /// This finds the optimal supply center for a harvester to deposit
    /// resources at, considering distance and availability.
    pub fn find_best_supply_center<W: ResourceWorld>(
        &mut self,
        query_object_id: ObjectId,
        world: &W,
    ) -> Option<ObjectId> {
        if !world.object_exists(query_object_id) {
            return None;
        }

        if !world.has_ai(query_object_id) {
            return None;
        }

        Self::clean_invalid_objects(world, &mut self.supply_centers);

        if self.supply_centers.is_empty() {
            return None;
        }

        if let Some(preferred) = world.preferred_dock(query_object_id) {
            if world.object_exists(preferred) && world.is_supply_center_dock(preferred) {
                if let Some((cost, _)) = compute_relative_cost(world, query_object_id, preferred) {
                    if cost.is_finite() {
                        return Some(preferred);
                    }
                }
            }
        }

        let mut best: Option<(ObjectId, f32)> = None;

        for &center_id in &self.supply_centers {
            if let Some((cost, _)) = compute_relative_cost(world, query_object_id, center_id) {
                if cost
                    < best
                        .map(|(_, best_cost)| best_cost)
                        .unwrap_or(f32::INFINITY)
                {
                    best = Some((center_id, cost));
                }
            }
        }

        best.map(|(id, _)| id)
    }

    /// Get all supply warehouse IDs
    pub fn get_supply_warehouses(&self) -> &VecDeque<ObjectId> {
        &self.supply_warehouses
    }

    /// Get all supply center IDs
    pub fn get_supply_centers(&self) -> &VecDeque<ObjectId> {
        &self.supply_centers
    }

    /// Get number of supply warehouses
    pub fn get_warehouse_count(&self) -> usize {
        self.supply_warehouses.len()
    }

    /// Get number of supply centers
    pub fn get_center_count(&self) -> usize {
        self.supply_centers.len()
    }

    /// Clear all tracked objects
    pub fn clear(&mut self) {
        self.supply_warehouses.clear();
        self.supply_centers.clear();
    }

    /// Check if we have any supply infrastructure
    pub fn has_supply_infrastructure(&self) -> bool {
        !self.supply_warehouses.is_empty() || !self.supply_centers.is_empty()
    }

    // Private helper methods

    /// Remove invalid object IDs from a collection
    ///
    /// This would check if objects still exist in the game world
    /// and remove any that have been destroyed or are no longer valid.
    fn clean_invalid_objects<W: ResourceWorld>(world: &W, objects: &mut VecDeque<ObjectId>) {
        objects.retain(|id| world.object_exists(*id));
    }
}

fn compute_relative_cost<W: ResourceWorld>(
    world: &W,
    query_id: ObjectId,
    dest_id: ObjectId,
) -> Option<(f32, f32)> {
    if !world.can_transfer_supplies_at(query_id, dest_id) {
        return None;
    }

    if !world.is_clear_to_approach(dest_id, query_id) {
        return None;
    }

    let distance_sq = world.distance_squared(query_id, dest_id)?;
    Some((distance_sq, distance_sq))
}

impl Default for ResourceGatheringManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for ResourceGatheringManager {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|err| err.to_string())?;

        let mut centers: Vec<u32> = match xfer.get_xfer_mode() {
            XferMode::Load => Vec::new(),
            _ => self.supply_centers.iter().copied().collect(),
        };
        let mut warehouses: Vec<u32> = match xfer.get_xfer_mode() {
            XferMode::Load => Vec::new(),
            _ => self.supply_warehouses.iter().copied().collect(),
        };

        xfer.xfer_vec_unsigned_int(&mut centers)
            .map_err(|err| err.to_string())?;
        xfer.xfer_vec_unsigned_int(&mut warehouses)
            .map_err(|err| err.to_string())?;

        if matches!(xfer.get_xfer_mode(), XferMode::Load) {
            self.supply_centers = VecDeque::from(centers);
            self.supply_warehouses = VecDeque::from(warehouses);
        }

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::{HashMap, HashSet};

    struct FakeWorld {
        existing: HashSet<ObjectId>,
        distances: HashMap<(ObjectId, ObjectId), f32>,
        transfer: HashSet<(ObjectId, ObjectId)>,
        clear: HashSet<(ObjectId, ObjectId)>,
        preferred: HashMap<ObjectId, ObjectId>,
        scan_distance: HashMap<ObjectId, f32>,
    }

    impl FakeWorld {
        fn new() -> Self {
            Self {
                existing: HashSet::new(),
                distances: HashMap::new(),
                transfer: HashSet::new(),
                clear: HashSet::new(),
                preferred: HashMap::new(),
                scan_distance: HashMap::new(),
            }
        }

        fn with_object(mut self, id: ObjectId) -> Self {
            self.existing.insert(id);
            self
        }

        fn with_distance(mut self, from: ObjectId, to: ObjectId, distance: f32) -> Self {
            self.distances.insert((from, to), distance * distance);
            self.transfer.insert((from, to));
            self.clear.insert((to, from));
            self
        }

        fn with_preferred(mut self, query: ObjectId, dock: ObjectId) -> Self {
            self.preferred.insert(query, dock);
            self
        }

        fn with_scan_distance(mut self, query: ObjectId, distance: f32) -> Self {
            self.scan_distance.insert(query, distance);
            self
        }

        fn with_blocked_transfer(mut self, from: ObjectId, to: ObjectId) -> Self {
            self.transfer.remove(&(from, to));
            self
        }

        fn with_clearance(mut self, dest: ObjectId, query: ObjectId, is_clear: bool) -> Self {
            if is_clear {
                self.clear.insert((dest, query));
            } else {
                self.clear.remove(&(dest, query));
            }
            self
        }
    }

    impl ResourceWorld for FakeWorld {
        fn object_exists(&self, id: ObjectId) -> bool {
            self.existing.contains(&id)
        }

        fn has_ai(&self, id: ObjectId) -> bool {
            self.existing.contains(&id)
        }

        fn can_transfer_supplies_at(&self, query_id: ObjectId, dest_id: ObjectId) -> bool {
            self.transfer.contains(&(query_id, dest_id))
        }

        fn is_clear_to_approach(&self, dest_id: ObjectId, query_id: ObjectId) -> bool {
            self.clear.contains(&(dest_id, query_id))
        }

        fn distance_squared(&self, query_id: ObjectId, dest_id: ObjectId) -> Option<f32> {
            self.distances.get(&(query_id, dest_id)).copied()
        }

        fn is_supply_warehouse_dock(&self, dock_id: ObjectId) -> bool {
            self.existing.contains(&dock_id)
        }

        fn is_supply_center_dock(&self, dock_id: ObjectId) -> bool {
            self.existing.contains(&dock_id)
        }

        fn preferred_dock(&self, query_id: ObjectId) -> Option<ObjectId> {
            self.preferred.get(&query_id).copied()
        }

        fn warehouse_scan_distance(&self, query_id: ObjectId) -> Option<f32> {
            self.scan_distance.get(&query_id).copied()
        }
    }

    #[test]
    fn manager_tracks_infrastructure() {
        let mut manager = ResourceGatheringManager::new();
        assert!(!manager.has_supply_infrastructure());

        manager.add_supply_center(10);
        manager.add_supply_center(10); // dedup
        manager.add_supply_warehouse(20);

        assert_eq!(manager.get_center_count(), 1);
        assert_eq!(manager.get_warehouse_count(), 1);
        assert!(manager.has_supply_infrastructure());

        manager.remove_supply_center(10);
        manager.remove_supply_warehouse(20);
        assert!(!manager.has_supply_infrastructure());
    }

    #[test]
    fn best_warehouse_respects_cost_and_range() {
        let world = FakeWorld::new()
            .with_object(1)
            .with_object(100)
            .with_object(200)
            .with_distance(1, 100, 60.0)
            .with_distance(1, 200, 15.0)
            .with_scan_distance(1, 50.0);

        let mut manager = ResourceGatheringManager::new();
        manager.add_supply_warehouse(100);
        manager.add_supply_warehouse(200);

        // Warehouse 100 is outside scan range; 200 should be selected.
        assert_eq!(manager.find_best_supply_warehouse(1, &world), Some(200));
    }

    #[test]
    fn preferred_dock_overrides() {
        let world = FakeWorld::new()
            .with_object(1)
            .with_object(300)
            .with_object(400)
            .with_distance(1, 300, 80.0)
            .with_distance(1, 400, 10.0)
            .with_preferred(1, 300);

        let mut manager = ResourceGatheringManager::new();
        manager.add_supply_warehouse(300);
        manager.add_supply_warehouse(400);

        assert_eq!(manager.find_best_supply_warehouse(1, &world), Some(300));
    }

    #[test]
    fn invalid_entries_are_pruned() {
        let world = FakeWorld::new().with_object(1).with_object(900);

        let mut manager = ResourceGatheringManager::new();
        manager.add_supply_warehouse(800); // Does not exist -> removed
        manager.add_supply_warehouse(900);

        assert_eq!(manager.find_best_supply_warehouse(1, &world), Some(900));
        assert_eq!(manager.get_warehouse_count(), 1);
    }

    #[test]
    fn best_center_matches_lowest_cost() {
        let world = FakeWorld::new()
            .with_object(1)
            .with_object(500)
            .with_object(600)
            .with_distance(1, 500, 25.0)
            .with_distance(1, 600, 5.0);

        let mut manager = ResourceGatheringManager::new();
        manager.add_supply_center(500);
        manager.add_supply_center(600);

        assert_eq!(manager.find_best_supply_center(1, &world), Some(600));
    }

    #[test]
    fn center_preferred_override_respected() {
        let world = FakeWorld::new()
            .with_object(1)
            .with_object(700)
            .with_object(710)
            .with_distance(1, 700, 3.0)
            .with_distance(1, 710, 30.0)
            .with_preferred(1, 710);

        let mut manager = ResourceGatheringManager::new();
        manager.add_supply_center(700);
        manager.add_supply_center(710);

        assert_eq!(manager.find_best_supply_center(1, &world), Some(710));
    }

    #[test]
    fn blocked_transfer_and_clearance_skip_candidate() {
        let world = FakeWorld::new()
            .with_object(1)
            .with_object(800)
            .with_object(810)
            .with_distance(1, 800, 5.0)
            .with_distance(1, 810, 7.0)
            .with_blocked_transfer(1, 800)
            .with_clearance(810, 1, false);

        let mut manager = ResourceGatheringManager::new();
        manager.add_supply_center(800);
        manager.add_supply_center(810);

        assert_eq!(manager.find_best_supply_center(1, &world), None);
    }
}
