#![allow(dead_code)]
//! W3D Chunk type definitions and constants based on the C++ implementation

use std::fmt;

/// W3D file format version utilities
pub const fn make_version(major: u16, minor: u16) -> u32 {
    ((major as u32) << 16) | (minor as u32)
}

pub const fn get_major_version(ver: u32) -> u16 {
    (ver >> 16) as u16
}

pub const fn get_minor_version(ver: u32) -> u16 {
    (ver & 0xFFFF) as u16
}

pub const W3D_CURRENT_VERSION: u32 = make_version(3, 0);

/// Maximum length for names in W3D format
pub const W3D_NAME_LEN: usize = 16;

/// W3D chunk types enumeration
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u32)]
pub enum W3dChunkType {
    // Main chunk types
    Mesh = 0x00000000,
    Hierarchy = 0x00000100,
    Animation = 0x00000200,
    // Compressed animations (timecoded/adaptive delta)
    CompressedAnimation = 0x00000280,
    HModel = 0x00000300,
    LodModel = 0x00000400,
    TileMap = 0x00000600,

    // Mesh sub-chunks
    MeshHeader = 0x00000001,
    Vertices = 0x00000002,
    VertexNormals = 0x00000003,
    SurrenderNormals = 0x00000004,
    TexCoords = 0x00000005,
    Materials = 0x00000006,
    TrianglesObsolete = 0x00000007,
    QuadranglesObsolete = 0x00000008,
    SurrenderTriangles = 0x00000009,
    PovTrianglesObsolete = 0x0000000A,
    PovQuadranglesObsolete = 0x0000000B,
    MeshUserText = 0x0000000C,
    VertexColors = 0x0000000D,
    VertexInfluences = 0x0000000E,
    Damage = 0x0000000F,
    DamageHeader = 0x00000010,
    DamageVertices = 0x00000011,
    DamageColors = 0x00000012,
    DamageMaterialsObsolete = 0x00000013,
    Materials2 = 0x00000014,
    Materials3 = 0x00000015,
    Material3 = 0x00000016,
    Material3Name = 0x00000017,
    Material3Info = 0x00000018,
    Material3DcMap = 0x00000019,
    Map3Filename = 0x0000001A,
    Map3Info = 0x0000001B,
    Material3DiMap = 0x0000001C,
    Material3ScMap = 0x0000001D,
    Material3SiMap = 0x0000001E,
    MeshHeader3 = 0x0000001F,
    Triangles = 0x00000020,
    PerTriMaterials = 0x00000021,
    MaterialPass = 0x00000038,
    VertexMaterialIds = 0x00000039,
    ShaderIds = 0x0000003A,
    DcgState = 0x0000003B,
    DigState = 0x0000003C,
    ScgState = 0x0000003D,
    SigState = 0x0000003E,
    TextureStage = 0x00000048,
    TextureIds = 0x00000049,
    StageTexCoords = 0x0000004A,
    PerFaceTexCoordIds = 0x0000004B,
    // Deform / Damage (morph-like per-vertex offsets)
    Deform = 0x00000058,
    DeformSet = 0x00000059,
    DeformKeyframe = 0x0000005A,
    DeformData = 0x0000005B,
    // AABTree for culling/collision
    AABTree = 0x00000090,
    AABTreeHeader = 0x00000091,
    AABTreePolyIndices = 0x00000092,
    AABTreeNodes = 0x00000093,

    // Shader chunks
    Shaders = 0x00000029,
    Shader = 0x0000002A,
    VertexMaterials = 0x0000002B,
    VertexMaterial = 0x0000002C,
    VertexMaterialName = 0x0000002D,
    VertexMaterialInfo = 0x0000002E,
    Textures = 0x00000030,
    Texture = 0x00000031,
    TextureName = 0x00000032,
    TextureInfo = 0x00000033,

    // Hierarchy sub-chunks
    HierarchyHeader = 0x00000101,
    Pivots = 0x00000102,
    PivotFixups = 0x00000103,

    // Animation sub-chunks
    AnimationHeader = 0x00000201,
    AnimationChannel = 0x00000202,
    BitChannel = 0x00000203,
    // Compressed animation sub-chunks
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

    // HModel sub-chunks
    HModelHeader = 0x00000301,
    Node = 0x00000302,
    CollisionNode = 0x00000303,
    SkinNode = 0x00000304,
    HModelAuxData = 0x00000305,
    ShadowNode = 0x00000306,

    // LOD Model sub-chunks
    LodModelHeader = 0x00000401,
    Lod = 0x00000402,
    // Lights
    Light = 0x00000460,
    LightInfo = 0x00000461,
    SpotLightInfo = 0x00000462,
    NearAttenuation = 0x00000463,
    FarAttenuation = 0x00000464,
    LightTransform = 0x00000465,

    // TileMap sub-chunks
    TileMapName = 0x00000601,
    TileMapHeader = 0x00000602,
    TileMapTileInstances = 0x00000603,
    TileMapPartitionTree = 0x00000604,
    TileMapPartitionNode = 0x00000605,

