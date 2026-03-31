////////////////////////////////////////////////////////////////////////////////
//																			//
//  (c) 2001-2003 Electronic Arts Inc.										//
//																			//
////////////////////////////////////////////////////////////////////////////////

//! Things are the base class for objects and drawables
//! Objects are logic side representations while drawables are client side
//! Common data will be held in the Thing and systems that need to work with
//! both of them will work with "Things"

use crate::common::ini::get_global_data as get_engine_global_data;
use crate::common::{
    rts::{AsciiString, Real},
    system::{Coord3D, Matrix3D},
    thing::thing_template::ThingTemplate,
};
use std::sync::{Arc, Mutex, OnceLock};

/// Kind of type enumeration for object classification
pub type KindOfType = u32;
pub type KindOfMaskType = u64;

/// Global terrain height provider, registered from GameLogic during init.
/// Returns ground height at (x, y). Falls back to 0.0 if not registered.
static GROUND_HEIGHT_PROVIDER: OnceLock<
    Mutex<Box<dyn Fn(f32, f32) -> f32 + Send + Sync>>,
> = OnceLock::new();

/// Global underwater check provider, registered from GameLogic during init.
/// Returns (is_underwater, water_level). Falls back to (false, 0.0) if not registered.
static UNDERWATER_PROVIDER: OnceLock<
    Mutex<Box<dyn Fn(f32, f32) -> (bool, f32) + Send + Sync>>,
> = OnceLock::new();

/// Register a terrain height provider from the GameLogic layer.
/// This is called once during game initialization.
pub fn register_terrain_height_provider(
    provider: impl Fn(f32, f32) -> f32 + Send + Sync + 'static,
) {
    GROUND_HEIGHT_PROVIDER
        .get_or_init(|| Mutex::new(Box::new(provider)));
}

/// Register an underwater check provider from the GameLogic layer.
pub fn register_underwater_provider(
    provider: impl Fn(f32, f32) -> (bool, f32) + Send + Sync + 'static,
) {
    UNDERWATER_PROVIDER
        .get_or_init(|| Mutex::new(Box::new(provider)));
}

/// Cache flags for optimizing recalculations
#[derive(Debug, Clone, Copy)]
struct CacheFlags(u8);

impl CacheFlags {
    const VALID_DIRVECTOR: CacheFlags = CacheFlags(0x01);
    const VALID_ALTITUDE_TERRAIN: CacheFlags = CacheFlags(0x02);
    const VALID_ALTITUDE_SEALEVEL: CacheFlags = CacheFlags(0x04);

    fn new() -> Self {
        CacheFlags(0)
    }

    fn has(self, flag: CacheFlags) -> bool {
        (self.0 & flag.0) != 0
    }

    fn set(&mut self, flag: CacheFlags) {
        self.0 |= flag.0;
    }

    fn clear(&mut self, flag: CacheFlags) {
        self.0 &= !flag.0;
    }

    fn clear_all(&mut self) {
        self.0 = 0;
    }
}

/// Forward declarations
pub trait Object: Send + Sync {
    // Object-specific methods would be defined here
}

pub trait Drawable: Send + Sync {
    // Drawable-specific methods would be defined here
}

/// Thing trait - common base for objects and drawables
pub trait Thing: Send + Sync {
    /// Get the thing template for this thing
    fn get_template(&self) -> &ThingTemplate;

    /// Convenience method for checking kindof flags
    fn is_kind_of(&self, kind: KindOfType) -> bool;
    fn is_kind_of_multi(
        &self,
        must_be_set: &KindOfMaskType,
        must_be_clear: &KindOfMaskType,
    ) -> bool;
    fn is_any_kind_of(&self, any_kind_of: &KindOfMaskType) -> bool;

    /// Position and orientation methods
    fn set_position(&mut self, pos: &Coord3D);
    fn set_position_z(&mut self, z: Real);
    fn set_orientation(&mut self, angle: Real);

