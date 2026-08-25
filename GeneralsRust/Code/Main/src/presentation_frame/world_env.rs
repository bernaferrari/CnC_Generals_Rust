use super::*;
use game_engine::common::ini::ini_game_data::{
    GlobalData as AuthoredGlobalData, MAX_GLOBAL_LIGHTS, TIME_OF_DAY_COUNT,
    TerrainLighting as AuthoredTerrainLighting, TimeOfDay, get_global_data,
};
use std::sync::Arc;

/// GameData light frozen for Main's WGPU presentation path.
///
/// `light_pos` deliberately stays in the authored C++ W3D coordinate basis
/// (X/Y/Z, Z-up). Consumers that render in Main's Y-up basis must call
/// [`Self::render_light_pos`] rather than silently treating authored values as
/// WGPU coordinates. C++ `W3DDisplay::setTimeOfDay` loops all
/// `LightEnvironmentClass::MAX_LIGHTS` object lights and scales infantry copies.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct PresentationPrimaryGlobalLight {
    pub ambient: [f32; 3],
    pub diffuse: [f32; 3],
    pub light_pos: [f32; 3],
    /// `NumberGlobalLights` admits this object-scene directional light.
    /// Terrain still receives its authored index-zero values, matching the
    /// existing Display/TerrainVisual path even when no object light is live.
    pub object_light_active: bool,
}

impl PresentationPrimaryGlobalLight {
    fn from_authored(source: &AuthoredTerrainLighting, object_light_active: bool) -> Self {
        Self {
            ambient: [source.ambient.r, source.ambient.g, source.ambient.b],
            diffuse: [source.diffuse.r, source.diffuse.g, source.diffuse.b],
            light_pos: [source.light_pos.x, source.light_pos.y, source.light_pos.z],
            object_light_active,
        }
    }

    fn from_row(row: [f32; 9], object_light_active: bool) -> Self {
        Self {
            ambient: [row[0], row[1], row[2]],
            diffuse: [row[3], row[4], row[5]],
            light_pos: [row[6], row[7], row[8]],
            object_light_active,
        }
    }

    /// Convert authored C++ W3D X/Y/Z (Z-up) to Main's X/Z/Y (Y-up) render
    /// basis. This is the same conversion used for dynamic W3D scene lights.
    #[inline]
    pub fn render_light_pos(self) -> [f32; 3] {
        [self.light_pos[0], self.light_pos[2], self.light_pos[1]]
    }

