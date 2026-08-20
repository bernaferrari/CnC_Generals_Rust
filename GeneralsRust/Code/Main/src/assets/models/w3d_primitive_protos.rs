//! Live Main parsers for Box / Ring / Sphere / Null prototypes.
//!
//! C++ `WW3DAssetManager` registers these so HLOD children and
//! `Create_Render_Obj` can instantiate them. Oriented boxes (`attributes & 1`)
//! are `CLASSID_OBBOX`.

use super::prelude::*;

/// C++ `W3D_BOX_ATTRIBUTE_ORIENTED`.
pub const W3D_BOX_ATTRIBUTE_ORIENTED: u32 = 0x0000_0001;

#[derive(Debug, Clone, PartialEq)]
pub struct W3dBoxProto {
    pub name: String,
    pub attributes: u32,
    pub center: [f32; 3],
    pub extent: [f32; 3],
}

impl W3dBoxProto {
    pub fn is_oriented(&self) -> bool {
        self.attributes & W3D_BOX_ATTRIBUTE_ORIENTED != 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct W3dSphereProto {
    pub name: String,
    pub center: [f32; 3],
    pub radius: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct W3dRingProto {
    pub name: String,
    pub center: [f32; 3],
    pub inner_radius: f32,
    pub outer_radius: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct W3dNullProto {
    pub name: String,
}

pub fn parse_box_chunk(data: &[u8]) -> Option<W3dBoxProto> {
    // version + attributes + name[32] + rgb+pad + center[3] + extent[3]
    if data.len() < 4 + 4 + 32 + 4 + 12 + 12 {
        return None;
    }
    let attributes = u32::from_le_bytes(data[4..8].try_into().ok()?);
    let name = w3d_string_from_bytes(&data[8..40]);
    let cx = f32::from_le_bytes(data[44..48].try_into().ok()?);
    let cy = f32::from_le_bytes(data[48..52].try_into().ok()?);
    let cz = f32::from_le_bytes(data[52..56].try_into().ok()?);
    let ex = f32::from_le_bytes(data[56..60].try_into().ok()?);
    let ey = f32::from_le_bytes(data[60..64].try_into().ok()?);
    let ez = f32::from_le_bytes(data[64..68].try_into().ok()?);
    Some(W3dBoxProto {
        name,
        attributes,
        center: [cx, cy, cz],
        extent: [ex, ey, ez],
    })
}

pub fn parse_null_chunk(data: &[u8]) -> Option<W3dNullProto> {
    if data.len() < 16 + 32 {
        return None;
    }
    Some(W3dNullProto {
        name: w3d_string_from_bytes(&data[16..48.min(data.len())]),
    })
}

/// Sphere / ring W3D payloads are `CHUNKID_DEF = 1` containers.
pub fn parse_sphere_chunk(data: &[u8]) -> Option<W3dSphereProto> {
    if let Some((name, center, extent)) = parse_def_name_center_extent(data) {
        let radius = extent[0].max(extent[1]).max(extent[2]);
        return Some(W3dSphereProto {
            name,
            center,
            radius,
        });
    }
    if data.len() >= 40 {
        return Some(W3dSphereProto {
            name: w3d_string_from_bytes(&data[8..24]),
            center: [0.0, 0.0, 0.0],
            radius: 1.0,
        });
    }
    None
}

pub fn parse_ring_chunk(data: &[u8]) -> Option<W3dRingProto> {
    if let Some((name, center, _extent)) = parse_def_name_center_extent(data) {
        return Some(W3dRingProto {
            name,
            center,
            inner_radius: 0.0,
            outer_radius: 1.0,
        });
    }
    if data.len() >= 40 {
        return Some(W3dRingProto {
            name: w3d_string_from_bytes(&data[8..24]),
            center: [0.0, 0.0, 0.0],
            inner_radius: 0.0,
            outer_radius: 1.0,
        });
    }
    None
}

fn parse_def_name_center_extent(data: &[u8]) -> Option<(String, [f32; 3], [f32; 3])> {
    const CHUNKID_DEF: u32 = 1;
    let mut offset = 0usize;
    while offset + 8 <= data.len() {
        let chunk_type = u32::from_le_bytes(data[offset..offset + 4].try_into().ok()?);
        let raw_size = u32::from_le_bytes(data[offset + 4..offset + 8].try_into().ok()?);
        let chunk_size = (raw_size & 0x7FFF_FFFF) as usize;
        let start = offset + 8;
        if start + chunk_size > data.len() {
            break;
        }
        if chunk_type == CHUNKID_DEF && chunk_size >= 8 + 32 + 24 {
            let payload = &data[start..start + chunk_size];
            let name = w3d_string_from_bytes(&payload[8..40]);
            let cx = f32::from_le_bytes(payload[40..44].try_into().ok()?);
            let cy = f32::from_le_bytes(payload[44..48].try_into().ok()?);
            let cz = f32::from_le_bytes(payload[48..52].try_into().ok()?);
            let ex = f32::from_le_bytes(payload[52..56].try_into().ok()?);
            let ey = f32::from_le_bytes(payload[56..60].try_into().ok()?);
            let ez = f32::from_le_bytes(payload[60..64].try_into().ok()?);
            return Some((name, [cx, cy, cz], [ex, ey, ez]));
        }
        offset = start + chunk_size;
    }
    None
}
