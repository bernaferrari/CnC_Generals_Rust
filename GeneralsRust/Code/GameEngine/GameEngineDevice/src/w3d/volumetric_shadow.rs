//! CPU silhouette helpers for C++ `W3DDevice/GameClient/Shadow/W3DVolumetricShadow.cpp`.

use glam::Vec3;

pub const MAX_SHADOW_CASTER_MESHES: usize = 160;
pub const MAX_SILHOUETTE_EDGES: usize = 1024;
pub const SHADOW_EXTRUSION_BUFFER: f32 = 0.1;
pub const AIRBORNE_UNIT_GROUND_DELTA: f32 = 2.0;
pub const MAX_SHADOW_LENGTH_SCALE_FACTOR: f32 = 1.0;
pub const MAX_SHADOW_LENGTH_EXTRA_AIRBORNE_SCALE_FACTOR: f32 = 1.5;
pub const MAX_EXTRUSION_LENGTH: f32 = 512.0 * 10.0;
pub const MAX_SHADOW_EXTRUSION_UNDER_OBJECT_BEFORE_CLAMP: f32 = 5.0;
pub const SHADOW_SAMPLING_INTERVAL: f32 = 10.0 * 2.0;
pub const OVERHANGING_OBJECT_CLAMP_ANGLE: f32 = 80.0 / 180.0 * std::f32::consts::PI;
pub const MAX_POLYGON_NEIGHBORS: usize = 3;
pub const NO_NEIGHBOR: i16 = -1;
const POLY_VISIBLE: u8 = 0x01;
const POLY_PROCESSED: u8 = 0x02;

/// C++ `NeighborEdge`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NeighborEdge {
    pub neighbor_index: i16,
    pub neighbor_edge_index: [u16; 2],
}

impl Default for NeighborEdge {
    fn default() -> Self {
        Self {
            neighbor_index: NO_NEIGHBOR,
            neighbor_edge_index: [0, 0],
        }
    }
}

/// C++ `PolyNeighbor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PolyNeighbor {
    pub my_index: u16,
    pub status: u8,
    pub neighbor: [NeighborEdge; MAX_POLYGON_NEIGHBORS],
}

impl PolyNeighbor {
    fn new(my_index: u16) -> Self {
        Self {
            my_index,
            status: 0,
            neighbor: [NeighborEdge::default(); MAX_POLYGON_NEIGHBORS],
        }
    }

    pub fn is_visible(&self) -> bool {
        self.status & POLY_VISIBLE != 0
    }
}

/// C++ `Geometry` subset used by silhouette/volume construction.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowGeometryMesh {
    pub vertices: Vec<Vec3>,
    pub polygons: Vec<[u16; 3]>,
    pub polygon_normals: Vec<Vec3>,
    pub poly_neighbors: Vec<PolyNeighbor>,
}

impl ShadowGeometryMesh {
    pub fn new(vertices: Vec<Vec3>, polygons: Vec<[u16; 3]>) -> Self {
        let mut mesh = Self {
            vertices,
            polygons,
            polygon_normals: Vec::new(),
            poly_neighbors: Vec::new(),
        };
        mesh.build_polygon_normals();
        mesh.build_polygon_neighbors();
        mesh
    }

    pub fn build_polygon_normals(&mut self) {
        self.polygon_normals = self
            .polygons
            .iter()
            .map(|poly| {
                let a = self.vertices[poly[0] as usize];
                let b = self.vertices[poly[1] as usize];
                let c = self.vertices[poly[2] as usize];
                (b - a).cross(c - a).normalize_or_zero()
            })
            .collect();
    }

    /// C++ `W3DShadowGeometryMesh::buildPolygonNeighbors`.
    pub fn build_polygon_neighbors(&mut self) {
        self.build_polygon_normals();
        let num_polys = self.polygons.len();
        self.poly_neighbors = (0..num_polys)
            .map(|i| PolyNeighbor::new(i as u16))
            .collect();

        for i in 0..num_polys {
            let poly = self.polygons[i];
            let normal = self.polygon_normals[i];
            for j in 0..num_polys {
                if i == j {
                    continue;
                }
                let other = self.polygons[j];
                let mut index1 = None;
                let mut index2 = None;
                let mut index1_pos = (0usize, 0usize);

                for a in 0..3 {
                    for b in 0..3 {
                        if poly[a] != other[b] {
                            continue;
                        }
                        if index1.is_none() {
                            index1 = Some(poly[a]);
                            index1_pos = (a, b);
                        } else if index2.is_none() {
                            if !opposite_edge_winding(
                                a as i32 - index1_pos.0 as i32,
                                b as i32 - index1_pos.1 as i32,
                            ) {
                                continue;
                            }
                            if (self.polygon_normals[j].dot(normal) + 1.0).abs() <= 0.01 {
                                continue;
                            }
                            index2 = Some(poly[a]);
                        } else {
                            index1 = None;
                            index2 = None;
                            continue;
                        }
                    }
                }

                let (Some(edge_start), Some(edge_end)) = (index1, index2) else {
                    continue;
                };
                if let Some(slot) = self.poly_neighbors[i]
                    .neighbor
                    .iter_mut()
                    .find(|edge| edge.neighbor_index == NO_NEIGHBOR)
                {
                    slot.neighbor_index = j as i16;
                    slot.neighbor_edge_index = [edge_start, edge_end];
                }
            }
        }
    }

