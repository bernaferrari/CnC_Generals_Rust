//! TerrainLogic waypoint behavior.

use super::*;

impl TerrainLogic {
    /// Get first waypoint
    pub fn get_first_waypoint(&self) -> Option<&Waypoint> {
        self.waypoint_list_head.as_ref().map(|w| w.as_ref())
    }

    /// Get waypoint by name
    pub fn get_waypoint_by_name(&self, name: &AsciiString) -> Option<&Waypoint> {
        let mut current = self.waypoint_list_head.as_deref();
        while let Some(waypoint) = current {
            if waypoint.get_name() == name {
                return Some(waypoint);
            }
            current = waypoint.next.as_deref();
        }
        None
    }

    /// Get waypoint by ID
    pub fn get_waypoint_by_id(&self, id: WaypointID) -> Option<&Waypoint> {
        let mut current = self.waypoint_list_head.as_deref();
        while let Some(waypoint) = current {
            if waypoint.get_id() == id {
                return Some(waypoint);
            }
            current = waypoint.next.as_deref();
        }
        None
    }

    /// Walk `get_link(0)` from `start` with a visited-set and hop cap.
    ///
    /// C++ camera (`W3DView::moveCameraAlongWaypointPath`) stops at
    /// `MAX_WAYPOINTS`; C++ `AIUpdateInterface::setPathFromWaypoint` stops at
    /// `WAYPOINT_PATH_LIMIT`. A 1-link ring (ShellMapMD `Car_Path`) revisits
    /// an id and must not hang the Menu ScriptEngine frame.
    pub fn walk_link0_chain<'a>(
        &'a self,
        start: &'a Waypoint,
        max_hops: usize,
    ) -> Vec<&'a Waypoint> {
        let mut out = Vec::new();
        let mut visited = HashSet::new();
        let mut current = Some(start);
        let limit = max_hops.max(1);
        while let Some(node) = current {
            if out.len() >= limit || !visited.insert(node.get_id()) {
                break;
            }
            out.push(node);
            current = node.get_link(0).and_then(|id| self.get_waypoint_by_id(id));
        }
        out
    }

    /// Get closest waypoint that matches a path label
    pub fn get_closest_waypoint_on_path(&self, pos: &Coord3D, label: &str) -> Option<&Waypoint> {
        let mut current = self.waypoint_list_head.as_deref();
        let mut best: Option<&Waypoint> = None;
        let mut best_dist_sqr = f32::MAX;

        while let Some(waypoint) = current {
            if waypoint.matches_path_label(label) {
                let dx = waypoint.location.x - pos.x;
                let dy = waypoint.location.y - pos.y;
                let dist_sqr = dx * dx + dy * dy;
                if dist_sqr < best_dist_sqr {
                    best_dist_sqr = dist_sqr;
                    best = Some(waypoint);
                }
            }
            current = waypoint.next.as_deref();
        }

        best
    }

    pub(super) fn get_waypoint_by_id_mut(&mut self, id: WaypointID) -> Option<&mut Waypoint> {
        let mut current = self.waypoint_list_head.as_deref_mut();
        while let Some(waypoint) = current {
            if waypoint.get_id() == id {
                return Some(waypoint);
            }
            current = waypoint.next.as_deref_mut();
        }
        None
    }

    pub(super) fn add_waypoint_from_map(&mut self, waypoint: &MapWaypoint) {
        let mut location = Coord3D::new(
            waypoint.location.x,
            waypoint.location.y,
            waypoint.location.z,
        );
        location.z = self.get_ground_height(location.x, location.y, None);
        let new_waypoint = Waypoint::new(
            waypoint.id,
            AsciiString::from(waypoint.name.as_str()),
            &location,
            AsciiString::from(waypoint.path_label1.as_str()),
            AsciiString::from(waypoint.path_label2.as_str()),
            AsciiString::from(waypoint.path_label3.as_str()),
            waypoint.bi_directional,
        );

        let mut boxed = Box::new(new_waypoint);
        boxed.next = self.waypoint_list_head.take();
        self.waypoint_list_head = Some(boxed);
    }

    pub(super) fn add_waypoint_link(&mut self, id1: WaypointID, id2: WaypointID) {
        if id1 == id2 {
            return;
        }

        let should_link_back = {
            let Some(way1) = self.get_waypoint_by_id_mut(id1) else {
                return;
            };
            if !way1.has_link(id2) {
                way1.add_link(id2);
            }
            way1.get_bi_directional()
        };

        if should_link_back {
            if let Some(way2) = self.get_waypoint_by_id_mut(id2) {
                if !way2.has_link(id1) {
                    way2.add_link(id1);
                }
            }
        }
    }

    // ============================================================================
    // TRIGGER AREA METHODS
    // Matches C++ ThePolygonTriggerListPtr interface
    // ============================================================================

    /// Get trigger area by name
    /// Matches C++ ThePolygonTriggerListPtr->getPolygonTriggerByName
    pub fn get_trigger_area_by_name(&self, name: &str) -> Option<&PolygonTrigger> {
        self.trigger_areas.get_by_name(name)
    }

    /// Get mutable trigger area by name
    pub fn get_trigger_area_by_name_mut(&mut self, name: &str) -> Option<&mut PolygonTrigger> {
        self.trigger_areas.get_by_name_mut(name)
    }

    /// Get all trigger areas
    pub fn get_trigger_areas(&self) -> &PolygonTriggerList {
        &self.trigger_areas
    }

    /// Get mutable trigger areas list
    pub fn get_trigger_areas_mut(&mut self) -> &mut PolygonTriggerList {
        &mut self.trigger_areas
    }

    /// Add a trigger area
    pub fn add_trigger_area(&mut self, trigger: PolygonTrigger) {
        let trigger_name_ascii = trigger.get_trigger_name().clone();
        let trigger_name = trigger_name_ascii.to_string();

        if trigger.is_water_area() {
            let trigger_id = trigger.get_id();
            let water_height = trigger
                .get_point(0)
                .map(|point| point.z as f32)
                .unwrap_or(self.grid_water_handle.get_current_height());
            let bounds = trigger.get_bounds();
            let water_bounds = Region3D::new(
                Coord3D::new(bounds.lo.x as f32, bounds.lo.y as f32, water_height),
                Coord3D::new(bounds.hi.x as f32, bounds.hi.y as f32, water_height),
            );
            let handle = WaterHandle::new(trigger_name_ascii.clone(), water_height, water_bounds);
            self.water_handles_by_trigger_id
                .entry(trigger_id)
                .or_insert(handle.clone());
            self.water_handles
                .entry(trigger_name_ascii.clone())
                .or_insert(handle);
        }

        self.trigger_areas.add(trigger);

        let area_tracker = crate::scripting::engine::get_area_tracker();
        if let Err(err) = area_tracker.register_polygon_area(&trigger_name) {
            log::warn!(
                "Failed to register polygon trigger area '{}' with script tracker: {}",
                trigger_name,
                err
            );
        }
    }
}
