//! Prototype loaders for Collection, Box, Ring, Sphere, Aggregate, DistLOD,
//! Null, Dazzle, HLod, and ParticleEmitter.
//!
//! C++ registers these on `WW3DAssetManager` (`assetmgr.cpp:148-164`) plus
//! `_ParticleEmitterLoader` from `W3DDisplay.cpp:700`.

use crate::agg_def::{AggregatePrototype, AggregateSubobject};
use crate::assets::Prototype;
use crate::chunk_reader::ChunkReader;
use crate::loaders::HlodLoader;
use crate::prototype_loader::PrototypeLoader;
use crate::prototypes::{
    BoxPrototype, CollectionPlaceholder, CollectionPrototype, CollectionTransformNode,
    DazzlePrototype, LodEntry, LodModelPrototype, NullPrototype, ParticleEmitterPrototype,
    RingPrototype, SpherePrototype,
};
use glam::{Mat4, Vec4};
use ww3d_core::{
    W3D_CHUNK_AGGREGATE, W3D_CHUNK_AGGREGATE_CLASS_INFO, W3D_CHUNK_AGGREGATE_HEADER,
    W3D_CHUNK_AGGREGATE_INFO, W3D_CHUNK_BOX, W3D_CHUNK_COLLECTION, W3D_CHUNK_COLLECTION_HEADER,
    W3D_CHUNK_COLLECTION_OBJ_NAME, W3D_CHUNK_DAZZLE, W3D_CHUNK_DAZZLE_NAME,
    W3D_CHUNK_DAZZLE_TYPENAME, W3D_CHUNK_EMITTER, W3D_CHUNK_EMITTER_HEADER, W3D_CHUNK_EMITTER_INFO,
    W3D_CHUNK_EMITTER_INFOV2, W3D_CHUNK_HLOD, W3D_CHUNK_LOD, W3D_CHUNK_LODMODEL,
    W3D_CHUNK_LODMODEL_HEADER, W3D_CHUNK_NULL_OBJECT, W3D_CHUNK_PLACEHOLDER, W3D_CHUNK_POINTS,
    W3D_CHUNK_RING, W3D_CHUNK_SPHERE, W3D_CHUNK_TRANSFORM_NODE, W3DError, W3dAggregateMiscInfo,
    W3dRGBAStruct, W3dVectorStruct, w3d_string_from_bytes,
};

fn chunk_err(err: crate::chunk_reader::ChunkError) -> W3DError {
    W3DError::IoError(err.to_string())
}

fn read_cstring<R: std::io::Read + std::io::Seek>(
    reader: &mut ChunkReader<R>,
    len: usize,
) -> Result<String, W3DError> {
    let mut buf = vec![0u8; len];
    reader.read(&mut buf).map_err(chunk_err)?;
    Ok(w3d_string_from_bytes(&buf))
}

fn read_remaining_string<R: std::io::Read + std::io::Seek>(
    reader: &mut ChunkReader<R>,
) -> Result<String, W3DError> {
    let remaining = reader.remaining().map_err(chunk_err)? as usize;
    if remaining == 0 {
        return Ok(String::new());
    }
    let mut buf = vec![0u8; remaining];
    reader.read(&mut buf).map_err(chunk_err)?;
    Ok(w3d_string_from_bytes(&buf))
}

/// MAX 3x4 dump `float transform[4][3]` → glam Mat4 (C++ collect.cpp:972-974).
fn max_3x4_to_mat4(cols: [[f32; 3]; 4]) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(cols[0][0], cols[0][1], cols[0][2], 0.0),
        Vec4::new(cols[1][0], cols[1][1], cols[1][2], 0.0),
        Vec4::new(cols[2][0], cols[2][1], cols[2][2], 0.0),
        Vec4::new(cols[3][0], cols[3][1], cols[3][2], 1.0),
    )
}

