//! Game-specific extensions for glam vector types.

use crate::WWMath;
use glam::Vec3;

/// Game-specific extensions for Vec3
pub trait Vec3Extensions {
    /// Quick length approximation using Graphics Gems 1 method (+/- 8% error)
    fn quick_length(self) -> f32;

    /// Scale this vector by another vector's components (component-wise multiplication)
    fn scale_by(self, scale: Vec3) -> Vec3;

    /// Rotate around X axis
    fn rotate_x(self, angle: f32) -> Vec3;

    /// Rotate around X axis with precomputed sin and cos
    fn rotate_x_with_sin_cos(self, sin_theta: f32, cos_theta: f32) -> Vec3;

    /// Rotate around Y axis
    fn rotate_y(self, angle: f32) -> Vec3;

    /// Rotate around Y axis with precomputed sin and cos
    fn rotate_y_with_sin_cos(self, sin_theta: f32, cos_theta: f32) -> Vec3;

    /// Rotate around Z axis
    fn rotate_z(self, angle: f32) -> Vec3;

    /// Rotate around Z axis with precomputed sin and cos
    fn rotate_z_with_sin_cos(self, sin_theta: f32, cos_theta: f32) -> Vec3;

    /// Check if all components are valid floats
    #[allow(clippy::wrong_self_convention)]
    fn is_valid(self) -> bool;

    /// Cap absolute values to those of another vector (matches C++ behavior)
    fn cap_absolute_to(self, other: Vec3) -> Vec3;

    /// Calculate quick distance approximation between two points
    fn quick_distance(self, other: Vec3) -> f32;

    /// Line intersection functions
    fn find_x_at_y(y: f32, p1: Vec3, p2: Vec3) -> f32;
    fn find_x_at_z(z: f32, p1: Vec3, p2: Vec3) -> f32;
    fn find_y_at_x(x: f32, p1: Vec3, p2: Vec3) -> f32;
    fn find_y_at_z(z: f32, p1: Vec3, p2: Vec3) -> f32;
    fn find_z_at_x(x: f32, p1: Vec3, p2: Vec3) -> f32;
    fn find_z_at_y(y: f32, p1: Vec3, p2: Vec3) -> f32;

    /// Color conversion functions (treating Vec3 as RGB)
    fn convert_to_abgr(self) -> u32;
    fn convert_to_argb(self) -> u32;

    /// Check if two vectors are equal within an epsilon
    fn equal_within_epsilon(self, other: Vec3, epsilon: f32) -> bool;

    /// Update this vector to have the minimum components of itself and another vector
    fn update_min(&mut self, other: &Vec3);

    /// Update this vector to have the maximum components of itself and another vector  
    fn update_max(&mut self, other: &Vec3);
}

