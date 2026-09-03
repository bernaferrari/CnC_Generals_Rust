//! TerrainLogic bridges behavior.

use super::*;

impl TerrainLogic {
    /// Get first bridge
    pub fn get_first_bridge(&self) -> Option<&Bridge> {
        self.bridge_list_head.as_ref().map(|b| b.as_ref())
    }

    /// Visit every live bridge (C++ TerrainLogic bridge list walk).
    pub fn for_each_bridge<F: FnMut(&Bridge)>(&self, mut f: F) {
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            f(bridge);
            current = bridge.next.as_deref();
        }
    }

    /// Visit every live bridge mutably.
    pub fn for_each_bridge_mut<F: FnMut(&mut Bridge)>(&mut self, mut f: F) {
        let mut current = self.bridge_list_head.as_deref_mut();
        while let Some(bridge) = current {
            f(bridge);
            current = bridge.next.as_deref_mut();
        }
    }

    /// Store a live GenericBridge object id on the leftover span matching `from_left`.
    pub fn bind_bridge_object_id_at(&mut self, from_left: Coord3D, object_id: ObjectID) {
        self.for_each_bridge_mut(|bridge| {
            let fl = bridge.get_bridge_info().from_left;
            if (fl.x - from_left.x).abs() < 0.01 && (fl.y - from_left.y).abs() < 0.01 {
                bridge.set_bridge_object_id(object_id);
            }
        });
    }

    /// C++ Bridge::updateDamageState live writeback — set leftover rubble/pristine.
    pub fn set_bridge_damage_state_for_object(
        &mut self,
        object_id: ObjectID,
        state: BodyDamageType,
    ) {
        if object_id == crate::common::INVALID_ID {
            return;
        }
        let mut changed = false;
        self.for_each_bridge_mut(|bridge| {
            if bridge.get_bridge_info().bridge_object_id == object_id {
                let info = bridge.bridge_info_mut();
                if info.cur_damage_state != state {
                    info.cur_damage_state = state;
                    info.damage_state_changed = true;
                    changed = true;
                }
            }
        });
        if changed {
            self.bridge_damage_states_changed = true;
            if let Some(radar) = crate::helpers::TheRadar::get() {
                radar.queue_terrain_refresh();
            }
        }
    }

    /// Deck Z for a live host XZ sample (C++ XY). None when not on a live span.
    pub fn host_deck_height_at(&self, world_x: f32, world_y: f32) -> Option<f32> {
        let loc = Coord3D::new(world_x, world_y, 0.0);
        let bridge = self.find_bridge_at(&loc)?;
        if bridge.get_bridge_info().cur_damage_state == BodyDamageType::Rubble {
            return None;
        }
        Some(bridge.get_bridge_height(&loc, None))
    }

    /// C++ WaveGuideUpdate::startMoving WaveGuide1 walk.
    pub fn bind_wave_guide1(&self) -> WaveGuide1Bind {
        let Some(waypoint) = self.get_waypoint_by_name(&AsciiString::from("WaveGuide1")) else {
            return WaveGuide1Bind::MissingWaypoint;
        };
        let mut last = *waypoint.get_location();
        let mut verify = Some(waypoint);
        while let Some(node) = verify {
            if node.get_num_links() > 1 {
                return WaveGuide1Bind::InvalidPath;
            }
            last = *node.get_location();
            verify = node.get_link(0).and_then(|id| self.get_waypoint_by_id(id));
        }
        let Some(next_id) = waypoint.get_link(0) else {
            return WaveGuide1Bind::InvalidPath;
        };
        let Some(next) = self.get_waypoint_by_id(next_id) else {
            return WaveGuide1Bind::InvalidPath;
        };
        let angle = (next.get_location().y - waypoint.get_location().y)
            .atan2(next.get_location().x - waypoint.get_location().x);
        WaveGuide1Bind::Follow {
            first: *waypoint.get_location(),
            last,
            angle,
        }
    }

    pub fn bridge_damage_states_changed(&self) -> bool {
        self.bridge_damage_states_changed
    }

    /// Find bridge at location
    pub fn find_bridge_at(&self, location: &Coord3D) -> Option<&Bridge> {
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.is_point_on_bridge(location) {
                return Some(bridge);
            }
            current = bridge.next.as_deref();
        }
        None
    }

    /// Find bridge at location (mutable)
    pub fn find_bridge_at_mut(&mut self, location: &Coord3D) -> Option<&mut Bridge> {
        let mut current = self.bridge_list_head.as_deref_mut();
        while let Some(bridge) = current {
            if bridge.is_point_on_bridge(location) {
                return Some(bridge);
            }
            current = bridge.next.as_deref_mut();
        }
        None
    }

    /// Delete the first bridge that contains the given location.
    pub fn delete_bridge_at(&mut self, location: &Coord3D) -> bool {
        let Some(bridge) = self.find_bridge_at(location) else {
            return false;
        };

        let bridge_object_id = bridge.get_bridge_info().bridge_object_id;
        let bridge_layer = bridge.get_layer();

        let ai_store = the_ai(); if let Some(ai_guard) = ai_store.read().ok() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pathfinder_guard) = pathfinder.write() {
                    pathfinder_guard.change_bridge_state(bridge_layer, false);
                }
            }
        }

        if bridge_object_id != crate::common::INVALID_ID {
            let _ = crate::helpers::TheGameLogic::destroy_object_by_id(bridge_object_id);
        }

        self.remove_bridge_at(location)
    }

    /// Find bridge at layer
    pub fn find_bridge_layer_at(
        &self,
        location: &Coord3D,
        layer: PathfindLayerEnum,
        clip: bool,
    ) -> Option<&Bridge> {
        if layer == PathfindLayerEnum::Ground {
            return None;
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.get_layer() == layer && (!clip || bridge.is_point_on_bridge(location)) {
                return Some(bridge);
            }
            current = bridge.next.as_deref();
        }
        None
    }

    /// Determines whether the object interacts with the bridge on specified layer.
    pub fn object_interacts_with_bridge_layer(
        &self,
        obj: &Object,
        layer: PathfindLayerEnum,
        consider_bridge_health: bool,
    ) -> bool {
        if layer == PathfindLayerEnum::Ground {
            return false;
        }
        if layer == PathfindLayerEnum::Wall {
            if matches!(obj.get_layer(), crate::common::PathfindLayerEnum::Wall) {
                return true;
            }
            return self.is_point_on_wall(obj.get_position());
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.get_layer() == layer {
                let mut matches = false;
                if bridge.is_point_on_bridge(obj.get_position()) {
                    matches = true;
                }

                let mut radius = obj.get_geometry_info().get_minor_radius();
                radius += PATHFIND_CELL_SIZE_F * 0.5;
                let mut bounds = Region2D::default();
                bounds.lo.x = obj.get_position().x - radius;
                bounds.lo.y = obj.get_position().y - radius;
                bounds.hi.x = obj.get_position().x + radius;
                bounds.hi.y = obj.get_position().y + radius;

                if bridge.is_cell_on_end(&bounds) {
                    matches = true;
                }

                if matches {
                    let bridge_height = bridge.get_bridge_height(obj.get_position(), None);
                    let delta = (obj.get_position().z - bridge_height).abs();
                    if delta > LAYER_Z_CLOSE_ENOUGH_F {
                        return false;
                    }
                    if consider_bridge_health
                        && bridge.get_bridge_info().cur_damage_state == BodyDamageType::Rubble
                    {
                        return false;
                    }
                    return true;
                }
                return false;
            }
            current = bridge.next.as_deref();
        }
        false
    }

    /// Determines whether the object interacts with the bridge end on specified layer.
    pub fn object_interacts_with_bridge_end(&self, obj: &Object, layer: PathfindLayerEnum) -> bool {
        if layer == PathfindLayerEnum::Ground {
            return false;
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.get_layer() == layer {
                let mut radius = obj.get_geometry_info().get_minor_radius();
                radius += PATHFIND_CELL_SIZE_F * 0.5;
                let mut bounds = Region2D::default();
                bounds.lo.x = obj.get_position().x - radius;
                bounds.lo.y = obj.get_position().y - radius;
                bounds.hi.x = obj.get_position().x + radius;
                bounds.hi.y = obj.get_position().y + radius;

                if bridge.is_cell_on_end(&bounds) {
                    let bridge_height = bridge.get_bridge_height(obj.get_position(), None);
                    let delta = (obj.get_position().z - bridge_height).abs();
                    if delta > LAYER_Z_CLOSE_ENOUGH_F {
                        return false;
                    }
                    return true;
                }
                return false;
            }
            current = bridge.next.as_deref();
        }
        false
    }

    /// Add bridge to logic
    pub fn add_bridge_to_logic(&mut self, bridge_info: BridgeInfo, template_name: AsciiString) {
        let mut new_bridge = Box::new(Bridge::new(bridge_info, template_name));
        let layer = Self::register_bridge_with_pathfinder(new_bridge.get_bridge_info())
            .unwrap_or(PathfindLayerEnum::Bridge1);
        new_bridge.set_layer(layer);
        new_bridge.next = self.bridge_list_head.take();
        self.bridge_list_head = Some(new_bridge);
    }

    /// Add a landmark bridge from object geometry (live host + leftover Object).
    ///
    /// C++ `TerrainLogic::addLandmarkBridgeToLogic` + `Bridge::Bridge(Object*)`
    /// derive the four deck corners from position / orientation / major+minor
    /// radius, then register the span with the pathfinder.
    pub fn add_landmark_bridge_from_geometry(
        &mut self,
        position: Coord3D,
        angle: Real,
        halfsize_x: Real,
        halfsize_y: Real,
        bridge_object_id: ObjectID,
        template_name: AsciiString,
    ) {
        if bridge_object_id != crate::common::INVALID_ID {
            let mut exists = false;
            self.for_each_bridge(|bridge| {
                if bridge.get_bridge_info().bridge_object_id == bridge_object_id {
                    exists = true;
                }
            });
            if exists {
                return;
            }
        }
        let bridge_info =
            Self::bridge_info_from_parts(position, angle, halfsize_x, halfsize_y, bridge_object_id);
        self.add_bridge_to_logic(bridge_info, template_name);
    }

    /// Add a landmark bridge to logic from an existing leftover object.
    /// Reference: C++ TerrainLogic::addLandmarkBridgeToLogic()
    pub fn add_landmark_bridge_to_logic(&mut self, bridge_obj: &Object) {
        self.add_landmark_bridge_from_geometry(
            *bridge_obj.get_position(),
            bridge_obj.get_orientation(),
            bridge_obj.get_geometry_info().get_major_radius(),
            bridge_obj.get_geometry_info().get_minor_radius(),
            bridge_obj.get_id(),
            bridge_obj.get_template().get_name().clone(),
        );
    }

    /// C++ `TerrainLogic` water-grid enable flag after `newMap`.
    pub fn is_water_grid_enabled(&self) -> bool {
        self.water_grid_enabled
    }

    /// Delete a specific bridge from the terrain system.
    /// Reference: C++ TerrainLogic::deleteBridge()
    ///
    /// Removes the bridge from the list and destroys its associated object.
    pub fn delete_bridge(&mut self, location: &Coord3D) -> bool {
        self.delete_bridge_at(location)
    }

    /// C++ TerrainLogic::loadPostProcess (TerrainLogic.cpp:2994-3009):
    /// delete any bridge whose `bridgeObjectID` no longer resolves.
    pub fn load_post_process(&mut self) {
        let mut orphan_ids = Vec::new();
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let id = bridge.get_bridge_info().bridge_object_id;
            if crate::helpers::TheGameLogic::find_object_by_id(id).is_none() {
                orphan_ids.push(id);
            }
            current = bridge.next.as_deref();
        }
        for id in orphan_ids {
            self.delete_bridge_by_object_id(id);
        }
    }

    fn delete_bridge_by_object_id(&mut self, bridge_object_id: ObjectID) -> bool {
        let mut current = self.bridge_list_head.as_deref();
        let mut found_layer = None;
        while let Some(bridge) = current {
            if bridge.get_bridge_info().bridge_object_id == bridge_object_id {
                found_layer = Some(bridge.get_layer());
                break;
            }
            current = bridge.next.as_deref();
        }
        let Some(bridge_layer) = found_layer else {
            return false;
        };

        let ai_store = the_ai(); if let Some(ai_guard) = ai_store.read().ok() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pathfinder_guard) = pathfinder.write() {
                    pathfinder_guard.change_bridge_state(bridge_layer, false);
                }
            }
        }

        if bridge_object_id != crate::common::INVALID_ID {
            let _ = crate::helpers::TheGameLogic::destroy_object_by_id(bridge_object_id);
        }

        let mut link = &mut self.bridge_list_head;
        loop {
            let should_remove = match link.as_ref() {
                Some(bridge) => bridge.get_bridge_info().bridge_object_id == bridge_object_id,
                None => return false,
            };
            if should_remove {
                let next = link.as_mut().and_then(|bridge| bridge.next.take());
                *link = next;
                self.bridge_damage_states_changed = true;
                return true;
            }
            link = &mut link.as_mut().expect("bridge node exists").next;
        }
    }

    /// Update bridge damage states
    pub fn update_bridge_damage_states(&mut self) {
        self.bridge_damage_states_changed = false;
        let mut current = self.bridge_list_head.as_deref_mut();
        while let Some(bridge) = current {
            bridge.update_damage_state();
            if bridge.get_bridge_info().damage_state_changed {
                self.bridge_damage_states_changed = true;
            }
            current = bridge.next.as_deref_mut();
        }
    }

    /// Checks if the specified bridge object has just been repaired.
    pub fn is_bridge_repaired(&self, bridge_id: ObjectID) -> bool {
        if bridge_id == crate::common::INVALID_ID {
            return false;
        }
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let info = bridge.get_bridge_info();
            if info.bridge_object_id == bridge_id {
                return info.damage_state_changed
                    && info.cur_damage_state != BodyDamageType::Rubble;
            }
            current = bridge.next.as_deref();
        }
        false
    }

    /// Checks if the specified bridge object has just broken (entered rubble state).
    pub fn is_bridge_broken(&self, bridge_id: ObjectID) -> bool {
        if bridge_id == crate::common::INVALID_ID {
            return false;
        }
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let info = bridge.get_bridge_info();
            if info.bridge_object_id == bridge_id {
                return info.damage_state_changed
                    && info.cur_damage_state == BodyDamageType::Rubble;
            }
            current = bridge.next.as_deref();
        }
        false
    }

    /// Gets the attack points for a bridge.
    ///
    /// Bridges have two targetable points at either end. This method calculates
    /// those points based on the bridge's geometry.
    ///
    /// Reference: TerrainLogic.cpp lines 1905-1934 getBridgeAttackPoints()
    pub fn get_bridge_attack_points(
        &self,
        bridge_id: ObjectID,
        attack_info: &mut BridgeAttackInfo,
    ) {
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let info = bridge.get_bridge_info();
            if info.bridge_object_id == bridge_id {
                // Found the right bridge - calculate attack points
                // C++ lines 1914-1926

                // Calculate direction vector from 'from' to 'to' (normalized)
                let mut delta = Coord3D::new(
                    info.to.x - info.from.x,
                    info.to.y - info.from.y,
                    info.to.z - info.from.z,
                );
                let delta_len = delta.length();
                if delta_len > f32::EPSILON {
                    delta.x /= delta_len;
                    delta.y /= delta_len;
                    delta.z /= delta_len;
                }

                // Calculate width vector to get half-width offset
                let width = Coord3D::new(
                    info.from_right.x - info.from_left.x,
                    info.from_right.y - info.from_left.y,
                    info.from_right.z - info.from_left.z,
                );
                let half_width = width.length() / 2.0;

                // Attack point 1: at 'from' end, offset by half-width along bridge direction
                attack_info.attack_point1.x = info.from.x + delta.x * half_width;
                attack_info.attack_point1.y = info.from.y + delta.y * half_width;
                attack_info.attack_point1.z = info.from.z + delta.z * half_width;

                // Attack point 2: at 'to' end, offset by half-width back along bridge direction
                attack_info.attack_point2.x = info.to.x - delta.x * half_width;
                attack_info.attack_point2.y = info.to.y - delta.y * half_width;
                attack_info.attack_point2.z = info.to.z - delta.z * half_width;

                return;
            }
            current = bridge.next.as_deref();
        }

        // Fallback: C++ TerrainLogic.cpp:1936-1937 uses the bridge OBJECT position,
        // not the map origin, when no Bridge list entry matches the object ID.
        if let Some(bridge_obj) = crate::helpers::TheGameLogic::find_object_by_id(bridge_id) {
            if let Ok(guard) = bridge_obj.read() {
                let pos = *guard.get_position();
                attack_info.attack_point1 = pos;
                attack_info.attack_point2 = pos;
                return;
            }
        }
        attack_info.attack_point1 = Coord3D::origin();
        attack_info.attack_point2 = Coord3D::origin();
    }
}
