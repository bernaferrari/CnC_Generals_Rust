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

/// Geometry information (matching C++ GeometryInfo)
#[derive(Debug, Clone)]
pub struct GeometryInfo {
    pub position: Coord3D,
    pub angle: Real,
    pub bounds: AABox,
    pub height_above_terrain: Real,
    pub geometry_type: EngineGeometryType,
    pub is_small: bool,
}

impl Default for GeometryInfo {
    fn default() -> Self {
        Self {
            position: Coord3D::origin(),
            angle: 0.0,
            bounds: AABox::default(),
            height_above_terrain: 0.0,
            geometry_type: EngineGeometryType::Box,
            is_small: false,
        }
    }
}

impl GeometryInfo {
    pub fn get_geometry_type(&self) -> EngineGeometryType {
        self.geometry_type
    }

    pub fn set_geometry_type(&mut self, geometry_type: EngineGeometryType) {
        self.geometry_type = geometry_type;
    }

    pub fn get_is_small(&self) -> bool {
        self.is_small
    }

    pub fn set_is_small(&mut self, is_small: bool) {
        self.is_small = is_small;
    }

    /// Get the bounding sphere radius (3D, includes height)
    pub fn get_bounding_sphere_radius(&self) -> Real {
        let dx = self.bounds.max.x - self.bounds.min.x;
        let dy = self.bounds.max.y - self.bounds.min.y;
        let dz = self.bounds.max.z - self.bounds.min.z;
        ((dx * dx + dy * dy + dz * dz).sqrt() / 2.0).max(0.0)
    }

    /// Get the bounding circle radius (2D, XY plane only)
    pub fn get_bounding_circle_radius(&self) -> Real {
        let dx = self.bounds.max.x - self.bounds.min.x;
        let dy = self.bounds.max.y - self.bounds.min.y;
        ((dx * dx + dy * dy).sqrt() / 2.0).max(0.0)
    }

    /// Get the major radius (largest XY half-extent).
    pub fn get_major_radius(&self) -> Real {
        let dx = (self.bounds.max.x - self.bounds.min.x).abs();
        let dy = (self.bounds.max.y - self.bounds.min.y).abs();
        (dx.max(dy) * 0.5).max(0.0)
    }

    /// Get the minor radius (smallest XY half-extent).
    pub fn get_minor_radius(&self) -> Real {
        let dx = (self.bounds.max.x - self.bounds.min.x).abs();
        let dy = (self.bounds.max.y - self.bounds.min.y).abs();
        (dx.min(dy) * 0.5).max(0.0)
    }

    /// Get max height above position (matches C++ geometry max height).
    pub fn get_max_height_above_position(&self) -> Real {
        self.bounds.max.z
    }

    /// C++ `GeometryInfo::setMaxHeightAbovePosition`.
    pub fn set_max_height_above_position(&mut self, z: Real) {
        self.bounds.max.z = z;
    }

    /// Get max height below position (matches C++ GeometryInfo::getMaxHeightBelowPosition).
    pub fn get_max_height_below_position(&self) -> Real {
        let below = -self.bounds.min.z;
        if below < 0.0 {
            0.0
        } else {
            below
        }
    }

    /// Get the geometry center position given a base position.
    pub fn get_center_position(&self, pos: &Coord3D) -> Coord3D {
        Coord3D::new(
            pos.x + (self.bounds.min.x + self.bounds.max.x) * 0.5,
            pos.y + (self.bounds.min.y + self.bounds.max.y) * 0.5,
            pos.z + (self.bounds.min.z + self.bounds.max.z) * 0.5,
        )
    }

    /// Calculate min/max pitches from this geometry at `this_pos` to `that` at `that_pos`.
    /// Matches C++ GeometryInfo::calcPitches (Geometry.cpp).
    pub fn calc_pitches(
        &self,
        this_pos: &Coord3D,
        that: &GeometryInfo,
        that_pos: &Coord3D,
    ) -> (Real, Real) {
        let this_center = self.get_center_position(this_pos);
        let dxy =
            ((that_pos.x - this_center.x).powi(2) + (that_pos.y - this_center.y).powi(2)).sqrt();

        let dz_max = (that_pos.z + that.get_max_height_above_position()) - this_center.z;
        let max_pitch = dz_max.atan2(dxy);

        let dz_min = (that_pos.z - that.get_max_height_below_position()) - this_center.z;
        let min_pitch = dz_min.atan2(dxy);

        (min_pitch, max_pitch)
    }

    pub fn tweak_extents(
        &mut self,
        extent_mod_type: GeometryExtentModType,
        extent_mod_amount: Real,
    ) {
        match extent_mod_type {
            GeometryExtentModType::Major => {
                let center_x = (self.bounds.min.x + self.bounds.max.x) * 0.5;
                let center_y = (self.bounds.min.y + self.bounds.max.y) * 0.5;
                let half_x = (self.bounds.max.x - self.bounds.min.x).abs() * 0.5;
                let half_y = (self.bounds.max.y - self.bounds.min.y).abs() * 0.5;
                let radius = self.get_major_radius() + extent_mod_amount;

                if half_x >= half_y {
                    self.bounds.min.x = center_x - radius;
                    self.bounds.max.x = center_x + radius;
                } else {
                    self.bounds.min.y = center_y - radius;
                    self.bounds.max.y = center_y + radius;
                }
            }
            GeometryExtentModType::Minor => {
                let center_x = (self.bounds.min.x + self.bounds.max.x) * 0.5;
                let center_y = (self.bounds.min.y + self.bounds.max.y) * 0.5;
                let half_x = (self.bounds.max.x - self.bounds.min.x).abs() * 0.5;
                let half_y = (self.bounds.max.y - self.bounds.min.y).abs() * 0.5;
                let radius = self.get_minor_radius() + extent_mod_amount;

                if half_x <= half_y {
                    self.bounds.min.x = center_x - radius;
                    self.bounds.max.x = center_x + radius;
                } else {
                    self.bounds.min.y = center_y - radius;
                    self.bounds.max.y = center_y + radius;
                }
            }
            GeometryExtentModType::Height => {
                self.bounds.max.z = self.get_max_height_above_position() + extent_mod_amount;
                if self.bounds.max.z < self.bounds.min.z {
                    self.bounds.min.z = self.bounds.max.z;
                }
            }
            GeometryExtentModType::Type => {
                self.geometry_type = match self.geometry_type {
                    EngineGeometryType::Sphere => EngineGeometryType::Cylinder,
                    EngineGeometryType::Cylinder => EngineGeometryType::Box,
                    EngineGeometryType::Box => EngineGeometryType::Sphere,
                };
            }
        }

        self.is_small = false;
    }

    pub fn get_descriptive_string(&self) -> String {
        format!(
            "{}/{}({} {} {})",
            geometry_type_to_u32(self.geometry_type),
            self.is_small as u32,
            self.get_major_radius(),
            self.get_minor_radius(),
            self.get_max_height_above_position()
        )
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone)]
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

