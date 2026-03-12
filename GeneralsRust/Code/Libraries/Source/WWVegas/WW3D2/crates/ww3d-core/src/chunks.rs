/// W3D File Format Chunk Types
/// Ported from w3d_file.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum W3DChunkType {
    // Mesh chunks
    Mesh = 0x00000000,
    Vertices = 0x00000002,
    VertexNormals = 0x00000003,
    MeshUserText = 0x0000000C,
    VertexInfluences = 0x0000000E,
    MeshHeader3 = 0x0000001F,
    Triangles = 0x00000020,
    VertexShadeIndices = 0x00000022,

    // Prelit material wrappers
    PrelitUnlit = 0x00000023,
    PrelitVertex = 0x00000024,
    PrelitLightmapMultiPass = 0x00000025,
    PrelitLightmapMultiTexture = 0x00000026,

    // Material info
    MaterialInfo = 0x00000028,
    Shaders = 0x00000029,
    VertexMaterials = 0x0000002A,
    VertexMaterial = 0x0000002B,
    VertexMaterialName = 0x0000002C,
    VertexMaterialInfo = 0x0000002D,
    VertexMapperArgs0 = 0x0000002E,
    VertexMapperArgs1 = 0x0000002F,

    // Texture info
    Textures = 0x00000030,
    Texture = 0x00000031,
    TextureName = 0x00000032,
    TextureInfo = 0x00000033,

    // Material pass
    MaterialPass = 0x00000038,
    VertexMaterialIds = 0x00000039,
    ShaderIds = 0x0000003A,
    Dcg = 0x0000003B, // Diffuse Color Group
    Dig = 0x0000003C, // Diffuse Illumination Group
    Scg = 0x0000003E, // Specular Color Group

    // Texture stage
    TextureStage = 0x00000048,
    TextureIds = 0x00000049,
    StageTexcoords = 0x0000004A,
    PerFaceTexcoordIds = 0x0000004B,

    // Deform chunks
    Deform = 0x00000058,
    DeformSet = 0x00000059,
    DeformKeyframe = 0x0000005A,
    DeformData = 0x0000005B,

    // PS2 Shaders
    Ps2Shaders = 0x00000080,

    // AABTree
    Aabtree = 0x00000090,
    AabtreeHeader = 0x00000091,
    AabtreePolyindices = 0x00000092,
    AabtreeNodes = 0x00000093,

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
    TimeCodedAnimChannel = 0x00000204,
    TimeCodedBitChannel = 0x00000205,
    AdaptiveDeltaAnimChannel = 0x00000206,

    // Compressed Animation
    CompressedAnimation = 0x00000280,
    CompressedAnimationHeader = 0x00000281,
    CompressedAnimationChannel = 0x00000282,
    CompressedBitChannel = 0x00000283,

    // Morph Animation
    MorphAnimation = 0x000002C0,
    MorphanimHeader = 0x000002C1,
    MorphanimChannel = 0x000002C2,
    MorphanimPosename = 0x000002C3,
    MorphanimKeydata = 0x000002C4,
    MorphanimPivotchanneldata = 0x000002C5,

    // HModel
    Hmodel = 0x00000300,
    HmodelHeader = 0x00000301,
    Node = 0x00000302,
    CollisionNode = 0x00000303,
    SkinNode = 0x00000304,

    // LOD Model
    Lodmodel = 0x00000400,
    LodmodelHeader = 0x00000401,
    Lod = 0x00000402,

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
    EmitterInfov2 = 0x00000504,
    EmitterProps = 0x00000505,
    EmitterLineProperties = 0x0000050B,
    EmitterRotationKeyframes = 0x0000050C,
    EmitterFrameKeyframes = 0x0000050D,
    EmitterBlurTimeKeyframes = 0x0000050E,
    EmitterExtraInfo = 0x0000050F,

    // Aggregate
    Aggregate = 0x00000600,
    AggregateHeader = 0x00000601,
    AggregateInfo = 0x00000602,
    TextureReplacerInfo = 0x00000603,
    AggregateClassInfo = 0x00000604,

    // HLOD
    Hlod = 0x00000700,
    HlodHeader = 0x00000701,
    HlodLodArray = 0x00000702,
    HlodSubObjectArrayHeader = 0x00000703,
    HlodSubObject = 0x00000704,
    HlodAggregateArray = 0x00000705,
    HlodProxyArray = 0x00000706,

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
    DazzleTypename = 0x00000902,

    // Sound Render Object
    Soundrobj = 0x00000A00,
    SoundrobjHeader = 0x00000A01,
    SoundrobjDefinition = 0x00000A02,

    // Shader Mesh
    Shdmesh = 0x00000B00,
    ShdmeshName = 0x00000B01,
    ShdmeshHeader = 0x00000B02,
    ShdmeshUserText = 0x00000B03,

    // Shader Submesh
    Shdsubmesh = 0x00000B20,
    ShdsubmeshHeader = 0x00000B21,
    ShdsubmeshShader = 0x00000B40,
    ShdsubmeshShaderClassid = 0x00000B41,
    ShdsubmeshShaderDef = 0x00000B42,
    ShdsubmeshVertices = 0x00000B23,
    ShdsubmeshVertexNormals = 0x00000B24,
    ShdsubmeshTriangles = 0x00000B25,
    ShdsubmeshVertexShadeIndices = 0x00000B26,
    ShdsubmeshUv0 = 0x00000B27,
    ShdsubmeshUv1 = 0x00000B28,
    ShdsubmeshTangentBasisS = 0x00000B29,
    ShdsubmeshTangentBasisT = 0x00000B2A,
    ShdsubmeshTangentBasisSxt = 0x00000B2B,
    ShdsubmeshVertexColor = 0x00000B2C,
    ShdsubmeshVertexInfluences = 0x00000B2D,
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
            0x00000023 => Some(Self::PrelitUnlit),
            0x00000024 => Some(Self::PrelitVertex),
            0x00000025 => Some(Self::PrelitLightmapMultiPass),
            0x00000026 => Some(Self::PrelitLightmapMultiTexture),
            0x00000028 => Some(Self::MaterialInfo),
            0x00000029 => Some(Self::Shaders),
            0x0000002A => Some(Self::VertexMaterials),
            0x0000002B => Some(Self::VertexMaterial),
            0x0000002C => Some(Self::VertexMaterialName),
            0x0000002D => Some(Self::VertexMaterialInfo),
            0x0000002E => Some(Self::VertexMapperArgs0),
            0x0000002F => Some(Self::VertexMapperArgs1),
            0x00000030 => Some(Self::Textures),
            0x00000031 => Some(Self::Texture),
            0x00000032 => Some(Self::TextureName),
            0x00000033 => Some(Self::TextureInfo),
            0x00000038 => Some(Self::MaterialPass),
            0x00000039 => Some(Self::VertexMaterialIds),
            0x0000003A => Some(Self::ShaderIds),
            0x0000003B => Some(Self::Dcg),
            0x0000003C => Some(Self::Dig),
            0x0000003E => Some(Self::Scg),
            0x00000048 => Some(Self::TextureStage),
            0x00000049 => Some(Self::TextureIds),
            0x0000004A => Some(Self::StageTexcoords),
            0x0000004B => Some(Self::PerFaceTexcoordIds),
            0x00000058 => Some(Self::Deform),
            0x00000059 => Some(Self::DeformSet),
            0x0000005A => Some(Self::DeformKeyframe),
            0x0000005B => Some(Self::DeformData),
            0x00000080 => Some(Self::Ps2Shaders),
            0x00000090 => Some(Self::Aabtree),
            0x00000091 => Some(Self::AabtreeHeader),
            0x00000092 => Some(Self::AabtreePolyindices),
            0x00000093 => Some(Self::AabtreeNodes),
            0x00000100 => Some(Self::Hierarchy),
            0x00000101 => Some(Self::HierarchyHeader),
            0x00000102 => Some(Self::Pivots),
            0x00000103 => Some(Self::PivotFixups),
            0x00000200 => Some(Self::Animation),
            0x00000201 => Some(Self::AnimationHeader),
            0x00000202 => Some(Self::AnimationChannel),
            0x00000203 => Some(Self::BitChannel),
            0x00000204 => Some(Self::TimeCodedAnimChannel),
            0x00000205 => Some(Self::TimeCodedBitChannel),
            0x00000206 => Some(Self::AdaptiveDeltaAnimChannel),
            0x00000280 => Some(Self::CompressedAnimation),
            0x00000281 => Some(Self::CompressedAnimationHeader),
            0x00000282 => Some(Self::CompressedAnimationChannel),
            0x00000283 => Some(Self::CompressedBitChannel),
            0x000002C0 => Some(Self::MorphAnimation),
            0x000002C1 => Some(Self::MorphanimHeader),
            0x000002C2 => Some(Self::MorphanimChannel),
            0x000002C3 => Some(Self::MorphanimPosename),
            0x000002C4 => Some(Self::MorphanimKeydata),
            0x000002C5 => Some(Self::MorphanimPivotchanneldata),
            0x00000300 => Some(Self::Hmodel),
            0x00000301 => Some(Self::HmodelHeader),
            0x00000302 => Some(Self::Node),
            0x00000303 => Some(Self::CollisionNode),
            0x00000304 => Some(Self::SkinNode),
            0x00000400 => Some(Self::Lodmodel),
            0x00000401 => Some(Self::LodmodelHeader),
            0x00000402 => Some(Self::Lod),
            0x00000420 => Some(Self::Collection),
            0x00000421 => Some(Self::CollectionHeader),
            0x00000422 => Some(Self::CollectionObjName),
            0x00000423 => Some(Self::Placeholder),
            0x00000424 => Some(Self::TransformNode),
            0x00000440 => Some(Self::Points),
            0x00000460 => Some(Self::Light),
            0x00000461 => Some(Self::LightInfo),
            0x00000462 => Some(Self::SpotLightInfo),
            0x00000463 => Some(Self::NearAttenuation),
            0x00000464 => Some(Self::FarAttenuation),
            0x00000500 => Some(Self::Emitter),
            0x00000501 => Some(Self::EmitterHeader),
            0x00000502 => Some(Self::EmitterUserData),
            0x00000503 => Some(Self::EmitterInfo),
            0x00000504 => Some(Self::EmitterInfov2),
            0x00000505 => Some(Self::EmitterProps),
            0x0000050B => Some(Self::EmitterLineProperties),
            0x0000050C => Some(Self::EmitterRotationKeyframes),
            0x0000050D => Some(Self::EmitterFrameKeyframes),
            0x0000050E => Some(Self::EmitterBlurTimeKeyframes),
            0x0000050F => Some(Self::EmitterExtraInfo),
            0x00000600 => Some(Self::Aggregate),
            0x00000601 => Some(Self::AggregateHeader),
            0x00000602 => Some(Self::AggregateInfo),
            0x00000603 => Some(Self::TextureReplacerInfo),
            0x00000604 => Some(Self::AggregateClassInfo),
            0x00000700 => Some(Self::Hlod),
            0x00000701 => Some(Self::HlodHeader),
            0x00000702 => Some(Self::HlodLodArray),
            0x00000703 => Some(Self::HlodSubObjectArrayHeader),
            0x00000704 => Some(Self::HlodSubObject),
            0x00000705 => Some(Self::HlodAggregateArray),
            0x00000706 => Some(Self::HlodProxyArray),
            0x00000740 => Some(Self::Box),
            0x00000741 => Some(Self::Sphere),
            0x00000742 => Some(Self::Ring),
            0x00000750 => Some(Self::NullObject),
            0x00000800 => Some(Self::Lightscape),
            0x00000801 => Some(Self::LightscapeLight),
            0x00000802 => Some(Self::LightTransform),
            0x00000900 => Some(Self::Dazzle),
            0x00000901 => Some(Self::DazzleName),
            0x00000902 => Some(Self::DazzleTypename),
            0x00000A00 => Some(Self::Soundrobj),
            0x00000A01 => Some(Self::SoundrobjHeader),
            0x00000A02 => Some(Self::SoundrobjDefinition),
            0x00000B00 => Some(Self::Shdmesh),
            0x00000B01 => Some(Self::ShdmeshName),
            0x00000B02 => Some(Self::ShdmeshHeader),
            0x00000B03 => Some(Self::ShdmeshUserText),
            0x00000B20 => Some(Self::Shdsubmesh),
            0x00000B21 => Some(Self::ShdsubmeshHeader),
            0x00000B40 => Some(Self::ShdsubmeshShader),
            0x00000B41 => Some(Self::ShdsubmeshShaderClassid),
            0x00000B42 => Some(Self::ShdsubmeshShaderDef),
            0x00000B23 => Some(Self::ShdsubmeshVertices),
            0x00000B24 => Some(Self::ShdsubmeshVertexNormals),
            0x00000B25 => Some(Self::ShdsubmeshTriangles),
            0x00000B26 => Some(Self::ShdsubmeshVertexShadeIndices),
            0x00000B27 => Some(Self::ShdsubmeshUv0),
            0x00000B28 => Some(Self::ShdsubmeshUv1),
            0x00000B29 => Some(Self::ShdsubmeshTangentBasisS),
            0x00000B2A => Some(Self::ShdsubmeshTangentBasisT),
            0x00000B2B => Some(Self::ShdsubmeshTangentBasisSxt),
            0x00000B2C => Some(Self::ShdsubmeshVertexColor),
            0x00000B2D => Some(Self::ShdsubmeshVertexInfluences),
            _ => None,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

