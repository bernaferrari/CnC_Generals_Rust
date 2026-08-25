//! C++-matching terrain scorch buffer + GPU mesh bake.
//!
//! Oracle:
//! - `BaseHeightMapRenderObjClass::addScorch` (BaseHeightMap.cpp:2020)
//! - `BaseHeightMapRenderObjClass::updateScorches` (BaseHeightMap.cpp:1881)
//! - `W3DGameClient::addScorch` → `TheTerrainRenderObject->addScorch`
//!
//! FXList `TerrainScorchFXNugget` and GameLogic `TheGameClient->addScorch`
//! land here instead of a generic timed decal.

use std::sync::{Mutex, OnceLock};

use crate::terrain::textures::FLIPPED_MASK;
use gamelogic::common::types::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};

fn height_map_cell_flip(map: &crate::terrain::height_map::HeightMap, x: i32, y: i32) -> bool {
    if x < 0 || y < 0 {
        return false;
    }
    let idx = (y as u32 as usize)
        .saturating_mul(map.width as usize)
        .saturating_add(x as u32 as usize);
    if let Some(&ndx) = map.cliff_info_ndxes.get(idx) {
        if ndx > 0 {
            if let Some(info) = map.cliff_info.get(ndx as usize) {
                if info.flip {
                    return true;
                }
            }
        }
    }
    let blend_ndx = map.blend_tile_ndxes.get(idx).copied().unwrap_or(0);
    if blend_ndx > 0 {
        if let Some(tile) = map.blended_tiles.get(blend_ndx as usize) {
            return (tile.inverted & FLIPPED_MASK) != 0;
        }
    }
    false
}

/// C++ `MAX_SCORCH_MARKS`.
pub const MAX_SCORCH_MARKS: usize = 500;
/// C++ `SCORCH_MARKS_IN_TEXTURE`.
pub const SCORCH_MARKS_IN_TEXTURE: i32 = 9;
/// C++ `SCORCH_PER_ROW`.
pub const SCORCH_PER_ROW: i32 = 3;
/// C++ `MAX_SCORCH_VERTEX`.
pub const MAX_SCORCH_VERTEX: usize = 8194;
/// C++ `MAX_SCORCH_INDEX`.
pub const MAX_SCORCH_INDEX: usize = 6 * 8194;
/// C++ `amtToFloat = MAP_HEIGHT_SCALE/10`.
pub const SCORCH_FLOAT_AMOUNT: f32 = MAP_HEIGHT_SCALE / 10.0;

/// C++ `TScorch`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScorchMark {
    pub location: [f32; 3],
    pub radius: f32,
    pub scorch_type: i32,
}

/// C++ `VertexFormatXYZDUV1` scorch vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScorchVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub diffuse: u32,
    pub u1: f32,
    pub v1: f32,
}

/// Baked scorch overlay (CPU mirror of `m_vertexScorch` / `m_indexScorch`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ScorchGpuMesh {
    pub vertices: Vec<ScorchVertex>,
    pub indices: Vec<u16>,
}

/// Height + flip queries used by `updateScorches`.
pub trait ScorchHeightSource {
    fn x_extent(&self) -> i32;
    fn y_extent(&self) -> i32;
    fn border_size(&self) -> i32;
    /// C++ `getClipHeight(x,y) * MAP_HEIGHT_SCALE` (clamped indices).
    fn clip_height_world(&self, x: i32, y: i32) -> f32;
    /// C++ `WorldHeightMap::getFlipState`.
    fn flip_state(&self, x: i32, y: i32) -> bool;
}

impl ScorchHeightSource for crate::terrain::height_map::HeightMap {
    fn x_extent(&self) -> i32 {
        self.width as i32
    }

    fn y_extent(&self) -> i32 {
        self.height as i32
    }

    fn border_size(&self) -> i32 {
        self.border_size
    }

    fn clip_height_world(&self, x: i32, y: i32) -> f32 {
        let max_x = self.width.saturating_sub(1);
        let max_y = self.height.saturating_sub(1);
        let cx = x.clamp(0, max_x as i32) as u32;
        let cy = y.clamp(0, max_y as i32) as u32;
        self.world_height_at_index(cx, cy)
    }

    fn flip_state(&self, x: i32, y: i32) -> bool {
        height_map_cell_flip(self, x, y)
    }
}

/// Live scorch list (C++ `m_scorches` + `m_numScorches` + `m_scorchesInBuffer`).
#[derive(Debug, Clone, Default)]
pub struct TerrainScorchBuffer {
    scorches: Vec<ScorchMark>,
    /// C++ `m_scorchesInBuffer`; `0` after add forces a rebuild.
    pub scorches_in_buffer: i32,
}

