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
use game_engine::common::system::xfer::Xfer as CommonXfer;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use std::io::{self, Cursor, ErrorKind};

const W3D_TERRAIN_VISUAL_VERSION: u8 = 3;
const TERRAIN_VISUAL_BASE_VERSION: u8 = 1;
const BASE_HEIGHT_MAP_VERSION: u8 = 1;
const TREE_BUFFER_VERSION: u8 = 1;
const PROP_BUFFER_VERSION: u8 = 1;
const WATER_RENDER_OBJ_VERSION: u8 = 1;

pub fn capture_live_terrain_visual_xfer_bytes() -> Result<Vec<u8>, String> {
    let handle = gamelogic::terrain::get_terrain_logic();
    let terrain = handle.read().map_err(|_| "terrain lock poisoned")?;
    capture_live_terrain_visual_for_terrain(&terrain)
}

pub fn capture_live_terrain_visual_for_terrain(
    terrain: &gamelogic::terrain::TerrainLogic,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut grid = terrain.water_grid_state().clone();
    let mut heights = terrain.logic_height_map_bytes().to_vec();
    let mut xfer = CommonXferSave::new(Cursor::new(&mut bytes), 3);
    xfer_live_terrain_visual(&mut xfer, &mut grid, &mut heights)?;
    drop(xfer);
    Ok(bytes)
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
    if bytes.is_empty() || (bytes.len() == 1 && bytes[0] == 1) {
        return Ok(());
    }
    // Validate the entire payload on detached logical values. A failed load
    // cannot replace this world's grid, height bytes, or water enable state.
    let mut grid = terrain.water_grid_state().clone();
    let mut heights = terrain.logic_height_map_bytes().to_vec();
    let mut xfer = CommonXferLoad::new(Cursor::new(bytes.to_vec()), 3);
    xfer_live_terrain_visual(&mut xfer, &mut grid, &mut heights)?;
    terrain.apply_logic_height_map_bytes(&heights);
    terrain.restore_water_grid_state(grid);
    terrain.water_grid_state().publish_visual();
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
) -> Result<(), String> {
    let mut version = W3D_TERRAIN_VISUAL_VERSION;
    xfer.xfer_version(&mut version, W3D_TERRAIN_VISUAL_VERSION)
        .map_err(|e| e.to_string())?;
    let mut base_version = TERRAIN_VISUAL_BASE_VERSION;
    xfer.xfer_version(&mut base_version, TERRAIN_VISUAL_BASE_VERSION)
        .map_err(|e| e.to_string())?;

    let mut water_enabled = grid.enabled;
    xfer.xfer_bool(&mut water_enabled)
        .map_err(|e| e.to_string())?;
    if water_enabled {
        xfer_water_grid_snapshot(xfer, grid)?;
    }
    grid.enabled = water_enabled;

    if version >= 2 {
        let mut height_map_len = height_data.len() as i32;
        xfer.xfer_int(&mut height_map_len)
            .map_err(|e| e.to_string())?;
        if height_map_len < 0 {
            return Err("negative height-map length".into());
        }
        if xfer.get_xfer_mode() == game_engine::common::system::xfer::XferMode::Load {
            *height_data = vec![0u8; height_map_len as usize];
        } else if height_data.len() != height_map_len as usize {
            height_data.resize(height_map_len.max(0) as usize, 0);
        }
        if height_map_len > 0 {
            xfer.xfer_user_bytes(height_data)
                .map_err(|e| e.to_string())?;
        }
    }

    if version >= 3 {
        let mut base_height_version = BASE_HEIGHT_MAP_VERSION;
        xfer.xfer_version(&mut base_height_version, BASE_HEIGHT_MAP_VERSION)
            .map_err(|e| e.to_string())?;
        let mut tree_version = TREE_BUFFER_VERSION;
        xfer.xfer_version(&mut tree_version, TREE_BUFFER_VERSION)
            .map_err(|e| e.to_string())?;
        let mut num_trees = 0i32;
        xfer.xfer_int(&mut num_trees).map_err(|e| e.to_string())?;
        if num_trees != 0 {
            return Err("live CHUNK_TerrainVisual writes empty tree buffer".into());
        }
        let mut prop_version = PROP_BUFFER_VERSION;
        xfer.xfer_version(&mut prop_version, PROP_BUFFER_VERSION)
            .map_err(|e| e.to_string())?;
    }

    xfer_live_scorches(xfer)
}

fn xfer_live_scorches(xfer: &mut dyn CommonXfer) -> Result<(), String> {
    if xfer.is_writing() {
        let mut marks = terrain_scorch_marks();
        let mut count = marks.len() as i32;
        xfer.xfer_int(&mut count).map_err(|e| e.to_string())?;
        for mark in &mut marks {
            xfer_scorch_mark(xfer, mark)?;
        }
        return Ok(());
    }

    let mut count = 0i32;
    match xfer.xfer_int(&mut count) {
        Ok(()) => {}
        Err(err) if is_eof(&err) => return Ok(()),
        Err(err) => return Err(err.to_string()),
    }
    if count < 0 {
        return Err("negative scorch count".into());
    }
    let mut marks = vec![
        ScorchMark {
            location: [0.0; 3],
            radius: 0.0,
            scorch_type: 0,
        };
        count as usize
    ];
    for mark in &mut marks {
        xfer_scorch_mark(xfer, mark)?;
    }
    clear_terrain_scorches();
    for mark in marks {
        add_terrain_scorch(mark.location, mark.radius, mark.scorch_type);
    }
    Ok(())
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

fn is_eof(err: &io::Error) -> bool {
    err.kind() == ErrorKind::UnexpectedEof
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_terrain_visual_xfer_keeps_scorches() {
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
