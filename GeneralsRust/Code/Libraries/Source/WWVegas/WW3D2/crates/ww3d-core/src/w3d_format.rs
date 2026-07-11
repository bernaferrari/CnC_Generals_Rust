/// W3D File Format Data Structures
/// Ported from w3d_file.h with binary compatibility
use crate::W3DChunkType;
use binrw::{BinRead, BinWrite};
use glam::{Mat4, Quat, Vec3};

/// Length of fixed-size name fields used throughout the W3D format.
pub const W3D_NAME_LEN: usize = 16;
/// Maximum depth of mesh/bone paths stored in aggregate texture replacers.
pub const MESH_PATH_ENTRIES: usize = 15;
/// Length of an individual mesh/bone path entry (two name slots concatenated).
pub const MESH_PATH_ENTRY_LEN: usize = W3D_NAME_LEN * 2;

// Chunk header
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dChunkHeader {
    pub chunk_type: u32,
    pub chunk_size: u32,
}

impl W3dChunkHeader {
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

// Vector structure
#[derive(Debug, Clone, Copy, Default, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dVectorStruct {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<W3dVectorStruct> for Vec3 {
    fn from(v: W3dVectorStruct) -> Self {
        Vec3::new(v.x, v.y, v.z)
    }
}

impl From<Vec3> for W3dVectorStruct {
    fn from(v: Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

// Quaternion structure
#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dQuaternionStruct {
    pub w: f32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<W3dQuaternionStruct> for Quat {
    fn from(q: W3dQuaternionStruct) -> Self {
        Quat::from_xyzw(q.x, q.y, q.z, q.w)
    }
}

impl From<Quat> for W3dQuaternionStruct {
    fn from(q: Quat) -> Self {
        let (x, y, z, w) = q.into();
        Self { w, x, y, z }
    }
}

// Texture coordinate structure
#[derive(Debug, Clone, Copy, Default, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTexCoordStruct {
    pub u: f32,
    pub v: f32,
}

// Texture structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTextureStruct {
    pub name: [u8; 256], // Texture name as fixed-size array
    pub texture_info: W3dTextureInfoStruct,
}

// Texture attribute flags (mirrors original WW3D definitions)
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
pub const W3D_TEXTURE_HINT_SHIFT: u16 = 8;
pub const W3D_TEXTURE_HINT_MASK: u16 = 0x0000_ff00;
pub const W3D_TEXTURE_HINT_BASE: u16 = 0x0000;
pub const W3D_TEXTURE_HINT_EMISSIVE: u16 = 0x0100;
pub const W3D_TEXTURE_HINT_ENVIRONMENT: u16 = 0x0200;
pub const W3D_TEXTURE_HINT_SHINY_MASK: u16 = 0x0300;
pub const W3D_TEXTURE_TYPE_MASK: u16 = 0x1000;
pub const W3D_TEXTURE_TYPE_COLORMAP: u16 = 0x0000;
pub const W3D_TEXTURE_TYPE_BUMPMAP: u16 = 0x1000;

pub const W3D_TEXTURE_ANIM_LOOP: u16 = 0x0000;
pub const W3D_TEXTURE_ANIM_PINGPONG: u16 = 0x0001;
pub const W3D_TEXTURE_ANIM_ONCE: u16 = 0x0002;
pub const W3D_TEXTURE_ANIM_MANUAL: u16 = 0x0003;

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

pub const W3D_VERTEX_CHANNEL_LOCATION: u32 = 0x00000001;
pub const W3D_VERTEX_CHANNEL_NORMAL: u32 = 0x00000002;
pub const W3D_VERTEX_CHANNEL_TEXCOORD: u32 = 0x00000004;
pub const W3D_VERTEX_CHANNEL_COLOR: u32 = 0x00000008;
pub const W3D_VERTEX_CHANNEL_BONEID: u32 = 0x00000010;

pub const W3D_FACE_CHANNEL_FACE: u32 = 0x00000001;

