// ============================================================================
// LOCOMOTOR INSTANCE
// ============================================================================

/// Active path being followed
#[derive(Debug, Clone)]
pub struct ActivePath {
    /// Full path waypoints
    pub waypoints: Vec<Coord3D>,
    /// Layer per waypoint
    pub layers: Vec<PathfindLayerEnum>,
    /// Current waypoint index
    pub current_waypoint: usize,
    /// Distance remaining to current waypoint
    pub distance_to_waypoint: Real,
    /// Total path distance
    pub total_distance: Real,
    /// Distance traveled so far
    pub distance_traveled: Real,
    /// Path start frame
    pub start_frame: u32,
}

impl ActivePath {
    /// Create new active path
    pub fn new(waypoints: Vec<Coord3D>, start_frame: u32) -> Self {
        let layers = vec![PathfindLayerEnum::Ground; waypoints.len()];
        Self::new_with_layers(waypoints, layers, start_frame)
    }

    /// Create new active path with explicit layers per waypoint.
    pub fn new_with_layers(
        waypoints: Vec<Coord3D>,
        layers: Vec<PathfindLayerEnum>,
        start_frame: u32,
    ) -> Self {
        let total_distance = Self::calculate_path_distance(&waypoints);
        let distance_to_waypoint = if waypoints.len() >= 2 {
            (waypoints[1] - waypoints[0]).length()
        } else {
            0.0
        };

        let mut layers = layers;
        if layers.len() != waypoints.len() {
            layers.resize(waypoints.len(), PathfindLayerEnum::Ground);
        }

        Self {
            waypoints,
            layers,
            current_waypoint: 0,
            distance_to_waypoint,
            total_distance,
            distance_traveled: 0.0,
            start_frame,
        }
    }

    /// Calculate total path distance
    fn calculate_path_distance(waypoints: &[Coord3D]) -> Real {
        let mut total = 0.0;
        for i in 1..waypoints.len() {
            total += (waypoints[i] - waypoints[i - 1]).length();
        }
        total
    }

    /// Get current target waypoint
    pub fn current_target(&self) -> Option<Coord3D> {
        if self.current_waypoint < self.waypoints.len() {
            Some(self.waypoints[self.current_waypoint])
        } else {
            None
        }
    }

    pub fn current_layer(&self) -> Option<PathfindLayerEnum> {
        if self.current_waypoint < self.layers.len() {
            Some(self.layers[self.current_waypoint])
        } else {
            None
        }
    }

    /// Get next waypoint after current
    pub fn next_waypoint(&self) -> Option<Coord3D> {
        if self.current_waypoint + 1 < self.waypoints.len() {
            Some(self.waypoints[self.current_waypoint + 1])
        } else {
            None
        }
    }

    /// Advance to next waypoint
    pub fn advance_waypoint(&mut self) -> bool {
        if self.current_waypoint + 1 < self.waypoints.len() {
            self.distance_traveled += self.distance_to_waypoint;
            self.current_waypoint += 1;

            if self.current_waypoint + 1 < self.waypoints.len() {
                self.distance_to_waypoint = (self.waypoints[self.current_waypoint + 1]
                    - self.waypoints[self.current_waypoint])
                    .length();
            } else {
                self.distance_to_waypoint = 0.0;
            }
            true
        } else {
            false
        }
    }

    /// Get distance remaining on path
    pub fn distance_remaining(&self) -> Real {
        self.total_distance - self.distance_traveled - self.distance_to_waypoint
    }

    /// Check if path is complete
    pub fn is_complete(&self) -> bool {
        self.current_waypoint + 1 >= self.waypoints.len()
    }

    /// Get number of waypoints
    pub fn waypoint_count(&self) -> usize {
        self.waypoints.len()
    }

    /// Append a waypoint to the active path and update distance totals.
    pub fn append_waypoint(&mut self, waypoint: Coord3D) {
        if let Some(last) = self.waypoints.last().copied() {
            let delta = (waypoint - last).length();
            self.total_distance += delta;
            if self.current_waypoint + 1 >= self.waypoints.len() {
                self.distance_to_waypoint = delta;
            }
        } else {
            self.total_distance = 0.0;
            self.distance_to_waypoint = 0.0;
        }
        self.waypoints.push(waypoint);
        self.layers.push(PathfindLayerEnum::Ground);
    }

    /// Update the last waypoint and recompute path distance.
    pub fn set_last_waypoint(&mut self, waypoint: Coord3D) {
        if let Some(last) = self.waypoints.last_mut() {
            *last = waypoint;
            self.total_distance = Self::calculate_path_distance(&self.waypoints);
            if self.current_waypoint + 1 < self.waypoints.len() {
                self.distance_to_waypoint = (self.waypoints[self.current_waypoint + 1]
                    - self.waypoints[self.current_waypoint])
                    .length();
            } else {
                self.distance_to_waypoint = 0.0;
            }
        }
    }
}

