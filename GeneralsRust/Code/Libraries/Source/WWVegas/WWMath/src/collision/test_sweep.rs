#[cfg(test)]
mod tests {
    use crate::{AABox, Triangle, Vec3, CastResult, CollisionMath};

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
}

