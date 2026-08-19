// Regions, waypoints, body/bridge, and map scale
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

/// Team member list type (matching C++ MAKE_DLINK)
pub type TeamMemberList = Vec<ObjectID>;

// Map and terrain related types

/// Waypoint ID type (matching C++ WaypointID)
pub type WaypointID = u32;

/// Invalid waypoint ID constant  
pub const INVALID_WAYPOINT_ID: WaypointID = 0x7FFFFFFF;

/// Body damage type (matching C++ BodyDamageType)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum BodyDamageType {
    Pristine,
    Damaged,
    ReallyDamaged,
    Rubble,
}

/// Bridge tower type (matching C++ BridgeTowerType / TerrainRoads.h)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeTowerType {
    FromLeft = 0,
    FromRight = 1,
    ToLeft = 2,
    ToRight = 3,
}

impl BridgeTowerType {
    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::FromLeft),
            1 => Some(Self::FromRight),
            2 => Some(Self::ToLeft),
            3 => Some(Self::ToRight),
            _ => None,
        }
    }
}

/// Maximum number of bridge towers (C++ BRIDGE_MAX_TOWERS)
pub const BRIDGE_MAX_TOWERS: usize = 4;

/// 2D region (matching C++ Region2D)
#[derive(Debug, Clone, Copy)]
pub struct Region2D {
    pub lo: Coord2D,
    pub hi: Coord2D,
}

impl Default for Region2D {
    fn default() -> Self {
        Self {
            lo: Coord2D::ZERO,
            hi: Coord2D::ZERO,
        }
    }
}

impl Region2D {
    pub fn new(lo: Coord2D, hi: Coord2D) -> Self {
        Self { lo, hi }
    }
}

/// Integer 2D region (matching C++ IRegion2D)  
#[derive(Debug, Clone, Copy)]
pub struct IRegion2D {
    pub lo: ICoord2D,
    pub hi: ICoord2D,
}

impl Default for IRegion2D {
    fn default() -> Self {
        Self {
            lo: ICoord2D::ZERO,
            hi: ICoord2D::ZERO,
        }
    }
}

impl IRegion2D {
    pub fn new(lo: ICoord2D, hi: ICoord2D) -> Self {
        Self { lo, hi }
    }
}

/// 3D region (matching C++ Region3D)
#[derive(Debug, Clone, Copy)]
pub struct Region3D {
    pub lo: Coord3D,
    pub hi: Coord3D,
}

impl Default for Region3D {
    fn default() -> Self {
        Self {
            lo: Coord3D::origin(),
            hi: Coord3D::origin(),
        }
    }
}

impl Region3D {
    pub fn new(lo: Coord3D, hi: Coord3D) -> Self {
        Self { lo, hi }
    }
}

/// Map dimensions and scaling constants (matching C++ definitions)
pub const MAP_XY_FACTOR: f32 = 10.0; // How wide and tall each height map square is in world space
pub const MAP_HEIGHT_SCALE: f32 = MAP_XY_FACTOR / 16.0; // Divide all map heights by 8

/// Pathfind cell size constants

/// Locomotor surface type mask (matching C++ LocomotorSurfaceTypeMask)
pub type LocomotorSurfaceTypeMask = u32;
