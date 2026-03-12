//! Euler angles class - matches C++ EulerAnglesClass
//!
//! This implements Euler angle rotations using the standard aerospace convention:
//! - Roll (X axis)
//! - Pitch (Y axis)
//! - Yaw (Z axis)

use crate::wwmath::Matrix3x3;

/// Euler angle orders for conversion between matrices and Euler angles
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EulerOrder {
    /// XYZ order: Roll, Pitch, Yaw (aerospace standard)
    XYZ = 0,
    /// XZY order
    XZY = 1,
    /// YXZ order
    YXZ = 2,
    /// YZX order
    YZX = 3,
    /// ZXY order
    ZXY = 4,
    /// ZYX order
    ZYX = 5,
}

/// Euler angles representation - matches C++ EulerAnglesClass
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EulerAngles {
    /// Rotation angles in radians
    pub angles: [f64; 3],
    /// Order of rotations
    pub order: EulerOrder,
}

impl EulerAngles {
    /// Create new Euler angles with specified order
    pub fn new(order: EulerOrder) -> Self {
        Self {
            angles: [0.0, 0.0, 0.0],
            order,
        }
    }

    /// Create Euler angles from individual angles
    pub fn from_angles(angle0: f64, angle1: f64, angle2: f64, order: EulerOrder) -> Self {
        Self {
            angles: [angle0, angle1, angle2],
            order,
        }
    }

    /// Get angle at specified index
    pub fn get_angle(&self, index: usize) -> f64 {
        self.angles[index]
    }

    /// Set angle at specified index
    pub fn set_angle(&mut self, index: usize, angle: f64) {
        self.angles[index] = angle;
    }

    /// Convert Euler angles to rotation matrix - matches C++ To_Matrix
    pub fn to_matrix(&self) -> Matrix3x3 {
        // Implementation would convert Euler angles to rotation matrix
        // This is a simplified version - full implementation would handle all orders
        let (sin0, cos0) = (self.angles[0] as f32).sin_cos();
        let (sin1, cos1) = (self.angles[1] as f32).sin_cos();
        let (sin2, cos2) = (self.angles[2] as f32).sin_cos();

        let mut result = Matrix3x3::new();
        match self.order {
            EulerOrder::XYZ => {
                // Roll * Pitch * Yaw
                result.set_from_values(
                    cos1 * cos2,
                    cos1 * (-sin2),
                    sin1,
                    sin0 * sin1 * cos2 + cos0 * sin2,
                    sin0 * sin1 * (-sin2) + cos0 * cos2,
                    -sin0 * cos1,
                    cos0 * (-sin1) * cos2 + sin0 * sin2,
                    cos0 * (-sin1) * (-sin2) + sin0 * cos2,
                    cos0 * cos1,
                );
            }
            _ => {
                // Simplified - only XYZ implemented for now
                result.set_from_values(
                    cos1 * cos2,
                    cos1 * (-sin2),
                    sin1,
                    sin0 * sin1 * cos2 + cos0 * sin2,
                    sin0 * sin1 * (-sin2) + cos0 * cos2,
                    -sin0 * cos1,
                    cos0 * (-sin1) * cos2 + sin0 * sin2,
                    cos0 * (-sin1) * (-sin2) + sin0 * cos2,
                    cos0 * cos1,
                );
            }
        }
        result
    }

    /// Convert rotation matrix to Euler angles - matches C++ From_Matrix
    pub fn from_matrix(matrix: &Matrix3x3, order: EulerOrder) -> Self {
        // Simplified implementation - full version would handle singularities
        let mut angles = [0.0f64; 3];

        match order {
            EulerOrder::XYZ => {
                // Extract angles from XYZ rotation matrix
                let sy = (matrix[0][2] * matrix[0][2] + matrix[1][2] * matrix[1][2]).sqrt();
                if sy > 1e-6 {
                    angles[1] = (matrix[2][2] as f64).atan2(sy as f64); // pitch
                    angles[0] = ((-matrix[1][2]) as f64).atan2(matrix[0][2] as f64); // roll
                    angles[2] = (matrix[2][1] as f64).atan2(matrix[2][0] as f64);
                // yaw
                } else {
                    // Gimbal lock
                    angles[1] = if matrix[2][2] > 0.0 {
                        std::f64::consts::PI / 2.0
                    } else {
                        -std::f64::consts::PI / 2.0
                    };
                    angles[0] = 0.0;
                    angles[2] = (matrix[1][0] as f64).atan2(matrix[0][0] as f64);
                }
            }
            _ => {
                // Simplified - only XYZ implemented
                angles = [0.0, 0.0, 0.0];
            }
        }

        Self { angles, order }
    }
}

impl Default for EulerAngles {
    fn default() -> Self {
        Self::new(EulerOrder::XYZ)
    }
}
