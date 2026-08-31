//! C++-matching CPU mesh bake for W3D water / road / bridge GPU upload.
//!
//! Oracle:
//! - `GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/Water/W3DWater.cpp`
//!   `generateVertexBuffer` / `generateIndexBuffer`
//! - `GeneralsMD/Code/GameEngineDevice/Source/W3DDevice/GameClient/W3DRoadBuffer.cpp`
//!   `loadFloat4PtSection`
//! - `GeneralsMD/Code/GameEngineDevice/Include/W3DDevice/GameClient/W3DBridgeBuffer.h`
//!   `W3DBridgeBuffer` / `W3DBridge::getIndicesNVertices`
//!
//! These bakes feed the shipped wgpu TerrainVisual overlay pipelines (TriangleList).

use crate::fx_list::{
    DisplayDynamicLight, do_the_dynamic_light, far_atten_factor, scene_dynamic_lights,
};
use gamelogic::common::types::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};

/// C++ `PATCH_SIZE` — vertices on one water-patch edge.
pub const PATCH_SIZE: usize = 15;
/// C++ `PATCH_UV_TILES`.
pub const PATCH_UV_TILES: f32 = 42.0;
/// C++ `PATCH_WIDTH`.
pub const PATCH_WIDTH: usize = PATCH_SIZE - 1;
/// C++ `PATCH_UV_SCALE` = `PATCH_UV_TILES / PATCH_WIDTH`.
pub const PATCH_UV_SCALE: f32 = PATCH_UV_TILES / PATCH_WIDTH as f32;
/// C++ `PATCH_SCALE` multiplier; world cell = this * `MAP_XY_FACTOR`.
pub const PATCH_SCALE: f32 = 4.0;
/// C++ `SEA_REFLECTION_SIZE`.
pub const SEA_REFLECTION_SIZE: u32 = 256;
/// C++ `NUM_BUMP_FRAMES`.
pub const NUM_BUMP_FRAMES: usize = 32;
/// C++ `WATER_MESH_X_VERTICES`.
pub const WATER_MESH_X_VERTICES: usize = 128;
/// C++ `WATER_MESH_Y_VERTICES`.
pub const WATER_MESH_Y_VERTICES: usize = 128;

/// C++ `MAX_SEG_VERTEX`.
pub const MAX_SEG_VERTEX: usize = 500;
/// C++ `MAX_SEG_INDEX`.
pub const MAX_SEG_INDEX: usize = 2000;
/// C++ `DEFAULT_ROAD_SCALE`.
pub const DEFAULT_ROAD_SCALE: f32 = 8.0;
/// C++ `MIN_ROAD_SEGMENT`.
pub const MIN_ROAD_SEGMENT: f32 = 0.25;
/// C++ `MAX_LINKS`.
pub const MAX_LINKS: usize = 6;
/// C++ `NUM_CORNERS`.
pub const NUM_CORNERS: usize = 4;
/// C++ `FLOAT_AMOUNT = MAP_HEIGHT_SCALE/8`.
pub const ROAD_FLOAT_AMOUNT: f32 = MAP_HEIGHT_SCALE / 8.0;
/// C++ `MAX_ERROR = MAP_HEIGHT_SCALE*1.1`.
pub const ROAD_MAX_ERROR: f32 = MAP_HEIGHT_SCALE * 1.1;
/// C++ `maxRows` inside `loadFloat4PtSection`.
pub const ROAD_MAX_ROWS: usize = 100;

/// C++ `W3DBridgeBuffer::MAX_BRIDGE_VERTEX`.
pub const MAX_BRIDGE_VERTEX: usize = 12_000;
/// C++ `W3DBridgeBuffer::MAX_BRIDGE_INDEX`.
pub const MAX_BRIDGE_INDEX: usize = 2 * MAX_BRIDGE_VERTEX;
/// C++ `W3DBridgeBuffer::MAX_BRIDGES`.
pub const MAX_BRIDGES: usize = 200;
/// C++ `BRIDGE_FLOAT_AMT`.
pub const BRIDGE_FLOAT_AMT: f32 = 0.25;

/// C++ `SEA_PATCH_VERTEX`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SeaPatchVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub c: u32,
    pub tu: f32,
    pub tv: f32,
}

/// C++ `VertexFormatXYZDUV1` road vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoadSegVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: u32,
    pub u1: f32,
    pub v1: f32,
}

/// C++ `VertexFormatXYZNDUV1` bridge vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeOverlayVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: u32,
    pub nx: f32,
    pub ny: f32,
    pub nz: f32,
    pub u1: f32,
    pub v1: f32,
}

/// CPU bridge render buffers matching `W3DBridgeBuffer` VB/IB contents.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BridgeOverlayBuffers {
    pub vertices: Vec<BridgeOverlayVertex>,
    pub indices: Vec<u16>,
}

/// C++ `WaterRenderObjClass::generateIndexBuffer` index count.
#[must_use]
pub fn water_strip_index_count(size_x: usize, size_y: usize) -> usize {
    if size_x < 2 || size_y < 2 {
        return 0;
    }
    (size_y - 1) * (size_x * 2 + 2) - 2
}

/// C++ `WaterRenderObjClass::generateVertexBuffer` for a static sea patch.
/// Positions stay in patch-local grid: `(x, m_level, z)`.
#[must_use]
pub fn generate_water_vertex_buffer(
    size_x: usize,
    size_y: usize,
    water_level: f32,
    transparent_water_diffuse: u32,
) -> Vec<SeaPatchVertex> {
    if size_x == 0 || size_y == 0 {
        return Vec::new();
    }
    let mut vertices = Vec::with_capacity(size_x * size_y);
    for z in 0..size_y {
        for x in 0..size_x {
            vertices.push(SeaPatchVertex {
                x: x as f32,
                y: water_level,
                z: z as f32,
                c: transparent_water_diffuse,
                tu: x as f32 * PATCH_UV_SCALE,
                tv: z as f32 * PATCH_UV_SCALE,
            });
        }
    }
    vertices
}

/// C++ `WaterRenderObjClass::generateIndexBuffer` triangle-strip with degenerates.
#[must_use]
pub fn generate_water_index_buffer(size_x: usize, size_y: usize) -> Vec<u16> {
    let num_indices = water_strip_index_count(size_x, size_y);
    if num_indices == 0 {
        return Vec::new();
    }
    let mut indices = vec![0u16; num_indices];
    let mut i = 0usize;
    let mut k = 0usize;
    let mut j = 0usize;
    while i < num_indices {
        while k < size_x * (j + 1) && i + 1 < num_indices {
            indices[i] = (k + size_x) as u16;
            indices[i + 1] = k as u16;
            k += 1;
            i += 2;
        }
        if i < num_indices {
            indices[i] = (k - 1) as u16;
            if i + 1 < num_indices {
                indices[i + 1] = (k + size_x) as u16;
            }
            i += 2;
        }
        j += 1;
    }
    indices
}

/// Expand a triangle strip (with degenerates) to a TriangleList index buffer.
#[must_use]
pub fn triangle_list_from_strip(strip: &[u16]) -> Vec<u32> {
    let mut list = Vec::new();
    if strip.len() < 3 {
        return list;
    }
    for i in 0..strip.len() - 2 {
        let a = strip[i] as u32;
        let b = strip[i + 1] as u32;
        let c = strip[i + 2] as u32;
        if a == b || b == c || a == c {
            continue;
        }
        if i % 2 == 0 {
            list.extend_from_slice(&[a, b, c]);
        } else {
            list.extend_from_slice(&[a, c, b]);
        }
    }
    list
}

