//! Random Vector3 generation functionality
//!
//! This module provides various randomizer implementations for generating
//! random Vector3 points within different geometric shapes.

use crate::vector3::Vector3;
use crate::WWMath;
use rand::{thread_rng, Rng};

/// Class ID constants for randomizer identification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum RandomizerClassId {
    Unknown = 0xFFFF_FFFF,
    SolidBox = 0,
    SolidSphere = 1,
    HollowSphere = 2,
    SolidCylinder = 3,
}

/// Trait for Vector3 randomizers - generates random Vector3 points within different shapes
pub trait Vector3Randomizer {
    /// Get RTTI class identification
    fn class_id(&self) -> RandomizerClassId;

    /// Generate a random vector
    fn get_vector(&mut self) -> Vector3;

    /// Get the maximum component possible for generated vectors
    fn get_maximum_extent(&self) -> f32;

    /// Scale all vectors produced in future by the given factor
    fn scale(&mut self, scale: f32);

    /// Clone the randomizer
    fn clone_randomizer(&self) -> Box<dyn Vector3Randomizer>;
}

/// Generates points uniformly distributed inside a box centered on the origin
#[derive(Debug, Clone)]
pub struct Vector3SolidBoxRandomizer {
    extents: Vector3,
}

impl Vector3SolidBoxRandomizer {
    pub fn new(extents: Vector3) -> Self {
        Self {
            extents: Vector3::new(extents.x.max(0.0), extents.y.max(0.0), extents.z.max(0.0)),
        }
    }

    pub fn get_extents(&self) -> Vector3 {
        self.extents
    }
}

impl Vector3Randomizer for Vector3SolidBoxRandomizer {
    fn class_id(&self) -> RandomizerClassId {
        RandomizerClassId::SolidBox
    }

    fn get_vector(&mut self) -> Vector3 {
        let mut rng = thread_rng();
        Vector3::new(
            (rng.gen::<f32>() - 0.5) * 2.0 * self.extents.x,
            (rng.gen::<f32>() - 0.5) * 2.0 * self.extents.y,
            (rng.gen::<f32>() - 0.5) * 2.0 * self.extents.z,
        )
    }

    fn get_maximum_extent(&self) -> f32 {
        self.extents.x.max(self.extents.y.max(self.extents.z))
    }

    fn scale(&mut self, scale: f32) {
        let scale = scale.max(0.0);
        self.extents.x *= scale;
        self.extents.y *= scale;
        self.extents.z *= scale;
    }

    fn clone_randomizer(&self) -> Box<dyn Vector3Randomizer> {
        Box::new(self.clone())
    }
}

/// Generates points uniformly distributed inside a sphere centered on the origin
#[derive(Debug, Clone)]
pub struct Vector3SolidSphereRandomizer {
    radius: f32,
}

impl Vector3SolidSphereRandomizer {
    pub fn new(radius: f32) -> Self {
        Self {
            radius: radius.max(0.0),
        }
    }

    pub fn get_radius(&self) -> f32 {
        self.radius
    }
}

impl Vector3Randomizer for Vector3SolidSphereRandomizer {
    fn class_id(&self) -> RandomizerClassId {
        RandomizerClassId::SolidSphere
    }

    fn get_vector(&mut self) -> Vector3 {
        let mut rng = thread_rng();
        let rad_squared = self.radius * self.radius;

        loop {
            let vector = Vector3::new(
                (rng.gen::<f32>() - 0.5) * 2.0 * self.radius,
                (rng.gen::<f32>() - 0.5) * 2.0 * self.radius,
                (rng.gen::<f32>() - 0.5) * 2.0 * self.radius,
            );

            if vector.length_squared() <= rad_squared {
                return vector;
            }
        }
    }

    fn get_maximum_extent(&self) -> f32 {
        self.radius
    }

    fn scale(&mut self, scale: f32) {
        let scale = scale.max(0.0);
        self.radius *= scale;
    }

    fn clone_randomizer(&self) -> Box<dyn Vector3Randomizer> {
        Box::new(self.clone())
    }
}

/// Generates points uniformly distributed on the surface of a sphere centered on the origin
#[derive(Debug, Clone)]
pub struct Vector3HollowSphereRandomizer {
    radius: f32,
}

impl Vector3HollowSphereRandomizer {
    pub fn new(radius: f32) -> Self {
        Self {
            radius: radius.max(0.0),
        }
    }

    pub fn get_radius(&self) -> f32 {
        self.radius
    }
}

impl Vector3Randomizer for Vector3HollowSphereRandomizer {
    fn class_id(&self) -> RandomizerClassId {
        RandomizerClassId::HollowSphere
    }

    fn get_vector(&mut self) -> Vector3 {
        let mut rng = thread_rng();

        loop {
            let vector = Vector3::new(
                (rng.gen::<f32>() - 0.5) * 2.0,
                (rng.gen::<f32>() - 0.5) * 2.0,
                (rng.gen::<f32>() - 0.5) * 2.0,
            );

            let v_l2 = vector.length_squared();
            if v_l2 <= 1.0 && v_l2 > 0.0 {
                let scale = self.radius * WWMath::inv_sqrt(v_l2);
                return vector * scale;
            }
        }
    }

    fn get_maximum_extent(&self) -> f32 {
        self.radius
    }

    fn scale(&mut self, scale: f32) {
        let scale = scale.max(0.0);
        self.radius *= scale;
    }

