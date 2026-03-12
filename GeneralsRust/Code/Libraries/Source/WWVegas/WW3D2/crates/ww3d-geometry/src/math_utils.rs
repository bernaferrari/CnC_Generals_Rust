//! Math utility functions for vector, matrix, and quaternion operations.
//!
//! This module provides commonly-used mathematical operations ported from the C++ WW3D codebase.
//! Functions are optimized for game development use cases.

use glam::{Mat4, Quat, Vec2, Vec3, Vec4};

/// Linear interpolation between two 3D vectors.
///
/// # Arguments
/// * `a` - Start vector
/// * `b` - End vector
/// * `t` - Interpolation factor (0.0 = a, 1.0 = b)
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Lerp`
#[inline]
pub fn vec3_lerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

/// Spherical linear interpolation between two 3D vectors.
///
/// Provides smooth interpolation along the great circle arc.
///
/// # C++ Reference
/// `quat.cpp`: `Slerp` (adapted for vectors)
pub fn vec3_slerp(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let a_norm = a.normalize();
    let b_norm = b.normalize();

    let dot = a_norm.dot(b_norm).clamp(-1.0, 1.0);
    let theta = dot.acos();

    if theta.abs() < 1e-6 {
        // Vectors are nearly parallel, use linear interpolation
        return vec3_lerp(a, b, t);
    }

    let sin_theta = theta.sin();
    let ratio_a = ((1.0 - t) * theta).sin() / sin_theta;
    let ratio_b = (t * theta).sin() / sin_theta;

    let a_len = a.length();
    let b_len = b.length();
    let len = a_len + (b_len - a_len) * t;

    (a_norm * ratio_a + b_norm * ratio_b) * len
}

/// Projects vector `v` onto vector `onto`.
///
/// # C++ Reference
/// Common pattern in physics calculations
#[inline]
pub fn vec3_project(v: Vec3, onto: Vec3) -> Vec3 {
    let onto_len_sq = onto.length_squared();
    if onto_len_sq < 1e-8 {
        return Vec3::ZERO;
    }
    onto * (v.dot(onto) / onto_len_sq)
}

/// Returns a vector perpendicular to the input vector.
///
/// Chooses the perpendicular vector in a deterministic way.
#[inline]
pub fn vec3_perpendicular(v: Vec3) -> Vec3 {
    let abs_v = v.abs();

    // Choose axis that is least aligned with input vector
    let axis = if abs_v.x <= abs_v.y && abs_v.x <= abs_v.z {
        Vec3::X
    } else if abs_v.y <= abs_v.z {
        Vec3::Y
    } else {
        Vec3::Z
    };

    v.cross(axis).normalize()
}

/// Quick approximate length calculation (faster but less accurate).
///
/// From Graphics Gems 1, gives +/- 8% error.
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Quick_Length`
#[inline]
pub fn vec3_quick_length(v: Vec3) -> f32 {
    let mut max = v.x.abs();
    let mut mid = v.y.abs();
    let mut min = v.z.abs();

    // Sort components
    if max < mid {
        std::mem::swap(&mut max, &mut mid);
    }
    if max < min {
        std::mem::swap(&mut max, &mut min);
    }
    if mid < min {
        std::mem::swap(&mut mid, &mut min);
    }

    max + (11.0 / 32.0) * mid + (1.0 / 4.0) * min
}

