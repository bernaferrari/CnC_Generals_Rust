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

/// C++ `PATHFIND_CELL_SIZE_F` (`AIPathfind.h:416`).
pub const PATHFIND_CELL_SIZE_F: Real = 10.0;
/// C++ twitch range: `0.4 * PATHFIND_CELL_SIZE_F`.
pub const WAREHOUSE_TWITCH_RANGE: Real = 0.4 * PATHFIND_CELL_SIZE_F;

/// Geometry snapshot for C++ `SupplyWarehouseDockUpdate::action` close-enough.
#[derive(Debug, Clone, Copy)]
pub struct WarehouseDockProximity {
    pub docker_pos: Coord3D,
    pub warehouse_pos: Coord3D,
    pub docker_bounding_circle_radius: Real,
    pub twitch_x: Real,
    pub twitch_y: Real,
}

/// C++ `setDockCrippled` side effect after the latch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DockCrippleSideEffect {
    None,
    /// Docker is inside and must `kill()` unless airborne.
    KillGroundDocker(ObjectID),
    /// Docker is between Approach and Enter: `aiIdle` + `setForceWantingState`.
    RetryDocker(ObjectID),
}

/// C++ close-enough: 2D center distance² vs `(boundingCircle * 2)²`.
pub fn warehouse_close_enough_sqr(docker_bounding_circle_radius: Real) -> Real {
    let diameter = docker_bounding_circle_radius * 2.0;
    diameter * diameter
}

/// Horizontal (C++ XY / host XZ) distance squared.
pub fn warehouse_horizontal_dist_sqr(ax: Real, ay: Real, bx: Real, by: Real) -> Real {
    let dx = ax - bx;
    let dy = ay - by;
    dx * dx + dy * dy
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
    pub fn action(
        &mut self,
        _docker_id: ObjectID,
        proximity: Option<&WarehouseDockProximity>,
    ) -> Result<(bool, Option<Coord3D>), String> {
        if self.boxes_stored == 0 {
            return Ok((false, None));
        }

        if let Some(prox) = proximity {
            let close_enough_sqr = warehouse_close_enough_sqr(prox.docker_bounding_circle_radius);
            let cur_dist_sqr = warehouse_horizontal_dist_sqr(
                prox.docker_pos.x,
                prox.docker_pos.y,
                prox.warehouse_pos.x,
                prox.warehouse_pos.y,
            );
            if cur_dist_sqr > close_enough_sqr {
                let twitched = Coord3D {
                    x: prox.docker_pos.x + prox.twitch_x,
                    y: prox.docker_pos.y + prox.twitch_y,
                    z: prox.docker_pos.z,
                };
                return Ok((false, Some(twitched)));
            }
        }

        // Decrease boxes (docker will see we're shy by one from within gainOneBox)
        self.boxes_stored -= 1;
        Ok((true, None))
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
    pub fn set_dock_crippled(&mut self, crippled: bool) -> DockCrippleSideEffect {
        let effect = if crippled {
            match self.active_docker {
                Some(id) if self.docker_inside => DockCrippleSideEffect::KillGroundDocker(id),
                Some(id) => DockCrippleSideEffect::RetryDocker(id),
                None => DockCrippleSideEffect::None,
            }
        } else {
            DockCrippleSideEffect::None
        };
        self.is_crippled = crippled;
        effect
    }

    pub fn is_crippled(&self) -> bool {
        self.is_crippled
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

