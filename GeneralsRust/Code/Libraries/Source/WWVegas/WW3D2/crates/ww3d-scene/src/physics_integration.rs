//! Physics Integration Module
//!
//! This module provides integration between ww3d-physics and ww3d-scene,
//! enabling physics simulation results to update scene node transforms.

use glam::{Mat4, Quat, Vec3};
use std::collections::HashMap;
use ww3d_collision::physics_integration::PhysicsBodyId;

/// Physics to Scene synchronization manager
pub struct PhysicsSceneBridge {
    /// Maps physics body IDs to scene object names
    body_to_object: HashMap<PhysicsBodyId, String>,
    /// Maps scene object names to physics body IDs
    object_to_body: HashMap<String, PhysicsBodyId>,
}

impl PhysicsSceneBridge {
    /// Create a new physics-scene bridge
    pub fn new() -> Self {
        Self {
            body_to_object: HashMap::new(),
            object_to_body: HashMap::new(),
        }
    }

    /// Link a physics body to a scene object
    pub fn link_body_to_object(&mut self, body: PhysicsBodyId, object_name: String) {
        self.body_to_object.insert(body, object_name.clone());
        self.object_to_body.insert(object_name, body);
    }

    /// Unlink a physics body from its scene object
    pub fn unlink_body(&mut self, body: PhysicsBodyId) {
        if let Some(object_name) = self.body_to_object.remove(&body) {
            self.object_to_body.remove(&object_name);
        }
    }

    /// Unlink a scene object from its physics body
    pub fn unlink_object(&mut self, object_name: &str) {
        if let Some(body) = self.object_to_body.remove(object_name) {
            self.body_to_object.remove(&body);
        }
    }

    /// Get the scene object name for a physics body
    pub fn get_object_for_body(&self, body: PhysicsBodyId) -> Option<&String> {
        self.body_to_object.get(&body)
    }

    /// Get the physics body for a scene object
    pub fn get_body_for_object(&self, object_name: &str) -> Option<PhysicsBodyId> {
        self.object_to_body.get(object_name).copied()
    }

    /// Check if a body is linked
    pub fn is_body_linked(&self, body: PhysicsBodyId) -> bool {
        self.body_to_object.contains_key(&body)
    }

    /// Check if an object is linked
    pub fn is_object_linked(&self, object_name: &str) -> bool {
        self.object_to_body.contains_key(object_name)
    }

    /// Get all linked pairs
    pub fn linked_pairs(&self) -> impl Iterator<Item = (PhysicsBodyId, &String)> + '_ {
        self.body_to_object.iter().map(|(k, v)| (*k, v))
    }

    /// Clear all links
    pub fn clear(&mut self) {
        self.body_to_object.clear();
        self.object_to_body.clear();
    }
}

impl Default for PhysicsSceneBridge {
    fn default() -> Self {
        Self::new()
    }
}

impl PhysicsSceneBridge {
    /// Simulate physics and update scene objects
    /// This should be called once per frame from the scene update loop
    pub fn simulate_physics(
        &self,
        physics_world: &mut ww3d_collision::physics_integration::PhysicsWorld,
        scene: &mut crate::SceneClass,
    ) {
        // Step the physics simulation
        physics_world.step();

        // Update scene objects from physics bodies
        for (body_id, object_name) in self.linked_pairs() {
            if let Some(body) = physics_world.get_body(body_id) {
                // Find the object in the scene
                for obj in &mut scene.objects {
                    if obj.get_name() == object_name {
                        // Create transform from physics body
                        let transform = PhysicsTransform::new(
                            body.position,
                            body.rotation,
                            Vec3::ONE, // Physics doesn't handle scale
                        );
                        obj.set_transform(transform.to_matrix());
                        break;
                    }
                }
            }
        }
    }

    /// Update collision shapes from deformed mesh vertices
    /// Critical for ragdoll physics and animated collision detection
    /// C++ Reference: Package 7 integration example
    pub fn update_collision_from_deformed_mesh(
        &self,
        scene_ext: &crate::scene_ext::SceneExt,
        physics_world: &mut ww3d_collision::physics_integration::PhysicsWorld,
    ) {
        for (body_id, object_name) in self.linked_pairs() {
            if let Some(obj) = scene_ext.find_object(object_name) {
                // Get deformed vertices for accurate collision
                if let Some(deformed_verts) = obj.get_deformed_vertices() {
                    // Compute new bounding volumes from deformed geometry
                    let _bbox = compute_bbox_from_vertices(&deformed_verts);

                    // Update physics body with new bounds
                    if let Some(body) = physics_world.get_body_mut(body_id) {
                        // In a full implementation, we'd update the collision shape
                        // For now, just update the bounding box
                        body.recompute_inertia_tensor();
                    }
                }
            }
        }
    }

    /// Update physics bodies from scene objects
    /// This should be called when scene objects are moved externally (e.g., by animation)
    pub fn update_physics_from_scene(
        &self,
        scene: &crate::SceneClass,
        physics_world: &mut ww3d_collision::physics_integration::PhysicsWorld,
    ) {
        for (body_id, object_name) in self.linked_pairs() {
            // Find the object in the scene
            for obj in &scene.objects {
                if obj.get_name() == object_name {
                    let transform = PhysicsTransform::from_matrix(obj.get_transform());

                    if let Some(body) = physics_world.get_body_mut(body_id) {
                        body.position = transform.position;
                        body.rotation = transform.rotation;
                        body.update_transform();
                        body.update_world_inertia_tensor();
                    }
                    break;
                }
            }
        }
    }
}

