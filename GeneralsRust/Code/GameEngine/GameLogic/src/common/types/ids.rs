// Team/player IDs, money, health, and percentage helpers
//
// Split from `types.rs` for module-size parity.
// Observable behavior is unchanged.

// Team and player management
/// Team identifier
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TeamId(pub u8);

impl TeamId {
    /// Neutral/observer team
    pub const NEUTRAL: TeamId = TeamId(0);

    /// Team 1 (first player team)
    pub const TEAM_1: TeamId = TeamId(1);

    /// Team 2 (second player team)  
    pub const TEAM_2: TeamId = TeamId(2);

    /// Creates a new team ID, ensuring it's within valid range
    pub fn new(id: u8) -> Option<TeamId> {
        if id <= MAX_PLAYER_COUNT as u8 {
            Some(TeamId(id))
        } else {
            None
        }
    }

    /// Gets the raw team ID value
    pub fn value(self) -> u8 {
        self.0
    }
}

/// Player identifier
#[derive(Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PlayerId(pub u8);

impl PlayerId {
    /// Neutral/observer player
    pub const NEUTRAL: PlayerId = PlayerId(0);

    /// First playable player (Player 1 in the original SAGE enums)
    pub const FIRST: PlayerId = PlayerId(1);

    /// Creates a new player ID, ensuring it's within valid range
    pub fn new(id: u8) -> Option<PlayerId> {
        if id <= MAX_PLAYER_COUNT as u8 {
            Some(PlayerId(id))
        } else {
            None
        }
    }

    /// Gets the raw player ID value
    pub fn value(self) -> u8 {
        self.0
    }

    /// Returns the wrapped value (compatibility with the C++ `Get()` helper).
    pub fn get(self) -> u8 {
        self.value()
    }

    /// Returns the wrapped value as a `u32` for systems that key by `u32`.
    pub fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

impl std::fmt::Debug for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PlayerId({})", self.0)
    }
}

impl std::fmt::Display for PlayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Geometry and positioning
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeometryExtentModType {
    Type,
    Major,
    Minor,
    Height,
}

pub(crate) fn geometry_type_to_u32(geometry_type: EngineGeometryType) -> u32 {
    match geometry_type {
        EngineGeometryType::Sphere => 0,
        EngineGeometryType::Cylinder => 1,
        EngineGeometryType::Box => 2,
    }
}

pub(crate) fn geometry_type_from_u32(value: u32) -> EngineGeometryType {
    match value {
        0 => EngineGeometryType::Sphere,
        1 => EngineGeometryType::Cylinder,
        2 => EngineGeometryType::Box,
        _ => EngineGeometryType::Box,
    }
}

/// Original shape state is stored once, using Common's C++ GeometryInfo.
/// Position and angle are observations of the owning Thing, not shape dimensions.
#[derive(Debug, Clone, Copy)]
pub struct GeometryInfo {
    pub position: Coord3D,
    pub angle: Real,
    pub height_above_terrain: Real,
    pub shape: game_engine::common::system::GeometryInfo,
}

impl Default for GeometryInfo {
    fn default() -> Self {
        Self {
            position: Coord3D::origin(),
            angle: 0.0,
            height_above_terrain: 0.0,
            shape: game_engine::common::system::GeometryInfo::new(
                EngineGeometryType::Box,
                false,
                0.0,
                0.0,
                0.0,
            ),
        }
    }
}

