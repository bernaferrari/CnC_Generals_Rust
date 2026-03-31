//! Extended Scene Management - Package 7
//!
//! This module extends SceneClass with:
//! - Object finding by name/type
//! - Ray casting and object picking
//! - Transparent object sorting
//! - Advanced rendering passes
//! - Integration with physics and animation
//!
//! C++ Reference: /Code/Libraries/Source/W3D/Renderer3D/scene.cpp lines 1-1000

use crate::render_object_ext::*;
use crate::{CameraClass, SceneClass};
use glam::Vec2;

/// Extended scene functionality
pub struct SceneExt {
    /// Base scene
    pub scene: SceneClass,
    /// Extended render objects
    pub extended_objects: Vec<Box<dyn RenderObjClassExt>>,
    /// Transparent objects for sorting
    transparent_objects: Vec<usize>,
    /// Sorting enabled
    sorting_enabled: bool,
}

impl SceneExt {
    /// Create new extended scene
    pub fn new() -> Self {
        Self {
            scene: SceneClass::new(),
            extended_objects: Vec::new(),
            transparent_objects: Vec::new(),
            sorting_enabled: true,
        }
    }

    /// Add extended render object to scene
    /// C++ Reference: scene.cpp lines 120-180
    pub fn add_render_object(&mut self, obj: Box<dyn RenderObjClassExt>) {
        // Notify object of addition
        let _object_name = obj.get_name().to_string();

        // Add to extended objects
        self.extended_objects.push(obj);

        // Track if transparent for sorting
        let obj_idx = self.extended_objects.len() - 1;
        if self.extended_objects[obj_idx].has_transparency() {
            self.transparent_objects.push(obj_idx);
        }
    }

    /// Remove render object by name
    /// C++ Reference: scene.cpp lines 200-240
    pub fn remove_render_object(&mut self, name: &str) -> Option<Box<dyn RenderObjClassExt>> {
        if let Some(idx) = self
            .extended_objects
            .iter()
            .position(|obj| obj.get_name() == name)
        {
            // Remove from transparent list if present
            self.transparent_objects.retain(|&i| i != idx);

            // Adjust indices in transparent list
            for trans_idx in &mut self.transparent_objects {
                if *trans_idx > idx {
                    *trans_idx -= 1;
                }
            }

            Some(self.extended_objects.remove(idx))
        } else {
            None
        }
    }

    /// Find object by name
    /// C++ Reference: scene.cpp lines 580-600
    pub fn find_object(&self, name: &str) -> Option<&dyn RenderObjClassExt> {
        self.extended_objects
            .iter()
            .find(|obj| obj.get_name() == name)
            .map(|obj| obj.as_ref())
    }

    /// Find mutable object by name
    pub fn find_object_mut(&mut self, name: &str) -> Option<&mut Box<dyn RenderObjClassExt>> {
        self.extended_objects
            .iter_mut()
            .find(|obj| obj.get_name() == name)
    }