/// World-space Y-up water patch covering `[min_x,max_x] x [min_z,max_z]` at `water_y`.
/// Vertex UVs stay C++ `x*PATCH_UV_SCALE` / `z*PATCH_UV_SCALE`.
#[must_use]
pub fn bake_water_patch_world(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    water_y: f32,
    transparent_water_diffuse: u32,
) -> (Vec<SeaPatchVertex>, Vec<u16>, Vec<u32>) {
    let local =
        generate_water_vertex_buffer(PATCH_SIZE, PATCH_SIZE, water_y, transparent_water_diffuse);
    let strip = generate_water_index_buffer(PATCH_SIZE, PATCH_SIZE);
    let list = triangle_list_from_strip(&strip);
    let span_x = max_x - min_x;
    let span_z = max_z - min_z;
    let mut world: Vec<SeaPatchVertex> = local
        .into_iter()
        .map(|v| SeaPatchVertex {
            x: min_x + (v.x / PATCH_WIDTH as f32) * span_x,
            y: water_y,
            z: min_z + (v.z / PATCH_WIDTH as f32) * span_z,
            c: v.c,
            tu: v.tu,
            tv: v.tv,
        })
        .collect();
    apply_water_point_lights(&mut world);
    (world, strip, list)
}

/// Tile C++ 15×15 sea patches across the water extent instead of stretching
/// one patch over the whole map. Each tile keeps native `PATCH_WIDTH` UVs.
#[must_use]
pub fn bake_water_tiles_world(
    min_x: f32,
    min_z: f32,
    max_x: f32,
    max_z: f32,
    water_y: f32,
    transparent_water_diffuse: u32,
) -> (Vec<SeaPatchVertex>, Vec<u32>) {
    const MAX_TILES_PER_AXIS: usize = 24;
    let span_x = (max_x - min_x).max(0.0);
    let span_z = (max_z - min_z).max(0.0);
    if span_x <= 0.0 || span_z <= 0.0 {
        return (Vec::new(), Vec::new());
    }
    let tile = PATCH_WIDTH as f32;
    let tiles_x = ((span_x / tile).ceil() as usize).clamp(1, MAX_TILES_PER_AXIS);
    let tiles_z = ((span_z / tile).ceil() as usize).clamp(1, MAX_TILES_PER_AXIS);
    let step_x = span_x / tiles_x as f32;
    let step_z = span_z / tiles_z as f32;
    let mut verts = Vec::new();
    let mut indices = Vec::new();
    for tz in 0..tiles_z {
        for tx in 0..tiles_x {
            let x0 = min_x + tx as f32 * step_x;
            let z0 = min_z + tz as f32 * step_z;
            let x1 = if tx + 1 == tiles_x {
                max_x
            } else {
                x0 + step_x
            };
            let z1 = if tz + 1 == tiles_z {
                max_z
            } else {
                z0 + step_z
            };
            let (patch, _strip, list) =
                bake_water_patch_world(x0, z0, x1, z1, water_y, transparent_water_diffuse);
            let base = verts.len() as u32;
            indices.extend(list.into_iter().map(|i| i + base));
            verts.extend(patch);
        }
    }
    (verts, indices)
}

/// One terrain directional light for C++ standing-water lighting.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterTerrainLight {
    pub light_pos: [f32; 3],
    pub diffuse: [f32; 3],
}

/// C++ `W3DWater.cpp` standing-water diffuse (global terrain lights + waterDiffuse alpha).
pub fn compute_standing_water_diffuse(
    standing_color: [f32; 3],
    water_diffuse: u32,
    terrain_ambient: [f32; 3],
    terrain_lights: &[WaterTerrainLight],
) -> u32 {
    let mut shade_r;
    let mut shade_g;
    let mut shade_b;
    if (standing_color[0] - 1.0).abs() < 1.0e-5
        && (standing_color[1] - 1.0).abs() < 1.0e-5
        && (standing_color[2] - 1.0).abs() < 1.0e-5
    {
        shade_r = terrain_ambient[0];
        shade_g = terrain_ambient[1];
        shade_b = terrain_ambient[2];
        for light in terrain_lights {
            if -light.light_pos[2] > 0.0 {
                let nz = -light.light_pos[2];
                shade_r += nz * light.diffuse[0];
                shade_g += nz * light.diffuse[1];
                shade_b += nz * light.diffuse[2];
            }
        }
        let water_r = (water_diffuse & 0xFF) as f32 / 255.0;
        let water_g = ((water_diffuse >> 8) & 0xFF) as f32 / 255.0;
        let water_b = ((water_diffuse >> 16) & 0xFF) as f32 / 255.0;
        shade_r *= water_r * 255.0;
        shade_g *= water_g * 255.0;
        shade_b *= water_b * 255.0;
    } else {
        shade_r = standing_color[0] * 255.0;
        shade_g = standing_color[1] * 255.0;
        shade_b = standing_color[2] * 255.0;
        if shade_r == 0.0 && shade_g == 0.0 && shade_b == 0.0 {
            shade_r = 255.0;
            shade_g = 255.0;
            shade_b = 255.0;
        }
    }
    let packed = (real_to_int(shade_b) as u32)
        | ((real_to_int(shade_g) as u32) << 8)
        | ((real_to_int(shade_r) as u32) << 16);
    packed | (water_diffuse & 0xFF00_0000)
}

/// Apply live `createLightPulse` POINT lights to a Y-up water patch (HeightMap formula).
pub fn apply_water_point_lights(vertices: &mut [SeaPatchVertex]) {
    let lights = scene_dynamic_lights();
    if lights.is_empty() {
        return;
    }
    for vertex in vertices {
        // Water patch is Y-up; C++ map XYZ is Z-up.
        let xyz = [vertex.x, vertex.z, vertex.y];
        let normal = [0.0, 0.0, 1.0];
        vertex.c = do_the_dynamic_light(xyz, normal, vertex.c, &lights);
    }
}

#[derive(Clone, Copy)]
struct RoadColumn {
    collapsed: bool,
    deleted: bool,
    vtx: [[f32; 3]; ROAD_MAX_ROWS],
    vertex_index: [i32; ROAD_MAX_ROWS],
    u_index: f32,
}

impl Default for RoadColumn {
    fn default() -> Self {
        Self {
            collapsed: false,
            deleted: true,
            vtx: [[0.0; 3]; ROAD_MAX_ROWS],
            vertex_index: [-1; ROAD_MAX_ROWS],
            u_index: 0.0,
        }
    }
}

