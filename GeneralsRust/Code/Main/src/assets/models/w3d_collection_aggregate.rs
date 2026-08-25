//! Live Main parsers for Collection / Aggregate / DistLOD prototypes.

use super::prelude::*;
use super::w3d_format::{
    W3D_CHUNK_AGGREGATE_HEADER, W3D_CHUNK_AGGREGATE_INFO, W3D_CHUNK_COLLECTION_HEADER,
    W3D_CHUNK_COLLECTION_OBJ_NAME, W3D_CHUNK_LOD, W3D_CHUNK_LODMODEL_HEADER,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dCollectionProto {
    pub name: String,
    pub version: u32,
    pub object_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dAggregateSubobject {
    pub subobject_name: String,
    pub bone_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dAggregateProto {
    pub name: String,
    pub version: u32,
    pub base_model_name: String,
    pub subobjects: Vec<W3dAggregateSubobject>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct W3dDistLodEntry {
    pub render_obj_name: String,
    pub lod_min: f32,
    pub lod_max: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct W3dDistLodProto {
    pub name: String,
    pub version: u32,
    pub lods: Vec<W3dDistLodEntry>,
}

pub fn parse_collection_chunk(data: &[u8]) -> Option<W3dCollectionProto> {
    let mut offset = 0usize;
    let header = next_chunk(data, &mut offset)?;
    if header.0 != W3D_CHUNK_COLLECTION_HEADER || header.1.len() < 16 + 4 {
        return None;
    }
    let version = u32::from_le_bytes(header.1[0..4].try_into().ok()?);
    let name = w3d_string_from_bytes(&header.1[4..20.min(header.1.len())]);
    let mut proto = W3dCollectionProto {
        name,
        version,
        object_names: Vec::new(),
    };
    while let Some((chunk_type, payload)) = next_chunk(data, &mut offset) {
        if chunk_type == W3D_CHUNK_COLLECTION_OBJ_NAME {
            proto.object_names.push(cstring_payload(payload));
        }
    }
    Some(proto)
}

pub fn parse_aggregate_chunk(data: &[u8]) -> Option<W3dAggregateProto> {
    let mut proto = W3dAggregateProto {
        name: String::new(),
        version: 0,
        base_model_name: String::new(),
        subobjects: Vec::new(),
    };
    let mut offset = 0usize;
    while let Some((chunk_type, payload)) = next_chunk(data, &mut offset) {
        match chunk_type {
            W3D_CHUNK_AGGREGATE_HEADER if payload.len() >= 20 => {
                proto.version = u32::from_le_bytes(payload[0..4].try_into().ok()?);
                proto.name = w3d_string_from_bytes(&payload[4..20]);
            }
            W3D_CHUNK_AGGREGATE_INFO if payload.len() >= 36 => {
                proto.base_model_name = w3d_string_from_bytes(&payload[0..32]);
                let count = u32::from_le_bytes(payload[32..36].try_into().ok()?) as usize;
                let mut cursor = 36usize;
                for _ in 0..count {
                    if cursor + 64 > payload.len() {
                        break;
                    }
                    proto.subobjects.push(W3dAggregateSubobject {
                        subobject_name: w3d_string_from_bytes(&payload[cursor..cursor + 32]),
                        bone_name: w3d_string_from_bytes(&payload[cursor + 32..cursor + 64]),
                    });
                    cursor += 64;
                }
            }
            _ => {}
        }
    }
    if proto.name.is_empty() {
        None
    } else {
        Some(proto)
    }
}

pub fn parse_dist_lod_chunk(data: &[u8]) -> Option<W3dDistLodProto> {
    let mut offset = 0usize;
    let header = next_chunk(data, &mut offset)?;
    if header.0 != W3D_CHUNK_LODMODEL_HEADER || header.1.len() < 4 + 16 + 2 {
        return None;
    }
    let version = u32::from_le_bytes(header.1[0..4].try_into().ok()?);
    let name = w3d_string_from_bytes(&header.1[4..20]);
    let mut proto = W3dDistLodProto {
        name,
        version,
        lods: Vec::new(),
    };
    while let Some((chunk_type, payload)) = next_chunk(data, &mut offset) {
        if chunk_type == W3D_CHUNK_LOD && payload.len() >= 32 + 8 {
            proto.lods.push(W3dDistLodEntry {
                render_obj_name: w3d_string_from_bytes(&payload[0..32]),
                lod_min: f32::from_le_bytes(payload[32..36].try_into().ok()?),
                lod_max: f32::from_le_bytes(payload[36..40].try_into().ok()?),
            });
        }
    }
    Some(proto)
}

fn next_chunk<'a>(data: &'a [u8], offset: &mut usize) -> Option<(u32, &'a [u8])> {
    if *offset + 8 > data.len() {
        return None;
    }
    let chunk_type = u32::from_le_bytes(data[*offset..*offset + 4].try_into().ok()?);
    let raw_size = u32::from_le_bytes(data[*offset + 4..*offset + 8].try_into().ok()?);
    let chunk_size = (raw_size & 0x7FFF_FFFF) as usize;
    let start = *offset + 8;
    if start + chunk_size > data.len() {
        return None;
    }
    *offset = start + chunk_size;
    Some((chunk_type, &data[start..start + chunk_size]))
}

fn cstring_payload(payload: &[u8]) -> String {
    let end = payload
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(payload.len());
    String::from_utf8_lossy(&payload[..end]).into_owned()
}
