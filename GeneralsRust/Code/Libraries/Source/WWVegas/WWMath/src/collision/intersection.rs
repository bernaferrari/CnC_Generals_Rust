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
}

/// Context structure for AABox-Triangle intersection tests
#[allow(dead_code)]
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
