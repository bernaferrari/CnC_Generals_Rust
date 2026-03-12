#[cfg(test)]
mod tests {
    use crate::{EulerAngles, EulerOrder, Vec3, NormalCone, AABox, Triangle, CollisionMath, CastResult};

    #[test]
    fn parity_euler_xyzr_roundtrip_specific() {
        // Known angles (in radians)
        let a0 = 30f64.to_radians();
        let a1 = 10f64.to_radians();
        let a2 = (-20f64).to_radians();

        // Build matrix via EulerAngles->to_matrix (wwmath implementation)
        let e = EulerAngles { angles: [a0, a1, a2], order: EulerOrder::XYZr };
        let m = e.to_matrix();

        // Recover angles and compare
        let e2 = EulerAngles::from_matrix(&m, EulerOrder::XYZr);
        let eps = 1e-4;
        assert!((e2.get_angle(0) - a0).abs() < eps);
        assert!((e2.get_angle(1) - a1).abs() < eps);
        assert!((e2.get_angle(2) - a2).abs() < eps);
    }

    #[test]
    fn parity_euler_repeated_axes_and_gimbal_lock() {
        // Repeated axes order (ZXZs) round-trip
        let a0 = 10f64.to_radians();
        let a1 = 45f64.to_radians();
        let a2 = 20f64.to_radians();
        let e = EulerAngles { angles: [a0, a1, a2], order: EulerOrder::ZXZs };
        let m = e.to_matrix();
        let e2 = EulerAngles::from_matrix(&m, EulerOrder::ZXZs);
        let eps = 1e-4;
        assert!((e2.get_angle(0) - a0).abs() < eps);
        assert!((e2.get_angle(1) - a1).abs() < eps);
        assert!((e2.get_angle(2) - a2).abs() < eps);

        // Gimbal lock case for XYZr: pitch = 90 degrees
        let e_gl = EulerAngles { angles: [0.3, std::f64::consts::FRAC_PI_2, -0.7], order: EulerOrder::XYZr };
        let m_gl = e_gl.to_matrix();
        let e_gl2 = EulerAngles::from_matrix(&m_gl, EulerOrder::XYZr);
        // At gimbal lock, first/third may shift, but middle angle should be near 90°
        assert!((e_gl2.get_angle(1) - std::f64::consts::FRAC_PI_2).abs() < 1e-4);
    }

    #[test]
    fn parity_euler_zyxr_and_xyxr_roundtrip() {
        // ZYXr (rotating axes, no repeat)
        let a0 = (-15f64).to_radians();
        let a1 = (25f64).to_radians();
        let a2 = (70f64).to_radians();
        let e_zyxr = EulerAngles { angles: [a0, a1, a2], order: EulerOrder::ZYXr };
        let m_zyxr = e_zyxr.to_matrix();
        let e_zyxr_back = EulerAngles::from_matrix(&m_zyxr, EulerOrder::ZYXr);
        let eps = 1e-4;
        assert!((e_zyxr_back.get_angle(0) - a0).abs() < eps);
        assert!((e_zyxr_back.get_angle(1) - a1).abs() < eps);
        assert!((e_zyxr_back.get_angle(2) - a2).abs() < eps);

        // XYXr (rotating axes, repeated)
        let b0 = (5f64).to_radians();
        let b1 = (35f64).to_radians();
        let b2 = (-12f64).to_radians();
        let e_xyxr = EulerAngles { angles: [b0, b1, b2], order: EulerOrder::XYXr };
        let m_xyxr = e_xyxr.to_matrix();
        let e_xyxr_back = EulerAngles::from_matrix(&m_xyxr, EulerOrder::XYXr);
        assert!((e_xyxr_back.get_angle(0) - b0).abs() < eps);
        assert!((e_xyxr_back.get_angle(1) - b1).abs() < eps);
        assert!((e_xyxr_back.get_angle(2) - b2).abs() < eps);
    }

    #[test]
    fn parity_normal_cone_opposites() {
        let mut cone = NormalCone::from_direction(Vec3::new(0.0, 0.0, 1.0));
        cone.merge_normal(&Vec3::new(0.0, 0.0, -1.0));
        assert!(cone.is_complete_sphere());
    }

    #[test]
    fn parity_swept_aabb_triangle_hit() {
        // Simple face hit towards z=0 plane
        let box_ref = AABox::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.5, 0.5, 0.5));
        let movement = Vec3::new(0.0, 0.0, 3.0);
        let tri = Triangle::new(
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let mut result = CastResult::new();
        let hit = CollisionMath::collide_aabox_triangle(&box_ref, &movement, &tri, &mut result);
        assert!(hit);
        assert!(result.fraction > 0.0 && result.fraction <= 1.0);
        // Normal should oppose motion (roughly -Z)
        assert!(result.normal.z < -0.5);
        // Expected first contact when top face reaches z=0: t = 1.5/3 = 0.5
        assert!((result.fraction - 0.5).abs() < 1e-4);
    }

    #[test]
    fn parity_swept_aabb_triangle_edge_vertex_hits() {
        // Edge hit: move along +x towards a triangle whose edge lies near x=0
        let box_edge = AABox::new(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(0.4, 0.4, 0.4));
        let move_edge = Vec3::new(3.0, 0.0, 0.0);
        // Triangle with an edge roughly along y at x≈0, z crosses small range
        let tri_edge = Triangle::new(
            Vec3::new(0.0, -1.0, -0.2),
            Vec3::new(0.0, 1.0, -0.2),
            Vec3::new(0.0, 0.0, 0.2),
        );
        let mut res_edge = CastResult::new();
        let hit_edge = CollisionMath::collide_aabox_triangle(&box_edge, &move_edge, &tri_edge, &mut res_edge);
        assert!(hit_edge);
        assert!(res_edge.fraction >= 0.0 && res_edge.fraction <= 1.0);
        // Normal should oppose movement
        assert!(res_edge.normal.dot(move_edge) <= 0.0);

        // Vertex hit: small box moves towards a single vertex at origin
        let box_vert = AABox::new(Vec3::new(-1.5, -1.5, -1.5), Vec3::new(0.2, 0.2, 0.2));
        let move_vert = Vec3::new(2.0, 2.0, 2.0);
        let tri_vert = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        let mut res_vert = CastResult::new();
        let hit_vert = CollisionMath::collide_aabox_triangle(&box_vert, &move_vert, &tri_vert, &mut res_vert);
        assert!(hit_vert);
        assert!(res_vert.fraction >= 0.0 && res_vert.fraction <= 1.0);
        assert!(res_vert.normal.dot(move_vert) <= 0.0);
    }

    #[test]
    fn parity_normal_cone_merge_two_cones() {
        // Start with degenerate cones (angle = 1.0) along Z and X axes
        let mut cone = NormalCone::from_direction(Vec3::new(0.0, 0.0, 1.0));
        let other = NormalCone::from_direction(Vec3::new(1.0, 0.0, 0.0));
        // Merge the other cone's coplanar normals into the first
        cone.merge_cone(&other);
        // Expected: cone widens (angle decreases) and center tilts toward (1,0,1).normalize()
        let target = (Vec3::new(1.0, 0.0, 1.0)).normalize();
        // Angle should be hemisphere or wider (<= 0)
        assert!(cone.angle <= 0.0 + 1e-4);
        // Center within a reasonable angular distance (~15 degrees) of the target
        let dot = cone.center.dot(target);
        assert!(dot > (15f32.to_radians()).cos());
    }
}
