//! Terrain Road System
//!
//! Manages road networks, pathways, and other linear infrastructure
//! elements that modify terrain appearance and affect gameplay.

use crate::terrain::{TerrainError, TerrainResult};
use gamelogic::common::types::{MAP_HEIGHT_SCALE, MAP_XY_FACTOR};
use glam::{Mat4, Vec3};
use nalgebra::Point2;
use std::collections::HashMap;
use wgpu::RenderPass;

/// Unique identifier for roads
pub type RoadId = u32;

/// Unique identifier for road segments
pub type RoadSegmentId = u32;

/// Road network containing connected road segments
#[derive(Debug, Clone)]
pub struct Road {
    /// Unique identifier
    pub id: RoadId,

    /// Display name
    pub name: String,

    /// Road type determining appearance and properties
    pub road_type: RoadType,

    /// Connected road segments forming the road
    pub segments: Vec<RoadSegmentId>,

    /// Whether road affects unit movement
    pub affects_movement: bool,

    /// Movement speed modifier for units on this road
    pub speed_modifier: f32,

    /// Road priority for intersection rendering
    pub priority: u8,

    /// Whether road is visible
    pub visible: bool,
}

/// Individual road segment between control points
#[derive(Debug, Clone)]
pub struct RoadSegment {
    /// Unique identifier
    pub id: RoadSegmentId,

    /// Parent road ID
    pub road_id: RoadId,

    /// Start point in world coordinates
    pub start: Vec3,

    /// End point in world coordinates
    pub end: Vec3,

    /// Control points for curve generation (Bezier curve)
    pub control_points: Vec<Vec3>,

    /// Road width in world units
    pub width: f32,

    /// Segment properties
    pub properties: RoadSegmentProperties,

    /// Generated geometry for rendering
    pub geometry: Option<RoadGeometry>,

    /// Whether geometry needs regeneration
    pub dirty: bool,
}

/// Properties for road segments
#[derive(Debug, Clone)]
pub struct RoadSegmentProperties {
    /// Surface elevation above terrain
    pub elevation: f32,

    /// Banking angle for curves (radians)
    pub banking: f32,

    /// Surface roughness (affects movement)
    pub roughness: f32,

    /// Whether segment has guardrails
    pub has_guardrails: bool,

    /// Whether segment has road markings
    pub has_markings: bool,

    /// Custom texture override
    pub texture_override: Option<String>,

    /// Synthetic intersection origin used for runtime tee/four-way reconstruction.
    pub synthetic_intersection: Option<RoadSyntheticIntersectionKind>,

    /// C++ parity metadata: number of same-type segments sharing start endpoint.
    pub endpoint_start_count: u8,
    /// C++ parity metadata: number of same-type segments sharing end endpoint.
    pub endpoint_end_count: u8,
    /// C++ parity metadata: whether this start endpoint is the latest endpoint in the chain.
    pub endpoint_start_last: bool,
    /// C++ parity metadata: whether this end endpoint is the latest endpoint in the chain.
    pub endpoint_end_last: bool,
    /// C++ parity metadata: whether start endpoint is shared by more than one segment.
    pub endpoint_start_multi: bool,
    /// C++ parity metadata: whether end endpoint is shared by more than one segment.
    pub endpoint_end_multi: bool,
}

/// Synthetic runtime intersections inserted for parity passes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoadSyntheticIntersectionKind {
    Tee,
    FourWay,
    ThreeWayY,
    ThreeWayH,
    ThreeWayHFlip,
}

/// Types of roads with different appearances
#[derive(Debug, Clone, PartialEq)]
pub enum RoadType {
    /// Dirt path or trail
    DirtPath { wear_factor: f32 },

    /// Gravel road
    GravelRoad { stone_size: f32, compaction: f32 },

    /// Paved asphalt road
    AsphaltRoad {
        condition: RoadCondition,
        lane_markings: bool,
    },

    /// Concrete highway
    ConcreteHighway {
        condition: RoadCondition,
        barriers: bool,
        lanes: u8,
    },

    /// Wooden bridge
    WoodenBridge {
        plank_width: f32,
        support_spacing: f32,
    },

    /// Stone bridge
    StoneBridge {
        arch_count: u8,
        stone_type: StoneType,
    },

    /// Railroad tracks
    Railroad {
        rail_gauge: f32,
        tie_spacing: f32,
        electrified: bool,
    },
}

/// Road condition affecting appearance
#[derive(Debug, Clone, PartialEq)]
pub enum RoadCondition {
    /// Well maintained
    Excellent,
    /// Some wear but functional
    Good,
    /// Visible damage, some potholes
    Fair,
    /// Significant damage
    Poor,
    /// Heavily damaged, barely passable
    Destroyed,
}

/// Stone types for bridges and structures
#[derive(Debug, Clone, PartialEq)]
pub enum StoneType {
    Granite,
    Limestone,
    Sandstone,
    Brick,
}

/// Generated geometry for road rendering
#[derive(Debug, Clone)]
pub struct RoadGeometry {
    /// Vertices forming road surface
    pub vertices: Vec<RoadVertex>,

    /// Indices for triangle rendering
    pub indices: Vec<u32>,

    /// UV coordinates for texturing
    pub uvs: Vec<Point2<f32>>,

    /// Vertex colors for detail
    pub colors: Vec<[f32; 4]>,

    /// C++ road-strip lateral sample positions for each collapsed two-vertex row.
    pub row_height_samples: Vec<Vec<Vec3>>,

    /// Geometry for road edges/shoulders
    pub edge_geometry: Option<EdgeGeometry>,

    /// Geometry for road markings
    pub marking_geometry: Option<MarkingGeometry>,
}

/// Vertex data for road surfaces
#[derive(Debug, Clone)]
pub struct RoadVertex {
    /// World position
    pub position: [f32; 3],

    /// Surface normal
    pub normal: [f32; 3],

    /// Texture coordinates
    pub tex_coords: [f32; 2],

    /// Vertex color
    pub color: [f32; 4],

    /// Distance along road (for animated textures)
    pub road_distance: f32,
}

/// Minimap-ready road sample point.
#[derive(Debug, Clone, Copy)]
pub struct RoadMinimapSample {
    pub position: Vec3,
    pub width: f32,
    pub tint_rgb: [u8; 3],
}

/// High level road management system

/// Geometry for road edges and shoulders
#[derive(Debug, Clone)]
pub struct EdgeGeometry {
    pub vertices: Vec<RoadVertex>,
    pub indices: Vec<u32>,
    pub width: f32,
}

/// Geometry for road markings and signs
#[derive(Debug, Clone)]
pub struct MarkingGeometry {
    pub vertices: Vec<RoadVertex>,
    pub indices: Vec<u32>,
    pub marking_type: MarkingType,
}

/// Types of road markings
#[derive(Debug, Clone, PartialEq)]
pub enum MarkingType {
    /// Center line dividing traffic
    CenterLine,
    /// Lane separator
    LaneLine,
    /// Edge line marking road boundary
    EdgeLine,
    /// Crosswalk markings
    Crosswalk,
    /// Stop line at intersections
    StopLine,
    /// Directional arrows
    Arrow(ArrowType),
}

/// Types of directional arrows
#[derive(Debug, Clone, PartialEq)]
pub enum ArrowType {
    Straight,
    Left,
    Right,
    LeftStraight,
    RightStraight,
    UTurn,
}

/// Road intersection handling multiple road connections
#[derive(Debug, Clone)]
pub struct RoadIntersection {
    /// Unique identifier
    pub id: u32,

    /// World position of intersection center
    pub position: Vec3,

    /// Connected road segments
    pub connected_segments: Vec<RoadSegmentId>,

    /// Intersection type
    pub intersection_type: IntersectionType,

    /// Traffic control (if any)
    pub traffic_control: Option<TrafficControl>,

    /// Custom geometry for intersection
    pub geometry: Option<RoadGeometry>,
}

/// Types of road intersections
#[derive(Debug, Clone, PartialEq)]
pub enum IntersectionType {
    /// Simple T-junction
    TJunction,
    /// Four-way intersection
    Crossroads,
    /// Roundabout
    Roundabout { radius: f32 },
    /// Highway on/off ramp
    Ramp,
}

/// Traffic control systems at intersections
#[derive(Debug, Clone, PartialEq)]
pub enum TrafficControl {
    /// No traffic control
    None,
    /// Stop signs
    StopSign,
    /// Traffic lights
    TrafficLight {
        cycle_time: f32,
        current_phase: TrafficPhase,
    },
    /// Yield signs
    YieldSign,
}

/// Traffic light phases
#[derive(Debug, Clone, PartialEq)]
pub enum TrafficPhase {
    NorthSouthGreen,
    NorthSouthYellow,
    EastWestGreen,
    EastWestYellow,
    AllRed,
}

/// Manages road networks and rendering
#[derive(Debug)]
pub struct RoadManager {
    /// All roads in the system
    roads: HashMap<RoadId, Road>,

    /// All road segments
    segments: HashMap<RoadSegmentId, RoadSegment>,

    /// Road intersections
    intersections: HashMap<u32, RoadIntersection>,

    /// Next available IDs
    next_road_id: RoadId,
    next_segment_id: RoadSegmentId,
    next_intersection_id: u32,

    /// Road generation parameters
    generation_config: RoadGenerationConfig,

    /// Performance statistics
    stats: RoadStats,

    /// Tracks whether road geometry changed and needs terrain-normal reprojection.
    terrain_normals_dirty: bool,
}

/// Configuration for road generation
#[derive(Debug, Clone)]
pub struct RoadGenerationConfig {
    /// Default road width
    pub default_width: f32,

    /// Curve tessellation resolution
    pub curve_resolution: u32,

    /// Maximum curve deviation
    pub max_curve_deviation: f32,

    /// Minimum segment length
    pub min_segment_length: f32,

    /// Intersection merge distance
    pub intersection_merge_distance: f32,

    /// Whether to generate edge geometry
    pub generate_edges: bool,

    /// Whether to generate road markings
    pub generate_markings: bool,
}

/// Performance statistics for road system
#[derive(Debug, Default)]
pub struct RoadStats {
    pub total_roads: u32,
    pub total_segments: u32,
    pub total_intersections: u32,
    pub total_vertices: u32,
    pub total_triangles: u32,
    pub geometry_memory: u64,
    pub generation_time: std::time::Duration,
    pub render_calls: u64,
}

impl Default for RoadSegmentProperties {
    fn default() -> Self {
        Self {
            elevation: 0.0,
            banking: 0.0,
            roughness: 0.1,
            has_guardrails: false,
            has_markings: false,
            texture_override: None,
            synthetic_intersection: None,
            endpoint_start_count: 0,
            endpoint_end_count: 0,
            endpoint_start_last: true,
            endpoint_end_last: true,
            endpoint_start_multi: false,
            endpoint_end_multi: false,
        }
    }
}

