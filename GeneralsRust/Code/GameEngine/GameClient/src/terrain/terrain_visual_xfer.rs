//! Live `CHUNK_TerrainVisual` payload.
//!
//! C++ `W3DTerrainVisual::xfer` v3 (`W3DTerrainVisual.cpp:1174-1274`) writes
//! water-grid enable + WaterRenderObj snapshot, logic height-map bytes (v2+),
//! then `xferSnapshot(m_terrainRenderObject)`. Live host also persists the
//! `BaseHeightMapRenderObjClass` scorch overlay so napalm / PUC / map scorches
//! survive save/load.

use crate::terrain::scorch_mesh::{
    ScorchMark, add_terrain_scorch, clear_terrain_scorches, terrain_scorch_marks,
};
use crate::terrain::terrain_visual::get_terrain_visual;
use crate::terrain::tree_buffer::{MAX_TREES, TreeRandom, TreeSaveRecord, W3DToppleState};
use game_engine::common::system::xfer::{Xfer as CommonXfer, XferMode as CommonXferMode};
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::io::{self, Cursor};

const W3D_TERRAIN_VISUAL_VERSION: u8 = 3;
const TERRAIN_VISUAL_BASE_VERSION: u8 = 1;
const BASE_HEIGHT_MAP_VERSION: u8 = 1;
const TREE_BUFFER_VERSION: u8 = 1;
const PROP_BUFFER_VERSION: u8 = 1;
const WATER_RENDER_OBJ_VERSION: u8 = 1;

/// Fully parsed logical values and optional native extension. Never published
/// while decoding or admitting a detached candidate.
#[derive(Debug, Clone)]
pub struct PreparedTerrainVisual {
    grid: gamelogic::terrain_water::WaterGridState,
    heights: Vec<u8>,
    height_present: bool,
    tree_records: Option<Vec<TreeSaveRecord>>,
    scorches: Option<Vec<ScorchMark>>,
}
impl PreparedTerrainVisual {
    pub fn admit_logical(
        &self,
        terrain: &mut gamelogic::terrain::TerrainLogic,
    ) -> Result<(), String> {
        if self.height_present {
            terrain.try_apply_logic_height_map_bytes_with_effects(&self.heights, &mut || {})?;
        }
        terrain.restore_water_grid_state(self.grid.clone());
        Ok(())
    }
    pub fn grid(&self) -> &gamelogic::terrain_water::WaterGridState {
        &self.grid
    }
    pub fn heights(&self) -> &[u8] {
        &self.heights
    }
    /// The original v3 TREE slot, in record order, including deleted entries.
    pub fn tree_records(&self) -> Option<&[TreeSaveRecord]> {
        self.tree_records.as_deref()
    }
    pub fn scorches(&self) -> Option<&[ScorchMark]> {
        self.scorches.as_deref()
    }

    /// Apply only to the receiving native owner after logical admission commits.
    pub fn apply_native(
        &self,
        visual: &mut crate::terrain::terrain_visual::TerrainVisualImpl,
        tree_random: &mut impl TreeRandom,
    ) {
        visual.admit_water_state(&self.grid);
        if self.height_present {
            visual.apply_logic_height_map_bytes(&self.heights);
        }
        if let Some(records) = self.tree_records() {
            visual.tree_buffer_mut().load_records_with_rng(records, tree_random);
        }
        if let Some(marks) = &self.scorches {
            visual.clear_scorches();
            for mark in marks {
                visual.add_scorch(mark.location, mark.radius, mark.scorch_type);
            }
        }
    }

    /// Transitional native adapter, used ONLY after the candidate commits.
    /// x3l7v remains open until the renderer owns these water/scorch services.
    pub fn commit_legacy_native(&self) {
        self.grid.publish_visual();
        if self.height_present {
            // Existing native invalidation remains explicit and post-commit.
            // Geometry itself comes from the receiving immutable height frame.
            if let Ok(mut visual) = get_terrain_visual() {
                if let Some(visual) = visual.as_mut() {
                    visual.static_lighting_changed();
                }
            }
        }
        if let Some(marks) = &self.scorches {
            clear_terrain_scorches();
            for mark in marks {
                add_terrain_scorch(mark.location, mark.radius, mark.scorch_type);
            }
        }
    }
}


