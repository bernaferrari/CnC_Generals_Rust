// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
//
// Cell model: coordinates, terrain/flag enums, pathfinding layers and the
// per-cell data C++ keeps in `PathfindCell`.

use glam::Vec3 as Coord3D;

/// Movement cost constants matching C++ AIPathfind.cpp:1649-1650
pub const COST_ORTHOGONAL: u32 = 10;
pub const COST_DIAGONAL: u32 = 14;
/// C++ notZonePassable penalty: `100 * COST_ORTHOGONAL`.
pub const ZONE_IMPASSABLE_COST: u32 = 100 * COST_ORTHOGONAL;

/// Pathfinding cell size matching C++ AIPathfind.h:415-416
pub const PATHFIND_CELL_SIZE: i32 = 10;
pub const PATHFIND_CELL_SIZE_F: f32 = 10.0;

/// Maximum frames ahead for synchronization matching C++ Connection.cpp
pub const MAX_FRAMES_AHEAD: u32 = 300;
pub(crate) const SURFACE_GROUND: u32 = 0x01;
pub(crate) const SURFACE_WATER: u32 = 0x02;
pub(crate) const SURFACE_CLIFF: u32 = 0x04;
pub(crate) const SURFACE_AIR: u32 = 0x08;
pub(crate) const SURFACE_RUBBLE: u32 = 0x10;

/// Cell type matching C++ AIPathfind.h:233-242
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PathfindCellType {
    Clear = 0x00,            // Clear, unobstructed ground
    Water = 0x01,            // Water area
    Cliff = 0x02,            // Steep altitude change
    Rubble = 0x03,           // Cell occupied by rubble
    Obstacle = 0x04,         // Occupied by a structure
    BridgeImpassable = 0x05, // Impassable bridge piece
    Impassable = 0x06,       // Impassable except for aircraft
}

/// Cell flags matching C++ AIPathfind.h:244-251
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CellFlags {
    NoUnits = 0x00,             // No units in this cell
    UnitGoal = 0x01,            // Unit heading to this cell
    UnitPresentMoving = 0x02,   // Unit moving through cell
    UnitPresentFixed = 0x03,    // Unit stationary in cell
    UnitGoalOtherMoving = 0x05, // Unit moving + another has goal
}

/// Pathfinding layer enum matching C++ GameType.h
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1,
    /// C++ first unnamed bridge slot (`LAYER_GROUND + 1`).
    Top = 2,
    Layer3 = 3,
    Layer4 = 4,
    Layer5 = 5,
    Layer6 = 6,
    Layer7 = 7,
    Layer8 = 8,
    Layer9 = 9,
    Layer10 = 10,
    Layer11 = 11,
    Layer12 = 12,
    Layer13 = 13,
    Layer14 = 14,
    /// C++ `LAYER_WALL = LAYER_LAST = 15`.
    Wall = 15,
}

impl PathfindLayerEnum {
    pub const LAST: Self = Self::Wall;

    pub fn from_u32(value: u32) -> Self {
        match value {
            0 => Self::Invalid,
            1 => Self::Ground,
            2 => Self::Top,
            3 => Self::Layer3,
            4 => Self::Layer4,
            5 => Self::Layer5,
            6 => Self::Layer6,
            7 => Self::Layer7,
            8 => Self::Layer8,
            9 => Self::Layer9,
            10 => Self::Layer10,
            11 => Self::Layer11,
            12 => Self::Layer12,
            13 => Self::Layer13,
            14 => Self::Layer14,
            15 => Self::Wall,
            _ => Self::Invalid,
        }
    }

    pub fn is_elevated(self) -> bool {
        let v = self as u8;
        v >= 2 && v <= 15
    }
}

/// Grid coordinate for pathfinding
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GridCoord {
    pub x: i32,
    pub y: i32,
}