impl Default for RoadGenerationConfig {
    fn default() -> Self {
        Self {
            default_width: 8.0,
            curve_resolution: 16,
            max_curve_deviation: 2.0,
            min_segment_length: 5.0,
            intersection_merge_distance: 15.0,
            generate_edges: true,
            generate_markings: true,
        }
    }
}

impl Road {
    /// Create new road
    pub fn new(id: RoadId, name: String, road_type: RoadType) -> Self {
        Self {
            id,
            name,
            road_type,
            segments: Vec::new(),
            affects_movement: true,
            speed_modifier: 1.2, // Roads typically increase movement speed
            priority: 0,
            visible: true,
        }
    }

    /// Add segment to road
    pub fn add_segment(&mut self, segment_id: RoadSegmentId) {
        if !self.segments.contains(&segment_id) {
            self.segments.push(segment_id);
        }
    }

    /// Remove segment from road
    pub fn remove_segment(&mut self, segment_id: RoadSegmentId) {
        self.segments.retain(|&id| id != segment_id);
    }

    /// Get total road length
    pub fn get_length(&self, segments: &HashMap<RoadSegmentId, RoadSegment>) -> f32 {
        self.segments
            .iter()
            .filter_map(|&id| segments.get(&id))
            .map(|segment| segment.get_length())
            .sum()
    }
}

impl RoadSegment {
    const ROAD_FLOAT_HEIGHT_BIAS: f32 = MAP_HEIGHT_SCALE / 8.0;
    const TEE_WIDTH_ADJUSTMENT: f32 = 1.03;
    const CORNER_RADIUS: f32 = 1.5;
    const TIGHT_CORNER_RADIUS: f32 = 0.5;

    fn runtime_texture_override_value(&self, key: &str) -> Option<&str> {
        let metadata = self.properties.texture_override.as_deref()?;
        metadata.split_whitespace().find_map(|token| {
            let (k, v) = token.split_once('=')?;
            if k == key {
                Some(v)
            } else {
                None
            }
        })
    }

    fn runtime_texture_override_f32(&self, key: &str) -> Option<f32> {
        self.runtime_texture_override_value(key)
            .and_then(|value| value.parse::<f32>().ok())
            .filter(|value| value.is_finite())
    }

    fn runtime_kind_is(&self, expected: &str) -> bool {
        self.runtime_texture_override_value("Kind")
            .map(|kind| kind.eq_ignore_ascii_case(expected))
            .unwrap_or(false)
    }

    fn runtime_linear_resolution_for_length(length: f32) -> u32 {
        (((length.max(0.0) / MAP_XY_FACTOR) as u32).saturating_add(1)).max(2)
    }

    fn runtime_surface_resolution(
        &self,
        config: &RoadGenerationConfig,
        segment_length: f32,
    ) -> u32 {
        let linear = Self::runtime_linear_resolution_for_length(segment_length);
        if self.control_points.is_empty() {
            linear
        } else {
            config.curve_resolution.max(linear)
        }
    }

    fn runtime_aux_resolution(&self, segment_length: f32) -> u32 {
        let linear = Self::runtime_linear_resolution_for_length(segment_length);
        if self.control_points.is_empty() {
            linear
        } else {
            16u32.max(linear)
        }
    }

    fn horizontal_direction(&self) -> Vec3 {
        let mut direction = self.end - self.start;
        direction.y = 0.0;
        if direction.length_squared() <= 1.0e-6 {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            direction.normalize()
        }
    }

    /// C++ parity: W3DRoadBuffer::loadFloat4PtSection
    ///
    /// Tessellates a quadrilateral into a strip mesh using bilinear interpolation,
    /// matching the C++ algorithm exactly.  Produces a uCount×2 collapsed grid
    /// (bottom row + top row per column) ready for terrain projection and
    /// strip-row collapse in the follow-up `apply_terrain_heights_and_normals`
    /// pass.
    ///
    /// The bilinear formula mirrors C++:
    /// ```text
    ///   P(i,j) = origin
    ///          + uVector1 * iFactor * (1-jFactor)
    ///          + uVector2 * iFactor * jFactor
    ///          + vVector1 * (1-iFactor) * jFactor
    ///          + vVector2 * iFactor * jFactor
    /// ```
    /// with `uVector2 += (vVector1 - vVector2)` perspective correction.
    ///
    /// UV coordinates are computed via dot-product mapping against the
    /// normalised road direction and road normal, exactly as C++ does.
    fn generate_float4pt_strip_geometry(
        &self,
        loc: Vec3,
        road_normal: Vec3,
        road_vector: Vec3,
        corners: [Vec3; 4], // [bottomLeft, bottomRight, topLeft, topRight]
        u_offset: f32,
        v_offset: f32,
        u_scale: f32,
        v_scale: f32,
    ) -> TerrainResult<RoadGeometry> {
        const MAX_SEG_VERTEX: usize = 500;
        const MAX_SEG_INDEX: usize = 2000;
        const MAX_ROWS: u32 = 100;

        let road_len = road_vector.length().max(1.0e-6);
        let half_height = road_normal.length().max(1.0e-6);

        let mut road_vector_dir = road_vector / road_len;
        road_vector_dir.y = 0.0;
        if road_vector_dir.length_squared() > 1.0e-6 {
            road_vector_dir = road_vector_dir.normalize();
        } else {
            road_vector_dir = Vec3::new(1.0, 0.0, 0.0);
        }

        let mut road_normal_dir = road_normal / half_height;
        road_normal_dir.y = 0.0;
        if road_normal_dir.length_squared() > 1.0e-6 {
            road_normal_dir = road_normal_dir.normalize();
        } else {
            road_normal_dir = Vec3::new(-road_vector_dir.z, 0.0, road_vector_dir.x);
            if road_normal_dir.length_squared() > 1.0e-6 {
                road_normal_dir = road_normal_dir.normalize();
            } else {
                road_normal_dir = Vec3::new(0.0, 0.0, 1.0);
            }
        }

        let u_count = (((road_len / MAP_XY_FACTOR) as u32).saturating_add(1)).max(2);
        let _v_count = (((2.0 * half_height) / MAP_XY_FACTOR) as u32)
            .saturating_add(1)
            .max(2)
            .min(MAX_ROWS);

        let elevation = self.properties.elevation;
        let u_scale = u_scale.max(1.0e-6);
        let v_scale = v_scale.max(1.0e-6);

        let origin = corners[0];
        let u_vector1 = corners[1] - corners[0];
        let mut u_vector2 = corners[3] - corners[2];
        let v_vector1 = corners[2] - corners[0];
        let v_vector2 = corners[3] - corners[1];
        u_vector2 += v_vector1 - v_vector2;

        let mut vertices = Vec::with_capacity((u_count as usize * 2).min(MAX_SEG_VERTEX));
        let mut row_height_samples = Vec::with_capacity(u_count as usize);
        let mut indices =
            Vec::with_capacity(((u_count.saturating_sub(1)) as usize * 6).min(MAX_SEG_INDEX));

        for i in 0..u_count {
            let i_factor = if u_count > 1 {
                i as f32 / (u_count - 1) as f32
            } else {
                0.0
            };
            let i_bar = 1.0 - i_factor;

            let bottom_pos = origin + u_vector1 * i_factor;

            let top_pos = origin + u_vector2 * i_factor + v_vector1 * i_bar + v_vector2 * i_factor;

            let mut row_samples = Vec::with_capacity(_v_count as usize);
            for j in 0.._v_count {
                let j_factor = if _v_count > 1 {
                    j as f32 / (_v_count - 1) as f32
                } else {
                    0.0
                };
                let j_bar = 1.0 - j_factor;
                row_samples.push(
                    origin
                        + u_vector1 * j_bar * i_factor
                        + u_vector2 * j_factor * i_factor
                        + v_vector1 * i_bar * j_factor
                        + v_vector2 * i_factor * j_factor,
                );
            }

            if vertices.len() + 2 > MAX_SEG_VERTEX {
                break;
            }

            let bottom_cur = Vec3::new(bottom_pos.x - loc.x, 0.0, bottom_pos.z - loc.z);
            let top_cur = Vec3::new(top_pos.x - loc.x, 0.0, top_pos.z - loc.z);
            let bottom_u = road_vector_dir.dot(bottom_cur);
            let bottom_v = road_normal_dir.dot(bottom_cur);
            let top_u = road_vector_dir.dot(top_cur);
            let top_v = road_normal_dir.dot(top_cur);

            vertices.push(RoadVertex {
                position: [bottom_pos.x, bottom_pos.y + elevation, bottom_pos.z],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [
                    u_offset + bottom_u / (u_scale * 4.0),
                    v_offset - bottom_v / (v_scale * 4.0),
                ],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: road_len * i_factor,
            });
            vertices.push(RoadVertex {
                position: [top_pos.x, top_pos.y + elevation, top_pos.z],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [
                    u_offset + top_u / (u_scale * 4.0),
                    v_offset - top_v / (v_scale * 4.0),
                ],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: road_len * i_factor,
            });
            row_height_samples.push(row_samples);

            if i > 0 && indices.len() + 6 <= MAX_SEG_INDEX {
                let base = i * 2;
                indices.extend_from_slice(&[base - 2, base - 1, base, base - 1, base + 1, base]);
            }
        }

        let uvs = vertices
            .iter()
            .map(|vertex| Point2::new(vertex.tex_coords[0], vertex.tex_coords[1]))
            .collect();
        let colors = vertices.iter().map(|vertex| vertex.color).collect();
        Ok(RoadGeometry {
            vertices,
            indices,
            uvs,
            colors,
            row_height_samples,
            edge_geometry: None,
            marking_geometry: None,
        })
    }

    fn generate_float_section(
        &self,
        loc: Vec3,
        road_vector_in: Vec3,
        half_height: f32,
        left: f32,
        right: f32,
        u_offset: f32,
        v_offset: f32,
        scale: f32,
    ) -> TerrainResult<RoadGeometry> {
        let mut road_vector = Vec3::new(road_vector_in.x, 0.0, road_vector_in.z);
        if road_vector.length_squared() <= 1.0e-6 {
            road_vector = Vec3::new(1.0, 0.0, 0.0);
        } else {
            road_vector = road_vector.normalize();
        }
        let mut road_normal = Vec3::new(-road_vector.z, 0.0, road_vector.x);
        if road_normal.length_squared() <= 1.0e-6 {
            road_normal = Vec3::new(0.0, 0.0, 1.0);
        } else {
            road_normal = road_normal.normalize();
        }

        road_vector *= right;
        road_normal *= half_height.abs();
        let mut road_left = road_vector;
        road_left = road_left.normalize_or_zero() * left;
        road_vector += road_left;
        let left_center = loc - road_left;

        let bottom_left = left_center - road_normal;
        let bottom_right = bottom_left + road_vector;
        let top_right = bottom_right + road_normal * 2.0;
        let top_left = bottom_left + road_normal * 2.0;

        self.generate_float4pt_strip_geometry(
            loc,
            road_normal,
            road_vector,
            [bottom_left, bottom_right, top_left, top_right],
            u_offset,
            v_offset,
            scale,
            scale,
        )
    }