    fn clone_randomizer(&self) -> Box<dyn Vector3Randomizer> {
        Box::new(self.clone())
    }
}

/// Generates points uniformly distributed inside a cylinder centered on the origin
/// (set extent to 0 for a disk)
#[derive(Debug, Clone)]
pub struct Vector3SolidCylinderRandomizer {
    extent: f32, // height in X direction
    radius: f32,
}

impl Vector3SolidCylinderRandomizer {
    pub fn new(extent: f32, radius: f32) -> Self {
        Self {
            extent: extent.max(0.0),
            radius: radius.max(0.0),
        }
    }

    pub fn get_radius(&self) -> f32 {
        self.radius
    }

    pub fn get_height(&self) -> f32 {
        self.extent
    }
}

impl Vector3Randomizer for Vector3SolidCylinderRandomizer {
    fn class_id(&self) -> RandomizerClassId {
        RandomizerClassId::SolidCylinder
    }

    fn get_vector(&mut self) -> Vector3 {
        let mut rng = thread_rng();
        let x = (rng.gen::<f32>() - 0.5) * 2.0 * self.extent;

        // Generate 2D vectors in a square and discard the ones not in a circle
        let rad_squared = self.radius * self.radius;
        loop {
            let y = (rng.gen::<f32>() - 0.5) * 2.0 * self.radius;
            let z = (rng.gen::<f32>() - 0.5) * 2.0 * self.radius;

            if y * y + z * z <= rad_squared {
                return Vector3::new(x, y, z);
            }
        }
    }

    fn get_maximum_extent(&self) -> f32 {
        self.extent.max(self.radius)
    }

    fn scale(&mut self, scale: f32) {
        let scale = scale.max(0.0);
        self.extent *= scale;
        self.radius *= scale;
    }

    fn clone_randomizer(&self) -> Box<dyn Vector3Randomizer> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_solid_box_randomizer() {
        let mut randomizer = Vector3SolidBoxRandomizer::new(Vector3::new(10.0, 5.0, 2.0));
        assert_eq!(randomizer.class_id(), RandomizerClassId::SolidBox);
        assert_eq!(randomizer.get_maximum_extent(), 10.0);
        assert_eq!(randomizer.get_extents(), Vector3::new(10.0, 5.0, 2.0));

        // Generate some vectors and check they're within bounds
        for _ in 0..100 {
            let v = randomizer.get_vector();
            assert!(v.x.abs() <= 10.0);
            assert!(v.y.abs() <= 5.0);
            assert!(v.z.abs() <= 2.0);
        }

        randomizer.scale(2.0);
        assert_eq!(randomizer.get_extents(), Vector3::new(20.0, 10.0, 4.0));
    }

    #[test]
    fn test_solid_sphere_randomizer() {
        let mut randomizer = Vector3SolidSphereRandomizer::new(5.0);
        assert_eq!(randomizer.class_id(), RandomizerClassId::SolidSphere);
        assert_eq!(randomizer.get_maximum_extent(), 5.0);
        assert_eq!(randomizer.get_radius(), 5.0);

        // Generate some vectors and check they're within the sphere
        for _ in 0..100 {
            let v = randomizer.get_vector();
            assert!(v.length() <= 5.0);
        }

        randomizer.scale(2.0);
        assert_eq!(randomizer.get_radius(), 10.0);
    }

    #[test]
    fn test_hollow_sphere_randomizer() {
        let mut randomizer = Vector3HollowSphereRandomizer::new(3.0);
        assert_eq!(randomizer.class_id(), RandomizerClassId::HollowSphere);
        assert_eq!(randomizer.get_maximum_extent(), 3.0);
        assert_eq!(randomizer.get_radius(), 3.0);

        // Generate some vectors and check they're on the sphere surface
        for _ in 0..100 {
            let v = randomizer.get_vector();
            let len = v.length();
            assert!((len - 3.0).abs() < 1e-5); // Should be exactly on the surface
        }

        randomizer.scale(2.0);
        assert_eq!(randomizer.get_radius(), 6.0);
    }

    #[test]
    fn test_solid_cylinder_randomizer() {
        let mut randomizer = Vector3SolidCylinderRandomizer::new(8.0, 4.0);
        assert_eq!(randomizer.class_id(), RandomizerClassId::SolidCylinder);
        assert_eq!(randomizer.get_maximum_extent(), 8.0);
        assert_eq!(randomizer.get_height(), 8.0);
        assert_eq!(randomizer.get_radius(), 4.0);

        // Generate some vectors and check they're within the cylinder
        for _ in 0..100 {
            let v = randomizer.get_vector();
            assert!(v.x.abs() <= 8.0);
            let radial_distance = (v.y * v.y + v.z * v.z).sqrt();
            assert!(radial_distance <= 4.0);
        }

        randomizer.scale(0.5);
        assert_eq!(randomizer.get_height(), 4.0);
        assert_eq!(randomizer.get_radius(), 2.0);
    }

    #[test]
    fn test_clone_randomizer() {
        let randomizer = Vector3SolidBoxRandomizer::new(Vector3::new(1.0, 2.0, 3.0));
        let cloned = randomizer.clone_randomizer();
        assert_eq!(cloned.class_id(), RandomizerClassId::SolidBox);
        assert_eq!(cloned.get_maximum_extent(), 3.0);
    }
}
