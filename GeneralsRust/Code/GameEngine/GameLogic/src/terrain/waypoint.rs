//! Terrain waypoint and trigger admission adapters.

use super::*;

impl TerrainLogic {
    pub fn waypoints(&self) -> &WaypointGraph<AsciiString> {
        &self.waypoints
    }

    pub fn get_first_waypoint(&self) -> Option<&Waypoint> {
        self.waypoints.first()
    }

    pub fn get_waypoint_by_name(&self, name: &AsciiString) -> Option<&Waypoint> {
        self.waypoints.by_name(name.as_str())
    }

    pub fn get_waypoint_by_id(&self, id: WaypointID) -> Option<&Waypoint> {
        self.waypoints.by_id(id)
    }

    pub fn walk_link0_chain<'a>(&'a self, start: &'a Waypoint, max_hops: usize) -> Vec<&'a Waypoint> {
        self.waypoints.walk_link0_chain(start, max_hops)
    }

    pub fn get_closest_waypoint_on_path(&self, pos: &Coord3D, label: &str) -> Option<&Waypoint> {
        self.waypoints.closest_on_path(pos, label)
    }

    pub fn bind_wave_guide1(&self) -> WaveGuide1Bind {
        self.waypoints.bind_wave_guide1()
    }

    #[cfg(test)]
    pub(super) fn get_waypoint_by_id_mut(&mut self, id: WaypointID) -> Option<&mut Waypoint> {
        self.waypoints.by_id_mut(id)
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

        self.waypoints.prepend(new_waypoint);
    }

    pub(super) fn add_waypoint_link(&mut self, from: WaypointID, to: WaypointID) {
        self.waypoints.add_link(from, to);
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

    /// Replace this map's ordered trigger table and its derived water handles.
    /// PolygonTrigger.cpp:111 clears the old list before appending every row.
    /// Duplicate name/ID lookups keep the first row; no name-based filtering.
    pub fn replace_trigger_areas(&mut self, triggers: impl IntoIterator<Item = PolygonTrigger>) {
        self.replace_trigger_areas_with_registration(triggers, true);
    }

    /// Main owns the polygon table through its receiving TerrainLogic. No
    /// standalone scripting tracker is published during candidate admission.
    pub fn replace_receiving_trigger_areas(
        &mut self,
        triggers: impl IntoIterator<Item = PolygonTrigger>,
    ) {
        self.replace_trigger_areas_with_registration(triggers, false);
    }

    fn replace_trigger_areas_with_registration(
        &mut self,
        triggers: impl IntoIterator<Item = PolygonTrigger>,
        standalone: bool,
    ) {
        self.trigger_areas.clear();
        self.water_handles.clear();
        self.water_handles_by_trigger_id.clear();
        for trigger in triggers {
            self.add_trigger_area_with_registration(trigger, standalone);
        }
    }

    /// Add a trigger area
    pub fn add_trigger_area(&mut self, trigger: PolygonTrigger) {
        self.add_trigger_area_with_registration(trigger, true);
    }
    fn add_trigger_area_with_registration(&mut self, trigger: PolygonTrigger, standalone: bool) {
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

        if !standalone {
            return;
        }
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
