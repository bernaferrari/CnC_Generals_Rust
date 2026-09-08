// Live GPU overlays: shoreline, water grid, rivers, bibs, tank tracks,
// custom edging, snow flakes, heat-haze smudge, FOW shroud, flat LOD.

use super::*;
use crate::snow::{
    camera_facing_quad_corners, get_snow_manager, get_weather_setting, SnowVisibleBoxXy,
};
use crate::system::smudge::get_smudge_manager;
use super::water_tracks::{decode_wak_records, water_track_wak_path};
use super::IRegion2D as BgRegion;
use game_engine::common::ini::ini_water::get_water_transparency;
use game_engine::map_object::MAP_XY_FACTOR as MAP_XY;

const NUM_BUMP_FRAMES: i32 = 32;
const WATER_GRID_DAMP: f32 = 0.93;
const FEATHER_THICKNESS: f32 = 4.0;

impl TerrainVisualImpl {
    fn apply_tree_world_bounds(&mut self) {
        let (w, h) = self.config.world_size;
        self.tree_buffer
            .set_bounds(TreeRegion2D::new(Vec2::ZERO, Vec2::new(w.max(1.0), h.max(1.0))));
    }

    /// C++ `W3DTerrainVisual::setShoreLineDetail` / `updateShorelineTiles`.
    pub fn rebuild_shoreline(&mut self) {
        let Some(height_map) = self.height_map.as_ref() else {
            self.overlay.shoreline_tiles.clear();
            self.shoreline_meshes.clear();
            return;
        };
        let (show_soft, transparent) = {
            let show = get_global_data()
                .map(|g| g.read().show_soft_water_edge)
                .unwrap_or(true);
            let (depth, opacity) = get_water_transparency()
                .and_then(|t| t.read().ok().map(|s| {
                    let s = s.get_final_override();
                    (s.transparent_water_depth, s.min_water_opacity)
                }))
                .unwrap_or((3.0, 1.0));
            (show, depth.max(0.0) * opacity.max(0.01))
        };
        let water_y = get_global_data()
            .map(|g| g.read().water_position_z)
            .unwrap_or(0.0);
        let tiles = height_map.rebuild_shoreline_tiles(
            |x, z| self.sample_water_height(x, z).unwrap_or(water_y),
            transparent,
            show_soft,
        );
        self.overlay.shoreline_tiles = tiles;
        self.upload_shoreline_meshes();
        self.overlay.overlays_dirty = true;
    }

    /// C++ `setTerrainTracksDetail` + HeightMap flush of tread strips.
    pub fn rebuild_tank_tracks(&mut self) {
        self.set_terrain_tracks_detail();
        self.upload_tank_track_meshes();
    }

    /// C++ `BaseHeightMapRenderObjClass::unitMoved`.
    pub fn unit_moved(&mut self, unit: TreeCollisionUnit, frame: u32) {
        self.tree_buffer.unit_moved(unit, frame);
        self.tree_buffer.force_vertex_rebuild();
    }

    pub fn add_tree(
        &mut self,
        drawable_id: u32,
        location: Vec3,
        scale: f32,
        angle: f32,
        random_scale_amount: f32,
        data: crate::terrain::TreeModuleData,
        bounds: crate::terrain::TreeSphere,
    ) {
        self.tree_buffer.add_tree(
            drawable_id,
            location,
            scale,
            angle,
            random_scale_amount,
            data,
            bounds,
        );
    }

    /// C++ `W3DShroud::setShroudLevel` projected onto terrain vertices.
    pub fn set_shroud_level(&mut self, cell_x: i32, cell_y: i32, alpha: u8) {
        if self.overlay.shroud_width <= 0 || self.overlay.shroud_height <= 0 {
            return;
        }
        if cell_x < 0
            || cell_y < 0
            || cell_x >= self.overlay.shroud_width
            || cell_y >= self.overlay.shroud_height
        {
            return;
        }
        let idx = (cell_y * self.overlay.shroud_width + cell_x) as usize;
        if let Some(slot) = self.overlay.shroud_cells.get_mut(idx) {
            *slot = alpha;
            self.overlay.overlays_dirty = true;
        }
    }

    pub fn init_shroud_overlay(&mut self, width: i32, height: i32, cell_size: f32, origin: [f32; 2]) {
        let width = width.max(0);
        let height = height.max(0);
        self.overlay.shroud_width = width;
        self.overlay.shroud_height = height;
        self.overlay.shroud_cell_size = cell_size.max(1.0);
        self.overlay.shroud_origin = origin;
        self.overlay.shroud_cells = vec![255u8; (width as usize).saturating_mul(height as usize)];
        self.chunk_meshes.clear();
    }

    /// Bulk-replace the shroud overlay from a presentation R8 payload.
    ///
    /// Live feed path (per presentation frame, `RenderPipeline::
    /// update_minimap_fow_texture`): an identical payload is a no-op, and a
    /// same-size update reuses the existing cell buffer — only the shroud
    /// state changes, which `sync_shroud_dest_texture` re-uploads to the GPU
    /// each terrain update. A real cell change also invalidates the terrain
    /// chunk meshes, because terrain FOW darkening is baked into chunk vertex
    /// diffuse (`shroud_alpha_at_world`) at bake time, mirroring how the
    /// C++ `W3DShroud` destination texture feeds the per-object material
    /// passes. Oversized payloads are truncated; undersized payloads are
    /// rejected (fail-open: previous overlay state retained).
    pub fn set_shroud_overlay_r8(
        &mut self,
        width: i32,
        height: i32,
        cell_size: f32,
        origin: [f32; 2],
        data: &[u8],
    ) {
        let width = width.max(0);
        let height = height.max(0);
        let expected = (width as usize).saturating_mul(height as usize);
        if width == 0 || height == 0 || expected == 0 || data.len() < expected {
            return;
        }
        let same_size = self.overlay.shroud_width == width
            && self.overlay.shroud_height == height
            && self.overlay.shroud_cells.len() == expected;
        if same_size && self.overlay.shroud_cells.as_slice() == &data[..expected] {
            // Identical payload: nothing to rebake, nothing to re-upload.
            return;
        }
        if !same_size {
            self.init_shroud_overlay(width, height, cell_size, origin);
        } else {
            self.overlay.shroud_cell_size = cell_size.max(1.0);
            self.overlay.shroud_origin = origin;
        }
        self.overlay.shroud_cells[..expected].copy_from_slice(&data[..expected]);
        // Terrain FOW darkening is baked into chunk vertex diffuse, so a
        // real cell change must remesh the terrain chunks (see doc above).
        self.chunk_meshes.clear();
    }

    pub fn shroud_alpha_at_world(&self, world_x: f32, world_z: f32) -> f32 {
        if self.overlay.shroud_cells.is_empty()
            || self.overlay.shroud_width <= 0
            || self.overlay.shroud_cell_size <= f32::EPSILON
        {
            return 1.0;
        }
        let x = ((world_x - self.overlay.shroud_origin[0]) / self.overlay.shroud_cell_size).floor()
            as i32;
        let y = ((world_z - self.overlay.shroud_origin[1]) / self.overlay.shroud_cell_size).floor()
            as i32;
        let x = x.clamp(0, self.overlay.shroud_width - 1);
        let y = y.clamp(0, self.overlay.shroud_height - 1);
        let idx = (y * self.overlay.shroud_width + x) as usize;
        self.overlay
            .shroud_cells
            .get(idx)
            .copied()
            .unwrap_or(255) as f32
            / 255.0
    }

    pub fn set_map_water_areas(&mut self, areas: Vec<TerrainWaterArea>) {
        self.overlay.water_areas = areas;
        self.overlay.overlays_dirty = true;
        if let Some(device) = self.device.clone() {
            let _ = self.sync_polygon_water_meshes(device.as_ref());
        }
    }

    pub fn add_heat_smudge(&mut self, smudge: TerrainSmudge) {
        self.overlay.smudges.push(smudge);
        self.overlay.overlays_dirty = true;
    }

