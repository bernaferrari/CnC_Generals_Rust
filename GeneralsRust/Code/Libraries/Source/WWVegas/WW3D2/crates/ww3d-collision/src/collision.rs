//! WW3D2 Collision Test Classes
//!
//! Port of GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/coltest.h and coltest.cpp
//!
//! Provides collision test classes used by the rendering system for ray casting,
//! box collision, and triangle intersection tests. These wrap the bounding volume
//! types with test-specific state (sweep bounds, movement vectors, results).

pub use crate::bounding_volumes::{
    aabox::AABoxClass,
    collision_detection::{CollisionResult, RayCollisionQuery},
    obbox::OBBoxClass,
    plane::{PlaneClass, PlaneClassification},
    sphere::SphereClass,
};

use glam::Vec3;

/// Collision type flags (matches C++ COLL_TYPE_xxx from coltype.h)
pub mod collision_type {
    /// Perform test against everything
    pub const COLL_TYPE_ALL: u32 = 0x01;
    /// Type 0 collision objects (physical)
    pub const COLL_TYPE_0: u32 = 0x02;
    /// Type 1 collision objects (projectile)
    pub const COLL_TYPE_1: u32 = 0x04;
    /// Type 2 collision objects (vis)
    pub const COLL_TYPE_2: u32 = 0x08;
    /// Type 3 collision objects (camera)
    pub const COLL_TYPE_3: u32 = 0x10;
    /// Type 4 collision objects (vehicle)
    pub const COLL_TYPE_4: u32 = 0x20;
    /// Type 5 collision objects
    pub const COLL_TYPE_5: u32 = 0x40;
    /// Type 6 collision objects
    pub const COLL_TYPE_6: u32 = 0x80;

    /// Physics collisions
    pub const COLL_TYPE_PHYSICAL: u32 = COLL_TYPE_0;
    /// Projectile collisions
    pub const COLL_TYPE_PROJECTILE: u32 = COLL_TYPE_1;
    /// "Vis node" detection
    pub const COLL_TYPE_VIS: u32 = COLL_TYPE_2;
    /// Camera collision
    pub const COLL_TYPE_CAMERA: u32 = COLL_TYPE_3;
    /// Vehicle collisions
    pub const COLL_TYPE_VEHICLE: u32 = COLL_TYPE_4;
}

/// Rotation types for AABox collision tests (matches C++ AABoxCollisionTestClass::ROTATION_TYPE)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RotationType {
    /// No rotation
    None = 0,
    /// 90 degrees around Z
    Z90 = 1,
    /// 180 degrees around Z
    Z180 = 2,
    /// 270 degrees around Z
    Z270 = 3,
}

/// Base collision test class (matches C++ CollisionTestClass)
#[derive(Debug, Clone)]
pub struct CollisionTestClass {
    /// Collision result structure
    pub result: CastResultStruct,
    /// Collision type bitmask
    pub collision_type: u32,
    /// The render object that was collided with (set during test)
    pub collided_render_obj_ptr: usize,
}

impl CollisionTestClass {
    pub fn new(result: CastResultStruct, collision_type: u32) -> Self {
        Self {
            result,
            collision_type,
            collided_render_obj_ptr: 0,
        }
    }
}

/// Cast result structure (matches C++ CastResultStruct from castres.h)
#[derive(Debug, Clone, Copy)]
pub struct CastResultStruct {
    /// Fraction along the cast where hit occurred (0-1)
    pub fraction: f32,
    /// Surface normal at hit point
    pub normal: Vec3,
    /// Whether a hit was detected
    pub start_bad: bool,
}

impl Default for CastResultStruct {
    fn default() -> Self {
        Self {
            fraction: 1.0,
            normal: Vec3::ZERO,
            start_bad: false,
        }
    }
}

/// Ray collision test class (matches C++ RayCollisionTestClass)
#[derive(Debug, Clone)]
pub struct RayCollisionTestClass {
    /// Base test data
    pub base: CollisionTestClass,
    /// The ray being tested
    pub ray: LineSegClass,
    /// Whether to check translucent geometry
    pub check_translucent: bool,
    /// Whether to check hidden geometry
    pub check_hidden: bool,
}

/// Line segment class (matches C++ LineSegClass)
#[derive(Debug, Clone, Copy)]
pub struct LineSegClass {
    /// Start point of the segment
    pub start: Vec3,
    /// End point of the segment
    pub end: Vec3,
}

impl LineSegClass {
    pub fn new(start: Vec3, end: Vec3) -> Self {
        Self { start, end }
    }

    /// Get the direction vector from start to end
    pub fn direction(&self) -> Vec3 {
        self.end - self.start
    }

    /// Get the length of the segment
    pub fn length(&self) -> f32 {
        self.direction().length()
    }
}

impl RayCollisionTestClass {
    pub fn new(
        ray: LineSegClass,
        result: CastResultStruct,
        collision_type: u32,
        check_translucent: bool,
        check_hidden: bool,
    ) -> Self {
        Self {
            base: CollisionTestClass::new(result, collision_type),
            ray,
            check_translucent,
            check_hidden,
        }
    }

