//! TerrainLogic map height behavior.

use super::*;

impl TerrainLogic {
    pub fn new() -> Self {
        Self {
            map_data: Vec::new(),
            map_dx: 0,
            map_dy: 0,
            map_min_z: 0.0,
            map_max_z: 1.0,
            boundaries: Vec::new(),
            border_size: 0,
            cliff_state: crate::terrain_cliff::CliffBitfield::new(),

            active_boundary: 0,
            waypoint_list_head: None,
            bridge_list_head: None,
            bridge_damage_states_changed: false,
            filename_string: String::new().into(),
            query_load_pending: false,
            water_grid_enabled: false,
            grid_water_handle: WaterHandle::new(
                WATER_GRID_NAME_CPP.to_string().into(),
                0.0,
                Region3D::default(),
            ),
            water_to_update: Vec::new(),
            water_handles: HashMap::new(),
            water_handles_by_trigger_id: HashMap::new(),
            terrain_data: None,
            trigger_areas: PolygonTriggerList::new(),
        }
    }

    /// Load map data from parsed map file
    /// Reference: C++ TerrainLogic.cpp loadMap() integration
    ///
    /// # Arguments
    /// * `map_data` - Parsed map data from MapLoader
    pub fn load_map_data(&mut self, map_data: crate::system::map_loader::MapData) {
        self.query_load_pending = false;

        // Store heightmap. MapData.width/height are playable (minus 2*border);
        // sample buffer is C++ full extent including border. Keep map_dx/dy as
        // full stride so indexing matches WorldHeightMap::getXExtent().
        self.map_data = map_data.heightmap.clone();
        let border = map_data.border_size.max(0);
        let playable_w = map_data.width as i32;
        let playable_h = map_data.height as i32;
        let full_w = playable_w.saturating_add(2 * border);
        let full_h = playable_h.saturating_add(2 * border);
        let data_len = self.map_data.len() as i32;
        if border > 0 && full_w > 0 && full_h > 0 && data_len == full_w.saturating_mul(full_h) {
            self.map_dx = full_w;
            self.map_dy = full_h;
        } else {
            self.map_dx = playable_w;
            self.map_dy = playable_h;
        }
        if let Some((&min_height, &max_height)) =
            self.map_data.iter().min().zip(self.map_data.iter().max())
        {
            self.map_min_z = min_height as f32 * MAP_HEIGHT_SCALE;
            self.map_max_z = max_height as f32 * MAP_HEIGHT_SCALE;
        } else {
            self.map_min_z = 0.0;
            self.map_max_z = 1.0;
        }
        self.boundaries = map_data.boundaries.clone();
        self.border_size = map_data.border_size;
        self.cliff_state
            .rebuild(&self.map_data, self.map_dx, self.map_dy);

        // Store terrain data including bridges
        self.terrain_data = Some(TerrainData {
            heightmap: map_data.heightmap,
            width: map_data.width as i32,
            height: map_data.height as i32,
            bridges: map_data.bridges,
        });

        // Rebuild grid-water handle using C++ sentinel name and map extent.
        let grid_height = map_data
            .water_height
            .unwrap_or(self.grid_water_handle.get_current_height());
        self.grid_water_handle = WaterHandle::new(
            WATER_GRID_NAME_CPP.to_string().into(),
            grid_height,
            self.get_extent_including_border(),
        );

        // Load polygon trigger areas from map data
        self.trigger_areas.clear();
        self.water_handles.clear();
        self.water_handles_by_trigger_id.clear();
        for trigger in map_data.polygon_triggers {
            self.add_trigger_area(trigger);
        }

        // Load waypoints and links
        self.waypoint_list_head = None;
        for waypoint in &map_data.waypoints {
            self.add_waypoint_from_map(waypoint);
        }
        for (id1, id2) in &map_data.waypoint_links {
            self.add_waypoint_link(*id1, *id2);
        }

        // C++ W3DBridgeBuffer::addBridge → TerrainLogic::addBridgeToLogic
        // (TerrainLogic.cpp:1514, W3DBridgeBuffer.cpp:1059). Map bridges used
        // to sit in TerrainData only, so live pathfinding/height never saw them.
        self.bridge_list_head = None;
        let bridges = self
            .terrain_data
            .as_ref()
            .map(|terrain_data| terrain_data.bridges.clone())
            .unwrap_or_default();
        for (index, bridge) in bridges.iter().enumerate() {
            let Some(info) = Self::bridge_info_from_map_data(bridge, index as i32) else {
                continue;
            };
            self.add_bridge_to_logic(info, AsciiString::from(bridge.template_name.as_str()));
        }
    }

