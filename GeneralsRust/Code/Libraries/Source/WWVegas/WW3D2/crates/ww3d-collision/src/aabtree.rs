use crate::collision_tests::{AABoxCollisionTest, CollisionTest};
use crate::intersection::{RayCollisionTest, Triangle};
use glam::Vec3;

/// Axis-Aligned Bounding Box Tree for spatial partitioning (ported from aabtree.cpp)
#[derive(Debug)]
pub struct AABTree {
    pub nodes: Vec<CullNode>,
    pub poly_indices: Vec<u32>,
    pub node_count: usize,
    pub poly_count: usize,
}

/// Cull node structure that matches the C++ implementation
#[derive(Debug, Clone)]
pub struct CullNode {
    pub min: Vec3,
    pub max: Vec3,
    pub front_or_poly0: u32,
    pub back_or_poly_count: u32,
}

const LEAF_FLAG: u32 = 0x8000_0000;

impl CullNode {
    pub fn new() -> Self {
        Self {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
            front_or_poly0: 0,
            back_or_poly_count: 0,
        }
    }

    pub fn is_leaf(&self) -> bool {
        (self.front_or_poly0 & LEAF_FLAG) != 0
    }

    pub fn get_front_child(&self) -> usize {
        (self.front_or_poly0 & !LEAF_FLAG) as usize
    }

    pub fn get_back_child(&self) -> usize {
        self.back_or_poly_count as usize
    }

    pub fn get_poly0(&self) -> usize {
        (self.front_or_poly0 & !LEAF_FLAG) as usize
    }

    pub fn get_poly_count(&self) -> usize {
        self.back_or_poly_count as usize
    }

    pub fn set_front_child(&mut self, index: usize) {
        self.front_or_poly0 = (index as u32) & !LEAF_FLAG;
    }

    pub fn set_back_child(&mut self, index: usize) {
        self.back_or_poly_count = index as u32;
    }

    pub fn set_poly0(&mut self, index: usize) {
        self.front_or_poly0 = ((index as u32) & !LEAF_FLAG) | LEAF_FLAG; // Mark as leaf
    }

    pub fn set_poly_count(&mut self, count: usize) {
        self.back_or_poly_count = count as u32;
    }
}

/// Mesh geometry interface for AABTree operations
pub trait MeshGeometry {
    fn get_vertex_array(&self) -> &[Vec3];
    fn get_polygon_array(&self) -> &[[u32; 3]]; // Triangle indices
    fn get_polygon_count(&self) -> usize;
    fn get_poly_surface_type(&self, poly_index: usize) -> u32;
}

/// Active Polygon Table context for OBBox tests
pub struct OBBoxAPTContext {
    pub apt: Vec<u32>,
}

impl OBBoxAPTContext {
    pub fn new() -> Self {
        Self { apt: Vec::new() }
    }

    pub fn add(&mut self, poly_index: u32) {
        self.apt.push(poly_index);
    }
}

