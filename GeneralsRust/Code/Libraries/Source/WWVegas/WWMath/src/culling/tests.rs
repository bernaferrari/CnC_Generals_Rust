//! Comprehensive tests for the culling system implementations

#[cfg(test)]
use crate::{AABox, Cullable, Frustum, Matrix3D, Sphere, Vector2, Vector3};
#[cfg(test)]
use std::sync::Arc;

#[cfg(test)]
#[derive(Debug)]
pub struct TestCullable {
    id: u64,
    bbox: AABox,
}

#[cfg(test)]
impl TestCullable {
    pub fn new(id: u64, center: Vector3, extent: Vector3) -> Self {
        Self {
            id,
            bbox: AABox::new(center, extent),
        }
    }
}

#[cfg(test)]
impl Cullable for TestCullable {
    fn get_cull_box(&self) -> AABox {
        self.bbox
    }

    fn set_cull_box(&mut self, box_: AABox, _just_loaded: bool) {
        self.bbox = box_;
    }

    fn get_id(&self) -> u64 {
        self.id
    }
}

#[cfg(test)]
mod tests {
    use super::TestCullable;
    use crate::culling::{
        CollisionMath, CullCollection, CullStats, CullType, Cullable, GridCullSystem, OverlapType,
    };
    use crate::{AABox, Frustum, Matrix3D, Sphere, Vector2, Vector3};
    use std::sync::Arc;

    #[test]
    fn test_cull_type_values() {
        assert_eq!(CullType::Outside as u8, 0);
        assert_eq!(CullType::Intersecting as u8, 1);
        assert_eq!(CullType::Inside as u8, 2);
    }

    #[test]
    fn test_cull_stats_default() {
        let stats = CullStats::new();
        assert_eq!(stats.node_count, 0);
        assert_eq!(stats.nodes_accepted, 0);
        assert_eq!(stats.nodes_trivially_accepted, 0);
        assert_eq!(stats.nodes_rejected, 0);
    }

    #[test]
    fn test_cull_stats_reset() {
        let mut stats = CullStats::new();
        stats.node_count = 100;
        stats.nodes_accepted = 50;
        stats.nodes_trivially_accepted = 25;
        stats.nodes_rejected = 25;

        stats.reset();

        assert_eq!(stats.node_count, 100); // node_count should not reset
        assert_eq!(stats.nodes_accepted, 0);
        assert_eq!(stats.nodes_trivially_accepted, 0);
        assert_eq!(stats.nodes_rejected, 0);
    }

