use super::*;

/// Compact road segment for presentation-side road mesh bake.
/// Coordinates match `RuntimeRoadSegment` world space (from/to as [x,y,z]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationRoadSegment {
    pub template_name: String,
    pub from: [f32; 3],
    pub to: [f32; 3],
    pub width: f32,
    pub width_in_texture: f32,
    pub road_type_id: u32,
    pub start_is_angled: bool,
    pub start_is_join: bool,
    pub end_is_angled: bool,
    pub end_is_join: bool,
    pub curve_radius: f32,
}

/// Compact bridge segment (start/end world xyz, width, template).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationBridgeSegment {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub width: f32,
    pub template_name: String,
}

/// World/environment identity frozen for the render pass.
///
/// Lets lighting / shell / map-name / bounds / heightmap-hint / roads consumers avoid
/// re-locking live `GameLogic` mid-frame when a presentation snapshot is set.
/// Fail-closed: not a full SAGE heightmap mesh or dirty-rect road stream.
/// Frozen terrain source-tile class for visual bake without live GameLogic.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationTerrainTextureClass {
    pub first_tile: i32,
    pub num_tiles: i32,
    pub width: i32,
    pub name: String,
}

/// Frozen runtime heightmap for terrain-visual bake without live GameLogic.
/// Mirrors `game_client::terrain::height_map::HeightMap` POD fields.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationRuntimeHeightmap {
    pub width: u32,
    pub height: u32,
    pub heights: Vec<f32>,
    pub max_height: f32,
    pub scale: f32,
    pub min_height: f32,
    pub height_range: f32,
    pub border_size: i32,
    pub tile_ndxes: Vec<i16>,
    pub blend_tile_ndxes: Vec<i16>,
    pub draw_origin_x: i32,
    pub draw_origin_y: i32,
    pub draw_width: i32,
    pub draw_height: i32,
}

impl PresentationRuntimeHeightmap {
    #[cfg(feature = "game_client")]
    pub fn from_height_map(hm: &game_client::terrain::height_map::HeightMap) -> Self {
        Self {
            width: hm.width,
            height: hm.height,
            heights: hm.heights.clone(),
            max_height: hm.max_height,
            scale: hm.scale,
            min_height: hm.min_height,
            height_range: hm.height_range,
            border_size: hm.border_size,
            tile_ndxes: hm.tile_ndxes.clone(),
            blend_tile_ndxes: hm.blend_tile_ndxes.clone(),
            draw_origin_x: hm.draw_origin_x,
            draw_origin_y: hm.draw_origin_y,
            draw_width: hm.draw_width,
            draw_height: hm.draw_height,
        }
    }

    #[cfg(feature = "game_client")]
    pub fn to_height_map(&self) -> game_client::terrain::height_map::HeightMap {
        game_client::terrain::height_map::HeightMap {
            width: self.width,
            height: self.height,
            heights: self.heights.clone(),
            max_height: self.max_height,
            scale: self.scale,
            min_height: self.min_height,
            height_range: self.height_range,
            border_size: self.border_size,
            tile_ndxes: self.tile_ndxes.clone(),
            blend_tile_ndxes: self.blend_tile_ndxes.clone(),
            draw_origin_x: self.draw_origin_x,
            draw_origin_y: self.draw_origin_y,
            draw_width: self.draw_width,
            draw_height: self.draw_height,
        }
    }

