//! Terrain Road System
//!
//! Manages road networks, pathways, and other linear infrastructure
//! elements that modify terrain appearance and affect gameplay.

use crate::terrain::{TerrainError, TerrainResult};
use glam::{Mat4, Vec3};
use nalgebra::Point2;
use std::collections::HashMap;

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
        let resolution = if self.control_points.is_empty() {
            2 // Just start and end for straight segments
        } else {
            config.curve_resolution
        };

        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut uvs = Vec::new();
        let mut colors = Vec::new();

        let half_width = self.width / 2.0;
        let segment_length = self.get_length();

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
        }

        // Generate indices for triangle strips
        for i in 0..(resolution - 1) {
            let base = (i * 2) as u32;

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
        let resolution = if self.control_points.is_empty() {
            2
        } else {
            16
        };

        let edge_width = (self.width * 0.2).max(1.0);
        let segment_length = self.get_length();

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
            let base = (i * 4) as u32;

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
        let resolution = if self.control_points.is_empty() {
            2
        } else {
            16
        };

        let mark_width = (self.width * 0.05).max(0.1);
        let segment_length = self.get_length();

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
            let base = (i * 2) as u32;
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
        Ok(())
    }

    /// Update road system geometry and stats.
    pub fn update(&mut self) -> TerrainResult<()> {
        let start_time = std::time::Instant::now();
        let mut total_vertices = 0u32;
        let mut total_triangles = 0u32;
        let mut total_memory = 0u64;

        for segment in self.segments.values_mut() {
            if segment.dirty {
                segment.generate_geometry(&self.generation_config)?;
                segment.dirty = false;
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
        Ok(())
    }

    /// Reproject generated road normals against terrain normals sampled in world space.
    ///
    /// This closes the tangent-only fallback and lets roads follow live terrain lighting.
    pub fn apply_terrain_normals<F>(&mut self, mut sample_normal: F)
    where
        F: FnMut(Vec3) -> Vec3,
    {
        for segment in self.segments.values_mut() {
            let Some(geometry) = segment.geometry.as_mut() else {
                continue;
            };

            for vertex in &mut geometry.vertices {
                let pos = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                let sampled = sample_normal(pos);
                vertex.normal = Self::sanitize_sampled_normal(sampled, vertex.normal);
            }

            if let Some(edge) = geometry.edge_geometry.as_mut() {
                for vertex in &mut edge.vertices {
                    let pos = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                    let sampled = sample_normal(pos);
                    vertex.normal = Self::sanitize_sampled_normal(sampled, vertex.normal);
                }
            }

            if let Some(marking) = geometry.marking_geometry.as_mut() {
                for vertex in &mut marking.vertices {
                    let pos = Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                    let sampled = sample_normal(pos);
                    vertex.normal = Self::sanitize_sampled_normal(sampled, vertex.normal);
                }
            }
        }
    }

    /// Validate road geometry for render submission.
    ///
    /// GPU submission is performed by the higher-level terrain renderer. This pass mirrors the
    /// C++ split where road systems prepare/validate geometry before draw calls.
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

        for segment_id in dirty_segments {
            if let Some(segment) = self.segments.get_mut(&segment_id) {
                segment.generate_geometry(&self.generation_config)?;
            }
        }

        self.update_statistics();
        self.stats.generation_time = start_time.elapsed();

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
    }

    fn validate_geometry(
        segment_id: RoadSegmentId,
        label: &str,
        vertices: &[RoadVertex],
        indices: &[u32],
    ) -> TerrainResult<()> {
        if indices.len() % 3 != 0 {
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