impl TerrainScorchBuffer {
    pub fn new() -> Self {
        Self {
            scorches: Vec::with_capacity(MAX_SCORCH_MARKS),
            scorches_in_buffer: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.scorches.len()
    }

    pub fn is_empty(&self) -> bool {
        self.scorches.is_empty()
    }

    pub fn marks(&self) -> &[ScorchMark] {
        &self.scorches
    }

    /// C++ `clearAllScorches`.
    pub fn clear(&mut self) {
        self.scorches.clear();
        self.scorches_in_buffer = 0;
    }

    /// C++ `BaseHeightMapRenderObjClass::addScorch`.
    /// Returns `false` when the mark is treated as a duplicate.
    pub fn add_scorch(&mut self, location: [f32; 3], radius: f32, scorch_type: i32) -> bool {
        if self.scorches.len() >= MAX_SCORCH_MARKS {
            self.scorches.remove(0);
        }

        let limit = radius / 4.0;
        for existing in &self.scorches {
            if (existing.location[0] - location[0]).abs() < limit
                && (existing.location[1] - location[1]).abs() < limit
                && (radius - existing.radius).abs() < limit
                && existing.scorch_type == scorch_type
            {
                return false;
            }
        }

        self.scorches.push(ScorchMark {
            location,
            radius,
            scorch_type,
        });
        self.scorches_in_buffer = 0;
        true
    }

    /// C++ `updateScorches` vertex/index bake.
    pub fn update_scorches(
        &mut self,
        height: &dyn ScorchHeightSource,
        diffuse: u32,
    ) -> ScorchGpuMesh {
        if self.scorches.is_empty() {
            return ScorchGpuMesh::default();
        }

        let mut mesh = ScorchGpuMesh::default();
        self.scorches_in_buffer = 0;
        let border = height.border_size();
        let x_extent = height.x_extent();
        let y_extent = height.y_extent();

        for mark in self.scorches.iter().rev() {
            self.scorches_in_buffer += 1;
            let radius = mark.radius;
            let loc = mark.location;
            let mut ty = mark.scorch_type;
            if ty < 0 || ty >= SCORCH_MARKS_IN_TEXTURE {
                ty = 0;
            }

            let mut min_x = ((loc[0] - radius) / MAP_XY_FACTOR).floor() as i32;
            let mut min_y = ((loc[1] - radius) / MAP_XY_FACTOR).floor() as i32;
            if min_x < -border {
                min_x = -border;
            }
            if min_y < -border {
                min_y = -border;
            }
            let mut max_x = ((loc[0] + radius) / MAP_XY_FACTOR).ceil() as i32;
            let mut max_y = ((loc[1] + radius) / MAP_XY_FACTOR).ceil() as i32;
            max_x += 1;
            max_y += 1;
            if max_x > x_extent - border {
                max_x = x_extent - border;
            }
            if max_y > y_extent - border {
                max_y = y_extent - border;
            }

            let start_vertex = mesh.vertices.len();
            for j in min_y..max_y {
                for i in min_x..max_x {
                    if mesh.vertices.len() >= MAX_SCORCH_VERTEX {
                        return mesh;
                    }
                    let the_z =
                        SCORCH_FLOAT_AMOUNT + height.clip_height_world(i + border, j + border);
                    let u_offset = (ty % SCORCH_PER_ROW) as f32 * 1.5;
                    let v_offset = (ty / SCORCH_PER_ROW) as f32 * 1.5;
                    let x = i as f32 * MAP_XY_FACTOR;
                    let y = j as f32 * MAP_XY_FACTOR;
                    let u1 = (u_offset + 0.5 + (x - loc[0]) / (2.0 * radius))
                        / (SCORCH_PER_ROW as f32 + 1.0);
                    let v1 = (v_offset + 0.5 + (y - loc[1]) / (2.0 * radius))
                        / (SCORCH_PER_ROW as f32 + 1.0);
                    mesh.vertices.push(ScorchVertex {
                        x,
                        y,
                        z: the_z,
                        diffuse,
                        u1,
                        v1,
                    });
                }
            }

            let y_offset = max_x - min_x;
            for j in 0..(max_y - min_y - 1) {
                for i in 0..(max_x - min_x - 1) {
                    if mesh.indices.len() + 6 > MAX_SCORCH_INDEX {
                        return mesh;
                    }
                    let x_ndx = i + min_x + border;
                    let y_ndx = j + min_y + border;
                    let flip = height.flip_state(x_ndx, y_ndx);
                    let base = start_vertex as u16;
                    let yo = y_offset as u16;
                    let ii = i as u16;
                    let jj = j as u16;
                    if flip {
                        mesh.indices.push(base + jj * yo + ii + 1);
                        mesh.indices.push(base + jj * yo + ii + yo);
                        mesh.indices.push(base + jj * yo + ii);
                        mesh.indices.push(base + jj * yo + ii + 1);
                        mesh.indices.push(base + jj * yo + ii + 1 + yo);
                        mesh.indices.push(base + jj * yo + ii + yo);
                    } else {
                        mesh.indices.push(base + jj * yo + ii);
                        mesh.indices.push(base + jj * yo + ii + 1 + yo);
                        mesh.indices.push(base + jj * yo + ii + yo);
                        mesh.indices.push(base + jj * yo + ii);
                        mesh.indices.push(base + jj * yo + ii + 1);
                        mesh.indices.push(base + jj * yo + ii + 1 + yo);
                    }
                }
            }
        }

        mesh
    }
}

fn global_scorch_buffer() -> &'static Mutex<TerrainScorchBuffer> {
    static BUFFER: OnceLock<Mutex<TerrainScorchBuffer>> = OnceLock::new();
    BUFFER.get_or_init(|| Mutex::new(TerrainScorchBuffer::new()))
}