    fn get_position(&self) -> &Coord3D;
    fn get_orientation(&self) -> Real;
    fn get_unit_direction_vector_2d(&self) -> &Coord3D;
    fn get_unit_direction_vector_3d(&self) -> Coord3D;

    /// Height calculations
    fn get_height_above_terrain(&self) -> Real;
    fn get_height_above_terrain_or_water(&self) -> Real;
    fn is_above_terrain(&self) -> bool {
        self.get_height_above_terrain() > 0.0
    }
    fn is_above_terrain_or_water(&self) -> bool {
        self.get_height_above_terrain_or_water() > 0.0
    }
    fn is_significantly_above_terrain(&self) -> bool;

    /// Matrix operations
    fn set_transform_matrix(&mut self, mx: &Matrix3D);
    fn get_transform_matrix(&self) -> &Matrix3D;
    fn transform_point(&self, input: &Coord3D, output: &mut Coord3D);

    /// Bone transformation
    fn convert_bone_pos_to_world_pos(
        &self,
        bone_pos: &Coord3D,
        bone_transform: &Matrix3D,
        world_pos: &mut Coord3D,
        world_transform: &mut Matrix3D,
    );

    /// Cast to specific types
    fn as_object(&self) -> Option<&dyn Object> {
        None
    }
    fn as_drawable(&self) -> Option<&dyn Drawable> {
        None
    }
    fn as_object_mut(&mut self) -> Option<&mut dyn Object> {
        None
    }
    fn as_drawable_mut(&mut self) -> Option<&mut dyn Drawable> {
        None
    }

    /// Called when transform changes
    fn react_to_transform_change(&mut self, old_mtx: &Matrix3D, old_pos: &Coord3D, old_angle: Real);
}

/// Base implementation of Thing
pub struct BaseThing {
    template: Arc<ThingTemplate>,
    #[cfg(any(debug_assertions, feature = "internal"))]
    template_name: AsciiString,

    // Transform data - m_transform is the authoritative source
    transform: Matrix3D,

    // Cached values for efficiency
    cached_pos: Coord3D,
    cached_angle: Real,
    cached_dir_vector: Coord3D,
    cached_altitude_above_terrain: Real,
    cached_altitude_above_terrain_or_water: Real,
    cache_flags: CacheFlags,
}

impl BaseThing {
    /// Create a new Thing with the given template
    pub fn new(thing_template: Arc<ThingTemplate>) -> Self {
        if thing_template.is_null_template() {
            panic!("Cannot create thing without template");
        }

        Self {
            template: thing_template.clone(),
            #[cfg(any(debug_assertions, feature = "internal"))]
            template_name: thing_template.get_name().clone(),

            transform: Matrix3D::identity(),
            cached_pos: Coord3D::new(0.0, 0.0, 0.0),
            cached_angle: 0.0,
            cached_dir_vector: Coord3D::new(0.0, 0.0, 0.0),
            cached_altitude_above_terrain: 0.0,
            cached_altitude_above_terrain_or_water: 0.0,
            cache_flags: CacheFlags::new(),
        }
    }

    /// Calculate the actual height above terrain without using cache
    fn calculate_height_above_terrain(&self) -> Real {
        let pos = self.get_position();
        // This would call TheTerrainLogic->getGroundHeight in the real implementation
        let terrain_z = self.get_ground_height(pos.x, pos.y);
        pos.z - terrain_z
    }

    /// Get ground height at coordinates via the registered terrain provider.
    /// Returns 0.0 if no provider is registered (before terrain init).
    fn get_ground_height(&self, x: Real, y: Real) -> Real {
        GROUND_HEIGHT_PROVIDER
            .get()
            .and_then(|p| p.lock().ok())
            .map(|f| f(x, y))
            .unwrap_or(0.0)
    }

    /// Check if underwater and get water level via the registered provider.
    /// Returns (false, 0.0) if no provider is registered.
    fn is_underwater(&self, x: Real, y: Real) -> (bool, Real) {
        UNDERWATER_PROVIDER
            .get()
            .and_then(|p| p.lock().ok())
            .map(|f| f(x, y))
            .unwrap_or((false, 0.0))
    }

