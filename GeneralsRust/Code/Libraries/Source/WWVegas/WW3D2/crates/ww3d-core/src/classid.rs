/// WW3D Class IDs for runtime type identification
/// Ported from classid.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ClassID {
    IndirectTextureClass = 0x10000,
    VariableTextureClass,
    FileListTextureClass,
    ResizeableTextureInstanceClass,
    AnimTextureInstanceClass,
    ManualAnimTextureInstanceClass,
    TimeAnimTextureInstanceClass,
    PointGroupClass,
    MeshModelClass,
    CachedTextureFileClass,
    StreamingTextureClass,
    StreamingTextureInstanceClass,
}

impl ClassID {
    pub fn from_u32(id: u32) -> Option<Self> {
        match id {
            0x10000 => Some(Self::IndirectTextureClass),
            0x10001 => Some(Self::VariableTextureClass),
            0x10002 => Some(Self::FileListTextureClass),
            0x10003 => Some(Self::ResizeableTextureInstanceClass),
            0x10004 => Some(Self::AnimTextureInstanceClass),
            0x10005 => Some(Self::ManualAnimTextureInstanceClass),
            0x10006 => Some(Self::TimeAnimTextureInstanceClass),
            0x10007 => Some(Self::PointGroupClass),
            0x10008 => Some(Self::MeshModelClass),
            0x10009 => Some(Self::CachedTextureFileClass),
            0x1000A => Some(Self::StreamingTextureClass),
            0x1000B => Some(Self::StreamingTextureInstanceClass),
            _ => None,
        }
    }

    pub fn as_u32(self) -> u32 {
        self as u32
    }
}

/// Render object class IDs (`RenderObjClass::CLASSID_*` in the legacy engine).
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderObjClassId {
    /// Legacy unknown sentinel (`CLASSID_UNKNOWN`).
    Unknown = 0xFFFF_FFFF,
    Mesh = 0,
    HModel = 1,
    DistLod = 2,
    PredLodGroup = 3,
    TileMap = 4,
    Image3D = 5,
    Line3D = 6,
    Bitmap2D = 7,
    Camera = 8,
    DynaMesh = 9,
    DynaScreenMesh = 10,
    TextDraw = 11,
    Fog = 12,
    LayerFog = 13,
    Light = 14,
    ParticleEmitter = 15,
    ParticleBuffer = 16,
    ScreenPointGroup = 17,
    ViewPointGroup = 18,
    WorldPointGroup = 19,
    Text2D = 20,
    Text3D = 21,
    Null = 22,
    Collection = 23,
    Flare = 24,
    Hlod = 25,
    AaBox = 26,
    ObBox = 27,
    SegLine = 28,
    Sphere = 29,
    Ring = 30,
    BoundFog = 31,
    Dazzle = 32,
    Sound = 33,
    SegLineTrail = 34,
    Land = 35,
    ShdMesh = 36,
}

impl RenderObjClassId {
    /// Convert a raw numeric class identifier into the strongly typed enum.
    pub fn from_u32(id: u32) -> Self {
        match id {
            0 => Self::Mesh,
            1 => Self::HModel,
            2 => Self::DistLod,
            3 => Self::PredLodGroup,
            4 => Self::TileMap,
            5 => Self::Image3D,
            6 => Self::Line3D,
            7 => Self::Bitmap2D,
            8 => Self::Camera,
            9 => Self::DynaMesh,
            10 => Self::DynaScreenMesh,
            11 => Self::TextDraw,
            12 => Self::Fog,
            13 => Self::LayerFog,
            14 => Self::Light,
            15 => Self::ParticleEmitter,
            16 => Self::ParticleBuffer,
            17 => Self::ScreenPointGroup,
            18 => Self::ViewPointGroup,
            19 => Self::WorldPointGroup,
            20 => Self::Text2D,
            21 => Self::Text3D,
            22 => Self::Null,
            23 => Self::Collection,
            24 => Self::Flare,
            25 => Self::Hlod,
            26 => Self::AaBox,
            27 => Self::ObBox,
            28 => Self::SegLine,
            29 => Self::Sphere,
            30 => Self::Ring,
            31 => Self::BoundFog,
            32 => Self::Dazzle,
            33 => Self::Sound,
            34 => Self::SegLineTrail,
            35 => Self::Land,
            36 => Self::ShdMesh,
            _ => Self::Unknown,
        }
    }

    /// Return the numeric value used by the legacy engine.
    pub fn as_u32(self) -> u32 {
        self as u32
    }

    /// Indicates whether the ID maps to the unknown sentinel.
    pub fn is_unknown(self) -> bool {
        matches!(self, Self::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classid_values() {
        assert_eq!(ClassID::IndirectTextureClass.as_u32(), 0x10000);
        assert_eq!(ClassID::MeshModelClass.as_u32(), 0x10008);
    }

    #[test]
    fn test_classid_roundtrip() {
        let id = ClassID::PointGroupClass;
        let value = id.as_u32();
        let back = ClassID::from_u32(value).unwrap();
        assert_eq!(id, back);
    }

    #[test]
    fn render_obj_classid_roundtrip_known() {
        let id = RenderObjClassId::Ring;
        assert_eq!(RenderObjClassId::from_u32(id.as_u32()), id);
    }

    #[test]
    fn render_obj_classid_unknown_maps() {
        let id = RenderObjClassId::from_u32(0xABCD);
        assert!(id.is_unknown());
        assert_eq!(id.as_u32(), 0xFFFF_FFFF);
    }
}