/// C++ `GameClientRandomValue(SCORCH_1, SCORCH_4)` when FX type is `RANDOM` / `< 0`.
pub fn resolve_scorch_type(scorch: i32) -> i32 {
    if scorch < 0 {
        use rand::Rng;
        rand::thread_rng().gen_range(0..=3)
    } else {
        scorch
    }
}

/// C++ `TheGameClient->addScorch(pos, radius, type)`.
pub fn add_terrain_scorch(location: [f32; 3], radius: f32, scorch_type: i32) -> bool {
    global_scorch_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .add_scorch(location, radius, scorch_type)
}

pub fn clear_terrain_scorches() {
    global_scorch_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .clear();
}

pub fn terrain_scorch_marks() -> Vec<ScorchMark> {
    global_scorch_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .marks()
        .to_vec()
}

pub fn terrain_scorch_count() -> usize {
    global_scorch_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .len()
}

/// C++ `m_scorchesInBuffer`. `0` after `addScorch` forces `updateScorches`.
pub fn terrain_scorches_in_buffer() -> i32 {
    global_scorch_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .scorches_in_buffer
}

/// Bake the live FX/GameClient scorch buffer against a height source.
pub fn bake_terrain_scorch_gpu_mesh(
    height: &dyn ScorchHeightSource,
    diffuse: u32,
) -> ScorchGpuMesh {
    global_scorch_buffer()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .update_scorches(height, diffuse)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHeight {
        flip: bool,
    }

    impl ScorchHeightSource for MockHeight {
        fn x_extent(&self) -> i32 {
            4
        }
        fn y_extent(&self) -> i32 {
            4
        }
        fn border_size(&self) -> i32 {
            0
        }
        fn clip_height_world(&self, _x: i32, _y: i32) -> f32 {
            0.0
        }
        fn flip_state(&self, _x: i32, _y: i32) -> bool {
            self.flip
        }
    }

    #[test]
    fn flipped_cell_uses_opposite_triangle_winding() {
        let mut buffer = TerrainScorchBuffer::new();
        buffer.add_scorch([10.0, 10.0, 0.0], 20.0, 0);
        let unflipped = buffer.update_scorches(&MockHeight { flip: false }, 0xFFFFFFFF);
        buffer.scorches_in_buffer = 0;
        let flipped = buffer.update_scorches(&MockHeight { flip: true }, 0xFFFFFFFF);
        assert!(!unflipped.indices.is_empty());
        assert_eq!(unflipped.indices.len(), flipped.indices.len());
        assert_ne!(unflipped.indices, flipped.indices);
        assert_eq!(flipped.indices[0], unflipped.indices[1]);
    }

    #[test]
    fn height_map_reads_cliff_and_blend_flip_bits() {
        let mut map = crate::terrain::height_map::HeightMap::new(4, 4, 10.0, 1.0);
        map.cliff_info.push(crate::terrain::height_map::TCliffInfo {
            flip: true,
            ..crate::terrain::height_map::TCliffInfo::default()
        });
        map.cliff_info_ndxes[1] = (map.cliff_info.len() - 1) as i16;
        assert!(height_map_cell_flip(&map, 1, 0));
        assert!(!height_map_cell_flip(&map, 0, 0));

        let mut blend = crate::terrain::textures::BlendTileInfo::new();
        blend.inverted = FLIPPED_MASK;
        map.blended_tiles
            .push(crate::terrain::textures::BlendTileInfo::new());
        map.blended_tiles.push(blend);
        map.blend_tile_ndxes[2] = 1;
        assert!(height_map_cell_flip(&map, 2, 0));
    }
}