    pub fn clear_heat_smudges(&mut self) {
        self.overlay.smudges.clear();
        self.smudge_mesh = None;
    }

    fn sample_water_height(&self, world_x: f32, world_z: f32) -> Option<f32> {
        if let Some(h) = self.get_water_grid_height(world_x, world_z) {
            return Some(h);
        }
        get_global_data().map(|g| g.read().water_position_z)
    }

    fn load_water_tracks_from_map(&mut self) {
        if self.filename.is_empty() {
            return;
        }
        let wak = water_track_wak_path(&self.filename);
        let Ok(bytes) = std::fs::read(&wak) else {
            return;
        };
        if let Ok(records) = decode_wak_records(&bytes) {
            self.water_tracks.load_records(&records);
            self.flush_water_tracks();
        }
    }

    fn ingest_logic_water_areas(&mut self) {
        if !self.overlay.water_areas.is_empty() {
            return;
        }
        let Ok(logic) = gamelogic::terrain::get_terrain_logic().read() else {
            return;
        };
        let mut areas = Vec::new();
        for trigger in logic.get_trigger_areas().get_triggers() {
            if !trigger.is_water_area() || !trigger.get_should_render() {
                continue;
            }
            let mut points = Vec::new();
            for i in 0..trigger.get_num_points() {
                if let Some(p) = trigger.get_point(i) {
                    points.push([p.x as f32, p.z as f32, p.y as f32]);
                }
            }
            if points.len() >= 3 {
                areas.push(TerrainWaterArea {
                    points,
                    is_river: trigger.is_river(),
                    river_start: trigger.get_river_start(),
                });
            }
        }
        if !areas.is_empty() {
            self.overlay.water_areas = areas;
        }
    }

    fn drain_leftover_water_velocity(&mut self) {
        let Some(logic) = gamelogic::helpers::TheTerrainVisual::get() else {
            return;
        };
        for (x, y, velocity, preferred) in logic.take_water_velocity_impulses() {
            self.add_water_velocity(x, y, velocity, preferred);
        }
    }

    /// C++ `WaterRenderObjClass::update` water-grid field step
    /// (W3DWater.cpp:1220-1289): damped velocity, constant gravity*3 force
    /// toward the preferred side, and a 1.0/1.0 at-rest snap. Height clamps
    /// are NOT applied here (C++ clamps only in `changeGridHeight`).
    fn simulate_water_grid(&mut self, _dt: f32) {
        self.drain_leftover_water_velocity();
        if !self.water_grid_enabled {
            return;
        }
        let (cells_x, cells_y, _) = self.water_grid.resolution;
        if cells_x < 1.0 || cells_y < 1.0 {
            return;
        }
        const PREFERRED_HEIGHT_FUDGE: f32 = 1.0;
        const AT_REST_VELOCITY_FUDGE: f32 = 1.0;
        let gravity = get_global_data()
            .map(|g| g.read().gravity)
            .unwrap_or(-1.0);
        let max_x = cells_x as i32;
        let max_y = cells_y as i32;
        let keys: Vec<(i32, i32)> = self.water_grid.point_motions.keys().copied().collect();
        let mut any_motion = false;
        for key in keys {
            let (x, y) = key;
            if x < 0 || y < 0 || x > max_x || y > max_y {
                continue;
            }
            let Some(motion) = self.water_grid.point_motions.get(&key).copied() else {
                continue;
            };
            if !motion.in_motion {
                continue;
            }
            let height = self
                .water_grid
                .height_deltas
                .get(&key)
                .copied()
                .unwrap_or(0.0);
            // C++ W3DWater.cpp:1248-1254: damp, then a constant sign force.
            let mut velocity = motion.velocity * WATER_GRID_DAMP;
            if height < motion.preferred_height {
                velocity -= gravity * 3.0;
            } else {
                velocity += gravity * 3.0;
            }
            let new_height = height + velocity;
            if (new_height - motion.preferred_height).abs() < PREFERRED_HEIGHT_FUDGE
                && velocity.abs() < AT_REST_VELOCITY_FUDGE
            {
                // C++ W3DWater.cpp:1263-1271: settle onto the preferred height.
                self.water_grid
                    .height_deltas
                    .insert(key, motion.preferred_height);
                if let Some(m) = self.water_grid.point_motions.get_mut(&key) {
                    m.velocity = 0.0;
                    m.in_motion = false;
                }
            } else {
                self.water_grid.height_deltas.insert(key, new_height);
                if let Some(m) = self.water_grid.point_motions.get_mut(&key) {
                    m.velocity = velocity;
                }
                any_motion = true;
            }
        }
        if any_motion {
            self.overlay.water_grid_dirty = true;
        }
        self.overlay.bump_frame = (self.overlay.bump_frame + 1) % NUM_BUMP_FRAMES;
    }