/// Quick approximate distance calculation.
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Quick_Distance`
#[inline]
pub fn vec3_quick_distance(p1: Vec3, p2: Vec3) -> f32 {
    vec3_quick_length(p1 - p2)
}

/// Creates a rotation matrix from Euler angles (pitch, yaw, roll).
///
/// Angles are in radians. Rotation order is: Y (yaw) -> X (pitch) -> Z (roll)
///
/// # C++ Reference
/// `matrix3d.cpp`: Rotation combination pattern
pub fn matrix_from_euler(pitch: f32, yaw: f32, roll: f32) -> Mat4 {
    Mat4::from_euler(glam::EulerRot::YXZ, yaw, pitch, roll)
}

/// Creates a "look-at" matrix for camera positioning.
///
/// # Arguments
/// * `eye` - Camera position
/// * `target` - Point to look at
/// * `up` - Up direction (usually Vec3::Y)
///
/// # C++ Reference
/// `matrix3d.cpp`: `Matrix3D::Look_At`
pub fn matrix_look_at(eye: Vec3, target: Vec3, up: Vec3) -> Mat4 {
    Mat4::look_at_rh(eye, target, up)
}

/// Decomposes a transformation matrix into translation, rotation, and scale components.
///
/// Returns (translation, rotation_quat, scale)
///
/// # C++ Reference
/// Pattern from various matrix decomposition routines
pub fn matrix_decompose(m: Mat4) -> (Vec3, Quat, Vec3) {
    let translation = m.w_axis.truncate();

    // Extract scale from each axis
    let scale_x = m.x_axis.truncate().length();
    let scale_y = m.y_axis.truncate().length();
    let scale_z = m.z_axis.truncate().length();
    let scale = Vec3::new(scale_x, scale_y, scale_z);

    // Remove scale to get rotation matrix
    let rotation_mat = Mat4::from_cols(
        m.x_axis / scale_x,
        m.y_axis / scale_y,
        m.z_axis / scale_z,
        Vec4::W,
    );

    let rotation = Quat::from_mat4(&rotation_mat);

    (translation, rotation, scale)
}

/// Linear interpolation between two matrices.
///
/// # C++ Reference
/// `matrix3d.cpp`: `Matrix3D::Lerp`
pub fn matrix_lerp(a: Mat4, b: Mat4, t: f32) -> Mat4 {
    let (a_trans, a_rot, a_scale) = matrix_decompose(a);
    let (b_trans, b_rot, b_scale) = matrix_decompose(b);

    let trans = vec3_lerp(a_trans, b_trans, t);
    let rot = a_rot.slerp(b_rot, t);
    let scale = vec3_lerp(a_scale, b_scale, t);

    Mat4::from_scale_rotation_translation(scale, rot, trans)
}

/// Fast quaternion spherical linear interpolation.
///
/// Less accurate but faster than standard slerp for small angles.
///
/// # C++ Reference
/// `quat.cpp`: `Fast_Slerp`
pub fn quat_fast_slerp(a: Quat, b: Quat, t: f32) -> Quat {
    // For small angles, use normalized lerp (nlerp)
    let dot = a.dot(b);

    let b_adjusted = if dot < 0.0 {
        // Take shorter path
        -b
    } else {
        b
    };

    // Normalized linear interpolation
    let result = a * (1.0 - t) + b_adjusted * t;
    result.normalize()
}

/// Rotates a vector around the X axis.
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Rotate_X`
#[inline]
pub fn vec3_rotate_x(v: Vec3, angle: f32) -> Vec3 {
    let s = angle.sin();
    let c = angle.cos();
    Vec3::new(v.x, c * v.y - s * v.z, s * v.y + c * v.z)
}

/// Rotates a vector around the Y axis.
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Rotate_Y`
#[inline]
pub fn vec3_rotate_y(v: Vec3, angle: f32) -> Vec3 {
    let s = angle.sin();
    let c = angle.cos();
    Vec3::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z)
}

/// Rotates a vector around the Z axis.
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Rotate_Z`
#[inline]
pub fn vec3_rotate_z(v: Vec3, angle: f32) -> Vec3 {
    let s = angle.sin();
    let c = angle.cos();
    Vec3::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z)
}

/// Checks if two vectors are equal within an epsilon tolerance.
///
/// # C++ Reference
/// `vector3.h`: `Equal_Within_Epsilon`
#[inline]
pub fn vec3_equal_within_epsilon(a: Vec3, b: Vec3, epsilon: f32) -> bool {
    (a.x - b.x).abs() < epsilon && (a.y - b.y).abs() < epsilon && (a.z - b.z).abs() < epsilon
}

/// Checks if two quaternions are equal within an epsilon tolerance.
///
/// # C++ Reference
/// `quat.h`: `Equal_Within_Epsilon`
#[inline]
pub fn quat_equal_within_epsilon(a: Quat, b: Quat, epsilon: f32) -> bool {
    (a.x - b.x).abs() < epsilon
        && (a.y - b.y).abs() < epsilon
        && (a.z - b.z).abs() < epsilon
        && (a.w - b.w).abs() < epsilon
}

/// Clamps each component of a vector to a given range.
#[inline]
pub fn vec3_clamp(v: Vec3, min: Vec3, max: Vec3) -> Vec3 {
    Vec3::new(
        v.x.clamp(min.x, max.x),
        v.y.clamp(min.y, max.y),
        v.z.clamp(min.z, max.z),
    )
}