    fn generate_synthetic_intersection_geometry(
        &self,
        kind: RoadSyntheticIntersectionKind,
    ) -> TerrainResult<RoadGeometry> {
        match kind {
            RoadSyntheticIntersectionKind::Tee | RoadSyntheticIntersectionKind::FourWay => {}
            RoadSyntheticIntersectionKind::ThreeWayY => {
                return self.generate_three_way_y_geometry()
            }
            RoadSyntheticIntersectionKind::ThreeWayH => {
                return self.generate_three_way_h_geometry(false);
            }
            RoadSyntheticIntersectionKind::ThreeWayHFlip => {
                return self.generate_three_way_h_geometry(true);
            }
        }

        let loc1 = self.start;
        let loc2 = self.end;
        let road_vector = loc2 - loc1;
        let scale = self.width.max(0.1);
        let width_in_texture = self
            .runtime_texture_override_f32("WidthInTexture")
            .unwrap_or(1.0)
            .max(0.1);
        let tee_factor = scale * Self::TEE_WIDTH_ADJUSTMENT / 2.0;
        let left = width_in_texture * scale / 2.0;
        let right = tee_factor;
        let u_offset = 425.0 / 512.0;
        let v_offset = match kind {
            RoadSyntheticIntersectionKind::Tee => 255.0 / 512.0,
            RoadSyntheticIntersectionKind::FourWay => 425.0 / 512.0,
            RoadSyntheticIntersectionKind::ThreeWayY
            | RoadSyntheticIntersectionKind::ThreeWayH
            | RoadSyntheticIntersectionKind::ThreeWayHFlip => unreachable!(),
        };

        self.generate_float_section(
            loc1,
            road_vector,
            tee_factor,
            left,
            right,
            u_offset,
            v_offset,
            scale,
        )
    }

    fn road_frame_for_w3d_join(&self) -> (Vec3, Vec3, f32) {
        let mut road_vector = self.end - self.start;
        road_vector.y = 0.0;
        let mut road_normal = Vec3::new(-road_vector.z, 0.0, road_vector.x);
        if road_vector.x.abs() < 1.0e-6 && road_vector.z.abs() < 1.0e-6 {
            road_vector = Vec3::new(1.0, 0.0, 0.0);
            road_normal = Vec3::new(0.0, 0.0, 1.0);
        } else {
            road_vector = road_vector.normalize();
            road_normal = road_normal.normalize_or_zero();
        }

        (road_vector, road_normal, self.width.max(0.1))
    }

    fn generate_three_way_y_geometry(&self) -> TerrainResult<RoadGeometry> {
        const U_OFFSET: f32 = 255.0 / 512.0;
        const V_OFFSET: f32 = 226.0 / 512.0;

        let (mut road_vector, mut road_normal, scale) = self.road_frame_for_w3d_join();
        road_vector *= scale;
        road_normal *= scale;
        road_vector *= 1.59;

        let loc = self.start;
        let top_left = loc + road_normal * 0.29 - road_vector * 0.5;
        let bottom_left = top_left - road_normal * 1.08;
        let bottom_right = bottom_left + road_vector;
        let top_right = top_left + road_vector;

        self.generate_float4pt_strip_geometry(
            loc,
            road_normal,
            road_vector,
            [bottom_left, bottom_right, top_left, top_right],
            U_OFFSET,
            V_OFFSET,
            scale,
            scale,
        )
    }

    fn generate_three_way_h_geometry(&self, flip: bool) -> TerrainResult<RoadGeometry> {
        const U_OFFSET: f32 = 202.0 / 512.0;
        const V_OFFSET: f32 = 364.0 / 512.0;

        let (mut road_vector, mut road_normal, scale) = self.road_frame_for_w3d_join();
        let width_in_texture = self
            .runtime_texture_override_f32("WidthInTexture")
            .unwrap_or(1.0)
            .max(0.1);

        road_vector *= scale;
        road_normal *= scale;
        road_normal *= 1.35;

        let loc = self.start;
        let bottom_left = if flip {
            loc - road_normal * 0.20 - road_vector * width_in_texture / 2.0
        } else {
            loc - road_normal * 0.8 - road_vector * width_in_texture / 2.0
        };
        let width = road_vector * width_in_texture / 2.0 + road_vector * 1.2;
        let bottom_right = bottom_left + width;
        let top_right = bottom_right + road_normal;
        let top_left = bottom_left + road_normal;
        if flip {
            road_normal = -road_normal;
        }

        self.generate_float4pt_strip_geometry(
            loc,
            road_normal,
            road_vector,
            [bottom_left, bottom_right, top_left, top_right],
            U_OFFSET,
            V_OFFSET,
            scale,
            scale,
        )
    }

    fn generate_alpha_join_geometry(&self) -> TerrainResult<RoadGeometry> {
        const U_OFFSET: f32 = 106.0 / 512.0;
        const V_OFFSET: f32 = 425.0 / 512.0;
        const ALONG_SPAN: f32 = 48.0 / 128.0;
        const EXTRA_NORMAL_SCALE: f32 = 1.0 + (8.0 / 128.0);

        let mut road_dir = self.end - self.start;
        road_dir.y = 0.0;
        if road_dir.length_squared() <= 1.0e-6 {
            road_dir = Vec3::new(1.0, 0.0, 0.0);
        } else {
            road_dir = road_dir.normalize();
        }
        let mut road_normal = Vec3::new(-road_dir.z, 0.0, road_dir.x);
        if road_normal.length_squared() <= 1.0e-6 {
            road_normal = Vec3::new(0.0, 0.0, 1.0);
        } else {
            road_normal = road_normal.normalize();
        }

        let scale = self.width.max(0.1);
        let width_in_texture = self
            .runtime_texture_override_f32("WidthInTexture")
            .unwrap_or(1.0)
            .max(0.1);

        let road_vec = road_dir * (scale * ALONG_SPAN);
        let normal_vec = road_normal * (width_in_texture * EXTRA_NORMAL_SCALE);

        let anchor = self.start;
        let top_left = anchor + normal_vec * 0.5 - road_vec * 0.65;
        let bottom_left = top_left - normal_vec;
        let top_right = top_left + road_vec;
        let bottom_right = bottom_left + road_vec;
        let up_normal = [0.0, 1.0, 0.0];
        let y_bias = self.properties.elevation;

        let _ = up_normal;
        let _ = y_bias;
        self.generate_float4pt_strip_geometry(
            anchor,
            normal_vec,
            road_vec,
            [bottom_left, bottom_right, top_left, top_right],
            U_OFFSET,
            V_OFFSET,
            scale,
            width_in_texture,
        )
    }

    fn generate_w3d_segment_geometry(&self) -> TerrainResult<RoadGeometry> {
        const U_OFFSET: f32 = 0.0;
        const V_OFFSET: f32 = 85.0 / 512.0;

        let road_vector = self.end - self.start;
        let mut road_normal = Vec3::new(-road_vector.z, 0.0, road_vector.x);
        if road_normal.length_squared() <= 1.0e-6 {
            road_normal = Vec3::new(0.0, 0.0, 1.0);
        } else {
            road_normal = road_normal.normalize();
        }

        let scale = self.width.max(0.1);
        let width_in_texture = self
            .runtime_texture_override_f32("WidthInTexture")
            .unwrap_or(1.0)
            .max(0.1);
        road_normal *= width_in_texture * scale / 2.0;

        let bottom_left = self.start - road_normal;
        let top_left = self.start + road_normal;
        let bottom_right = self.end - road_normal;
        let top_right = self.end + road_normal;

        self.generate_float4pt_strip_geometry(
            self.start,
            road_normal,
            road_vector,
            [bottom_left, bottom_right, top_left, top_right],
            U_OFFSET,
            V_OFFSET,
            scale,
            scale,
        )
    }

    fn generate_curve_geometry(&self) -> TerrainResult<RoadGeometry> {
        let mut road_vector = self.end - self.start;
        road_vector.y = 0.0;
        let mut road_normal = Vec3::new(-road_vector.z, 0.0, road_vector.x);
        if road_vector.length_squared() <= 1.0e-6 {
            road_vector = Vec3::new(1.0, 0.0, 0.0);
            road_normal = Vec3::new(0.0, 0.0, 1.0);
        } else {
            road_vector = road_vector.normalize();
            road_normal = road_normal.normalize_or_zero();
        }

        let curve_radius = self
            .runtime_texture_override_f32("CurveRadius")
            .unwrap_or(Self::CORNER_RADIUS);
        let mut v_offset = 255.0 / 512.0;
        if (curve_radius - Self::TIGHT_CORNER_RADIUS).abs() <= 0.05 {
            v_offset = 425.0 / 512.0;
        }
        let u_offset = 4.0 / 512.0;
        let scale = self.width.max(0.1);
        let width_in_texture = self
            .runtime_texture_override_f32("WidthInTexture")
            .unwrap_or(1.0)
            .max(0.1);
        let curve_height = width_in_texture * scale / 2.0;

        road_vector *= scale;
        road_normal *= curve_height.abs();
        let loc1 = self.start;
        let mut bottom_left = loc1 - road_normal;
        let mut bottom_right = bottom_left + road_vector;
        let mut top_right = bottom_right + road_normal * 2.0;
        let mut top_left = bottom_left + road_normal * 2.0;

        if (curve_radius - Self::TIGHT_CORNER_RADIUS).abs() <= 0.05 {
            bottom_right = bottom_left + road_vector * 0.5;
            top_right = bottom_right + road_normal * 2.0;
            top_left = bottom_left + road_normal * 2.0;
            bottom_right += road_vector * 0.1;
            bottom_right += road_normal * 0.2;
            bottom_left -= road_normal * 0.1;
            bottom_left -= road_vector * 0.02;
            top_left -= road_vector * 0.02;
            top_right -= road_vector * 0.4;
            top_right += road_normal * 0.2;
        } else {
            bottom_right += road_vector * 0.1;
            bottom_right += road_normal * 0.4;
            bottom_left -= road_normal * 0.2;
            bottom_left -= road_vector * 0.02;
            top_left -= road_vector * 0.02;
            top_right -= road_vector * 0.4;
            top_right += road_normal * 0.4;
        }

        self.generate_float4pt_strip_geometry(
            loc1,
            road_normal,
            road_vector,
            [bottom_left, bottom_right, top_left, top_right],
            u_offset,
            v_offset,
            scale,
            scale,
        )
    }