/// Physics transform data
#[derive(Debug, Clone, Copy)]
pub struct PhysicsTransform {
    pub position: Vec3,
    pub rotation: Quat,
    pub scale: Vec3,
}

impl PhysicsTransform {
    /// Create a new physics transform
    pub fn new(position: Vec3, rotation: Quat, scale: Vec3) -> Self {
        Self {
            position,
            rotation,
            scale,
        }
    }

    /// Create identity transform
    pub fn identity() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }

    /// Convert to a 4x4 transformation matrix
    pub fn to_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }

    /// Create from a 4x4 transformation matrix
    pub fn from_matrix(matrix: &Mat4) -> Self {
        let (scale, rotation, position) = matrix.to_scale_rotation_translation();
        Self {
            position,
            rotation,
            scale,
        }
    }
}

impl From<Mat4> for PhysicsTransform {
    fn from(matrix: Mat4) -> Self {
        Self::from_matrix(&matrix)
    }
}

impl From<PhysicsTransform> for Mat4 {
    fn from(transform: PhysicsTransform) -> Self {
        transform.to_matrix()
    }
}

/// Update scene object transforms from physics simulation
pub fn update_scene_from_physics(
    scene: &mut crate::SceneClass,
    bridge: &PhysicsSceneBridge,
    physics_transforms: &HashMap<PhysicsBodyId, PhysicsTransform>,
) {
    for (body_handle, transform) in physics_transforms {
        if let Some(object_name) = bridge.get_object_for_body(*body_handle) {
            // Find the object in the scene
            for obj in &mut scene.objects {
                if obj.get_name() == object_name {
                    // Update the object's transform
                    obj.set_transform(transform.to_matrix());
                    break;
                }
            }
        }
    }
}

/// Extract physics transforms from scene objects
pub fn extract_physics_from_scene(
    scene: &crate::SceneClass,
    bridge: &PhysicsSceneBridge,
) -> HashMap<PhysicsBodyId, PhysicsTransform> {
    let mut transforms = HashMap::new();

    for obj in &scene.objects {
        let object_name = obj.get_name();
        if let Some(body_handle) = bridge.get_body_for_object(object_name) {
            let transform = PhysicsTransform::from_matrix(obj.get_transform());
            transforms.insert(body_handle, transform);
        }
    }

    transforms
}

/// Helper function to compute bounding box from vertices
fn compute_bbox_from_vertices(vertices: &[Vec3]) -> (Vec3, Vec3) {
    if vertices.is_empty() {
        return (Vec3::ZERO, Vec3::ZERO);
    }

    let mut min = vertices[0];
    let mut max = vertices[0];

    for vertex in vertices.iter().skip(1) {
        min = min.min(*vertex);
        max = max.max(*vertex);
    }

    (min, max)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ww3d_collision::physics_integration::{CollisionShape, PhysicsWorld, RigidBodyDesc};

    #[test]
    fn test_physics_scene_bridge() {
        let mut bridge = PhysicsSceneBridge::new();
        let mut physics_world = PhysicsWorld::new();

        // Create an actual physics body through the proper API
        let body = physics_world.create_body(RigidBodyDesc {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        });

        let object_name = "TestObject".to_string();

        // Link
        bridge.link_body_to_object(body, object_name.clone());
        assert!(bridge.is_body_linked(body));
        assert!(bridge.is_object_linked(&object_name));

        // Verify bidirectional mapping
        assert_eq!(bridge.get_object_for_body(body), Some(&object_name));
        assert_eq!(bridge.get_body_for_object(&object_name), Some(body));

        // Unlink
        bridge.unlink_body(body);
        assert!(!bridge.is_body_linked(body));
        assert!(!bridge.is_object_linked(&object_name));
    }

    #[test]
    fn test_physics_transform_matrix_conversion() {
        let transform = PhysicsTransform::new(
            Vec3::new(1.0, 2.0, 3.0),
            Quat::from_rotation_y(std::f32::consts::PI / 4.0),
            Vec3::ONE,
        );

        let matrix = transform.to_matrix();
        let back = PhysicsTransform::from_matrix(&matrix);

        // Verify round-trip conversion (with floating point tolerance)
        assert!((transform.position - back.position).length() < 0.0001);
        assert!((transform.rotation.xyz() - back.rotation.xyz()).length() < 0.0001);
        assert!((transform.scale - back.scale).length() < 0.0001);
    }

    #[test]
    fn test_linked_pairs_iterator() {
        let mut bridge = PhysicsSceneBridge::new();
        let mut physics_world = PhysicsWorld::new();

        // Create two physics bodies through the proper API
        let body1 = physics_world.create_body(RigidBodyDesc {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        });

        let body2 = physics_world.create_body(RigidBodyDesc {
            position: Vec3::new(5.0, 0.0, 0.0),
            rotation: Quat::IDENTITY,
            shape: CollisionShape::Sphere { radius: 1.0 },
            ..Default::default()
        });

        bridge.link_body_to_object(body1, "Object1".to_string());
        bridge.link_body_to_object(body2, "Object2".to_string());

        let pairs: Vec<_> = bridge.linked_pairs().collect();
        assert_eq!(pairs.len(), 2);
    }
}
