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
    // Line-box (colmathline.cpp Test_Aligned_Box) and OBBox collide
    // ========================================================================================

    /// Collide line segment with AABox — C++ CollisionMath::Collide(LineSeg, AABox)
    pub fn collide_line_aabox(
        line: &LineSegment,
        box_ref: &AABox,
        result: &mut CastResult,
    ) -> bool {
        let mut test = AlignedBoxRayTest::new(box_ref, line.get_p0(), line.get_dp());
        if !test.run() {
            return false;
        }
        if test.inside {
            result.start_bad = true;
            result.fraction = 0.0;
            return true;
        }
        if test.fraction < result.fraction {
            result.fraction = test.fraction;
            result.normal = BOX_FACE_NORMALS[test.axis][test.side];
            if result.compute_contact_point {
                result.contact_point = line.get_p0() + result.fraction * line.get_dp();
            }
            return true;
        }
        false
    }

    /// Collide line with OBBox — C++ CollisionMath::Collide(LineSeg, OBBox)
    /// Transforms the ray by Basis.Transpose() then runs the aligned slab test.
    pub fn collide_line_obbox(
        line: &LineSegment,
        box_ref: &OBBox,
        result: &mut CastResult,
    ) -> bool {
        let local_p0 = box_ref
            .basis
            .transpose_rotate_vector(line.get_p0() - box_ref.center)
            + box_ref.center;
        let local_dp = box_ref.basis.transpose_rotate_vector(line.get_dp());
        let aligned = AABox::new(box_ref.center, box_ref.extent);
        let mut test = AlignedBoxRayTest::new(&aligned, local_p0, local_dp);
        if !test.run() {
            return false;
        }
        if test.inside {
            result.start_bad = true;
            result.fraction = 0.0;
            return true;
        }
        if test.fraction < result.fraction {
            result.fraction = test.fraction;
            let axis = test.axis;
            let mut normal = Vector3::new(
                box_ref.basis.row[0][axis],
                box_ref.basis.row[1][axis],
                box_ref.basis.row[2][axis],
            );
            if test.side == BOX_SIDE_NEGATIVE {
                normal = -normal;
            }
            result.normal = normal;
            if result.compute_contact_point {
                result.contact_point = line.get_p0() + result.fraction * line.get_dp();
            }
            return true;
        }
        false
    }

    /// Collide moving AABox into OBBox — C++ CollisionMath::Collide(AABox, move, OBBox, move2)
    pub fn collide_aabox_obbox(
        box1: &AABox,
        move1: &Vector3,
        box2: &OBBox,
        move2: &Vector3,
        result: &mut CastResult,
    ) -> bool {
        let a = OBBox::from_center_extent(box1.center, box1.extent);
        Self::collide_obbox_obbox(&a, move1, box2, move2, result)
    }

    /// Collide moving OBBox into plane — C++ CollisionMath::Collide(OBBox, move, Plane)
    pub fn collide_obbox_plane(
        box_ref: &OBBox,
        movement: &Vector3,
        plane: &Plane,
        result: &mut CastResult,
    ) -> bool {
        let a0 = basis_column(&box_ref.basis, 0);
        let a1 = basis_column(&box_ref.basis, 1);
        let a2 = basis_column(&box_ref.basis, 2);
        let extent = box_ref.extent.x * plane.normal.dot(a0).abs()
            + box_ref.extent.y * plane.normal.dot(a1).abs()
            + box_ref.extent.z * plane.normal.dot(a2).abs();
        let dist = plane.normal.dot(box_ref.center) - plane.dist;
        let move_dist = plane.normal.dot(*movement);

        let frac = if dist > extent {
            if dist + move_dist > extent {
                1.0
            } else {
                (extent - dist) / move_dist
            }
        } else if dist < -extent {
            if dist + move_dist < -extent {
                1.0
            } else {
                (-extent - dist) / move_dist
            }
        } else {
            result.start_bad = true;
            result.normal = plane.normal;
            result.fraction = 0.0;
            return true;
        };

        if frac < result.fraction {
            result.fraction = frac;
            result.normal = plane.normal;
            if result.compute_contact_point {
                result.contact_point = box_ref.center + *movement * result.fraction;
            }
            return true;
        }
        false
    }

    /// Swept OBBox vs triangle — C++ CollisionMath::Collide(OBBox, move, Tri, move2)
    /// Transforms the triangle into the box basis and runs the AABox-tri swept SAT.
    pub fn collide_obbox_triangle(
        box_ref: &OBBox,
        movement: &Vector3,
        tri: &Triangle,
        tri_move: &Vector3,
        result: &mut CastResult,
    ) -> bool {
        let rel_move = *movement - *tri_move;
        let local_move = box_ref.basis.transpose_rotate_vector(rel_move);
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
        let mut local_result = CastResult {
            start_bad: false,
            fraction: result.fraction,
            normal: Vector3::ZERO,
            surface_type: result.surface_type,
            compute_contact_point: result.compute_contact_point,
            contact_point: Vector3::ZERO,
        };
        if Self::collide_aabox_triangle(&local_box, &local_move, &local_tri, &mut local_result) {
            result.start_bad = local_result.start_bad;
            result.fraction = local_result.fraction;
            result.normal = box_ref.basis.rotate_vector(local_result.normal);
            result.surface_type = local_result.surface_type;
            if result.compute_contact_point {
                result.contact_point =
                    box_ref.center + box_ref.basis.rotate_vector(local_result.contact_point);
            }
            true
        } else {
            false
        }
    }

    /// Collide moving OBBox into AABox — C++ CollisionMath::Collide(OBBox, move, AABox, move2)
    pub fn collide_obbox_aabox(
        box1: &OBBox,
        move1: &Vector3,
        box2: &AABox,
        move2: &Vector3,
        result: &mut CastResult,
    ) -> bool {
        let b = OBBox::from_center_extent(box2.center, box2.extent);
        Self::collide_obbox_obbox(box1, move1, &b, move2, result)
    }

    /// Swept OBBox-OBBox SAT — C++ CollisionMath::Collide(OBBox, move, OBBox, move2)
    pub fn collide_obbox_obbox(
        box1: &OBBox,
        move1: &Vector3,
        box2: &OBBox,
        move2: &Vector3,
        result: &mut CastResult,
    ) -> bool {
        let mut ctx = ObbObbSweepContext::new(box1, move1, box2, move2);
        let a_axes = [
            basis_column(&box1.basis, 0),
            basis_column(&box1.basis, 1),
            basis_column(&box1.basis, 2),
        ];
        let b_axes = [
            basis_column(&box2.basis, 0),
            basis_column(&box2.basis, 1),
            basis_column(&box2.basis, 2),
        ];
        for axis in a_axes {
            if ctx.check_axis(axis, box1, box2) {
                return finalize_obb_obb(&ctx, result);
            }
        }
        for axis in b_axes {
            if ctx.check_axis(axis, box1, box2) {
                return finalize_obb_obb(&ctx, result);
            }
        }
        for &a_axis in &a_axes {
            for &b_axis in &b_axes {
                let axis = a_axis.cross(b_axis);
                if axis.length_squared() > crate::EPSILON2 && ctx.check_axis(axis, box1, box2) {
                    return finalize_obb_obb(&ctx, result);
                }
            }
        }
        finalize_obb_obb(&ctx, result)
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

const AXIS_INTERSECTION: i32 = 0;
const AXIS_N: i32 = 1;
const AXIS_A0: i32 = 2;
const AXIS_A1: i32 = 3;
const AXIS_A2: i32 = 4;
const AXIS_A0E0: i32 = 5;

const BOX_SIDE_NEGATIVE: usize = 0;
const BOX_SIDE_POSITIVE: usize = 1;

const BOX_FACE_NORMALS: [[Vector3; 2]; 3] = [
    [Vector3::new(-1.0, 0.0, 0.0), Vector3::new(1.0, 0.0, 0.0)],
    [Vector3::new(0.0, -1.0, 0.0), Vector3::new(0.0, 1.0, 0.0)],
    [Vector3::new(0.0, 0.0, -1.0), Vector3::new(0.0, 0.0, 1.0)],
];

/// C++ colmathaabtri.cpp BTCollisionStruct + axis tests.
struct AABTriCollisionContext {
    box_ref: AABox,
    triangle: Triangle,
    box_move: Vector3,
    start_bad: bool,
    max_frac: f32,
    axis_id: i32,
    point: i32,
    side: i32,
    test_axis_id: i32,
    test_side: i32,
    test_point: i32,
    test_axis: Vector3,
    d: Vector3,
    movement: Vector3,
    edges: [Vector3; 3],
    normal: Vector3,
    ae: [[f32; 3]; 3],
    an: [f32; 3],
    axe: [[Vector3; 3]; 3],
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
        let ae = [
            [edges[0].x, edges[1].x, edges[2].x],
            [edges[0].y, edges[1].y, edges[2].y],
            [edges[0].z, edges[1].z, edges[2].z],
        ];
        let an = [normal.x, normal.y, normal.z];
        Self {
            box_ref: *box_ref,
            triangle: tri.clone(),
            box_move: *box_move,
            start_bad: true,
            max_frac: -0.01,
            axis_id: AXIS_INTERSECTION,
            point: 0,
            side: 0,
            test_axis_id: AXIS_INTERSECTION,
            test_side: 1,
            test_point: 0,
            test_axis: Vector3::ZERO,
            d,
            movement,
            edges,
            normal,
            ae,
            an,
            axe: [[Vector3::ZERO; 3]; 3],
        }
    }

    /// C++ aabtri_separation_test
    fn separation_test(&mut self, lp: f32, leb0: f32, leb1: f32) -> bool {
        let mut eps = 0.0;
        if lp - leb0 <= 0.0 {
            eps = COLLISION_EPSILON * self.test_axis.length();
        }
        if lp - leb0 > -eps {
            self.start_bad = false;
            if leb1 - leb0 > 0.0 {
                let frac = (lp - leb0) / (leb1 - leb0);
                if frac >= 1.0 {
                    self.axis_id = self.test_axis_id;
                    self.max_frac = 1.0;
                    return true;
                } else if frac > self.max_frac {
                    self.max_frac = frac;
                    self.axis_id = self.test_axis_id;
                    self.side = self.test_side;
                    self.point = self.test_point;
                }
            } else {
                self.axis_id = self.test_axis_id;
                self.max_frac = 1.0;
                return true;
            }
        }
        false
    }

    /// C++ aabtri_check_normal_axis
    fn check_normal_axis(&mut self) -> bool {
        self.test_axis = self.normal;
        self.test_axis_id = AXIS_N;
        let mut dist = self.d.dot(self.test_axis);
        let mut axismove = self.movement.dot(self.test_axis);
        if dist < 0.0 {
            dist = -dist;
            axismove = -axismove;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }
        let leb0 = self.box_ref.extent.x * self.an[0].abs()
            + self.box_ref.extent.y * self.an[1].abs()
            + self.box_ref.extent.z * self.an[2].abs();
        let leb1 = leb0 + axismove;
        self.test_point = 0;
        self.separation_test(dist, leb0, leb1)
    }

    /// C++ aabtri_check_basis_axis
    fn check_basis_axis(&mut self, axis: usize) -> bool {
        self.test_axis = match axis {
            0 => Vector3::X,
            1 => Vector3::Y,
            _ => Vector3::Z,
        };
        self.test_axis_id = AXIS_A0 + axis as i32;
        let mut dist = self.d.dot(self.test_axis);
        let mut axismove = self.movement.dot(self.test_axis);
        let mut dp1 = self.ae[axis][0];
        let mut dp2 = self.ae[axis][1];
        if dist < 0.0 {
            dist = -dist;
            axismove = -axismove;
            dp1 = -dp1;
            dp2 = -dp2;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }
        let leb0 = match axis {
            0 => self.box_ref.extent.x,
            1 => self.box_ref.extent.y,
            _ => self.box_ref.extent.z,
        };
        let leb1 = leb0 + axismove;
        let mut lp = 0.0;
        self.test_point = 0;
        if dp1 < lp {
            lp = dp1;
            self.test_point = 1;
        }
        if dp2 < lp {
            lp = dp2;
            self.test_point = 2;
        }
        self.separation_test(dist + lp, leb0, leb1)
    }

    /// C++ aabtri_check_cross_axis
    fn check_cross_axis(&mut self, box_axis: usize, edge_idx: usize) -> bool {
        let e = self.edges[edge_idx];
        let axis = match box_axis {
            0 => Vector3::new(0.0, -e.z, e.y),
            1 => Vector3::new(e.z, 0.0, -e.x),
            _ => Vector3::new(-e.y, e.x, 0.0),
        };
        self.axe[box_axis][edge_idx] = axis;
        if axis.length_squared() <= crate::EPSILON2 {
            return false;
        }
        self.test_axis = axis;
        self.test_axis_id = AXIS_A0E0 + (edge_idx as i32) * 3 + box_axis as i32;
        let mut dp = if edge_idx == 0 {
            self.an[box_axis]
        } else {
            -self.an[box_axis]
        };
        let dpi = if edge_idx == 0 { 2 } else { 1 };
        let leb0 = match box_axis {
            0 => {
                self.box_ref.extent.y * self.ae[2][edge_idx].abs()
                    + self.box_ref.extent.z * self.ae[1][edge_idx].abs()
            }
            1 => {
                self.box_ref.extent.x * self.ae[2][edge_idx].abs()
                    + self.box_ref.extent.z * self.ae[0][edge_idx].abs()
            }
            _ => {
                self.box_ref.extent.x * self.ae[1][edge_idx].abs()
                    + self.box_ref.extent.y * self.ae[0][edge_idx].abs()
            }
        };
        let mut p0 = self.d.dot(self.test_axis);
        let mut axismove = self.movement.dot(self.test_axis);
        if p0 < 0.0 {
            p0 = -p0;
            axismove = -axismove;
            dp = -dp;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }
        let leb1 = leb0 + axismove;
        let mut lp = 0.0;
        self.test_point = 0;
        if dp < 0.0 {
            lp = dp;
            self.test_point = dpi;
        }
        self.separation_test(p0 + lp, leb0, leb1)
    }

    /// C++ last-ditch A0/A1/A2 × Move axes
    fn check_move_axis(&mut self, axis: usize) -> bool {
        self.test_point = self.point;
        self.test_axis_id = self.axis_id;
        self.test_axis = match axis {
            0 => Vector3::new(0.0, -self.movement.z, self.movement.y),
            1 => Vector3::new(self.movement.z, 0.0, -self.movement.x),
            _ => Vector3::new(-self.movement.y, self.movement.x, 0.0),
        };
        if self.test_axis.length_squared() <= crate::EPSILON2 {
            return false;
        }
        let mut dist = self.d.dot(self.test_axis);
        let mut axismove = self.movement.dot(self.test_axis);
        if dist < 0.0 {
            dist = -dist;
            axismove = -axismove;
            self.test_axis = -self.test_axis;
            self.test_side = -1;
        } else {
            self.test_side = 1;
        }
        let leb0 = self.box_ref.extent.x * self.test_axis.x.abs()
            + self.box_ref.extent.y * self.test_axis.y.abs()
            + self.box_ref.extent.z * self.test_axis.z.abs();
        let leb1 = leb0 + axismove;
        let mut lp = 0.0;
        let tmp0 = self.edges[0].dot(self.test_axis);
        if tmp0 < lp {
            lp = tmp0;
        }
        let tmp1 = self.edges[1].dot(self.test_axis);
        if tmp1 < lp {
            lp = tmp1;
        }
        self.separation_test(dist + lp, leb0, leb1)
    }

    /// C++ aabtri_compute_contact_normal
    fn compute_contact_normal(&self) -> Vector3 {
        let side = -(self.side as f32);
        let n = match self.axis_id {
            AXIS_N => side * self.normal,
            AXIS_A0 => side * Vector3::X,
            AXIS_A1 => side * Vector3::Y,
            AXIS_A2 => side * Vector3::Z,
            id if (AXIS_A0E0..=AXIS_A0E0 + 8).contains(&id) => {
                let idx = (id - AXIS_A0E0) as usize;
                let box_axis = idx % 3;
                let edge_idx = idx / 3;
                side * self.axe[box_axis][edge_idx]
            }
            _ => self.normal,
        };
        let len = n.length();
        if len > crate::EPSILON {
            n / len
        } else {
            let fallback = self.normal;
            let fl = fallback.length();
            if fl > crate::EPSILON {
                fallback / fl
            } else {
                Vector3::Z
            }
        }
    }

    fn compute_contact_point(&self) -> Vector3 {
        self.box_ref.center + self.box_move * self.max_frac
    }
}

