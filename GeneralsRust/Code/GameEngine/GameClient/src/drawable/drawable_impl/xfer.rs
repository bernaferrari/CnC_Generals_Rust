//! Snapshot/xfer helpers for drawable enums, matrices, and module buckets.

use super::*;
use game_engine::common::bit_flags::ModelConditionBitFlags;
use game_engine::common::system::{Xfer, XferMode, XferVersion};

pub(crate) fn xfer_matrix3d(xfer: &mut dyn Xfer, value: &mut Matrix4) -> Result<(), String> {
    let mut version: XferVersion = 1;
    xfer.xfer_version(&mut version, 1)
        .map_err(|e| format!("{:?}", e))?;
    xfer_matrix3d_user(xfer, value)
}

pub(crate) fn xfer_matrix3d_user(xfer: &mut dyn Xfer, value: &mut Matrix4) -> Result<(), String> {
    for row in 0..3 {
        for col in 0..4 {
            xfer.xfer_real(&mut value.elements[row][col])
                .map_err(|e| format!("{:?}", e))?;
        }
    }
    if xfer.get_xfer_mode() == XferMode::Load {
        value.elements[3] = [0.0, 0.0, 0.0, 1.0];
    }
    Ok(())
}

pub(crate) fn xfer_model_condition_flags(
    xfer: &mut dyn Xfer,
    flags: &mut ModelConditionBitFlags,
) -> Result<(), String> {
    let mut stream_bit_count = flags.size().min(u16::MAX as usize) as u16;
    xfer.xfer_unsigned_short(&mut stream_bit_count)
        .map_err(|e| format!("{:?}", e))?;

    let stream_bit_count = stream_bit_count as usize;
    match xfer.get_xfer_mode() {
        XferMode::Save | XferMode::Crc => {
            for i in 0..stream_bit_count {
                let mut value = flags.test(i);
                xfer.xfer_bool(&mut value).map_err(|e| format!("{:?}", e))?;
            }
        }
        XferMode::Load => {
            flags.clear();
            for i in 0..stream_bit_count {
                let mut value = false;
                xfer.xfer_bool(&mut value).map_err(|e| format!("{:?}", e))?;
                if i < flags.size() {
                    flags.set(i, value);
                }
            }
        }
        XferMode::Invalid => {
            return Err("xfer_model_condition_flags - invalid xfer mode".to_string());
        }
    }

    Ok(())
}

pub(crate) fn stealth_look_to_u32(look: StealthLook) -> u32 {
    match look {
        StealthLook::None => 0,
        StealthLook::VisibleFriendly => 1,
        StealthLook::DisguisedEnemy => 2,
        StealthLook::VisibleDetected => 3,
        StealthLook::VisibleFriendlyDetected => 4,
        StealthLook::Invisible => 5,
    }
}

pub(crate) fn stealth_look_from_u32(value: u32) -> StealthLook {
    match value {
        1 => StealthLook::VisibleFriendly,
        2 => StealthLook::DisguisedEnemy,
        3 => StealthLook::VisibleDetected,
        4 => StealthLook::VisibleFriendlyDetected,
        5 => StealthLook::Invisible,
        _ => StealthLook::None,
    }
}

pub(crate) fn terrain_decal_to_u32(decal: TerrainDecalType) -> u32 {
    match decal {
        TerrainDecalType::Demoralized => 0,
        TerrainDecalType::Horde => 1,
        TerrainDecalType::HordeWithNationalism => 2,
        TerrainDecalType::HordeVehicle => 3,
        TerrainDecalType::HordeWithNationalismVehicle => 4,
        TerrainDecalType::Crate => 5,
        TerrainDecalType::HordeWithFanaticism => 6,
        TerrainDecalType::ChemSuit => 7,
        TerrainDecalType::None => 8,
        TerrainDecalType::ShadowTexture => 9,
    }
}

pub(crate) fn terrain_decal_from_u32(value: u32) -> TerrainDecalType {
    match value {
        0 => TerrainDecalType::Demoralized,
        1 => TerrainDecalType::Horde,
        2 => TerrainDecalType::HordeWithNationalism,
        3 => TerrainDecalType::HordeVehicle,
        4 => TerrainDecalType::HordeWithNationalismVehicle,
        5 => TerrainDecalType::Crate,
        6 => TerrainDecalType::HordeWithFanaticism,
        7 => TerrainDecalType::ChemSuit,
        9 => TerrainDecalType::ShadowTexture,
        _ => TerrainDecalType::None,
    }
}