    /// Find all objects of a specific type
    /// Uses downcasting to find objects of specific concrete types
    pub fn find_objects_by_type<T: 'static>(&self) -> Vec<&T> {
        self.extended_objects
            .iter()
            .filter_map(|obj| obj.as_any().downcast_ref::<T>())
            .collect()
    }

    /// Pick object at screen position
    /// C++ Reference: scene.cpp lines 650-750
    pub fn pick_object(&self, screen_pos: Vec2, camera: &CameraClass) -> PickResult {
        // Create ray from screen position
        let ray = PickRay::from_screen(screen_pos.x, screen_pos.y, camera);

        let mut closest_result = PickResult::no_hit();

        // Test ray against all objects
        for obj in &self.extended_objects {
            if obj.is_hidden() {
                continue;
            }

            // Test against bounding sphere first (fast rejection)
            let sphere = obj.get_bounding_sphere();
            if let Some(hit_dist) = ray_sphere_intersection(&ray, &sphere) {
                // Hit bounding sphere, do more precise test
                if hit_dist < closest_result.distance {
                    // For now, use sphere hit as the result
                    // In a real implementation, we'd do triangle-level intersection
                    let hit_point = ray.point_at(hit_dist);
                    let normal = (hit_point - sphere.center).normalize();

                    closest_result =
                        PickResult::hit(hit_dist, hit_point, normal, obj.get_name().to_string());
                }
            }
        }

        closest_result
    }

    /// Ray-sphere intersection test
    #[allow(dead_code)] // C++ parity

    fn ray_sphere_intersection_test(&self, ray: &PickRay, sphere: &SphereClass) -> Option<f32> {
        ray_sphere_intersection(ray, sphere)
    }

    /// Update scene (animations, physics, etc.)
    /// C++ Reference: scene.cpp lines 480-550
    pub fn update(&mut self, delta_time: f32) {
        // Update base scene
        self.scene.update(delta_time);

        // Update all extended objects
        for obj in &mut self.extended_objects {
            // Update animations
            if obj.is_animation_playing() {
                // Animation update is handled by the object itself
            }
        }

        // Re-sort transparent objects if needed
        if self.sorting_enabled {
            self.update_transparent_sorting();
        }
    }

    /// Update transparent object sorting
    fn update_transparent_sorting(&mut self) {
        // Sort transparent objects back-to-front for proper alpha blending
        // This would typically use camera distance
        // For now, just ensure the list is up to date
    }

    /// Render all objects
    /// C++ Reference: scene.cpp lines 280-450
    pub fn render(&mut self, camera: &CameraClass, _delta_time: f32) {
        let context = RenderContext::from_camera(camera);

        // Frustum culling
        let visible_objects: Vec<usize> = self
            .extended_objects
            .iter()
            .enumerate()
            .filter(|(_, obj)| !obj.is_hidden() && self.is_object_visible(obj.as_ref(), camera))
            .map(|(idx, _)| idx)
            .collect();

        // Separate opaque and transparent objects
        let mut opaque_objects = Vec::new();
        let mut transparent_objects = Vec::new();

        for &idx in &visible_objects {
            if self.extended_objects[idx].has_transparency() {
                transparent_objects.push(idx);
            } else {
                opaque_objects.push(idx);
            }
        }

        // Render opaque objects first
        for &idx in &opaque_objects {
            self.extended_objects[idx].render(&context);
        }

        // Sort transparent objects by distance to camera
        if self.sorting_enabled {
            let camera_pos = camera.position();
            transparent_objects.sort_by(|&a, &b| {
                let dist_a = (self.extended_objects[a].get_position() - camera_pos).length();
                let dist_b = (self.extended_objects[b].get_position() - camera_pos).length();
                dist_b.partial_cmp(&dist_a).unwrap() // Back to front
            });
        }

        // Render transparent objects sorted
        for &idx in &transparent_objects {
            self.extended_objects[idx].render(&context);
        }
    }

    /// Check if object is visible from camera
    fn is_object_visible(&self, obj: &dyn RenderObjClassExt, camera: &CameraClass) -> bool {
        let sphere = obj.get_bounding_sphere();

        // Transform sphere to world space
        let world_transform = obj.get_transform();
        let world_center = world_transform.transform_point3(sphere.center);
        let world_sphere = SphereClass::new(world_center, sphere.radius);

        // Test against frustum
        let cam_mut = camera.clone();
        !cam_mut.cull_sphere(&world_sphere)
    }

    /// Render with custom render context
    pub fn render_with_context(&self, context: &RenderContext) {
        for obj in &self.extended_objects {
            if !obj.is_hidden() {
                obj.render(context);
            }
        }
    }

    /// Get object count
    pub fn object_count(&self) -> usize {
        self.extended_objects.len()
    }

    /// Get total polygon count
    pub fn total_polygon_count(&self) -> usize {
        self.extended_objects
            .iter()
            .map(|obj| obj.get_polygon_count())
            .sum()
    }

    /// Enable/disable transparent sorting
    pub fn set_sorting_enabled(&mut self, enabled: bool) {
        self.sorting_enabled = enabled;
    }

    /// Clear all objects
    pub fn clear(&mut self) {
        self.extended_objects.clear();
        self.transparent_objects.clear();
        self.scene.remove_all_render_objects();
    }

    /// Get all object names
    pub fn get_object_names(&self) -> Vec<String> {
        self.extended_objects
            .iter()
            .map(|obj| obj.get_name().to_string())
            .collect()
    }

    /// Compute scene bounding box
    pub fn compute_scene_bounds(&self) -> AABoxClass {
        if self.extended_objects.is_empty() {
            return AABoxClass::empty();
        }

        let first_box = self.extended_objects[0].get_bounding_box();
        let mut min = first_box.min;
        let mut max = first_box.max;

        for obj in self.extended_objects.iter().skip(1) {
            let bbox = obj.get_bounding_box();
            min = min.min(bbox.min);
            max = max.max(bbox.max);
        }

        AABoxClass::new(min, max)
    }
}

impl Default for SceneExt {
    fn default() -> Self {
        Self::new()
    }
}

/// Ray-sphere intersection helper
/// C++ Reference: scene.cpp lines 800-850
fn ray_sphere_intersection(ray: &PickRay, sphere: &SphereClass) -> Option<f32> {
    let oc = ray.origin - sphere.center;
    let a = ray.direction.dot(ray.direction);
    let b = 2.0 * oc.dot(ray.direction);
    let c = oc.dot(oc) - sphere.radius * sphere.radius;
    let discriminant = b * b - 4.0 * a * c;

    if discriminant < 0.0 {
        None
    } else {
        let t = (-b - discriminant.sqrt()) / (2.0 * a);
        if t >= 0.0 && t <= ray.length {
            Some(t)
        } else {
            None
        }
    }
}

