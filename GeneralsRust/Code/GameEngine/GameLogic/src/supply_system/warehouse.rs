// ============================================================================
// SUPPLY WAREHOUSE DOCK UPDATE
// ============================================================================

/// Supply warehouse dock update module data
/// Matches C++ SupplyWarehouseDockUpdateModuleData
#[derive(Debug, Clone)]
pub struct SupplyWarehouseDockUpdateData {
    /// Number of supply boxes to start with
    pub starting_boxes: i32,
    /// Whether to delete the warehouse when empty
    pub delete_when_empty: bool,
    /// Number of approach positions
    pub num_approaches: usize,
    /// Action delay time in frames
    pub action_delay: u32,
}

impl Default for SupplyWarehouseDockUpdateData {
    fn default() -> Self {
        Self {
            starting_boxes: 1,
            delete_when_empty: false,
            num_approaches: 3,
            action_delay: 30,
        }
    }
}

/// Supply warehouse dock update module
/// Matches C++ SupplyWarehouseDockUpdate
#[derive(Debug)]
pub struct SupplyWarehouseDockUpdate {
    /// Configuration data
    data: SupplyWarehouseDockUpdateData,
    /// Current number of boxes stored
    boxes_stored: i32,
    /// Currently docked object
    active_docker: Option<ObjectID>,
    /// Whether docker is inside the warehouse
    docker_inside: bool,
    /// Whether dock is crippled
    is_crippled: bool,
}

impl SupplyWarehouseDockUpdate {
    pub fn new(data: SupplyWarehouseDockUpdateData) -> Self {
        let boxes_stored = data.starting_boxes;
        Self {
            data,
            boxes_stored,
            active_docker: None,
            docker_inside: false,
            is_crippled: false,
        }
    }

    /// Perform dock action - give boxes to truck
    /// Matches C++ SupplyWarehouseDockUpdate::action()
    pub fn action(&mut self, _docker_id: ObjectID) -> Result<bool, String> {
        if self.boxes_stored == 0 {
            return Ok(false);
        }

        // Decrease boxes (docker will see we're shy by one from within gainOneBox)
        self.boxes_stored -= 1;

        // Return true if truck successfully gained the box
        // The truck will call gainOneBox() to actually take it
        Ok(true)
    }

    /// Give one box to a truck
    /// Called by truck AI after action() succeeds
    pub fn give_box(&mut self) -> Option<i32> {
        if self.boxes_stored >= 0 {
            let remaining = self.boxes_stored;
            Some(remaining)
        } else {
            // Take it back if no one gained it
            self.boxes_stored += 1;
            None
        }
    }

    /// Set the cash value and calculate boxes needed
    /// Matches C++ SupplyWarehouseDockUpdate::setCashValue()
    pub fn set_cash_value(&mut self, cash_value: i32) {
        self.boxes_stored = (cash_value as f32 / BASE_VALUE_PER_SUPPLY_BOX as f32).ceil() as i32;
    }

    /// Set dock crippled state
    /// Matches C++ SupplyWarehouseDockUpdate::setDockCrippled()
    pub fn set_dock_crippled(&mut self, crippled: bool) {
        self.is_crippled = crippled;

        if crippled && self.active_docker.is_some() {
            // If docker is inside, kill it (handled by game logic)
            // If between approach and enter, tell it to stop and retry later
            // This is handled by the AI system
        }
    }

    pub fn get_boxes_stored(&self) -> i32 {
        self.boxes_stored
    }

    pub fn is_empty(&self) -> bool {
        self.boxes_stored == 0
    }

    pub fn should_delete_when_empty(&self) -> bool {
        self.data.delete_when_empty
    }

    pub fn set_active_docker(&mut self, docker_id: Option<ObjectID>, inside: bool) {
        self.active_docker = docker_id;
        self.docker_inside = inside;
    }
}