/// C++ `W3DRoadBuffer::loadFloat4PtSection`.
///
/// Map plane is C++ XY, height is Z. `sample_max_cell_height(x, y)` matches
/// `TheTerrainRenderObject->getMaxCellHeight`.
pub fn load_float_4pt_section(
    loc: [f32; 2],
    mut road_normal: [f32; 2],
    mut road_vector: [f32; 2],
    corners: [[f32; 2]; 4],
    u_offset: f32,
    v_offset: f32,
    u_scale: f32,
    v_scale: f32,
    sample_max_cell_height: impl Fn(f32, f32) -> f32,
) -> (Vec<RoadSegVertex>, Vec<u16>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let road_len = vec2_len(road_vector).max(1.0e-6);
    let half_height = vec2_len(road_normal).max(1.0e-6);
    road_normal = vec2_norm(road_normal);
    road_vector = vec2_norm(road_vector);

    let mut u_count = (road_len / MAP_XY_FACTOR) as i32 + 1;
    if u_count < 2 {
        u_count = 2;
    }
    let mut v_count = ((2.0 * half_height) / MAP_XY_FACTOR) as i32 + 1;
    if v_count < 2 {
        v_count = 2;
    }
    if v_count as usize > ROAD_MAX_ROWS {
        v_count = ROAD_MAX_ROWS as i32;
    }
    let v_count = v_count as usize;
    let u_count = u_count as usize;

    let origin = [corners[0][0], corners[0][1], 0.0];
    let u_vector1 = [
        corners[1][0] - corners[0][0],
        corners[1][1] - corners[0][1],
        0.0,
    ];
    let v_vector1 = [
        corners[2][0] - corners[0][0],
        corners[2][1] - corners[0][1],
        0.0,
    ];
    let v_vector2 = [
        corners[3][0] - corners[1][0],
        corners[3][1] - corners[1][1],
        0.0,
    ];
    let mut u_vector2 = [
        corners[3][0] - corners[2][0],
        corners[3][1] - corners[2][1],
        0.0,
    ];
    u_vector2 = [
        u_vector2[0] + (v_vector1[0] - v_vector2[0]),
        u_vector2[1] + (v_vector1[1] - v_vector2[1]),
        0.0,
    ];

    let mut prev_column = RoadColumn::default();
    let mut cur_column = RoadColumn::default();
    let mut next_column = RoadColumn::default();

    for i in 0..=u_count {
        let i_factor = i as f32 / (u_count - 1) as f32;
        let i_bar = 1.0 - i_factor;
        if i < u_count {
            next_column.collapsed = false;
            next_column.deleted = false;
            next_column.u_index = i as f32;

            let mut min_height = f32::MAX;
            let mut max_height = f32::MIN;
            for j in 0..v_count {
                let j_factor = j as f32 / (v_count - 1) as f32;
                let j_bar = 1.0 - j_factor;
                let x = origin[0]
                    + u_vector1[0] * j_bar * i_factor
                    + u_vector2[0] * j_factor * i_factor
                    + v_vector1[0] * i_bar * j_factor
                    + v_vector2[0] * i_factor * j_factor;
                let y = origin[1]
                    + u_vector1[1] * j_bar * i_factor
                    + u_vector2[1] * j_factor * i_factor
                    + v_vector1[1] * i_bar * j_factor
                    + v_vector2[1] * i_factor * j_factor;
                let z = sample_max_cell_height(x, y);
                min_height = min_height.min(z);
                max_height = max_height.max(z);
                next_column.vtx[j] = [x, y, z];
                next_column.vertex_index[j] = -1;
            }

            // C++ currently always collapses (`if (true)`).
            next_column.collapsed = true;
            next_column.vtx[0][2] = max_height;
            next_column.vtx[1] = next_column.vtx[v_count - 1];
            next_column.vtx[1][2] = max_height;

            if i < 2 {
                cur_column = next_column;
            } else if prev_column.collapsed && cur_column.collapsed && next_column.collapsed {
                let denom = next_column.u_index - prev_column.u_index;
                if denom.abs() > f32::EPSILON {
                    let mut ok_to_delete = false;
                    let the_z = (prev_column.vtx[0][2]
                        * (cur_column.u_index - prev_column.u_index)
                        + next_column.vtx[0][2] * (next_column.u_index - cur_column.u_index))
                        / denom;
                    if the_z >= cur_column.vtx[0][2]
                        && the_z < cur_column.vtx[0][2] + ROAD_MAX_ERROR
                    {
                        let the_z2 = (prev_column.vtx[1][2]
                            * (cur_column.u_index - prev_column.u_index)
                            + next_column.vtx[1][2] * (next_column.u_index - cur_column.u_index))
                            / denom;
                        if the_z2 >= cur_column.vtx[1][2]
                            && the_z2 < cur_column.vtx[1][2] + ROAD_MAX_ERROR
                        {
                            ok_to_delete = true;
                        }
                    }
                    if ok_to_delete {
                        cur_column.deleted = true;
                    }
                }
            }
        }

        if !cur_column.deleted && i != 1 {
            for j in 0..v_count {
                if vertices.len() >= MAX_SEG_VERTEX {
                    break;
                }
                let dx = cur_column.vtx[j][0] - loc[0];
                let dy = cur_column.vtx[j][1] - loc[1];
                let v_dot = road_normal[0] * dx + road_normal[1] * dy;
                let u_dot = road_vector[0] * dx + road_vector[1] * dy;
                let u_scale = u_scale.max(1.0e-6);
                let v_scale = v_scale.max(1.0e-6);
                cur_column.vertex_index[j] = vertices.len() as i32;
                vertices.push(RoadSegVertex {
                    x: cur_column.vtx[j][0],
                    y: cur_column.vtx[j][1],
                    z: cur_column.vtx[j][2] + ROAD_FLOAT_AMOUNT,
                    diffuse: 0,
                    u1: u_offset + u_dot / (u_scale * 4.0),
                    v1: v_offset - v_dot / (v_scale * 4.0),
                });
                if j == 1 && cur_column.collapsed {
                    break;
                }
            }
            if vertices.len() >= MAX_SEG_VERTEX {
                break;
            }
            if i > 1 {
                let mut j = 0usize;
                let mut k = 0usize;
                while j < v_count.saturating_sub(1) && k < v_count.saturating_sub(1) {
                    if indices.len() >= MAX_SEG_INDEX {
                        break;
                    }
                    if k == 0 || !prev_column.collapsed {
                        let a = prev_column.vertex_index[j + 1];
                        let b = prev_column.vertex_index[j];
                        let c = cur_column.vertex_index[k];
                        if a >= 0 && b >= 0 && c >= 0 && indices.len() + 3 <= MAX_SEG_INDEX {
                            indices.extend_from_slice(&[a as u16, b as u16, c as u16]);
                        }
                    }
                    if j == 0 || !cur_column.collapsed {
                        let offset = if cur_column.collapsed && !prev_column.collapsed {
                            v_count - 1
                        } else {
                            1
                        };
                        let a = prev_column.vertex_index[j + offset];
                        let b = cur_column.vertex_index[k];
                        let c = cur_column.vertex_index[k + 1];
                        if a >= 0 && b >= 0 && c >= 0 && indices.len() + 3 <= MAX_SEG_INDEX {
                            indices.extend_from_slice(&[a as u16, b as u16, c as u16]);
                        }
                    }
                    if prev_column.collapsed && cur_column.collapsed {
                        break;
                    }
                    if !prev_column.collapsed {
                        j += 1;
                    }
                    if !cur_column.collapsed {
                        k += 1;
                    }
                }
                prev_column = cur_column;
            } else if i == 0 {
                prev_column = cur_column;
            }
            if indices.len() >= MAX_SEG_INDEX {
                break;
            }
        }
        cur_column = next_column;
    }

    // C++ W3DRoadBuffer::loadFloat4PtSection: static diffuse then POINT lights.
    light_road_seg_vertices(&mut vertices, 0xFFFF_FFFF);
    (vertices, indices)
}

/// C++ `REAL_TO_INT` used when packing road `vb.diffuse`.
fn real_to_int(value: f32) -> i32 {
    if value >= 0.0 {
        (value + 0.5) as i32
    } else {
        (value - 0.5) as i32
    }
}

