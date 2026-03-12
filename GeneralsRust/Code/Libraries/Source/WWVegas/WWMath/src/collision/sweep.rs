/*
 * Collision/Sweep Tests
 *
 * Moving collision detection for swept volumes.
 * These functions test for collision between moving geometric primitives.
 */

use super::*;
use crate::EPSILON;

impl CollisionMath {
    // ========================================================================================
    // Line Segment Collision Tests
    // ========================================================================================

    /// Collide line segment with AAPlane
    pub fn collide_line_aaplane(
        line: &LineSegment,
        plane: &AAPlane,
        result: &mut CastResult,
    ) -> bool {
        let (start_val, end_val) = match plane.normal {
            AxisEnum::XNormal => (line.start().x, line.end().x),
            AxisEnum::YNormal => (line.start().y, line.end().y),
            AxisEnum::ZNormal => (line.start().z, line.end().z),
        };

        let den = end_val - start_val;

        // Check if line is parallel to plane
        if den.abs() < EPSILON {
            return false;
        }

        let num = plane.dist - start_val;
        let t = num / den;

        // Check if intersection is within line segment
        if !(0.0..=1.0).contains(&t) {
            return false;
        }

        if t < result.fraction {
            result.fraction = t;
            result.normal = match plane.normal {
                AxisEnum::XNormal => Vector3::new(1.0, 0.0, 0.0),
                AxisEnum::YNormal => Vector3::new(0.0, 1.0, 0.0),
                AxisEnum::ZNormal => Vector3::new(0.0, 0.0, 1.0),
            };

            if result.compute_contact_point {
                result.contact_point = line.start() + t * (line.end() - line.start());
            }
            return true;
        }

        false
    }

    /// Collide line segment with plane
    pub fn collide_line_plane(line: &LineSegment, plane: &Plane, result: &mut CastResult) -> bool {
        let dir = line.end() - line.start();
        let den = plane.normal.dot(dir);

        // Check if line is parallel to plane
        if den.abs() < EPSILON {
            return false;
        }

        let num = plane.dist - plane.normal.dot(line.start());
        let t = num / den;

        // Check if intersection is within line segment
        if !(0.0..=1.0).contains(&t) {
            return false;
        }

        if t < result.fraction {
            result.fraction = t;
            result.normal = plane.normal;

            if result.compute_contact_point {
                result.contact_point = line.start() + t * dir;
            }
            return true;
        }

        false
    }

    /// Collide line segment with triangle
    pub fn collide_line_triangle(
        line: &LineSegment,
        tri: &Triangle,
        result: &mut CastResult,
    ) -> bool {
        // Compute triangle normal and plane
        let edge1 = tri.vertices[1] - tri.vertices[0];
        let edge2 = tri.vertices[2] - tri.vertices[0];
        let normal = edge1.cross(edge2).normalize();
        let plane_d = normal.dot(tri.vertices[0]);
        let plane = Plane::new(normal, plane_d);

        // First check intersection with triangle plane
        let dir = line.end() - line.start();
        let den = plane.normal.dot(dir);

        if den.abs() < EPSILON {
            return false;
        }

        let num = plane.dist - plane.normal.dot(line.start());
        let t = num / den;

        if !(0.0..=1.0).contains(&t) {
            return false;
        }

        let intersection_point = line.start() + t * dir;

        // Check if intersection point is inside triangle using barycentric coordinates
        if !Self::point_in_triangle(&intersection_point, tri) {
            return false;
        }

        if t < result.fraction {
            result.fraction = t;
            result.normal = plane.normal;

            if result.compute_contact_point {
                result.contact_point = intersection_point;
            }
            return true;
        }

        false
    }

    /// Collide line segment with sphere
    pub fn collide_line_sphere(
        line: &LineSegment,
        sphere: &Sphere,
        result: &mut CastResult,
    ) -> bool {
        // Based on Graphics Gems ray-sphere intersection
        let dc = sphere.center - line.start();
        let dir = line.end() - line.start();
        let length = dir.length();
        let dir_normalized = dir / length;

        let c_len = dc.dot(dir_normalized);
        let disc = sphere.radius * sphere.radius - (dc.length_squared() - c_len * c_len);

        if disc < 0.0 {
            return false;
        }

        let d = disc.sqrt();
        let mut frac = (c_len - d) / length;

        if frac < 0.0 {
            frac = (c_len + d) / length;
        }

        if frac < 0.0 || frac >= result.fraction {
            return false;
        }

        result.fraction = frac;

        let contact_point = line.start() + (c_len - d) * dir_normalized;
        let norm = (contact_point - sphere.center).normalize();
        result.normal = norm;

        if result.compute_contact_point {
            result.contact_point = line.start() + result.fraction * dir;
        }

        true
    }

    // ========================================================================================
    // AABox Collision Tests
    // ========================================================================================

