/*
 * Intersection Tests
 *
 * Simple boolean intersection tests between geometric primitives.
 * These functions return true if the objects intersect, false otherwise.
 */

use super::*;
use crate::EPSILON;

impl CollisionMath {
    /// Test intersection between two axis-aligned bounding boxes
    pub fn intersection_test_aabox_aabox(box1: &AABox, box2: &AABox) -> bool {
        let dc = box2.center - box1.center;

        if box1.extent.x + box2.extent.x < dc.x.abs() {
            return false;
        }
        if box1.extent.y + box2.extent.y < dc.y.abs() {
            return false;
        }
        if box1.extent.z + box2.extent.z < dc.z.abs() {
            return false;
        }

        true
    }

    /// Test intersection between AABox and triangle
    pub fn intersection_test_aabox_triangle(box_ref: &AABox, tri: &Triangle) -> bool {
        // Use separating axis theorem with 13 potential separating axes
        let context = AABTriIntersectContext::new(box_ref, tri);

        // Test triangle normal
        if context.check_normal_axis() {
            return false;
        }

        // Test box axes (3 tests)
        if context.check_basis_axis(0, box_ref.extent.x) {
            return false;
        }
        if context.check_basis_axis(1, box_ref.extent.y) {
            return false;
        }
        if context.check_basis_axis(2, box_ref.extent.z) {
            return false;
        }

        // Test cross product axes (9 tests)
        for box_axis in 0..3 {
            for edge_idx in 0..3 {
                if context.check_cross_axis(box_axis, edge_idx) {
                    return false;
                }
            }
        }

        true
    }

    /// Test intersection between sphere and AABox
    pub fn intersection_test_sphere_aabox(sphere: &Sphere, box_ref: &AABox) -> bool {
        // Find closest point on box to sphere center
        let closest_point = Vector3::new(
            sphere.center.x.clamp(
                box_ref.center.x - box_ref.extent.x,
                box_ref.center.x + box_ref.extent.x,
            ),
            sphere.center.y.clamp(
                box_ref.center.y - box_ref.extent.y,
                box_ref.center.y + box_ref.extent.y,
            ),
            sphere.center.z.clamp(
                box_ref.center.z - box_ref.extent.z,
                box_ref.center.z + box_ref.extent.z,
            ),
        );

        let distance_sq = (sphere.center - closest_point).length_squared();
        distance_sq <= sphere.radius * sphere.radius
    }

    /// Test intersection between two spheres
    pub fn intersection_test_sphere_sphere(sphere1: &Sphere, sphere2: &Sphere) -> bool {
        let radius_sum = sphere1.radius + sphere2.radius;
        let distance_sq = (sphere1.center - sphere2.center).length_squared();
        distance_sq <= radius_sum * radius_sum
    }

    /// C++ CollisionMath::Intersection_Test(AABox, OBBox)
    pub fn intersection_test_aabox_obbox(box1: &AABox, box2: &OBBox) -> bool {
        let a = OBBox::from_center_extent(box1.center, box1.extent);
        Self::intersection_test_obbox_obbox(&a, box2)
    }

    /// C++ CollisionMath::Intersection_Test(OBBox, Tri)
    pub fn intersection_test_obbox_triangle(box_ref: &OBBox, tri: &Triangle) -> bool {
        let local_v0 = box_ref
            .basis
            .transpose_rotate_vector(tri.vertices[0] - box_ref.center);
        let local_v1 = box_ref
            .basis
            .transpose_rotate_vector(tri.vertices[1] - box_ref.center);
        let local_v2 = box_ref
            .basis
            .transpose_rotate_vector(tri.vertices[2] - box_ref.center);
        let local_box = AABox::new(Vector3::ZERO, box_ref.extent);
        let local_tri = Triangle::new(local_v0, local_v1, local_v2);
        Self::intersection_test_aabox_triangle(&local_box, &local_tri)
    }

    /// C++ CollisionMath::Intersection_Test(OBBox, AABox)
    pub fn intersection_test_obbox_aabox(box1: &OBBox, box2: &AABox) -> bool {
        Self::intersection_test_aabox_obbox(box2, box1)
    }

    /// C++ CollisionMath::Intersection_Test(OBBox, OBBox) — 15-axis SAT
    pub fn intersection_test_obbox_obbox(a: &OBBox, b: &OBBox) -> bool {
        let a_axes = [
            Vector3::new(a.basis.row[0][0], a.basis.row[1][0], a.basis.row[2][0]),
            Vector3::new(a.basis.row[0][1], a.basis.row[1][1], a.basis.row[2][1]),
            Vector3::new(a.basis.row[0][2], a.basis.row[1][2], a.basis.row[2][2]),
        ];
        let b_axes = [
            Vector3::new(b.basis.row[0][0], b.basis.row[1][0], b.basis.row[2][0]),
            Vector3::new(b.basis.row[0][1], b.basis.row[1][1], b.basis.row[2][1]),
            Vector3::new(b.basis.row[0][2], b.basis.row[1][2], b.basis.row[2][2]),
        ];
        let t = b.center - a.center;
        let project = |box_ref: &OBBox, axes: &[Vector3; 3], axis: Vector3| {
            box_ref.extent.x * axes[0].dot(axis).abs()
                + box_ref.extent.y * axes[1].dot(axis).abs()
                + box_ref.extent.z * axes[2].dot(axis).abs()
        };
        let separated = |axis: Vector3| {
            if axis.length_squared() <= crate::EPSILON2 {
                return false;
            }
            t.dot(axis).abs() > project(a, &a_axes, axis) + project(b, &b_axes, axis)
        };
        for &axis in &a_axes {
            if separated(axis) {
                return false;
            }
        }
        for &axis in &b_axes {
            if separated(axis) {
                return false;
            }
        }
        for &a_axis in &a_axes {
            for &b_axis in &b_axes {
                if separated(a_axis.cross(b_axis)) {
                    return false;
                }
            }
        }
        true
    }