#[allow(dead_code)] // C++ parity
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

/// C++ colmathline.cpp Test_Aligned_Box
struct AlignedBoxRayTest {
    min: Vector3,
    max: Vector3,
    p0: Vector3,
    dp: Vector3,
    fraction: f32,
    inside: bool,
    axis: usize,
    side: usize,
}

impl AlignedBoxRayTest {
    fn new(box_ref: &AABox, p0: Vector3, dp: Vector3) -> Self {
        Self {
            min: box_ref.center - box_ref.extent,
            max: box_ref.center + box_ref.extent,
            p0,
            dp,
            fraction: 0.0,
            inside: false,
            axis: 0,
            side: BOX_SIDE_POSITIVE,
        }
    }

    fn run(&mut self) -> bool {
        let p0 = [self.p0.x, self.p0.y, self.p0.z];
        let dp = [self.dp.x, self.dp.y, self.dp.z];
        let min = [self.min.x, self.min.y, self.min.z];
        let max = [self.max.x, self.max.y, self.max.z];
        let mut candidate = [0.0f32; 3];
        let mut maxt = [0.0f32; 3];
        let mut quadrant = [0usize; 3];
        let mut inside = true;

        for i in 0..3 {
            if p0[i] < min[i] {
                quadrant[i] = BOX_SIDE_NEGATIVE;
                candidate[i] = min[i];
                inside = false;
            } else if p0[i] > max[i] {
                quadrant[i] = BOX_SIDE_POSITIVE;
                candidate[i] = max[i];
                inside = false;
            } else {
                quadrant[i] = 2; // BOX_SIDE_MIDDLE
            }
        }

        if inside {
            self.fraction = 0.0;
            self.inside = true;
            return true;
        }

        for i in 0..3 {
            if quadrant[i] != 2 && dp[i] != 0.0 {
                maxt[i] = (candidate[i] - p0[i]) / dp[i];
            } else {
                maxt[i] = -1.0;
            }
        }

        let mut plane = 0usize;
        for i in 1..3 {
            if maxt[i] > maxt[plane] {
                plane = i;
            }
        }
        if maxt[plane] < 0.0 {
            return false;
        }

        for i in 0..3 {
            if plane != i {
                let coord = p0[i] + maxt[plane] * dp[i];
                if coord < min[i] || coord > max[i] {
                    return false;
                }
            }
        }

        self.fraction = maxt[plane];
        self.inside = false;
        self.axis = plane;
        self.side = quadrant[plane];
        true
    }
}