    /// Create new road segment
    pub fn new(id: RoadSegmentId, road_id: RoadId, start: Vec3, end: Vec3, width: f32) -> Self {
        Self {
            id,
            road_id,
            start,
            end,
            control_points: Vec::new(),
            width,
            properties: RoadSegmentProperties::default(),
            geometry: None,
            dirty: true,
        }
    }

    /// Add control point for curved segment
    pub fn add_control_point(&mut self, point: Vec3) {
        self.control_points.push(point);
        self.dirty = true;
    }

    /// Get segment length
    pub fn get_length(&self) -> f32 {
        if self.control_points.is_empty() {
            // Straight line distance
            (self.end - self.start).length()
        } else {
            // Approximate curve length using control points
            let mut length = 0.0;
            let mut prev_point = self.start;

            // Sample points along curve
            for i in 1..=20 {
                let t = i as f32 / 20.0;
                let point = self.sample_curve(t);
                length += (point - prev_point).length();
                prev_point = point;
            }

            length
        }
    }

    /// Sample point along curve at parameter t [0,1]
    pub fn sample_curve(&self, t: f32) -> Vec3 {
        if self.control_points.is_empty() {
            // Linear interpolation for straight segment
            self.start + t * (self.end - self.start)
        } else if self.control_points.len() == 1 {
            // Quadratic Bezier curve
            let p0 = self.start;
            let p1 = self.control_points[0];
            let p2 = self.end;

            let inv_t = 1.0 - t;
            p0 * (inv_t * inv_t) + p1 * (2.0 * inv_t * t) + p2 * (t * t)
        } else {
            // Cubic Bezier curve (using first two control points)
            let p0 = self.start;
            let p1 = self.control_points[0];
            let p2 = self.control_points[1];
            let p3 = self.end;

            let inv_t = 1.0 - t;
            let inv_t2 = inv_t * inv_t;
            let inv_t3 = inv_t2 * inv_t;
            let t2 = t * t;
            let t3 = t2 * t;

            p0 * inv_t3 + p1 * (3.0 * inv_t2 * t) + p2 * (3.0 * inv_t * t2) + p3 * t3
        }
    }

    fn sample_tangent_for_resolution(&self, i: u32, resolution: u32) -> Vec3 {
        let tangent = if i < resolution.saturating_sub(1) {
            let next_t = if resolution == 2 {
                1.0
            } else {
                (i + 1) as f32 / (resolution - 1) as f32
            };
            let center_t = if resolution == 2 {
                i as f32
            } else {
                i as f32 / (resolution - 1) as f32
            };
            let center_point = self.sample_curve(center_t);
            let next_point = self.sample_curve(next_t);
            next_point - center_point
        } else {
            let prev_t = if resolution > 1 {
                (i.saturating_sub(1)) as f32 / (resolution - 1) as f32
            } else {
                0.0
            };
            let center_t = if resolution == 2 {
                i as f32
            } else {
                i as f32 / (resolution - 1) as f32
            };
            let center_point = self.sample_curve(center_t);
            let prev_point = self.sample_curve(prev_t);
            center_point - prev_point
        };

        if tangent.length_squared() > 1.0e-8 {
            tangent.normalize()
        } else {
            Vec3::new(1.0, 0.0, 0.0)
        }
    }

    /// Build a stable right/normal basis from a tangent.
    /// Keeps the resulting normal oriented upward when possible for consistent lighting.
    fn build_frame_from_tangent(tangent: Vec3) -> (Vec3, Vec3) {
        let up = Vec3::new(0.0, 1.0, 0.0);
        let mut right = tangent.cross(up);
        if right.length_squared() <= 1.0e-8 {
            right = tangent.cross(Vec3::new(1.0, 0.0, 0.0));
        }
        if right.length_squared() <= 1.0e-8 {
            right = Vec3::new(0.0, 0.0, 1.0);
        } else {
            right = right.normalize();
        }

        let mut normal = right.cross(tangent);
        if normal.length_squared() <= 1.0e-8 {
            normal = up;
        } else {
            normal = normal.normalize();
        }

        if normal.y < 0.0 {
            normal = -normal;
            right = -right;
        }

        (right, normal)
    }

    /// Generate geometry for this segment
    pub fn generate_geometry(&mut self, config: &RoadGenerationConfig) -> TerrainResult<()> {
        if self.runtime_kind_is("ALPHA_JOIN") {
            self.geometry = Some(self.generate_alpha_join_geometry()?);
            self.dirty = false;
            return Ok(());
        }
        if self.runtime_kind_is("SEGMENT") {
            self.geometry = Some(self.generate_w3d_segment_geometry()?);
            self.dirty = false;
            return Ok(());
        }
        if self.runtime_kind_is("CURVE") {
            self.geometry = Some(self.generate_curve_geometry()?);
            self.dirty = false;
            return Ok(());
        }
        if let Some(kind) = self.properties.synthetic_intersection {
            self.geometry = Some(self.generate_synthetic_intersection_geometry(kind)?);
            self.dirty = false;
            return Ok(());
        }

        let segment_length = self.get_length();
        let resolution = self.runtime_surface_resolution(config, segment_length);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();
        let mut row_height_samples = Vec::new();

        let half_width = self.width / 2.0;
        let v_count = (((self.width.max(0.0) / MAP_XY_FACTOR) as u32).saturating_add(1))
            .max(2)
            .min(100);

        // Generate vertices along the segment
        for i in 0..resolution {
            let t = if resolution == 2 {
                i as f32
            } else {
                i as f32 / (resolution - 1) as f32
            };
            let center_point = self.sample_curve(t);
            let tangent = self.sample_tangent_for_resolution(i, resolution);
            let (right, normal) = Self::build_frame_from_tangent(tangent);

            // Create vertices for left and right edges
            let left_point = center_point + right * half_width;
            let right_point = center_point - right * half_width;
            let distance_along_road = (t * segment_length) / self.width; // For texture tiling
            let mut row_samples = Vec::with_capacity(v_count as usize);
            for j in 0..v_count {
                let j_factor = if v_count > 1 {
                    j as f32 / (v_count - 1) as f32
                } else {
                    0.0
                };
                row_samples.push(left_point.lerp(right_point, j_factor));
            }

            // Left vertex
            vertices.push(RoadVertex {
                position: [
                    left_point.x,
                    left_point.y + self.properties.elevation,
                    left_point.z,
                ],
                normal: normal.into(),
                tex_coords: [0.0, distance_along_road],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: t * segment_length,
            });

            // Right vertex
            vertices.push(RoadVertex {
                position: [
                    right_point.x,
                    right_point.y + self.properties.elevation,
                    right_point.z,
                ],
                normal: normal.into(),
                tex_coords: [1.0, distance_along_road],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: t * segment_length,
            });
            row_height_samples.push(row_samples);
        }

        // Generate indices for triangle strips
        for i in 0..(resolution - 1) {
            let base = i * 2;

            // First triangle
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 1);

            // Second triangle
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base + 3);
        }

        // Create geometry
        self.geometry = Some(RoadGeometry {
            vertices,
            indices,
            uvs,
            colors,
            row_height_samples,
            edge_geometry: if config.generate_edges {
                Some(self.generate_edge_geometry()?)
            } else {
                None
            },
            marking_geometry: if config.generate_markings && self.properties.has_markings {
                Some(self.generate_marking_geometry()?)
            } else {
                None
            },
        });

        self.dirty = false;
        Ok(())
    }

    /// Generate edge geometry (shoulders, guardrails)
    fn generate_edge_geometry(&self) -> TerrainResult<EdgeGeometry> {
        let segment_length = self.get_length();
        let resolution = self.runtime_aux_resolution(segment_length);
        let edge_width = (self.width * 0.2).max(1.0);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for i in 0..resolution {
            let t = if resolution == 2 {
                i as f32
            } else {
                i as f32 / (resolution - 1) as f32
            };

            let center_point = self.sample_curve(t);
            let tangent = self.sample_tangent_for_resolution(i, resolution);
            let (right, normal) = Self::build_frame_from_tangent(tangent);

            let half_width = self.width / 2.0;
            let inner_left = center_point + right * half_width;
            let inner_right = center_point - right * half_width;
            let outer_left = inner_left + right * edge_width;
            let outer_right = inner_right - right * edge_width;

            let distance_along_road = (t * segment_length) / self.width;

            // Left edge strip (inner -> outer)
            vertices.push(RoadVertex {
                position: [
                    inner_left.x,
                    inner_left.y + self.properties.elevation,
                    inner_left.z,
                ],
                normal: normal.into(),
                tex_coords: [0.0, distance_along_road],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: t * segment_length,
            });
            vertices.push(RoadVertex {
                position: [
                    outer_left.x,
                    outer_left.y + self.properties.elevation,
                    outer_left.z,
                ],
                normal: normal.into(),
                tex_coords: [1.0, distance_along_road],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: t * segment_length,
            });

            // Right edge strip (outer -> inner)
            vertices.push(RoadVertex {
                position: [
                    outer_right.x,
                    outer_right.y + self.properties.elevation,
                    outer_right.z,
                ],
                normal: normal.into(),
                tex_coords: [0.0, distance_along_road],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: t * segment_length,
            });
            vertices.push(RoadVertex {
                position: [
                    inner_right.x,
                    inner_right.y + self.properties.elevation,
                    inner_right.z,
                ],
                normal: normal.into(),
                tex_coords: [1.0, distance_along_road],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: t * segment_length,
            });
        }

        for i in 0..(resolution - 1) {
            let base = i * 4;

            // Left strip
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 1);
            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base + 3);

            // Right strip
            let right_base = base + 2;
            indices.push(right_base);
            indices.push(right_base + 2);
            indices.push(right_base + 1);
            indices.push(right_base + 1);
            indices.push(right_base + 2);
            indices.push(right_base + 3);
        }

        Ok(EdgeGeometry {
            vertices,
            indices,
            width: edge_width,
        })
    }

    /// Generate marking geometry (lines, arrows)
    fn generate_marking_geometry(&self) -> TerrainResult<MarkingGeometry> {
        let segment_length = self.get_length();
        let resolution = self.runtime_aux_resolution(segment_length);
        let mark_width = (self.width * 0.05).max(0.1);

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for i in 0..resolution {
            let t = if resolution == 2 {
                i as f32
            } else {
                i as f32 / (resolution - 1) as f32
            };

            let center_point = self.sample_curve(t);
            let tangent = self.sample_tangent_for_resolution(i, resolution);
            let (right, normal) = Self::build_frame_from_tangent(tangent);

            let left = center_point + right * (mark_width * 0.5);
            let right_pt = center_point - right * (mark_width * 0.5);
            let distance_along_road = (t * segment_length) / self.width;

            vertices.push(RoadVertex {
                position: [left.x, left.y + self.properties.elevation + 0.02, left.z],
                normal: normal.into(),
                tex_coords: [0.0, distance_along_road],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: t * segment_length,
            });
            vertices.push(RoadVertex {
                position: [
                    right_pt.x,
                    right_pt.y + self.properties.elevation + 0.02,
                    right_pt.z,
                ],
                normal: normal.into(),
                tex_coords: [1.0, distance_along_road],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: t * segment_length,
            });
        }

        for i in 0..(resolution - 1) {
            let base = i * 2;
            indices.push(base);
            indices.push(base + 2);
            indices.push(base + 1);

            indices.push(base + 1);
            indices.push(base + 2);
            indices.push(base + 3);
        }

        Ok(MarkingGeometry {
            vertices,
            indices,
            marking_type: MarkingType::CenterLine,
        })
    }
}