    /// C++ CollisionMath::Intersection_Test(Sphere, OBBox)
    pub fn intersection_test_sphere_obbox(sphere: &Sphere, box_ref: &OBBox) -> bool {
        let local = box_ref
            .basis
            .transpose_rotate_vector(sphere.center - box_ref.center);
        let local_box = AABox::new(Vector3::ZERO, box_ref.extent);
        let local_sphere = Sphere {
            center: local,
            radius: sphere.radius,
        };
        Self::intersection_test_sphere_aabox(&local_sphere, &local_box)
    }
}

/// Context structure for AABox-Triangle intersection tests
#[allow(dead_code)] // C++ parity
struct AABTriIntersectContext {
    box_ref: AABox,
    triangle: Triangle,
    d: Vector3,          // Vector from box center to triangle vertex 0
    edges: [Vector3; 3], // Triangle edge vectors
    normal: Vector3,     // Triangle normal (not normalized)
    ae: [[f32; 3]; 3],   // Dot products of box axes and triangle edges
    an: [f32; 3],        // Dot products of box axes and triangle normal
}

impl AABTriIntersectContext {
    fn new(box_ref: &AABox, tri: &Triangle) -> Self {
        let d = tri.vertices[0] - box_ref.center;
        let edges = [
            tri.vertices[1] - tri.vertices[0],
            tri.vertices[2] - tri.vertices[0],
            (tri.vertices[2] - tri.vertices[0]) - (tri.vertices[1] - tri.vertices[0]),
        ];
        let normal = edges[0].cross(edges[1]);

        let ae = [
            [edges[0].x, edges[1].x, edges[2].x],
            [edges[0].y, edges[1].y, edges[2].y],
            [edges[0].z, edges[1].z, edges[2].z],
        ];

        let an = [normal.x, normal.y, normal.z];

        Self {
            box_ref: *box_ref,
            triangle: tri.clone(),
            d,
            edges,
            normal,
            ae,
            an,
        }
    }

    fn check_normal_axis(&self) -> bool {
        let dist = self.d.dot(self.normal);
        let leb0 = self.box_ref.extent.x * self.an[0].abs()
            + self.box_ref.extent.y * self.an[1].abs()
            + self.box_ref.extent.z * self.an[2].abs();

        let lp = if dist < 0.0 { -dist } else { dist };
        lp - leb0 > -EPSILON
    }

    fn check_basis_axis(&self, axis: usize, extent: f32) -> bool {
        let dist = match axis {
            0 => self.d.x,
            1 => self.d.y,
            2 => self.d.z,
            _ => unreachable!(),
        };

        let dp1 = self.ae[axis][0];
        let dp2 = self.ae[axis][1];

        let (dist, dp1, dp2) = if dist < 0.0 {
            (-dist, -dp1, -dp2)
        } else {
            (dist, dp1, dp2)
        };

        let lp = dist + dp1.min(0.0).min(dp2.min(0.0));
        lp - extent > -EPSILON
    }

    fn check_cross_axis(&self, box_axis: usize, edge_idx: usize) -> bool {
        let axis = match (box_axis, edge_idx) {
            (0, _) => Vector3::new(0.0, -self.edges[edge_idx].z, self.edges[edge_idx].y),
            (1, _) => Vector3::new(self.edges[edge_idx].z, 0.0, -self.edges[edge_idx].x),
            (2, _) => Vector3::new(-self.edges[edge_idx].y, self.edges[edge_idx].x, 0.0),
            _ => unreachable!(),
        };

        if axis.length_squared() <= EPSILON * EPSILON {
            return false;
        }

        let p0 = self.d.dot(axis);
        let dp = if edge_idx < 2 {
            if edge_idx == 0 {
                self.an[box_axis]
            } else {
                -self.an[box_axis]
            }
        } else {
            -self.an[box_axis]
        };

        let (p0, dp) = if p0 < 0.0 { (-p0, -dp) } else { (p0, dp) };

        let leb0 = match box_axis {
            0 => {
                self.box_ref.extent.y * self.ae[2][edge_idx].abs()
                    + self.box_ref.extent.z * self.ae[1][edge_idx].abs()
            }
            1 => {
                self.box_ref.extent.x * self.ae[2][edge_idx].abs()
                    + self.box_ref.extent.z * self.ae[0][edge_idx].abs()
            }
            2 => {
                self.box_ref.extent.x * self.ae[1][edge_idx].abs()
                    + self.box_ref.extent.y * self.ae[0][edge_idx].abs()
            }
            _ => unreachable!(),
        };

        let lp = p0 + if dp < 0.0 { dp } else { 0.0 };
        lp - leb0 > -EPSILON
    }
}
