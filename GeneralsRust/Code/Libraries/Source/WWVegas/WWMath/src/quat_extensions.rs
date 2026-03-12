//! Game-specific extensions for glam quaternion types.

use crate::WWMath;
use glam::{Mat3, Quat, Vec3};

/// Game-specific extensions for Quat
pub trait QuatExtensions {
    /// Check if quaternion is valid (no NaN or infinite values)
    #[allow(clippy::wrong_self_convention)]
    fn is_valid(self) -> bool;

    /// Rotate a vector by this quaternion (alternative to glam's * operator)
    fn rotate_vector(self, v: Vec3) -> Vec3;

    /// Make this quaternion the closest representation to another quaternion
    /// (handles the dual-cover property of quaternions)
    fn make_closest(self, other: Quat) -> Quat;

    /// Create a random quaternion (uniform distribution on the 4D sphere)
    fn random() -> Quat;

    /// SLERP with caching optimization for repeated interpolations
    fn slerp_cached(q1: Quat, q2: Quat, t: f32, cache: &mut SlerpCache) -> Quat;

    /// Convert to 3x3 rotation matrix
    fn to_mat3(self) -> Mat3;
}

/// Cached SLERP information for optimized repeated interpolations
#[derive(Debug, Copy, Clone, Default)]
pub struct SlerpCache {
    pub sin_omega: f32,
    pub omega: f32,
    pub flip: bool,
    pub linear: bool,
}

impl QuatExtensions for Quat {
    /// Check if quaternion is valid (no NaN or infinite values)
    #[allow(clippy::wrong_self_convention)]
    fn is_valid(self) -> bool {
        WWMath::is_valid_float(self.x)
            && WWMath::is_valid_float(self.y)
            && WWMath::is_valid_float(self.z)
            && WWMath::is_valid_float(self.w)
    }

    /// Rotate a vector by this quaternion (equivalent to self * v)
    fn rotate_vector(self, v: Vec3) -> Vec3 {
        self * v
    }

    /// Make this quaternion the closest representation to another quaternion
    /// Handles the fact that q and -q represent the same rotation
    fn make_closest(mut self, other: Quat) -> Quat {
        let dot = self.x * other.x + self.y * other.y + self.z * other.z + self.w * other.w;
        if dot < 0.0 {
            self = -self;
        }
        self
    }

    /// Create a random quaternion with uniform distribution
    fn random() -> Quat {
        use crate::WWMath;

        // Use Marsaglia's method for uniform quaternion distribution
        let u1 = WWMath::unit_random();
        let u2 = WWMath::unit_random();
        let u3 = WWMath::unit_random();

        let sqrt1_u1 = (1.0_f32 - u1).sqrt();
        let sqrt_u1 = u1.sqrt();
        let two_pi_u2 = 2.0 * WWMath::PI * u2;
        let two_pi_u3 = 2.0 * WWMath::PI * u3;

        Quat::from_xyzw(
            sqrt1_u1 * two_pi_u2.sin(),
            sqrt1_u1 * two_pi_u2.cos(),
            sqrt_u1 * two_pi_u3.sin(),
            sqrt_u1 * two_pi_u3.cos(),
        )
    }

    /// SLERP with caching for repeated interpolations between same quaternions
    fn slerp_cached(q1: Quat, q2: Quat, t: f32, cache: &mut SlerpCache) -> Quat {
        // Use cached values if available, otherwise compute and cache
        if cache.linear {
            // Linear interpolation fallback
            (q1 * (1.0 - t) + q2 * t).normalize()
        } else {
            // Spherical linear interpolation
            let sin_omega_t = (cache.omega * t).sin();
            let sin_omega_1_minus_t = (cache.omega * (1.0 - t)).sin();

            let result = (q1 * sin_omega_1_minus_t + q2 * sin_omega_t) / cache.sin_omega;
            if cache.flip {
                -result
            } else {
                result
            }
        }
    }

    /// Convert quaternion to 3x3 rotation matrix
    fn to_mat3(self) -> Mat3 {
        Mat3::from_quat(self)
    }
}

/// Compute and cache SLERP parameters for repeated interpolation
pub fn compute_slerp_cache(q1: Quat, q2: Quat, cache: &mut SlerpCache) {
    const SLERP_EPSILON: f32 = 0.001;

    let mut dot = q1.dot(q2);
    cache.flip = dot < 0.0;
    if cache.flip {
        dot = -dot;
    }

    if dot > 1.0 - SLERP_EPSILON {
        // Quats are very close, use linear interpolation
        cache.linear = true;
        cache.sin_omega = 1.0;
        cache.omega = 0.0;
    } else {
        // Use spherical linear interpolation
        cache.linear = false;
        cache.omega = dot.acos();
        cache.sin_omega = cache.omega.sin();
    }
}
