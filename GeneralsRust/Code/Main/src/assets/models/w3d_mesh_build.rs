//! Mechanical split from `assets/models.rs`. No behavior change.
#![allow(dead_code, unused_imports)]
use super::prelude::*;
use super::w3d_anim::*;
use super::w3d_format::*;
use super::w3d_loader::*;
use super::w3d_loader_parse::*;
use super::w3d_mesh::*;
use super::w3d_model::*;
use super::*;

pub(super) struct MeshHeader {
    pub version: u32,
    pub flags: u32,
    pub num_triangles: u32,
    pub num_vertices: u32,
    pub mesh_name: String,
    pub container_name: String,
}

/// Get common C&C unit models - updated with actual units found in archives
pub fn get_common_cnc_units() -> Vec<&'static str> {
    vec![
        // USA Units
        "humvee",   // avhummer - Confirmed exists
        "crusader", // avcrusader - Confirmed exists
        "chinook",  // avchinook - Confirmed exists
        "comanche", // avcomanche - Attack helicopter
        "abrams",   // Maps to crusader (main US tank)
        // China Units
        "mig",          // nvmign - Confirmed exists
        "helix",        // nvhelix - Confirmed exists
        "gattling",     // nvgatttank - Confirmed exists
        "battlemaster", // Chinese main battle tank
        "dragon",       // Dragon tank
        // GLA Units
        "scorpion",  // uvscorpion - Confirmed exists
        "toxin",     // uvtoxintrk - Confirmed exists
        "scud",      // SCUD launcher
        "technical", // Technical truck
        "marauder",  // GLA tank
        // Test units with confirmed models
        "test_tank",    // Uses uvscorpion
        "test_vehicle", // Uses avhummer
        "test_air",     // Uses nvhelix
    ]
}

pub(super) fn deduplicate_stage_uv_layers(
    layers: Vec<Vec<[f32; 2]>>,
) -> (Vec<Vec<[f32; 2]>>, Vec<u8>) {
    pub(super) const MAX_CHANNELS: usize = 4;
    let mut unique_layers: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut stage_channels: Vec<u8> = Vec::new();
    let mut crc_map = HashMap::new();

    for coords in layers {
        if coords.is_empty() {
            if unique_layers.is_empty() {
                unique_layers.push(Vec::new());
            }
            stage_channels.push(0);
            continue;
        }

        let mut hasher = Hasher::new();
        hasher.update(bytemuck::cast_slice(&coords));
        let crc = hasher.finalize();

        let channel = if let Some(&existing) = crc_map.get(&crc) {
            existing
        } else {
            let assigned = if unique_layers.len() < MAX_CHANNELS {
                let ch = unique_layers.len() as u8;
                unique_layers.push(coords.clone());
                ch
            } else {
                (MAX_CHANNELS.saturating_sub(1)) as u8
            };
            crc_map.insert(crc, assigned);
            assigned
        };

        stage_channels.push(channel);
    }

    if unique_layers.is_empty() {
        unique_layers.push(Vec::new());
    }

    (unique_layers, stage_channels)
}