    // Unknown chunk type
    Unknown,
}

impl From<u32> for W3dChunkType {
    fn from(value: u32) -> Self {
        match value {
            0x00000000 => W3dChunkType::Mesh,
            0x00000001 => W3dChunkType::MeshHeader,
            0x00000002 => W3dChunkType::Vertices,
            0x00000003 => W3dChunkType::VertexNormals,
            0x00000004 => W3dChunkType::SurrenderNormals,
            0x00000005 => W3dChunkType::TexCoords,
            0x00000006 => W3dChunkType::Materials,
            0x00000007 => W3dChunkType::TrianglesObsolete,
            0x00000008 => W3dChunkType::QuadranglesObsolete,
            0x00000009 => W3dChunkType::SurrenderTriangles,
            0x0000000A => W3dChunkType::PovTrianglesObsolete,
            0x0000000B => W3dChunkType::PovQuadranglesObsolete,
            0x0000000C => W3dChunkType::MeshUserText,
            0x0000000D => W3dChunkType::VertexColors,
            0x0000000E => W3dChunkType::VertexInfluences,
            0x0000000F => W3dChunkType::Damage,
            0x00000010 => W3dChunkType::DamageHeader,
            0x00000011 => W3dChunkType::DamageVertices,
            0x00000012 => W3dChunkType::DamageColors,
            0x00000013 => W3dChunkType::DamageMaterialsObsolete,
            0x00000014 => W3dChunkType::Materials2,
            0x00000015 => W3dChunkType::Materials3,
            0x00000016 => W3dChunkType::Material3,
            0x00000017 => W3dChunkType::Material3Name,
            0x00000018 => W3dChunkType::Material3Info,
            0x00000019 => W3dChunkType::Material3DcMap,
            0x0000001A => W3dChunkType::Map3Filename,
            0x0000001B => W3dChunkType::Map3Info,
            0x0000001C => W3dChunkType::Material3DiMap,
            0x0000001D => W3dChunkType::Material3ScMap,
            0x0000001E => W3dChunkType::Material3SiMap,
            0x0000001F => W3dChunkType::MeshHeader3,
            0x00000020 => W3dChunkType::Triangles,
            0x00000021 => W3dChunkType::PerTriMaterials,
            0x00000029 => W3dChunkType::Shaders,
            0x0000002A => W3dChunkType::Shader,
            0x0000002B => W3dChunkType::VertexMaterials,
            0x0000002C => W3dChunkType::VertexMaterial,
            0x0000002D => W3dChunkType::VertexMaterialName,
            0x0000002E => W3dChunkType::VertexMaterialInfo,
            0x00000030 => W3dChunkType::Textures,
            0x00000031 => W3dChunkType::Texture,
            0x00000032 => W3dChunkType::TextureName,
            0x00000033 => W3dChunkType::TextureInfo,
            0x00000038 => W3dChunkType::MaterialPass,
            0x00000039 => W3dChunkType::VertexMaterialIds,
            0x0000003A => W3dChunkType::ShaderIds,
            0x0000003B => W3dChunkType::DcgState,
            0x0000003C => W3dChunkType::DigState,
            0x0000003D => W3dChunkType::ScgState,
            0x0000003E => W3dChunkType::SigState,
            0x00000048 => W3dChunkType::TextureStage,
            0x00000049 => W3dChunkType::TextureIds,
            0x0000004A => W3dChunkType::StageTexCoords,
            0x0000004B => W3dChunkType::PerFaceTexCoordIds,
            0x00000090 => W3dChunkType::AABTree,
            0x00000091 => W3dChunkType::AABTreeHeader,
            0x00000092 => W3dChunkType::AABTreePolyIndices,
            0x00000093 => W3dChunkType::AABTreeNodes,
            0x00000058 => W3dChunkType::Deform,
            0x00000059 => W3dChunkType::DeformSet,
            0x0000005A => W3dChunkType::DeformKeyframe,
            0x0000005B => W3dChunkType::DeformData,
            0x00000100 => W3dChunkType::Hierarchy,
            0x00000101 => W3dChunkType::HierarchyHeader,
            0x00000102 => W3dChunkType::Pivots,
            0x00000103 => W3dChunkType::PivotFixups,
            0x00000200 => W3dChunkType::Animation,
            0x00000201 => W3dChunkType::AnimationHeader,
            0x00000202 => W3dChunkType::AnimationChannel,
            0x00000203 => W3dChunkType::BitChannel,
            0x00000280 => W3dChunkType::CompressedAnimation,
            0x00000281 => W3dChunkType::CompressedAnimationHeader,
            0x00000282 => W3dChunkType::CompressedAnimationChannel,
            0x00000283 => W3dChunkType::CompressedBitChannel,
            0x000002C0 => W3dChunkType::MorphAnimation,
            0x000002C1 => W3dChunkType::MorphAnimHeader,
            0x000002C2 => W3dChunkType::MorphAnimChannel,
            0x000002C3 => W3dChunkType::MorphAnimPoseName,
            0x000002C4 => W3dChunkType::MorphAnimKeyData,
            0x000002C5 => W3dChunkType::MorphAnimPivotChannelData,
            0x00000300 => W3dChunkType::HModel,
            0x00000301 => W3dChunkType::HModelHeader,
            0x00000302 => W3dChunkType::Node,
            0x00000303 => W3dChunkType::CollisionNode,
            0x00000304 => W3dChunkType::SkinNode,
            0x00000305 => W3dChunkType::HModelAuxData,
            0x00000306 => W3dChunkType::ShadowNode,
            0x00000400 => W3dChunkType::LodModel,
            0x00000401 => W3dChunkType::LodModelHeader,
            0x00000402 => W3dChunkType::Lod,
            0x00000460 => W3dChunkType::Light,
            0x00000461 => W3dChunkType::LightInfo,
            0x00000462 => W3dChunkType::SpotLightInfo,
            0x00000463 => W3dChunkType::NearAttenuation,
            0x00000464 => W3dChunkType::FarAttenuation,
            0x00000465 => W3dChunkType::LightTransform,
            0x00000600 => W3dChunkType::TileMap,
            0x00000601 => W3dChunkType::TileMapName,
            0x00000602 => W3dChunkType::TileMapHeader,
            0x00000603 => W3dChunkType::TileMapTileInstances,
            0x00000604 => W3dChunkType::TileMapPartitionTree,
            0x00000605 => W3dChunkType::TileMapPartitionNode,
            _ => W3dChunkType::Unknown,
        }
    }
}

