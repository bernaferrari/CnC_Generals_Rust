//! TerrainLogic map height behavior.

use super::*;
use crate::system::map_loader::{BridgeData, MapData};
use generals_sim::terrain::TerrainHeightMap;
use math_utilities::Vector3;

impl TerrainLogic {
    pub fn new() -> Self {
        Self {
            height_map: TerrainHeightMap::default(),
            boundaries: Vec::new(),
            active_boundary: 0,
            waypoint_list_head: None,
            bridges: BridgeList::default(),
            filename_string: String::new().into(),
            query_load_pending: false,
            water_grid_enabled: false,
            water_grid: generals_sim::terrain::WaterGridState::default(),
            grid_water_handle: WaterHandle::new(
                WATER_GRID_NAME_CPP.to_string().into(),
                0.0,
                Region3D::default(),
            ),
            water_to_update: WaterTransitions::default(),
            water_handles: HashMap::new(),
            water_handles_by_trigger_id: HashMap::new(),
            authored_bridges: Vec::new(),
            trigger_areas: PolygonTriggerList::new(),
        }
    }

    /// Load map data from parsed map file
    /// Reference: C++ TerrainLogic.cpp loadMap() integration
    ///
    /// # Arguments
    /// * `map_data` - Parsed map data from MapLoader
    pub fn load_map_data(&mut self, map_data: MapData) {
        for (info, template) in self.load_map_geometry(map_data) {
            self.add_bridge_to_logic(info, template);
        }
    }

    /// Admit the decoded terrain and reserve bridge layers on the same Core
    /// match. A new map always starts with fresh layers.
    pub(crate) fn admit_core_map(
        &mut self,
        map_data: MapData,
        mut pathfinder: Option<&mut crate::ai::Pathfinder>,
    ) -> Result<(), String> {
        let bridges = self.try_load_authored_map_geometry(map_data)?;
        if let Some(pathfinder) = pathfinder.as_deref_mut() {
            pathfinder.reset();
        }
        for (info, template) in bridges {
            self.add_bridge_in(pathfinder.as_deref_mut(), info, template);
        }
        Ok(())
    }

    /// Parse terrain content without discovering an AI/pathfinder owner. The
    /// caller admits returned bridges in source order and supplies their layers.
    pub fn load_map_geometry(&mut self, map_data: MapData) -> Vec<(BridgeInfo, AsciiString)> {
        match self.try_load_map_geometry(map_data) {
            Ok(bridges) => bridges,
            Err(error) => {
                log::error!("terrain map admission: {error}");
                Vec::new()
            }
        }
    }
    /// Check a decoded map before a caller resets map-scoped subsystems.
    pub fn validate_map_geometry(map_data: &MapData) -> Result<(), String> {
        TerrainHeightMap::map_dimensions(
            map_data.width,
            map_data.height,
            map_data.border_size,
            map_data.heightmap.len(),
        )
        .map(|_| ())
    }

    pub fn try_load_map_geometry(
        &mut self,
        map_data: MapData,
    ) -> Result<Vec<(BridgeInfo, AsciiString)>, String> {
        let bridges = self.install_map_terrain(map_data)?;
        Ok(self.install_authored_bridges(bridges))
    }

    /// Install pristine map terrain and attach each authored bridge to its endpoints.
    pub fn try_load_authored_map_geometry(
        &mut self,
        map_data: MapData,
    ) -> Result<Vec<(BridgeInfo, AsciiString)>, String> {
        let mut bridges = self.install_map_terrain(map_data)?;
        // W3DBridgeBuffer::loadBridges samples the logic heightmap and adds
        // BRIDGE_FLOAT_AMT before it admits the span to the terrain and AI.
        const BRIDGE_FLOAT_AMT: f32 = 0.25;
        for bridge in &mut bridges {
            bridge.from.z =
                self.get_ground_height(bridge.from.x, bridge.from.y, None) + BRIDGE_FLOAT_AMT;
            bridge.to.z = self.get_ground_height(bridge.to.x, bridge.to.y, None) + BRIDGE_FLOAT_AMT;
            bridge.height = (bridge.from.z + bridge.to.z) / 2.0;
        }
        Ok(self.install_authored_bridges(bridges))
    }

    fn install_map_terrain(&mut self, map_data: MapData) -> Result<Vec<BridgeData>, String> {
        self.height_map.admit(
            map_data.width,
            map_data.height,
            map_data.border_size,
            map_data.heightmap,
        )?;
        self.query_load_pending = false;
        self.boundaries = map_data.boundaries;

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
        self.replace_trigger_areas(map_data.polygon_triggers);

        // Load waypoints and links
        self.waypoint_list_head = None;
        for waypoint in &map_data.waypoints {
            self.add_waypoint_from_map(waypoint);
        }
        for (id1, id2) in &map_data.waypoint_links {
            self.add_waypoint_link(*id1, *id2);
        }

        Ok(map_data.bridges)
    }

