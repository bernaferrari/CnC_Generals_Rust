//! Live Main parser for `W3D_CHUNK_EMITTER` (0x500).
//!
//! C++ `ParticleEmitterLoaderClass::Load_W3D` registers a prototype so HLOD
//! children named after the emitter can `Create_Render_Obj`.

use super::prelude::*;
use super::w3d_format::{
    W3D_CHUNK_EMITTER_HEADER, W3D_CHUNK_EMITTER_INFO, W3D_CHUNK_EMITTER_INFOV2,
};

#[derive(Debug, Clone, PartialEq)]
pub struct W3dEmitterProto {
    pub name: String,
    pub version: u32,
    pub texture_filename: String,
    pub start_size: f32,
    pub end_size: f32,
    pub lifetime: f32,
    pub emission_rate: f32,
    pub burst_size: u32,
}

pub fn parse_emitter_chunk(data: &[u8]) -> Option<W3dEmitterProto> {
    let mut proto = W3dEmitterProto {
        name: String::new(),
        version: 0,
        texture_filename: String::new(),
        start_size: 1.0,
        end_size: 1.0,
        lifetime: 1.0,
        emission_rate: 1.0,
        burst_size: 1,
    };
    let mut offset = 0usize;
    while offset + 8 <= data.len() {
        let chunk_type = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
        let raw_size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().ok()?);
        let chunk_size = (raw_size & 0x7FFF_FFFF) as usize;
        let start = offset + 8;
        if start + chunk_size > data.len() {
            break;
        }
        let payload = &data[start..start + chunk_size];
        match chunk_type {
            W3D_CHUNK_EMITTER_HEADER if payload.len() >= 20 => {
                proto.version = u32::from_le_bytes(payload[0..4].try_into().ok()?);
                proto.name = w3d_string_from_bytes(&payload[4..20]);
            }
            W3D_CHUNK_EMITTER_INFO if payload.len() >= 260 + 16 => {
                proto.texture_filename = w3d_string_from_bytes(&payload[0..260]);
                proto.start_size = f32::from_le_bytes(payload[260..264].try_into().ok()?);
                proto.end_size = f32::from_le_bytes(payload[264..268].try_into().ok()?);
                proto.lifetime = f32::from_le_bytes(payload[268..272].try_into().ok()?);
                proto.emission_rate = f32::from_le_bytes(payload[272..276].try_into().ok()?);
            }
            W3D_CHUNK_EMITTER_INFOV2 if payload.len() >= 4 => {
                proto.burst_size = u32::from_le_bytes(payload[0..4].try_into().ok()?);
            }
            _ => {}
        }
        offset = start + chunk_size;
    }
    if proto.name.is_empty() {
        proto.name = "Emitter".into();
    }
    Some(proto)
}