impl AABTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            poly_indices: Vec::new(),
            node_count: 0,
            poly_count: 0,
        }
    }

    /// Cast ray through the tree (ported from aabtree.cpp)
    pub fn cast_ray(&self, ray_test: &mut RayCollisionTest, mesh: &dyn MeshGeometry) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.cast_ray_recursive(&self.nodes[0], ray_test, mesh)
    }

    fn cast_ray_recursive(
        &self,
        node: &CullNode,
        ray_test: &mut RayCollisionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        // Cull against node bounds
        if ray_test.cull(node.min, node.max) {
            return false;
        }

        if node.is_leaf() {
            return self.cast_ray_to_polys(node, ray_test, mesh);
        } else {
            let mut result = false;
            if let Some(child) = self.nodes.get(node.get_front_child()) {
                result |= self.cast_ray_recursive(child, ray_test, mesh);
            }
            if let Some(child) = self.nodes.get(node.get_back_child()) {
                result |= self.cast_ray_recursive(child, ray_test, mesh);
            }
            result
        }
    }

    fn cast_ray_to_polys(
        &self,
        node: &CullNode,
        ray_test: &mut RayCollisionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        if node.get_poly_count() == 0 {
            return false;
        }

        let vertices = mesh.get_vertex_array();
        let polygons = mesh.get_polygon_array();
        let poly0 = node.get_poly0();
        let poly_count = node.get_poly_count();

        let mut poly_hit = None;

        for poly_counter in 0..poly_count {
            if let Some(&poly_index_u32) = self.poly_indices.get(poly0 + poly_counter) {
                let poly_index = poly_index_u32 as usize;

                if poly_index >= polygons.len() {
                    continue;
                }

                let triangle_indices = &polygons[poly_index];

                // Build triangle
                let v0 = vertices[triangle_indices[0] as usize];
                let v1 = vertices[triangle_indices[1] as usize];
                let v2 = vertices[triangle_indices[2] as usize];

                let triangle = Triangle {
                    vertices: [v0, v1, v2],
                    normal: (v1 - v0).cross(v2 - v0).normalize_or_zero(),
                };

                // Test ray against triangle
                if self.ray_triangle_intersect(ray_test, &triangle) {
                    poly_hit = Some(poly_index);
                    ray_test.result.surface_type = mesh.get_poly_surface_type(poly_index);
                }

                if ray_test.result.start_bad {
                    return true;
                }
            }
        }

        poly_hit.is_some()
    }

    fn ray_triangle_intersect(&self, ray_test: &mut RayCollisionTest, triangle: &Triangle) -> bool {
        let edge1 = triangle.vertices[1] - triangle.vertices[0];
        let edge2 = triangle.vertices[2] - triangle.vertices[0];

        let h = ray_test.ray_direction.cross(edge2);
        let a = edge1.dot(h);

        if a.abs() < 0.0001 {
            return false; // Ray is parallel to triangle
        }

        let f = 1.0 / a;
        let s = ray_test.ray_origin - triangle.vertices[0];
        let u = f * s.dot(h);

        if u < 0.0 || u > 1.0 {
            return false;
        }

        let q = s.cross(edge1);
        let v = f * ray_test.ray_direction.dot(q);

        if v < 0.0 || u + v > 1.0 {
            return false;
        }

        let t = f * edge2.dot(q);

        if t > 0.0001 && t < ray_test.result.fraction {
            ray_test.result.fraction = t;
            ray_test.result.normal = triangle.normal;
            return true;
        }

        false
    }

    /// Cast AABox through the tree
    pub fn cast_aabox(&self, box_test: &mut AABoxCollisionTest, mesh: &dyn MeshGeometry) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.cast_aabox_recursive(&self.nodes[0], box_test, mesh)
    }

    fn cast_aabox_recursive(
        &self,
        node: &CullNode,
        box_test: &mut AABoxCollisionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        // Cull against node bounds
        if box_test.cull(node.min, node.max) {
            return false;
        }

        if node.is_leaf() {
            return self.cast_aabox_to_polys(node, box_test, mesh);
        } else {
            let mut result = false;
            if let Some(child) = self.nodes.get(node.get_front_child()) {
                result |= self.cast_aabox_recursive(child, box_test, mesh);
            }
            if let Some(child) = self.nodes.get(node.get_back_child()) {
                result |= self.cast_aabox_recursive(child, box_test, mesh);
            }
            result
        }
    }

    fn cast_aabox_to_polys(
        &self,
        node: &CullNode,
        box_test: &mut AABoxCollisionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        if node.get_poly_count() == 0 {
            return false;
        }

        let vertices = mesh.get_vertex_array();
        let polygons = mesh.get_polygon_array();
        let poly0 = node.get_poly0();
        let poly_count = node.get_poly_count();

        let mut poly_hit = None;

        for poly_counter in 0..poly_count {
            if let Some(&poly_index_u32) = self.poly_indices.get(poly0 + poly_counter) {
                let poly_index = poly_index_u32 as usize;

                if poly_index >= polygons.len() {
                    continue;
                }

                let triangle_indices = &polygons[poly_index];

                // Build triangle
                let v0 = vertices[triangle_indices[0] as usize];
                let v1 = vertices[triangle_indices[1] as usize];
                let v2 = vertices[triangle_indices[2] as usize];

                let triangle = Triangle {
                    vertices: [v0, v1, v2],
                    normal: (v1 - v0).cross(v2 - v0).normalize_or_zero(),
                };

                // Test AABox against triangle
                if self.aabox_triangle_collide(box_test, &triangle) {
                    poly_hit = Some(poly_index);
                    box_test.result.surface_type = mesh.get_poly_surface_type(poly_index);
                }

                if box_test.result.start_bad {
                    return true;
                }
            }
        }

        poly_hit.is_some()
    }

    fn aabox_triangle_collide(
        &self,
        box_test: &mut AABoxCollisionTest,
        triangle: &Triangle,
    ) -> bool {
        // Simplified AABox-Triangle collision test
        // This would need a full SAT implementation for accuracy

        // Quick AABB vs triangle AABB test
        let tri_min = triangle
            .vertices
            .iter()
            .fold(triangle.vertices[0], |acc, &v| acc.min(v));
        let tri_max = triangle
            .vertices
            .iter()
            .fold(triangle.vertices[0], |acc, &v| acc.max(v));

        let box_min = box_test.aabb.center - box_test.aabb.extent;
        let box_max = box_test.aabb.center + box_test.aabb.extent;

        let end_min = box_test.aabb.center + box_test.movement - box_test.aabb.extent;
        let end_max = box_test.aabb.center + box_test.movement + box_test.aabb.extent;

        let sweep_min = box_min.min(end_min);
        let sweep_max = box_max.max(end_max);

        // AABB overlap test
        if sweep_min.x <= tri_max.x
            && sweep_max.x >= tri_min.x
            && sweep_min.y <= tri_max.y
            && sweep_max.y >= tri_min.y
            && sweep_min.z <= tri_max.z
            && sweep_max.z >= tri_min.z
        {
            return true;
        }

        false
    }

    /// Update bounding boxes after mesh modification
    pub fn update_bounding_boxes(&mut self, mesh: &dyn MeshGeometry) {
        if !self.nodes.is_empty() {
            self.update_bounding_boxes_recursive(0, mesh);
        }
    }

    fn update_bounding_boxes_recursive(&mut self, node_index: usize, mesh: &dyn MeshGeometry) {
        let mut min = Vec3::splat(100000.0);
        let mut max = Vec3::splat(-100000.0);

        let node = &self.nodes[node_index];

        if !node.is_leaf() {
            // Recurse to children first
            let front_child = node.get_front_child();
            let back_child = node.get_back_child();

            let mut children = [usize::MAX; 2];
            let mut child_count = 0;

            if front_child < self.nodes.len() {
                self.update_bounding_boxes_recursive(front_child, mesh);
                children[child_count] = front_child;
                child_count += 1;
            }
            if back_child < self.nodes.len() {
                self.update_bounding_boxes_recursive(back_child, mesh);
                children[child_count] = back_child;
                child_count += 1;
            }

            for &child_idx in &children[..child_count] {
                min = min.min(self.nodes[child_idx].min);
                max = max.max(self.nodes[child_idx].max);
            }
        } else {
            // Bound polygons
            let poly0 = node.get_poly0();
            let poly_count = node.get_poly_count();
            let vertices = mesh.get_vertex_array();
            let polygons = mesh.get_polygon_array();

            for poly_counter in 0..poly_count {
                if let Some(&poly_index_u32) = self.poly_indices.get(poly0 + poly_counter) {
                    let poly_index = poly_index_u32 as usize;
                    if poly_index >= polygons.len() {
                        continue;
                    }
                    let triangle_indices = &polygons[poly_index];
                    for &vert_index in triangle_indices {
                        if (vert_index as usize) < vertices.len() {
                            let vertex = vertices[vert_index as usize];
                            min = min.min(vertex);
                            max = max.max(vertex);
                        }
                    }
                }
            }
        }

        self.nodes[node_index].min = min;
        self.nodes[node_index].max = max;
    }

    /// Scale the entire tree uniformly
    pub fn scale(&mut self, factor: f32) {
        for node in &mut self.nodes {
            node.min *= factor;
            node.max *= factor;
        }
    }
}
