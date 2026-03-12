//! W3D File Format Data Structures
//! Core types and constants for W3D file format

use crate::w3d_chunks::W3DChunkType;

/// Length of fixed-size name fields used throughout the W3D format
pub const W3D_NAME_LEN: usize = 16;

/// Maximum depth of mesh/bone paths stored in aggregate texture replacers
pub const MESH_PATH_ENTRIES: usize = 15;

/// Length of an individual mesh/bone path entry (two name slots concatenated)
pub const MESH_PATH_ENTRY_LEN: usize = W3D_NAME_LEN * 2;

/// W3D chunk header structure
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct W3dChunkHeader {
    pub chunk_type: u32,
    pub chunk_size: u32,
}

impl W3dChunkHeader {
    pub fn new(chunk_type: u32, chunk_size: u32) -> Self {
        Self {
            chunk_type,
            chunk_size,
        }
    }

    pub fn chunk_type(&self) -> Option<W3DChunkType> {
        W3DChunkType::from_u32(self.chunk_type)
    }

    /// Get the actual chunk size (masking out the sub-chunk flag)
    pub fn actual_size(&self) -> u32 {
        self.chunk_size & 0x7FFFFFFF
    }

    /// Check if this chunk contains sub-chunks
    pub fn has_sub_chunks(&self) -> bool {
        (self.chunk_size & 0x80000000) != 0
    }
}

/// 3D vector structure used in W3D files
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dVectorStruct {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl W3dVectorStruct {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// Quaternion structure used in W3D files
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dQuaternionStruct {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl W3dQuaternionStruct {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { w, x, y, z }
    }

    pub fn identity() -> Self {
        Self {
            w: 1.0,
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }
}

/// Texture coordinate structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dTexCoordStruct {
    pub u: f32,
    pub v: f32,
}

impl W3dTexCoordStruct {
    pub fn new(u: f32, v: f32) -> Self {
        Self { u, v }
    }
}

/// RGBA color structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dRGBAStruct {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl W3dRGBAStruct {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }

    pub fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }
}

/// Triangle structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dTriangleStruct {
    pub vindex: [u32; 3],
    pub attributes: u32,
    pub normal: W3dVectorStruct,
    pub distance: f32,
}

impl W3dTriangleStruct {
    pub fn new(v0: u32, v1: u32, v2: u32) -> Self {
        Self {
            vindex: [v0, v1, v2],
            attributes: 0,
            normal: W3dVectorStruct::zero(),
            distance: 0.0,
        }
    }
}

/// Mesh header structure (Version 3)
#[derive(Debug, Clone, Default)]
#[repr(C)]
pub struct W3dMeshHeader3Struct {
    pub version: u32,
    pub attrs: u32,
    pub mesh_name: [u8; 16],
    pub container_name: [u8; 16],
    pub num_tris: u32,
    pub num_verts: u32,
    pub num_materials: u32,
    pub num_damage_stages: u32,
    pub sort_level: i32,
    pub prelit_version: u32,
    pub future_counts: [u32; 1],
    pub vertex_channels: u32,
    pub face_channels: u32,
    pub bbox_min: W3dVectorStruct,
    pub bbox_max: W3dVectorStruct,
    pub sph_center: W3dVectorStruct,
    pub sph_radius: f32,
}

/// Vertex influence structure for skinning
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dVertInfStruct {
    pub bone_idx: u16,
    pub pad: [u8; 6],
}

impl W3dVertInfStruct {
    pub fn new(bone_idx: u16) -> Self {
        Self {
            bone_idx,
            pad: [0; 6],
        }
    }
}

/// Material info structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dMaterialInfoStruct {
    pub pass_count: u32,
    pub vert_matl_count: u32,
    pub shader_count: u32,
    pub texture_count: u32,
}

/// Shader structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dShaderStruct {
    pub depth_compare: u8,
    pub depth_mask: u8,
    pub color_mask: u8,
    pub dest_blend: u8,
    pub fog_func: u8,
    pub pri_gradient: u8,
    pub sec_gradient: u8,
    pub src_blend: u8,
    pub texturing: u8,
    pub detail_color_func: u8,
    pub detail_alpha_func: u8,
    pub shader_preset: u8,
    pub alpha_test: u8,
    pub post_detail_color_func: u8,
    pub post_detail_alpha_func: u8,
    pub pad: u8,
}

