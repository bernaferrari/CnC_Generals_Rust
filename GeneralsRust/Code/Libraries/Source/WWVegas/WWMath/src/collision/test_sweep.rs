#[cfg(test)]
mod tests {
    use crate::{AABox, CastResult, CollisionMath, Triangle, Vec3};

    #[test]
    fn swept_aabox_triangle_face_hit() {
        let box_ref = AABox::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.5, 0.5, 0.5));
        let movement = Vec3::new(0.0, 0.0, 3.0);
        let tri = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let mut result = CastResult::new();
        result.compute_contact_point = true;
        let hit = CollisionMath::collide_aabox_triangle(&box_ref, &movement, &tri, &mut result);
        assert!(hit);
        assert!(result.fraction >= 0.0 && result.fraction <= 1.0);
        assert!(result.normal.z < -0.5);
    }

    #[test]
    fn swept_aabox_triangle_edge_hit() {
        let box_ref = AABox::new(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(0.5, 0.5, 0.5));
        let movement = Vec3::new(3.0, 0.0, 0.0);
        let tri = Triangle::new(
            Vec3::new(0.0, -1.0, -0.1),
            Vec3::new(0.0, 1.0, -0.1),
            Vec3::new(0.0, 0.0, 0.1),
        );
        let mut result = CastResult::new();
        let hit = CollisionMath::collide_aabox_triangle(&box_ref, &movement, &tri, &mut result);
        assert!(hit);
        assert!(result.fraction >= 0.0 && result.fraction <= 1.0);
    }

    #[test]
    fn swept_aabox_triangle_vertex_hit() {
        let box_ref = AABox::new(Vec3::new(-1.5, -1.5, -1.5), Vec3::new(0.25, 0.25, 0.25));
        let movement = Vec3::new(2.0, 2.0, 2.0);
        let tri = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let mut result = CastResult::new();
        let hit = CollisionMath::collide_aabox_triangle(&box_ref, &movement, &tri, &mut result);
        assert!(hit);
        assert!(result.fraction >= 0.0 && result.fraction <= 1.0);
    }

    // C++ colmathaabtri.cpp CollisionMath::Collide — StartBad when already intersecting
    #[test]
    fn swept_aabox_triangle_start_bad() {
        let box_ref = AABox::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let movement = Vec3::new(0.0, 0.0, 1.0);
        let tri = Triangle::new(
            Vec3::new(-0.5, -0.5, 0.0),
            Vec3::new(0.5, -0.5, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
        );
        let mut result = CastResult::new();
        let hit = CollisionMath::collide_aabox_triangle(&box_ref, &movement, &tri, &mut result);
        assert!(hit);
        assert!(result.start_bad);
        assert_eq!(result.fraction, 0.0);
    }

    // C++ colmathaabtri.cpp — separated box that never reaches the triangle
    #[test]
    fn swept_aabox_triangle_miss() {
        let box_ref = AABox::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.25, 0.25, 0.25));
        let movement = Vec3::new(0.0, 0.0, 1.0);
        let tri = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let mut result = CastResult::new();
        let hit = CollisionMath::collide_aabox_triangle(&box_ref, &movement, &tri, &mut result);
        assert!(!hit);
        assert!(!result.start_bad);
    }

    // C++ colmathline.cpp CollisionMath::Collide(LineSeg, AABox)
    #[test]
    fn collide_line_aabox_hits_face() {
        let line = crate::LineSegment::new(Vec3::new(-3.0, 0.0, 0.0), Vec3::new(3.0, 0.0, 0.0));
        let box_ref = AABox::new(Vec3::ZERO, Vec3::splat(1.0));
        let mut result = CastResult::new();
        assert!(CollisionMath::collide_line_aabox(
            &line,
            &box_ref,
            &mut result
        ));
        assert!(!result.start_bad);
        assert!(result.fraction > 0.0 && result.fraction < 1.0);
        assert!(result.normal.x < -0.5);
    }

    // C++ colmathline.cpp:283 Basis.Transpose() — 45° thin OBB is not its AABB
    #[test]
    fn collide_line_obbox_uses_orientation() {
        let angle = std::f32::consts::FRAC_PI_4;
        let (s, c) = angle.sin_cos();
        let basis = crate::Matrix3 {
            row: [
                Vec3::new(c, -s, 0.0),
                Vec3::new(s, c, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
        };
        let obbox =
            crate::OBBox::from_center_extent_basis(Vec3::ZERO, Vec3::new(2.0, 0.05, 0.05), basis);
        // Ray down -Z through (1.2, 1.2): hits rotated box, misses AABB of local extents
        let hit_line = crate::LineSegment::new(Vec3::new(1.2, 1.2, 2.0), Vec3::new(1.2, 1.2, -2.0));
        let mut hit = CastResult::new();
        assert!(CollisionMath::collide_line_obbox(
            &hit_line, &obbox, &mut hit
        ));

        let miss_line =
            crate::LineSegment::new(Vec3::new(1.5, 0.0, -2.0), Vec3::new(1.5, 0.0, 2.0));
        let mut miss = CastResult::new();
        assert!(!CollisionMath::collide_line_obbox(
            &miss_line, &obbox, &mut miss
        ));
    }

    // C++ colmathobbtri.cpp — continuous sweep hits a thin triangle start/end both miss
    #[test]
    fn collide_obbox_triangle_swept_hits_thin() {
        let obbox = crate::OBBox::from_center_extent(Vec3::new(0.0, 0.0, -2.0), Vec3::splat(0.25));
        let movement = Vec3::new(0.0, 0.0, 4.0);
        let tri = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let mut result = CastResult::new();
        assert!(CollisionMath::collide_obbox_triangle(
            &obbox,
            &movement,
            &tri,
            &Vec3::ZERO,
            &mut result
        ));
        assert!(!result.start_bad);
        assert!(result.fraction > 0.0 && result.fraction < 1.0);
    }
}