impl GridCoord {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Convert world coordinates to grid coordinates
    /// Matches C++ worldToCell() at AIPathfind.h:934
    pub fn from_world(pos: &Coord3D) -> Self {
        Self {
            x: (pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
            y: (pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
        }
    }

    /// Convert grid coordinates to the cell center using the caller-resolved layer height.
    /// Matches the XY portion of C++ adjustCoordToCell(); terrain height belongs to the caller.
    pub fn to_world(&self, height: f32) -> Coord3D {
        let x = (self.x as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
        let y = (self.y as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
        Coord3D::new(x, y, height)
    }

    /// Manhattan distance for heuristic
    pub fn manhattan_distance(&self, other: &GridCoord) -> u32 {
        let dx: i32 = (self.x - other.x).abs();
        let dy: i32 = (self.y - other.y).abs();
        COST_ORTHOGONAL * (dx + dy) as u32
    }

    /// Diagonal distance heuristic (more accurate than Manhattan)
    /// Matches C++ PathfindCell::costToGoal() at AIPathfind.cpp:1654
    pub fn diagonal_distance(&self, other: &GridCoord) -> u32 {
        let dx = (self.x - other.x).abs() as u32;
        let dy = (self.y - other.y).abs() as u32;

        if dx > dy {
            COST_ORTHOGONAL * dx + (COST_ORTHOGONAL * dy) / 2
        } else {
            COST_ORTHOGONAL * dy + (COST_ORTHOGONAL * dx) / 2
        }
    }

    /// Get 8 neighboring cells (orthogonal + diagonal)
    /// Matches C++ examineNeighboringCells() at AIPathfind.cpp:6125-6128
    pub fn neighbors(&self) -> [GridCoord; 8] {
        [
            GridCoord::new(self.x + 1, self.y),     // Right
            GridCoord::new(self.x, self.y + 1),     // Up
            GridCoord::new(self.x - 1, self.y),     // Left
            GridCoord::new(self.x, self.y - 1),     // Down
            GridCoord::new(self.x + 1, self.y + 1), // Right-Up
            GridCoord::new(self.x - 1, self.y + 1), // Left-Up
            GridCoord::new(self.x - 1, self.y - 1), // Left-Down
            GridCoord::new(self.x + 1, self.y - 1), // Right-Down
        ]
    }

    /// Check if this is a diagonal neighbor
    pub fn is_diagonal(&self, other: &GridCoord) -> bool {
        let dx: i32 = (self.x - other.x).abs();
        let dy: i32 = (self.y - other.y).abs();
        dx == 1 && dy == 1
    }
}

/// Pathfinding cell data
#[derive(Debug, Clone)]
pub struct PathfindCell {
    cell_type: PathfindCellType,
    flags: CellFlags,
    layer: PathfindLayerEnum,
    /// C++ PathfindCell::m_connectLayer (bridge/wall entry link).
    connect_layer: PathfindLayerEnum,
    zone: u16,
    pinched: bool,
    /// Read directly by the movement-cost model (C++ cost multiplier).
    pub(crate) cost_multiplier: f32,
}

impl PathfindCell {
    pub fn new() -> Self {
        Self {
            cell_type: PathfindCellType::Clear,
            flags: CellFlags::NoUnits,
            layer: PathfindLayerEnum::Ground,
            connect_layer: PathfindLayerEnum::Invalid,
            zone: 0,
            pinched: false,
            cost_multiplier: 1.0,
        }
    }

    pub fn get_type(&self) -> PathfindCellType {
        self.cell_type
    }

    pub fn set_type(&mut self, cell_type: PathfindCellType) {
        self.cell_type = cell_type;
    }

    pub fn get_layer(&self) -> PathfindLayerEnum {
        self.layer
    }

    pub fn set_layer(&mut self, layer: PathfindLayerEnum) {
        self.layer = layer;
    }

    pub fn get_connect_layer(&self) -> PathfindLayerEnum {
        self.connect_layer
    }

    pub fn set_connect_layer(&mut self, layer: PathfindLayerEnum) {
        self.connect_layer = layer;
    }

    pub fn get_flags(&self) -> CellFlags {
        self.flags
    }

    pub fn set_flags(&mut self, flags: CellFlags) {
        self.flags = flags;
    }

    pub fn is_pinched(&self) -> bool {
        self.pinched
    }

    pub fn set_pinched(&mut self, pinched: bool) {
        self.pinched = pinched;
    }

    /// Check if cell is impassable for ground units
    /// Matches C++ IS_IMPASSABLE() at AIPathfind.cpp:55-67
    pub fn is_impassable(&self) -> bool {
        matches!(
            self.cell_type,
            PathfindCellType::Impassable
                | PathfindCellType::Obstacle
                | PathfindCellType::BridgeImpassable
        )
    }
}