fn read_max_3x4<R: std::io::Read + std::io::Seek>(
    reader: &mut ChunkReader<R>,
) -> Result<Mat4, W3DError> {
    let mut cols = [[0.0f32; 3]; 4];
    for col in &mut cols {
        col[0] = reader.read_f32().map_err(chunk_err)?;
        col[1] = reader.read_f32().map_err(chunk_err)?;
        col[2] = reader.read_f32().map_err(chunk_err)?;
    }
    Ok(max_3x4_to_mat4(cols))
}

#[derive(Debug, Default)]
pub struct CollectionLoader;

impl PrototypeLoader for CollectionLoader {
    fn get_name(&self) -> &str {
        "CollectionLoader"
    }

    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_COLLECTION
    }

    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        if !reader.open_chunk().map_err(chunk_err)? {
            return Err(W3DError::IoError("collection missing header".into()));
        }
        if reader.current_chunk_id().map_err(chunk_err)? != W3D_CHUNK_COLLECTION_HEADER {
            return Err(W3DError::IoError("expected collection header".into()));
        }
        let version = reader.read_u32().map_err(chunk_err)?;
        let name = read_cstring(&mut reader, 16)?;
        let _count = reader.read_u32().map_err(chunk_err)?;
        let _pad0 = reader.read_u32().map_err(chunk_err)?;
        let _pad1 = reader.read_u32().map_err(chunk_err)?;
        reader.close_chunk().map_err(chunk_err)?;

        let mut proto = CollectionPrototype {
            name,
            version,
            object_names: Vec::new(),
            placeholders: Vec::new(),
            transform_nodes: Vec::new(),
            snap_points: Vec::new(),
        };

        while reader.open_chunk().map_err(chunk_err)? {
            match reader.current_chunk_id().map_err(chunk_err)? {
                W3D_CHUNK_COLLECTION_OBJ_NAME => {
                    proto.object_names.push(read_remaining_string(&mut reader)?);
                }
                W3D_CHUNK_PLACEHOLDER => {
                    let _version = reader.read_u32().map_err(chunk_err)?;
                    let transform = read_max_3x4(&mut reader)?;
                    let name_len = reader.read_u32().map_err(chunk_err)? as usize;
                    let name = read_cstring(&mut reader, name_len)?;
                    proto
                        .placeholders
                        .push(CollectionPlaceholder { name, transform });
                }
                W3D_CHUNK_TRANSFORM_NODE => {
                    let _version = reader.read_u32().map_err(chunk_err)?;
                    let transform = read_max_3x4(&mut reader)?;
                    let name_len = reader.read_u32().map_err(chunk_err)? as usize;
                    let name = read_cstring(&mut reader, name_len)?;
                    proto
                        .transform_nodes
                        .push(CollectionTransformNode { name, transform });
                }
                W3D_CHUNK_POINTS => {
                    let remaining = reader.remaining().map_err(chunk_err)? as usize;
                    let count = remaining / 12;
                    for _ in 0..count {
                        let v = reader.read_vec3().map_err(chunk_err)?;
                        proto.snap_points.push(W3dVectorStruct {
                            x: v.x,
                            y: v.y,
                            z: v.z,
                        });
                    }
                }
                _ => {}
            }
            reader.close_chunk().map_err(chunk_err)?;
        }

        Ok(Box::new(proto))
    }
}

#[derive(Debug, Default)]
pub struct BoxLoader;

impl PrototypeLoader for BoxLoader {
    fn get_name(&self) -> &str {
        "BoxLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_BOX
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        if data.len() < 4 + 4 + 32 + 4 + 12 + 12 {
            return Err(W3DError::IoError("truncated box chunk".into()));
        }
        let mut off = 0usize;
        let read_u32 = |off: &mut usize| {
            let v = u32::from_le_bytes(data[*off..*off + 4].try_into().unwrap());
            *off += 4;
            v
        };
        let read_f32 = |off: &mut usize| {
            let v = f32::from_le_bytes(data[*off..*off + 4].try_into().unwrap());
            *off += 4;
            v
        };
        let _version = read_u32(&mut off);
        let attributes = read_u32(&mut off);
        let name = w3d_string_from_bytes(&data[off..off + 32]);
        off += 32;
        let r = data[off];
        let g = data[off + 1];
        let b = data[off + 2];
        off += 4; // C++ W3dRGBStruct is r,g,b,pad
        let cx = read_f32(&mut off);
        let cy = read_f32(&mut off);
        let cz = read_f32(&mut off);
        let ex = read_f32(&mut off);
        let ey = read_f32(&mut off);
        let ez = read_f32(&mut off);
        Ok(Box::new(BoxPrototype {
            name,
            attributes,
            color: W3dRGBAStruct { r, g, b, a: 255 },
            center: W3dVectorStruct {
                x: cx,
                y: cy,
                z: cz,
            },
            extent: W3dVectorStruct {
                x: ex,
                y: ey,
                z: ez,
            },
        }))
    }
}