// Texture info structure
#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTextureInfoStruct {
    pub attributes: u16,
    pub animation_type: u16,
    pub frame_count: u32,
    pub frame_rate: f32,
}

// RGB color structure (legacy format)
#[derive(Debug, Clone, Copy, Default, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dRGBStruct {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

// RGBA color structure
#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dRGBAStruct {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

// Triangle structure
#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTriangleStruct {
    pub vindex: [u32; 3],
    pub attributes: u32,
    pub normal: W3dVectorStruct,
    pub distance: f32,
}

// Mesh header v3
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
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

impl Default for W3dMeshHeader3Struct {
    fn default() -> Self {
        Self {
            version: 0,
            attrs: 0,
            mesh_name: [0; 16],
            container_name: [0; 16],
            num_tris: 0,
            num_verts: 0,
            num_materials: 0,
            num_damage_stages: 0,
            sort_level: 0,
            prelit_version: 0,
            future_counts: [0],
            vertex_channels: 0,
            face_channels: 0,
            bbox_min: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            bbox_max: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            sph_center: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            sph_radius: 0.0,
        }
    }
}

// Vertex influence structure
// CRITICAL: C++ uses single-bone-per-vertex skinning, NOT multi-bone!
// C++ structure: uint16 BoneIdx + uint8 Pad[6] = 8 bytes total
#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dVertInfStruct {
    pub bone_idx: u16, // Single bone index (C++ uses uint16)
    pub pad: [u8; 6],  // Padding for 8-byte alignment (MUST preserve for binary compat)
}

// Material info structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dMaterialInfoStruct {
    pub pass_count: u32,
    pub vert_matl_count: u32,
    pub shader_count: u32,
    pub texture_count: u32,
}

// Shader structure
#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
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
}

// Material pass structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dMaterialPassStruct {
    pub vm_id: u32,
    pub shader_id: u32,
    pub dcg: [u32; 3], // Diffuse Color Group
    pub dig: [u32; 3], // Diffuse Illumination Group
    pub scg: [u32; 3], // Specular Color Group
    pub texture_count: u32,
}

// Texture stage structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTextureStageStruct {
    pub tx_id: u32,
    pub per_face_tx_coord_id: u32,
    pub stage_tex_coord_id: u32,
}

// Vertex material name structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dVertexMaterialNameStruct {
    pub material_name: [u8; 256],
}

// Hierarchy header structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dHierarchyStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub num_pivots: u32,
    pub center: W3dVectorStruct,
}

impl Default for W3dHierarchyStruct {
    fn default() -> Self {
        Self {
            version: 0,
            name: [0; 16],
            num_pivots: 0,
            center: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
        }
    }
}

// Pivot structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dPivotStruct {
    pub name: [u8; 16],
    pub parent_idx: i32,
    pub translation: W3dVectorStruct,
    pub euler_angles: W3dVectorStruct,
}

// Animation header structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
#[derive(Default)]
pub struct W3dAnimationStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub hiera_name: [u8; 16],
    pub num_frames: u32,
    pub frame_rate: u32,
}

// Animation channel structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAnimChannelStruct {
    pub first_frame: u16,
    pub last_frame: u16,
    pub vector_len: u16,
    pub flags: u16,
    pub pivot: u16,
    pub pad: u16,
    #[br(ignore)] // Don't serialize/deserialize the runtime data field
    pub data: Option<Vec<f32>>, // Animation data (added for runtime use)
}

// Compressed animation header structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dCompressedAnimationStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub hiera_name: [u8; 16],
    pub num_frames: u32,
    pub frame_rate: u16,
    pub flavor: u16,
}

// HModel header structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dHModelHeaderStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub hierarchy_name: [u8; 16],
    pub num_connections: u32,
}

// HModel node structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dHModelNodeStruct {
    pub render_obj_name: [u8; 16],
    pub pivot_idx: u32,
}

// Light structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dLightStruct {
    pub attributes: u32,
    pub ambient: W3dRGBAStruct,
    pub diffuse: W3dRGBAStruct,
    pub specular: W3dRGBAStruct,
    pub intensity: f32,
}

