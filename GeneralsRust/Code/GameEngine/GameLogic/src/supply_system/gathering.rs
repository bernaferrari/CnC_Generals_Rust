// ============================================================================
// RESOURCE GATHERING MANAGER
// ============================================================================

/// Part of a Player's brain that tracks supply warehouses / centers and
/// picks gather / return docks for trucks.
///
/// Source: `GeneralsMD/.../Common/RTS/ResourceGatheringManager.{h,cpp}`.
/// Owned by `Player` (C++ `Player::m_resourceGatheringManager`); never a
/// process-wide service.
#[derive(Debug)]
pub struct ResourceGatheringManager {
    /// C++ `m_supplyWarehouses` (`std::list<ObjectID>`).
    supply_warehouses: Vec<ObjectID>,
    /// C++ `m_supplyCenters` (`std::list<ObjectID>`).
    supply_centers: Vec<ObjectID>,
}

/// C++ `findBestSupplyWarehouse` seeds `maxDistanceSquared = 100000` before the
/// supply-truck AI optionally replaces it with `scan * scan`.
pub const DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED: Real = 100_000.0;

impl ResourceGatheringManager {
    pub fn new() -> Self {
        Self {
            supply_warehouses: Vec::new(),
            supply_centers: Vec::new(),
        }
    }

    /// C++ `ResourceGatheringManager::addSupplyCenter` — appends without dedup.
    pub fn add_supply_center(&mut self, center_id: ObjectID) {
        self.supply_centers.push(center_id);
    }

    /// C++ `ResourceGatheringManager::removeSupplyCenter`.
    pub fn remove_supply_center(&mut self, center_id: ObjectID) {
        self.supply_centers.retain(|&id| id != center_id);
    }

    /// C++ `ResourceGatheringManager::addSupplyWarehouse` — appends without dedup.
    pub fn add_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        self.supply_warehouses.push(warehouse_id);
    }

    /// C++ `ResourceGatheringManager::removeSupplyWarehouse`.
    pub fn remove_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        self.supply_warehouses.retain(|&id| id != warehouse_id);
    }

    /// Drop IDs whose live object is gone (C++ erases NULL `findObjectByID`
    /// results while walking the list during find).
    pub fn retain_existing_warehouses(&mut self, exists: impl Fn(ObjectID) -> bool) {
        self.supply_warehouses.retain(|&id| exists(id));
    }

    pub fn retain_existing_centers(&mut self, exists: impl Fn(ObjectID) -> bool) {
        self.supply_centers.retain(|&id| exists(id));
    }

    /// Pure dock scoring once the live world has answered transfer / approach /
    /// position. Mirrors C++ `computeRelativeCost` + the warehouse scan loop.
    ///
    /// `preferred_ok` is true only when the call site already verified the
    /// preferred dock has a warehouse module and finite relative cost (C++ does
    /// not require the preferred ID to already be on `m_supplyWarehouses`).
    pub fn find_best_supply_warehouse(
        &self,
        truck_position: &Coord3D,
        preferred_dock: Option<ObjectID>,
        preferred_ok: bool,
        max_distance_squared: Real,
        warehouse_positions: &HashMap<ObjectID, Coord3D>,
        warehouse_available: &HashMap<ObjectID, bool>,
    ) -> Option<ObjectID> {
        if preferred_ok && let Some(preferred) = preferred_dock {
            return Some(preferred);
        }

        let mut best_warehouse = None;
        let mut best_cost = Real::MAX;

        for &warehouse_id in &self.supply_warehouses {
            if warehouse_available.get(&warehouse_id) != Some(&true) {
                continue;
            }
            let Some(warehouse_pos) = warehouse_positions.get(&warehouse_id) else {
                continue;
            };
            let distance_squared = truck_position.distance_squared_to(warehouse_pos);
            if distance_squared < best_cost && distance_squared < max_distance_squared {
                best_warehouse = Some(warehouse_id);
                best_cost = distance_squared;
            }
        }

        best_warehouse
    }

    /// Pure center scoring. Preferred dock may win without already being listed
    /// (C++ checks `SupplyCenterDockUpdate` on the preferred object).
    pub fn find_best_supply_center(
        &self,
        truck_position: &Coord3D,
        preferred_dock: Option<ObjectID>,
        preferred_ok: bool,
        center_positions: &HashMap<ObjectID, Coord3D>,
        center_available: &HashMap<ObjectID, bool>,
    ) -> Option<ObjectID> {
        if preferred_ok && let Some(preferred) = preferred_dock {
            return Some(preferred);
        }

        let mut best_center = None;
        let mut best_cost = Real::MAX;

        for &center_id in &self.supply_centers {
            if center_available.get(&center_id) != Some(&true) {
                continue;
            }
            let Some(center_pos) = center_positions.get(&center_id) else {
                continue;
            };
            let distance_squared = truck_position.distance_squared_to(center_pos);
            if distance_squared < best_cost {
                best_center = Some(center_id);
                best_cost = distance_squared;
            }
        }

        best_center
    }

    pub fn get_supply_warehouses(&self) -> &[ObjectID] {
        &self.supply_warehouses
    }

    pub fn get_supply_centers(&self) -> &[ObjectID] {
        &self.supply_centers
    }

    pub fn warehouse_count(&self) -> usize {
        self.supply_warehouses.len()
    }

    pub fn center_count(&self) -> usize {
        self.supply_centers.len()
    }

    pub fn clear(&mut self) {
        self.supply_warehouses.clear();
        self.supply_centers.clear();
    }
}