/// Typed terrain-visual continuation stored by the versioned save archive.
/// Height bytes are the logical height-map grid itself, not an opaque nested
/// transfer stream. Water cells are sparse because zero-valued wire cells are
/// discarded by the original loader.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TerrainVisualArchive {
    pub water_enabled: bool,
    pub water_cells: Vec<TerrainWaterCellArchive>,
    pub heights: Vec<u8>,
    pub tree_records: Vec<TreeSaveRecord>,
    /// None means the writer made no scorch observation; Some(empty) is an
    /// explicit clear. Preserve that distinction from the optional tail.
    pub scorches: Option<Vec<ScorchMark>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TerrainWaterCellArchive {
    pub x: i32,
    pub y: i32,
    pub height_delta: f32,
    pub velocity: f32,
    pub status: u8,
    pub preferred_height: u8,
}

fn canonical_tree_record(record: &TreeSaveRecord) -> TreeSaveRecord {
    let mut record = record.clone();
    // The original matrix xfer stores the first three affine rows. Row three
    // remains the identity row on load, so do not preserve presentation-only
    // deviation that the production format never continued.
    let mut cols = record.matrix.to_cols_array();
    cols[3] = 0.0;
    cols[7] = 0.0;
    cols[11] = 0.0;
    cols[15] = 1.0;
    record.matrix = Mat4::from_cols_array(&cols);
    record
}

/// Capture the same semantic domain as the original v3 terrain-visual xfer,
/// without first serializing it into a positional byte stream.
pub fn capture_typed_terrain_visual_with_tree_records(
    terrain: &gamelogic::terrain::TerrainLogic,
    native_water: Option<&gamelogic::terrain_water::WaterGridState>,
    scorches: Option<&[ScorchMark]>,
    tree_records: Vec<TreeSaveRecord>,
) -> Result<TerrainVisualArchive, String> {
    terrain.validate_height_map()?;
    let grid = native_water
        .unwrap_or_else(|| terrain.water_grid_state())
        .clone();
    let mut water_cells = Vec::new();
    if grid.enabled {
        let cells_x = grid.resolution.0 as i32;
        let cells_y = grid.resolution.1 as i32;
        let mesh_size = (cells_x as i64 + 1 + 2)
            .saturating_mul(cells_y as i64 + 1 + 2)
            .max(0) as usize;
        let stride = cells_x as usize + 3;
        for index in 0..mesh_size {
            let x = (index % stride) as i32 - 1;
            let y = (index / stride) as i32 - 1;
            let key = (x, y);
            let height_delta = grid.height_deltas.get(&key).copied().unwrap_or(0.0);
            let motion = grid.mesh_motion.get(&key).copied().unwrap_or_default();
            if height_delta.to_bits() != 0
                || motion.velocity.to_bits() != 0
                || motion.status != 0
                || motion.preferred_height != 0
            {
                water_cells.push(TerrainWaterCellArchive {
                    x,
                    y,
                    height_delta,
                    velocity: motion.velocity,
                    status: motion.status,
                    preferred_height: motion.preferred_height,
                });
            }
        }
    }
    Ok(TerrainVisualArchive {
        water_enabled: grid.enabled,
        water_cells,
        heights: terrain.logic_height_map_bytes().to_vec(),
        tree_records: tree_records.iter().map(canonical_tree_record).collect(),
        scorches: scorches.map(|marks| marks.to_vec()),
    })
}

fn canonical_matrix_row(matrix: Mat4) -> [f32; 4] {
    let cols = matrix.to_cols_array();
    [cols[3], cols[7], cols[11], cols[15]]
}