// Light info structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dLightInfoStruct {
    pub spot_direction: W3dVectorStruct,
    pub spot_angle: f32,
    pub spot_exponent: f32,
}

// Emitter structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterStruct {
    pub version: u32,
    pub name: [u8; 16],
}

// Emitter info structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterInfoStruct {
    pub texture_filename: [u8; 260],
    pub start_size: f32,
    pub end_size: f32,
    pub lifetime: f32,
    pub emission_rate: f32,
    pub max_emissions: f32,
    pub velocity_random: f32,
    pub position_random: f32,
    pub fade_time: f32,
    pub gravity: f32,
    pub elasticity: f32,
    pub transparency: u8,
    pub particle_type: u8,
    pub burst_size: u16,
}

// Extended emitter structures from W3D format

// Emitter render mode constants
pub const W3D_EMITTER_RENDER_MODE_TRI_PARTICLES: u32 = 0;
pub const W3D_EMITTER_RENDER_MODE_QUAD_PARTICLES: u32 = 1;
pub const W3D_EMITTER_RENDER_MODE_LINE: u32 = 2;
pub const W3D_EMITTER_RENDER_MODE_LINEGRP_TETRA: u32 = 3;
pub const W3D_EMITTER_RENDER_MODE_LINEGRP_PRISM: u32 = 4;

// Emitter frame mode constants
pub const W3D_EMITTER_FRAME_MODE_1X1: u32 = 0;
pub const W3D_EMITTER_FRAME_MODE_2X2: u32 = 1;
pub const W3D_EMITTER_FRAME_MODE_4X4: u32 = 2;
pub const W3D_EMITTER_FRAME_MODE_8X8: u32 = 3;
pub const W3D_EMITTER_FRAME_MODE_16X16: u32 = 4;

// Emitter header structure (W3D_CHUNK_EMITTER_HEADER)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterHeaderStruct {
    pub version: u32,
    pub name: [u8; 16],
}

// Volume randomizer structure (used in emitter info v2)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dVolumeRandomizerStruct {
    pub class_id: u32,
    pub value1: f32,
    pub value2: f32,
    pub value3: f32,
    pub reserved: [u32; 4],
}

// Emitter info structure v2 (W3D_CHUNK_EMITTER_INFOV2)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterInfoStructV2 {
    pub burst_size: u32,
    pub creation_volume: W3dVolumeRandomizerStruct,
    pub velocity_random: W3dVolumeRandomizerStruct,
    pub outward_vel: f32,
    pub vel_inherit: f32,
    pub render_mode: u32, // W3D_EMITTER_RENDER_MODE_*
    pub frame_mode: u32,  // W3D_EMITTER_FRAME_MODE_*
    pub reserved: [u32; 6],
}

// Emitter property structure (W3D_CHUNK_EMITTER_PROPS)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterPropertyStruct {
    pub color_keyframes: u32,
    pub opacity_keyframes: u32,
    pub size_keyframes: u32,
    pub reserved: [u32; 5],
}