    /// Collide moving AABox with plane
    pub fn collide_aabox_plane(
        box_ref: &AABox,
        movement: &Vector3,
        plane: &Plane,
        result: &mut CastResult,
    ) -> bool {
        let extent = box_ref.project_to_axis(plane.normal);
        let dist = plane.normal.dot(box_ref.center) - plane.dist;
        let move_dist = plane.normal.dot(*movement);

        let frac = if dist > extent {
            if dist + move_dist > extent {
                1.0 // Entire move OK
            } else {
                (extent - dist) / move_dist // Partial move allowed
            }
        } else if dist < -extent {
            if dist + move_dist < -extent {
                1.0 // Entire move OK
            } else {
                (-extent - dist) / move_dist // Partial move allowed
            }
        } else {
            result.start_bad = true;
            result.normal = plane.normal;
            return true;
        };

        if frac < result.fraction {
            result.fraction = frac;
            result.normal = plane.normal;

            if result.compute_contact_point {
                let move_dir = movement.normalize();
                let move_extent = box_ref.extent.dot(move_dir);
                result.contact_point =
                    box_ref.center + *movement * result.fraction + move_dir * move_extent;
            }
            return true;
        }

        false
    }

    /// Collide moving AABox with triangle
    pub fn collide_aabox_triangle(
        box_ref: &AABox,
        movement: &Vector3,
        tri: &Triangle,
        result: &mut CastResult,
    ) -> bool {
        #[cfg(feature = "collision-stats")]
        {
            // Track statistics if enabled
        }

        let mut context = AABTriCollisionContext::new(box_ref, movement, tri, &Vector3::ZERO);

        // Test triangle normal
        if context.check_normal_axis() {
            return Self::finalize_aabtri_collision(context, result);
        }

        // Test box axes
        for axis in 0..3 {
            if context.check_basis_axis(axis) {
                return Self::finalize_aabtri_collision(context, result);
            }
        }

        // Test cross product axes
        for box_axis in 0..3 {
            for edge_idx in 0..3 {
                if context.check_cross_axis(box_axis, edge_idx) {
                    return Self::finalize_aabtri_collision(context, result);
                }
            }
        }

        // Test axes based on movement vector
        if !context.start_bad {
            for axis in 0..3 {
                if context.check_move_axis(axis) {
                    return Self::finalize_aabtri_collision(context, result);
                }
            }
        }

        Self::finalize_aabtri_collision(context, result)
    }

    /// Collide two moving AABoxes
    pub fn collide_aabox_aabox(
        box1: &AABox,
        move1: &Vector3,
        box2: &AABox,
        move2: &Vector3,
        result: &mut CastResult,
    ) -> bool {
        let relative_move = *move2 - *move1;
        let mut context = AABCollisionContext::new(box1, move1, box2, &relative_move);

        // Test separation on each axis
        for axis in 0..3 {
            if context.separation_test(axis) {
                return Self::finalize_aab_collision(context, result);
            }
        }

        Self::finalize_aab_collision(context, result)
    }

    // ========================================================================================
    // Helper Functions
    // ========================================================================================

    /// Check if a point is inside a triangle using barycentric coordinates
    fn point_in_triangle(point: &Vector3, tri: &Triangle) -> bool {
        let v0 = tri.vertices[2] - tri.vertices[0];
        let v1 = tri.vertices[1] - tri.vertices[0];
        let v2 = *point - tri.vertices[0];

        let dot00 = v0.dot(v0);
        let dot01 = v0.dot(v1);
        let dot02 = v0.dot(v2);
        let dot11 = v1.dot(v1);
        let dot12 = v1.dot(v2);

        let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
        let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
        let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

        (u >= 0.0) && (v >= 0.0) && (u + v <= 1.0)
    }

    fn finalize_aabtri_collision(
        mut context: AABTriCollisionContext,
        result: &mut CastResult,
    ) -> bool {
        if context.max_frac < 0.0 {
            context.max_frac = 0.0;
        }

        if context.start_bad {
            result.start_bad = true;
            result.fraction = 0.0;
            result.normal = context.triangle.normal;
            return true;
        }

        if context.max_frac <= result.fraction && context.max_frac < 1.0 {
            let normal = context.compute_contact_normal();

            if (context.max_frac - result.fraction).abs() > EPSILON
                || normal.dot(context.box_move) < result.normal.dot(context.box_move)
            {
                result.normal = normal;
            }

            result.fraction = context.max_frac;

            if result.compute_contact_point {
                result.contact_point = context.compute_contact_point();
            }

            return true;
        }

        false
    }

