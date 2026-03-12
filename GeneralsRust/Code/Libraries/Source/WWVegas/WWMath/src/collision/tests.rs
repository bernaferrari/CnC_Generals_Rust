/*
 * Collision Detection Tests
 *
 * Comprehensive test suite for all collision detection functionality
 */

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;

    // ========================================================================================
    // Test Helpers
    // ========================================================================================

    fn create_unit_aabox() -> AABox {
        AABox::new(Vector3::ZERO, Vector3::ONE)
    }

    fn create_unit_sphere() -> Sphere {
        Sphere::new(Vector3::ZERO, 1.0)
    }

    fn create_unit_triangle() -> Triangle {
        Triangle::new(
            Vector3::new(-1.0, -1.0, 0.0),
            Vector3::new(1.0, -1.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        )
    }

    fn create_xy_plane() -> Plane {
        Plane::new(Vector3::new(0.0, 0.0, 1.0), 0.0)
    }

    fn create_z_aaplane() -> AAPlane {
        AAPlane::new(AxisEnum::ZNormal, 0.0)
    }

    // ========================================================================================
    // Basic Type Tests
    // ========================================================================================

    #[test]
    fn test_cast_result_default() {
        let result = CastResult::default();
        assert!(!result.start_bad);
        assert_eq!(result.fraction, 1.0);
        assert_eq!(result.normal, Vector3::ZERO);
        assert_eq!(result.surface_type, 0);
        assert!(!result.compute_contact_point);
        assert_eq!(result.contact_point, Vector3::ZERO);
    }

    #[test]
    fn test_overlap_type_constants() {
        assert_eq!(OverlapType::OUTSIDE, OverlapType::Positive);
        assert_eq!(OverlapType::INSIDE, OverlapType::Negative);
        assert_eq!(OverlapType::OVERLAPPED, OverlapType::Both);
    }

    // ========================================================================================
    // Intersection Tests
    // ========================================================================================

    #[test]
    fn test_aabox_aabox_intersection() {
        let box1 = AABox::new(Vector3::ZERO, Vector3::ONE);
        let box2 = AABox::new(Vector3::new(0.5, 0.5, 0.5), Vector3::ONE);
        let box3 = AABox::new(Vector3::new(3.0, 0.0, 0.0), Vector3::ONE);

        assert!(CollisionMath::intersection_test_aabox_aabox(&box1, &box2));
        assert!(!CollisionMath::intersection_test_aabox_aabox(&box1, &box3));
    }

    #[test]
    fn test_sphere_aabox_intersection() {
        let sphere = Sphere::new(Vector3::ZERO, 1.5);
        let box1 = AABox::new(Vector3::ZERO, Vector3::ONE);
        let box2 = AABox::new(Vector3::new(3.0, 0.0, 0.0), Vector3::ONE);

        assert!(CollisionMath::intersection_test_sphere_aabox(
            &sphere, &box1
        ));
        assert!(!CollisionMath::intersection_test_sphere_aabox(
            &sphere, &box2
        ));
    }

    #[test]
    fn test_sphere_sphere_intersection() {
        let sphere1 = Sphere::new(Vector3::ZERO, 1.0);
        let sphere2 = Sphere::new(Vector3::new(1.5, 0.0, 0.0), 1.0);
        let sphere3 = Sphere::new(Vector3::new(3.0, 0.0, 0.0), 1.0);

        assert!(CollisionMath::intersection_test_sphere_sphere(
            &sphere1, &sphere2
        ));
        assert!(!CollisionMath::intersection_test_sphere_sphere(
            &sphere1, &sphere3
        ));
    }

    #[test]
    fn test_aabox_triangle_intersection() {
        let box_ref = AABox::new(Vector3::ZERO, Vector3::ONE);
        let tri1 = Triangle::new(
            Vector3::new(-0.5, -0.5, -0.5),
            Vector3::new(0.5, -0.5, -0.5),
            Vector3::new(0.0, 0.5, 0.5),
        );
        let tri2 = Triangle::new(
            Vector3::new(2.0, 2.0, 2.0),
            Vector3::new(3.0, 2.0, 2.0),
            Vector3::new(2.5, 3.0, 2.0),
        );

        assert!(CollisionMath::intersection_test_aabox_triangle(
            &box_ref, &tri1
        ));
        assert!(!CollisionMath::intersection_test_aabox_triangle(
            &box_ref, &tri2
        ));
    }

    // ========================================================================================
    // Overlap Tests
    // ========================================================================================

    #[test]
    fn test_aaplane_point_overlap() {
        let plane = AAPlane::new(AxisEnum::ZNormal, 0.0);
        let point1 = Vector3::new(0.0, 0.0, 1.0);
        let point2 = Vector3::new(0.0, 0.0, -1.0);
        let point3 = Vector3::new(0.0, 0.0, 0.0);

        assert_eq!(
            CollisionMath::overlap_test_aaplane_point(&plane, &point1),
            OverlapType::Positive
        );
        assert_eq!(
            CollisionMath::overlap_test_aaplane_point(&plane, &point2),
            OverlapType::Negative
        );
        assert_eq!(
            CollisionMath::overlap_test_aaplane_point(&plane, &point3),
            OverlapType::On
        );
    }

    #[test]
    fn test_plane_point_overlap() {
        let plane = Plane::new(Vector3::new(0.0, 0.0, 1.0), 0.0);
        let point1 = Vector3::new(0.0, 0.0, 1.0);
        let point2 = Vector3::new(0.0, 0.0, -1.0);
        let point3 = Vector3::new(0.0, 0.0, 0.0);

        assert_eq!(
            CollisionMath::overlap_test_plane_point(&plane, &point1),
            OverlapType::Positive
        );
        assert_eq!(
            CollisionMath::overlap_test_plane_point(&plane, &point2),
            OverlapType::Negative
        );
        assert_eq!(
            CollisionMath::overlap_test_plane_point(&plane, &point3),
            OverlapType::On
        );
    }

    #[test]
    fn test_sphere_point_overlap() {
        let sphere = Sphere::new(Vector3::ZERO, 1.0);
        let point1 = Vector3::new(2.0, 0.0, 0.0);
        let point2 = Vector3::new(0.5, 0.0, 0.0);
        let point3 = Vector3::new(1.0, 0.0, 0.0);

        assert_eq!(
            CollisionMath::overlap_test_sphere_point(&sphere, &point1),
            OverlapType::Positive
        );
        assert_eq!(
            CollisionMath::overlap_test_sphere_point(&sphere, &point2),
            OverlapType::Negative
        );
        assert_eq!(
            CollisionMath::overlap_test_sphere_point(&sphere, &point3),
            OverlapType::On
        );
    }

    #[test]
    fn test_aabox_point_overlap() {
        let box_ref = AABox::new(Vector3::ZERO, Vector3::ONE);
        let point1 = Vector3::new(2.0, 0.0, 0.0);
        let point2 = Vector3::new(0.5, 0.0, 0.0);

        assert_eq!(
            CollisionMath::overlap_test_aabox_point(&box_ref, &point1),
            OverlapType::Positive
        );
        assert_eq!(
            CollisionMath::overlap_test_aabox_point(&box_ref, &point2),
            OverlapType::Negative
        );
    }

    #[test]
    fn test_aabox_aabox_overlap() {
        let box1 = AABox::new(Vector3::ZERO, Vector3::ONE);
        let box2 = AABox::new(Vector3::new(0.5, 0.0, 0.0), Vector3::new(0.5, 0.5, 0.5));
        let box3 = AABox::new(Vector3::new(3.0, 0.0, 0.0), Vector3::ONE);

        assert_eq!(
            CollisionMath::overlap_test_aabox_aabox(&box1, &box2),
            OverlapType::Both
        );
        assert_eq!(
            CollisionMath::overlap_test_aabox_aabox(&box1, &box3),
            OverlapType::Positive
        );
    }

    #[test]
    fn test_sphere_sphere_overlap() {
        let sphere1 = Sphere::new(Vector3::ZERO, 1.0);
        let sphere2 = Sphere::new(Vector3::new(1.5, 0.0, 0.0), 1.0);
        let sphere3 = Sphere::new(Vector3::new(3.0, 0.0, 0.0), 1.0);

        assert_eq!(
            CollisionMath::overlap_test_sphere_sphere(&sphere1, &sphere2),
            OverlapType::Negative
        );
        assert_eq!(
            CollisionMath::overlap_test_sphere_sphere(&sphere1, &sphere3),
            OverlapType::Positive
        );
    }

    // ========================================================================================
    // Line Segment Collision Tests
    // ========================================================================================

    #[test]
    fn test_line_plane_collision() {
        let line = LineSegment::new(Vector3::new(0.0, 0.0, -1.0), Vector3::new(0.0, 0.0, 1.0));
        let plane = Plane::new(Vector3::new(0.0, 0.0, 1.0), 0.0);
        let mut result = CastResult::new();
        result.compute_contact_point = true;

        assert!(CollisionMath::collide_line_plane(
            &line,
            &plane,
            &mut result
        ));
        assert_eq!(result.fraction, 0.5);
        assert_eq!(result.contact_point, Vector3::ZERO);
    }

    #[test]
    fn test_line_sphere_collision() {
        let line = LineSegment::new(Vector3::new(-2.0, 0.0, 0.0), Vector3::new(2.0, 0.0, 0.0));
        let sphere = Sphere::new(Vector3::ZERO, 1.0);
        let mut result = CastResult::new();

        assert!(CollisionMath::collide_line_sphere(
            &line,
            &sphere,
            &mut result
        ));
        assert!(result.fraction >= 0.0 && result.fraction <= 1.0);
    }

    // ========================================================================================
    // AABox Collision Tests
    // ========================================================================================

    #[test]
    fn test_aabox_plane_collision() {
        let box_ref = AABox::new(Vector3::new(0.0, 0.0, 2.0), Vector3::ONE);
        let movement = Vector3::new(0.0, 0.0, -3.0);
        let plane = Plane::new(Vector3::new(0.0, 0.0, 1.0), 0.0);
        let mut result = CastResult::new();

        assert!(CollisionMath::collide_aabox_plane(
            &box_ref,
            &movement,
            &plane,
            &mut result
        ));
        assert!(result.fraction > 0.0 && result.fraction < 1.0);
    }

    #[test]
    fn test_aabox_aabox_collision() {
        let box1 = AABox::new(Vector3::new(-2.0, 0.0, 0.0), Vector3::ONE);
        let move1 = Vector3::new(3.0, 0.0, 0.0);
        let box2 = AABox::new(Vector3::ZERO, Vector3::ONE);
        let move2 = Vector3::ZERO;
        let mut result = CastResult::new();

        assert!(CollisionMath::collide_aabox_aabox(
            &box1,
            &move1,
            &box2,
            &move2,
            &mut result
        ));
        assert!(result.fraction >= 0.0 && result.fraction < 1.0);
    }

    // ========================================================================================
    // Performance Tests
    // ========================================================================================

    #[test]
    fn test_collision_performance() {
        use std::time::Instant;

        let box1 = AABox::new(Vector3::ZERO, Vector3::ONE);
        let box2 = AABox::new(Vector3::new(0.5, 0.0, 0.0), Vector3::ONE);

        let start = Instant::now();
        for _ in 0..10000 {
            CollisionMath::intersection_test_aabox_aabox(&box1, &box2);
        }
        let duration = start.elapsed();

        println!("10000 AABox intersection tests took: {:?}", duration);
        assert!(duration.as_millis() < 100); // Should be very fast
    }

    // ========================================================================================
    // Edge Case Tests
    // ========================================================================================

    #[test]
    fn test_zero_sized_objects() {
        let zero_box = AABox::new(Vector3::ZERO, Vector3::ZERO);
        let zero_sphere = Sphere::new(Vector3::ZERO, 0.0);
        let normal_box = AABox::new(Vector3::ZERO, Vector3::ONE);

        // Zero-sized objects should behave predictably
        assert!(CollisionMath::intersection_test_aabox_aabox(
            &zero_box,
            &normal_box
        ));
        assert!(!CollisionMath::intersection_test_sphere_aabox(
            &zero_sphere,
            &normal_box
        ));
    }

    #[test]
    fn test_degenerate_triangles() {
        // Degenerate triangle (all points collinear)
        let tri = Triangle::new(
            Vector3::ZERO,
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
        );
        let box_ref = AABox::new(Vector3::ZERO, Vector3::ONE);

        // Should handle gracefully without crashing
        let result = CollisionMath::intersection_test_aabox_triangle(&box_ref, &tri);
        // Result can be either true or false, but shouldn't panic
        let _ = result;
    }

    #[test]
    fn test_very_small_epsilon_values() {
        let box1 = AABox::new(Vector3::ZERO, Vector3::ONE);
        let box2 = AABox::new(
            Vector3::new(2.0 + WWMath::EPSILON * 0.1, 0.0, 0.0),
            Vector3::ONE,
        );

        // Should handle near-epsilon separations correctly
        assert!(!CollisionMath::intersection_test_aabox_aabox(&box1, &box2));
    }

    // ========================================================================================
    // Documentation Tests (Example Usage)
    // ========================================================================================

    #[test]
    fn test_example_usage() {
        // Create two boxes
        let player_box = AABox::new(Vector3::new(0.0, 0.0, 0.0), Vector3::new(0.5, 1.0, 0.5));
        let wall_box = AABox::new(Vector3::new(2.0, 0.0, 0.0), Vector3::new(0.1, 2.0, 5.0));

        // Test if player is moving towards wall
        let movement = Vector3::new(3.0, 0.0, 0.0);
        let mut collision_result = CastResult::new();
        collision_result.compute_contact_point = true;

        if CollisionMath::collide_aabox_aabox(
            &player_box,
            &movement,
            &wall_box,
            &Vector3::ZERO,
            &mut collision_result,
        ) {
            println!(
                "Collision detected at fraction: {}",
                collision_result.fraction
            );
            println!("Contact normal: {:?}", collision_result.normal);

            // Move player to collision point
            let safe_movement = movement * collision_result.fraction;
            let final_position = player_box.center + safe_movement;
            println!("Player stops at: {:?}", final_position);
        }
    }
}