/// C++ `W3DRoadBuffer` per-vertex POINT light (fixed `shade = 0.5 * factor`, XY AABB).
pub fn do_road_dynamic_light(
    xyz: [f32; 3],
    base_diffuse: u32,
    lights: &[DisplayDynamicLight],
) -> u32 {
    const OO255: f32 = 1.0 / 255.0;
    let mut shade_r = (((base_diffuse >> 16) & 0xFF) as f32) * OO255;
    let mut shade_g = (((base_diffuse >> 8) & 0xFF) as f32) * OO255;
    let mut shade_b = ((base_diffuse & 0xFF) as f32) * OO255;

    for light in lights {
        if !light.enabled || !light.far_attenuation {
            continue;
        }
        if light.pos[0] - light.far_atten_end > xyz[0]
            || light.pos[0] + light.far_atten_end < xyz[0]
            || light.pos[1] - light.far_atten_end > xyz[1]
            || light.pos[1] + light.far_atten_end < xyz[1]
        {
            continue;
        }
        let dx = xyz[0] - light.pos[0];
        let dy = xyz[1] - light.pos[1];
        let dz = xyz[2] - light.pos[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();
        let Some(factor) = far_atten_factor(dist, light.far_atten_start, light.far_atten_end)
        else {
            continue;
        };
        let shade = (0.5 * factor).clamp(0.0, 1.0);
        shade_r += shade * light.color[0];
        shade_g += shade * light.color[1];
        shade_b += shade * light.color[2];
        shade_r += factor * light.color[0];
        shade_g += factor * light.color[1];
        shade_b += factor * light.color[2];
    }

    shade_r = shade_r.clamp(0.0, 1.0) * 255.0;
    shade_g = shade_g.clamp(0.0, 1.0) * 255.0;
    shade_b = shade_b.clamp(0.0, 1.0) * 255.0;
    (real_to_int(shade_b) as u32)
        | ((real_to_int(shade_g) as u32) << 8)
        | ((real_to_int(shade_r) as u32) << 16)
        | (255u32 << 24)
}

/// Bake live `createLightPulse` POINT lights into C++ road VB diffuse.
pub fn light_road_seg_vertices(vertices: &mut [RoadSegVertex], static_diffuse: u32) {
    let lights = scene_dynamic_lights();
    for vertex in vertices {
        let base = if vertex.diffuse == 0 {
            static_diffuse
        } else {
            vertex.diffuse
        };
        vertex.diffuse = do_road_dynamic_light([vertex.x, vertex.y, vertex.z], base, &lights);
    }
}

/// Bake a straight road segment in C++ map-XY space.
/// Returns `None` when the vector is shorter than `MIN_ROAD_SEGMENT`.
pub fn bake_straight_road_segment(
    start: [f32; 2],
    end: [f32; 2],
    width: f32,
    u_offset: f32,
    v_offset: f32,
    scale: f32,
    sample_max_cell_height: impl Fn(f32, f32) -> f32,
) -> Option<(Vec<RoadSegVertex>, Vec<u16>)> {
    let road_vector = [end[0] - start[0], end[1] - start[1]];
    if road_vector[0].abs() < MIN_ROAD_SEGMENT && road_vector[1].abs() < MIN_ROAD_SEGMENT {
        return None;
    }
    let len = vec2_len(road_vector);
    if len < MIN_ROAD_SEGMENT {
        return None;
    }
    let mut road_normal = [-road_vector[1], road_vector[0]];
    let nlen = vec2_len(road_normal);
    if nlen <= f32::EPSILON {
        road_normal = [0.0, 1.0];
    } else {
        road_normal = [road_normal[0] / nlen, road_normal[1] / nlen];
    }
    let half = width.max(0.1) * 0.5;
    road_normal = [road_normal[0] * half, road_normal[1] * half];
    let bottom_left = [start[0] - road_normal[0], start[1] - road_normal[1]];
    let top_left = [start[0] + road_normal[0], start[1] + road_normal[1]];
    let bottom_right = [end[0] - road_normal[0], end[1] - road_normal[1]];
    let top_right = [end[0] + road_normal[0], end[1] + road_normal[1]];
    Some(load_float_4pt_section(
        start,
        road_normal,
        road_vector,
        [bottom_left, bottom_right, top_left, top_right],
        u_offset,
        v_offset,
        scale.max(0.1),
        scale.max(0.1),
        sample_max_cell_height,
    ))
}

/// Default left/section/right quads used when W3D bridge assets are unavailable.
#[must_use]
pub fn default_sectional_bridge_model(
    scale: f32,
) -> (BridgeMeshQuad, BridgeMeshQuad, BridgeMeshQuad) {
    (
        BridgeMeshQuad::new(0.0, 20.0, -5.0, 5.0, scale),
        BridgeMeshQuad::new(20.0, 60.0, -5.0, 5.0, scale),
        BridgeMeshQuad::new(60.0, 80.0, -5.0, 5.0, scale),
    )
}

/// Axis-aligned bridge mesh section in model space (C++ XY, Z up).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BridgeMeshQuad {
    pub min_x: f32,
    pub max_x: f32,
    pub min_y: f32,
    pub max_y: f32,
    pub scale: f32,
}

impl BridgeMeshQuad {
    #[must_use]
    pub fn new(min_x: f32, max_x: f32, min_y: f32, max_y: f32, scale: f32) -> Self {
        Self {
            min_x,
            max_x,
            min_y,
            max_y,
            scale,
        }
    }

    fn vertices(self) -> [([f32; 3], f32, f32); 4] {
        [
            ([self.min_x, self.min_y, 0.0], 0.0, 1.0),
            ([self.max_x, self.min_y, 0.0], 1.0, 1.0),
            ([self.max_x, self.max_y, 0.0], 1.0, 0.0),
            ([self.min_x, self.max_y, 0.0], 0.0, 0.0),
        ]
    }
}

/// C++ `W3DBridge::getIndicesNVertices` sectional/fixed bake.
pub fn bake_bridge_span(
    from: [f32; 3],
    to: [f32; 3],
    sectional: bool,
    left: BridgeMeshQuad,
    section: Option<BridgeMeshQuad>,
    right: Option<BridgeMeshQuad>,
    diffuse: u32,
) -> BridgeOverlayBuffers {
    let mut buffers = BridgeOverlayBuffers::default();
    let mut vec = [to[0] - from[0], to[1] - from[1], to[2] - from[2]];
    let desired_length = vec3_len(vec).max(1.0);
    if vec3_len2(vec) < 1.0 {
        vec = vec3_norm(vec);
    }
    let mut vec_normal = [-vec[1], vec[0], 0.0];
    vec_normal = vec3_norm(vec_normal);
    let delta_z = (to[2] - from[2]) / desired_length;
    let delta_x = (1.0 - delta_z * delta_z).max(0.0).sqrt();
    let mut vec_z = [-delta_z, 0.0, delta_x];

    let scale = left.scale.max(0.01);
    let length = (if sectional {
        right.map(|r| r.max_x).unwrap_or(left.max_x) - left.min_x
    } else {
        left.max_x - left.min_x
    })
    .max(1.0);

    if !sectional || section.is_none() || right.is_none() {
        vec = [vec[0] / length, vec[1] / length, vec[2] / length];
        vec_normal = [
            vec_normal[0] * scale,
            vec_normal[1] * scale,
            vec_normal[2] * scale,
        ];
        vec_z = [vec_z[0] * scale, vec_z[1] * scale, vec_z[2] * scale];
        append_bridge_quad(
            &mut buffers,
            left,
            from,
            -left.min_x,
            vec,
            vec_normal,
            vec_z,
            diffuse,
        );
        return buffers;
    }

    let section = section.unwrap();
    let right = right.unwrap();
    let span_length = right.min_x - left.max_x;
    let mut num_spans = 1i32;
    if span_length.abs() > f32::EPSILON {
        let spannable = desired_length - (length - span_length);
        num_spans = ((spannable + span_length / 2.0) / span_length).floor() as i32;
        if num_spans < 0 {
            num_spans = 0;
        }
    }
    let bridge_length = (length + (num_spans - 1) as f32 * span_length).max(1.0);
    vec = [
        vec[0] / bridge_length,
        vec[1] / bridge_length,
        vec[2] / bridge_length,
    ];
    vec_normal = [
        vec_normal[0] * scale,
        vec_normal[1] * scale,
        vec_normal[2] * scale,
    ];
    vec_z = [vec_z[0] * scale, vec_z[1] * scale, vec_z[2] * scale];
    let x_offset = -left.min_x;
    append_bridge_quad(
        &mut buffers,
        left,
        from,
        x_offset,
        vec,
        vec_normal,
        vec_z,
        diffuse,
    );
    for i in 0..num_spans {
        append_bridge_quad(
            &mut buffers,
            section,
            from,
            x_offset + i as f32 * span_length,
            vec,
            vec_normal,
            vec_z,
            diffuse,
        );
    }
    append_bridge_quad(
        &mut buffers,
        right,
        from,
        x_offset + (num_spans - 1) as f32 * span_length,
        vec,
        vec_normal,
        vec_z,
        diffuse,
    );
    buffers
}