/// Prepare typed archive data against the receiving map without touching live
/// logical or native terrain state.
pub fn prepare_typed_terrain_visual_for_terrain(
    terrain: &gamelogic::terrain::TerrainLogic,
    archive: &TerrainVisualArchive,
) -> Result<Option<PreparedTerrainVisual>, String> {
    terrain.validate_height_map()?;
    let mut grid = terrain.water_grid_state().clone();
    let receiving_water_enabled = grid.enabled;
    if archive.water_enabled != receiving_water_enabled {
        return Err("water-grid enabled flag differs from receiving map".into());
    }
    if archive.water_enabled {
        let cells_x = grid.resolution.0 as i32;
        let cells_y = grid.resolution.1 as i32;
        grid.height_deltas.clear();
        grid.mesh_motion.clear();
        let mut keys = std::collections::HashSet::with_capacity(archive.water_cells.len());
        for cell in &archive.water_cells {
            if cell.x < -1
                || cell.y < -1
                || cell.x > cells_x + 1
                || cell.y > cells_y + 1
                || !keys.insert((cell.x, cell.y))
            {
                return Err("invalid or duplicate water-grid cell in terrain visual archive".into());
            }
            let key = (cell.x, cell.y);
            if cell.height_delta.to_bits() != 0 {
                grid.height_deltas.insert(key, cell.height_delta);
            }
            if cell.velocity.to_bits() != 0 || cell.status != 0 || cell.preferred_height != 0 {
                grid.mesh_motion.insert(
                    key,
                    gamelogic::terrain_water::WaterGridMeshMotion {
                        velocity: cell.velocity,
                        status: cell.status,
                        preferred_height: cell.preferred_height,
                    },
                );
            }
        }
    } else if !archive.water_cells.is_empty() {
        return Err("disabled water grid cannot carry water cells".into());
    }
    grid.enabled = archive.water_enabled;

    if archive.tree_records.len() > MAX_TREES {
        return Err(format!(
            "TREE count {} exceeds 0..={MAX_TREES}",
            archive.tree_records.len()
        ));
    }
    if archive
        .tree_records
        .iter()
        .any(|record| canonical_matrix_row(record.matrix) != [0.0, 0.0, 0.0, 1.0])
    {
        return Err("tree matrix contains a non-affine bottom row".into());
    }
    if terrain.logic_height_map_bytes().is_empty() && !archive.heights.is_empty() {
        return Err("terrain visual heights require a loaded receiving map".into());
    }
    Ok(Some(PreparedTerrainVisual {
        grid,
        heights: archive.heights.clone(),
        height_present: true,
        tree_records: Some(archive.tree_records.clone()),
        scorches: archive.scorches.clone(),
    }))
}

/// The caller supplies its actual receiving terrain and native observation.
/// None omits the historical optional scorch tail; it never invents a clear.
pub fn capture_owned_terrain_visual_for_terrain(
    terrain: &gamelogic::terrain::TerrainLogic,
    scorches: Option<&[ScorchMark]>,
) -> Result<Vec<u8>, String> {
    capture_terrain_visual_with_water(terrain, None, scorches)
}

/// Native saves supply the actual water-render observation. Logical-only
/// archives retain their existing Core water owner and identical wire codec.
pub fn capture_terrain_visual_with_water(
    terrain: &gamelogic::terrain::TerrainLogic,
    native_water: Option<&gamelogic::terrain_water::WaterGridState>,
    scorches: Option<&[ScorchMark]>,
) -> Result<Vec<u8>, String> {
    capture_terrain_visual_with_tree_records(terrain, native_water, scorches, Vec::new())
}

/// Capture the ordered records from the same native terrain as the other
/// visual observations. An empty list writes the original zero-record slot.
pub fn capture_terrain_visual_with_tree_records(
    terrain: &gamelogic::terrain::TerrainLogic,
    native_water: Option<&gamelogic::terrain_water::WaterGridState>,
    scorches: Option<&[ScorchMark]>,
    mut tree_records: Vec<TreeSaveRecord>,
) -> Result<Vec<u8>, String> {
    terrain.validate_height_map()?;
    let mut bytes = Vec::new();
    let mut grid = native_water
        .unwrap_or_else(|| terrain.water_grid_state())
        .clone();
    let mut heights = terrain.logic_height_map_bytes().to_vec();
    let mut xfer = CommonXferSave::new(Cursor::new(&mut bytes), 3);
    xfer_live_terrain_visual(&mut xfer, &mut grid, &mut heights, &mut tree_records, None)?;
    if let Some(marks) = scorches {
        let mut count =
            i32::try_from(marks.len()).map_err(|_| "scorch count exceeds source integer domain")?;
        xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
        for mut mark in marks.iter().copied() {
            xfer_scorch_mark(&mut xfer, &mut mark)?;
        }
    }
    drop(xfer);
    Ok(bytes)
}