fn basis_column(basis: &crate::Matrix3, i: usize) -> Vector3 {
    Vector3::new(basis.row[0][i], basis.row[1][i], basis.row[2][i])
}

fn project_obb_extent(box_ref: &OBBox, axis: Vector3) -> f32 {
    box_ref.extent.x * axis.dot(basis_column(&box_ref.basis, 0)).abs()
        + box_ref.extent.y * axis.dot(basis_column(&box_ref.basis, 1)).abs()
        + box_ref.extent.z * axis.dot(basis_column(&box_ref.basis, 2)).abs()
}

/// Swept SAT scratch for C++ collide_obb_obb (colmathobbobb.cpp)
struct ObbObbSweepContext {
    d: Vector3,
    move_rel: Vector3,
    start_bad: bool,
    max_frac: f32,
    test_axis: Vector3,
    side: f32,
}

impl ObbObbSweepContext {
    fn new(box1: &OBBox, move1: &Vector3, box2: &OBBox, move2: &Vector3) -> Self {
        Self {
            d: box2.center - box1.center,
            move_rel: *move1 - *move2,
            start_bad: true,
            max_frac: -0.01,
            test_axis: Vector3::ZERO,
            side: 1.0,
        }
    }

    fn check_axis(&mut self, mut axis: Vector3, box1: &OBBox, box2: &OBBox) -> bool {
        let len2 = axis.length_squared();
        if len2 <= crate::EPSILON2 {
            return false;
        }
        axis /= len2.sqrt();
        let mut dist = self.d.dot(axis);
        let mut axismove = self.move_rel.dot(axis);
        if dist < 0.0 {
            dist = -dist;
            axismove = -axismove;
            axis = -axis;
            self.side = -1.0;
        } else {
            self.side = 1.0;
        }
        self.test_axis = axis;
        let leb0 = project_obb_extent(box1, axis) + project_obb_extent(box2, axis);
        let leb1 = leb0 + axismove;
        let mut eps = 0.0;
        if dist - leb0 <= 0.0 {
            eps = COLLISION_EPSILON;
        }
        if dist - leb0 > -eps {
            self.start_bad = false;
            if leb1 - leb0 > 0.0 {
                let frac = (dist - leb0) / (leb1 - leb0);
                if frac >= 1.0 {
                    self.max_frac = 1.0;
                    return true;
                } else if frac > self.max_frac {
                    self.max_frac = frac;
                }
            } else {
                self.max_frac = 1.0;
                return true;
            }
        }
        false
    }
}

fn finalize_obb_obb(ctx: &ObbObbSweepContext, result: &mut CastResult) -> bool {
    let mut max_frac = ctx.max_frac;
    if max_frac < 0.0 {
        max_frac = 0.0;
    }
    if ctx.start_bad {
        result.start_bad = true;
        result.fraction = 0.0;
        return true;
    }
    if max_frac <= result.fraction && max_frac < 1.0 {
        result.fraction = max_frac;
        result.normal = -ctx.side * ctx.test_axis;
        return true;
    }
    false
}