// Emitter keyframe structures
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterColorKeyframeStruct {
    pub time: f32,
    pub color: W3dRGBAStruct,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterOpacityKeyframeStruct {
    pub time: f32,
    pub opacity: f32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterSizeKeyframeStruct {
    pub time: f32,
    pub size: f32,
}

// Line properties structure (W3D_CHUNK_EMITTER_LINE_PROPERTIES)
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterLinePropertiesStruct {
    pub flags: u32,
    pub subdivision_level: u32,
    pub noise_amplitude: f32,
    pub merge_abort_factor: f32,
    pub texture_tile_factor: f32,
    pub u_per_sec: f32,
    pub v_per_sec: f32,
    pub reserved: [u32; 9],
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterRotationKeyframeStruct {
    pub time: f32,
    pub rotation: f32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterFrameKeyframeStruct {
    pub time: f32,
    pub frame: f32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterBlurTimeKeyframeStruct {
    pub time: f32,
    pub blur_time: f32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterUserDataStruct {
    pub type_id: u32,
    pub size: u32,
    #[br(count = size)]
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dEmitterExtraInfoStruct {
    pub reserved: [u32; 16],
}

// Aggregate header structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAggregateStruct {
    pub version: u32,
    pub name: [u8; 16],
}

#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAggregateInfoStruct {
    pub base_model_name: [u8; W3D_NAME_LEN * 2],
    pub subobject_count: u32,
}

#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAggregateSubobjectStruct {
    pub subobject_name: [u8; W3D_NAME_LEN * 2],
    pub bone_name: [u8; W3D_NAME_LEN * 2],
}

#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAggregateMiscInfo {
    pub original_class_id: u32,
    pub flags: u32,
    pub reserved: [u32; 3],
}

#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTextureReplacerHeaderStruct {
    pub replaced_textures_count: u32,
}

#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTextureReplacerStruct {
    pub mesh_path: [[u8; MESH_PATH_ENTRY_LEN]; MESH_PATH_ENTRIES],
    pub bone_path: [[u8; MESH_PATH_ENTRY_LEN]; MESH_PATH_ENTRIES],
    pub old_texture_name: [u8; 260],
    pub new_texture_name: [u8; 260],
    pub texture_params: W3dTextureInfoStruct,
}

// LOD model structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dLodModelHeaderStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub num_lods: u16,
    pub pad: u16,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dLodStruct {
    pub render_obj_name: [u8; 32],
    pub lod_min: f32,
    pub lod_max: f32,
}

// Collection structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dCollectionHeaderStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub render_object_count: u32,
    pub pad: [u32; 2],
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dPlaceholderStruct {
    pub version: u32,
    pub transform: [[f32; 4]; 3],
    pub name_len: u32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTransformNodeStruct {
    pub version: u32,
    pub transform: [[f32; 4]; 3],
    pub name_len: u32,
}

// Box object structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dBoxStruct {
    pub version: u32,
    pub attributes: u32,
    pub name: [u8; 32],
    pub color: W3dRGBAStruct,
    pub center: W3dVectorStruct,
    pub extent: W3dVectorStruct,
}

// Sphere object structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dSphereStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub color: W3dRGBAStruct,
    pub center: W3dVectorStruct,
    pub radius: f32,
}

// Ring object structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dRingStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub color: W3dRGBAStruct,
    pub center: W3dVectorStruct,
    pub inner_radius: f32,
    pub outer_radius: f32,
}

// Null object structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dNullObjectStruct {
    pub version: u32,
    pub attributes: u32,
    pub pad: [u32; 2],
    pub name: [u8; 32],
}

// Dazzle structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dDazzleStruct {
    pub name: [u8; 32],
    pub type_name: [u8; 32],
}

// Sound render object header
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dSoundRObjHeaderStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub flags: u32,
    pub padding: [u32; 8],
}

// Sound object structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dSoundObjStruct {
    pub name: [u8; 16],
    pub filename: [u8; 16],
    pub volume: f32,
    pub dropoff_radius: f32,
    pub priority: u32,
}