/// Complete, global-free parsing against the receiving map's water layout.
pub fn prepare_terrain_visual_for_terrain(
    terrain: &gamelogic::terrain::TerrainLogic,
    bytes: &[u8],
) -> Result<Option<PreparedTerrainVisual>, String> {
    if bytes.is_empty() || bytes == [1] {
        return Ok(None);
    }
    terrain.validate_height_map()?;
    let mut grid = terrain.water_grid_state().clone();
    let mut heights = terrain.logic_height_map_bytes().to_vec();
    let mut cursor = Cursor::new(bytes);
    let mut tree_records = Vec::new();
    let version = {
        let mut xfer = CommonXferLoad::new(&mut cursor, 3);
        xfer_live_terrain_visual(
            &mut xfer,
            &mut grid,
            &mut heights,
            &mut tree_records,
            Some(bytes.len()),
        )?
    };
    let remaining = bytes.len() - cursor.position() as usize;
    let scorches = if remaining == 0 {
        None
    } else {
        if remaining < 4 {
            return Err("truncated optional scorch count".into());
        }
        let mut xfer = CommonXferLoad::new(&mut cursor, 3);
        let mut count = 0;
        xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
        if count < 0 || count as usize > (remaining - 4) / 20 {
            return Err("scorch count exceeds complete payload".into());
        }
        let mut marks = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let mut mark = ScorchMark {
                location: [0.; 3],
                radius: 0.,
                scorch_type: 0,
            };
            xfer_scorch_mark(&mut xfer, &mut mark)?;
            marks.push(mark);
        }
        Some(marks)
    };
    if cursor.position() != bytes.len() as u64 {
        return Err("trailing terrain visual payload".into());
    }
    // Prefix Xfer admission remains valid, but an unloaded map cannot accept
    // orphan bytes. Checking this before returning keeps admission atomic.
    if terrain.logic_height_map_bytes().is_empty() && !heights.is_empty() {
        return Err("terrain visual heights require a loaded receiving map".into());
    }
    Ok(Some(PreparedTerrainVisual {
        grid,
        heights,
        height_present: version >= 2,
        tree_records: (version >= 3).then_some(tree_records),
        scorches,
    }))
}

// Standalone compatibility adapters. SaveFileManager never selects these owners.
pub fn capture_live_terrain_visual_xfer_bytes() -> Result<Vec<u8>, String> {
    let handle = gamelogic::terrain::get_terrain_logic();
    let terrain = handle.read().map_err(|_| "terrain lock poisoned")?;
    capture_live_terrain_visual_for_terrain(&terrain)
}
pub fn capture_live_terrain_visual_for_terrain(
    terrain: &gamelogic::terrain::TerrainLogic,
) -> Result<Vec<u8>, String> {
    capture_owned_terrain_visual_for_terrain(terrain, Some(&terrain_scorch_marks()))
}
pub fn restore_live_terrain_visual_from_xfer_bytes(bytes: &[u8]) -> Result<(), String> {
    let handle = gamelogic::terrain::get_terrain_logic();
    let mut terrain = handle.write().map_err(|_| "terrain lock poisoned")?;
    restore_live_terrain_visual_for_terrain(&mut terrain, bytes)
}
pub fn restore_live_terrain_visual_for_terrain(
    terrain: &mut gamelogic::terrain::TerrainLogic,
    bytes: &[u8],
) -> Result<(), String> {
    if let Some(prepared) = prepare_terrain_visual_for_terrain(terrain, bytes)? {
        prepared.admit_logical(terrain)?;
        prepared.commit_legacy_native();
    }
    Ok(())
}

