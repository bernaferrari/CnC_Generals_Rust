// ============================================================================
// RESOURCE GATHERING MANAGER
// ============================================================================

/// Manages supply warehouses and centers for a player
/// Matches C++ ResourceGatheringManager.cpp
#[derive(Debug)]
pub struct ResourceGatheringManager {
    /// List of supply warehouse IDs
    supply_warehouses: Vec<ObjectID>,
    /// List of supply center IDs
    supply_centers: Vec<ObjectID>,
}

impl ResourceGatheringManager {
    pub fn new() -> Self {
        Self {
            supply_warehouses: Vec::new(),
            supply_centers: Vec::new(),
        }
    }

    /// Add a supply center
    /// Matches C++ ResourceGatheringManager::addSupplyCenter()
    pub fn add_supply_center(&mut self, center_id: ObjectID) {
        if !self.supply_centers.contains(&center_id) {
            self.supply_centers.push(center_id);
        }
    }

    /// Remove a supply center
    /// Matches C++ ResourceGatheringManager::removeSupplyCenter()
    pub fn remove_supply_center(&mut self, center_id: ObjectID) {
        self.supply_centers.retain(|&id| id != center_id);
    }

    /// Add a supply warehouse
    /// Matches C++ ResourceGatheringManager::addSupplyWarehouse()
    pub fn add_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        if !self.supply_warehouses.contains(&warehouse_id) {
            self.supply_warehouses.push(warehouse_id);
        }
    }

    /// Remove a supply warehouse
    /// Matches C++ ResourceGatheringManager::removeSupplyWarehouse()
    pub fn remove_supply_warehouse(&mut self, warehouse_id: ObjectID) {
        self.supply_warehouses.retain(|&id| id != warehouse_id);
    }

    /// Find best supply warehouse for a truck
    /// Matches C++ ResourceGatheringManager::findBestSupplyWarehouse()
    pub fn find_best_supply_warehouse(
        &self,
        truck_position: &Coord3D,
        preferred_dock: Option<ObjectID>,
        max_distance: Real,
        warehouse_positions: &HashMap<ObjectID, Coord3D>,
        warehouse_available: &HashMap<ObjectID, bool>,
    ) -> Option<ObjectID> {
        // Check preferred dock first
        if let Some(preferred) = preferred_dock {
            if self.supply_warehouses.contains(&preferred) {
                if let Some(&available) = warehouse_available.get(&preferred) {
                    if available {
                        return Some(preferred);
                    }
                }
            }
        }

        // Find best warehouse by distance
        let max_distance_squared = max_distance * max_distance;
        let mut best_warehouse = None;
        let mut best_cost = Real::MAX;

        for &warehouse_id in &self.supply_warehouses {
            if let Some(&available) = warehouse_available.get(&warehouse_id) {
                if !available {
                    continue;
                }
            }

            if let Some(warehouse_pos) = warehouse_positions.get(&warehouse_id) {
                let distance_squared = truck_position.distance_squared_to(warehouse_pos);

                if distance_squared < best_cost && distance_squared < max_distance_squared {
                    best_warehouse = Some(warehouse_id);
                    best_cost = distance_squared;
                }
            }
        }

        best_warehouse
    }

    /// Find best supply center for a truck
    /// Matches C++ ResourceGatheringManager::findBestSupplyCenter()
    pub fn find_best_supply_center(
        &self,
        truck_position: &Coord3D,
        preferred_dock: Option<ObjectID>,
        center_positions: &HashMap<ObjectID, Coord3D>,
        center_available: &HashMap<ObjectID, bool>,
    ) -> Option<ObjectID> {
        // Check preferred dock first
        if let Some(preferred) = preferred_dock {
            if self.supply_centers.contains(&preferred) {
                if let Some(&available) = center_available.get(&preferred) {
                    if available {
                        return Some(preferred);
                    }
                }
            }
        }

        // Find best center by distance (no max distance limit for centers)
        let mut best_center = None;
        let mut best_cost = Real::MAX;

        for &center_id in &self.supply_centers {
            if let Some(&available) = center_available.get(&center_id) {
                if !available {
                    continue;
                }
            }

            if let Some(center_pos) = center_positions.get(&center_id) {
                let distance_squared = truck_position.distance_squared_to(center_pos);

                if distance_squared < best_cost {
                    best_center = Some(center_id);
                    best_cost = distance_squared;
                }
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

