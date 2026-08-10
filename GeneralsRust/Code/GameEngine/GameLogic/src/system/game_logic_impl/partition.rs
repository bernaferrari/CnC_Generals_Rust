/// Partition manager for spatial partitioning (matches C++ PartitionManager grid behavior)
#[derive(Debug, Default)]
pub struct PartitionManager {
    grid: HashMap<(i32, i32), Vec<ObjectID>>,
    object_cells: HashMap<ObjectID, (i32, i32)>,
    object_positions: HashMap<ObjectID, Coord3D>,
    cell_size: Real,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            grid: HashMap::new(),
            object_cells: HashMap::new(),
            object_positions: HashMap::new(),
            cell_size: 100.0,
        }
    }

    /// C++ PartitionManager::crc walks every cell. Host grid is sparse: dump
    /// cell size plus each occupied cell's sorted occupant IDs.
    fn crc_into(&self, xfer: &mut dyn Xfer) {
        let mut cell_size = self.cell_size;
        let _ = xfer.xfer_real(&mut cell_size);
        let mut total = self.object_cells.len() as i32;
        let _ = xfer.xfer_int(&mut total);
        let mut cells: Vec<(i32, i32, ObjectID)> = self
            .object_cells
            .iter()
            .map(|(&id, &(x, y))| (x, y, id))
            .collect();
        cells.sort_unstable();
        for (mut x, mut y, mut id) in cells {
            let _ = xfer.xfer_int(&mut x);
            let _ = xfer.xfer_int(&mut y);
            let _ = xfer.xfer_unsigned_int(&mut id);
        }
    }

    /// Find objects within radius of position (2D X/Y distance).
    pub fn find_objects_in_radius(&self, center: Coord3D, radius: Real) -> Vec<ObjectID> {
        let mut result = Vec::new();
        let radius_squared = radius * radius;

        let min_cell =
            self.position_to_cell([center.x - radius, center.y - radius, center.z].into());
        let max_cell =
            self.position_to_cell([center.x + radius, center.y + radius, center.z].into());

        for x in min_cell.0..=max_cell.0 {
            for y in min_cell.1..=max_cell.1 {
                if let Some(objects) = self.grid.get(&(x, y)) {
                    for &object_id in objects {
                        let Some(pos) = self.object_positions.get(&object_id) else {
                            continue;
                        };
                        let dx = pos.x - center.x;
                        let dy = pos.y - center.y;
                        if dx * dx + dy * dy <= radius_squared {
                            result.push(object_id);
                        }
                    }
                }
            }
        }

        result
    }

    pub fn update(&mut self) -> Result<(), GameLogicError> {
        // Host path: OBJECT_REGISTRY is empty; cells were filled via register_object.
        // Keep cached positions — C++ PartitionManager still tracks live objects.
        if dual_world_registry_unavailable() {
            return Ok(());
        }
        let object_ids = OBJECT_REGISTRY.get_all_object_ids();
        let mut seen = HashSet::with_capacity(object_ids.len());

        for obj_id in &object_ids {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(*obj_id) else {
                continue;
            };
            let Ok(obj) = obj_arc.read() else {
                continue;
            };
            let id = obj.get_id();
            let pos = obj.get_position();
            self.add_object(id, (pos.x, pos.y, pos.z));
            seen.insert(id);
        }

        let stale: Vec<ObjectID> = self
            .object_positions
            .keys()
            .copied()
            .filter(|id| !seen.contains(id))
            .collect();
        for id in stale {
            self.remove_object(id);
        }
        Ok(())
    }

    pub fn add_object(&mut self, object_id: ObjectID, position: (f32, f32, f32)) {
        let pos = Coord3D::new(position.0, position.1, position.2);
        let cell = self.position_to_cell(pos);

        if let Some(old_cell) = self.object_cells.get(&object_id) {
            if let Some(objects) = self.grid.get_mut(old_cell) {
                objects.retain(|&id| id != object_id);
            }
        }

        self.grid
            .entry(cell)
            .or_insert_with(Vec::new)
            .push(object_id);
        self.object_cells.insert(object_id, cell);
        self.object_positions.insert(object_id, pos);
    }

    pub fn remove_object(&mut self, object_id: ObjectID) {
        self.object_positions.remove(&object_id);
        if let Some(cell) = self.object_cells.remove(&object_id) {
            if let Some(objects) = self.grid.get_mut(&cell) {
                objects.retain(|&id| id != object_id);
                if objects.is_empty() {
                    self.grid.remove(&cell);
                }
            }
        }
    }

    /// Rebuild the spatial partition index
    /// Used after loading a saved game to reconstruct spatial data
    pub fn rebuild(&mut self) {
        if dual_world_registry_unavailable() {
            // Re-grid from cached positions instead of wiping the host-path index.
            let snapshot: Vec<(ObjectID, Coord3D)> = self
                .object_positions
                .iter()
                .map(|(id, pos)| (*id, *pos))
                .collect();
            self.grid.clear();
            self.object_cells.clear();
            self.object_positions.clear();
            for (id, pos) in snapshot {
                self.add_object(id, (pos.x, pos.y, pos.z));
            }
            return;
        }

        self.grid.clear();
        self.object_cells.clear();
        self.object_positions.clear();

        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            let Some((id, xyz)) = OBJECT_REGISTRY.with_object(obj_id, |obj| {
                let pos = obj.get_position();
                (obj.get_id(), (pos.x, pos.y, pos.z))
            }) else {
                continue;
            };
            self.add_object(id, xyz);
        }
    }

    /// Register an object at a specific position
    /// Used during save game restoration
    pub fn register_object(&mut self, object_id: ObjectID, x: f32, y: f32) {
        self.add_object(object_id, (x, y, 0.0));
    }

    fn position_to_cell(&self, position: Coord3D) -> (i32, i32) {
        let x = (position.x / self.cell_size).floor() as i32;
        let y = (position.y / self.cell_size).floor() as i32;
        (x, y)
    }
}