fn xfer_water_grid_snapshot(
    xfer: &mut dyn CommonXfer,
    grid: &mut gamelogic::terrain_water::WaterGridState,
) -> Result<(), String> {
    let mut version = WATER_RENDER_OBJ_VERSION;
    xfer.xfer_version(&mut version, WATER_RENDER_OBJ_VERSION)
        .map_err(|e| e.to_string())?;
    let expected_x = grid.resolution.0 as i32;
    let expected_y = grid.resolution.1 as i32;
    let mut cells_x = expected_x;
    let mut cells_y = expected_y;
    xfer.xfer_int(&mut cells_x).map_err(|e| e.to_string())?;
    xfer.xfer_int(&mut cells_y).map_err(|e| e.to_string())?;
    if cells_x < 0 || cells_y < 0 || cells_x != expected_x || cells_y != expected_y {
        return Err("water-grid dimensions differ from loaded map".into());
    }
    let mesh_size = (cells_x as i64 + 1 + 2)
        .saturating_mul(cells_y as i64 + 1 + 2)
        .max(0) as usize;
    if !xfer.is_writing() {
        grid.height_deltas.clear();
        grid.mesh_motion.clear();
    }
    let stride = cells_x as usize + 3;
    for index in 0..mesh_size {
        // Original invisible border: mesh[(y+1)*(cellsX+3)+x+1].
        let key = ((index % stride) as i32 - 1, (index / stride) as i32 - 1);
        let motion = grid.mesh_motion.get(&key).copied().unwrap_or_default();
        let mut height = grid.height_deltas.get(&key).copied().unwrap_or(0.0);
        let mut velocity = motion.velocity;
        let mut status = motion.status;
        let mut preferred = motion.preferred_height;
        xfer.xfer_real(&mut height).map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut velocity).map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_byte(&mut status)
            .map_err(|e| e.to_string())?;
        xfer.xfer_unsigned_byte(&mut preferred)
            .map_err(|e| e.to_string())?;
        if !xfer.is_writing() {
            if height.to_bits() != 0 {
                grid.height_deltas.insert(key, height);
            }
            if velocity.to_bits() != 0 || status != 0 || preferred != 0 {
                grid.mesh_motion.insert(
                    key,
                    gamelogic::terrain_water::WaterGridMeshMotion {
                        velocity,
                        status,
                        preferred_height: preferred,
                    },
                );
            }
        }
    }
    Ok(())
}

fn xfer_live_terrain_visual(
    xfer: &mut dyn CommonXfer,
    grid: &mut gamelogic::terrain_water::WaterGridState,
    height_data: &mut Vec<u8>,
    tree_records: &mut Vec<TreeSaveRecord>,
    payload_limit: Option<usize>,
) -> Result<u8, String> {
    let mut version = W3D_TERRAIN_VISUAL_VERSION;
    xfer.xfer_version(&mut version, W3D_TERRAIN_VISUAL_VERSION)
        .map_err(|e| e.to_string())?;
    let mut base_version = TERRAIN_VISUAL_BASE_VERSION;
    xfer.xfer_version(&mut base_version, TERRAIN_VISUAL_BASE_VERSION)
        .map_err(|e| e.to_string())?;

    let receiving_water_enabled = grid.enabled;
    let mut water_enabled = receiving_water_enabled;
    xfer.xfer_bool(&mut water_enabled)
        .map_err(|e| e.to_string())?;
    if !xfer.is_writing() && water_enabled != receiving_water_enabled {
        return Err("water-grid enabled flag differs from receiving map".into());
    }
    if water_enabled {
        xfer_water_grid_snapshot(xfer, grid)?;
    }
    grid.enabled = water_enabled;

    if version >= 2 {
        let mut height_map_len = height_data.len() as i32;
        xfer.xfer_int(&mut height_map_len)
            .map_err(|e| e.to_string())?;
        if height_map_len < 0 || payload_limit.is_some_and(|limit| height_map_len as usize > limit)
        {
            return Err("height-map length exceeds complete payload".into());
        }
        if xfer.get_xfer_mode() == game_engine::common::system::xfer::XferMode::Load {
            *height_data = vec![0u8; height_map_len as usize];
        } else if height_data.len() != height_map_len as usize {
            height_data.resize(height_map_len.max(0) as usize, 0);
        }
        if height_map_len > 0 {
            xfer.xfer_user(height_data)
                .map_err(|e| e.to_string())?;
        }
    }

    if version >= 3 {
        let mut base_height_version = BASE_HEIGHT_MAP_VERSION;
        xfer.xfer_version(&mut base_height_version, BASE_HEIGHT_MAP_VERSION)
            .map_err(|e| e.to_string())?;
        xfer_cpp_tree_buffer(xfer, tree_records).map_err(|err| err.to_string())?;
        let mut prop_version = PROP_BUFFER_VERSION;
        xfer.xfer_version(&mut prop_version, PROP_BUFFER_VERSION)
            .map_err(|e| e.to_string())?;
    }

    Ok(version)
}

