// Primitive aliases, coordinates, ModuleData, and Color
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

/// Shared result type used across legacy subsystems.
pub type GameResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub use super::xfer::{Xfer, XferBlockSize, XferMode, XferStatus, XferVersion};

// Core geometric types matching C++ definitions
/// 3D coordinate type used throughout the game logic system
pub type Coord3D = Vec3;

/// 2D coordinate type used throughout the game logic system  
pub type Coord2D = Vec2;

/// Integer 2D coordinate type  
pub type ICoord2D = IVec2;

/// Integer 3D coordinate type
pub type ICoord3D = IVec3;

/// 3D vector type used for directions and offsets
pub type Vec3D = Vec3;

/// Alias for Vec3D to match C++ usage
pub type Vector3 = Vec3D;

/// Helper trait to provide `origin()` constructors for coordinate aliases.
pub trait CoordOrigin {
    fn origin() -> Self;
}

impl CoordOrigin for Coord3D {
    fn origin() -> Self {
        Vec3::ZERO
    }
}

impl CoordOrigin for Coord2D {
    fn origin() -> Self {
        Vec2::ZERO
    }
}

impl CoordOrigin for ICoord2D {
    fn origin() -> Self {
        IVec2::ZERO
    }
}

impl CoordOrigin for ICoord3D {
    fn origin() -> Self {
        IVec3::ZERO
    }
}

#[derive(Clone)]
pub struct TemplateModuleInfo {
    pub name: AsciiString,
    pub module_tag: AsciiString,
    pub data: Arc<dyn EngineModuleData>,
    pub interface_mask: ModuleInterfaceType,
}

impl TemplateModuleInfo {
    pub fn interface_flags(&self) -> ModuleInterfaceType {
        self.interface_mask
    }
}

/// 3D transformation matrix (SAGE Matrix3D is 4x4 with translation terms)
pub type Matrix3D = Mat4;

/// 4x4 transformation matrix
pub type Matrix4D = Mat4;

/// Real number type (matching C++ Real)
pub type Real = f32;

/// Boolean type (matching C++ Bool)
pub type Bool = bool;

/// Integer type (matching C++ Int)
pub type Int = i32;

/// Unsigned integer type (matching C++ UnsignedInt)
pub type UnsignedInt = u32;

/// Legacy object identifier alias (matching C++ ObjectId)
pub type ObjectId = ObjectID;

/// Unsigned short type (matching C++ UnsignedShort)
pub type UnsignedShort = u16;

/// Short type (matching C++ Short)
pub type Short = i16;

/// Byte type (matching C++ Byte)
pub type Byte = u8;

/// Unsigned byte type (matching C++ UnsignedByte)
pub type UnsignedByte = u8;

// Object identification types
/// Mathematical constants
pub const PI: f32 = std::f32::consts::PI;

/// Timing constants
pub const LOGICFRAMES_PER_SECOND: u32 = 30;
pub const SECONDS_PER_LOGICFRAME_REAL: f32 = 1.0 / LOGICFRAMES_PER_SECOND as f32;

/// Unique identifier for game objects (matching C++ ObjectID)
pub type ObjectID = u32;

/// Player index (matching C++ PlayerIndex)
pub type PlayerIndex = Int;

/// Invalid/null object ID constant
pub const INVALID_ID: ObjectID = 0;

/// Helper trait to enable downcasting from trait objects.
pub trait AsAny {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: std::any::Any> AsAny for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Module data base trait for all behavior modules
pub trait ModuleData: AsAny + Send + Sync + std::fmt::Debug + std::any::Any {
    /// Returns the canonical module type name (mirrors C++ ModuleDataClass::Get_Module_Name).
    fn get_module_type(&self) -> &str {
        let full = std::any::type_name::<Self>();
        full.rsplit("::").next().unwrap_or(full)
    }

    fn get_radar_update_config(
        &self,
    ) -> Option<game_engine::common::thing::module::RadarUpdateConfig> {
        None
    }

    fn get_active_shroud_upgrade_config(
        &self,
    ) -> Option<game_engine::common::thing::module::ActiveShroudUpgradeConfig> {
        None
    }

    fn get_radar_upgrade_config(
        &self,
    ) -> Option<game_engine::common::thing::module::RadarUpgradeConfig> {
        None
    }

    fn get_dynamic_shroud_clearing_range_update_config(
        &self,
    ) -> Option<game_engine::common::thing::module::DynamicShroudClearingRangeUpdateConfig> {
        None
    }

    fn get_shroud_crate_collide_config(
        &self,
    ) -> Option<game_engine::common::thing::module::ShroudCrateCollideConfig> {
        None
    }
}

impl dyn ModuleData {
    pub fn downcast_ref<T: 'static>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }
}

/// Extension trait for Arc<dyn ModuleData> to provide as_any_arc method
pub trait ModuleDataExt {
    fn as_any_arc(self) -> Arc<dyn std::any::Any + Send + Sync>;
}

impl ModuleDataExt for Arc<dyn ModuleData> {
    fn as_any_arc(self) -> Arc<dyn std::any::Any + Send + Sync> {
        // Since ModuleData now extends Any + Send + Sync, we can cast safely
        self as Arc<dyn std::any::Any + Send + Sync>
    }
}

// Game constants
/// Maximum number of players/sides in a game
pub const MAX_PLAYER_COUNT: usize = 16;

/// Maximum number of objects that can exist simultaneously
pub const MAX_OBJECT_COUNT: u32 = 65536;

/// Maximum number of weapon slots
pub const WEAPONSLOT_COUNT: usize = 3;

/// Maximum number of disabled types
pub const DISABLED_COUNT: usize = 13;

/// Maximum trigger area infos
pub const MAX_TRIGGER_AREA_INFOS: usize = 5;

/// Construction complete percentage
pub const CONSTRUCTION_COMPLETE: Real = 100.0;

/// Never timestamp
pub const NEVER: UnsignedInt = 0xFFFFFFFF;

/// Distance calculation mode constants
pub const FROM_CENTER_2D: i32 = 0;
pub const FROM_EDGE_2D: i32 = 1;
pub const FROM_CENTER_3D: i32 = 2;
pub const FROM_BOUNDING_SPHERE_2D: i32 = 3;

/// Distance calculation type
pub type DistanceType = i32;

/// Message type for game messaging system
pub type MessageType = u32;

/// Common message types
pub const MSG_CREATE_SELECTED_GROUP: MessageType = 1001;

/// Frame counter type - represents game simulation frames
pub type FrameNumber = u32;

/// Time in milliseconds
pub type TimeMs = u32;

// Color and rendering types
/// RGBA color type (matching C++ Color)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub const fn transparent() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
        }
    }

    pub const fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        }
    }

    pub const fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    /// Convert to packed ARGB (matches C++ Color usage in decals).
    pub const fn to_argb_u32(self) -> u32 {
        ((self.a as u32) << 24) | ((self.b as u32) << 16) | ((self.g as u32) << 8) | (self.r as u32)
    }
}

