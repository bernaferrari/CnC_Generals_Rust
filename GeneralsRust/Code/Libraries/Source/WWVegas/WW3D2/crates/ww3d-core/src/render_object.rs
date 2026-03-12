/// Render object trait system
///
/// This module defines the core render object trait that all renderable objects must implement.
/// It provides the foundation for the WW3D rendering system.
use crate::errors::W3DResult;
use crate::RenderObjClassId;
use glam::{Mat4, Vec3};
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

/// Bounding sphere for culling and intersection tests
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundingSphere {
    pub center: Vec3,
    pub radius: f32,
}

impl BoundingSphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn zero() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 0.0,
        }
    }

    pub fn merge(&self, other: &BoundingSphere) -> BoundingSphere {
        let delta = other.center - self.center;
        let dist = delta.length();

        if dist + other.radius <= self.radius {
            // Other is inside self
            *self
        } else if dist + self.radius <= other.radius {
            // Self is inside other
            *other
        } else {
            // Merge
            let new_radius = (dist + self.radius + other.radius) * 0.5;
            let new_center = self.center + delta * ((new_radius - self.radius) / dist);
            BoundingSphere::new(new_center, new_radius)
        }
    }
}

/// Axis-aligned bounding box
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABox {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABox {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn zero() -> Self {
        Self {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
        }
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        if points.is_empty() {
            return Self::zero();
        }

        let mut min = points[0];
        let mut max = points[0];

        for point in points.iter().skip(1) {
            min = min.min(*point);
            max = max.max(*point);
        }

        Self::new(min, max)
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn merge(&self, other: &AABox) -> AABox {
        AABox::new(self.min.min(other.min), self.max.max(other.max))
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    pub fn intersects(&self, other: &AABox) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }
}

/// Ray for collision tests
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// Ray collision test result
#[derive(Debug, Clone, Copy)]
pub struct RayCollisionResult {
    pub hit: bool,
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

impl RayCollisionResult {
    pub fn no_hit() -> Self {
        Self {
            hit: false,
            distance: f32::MAX,
            point: Vec3::ZERO,
            normal: Vec3::ZERO,
        }
    }

    pub fn new(distance: f32, point: Vec3, normal: Vec3) -> Self {
        Self {
            hit: true,
            distance,
            point,
            normal,
        }
    }
}

/// Render information passed to render calls
#[derive(Debug, Clone)]
pub struct RenderInfo {
    /// View-projection matrix
    pub view_projection: Mat4,
    /// View matrix
    pub view: Mat4,
    /// Projection matrix
    pub projection: Mat4,
    /// Camera position in world space
    pub camera_position: Vec3,
    /// Frame time in seconds
    pub delta_time: f32,
    /// Total elapsed time
    pub elapsed_time: f32,
}

impl RenderInfo {
    pub fn new() -> Self {
        Self {
            view_projection: Mat4::IDENTITY,
            view: Mat4::IDENTITY,
            projection: Mat4::IDENTITY,
            camera_position: Vec3::ZERO,
            delta_time: 0.0,
            elapsed_time: 0.0,
        }
    }
}

impl Default for RenderInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Special render modes for non-standard rendering passes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpecialRenderMode {
    /// Shadow map rendering
    Shadow,
    /// Depth-only rendering
    DepthOnly,
    /// Object ID rendering for picking
    ObjectId,
    /// Wireframe rendering
    Wireframe,
}

/// Special render information
#[derive(Debug, Clone)]
pub struct SpecialRenderInfo {
    pub mode: SpecialRenderMode,
    pub render_info: RenderInfo,
}

/// Render hook interface for pre/post render callbacks
pub trait RenderHook: Debug {
    /// Called before rendering the object
    /// Returns false to skip rendering
    fn pre_render(&mut self, render_obj: &dyn RenderObject, info: &RenderInfo) -> bool;

    /// Called after rendering the object
    fn post_render(&mut self, render_obj: &dyn RenderObject, info: &RenderInfo);
}

/// Core render object trait
///
/// All renderable objects in WW3D must implement this trait.
pub trait RenderObject: Debug + Send + Sync {
    /// Get the class ID for RTTI
    fn class_id(&self) -> RenderObjClassId;

    /// Get the object name
    fn name(&self) -> &str;

    /// Set the object name
    fn set_name(&mut self, name: String);

    /// Clone this render object
    fn clone_object(&self) -> Box<dyn RenderObject>;

    /// Render the object
    fn render(&mut self, info: &RenderInfo) -> W3DResult<()>;

    /// Special rendering for non-standard passes
    fn special_render(&mut self, _info: &SpecialRenderInfo) -> W3DResult<()> {
        // Default implementation does nothing
        Ok(())
    }

    /// Get the object-space bounding sphere
    fn get_obj_space_bounding_sphere(&self) -> BoundingSphere {
        BoundingSphere::zero()
    }

    /// Get the object-space bounding box
    fn get_obj_space_bounding_box(&self) -> AABox {
        AABox::zero()
    }

    /// Get the world transform
    fn get_transform(&self) -> Mat4 {
        Mat4::IDENTITY
    }

    /// Set the world transform
    fn set_transform(&mut self, transform: Mat4);

    /// Cast a ray against this object
    fn cast_ray(&self, _ray: &Ray) -> RayCollisionResult {
        RayCollisionResult::no_hit()
    }

    /// Test intersection with an axis-aligned box
    fn intersect_aabox(&self, _bbox: &AABox) -> bool {
        false
    }

    /// Scale the object
    fn scale(&mut self, scale: f32) {
        self.scale_xyz(scale, scale, scale);
    }

    /// Scale the object with different factors per axis
    fn scale_xyz(&mut self, _sx: f32, _sy: f32, _sz: f32) {
        // Default implementation does nothing
    }

    /// Get the number of polygons in this object
    fn get_num_polys(&self) -> usize {
        0
    }

    /// Get the sort level for transparent rendering
    fn get_sort_level(&self) -> i32 {
        0
    }

    /// Set the sort level
    fn set_sort_level(&mut self, _level: i32) {
        // Default implementation does nothing
    }

    /// Update cached bounding volumes
    fn update_cached_bounding_volumes(&mut self) {
        // Default implementation does nothing
    }

    /// Allow downcasting to Any for type-specific operations
    fn as_any(&self) -> &dyn Any;

    /// Allow mutable downcasting to Any
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Reference-counted render object
pub type RenderObjectRef = Arc<dyn RenderObject>;

/// Render object builder trait for creating objects from W3D files
pub trait RenderObjectBuilder: Debug {
    /// Build a render object from W3D data
    fn build(&self, data: &[u8]) -> W3DResult<Box<dyn RenderObject>>;

    /// Get the class ID this builder creates
    fn class_id(&self) -> RenderObjClassId;
}

/// Collection of render objects that can be rendered as a group
#[derive(Debug)]
pub struct RenderObjectCollection {
    objects: Vec<Box<dyn RenderObject>>,
    transform: Mat4,
}

impl RenderObjectCollection {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            transform: Mat4::IDENTITY,
        }
    }

    pub fn add(&mut self, object: Box<dyn RenderObject>) {
        self.objects.push(object);
    }

    pub fn remove(&mut self, index: usize) -> Option<Box<dyn RenderObject>> {
        if index < self.objects.len() {
            Some(self.objects.remove(index))
        } else {
            None
        }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&dyn RenderObject> {
        self.objects.get(index).map(|o| o.as_ref())
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut (dyn RenderObject + '_)> {
        if let Some(obj) = self.objects.get_mut(index) {
            Some(obj.as_mut())
        } else {
            None
        }
    }

    pub fn objects_slice(&self) -> &[Box<dyn RenderObject>] {
        &self.objects
    }

    pub fn objects_slice_mut(&mut self) -> &mut [Box<dyn RenderObject>] {
        &mut self.objects
    }

    pub fn set_transform(&mut self, transform: Mat4) {
        self.transform = transform;
        for obj in &mut self.objects {
            obj.set_transform(transform);
        }
    }

    pub fn get_transform(&self) -> Mat4 {
        self.transform
    }

    pub fn render(&mut self, info: &RenderInfo) -> W3DResult<()> {
        for obj in &mut self.objects {
            obj.render(info)?;
        }
        Ok(())
    }

    pub fn get_bounding_sphere(&self) -> BoundingSphere {
        if self.objects.is_empty() {
            return BoundingSphere::zero();
        }

        let mut sphere = self.objects[0].get_obj_space_bounding_sphere();
        for obj in self.objects.iter().skip(1) {
            sphere = sphere.merge(&obj.get_obj_space_bounding_sphere());
        }
        sphere
    }

    pub fn get_bounding_box(&self) -> AABox {
        if self.objects.is_empty() {
            return AABox::zero();
        }

        let mut bbox = self.objects[0].get_obj_space_bounding_box();
        for obj in self.objects.iter().skip(1) {
            bbox = bbox.merge(&obj.get_obj_space_bounding_box());
        }
        bbox
    }
}

impl Default for RenderObjectCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bounding_sphere_merge() {
        let s1 = BoundingSphere::new(Vec3::ZERO, 1.0);
        let s2 = BoundingSphere::new(Vec3::new(2.0, 0.0, 0.0), 1.0);
        let merged = s1.merge(&s2);

        assert!(merged.radius >= 1.0);
        assert!(merged.center.x > 0.0);
    }

    #[test]
    fn test_aabox_contains_point() {
        let bbox = AABox::new(Vec3::ZERO, Vec3::ONE);

        assert!(bbox.contains_point(Vec3::new(0.5, 0.5, 0.5)));
        assert!(bbox.contains_point(Vec3::ZERO));
        assert!(bbox.contains_point(Vec3::ONE));
        assert!(!bbox.contains_point(Vec3::new(2.0, 0.5, 0.5)));
    }

    #[test]
    fn test_aabox_intersects() {
        let bbox1 = AABox::new(Vec3::ZERO, Vec3::ONE);
        let bbox2 = AABox::new(Vec3::new(0.5, 0.5, 0.5), Vec3::new(1.5, 1.5, 1.5));
        let bbox3 = AABox::new(Vec3::new(2.0, 2.0, 2.0), Vec3::new(3.0, 3.0, 3.0));

        assert!(bbox1.intersects(&bbox2));
        assert!(!bbox1.intersects(&bbox3));
    }

    #[test]
    fn test_ray_point_at() {
        let ray = Ray::new(Vec3::ZERO, Vec3::X);
        let point = ray.point_at(5.0);

        assert_eq!(point, Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn test_render_object_collection() {
        let mut collection = RenderObjectCollection::new();
        assert!(collection.is_empty());
        assert_eq!(collection.len(), 0);

        collection.set_transform(Mat4::from_scale(Vec3::splat(2.0)));
        assert_eq!(
            collection.get_transform(),
            Mat4::from_scale(Vec3::splat(2.0))
        );
    }
}