    /// Normalize angle to -PI..PI range
    fn normalize_angle(angle: Real) -> Real {
        use std::f32::consts::PI;
        let mut result = angle;
        while result > PI {
            result -= 2.0 * PI;
        }
        while result < -PI {
            result += 2.0 * PI;
        }
        result
    }
}

impl Thing for BaseThing {
    fn get_template(&self) -> &ThingTemplate {
        &self.template
    }

    fn is_kind_of(&self, kind: KindOfType) -> bool {
        self.template.is_kind_of(kind)
    }

    fn is_kind_of_multi(
        &self,
        must_be_set: &KindOfMaskType,
        must_be_clear: &KindOfMaskType,
    ) -> bool {
        self.template.is_kind_of_multi(must_be_set, must_be_clear)
    }

    fn is_any_kind_of(&self, any_kind_of: &KindOfMaskType) -> bool {
        self.template.is_any_kind_of(any_kind_of)
    }

    fn set_position(&mut self, pos: &Coord3D) {
        // Store old values for change notification
        let old_angle = self.cached_angle;
        let old_pos = self.cached_pos;
        let old_mtx = self.transform;

        // Check if we need to stick to terrain slope
        if !self.template.is_kind_of(0x1000) {
            // KINDOF_STICK_TO_TERRAIN_SLOPE placeholder
            // Normal positioning
            self.transform.set_translation(pos.x, pos.y, pos.z);
            self.cached_pos = *pos;
            self.cache_flags.clear(CacheFlags::VALID_ALTITUDE_TERRAIN);
            self.cache_flags.clear(CacheFlags::VALID_ALTITUDE_SEALEVEL);

            self.react_to_transform_change(&old_mtx, &old_pos, old_angle);
        } else {
            // Align to terrain - would need terrain logic integration
            let mtx = Matrix3D::identity();
            // TheTerrainLogic->alignOnTerrain(getOrientation(), *pos, true, mtx);
            self.set_transform_matrix(&mtx);
        }

        debug_assert!(
            !pos.x.is_nan() && !pos.y.is_nan() && !pos.z.is_nan(),
            "Thing position contains NaN values"
        );
    }

    fn set_position_z(&mut self, z: Real) {
        if !self.template.is_kind_of(0x1000) {
            // KINDOF_STICK_TO_TERRAIN_SLOPE
            let old_angle = self.cached_angle;
            let old_pos = self.cached_pos;
            let old_mtx = self.transform;

            self.transform.set_z_translation(z);
            self.cached_pos.z = z;

            if self.cache_flags.has(CacheFlags::VALID_ALTITUDE_TERRAIN) {
                self.cached_altitude_above_terrain += z - old_pos.z;
            }
            if self.cache_flags.has(CacheFlags::VALID_ALTITUDE_SEALEVEL) {
                self.cached_altitude_above_terrain_or_water += z - old_pos.z;
            }

            self.react_to_transform_change(&old_mtx, &old_pos, old_angle);
        } else {
            // Stick to terrain slope
            let mtx = Matrix3D::identity();
            let _pos = self.cached_pos;
            // TheTerrainLogic->alignOnTerrain(getOrientation(), pos, true, mtx);
            self.set_transform_matrix(&mtx);
        }
    }

    fn set_orientation(&mut self, angle: Real) {
        let old_angle = self.cached_angle;
        let old_pos = self.cached_pos;
        let old_mtx = self.transform;

        let pos = Coord3D::new(
            self.transform.get_x_translation(),
            self.transform.get_y_translation(),
            self.transform.get_z_translation(),
        );

        if self.template.is_kind_of(0x1000) { // KINDOF_STICK_TO_TERRAIN_SLOPE
             // Align to terrain
             // TheTerrainLogic->alignOnTerrain(angle, pos, true, self.transform);
        } else {
            // Standard orientation - straight up in Z axis
            let cos_angle = angle.cos();
            let sin_angle = angle.sin();

            // Create rotation matrix
            self.transform = Matrix3D::new([
                [cos_angle, -sin_angle, 0.0, pos.x],
                [sin_angle, cos_angle, 0.0, pos.y],
                [0.0, 0.0, 1.0, pos.z],
                [0.0, 0.0, 0.0, 1.0],
            ]);
        }

        self.cached_angle = Self::normalize_angle(angle);
        self.cached_pos = pos;
        self.cache_flags.clear(CacheFlags::VALID_DIRVECTOR);

        self.react_to_transform_change(&old_mtx, &old_pos, old_angle);
    }