    fn install_authored_bridges(
        &mut self,
        bridges: Vec<BridgeData>,
    ) -> Vec<(BridgeInfo, AsciiString)> {
        // C++ reserves layers in authored order; the terrain list prepends.
        self.bridges.clear();
        self.authored_bridges = bridges;
        self.authored_bridges
            .iter()
            .enumerate()
            .filter_map(|(index, bridge)| {
                Self::bridge_info_from_map_data(bridge, index as i32)
                    .map(|info| (info, AsciiString::from(bridge.template_name.as_str())))
            })
            .collect()
    }

    /// C++ W3DBridgeBuffer passes properties from POINT1 to the bridge actor.
    pub fn authored_bridge_properties(
        &self,
        bridge_index: i32,
    ) -> Option<&game_engine::common::dict::Dict> {
        let index = usize::try_from(bridge_index).ok()?;
        self.authored_bridges
            .get(index)
            .map(|bridge| &bridge.properties)
    }

    /// Snapshot parsed map bridge geometry.
    pub fn bridge_data_snapshot(&self) -> Vec<BridgeData> {
        self.authored_bridges.clone()
    }

    /// Get map extent including border in world coordinates.
    pub fn get_extent_including_border(&self) -> Region3D {
        let (samples_x, samples_y) = self.height_map.extents();
        let (min_z, max_z) = self.height_map.vertical_range();
        let border = (self.height_map.border_size().max(0) as f32) * MAP_XY_FACTOR;
        let width = (samples_x.max(0) as f32) * MAP_XY_FACTOR - border;
        let height = (samples_y.max(0) as f32) * MAP_XY_FACTOR - border;
        Region3D::new(
            Coord3D::new(-border, -border, min_z),
            Coord3D::new(width, height, max_z),
        )
    }

    /// Get largest pathfind boundary in world coordinates.
    pub fn get_maximum_pathfind_extent(&self) -> Region3D {
        let (min_z, max_z) = self.height_map.vertical_range();
        let mut hi_x: f32 = 0.0;
        let mut hi_y: f32 = 0.0;
        for boundary in &self.boundaries {
            hi_x = hi_x.max(boundary.x as f32 * MAP_XY_FACTOR);
            hi_y = hi_y.max(boundary.y as f32 * MAP_XY_FACTOR);
        }

        Region3D::new(
            Coord3D::new(0.0, 0.0, min_z),
            Coord3D::new(hi_x, hi_y, max_z),
        )
    }

    /// Get the map extent in world coordinates.
    /// Reference: C++ TerrainLogic::getExtent()
    ///
    /// Returns the bounding box of the playable map area.
    pub fn get_extent(&self) -> Region3D {
        let (min_z, max_z) = self.height_map.vertical_range();
        let active_boundary = self
            .boundaries
            .get(self.active_boundary.max(0) as usize)
            .copied()
            .unwrap_or_else(|| ICoord2D::new(0, 0));

        Region3D::new(
            Coord3D::new(0.0, 0.0, min_z),
            Coord3D::new(
                active_boundary.x as f32 * MAP_XY_FACTOR,
                active_boundary.y as f32 * MAP_XY_FACTOR,
                max_z,
            ),
        )
    }

    /// Resolve a saved boundary without selecting it or mutating a session.
    pub fn extent_for_saved_boundary(&self, index: i32) -> Option<Region3D> {
        if index == self.active_boundary {
            return Some(self.get_extent());
        }
        let boundary = self.boundaries.get(usize::try_from(index).ok()?)?;
        if boundary.x == 0 || boundary.y == 0 {
            return None;
        }
        let (min_z, max_z) = self.height_map.vertical_range();
        Some(Region3D::new(
            Coord3D::new(0.0, 0.0, min_z),
            Coord3D::new(
                boundary.x as f32 * MAP_XY_FACTOR,
                boundary.y as f32 * MAP_XY_FACTOR,
                max_z,
            ),
        ))
    }

    /// Initialize the terrain system
    pub fn init(&mut self) {
        // Initialize terrain system
        self.reset();
    }

    /// Reset the terrain system
    pub fn reset(&mut self) {
        self.height_map.clear();
        self.boundaries.clear();

        self.active_boundary = 0;
        self.waypoint_list_head = None;
        self.bridges.clear();
        self.water_to_update.clear();
        self.water_handles.clear();
        self.water_handles_by_trigger_id.clear();
        self.authored_bridges.clear();
        self.bridges.begin_damage_frame();
        self.trigger_areas.clear();
        self.water_grid_enabled = false;
        self.water_grid = generals_sim::terrain::WaterGridState::default();

        self.query_load_pending = false;
    }