    /// C++ `W3DVolumetricShadow::buildSilhouette` for one mesh.
    pub fn build_silhouette(&mut self, light_pos_object: Vec3) -> Vec<[u16; 2]> {
        for poly in &mut self.poly_neighbors {
            poly.status = 0;
        }

        for i in 0..self.polygons.len() {
            let poly = self.polygons[i];
            let normal = self.polygon_normals[i];
            let vertex = self.vertices[poly[0] as usize];
            let light_vector = vertex - light_pos_object;
            if light_vector.dot(normal) < 0.0 {
                self.poly_neighbors[i].status |= POLY_VISIBLE;
            }
        }

        let mut silhouette = Vec::new();
        for i in 0..self.polygons.len() {
            let mut visible_neighborless = false;
            let us = self.poly_neighbors[i].clone();
            for j in 0..MAX_POLYGON_NEIGHBORS {
                let edge = us.neighbor[j];
                let other = if edge.neighbor_index != NO_NEIGHBOR {
                    let other = self.poly_neighbors[edge.neighbor_index as usize].clone();
                    if other.status & POLY_PROCESSED != 0 {
                        continue;
                    }
                    Some(other)
                } else {
                    None
                };

                if us.status & POLY_VISIBLE != 0 {
                    if let Some(other) = other {
                        if other.status & POLY_VISIBLE == 0 {
                            silhouette.push(self.silhouette_edge(&us, &other));
                        }
                    } else {
                        visible_neighborless = true;
                    }
                } else if let Some(other) = other {
                    if other.status & POLY_VISIBLE != 0 {
                        silhouette.push(self.silhouette_edge(&other, &us));
                    }
                }
            }

            if visible_neighborless {
                silhouette.extend(self.neighborless_edges(&us));
            }
            self.poly_neighbors[i].status |= POLY_PROCESSED;
        }

        silhouette.truncate(MAX_SILHOUETTE_EDGES);
        silhouette
    }

    fn silhouette_edge(&self, visible: &PolyNeighbor, hidden: &PolyNeighbor) -> [u16; 2] {
        let neighbor_edge = visible
            .neighbor
            .iter()
            .find(|edge| edge.neighbor_index == hidden.my_index as i16)
            .copied()
            .unwrap_or_default();
        let visible_indices = self.polygons[visible.my_index as usize];

        if visible_indices[0] != neighbor_edge.neighbor_edge_index[0]
            && visible_indices[0] != neighbor_edge.neighbor_edge_index[1]
        {
            [visible_indices[1], visible_indices[2]]
        } else if visible_indices[1] != neighbor_edge.neighbor_edge_index[0]
            && visible_indices[1] != neighbor_edge.neighbor_edge_index[1]
        {
            [visible_indices[2], visible_indices[0]]
        } else {
            [visible_indices[0], visible_indices[1]]
        }
    }

    fn neighborless_edges(&self, us: &PolyNeighbor) -> Vec<[u16; 2]> {
        let indices = self.polygons[us.my_index as usize];
        let mut result = Vec::new();
        for i in 0..3 {
            let edge_start = indices[i];
            let edge_end = indices[(i + 1) % 3];
            let has_neighbor_edge = us.neighbor.iter().any(|edge| {
                edge.neighbor_index != NO_NEIGHBOR
                    && ((edge.neighbor_edge_index[0] == edge_start
                        && edge.neighbor_edge_index[1] == edge_end)
                        || (edge.neighbor_edge_index[1] == edge_start
                            && edge.neighbor_edge_index[0] == edge_end))
            });
            if !has_neighbor_edge {
                result.push([edge_start, edge_end]);
            }
        }
        result
    }
}

/// CPU shadow volume result from C++ silhouette extrusion.
#[derive(Debug, Clone, PartialEq)]
pub struct ShadowVolumeCpu {
    pub vertices: Vec<Vec3>,
    pub indices: Vec<u16>,
}

pub fn construct_shadow_volume(
    mesh: &ShadowGeometryMesh,
    silhouette: &[[u16; 2]],
    light_pos_object: Vec3,
    shadow_extrude_distance: f32,
) -> ShadowVolumeCpu {
    let mut vertices = Vec::with_capacity(silhouette.len() * 4);
    let mut indices = Vec::with_capacity(silhouette.len() * 6);

    for edge in silhouette {
        let p0 = mesh.vertices[edge[0] as usize];
        let p1 = mesh.vertices[edge[1] as usize];
        let e0 = p0 + (p0 - light_pos_object).normalize_or_zero() * shadow_extrude_distance;
        let e1 = p1 + (p1 - light_pos_object).normalize_or_zero() * shadow_extrude_distance;
        let base = vertices.len() as u16;
        vertices.extend_from_slice(&[p0, p1, e1, e0]);
        indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    ShadowVolumeCpu { vertices, indices }
}

fn opposite_edge_winding(diff1: i32, diff2: i32) -> bool {
    cpp_winding_code(diff1) != cpp_winding_code(diff2)
}

fn cpp_winding_code(diff: i32) -> i32 {
    let sign = if diff < 0 { i32::MIN } else { 0 };
    sign ^ (((diff.abs() & 2) << 30) as i32)
}