    fn get_position(&self) -> &Coord3D {
        &self.cached_pos
    }

    fn get_orientation(&self) -> Real {
        self.cached_angle
    }

    fn get_unit_direction_vector_2d(&self) -> &Coord3D {
        if !self.cache_flags.has(CacheFlags::VALID_DIRVECTOR) {
            let angle = self.get_orientation();
            let _cached_dir = self.cached_dir_vector;
            // Note: In mutable context, we'd update the cache here
            // For immutable access, we return the current cached value
            let _ = (angle, _cached_dir); // Silence warnings
        }
        &self.cached_dir_vector
    }

    fn get_unit_direction_vector_3d(&self) -> Coord3D {
        let x_vector = self.transform.get_x_vector();
        let normalized = x_vector.normalize();
        Coord3D::new(normalized.x, normalized.y, normalized.z)
    }

    fn get_height_above_terrain(&self) -> Real {
        if !self.cache_flags.has(CacheFlags::VALID_ALTITUDE_TERRAIN) {
            // In mutable context, we'd cache the result
            return self.calculate_height_above_terrain();
        }
        self.cached_altitude_above_terrain
    }

    fn get_height_above_terrain_or_water(&self) -> Real {
        if !self.cache_flags.has(CacheFlags::VALID_ALTITUDE_SEALEVEL) {
            let pos = self.get_position();
            let (is_underwater, water_z) = self.is_underwater(pos.x, pos.y);

            if is_underwater {
                return pos.z - water_z;
            } else {
                return self.get_height_above_terrain();
            }
        }
        self.cached_altitude_above_terrain_or_water
    }

    fn is_significantly_above_terrain(&self) -> bool {
        // If it's high enough that it will take more than 3 frames to return to ground
        let gravity = get_engine_global_data()
            .map(|data| data.read().gravity)
            .unwrap_or(-9.8);
        self.get_height_above_terrain() > -(3.0 * 3.0) * gravity
    }

    fn set_transform_matrix(&mut self, mx: &Matrix3D) {
        let old_angle = self.cached_angle;
        let old_pos = self.cached_pos;
        let old_mtx = self.transform;

        self.transform = *mx;
        self.cached_pos.x = mx.get_x_translation();
        self.cached_pos.y = mx.get_y_translation();
        self.cached_pos.z = mx.get_z_translation();
        self.cached_angle = mx.get_z_rotation();
        self.cache_flags.clear_all();

        self.react_to_transform_change(&old_mtx, &old_pos, old_angle);
    }

    fn get_transform_matrix(&self) -> &Matrix3D {
        &self.transform
    }

    fn transform_point(&self, input: &Coord3D, output: &mut Coord3D) {
        let transformed = self.transform.transform_vector(input);
        *output = transformed;
    }

    fn convert_bone_pos_to_world_pos(
        &self,
        bone_pos: &Coord3D,
        bone_transform: &Matrix3D,
        world_pos: &mut Coord3D,
        world_transform: &mut Matrix3D,
    ) {
        if !world_transform.is_null() {
            *world_transform = self.transform.multiply(bone_transform);
        }

        if !world_pos.is_null() {
            let vector = self.transform.transform_vector(bone_pos);
            *world_pos = vector;
        }
    }

    fn react_to_transform_change(
        &mut self,
        _old_mtx: &Matrix3D,
        _old_pos: &Coord3D,
        _old_angle: Real,
    ) {
        // Base implementation - would be overridden by subclasses
    }
}