    pub fn is_usable(&self) -> bool {
        self.width > 0
            && self.height > 0
            && self.heights.len() == (self.width as usize).saturating_mul(self.height as usize)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationWorldEnv {
    pub map_name: String,
    /// Wave 509: snow weather residual for mesh model-condition SNOW bank.
    #[serde(default)]
    pub is_snow: bool,
    /// Wave 509: night residual for mesh model-condition NIGHT bank.
    #[serde(default)]
    pub is_night: bool,
    pub world_min: [f32; 3],
    pub world_max: [f32; 3],
    pub heightmap_hint: Option<String>,
    /// Script/map skybox enable residual.
    pub skybox_enabled: bool,
    /// Optional skybox texture names (front, back, left, right, top).
    pub skybox_textures: Option<[String; 5]>,
    pub sun_direction: Option<[f32; 3]>,
    pub sun_color: Option<[f32; 3]>,
    pub ambient_color: Option<[f32; 3]>,
    pub fog_color: Option<[f32; 3]>,
    pub fog_start: Option<f32>,
    pub fog_end: Option<f32>,
    /// Placed-object count from last parsed map metadata (prewarm signature).
    pub map_object_count: u32,
    pub has_map_metadata: bool,
    /// First N map-object template names for model prewarm (observe path).
    /// Fail-closed: not full ThingTemplate graph.
    pub prewarm_template_names: Vec<String>,
    /// Coarse height samples for minimap/terrain residual (row-major, width×height).
    /// Fail-closed: not full SAGE heightmap mesh / bilinear retail sample grid.
    pub height_grid_w: u32,
    pub height_grid_h: u32,
    pub height_samples: Vec<f32>,
    /// True when at least one sample came from live terrain (not empty default).
    pub height_samples_from_terrain: bool,
    /// Map road segments frozen for terrain-road bake without live GameLogic.
    pub road_segments: Vec<PresentationRoadSegment>,
    /// Bridge segments frozen for terrain-road bake.
    pub bridge_segments: Vec<PresentationBridgeSegment>,
    /// Full runtime heightmap freeze for terrain-visual bake (no live GameLogic).
    pub runtime_heightmap: Option<PresentationRuntimeHeightmap>,
    /// Terrain texture classes freeze for source-tile bake without live GameLogic.
    pub terrain_texture_classes: Vec<PresentationTerrainTextureClass>,
}

impl PresentationWorldEnv {
    pub fn from_logic(logic: &GameLogic) -> Self {
        let (wmin, wmax) = logic.world_bounds();
        let meta = logic.last_parsed_map_settings();
        let heightmap_hint = logic
            .heightmap_hint()
            .and_then(|p| p.to_str().map(|s| s.to_string()))
            .or_else(|| {
                meta.as_ref()
                    .and_then(|m| m.heightmap_path.as_ref())
                    .and_then(|p| p.to_str().map(|s| s.to_string()))
            });
        // Coarse height grid for minimap residual (fixed 64×64 — small, deterministic).
        const HG_W: u32 = 64;
        const HG_H: u32 = 64;
        let span_x = (wmax.x - wmin.x).max(1.0);
        let span_z = (wmax.z - wmin.z).max(1.0);
        let mut height_samples = vec![0.0f32; (HG_W * HG_H) as usize];
        let mut height_samples_from_terrain = false;
        for y in 0..HG_H {
            for x in 0..HG_W {
                let u = (x as f32 + 0.5) / HG_W as f32;
                let v = (y as f32 + 0.5) / HG_H as f32;
                let world = glam::Vec3::new(wmin.x + u * span_x, 0.0, wmin.z + v * span_z);
                if let Some(h) = logic.terrain_height_at(world) {
                    height_samples[(y * HG_W + x) as usize] = h;
                    height_samples_from_terrain = true;
                }
            }
        }

        let road_segments: Vec<PresentationRoadSegment> = logic
            .terrain_road_segments_snapshot()
            .into_iter()
            .map(|s| PresentationRoadSegment {
                template_name: s.template_name,
                from: [s.from.x, s.from.y, s.from.z],
                to: [s.to.x, s.to.y, s.to.z],
                width: s.width,
                width_in_texture: s.width_in_texture,
                road_type_id: s.road_type_id,
                start_is_angled: s.start_is_angled,
                start_is_join: s.start_is_join,
                end_is_angled: s.end_is_angled,
                end_is_join: s.end_is_join,
                curve_radius: s.curve_radius,
            })
            .collect();
        let bridge_segments: Vec<PresentationBridgeSegment> = logic
            .terrain_bridge_segments_snapshot()
            .into_iter()
            .map(
                |(start, end, width, template_name)| PresentationBridgeSegment {
                    start: start.to_array(),
                    end: end.to_array(),
                    width,
                    template_name,
                },
            )
            .collect();
        // Cap prewarm names so snapshot stays small (startup model resolve only).
        const PREWARM_CAP: usize = 256;
        let prewarm_template_names: Vec<String> = meta
            .as_ref()
            .map(|m| {
                m.objects
                    .iter()
                    .filter_map(|o| {
                        let n = o.template.trim();
                        if n.is_empty() {
                            None
                        } else {
                            Some(n.to_string())
                        }
                    })
                    .take(PREWARM_CAP)
                    .collect()
            })
            .unwrap_or_default();

        #[cfg(feature = "game_client")]
        let runtime_heightmap = logic
            .terrain_heightmap_snapshot()
            .map(|hm| PresentationRuntimeHeightmap::from_height_map(&hm));
        #[cfg(not(feature = "game_client"))]
        let runtime_heightmap = None;
        let terrain_texture_classes: Vec<PresentationTerrainTextureClass> = logic
            .terrain_texture_classes_snapshot()
            .into_iter()
            .map(|c| PresentationTerrainTextureClass {
                first_tile: c.first_tile,
                num_tiles: c.num_tiles,
                width: c.width,
                name: c.name,
            })
            .collect();

        let weather = logic.weather_state().current_weather.to_ascii_lowercase();
        let is_snow = weather.contains("snow");
        // Night residual: weather name or evening/night tokens (fail-closed TOD runtime).
        let is_night = weather.contains("night") || weather.contains("evening");
        Self {
            map_name: logic.get_current_map_name().trim().to_string(),
            is_snow,
            is_night,
            world_min: [wmin.x, wmin.y, wmin.z],
            world_max: [wmax.x, wmax.y, wmax.z],
            heightmap_hint,
            skybox_enabled: logic.is_skybox_enabled(),
            skybox_textures: meta.as_ref().and_then(|m| m.skybox_textures.clone()),
            sun_direction: meta.as_ref().and_then(|m| m.sun_direction),
            sun_color: meta.as_ref().and_then(|m| m.sun_color.or(m.sky_color)),
            ambient_color: meta
                .as_ref()
                .and_then(|m| m.ambient_color.or(m.fog_color).or(m.sky_color)),
            fog_color: meta
                .as_ref()
                .and_then(|m| m.fog_color.or(m.sky_color).or(m.sun_color)),
            fog_start: meta.as_ref().and_then(|m| m.fog_start),
            fog_end: meta.as_ref().and_then(|m| m.fog_end),
            map_object_count: meta.as_ref().map(|m| m.objects.len() as u32).unwrap_or(0),
            has_map_metadata: meta.is_some(),
            prewarm_template_names,
            height_grid_w: HG_W,
            height_grid_h: HG_H,
            height_samples,
            height_samples_from_terrain,
            road_segments,
            bridge_segments,
            runtime_heightmap,
            terrain_texture_classes,
        }
    }

    #[inline]
    pub fn world_bounds_vec3(&self) -> (glam::Vec3, glam::Vec3) {
        (
            glam::Vec3::from_array(self.world_min),
            glam::Vec3::from_array(self.world_max),
        )
    }

    #[inline]
    pub fn fog_range(&self) -> Option<(f32, f32)> {
        self.fog_start.zip(self.fog_end)
    }

    /// Bilinear-ish nearest sample from the coarse height grid (world XZ).
    /// Returns None when the grid is empty / not from terrain.
    pub fn sample_height(&self, world_x: f32, world_z: f32) -> Option<f32> {
        if !self.height_samples_from_terrain
            || self.height_grid_w == 0
            || self.height_grid_h == 0
            || self.height_samples.is_empty()
        {
            return None;
        }
        let (wmin, wmax) = self.world_bounds_vec3();
        let span_x = (wmax.x - wmin.x).max(1.0);
        let span_z = (wmax.z - wmin.z).max(1.0);
        let u = ((world_x - wmin.x) / span_x).clamp(0.0, 1.0);
        let v = ((world_z - wmin.z) / span_z).clamp(0.0, 1.0);
        let x = ((u * (self.height_grid_w as f32 - 1.0)).round() as u32)
            .min(self.height_grid_w.saturating_sub(1));
        let y = ((v * (self.height_grid_h as f32 - 1.0)).round() as u32)
            .min(self.height_grid_h.saturating_sub(1));
        let idx = (y * self.height_grid_w + x) as usize;
        self.height_samples.get(idx).copied()
    }

    /// Prewarm signature fragment (map|meta|objects|heightmap|shell) without live logic.
    pub fn prewarm_signature(&self, shell_bypass: bool) -> String {
        format!(
            "{}|meta:{}|objects:{}|heightmap:{}|shell:{}",
            self.map_name,
            self.has_map_metadata,
            self.map_object_count,
            self.heightmap_hint.as_deref().unwrap_or(""),
            shell_bypass
        )
    }
}