impl Vec3Extensions for Vec3 {
    /// Quick length approximation using Graphics Gems 1 method (+/- 8% error)
    fn quick_length(self) -> f32 {
        let mut max = self.x.abs();
        let mut mid = self.y.abs();
        let mut min = self.z.abs();

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

    /// Scale this vector by another vector's components (component-wise multiplication)
    fn scale_by(self, scale: Vec3) -> Vec3 {
        Vec3::new(self.x * scale.x, self.y * scale.y, self.z * scale.z)
    }

    /// Rotate around X axis
    fn rotate_x(self, angle: f32) -> Vec3 {
        let cos_theta = angle.cos();
        let sin_theta = angle.sin();
        self.rotate_x_with_sin_cos(sin_theta, cos_theta)
    }

    /// Rotate around X axis with precomputed sin and cos
    fn rotate_x_with_sin_cos(self, sin_theta: f32, cos_theta: f32) -> Vec3 {
        Vec3::new(
            self.x,
            self.y * cos_theta - self.z * sin_theta,
            self.y * sin_theta + self.z * cos_theta,
        )
    }

    /// Rotate around Y axis
    fn rotate_y(self, angle: f32) -> Vec3 {
        let cos_theta = angle.cos();
        let sin_theta = angle.sin();
        self.rotate_y_with_sin_cos(sin_theta, cos_theta)
    }

    /// Rotate around Y axis with precomputed sin and cos
    fn rotate_y_with_sin_cos(self, sin_theta: f32, cos_theta: f32) -> Vec3 {
        Vec3::new(
            self.x * cos_theta + self.z * sin_theta,
            self.y,
            -self.x * sin_theta + self.z * cos_theta,
        )
    }

    /// Rotate around Z axis
    fn rotate_z(self, angle: f32) -> Vec3 {
        let cos_theta = angle.cos();
        let sin_theta = angle.sin();
        self.rotate_z_with_sin_cos(sin_theta, cos_theta)
    }

    /// Rotate around Z axis with precomputed sin and cos
    fn rotate_z_with_sin_cos(self, sin_theta: f32, cos_theta: f32) -> Vec3 {
        Vec3::new(
            self.x * cos_theta - self.y * sin_theta,
            self.x * sin_theta + self.y * cos_theta,
            self.z,
        )
    }

    /// Check if all components are valid floats
    #[allow(clippy::wrong_self_convention)]
    fn is_valid(self) -> bool {
        WWMath::is_valid_float(self.x)
            && WWMath::is_valid_float(self.y)
            && WWMath::is_valid_float(self.z)
    }

    /// Cap absolute values to those of another vector (matches C++ behavior)
    fn cap_absolute_to(self, other: Vec3) -> Vec3 {
        let mut result = self;

        if result.x > 0.0 {
            if other.x < result.x {
                result.x = other.x;
            }
        } else if -other.x > result.x {
            result.x = -other.x;
        }

        if result.y > 0.0 {
            if other.y < result.y {
                result.y = other.y;
            }
        } else if -other.y > result.y {
            result.y = -other.y;
        }

        if result.z > 0.0 {
            if other.z < result.z {
                result.z = other.z;
            }
        } else if -other.z > result.z {
            result.z = -other.z;
        }

        result
    }

    /// Calculate quick distance approximation between two points
    fn quick_distance(self, other: Vec3) -> f32 {
        (other - self).quick_length()
    }

    /// Line intersection functions
    fn find_x_at_y(y: f32, p1: Vec3, p2: Vec3) -> f32 {
        let t = (y - p1.y) / (p2.y - p1.y);
        WWMath::lerp(p1.x, p2.x, t)
    }

    fn find_x_at_z(z: f32, p1: Vec3, p2: Vec3) -> f32 {
        let t = (z - p1.z) / (p2.z - p1.z);
        WWMath::lerp(p1.x, p2.x, t)
    }

    fn find_y_at_x(x: f32, p1: Vec3, p2: Vec3) -> f32 {
        let t = (x - p1.x) / (p2.x - p1.x);
        WWMath::lerp(p1.y, p2.y, t)
    }

    fn find_y_at_z(z: f32, p1: Vec3, p2: Vec3) -> f32 {
        let t = (z - p1.z) / (p2.z - p1.z);
        WWMath::lerp(p1.y, p2.y, t)
    }

    fn find_z_at_x(x: f32, p1: Vec3, p2: Vec3) -> f32 {
        let t = (x - p1.x) / (p2.x - p1.x);
        WWMath::lerp(p1.z, p2.z, t)
    }

    fn find_z_at_y(y: f32, p1: Vec3, p2: Vec3) -> f32 {
        let t = (y - p1.y) / (p2.y - p1.y);
        WWMath::lerp(p1.z, p2.z, t)
    }

    /// Color conversion functions (treating Vec3 as RGB)
    fn convert_to_abgr(self) -> u32 {
        let r = (WWMath::clamp(self.x, 0.0, 1.0) * 255.0 + 0.5) as u32;
        let g = (WWMath::clamp(self.y, 0.0, 1.0) * 255.0 + 0.5) as u32;
        let b = (WWMath::clamp(self.z, 0.0, 1.0) * 255.0 + 0.5) as u32;
        0xFF000000 | (b << 16) | (g << 8) | r
    }

    fn convert_to_argb(self) -> u32 {
        let r = (WWMath::clamp(self.x, 0.0, 1.0) * 255.0 + 0.5) as u32;
        let g = (WWMath::clamp(self.y, 0.0, 1.0) * 255.0 + 0.5) as u32;
        let b = (WWMath::clamp(self.z, 0.0, 1.0) * 255.0 + 0.5) as u32;
        0xFF000000 | (r << 16) | (g << 8) | b
    }

    /// Check if two vectors are equal within an epsilon
    fn equal_within_epsilon(self, other: Vec3, epsilon: f32) -> bool {
        (self.x - other.x).abs() < epsilon
            && (self.y - other.y).abs() < epsilon
            && (self.z - other.z).abs() < epsilon
    }

    /// Update this vector to have the minimum components of itself and another vector
    fn update_min(&mut self, other: &Vec3) {
        self.x = self.x.min(other.x);
        self.y = self.y.min(other.y);
        self.z = self.z.min(other.z);
    }

    /// Update this vector to have the maximum components of itself and another vector  
    fn update_max(&mut self, other: &Vec3) {
        self.x = self.x.max(other.x);
        self.y = self.y.max(other.y);
        self.z = self.z.max(other.z);
    }
}
