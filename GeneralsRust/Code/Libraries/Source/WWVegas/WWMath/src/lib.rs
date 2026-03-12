#![cfg_attr(test, cfg(feature = "internal"))]

//! WWMath - Comprehensive 3D Mathematics Library
//!
//! This is the complete mathematical foundation for Command & Conquer Generals Zero Hour,
//! providing all the vector, matrix, quaternion, spline, collision, and culling mathematics
//! required by the game engine.
//!
//! ## Core Components
//!
//! ### Vectors and Matrices
//! - **Vector2/3/4**: 2D, 3D, and 4D vector mathematics
//! - **Matrix3/3D/4**: 3x3, 3D transformation, and 4x4 matrix operations
//! - **Quaternion**: Rotation mathematics and interpolation
//!
//! ### Geometric Primitives
//! - **AABox**: Axis-aligned bounding boxes
//! - **Sphere**: Sphere mathematics and intersections
//! - **Plane/AAPlane**: Plane mathematics and classifications
//! - **Frustum**: View frustum culling
//! - **Triangle**: Triangle mathematics and ray casting
//! - **LineSegment**: Line segment operations
//!
//! ### Splines and Curves
//! - **Hermite/Cardinal/CatmullRom/TCB Splines**: Parametric curve mathematics
//! - **Curve3D/1D**: General curve framework
//! - **VehicleCurve**: Specialized vehicle path curves
//!
//! ### Collision and Culling
//! - **CollisionMath**: Comprehensive collision detection
//! - **CullSystem**: Spatial culling with Grid and AAB Tree implementations
//! - **Intersection/Overlap**: Geometric intersection tests
//!
//! ### Advanced Systems
//! - **ODESystem**: Ordinary differential equation integration
//! - **VectorProcessor**: SIMD-optimized vector operations
//! - **LookupTable**: Mathematical lookup table management
//!
//! ## Example Usage
//!
//! ```rust
//! use wwmath::{Vector3, Matrix4, Quaternion, WWMath};
//!
//! // Basic vector operations
//! let v1 = Vector3::new(1.0, 2.0, 3.0);
//! let v2 = Vector3::new(4.0, 5.0, 6.0);
//! let dot_product = v1.dot(&v2);
//!
//! // Matrix transformations
//! let transform = Matrix4::identity();
//! let translated = transform.translate(&Vector3::new(10.0, 0.0, 0.0));
//!
//! // Quaternion rotations
//! let rotation = Quaternion::from_axis_angle(&Vector3::unit_y(), WWMath::deg_to_rad(45.0));
//! let rotated_vector = rotation.rotate_vector(&v1);
//! ```

#[warn(missing_docs)]
#[warn(rustdoc::missing_crate_level_docs)]
#[warn(clippy::all)]
#[warn(clippy::pedantic)]
#[allow(clippy::module_name_repetitions)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::struct_excessive_bools)]
pub mod aabox;
pub mod aaplane;
pub mod affine;
pub mod cardinal_spline;
pub mod castres;
pub mod catmull_rom_spline;
pub mod collision;
pub mod culling;
pub mod curve;
pub mod euler;
pub mod frustum;
pub mod hermite_spline;
pub mod line_segment;
pub mod lookup_table;
pub mod matrix3;
pub mod matrix3d;
pub mod matrix4;
pub mod matrix_extensions;
pub mod normal_cone;
pub mod ode;
pub mod plane;
pub mod pot;
pub mod quat;
pub mod quat_extensions;
pub mod rect;
pub mod sphere;
pub mod tcb_spline;
pub mod triangle;
pub mod vec2_extensions;
// Core math modules - import from the comprehensive implementation
pub mod vector2;
pub mod vector2i;
pub mod vector3;
pub mod vector3_random;
pub mod vector3i;
pub mod vector4;
pub mod vector_extensions;
pub mod vector_processor;
pub mod vehicle_curve;
pub mod wwmath;
pub mod wwmath_ids;