impl From<W3dChunkType> for u32 {
    fn from(chunk_type: W3dChunkType) -> Self {
        chunk_type as u32
    }
}

impl fmt::Display for W3dChunkType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Mesh attribute flags
#[repr(u32)]
pub enum MeshAttribute {
    None = 0x00000000,
    CollisionBox = 0x00000001,
    Skin = 0x00000002,
    Shadow = 0x00000004,
    Aligned = 0x00000008,
    CollisionTypeMask = 0x00000FF0,
    CollisionTypePhysical = 0x00000010,
    CollisionTypeProjectile = 0x00000020,
    Hidden = 0x00001000,
}

/// Vertex channel flags
pub const W3D_VERTEX_CHANNEL_LOCATION: u32 = 0x00000001;
pub const W3D_VERTEX_CHANNEL_NORMAL: u32 = 0x00000002;
pub const W3D_VERTEX_CHANNEL_TEXCOORD: u32 = 0x00000004;
pub const W3D_VERTEX_CHANNEL_COLOR: u32 = 0x00000008;
pub const W3D_VERTEX_CHANNEL_BONEID: u32 = 0x00000010;

/// Face channel flags
pub const W3D_FACE_CHANNEL_FACE: u32 = 0x00000001;

/// Material attribute flags
pub const W3DMATERIAL_USE_ALPHA: u32 = 0x00000001;
pub const W3DMATERIAL_USE_SORTING: u32 = 0x00000002;
pub const W3DMATERIAL_HINT_DIT_OVER_DCT: u32 = 0x00000010;
pub const W3DMATERIAL_HINT_SIT_OVER_SCT: u32 = 0x00000020;
pub const W3DMATERIAL_HINT_DIT_OVER_DIG: u32 = 0x00000040;
pub const W3DMATERIAL_HINT_SIT_OVER_SIG: u32 = 0x00000080;
pub const W3DMATERIAL_HINT_FAST_SPECULAR_AFTER_ALPHA: u32 = 0x00000100;

/// Mapping types
pub const W3DMAPPING_UV: u16 = 0;
pub const W3DMAPPING_ENVIRONMENT: u16 = 1;

/// Animation channel types
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AnimationChannel {
    X = 0,
    Y = 1,
    Z = 2,
    XR = 3,
    YR = 4,
    ZR = 5,
    Q = 6, // Quaternion rotation
}

/// Bit channel types
#[repr(u16)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BitChannel {
    Vis = 0, // Visibility channel
}

/// Chunk header as it appears in the file
#[derive(Debug, Clone, Copy)]
pub struct W3dChunkHeader {
    pub chunk_type: W3dChunkType,
    pub chunk_size: u32,
}

impl W3dChunkHeader {
    pub fn new(chunk_type: W3dChunkType, chunk_size: u32) -> Self {
        Self {
            chunk_type,
            chunk_size,
        }
    }

    /// Returns true if this chunk contains sub-chunks (MSB set)
    pub fn has_sub_chunks(&self) -> bool {
        self.chunk_size & 0x80000000 != 0
    }

    /// Returns the actual data size (MSB cleared)
    pub fn data_size(&self) -> u32 {
        self.chunk_size & 0x7FFFFFFF
    }
}
