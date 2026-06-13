//! W3D polygon clipping helpers.
//!
//! Mirrors C++ `W3DDevice/GameClient/W3DPoly.cpp`.

/// C++ `Vector3` shape used by the W3D polygon clipper.
pub type Vector3 = [f32; 3];

/// C++ `PlaneClass` subset required by `ClipPolyClass`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneClass {
    /// Plane normal.
    pub n: Vector3,
    /// Plane distance.
    pub d: f32,
}

impl PlaneClass {
    /// Create a plane from a normal and distance.
    pub const fn new(n: Vector3, d: f32) -> Self {
        Self { n, d }
    }

    /// Matches C++ `PlaneClass::In_Front`.
    pub fn in_front(&self, point: Vector3) -> bool {
        dot(point, self.n) > self.d
    }

    /// Matches C++ `PlaneClass::Compute_Intersection`.
    pub fn compute_intersection(&self, p0: Vector3, p1: Vector3) -> Option<f32> {
        let den = dot(self.n, sub(p1, p0));
        if den == 0.0 {
            return None;
        }

        let t = -(dot(self.n, p0) - self.d) / den;
        if !(0.0..=1.0).contains(&t) {
            return None;
        }

        Some(t)
    }
}

/// C++ `ClipPolyClass`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ClipPolyClass {
    /// Polygon vertices.
    pub verts: Vec<Vector3>,
}

impl ClipPolyClass {
    /// Create an empty clipping polygon.
    pub fn new() -> Self {
        Self::default()
    }

    /// Matches C++ `Reset`.
    pub fn reset(&mut self) {
        self.verts.clear();
    }

    /// Matches C++ `Add_Vertex`.
    pub fn add_vertex(&mut self, point: Vector3) {
        self.verts.push(point);
    }

    /// Matches C++ `Clip`.
    pub fn clip(&self, plane: &PlaneClass, dest: &mut ClipPolyClass) {
        dest.reset();

        let vcount = self.verts.len();
        if vcount <= 2 {
            return;
        }

        let mut i = 0usize;
        let mut iprev = vcount - 1;
        let mut prev_point_in_front = !plane.in_front(self.verts[iprev]);

        for _ in 0..vcount {
            let cur_point_in_front = !plane.in_front(self.verts[i]);
            if prev_point_in_front {
                if cur_point_in_front {
                    dest.add_vertex(self.verts[i]);
                } else if let Some(alpha) =
                    plane.compute_intersection(self.verts[iprev], self.verts[i])
                {
                    dest.add_vertex(lerp(self.verts[iprev], self.verts[i], alpha));
                }
            } else if cur_point_in_front {
                if let Some(alpha) = plane.compute_intersection(self.verts[iprev], self.verts[i]) {
                    dest.add_vertex(lerp(self.verts[iprev], self.verts[i], alpha));
                }
                dest.add_vertex(self.verts[i]);
            }

            prev_point_in_front = cur_point_in_front;
            iprev = i;
            i += 1;
            if i >= vcount {
                i = 0;
            }
        }
    }
}