/// Component-wise minimum of two vectors.
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Update_Min`
#[inline]
pub fn vec3_min(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x.min(b.x), a.y.min(b.y), a.z.min(b.z))
}

/// Component-wise maximum of two vectors.
///
/// # C++ Reference
/// `vector3.h`: `Vector3::Update_Max`
#[inline]
pub fn vec3_max(a: Vec3, b: Vec3) -> Vec3 {
    Vec3::new(a.x.max(b.x), a.y.max(b.y), a.z.max(b.z))
}

/// Computes a vector from an angle (2D, in X-Y plane).
///
/// # C++ Reference
/// `mathutil.cpp`: `cMathUtil::Angle_To_Vector`
#[inline]
pub fn angle_to_vector_2d(angle: f32) -> Vec2 {
    Vec2::new(angle.cos(), angle.sin())
}

/// Computes an angle from a 2D vector.
///
/// # C++ Reference
/// `mathutil.cpp`: `cMathUtil::Vector_To_Angle`
#[inline]
pub fn vector_to_angle_2d(v: Vec2) -> f32 {
    v.y.atan2(v.x)
}

/// Rotates a 2D vector by an angle.
///
/// # C++ Reference
/// `mathutil.cpp`: `cMathUtil::Rotate_Vector`
pub fn vec2_rotate(v: Vec2, angle: f32) -> Vec2 {
    let s = angle.sin();
    let c = angle.cos();
    Vec2::new(v.x * c - v.y * s, v.x * s + v.y * c)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    #[test]
    fn test_vec3_lerp() {
        let a = Vec3::new(0.0, 0.0, 0.0);
        let b = Vec3::new(10.0, 10.0, 10.0);

        let result = vec3_lerp(a, b, 0.5);
        assert!((result - Vec3::new(5.0, 5.0, 5.0)).length() < EPSILON);

        assert!((vec3_lerp(a, b, 0.0) - a).length() < EPSILON);
        assert!((vec3_lerp(a, b, 1.0) - b).length() < EPSILON);
    }

    #[test]
    fn test_vec3_project() {
        let v = Vec3::new(1.0, 1.0, 0.0);
        let onto = Vec3::new(1.0, 0.0, 0.0);

        let result = vec3_project(v, onto);
        assert!((result - Vec3::new(1.0, 0.0, 0.0)).length() < EPSILON);
    }

    #[test]
    fn test_vec3_perpendicular() {
        let v = Vec3::new(1.0, 0.0, 0.0);
        let perp = vec3_perpendicular(v);

        // Should be orthogonal
        assert!(v.dot(perp).abs() < EPSILON);
        // Should be unit length
        assert!((perp.length() - 1.0).abs() < EPSILON);
    }

    #[test]
    fn test_vec3_quick_length() {
        let v = Vec3::new(3.0, 4.0, 0.0);
        let exact = v.length();
        let approx = vec3_quick_length(v);

        // Should be within 8% error
        let error = (exact - approx).abs() / exact;
        assert!(error < 0.09);
    }

    #[test]
    fn test_vec3_rotate_x() {
        let v = Vec3::new(0.0, 1.0, 0.0);
        let angle = std::f32::consts::FRAC_PI_2; // 90 degrees

        let result = vec3_rotate_x(v, angle);
        assert!((result - Vec3::new(0.0, 0.0, 1.0)).length() < EPSILON);
    }

    #[test]
    fn test_vec3_rotate_y() {
        let v = Vec3::new(1.0, 0.0, 0.0);
        let angle = std::f32::consts::FRAC_PI_2;

        let result = vec3_rotate_y(v, angle);
        assert!((result - Vec3::new(0.0, 0.0, -1.0)).length() < EPSILON);
    }

    #[test]
    fn test_vec3_rotate_z() {
        let v = Vec3::new(1.0, 0.0, 0.0);
        let angle = std::f32::consts::FRAC_PI_2;

        let result = vec3_rotate_z(v, angle);
        assert!((result - Vec3::new(0.0, 1.0, 0.0)).length() < EPSILON);
    }

    #[test]
    fn test_matrix_decompose() {
        let trans = Vec3::new(1.0, 2.0, 3.0);
        let rot = Quat::from_rotation_y(std::f32::consts::FRAC_PI_4);
        let scale = Vec3::new(2.0, 3.0, 4.0);

        let mat = Mat4::from_scale_rotation_translation(scale, rot, trans);
        let (d_trans, d_rot, d_scale) = matrix_decompose(mat);

        assert!((d_trans - trans).length() < EPSILON);
        assert!((d_scale - scale).length() < EPSILON);
        assert!(quat_equal_within_epsilon(d_rot, rot, EPSILON));
    }

    #[test]
    fn test_vec3_equal_within_epsilon() {
        let a = Vec3::new(1.0, 2.0, 3.0);
        let b = Vec3::new(1.0001, 2.0001, 3.0001);

        assert!(vec3_equal_within_epsilon(a, b, 0.001));
        assert!(!vec3_equal_within_epsilon(a, b, 0.00001));
    }

    #[test]
    fn test_vec2_rotate() {
        let v = Vec2::new(1.0, 0.0);
        let angle = std::f32::consts::FRAC_PI_2;

        let result = vec2_rotate(v, angle);
        assert!((result - Vec2::new(0.0, 1.0)).length() < EPSILON);
    }

    #[test]
    fn test_angle_to_vector_2d() {
        let angle = std::f32::consts::FRAC_PI_4; // 45 degrees
        let v = angle_to_vector_2d(angle);

        let expected = Vec2::new(2.0_f32.sqrt() / 2.0, 2.0_f32.sqrt() / 2.0);
        assert!((v - expected).length() < EPSILON);
    }
}