    fn finalize_aab_collision(context: AABCollisionContext, result: &mut CastResult) -> bool {
        if context.start_bad {
            result.start_bad = true;
            result.fraction = 0.0;
            return true;
        }

        if context.max_frac < result.fraction {
            result.fraction = context.max_frac;
            result.normal = Vector3::ZERO;
            result.normal[context.axis_id] = -context.side;

            if result.compute_contact_point {
                // Contact point computation for AABox-AABox not currently supported
            }

            return true;
        }

        false
    }
}

// ========================================================================================
// Context Structures for Complex Collision Tests
// ========================================================================================

#[allow(dead_code)]
struct AABTriCollisionContext {
    box_ref: AABox,
    triangle: Triangle,
    box_move: Vector3,
    _tri_move: Vector3,
    start_bad: bool,
    max_frac: f32,
    axis_id: i32,
    _point: i32,
    side: i32,
    d: Vector3,          // Vector from box center to triangle vertex 0
    movement: Vector3,   // Relative movement vector
    edges: [Vector3; 3], // Triangle edge vectors
    normal: Vector3,     // Triangle normal
}

impl AABTriCollisionContext {
    fn new(box_ref: &AABox, box_move: &Vector3, tri: &Triangle, tri_move: &Vector3) -> Self {
        let d = tri.vertices[0] - box_ref.center;
        let movement = *box_move - *tri_move;
        let edges = [
            tri.vertices[1] - tri.vertices[0],
            tri.vertices[2] - tri.vertices[0],
            (tri.vertices[2] - tri.vertices[0]) - (tri.vertices[1] - tri.vertices[0]),
        ];
        let normal = edges[0].cross(edges[1]);

        Self {
            box_ref: *box_ref,
            triangle: tri.clone(),
            box_move: *box_move,
            _tri_move: *tri_move,
            start_bad: true,
            max_frac: -0.01,
            axis_id: 0,
            _point: 0,
            side: 0,
            d,
            movement,
            edges,
            normal,
        }
    }

    fn check_normal_axis(&mut self) -> bool {
        // Implementation of triangle normal axis test
        false // Placeholder
    }

    fn check_basis_axis(&mut self, _axis: usize) -> bool {
        // Implementation of box basis axis test
        false // Placeholder
    }

    fn check_cross_axis(&mut self, _box_axis: usize, _edge_idx: usize) -> bool {
        // Implementation of cross product axis test
        false // Placeholder
    }

    fn check_move_axis(&mut self, _axis: usize) -> bool {
        // Implementation of movement vector axis test
        false // Placeholder
    }

    fn compute_contact_normal(&self) -> Vector3 {
        // Compute collision normal based on separating axis
        self.normal.normalize()
    }

    fn compute_contact_point(&self) -> Vector3 {
        // Compute contact point based on collision configuration
        self.box_ref.center
    }
}

#[allow(dead_code)]
struct AABCollisionContext {
    box1: AABox,
    box2: AABox,
    _move1: Vector3,
    relative_move: Vector3,
    start_bad: bool,
    max_frac: f32,
    axis_id: usize,
    side: f32,
}

impl AABCollisionContext {
    fn new(box1: &AABox, move1: &Vector3, box2: &AABox, relative_move: &Vector3) -> Self {
        Self {
            box1: *box1,
            box2: *box2,
            _move1: *move1,
            relative_move: *relative_move,
            start_bad: true,
            max_frac: 0.0,
            axis_id: 0,
            side: 0.0,
        }
    }

    fn separation_test(&mut self, axis: usize) -> bool {
        let extents1 = [self.box1.extent.x, self.box1.extent.y, self.box1.extent.z];
        let extents2 = [self.box2.extent.x, self.box2.extent.y, self.box2.extent.z];
        let centers1 = [self.box1.center.x, self.box1.center.y, self.box1.center.z];
        let centers2 = [self.box2.center.x, self.box2.center.y, self.box2.center.z];
        let move_vals = [
            self.relative_move.x,
            self.relative_move.y,
            self.relative_move.z,
        ];

        let ra = extents1[axis];
        let rb = extents2[axis];
        let u0 = centers2[axis] - centers1[axis];
        let u1 = u0 + move_vals[axis];

        let rsum = ra + rb;

        if u0 + EPSILON > rsum {
            self.start_bad = false;
            if u1 > rsum {
                self.max_frac = 1.0;
                return true;
            } else {
                let tmp = (rsum - u0) / (u1 - u0);
                if tmp > self.max_frac {
                    self.max_frac = tmp;
                    self.axis_id = axis;
                    self.side = 1.0;
                }
            }
        } else if u0 - EPSILON < -rsum {
            self.start_bad = false;
            if u1 < -rsum {
                self.max_frac = 1.0;
                return true;
            } else {
                let tmp = (-rsum - u0) / (u1 - u0);
                if tmp > self.max_frac {
                    self.max_frac = tmp;
                    self.axis_id = axis;
                    self.side = -1.0;
                }
            }
        }

        false
    }
}