// Morph animation structures
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dMorphAnimStruct {
    pub name: [u8; 16],
    pub hiera_name: [u8; 16],
    pub frame_count: u32,
    pub morph_count: u32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dMorphAnimChannelStruct {
    pub morph_name: [u8; 32],
    pub pivot_idx: u32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dMorphAnimKeyframeStruct {
    pub morph_pos: W3dVectorStruct,
}

// HLOD structures
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dHlodStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub hierarchy_name: [u8; 16],
    pub num_lods: u32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dHlodLodArrayStruct {
    pub model_count: u32,
    pub max_screen_size: f32,
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dHlodSubObjectStruct {
    pub name: [u8; 32],
    pub bone_index: u32,
}

// Helper functions for string conversion
pub fn w3d_string_from_bytes(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).to_string()
}

pub fn w3d_string_to_bytes(s: &str, len: usize) -> Vec<u8> {
    let mut bytes = s.as_bytes().to_vec();
    bytes.resize(len, 0);
    bytes
}

// Helper for matrix conversion
pub fn matrix3x4_to_mat4(m: [[f32; 4]; 3]) -> Mat4 {
    Mat4::from_cols(
        Vec3::new(m[0][0], m[1][0], m[2][0]).extend(0.0),
        Vec3::new(m[0][1], m[1][1], m[2][1]).extend(0.0),
        Vec3::new(m[0][2], m[1][2], m[2][2]).extend(0.0),
        Vec3::new(m[0][3], m[1][3], m[2][3]).extend(1.0),
    )
}

pub fn mat4_to_matrix3x4(m: Mat4) -> [[f32; 4]; 3] {
    let cols = m.to_cols_array_2d();
    [
        [cols[0][0], cols[1][0], cols[2][0], cols[3][0]],
        [cols[0][1], cols[1][1], cols[2][1], cols[3][1]],
        [cols[0][2], cols[1][2], cols[2][2], cols[3][2]],
    ]
}

// Vertex material structure
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dVertexMaterialStruct {
    pub attributes: u32,
    pub ambient: W3dRGBAStruct,
    pub diffuse: W3dRGBAStruct,
    pub specular: W3dRGBAStruct,
    pub emissive: W3dRGBAStruct,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
}

// W3D Mesh structure - complete mesh data
#[derive(Debug, Clone)]
#[repr(C)]
pub struct W3dMesh {
    pub header: W3dMeshHeader3Struct,
    pub vertices: Vec<W3dVectorStruct>,
    pub normals: Vec<W3dVectorStruct>,
    pub triangles: Vec<W3dTriangleStruct>,
    pub materials: Vec<W3dVertexMaterialStruct>,
    pub shaders: Vec<W3dShaderStruct>,
    pub texture_coords: Vec<W3dTexCoordStruct>,
    pub material_info: Option<W3dMaterialInfoStruct>,
    /// Vertex bone influences for skinned meshes
    /// C++ Reference: W3D_CHUNK_VERTEX_INFLUENCES
    pub vertex_influences: Vec<W3dVertInfStruct>,
    /// Texture information
    /// C++ Reference: W3D_CHUNK_TEXTURES
    pub textures: Vec<W3dTextureStruct>,
    /// Material pass information for multi-pass rendering
    /// C++ Reference: W3D_CHUNK_MATERIAL_PASS
    pub material_pass: Option<W3dMaterialPassStruct>,
}

impl Default for W3dMesh {
    fn default() -> Self {
        Self::new()
    }
}

impl W3dMesh {
    pub fn new() -> Self {
        Self {
            header: W3dMeshHeader3Struct::default(),
            vertices: Vec::new(),
            normals: Vec::new(),
            triangles: Vec::new(),
            materials: Vec::new(),
            shaders: Vec::new(),
            texture_coords: Vec::new(),
            material_info: None,
            vertex_influences: Vec::new(),
            textures: Vec::new(),
            material_pass: None,
        }
    }
}

// W3D Hierarchy structure - bone/skeleton data
#[derive(Debug, Clone)]
#[repr(C)]
pub struct W3dHierarchy {
    pub header: W3dHierarchyStruct,
    pub pivots: Vec<W3dPivotStruct>,
}

impl Default for W3dHierarchy {
    fn default() -> Self {
        Self::new()
    }
}

impl W3dHierarchy {
    pub fn new() -> Self {
        Self {
            header: W3dHierarchyStruct::default(),
            pivots: Vec::new(),
        }
    }
}

// W3D Animation structure - keyframe animation data
#[derive(Debug, Clone)]
#[repr(C)]
pub struct W3dAnimation {
    pub header: W3dAnimationStruct,
    pub channels: Vec<W3dAnimChannelStruct>,
}

impl Default for W3dAnimation {
    fn default() -> Self {
        Self::new()
    }
}

impl W3dAnimation {
    pub fn new() -> Self {
        Self {
            header: W3dAnimationStruct::default(),
            channels: Vec::new(),
        }
    }
}

// Helper methods for string conversion
impl W3dMeshHeader3Struct {
    pub fn mesh_name_str(&self) -> String {
        String::from_utf8_lossy(&self.mesh_name)
            .trim_end_matches('\0')
            .to_string()
    }

    pub fn container_name_str(&self) -> String {
        String::from_utf8_lossy(&self.container_name)
            .trim_end_matches('\0')
            .to_string()
    }
}

impl W3dHierarchyStruct {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_string()
    }
}

impl W3dPivotStruct {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_string()
    }

    pub fn base_transform(&self) -> Mat4 {
        // Create transform from translation and euler angles
        let translation_vec = Vec3::new(self.translation.x, self.translation.y, self.translation.z);
        let translation = Mat4::from_translation(translation_vec);

        // Create rotation from euler angles (XYZ order)
        let rotation = Mat4::from_euler(
            glam::EulerRot::XYZ,
            self.euler_angles.x,
            self.euler_angles.y,
            self.euler_angles.z,
        );

        translation * rotation
    }
}

