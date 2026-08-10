// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

/// Water handle for terrain water systems
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WaterHandle(pub u32);

/// CPU mirror of C++ vertex water-grid state.
#[derive(Debug, Clone, PartialEq)]
pub struct WaterGridCpuState {
    pub height_clamps: (f32, f32),
    pub attenuation: (f32, f32, f32, f32),
    pub transform: Mat4,
    pub resolution: (f32, f32, f32),
    pub height_deltas: BTreeMap<(i32, i32), f32>,
    pub point_motions: BTreeMap<(i32, i32), WaterGridPointMotion>,
    pub velocity_events: Vec<WaterGridVelocityEvent>,
}

impl Default for WaterGridCpuState {
    fn default() -> Self {
        Self {
            height_clamps: (0.0, 0.0),
            attenuation: (0.0, 0.0, 0.0, 0.0),
            transform: Mat4::IDENTITY,
            resolution: (0.0, 0.0, 1.0),
            height_deltas: BTreeMap::new(),
            point_motions: BTreeMap::new(),
            velocity_events: Vec::new(),
        }
    }
}

/// CPU mirror of C++ `WaterMeshData` motion fields for a touched grid vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterGridPointMotion {
    pub velocity: f32,
    pub preferred_height: f32,
    pub in_motion: bool,
}

/// CPU record for C++ `addWaterVelocity`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WaterGridVelocityEvent {
    pub world_x: f32,
    pub world_y: f32,
    pub velocity: f32,
    pub preferred_height: f32,
}

/// Owner namespace for C++ terrain bib records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TerrainBibOwnerKind {
    Object,
    Drawable,
}

/// CPU record for C++ `addTerrainBib`/`addTerrainBibDrawable`.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainBibRecord {
    pub owner_id: u32,
    pub owner_kind: TerrainBibOwnerKind,
    pub corners: [[f32; 3]; 4],
    pub highlight: bool,
}

/// CPU record for C++ `W3DTerrainRenderObject::addProp`.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainPropRecord {
    pub position: [f32; 3],
    pub angle: f32,
    pub scale: f32,
    pub model_name: String,
}

/// CPU record for C++ `removeTreesAndPropsForConstruction`.
#[derive(Debug, Clone, PartialEq)]
pub struct TerrainConstructionRemoval {
    pub position: [f32; 3],
    pub major_radius: f32,
    pub minor_radius: f32,
    pub geometry_is_box: bool,
    pub angle: f32,
}

/// Runtime road segment descriptor passed from game-logic map parsing.
#[derive(Debug, Clone)]
pub struct RuntimeRoadVisualSegment {
    pub start: [f32; 3],
    pub end: [f32; 3],
    pub width: f32,
    pub template_name: String,
    pub width_in_texture: f32,
    pub road_type_id: u32,
    pub start_is_angled: bool,
    pub start_is_join: bool,
    pub end_is_angled: bool,
    pub end_is_join: bool,
    pub curve_radius: f32,
}

#[derive(Debug, Clone, Copy)]
struct RuntimeRoadEndpointTopology {
    start_count: u32,
    end_count: u32,
    start_last: bool,
    end_last: bool,
}