// Public W3D Chunk Constants
// These constants are exported for use by loaders that need u32 chunk IDs
// All values are from w3d_file.h

// Mesh chunks
pub const W3D_CHUNK_MESH: u32 = 0x00000000;
pub const W3D_CHUNK_VERTICES: u32 = 0x00000002;
pub const W3D_CHUNK_VERTEX_NORMALS: u32 = 0x00000003;
pub const W3D_CHUNK_MESH_USER_TEXT: u32 = 0x0000000C;
pub const W3D_CHUNK_VERTEX_INFLUENCES: u32 = 0x0000000E;
pub const W3D_CHUNK_MESH_HEADER3: u32 = 0x0000001F;
pub const W3D_CHUNK_TRIANGLES: u32 = 0x00000020;
pub const W3D_CHUNK_VERTEX_SHADE_INDICES: u32 = 0x00000022;

// Material info chunks
pub const W3D_CHUNK_MATERIAL_INFO: u32 = 0x00000028;
pub const W3D_CHUNK_SHADERS: u32 = 0x00000029;
pub const W3D_CHUNK_SHADER_IDS: u32 = 0x0000003A;
pub const W3D_CHUNK_TEXTURES: u32 = 0x00000030;
pub const W3D_CHUNK_MATERIAL_PASS: u32 = 0x00000038;