fn append_bridge_quad(
    buffers: &mut BridgeOverlayBuffers,
    mesh: BridgeMeshQuad,
    start: [f32; 3],
    x_offset: f32,
    vec: [f32; 3],
    vec_normal: [f32; 3],
    vec_z: [f32; 3],
    diffuse: u32,
) {
    if buffers.vertices.len() + 4 + 2 >= MAX_BRIDGE_VERTEX {
        return;
    }
    if buffers.indices.len() + 6 + 6 >= MAX_BRIDGE_INDEX {
        return;
    }
    let vertex_offset = buffers.vertices.len() as u16;
    for (pos, u, v) in mesh.vertices() {
        let v_loc = [
            start[0] + vec[0] * (pos[0] + x_offset) + vec_normal[0] * pos[1] + vec_z[0] * pos[2],
            start[1] + vec[1] * (pos[0] + x_offset) + vec_normal[1] * pos[1] + vec_z[1] * pos[2],
            start[2] + vec[2] * (pos[0] + x_offset) + vec_normal[2] * pos[1] + vec_z[2] * pos[2],
        ];
        buffers.vertices.push(BridgeOverlayVertex {
            x: v_loc[0],
            y: v_loc[1],
            z: v_loc[2],
            diffuse: diffuse | 0xff00_0000,
            nx: 0.0,
            ny: 0.0,
            nz: 1.0,
            u1: u,
            v1: v,
        });
    }
    buffers.indices.extend_from_slice(&[
        vertex_offset,
        vertex_offset + 1,
        vertex_offset + 2,
        vertex_offset,
        vertex_offset + 2,
        vertex_offset + 3,
    ]);
}

fn vec2_len(v: [f32; 2]) -> f32 {
    (v[0] * v[0] + v[1] * v[1]).sqrt()
}

fn vec2_norm(v: [f32; 2]) -> [f32; 2] {
    let len = vec2_len(v);
    if len > f32::EPSILON {
        [v[0] / len, v[1] / len]
    } else {
        v
    }
}

fn vec3_len2(v: [f32; 3]) -> f32 {
    v[0] * v[0] + v[1] * v[1] + v[2] * v[2]
}

fn vec3_len(v: [f32; 3]) -> f32 {
    vec3_len2(v).sqrt()
}

fn vec3_norm(v: [f32; 3]) -> [f32; 3] {
    let len = vec3_len(v);
    if len > f32::EPSILON {
        [v[0] / len, v[1] / len, v[2] / len]
    } else {
        v
    }
}

/// Convert C++ map-XY / Z-up vertex into TerrainVisual Y-up `[x, height, y]`.
#[must_use]
pub fn cpp_map_to_y_up(x: f32, y: f32, z: f32) -> [f32; 3] {
    [x, z, y]
}

/// Unpack C++ BGRA `vb.diffuse` (R in bits 16-23) to linear RGB.
#[must_use]
pub fn unpack_bgra_rgb(diffuse: u32) -> [f32; 3] {
    [
        ((diffuse >> 16) & 0xFF) as f32 / 255.0,
        ((diffuse >> 8) & 0xFF) as f32 / 255.0,
        (diffuse & 0xFF) as f32 / 255.0,
    ]
}

/// Unpack C++ BGRA including waterDiffuse alpha (bits 24-31).
#[must_use]
pub fn unpack_bgra_rgba(diffuse: u32) -> [f32; 4] {
    let rgb = unpack_bgra_rgb(diffuse);
    [
        rgb[0],
        rgb[1],
        rgb[2],
        ((diffuse >> 24) & 0xFF) as f32 / 255.0,
    ]
}

/// Shipped wgpu water overlay vertex. `packed_c` is C++ `SEA_PATCH_VERTEX.c`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterGpuVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub tex_coords: [f32; 2],
    pub alpha: f32,
    pub packed_c: u32,
}

// SAFETY: `#[repr(C)]` water vertex of f32 arrays + packed_c u32; byte-exact
// SAFETY: layout documented against C++ SEA_PATCH_VERTEX, uploaded as raw bytes.
unsafe impl bytemuck::Pod for WaterGpuVertex {}
// SAFETY: All-zero fields are valid values; alpha 0.0 is meaningful, not UB.
unsafe impl bytemuck::Zeroable for WaterGpuVertex {}

impl WaterGpuVertex {
    #[must_use]
    pub fn from_sea_patch(vertex: &SeaPatchVertex) -> Self {
        let rgba = unpack_bgra_rgba(vertex.c);
        Self {
            // bake_water_patch_world already emits Y-up world positions.
            position: [vertex.x, vertex.y, vertex.z],
            color: [rgba[0], rgba[1], rgba[2]],
            tex_coords: [vertex.tu, vertex.tv],
            alpha: rgba[3],
            packed_c: vertex.c,
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<WaterGpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 8]>() + std::mem::size_of::<f32>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

/// C++ sea-patch VB → shipped wgpu upload (keeps HeightMap POINT/global `c`).
#[must_use]
pub fn fill_water_gpu_upload_vertices(cpu: &[SeaPatchVertex]) -> Vec<WaterGpuVertex> {
    cpu.iter().map(WaterGpuVertex::from_sea_patch).collect()
}

/// Shipped wgpu overlay vertex: Y-up + unpacked doLighting/do_road_dynamic_light + packed BGRA.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayGpuVertex {
    pub position: [f32; 3],
    pub color: [f32; 3],
    pub tex_coords: [f32; 2],
    pub road_width: f32,
    pub diffuse: u32,
}

// SAFETY: `#[repr(C)]` overlay vertex of f32 arrays + diffuse u32, no padding
// SAFETY: holes; consumed only as raw vertex-buffer bytes.
unsafe impl bytemuck::Pod for OverlayGpuVertex {}
// SAFETY: Zeroed fields are valid defaults; no pointer or niche members.
unsafe impl bytemuck::Zeroable for OverlayGpuVertex {}

impl OverlayGpuVertex {
    #[must_use]
    pub fn from_cpp_xyzduv(x: f32, y: f32, z: f32, diffuse: u32, u1: f32, v1: f32) -> Self {
        Self {
            position: cpp_map_to_y_up(x, y, z),
            color: unpack_bgra_rgb(diffuse),
            tex_coords: [u1, v1],
            road_width: 1.0,
            diffuse,
        }
    }

    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<OverlayGpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
                wgpu::VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 8]>() + std::mem::size_of::<f32>())
                        as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