    fn upload_shoreline_meshes(&mut self) {
        let Some(device) = self.device.as_ref() else {
            self.shoreline_meshes.clear();
            return;
        };
        if self.overlay.shoreline_tiles.is_empty() {
            self.shoreline_meshes.clear();
            return;
        }
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for tile in &self.overlay.shoreline_tiles {
            let base = vertices.len() as u32;
            for (i, corner) in tile.verts.iter().enumerate() {
                let a = 1.0 - tile.t[i].clamp(0.0, 1.0);
                vertices.push(WaterGpuVertex {
                    position: *corner,
                    color: [1.0, 1.0, 1.0],
                    tex_coords: [if i == 1 || i == 2 { 1.0 } else { 0.0 }, if i >= 2 { 1.0 } else { 0.0 }],
                    alpha: a,
                    packed_c: ((a * 255.0) as u32) << 24 | 0x00ff_ffff,
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        if vertices.is_empty() {
            self.shoreline_meshes.clear();
            return;
        }
        self.shoreline_meshes = vec![GpuWaterPlane {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shoreline Feather"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Shoreline Feather Indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: indices.len() as u32,
            texture_name: String::new(),
            jba: false,
            river: false,
        }];
    }


    /// C++ `WaterRenderObjClass` creates its wave grid only for actual water
    /// tables (W3DWater.cpp:2242-2343): a grid cell is wet when the water
    /// plane stands above the terrain at any corner (shoreline cells keep
    /// their sheet; dry land gets none). Without a height map the grid stays
    /// enabled — there is no terrain to occlude it.
    fn water_grid_cell_is_wet(&self, corners: &[[f32; 2]; 4], water_y: f32) -> bool {
        match self.height_map.as_ref() {
            None => true,
            Some(height_map) => corners
                .iter()
                .any(|[x, z]| water_y > height_map.get_height_at(*x, *z)),
        }
    }

    fn upload_water_grid_mesh(&mut self, device: &wgpu::Device) {
        if !self.water_grid_enabled {
            self.water_grid_mesh = None;
            return;
        }
        let (cells_x, cells_y, cell_size) = self.water_grid.resolution;
        if cells_x < 1.0 || cells_y < 1.0 || cell_size <= 0.0 {
            self.water_grid_mesh = None;
            return;
        }
        let nx = cells_x as usize + 1;
        let ny = cells_y as usize + 1;
        let base_y = self.water_grid.transform.w_axis.z;
        let origin = self.water_grid.transform.transform_point3(Vec3::ZERO);
        // C++ renderWaterMesh (W3DWater.cpp:2242-2343): repeat-scaled UVs,
        // sine-wobbled V scrolled by riverVOrigin, waterDiffuse color with
        // alpha reduced by 0x20 for the wave mesh.
        game_engine::common::ini::ini_water::initialize_water_settings();
        let tod = get_global_data()
            .map(|g| g.read().time_of_day as usize)
            .unwrap_or(0);
        let setting = game_engine::common::ini::ini_water::get_water_setting(
            game_engine::common::ini::ini_water::TimeOfDay::from_index(tod),
        )
        .and_then(|lock| lock.read().ok().map(|g| g.clone()));
        let water_diffuse = setting
            .as_ref()
            .map(|s| pack_water_setting_diffuse(s.surface_color))
            .unwrap_or(0xffff_ffff);
        let repeat = setting
            .as_ref()
            .map(|s| s.water_repeat_count.max(0) as f32)
            .unwrap_or(6.0);
        let grid_diffuse = {
            let rgb = water_diffuse & 0x00ff_ffff;
            let alpha = (water_diffuse >> 24).wrapping_sub(0x20) & 0xff;
            rgb | (alpha << 24)
        };
        let u_scale = repeat / 128.0 * cell_size / 10.0 * 0.2;
        let v_scale = u_scale;
        let rv = self.overlay.river_v_origin;
        let uv_cos_scale = 0.02 * (3.0 * rv).cos();
        let sin_offset = 25.0 * rv;
        let map_coeff = std::f32::consts::PI / (8.0 * MAP_XY);
        let mut patch = Vec::with_capacity(nx * ny);
        for j in 0..ny {
            let y = j as f32 * cell_size;
            let v1_offset = rv + j as f32 * v_scale
                + uv_cos_scale * (sin_offset + y * map_coeff).sin();
            for i in 0..nx {
                let h = self
                    .water_grid
                    .height_deltas
                    .get(&(i as i32, j as i32))
                    .copied()
                    .unwrap_or(0.0);
                let wx = origin.x + i as f32 * cell_size;
                let wz = origin.z + j as f32 * cell_size;
                patch.push(crate::terrain::SeaPatchVertex {
                    x: wx,
                    y: base_y + h,
                    z: wz,
                    c: grid_diffuse,
                    tu: i as f32 * u_scale,
                    tv: v1_offset,
                });
            }
        }
        let mut indices = Vec::new();
        for j in 0..ny.saturating_sub(1) {
            for i in 0..nx.saturating_sub(1) {
                // C++ W3DWater.cpp:2242-2343: the wave grid only exists over
                // actual water tables — a dry cell gets no geometry instead
                // of drawing a water sheet over land.
                let corners = [
                    [origin.x + i as f32 * cell_size, origin.z + j as f32 * cell_size],
                    [
                        origin.x + (i + 1) as f32 * cell_size,
                        origin.z + j as f32 * cell_size,
                    ],
                    [
                        origin.x + (i + 1) as f32 * cell_size,
                        origin.z + (j + 1) as f32 * cell_size,
                    ],
                    [
                        origin.x + i as f32 * cell_size,
                        origin.z + (j + 1) as f32 * cell_size,
                    ],
                ];
                if !self.water_grid_cell_is_wet(&corners, base_y) {
                    continue;
                }
                let i0 = (j * nx + i) as u32;
                let i1 = i0 + 1;
                let i2 = i0 + nx as u32 + 1;
                let i3 = i0 + nx as u32;
                indices.extend_from_slice(&[i0, i1, i2, i0, i2, i3]);
            }
        }
        if patch.is_empty() || indices.is_empty() {
            self.water_grid_mesh = None;
            return;
        }
        let vertices = fill_water_gpu_upload_vertices(&patch);
        self.water_grid_mesh = Some(GpuWaterPlane {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Water Grid Mesh"),
                contents: cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Water Grid Indices"),
                contents: cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: indices.len() as u32,
            texture_name: String::new(),
            jba: false,
            river: false,
        });
        self.overlay.water_grid_dirty = false;
    }

    fn standing_water_diffuse_packed(&self) -> u32 {
        game_engine::common::ini::ini_water::initialize_water_settings();
        let standing_color = get_water_transparency()
            .and_then(|lock| {
                lock.read().ok().map(|g| {
                    let s = g.get_final_override();
                    [
                        s.standing_water_color.0,
                        s.standing_water_color.1,
                        s.standing_water_color.2,
                    ]
                })
            })
            .unwrap_or([1.0, 1.0, 1.0]);
        let tod = get_global_data()
            .map(|g| g.read().time_of_day as usize)
            .unwrap_or(0);
        let water_set = game_engine::common::ini::ini_water::get_water_setting(
            game_engine::common::ini::ini_water::TimeOfDay::from_index(tod),
        )
        .and_then(|lock| lock.read().ok().map(|g| g.clone()));
        let water_diffuse = water_set
            .as_ref()
            .map(|s| pack_water_setting_diffuse(s.surface_color))
            .unwrap_or(0xffff_ffff);
        compute_standing_water_diffuse(
            standing_color,
            water_diffuse,
            self.ambient_color,
            &[WaterTerrainLight {
                light_pos: [
                    self.sun_direction.x,
                    self.sun_direction.z,
                    self.sun_direction.y,
                ],
                diffuse: self.sun_color,
            }],
        )
    }

    fn sync_polygon_water_meshes(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        self.ingest_logic_water_areas();
        self.polygon_water_meshes.clear();
        if self.overlay.water_areas.is_empty() {
            return Ok(());
        }
        let feather = get_global_data()
            .map(|g| g.read().feather_water.max(0))
            .unwrap_or(0);
        let diffuse = self.standing_water_diffuse_packed();
        let river_v = self.overlay.river_v_origin;
        let layers = if feather > 0 { feather.min(5) } else { 1 };
        for area in &self.overlay.water_areas {
            if area.points.len() < 3 {
                continue;
            }
            if area.is_river {
                let (verts, indices) =
                    bake_river_strip(&area.points, area.river_start, river_v, diffuse);
                if verts.is_empty() || indices.is_empty() {
                    continue;
                }
                let gpu = fill_water_gpu_upload_vertices(&verts);
                if let Some(mut mesh) =
                    Self::upload_water_overlay(device, "Polygon Water", &gpu, &indices)
                {
                    mesh.jba = true;
                    mesh.river = true;
                    self.polygon_water_meshes.push(mesh);
                }
                continue;
            }
            let n = area.points.len();
            let mut k = 1usize;
            while k + 1 < n {
                let pt3 = area.points[0];
                let pt2 = area.points[k];
                let pt1 = area.points[k + 1];
                let pt0 = if k + 2 < n {
                    area.points[k + 2]
                } else {
                    area.points[k + 1]
                };
                let quad = [pt0, pt1, pt2, pt3];
                for layer in 0..layers {
                    let z_off = if feather > 0 {
                        layer as f32 * (FEATHER_THICKNESS / feather as f32)
                    } else {
                        0.0
                    };
                    let (verts, indices) =
                        bake_trapezoid_water(&quad, z_off, river_v, diffuse, feather);
                    if verts.is_empty() || indices.is_empty() {
                        continue;
                    }
                    let gpu = fill_water_gpu_upload_vertices(&verts);
                    if let Some(mut mesh) =
                        Self::upload_water_overlay(device, "Polygon Water", &gpu, &indices)
                    {
                        mesh.jba = true;
                        self.polygon_water_meshes.push(mesh);
                    }
                }
                k += 2;
            }
        }
        Ok(())
    }

    fn upload_water_overlay(
        device: &wgpu::Device,
        label: &str,
        vertices: &[WaterGpuVertex],
        indices: &[u32],
    ) -> Option<GpuWaterPlane> {
        if vertices.is_empty() || indices.is_empty() {
            return None;
        }
        Some(GpuWaterPlane {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: indices.len() as u32,
            texture_name: String::new(),
            jba: false,
            river: false,
        })
    }

    fn upload_bib_meshes(&mut self) {
        let Some(device) = self.device.clone() else {
            self.bib_meshes.clear();
            return;
        };
        if self.terrain_bibs.is_empty() {
            self.bib_meshes.clear();
            return;
        }
        // C++ W3DBibBuffer::renderBibs (W3DBibBuffer.cpp:392-413): bibs draw
        // alpha-blended through `detailAlphaShader` with TBBib.tga for normal
        // bibs and TBRedBib.tga for highlight bibs (CLAMP addressing,
        // :208-213). Resolve both before baking so the vertex tint/alpha can
        // stand in for the missing art (this install ships neither TGA).
        self.ensure_bib_texture_bind_groups();
        let art_bound = self.bib_art_bound();
        let height_at = |x: f32, z: f32| {
            self.height_map
                .as_ref()
                .map(|hm| hm.get_height_at(x, z))
                .unwrap_or(0.0)
                + ROAD_FLOAT_AMOUNT
        };
        // C++ loadBibsInVertexAndIndexBuffers (W3DBibBuffer.cpp:126-156):
        // per-bib UVs are (0,1) (1,1) (1,0) (0,0) in corner order.
        const BIB_UVS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let mut normal_vertices = Vec::new();
        let mut normal_indices = Vec::new();
        let mut highlight_vertices = Vec::new();
        let mut highlight_indices = Vec::new();
        for bib in &self.terrain_bibs {
            let (vertices, indices) = if bib.highlight {
                (&mut highlight_vertices, &mut highlight_indices)
            } else {
                (&mut normal_vertices, &mut normal_indices)
            };
            let base = vertices.len() as u32;
            let (color, alpha) = bib_stand_in_appearance(bib.highlight, art_bound);
            for (i, corner) in bib.corners.iter().enumerate() {
                let y = height_at(corner[0], corner[2]);
                vertices.push(OverlayGpuVertex {
                    position: [corner[0], y, corner[2]],
                    color,
                    tex_coords: BIB_UVS[i],
                    road_width: alpha,
                    diffuse: 0xffff_ffff,
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        let mut meshes = Vec::new();
        if let Some(mut mesh) =
            Self::upload_overlay_mesh(&device, "Faction Bibs", &normal_vertices, &normal_indices)
        {
            mesh.bib_highlight = false;
            meshes.push(mesh);
        }
        if let Some(mut mesh) = Self::upload_overlay_mesh(
            &device,
            "Faction Bibs Highlight",
            &highlight_vertices,
            &highlight_indices,
        ) {
            mesh.bib_highlight = true;
            meshes.push(mesh);
        }
        self.bib_meshes = meshes;
    }

    /// C++ `W3DPropBuffer::drawProps` renders every `TerrainPropRecord` as a
    /// full W3D model; this port has no prop models, so each prop draws as a
    /// crossed-card stand-in (two quads, the tree-buffer convention)
    /// height-snapped to the terrain and tinted deterministically from the
    /// prop name hash. Drawn through the alpha-blended overlay pipeline so
    /// the cards read as scenery rather than collision geometry.
    fn upload_prop_meshes(&mut self) {
        const PROP_CARD_UVS: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        let Some(device) = self.device.as_ref() else {
            self.prop_meshes.clear();
            return;
        };
        if self.terrain_props.is_empty() {
            self.prop_meshes.clear();
            return;
        }
        let height_at = |x: f32, z: f32| {
            self.height_map
                .as_ref()
                .map(|hm| hm.get_height_at(x, z))
                .unwrap_or(0.0)
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for prop in &self.terrain_props {
            let [px, _, pz] = prop.position;
            let y = height_at(px, pz);
            // C++ W3DPropBuffer scales each prop about its model size; the
            // stand-in card derives its footprint from the same scale field.
            let size = (prop.scale.max(0.1) * 10.0).min(40.0);
            let half = size * 0.5;
            let (sin, cos) = prop.angle.sin_cos();
            // Deterministic tint: hash the model name into a green-brown band
            // (rocks vs bushes) the way the tree buffer tints stand-ins.
            let mut hash: u32 = 0x811c_9dc5;
            for byte in prop.model_name.bytes() {
                hash = hash.wrapping_mul(0x0100_0193).wrapping_add(byte as u32);
            }
            let t = (hash & 0xff) as f32 / 255.0;
            let tint = [
                0.30 + 0.20 * t,
                0.34 + 0.16 * t,
                0.22 + 0.10 * (1.0 - t),
            ];
            for axis in 0..2 {
                let (dx, dz) = if axis == 0 {
                    (cos * half, sin * half)
                } else {
                    (-sin * half, cos * half)
                };
                let corners = [
                    [px - dx, y, pz - dz],
                    [px + dx, y, pz + dz],
                    [px + dx, y + size, pz + dz],
                    [px - dx, y + size, pz - dz],
                ];
                let base = vertices.len() as u32;
                for (i, corner) in corners.iter().enumerate() {
                    vertices.push(OverlayGpuVertex {
                        position: *corner,
                        color: tint,
                        tex_coords: PROP_CARD_UVS[i],
                        road_width: 0.9,
                        diffuse: 0xffff_ffff,
                    });
                }
                indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        self.prop_meshes = Self::upload_overlay_mesh(device, "Terrain Props", &vertices, &indices)
            .into_iter()
            .collect();
    }

    /// True when at least one real bib TGA resolved (bind groups keyed
    /// `bib_normal` / `bib_highlight` in `water_named_bind_groups`).
    fn bib_art_bound(&self) -> bool {
        self.water_named_bind_groups.contains_key("bib_highlight")
            || self.water_named_bind_groups.contains_key("bib_normal")
    }

    /// C++ W3DBibBuffer constructor (W3DBibBuffer.cpp:208-213): TBBib.tga +
    /// TBRedBib.tga with CLAMP addressing. Cached by name; retried only when
    /// the bind groups are missing (map/device reset clears the cache).
    fn ensure_bib_texture_bind_groups(&mut self) {
        const BIB_TEXTURES: [(&str, &str); 2] = [
            ("bib_normal", "TBBib.tga"),
            ("bib_highlight", "TBRedBib.tga"),
        ];
        if BIB_TEXTURES
            .iter()
            .all(|(key, _)| self.water_named_bind_groups.contains_key(*key))
        {
            return;
        }
        let Some(device) = self.device.clone() else {
            return;
        };
        let Some(layout) = self.road_texture_bind_group_layout.clone() else {
            return;
        };
        let device = device.as_ref();
        for (key, file) in BIB_TEXTURES {
            if self.water_named_bind_groups.contains_key(key) {
                continue;
            }
            let Some((texture, source)) =
                self.load_first_available_named_overlay_texture(device, file)
            else {
                // Asset gap (documented): TBBib.tga / TBRedBib.tga are absent
                // from this install. Without the real art the bib reads as
                // the translucent stand-in tint baked into vertex color.
                log::debug!("bib texture {file} not found ({key}); vertex-color fallback");
                continue;
            };
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
            // C++ sets CLAMP addressing on both bib textures (:210-213).
            let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Faction Bib Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(&format!("Faction Bib {file} Bind Group")),
                layout: layout.as_ref(),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            });
            self.water_named_bind_groups.insert(
                key.to_string(),
                NamedWaterBind {
                    _texture: texture,
                    bind_group,
                },
            );
            let _ = source;
        }
    }

    fn upload_tank_track_meshes(&mut self) {
        let Some(device) = self.device.clone() else {
            self.tank_track_meshes.clear();
            return;
        };
        if self.height_map.is_none() {
            self.tank_track_meshes.clear();
            return;
        }
        let flush = self.terrain_tracks.flush(0xffff_ffff);
        if flush.vertices.is_empty() || flush.indices.is_empty() {
            self.tank_track_meshes.clear();
            return;
        }
        let gpu: Vec<OverlayGpuVertex> = flush
            .vertices
            .iter()
            .map(|v| OverlayGpuVertex::from_cpp_xyzduv(v.x, v.y, v.z, v.diffuse, v.u1, v.v1))
            .collect();
        let indices: Vec<u32> = flush.indices.iter().map(|i| *i as u32).collect();
        self.tank_track_meshes =
            Self::upload_overlay_mesh(device.as_ref(), "Tank Tracks", &gpu, &indices)
                .into_iter()
                .collect();
    }

    fn upload_custom_edge_meshes(&mut self) {
        let Some(device) = self.device.as_ref() else {
            self.custom_edge_meshes.clear();
            return;
        };
        let Some(height_map) = self.height_map.as_ref() else {
            self.custom_edge_meshes.clear();
            return;
        };
        let scale = if height_map.scale.abs() > f32::EPSILON {
            height_map.scale
        } else {
            MAP_XY
        };
        let border = height_map.border_size as f32;
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let max_x = height_map.width.saturating_sub(1) as i32;
        let max_y = height_map.height.saturating_sub(1) as i32;
        for j in 0..max_y {
            for i in 0..max_x {
                let blend = height_map.get_blend_tile_index(i, j);
                if blend <= 0 {
                    continue;
                }
                let info = height_map
                    .blended_tiles
                    .get(blend as usize)
                    .or_else(|| height_map.extra_blended_tiles.get(blend as usize));
                let Some(info) = info else {
                    continue;
                };
                if info.custom_blend_edge_class < 0 {
                    continue;
                }
                let x0 = (i as f32 - border) * scale;
                let z0 = (j as f32 - border) * scale;
                let x1 = ((i + 1) as f32 - border) * scale;
                let z1 = ((j + 1) as f32 - border) * scale;
                let p0 = height_map.world_height_at_index(i as u32, j as u32);
                let p1 = height_map.world_height_at_index((i + 1) as u32, j as u32);
                let p2 = height_map.world_height_at_index((i + 1) as u32, (j + 1) as u32);
                let p3 = height_map.world_height_at_index(i as u32, (j + 1) as u32);
                let uv = height_map.get_uv_data(i, j, false);
                let base = vertices.len() as u32;
                let corners = [
                    ([x0, p0, z0], [uv.u[0], uv.v[0]]),
                    ([x1, p1, z0], [uv.u[1], uv.v[1]]),
                    ([x1, p2, z1], [uv.u[2], uv.v[2]]),
                    ([x0, p3, z1], [uv.u[3], uv.v[3]]),
                ];
                for (pos, tex) in corners {
                    vertices.push(OverlayGpuVertex {
                        position: pos,
                        color: [1.0, 1.0, 1.0],
                        tex_coords: tex,
                        road_width: 1.0,
                        diffuse: 0x80ff_ffff,
                    });
                }
                if uv.flip {
                    indices.extend_from_slice(&[base + 1, base + 3, base, base + 1, base + 2, base + 3]);
                } else {
                    indices.extend_from_slice(&[base, base + 2, base + 3, base, base + 1, base + 2]);
                }
            }
        }
        self.custom_edge_meshes =
            Self::upload_overlay_mesh(device, "Custom Edging", &vertices, &indices)
                .into_iter()
                .collect();
    }

    fn upload_snow_mesh(&mut self, camera: Vec3, view_matrix: &Mat4) {
        let Some(device) = self.device.as_ref() else {
            self.snow_mesh = None;
            return;
        };
        let enabled = get_weather_setting()
            .and_then(|s| s.read().ok().map(|g| g.snow_enabled))
            .unwrap_or(false);
        if !enabled {
            self.snow_mesh = None;
            return;
        }
        let Some(manager) = get_snow_manager() else {
            self.snow_mesh = None;
            return;
        };
        let Ok(guard) = manager.lock() else {
            self.snow_mesh = None;
            return;
        };
        if !guard.is_visible() {
            self.snow_mesh = None;
            return;
        }
        let (camera_y_up, right, up, view_is_z_up) =
            Self::snow_camera_axes_y_up(camera, view_matrix);
        let visible_xy = self.snow_terrain_visible_xy();
        let flakes = guard.flake_positions_y_up_clipped(camera_y_up, visible_xy);
        let quad = guard.quad_size();
        drop(guard);
        if flakes.is_empty() {
            self.snow_mesh = None;
            return;
        }
        let mut vertices = Vec::with_capacity(flakes.len() * 4);
        let mut indices = Vec::with_capacity(flakes.len() * 6);
        let right_a = [right.x, right.y, right.z];
        let up_a = [up.x, up.y, up.z];
        for flake in flakes {
            // Overlay vertices must live in the same space as `view_matrix`.
            let center = if view_is_z_up {
                [flake[0], flake[2], flake[1]]
            } else {
                flake
            };
            let base = vertices.len() as u32;
            for (pos, uv) in camera_facing_quad_corners(center, right_a, up_a, quad) {
                vertices.push(OverlayGpuVertex {
                    position: pos,
                    color: [1.0, 1.0, 1.0],
                    tex_coords: uv,
                    road_width: 1.0,
                    diffuse: 0xc0ff_ffff,
                });
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        self.snow_mesh = Self::upload_overlay_mesh(device, "Snow Flakes", &vertices, &indices);
    }

    /// Eye + billboard axes. Overlay GPU is Y-up; the live view may still be
    /// C++ Z-up (`look_at` with world up = Z).
    fn snow_camera_axes_y_up(camera: Vec3, view_matrix: &Mat4) -> ([f32; 3], Vec3, Vec3, bool) {
        let inv = view_matrix.inverse();
        let right = inv.transform_vector3(Vec3::X);
        let up = inv.transform_vector3(Vec3::Y);
        let y_as_cam_up = view_matrix.transform_vector3(Vec3::Y).y.abs();
        let z_as_cam_up = view_matrix.transform_vector3(Vec3::Z).y.abs();
        let view_is_z_up = z_as_cam_up > y_as_cam_up;
        if view_is_z_up {
            ([camera.x, camera.z, camera.y], right, up, true)
        } else {
            ([camera.x, camera.y, camera.z], right, up, false)
        }
    }

    /// C++ `getMaximumVisibleBox(frustum, &bbox, TRUE)` X/Y for snow clip.
    /// Uses the height map min plane so we do not re-lock `THE_TERRAIN_VISUAL`.
    fn snow_terrain_visible_xy(&self) -> Option<SnowVisibleBoxXy> {
        let min_height = self
            .height_map
            .as_ref()
            .map(|h| h.min_height)
            .unwrap_or(0.0);
        crate::display::view::with_tactical_view_ref(|view| {
            let cam = view.get_3d_camera_position();
            let target = view.position();
            let aspect = (view.width() as f32 / view.height().max(1) as f32).max(0.01);
            let visible = crate::display::shadow_pass::maximum_visible_box(
                [cam.x, cam.y, cam.z],
                [target.x, target.y, target.z],
                1.0,
                20000.0,
                crate::display::view::vertical_fov_from_horizontal(view.field_of_view(), aspect),
                aspect,
                min_height,
            );
            Some(SnowVisibleBoxXy {
                center_x: visible.center[0],
                center_y: visible.center[1],
                extent_x: visible.extent[0],
                extent_y: visible.extent[1],
            })
        })
    }


    fn upload_smudge_mesh(&mut self) {
        let Some(device) = self.device.as_ref() else {
            self.smudge_mesh = None;
            return;
        };
        let mut smudges = self.overlay.smudges.clone();
        if let Ok(mgr) = get_smudge_manager().lock() {
            for s in mgr.collect_used_smudges() {
                if s.size > 0.0 && s.opacity > 0.0 {
                    smudges.push(TerrainSmudge {
                        position: [s.pos.x, s.pos.y, s.pos.z],
                        offset: [s.offset.x, s.offset.y],
                        size: s.size,
                        opacity: s.opacity,
                    });
                }
            }
        }
        if smudges.is_empty() {
            self.smudge_mesh = None;
            return;
        }
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        for smudge in &smudges {
            let half = smudge.size.max(0.1) * 0.5;
            let y = smudge.position[1] + 0.05;
            let ox = smudge.offset[0];
            let oz = smudge.offset[1];
            // C++ 5-vertex quad: 4 corners + center UV offset.
            let cx = smudge.position[0];
            let cz = smudge.position[2];
            let corners = [
                [cx - half, y, cz - half],
                [cx + half, y, cz - half],
                [cx + half, y, cz + half],
                [cx - half, y, cz + half],
                [cx + ox, y, cz + oz],
            ];
            let uvs = [
                [0.0, 0.0],
                [1.0, 0.0],
                [1.0, 1.0],
                [0.0, 1.0],
                [0.5 + ox * 0.15, 0.5 + oz * 0.15],
            ];
            let base = vertices.len() as u32;
            for i in 0..5 {
                vertices.push(OverlayGpuVertex {
                    position: corners[i],
                    color: [1.0, 0.85, 0.7],
                    tex_coords: uvs[i],
                    road_width: smudge.opacity,
                    diffuse: ((smudge.opacity * 180.0) as u32) << 24 | 0x00c0_a080,
                });
            }
            // Two triangles per corner pair through the warped center.
            indices.extend_from_slice(&[
                base,
                base + 1,
                base + 4,
                base + 1,
                base + 2,
                base + 4,
                base + 2,
                base + 3,
                base + 4,
                base + 3,
                base,
                base + 4,
            ]);
        }
        self.smudge_mesh = Self::upload_overlay_mesh(device, "Heat Smudge", &vertices, &indices);
    }

    fn upload_flat_lod_meshes(&mut self) {
        let Some(device) = self.device.as_ref() else {
            self.flat_lod_meshes.clear();
            return;
        };
        let use_flat = matches!(
            self.lod_setting,
            TerrainVisualLOD::Min | TerrainVisualLOD::Disable | TerrainVisualLOD::StretchNoClouds
        );
        if !use_flat {
            self.flat_lod_meshes.clear();
            return;
        }
        let Some(height_map) = self.height_map.as_ref() else {
            self.flat_lod_meshes.clear();
            return;
        };
        struct MapRef<'a>(&'a HeightMap);
        impl TerrainBackgroundHeightMap for MapRef<'_> {
            fn x_extent(&self) -> i32 {
                self.0.width as i32
            }
            fn y_extent(&self) -> i32 {
                self.0.height as i32
            }
            fn height(&self, x: i32, y: i32) -> i32 {
                self.0.get_raw_height(x, y) as i32
            }
            fn static_diffuse(&self, _x: i32, _y: i32) -> u32 {
                0xffff_ffff
            }
            fn border_size_inline(&self) -> i32 {
                self.0.border_size
            }
        }
        let mut bg = W3DTerrainBackground::default();
        let w = height_map.width.min(17) as i32;
        bg.allocate_terrain_buffers(&MapRef(height_map), 0, 0, w);
        let buffers = bg.do_tessellated_update(
            BgRegion::new(0, 0, height_map.width as i32, height_map.height as i32),
            &MapRef(height_map),
            true,
        );
        if buffers.vertices.is_empty() || buffers.indices.is_empty() {
            self.flat_lod_meshes.clear();
            return;
        }
        let gpu: Vec<OverlayGpuVertex> = buffers
            .vertices
            .iter()
            .map(|v| OverlayGpuVertex::from_cpp_xyzduv(v.x, v.y, v.z, v.diffuse, v.u1, v.v1))
            .collect();
        let indices: Vec<u32> = buffers.indices.iter().map(|i| *i as u32).collect();
        self.flat_lod_meshes =
            Self::upload_overlay_mesh(device, "Flat LOD Tiles", &gpu, &indices)
                .into_iter()
                .collect();
    }

    fn record_overlay_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(road_pipeline), Some(camera_bg)) = (
            self.road_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };
        let extra: Vec<&GpuRoadMesh> = self
            .tank_track_meshes
            .iter()
            .chain(self.custom_edge_meshes.iter())
            .chain(self.prop_meshes.iter())
            .chain(self.flat_lod_meshes.iter())
            .chain(self.smudge_mesh.iter())
            .collect();
        if let Some(road_bg) = self.road_texture_bind_group.as_ref() {
            if !extra.is_empty() {
                pass.set_pipeline(road_pipeline);
                pass.set_bind_group(0, camera_bg, &[]);
                pass.set_bind_group(1, road_bg, &[]);
                for mesh in extra {
                    pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                    pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..mesh.index_count, 0, 0..1);
                }
            }
        }
        // C++ W3DBibBuffer::renderBibs (W3DBibBuffer.cpp:404-412): normal
        // bibs draw under TBBib.tga, then highlight bibs under TBRedBib.tga.
        // Without resolved bib art the meshes fall back to the road bind
        // group and the baked stand-in tint/alpha.
        if !self.bib_meshes.is_empty() {
            pass.set_pipeline(road_pipeline);
            pass.set_bind_group(0, camera_bg, &[]);
            for mesh in &self.bib_meshes {
                let bib_bg = if mesh.bib_highlight {
                    self.water_named_bind_groups
                        .get("bib_highlight")
                        .or_else(|| self.water_named_bind_groups.get("bib_normal"))
                } else {
                    self.water_named_bind_groups.get("bib_normal")
                }
                .map(|named| &named.bind_group)
                .or(self.road_texture_bind_group.as_ref());
                let Some(bg) = bib_bg else {
                    continue;
                };
                pass.set_bind_group(1, bg, &[]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        if let (Some(snow), Some(snow_bg), Some(snow_pipeline)) = (
            self.snow_mesh.as_ref(),
            self.snow_texture_bind_group.as_ref(),
            self.snow_pipeline.as_ref(),
        ) {
            pass.set_pipeline(snow_pipeline);
            pass.set_bind_group(0, camera_bg, &[]);
            pass.set_bind_group(1, snow_bg, &[]);
            pass.set_vertex_buffer(0, snow.vertex_buffer.slice(..));
            pass.set_index_buffer(snow.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..snow.index_count, 0, 0..1);
        }
    }
    fn rebuild_all_overlays(&mut self) {
        self.upload_shoreline_meshes();
        if let Some(device) = self.device.clone() {
            self.upload_water_grid_mesh(device.as_ref());
            let _ = self.sync_polygon_water_meshes(device.as_ref());
        }
        self.upload_bib_meshes();
        self.upload_prop_meshes();
        self.upload_tank_track_meshes();
        self.upload_custom_edge_meshes();
        self.upload_smudge_mesh();
        self.upload_flat_lod_meshes();
        self.overlay.overlays_dirty = false;
    }
    /// C++ sort-order parity: the shoreline feather tiles belong to the terrain
    /// render (BaseHeightMap shoreline blend), drawn before scene objects.
    pub fn record_shoreline_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(water_pipeline), Some(camera_bg), Some(water_bg)) = (
            self.water_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
            self.water_texture_bind_group.as_ref(),
        ) else {
            return;
        };
        if self.shoreline_meshes.is_empty() {
            return;
        }
        pass.set_pipeline(water_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);
        pass.set_bind_group(1, water_bg, &[]);
        for mesh in &self.shoreline_meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    /// C++ `WaterRenderObjClass::renderWater` + `renderWaterMesh`: river strips
    /// (drawRiverWater), standing-water trapezoids (drawTrapezoidWater) and the
    /// animated water grid mesh. Rivers use the JBA river pixel shader with the
    /// alpha-edge stage; trapezoids use the flat-water trapezoid shader.
    pub fn record_extra_water_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let Some(camera_bg) = self.terrain_camera_bind_group.as_ref() else {
            return;
        };

        let river_pipeline = if self.water_additive_blend {
            self.river_gpu
                .river_additive_pipeline
                .as_ref()
                .or(self.river_gpu.pipeline.as_ref())
        } else {
            self.river_gpu.pipeline.as_ref()
        };
        let trapezoid_pipeline = if self.water_additive_blend {
            self.river_gpu
                .trapezoid_additive_pipeline
                .as_ref()
                .or(self.river_gpu.trapezoid_pipeline.as_ref())
        } else {
            self.river_gpu.trapezoid_pipeline.as_ref()
        };
        if let (Some(river_pipeline), Some(river_bg)) = (
            river_pipeline,
            self.river_gpu.bind_group.as_ref(),
        ) {
            let mut started = false;
            for mesh in self.polygon_water_meshes.iter().filter(|m| m.river) {
                if !started {
                    pass.set_pipeline(river_pipeline);
                    pass.set_bind_group(0, camera_bg, &[]);
                    pass.set_bind_group(1, river_bg, &[]);
                    started = true;
                }
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
        if let (Some(trapezoid_pipeline), Some(trapezoid_bg)) = (
            trapezoid_pipeline,
            self.river_gpu.trapezoid_bind_group.as_ref(),

        ) {
            // C++ renderWaterMesh draws with setupFlatWaterShader (trapezoid
            // shader); the trapezoid bind group carries `is_trapezoid = 1`
            // (no river alpha-edge stage).
            let mut started = false;
            for mesh in self
                .polygon_water_meshes
                .iter()
                .filter(|m| m.jba && !m.river)
                .chain(self.water_grid_mesh.iter())
            {
                if !started {
                    pass.set_pipeline(trapezoid_pipeline);
                    pass.set_bind_group(0, camera_bg, &[]);
                    pass.set_bind_group(1, trapezoid_bg, &[]);
                    started = true;
                }
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
    }
}

/// Vertex tint + alpha for a faction bib. With the C++ art (TBBib.tga /
/// TBRedBib.tga, W3DBibBuffer.cpp:208-209) the texture carries everything and
/// the tint stays white; without it the bib reads as a translucent stand-in —
/// red for the placement highlight, pale for a normal bib — through the
/// `detailAlphaShader`-style blend.
fn bib_stand_in_appearance(highlight: bool, art_bound: bool) -> ([f32; 3], f32) {
    match (highlight, art_bound) {
        (true, false) => ([1.0, 0.2, 0.2], 0.45),
        (true, true) => ([1.0, 1.0, 1.0], 1.0),
        (false, false) => ([0.95, 0.93, 0.9], 0.45),
        (false, true) => ([1.0, 1.0, 1.0], 1.0),
    }
}

const WATER_UV_FACTOR: f32 = 150.0;
const HEIGHT_TO_USE: f32 = 0.5;

/// C++ `loadSetting` packs `RGBAColorInt` as A<<24 | R<<16 | G<<8 | B with
/// raw 0-255 channels. The parsed Water.ini value keeps the raw scale, but
/// the not-yet-loaded INI default is a 0-1 float tuple — packing that raw
/// produced (0,0,1,1) near-black water. Detect the 0-1 scale and rescale.
/// Pathological 0-255 colors with every channel <= 1 do not occur in
/// retail Water.ini (minimum channel is 32).
fn pack_water_setting_diffuse(color: (f32, f32, f32, f32)) -> u32 {
    let scaled = if color.0 <= 1.0 && color.1 <= 1.0 && color.2 <= 1.0 && color.3 <= 1.0 {
        (
            color.0 * 255.0,
            color.1 * 255.0,
            color.2 * 255.0,
            color.3 * 255.0,
        )
    } else {
        color
    };
    let r = scaled.0.round().clamp(0.0, 255.0) as u32;
    let g = scaled.1.round().clamp(0.0, 255.0) as u32;
    let b = scaled.2.round().clamp(0.0, 255.0) as u32;
    let a = scaled.3.round().clamp(0.0, 255.0) as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

/// C++ `WaterRenderObjClass::drawTrapezoidWater` — authored Z + world UVs.
/// With `feather > 0` this is the C++ "WAVY_WATER" feather branch: per-layer
/// alpha table (W3DWater.cpp:3152-3156), `z += (sin(phase)-1)*0.5` wave and
/// wave-perturbed UVs (W3DWater.cpp:3178-3184); otherwise the flat branch
/// (W3DWater.cpp:3230-3235).
fn bake_trapezoid_water(
    points: &[[f32; 3]; 4],
    z_off: f32,
    v_origin: f32,
    diffuse: u32,
    feather: i32,
) -> (Vec<crate::terrain::SeaPatchVertex>, Vec<u32>) {
    let origin = Vec3::new(points[0][0], points[0][1] + z_off, points[0][2]);
    let p1 = Vec3::new(points[1][0], points[1][1], points[1][2]);
    let p2 = Vec3::new(points[2][0], points[2][1], points[2][2]);
    let p3 = Vec3::new(points[3][0], points[3][1], points[3][2]);
    let u_vec1 = p1 - origin;
    let v_vec1 = p3 - origin;
    let u_vec2 = p2 - p3;
    let v_vec2 = p2 - p1;
    let mut u_count = ((u_vec1.length() + u_vec2.length()) / (8.0 * MAP_XY)).floor() as i32;
    let mut v_count = ((v_vec1.length() + v_vec2.length()) / (8.0 * MAP_XY)).floor() as i32;
    if u_count < 1 {
        u_count = 1;
    }
    if v_count < 1 {
        v_count = 1;
    }
    u_count = u_count.min(50);
    v_count = v_count.min(50);
    u_count += 1;
    v_count += 1;
    // Feather layers swap the vertex alpha for the layer-count table and add
    // the traveling wave to Z and UVs; flat water keeps the ini diffuse alpha.
    let (vert_diffuse, feather_wave) = if feather > 0 {
        let alpha: i32 = match feather {
            5 => 80,
            4 => 110,
            3 => 140,
            2 => 200,
            _ => 255,
        };
        ((diffuse & 0x00ff_ffff) | ((alpha as u32) << 24), true)
    } else {
        (diffuse, false)
    };
    let const_a = 0.02 * (11.0 * v_origin).cos();
    let const_b = 0.02 * (5.0 * v_origin).cos();
    let const_c = 25.0 * v_origin;
    let const_d = std::f32::consts::PI / (4.0 * MAP_XY);
    let oo_water = 1.0 / WATER_UV_FACTOR;
    let du_step = 1.0 / (u_count - 1) as f32;
    let dv_step = 1.0 / (v_count - 1) as f32;
    let mut verts = Vec::with_capacity((u_count * v_count) as usize);
    for j in 0..v_count {
        let dv = j as f32 * dv_step;
        for i in 0..u_count {
            let du = i as f32 * du_step;
            let vertex = origin + u_vec1 * du + v_vec1 * dv + (v_vec2 - v_vec1) * (dv * du);
            // C++ feather branch: wave displaces Z and the wave height feeds
            // back into U/V; flat branch: plain sine-scroll UVs.
            let (tu, tv, y) = if feather_wave {
                let wave = ((const_c + vertex.x * const_d).sin() - 1.0) * 0.5;
                (
                    vertex.x * oo_water + const_a * wave,
                    vertex.z * oo_water + const_b * wave,
                    vertex.y + wave,
                )
            } else {
                (
                    vertex.x * oo_water + const_a * (const_c + vertex.x * const_d).sin(),
                    vertex.z * oo_water + const_b * (const_c + vertex.z * const_d).sin(),
                    vertex.y,
                )
            };
            verts.push(crate::terrain::SeaPatchVertex {
                x: vertex.x,
                y,
                z: vertex.z,
                c: vert_diffuse,
                tu,
                tv,
            });
        }
    }
    let mut indices = Vec::new();
    let stride = u_count as u32;
    for j in 0..(v_count - 1) as u32 {
        for i in 0..(u_count - 1) as u32 {
            let i0 = j * stride + i;
            let i1 = (j + 1) * stride + i + 1;
            let i2 = (j + 1) * stride + i;
            let i3 = j * stride + i + 1;
            indices.extend_from_slice(&[i0, i1, i2, i0, i3, i1]);
        }
    }
    (verts, indices)
}

/// C++ `WaterRenderObjClass::drawRiverWater` — bank pairs from `riverStart`.
fn bake_river_strip(
    points: &[[f32; 3]],
    river_start: i32,
    v_origin: f32,
    diffuse: u32,
) -> (Vec<crate::terrain::SeaPatchVertex>, Vec<u32>) {
    let n = points.len();
    if n < 4 {
        return (Vec::new(), Vec::new());
    }
    if river_start < 0 || river_start as usize >= n.saturating_sub(1) {
        return (Vec::new(), Vec::new());
    }
    let pair_count = n / 2;
    let rectangle_count = pair_count.saturating_sub(1);
    if rectangle_count == 0 {
        return (Vec::new(), Vec::new());
    }
    let mut total_len = 0.0f32;
    let mut end_len = 0.0f32;
    for i in 0..n - 1 {
        let a = points[i];
        let b = points[i + 1];
        let dx = a[0] - b[0];
        let dz = a[2] - b[2];
        let cur = (dx * dx + dz * dz).sqrt();
        total_len += cur;
        if i == river_start as usize {
            end_len = cur;
        }
    }
    if end_len <= f32::EPSILON {
        end_len = 1.0;
    }
    let length_of_river = (total_len * 0.5) - end_len;
    let repeat_count = length_of_river / end_len;
    let v_scale = repeat_count / rectangle_count as f32;
    let mut inner = river_start;
    let mut outer = river_start + 1;
    let mut verts = Vec::with_capacity(pair_count * 2);
    let const_a = 3.0 * v_origin;
    for i in 0..pair_count {
        let inner_pt = points[outer as usize];
        let outer_pt = points[inner as usize];
        outer += 1;
        inner -= 1;
        if inner < 0 {
            inner = (n - 1) as i32;
        }
        if outer >= n as i32 {
            outer = 0;
        }
        let wobble = -v_origin
            + v_scale * i as f32
            + (2.0 * std::f32::consts::PI * (v_scale * i as f32) - const_a).sin() / 22.0;
        verts.push(crate::terrain::SeaPatchVertex {
            x: inner_pt[0],
            y: inner_pt[1],
            z: inner_pt[2],
            c: diffuse,
            tu: HEIGHT_TO_USE,
            tv: wobble,
        });
        verts.push(crate::terrain::SeaPatchVertex {
            x: outer_pt[0],
            y: outer_pt[1],
            z: outer_pt[2],
            c: diffuse,
            tu: 0.0,
            tv: wobble,
        });
    }
    let mut indices = Vec::with_capacity(rectangle_count * 6);
    for i in 0..rectangle_count {
        let b = (i * 2) as u32;
        indices.extend_from_slice(&[b, b + 1, b + 3, b, b + 3, b + 2]);
    }
    (verts, indices)
}

#[cfg(test)]
mod tests {
    use super::{
        bake_river_strip, bake_trapezoid_water, TerrainVisualImpl, WATER_UV_FACTOR,
    };


    #[test]
    fn trapezoid_bake_uses_authored_z_and_world_uvs() {
        let points = [
            [0.0, 10.0, 0.0],
            [80.0, 12.0, 0.0],
            [80.0, 14.0, 80.0],
            [0.0, 16.0, 80.0],
        ];
        let (verts, indices) = bake_trapezoid_water(&points, 0.0, 0.0, 0xffff_ffff, 0);
        assert!(!verts.is_empty());
        assert!(!indices.is_empty());
        assert!(
            (verts[0].y - 10.0).abs() < 1.0e-3,
            "lake verts must keep authored Z, not flatten to water_position_z; y={}",
            verts[0].y
        );
        assert!(
            verts[0].tu.abs() < 0.05,
            "world UV at origin x=0 must be ~0, not grid tx*4; tu={}",
            verts[0].tu
        );
        let last = verts.last().copied().unwrap();
        assert!(
            (last.y - 14.0).abs() < 1.0e-2,
            "far corner interpolates authored Z; y={}",
            last.y
        );
        assert!(
            (last.tu - 80.0 / WATER_UV_FACTOR).abs() < 0.05,
            "world UV uses vertex.x/150; tu={}",
            last.tu
        );
    }

    #[test]
    fn river_bake_uses_bank_pairs_not_centerline_offset() {
        let points = [
            [0.0, 5.0, 0.0],
            [0.0, 5.0, 20.0],
            [40.0, 6.0, 20.0],
            [40.0, 6.0, 0.0],
        ];
        let (verts, indices) = bake_river_strip(&points, 0, 0.0, 0xffff_ffff);
        assert_eq!(verts.len(), 4);
        assert_eq!(indices.len(), 6);
        assert!(
            verts.iter().all(|v| v.x.abs() < 1.0 || (v.x - 40.0).abs() < 1.0),
            "bank pairs use authored XY, not ±12 centerline zigzag: {:?}",
            verts.iter().map(|v| v.x).collect::<Vec<_>>()
        );
        assert!(verts.iter().any(|v| (v.y - 5.0).abs() < 1.0e-3));
        assert!(verts.iter().any(|v| (v.y - 6.0).abs() < 1.0e-3));
    }

    #[test]
    fn leftover_add_water_velocity_drains_into_live_grid() {
        let Some(tv) = gamelogic::helpers::TheTerrainVisual::get() else {
            return;
        };
        let _ = tv.take_water_velocity_impulses();
        tv.add_water_velocity(10.0, 10.0, 1.5, 7.0);
        let mut visual = TerrainVisualImpl::new();
        visual.set_water_grid_resolution(4.0, 4.0, 10.0);
        visual.set_water_transform(0.0, 0.0, 0.0, 5.0);
        visual.set_water_attenuation_factors(1.0, 0.0, 0.0, 10.0);
        visual.enable_water_grid(true);
        visual.drain_leftover_water_velocity();
        assert!(
            !visual.water_grid_state().velocity_events.is_empty(),
            "leftover addWaterVelocity must drain onto the live water grid"
        );
    }

    /// The presentation R8 feed must land the payload in the shroud cells
    /// (replacing the empty/1x1-fallback state), honour the projected origin
    /// in world sampling, and treat an identical re-feed as a no-op state.
    #[test]
    fn shroud_overlay_r8_feed_replaces_fallback_state() {
        let mut visual = TerrainVisualImpl::new();
        assert!(
            visual.shroud_alpha_at_world(0.0, 0.0) == 1.0,
            "uninitialized overlay must fail open (fully visible)"
        );

        let (w, h) = (4i32, 3i32);
        let mut payload = vec![255u8; (w * h) as usize];
        payload[0] = 0; // shrouded
        payload[1] = 128; // fogged
        visual.set_shroud_overlay_r8(w, h, 40.0, [80.0, 40.0], &payload);

        assert_eq!(visual.shroud_alpha_at_world(80.0, 40.0), 0.0);
        assert_eq!(visual.shroud_alpha_at_world(120.0, 40.0), 128.0 / 255.0);
        assert_eq!(visual.shroud_alpha_at_world(160.0, 80.0), 1.0);
        // OOB clamps to the nearest cell instead of panicking.
        assert_eq!(visual.shroud_alpha_at_world(-1000.0, -1000.0), 0.0);

        // Identical re-feed: allowed and state-preserving (no panic, same
        // sampling), which keeps the per-frame presentation feed cheap.
        visual.set_shroud_overlay_r8(w, h, 40.0, [80.0, 40.0], &payload);
        assert_eq!(visual.shroud_alpha_at_world(80.0, 40.0), 0.0);

        // A real change flows through, including a resize.
        let small = vec![64u8; 4];
        visual.set_shroud_overlay_r8(2, 2, 40.0, [0.0, 0.0], &small);
        assert_eq!(visual.shroud_alpha_at_world(0.0, 0.0), 64.0 / 255.0);
        assert_eq!(visual.shroud_alpha_at_world(80.0, 40.0), 64.0 / 255.0);

        // Undersized payload is rejected: previous overlay state retained.
        visual.set_shroud_overlay_r8(4, 4, 40.0, [0.0, 0.0], &[0u8; 3]);
        assert_eq!(visual.shroud_alpha_at_world(0.0, 0.0), 64.0 / 255.0);
    }
}

