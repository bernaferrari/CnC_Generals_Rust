//! Live Main parser for `W3D_CHUNK_DAZZLE` (0x900).
//!
//! C++ `_DazzleLoader` registers headlights / sun glints / halos so HLOD
//! children can `Create_Render_Obj` them.

use super::prelude::*;
use super::w3d_format::{W3D_CHUNK_DAZZLE_NAME, W3D_CHUNK_DAZZLE_TYPENAME};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dDazzleProto {
    pub name: String,
    pub type_name: String,
}

pub fn parse_dazzle_chunk(data: &[u8]) -> Option<W3dDazzleProto> {
    let mut name = String::new();
    let mut type_name = String::new();
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
            W3D_CHUNK_DAZZLE_NAME => name = w3d_cstring_payload(payload),
            W3D_CHUNK_DAZZLE_TYPENAME => type_name = w3d_cstring_payload(payload),
            _ => {}
        }
        offset = start + chunk_size;
    }
    if name.is_empty() {
        name = "Dazzle".into();
    }
    Some(W3dDazzleProto { name, type_name })
}

fn w3d_cstring_payload(payload: &[u8]) -> String {
    let end = payload
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(payload.len());
    String::from_utf8_lossy(&payload[..end]).into_owned()
}
