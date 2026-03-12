//! Obsolete W3D chunk IDs and legacy structs (ported from w3d_obsolete.h).

use crate::w3d_format::{W3dRGBStruct, W3dTexCoordStruct, W3dVectorStruct, W3D_NAME_LEN};

pub const W3D_CHUNK_MESH_HEADER: u32 = 0x00000001;
pub const W3D_CHUNK_SURRENDER_NORMALS: u32 = 0x00000004;
pub const W3D_CHUNK_TEXCOORDS: u32 = 0x00000005;
pub const O_W3D_CHUNK_MATERIALS: u32 = 0x00000006;
pub const O_W3D_CHUNK_TRIANGLES: u32 = 0x00000007;
pub const O_W3D_CHUNK_QUADRANGLES: u32 = 0x00000008;
pub const O_W3D_CHUNK_SURRENDER_TRIANGLES: u32 = 0x00000009;
pub const O_W3D_CHUNK_POV_TRIANGLES: u32 = 0x0000000A;
pub const O_W3D_CHUNK_POV_QUADRANGLES: u32 = 0x0000000B;
pub const W3D_CHUNK_VERTEX_COLORS: u32 = 0x0000000D;
pub const W3D_CHUNK_DAMAGE: u32 = 0x0000000F;
pub const W3D_CHUNK_DAMAGE_HEADER: u32 = 0x00000010;
pub const W3D_CHUNK_DAMAGE_VERTICES: u32 = 0x00000011;
pub const W3D_CHUNK_DAMAGE_COLORS: u32 = 0x00000012;
pub const W3D_CHUNK_DAMAGE_MATERIALS: u32 = 0x00000013;
pub const O_W3D_CHUNK_MATERIALS2: u32 = 0x00000014;
pub const W3D_CHUNK_MATERIALS3: u32 = 0x00000015;
pub const W3D_CHUNK_MATERIAL3: u32 = 0x00000016;
pub const W3D_CHUNK_MATERIAL3_NAME: u32 = 0x00000017;
pub const W3D_CHUNK_MATERIAL3_INFO: u32 = 0x00000018;
pub const W3D_CHUNK_MATERIAL3_DC_MAP: u32 = 0x00000019;
pub const W3D_CHUNK_MAP3_FILENAME: u32 = 0x0000001A;
pub const W3D_CHUNK_MAP3_INFO: u32 = 0x0000001B;
pub const W3D_CHUNK_MATERIAL3_DI_MAP: u32 = 0x0000001C;
pub const W3D_CHUNK_MATERIAL3_SC_MAP: u32 = 0x0000001D;
pub const W3D_CHUNK_MATERIAL3_SI_MAP: u32 = 0x0000001E;
pub const W3D_CHUNK_PER_TRI_MATERIALS: u32 = 0x00000021;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct W3dMaterialStruct {
    pub material_name: [u8; W3D_NAME_LEN],
    pub primary_name: [u8; W3D_NAME_LEN],
    pub secondary_name: [u8; W3D_NAME_LEN],
    pub render_flags: u32,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct W3dMaterial2Struct {
    pub material_name: [u8; W3D_NAME_LEN],
    pub primary_name: [u8; W3D_NAME_LEN],
    pub secondary_name: [u8; W3D_NAME_LEN],
    pub render_flags: u32,
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
    pub primary_num_frames: u16,
    pub secondary_num_frames: u16,
    pub pad: [u8; 12],
}

pub const W3DMATERIAL_USE_ALPHA: u32 = 0x00000001;
pub const W3DMATERIAL_USE_SORTING: u32 = 0x00000002;
pub const W3DMATERIAL_HINT_DIT_OVER_DCT: u32 = 0x00000010;
pub const W3DMATERIAL_HINT_SIT_OVER_SCT: u32 = 0x00000020;
pub const W3DMATERIAL_HINT_DIT_OVER_DIG: u32 = 0x00000040;
pub const W3DMATERIAL_HINT_SIT_OVER_SIG: u32 = 0x00000080;
pub const W3DMATERIAL_HINT_FAST_SPECULAR_AFTER_ALPHA: u32 = 0x00000100;
pub const W3DMATERIAL_PSX_MASK: u32 = 0xFF000000;
pub const W3DMATERIAL_PSX_TRANS_MASK: u32 = 0x07000000;
pub const W3DMATERIAL_PSX_TRANS_NONE: u32 = 0x00000000;
pub const W3DMATERIAL_PSX_TRANS_100: u32 = 0x01000000;
pub const W3DMATERIAL_PSX_TRANS_50: u32 = 0x02000000;
pub const W3DMATERIAL_PSX_TRANS_25: u32 = 0x03000000;
pub const W3DMATERIAL_PSX_TRANS_MINUS_100: u32 = 0x04000000;
pub const W3DMATERIAL_PSX_NO_RT_LIGHTING: u32 = 0x08000000;

pub const W3DMAPPING_UV: u16 = 0;
pub const W3DMAPPING_ENVIRONMENT: u16 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct W3dMaterial3Struct {
    pub attributes: u32,
    pub diffuse_color: W3dRGBStruct,
    pub specular_color: W3dRGBStruct,
    pub emissive_coefficients: W3dRGBStruct,
    pub ambient_coefficients: W3dRGBStruct,
    pub diffuse_coefficients: W3dRGBStruct,
    pub specular_coefficients: W3dRGBStruct,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
    pub fog_coeff: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct W3dMap3Struct {
    pub mapping_type: u16,
    pub frame_count: u16,
    pub frame_rate: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct W3dSurrenderTriStruct {
    pub vindex: [u32; 3],
    pub tex_coord: [W3dTexCoordStruct; 3],
    pub material_idx: u32,
    pub normal: W3dVectorStruct,
    pub attributes: u32,
    pub gouraud: [W3dRGBStruct; 3],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct W3dMeshHeaderStruct {
    pub version: u32,
    pub mesh_name: [u8; W3D_NAME_LEN],
    pub attributes: u32,
    pub num_tris: u32,
    pub num_quads: u32,
    pub num_sr_tris: u32,
    pub num_pov_tris: u32,
    pub num_pov_quads: u32,
    pub num_vertices: u32,
    pub num_normals: u32,
    pub num_sr_normals: u32,
    pub num_tex_coords: u32,
    pub num_materials: u32,
    pub num_vert_colors: u32,
    pub num_vert_influences: u32,
    pub num_damage_stages: u32,
    pub future_counts: [u32; 5],
    pub lod_min: f32,
    pub lod_max: f32,
    pub min: W3dVectorStruct,
    pub max: W3dVectorStruct,
    pub sph_center: W3dVectorStruct,
    pub sph_radius: f32,
    pub translation: W3dVectorStruct,
    pub rotation: [f32; 9],
    pub mass_center: W3dVectorStruct,
    pub inertia: [f32; 9],
    pub volume: f32,
    pub hierarchy_tree_name: [u8; W3D_NAME_LEN],
}