impl RoadManager {
    fn road_minimap_tint(road_type: &RoadType) -> [u8; 3] {
        match road_type {
            RoadType::DirtPath { .. } => [138, 112, 77],
            RoadType::GravelRoad { .. } => [129, 123, 111],
            RoadType::AsphaltRoad { .. } => [124, 124, 128],
            RoadType::ConcreteHighway { .. } => [148, 146, 140],
            RoadType::WoodenBridge { .. } => [142, 102, 69],
            RoadType::StoneBridge { .. } => [130, 126, 120],
            RoadType::Railroad { .. } => [106, 98, 88],
        }
    }

    fn sanitize_sampled_normal(sampled: Vec3, fallback: [f32; 3]) -> [f32; 3] {
        let mut n = if sampled.length_squared() > 1.0e-8 && sampled.is_finite() {
            sampled.normalize()
        } else {
            Vec3::from_array(fallback)
        };

        if !n.is_finite() || n.length_squared() <= 1.0e-8 {
            n = Vec3::new(0.0, 1.0, 0.0);
        }
        if n.y < 0.0 {
            n = -n;
        }
        n.to_array()
    }

    fn sanitize_sampled_height(sampled: f32, fallback: f32) -> f32 {
        if sampled.is_finite() {
            sampled
        } else {
            fallback
        }
    }

    fn project_vertices_to_terrain<F>(
        vertices: &mut [RoadVertex],
        overlay_height: f32,
        clamp_rows: bool,
        row_height_samples: Option<&[Vec<Vec3>]>,
        sample_height: &mut F,
    ) where
        F: FnMut(Vec3) -> f32,
    {
        if clamp_rows {
            let mut pairs = vertices.chunks_exact_mut(2);
            for (row, pair) in (&mut pairs).enumerate() {
                let pos_a = Vec3::new(
                    pair[0].position[0],
                    pair[0].position[1],
                    pair[0].position[2],
                );
                let pos_b = Vec3::new(
                    pair[1].position[0],
                    pair[1].position[1],
                    pair[1].position[2],
                );
                let max_height = row_height_samples
                    .and_then(|samples| samples.get(row))
                    .filter(|samples| !samples.is_empty())
                    .map(|samples| {
                        samples.iter().fold(f32::NEG_INFINITY, |max_height, pos| {
                            let sampled = Self::sanitize_sampled_height(sample_height(*pos), pos.y);
                            max_height.max(sampled)
                        })
                    })
                    .filter(|height| height.is_finite())
                    .unwrap_or_else(|| {
                        let h_a = Self::sanitize_sampled_height(sample_height(pos_a), pos_a.y);
                        let h_b = Self::sanitize_sampled_height(sample_height(pos_b), pos_b.y);
                        h_a.max(h_b)
                    });
                let projected = max_height + RoadSegment::ROAD_FLOAT_HEIGHT_BIAS + overlay_height;
                pair[0].position[1] = projected;
                pair[1].position[1] = projected;
            }

            for vertex in pairs.into_remainder().iter_mut() {
                let pos = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                let sampled = Self::sanitize_sampled_height(sample_height(pos), pos.y);
                vertex.position[1] = sampled + RoadSegment::ROAD_FLOAT_HEIGHT_BIAS + overlay_height;
            }
        } else {
            for vertex in vertices.iter_mut() {
                let pos = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                let sampled = Self::sanitize_sampled_height(sample_height(pos), pos.y);
                vertex.position[1] = sampled + RoadSegment::ROAD_FLOAT_HEIGHT_BIAS + overlay_height;
            }
        }
    }

    fn reproject_vertex_normals<F>(vertices: &mut [RoadVertex], sample_normal: &mut F)
    where
        F: FnMut(Vec3) -> Vec3,
    {
        for vertex in vertices {
            let pos = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
            let sampled = sample_normal(pos);
            vertex.normal = Self::sanitize_sampled_normal(sampled, vertex.normal);
        }
    }

    fn apply_vertex_diffuse<F>(vertices: &mut [RoadVertex], sample_diffuse: &mut F)
    where
        F: FnMut(Vec3) -> [f32; 4],
    {
        for vertex in vertices {
            let pos = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
            let mut color = sample_diffuse(pos);
            for channel in &mut color {
                if !channel.is_finite() {
                    *channel = 1.0;
                } else {
                    *channel = channel.clamp(0.0, 1.0);
                }
            }
            color[3] = 1.0;
            vertex.color = color;
        }
    }

    fn refresh_geometry_colors(geometry: &mut RoadGeometry) {
        geometry.colors = geometry
            .vertices
            .iter()
            .map(|vertex| vertex.color)
            .collect();
    }

    fn rebuild_strip_indices(indices: &mut Vec<u32>, row_count: u32) {
        indices.clear();
        if row_count < 2 {
            return;
        }
        indices.reserve(((row_count - 1) * 6) as usize);
        for i in 0..(row_count - 1) {
            let base = i * 2;
            indices.extend_from_slice(&[base, base + 2, base + 1, base + 1, base + 2, base + 3]);
        }
    }

    fn collapse_strip_rows(vertices: &mut Vec<RoadVertex>, indices: &mut Vec<u32>) {
        if vertices.len() < 6 || !vertices.len().is_multiple_of(2) {
            return;
        }

        let row_count = vertices.len() / 2;
        let max_error = MAP_HEIGHT_SCALE * 1.1;
        let mut keep = vec![true; row_count];
        let mut prev_kept = 0usize;

        for cur in 1..(row_count - 1) {
            if !keep[cur] {
                continue;
            }
            let mut next = cur + 1;
            while next < row_count && !keep[next] {
                next += 1;
            }
            if next >= row_count {
                break;
            }

            let prev_u = prev_kept as f32;
            let cur_u = cur as f32;
            let next_u = next as f32;
            let denom = (next_u - prev_u).max(1.0e-6);

            let prev_top = vertices[prev_kept * 2].position[1];
            let cur_top = vertices[cur * 2].position[1];
            let next_top = vertices[next * 2].position[1];
            let interp_top = (prev_top * (cur_u - prev_u) + next_top * (next_u - cur_u)) / denom;

            let prev_bottom = vertices[prev_kept * 2 + 1].position[1];
            let cur_bottom = vertices[cur * 2 + 1].position[1];
            let next_bottom = vertices[next * 2 + 1].position[1];
            let interp_bottom =
                (prev_bottom * (cur_u - prev_u) + next_bottom * (next_u - cur_u)) / denom;

            let top_ok = interp_top >= cur_top && interp_top < (cur_top + max_error);
            let bottom_ok = interp_bottom >= cur_bottom && interp_bottom < (cur_bottom + max_error);
            if top_ok && bottom_ok {
                keep[cur] = false;
                continue;
            }

            prev_kept = cur;
        }

        let kept_rows = keep.iter().filter(|&&k| k).count();
        if kept_rows == row_count {
            Self::rebuild_strip_indices(indices, row_count as u32);
            return;
        }

        let mut compacted = Vec::with_capacity(kept_rows * 2);
        for (row, keep_row) in keep.into_iter().enumerate() {
            if !keep_row {
                continue;
            }
            compacted.push(vertices[row * 2].clone());
            compacted.push(vertices[row * 2 + 1].clone());
        }
        *vertices = compacted;
        Self::rebuild_strip_indices(indices, (vertices.len() / 2) as u32);
    }

    /// Initialize road system resources and prebuild geometry.
    pub fn init(&mut self) -> TerrainResult<()> {
        // Match C++ preload-style behavior: rebuild any pending/dirty geometry when the
        // subsystem comes online so first render has deterministic data.
        self.update_geometry()?;
        Ok(())
    }

    /// Reset road data to defaults
    pub fn reset(&mut self) -> TerrainResult<()> {
        self.roads.clear();
        self.segments.clear();
        self.intersections.clear();
        self.next_road_id = 1;
        self.next_segment_id = 1;
        self.next_intersection_id = 1;
        self.stats = RoadStats::default();
        self.terrain_normals_dirty = false;
        Ok(())
    }

    /// Update road system geometry and stats.
    pub fn update(&mut self) -> TerrainResult<()> {
        let start_time = std::time::Instant::now();
        let mut total_vertices = 0u32;
        let mut total_triangles = 0u32;
        let mut total_memory = 0u64;
        let mut regenerated_any = false;

        for segment in self.segments.values_mut() {
            if segment.dirty {
                segment.generate_geometry(&self.generation_config)?;
                segment.dirty = false;
                regenerated_any = true;
            }

            if let Some(ref geometry) = segment.geometry {
                total_vertices = total_vertices.saturating_add(geometry.vertices.len() as u32);
                total_triangles =
                    total_triangles.saturating_add((geometry.indices.len() / 3) as u32);
                total_memory = total_memory.saturating_add(
                    (geometry.vertices.len() * std::mem::size_of::<RoadVertex>()) as u64
                        + (geometry.indices.len() * std::mem::size_of::<u32>()) as u64
                        + (geometry.uvs.len() * std::mem::size_of::<Point2<f32>>()) as u64
                        + (geometry.colors.len() * std::mem::size_of::<[f32; 4]>()) as u64,
                );
            }
        }

        self.stats.total_vertices = total_vertices;
        self.stats.total_triangles = total_triangles;
        self.stats.geometry_memory = total_memory;
        self.stats.generation_time = start_time.elapsed();
        if regenerated_any {
            self.terrain_normals_dirty = true;
        }
        Ok(())
    }