/// Texture info structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dTextureInfoStruct {
    pub attributes: u16,
    pub animation_type: u16,
    pub frame_count: u32,
    pub frame_rate: f32,
}

/// Pivot structure used in hierarchies
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct W3dPivotStruct {
    pub name: [u8; 16],
    pub parent_idx: u32,
    pub translation: W3dVectorStruct,
    pub euler_angles: W3dVectorStruct,
    pub rotation: W3dQuaternionStruct,
}

impl Default for W3dPivotStruct {
    fn default() -> Self {
        Self {
            name: [0; 16],
            parent_idx: 0xFFFFFFFF,
            translation: W3dVectorStruct::zero(),
            euler_angles: W3dVectorStruct::zero(),
            rotation: W3dQuaternionStruct::identity(),
        }
    }
}

// W3D texture attribute flags
pub const W3D_TEXTURE_PUBLISH: u16 = 0x0001;
pub const W3D_TEXTURE_RESIZE_OBSOLETE: u16 = 0x0002;
pub const W3D_TEXTURE_NO_LOD: u16 = 0x0004;
pub const W3D_TEXTURE_CLAMP_U: u16 = 0x0008;
pub const W3D_TEXTURE_CLAMP_V: u16 = 0x0010;
pub const W3D_TEXTURE_ALPHA_BITMAP: u16 = 0x0020;
pub const W3D_TEXTURE_MIP_LEVELS_MASK: u16 = 0x00c0;
pub const W3D_TEXTURE_MIP_LEVELS_ALL: u16 = 0x0000;
pub const W3D_TEXTURE_MIP_LEVELS_2: u16 = 0x0040;
pub const W3D_TEXTURE_MIP_LEVELS_3: u16 = 0x0080;
pub const W3D_TEXTURE_MIP_LEVELS_4: u16 = 0x00c0;

// W3D mesh flags
pub const W3D_MESH_FLAG_COLLISION_TYPE_MASK: u32 = 0x00000FF0;
pub const W3D_MESH_FLAG_COLLISION_TYPE_PHYSICAL: u32 = 0x00000010;
pub const W3D_MESH_FLAG_COLLISION_TYPE_PROJECTILE: u32 = 0x00000020;
pub const W3D_MESH_FLAG_COLLISION_TYPE_VIS: u32 = 0x00000040;
pub const W3D_MESH_FLAG_COLLISION_TYPE_CAMERA: u32 = 0x00000080;
pub const W3D_MESH_FLAG_COLLISION_TYPE_VEHICLE: u32 = 0x00000100;
pub const W3D_MESH_FLAG_HIDDEN: u32 = 0x00001000;
pub const W3D_MESH_FLAG_TWO_SIDED: u32 = 0x00002000;
pub const W3D_MESH_FLAG_CAST_SHADOW: u32 = 0x00008000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_MASK: u32 = 0x00FF0000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_NORMAL: u32 = 0x00000000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED: u32 = 0x00010000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN: u32 = 0x00020000;
pub const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ORIENTED: u32 = 0x00060000;
pub const W3D_MESH_FLAG_PRELIT_MASK: u32 = 0x0F000000;
pub const W3D_MESH_FLAG_PRELIT_UNLIT: u32 = 0x01000000;
pub const W3D_MESH_FLAG_PRELIT_VERTEX: u32 = 0x02000000;
pub const W3D_MESH_FLAG_PRELIT_LIGHTMAP_MULTI_PASS: u32 = 0x04000000;
pub const W3D_MESH_FLAG_PRELIT_LIGHTMAP_MULTI_TEXTURE: u32 = 0x08000000;
pub const W3D_MESH_FLAG_SHATTERABLE: u32 = 0x10000000;
pub const W3D_MESH_FLAG_NPATCHABLE: u32 = 0x20000000;

// W3D vertex channel flags
pub const W3D_VERTEX_CHANNEL_LOCATION: u32 = 0x00000001;
pub const W3D_VERTEX_CHANNEL_NORMAL: u32 = 0x00000002;
pub const W3D_VERTEX_CHANNEL_TEXCOORD: u32 = 0x00000004;
pub const W3D_VERTEX_CHANNEL_COLOR: u32 = 0x00000008;
pub const W3D_VERTEX_CHANNEL_BONEID: u32 = 0x00000010;

// W3D face channel flags
pub const W3D_FACE_CHANNEL_FACE: u32 = 0x00000001;