    /// Update the terrain system
    pub fn update(&mut self) {
        // Update dynamic water tables
        self.update_dynamic_water();

        // Update bridge damage states
        self.update_bridge_damage_states();
    }

    /// Update an explicitly owned world's dynamic water against its receiving
    /// pathfinder. Missing owner pathfinders never fall through to TheAI.
    pub(crate) fn update_owned_water(
        &mut self,
        pathfinder: Option<&Arc<RwLock<crate::ai::Pathfinder>>>,
        frame: u32,
    ) {
        self.begin_host_bridge_frame();
        self.update_dynamic_water_with_owner(pathfinder, frame);
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
    pub fn new_map(&mut self, save_game: bool) {
        let global = game_engine::common::ini::get_global_data();
        let definitions = global.as_ref().map(|data| data.read().clone());
        let source = definitions
            .as_ref()
            .map(|data| data.map_name.as_str())
            .unwrap_or("");
        let mut delivery = crate::terrain_water::publish_map_water_legacy_native;
        self.new_map_with_water_configuration(
            save_game,
            source,
            definitions.as_ref(),
            Some(&mut delivery),
        );
    }

    pub fn new_map_with_water_configuration(
        &mut self,
        _save_game: bool,
        map_name: &str,
        definitions: Option<&game_engine::common::ini::ini_game_data::GlobalData>,
        deliver: Option<&mut dyn FnMut(&generals_sim::terrain::WaterGridState)>,
    ) {
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
        let enable = self.get_waypoint_by_name(&wave_guide).is_some();
        self.water_grid_enabled = enable;
        let configured = crate::terrain_water::configure_map_water(
            &mut self.water_grid,
            enable,
            map_name,
            definitions,
        );
        if configured {
            if let Some(deliver) = deliver {
                deliver(&self.water_grid);
            }
        }
    }

    pub fn has_height_map(&self) -> bool {
        self.height_map.has_height_map()
    }
    /// Get ground height at position
    pub fn get_ground_height(&self, x: f32, y: f32, normal: Option<&mut Coord3D>) -> f32 {
        self.height_map.get_ground_height(x, y, normal)
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
        the_ai()
            .read()
            .ok()
            .and_then(|ai| Some(&*ai.get_ai_data()).map(|data| data.wall_height))
            .unwrap_or(0.0)
    }

    pub(super) fn is_point_on_wall(&self, pos: &Coord3D) -> bool {
        let ai_store = the_ai();
        if let Ok(ai_guard) = ai_store.read() {
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

        for bridge in self.bridges.iter() {
            let info = bridge.get_bridge_info();
            if only_healthy_bridges && info.cur_damage_state == BodyDamageType::Rubble {
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
        }

        best_layer
    }

    /// Find farthest edge point
    /// Determine the layer for a destination position (C++ getLayerForDestination).
    pub fn get_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        self.get_layer_for_destination_with_wall(
            pos,
            || self.get_wall_height(),
            || self.is_point_on_wall(pos),
        )
    }

    pub(crate) fn get_layer_for_destination_with_wall(
        &self,
        pos: &Coord3D,
        wall_height: impl FnOnce() -> Real,
        is_point_on_wall: impl FnOnce() -> bool,
    ) -> PathfindLayerEnum {
        let ground_z = self.get_ground_height(pos.x, pos.y, None);
        let mut best_layer = PathfindLayerEnum::Ground;
        let mut best_distance = (pos.z - ground_z).abs();

        if best_distance == 0.0 {
            return best_layer;
        }

        let wall_height = wall_height();
        if best_distance > wall_height * 0.5 && is_point_on_wall() {
            let delta = (pos.z - wall_height).abs();
            if delta < best_distance {
                best_layer = PathfindLayerEnum::Wall;
                best_distance = delta;
            }
        }

        for bridge in self.bridges.iter() {
            if bridge.is_point_on_bridge(pos) {
                let bridge_z = bridge.get_bridge_height(pos, None);
                let delta = (pos.z - bridge_z).abs();
                if delta < best_distance {
                    best_layer = bridge.get_layer();
                    best_distance = delta;
                }
            }
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
        self.height_map.is_clear_line_of_sight(pos1, pos2)
    }

    /// Cast against this instance's logical terrain in original XYZ space.
    pub fn cast_terrain_ray(&self, start: Vector3, end: Vector3) -> Result<TerrainRayCast, String> {
        self.height_map.cast_terrain_ray(start, end)
    }

    /// Get source filename
    pub fn get_source_filename(&self) -> &AsciiString {
        &self.filename_string
    }
}