pub(crate) fn fading_mode_to_u32(mode: FadingMode) -> u32 {
    match mode {
        FadingMode::None => 0,
        FadingMode::FadingIn => 1,
        FadingMode::FadingOut => 2,
    }
}

pub(crate) fn fading_mode_from_u32(value: u32) -> FadingMode {
    match value {
        1 => FadingMode::FadingIn,
        2 => FadingMode::FadingOut,
        _ => FadingMode::None,
    }
}

pub(crate) fn vector3_to_color_bits(color: Vector3) -> i32 {
    // C++ xferColor encodes as ARGB i32. Convert from Vector3 (r,g,b 0-1) to ARGB.
    let r = (color.x.clamp(0.0, 1.0) * 255.0) as u32;
    let g = (color.y.clamp(0.0, 1.0) * 255.0) as u32;
    let b = (color.z.clamp(0.0, 1.0) * 255.0) as u32;
    (0xFF << 24 | r << 16 | g << 8 | b) as i32
}

pub(crate) fn color_bits_to_vector3(bits: i32) -> Vector3 {
    let bits = bits as u32;
    Vector3::new(
        ((bits >> 16) & 0xFF) as f32 / 255.0,
        ((bits >> 8) & 0xFF) as f32 / 255.0,
        (bits & 0xFF) as f32 / 255.0,
    )
}

pub(crate) fn xfer_drawable_modules(
    xfer: &mut dyn Xfer,
    modules: &mut [Box<dyn DrawModule>],
) -> Result<(), String> {
    // PARITY_NOTE: C++ Drawable::xferDrawableModules (Drawable.cpp line 4767).
    // Saves version, module type count, then per-type: module count + name-keyed blocks.
    const CURRENT_VERSION: XferVersion = 1;
    let mut version = CURRENT_VERSION;
    xfer.xfer_version(&mut version, CURRENT_VERSION)
        .map_err(|e| format!("{:?}", e))?;

    let mut module_types: u16 = 2;
    xfer.xfer_unsigned_short(&mut module_types)
        .map_err(|e| format!("{:?}", e))?;

    for module_type in 0..module_types {
        let module_type_index = module_type as usize;
        let mut module_indices = if xfer.get_xfer_mode() == XferMode::Save {
            modules
                .iter()
                .enumerate()
                .filter_map(|(index, module)| {
                    (module.drawable_module_type_index() == module_type_index
                        && module.snapshot_module_identifier().is_some())
                    .then_some(index)
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };

        let mut module_count = module_indices.len().min(u16::MAX as usize) as u16;
        xfer.xfer_unsigned_short(&mut module_count)
            .map_err(|e| format!("{:?}", e))?;

        if xfer.get_xfer_mode() == XferMode::Save {
            module_indices.truncate(module_count as usize);
            for module_index in module_indices {
                let module = &mut modules[module_index];
                let mut module_identifier = module
                    .snapshot_module_identifier()
                    .unwrap_or_default()
                    .to_string();
                xfer.xfer_ascii_string(&mut module_identifier)
                    .map_err(|e| format!("{:?}", e))?;
                xfer.begin_block().map_err(|e| format!("{:?}", e))?;
                module.xfer_snapshot(xfer)?;
                xfer.end_block().map_err(|e| format!("{:?}", e))?;
            }
        } else {
            for _ in 0..module_count {
                let mut module_identifier = String::new();
                xfer.xfer_ascii_string(&mut module_identifier)
                    .map_err(|e| format!("{:?}", e))?;

                let data_size = xfer.begin_block().map_err(|e| format!("{:?}", e))?;
                if let Some(module) = modules.iter_mut().find(|module| {
                    module.drawable_module_type_index() == module_type_index
                        && module.snapshot_module_identifier() == Some(module_identifier.as_str())
                }) {
                    module.xfer_snapshot(xfer)?;
                } else {
                    xfer.skip(data_size).map_err(|e| format!("{:?}", e))?;
                }
                xfer.end_block().map_err(|e| format!("{:?}", e))?;
            }
        }
    }

    Ok(())
}