// Re-export all core types for convenient usage
pub use aabox::{AABox, MinMaxAABox, OverlapResult};
pub use aaplane::{AAPlane, AxisEnum};
pub use cardinal_spline::{CardinalSpline1D, CardinalSpline3D};
pub use castres::{raycast_flags, CastResultStruct};
pub use catmull_rom_spline::{CatmullRomSpline1D, CatmullRomSpline3D};
pub use collision::{
    CastResult, CollisionMath, CollisionStats, OverlapType, COINCIDENCE_EPSILON, COLLISION_EPSILON,
};
pub use culling::{
    AABTreeCullSystem, AABTreeNode, CullCollection, CullStats, CullSystem, CullType, Cullable,
    GridCullSystem,
};
pub use culling::{CollisionMath as CullCollisionMath, OverlapType as CullOverlapType};
pub use curve::{
    Curve1D, Curve3D, CurveKey1D, CurveKey3D, LinearCurve1D, LinearCurve3D, Tangents1D, Tangents3D,
};
pub use euler::{EulerAngles, EulerOrder};
pub use frustum::Frustum;
pub use hermite_spline::{HermiteSpline1D, HermiteSpline3D};
pub use line_segment::LineSegment;
pub use lookup_table::{
    global_table_manager, Curve1D as LookupCurve1D, LinearCurve as LookupLinearCurve, LookupTable,
    LookupTableManager, SineCurve,
};
pub use matrix3::Matrix3;
pub use matrix3d::Matrix3D;
pub use matrix4::Matrix4;
pub use matrix4::Matrix4 as Mat4;
pub use normal_cone::NormalCone;
pub use ode::{HarmonicOscillator, IntegrationSystem, ODESystem, StateVector};
pub use plane::{Plane, PlaneSide};
pub use pot::{find_pot, find_pot_log2, is_power_of_2, next_power_of_2, prev_power_of_2};
pub use quat::Quaternion;
pub use rect::Rect;
pub use sphere::{add_spheres, spheres_intersect, transform_sphere, Sphere};
pub use tcb_spline::{TcbParams, TcbSpline1D, TcbSpline3D};
pub use triangle::{
    cast_semi_infinite_axis_aligned_ray_to_triangle, point_in_triangle_2d, Triangle,
    TRI_RAYCAST_FLAG_HIT_EDGE, TRI_RAYCAST_FLAG_NONE, TRI_RAYCAST_FLAG_START_IN_TRI,
};
pub use vector2::Vector2;
pub use vector2i::Vector2i;
pub use vector3::Vector3;
pub use vector3::Vector3 as Vec3;
pub use vector3_random::{
    RandomizerClassId, Vector3HollowSphereRandomizer, Vector3Randomizer, Vector3SolidBoxRandomizer,
    Vector3SolidCylinderRandomizer, Vector3SolidSphereRandomizer,
};
pub use vector3i::{Vector3i, Vector3i16};
pub use vector4::Vector4;
pub use vector_extensions::Vec3Extensions;
pub use vector_processor::VectorProcessor;
pub use vehicle_curve::{ArcInfo, VehicleCurve};
pub use wwmath::{
    WWMath, DEG_TO_RAD, EPSILON, EPSILON2, FLOAT_MAX, FLOAT_MIN, OO_SQRT2, OO_SQRT3, PI,
    RAD_TO_DEG, SQRT2, SQRT3,
};
pub use wwmath_ids::*;

/// WWMath version information
pub const WWMATH_VERSION: &str = "0.1.0";

/// Initialize the WWMath system
/// This should be called once during application startup
pub fn init() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize lookup table manager if needed
    lookup_table::global_table_manager();

    Ok(())
}

/// Get WWMath system information
pub fn system_info() -> String {
    format!(
        "WWMath v{} - Comprehensive 3D Mathematics Library",
        WWMATH_VERSION
    )
}