impl W3dAnimationStruct {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_string()
    }

    pub fn hiera_name_str(&self) -> String {
        String::from_utf8_lossy(&self.hiera_name)
            .trim_end_matches('\0')
            .to_string()
    }
}

impl W3dHModelHeaderStruct {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_string()
    }

    pub fn hierarchy_name_str(&self) -> String {
        String::from_utf8_lossy(&self.hierarchy_name)
            .trim_end_matches('\0')
            .to_string()
    }
}

impl W3dHModelNodeStruct {
    pub fn render_obj_name_str(&self) -> String {
        String::from_utf8_lossy(&self.render_obj_name)
            .trim_end_matches('\0')
            .to_string()
    }
}

impl W3dBoxStruct {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_string()
    }
}

impl W3dSphereStruct {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_string()
    }
}

impl W3dNullObjectStruct {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name)
            .trim_end_matches('\0')
            .to_string()
    }
}

// AABTree structures
#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAABTreeHeader {
    pub node_count: u32,
    pub poly_count: u32,
    pub padding: [u32; 6],
}

impl W3dAABTreeHeader {
    /// Validate AABTree header consistency with C++ format
    /// The padding fields should be zero in valid W3D files
    pub fn validate(&self) -> Result<(), String> {
        // Validate node count is reasonable (not zero, not excessively large)
        if self.node_count == 0 {
            return Err("AABTree node_count is zero".to_string());
        }
        if self.node_count > 1_000_000 {
            return Err(format!(
                "AABTree node_count suspiciously large: {}",
                self.node_count
            ));
        }

        // Validate poly count is reasonable
        if self.poly_count == 0 {
            return Err("AABTree poly_count is zero".to_string());
        }
        if self.poly_count > 10_000_000 {
            return Err(format!(
                "AABTree poly_count suspiciously large: {}",
                self.poly_count
            ));
        }

        // Validate padding is zero (standard format)
        for (i, &pad) in self.padding.iter().enumerate() {
            if pad != 0 {
                return Err(format!("AABTree padding[{}] is non-zero: {}", i, pad));
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAABTreeNode {
    pub min: W3dVectorStruct,
    pub max: W3dVectorStruct,
    pub front_or_poly0: u32,
    pub back_or_poly_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w3d_aabtree_header_matches_expected_binary_size() {
        assert_eq!(std::mem::size_of::<W3dAABTreeHeader>(), 32);
    }
}

// Note: Conversion to runtime AABTreeNode (ww3d-collision crate) happens during
// mesh loading. W3dAABTreeNode is the serialized file format representation.
// C++ equivalent: W3D_AABTREE_NODE struct converted to AABTreeNode class

// ============================================================================
// Animation Structures - W3D Format
// ============================================================================
// These structures support loading W3D animation files with compressed channels.
// Reference: C++ w3d_file.h animation chunk definitions

/// Raw uncompressed animation header (W3D_CHUNK_ANIMATION_HEADER)
/// Used for HRAW (uncompressed) animations
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAnimHeaderStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub hierarchy_name: [u8; 16],
    pub num_frames: u32,
    pub frame_rate: u32,
}

/// Compressed animation header (W3D_CHUNK_COMPRESSED_ANIMATION_HEADER)
/// Used for HCMP (compressed) animations with time-coded or adaptive delta channels
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dCompressedAnimHeaderStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub hierarchy_name: [u8; 16],
    pub num_frames: u32,
    pub frame_rate: u16,
    pub flavor: u16, // ANIM_FLAVOR_TIMECODED or ANIM_FLAVOR_ADAPTIVE_DELTA
}

/// Time-coded animation channel header (W3D_CHUNK_TIMECODED_CHANNEL)
/// Stores keyframes with time codes for efficient sparse animation
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTimeCodedAnimChannelStruct {
    pub num_time_codes: u32,
    pub pivot: u16,
    pub vector_len: u8,
    pub flags: u8,
    /// First data element - actual data follows in memory
    /// Format: [timecode: u32, values: f32[vector_len]] repeated num_time_codes times
    pub data: [u32; 1],
}

/// Adaptive delta animation channel header (W3D_CHUNK_ADAPTIVE_DELTA_CHANNEL)
/// Uses delta compression with adaptive scaling for high compression ratios
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dAdaptiveDeltaAnimChannelStruct {
    pub num_frames: u32,
    pub pivot: u16,
    pub vector_len: u8,
    pub flags: u8,
    pub scale: f32,
}

/// Bit channel for visibility data (W3D_CHUNK_BIT_CHANNEL)
/// Stores boolean visibility state per frame
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dBitChannelStruct {
    pub first_frame: u16,
    pub last_frame: u16,
    pub flags: u16,
    pub pivot: u16,
    pub default_val: u8,
    /// First data byte - actual bit data follows
    pub data: [u8; 1],
}

/// Time-coded bit channel for compressed visibility (W3D_CHUNK_TIMECODED_BIT_CHANNEL)
/// Stores visibility changes only at keyframes
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dTimeCodedBitChannelStruct {
    pub num_time_codes: u32,
    pub pivot: u16,
    pub flags: u8,
    pub default_val: u8,
    /// Time codes with packed bit values
    /// Format: Each u32 contains [31 bits: timecode][1 bit: visibility (MSB)]
    pub data: [u32; 1],
}

/// Morph animation header (W3D_CHUNK_MORPH_ANIMATION_HEADER)
/// Used for facial animations and mesh morphing
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dMorphAnimHeaderStruct {
    pub version: u32,
    pub name: [u8; 16],
    pub hierarchy_name: [u8; 16],
    pub frame_count: u32,
    pub frame_rate: f32,
    pub channel_count: u32,
}

/// Morph animation keyframe (W3D_CHUNK_MORPH_ANIMATION_KEY)
/// Maps morph frames to pose frames for time-coded morph blending
#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(little)]
#[repr(C)]
pub struct W3dMorphAnimKeyStruct {
    pub morph_frame: u32,
    pub pose_frame: u32,
}

// Animation flavor constants - compression types
pub const ANIM_FLAVOR_TIMECODED: u32 = 0;
pub const ANIM_FLAVOR_ADAPTIVE_DELTA: u32 = 1;

// Channel type constants - what the channel animates
pub const ANIM_CHANNEL_X: u16 = 0;
pub const ANIM_CHANNEL_Y: u16 = 1;
pub const ANIM_CHANNEL_Z: u16 = 2;
pub const ANIM_CHANNEL_XR: u16 = 3; // X rotation (Euler)
pub const ANIM_CHANNEL_YR: u16 = 4; // Y rotation (Euler)
pub const ANIM_CHANNEL_ZR: u16 = 5; // Z rotation (Euler)
pub const ANIM_CHANNEL_Q: u16 = 6; // Quaternion rotation

// Time-coded channel variants
pub const ANIM_CHANNEL_TIMECODED_X: u16 = 7;
pub const ANIM_CHANNEL_TIMECODED_Y: u16 = 8;
pub const ANIM_CHANNEL_TIMECODED_Z: u16 = 9;
pub const ANIM_CHANNEL_TIMECODED_Q: u16 = 10;

// Adaptive delta channel variants
pub const ANIM_CHANNEL_ADAPTIVE_DELTA_X: u16 = 11;
pub const ANIM_CHANNEL_ADAPTIVE_DELTA_Y: u16 = 12;
pub const ANIM_CHANNEL_ADAPTIVE_DELTA_Z: u16 = 13;
pub const ANIM_CHANNEL_ADAPTIVE_DELTA_Q: u16 = 14;

// Bit channel types for visibility
pub const BIT_CHANNEL_VIS: u16 = 0;
pub const BIT_CHANNEL_TIMECODED_VIS: u16 = 1;

// Time-coded binary movement flag (MSB of timecode)
// When set, indicates no interpolation should occur - use nearest keyframe
pub const W3D_TIMECODED_BINARY_MOVEMENT_FLAG: u32 = 0x80000000;

#[cfg(test)]
mod parity_tests {
    use super::*;
    use std::mem::{offset_of, size_of};

    #[test]
    fn verify_aabtree_header_parity() {
        // C++: sizeof(W3dMeshAABTreeHeader) == 32
        assert_eq!(size_of::<W3dAABTreeHeader>(), 32);
        assert_eq!(offset_of!(W3dAABTreeHeader, node_count), 0);
        assert_eq!(offset_of!(W3dAABTreeHeader, poly_count), 4);
        assert_eq!(offset_of!(W3dAABTreeHeader, padding), 8);
    }

    #[test]
    fn verify_aabtree_node_parity() {
        // C++: sizeof(W3dMeshAABTreeNode) == 32
        assert_eq!(size_of::<W3dAABTreeNode>(), 32);
        assert_eq!(offset_of!(W3dAABTreeNode, min), 0);
        assert_eq!(offset_of!(W3dAABTreeNode, max), 12);
        assert_eq!(offset_of!(W3dAABTreeNode, front_or_poly0), 24);
        assert_eq!(offset_of!(W3dAABTreeNode, back_or_poly_count), 28);
    }

    #[test]
    fn verify_mesh_header3_parity() {
        // C++: sizeof(W3dMeshHeader3Struct) == 116
        assert_eq!(size_of::<W3dMeshHeader3Struct>(), 116);
    }

    #[test]
    fn verify_aggregate_header_parity() {
        // C++: sizeof(W3dAggregateHeaderStruct) == 20
        assert_eq!(size_of::<W3dAggregateStruct>(), 20);
    }

    #[test]
    fn verify_aggregate_info_parity() {
        // C++: sizeof(W3dAggregateInfoStruct) == 36
        assert_eq!(size_of::<W3dAggregateInfoStruct>(), 36);
    }

    #[test]
    fn verify_aggregate_subobj_parity() {
        // C++: sizeof(W3dAggregateSubobjectStruct) == 64
        assert_eq!(size_of::<W3dAggregateSubobjectStruct>(), 64);
    }

    #[test]
    fn verify_texture_replacer_parity() {
        // C++: sizeof(W3dTextureReplacerStruct) == 1492 (approx calculation)
        // MeshPath: 15 * 32 = 480
        // BonePath: 15 * 32 = 480
        // Names: 260 + 260 = 520
        // Params: 12
        // Total: 480 + 480 + 520 + 12 = 1492
        assert_eq!(size_of::<W3dTextureReplacerStruct>(), 1492);
    }

    #[test]
    fn verify_texture_info_parity() {
        // C++: sizeof(W3dTextureInfoStruct) == 12
        assert_eq!(size_of::<W3dTextureInfoStruct>(), 12);
    }
}
