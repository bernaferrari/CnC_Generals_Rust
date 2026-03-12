/// AABTree Extended Features - OBBox operations and APT generation
///
/// Implements the missing features from C++ aabtree.cpp:
/// - OBBox tree traversal (Cast_OBBox_Recursive, Intersect_OBBox_Recursive)
/// - Active Polygon Table (APT) generation
/// - Semi-infinite ray casting
use crate::aabtree::{AABTree, CullNode, MeshGeometry};
use crate::bounding_volumes::OBBoxClass;
use crate::collision_math::CollisionMath;
use crate::intersection::{CastResult, Triangle};
use glam::Vec3;

/// OBBox collision test (matching C++ OBBoxCollisionTestClass)
#[derive(Debug, Clone)]
pub struct OBBoxCollisionTest {
    pub obbox: OBBoxClass,
    pub movement: Vec3,
    pub sweep_min: Vec3,
    pub sweep_max: Vec3,
    pub result: CastResult,
    pub collision_type: u32,
}

impl OBBoxCollisionTest {
    pub fn new(obbox: OBBoxClass, movement: Vec3, collision_type: u32) -> Self {
        let mut test = Self {
            obbox,
            movement,
            sweep_min: Vec3::ZERO,
            sweep_max: Vec3::ZERO,
            result: CastResult::default(),
            collision_type,
        };
        test.calculate_sweep_bounds();
        test
    }

    fn calculate_sweep_bounds(&mut self) {
        // Calculate world-space AABB for OBBox
        let corners = self.obbox.get_corners();

        let mut min = corners[0];
        let mut max = corners[0];

        for &corner in &corners {
            min = min.min(corner);
            max = max.max(corner);
        }

        // Expand for movement
        let end_min = min + self.movement;
        let end_max = max + self.movement;

        self.sweep_min = min.min(end_min);
        self.sweep_max = max.max(end_max);
    }

    pub fn cull(&self, node_min: Vec3, node_max: Vec3) -> bool {
        self.sweep_min.x > node_max.x
            || self.sweep_max.x < node_min.x
            || self.sweep_min.y > node_max.y
            || self.sweep_max.y < node_min.y
            || self.sweep_min.z > node_max.z
            || self.sweep_max.z < node_min.z
    }
}

/// OBBox intersection test (matching C++ OBBoxIntersectionTestClass)
#[derive(Debug, Clone)]
pub struct OBBoxIntersectionTest {
    pub obbox: OBBoxClass,
    pub sweep_min: Vec3,
    pub sweep_max: Vec3,
    pub collision_type: u32,
}

impl OBBoxIntersectionTest {
    pub fn new(obbox: OBBoxClass, collision_type: u32) -> Self {
        let corners = obbox.get_corners();

        let mut min = corners[0];
        let mut max = corners[0];

        for &corner in &corners {
            min = min.min(corner);
            max = max.max(corner);
        }

        Self {
            obbox,
            sweep_min: min,
            sweep_max: max,
            collision_type,
        }
    }

    pub fn cull(&self, node_min: Vec3, node_max: Vec3) -> bool {
        self.sweep_min.x > node_max.x
            || self.sweep_max.x < node_min.x
            || self.sweep_min.y > node_max.y
            || self.sweep_max.y < node_min.y
            || self.sweep_min.z > node_max.z
            || self.sweep_max.z < node_min.z
    }
}