    /// Cull test against axis-aligned min/max bounds
    pub fn cull_min_max(&self, min: Vec3, max: Vec3) -> bool {
        // Returns true if ray does NOT overlap the box
        let query = RayCollisionQuery::new(self.ray.start, self.ray.direction(), self.ray.length());
        let aabox = AABoxClass::from_min_max(min, max);
        query.test_aabox(&aabox).is_none()
    }

    /// Cull test against an AABox
    pub fn cull_aabox(&self, aabox: &AABoxClass) -> bool {
        let query = RayCollisionQuery::new(self.ray.start, self.ray.direction(), self.ray.length());
        query.test_aabox(aabox).is_none()
    }
}

/// AABox collision test class (matches C++ AABoxCollisionTestClass)
#[derive(Debug, Clone)]
pub struct AABoxCollisionTestClass {
    /// Base test data
    pub base: CollisionTestClass,
    /// The axis-aligned box being tested
    pub box_data: AABoxClass,
    /// Movement vector
    pub move_vec: Vec3,
    /// Minimum corner of sweep volume
    pub sweep_min: Vec3,
    /// Maximum corner of sweep volume
    pub sweep_max: Vec3,
}

impl AABoxCollisionTestClass {
    pub fn new(
        aabox: AABoxClass,
        move_vec: Vec3,
        result: CastResultStruct,
        collision_type: u32,
    ) -> Self {
        let mut test = Self {
            base: CollisionTestClass::new(result, collision_type),
            box_data: aabox,
            move_vec,
            sweep_min: Vec3::ZERO,
            sweep_max: Vec3::ZERO,
        };
        test.calculate_sweep_bounds();
        test
    }

    fn calculate_sweep_bounds(&mut self) {
        self.sweep_min = self.box_data.center - self.box_data.extent;
        self.sweep_max = self.box_data.center + self.box_data.extent;

        let end_min = self.box_data.center + self.move_vec - self.box_data.extent;
        let end_max = self.box_data.center + self.move_vec + self.box_data.extent;

        self.sweep_max = self.sweep_max.max(end_max);
        self.sweep_min = self.sweep_min.min(end_min);
    }

    /// Cull test against axis-aligned min/max bounds
    pub fn cull_min_max(&self, min: Vec3, max: Vec3) -> bool {
        self.sweep_min.x > max.x
            || self.sweep_max.x < min.x
            || self.sweep_min.y > max.y
            || self.sweep_max.y < min.y
            || self.sweep_min.z > max.z
            || self.sweep_max.z < min.z
    }

    /// Cull test against an AABox
    pub fn cull_aabox(&self, aabox: &AABoxClass) -> bool {
        let min_corner = aabox.center - aabox.extent;
        let max_corner = aabox.center + aabox.extent;
        self.cull_min_max(min_corner, max_corner)
    }

    /// Translate the test by a vector
    pub fn translate(&mut self, translation: Vec3) {
        self.box_data.center += translation;
        self.sweep_min += translation;
        self.sweep_max += translation;
    }

    /// Rotate the test around Z axis (for 90-degree increments)
    pub fn rotate(&mut self, rotation: RotationType) {
        match rotation {
            RotationType::None => {}
            RotationType::Z90 => {
                let tmp = self.box_data.center.x;
                self.box_data.center.x = -self.box_data.center.y;
                self.box_data.center.y = tmp;

                let tmp = self.move_vec.x;
                self.move_vec.x = -self.move_vec.y;
                self.move_vec.y = tmp;

                let tmp = self.box_data.extent.x;
                self.box_data.extent.x = self.box_data.extent.y;
                self.box_data.extent.y = tmp;

                let (min_x, min_y, max_x, max_y) = (
                    self.sweep_min.x,
                    self.sweep_min.y,
                    self.sweep_max.x,
                    self.sweep_max.y,
                );
                self.sweep_min.x = -max_y;
                self.sweep_min.y = min_x;
                self.sweep_max.x = -min_y;
                self.sweep_max.y = max_x;
            }
            RotationType::Z180 => {
                self.box_data.center.x = -self.box_data.center.x;
                self.box_data.center.y = -self.box_data.center.y;
                self.move_vec.x = -self.move_vec.x;
                self.move_vec.y = -self.move_vec.y;

                let (min_x, min_y, max_x, max_y) = (
                    self.sweep_min.x,
                    self.sweep_min.y,
                    self.sweep_max.x,
                    self.sweep_max.y,
                );
                self.sweep_min.x = -max_x;
                self.sweep_min.y = -max_y;
                self.sweep_max.x = -min_x;
                self.sweep_max.y = -min_y;
            }
            RotationType::Z270 => {
                let tmp = self.box_data.center.x;
                self.box_data.center.x = self.box_data.center.y;
                self.box_data.center.y = -tmp;

                let tmp = self.move_vec.x;
                self.move_vec.x = self.move_vec.y;
                self.move_vec.y = -tmp;

                let tmp = self.box_data.extent.x;
                self.box_data.extent.x = self.box_data.extent.y;
                self.box_data.extent.y = tmp;

                let (min_x, min_y, max_x, max_y) = (
                    self.sweep_min.x,
                    self.sweep_min.y,
                    self.sweep_max.x,
                    self.sweep_max.y,
                );
                self.sweep_min.x = min_y;
                self.sweep_min.y = -max_x;
                self.sweep_max.x = max_y;
                self.sweep_max.y = -min_x;
            }
        }
    }