// Hierarchy chunks
pub const W3D_CHUNK_HIERARCHY: u32 = 0x00000100;
pub const W3D_CHUNK_HIERARCHY_HEADER: u32 = 0x00000101;
pub const W3D_CHUNK_PIVOTS: u32 = 0x00000102;
pub const W3D_CHUNK_PIVOT_FIXUPS: u32 = 0x00000103;

// Animation chunks
pub const W3D_CHUNK_ANIMATION: u32 = 0x00000200;
pub const W3D_CHUNK_ANIMATION_HEADER: u32 = 0x00000201;
pub const W3D_CHUNK_ANIMATION_CHANNEL: u32 = 0x00000202;
pub const W3D_CHUNK_BIT_CHANNEL: u32 = 0x00000203;

// Compressed animation chunks
pub const W3D_CHUNK_COMPRESSED_ANIMATION: u32 = 0x00000280;
pub const W3D_CHUNK_COMPRESSED_ANIMATION_HEADER: u32 = 0x00000281;
pub const W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL: u32 = 0x00000282;
pub const W3D_CHUNK_COMPRESSED_BIT_CHANNEL: u32 = 0x00000283;
pub const W3D_CHUNK_TIMECODED_CHANNEL: u32 = 0x00000204;
pub const W3D_CHUNK_ADAPTIVEDELTA_CHANNEL: u32 = 0x00000206;

// HModel chunks
pub const W3D_CHUNK_HMODEL: u32 = 0x00000300;
pub const W3D_CHUNK_HMODEL_HEADER: u32 = 0x00000301;
pub const W3D_CHUNK_NODE: u32 = 0x00000302;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_type_values() {
        assert_eq!(W3DChunkType::Mesh.as_u32(), 0x00000000);
        assert_eq!(W3DChunkType::Hierarchy.as_u32(), 0x00000100);
        assert_eq!(W3DChunkType::Animation.as_u32(), 0x00000200);
    }

    #[test]
    fn test_chunk_type_roundtrip() {
        let chunk = W3DChunkType::MeshHeader3;
        let value = chunk.as_u32();
        let back = W3DChunkType::from_u32(value).unwrap();
        assert_eq!(chunk, back);
    }
}