impl Default for RuntimeRoadEndpointTopology {
    fn default() -> Self {
        Self {
            start_count: 0,
            end_count: 0,
            start_last: true,
            end_last: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RuntimeRoadIntersectionKind {
    Tee,
    FourWay,
}

impl RuntimeRoadIntersectionKind {
    fn from_endpoint_count(count: u32) -> Option<Self> {
        match count {
            2 => Some(Self::Tee),
            3 => Some(Self::FourWay),
            _ => None,
        }
    }

    fn max(self, other: Self) -> Self {
        match (self, other) {
            (Self::FourWay, _) | (_, Self::FourWay) => Self::FourWay,
            _ => Self::Tee,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Tee => "Tee",
            Self::FourWay => "FourWay",
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeRoadIntersectionCandidate {
    road_type_id: u32,
    kind: RuntimeRoadIntersectionKind,
    anchor_sum: Vec3,
    contribution_count: usize,
    road_width: f32,
    width_in_texture: f32,
    direction_sum: Vec3,
    fallback_direction: Option<Vec3>,
}

impl RuntimeRoadIntersectionCandidate {
    fn new(
        road_type_id: u32,
        kind: RuntimeRoadIntersectionKind,
        anchor: Vec3,
        road_width: f32,
        width_in_texture: f32,
        direction: Vec3,
    ) -> Self {
        Self {
            road_type_id,
            kind,
            anchor_sum: anchor,
            contribution_count: 1,
            road_width,
            width_in_texture,
            direction_sum: direction,
            fallback_direction: Some(direction),
        }
    }

    fn add_contribution(
        &mut self,
        anchor: Vec3,
        road_width: f32,
        width_in_texture: f32,
        direction: Vec3,
        kind: RuntimeRoadIntersectionKind,
    ) {
        self.kind = self.kind.max(kind);
        self.anchor_sum += anchor;
        self.contribution_count += 1;
        self.road_width = self.road_width.max(road_width);
        self.width_in_texture = self.width_in_texture.max(width_in_texture);
        self.direction_sum += direction;
        if self.fallback_direction.is_none() {
            self.fallback_direction = Some(direction);
        }
    }

    fn into_runtime_segment(self) -> Option<RuntimeRoadVisualSegment> {
        if self.contribution_count == 0 {
            return None;
        }

        let anchor = self.anchor_sum / self.contribution_count as f32;
        let mut direction = if self.direction_sum.length_squared() > 1.0e-6 {
            self.direction_sum.normalize()
        } else {
            self.fallback_direction.unwrap_or(Vec3::ZERO)
        };
        direction.y = 0.0;
        direction = direction.normalize_or_zero();
        if direction.length_squared() <= 1.0e-6 {
            return None;
        }

        let total_length = (self.road_width.max(1.0)
            * match self.kind {
                RuntimeRoadIntersectionKind::Tee => 0.35,
                RuntimeRoadIntersectionKind::FourWay => 0.5,
            })
        .max(1.0);
        let offset = direction * (total_length * 0.5);
        let start = anchor - offset;
        let end = anchor + offset;
        if (end - start).length_squared() <= 1.0e-4 {
            return None;
        }

        Some(RuntimeRoadVisualSegment {
            start: start.to_array(),
            end: end.to_array(),
            width: self.road_width.max(0.1),
            template_name: format!(
                "SyntheticIntersection_{}_{}",
                self.road_type_id,
                self.kind.label()
            ),
            width_in_texture: self.width_in_texture.max(0.0),
            road_type_id: self.road_type_id,
            start_is_angled: false,
            start_is_join: true,
            end_is_angled: false,
            end_is_join: true,
            curve_radius: 0.0,
        })
    }
}

/// Terrain LOD levels matching C++ implementation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TerrainVisualLOD {
    Invalid = 0,
    Min = 1,
    StretchNoClouds = 2,
    HalfClouds = 3,
    NoClouds = 4,
    StretchClouds = 5,
    NoWater = 6,
    Max = 7,
    #[default]
    Automatic = 8,
    Disable = 9,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerrainSourceTileClass {
    pub first_tile: i32,
    pub num_tiles: i32,
    pub width: i32,
    pub name: String,
}

/// Seismic simulation for dynamic terrain effects
#[derive(Debug, Clone)]
pub struct SeismicSimulationNode {
    pub center: Vec3,
    pub radius: f32,
    pub region: (Vec3, Vec3), // min, max
    pub clean: bool,
    pub magnitude: f32,
    pub life: u32,
}

impl SeismicSimulationNode {
    pub fn new(center: Vec3, radius: f32, magnitude: f32) -> Self {
        let region_size = radius;
        Self {
            center,
            radius: (radius - 1.0),
            region: (
                Vec3::new(center.x - region_size, center.y, center.z - region_size),
                Vec3::new(center.x + region_size, center.y, center.z + region_size),
            ),
            clean: false,
            magnitude,
            life: 0,
        }
    }
}

