// W3D File Format Definitions
// Ported from w3d_file.h

use std::io::{Read, Write};
use bytemuck::{Pod, Zeroable};

pub const W3D_NAME_LEN: usize = 16;

pub fn w3d_make_version(major: u16, minor: u16) -> u32 {
    ((major as u32) << 16) | (minor as u32)
}

pub fn w3d_get_major_version(ver: u32) -> u16 {
    (ver >> 16) as u16
}

pub fn w3d_get_minor_version(ver: u32) -> u16 {
    (ver & 0xFFFF) as u16
}

// Chunk Types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum W3DChunkType {
    // Mesh definition
    Mesh = 0x00000000,
    Vertices = 0x00000002,
    VertexNormals = 0x00000003,
    MeshUserText = 0x0000000C,
    VertexInfluences = 0x0000000E,
    MeshHeader3 = 0x0000001F,
    Triangles = 0x00000020,
    VertexShadeIndices = 0x00000022,

    // Prelit material chunk wrappers
    PrelitUnlit = 0x00000023,
    PrelitVertex = 0x00000024,
    PrelitLightmapMultiPass = 0x00000025,
    PrelitLightmapMultiTexture = 0x00000026,

    // Material information
    MaterialInfo = 0x00000028,
    Shaders = 0x00000029,

    // Vertex materials
    VertexMaterials = 0x0000002A,
    VertexMaterial = 0x0000002B,
    VertexMaterialName = 0x0000002C,
    VertexMaterialInfo = 0x0000002D,
    VertexMapperArgs0 = 0x0000002E,
    VertexMapperArgs1 = 0x0000002F,

    // Textures
    Textures = 0x00000030,
    Texture = 0x00000031,
    TextureName = 0x00000032,
    TextureInfo = 0x00000033,

    // Material pass
    MaterialPass = 0x00000038,
    VertexMaterialIds = 0x00000039,
    ShaderIds = 0x0000003A,
    DCG = 0x0000003B,
    DIG = 0x0000003C,
    SCG = 0x0000003E,

    // Texture stage
    TextureStage = 0x00000048,
    TextureIds = 0x00000049,
    StageTexcoords = 0x0000004A,
    PerFaceTexcoordIds = 0x0000004B,

    // Deformation
    Deform = 0x00000058,
    DeformSet = 0x00000059,
    DeformKeyframe = 0x0000005A,
    DeformData = 0x0000005B,

    // PS2 shaders
    PS2Shaders = 0x00000080,

    // AABTree
    AABTree = 0x00000090,
    AABTreeHeader = 0x00000091,
    AABTreePolyIndices = 0x00000092,
    AABTreeNodes = 0x00000093,

    // Hierarchy
    Hierarchy = 0x00000100,
    HierarchyHeader = 0x00000101,
    Pivots = 0x00000102,
    PivotFixups = 0x00000103,

    // Animation
    Animation = 0x00000200,
    AnimationHeader = 0x00000201,
    AnimationChannel = 0x00000202,
    BitChannel = 0x00000203,

    // Compressed animation
    CompressedAnimation = 0x00000280,
    CompressedAnimationHeader = 0x00000281,
    CompressedAnimationChannel = 0x00000282,
    CompressedBitChannel = 0x00000283,

    // Morph animation
    MorphAnimation = 0x000002C0,
    MorphAnimHeader = 0x000002C1,
    MorphAnimChannel = 0x000002C2,
    MorphAnimPoseName = 0x000002C3,
    MorphAnimKeyData = 0x000002C4,
    MorphAnimPivotChannelData = 0x000002C5,

    // HModel
    HModel = 0x00000300,
    HModelHeader = 0x00000301,
    Node = 0x00000302,
    CollisionNode = 0x00000303,
    SkinNode = 0x00000304,

    // LOD Model
    LODModel = 0x00000400,
    LODModelHeader = 0x00000401,
    LOD = 0x00000402,

    // Collection
    Collection = 0x00000420,
    CollectionHeader = 0x00000421,
    CollectionObjName = 0x00000422,
    Placeholder = 0x00000423,
    TransformNode = 0x00000424,

    // Points
    Points = 0x00000440,

    // Light
    Light = 0x00000460,
    LightInfo = 0x00000461,
    SpotLightInfo = 0x00000462,
    NearAttenuation = 0x00000463,
    FarAttenuation = 0x00000464,

    // Emitter
    Emitter = 0x00000500,
    EmitterHeader = 0x00000501,
    EmitterUserData = 0x00000502,
    EmitterInfo = 0x00000503,
    EmitterInfoV2 = 0x00000504,
    EmitterProps = 0x00000505,
    EmitterLineProperties = 0x00000509,
    EmitterRotationKeyframes = 0x0000050A,
    EmitterFrameKeyframes = 0x0000050B,
    EmitterBlurTimeKeyframes = 0x0000050C,
    EmitterExtraInfo = 0x0000050D,

    // Aggregate
    Aggregate = 0x00000600,
    AggregateHeader = 0x00000601,
    AggregateInfo = 0x00000602,
    TextureReplacerInfo = 0x00000603,
    AggregateClassInfo = 0x00000604,

    // HLod
    HLod = 0x00000700,
    HLodHeader = 0x00000701,
    HLodLodArray = 0x00000702,
    HLodSubObjectArrayHeader = 0x00000703,
    HLodSubObject = 0x00000704,
    HLodAggregateArray = 0x00000705,
    HLodProxyArray = 0x00000706,

    // Primitives
    Box = 0x00000740,
    Sphere = 0x00000741,
    Ring = 0x00000742,
    NullObject = 0x00000750,

    // Lightscape
    Lightscape = 0x00000800,
    LightscapeLight = 0x00000801,
    LightTransform = 0x00000802,

    // Dazzle
    Dazzle = 0x00000900,
    DazzleName = 0x00000901,
    DazzleTypeName = 0x00000902,

    // Sound render object
    SoundRObj = 0x00000A00,
    SoundRObjHeader = 0x00000A01,
    SoundRObjDefinition = 0x00000A02,
}