/// C++ W3DTreeBuffer::xfer v1. Read into a candidate until every record is
/// complete. Callers own whether and when any records are admitted.
pub(crate) fn xfer_cpp_tree_buffer(
    xfer: &mut dyn CommonXfer,
    records: &mut Vec<TreeSaveRecord>,
) -> io::Result<()> {
    let mut version = TREE_BUFFER_VERSION;
    xfer.xfer_version(&mut version, TREE_BUFFER_VERSION)?;
    let mut count = i32::try_from(records.len())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "TREE count exceeds source i32"))?;
    xfer.xfer_int(&mut count)?;
    if !(0..=MAX_TREES as i32).contains(&count) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("TREE count {count} exceeds 0..={MAX_TREES}"),
        ));
    }

    if xfer.get_xfer_mode() != CommonXferMode::Load {
        for record in records {
            xfer_cpp_tree_record(xfer, record)?;
        }
    } else {
        let mut candidate = Vec::with_capacity(count as usize);
        for _ in 0..count {
            let mut record = TreeSaveRecord {
                model_name: String::new(),
                model_texture: String::new(),
                location: Vec3::ZERO,
                scale: 0.,
                sin: 0.,
                cos: 0.,
                drawable_id: 0,
                angular_velocity: 0.,
                angular_acceleration: 0.,
                topple_direction: Vec3::ZERO,
                topple_state: W3DToppleState::Upright,
                angular_accumulation: 0.,
                options: 0,
                matrix: Mat4::IDENTITY,
                sink_frames_left: 0,
            };
            xfer_cpp_tree_record(xfer, &mut record)?;
            candidate.push(record);
        }
        *records = candidate;
    }
    Ok(())
}

fn xfer_cpp_tree_record(xfer: &mut dyn CommonXfer, record: &mut TreeSaveRecord) -> io::Result<()> {
    // W3DTreeBuffer.cpp:1953–1984. Empty model and texture are a tombstone
    // with the same remaining fields as every other record.
    xfer.xfer_ascii_string(&mut record.model_name)?;
    xfer.xfer_ascii_string(&mut record.model_texture)?;
    xfer.xfer_real(&mut record.location.x)?;
    xfer.xfer_real(&mut record.location.y)?;
    xfer.xfer_real(&mut record.location.z)?;
    xfer.xfer_real(&mut record.scale)?;
    xfer.xfer_real(&mut record.sin)?;
    xfer.xfer_real(&mut record.cos)?;
    xfer.xfer_drawable_id(&mut record.drawable_id)?;
    xfer.xfer_real(&mut record.angular_velocity)?;
    xfer.xfer_real(&mut record.angular_acceleration)?;
    xfer.xfer_real(&mut record.topple_direction.x)?;
    xfer.xfer_real(&mut record.topple_direction.y)?;
    xfer.xfer_real(&mut record.topple_direction.z)?;
    let mut state = (record.topple_state as i32).to_ne_bytes();
    xfer.xfer_user(&mut state)?;
    if xfer.get_xfer_mode() == CommonXferMode::Load {
        record.topple_state = match i32::from_ne_bytes(state) {
            0 => W3DToppleState::Upright,
            1 => W3DToppleState::Falling,
            2 => W3DToppleState::Fogged,
            3 => W3DToppleState::Shrouded,
            4 => W3DToppleState::Down,
            _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid TREE topple state")),
        };
    }
    xfer.xfer_real(&mut record.angular_accumulation)?;
    xfer.xfer_unsigned_int(&mut record.options)?;
    // Xfer::xferMatrix3D version 1 serializes three rows of four floats.
    let mut matrix_version = 1;
    xfer.xfer_version(&mut matrix_version, 1)?;
    let mut cols = record.matrix.to_cols_array();
    for row in 0..3 {
        for col in 0..4 {
            xfer.xfer_real(&mut cols[col * 4 + row])?;
        }
    }
    if xfer.get_xfer_mode() == CommonXferMode::Load {
        record.matrix = Mat4::from_cols_array(&cols);
    }
    xfer.xfer_unsigned_int(&mut record.sink_frames_left)
}