    /// Reproject generated road normals against terrain normals sampled in world space.
    ///
    /// This closes the tangent-only fallback and lets roads follow live terrain lighting.
    pub fn apply_terrain_normals<F>(&mut self, mut sample_normal: F)
    where
        F: FnMut(Vec3) -> Vec3,
    {
        if !self.terrain_normals_dirty {
            return;
        }

        for segment in self.segments.values_mut() {
            let Some(geometry) = segment.geometry.as_mut() else {
                continue;
            };

            Self::reproject_vertex_normals(&mut geometry.vertices, &mut sample_normal);

            if let Some(edge) = geometry.edge_geometry.as_mut() {
                Self::reproject_vertex_normals(&mut edge.vertices, &mut sample_normal);
            }

            if let Some(marking) = geometry.marking_geometry.as_mut() {
                Self::reproject_vertex_normals(&mut marking.vertices, &mut sample_normal);
            }
        }

        self.terrain_normals_dirty = false;
    }

    /// Reproject road vertices onto terrain heights and refresh normals.
    ///
    /// This mirrors C++ `loadFloat4PtSection` behavior where road meshes float slightly above
    /// terrain and each strip row is clamped to the maximum sampled height to avoid clipping.
    pub fn apply_terrain_heights_and_normals<FH, FN>(
        &mut self,
        mut sample_height: FH,
        mut sample_normal: FN,
    ) where
        FH: FnMut(Vec3) -> f32,
        FN: FnMut(Vec3) -> Vec3,
    {
        if !self.terrain_normals_dirty {
            return;
        }

        for segment in self.segments.values_mut() {
            let Some(geometry) = segment.geometry.as_mut() else {
                continue;
            };
            // C++ `loadFloat4PtSection` collapses projected rows for all road strips,
            // including synthetic tee/four-way joins and alpha caps.
            let clamp_surface_rows = true;

            Self::project_vertices_to_terrain(
                &mut geometry.vertices,
                segment.properties.elevation,
                clamp_surface_rows,
                Some(&geometry.row_height_samples),
                &mut sample_height,
            );
            if clamp_surface_rows {
                Self::collapse_strip_rows(&mut geometry.vertices, &mut geometry.indices);
            }
            Self::reproject_vertex_normals(&mut geometry.vertices, &mut sample_normal);

            if let Some(edge) = geometry.edge_geometry.as_mut() {
                Self::project_vertices_to_terrain(
                    &mut edge.vertices,
                    segment.properties.elevation,
                    true,
                    None,
                    &mut sample_height,
                );
                Self::reproject_vertex_normals(&mut edge.vertices, &mut sample_normal);
            }

            if let Some(marking) = geometry.marking_geometry.as_mut() {
                Self::project_vertices_to_terrain(
                    &mut marking.vertices,
                    segment.properties.elevation + 0.02,
                    true,
                    None,
                    &mut sample_height,
                );
                Self::reproject_vertex_normals(&mut marking.vertices, &mut sample_normal);
            }
        }

        self.terrain_normals_dirty = false;
    }

    /// Reproject road vertices and refresh C++-style static diffuse lighting.
    ///
    /// C++ `RoadSegment::updateSegLighting` samples `getStaticDiffuse` for each road
    /// vertex after terrain projection and stores an opaque diffuse color in the vertex
    /// buffer. This variant lets terrain callers provide that same world-space diffuse
    /// sample while keeping the existing height/normal-only API intact.
    pub fn apply_terrain_heights_normals_and_diffuse<FH, FN, FD>(
        &mut self,
        mut sample_height: FH,
        mut sample_normal: FN,
        mut sample_diffuse: FD,
    ) where
        FH: FnMut(Vec3) -> f32,
        FN: FnMut(Vec3) -> Vec3,
        FD: FnMut(Vec3) -> [f32; 4],
    {
        if !self.terrain_normals_dirty {
            return;
        }

        for segment in self.segments.values_mut() {
            let Some(geometry) = segment.geometry.as_mut() else {
                continue;
            };

            Self::project_vertices_to_terrain(
                &mut geometry.vertices,
                segment.properties.elevation,
                true,
                Some(&geometry.row_height_samples),
                &mut sample_height,
            );
            Self::collapse_strip_rows(&mut geometry.vertices, &mut geometry.indices);
            Self::reproject_vertex_normals(&mut geometry.vertices, &mut sample_normal);
            Self::apply_vertex_diffuse(&mut geometry.vertices, &mut sample_diffuse);
            Self::refresh_geometry_colors(geometry);

            if let Some(edge) = geometry.edge_geometry.as_mut() {
                Self::project_vertices_to_terrain(
                    &mut edge.vertices,
                    segment.properties.elevation,
                    true,
                    None,
                    &mut sample_height,
                );
                Self::reproject_vertex_normals(&mut edge.vertices, &mut sample_normal);
                Self::apply_vertex_diffuse(&mut edge.vertices, &mut sample_diffuse);
            }

            if let Some(marking) = geometry.marking_geometry.as_mut() {
                Self::project_vertices_to_terrain(
                    &mut marking.vertices,
                    segment.properties.elevation + 0.02,
                    true,
                    None,
                    &mut sample_height,
                );
                Self::reproject_vertex_normals(&mut marking.vertices, &mut sample_normal);
                Self::apply_vertex_diffuse(&mut marking.vertices, &mut sample_diffuse);
            }
        }

        self.terrain_normals_dirty = false;
    }

    pub fn needs_terrain_normal_reprojection(&self) -> bool {
        self.terrain_normals_dirty
    }

    pub fn invalidate_terrain_lighting(&mut self) {
        self.terrain_normals_dirty = true;
    }

    /// Validate road geometry for render submission.
    ///
    /// Checks all segment geometries (surface, edge, marking) for valid triangle counts.
    /// GPU submission is performed by `render_pass_draw`, which calls this validation
    /// internally and then iterates per-mesh draw calls.  Mirrors C++ W3DTerrainVisual
    /// road rendering which validates geometry before submitting indexed draws.
    pub fn render(&self, _view: &Mat4, _projection: &Mat4) -> TerrainResult<()> {
        for segment in self.segments.values() {
            let Some(geometry) = segment.geometry.as_ref() else {
                continue;
            };

            Self::validate_geometry(segment.id, "surface", &geometry.vertices, &geometry.indices)?;

            if let Some(edge) = geometry.edge_geometry.as_ref() {
                Self::validate_geometry(segment.id, "edge", &edge.vertices, &edge.indices)?;
            }
            if let Some(marking) = geometry.marking_geometry.as_ref() {
                Self::validate_geometry(
                    segment.id,
                    "marking",
                    &marking.vertices,
                    &marking.indices,
                )?;
            }
        }
        Ok(())
    }

    /// Submit GPU draw calls for all visible road surfaces.
    ///
    /// Caller must set the road pipeline and camera bind group (group 0) first.
    /// `mesh_iter` yields (vertex_slice, index_slice, index_count) per road mesh in
    /// priority order.  Each mesh results in one `draw_indexed` call, matching C++
    /// W3DTerrainVisual per-road-mesh submission.
    pub fn render_pass_draw<'a, FMeshes>(
        &self,
        render_pass: &mut RenderPass<'a>,
        mut mesh_iter: FMeshes,
    ) -> TerrainResult<()>
    where
        FMeshes: FnMut() -> Option<(wgpu::BufferSlice<'a>, wgpu::BufferSlice<'a>, u32)>,
    {
        self.render(&Mat4::IDENTITY, &Mat4::IDENTITY)?;

        while let Some((vertex_slice, index_slice, index_count)) = mesh_iter() {
            if index_count == 0 {
                continue;
            }
            render_pass.set_vertex_buffer(0, vertex_slice);
            render_pass.set_index_buffer(index_slice, wgpu::IndexFormat::Uint32);
            render_pass.draw_indexed(0..index_count, 0, 0..1);
        }

        Ok(())
    }

    /// Create new road manager
    pub fn new() -> Self {
        Self {
            roads: HashMap::new(),
            segments: HashMap::new(),
            intersections: HashMap::new(),
            next_road_id: 1,
            next_segment_id: 1,
            next_intersection_id: 1,
            generation_config: RoadGenerationConfig::default(),
            stats: RoadStats::default(),
            terrain_normals_dirty: false,
        }
    }

    /// Create new road
    pub fn create_road(&mut self, name: String, road_type: RoadType) -> RoadId {
        let road = Road::new(self.next_road_id, name, road_type);
        let id = road.id;

        self.roads.insert(id, road);
        self.next_road_id += 1;
        self.stats.total_roads += 1;

        id
    }

    /// Create new road segment
    pub fn create_segment(
        &mut self,
        road_id: RoadId,
        start: Vec3,
        end: Vec3,
        width: Option<f32>,
    ) -> TerrainResult<RoadSegmentId> {
        let width = width.unwrap_or(self.generation_config.default_width);
        let segment = RoadSegment::new(self.next_segment_id, road_id, start, end, width);
        let id = segment.id;

        // Add segment to road
        if let Some(road) = self.roads.get_mut(&road_id) {
            road.add_segment(id);
        } else {
            return Err(TerrainError::InvalidData(format!(
                "Road {} not found",
                road_id
            )));
        }

        self.segments.insert(id, segment);
        self.next_segment_id += 1;
        self.stats.total_segments += 1;
        self.terrain_normals_dirty = true;

        Ok(id)
    }

    /// Generate geometry for all dirty segments
    pub fn update_geometry(&mut self) -> TerrainResult<()> {
        let start_time = std::time::Instant::now();

        let dirty_segments: Vec<RoadSegmentId> = self
            .segments
            .values()
            .filter(|segment| segment.dirty)
            .map(|segment| segment.id)
            .collect();
        let rebuilt_any = !dirty_segments.is_empty();

        for segment_id in dirty_segments {
            if let Some(segment) = self.segments.get_mut(&segment_id) {
                segment.generate_geometry(&self.generation_config)?;
            }
        }

        self.update_statistics();
        self.stats.generation_time = start_time.elapsed();
        if rebuilt_any {
            self.terrain_normals_dirty = true;
        }

        Ok(())
    }

    /// Get road by ID
    pub fn get_road(&self, road_id: RoadId) -> Option<&Road> {
        self.roads.get(&road_id)
    }

    /// Get mutable road by ID
    pub fn get_road_mut(&mut self, road_id: RoadId) -> Option<&mut Road> {
        self.roads.get_mut(&road_id)
    }

    /// Get segment by ID
    pub fn get_segment(&self, segment_id: RoadSegmentId) -> Option<&RoadSegment> {
        self.segments.get(&segment_id)
    }

    /// Get mutable segment by ID
    pub fn get_segment_mut(&mut self, segment_id: RoadSegmentId) -> Option<&mut RoadSegment> {
        self.segments.get_mut(&segment_id)
    }

    /// Find roads near a world position
    pub fn find_roads_near(&self, position: Vec3, radius: f32) -> Vec<RoadId> {
        self.roads
            .values()
            .filter(|road| {
                road.segments
                    .iter()
                    .filter_map(|&id| self.segments.get(&id))
                    .any(|segment| {
                        let center = (segment.start + segment.end) * 0.5;
                        (center - position).length() <= radius
                    })
            })
            .map(|road| road.id)
            .collect()
    }

    /// Update road system statistics
    fn update_statistics(&mut self) {
        let mut total_vertices = 0;
        let mut total_triangles = 0;
        let mut geometry_memory = 0;

        for segment in self.segments.values() {
            if let Some(geometry) = &segment.geometry {
                total_vertices += geometry.vertices.len() as u32;
                total_triangles += geometry.indices.len() as u32 / 3;
                geometry_memory += std::mem::size_of_val(&*geometry.vertices) as u64;
                geometry_memory += std::mem::size_of_val(&*geometry.indices) as u64;
            }
        }

        self.stats.total_vertices = total_vertices;
        self.stats.total_triangles = total_triangles;
        self.stats.geometry_memory = geometry_memory;
    }

    /// Get performance statistics
    pub fn get_stats(&self) -> &RoadStats {
        &self.stats
    }

    /// Iterate visible road surface geometries for renderer submission.
    pub fn for_each_visible_surface_geometry<F>(&self, mut visitor: F)
    where
        F: FnMut(f32, &[RoadVertex], &[u32]),
    {
        let mut ordered: Vec<(u8, RoadSegmentId)> = Vec::new();
        for segment in self.segments.values() {
            let Some(road) = self.roads.get(&segment.road_id) else {
                continue;
            };
            if !road.visible {
                continue;
            }
            let Some(geometry) = segment.geometry.as_ref() else {
                continue;
            };
            if geometry.vertices.is_empty() || geometry.indices.is_empty() {
                continue;
            }

            ordered.push((road.priority, segment.id));
        }

        ordered.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));

        for (_, segment_id) in ordered {
            let Some(segment) = self.segments.get(&segment_id) else {
                continue;
            };
            let Some(geometry) = segment.geometry.as_ref() else {
                continue;
            };
            if geometry.vertices.is_empty() || geometry.indices.is_empty() {
                continue;
            }
            visitor(segment.width, &geometry.vertices, &geometry.indices);
        }
    }

    /// Export sampled road points for minimap/static map overlays.
    pub fn snapshot_minimap_samples(&self, samples_per_segment: u32) -> Vec<RoadMinimapSample> {
        let samples_per_segment = samples_per_segment.max(2);
        let mut samples = Vec::new();

        for segment in self.segments.values() {
            let Some(road) = self.roads.get(&segment.road_id) else {
                continue;
            };
            if !road.visible {
                continue;
            }
            let tint_rgb = Self::road_minimap_tint(&road.road_type);

            for i in 0..=samples_per_segment {
                let t = i as f32 / samples_per_segment as f32;
                let pos = segment.sample_curve(t);
                samples.push(RoadMinimapSample {
                    position: pos,
                    width: segment.width.max(0.1),
                    tint_rgb,
                });
            }
        }

        samples
    }

    /// Clear all roads and segments
    pub fn clear(&mut self) {
        self.roads.clear();
        self.segments.clear();
        self.intersections.clear();
        self.next_road_id = 1;
        self.next_segment_id = 1;
        self.next_intersection_id = 1;
        self.stats = RoadStats::default();
        self.terrain_normals_dirty = false;
    }

    fn validate_geometry(
        segment_id: RoadSegmentId,
        label: &str,
        vertices: &[RoadVertex],
        indices: &[u32],
    ) -> TerrainResult<()> {
        if !indices.len().is_multiple_of(3) {
            return Err(TerrainError::InvalidData(format!(
                "Road segment {} {} geometry has non-triangle index count {}",
                segment_id,
                label,
                indices.len()
            )));
        }

        let vertex_count = vertices.len();
        for index in indices {
            if (*index as usize) >= vertex_count {
                return Err(TerrainError::InvalidData(format!(
                    "Road segment {} {} geometry index {} out of bounds {}",
                    segment_id, label, index, vertex_count
                )));
            }
        }

        Ok(())
    }
}