/// C++ `loadFloat4PtSection` VB → shipped wgpu overlay upload (keeps `do_road_dynamic_light`).
#[must_use]
pub fn fill_road_gpu_upload_vertices(cpu: &[RoadSegVertex]) -> Vec<OverlayGpuVertex> {
    cpu.iter()
        .map(|v| OverlayGpuVertex::from_cpp_xyzduv(v.x, v.y, v.z, v.diffuse, v.u1, v.v1))
        .collect()
}

/// C++ bridge `VertexFormatXYZNDUV1` → wgpu overlay upload (keeps packed diffuse).
#[must_use]
pub fn fill_bridge_gpu_upload_vertices(cpu: &[BridgeOverlayVertex]) -> Vec<OverlayGpuVertex> {
    cpu.iter()
        .map(|v| OverlayGpuVertex::from_cpp_xyzduv(v.x, v.y, v.z, v.diffuse, v.u1, v.v1))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpp_water_constants_match_w3d_water_cpp() {
        assert_eq!(PATCH_SIZE, 15);
        assert_eq!(PATCH_WIDTH, 14);
        assert!((PATCH_UV_SCALE - 3.0).abs() < f32::EPSILON);
        assert_eq!(SEA_REFLECTION_SIZE, 256);
        assert_eq!(NUM_BUMP_FRAMES, 32);
        assert_eq!(WATER_MESH_X_VERTICES, 128);
        assert_eq!(WATER_MESH_Y_VERTICES, 128);
        assert!((PATCH_SCALE * MAP_XY_FACTOR - 40.0).abs() < f32::EPSILON);
    }

    #[test]
    fn generate_water_vertex_buffer_matches_cpp_sea_patch_layout() {
        let verts = generate_water_vertex_buffer(PATCH_SIZE, PATCH_SIZE, 7.5, 0x80ff_ff00);
        assert_eq!(verts.len(), PATCH_SIZE * PATCH_SIZE);
        assert_eq!(verts[0].x, 0.0);
        assert_eq!(verts[0].y, 7.5);
        assert_eq!(verts[0].z, 0.0);
        assert_eq!(verts[0].tu, 0.0);
        assert_eq!(verts[0].tv, 0.0);
        assert_eq!(verts[0].c, 0x80ff_ff00);

        let last = verts[PATCH_SIZE - 1];
        assert_eq!(last.x, 14.0);
        assert_eq!(last.y, 7.5);
        assert_eq!(last.z, 0.0);
        assert!((last.tu - 14.0 * PATCH_UV_SCALE).abs() < 1.0e-5);
        assert!((last.tu - 42.0).abs() < 1.0e-5);

        let corner = verts[PATCH_SIZE * (PATCH_SIZE - 1) + (PATCH_SIZE - 1)];
        assert_eq!(corner.x, 14.0);
        assert_eq!(corner.z, 14.0);
        assert!((corner.tv - 42.0).abs() < 1.0e-5);
    }

    #[test]
    fn generate_water_index_buffer_matches_cpp_strip_and_degenerates() {
        let indices = generate_water_index_buffer(PATCH_SIZE, PATCH_SIZE);
        assert_eq!(
            indices.len(),
            water_strip_index_count(PATCH_SIZE, PATCH_SIZE)
        );
        assert_eq!(indices.len(), (PATCH_SIZE - 1) * (PATCH_SIZE * 2 + 2) - 2);
        // First strip pair: (sizeX, 0), (sizeX+1, 1), ...
        assert_eq!(indices[0], PATCH_SIZE as u16);
        assert_eq!(indices[1], 0);
        assert_eq!(indices[2], (PATCH_SIZE + 1) as u16);
        assert_eq!(indices[3], 1);
        // Degenerate join after first row of 15 pairs (30 indices): last, first-of-next.
        let join = PATCH_SIZE * 2;
        assert_eq!(indices[join], (PATCH_SIZE - 1) as u16);
        assert_eq!(indices[join + 1], (PATCH_SIZE * 2) as u16);

        let list = triangle_list_from_strip(&indices);
        assert!(!list.is_empty());
        assert_eq!(list.len() % 3, 0);
        assert!(list.len() > indices.len());
    }

    #[test]
    fn bake_water_tiles_world_does_not_stretch_one_patch() {
        let (verts, list) = bake_water_tiles_world(-100.0, -50.0, 100.0, 50.0, 3.0, 0xffff_ffff);
        // 200×100 extent / PATCH_WIDTH(14) → more than one 15×15 sheet.
        assert!(
            verts.len() > PATCH_SIZE * PATCH_SIZE,
            "water must tile, not stretch one 15x15 patch; got {} verts",
            verts.len()
        );
        assert!(!list.is_empty());
        assert_eq!(list.len() % 3, 0);
        assert!((verts[0].x - (-100.0)).abs() < 1.0e-3);
        assert!((verts[0].z - (-50.0)).abs() < 1.0e-3);
        assert_eq!(verts[0].y, 3.0);
        let last = verts.last().copied().expect("verts");
        assert!((last.x - 100.0).abs() < 1.0e-3);
        assert!((last.z - 50.0).abs() < 1.0e-3);
    }

    #[test]
    fn bake_water_patch_world_preserves_cpp_uvs_and_maps_extent() {
        let (verts, strip, list) =
            bake_water_patch_world(-100.0, -50.0, 100.0, 50.0, 3.0, 0xffff_ffff);
        assert_eq!(verts.len(), PATCH_SIZE * PATCH_SIZE);
        assert_eq!(strip.len(), water_strip_index_count(PATCH_SIZE, PATCH_SIZE));
        assert_eq!(list.len() % 3, 0);
        assert!((verts[0].x - (-100.0)).abs() < 1.0e-4);
        assert!((verts[0].z - (-50.0)).abs() < 1.0e-4);
        assert_eq!(verts[0].y, 3.0);
        let last_x = verts[PATCH_SIZE - 1];
        assert!((last_x.x - 100.0).abs() < 1.0e-4);
        assert!((last_x.tu - 42.0).abs() < 1.0e-4);
    }

    #[test]
    fn compute_standing_water_diffuse_matches_cpp_w3d_water() {
        let water_diffuse = 0x80FF_CC99; // A=0x80, R=0xFF, G=0xCC, B=0x99 in ARGB naming... packed as u32 hex
        // C++ unpacks waterShadeR = low byte, G = <<8, B = <<16 then packs shadeB|G<<8|R<<16
        let lights = [WaterTerrainLight {
            light_pos: [0.0, 0.0, -1.0],
            diffuse: [0.5, 0.25, 0.0],
        }];
        let packed = compute_standing_water_diffuse(
            [1.0, 1.0, 1.0],
            water_diffuse,
            [0.2, 0.2, 0.2],
            &lights,
        );
        let water_r = (water_diffuse & 0xFF) as f32 / 255.0;
        let water_g = ((water_diffuse >> 8) & 0xFF) as f32 / 255.0;
        let water_b = ((water_diffuse >> 16) & 0xFF) as f32 / 255.0;
        let shade_r = (0.2 + 1.0 * 0.5) * water_r * 255.0;
        let shade_g = (0.2 + 1.0 * 0.25) * water_g * 255.0;
        let shade_b = (0.2 + 0.0) * water_b * 255.0;
        let expect = (super::real_to_int(shade_b) as u32)
            | ((super::real_to_int(shade_g) as u32) << 8)
            | ((super::real_to_int(shade_r) as u32) << 16)
            | 0x8000_0000;
        assert_eq!(packed, expect);
        assert_eq!(packed >> 24, 0x80);

        let black = compute_standing_water_diffuse([0.0, 0.0, 0.0], 0x40FF_FFFF, [0.0; 3], &[]);
        assert_eq!(black & 0x00FF_FFFF, 0x00FF_FFFF);
        assert_eq!(black >> 24, 0x40);
    }

    #[test]
    fn bake_water_patch_world_applies_point_lights_like_heightmap() {
        use crate::fx_list::{
            DisplayLightPulse, clear_scene_dynamic_lights, create_display_light_pulse,
            do_the_dynamic_light, drain_display_light_pulses, scene_dynamic_lights,
        };

        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
        assert!(create_display_light_pulse(DisplayLightPulse {
            pos: [0.0, 0.0, 3.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 10.0,
            outer_radius: 80.0,
            increase_frames: 0,
            decay_frames: 0,
        }));
        let (verts, _, _) = bake_water_patch_world(-20.0, -20.0, 20.0, 20.0, 3.0, 0x8033_3333);
        assert!(!verts.is_empty());
        let lights = scene_dynamic_lights();
        let v = verts[0];
        let expected = do_the_dynamic_light([v.x, v.z, v.y], [0.0, 0.0, 1.0], 0x8033_3333, &lights);
        assert_eq!(v.c, expected);
        let gpu = fill_water_gpu_upload_vertices(&verts);
        assert_eq!(gpu.len(), verts.len());
        for (cpu, uploaded) in verts.iter().zip(gpu.iter()) {
            let expected_c =
                do_the_dynamic_light([cpu.x, cpu.z, cpu.y], [0.0, 0.0, 1.0], 0x8033_3333, &lights);
            assert_eq!(cpu.c, expected_c);
            assert_eq!(
                uploaded.packed_c, expected_c,
                "wgpu VB must keep SeaPatchVertex.c"
            );
            assert_eq!(uploaded.position, [cpu.x, cpu.y, cpu.z]);
            assert_eq!(uploaded.color, unpack_bgra_rgb(expected_c));
            assert_eq!(uploaded.tex_coords, [cpu.tu, cpu.tv]);
            let expect_a = ((expected_c >> 24) & 0xFF) as f32 / 255.0;
            assert!((uploaded.alpha - expect_a).abs() < 1e-5);
        }
        let shader = include_str!("../shaders/water.wgsl");
        assert!(
            shader.contains("out.color = vertex.color"),
            "water VS must pass SeaPatchVertex.c"
        );
        assert!(
            shader.contains("return vec4<f32>(in.color, in.alpha)"),
            "water FS must use packed c, not fake shallow/deep"
        );
        assert!(
            !shader.contains("shallow = vec3<f32>(0.05, 0.16, 0.22)"),
            "water FS must not ignore HeightMap POINT/global light bake"
        );
        clear_scene_dynamic_lights();
    }

    #[test]
    fn cpp_road_constants_match_w3d_road_buffer_h() {
        assert_eq!(MAX_SEG_VERTEX, 500);
        assert_eq!(MAX_SEG_INDEX, 2000);
        assert_eq!(DEFAULT_ROAD_SCALE, 8.0);
        assert_eq!(MIN_ROAD_SEGMENT, 0.25);
        assert_eq!(MAX_LINKS, 6);
        assert_eq!(NUM_CORNERS, 4);
        assert!((ROAD_FLOAT_AMOUNT - MAP_HEIGHT_SCALE / 8.0).abs() < 1.0e-6);
        assert!((ROAD_MAX_ERROR - MAP_HEIGHT_SCALE * 1.1).abs() < 1.0e-6);
    }

    #[test]
    fn min_road_segment_rejects_degenerate_span() {
        assert!(
            bake_straight_road_segment(
                [0.0, 0.0],
                [0.1, 0.0],
                8.0,
                0.0,
                85.0 / 512.0,
                DEFAULT_ROAD_SCALE,
                |_, _| 0.0,
            )
            .is_none()
        );
    }

    #[test]
    fn load_float_4pt_section_applies_float_amount_and_collapses_flat_columns() {
        let baked = bake_straight_road_segment(
            [0.0, 0.0],
            [40.0, 0.0],
            10.0,
            0.0,
            85.0 / 512.0,
            DEFAULT_ROAD_SCALE,
            |_, _| 2.0,
        )
        .expect("road");
        let (verts, indices) = baked;
        assert!(!verts.is_empty());
        assert_eq!(indices.len() % 3, 0);
        for v in &verts {
            assert!((v.z - (2.0 + ROAD_FLOAT_AMOUNT)).abs() < 1.0e-5);
        }
        // uCount = 40/10+1 = 5; flat terrain deletes interior columns.
        assert!(verts.len() < 5 * 2);
        assert!(verts.len() >= 4);
        assert!(indices.len() >= 6);
    }

    #[test]
    fn load_float_4pt_section_keeps_columns_when_height_error_exceeds_max() {
        let baked = bake_straight_road_segment(
            [0.0, 0.0],
            [40.0, 0.0],
            10.0,
            0.0,
            85.0 / 512.0,
            DEFAULT_ROAD_SCALE,
            |x, _| {
                // Non-linear bump: interpolated neighbors cannot reproduce the peak
                // within C++ MAX_ERROR, so the column is retained.
                if (x - 20.0).abs() < 6.0 { 8.0 } else { 0.0 }
            },
        )
        .expect("road");
        let (verts, indices) = baked;
        let flat = bake_straight_road_segment(
            [0.0, 0.0],
            [40.0, 0.0],
            10.0,
            0.0,
            85.0 / 512.0,
            DEFAULT_ROAD_SCALE,
            |_, _| 0.0,
        )
        .expect("flat");
        assert!(verts.len() > flat.0.len());
        assert!(indices.len() > flat.1.len());
        assert!(
            verts
                .iter()
                .any(|v| (v.z - (8.0 + ROAD_FLOAT_AMOUNT)).abs() < 1.0e-4)
        );
        assert!(
            verts
                .iter()
                .any(|v| (v.z - ROAD_FLOAT_AMOUNT).abs() < 1.0e-4)
        );
    }

    #[test]
    fn load_float_4pt_section_uv_matches_cpp_dot_product_formula() {
        let (verts, _) = load_float_4pt_section(
            [0.0, 0.0],
            [0.0, 5.0],
            [20.0, 0.0],
            [[0.0, -5.0], [20.0, -5.0], [0.0, 5.0], [20.0, 5.0]],
            0.0,
            85.0 / 512.0,
            8.0,
            8.0,
            |_, _| 0.0,
        );
        assert!(verts.len() >= 2);
        // At loc, bottom-left offset is (0,-5): U=0, V=-5 after normalize N=(0,1)
        // v1 = vOffset - V/(vScale*4)
        let expected_v = 85.0 / 512.0 - (-5.0) / (8.0 * 4.0);
        assert!((verts[0].v1 - expected_v).abs() < 1.0e-4);
        assert!(verts[0].u1.abs() < 1.0e-4);
    }

    #[test]
    fn load_float_4pt_section_bakes_cpp_road_point_lights() {
        use crate::fx_list::{
            DisplayLightPulse, clear_scene_dynamic_lights, create_display_light_pulse,
            drain_display_light_pulses, scene_dynamic_lights,
        };

        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
        assert!(create_display_light_pulse(DisplayLightPulse {
            pos: [10.0, 0.0, 0.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 10.0,
            outer_radius: 40.0,
            increase_frames: 0,
            decay_frames: 0,
        }));

        let (verts, _) = load_float_4pt_section(
            [0.0, 0.0],
            [0.0, 5.0],
            [20.0, 0.0],
            [[0.0, -5.0], [20.0, -5.0], [0.0, 5.0], [20.0, 5.0]],
            0.0,
            85.0 / 512.0,
            8.0,
            8.0,
            |_, _| 0.0,
        );
        assert!(!verts.is_empty());
        let lights = scene_dynamic_lights();
        let v = verts[0];
        let expected = do_road_dynamic_light([v.x, v.y, v.z], 0xFFFF_FFFF, &lights);
        assert_eq!(v.diffuse, expected);
        assert_eq!(v.diffuse >> 24, 0xFF);
        // Gray static (C++ getStaticDiffuse) so additive POINT light is visible.
        let mut relit = verts;
        for vertex in &mut relit {
            vertex.diffuse = 0xFF33_3333;
        }
        light_road_seg_vertices(&mut relit, 0xFF33_3333);
        let expected_gray =
            do_road_dynamic_light([relit[0].x, relit[0].y, relit[0].z], 0xFF33_3333, &lights);
        assert_eq!(relit[0].diffuse, expected_gray);
        let r = (relit[0].diffuse >> 16) & 0xFF;
        let g = (relit[0].diffuse >> 8) & 0xFF;
        assert!(
            r > g,
            "red pulse must raise R vs G, packed={:#010x}",
            relit[0].diffuse
        );
        clear_scene_dynamic_lights();
    }

    #[test]
    fn do_road_dynamic_light_uses_half_shade_and_xy_aabb_like_cpp() {
        use crate::fx_list::{DisplayDynamicLight, far_atten_factor};

        let light = DisplayDynamicLight {
            pos: [0.0, 0.0, 0.0],
            color: [1.0, 0.0, 0.0],
            far_atten_start: 10.0,
            far_atten_end: 40.0,
            increase_frames: 0,
            decay_frames: 0,
            cur_increase_frames: 0,
            cur_decay_frames: 0,
            target_color: [1.0, 0.0, 0.0],
            target_far_atten_end: 40.0,
            decay_range: true,
            decay_color: true,
            far_attenuation: true,
            enabled: true,
        };
        let factor = far_atten_factor(20.0, 10.0, 40.0).unwrap();
        let shade = 0.5 * factor;
        let expected_r = ((0.0 + shade + factor).clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let lit = do_road_dynamic_light([0.0, 0.0, 20.0], 0xFF00_0000, &[light.clone()]);
        assert_eq!((lit >> 16) & 0xFF, expected_r);
        assert_eq!(lit & 0xFF, 0);

        let mut far = light.clone();
        far.pos = [1000.0, 0.0, 0.0];
        let missed = do_road_dynamic_light([0.0, 0.0, 0.0], 0xFF00_0000, &[far]);
        assert_eq!(
            missed, 0xFF00_0000,
            "XY AABB must skip distant POINT lights"
        );
    }

    #[test]
    fn road_gpu_upload_keeps_do_road_dynamic_light_diffuse() {
        use crate::fx_list::{
            DisplayLightPulse, clear_scene_dynamic_lights, create_display_light_pulse,
            drain_display_light_pulses, scene_dynamic_lights,
        };

        let _ = drain_display_light_pulses();
        clear_scene_dynamic_lights();
        assert!(create_display_light_pulse(DisplayLightPulse {
            pos: [10.0, 0.0, 0.0],
            color: [1.0, 0.0, 0.0],
            inner_radius: 10.0,
            outer_radius: 40.0,
            increase_frames: 0,
            decay_frames: 0,
        }));

        let (verts, indices) = load_float_4pt_section(
            [0.0, 0.0],
            [0.0, 5.0],
            [20.0, 0.0],
            [[0.0, -5.0], [20.0, -5.0], [0.0, 5.0], [20.0, 5.0]],
            0.0,
            85.0 / 512.0,
            8.0,
            8.0,
            |_, _| 0.0,
        );
        assert!(!verts.is_empty());
        assert!(!indices.is_empty());
        let lights = scene_dynamic_lights();
        let gpu = fill_road_gpu_upload_vertices(&verts);
        assert_eq!(gpu.len(), verts.len());
        for (cpu, uploaded) in verts.iter().zip(gpu.iter()) {
            let expected = do_road_dynamic_light([cpu.x, cpu.y, cpu.z], 0xFFFF_FFFF, &lights);
            assert_eq!(cpu.diffuse, expected);
            assert_eq!(
                uploaded.diffuse, expected,
                "wgpu VB must keep C++ packed diffuse"
            );
            assert_eq!(uploaded.position, cpp_map_to_y_up(cpu.x, cpu.y, cpu.z));
            assert_eq!(uploaded.color, unpack_bgra_rgb(expected));
            assert_eq!(uploaded.tex_coords, [cpu.u1, cpu.v1]);
        }
        clear_scene_dynamic_lights();
    }

    #[test]
    fn cpp_bridge_constants_match_w3d_bridge_buffer_h() {
        assert_eq!(MAX_BRIDGE_VERTEX, 12_000);
        assert_eq!(MAX_BRIDGE_INDEX, 24_000);
        assert_eq!(MAX_BRIDGES, 200);
        assert_eq!(BRIDGE_FLOAT_AMT, 0.25);
    }

    #[test]
    fn bake_sectional_bridge_matches_cpp_span_vertex_counts() {
        let (left, section, right) = default_sectional_bridge_model(1.0);
        let buffers = bake_bridge_span(
            [0.0, 0.0, BRIDGE_FLOAT_AMT],
            [120.0, 0.0, BRIDGE_FLOAT_AMT],
            true,
            left,
            Some(section),
            Some(right),
            0x0012_3456,
        );
        // left + 1 section + right = 3 quads → 12 verts / 18 indices for this length?
        // desired_length=120, length=80, span_length=40, spannable=120-(80-40)=80
        // num_spans = floor((80+20)/40)=2 → left + 2 sections + right = 16 verts / 24 indices
        assert_eq!(buffers.vertices.len(), 16);
        assert_eq!(buffers.indices.len(), 24);
        assert_eq!(&buffers.indices[..6], &[0, 1, 2, 0, 2, 3]);
        assert!((buffers.vertices[0].x - 0.0).abs() < 1.0e-4);
        assert!((buffers.vertices[0].y - (-5.0)).abs() < 1.0e-4);
        assert!((buffers.vertices[0].z - BRIDGE_FLOAT_AMT).abs() < 1.0e-4);
        assert_eq!(buffers.vertices[0].diffuse, 0xff12_3456);
        assert_eq!(buffers.vertices[0].nz, 1.0);
    }

    #[test]
    fn bake_fixed_bridge_uses_left_mesh_only() {
        let (left, _, _) = default_sectional_bridge_model(1.0);
        let buffers = bake_bridge_span(
            [0.0, 0.0, 0.0],
            [80.0, 0.0, 0.0],
            false,
            left,
            None,
            None,
            0,
        );
        assert_eq!(buffers.vertices.len(), 4);
        assert_eq!(buffers.indices.len(), 6);
    }

    #[test]
    fn cpp_map_to_y_up_swizzles_height_onto_wgpu_y() {
        assert_eq!(cpp_map_to_y_up(10.0, 20.0, 3.5), [10.0, 3.5, 20.0]);
    }
}