const CHUNKID_DEF: u32 = 1;

#[derive(Debug, Default)]
pub struct SphereLoader;

impl PrototypeLoader for SphereLoader {
    fn get_name(&self) -> &str {
        "SphereLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_SPHERE
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        let mut name = String::new();
        let mut center = W3dVectorStruct {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let mut radius = 1.0f32;
        let mut color = W3dRGBAStruct {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };

        while reader.open_chunk().map_err(chunk_err)? {
            if reader.current_chunk_id().map_err(chunk_err)? == CHUNKID_DEF {
                let _version = reader.read_u32().map_err(chunk_err)?;
                let _attributes = reader.read_u32().map_err(chunk_err)?;
                name = read_cstring(&mut reader, 32)?;
                let c = reader.read_vec3().map_err(chunk_err)?;
                center = W3dVectorStruct {
                    x: c.x,
                    y: c.y,
                    z: c.z,
                };
                let extent = reader.read_vec3().map_err(chunk_err)?;
                radius = extent.x.max(extent.y).max(extent.z);
                let _anim = reader.read_f32().map_err(chunk_err).unwrap_or(0.0);
                if let Ok(dc) = reader.read_vec3() {
                    color = W3dRGBAStruct {
                        r: (dc.x.clamp(0.0, 1.0) * 255.0) as u8,
                        g: (dc.y.clamp(0.0, 1.0) * 255.0) as u8,
                        b: (dc.z.clamp(0.0, 1.0) * 255.0) as u8,
                        a: 255,
                    };
                }
            }
            reader.close_chunk().map_err(chunk_err)?;
        }

        if name.is_empty() && data.len() >= 40 {
            // Fallback: simplified W3dSphereStruct at payload start.
            name = w3d_string_from_bytes(&data[8..24]);
        }

        Ok(Box::new(SpherePrototype {
            name,
            color,
            center,
            radius,
        }))
    }
}

#[derive(Debug, Default)]
pub struct RingLoader;

impl PrototypeLoader for RingLoader {
    fn get_name(&self) -> &str {
        "RingLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_RING
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        let mut name = String::new();
        let mut center = W3dVectorStruct {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        };
        let mut inner_radius = 0.0f32;
        let mut outer_radius = 1.0f32;
        let mut color = W3dRGBAStruct {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };

        while reader.open_chunk().map_err(chunk_err)? {
            if reader.current_chunk_id().map_err(chunk_err)? == CHUNKID_DEF {
                let _version = reader.read_u32().map_err(chunk_err)?;
                let _attributes = reader.read_u32().map_err(chunk_err)?;
                name = read_cstring(&mut reader, 32)?;
                let c = reader.read_vec3().map_err(chunk_err)?;
                center = W3dVectorStruct {
                    x: c.x,
                    y: c.y,
                    z: c.z,
                };
                let _extent = reader.read_vec3().map_err(chunk_err)?;
                let _anim = reader.read_f32().map_err(chunk_err)?;
                if let Ok(dc) = reader.read_vec3() {
                    color = W3dRGBAStruct {
                        r: (dc.x.clamp(0.0, 1.0) * 255.0) as u8,
                        g: (dc.y.clamp(0.0, 1.0) * 255.0) as u8,
                        b: (dc.z.clamp(0.0, 1.0) * 255.0) as u8,
                        a: 255,
                    };
                }
                let _alpha = reader.read_f32().ok();
                let _inner_scale = reader.read_vec2().ok();
                let _outer_scale = reader.read_vec2().ok();
                if let Ok(inner) = reader.read_vec2() {
                    inner_radius = inner.x;
                }
                if let Ok(outer) = reader.read_vec2() {
                    outer_radius = outer.x;
                }
            }
            reader.close_chunk().map_err(chunk_err)?;
        }

        if name.is_empty() && data.len() >= 40 {
            name = w3d_string_from_bytes(&data[8..24]);
        }

        Ok(Box::new(RingPrototype {
            name,
            color,
            center,
            inner_radius,
            outer_radius,
        }))
    }
}

#[derive(Debug, Default)]
pub struct AggregateLoader;

impl PrototypeLoader for AggregateLoader {
    fn get_name(&self) -> &str {
        "AggregateLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_AGGREGATE
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        let mut proto = AggregatePrototype {
            name: String::new(),
            version: 0,
            base_model_name: String::new(),
            subobjects: Vec::new(),
            misc_info: W3dAggregateMiscInfo {
                original_class_id: 0,
                flags: 0,
                reserved: [0; 3],
            },
            texture_replacers: Vec::new(),
        };

        while reader.open_chunk().map_err(chunk_err)? {
            match reader.current_chunk_id().map_err(chunk_err)? {
                W3D_CHUNK_AGGREGATE_HEADER => {
                    proto.version = reader.read_u32().map_err(chunk_err)?;
                    proto.name = read_cstring(&mut reader, 16)?;
                }
                W3D_CHUNK_AGGREGATE_INFO => {
                    proto.base_model_name = read_cstring(&mut reader, 32)?;
                    let count = reader.read_u32().map_err(chunk_err)?;
                    for _ in 0..count {
                        let subobject_name = read_cstring(&mut reader, 32)?;
                        let bone_name = read_cstring(&mut reader, 32)?;
                        proto.subobjects.push(AggregateSubobject {
                            subobject_name,
                            bone_name,
                        });
                    }
                }
                W3D_CHUNK_AGGREGATE_CLASS_INFO => {
                    proto.misc_info.original_class_id = reader.read_u32().map_err(chunk_err)?;
                    proto.misc_info.flags = reader.read_u32().map_err(chunk_err)?;
                    proto.misc_info.reserved = [
                        reader.read_u32().map_err(chunk_err)?,
                        reader.read_u32().map_err(chunk_err)?,
                        reader.read_u32().map_err(chunk_err)?,
                    ];
                }
                _ => {}
            }
            reader.close_chunk().map_err(chunk_err)?;
        }

        Ok(Box::new(proto))
    }
}

#[derive(Debug, Default)]
pub struct DistLodLoader;

impl PrototypeLoader for DistLodLoader {
    fn get_name(&self) -> &str {
        "DistLODLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_LODMODEL
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        if !reader.open_chunk().map_err(chunk_err)? {
            return Err(W3DError::IoError("distlod missing header".into()));
        }
        if reader.current_chunk_id().map_err(chunk_err)? != W3D_CHUNK_LODMODEL_HEADER {
            return Err(W3DError::IoError("expected lodmodel header".into()));
        }
        let version = reader.read_u32().map_err(chunk_err)?;
        let name = read_cstring(&mut reader, 16)?;
        let lod_count = reader.read_u16().map_err(chunk_err)? as usize;
        reader.close_chunk().map_err(chunk_err)?;

        let mut lods = Vec::with_capacity(lod_count);
        for _ in 0..lod_count {
            if !reader.open_chunk().map_err(chunk_err)? {
                break;
            }
            if reader.current_chunk_id().map_err(chunk_err)? != W3D_CHUNK_LOD {
                reader.close_chunk().map_err(chunk_err)?;
                continue;
            }
            let render_obj_name = read_cstring(&mut reader, 32)?;
            let lod_min = reader.read_f32().map_err(chunk_err)?;
            let lod_max = reader.read_f32().map_err(chunk_err)?;
            lods.push(LodEntry {
                render_obj_name,
                lod_min,
                lod_max,
            });
            reader.close_chunk().map_err(chunk_err)?;
        }

        Ok(Box::new(LodModelPrototype {
            name,
            version,
            lods,
        }))
    }
}

#[derive(Debug, Default)]
pub struct NullLoader;

impl PrototypeLoader for NullLoader {
    fn get_name(&self) -> &str {
        "NullLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_NULL_OBJECT
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        if data.len() < 16 + 32 {
            return Err(W3DError::IoError("truncated null object".into()));
        }
        // version + attributes + pad[2] + name[32]
        let name = w3d_string_from_bytes(&data[16..48.min(data.len())]);
        Ok(Box::new(NullPrototype { name }))
    }
}

#[derive(Debug, Default)]
pub struct DazzleLoader;

impl PrototypeLoader for DazzleLoader {
    fn get_name(&self) -> &str {
        "DazzleLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_DAZZLE
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        let mut name = String::new();
        let mut type_name = String::new();
        while reader.open_chunk().map_err(chunk_err)? {
            match reader.current_chunk_id().map_err(chunk_err)? {
                W3D_CHUNK_DAZZLE_NAME => name = read_remaining_string(&mut reader)?,
                W3D_CHUNK_DAZZLE_TYPENAME => type_name = read_remaining_string(&mut reader)?,
                _ => {}
            }
            reader.close_chunk().map_err(chunk_err)?;
        }
        if name.is_empty() {
            name = "Dazzle".into();
        }
        Ok(Box::new(DazzlePrototype { name, type_name }))
    }
}

#[derive(Debug, Default)]
pub struct HLodProtoLoader;

impl PrototypeLoader for HLodProtoLoader {
    fn get_name(&self) -> &str {
        "HLodLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_HLOD
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        let proto = HlodLoader::load_hlod(&mut reader).map_err(chunk_err)?;
        Ok(Box::new(proto))
    }
}

#[derive(Debug, Default)]
pub struct ParticleEmitterLoader;

impl PrototypeLoader for ParticleEmitterLoader {
    fn get_name(&self) -> &str {
        "ParticleEmitterLoader"
    }
    fn can_load(&self, chunk_type: u32) -> bool {
        chunk_type == W3D_CHUNK_EMITTER
    }
    fn load_w3d(
        &self,
        data: &[u8],
        _chunk_type: u32,
        _asset_name: &str,
    ) -> Result<Box<dyn Prototype>, W3DError> {
        let mut reader = ChunkReader::from_slice(data);
        let mut proto = ParticleEmitterPrototype {
            name: String::new(),
            version: 0,
            texture_filename: String::new(),
            start_size: 1.0,
            end_size: 1.0,
            lifetime: 1.0,
            emission_rate: 1.0,
            burst_size: 1,
        };

        while reader.open_chunk().map_err(chunk_err)? {
            match reader.current_chunk_id().map_err(chunk_err)? {
                W3D_CHUNK_EMITTER_HEADER => {
                    proto.version = reader.read_u32().map_err(chunk_err)?;
                    proto.name = read_cstring(&mut reader, 16)?;
                }
                W3D_CHUNK_EMITTER_INFO => {
                    proto.texture_filename = read_cstring(&mut reader, 260)?;
                    proto.start_size = reader.read_f32().map_err(chunk_err)?;
                    proto.end_size = reader.read_f32().map_err(chunk_err)?;
                    proto.lifetime = reader.read_f32().map_err(chunk_err)?;
                    proto.emission_rate = reader.read_f32().map_err(chunk_err)?;
                }
                W3D_CHUNK_EMITTER_INFOV2 => {
                    proto.burst_size = reader.read_u32().map_err(chunk_err)?;
                }
                _ => {}
            }
            reader.close_chunk().map_err(chunk_err)?;
        }

        if proto.name.is_empty() {
            proto.name = "Emitter".into();
        }
        Ok(Box::new(proto))
    }
}
