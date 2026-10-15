//! Frozen camera height query: BaseHeightMap.cpp:828-909 and .h:115-131.
use super::world_env::PresentationRuntimeHeightmap;
use crate::game_logic::terrain::triangle_split_height_sample;

/// Original logical map coordinates use ten world units per cell, with the
/// stored border added *after* floor. Playable `world_min`/`world_max` and the
/// render mesh scale are not an alternate origin for this C++ query.
pub(super) fn sample_camera_height(
    map: &PresentationRuntimeHeightmap,
    world_x: f32,
    world_z: f32,
) -> Option<f32> {
    if !map.is_usable()
        || !world_x.is_finite()
        || !world_z.is_finite()
        || !map.max_height.is_finite()
        || map.border_size < 0
        || map.width > i32::MAX as u32
        || map.height > i32::MAX as u32
    {
        return None;
    }
    let x = world_x * 0.1;
    let z = world_z * 0.1;
    let floor_x = x.floor();
    let floor_z = z.floor();
    let ix = (floor_x as i64).saturating_add(i64::from(map.border_size));
    let iz = (floor_z as i64).saturating_add(i64::from(map.border_size));
    let width = i64::from(map.width);
    let height = i64::from(map.height);
    // Original map bytes were normalized during content decoding. Recover the
    // byte before interpolation so subtraction/operand order remains original.
    // Synthetic non-byte grids are quantized; max_height/255 preserves their
    // configured physical scale (retail maps use 0.625 world units per byte).
    let sample = |x: i64, z: i64| -> Option<f32> {
        let index =
            z.clamp(0, height - 1) as usize * map.width as usize + x.clamp(0, width - 1) as usize;
        let value = *map.heights.get(index)?;
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return None;
        }
        Some((value * 255.0).round())
    };
    let raw = if ix < 1 || iz < 1 || ix > width - 3 || iz > height - 3 {
        sample(ix, iz)?
    } else {
        triangle_split_height_sample(
            sample(ix, iz)?,
            sample(ix + 1, iz)?,
            sample(ix, iz + 1)?,
            sample(ix + 1, iz + 1)?,
            x - floor_x,
            z - floor_z,
        )
    };
    Some(raw * (map.max_height / 255.0))
}

#[cfg(test)]
#[path = "camera_terrain_tests.rs"]
mod tests;