    /// Transform the test by a matrix
    pub fn transform(&mut self, tm: &glam::Mat4) {
        let old_center = self.box_data.center;
        let old_extent = self.box_data.extent;

        // Transform center and extent (simplified - expands to enclose rotated box)
        self.box_data.center = tm.transform_point3(old_center);
        self.move_vec = tm.transform_vector3(self.move_vec);

        // Transform all 8 corners of the sweep volume
        let corners = [
            Vec3::new(self.sweep_min.x, self.sweep_min.y, self.sweep_min.z),
            Vec3::new(self.sweep_min.x, self.sweep_max.y, self.sweep_min.z),
            Vec3::new(self.sweep_max.x, self.sweep_max.y, self.sweep_min.z),
            Vec3::new(self.sweep_max.x, self.sweep_min.y, self.sweep_min.z),
            Vec3::new(self.sweep_min.x, self.sweep_min.y, self.sweep_max.z),
            Vec3::new(self.sweep_min.x, self.sweep_max.y, self.sweep_max.z),
            Vec3::new(self.sweep_max.x, self.sweep_max.y, self.sweep_max.z),
            Vec3::new(self.sweep_max.x, self.sweep_min.y, self.sweep_max.z),
        ];

        let mut new_min = tm.transform_point3(corners[0]);
        let mut new_max = new_min;

        for corner in &corners[1..] {
            let p = tm.transform_point3(*corner);
            new_min = new_min.min(p);
            new_max = new_max.max(p);
        }

        self.sweep_min = new_min;
        self.sweep_max = new_max;
    }
}

/// OBBox collision test class (matches C++ OBBoxCollisionTestClass)
#[derive(Debug, Clone)]
pub struct OBBoxCollisionTestClass {
    /// Base test data
    pub base: CollisionTestClass,
    /// The oriented box being tested
    pub box_data: OBBoxClass,
    /// Movement vector
    pub move_vec: Vec3,
    /// Minimum corner of sweep volume
    pub sweep_min: Vec3,
    /// Maximum corner of sweep volume
    pub sweep_max: Vec3,
}

impl OBBoxCollisionTestClass {
    pub fn new(
        obbox: OBBoxClass,
        move_vec: Vec3,
        result: CastResultStruct,
        collision_type: u32,
    ) -> Self {
        let mut test = Self {
            base: CollisionTestClass::new(result, collision_type),
            box_data: obbox,
            move_vec,
            sweep_min: Vec3::ZERO,
            sweep_max: Vec3::ZERO,
        };
        test.calculate_sweep_bounds();
        test
    }

    fn calculate_sweep_bounds(&mut self) {
        let max_extent = Vec3::new(
            (self.box_data.basis[0].x * self.box_data.extent.x).abs()
                + (self.box_data.basis[0].y * self.box_data.extent.y).abs()
                + (self.box_data.basis[0].z * self.box_data.extent.z).abs()
                + 0.01,
            (self.box_data.basis[1].x * self.box_data.extent.x).abs()
                + (self.box_data.basis[1].y * self.box_data.extent.y).abs()
                + (self.box_data.basis[1].z * self.box_data.extent.z).abs()
                + 0.01,
            (self.box_data.basis[2].x * self.box_data.extent.x).abs()
                + (self.box_data.basis[2].y * self.box_data.extent.y).abs()
                + (self.box_data.basis[2].z * self.box_data.extent.z).abs()
                + 0.01,
        );

        self.sweep_min = self.box_data.center - max_extent;
        self.sweep_max = self.box_data.center + max_extent;

        let end_min = self.box_data.center + self.move_vec - max_extent;
        let end_max = self.box_data.center + self.move_vec + max_extent;

        self.sweep_max = self.sweep_max.max(end_max);
        self.sweep_min = self.sweep_min.min(end_min);
    }

    /// Cull test against axis-aligned min/max bounds
    pub fn cull_min_max(&self, min: Vec3, max: Vec3) -> bool {
        self.sweep_min.x > max.x
            || self.sweep_max.x < min.x
            || self.sweep_min.y > max.y
            || self.sweep_max.y < min.y
            || self.sweep_min.z > max.z
            || self.sweep_max.z < min.z
    }

    /// Cull test against an AABox
    pub fn cull_aabox(&self, aabox: &AABoxClass) -> bool {
        let min_corner = aabox.center - aabox.extent;
        let max_corner = aabox.center + aabox.extent;
        self.cull_min_max(min_corner, max_corner)
    }
}