fn xfer_scorch_mark(xfer: &mut dyn CommonXfer, mark: &mut ScorchMark) -> Result<(), String> {
    xfer.xfer_real(&mut mark.location[0])
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut mark.location[1])
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut mark.location[2])
        .map_err(|e| e.to_string())?;
    xfer.xfer_real(&mut mark.radius)
        .map_err(|e| e.to_string())?;
    xfer.xfer_int(&mut mark.scorch_type)
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_terrain_visual_xfer_keeps_scorches() {
        crate::test_support::ensure_ambient_host();
        clear_terrain_scorches();
        assert!(add_terrain_scorch([120.0, 40.0, 6.0], 18.0, 2));
        assert_eq!(terrain_scorch_marks().len(), 1);

        let bytes = capture_live_terrain_visual_xfer_bytes().expect("capture terrain visual");
        assert!(
            bytes.len() > 1,
            "CHUNK_TerrainVisual must not be NullSnapshot v1"
        );
        assert_eq!(bytes[0], W3D_TERRAIN_VISUAL_VERSION);

        clear_terrain_scorches();
        assert!(terrain_scorch_marks().is_empty());

        restore_live_terrain_visual_from_xfer_bytes(&bytes).expect("restore terrain visual");
        let marks = terrain_scorch_marks();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].location, [120.0, 40.0, 6.0]);
        assert_eq!(marks[0].radius, 18.0);
        assert_eq!(marks[0].scorch_type, 2);
        clear_terrain_scorches();
    }

    #[test]
    fn live_terrain_visual_xfer_empty_starts_with_cpp_w3d_layout() {
        crate::test_support::ensure_ambient_host();
        clear_terrain_scorches();
        if let Ok(mut visual) = get_terrain_visual() {
            if let Some(visual) = visual.as_mut() {
                visual.enable_water_grid(false);
            }
        }
        if let Ok(mut terrain) = gamelogic::terrain::get_terrain_logic().write() {
            terrain.reset();
        }
        let bytes = capture_live_terrain_visual_xfer_bytes().expect("empty capture");
        assert_eq!(
            &bytes[..13],
            &[
                3, // C++ W3DTerrainVisual xfer version
                1, // base TerrainVisual xfer version
                0, // water grid disabled
                0, 0, 0, 0, // height-map byte count
                1, // BaseHeightMapRenderObjClass xfer version
                1, // W3DTreeBuffer xfer version
                0, 0, 0, 0, // numTrees
            ]
        );
        assert_eq!(bytes[13], 1); // W3DPropBuffer xfer version
        // Rust's existing scorch persistence follows the C++ prefix.
        assert_eq!(&bytes[14..18], &[0, 0, 0, 0]); // scorch count
        assert_eq!(bytes.len(), 18);
    }
}

#[cfg(test)]
#[path = "terrain_visual_xfer_owner_tests.rs"]
mod owner_tests;