/// Extended AABTree operations
impl AABTree {
    /// Cast OBBox through tree (ported from Cast_OBBox_Recursive)
    pub fn cast_obbox(&self, test: &mut OBBoxCollisionTest, mesh: &dyn MeshGeometry) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.cast_obbox_recursive(&self.nodes[0], test, mesh)
    }

    fn cast_obbox_recursive(
        &self,
        node: &CullNode,
        test: &mut OBBoxCollisionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        // Cull against node bounds
        if test.cull(node.min, node.max) {
            return false;
        }

        if node.is_leaf() {
            return self.cast_obbox_to_polys(node, test, mesh);
        } else {
            let mut result = false;
            if let Some(child) = self.nodes.get(node.get_front_child()) {
                result |= self.cast_obbox_recursive(child, test, mesh);
            }
            if let Some(child) = self.nodes.get(node.get_back_child()) {
                result |= self.cast_obbox_recursive(child, test, mesh);
            }
            result
        }
    }

    fn cast_obbox_to_polys(
        &self,
        node: &CullNode,
        test: &mut OBBoxCollisionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        let poly_count = node.get_poly_count();
        if poly_count == 0 {
            return false;
        }

        let vertices = mesh.get_vertex_array();
        let polygons = mesh.get_polygon_array();
        let poly0 = node.get_poly0();

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

                // Test OBBox against triangle
                if CollisionMath::obbox_triangle_swept(
                    &test.obbox,
                    test.movement,
                    &triangle,
                    &mut test.result,
                ) {
                    poly_hit = Some(poly_index);
                    test.result.surface_type = mesh.get_poly_surface_type(poly_index);
                }

                if test.result.start_bad {
                    return true;
                }
            }
        }

        poly_hit.is_some()
    }

    /// Intersect OBBox with tree (ported from Intersect_OBBox_Recursive)
    pub fn intersect_obbox(
        &self,
        test: &mut OBBoxIntersectionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.intersect_obbox_recursive(&self.nodes[0], test, mesh)
    }

    fn intersect_obbox_recursive(
        &self,
        node: &CullNode,
        test: &mut OBBoxIntersectionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        // Cull against node bounds
        if test.cull(node.min, node.max) {
            return false;
        }

        if node.is_leaf() {
            return self.intersect_obbox_with_polys(node, test, mesh);
        } else {
            let mut result = false;
            if let Some(child) = self.nodes.get(node.get_front_child()) {
                result |= self.intersect_obbox_recursive(child, test, mesh);
            }
            if let Some(child) = self.nodes.get(node.get_back_child()) {
                result |= self.intersect_obbox_recursive(child, test, mesh);
            }
            result
        }
    }

    fn intersect_obbox_with_polys(
        &self,
        node: &CullNode,
        test: &mut OBBoxIntersectionTest,
        mesh: &dyn MeshGeometry,
    ) -> bool {
        let poly0 = node.get_poly0();
        let poly_count = node.get_poly_count();

        if poly_count == 0 {
            return false;
        }

        let vertices = mesh.get_vertex_array();
        let polygons = mesh.get_polygon_array();

        for poly_counter in 0..poly_count {
            if let Some(&poly_index_u32) = self.poly_indices.get(poly0 + poly_counter) {
                let poly_index = poly_index_u32 as usize;

                if poly_index >= polygons.len() {
                    continue;
                }

                let triangle_indices = &polygons[poly_index];

                let v0 = vertices[triangle_indices[0] as usize];
                let v1 = vertices[triangle_indices[1] as usize];
                let v2 = vertices[triangle_indices[2] as usize];

                let triangle = Triangle {
                    vertices: [v0, v1, v2],
                    normal: (v1 - v0).cross(v2 - v0).normalize_or_zero(),
                };

                if CollisionMath::obbox_triangle_intersection(&test.obbox, &triangle) {
                    return true;
                }
            }
        }

        false
    }

    /// Generate Active Polygon Table for OBBox (ported from Generate_APT)
    pub fn generate_apt(&self, obbox: &OBBoxClass, mesh: &dyn MeshGeometry) -> Vec<u32> {
        let mut apt = Vec::new();
        if !self.nodes.is_empty() {
            self.generate_apt_recursive(&self.nodes[0], obbox, mesh, &mut apt, None);
        }
        apt
    }

    /// Generate APT with backface culling
    pub fn generate_apt_with_view(
        &self,
        obbox: &OBBoxClass,
        view_dir: Vec3,
        mesh: &dyn MeshGeometry,
    ) -> Vec<u32> {
        let mut apt = Vec::new();
        if !self.nodes.is_empty() {
            self.generate_apt_recursive(&self.nodes[0], obbox, mesh, &mut apt, Some(view_dir));
        }
        apt
    }

    fn generate_apt_recursive(
        &self,
        node: &CullNode,
        obbox: &OBBoxClass,
        mesh: &dyn MeshGeometry,
        apt: &mut Vec<u32>,
        view_dir: Option<Vec3>,
    ) {
        // Quick cull test against node bounds
        let test = OBBoxIntersectionTest::new(obbox.clone(), 0);
        if test.cull(node.min, node.max) {
            return;
        }

        if node.is_leaf() {
            // Test polygons in this leaf
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

                    let v0 = vertices[triangle_indices[0] as usize];
                    let v1 = vertices[triangle_indices[1] as usize];
                    let v2 = vertices[triangle_indices[2] as usize];

                    let triangle = Triangle {
                        vertices: [v0, v1, v2],
                        normal: (v1 - v0).cross(v2 - v0).normalize_or_zero(),
                    };

                    // Backface culling if view direction provided
                    if let Some(view) = view_dir {
                        if triangle.normal.dot(view) >= 0.0 {
                            continue; // Backface
                        }
                    }

                    // Test if triangle intersects OBBox
                    if CollisionMath::obbox_triangle_intersection(obbox, &triangle) {
                        apt.push(poly_index as u32);
                    }
                }
            }
        } else {
            // Recurse to children
            if let Some(child) = self.nodes.get(node.get_front_child()) {
                self.generate_apt_recursive(child, obbox, mesh, apt, view_dir);
            }
            if let Some(child) = self.nodes.get(node.get_back_child()) {
                self.generate_apt_recursive(child, obbox, mesh, apt, view_dir);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SimpleMesh {
        vertices: Vec<Vec3>,
        polygons: Vec<[u32; 3]>,
    }

    impl MeshGeometry for SimpleMesh {
        fn get_vertex_array(&self) -> &[Vec3] {
            &self.vertices
        }

        fn get_polygon_array(&self) -> &[[u32; 3]] {
            &self.polygons
        }

        fn get_polygon_count(&self) -> usize {
            self.polygons.len()
        }

        fn get_poly_surface_type(&self, _poly_index: usize) -> u32 {
            0
        }
    }

    #[test]
    fn test_obbox_collision_test_creation() {
        let obbox = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::splat(1.0));

        let test = OBBoxCollisionTest::new(obbox, Vec3::X, 0);

        // Sweep bounds should be expanded
        assert!(test.sweep_max.x > 1.0);
    }

    #[test]
    fn test_apt_generation() {
        let mut tree = AABTree::new();
        tree.node_count = 1;
        tree.poly_count = 1;
        tree.nodes = vec![CullNode {
            min: Vec3::splat(-1.0),
            max: Vec3::splat(1.0),
            front_or_poly0: 0 | 0x8000_0000, // Leaf node
            back_or_poly_count: 1,
        }];
        tree.poly_indices = vec![0];

        let mesh = SimpleMesh {
            vertices: vec![
                Vec3::new(-0.5, -0.5, 0.0),
                Vec3::new(0.5, -0.5, 0.0),
                Vec3::new(0.0, 0.5, 0.0),
            ],
            polygons: vec![[0, 1, 2]],
        };

        let obbox = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::splat(1.0));

        let apt = tree.generate_apt(&obbox, &mesh);

        // Should contain at least the triangle
        assert!(!apt.is_empty());
    }
}