    /// Snapshot parsed map bridge geometry.
    pub fn bridge_data_snapshot(&self) -> Vec<crate::system::map_loader::BridgeData> {
        self.terrain_data
            .as_ref()
            .map(|terrain_data| terrain_data.bridges.clone())
            .unwrap_or_default()
    }

    /// Get map extent including border in world coordinates.
    pub fn get_extent_including_border(&self) -> Region3D {
        let border = (self.border_size.max(0) as f32) * MAP_XY_FACTOR;
        let width = (self.map_dx.max(0) as f32) * MAP_XY_FACTOR - border;
        let height = (self.map_dy.max(0) as f32) * MAP_XY_FACTOR - border;
        Region3D::new(
            Coord3D::new(-border, -border, self.map_min_z),
            Coord3D::new(width, height, self.map_max_z),
        )
    }

    /// Get largest pathfind boundary in world coordinates.
    pub fn get_maximum_pathfind_extent(&self) -> Region3D {
        let mut hi_x: f32 = 0.0;
        let mut hi_y: f32 = 0.0;
        for boundary in &self.boundaries {
            hi_x = hi_x.max(boundary.x as f32 * MAP_XY_FACTOR);
            hi_y = hi_y.max(boundary.y as f32 * MAP_XY_FACTOR);
        }

        Region3D::new(
            Coord3D::new(0.0, 0.0, self.map_min_z),
            Coord3D::new(hi_x, hi_y, self.map_max_z),
        )
    }

    /// Get the map extent in world coordinates.
    /// Reference: C++ TerrainLogic::getExtent()
    ///
    /// Returns the bounding box of the playable map area.
    pub fn get_extent(&self) -> Region3D {
        let active_boundary = self
            .boundaries
            .get(self.active_boundary.max(0) as usize)
            .copied()
            .unwrap_or_else(|| ICoord2D::new(0, 0));

        Region3D::new(
            Coord3D::new(0.0, 0.0, self.map_min_z),
            Coord3D::new(
                active_boundary.x as f32 * MAP_XY_FACTOR,
                active_boundary.y as f32 * MAP_XY_FACTOR,
                self.map_max_z,
            ),
        )
    }

    /// Initialize the terrain system
    pub fn init(&mut self) {
        // Initialize terrain system
        self.reset();
    }

    /// Reset the terrain system
    pub fn reset(&mut self) {
        self.map_data.clear();
        self.map_dx = 0;
        self.map_dy = 0;
        self.map_min_z = 0.0;
        self.map_max_z = 1.0;
        self.boundaries.clear();
        self.border_size = 0;
        self.cliff_state.clear();

        self.active_boundary = 0;
        self.waypoint_list_head = None;
        self.bridge_list_head = None;
        self.water_to_update.clear();
        self.water_handles.clear();
        self.water_handles_by_trigger_id.clear();
        self.terrain_data = None;
        self.bridge_damage_states_changed = false;
        self.trigger_areas.clear();
        self.water_grid_enabled = false;
        crate::terrain_water::reset_water_grid_state();

        self.query_load_pending = false;
    }

    /// Update the terrain system
    pub fn update(&mut self) {
        // Update dynamic water tables
        self.update_dynamic_water();

        // Update bridge damage states
        self.update_bridge_damage_states();
    }

    /// Load map from file
    pub fn load_map(&mut self, filename: AsciiString, _query: bool) -> bool {
        self.filename_string = filename.clone();
        self.query_load_pending = false;
        let requested = filename.as_str();

        let Some(map_path) = self.resolve_map_path(requested) else {
            log::warn!("TerrainLogic::load_map: map file '{}' not found", requested);
            return false;
        };

        match crate::system::map_loader::MapLoader::load(&map_path) {
            Ok(map_data) => {
                self.reset();
                self.filename_string = filename;
                self.load_map_data(map_data);
                self.query_load_pending = _query;
                true
            }
            Err(err) => {
                log::error!(
                    "TerrainLogic::load_map: failed to parse map '{}' (resolved '{}'): {:?}",
                    requested,
                    map_path.display(),
                    err
                );
                false
            }
        }
    }