    #[test]
    fn test_cull_collection_basic() {
        let mut collection = CullCollection::new();
        assert!(collection.is_empty());
        assert_eq!(collection.len(), 0);

        let obj1 = Arc::new(TestCullable::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        let obj2 = Arc::new(TestCullable::new(
            2,
            Vector3::new(2.0, 2.0, 2.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        collection.add(obj1);
        collection.add(obj2);

        assert!(!collection.is_empty());
        assert_eq!(collection.len(), 2);

        // Test iteration
        let first = collection.first().unwrap();
        assert_eq!(first.get_id(), 1);

        let second = collection.next().unwrap();
        assert_eq!(second.get_id(), 2);

        assert!(collection.next().is_none());
    }

    #[test]
    fn test_cull_collection_peek() {
        let mut collection = CullCollection::new();
        let obj = Arc::new(TestCullable::new(
            42,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        collection.add(obj);

        let peeked = collection.peek_first().unwrap();
        assert_eq!(peeked.get_id(), 42);

        // Peeking should not affect iteration
        let first = collection.first().unwrap();
        assert_eq!(first.get_id(), 42);
    }

    #[test]
    fn test_cull_collection_clear() {
        let mut collection = CullCollection::new();
        let obj = Arc::new(TestCullable::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        collection.add(obj);

        assert_eq!(collection.len(), 1);
        collection.clear();
        assert!(collection.is_empty());
        assert_eq!(collection.len(), 0);
    }

    #[test]
    fn test_collision_math_box_point() {
        let box_ = AABox::new(Vector3::ZERO, Vector3::new(2.0, 2.0, 2.0));

        // Point inside
        let inside_point = Vector3::new(1.0, 1.0, 1.0);
        let result = CollisionMath::overlap_test_box_point(&box_, inside_point);
        assert_ne!(result, OverlapType::Outside);

        // Point outside
        let outside_point = Vector3::new(5.0, 5.0, 5.0);
        let result = CollisionMath::overlap_test_box_point(&box_, outside_point);
        assert_eq!(result, OverlapType::Outside);

        // Point on boundary
        let boundary_point = Vector3::new(2.0, 2.0, 2.0);
        let result = CollisionMath::overlap_test_box_point(&box_, boundary_point);
        assert_ne!(result, OverlapType::Outside);
    }

    #[test]
    fn test_collision_math_box_box() {
        let box1 = AABox::new(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));
        let box2_inside = AABox::new(Vector3::new(0.5, 0.5, 0.5), Vector3::new(0.25, 0.25, 0.25));
        let box3_intersecting =
            AABox::new(Vector3::new(1.5, 1.5, 1.5), Vector3::new(1.0, 1.0, 1.0));
        let box4_outside = AABox::new(Vector3::new(5.0, 5.0, 5.0), Vector3::new(1.0, 1.0, 1.0));

        assert_ne!(
            CollisionMath::overlap_test_box_box(&box1, &box2_inside),
            OverlapType::Outside
        );
        assert_ne!(
            CollisionMath::overlap_test_box_box(&box1, &box3_intersecting),
            OverlapType::Outside
        );
        assert_eq!(
            CollisionMath::overlap_test_box_box(&box1, &box4_outside),
            OverlapType::Outside
        );
    }

    #[test]
    fn test_collision_math_box_sphere() {
        let box_ = AABox::new(Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));
        let sphere_intersecting = Sphere::new(Vector3::new(0.0, 0.0, 0.0), 0.5);
        let sphere_outside = Sphere::new(Vector3::new(5.0, 5.0, 5.0), 1.0);

        assert_ne!(
            CollisionMath::overlap_test_box_sphere(&box_, &sphere_intersecting),
            OverlapType::Outside
        );
        assert_eq!(
            CollisionMath::overlap_test_box_sphere(&box_, &sphere_outside),
            OverlapType::Outside
        );
    }

    #[test]
    fn test_collision_math_frustum_box() {
        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);

        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        let box_inside = AABox::new(Vector3::new(0.0, 0.0, -5.0), Vector3::new(0.1, 0.1, 0.1));
        let box_outside = AABox::new(
            Vector3::new(100.0, 100.0, -5.0),
            Vector3::new(1.0, 1.0, 1.0),
        );

        assert_ne!(
            CollisionMath::overlap_test_frustum_box(&frustum, &box_inside),
            OverlapType::Outside
        );
        assert_eq!(
            CollisionMath::overlap_test_frustum_box(&frustum, &box_outside),
            OverlapType::Outside
        );
    }

    #[test]
    fn test_grid_cull_system_creation() {
        let grid = GridCullSystem::new();
        assert_eq!(grid.get_object_count(), 0);

        let stats = grid.get_stats();
        assert!(stats.node_count > 0);
    }

    #[test]
    fn test_grid_cull_system_add_remove() {
        let mut grid = GridCullSystem::new();

        let obj1 = Arc::new(TestCullable::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        let obj2 = Arc::new(TestCullable::new(
            2,
            Vector3::new(10.0, 10.0, 10.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        grid.add_object(obj1.clone());
        assert_eq!(grid.get_object_count(), 1);

        grid.add_object(obj2.clone());
        assert_eq!(grid.get_object_count(), 2);

        grid.remove_object(&obj1);
        assert_eq!(grid.get_object_count(), 1);

        grid.remove_object(&obj2);
        assert_eq!(grid.get_object_count(), 0);
    }

    #[test]
    fn test_grid_cull_system_point_collection() {
        let mut grid = GridCullSystem::new();

        let obj_at_origin = Arc::new(TestCullable::new(
            1,
            Vector3::ZERO,
            Vector3::new(2.0, 2.0, 2.0),
        ));
        let obj_far_away = Arc::new(TestCullable::new(
            2,
            Vector3::new(50.0, 50.0, 50.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        grid.add_object(obj_at_origin);
        grid.add_object(obj_far_away);

        grid.reset_collection();
        grid.collect_objects_point(Vector3::new(1.0, 1.0, 1.0));

        let collection = grid.get_collection();
        assert!(collection.len() >= 1);

        if let Some(found_obj) = collection.peek_first() {
            assert_eq!(found_obj.get_id(), 1);
        }
    }

    #[test]
    fn test_grid_cull_system_box_collection() {
        let mut grid = GridCullSystem::new();

        let obj = Arc::new(TestCullable::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        grid.add_object(obj);

        let query_box = AABox::new(Vector3::new(0.5, 0.5, 0.5), Vector3::new(2.0, 2.0, 2.0));

        grid.reset_collection();
        grid.collect_objects_box(&query_box);

        let collection = grid.get_collection();
        assert!(collection.len() >= 1);
    }

    #[test]
    fn test_grid_cull_system_frustum_collection() {
        let mut grid = GridCullSystem::new();

        let obj_in_view = Arc::new(TestCullable::new(
            1,
            Vector3::new(0.0, 0.0, -5.0),
            Vector3::new(0.5, 0.5, 0.5),
        ));
        let obj_out_of_view = Arc::new(TestCullable::new(
            2,
            Vector3::new(100.0, 100.0, -5.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        grid.add_object(obj_in_view);
        grid.add_object(obj_out_of_view);

        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);
        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        grid.reset_collection();
        grid.collect_objects_frustum(&frustum);

        let collection = grid.get_collection();
        assert!(collection.len() >= 1);
    }

    #[test]
    fn test_grid_cull_system_update_culling() {
        let mut grid = GridCullSystem::new();

        let obj = Arc::new(TestCullable::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        grid.add_object(obj.clone());
        assert_eq!(grid.get_object_count(), 1);

        // Update object position (simulate movement)
        grid.update_culling(&obj);

        // Object should still be in the system
        assert_eq!(grid.get_object_count(), 1);
    }

    #[test]
    fn test_grid_cull_system_repartition() {
        let mut grid = GridCullSystem::new();

        // Add some objects first
        for i in 0..5 {
            let obj = Arc::new(TestCullable::new(
                i as u64,
                Vector3::new(i as f32 * 5.0, 0.0, 0.0),
                Vector3::new(1.0, 1.0, 1.0),
            ));
            grid.add_object(obj);
        }

        assert_eq!(grid.get_object_count(), 5);

        // Re-partition with different bounds
        grid.re_partition(
            Vector3::new(-50.0, -50.0, -50.0),
            Vector3::new(50.0, 50.0, 50.0),
            10.0,
        );

        // Objects should still be in the system
        assert_eq!(grid.get_object_count(), 5);
    }

    #[test]
    fn test_grid_cull_system_statistics() {
        let mut grid = GridCullSystem::new();
        let stats = grid.get_stats();

        assert!(stats.node_count > 0);
        assert_eq!(stats.nodes_accepted, 0);

        // Add object and perform collection to generate statistics
        let obj = Arc::new(TestCullable::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        grid.add_object(obj);

        grid.reset_collection();
        grid.collect_objects_point(Vector3::ZERO);

        let new_stats = grid.get_stats();
        assert!(new_stats.nodes_trivially_accepted > 0);
    }

    #[test]
    fn test_grid_cull_system_cell_size_settings() {
        let mut grid = GridCullSystem::new();

        let original_size = grid.get_min_cell_size();
        assert_eq!(original_size, Vector3::new(10.0, 10.0, 10.0));

        let new_size = Vector3::new(5.0, 5.0, 5.0);
        grid.set_min_cell_size(new_size);
        assert_eq!(grid.get_min_cell_size(), new_size);

        let original_count = grid.get_termination_count();
        assert!(original_count > 0);

        grid.set_termination_count(8192);
        assert_eq!(grid.get_termination_count(), 8192);
    }

    #[test]
    fn test_cullable_trait() {
        let mut obj = TestCullable::new(42, Vector3::ZERO, Vector3::new(1.0, 1.0, 1.0));

        assert_eq!(obj.get_id(), 42);
        assert_eq!(obj.get_cull_box().center, Vector3::ZERO);
        assert_eq!(obj.get_cull_box().extent, Vector3::new(1.0, 1.0, 1.0));

        let new_box = AABox::new(Vector3::new(5.0, 5.0, 5.0), Vector3::new(2.0, 2.0, 2.0));
        obj.set_cull_box(new_box, false);
        assert_eq!(obj.get_cull_box().center, Vector3::new(5.0, 5.0, 5.0));
        assert_eq!(obj.get_cull_box().extent, Vector3::new(2.0, 2.0, 2.0));
    }
}