    /// C++ `W3DScene::updateFixedLightEnvironments` infantry copy: scale
    /// ambient/diffuse and cap each channel at 1.0.
    #[inline]
    pub fn scaled_for_infantry(self, scale: f32) -> Self {
        Self {
            ambient: [
                (self.ambient[0] * scale).min(1.0),
                (self.ambient[1] * scale).min(1.0),
                (self.ambient[2] * scale).min(1.0),
            ],
            diffuse: [
                (self.diffuse[0] * scale).min(1.0),
                (self.diffuse[1] * scale).min(1.0),
                (self.diffuse[2] * scale).min(1.0),
            ],
            ..self
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct FrozenGlobalLights {
    object: [PresentationPrimaryGlobalLight; MAX_GLOBAL_LIGHTS],
    terrain: [PresentationPrimaryGlobalLight; MAX_GLOBAL_LIGHTS],
    infantry_scale: f32,
}

fn infantry_light_scale_for(global_data: &AuthoredGlobalData, time_index: usize) -> f32 {
    // C++ W3DScene.cpp:856-860 — script override wins when not -1.
    if (global_data.script_override_infantry_light_scale - (-1.0)).abs() > f32::EPSILON {
        global_data.script_override_infantry_light_scale
    } else {
        global_data
            .infantry_light_scale
            .get(time_index)
            .copied()
            .unwrap_or(1.5)
    }
}

fn freeze_all_game_data_lighting(global_data: &AuthoredGlobalData) -> Option<FrozenGlobalLights> {
    let time_index = match global_data.time_of_day {
        TimeOfDay::Invalid => return None,
        time_of_day => time_of_day as usize,
    };
    if time_index >= TIME_OF_DAY_COUNT {
        return None;
    }
    let active = global_data
        .num_global_lights
        .clamp(0, MAX_GLOBAL_LIGHTS as i32) as usize;
    let mut object = [PresentationPrimaryGlobalLight::default(); MAX_GLOBAL_LIGHTS];
    let mut terrain = [PresentationPrimaryGlobalLight::default(); MAX_GLOBAL_LIGHTS];
    for i in 0..MAX_GLOBAL_LIGHTS {
        object[i] = PresentationPrimaryGlobalLight::from_authored(
            &global_data.terrain_objects_lighting[time_index][i],
            i < active,
        );
        terrain[i] = PresentationPrimaryGlobalLight::from_authored(
            &global_data.terrain_lighting[time_index][i],
            i < active,
        );
    }
    Some(FrozenGlobalLights {
        object,
        terrain,
        infantry_scale: infantry_light_scale_for(global_data, time_index),
    })
}

/// Freeze the exact primary GameData lighting pair for the active authored
/// time of day. Unknown/invalid time-of-day state is deliberately omitted:
/// callers preserve any explicit map metadata rather than inventing a name or
/// time-based lighting fallback.
pub(crate) fn freeze_primary_game_data_lighting(
    global_data: &AuthoredGlobalData,
) -> Option<(
    PresentationPrimaryGlobalLight,
    PresentationPrimaryGlobalLight,
)> {
    let frozen = freeze_all_game_data_lighting(global_data)?;
    Some((frozen.object[0], frozen.terrain[0]))
}

fn freeze_current_all_game_data_lighting() -> Option<FrozenGlobalLights> {
    let global_data = get_global_data()?;
    let global_data = global_data.read();
    freeze_all_game_data_lighting(&global_data)
}

fn light_from_map_channels(
    ambient: Option<[f32; 3]>,
    diffuse: Option<[f32; 3]>,
    light_pos: Option<[f32; 3]>,
) -> Option<PresentationPrimaryGlobalLight> {
    Some(PresentationPrimaryGlobalLight {
        ambient: ambient?,
        diffuse: diffuse?,
        light_pos: light_pos?,
        object_light_active: true,
    })
}

fn merge_map_and_frozen_lights(
    meta: Option<&crate::game_logic::script_loader::MapMetadata>,
    frozen: Option<FrozenGlobalLights>,
) -> (
    [Option<PresentationPrimaryGlobalLight>; MAX_GLOBAL_LIGHTS],
    [Option<PresentationPrimaryGlobalLight>; MAX_GLOBAL_LIGHTS],
    Option<f32>,
) {
    let mut object_global_lights = [None; MAX_GLOBAL_LIGHTS];
    let mut terrain_global_lights = [None; MAX_GLOBAL_LIGHTS];
    if let Some(frozen) = frozen {
        for i in 0..MAX_GLOBAL_LIGHTS {
            object_global_lights[i] = Some(frozen.object[i]);
            terrain_global_lights[i] = Some(frozen.terrain[i]);
        }
    }
    if let Some(map_object) = meta.and_then(|m| {
        light_from_map_channels(
            m.objects_ambient_color,
            m.objects_sun_color,
            m.objects_sun_direction,
        )
    }) {
        object_global_lights[0] = Some(map_object);
    }
    if let Some(map_terrain) =
        meta.and_then(|m| light_from_map_channels(m.ambient_color, m.sun_color, m.sun_direction))
    {
        terrain_global_lights[0] = Some(map_terrain);
    }
    if let Some(m) = meta {
        for (i, row) in m.objects_extra_lights.iter().take(2).enumerate() {
            object_global_lights[i + 1] =
                Some(PresentationPrimaryGlobalLight::from_row(*row, true));
        }
        for (i, row) in m.terrain_extra_lights.iter().take(2).enumerate() {
            terrain_global_lights[i + 1] =
                Some(PresentationPrimaryGlobalLight::from_row(*row, true));
        }
    }
    let infantry_light_scale = frozen.map(|f| f.infantry_scale);
    (
        object_global_lights,
        terrain_global_lights,
        infantry_light_scale,
    )
}

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

/// Serializable copy of the per-blend metadata used by a runtime heightmap.
///
/// `BlendTileInfo` belongs to the optional GameClient crate, while a
/// `PresentationRuntimeHeightmap` must also be representable without that
/// feature.  Keep the snapshot-owned form here so a presentation frame does
/// not discard the alpha/flip metadata needed to rebuild its terrain mesh.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PresentationBlendTileInfo {
    pub blend_ndx: i32,
    pub horiz: u8,
    pub vert: u8,
    pub right_diagonal: u8,
    pub left_diagonal: u8,
    pub inverted: u8,
    pub long_diagonal: u8,
    pub custom_blend_edge_class: i32,
}

impl Default for PresentationBlendTileInfo {
    fn default() -> Self {
        // Match `game_client::terrain::textures::BlendTileInfo::new()`.
        Self {
            blend_ndx: 0,
            horiz: 0,
            vert: 0,
            right_diagonal: 0,
            left_diagonal: 0,
            inverted: 0,
            long_diagonal: 0,
            custom_blend_edge_class: -1,
        }
    }
}

impl PresentationBlendTileInfo {
    #[cfg(feature = "game_client")]
    fn from_game_client(tile: &game_client::terrain::textures::BlendTileInfo) -> Self {
        Self {
            blend_ndx: tile.blend_ndx,
            horiz: tile.horiz,
            vert: tile.vert,
            right_diagonal: tile.right_diagonal,
            left_diagonal: tile.left_diagonal,
            inverted: tile.inverted,
            long_diagonal: tile.long_diagonal,
            custom_blend_edge_class: tile.custom_blend_edge_class,
        }
    }

    #[cfg(feature = "game_client")]
    fn to_game_client(&self) -> game_client::terrain::textures::BlendTileInfo {
        game_client::terrain::textures::BlendTileInfo {
            blend_ndx: self.blend_ndx,
            horiz: self.horiz,
            vert: self.vert,
            right_diagonal: self.right_diagonal,
            left_diagonal: self.left_diagonal,
            inverted: self.inverted,
            long_diagonal: self.long_diagonal,
            custom_blend_edge_class: self.custom_blend_edge_class,
        }
    }
}

/// Frozen runtime heightmap for terrain-visual bake without live GameLogic.
/// Mirrors all `game_client::terrain::height_map::HeightMap` data used by the
/// terrain bake, including secondary blend metadata.
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
    #[serde(default)]
    pub extra_blend_tile_ndxes: Vec<i16>,
    /// C++ `m_blendedTiles`, required to recover normal extra-blend alpha and
    /// winding information after the immutable presentation hand-off.
    #[serde(default)]
    pub blended_tiles: Vec<PresentationBlendTileInfo>,
    /// C++ `m_extraBlendedTiles`, the fallback metadata bank for extra blend.
    #[serde(default)]
    pub extra_blended_tiles: Vec<PresentationBlendTileInfo>,
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
            extra_blend_tile_ndxes: hm.extra_blend_tile_ndxes.clone(),
            blended_tiles: hm
                .blended_tiles
                .iter()
                .map(PresentationBlendTileInfo::from_game_client)
                .collect(),
            extra_blended_tiles: hm
                .extra_blended_tiles
                .iter()
                .map(PresentationBlendTileInfo::from_game_client)
                .collect(),
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
            extra_blend_tile_ndxes: self.extra_blend_tile_ndxes.clone(),
            blended_tiles: self
                .blended_tiles
                .iter()
                .map(PresentationBlendTileInfo::to_game_client)
                .collect(),
            extra_blended_tiles: self
                .extra_blended_tiles
                .iter()
                .map(PresentationBlendTileInfo::to_game_client)
                .collect(),
            cliff_info: vec![game_client::terrain::height_map::TCliffInfo::default()],
            cliff_info_ndxes: vec![0i16; self.heights.len()],
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

    /// C++ `m_extraBlendTilePositions`: `i | (j << 16)` for cells with extra blend.
    pub fn extra_blend_tile_positions(&self) -> Vec<u32> {
        let width = self.width as i32;
        let height = self.height as i32;
        if width < 2 || height < 2 {
            return Vec::new();
        }
        let mut positions = Vec::new();
        for j in 0..(height - 1) {
            for i in 0..(width - 1) {
                let ndx = (j * width + i) as usize;
                if self.extra_blend_tile_ndxes.get(ndx).copied().unwrap_or(0) > 0 {
                    positions.push((i as u32) | ((j as u32) << 16));
                }
            }
        }
        positions
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
    /// Frozen C++ GlobalData shroud levels used by the W3D fogged-light
    /// environment. These are presentation inputs, never a live FOW query.
    #[serde(default = "default_clear_alpha")]
    pub clear_alpha: u8,
    #[serde(default = "default_fog_alpha")]
    pub fog_alpha: u8,
    /// Primary `TerrainObjectsLighting*` GameData record for the active TOD.
    /// This is distinct from terrain lighting: C++ applies the former to the
    /// W3D scene and the latter to TerrainVisual.
    #[serde(default)]
    pub primary_object_lighting: Option<PresentationPrimaryGlobalLight>,
    /// Primary `TerrainLighting*` GameData record for the active TOD.
    #[serde(default)]
    pub primary_terrain_lighting: Option<PresentationPrimaryGlobalLight>,
    /// C++ `W3DDisplay::setTimeOfDay` loops 3 object-scene lights.
    #[serde(default)]
    pub object_global_lights: [Option<PresentationPrimaryGlobalLight>; MAX_GLOBAL_LIGHTS],
    /// Authored terrain lights 0..2 for TerrainVisual.
    #[serde(default)]
    pub terrain_global_lights: [Option<PresentationPrimaryGlobalLight>; MAX_GLOBAL_LIGHTS],
    /// C++ `m_scriptOverrideInfantryLightScale` or `m_infantryLightScale[tod]`.
    #[serde(default)]
    pub infantry_light_scale: Option<f32>,
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
    pub runtime_heightmap: Option<Arc<PresentationRuntimeHeightmap>>,
    /// Terrain texture classes freeze for source-tile bake without live GameLogic.
    pub terrain_texture_classes: Vec<PresentationTerrainTextureClass>,
    /// Map InitialCameraPosition waypoint (C++ startNewGame lookAt), not the
    /// script MOVE_CAMERA_TO queue (`PresentationFrame.camera_focus`).
    #[serde(default)]
    pub initial_camera_position: Option<[f32; 3]>,
}

impl PresentationWorldEnv {
    pub fn from_logic(logic: &GameLogic) -> Self {
        Self::from_logic_with_runtime_heightmap(logic, None)
    }

    /// Build a frame environment using an engine-owned full terrain freeze when
    /// one is available. Direct builders retain the old one-shot fallback for
    /// tests and tools which do not have an engine cache.
    pub(crate) fn from_logic_with_runtime_heightmap(
        logic: &GameLogic,
        runtime_heightmap: Option<Arc<PresentationRuntimeHeightmap>>,
    ) -> Self {
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
        let runtime_heightmap = runtime_heightmap.or_else(|| {
            logic.terrain_heightmap_snapshot().map(|heightmap| {
                Arc::new(PresentationRuntimeHeightmap::from_height_map(&heightmap))
            })
        });
        #[cfg(not(feature = "game_client"))]
        let runtime_heightmap = {
            let _ = runtime_heightmap;
            None
        };
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
        let (follow_weather, is_night) = get_global_data()
            .map(|global| {
                let global = global.read();
                (
                    global.force_models_to_follow_weather,
                    // C++ Drawable::setTimeOfDay / Object bind: NIGHT iff tod==TIME_OF_DAY_NIGHT.
                    matches!(global.time_of_day, TimeOfDay::Night),
                )
            })
            .unwrap_or((true, false));
        let is_snow = weather.contains("snow") && follow_weather;

        // C++ W3DDisplay::setTimeOfDay applies all 3 object lights; TerrainVisual
        // uses the terrain array. Map objects-lighting wins for units/shadows.
        let (object_global_lights, terrain_global_lights, infantry_light_scale) =
            merge_map_and_frozen_lights(meta.as_ref(), freeze_current_all_game_data_lighting());
        let primary_object_lighting = object_global_lights[0];
        let primary_terrain_lighting = terrain_global_lights[0];
        // Units/shadows: C++ W3DDisplay.cpp:2128 uses objects lighting, not terrain.
        let unit_light = primary_object_lighting;
        let (clear_alpha, fog_alpha) = get_global_data()
            .map(|global| {
                let global = global.read();
                (global.clear_alpha, global.fog_alpha)
            })
            .unwrap_or((default_clear_alpha(), default_fog_alpha()));
        Self {
            map_name: logic.get_current_map_name().trim().to_string(),
            is_snow,
            is_night,
            world_min: [wmin.x, wmin.y, wmin.z],
            world_max: [wmax.x, wmax.y, wmax.z],
            heightmap_hint,
            skybox_enabled: logic.is_skybox_enabled(),
            skybox_textures: meta.as_ref().and_then(|m| m.skybox_textures.clone()),
            sun_direction: unit_light
                .map(|l| l.light_pos)
                .or_else(|| meta.as_ref().and_then(|m| m.objects_sun_direction)),
            sun_color: unit_light
                .map(|l| l.diffuse)
                .or_else(|| meta.as_ref().and_then(|m| m.objects_sun_color)),
            ambient_color: unit_light
                .map(|l| l.ambient)
                .or_else(|| meta.as_ref().and_then(|m| m.objects_ambient_color)),
            // C++ SceneClass FogEnabled defaults false; GlobalLighting has no fog.
            fog_color: None,
            fog_start: None,
            fog_end: None,

            clear_alpha,
            fog_alpha,
            primary_object_lighting,
            primary_terrain_lighting,
            object_global_lights,
            terrain_global_lights,
            infantry_light_scale,
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
            initial_camera_position: meta
                .as_ref()
                .and_then(|m| m.initial_camera_position.map(|p| [p.x, p.y, p.z])),
        }
    }

    /// C++ `W3DScene::updateFixedLightEnvironments` ratio, frozen with the
    /// presentation frame so ghost rendering never reads live GlobalData.
    #[inline]
    pub fn fogged_light_fraction(&self) -> f32 {
        if self.clear_alpha == 0 {
            0.0
        } else {
            (self.fog_alpha as f32 / self.clear_alpha as f32).clamp(0.0, 1.0)
        }
    }

    /// C++ W3DScene.cpp:856-881 infantry light copies.
    #[inline]
    pub fn infantry_scale(&self) -> f32 {
        self.infantry_light_scale.unwrap_or(1.5)
    }

    /// Object-scene lights scaled for infantry drawables.
    #[inline]
    pub fn infantry_scaled_object_lights(
        &self,
    ) -> [Option<PresentationPrimaryGlobalLight>; MAX_GLOBAL_LIGHTS] {
        let scale = self.infantry_scale();
        let mut out = [None; MAX_GLOBAL_LIGHTS];
        for (i, light) in self.object_global_lights.iter().enumerate() {
            out[i] = light.map(|l| l.scaled_for_infantry(scale));
        }
        out
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

    /// Sample the frozen full-resolution terrain using the same world-to-heightmap
    /// mapping as Main's authoritative `TerrainData`.
    ///
    /// Mouse picking must use this snapshot rather than re-locking `GameLogic`:
    /// a command is interpreted against the world which was actually presented
    /// to the player.  The coarse height grid remains appropriate for minimap
    /// and diagnostic consumers, but is not accurate enough for a camera ray.
    pub fn sample_gameplay_terrain_height(&self, world_x: f32, world_z: f32) -> Option<f32> {
        let heightmap = self.runtime_heightmap.as_ref()?;
        if !heightmap.is_usable() {
            return None;
        }

        let border = heightmap.border_size.max(0) as u32;
        let playable_width = heightmap
            .width
            .saturating_sub(border.saturating_mul(2))
            .max(2) as f32;
        let playable_height = heightmap
            .height
            .saturating_sub(border.saturating_mul(2))
            .max(2) as f32;
        let (world_min, world_max) = self.world_bounds_vec3();
        let scale_x = (world_max.x - world_min.x) / (playable_width - 1.0);
        let scale_z = (world_max.z - world_min.z) / (playable_height - 1.0);
        if !scale_x.is_finite()
            || !scale_z.is_finite()
            || scale_x.abs() <= f32::EPSILON
            || scale_z.abs() <= f32::EPSILON
        {
            return None;
        }

        let max_x = heightmap.width.saturating_sub(1) as f32;
        let max_z = heightmap.height.saturating_sub(1) as f32;
        let sample_x = ((world_x - world_min.x) / scale_x + border as f32).clamp(0.0, max_x);
        let sample_z = ((world_z - world_min.z) / scale_z + border as f32).clamp(0.0, max_z);
        let x0 = sample_x.floor() as u32;
        let z0 = sample_z.floor() as u32;
        let x1 = (x0 + 1).min(heightmap.width.saturating_sub(1));
        let z1 = (z0 + 1).min(heightmap.height.saturating_sub(1));
        let tx = sample_x - x0 as f32;
        let tz = sample_z - z0 as f32;
        let at = |x: u32, z: u32| {
            heightmap
                .heights
                .get((z * heightmap.width + x) as usize)
                .copied()
        };
        let (h00, h10, h01, h11) = (at(x0, z0)?, at(x1, z0)?, at(x0, z1)?, at(x1, z1)?);
        let lower = h00 * (1.0 - tx) + h10 * tx;
        let upper = h01 * (1.0 - tx) + h11 * tx;
        let normalized = lower * (1.0 - tz) + upper * tz;
        let height = normalized * heightmap.max_height;
        height.is_finite().then_some(height)
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

fn default_clear_alpha() -> u8 {
    255
}

fn default_fog_alpha() -> u8 {
    127
}

#[cfg(test)]
mod lighting_parity_tests {
    use super::*;
    use game_engine::common::ini::ini_game_data::{
        Coord3D, RGBColor, TimeOfDay, ensure_global_data,
    };

    #[test]
    fn freeze_applies_three_global_lights_and_infantry_scale() {
        // C++ W3DDisplay.cpp:2136 loops 3 lights; W3DScene.cpp:856-881 scales infantry.
        let handle = ensure_global_data();
        let previous = handle.read().clone();
        {
            let mut data = handle.write();
            data.time_of_day = TimeOfDay::Night;
            data.num_global_lights = 3;
            data.infantry_light_scale[TimeOfDay::Night as usize] = 1.5;
            data.script_override_infantry_light_scale = -1.0;
            for i in 0..MAX_GLOBAL_LIGHTS {
                let f = (i + 1) as f32;
                data.terrain_objects_lighting[TimeOfDay::Night as usize][i].ambient =
                    RGBColor::new(0.1 * f, 0.0, 0.0);
                data.terrain_objects_lighting[TimeOfDay::Night as usize][i].diffuse =
                    RGBColor::new(0.2 * f, 0.0, 0.0);
                data.terrain_objects_lighting[TimeOfDay::Night as usize][i].light_pos =
                    Coord3D::new(f, 0.0, 1.0);
                data.terrain_lighting[TimeOfDay::Night as usize][i].ambient =
                    RGBColor::new(0.01 * f, 0.0, 0.0);
            }
        }
        let frozen = freeze_all_game_data_lighting(&handle.read()).expect("night lights");
        assert!(frozen.object[0].object_light_active);
        assert!(frozen.object[1].object_light_active);
        assert!(frozen.object[2].object_light_active);
        assert!((frozen.object[1].ambient[0] - 0.2).abs() < 1e-5);
        assert!((frozen.object[2].diffuse[0] - 0.6).abs() < 1e-5);
        assert!((frozen.infantry_scale - 1.5).abs() < 1e-5);
        let scaled = frozen.object[0].scaled_for_infantry(frozen.infantry_scale);
        assert!((scaled.ambient[0] - 0.15).abs() < 1e-5);
        *handle.write() = previous;
    }

    #[test]
    fn world_env_prefers_map_objects_lighting_for_units() {
        // C++ W3DDisplay.cpp:2128 uses m_terrainObjectsLighting for units/shadows.
        let mut meta = crate::game_logic::script_loader::MapMetadata::default();
        meta.ambient_color = Some([0.1, 0.1, 0.1]);
        meta.sun_color = Some([0.2, 0.2, 0.2]);
        meta.sun_direction = Some([1.0, 0.0, 0.0]);
        meta.objects_ambient_color = Some([0.9, 0.8, 0.7]);
        meta.objects_sun_color = Some([0.15, 0.25, 0.35]);
        meta.objects_sun_direction = Some([9.0, 8.0, 7.0]);
        meta.objects_extra_lights = vec![
            [0.0, 0.0, 0.0, 0.4, 0.0, 0.0, 2.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 3.0, 0.0, 0.0],
        ];
        let (object_lights, terrain_lights, _) = merge_map_and_frozen_lights(Some(&meta), None);
        let unit = object_lights[0].expect("objects lighting");
        assert_eq!(unit.ambient, [0.9, 0.8, 0.7]);
        assert_eq!(unit.diffuse, [0.15, 0.25, 0.35]);
        assert_eq!(unit.light_pos, [9.0, 8.0, 7.0]);
        assert_eq!(terrain_lights[0].map(|l| l.ambient), Some([0.1, 0.1, 0.1]));
        assert_eq!(object_lights[1].map(|l| l.diffuse), Some([0.4, 0.0, 0.0]));
        assert_eq!(object_lights[2].map(|l| l.diffuse), Some([0.0, 0.5, 0.0]));
    }

    #[test]
    fn world_env_night_follows_global_time_of_day_not_weather_string() {
        use crate::game_logic::GameLogic;
        let handle = ensure_global_data();
        let previous = handle.read().clone();
        {
            let mut data = handle.write();
            data.time_of_day = TimeOfDay::Night;
        }
        let logic = GameLogic::new();
        let night = PresentationWorldEnv::from_logic(&logic);
        assert!(
            night.is_night,
            "TIME_OF_DAY_NIGHT must stamp MODELCONDITION_NIGHT"
        );
        {
            let mut data = handle.write();
            data.time_of_day = TimeOfDay::Afternoon;
        }
        let day = PresentationWorldEnv::from_logic(&logic);
        assert!(!day.is_night);
        *handle.write() = previous;
    }
}