impl GeometryInfo {
    pub fn bounds(&self) -> AABox {
        AABox {
            min: Coord3D::new(
                -self.shape.width,
                -self.shape.depth,
                -self.shape.get_max_height_below_position(),
            ),
            max: Coord3D::new(
                self.shape.width,
                self.shape.depth,
                self.shape.get_max_height_above_position(),
            ),
        }
    }
    /// Admission of an old bounds adapter into the one canonical shape.
    pub fn set_bounds(&mut self, bounds: AABox) {
        self.shape.set(
            self.shape.geometry_type,
            self.shape.is_small,
            bounds.max.z,
            (bounds.max.x - bounds.min.x) * 0.5,
            (bounds.max.y - bounds.min.y) * 0.5,
        );
    }
    pub fn get_geometry_type(&self) -> EngineGeometryType {
        self.shape.geometry_type
    }
    pub fn set_geometry_type(&mut self, value: EngineGeometryType) {
        self.shape.geometry_type = value;
        self.shape.calc_bounding_stuff();
    }
    pub fn get_is_small(&self) -> bool {
        self.shape.is_small
    }
    pub fn set_is_small(&mut self, value: bool) {
        self.shape.is_small = value;
    }
    pub fn get_bounding_sphere_radius(&self) -> Real {
        self.shape.bounding_sphere_radius
    }
    pub fn get_bounding_circle_radius(&self) -> Real {
        self.shape.bounding_circle_radius
    }
    pub fn get_z_delta_to_center_position(&self) -> Real {
        self.shape.get_z_delta_to_center_position()
    }
    pub fn get_major_radius(&self) -> Real {
        self.shape.width
    }
    pub fn get_minor_radius(&self) -> Real {
        self.shape.depth
    }
    pub fn get_footprint_area(&self) -> Real {
        self.shape.get_footprint_area()
    }
    pub fn get_max_height_above_position(&self) -> Real {
        self.shape.get_max_height_above_position()
    }
    pub fn set_max_height_above_position(&mut self, z: Real) {
        self.shape.set_max_height_above_position(z);
    }
    pub fn get_max_height_below_position(&self) -> Real {
        self.shape.get_max_height_below_position()
    }
    pub fn get_center_position(&self, pos: &Coord3D) -> Coord3D {
        Coord3D::new(
            pos.x,
            pos.y,
            pos.z + self.shape.get_z_delta_to_center_position(),
        )
    }
    pub fn calc_pitches(
        &self,
        this_pos: &Coord3D,
        that: &GeometryInfo,
        that_pos: &Coord3D,
    ) -> (Real, Real) {
        let center = self.get_center_position(this_pos);
        let dxy = ((that_pos.x - center.x).powi(2) + (that_pos.y - center.y).powi(2)).sqrt();
        (
            (that_pos.z - that.get_max_height_below_position() - center.z).atan2(dxy),
            (that_pos.z + that.get_max_height_above_position() - center.z).atan2(dxy),
        )
    }
    pub fn tweak_extents(&mut self, kind: GeometryExtentModType, amount: Real) {
        match kind {
            GeometryExtentModType::Major => self.shape.width += amount,
            GeometryExtentModType::Minor => self.shape.depth += amount,
            GeometryExtentModType::Height => self.shape.height += amount,
            GeometryExtentModType::Type => {
                self.shape.geometry_type = match self.shape.geometry_type {
                    EngineGeometryType::Sphere => EngineGeometryType::Cylinder,
                    EngineGeometryType::Cylinder => EngineGeometryType::Box,
                    EngineGeometryType::Box => EngineGeometryType::Sphere,
                }
            }
        }
        self.shape.calc_bounding_stuff();
        self.shape.is_small = false;
    }
    pub fn get_descriptive_string(&self) -> String {
        format!(
            "{}/{}({} {} {})",
            geometry_type_to_u32(self.shape.geometry_type),
            self.shape.is_small as u32,
            self.shape.width,
            self.shape.depth,
            self.shape.get_max_height_above_position()
        )
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy)]
pub struct AABox {
    pub min: Coord3D,
    pub max: Coord3D,
}

impl Default for AABox {
    fn default() -> Self {
        Self {
            min: Coord3D::origin(),
            max: Coord3D::origin(),
        }
    }
}

// Money and resources
/// Money/resource amount type
pub type Money = i32;

/// Health points type
pub type HealthPoints = f32;

/// Angle in radians
pub type Angle = f32;

/// Distance measurement
pub type Distance = f32;

/// Percentage value (0.0 to 1.0)
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct Percentage(f32);

impl Percentage {
    pub fn new(value: f32) -> Self {
        Self(value.clamp(0.0, 1.0))
    }

    pub fn value(self) -> f32 {
        self.0
    }

    pub fn from_percent(percent: f32) -> Self {
        Self::new(percent / 100.0)
    }

    pub fn to_percent(self) -> f32 {
        self.0 * 100.0
    }
}