impl Default for ResourceGatheringManager {
    fn default() -> Self {
        Self::new()
    }
}

impl Snapshotable for ResourceGatheringManager {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        // C++ ResourceGatheringManager::crc is intentionally empty.
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let xfer_io = |res: std::io::Result<()>| res.map_err(|err| err.to_string());

        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer_io(xfer.xfer_version(&mut version, current_version))?;

        if xfer.is_reading() {
            self.supply_warehouses.clear();
            self.supply_centers.clear();
        }

        xfer_io(xfer.xfer_stl_object_id_list(&mut self.supply_warehouses))?;
        xfer_io(xfer.xfer_stl_object_id_list(&mut self.supply_centers))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // C++ ResourceGatheringManager::loadPostProcess is intentionally empty.
        Ok(())
    }
}

#[cfg(test)]
mod resource_gathering_manager_unit_tests {
    use super::*;

    fn pos(x: Real, y: Real) -> Coord3D {
        Coord3D::new(x, y, 0.0)
    }

    #[test]
    fn add_allows_duplicate_ids_like_cpp() {
        let mut mgr = ResourceGatheringManager::new();
        mgr.add_supply_center(10);
        mgr.add_supply_center(10);
        mgr.add_supply_warehouse(20);
        assert_eq!(mgr.center_count(), 2);
        assert_eq!(mgr.warehouse_count(), 1);
        mgr.remove_supply_center(10);
        assert_eq!(mgr.center_count(), 0);
    }

    #[test]
    fn warehouse_respects_cost_and_scan_range() {
        let mgr = {
            let mut m = ResourceGatheringManager::new();
            m.add_supply_warehouse(100);
            m.add_supply_warehouse(200);
            m
        };
        let truck = pos(0.0, 0.0);
        let positions = HashMap::from([(100, pos(60.0, 0.0)), (200, pos(15.0, 0.0))]);
        let available = HashMap::from([(100, true), (200, true)]);
        // scan distance 50 → max sq 2500; 60 is out, 15 wins.
        let got = mgr.find_best_supply_warehouse(
            &truck,
            None,
            false,
            50.0 * 50.0,
            &positions,
            &available,
        );
        assert_eq!(got, Some(200));
    }

    #[test]
    fn warehouse_scan_boundary_is_strict_less_than() {
        let mgr = {
            let mut m = ResourceGatheringManager::new();
            m.add_supply_warehouse(100);
            m.add_supply_warehouse(200);
            m
        };
        let truck = pos(0.0, 0.0);
        let positions = HashMap::from([(100, pos(50.0, 0.0)), (200, pos(49.0, 0.0))]);
        let available = HashMap::from([(100, true), (200, true)]);
        let got = mgr.find_best_supply_warehouse(
            &truck,
            None,
            false,
            50.0 * 50.0,
            &positions,
            &available,
        );
        assert_eq!(got, Some(200));
    }

    #[test]
    fn preferred_warehouse_wins_without_list_membership() {
        let mgr = ResourceGatheringManager::new();
        let truck = pos(0.0, 0.0);
        let positions = HashMap::new();
        let available = HashMap::new();
        let got = mgr.find_best_supply_warehouse(
            &truck,
            Some(300),
            true,
            DEFAULT_WAREHOUSE_MAX_DISTANCE_SQUARED,
            &positions,
            &available,
        );
        assert_eq!(got, Some(300));
    }

    #[test]
    fn unavailable_or_missing_maps_are_skipped() {
        let mgr = {
            let mut m = ResourceGatheringManager::new();
            m.add_supply_center(800);
            m.add_supply_center(810);
            m
        };
        let truck = pos(0.0, 0.0);
        let positions = HashMap::from([(800, pos(5.0, 0.0)), (810, pos(7.0, 0.0))]);
        let available = HashMap::from([(800, false), (810, false)]);
        assert_eq!(
            mgr.find_best_supply_center(&truck, None, false, &positions, &available),
            None
        );
    }

    #[test]
    fn best_center_is_lowest_distance_cost() {
        let mgr = {
            let mut m = ResourceGatheringManager::new();
            m.add_supply_center(500);
            m.add_supply_center(600);
            m
        };
        let truck = pos(0.0, 0.0);
        let positions = HashMap::from([(500, pos(25.0, 0.0)), (600, pos(5.0, 0.0))]);
        let available = HashMap::from([(500, true), (600, true)]);
        assert_eq!(
            mgr.find_best_supply_center(&truck, None, false, &positions, &available),
            Some(600)
        );
    }

    #[test]
    fn retain_existing_prunes_dead_ids() {
        let mut mgr = ResourceGatheringManager::new();
        mgr.add_supply_warehouse(1);
        mgr.add_supply_warehouse(2);
        mgr.retain_existing_warehouses(|id| id == 2);
        assert_eq!(mgr.get_supply_warehouses(), &[2]);
    }
}