impl Default for RoadManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Legacy-friendly alias aligning with C++ naming
pub type RoadSystem = RoadManager;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_road_creation() {
        let road = Road::new(
            1,
            "Main Street".to_string(),
            RoadType::AsphaltRoad {
                condition: RoadCondition::Good,
                lane_markings: true,
            },
        );

        assert_eq!(road.id, 1);
        assert_eq!(road.name, "Main Street");
        assert!(road.segments.is_empty());
    }

    #[test]
    fn test_segment_creation() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(100.0, 0.0, 0.0);
        let segment = RoadSegment::new(1, 1, start, end, 8.0);

        assert_eq!(segment.id, 1);
        assert_eq!(segment.road_id, 1);
        assert_eq!(segment.width, 8.0);
        assert!(segment.dirty);
    }

    #[test]
    fn test_segment_length() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(100.0, 0.0, 0.0);
        let segment = RoadSegment::new(1, 1, start, end, 8.0);

        let length = segment.get_length();
        assert!((length - 100.0).abs() < 0.001);
    }

    #[test]
    fn test_curve_sampling() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(100.0, 0.0, 0.0);
        let mut segment = RoadSegment::new(1, 1, start, end, 8.0);

        let midpoint = segment.sample_curve(0.5);
        assert!((midpoint.x - 50.0).abs() < 0.001);

        // Test with control point
        segment.add_control_point(Vec3::new(50.0, 10.0, 0.0));
        let curved_midpoint = segment.sample_curve(0.5);
        assert!(curved_midpoint.y > 0.0); // Should be curved upward
    }

    #[test]
    fn test_road_manager() {
        let mut manager = RoadManager::new();

        let road_id = manager.create_road(
            "Test Road".to_string(),
            RoadType::DirtPath { wear_factor: 0.5 },
        );
        assert_eq!(road_id, 1);

        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(50.0, 0.0, 0.0);
        let segment_result = manager.create_segment(road_id, start, end, Some(6.0));
        assert!(segment_result.is_ok());

        let segment_id = segment_result.unwrap();
        assert_eq!(segment_id, 1);

        // Check that road now contains the segment
        let road = manager.get_road(road_id).unwrap();
        assert_eq!(road.segments.len(), 1);
        assert_eq!(road.segments[0], segment_id);
    }

    #[test]
    fn test_geometry_generation() {
        let mut segment = RoadSegment::new(
            1,
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 0.0, 0.0),
            4.0,
        );

        let config = RoadGenerationConfig::default();
        let result = segment.generate_geometry(&config);

        assert!(result.is_ok());
        assert!(segment.geometry.is_some());
        assert!(!segment.dirty);

        let geometry = segment.geometry.unwrap();
        assert!(!geometry.vertices.is_empty());
        assert!(!geometry.indices.is_empty());
    }

    #[test]
    fn test_geometry_generation_produces_slope_aware_normals() {
        let mut segment = RoadSegment::new(
            1,
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(10.0, 4.0, 0.0),
            4.0,
        );

        segment
            .generate_geometry(&RoadGenerationConfig::default())
            .unwrap();

        let geometry = segment.geometry.as_ref().unwrap();
        let n = geometry.vertices[0].normal;
        // Sloped road should no longer be hardcoded to world up.
        assert!(n[1] > 0.0);
        assert!(n[1] < 0.9999);
    }

    #[test]
    fn test_geometry_generation_handles_vertical_tangent_without_nan() {
        let mut segment = RoadSegment::new(
            1,
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 10.0, 0.0),
            4.0,
        );

        segment
            .generate_geometry(&RoadGenerationConfig::default())
            .unwrap();

        let geometry = segment.geometry.as_ref().unwrap();
        for vertex in &geometry.vertices {
            assert!(vertex.normal.iter().all(|c| c.is_finite()));
        }
    }

    #[test]
    fn test_straight_geometry_uses_map_cell_tessellation_density() {
        let mut segment = RoadSegment::new(
            1,
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(45.0, 0.0, 0.0),
            4.0,
        );
        segment
            .generate_geometry(&RoadGenerationConfig::default())
            .unwrap();
        let geometry = segment.geometry.as_ref().unwrap();

        let expected_resolution =
            (((segment.get_length() / MAP_XY_FACTOR) as u32).saturating_add(1)).max(2);
        assert_eq!(geometry.vertices.len(), (expected_resolution * 2) as usize);
    }

    #[test]
    fn test_render_validation_passes_for_generated_geometry() {
        let mut manager = RoadManager::new();
        let road_id = manager.create_road(
            "Render Test".to_string(),
            RoadType::DirtPath { wear_factor: 0.25 },
        );
        manager
            .create_segment(
                road_id,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(25.0, 0.0, 0.0),
                Some(6.0),
            )
            .unwrap();
        manager.update_geometry().unwrap();

        assert!(manager.render(&Mat4::IDENTITY, &Mat4::IDENTITY).is_ok());
    }

    #[test]
    fn test_render_validation_rejects_out_of_bounds_indices() {
        let mut manager = RoadManager::new();
        let road_id = manager.create_road(
            "Broken".to_string(),
            RoadType::DirtPath { wear_factor: 0.1 },
        );
        let segment_id = manager
            .create_segment(
                road_id,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(5.0, 0.0, 0.0),
                Some(4.0),
            )
            .unwrap();

        let segment = manager.get_segment_mut(segment_id).unwrap();
        segment.geometry = Some(RoadGeometry {
            vertices: vec![RoadVertex {
                position: [0.0, 0.0, 0.0],
                normal: [0.0, 1.0, 0.0],
                tex_coords: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
                road_distance: 0.0,
            }],
            indices: vec![0, 1, 0],
            uvs: Vec::new(),
            colors: Vec::new(),
            row_height_samples: Vec::new(),
            edge_geometry: None,
            marking_geometry: None,
        });
        segment.dirty = false;

        let result = manager.render(&Mat4::IDENTITY, &Mat4::IDENTITY);
        assert!(matches!(result, Err(TerrainError::InvalidData(_))));
    }

    #[test]
    fn test_apply_terrain_normals_reprojects_surface_normals() {
        let mut manager = RoadManager::new();
        let road_id = manager.create_road(
            "Normal Project".to_string(),
            RoadType::DirtPath { wear_factor: 0.2 },
        );
        let segment_id = manager
            .create_segment(
                road_id,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(12.0, 2.0, 0.0),
                Some(4.0),
            )
            .unwrap();
        manager.update_geometry().unwrap();

        manager.apply_terrain_normals(|_| Vec3::new(0.0, 1.0, 0.0));
        let geometry = manager
            .get_segment(segment_id)
            .and_then(|s| s.geometry.as_ref())
            .unwrap();
        assert!(geometry.vertices.iter().all(|v| {
            (v.normal[0] - 0.0).abs() < 1.0e-4
                && (v.normal[1] - 1.0).abs() < 1.0e-4
                && (v.normal[2] - 0.0).abs() < 1.0e-4
        }));
    }

    #[test]
    fn test_apply_terrain_heights_and_normals_clamps_rows_to_max_height() {
        let mut manager = RoadManager::new();
        let road_id = manager.create_road(
            "Height Project".to_string(),
            RoadType::DirtPath { wear_factor: 0.2 },
        );
        let segment_id = manager
            .create_segment(
                road_id,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Some(4.0),
            )
            .unwrap();
        manager.update_geometry().unwrap();

        manager.apply_terrain_heights_and_normals(
            |pos| {
                if pos.z >= 0.0 {
                    2.0
                } else {
                    5.0
                }
            },
            |_| Vec3::new(0.0, 1.0, 0.0),
        );

        let geometry = manager
            .get_segment(segment_id)
            .and_then(|s| s.geometry.as_ref())
            .unwrap();
        let expected = 5.0 + RoadSegment::ROAD_FLOAT_HEIGHT_BIAS;
        for pair in geometry.vertices.chunks_exact(2) {
            assert!((pair[0].position[1] - expected).abs() < 1.0e-4);
            assert!((pair[1].position[1] - expected).abs() < 1.0e-4);
            assert!((pair[0].normal[1] - 1.0).abs() < 1.0e-4);
            assert!((pair[1].normal[1] - 1.0).abs() < 1.0e-4);
        }
    }

    #[test]
    fn test_apply_terrain_heights_samples_full_width_before_collapsing_rows() {
        let mut manager = RoadManager::new();
        let road_id = manager.create_road(
            "Center Ridge Project".to_string(),
            RoadType::DirtPath { wear_factor: 0.2 },
        );
        let segment_id = manager
            .create_segment(
                road_id,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(30.0, 0.0, 0.0),
                Some(40.0),
            )
            .unwrap();
        manager.update_geometry().unwrap();

        manager.apply_terrain_heights_and_normals(
            |pos| if pos.z.abs() < 1.0 { 10.0 } else { 0.0 },
            |_| Vec3::new(0.0, 1.0, 0.0),
        );

        let geometry = manager
            .get_segment(segment_id)
            .and_then(|s| s.geometry.as_ref())
            .unwrap();
        assert!(
            geometry.row_height_samples.iter().any(|row| row.len() > 2),
            "wide road rows should retain C++ lateral height samples"
        );
        let expected = 10.0 + RoadSegment::ROAD_FLOAT_HEIGHT_BIAS;
        for pair in geometry.vertices.chunks_exact(2) {
            assert!((pair[0].position[1] - expected).abs() < 1.0e-4);
            assert!((pair[1].position[1] - expected).abs() < 1.0e-4);
        }
    }

    #[test]
    fn test_apply_terrain_heights_and_normals_clamps_synthetic_intersections_too() {
        let mut manager = RoadManager::new();
        let road_id = manager.create_road(
            "Synthetic Height Project".to_string(),
            RoadType::DirtPath { wear_factor: 0.2 },
        );
        let segment_id = manager
            .create_segment(
                road_id,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Some(4.0),
            )
            .unwrap();
        {
            let segment = manager.get_segment_mut(segment_id).unwrap();
            segment.properties.synthetic_intersection = Some(RoadSyntheticIntersectionKind::Tee);
        }
        manager.update_geometry().unwrap();

        manager.apply_terrain_heights_and_normals(
            |pos| if pos.x < 10.0 { 2.0 } else { 7.0 },
            |_| Vec3::new(0.0, 1.0, 0.0),
        );

        let geometry = manager
            .get_segment(segment_id)
            .and_then(|s| s.geometry.as_ref())
            .unwrap();
        for (row, pair) in geometry.vertices.chunks_exact(2).enumerate() {
            let max_height = geometry
                .row_height_samples
                .get(row)
                .filter(|samples| !samples.is_empty())
                .map(|samples| {
                    samples
                        .iter()
                        .map(|pos| if pos.x < 10.0 { 2.0 } else { 7.0 })
                        .fold(f32::NEG_INFINITY, f32::max)
                })
                .unwrap_or_else(|| {
                    let h0: f32 = if pair[0].position[0] < 10.0 { 2.0 } else { 7.0 };
                    let h1: f32 = if pair[1].position[0] < 10.0 { 2.0 } else { 7.0 };
                    h0.max(h1)
                });
            let expected = max_height + RoadSegment::ROAD_FLOAT_HEIGHT_BIAS;
            assert!((pair[0].position[1] - expected).abs() < 1.0e-4);
            assert!((pair[1].position[1] - expected).abs() < 1.0e-4);
        }
    }

    #[test]
    fn test_apply_terrain_diffuse_updates_road_vertex_colors() {
        let mut manager = RoadManager::new();
        let road_id = manager.create_road(
            "Lit Road".to_string(),
            RoadType::DirtPath { wear_factor: 0.2 },
        );
        let segment_id = manager
            .create_segment(
                road_id,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Some(10.0),
            )
            .unwrap();
        {
            let segment = manager.get_segment_mut(segment_id).unwrap();
            segment.properties.texture_override =
                Some("Kind=SEGMENT WidthInTexture=1.0".to_string());
        }
        manager.update_geometry().unwrap();

        manager.apply_terrain_heights_normals_and_diffuse(
            |_| 3.0,
            |_| Vec3::new(0.0, 1.0, 0.0),
            |pos| {
                if pos.x < 10.0 {
                    [0.25, 0.5, 0.75, 1.0]
                } else {
                    [0.9, 0.8, 0.7, 1.0]
                }
            },
        );

        let geometry = manager
            .get_segment(segment_id)
            .and_then(|s| s.geometry.as_ref())
            .unwrap();
        assert_eq!(geometry.vertices[0].color, [0.25, 0.5, 0.75, 1.0]);
        assert_eq!(
            geometry.vertices.last().unwrap().color,
            [0.9, 0.8, 0.7, 1.0]
        );
        assert_eq!(geometry.colors.len(), geometry.vertices.len());
        assert_eq!(geometry.colors[0], geometry.vertices[0].color);
        assert_eq!(
            *geometry.colors.last().unwrap(),
            geometry.vertices.last().unwrap().color
        );
    }

    fn assert_vertex_position(vertex: &RoadVertex, expected: Vec3) {
        assert!((vertex.position[0] - expected.x).abs() < 1.0e-4);
        assert!((vertex.position[1] - expected.y).abs() < 1.0e-4);
        assert!((vertex.position[2] - expected.z).abs() < 1.0e-4);
    }

    fn assert_near(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 1.0e-4);
    }

    #[test]
    fn test_three_way_y_geometry_uses_w3d_join_constants() {
        let segment = RoadSegment::new(
            1,
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            10.0,
        );

        let geometry = segment
            .generate_synthetic_intersection_geometry(RoadSyntheticIntersectionKind::ThreeWayY)
            .unwrap();

        assert_vertex_position(&geometry.vertices[0], Vec3::new(-7.95, 0.0, -7.9));
        assert_vertex_position(&geometry.vertices[1], Vec3::new(-7.95, 0.0, 2.9));
        assert_near(geometry.vertices[0].tex_coords[0], 0.29929686);
        assert_near(geometry.vertices[0].tex_coords[1], 0.63890624);
    }

    #[test]
    fn test_kind_segment_uses_w3d_preload_segment_quad() {
        let mut segment = RoadSegment::new(
            1,
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            10.0,
        );
        segment.properties.texture_override = Some("Kind=SEGMENT WidthInTexture=1.0".to_string());

        segment
            .generate_geometry(&RoadGenerationConfig::default())
            .unwrap();
        let geometry = segment.geometry.as_ref().unwrap();
        let last = geometry.vertices.len() - 2;

        assert_vertex_position(&geometry.vertices[0], Vec3::new(0.0, 0.0, -5.0));
        assert_vertex_position(&geometry.vertices[1], Vec3::new(0.0, 0.0, 5.0));
        assert_vertex_position(&geometry.vertices[last], Vec3::new(20.0, 0.0, -5.0));
        assert_vertex_position(&geometry.vertices[last + 1], Vec3::new(20.0, 0.0, 5.0));
        assert_near(geometry.vertices[0].tex_coords[0], 0.0);
        assert_near(geometry.vertices[0].tex_coords[1], 0.29101563);
        assert_near(geometry.vertices[last].tex_coords[0], 0.5);
        assert_near(geometry.vertices[last].tex_coords[1], 0.29101563);
    }

    #[test]
    fn test_three_way_h_geometry_uses_w3d_join_constants_and_flip() {
        let mut segment = RoadSegment::new(
            1,
            1,
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(20.0, 0.0, 0.0),
            10.0,
        );
        segment.properties.texture_override = Some("WidthInTexture=1.0".to_string());

        let geometry = segment
            .generate_synthetic_intersection_geometry(RoadSyntheticIntersectionKind::ThreeWayH)
            .unwrap();
        assert_vertex_position(&geometry.vertices[0], Vec3::new(-5.0, 0.0, -10.8));
        assert_vertex_position(&geometry.vertices[1], Vec3::new(-5.0, 0.0, 2.7));
        assert_vertex_position(&geometry.vertices[2], Vec3::new(12.0, 0.0, -10.8));
        assert_near(geometry.vertices[0].tex_coords[0], 0.26953125);
        assert_near(geometry.vertices[0].tex_coords[1], 0.9809375);

        let flipped = segment
            .generate_synthetic_intersection_geometry(RoadSyntheticIntersectionKind::ThreeWayHFlip)
            .unwrap();
        assert_vertex_position(&flipped.vertices[0], Vec3::new(-5.0, 0.0, -2.7));
        assert_vertex_position(&flipped.vertices[1], Vec3::new(-5.0, 0.0, 10.8));
        assert_vertex_position(&flipped.vertices[2], Vec3::new(12.0, 0.0, -2.7));
    }

    #[test]
    fn test_snapshot_minimap_samples_emits_segment_points() {
        let mut manager = RoadManager::new();
        let road_id = manager.create_road(
            "Overlay".to_string(),
            RoadType::DirtPath { wear_factor: 0.3 },
        );
        manager
            .create_segment(
                road_id,
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(20.0, 0.0, 0.0),
                Some(6.0),
            )
            .unwrap();

        let samples = manager.snapshot_minimap_samples(8);
        assert!(samples.len() >= 9);
        assert!(samples.iter().all(|s| s.width >= 0.1));
        assert!(samples.iter().all(|s| s.tint_rgb == [138, 112, 77]));
    }
}