impl W3DChunkType {
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0x00000000 => Some(Self::Mesh),
            0x00000002 => Some(Self::Vertices),
            0x00000003 => Some(Self::VertexNormals),
            0x0000000C => Some(Self::MeshUserText),
            0x0000000E => Some(Self::VertexInfluences),
            0x0000001F => Some(Self::MeshHeader3),
            0x00000020 => Some(Self::Triangles),
            0x00000022 => Some(Self::VertexShadeIndices),
            0x00000028 => Some(Self::MaterialInfo),
            0x00000029 => Some(Self::Shaders),
            0x0000002A => Some(Self::VertexMaterials),
            0x0000002B => Some(Self::VertexMaterial),
            0x0000002C => Some(Self::VertexMaterialName),
            0x0000002D => Some(Self::VertexMaterialInfo),
            0x00000030 => Some(Self::Textures),
            0x00000031 => Some(Self::Texture),
            0x00000032 => Some(Self::TextureName),
            0x00000033 => Some(Self::TextureInfo),
            0x00000038 => Some(Self::MaterialPass),
            0x00000039 => Some(Self::VertexMaterialIds),
            0x0000003A => Some(Self::ShaderIds),
            0x00000100 => Some(Self::Hierarchy),
            0x00000101 => Some(Self::HierarchyHeader),
            0x00000102 => Some(Self::Pivots),
            0x00000200 => Some(Self::Animation),
            0x00000201 => Some(Self::AnimationHeader),
            0x00000202 => Some(Self::AnimationChannel),
            0x00000300 => Some(Self::HModel),
            0x00000700 => Some(Self::HLod),
            _ => None,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DChunkHeader {
    pub chunk_type: u32,
    pub chunk_size: u32,
}

impl W3DChunkHeader {
    pub fn read<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut buf = [0u8; 8];
        reader.read_exact(&mut buf)?;
        Ok(Self {
            chunk_type: u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            chunk_size: u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
        })
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.chunk_type.to_le_bytes())?;
        writer.write_all(&self.chunk_size.to_le_bytes())?;
        Ok(())
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DVector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DQuaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DTexCoord {
    pub u: f32,
    pub v: f32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DRGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub pad: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DRGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DMaterialInfo {
    pub pass_count: u32,
    pub vertex_material_count: u32,
    pub shader_count: u32,
    pub texture_count: u32,
}

// Vertex Material Attributes
pub mod vertex_material_attributes {
    pub const USE_DEPTH_CUE: u32 = 0x00000001;
    pub const ARGB_EMISSIVE_ONLY: u32 = 0x00000002;
    pub const COPY_SPECULAR_TO_DIFFUSE: u32 = 0x00000004;
    pub const DEPTH_CUE_TO_ALPHA: u32 = 0x00000008;

    pub const STAGE0_MAPPING_MASK: u32 = 0x00FF0000;
    pub const STAGE0_MAPPING_UV: u32 = 0x00000000;
    pub const STAGE0_MAPPING_ENVIRONMENT: u32 = 0x00010000;
    pub const STAGE0_MAPPING_CHEAP_ENVIRONMENT: u32 = 0x00020000;
    pub const STAGE0_MAPPING_SCREEN: u32 = 0x00030000;
    pub const STAGE0_MAPPING_LINEAR_OFFSET: u32 = 0x00040000;

    pub const STAGE1_MAPPING_MASK: u32 = 0x0000FF00;
    pub const STAGE1_MAPPING_UV: u32 = 0x00000000;
    pub const STAGE1_MAPPING_ENVIRONMENT: u32 = 0x00000100;
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DVertexMaterial {
    pub attributes: u32,
    pub ambient: W3DRGB,
    pub diffuse: W3DRGB,
    pub specular: W3DRGB,
    pub emissive: W3DRGB,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
}

impl Default for W3DVertexMaterial {
    fn default() -> Self {
        Self {
            attributes: 0,
            ambient: W3DRGB { r: 255, g: 255, b: 255, pad: 0 },
            diffuse: W3DRGB { r: 255, g: 255, b: 255, pad: 0 },
            specular: W3DRGB { r: 0, g: 0, b: 0, pad: 0 },
            emissive: W3DRGB { r: 0, g: 0, b: 0, pad: 0 },
            shininess: 1.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }
}

// Shader Enums
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DShaderDepthCompare {
    PassNever = 0,
    PassLess = 1,
    PassEqual = 2,
    PassLEqual = 3,
    PassGreater = 4,
    PassNotEqual = 5,
    PassGEqual = 6,
    PassAlways = 7,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DShaderDepthMask {
    WriteDisable = 0,
    WriteEnable = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DShaderAlphaTest {
    Disable = 0,
    Enable = 1,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DShaderDestBlendFunc {
    Zero = 0,
    One = 1,
    SrcColor = 2,
    OneMinusSrcColor = 3,
    SrcAlpha = 4,
    OneMinusSrcAlpha = 5,
    SrcColorPrefog = 6,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DShaderPriGradient {
    Disable = 0,
    Modulate = 1,
    Add = 2,
    BumpEnvMap = 3,
    BumpEnvMapLuminance = 4,
    Modulate2X = 5,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DShaderSrcBlendFunc {
    Zero = 0,
    One = 1,
    SrcAlpha = 2,
    OneMinusSrcAlpha = 3,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DShader {
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

impl Default for W3DShader {
    fn default() -> Self {
        Self {
            depth_compare: W3DShaderDepthCompare::PassLEqual as u8,
            depth_mask: W3DShaderDepthMask::WriteEnable as u8,
            color_mask: 0,
            dest_blend: W3DShaderDestBlendFunc::Zero as u8,
            fog_func: 0,
            pri_gradient: W3DShaderPriGradient::Modulate as u8,
            sec_gradient: 0,
            src_blend: W3DShaderSrcBlendFunc::One as u8,
            texturing: 0,
            detail_color_func: 0,
            detail_alpha_func: 0,
            shader_preset: 0,
            alpha_test: 0,
            post_detail_color_func: 0,
            post_detail_alpha_func: 0,
            pad: 0,
        }
    }
}

// Mesh Header
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DMeshHeader3 {
    pub version: u32,
    pub attributes: u32,
    pub mesh_name: [u8; 16],
    pub container_name: [u8; 16],
    pub num_tris: u32,
    pub num_vertices: u32,
    pub num_materials: u32,
    pub num_damage_stages: u32,
    pub sort_level: i32,
    pub prelit_version: u32,
    pub future_count: u32,
    pub vertex_channels: u32,
    pub face_channels: u32,
    pub min_corner: W3DVector3,
    pub max_corner: W3DVector3,
    pub sph_center: W3DVector3,
    pub sph_radius: f32,
}

// Vertex influence for skinning
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DVertexInfluence {
    pub bone_idx: u16,
    pub xtra_idx: u8,
    pub bone_inf: u8,
    pub xtra_inf: u8,
    pub pad: [u8; 3],
}

// Triangle
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DTriangle {
    pub vindex: [u32; 3],
    pub attributes: u32,
    pub normal: W3DVector3,
    pub dist: f32,
}

// Hierarchy Header
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DHierarchyHeader {
    pub version: u32,
    pub name: [u8; 16],
    pub num_pivots: u32,
    pub center_pos: W3DVector3,
}

// Pivot (bone)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DPivot {
    pub name: [u8; 16],
    pub parent_idx: i32,
    pub translation: W3DVector3,
    pub euler_angles: W3DVector3,
    pub rotation: W3DQuaternion,
}

// Animation Header
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DAnimationHeader {
    pub version: u32,
    pub name: [u8; 16],
    pub hierarchy_name: [u8; 16],
    pub num_frames: u32,
    pub frame_rate: u32,
}

// Animation Channel Header
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DAnimationChannelHeader {
    pub first_frame: u32,
    pub last_frame: u32,
    pub vector_len: u16,
    pub flags: u16,
    pub pivot: u16,
    pub pad: u16,
}

pub const W3D_ANIMATION_CHANNEL_X: u16 = 0;
pub const W3D_ANIMATION_CHANNEL_Y: u16 = 1;
pub const W3D_ANIMATION_CHANNEL_Z: u16 = 2;
pub const W3D_ANIMATION_CHANNEL_Q: u16 = 6;
pub const W3D_ANIMATION_CHANNEL_TIMECODED: u16 = 0x8000;