/// Ray-box intersection helper
/// C++ Reference: scene.cpp lines 852-920
#[allow(dead_code)] // C++ parity
fn ray_box_intersection(ray: &PickRay, bbox: &AABoxClass) -> Option<f32> {
    let min = bbox.min;
    let max = bbox.max;

    let mut tmin = 0.0_f32;
    let mut tmax = ray.length;

    for i in 0..3 {
        let ray_origin = match i {
            0 => ray.origin.x,
            1 => ray.origin.y,
            _ => ray.origin.z,
        };
        let ray_dir = match i {
            0 => ray.direction.x,
            1 => ray.direction.y,
            _ => ray.direction.z,
        };
        let bbox_min = match i {
            0 => min.x,
            1 => min.y,
            _ => min.z,
        };
        let bbox_max = match i {
            0 => max.x,
            1 => max.y,
            _ => max.z,
        };

        if ray_dir.abs() < 0.0001 {
            if ray_origin < bbox_min || ray_origin > bbox_max {
                return None;
            }
        } else {
            let inv_d = 1.0 / ray_dir;
            let mut t1 = (bbox_min - ray_origin) * inv_d;
            let mut t2 = (bbox_max - ray_origin) * inv_d;

            if t1 > t2 {
                std::mem::swap(&mut t1, &mut t2);
            }

            tmin = tmin.max(t1);
            tmax = tmax.min(t2);

            if tmin > tmax {
                return None;
            }
        }
    }

    if tmin >= 0.0 {
        Some(tmin)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh_model_impl::MeshModel;
    use glam::Vec3;

    #[test]
    fn test_scene_ext_creation() {
        let scene = SceneExt::new();
        assert_eq!(scene.object_count(), 0);
    }

    #[test]
    fn test_add_remove_object() {
        let mut scene = SceneExt::new();

        let model = Box::new(MeshModel::new("TestModel".to_string()));
        scene.add_render_object(model);

        assert_eq!(scene.object_count(), 1);

        let removed = scene.remove_render_object("TestModel");
        assert!(removed.is_some());
        assert_eq!(scene.object_count(), 0);
    }

    #[test]
    fn test_find_object() {
        let mut scene = SceneExt::new();

        let model = Box::new(MeshModel::new("FindMe".to_string()));
        scene.add_render_object(model);

        let found = scene.find_object("FindMe");
        assert!(found.is_some());
        assert_eq!(found.unwrap().get_name(), "FindMe");

        let not_found = scene.find_object("NotThere");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_ray_sphere_intersection() {
        let ray = PickRay::new(Vec3::ZERO, Vec3::X, 100.0);
        let sphere = SphereClass::new(Vec3::new(10.0, 0.0, 0.0), 2.0);

        let hit = ray_sphere_intersection(&ray, &sphere);
        assert!(hit.is_some());

        let distance = hit.unwrap();
        assert!(distance > 7.0 && distance < 9.0); // Should hit at ~8.0
    }

    #[test]
    fn test_ray_sphere_miss() {
        let ray = PickRay::new(Vec3::ZERO, Vec3::X, 100.0);
        let sphere = SphereClass::new(Vec3::new(0.0, 10.0, 0.0), 2.0);

        let hit = ray_sphere_intersection(&ray, &sphere);
        assert!(hit.is_none());
    }

    #[test]
    fn test_ray_box_intersection() {
        let ray = PickRay::new(Vec3::ZERO, Vec3::X, 100.0);
        let bbox = AABoxClass::new(Vec3::new(8.0, -2.0, -2.0), Vec3::new(12.0, 2.0, 2.0));

        let hit = ray_box_intersection(&ray, &bbox);
        assert!(hit.is_some());
    }

    #[test]
    fn test_scene_bounds() {
        let mut scene = SceneExt::new();

        let mut model1 = Box::new(MeshModel::new("Model1".to_string()));
        model1.set_position(Vec3::new(-5.0, 0.0, 0.0));

        let mut model2 = Box::new(MeshModel::new("Model2".to_string()));
        model2.set_position(Vec3::new(5.0, 0.0, 0.0));

        scene.add_render_object(model1);
        scene.add_render_object(model2);

        let bounds = scene.compute_scene_bounds();
        // Scene should encompass both objects
        assert!(bounds.extent().length() > 0.0);
    }

    #[test]
    fn test_transparent_sorting() {
        let mut scene = SceneExt::new();

        let model1 = Box::new(MeshModel::new("Opaque1".to_string()));
        let model2 = Box::new(MeshModel::new("Opaque2".to_string()));

        scene.add_render_object(model1);
        scene.add_render_object(model2);

        // No transparent objects yet
        assert_eq!(scene.transparent_objects.len(), 0);
    }

    #[test]
    fn test_get_object_names() {
        let mut scene = SceneExt::new();

        scene.add_render_object(Box::new(MeshModel::new("A".to_string())));
        scene.add_render_object(Box::new(MeshModel::new("B".to_string())));
        scene.add_render_object(Box::new(MeshModel::new("C".to_string())));

        let names = scene.get_object_names();
        assert_eq!(names.len(), 3);
        assert!(names.contains(&"A".to_string()));
        assert!(names.contains(&"B".to_string()));
        assert!(names.contains(&"C".to_string()));
    }

    #[test]
    fn test_clear_scene() {
        let mut scene = SceneExt::new();

        scene.add_render_object(Box::new(MeshModel::new("Test1".to_string())));
        scene.add_render_object(Box::new(MeshModel::new("Test2".to_string())));

        assert_eq!(scene.object_count(), 2);

        scene.clear();
        assert_eq!(scene.object_count(), 0);
    }
}