fn dot(a: Vector3, b: Vector3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: Vector3, b: Vector3) -> Vector3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn lerp(a: Vector3, b: Vector3, alpha: f32) -> Vector3 {
    [
        a[0] + (b[0] - a[0]) * alpha,
        a[1] + (b[1] - a[1]) * alpha,
        a[2] + (b[2] - a[2]) * alpha,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn poly(points: &[Vector3]) -> ClipPolyClass {
        let mut out = ClipPolyClass::new();
        for point in points {
            out.add_vertex(*point);
        }
        out
    }

    fn assert_vec3_eq(actual: Vector3, expected: Vector3) {
        for axis in 0..3 {
            assert!(
                (actual[axis] - expected[axis]).abs() <= f32::EPSILON,
                "axis {axis}: actual={actual:?} expected={expected:?}"
            );
        }
    }

    #[test]
    fn reset_clears_without_shrinking_capacity() {
        let mut clip = ClipPolyClass::new();
        for x in 0..8 {
            clip.add_vertex([x as f32, 0.0, 0.0]);
        }
        let capacity = clip.verts.capacity();

        clip.reset();

        assert!(clip.verts.is_empty());
        assert_eq!(clip.verts.capacity(), capacity);
    }

    #[test]
    fn clip_with_two_or_fewer_vertices_returns_empty() {
        let source = poly(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]]);
        let plane = PlaneClass::new([1.0, 0.0, 0.0], 0.0);
        let mut dest = poly(&[[99.0, 99.0, 99.0]]);

        source.clip(&plane, &mut dest);

        assert!(dest.verts.is_empty());
    }

    #[test]
    fn all_inside_preserves_vertex_order() {
        let source = poly(&[
            [-1.0, -1.0, 0.0],
            [-0.5, -1.0, 0.0],
            [-0.5, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ]);
        let plane = PlaneClass::new([1.0, 0.0, 0.0], 0.0);
        let mut dest = ClipPolyClass::new();

        source.clip(&plane, &mut dest);

        assert_eq!(dest.verts, source.verts);
    }

    #[test]
    fn all_outside_returns_empty() {
        let source = poly(&[
            [1.0, -1.0, 0.0],
            [2.0, -1.0, 0.0],
            [2.0, 1.0, 0.0],
            [1.0, 1.0, 0.0],
        ]);
        let plane = PlaneClass::new([1.0, 0.0, 0.0], 0.0);
        let mut dest = ClipPolyClass::new();

        source.clip(&plane, &mut dest);

        assert!(dest.verts.is_empty());
    }

    #[test]
    fn clip_square_against_x_le_zero_emits_expected_vertices() {
        let source = poly(&[
            [-1.0, -1.0, 0.0],
            [1.0, -1.0, 0.0],
            [1.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0],
        ]);
        let plane = PlaneClass::new([1.0, 0.0, 0.0], 0.0);
        let mut dest = ClipPolyClass::new();

        source.clip(&plane, &mut dest);

        assert_eq!(dest.verts.len(), 4);
        assert_vec3_eq(dest.verts[0], [-1.0, -1.0, 0.0]);
        assert_vec3_eq(dest.verts[1], [0.0, -1.0, 0.0]);
        assert_vec3_eq(dest.verts[2], [0.0, 1.0, 0.0]);
        assert_vec3_eq(dest.verts[3], [-1.0, 1.0, 0.0]);
    }

    #[test]
    fn points_on_plane_are_inside() {
        let source = poly(&[[0.0, -1.0, 0.0], [0.0, 1.0, 0.0], [-1.0, 0.0, 0.0]]);
        let plane = PlaneClass::new([1.0, 0.0, 0.0], 0.0);
        let mut dest = ClipPolyClass::new();

        source.clip(&plane, &mut dest);

        assert_eq!(dest.verts, source.verts);
    }

    #[test]
    fn intersection_matches_cpp_t_and_lerp() {
        let plane = PlaneClass::new([1.0, 0.0, 0.0], 0.0);
        let p0 = [-2.0, 4.0, 6.0];
        let p1 = [2.0, 8.0, 10.0];

        let t = plane.compute_intersection(p0, p1).unwrap();
        let point = lerp(p0, p1, t);

        assert_eq!(t, 0.5);
        assert_vec3_eq(point, [0.0, 6.0, 8.0]);
    }

    #[test]
    fn six_plane_ping_pong_keeps_box_center_polygon() {
        let planes = [
            PlaneClass::new([1.0, 0.0, 0.0], 1.0),
            PlaneClass::new([-1.0, 0.0, 0.0], 1.0),
            PlaneClass::new([0.0, 1.0, 0.0], 1.0),
            PlaneClass::new([0.0, -1.0, 0.0], 1.0),
            PlaneClass::new([0.0, 0.0, 1.0], 1.0),
            PlaneClass::new([0.0, 0.0, -1.0], 1.0),
        ];
        let mut poly_a = poly(&[
            [-0.5, -0.5, 0.0],
            [0.5, -0.5, 0.0],
            [0.5, 0.5, 0.0],
            [-0.5, 0.5, 0.0],
        ]);
        let mut poly_b = ClipPolyClass::new();

        for (idx, plane) in planes.iter().enumerate() {
            if idx % 2 == 0 {
                poly_a.clip(plane, &mut poly_b);
            } else {
                poly_b.clip(plane, &mut poly_a);
            }
        }

        assert_eq!(poly_a.verts.len(), 4);
        assert_vec3_eq(poly_a.verts[0], [-0.5, -0.5, 0.0]);
        assert_vec3_eq(poly_a.verts[1], [0.5, -0.5, 0.0]);
        assert_vec3_eq(poly_a.verts[2], [0.5, 0.5, 0.0]);
        assert_vec3_eq(poly_a.verts[3], [-0.5, 0.5, 0.0]);
    }
}