    fn resolve_map_path(&self, filename: &str) -> Option<PathBuf> {
        let trimmed = filename.trim();
        if trimmed.is_empty() {
            return None;
        }

        let input = PathBuf::from(trimmed);
        let mut variants = path_with_map_variants(&input);
        if input.extension().is_none() && input.components().count() == 1 {
            variants.push(input.join(format!("{trimmed}.map")));
            variants.push(input.join(format!("{trimmed}.MAP")));
        }

        let mut candidates = Vec::new();
        if input.is_absolute() {
            candidates.extend(variants);
        } else {
            candidates.extend(variants.clone());

            if let Ok(cwd) = std::env::current_dir() {
                for relative in &variants {
                    candidates.push(cwd.join(relative));
                    candidates.push(cwd.join("Maps").join(relative));
                    candidates.push(cwd.join("maps").join(relative));
                    candidates.push(cwd.join("Data").join("Maps").join(relative));
                    candidates.push(cwd.join("data").join("maps").join(relative));
                    candidates.push(cwd.join("GeneralsZHData").join("Maps").join(relative));
                }
            }
        }

        let mut seen = HashSet::new();
        for candidate in candidates {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Set source map filename used by `get_source_filename()`.
    ///
    /// C++ parity: `TerrainLogic::loadMap()` stores `m_filenameString` before
    /// finalization and client notification paths consume that value.
    pub fn set_source_filename(&mut self, filename: AsciiString) {
        self.filename_string = filename;
    }

    /// Initialize for new map
    ///
    /// C++ parity: this is a post-load finalize step, not a full terrain reset.
    /// It aligns waypoint Z with loaded ground height and enables water grid only
    /// when the map defines the legacy `WaveGuide1` marker.
    pub fn new_map(&mut self, _save_game: bool) {
        if self.query_load_pending {
            self.query_load_pending = false;
            return;
        }

        let mut waypoint_heights = Vec::new();
        let mut current = self.waypoint_list_head.as_deref();
        while let Some(waypoint) = current {
            let loc = waypoint.get_location();
            waypoint_heights.push((
                waypoint.get_id(),
                self.get_ground_height(loc.x, loc.y, None),
            ));
            current = waypoint.get_next();
        }

        for (id, z) in waypoint_heights {
            if let Some(waypoint) = self.get_waypoint_by_id_mut(id) {
                waypoint.set_location_z(z);
            }
        }

        let wave_guide = AsciiString::from("WaveGuide1");
        self.enable_water_grid(self.get_waypoint_by_name(&wave_guide).is_some());
    }

    pub fn has_height_map(&self) -> bool {
        !self.map_data.is_empty() && self.map_dx > 0 && self.map_dy > 0
    }
    /// Get ground height at position
    pub fn get_ground_height(&self, x: f32, y: f32, normal: Option<&mut Coord3D>) -> f32 {
        if self.map_data.is_empty() || self.map_dx <= 0 || self.map_dy <= 0 {
            if let Some(n) = normal {
                *n = Coord3D::new(0.0, 0.0, 1.0);
            }
            return 0.0;
        }

        let map_x = x / MAP_XY_FACTOR;
        let map_y = y / MAP_XY_FACTOR;

        let ixf = map_x.floor();
        let iyf = map_y.floor();
        let fx = map_x - ixf;
        let fy = map_y - iyf;

        // C++ BaseHeightMap: ix/iy += getBorderSizeInline(); xExtent is full width.
        let ix = ixf as i32 + self.border_size.max(0);
        let iy = iyf as i32 + self.border_size.max(0);

        let x_extent = self.map_dx;
        let y_extent = self.map_dy;

        let get_height_sample = |gx: i32, gy: i32| -> f32 {
            let cx = gx.clamp(0, x_extent.saturating_sub(1).max(0));
            let cy = gy.clamp(0, y_extent.saturating_sub(1).max(0));
            let idx = (cy * x_extent + cx) as usize;
            if idx < self.map_data.len() {
                self.map_data[idx] as f32
            } else {
                0.0
            }
        };

        // C++ rejects ix/iy < 1 or > extent-3 (needs neighborhood for normals).
        if x_extent >= 3
            && y_extent >= 3
            && (ix > x_extent - 3 || iy > y_extent - 3 || iy < 1 || ix < 1)
        {
            if let Some(n) = normal {
                *n = Coord3D::new(0.0, 0.0, 1.0);
            }
            return get_height_sample(ix, iy) * MAP_HEIGHT_SCALE;
        }

        let p0 = get_height_sample(ix, iy);
        let p1 = get_height_sample(ix + 1, iy);
        let p2 = get_height_sample(ix + 1, iy + 1);
        let p3 = get_height_sample(ix, iy + 1);

        // Triangle-based barycentric interpolation matching C++ BaseHeightMapRenderObjClass::getHeightMapHeight
        // C++ tessellation: diagonal from (0,0) to (1,1)
        //   3-----2
        //   |    /|
        //   |  /  |
        //   |/    |
        //   0-----1
        let height = if fy > fx {
            // Upper triangle: vertices p0, p2, p3
            (p3 + (1.0 - fy) * (p0 - p3) + fx * (p2 - p3)) * MAP_HEIGHT_SCALE
        } else {
            // Lower triangle: vertices p0, p1, p2
            (p1 + fy * (p2 - p1) + (1.0 - fx) * (p0 - p1)) * MAP_HEIGHT_SCALE
        };

        if let Some(n) = normal {
            // C++ BaseHeightMapRenderObjClass::getHeightMapHeight (BaseHeightMap.cpp:914-970)
            // bilinearly smooths deltaZ_X/deltaZ_Y over 4 samples each (12 neighbors),
            // then builds l2r/n2f and a normalized cross product.
            let d0 = p0;
            let d1 = p1;
            let d2 = p2;
            let d3 = p3;
            let d4 = get_height_sample(ix, iy - 1);
            let d5 = get_height_sample(ix + 1, iy - 1);
            let d6 = get_height_sample(ix + 2, iy);
            let d7 = get_height_sample(ix + 2, iy + 1);
            let d8 = get_height_sample(ix + 1, iy + 2);
            let d9 = get_height_sample(ix, iy + 2);
            let _d10 = get_height_sample(ix - 1, iy + 1);
            let d11 = get_height_sample(ix - 1, iy);

            let delta_z_x0 = d1 - d11;
            let delta_z_x1 = d6 - d0;
            let delta_z_x2 = d7 - d3;
            let delta_z_x3 = d6 - d0;
            let delta_z_y0 = d3 - d4;
            let delta_z_y1 = d2 - d5;
            let delta_z_y2 = d8 - d1;
            let delta_z_y3 = d9 - d0;

            let delta_z_x_left = delta_z_x0 * (1.0 - fx) + fx * delta_z_x3;
            let delta_z_x_right = delta_z_x1 * (1.0 - fx) + fx * delta_z_x2;
            let delta_z_x = delta_z_x_left * (1.0 - fy) + fy * delta_z_x_right;
            let delta_z_y_left = delta_z_y0 * (1.0 - fx) + fx * delta_z_y3;
            let delta_z_y_right = delta_z_y1 * (1.0 - fx) + fx * delta_z_y2;
            let delta_z_y = delta_z_y_left * (1.0 - fy) + fy * delta_z_y_right;

            let l2r_x = 2.0 * MAP_XY_FACTOR / MAP_HEIGHT_SCALE;
            let n2f_y = 2.0 * MAP_XY_FACTOR / MAP_HEIGHT_SCALE;
            let mut nx = 0.0 * delta_z_y - n2f_y * delta_z_x;
            let mut ny = delta_z_x * 0.0 - l2r_x * delta_z_y;
            let mut nz = l2r_x * n2f_y - 0.0 * 0.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > f32::EPSILON {
                nx /= len;
                ny /= len;
                nz /= len;
            } else {
                nx = 0.0;
                ny = 0.0;
                nz = 1.0;
            }
            *n = Coord3D::new(nx, ny, nz);
        }

        height
    }

    /// Get layer height at position
    pub fn get_layer_height(
        &self,
        x: f32,
        y: f32,
        layer: PathfindLayerEnum,
        normal: Option<&mut Coord3D>,
        clip: bool,
    ) -> f32 {
        let mut ground_normal = Coord3D::new(0.0, 0.0, 1.0);
        let height = self.get_ground_height(x, y, Some(&mut ground_normal));

        if layer != PathfindLayerEnum::Ground {
            let pos = Coord3D::new(x, y, height);

            if layer == PathfindLayerEnum::Wall {
                if !clip || self.is_point_on_wall(&pos) {
                    if let Some(out) = normal {
                        *out = ground_normal;
                    }
                    return self.get_wall_height();
                }

                if let Some(out) = normal {
                    *out = ground_normal;
                }
                return height;
            }

            if let Some(bridge) = self.find_bridge_layer_at(&pos, layer, clip) {
                let mut bridge_normal = Coord3D::new(0.0, 0.0, 1.0);
                let bridge_height = bridge.get_bridge_height(&pos, Some(&mut bridge_normal));
                if bridge_height > height {
                    if let Some(out) = normal {
                        *out = bridge_normal;
                    }
                    return bridge_height;
                }
            }
        }

        if let Some(out) = normal {
            *out = ground_normal;
        }
        height
    }

    /// Find closest edge point
    pub fn find_closest_edge_point(&self, closest_to: &Coord3D) -> Coord3D {
        // C++ TerrainLogic.cpp:2036-2079 uses getExtent() (active boundary),
        // not getMaximumPathfindExtent(). W3DTerrainLogic.cpp:176-193.
        let extent = self.get_extent();
        let distances = [
            (closest_to.y - extent.lo.y).abs(), // top
            (closest_to.x - extent.hi.x).abs(), // right
            (closest_to.y - extent.hi.y).abs(), // bottom
            (closest_to.x - extent.lo.x).abs(), // left
        ];
        let mut best_index = 0usize;
        let mut best_distance = distances[0];
        for (idx, distance) in distances.iter().copied().enumerate().skip(1) {
            if distance < best_distance {
                best_distance = distance;
                best_index = idx;
            }
        }

        let mut ret = *closest_to;
        match best_index {
            0 => ret.y = extent.lo.y,
            1 => ret.x = extent.hi.x,
            2 => ret.y = extent.hi.y,
            _ => ret.x = extent.lo.x,
        }
        ret.z = self.get_ground_height(ret.x, ret.y, None);
        ret
    }

    /// Determine the highest pathfinding layer that should be used for a destination position.
    ///
    /// Mirrors the C++ intent: pick the highest layer at/below the position.
    pub fn get_highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        self.get_highest_layer_for_destination_with_health(pos, false)
    }

    pub(super) fn get_wall_height(&self) -> Real {
        THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|data| data.wall_height))
            .unwrap_or(0.0)
    }

    pub(super) fn is_point_on_wall(&self, pos: &Coord3D) -> bool {
        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(pathfinder_guard) = pathfinder.read() {
                    return pathfinder_guard.is_point_on_wall(pos);
                }
            }
        }
        self.is_point_on_wall_fallback(pos)
    }

    pub(super) fn is_point_on_wall_fallback(&self, pos: &Coord3D) -> bool {
        let cell_pad = PATHFIND_CELL_SIZE_F * 0.5;
        // Wave 341: empty dual-world → false.
        if dual_world_registry_unavailable() {
            return false;
        }
        for obj_id in OBJECT_REGISTRY.get_all_object_ids() {
            let obj = match OBJECT_REGISTRY.get_object(obj_id) {
                Some(v) => v,
                None => continue,
            };
            let obj_guard = match obj.read() {
                Ok(v) => v,
                Err(_) => continue,
            };
            if true {
                if !obj_guard.is_any_kind_of(&[KindOf::Barrier]) {
                    continue;
                }
                let wall_pos = obj_guard.get_position();
                let geom = obj_guard.get_template().get_template_geometry_info();
                let radius = geom.get_bounding_circle_radius();
                let dx = wall_pos.x - pos.x;
                let dy = wall_pos.y - pos.y;
                let dist_sq = dx * dx + dy * dy;
                let allowed = radius + cell_pad;
                if dist_sq <= allowed * allowed {
                    return true;
                }
            }
        }
        false
    }

    /// Variant that can optionally ignore broken bridges.
    pub fn get_highest_layer_for_destination_with_health(
        &self,
        pos: &Coord3D,
        only_healthy_bridges: bool,
    ) -> PathfindLayerEnum {
        let ground_z = self.get_ground_height(pos.x, pos.y, None);
        let mut best_layer = PathfindLayerEnum::Ground;
        let mut best_distance = pos.z - ground_z;

        let wall_height = self.get_wall_height();
        if best_distance > wall_height * 0.5 && self.is_point_on_wall(pos) {
            let delta = pos.z - wall_height;
            if delta >= 0.0 && delta.abs() < best_distance.abs() {
                best_layer = PathfindLayerEnum::Wall;
                best_distance = delta;
            }
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let info = bridge.get_bridge_info();
            if only_healthy_bridges && info.cur_damage_state == BodyDamageType::Rubble {
                current = bridge.next.as_deref();
                continue;
            }
            if bridge.is_point_on_bridge(pos) {
                let bridge_z = bridge.get_bridge_height(pos, None);
                let delta = pos.z - bridge_z;
                if delta >= 0.0 && delta.abs() < best_distance.abs() {
                    best_layer = bridge.get_layer();
                    best_distance = delta;
                }
            }
            current = bridge.next.as_deref();
        }

        best_layer
    }

    /// Find farthest edge point
    /// Determine the layer for a destination position (C++ getLayerForDestination).
    pub fn get_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        let ground_z = self.get_ground_height(pos.x, pos.y, None);
        let mut best_layer = PathfindLayerEnum::Ground;
        let mut best_distance = (pos.z - ground_z).abs();

        let wall_height = self.get_wall_height();
        if best_distance > wall_height * 0.5 && self.is_point_on_wall(pos) {
            let delta = (pos.z - wall_height).abs();
            if delta < best_distance {
                best_layer = PathfindLayerEnum::Wall;
                best_distance = delta;
            }
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.is_point_on_bridge(pos) {
                let bridge_z = bridge.get_bridge_height(pos, None);
                let delta = (pos.z - bridge_z).abs();
                if delta < best_distance {
                    best_layer = bridge.get_layer();
                    best_distance = delta;
                }
            }
            current = bridge.next.as_deref();
        }

        best_layer
    }

    /// C++ `makeAlignToNormalMatrix` — X points at `angle`, Z is the terrain normal.
    pub fn make_align_to_normal_matrix(angle: Real, pos: &Coord3D, normal: &Coord3D) -> Matrix3D {
        let z = *normal;
        let mut x = Coord3D::new(angle.cos(), angle.sin(), 0.0);
        if z.z != 0.0 {
            x.z = -(x.x * z.x + x.y * z.y) / z.z;
            let len = x.length();
            if len > f32::EPSILON {
                x /= len;
            }
        }
        let mut y = z.cross(x);
        let y_len = y.length();
        if y_len > f32::EPSILON {
            y /= y_len;
        }
        Matrix3D::from_cols(
            glam::Vec4::new(x.x, x.y, x.z, 0.0),
            glam::Vec4::new(y.x, y.y, y.z, 0.0),
            glam::Vec4::new(z.x, z.y, z.z, 0.0),
            glam::Vec4::new(pos.x, pos.y, pos.z, 1.0),
        )
    }

    /// C++ `TerrainLogic::alignOnTerrain`. Tilts STICK_TO_TERRAIN_SLOPE units to
    /// the ground normal and keeps world position (+2.5 bridge-layer hack).
    pub fn align_on_terrain(
        &self,
        angle: Real,
        pos: &Coord3D,
        stick_to_ground: bool,
        mtx: &mut Matrix3D,
    ) -> PathfindLayerEnum {
        let layer = self.get_layer_for_destination(pos);
        let mut terrain_normal = Coord3D::new(0.0, 0.0, 1.0);
        let mut terrain_at_pos =
            self.get_layer_height(pos.x, pos.y, layer, Some(&mut terrain_normal), true);
        if layer != PathfindLayerEnum::Ground {
            terrain_at_pos += 2.5;
        }
        let mut aligned_pos = *pos;
        if stick_to_ground {
            aligned_pos.z = terrain_at_pos;
        }
        *mtx = Self::make_align_to_normal_matrix(angle, &aligned_pos, &terrain_normal);
        layer
    }

    pub fn find_farthest_edge_point(&self, farthest_from: &Coord3D) -> Coord3D {
        // C++ TerrainLogic.cpp:2088-2110 uses getExtent() (active boundary).
        let extent = self.get_extent();
        let width = extent.hi.x - extent.lo.x;
        let height = extent.hi.y - extent.lo.y;

        let mut ret = *farthest_from;
        if farthest_from.x < width * 0.5 {
            ret.x = extent.hi.x;
        } else {
            ret.x = extent.lo.x;
        }

        if farthest_from.y < height * 0.5 {
            ret.y = extent.hi.y;
        } else {
            ret.y = extent.lo.y;
        }

        ret.z = self.get_ground_height(ret.x, ret.y, None);
        ret
    }

    /// Check clear line of sight
    pub fn is_clear_line_of_sight(&self, pos1: &Coord3D, pos2: &Coord3D) -> bool {
        crate::terrain_los::is_clear_line_of_sight(
            pos1,
            pos2,
            &self.map_data,
            self.map_dx,
            self.map_dy,
            self.border_size,
            self.map_max_z,
        )
    }

    /// Get source filename
    pub fn get_source_filename(&self) -> &AsciiString {
        &self.filename_string
    }
}
