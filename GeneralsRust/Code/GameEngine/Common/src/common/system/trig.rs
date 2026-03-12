////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: trig.rs ///////////////////////////////////////////////////////////////
// Author: Michael S. Booth, March 1994
// Converted to Generals by Matthew D. Campbell, February 2002
// Fast trig functions using lookup tables
///////////////////////////////////////////////////////////////////////////////

use lazy_static::lazy_static;
use std::f32::consts::{PI, TAU};

const DEG2RAD: f32 = PI / 180.0;
const TRIG_RES: usize = 4096;

// Fixed point constants with 12 fractional bits
const INT_ONE: i32 = 4096;
const INT_TWOPI: i32 = 25736;
const _INT_THREEPIOVERTWO: i32 = 19302;
const _INT_PI: i32 = 12868;
const INT_HALFPI: i32 = 6434;

lazy_static! {
    static ref SIN_LOOKUP: Vec<f32> = {
        let mut table = Vec::with_capacity(TRIG_RES);
        for i in 0..TRIG_RES {
            let angle = (i as f32) * TAU / (TRIG_RES as f32);
            table.push(angle.sin());
        }
        table
    };

    static ref COS_LOOKUP: Vec<f32> = {
        let mut table = Vec::with_capacity(TRIG_RES);
        for i in 0..TRIG_RES {
            let angle = (i as f32) * TAU / (TRIG_RES as f32);
            table.push(angle.cos());
        }
        table
    };

    static ref ARCCOS_LOOKUP: Vec<f32> = {
        let mut table = Vec::with_capacity(1024);
        for i in 0..1024 {
            let x = (i as f32) / 1023.0 * 2.0 - 1.0; // Map to [-1, 1]
            table.push(x.acos());
        }
        table
    };
}

/// Fast sine using lookup table
pub fn fast_sin(angle: f32) -> f32 {
    let normalized = (angle / TAU) % 1.0;
    let index = ((normalized + 1.0) % 1.0 * TRIG_RES as f32) as usize % TRIG_RES;
    SIN_LOOKUP[index]
}

/// Fast cosine using lookup table
pub fn fast_cos(angle: f32) -> f32 {
    let normalized = (angle / TAU) % 1.0;
    let index = ((normalized + 1.0) % 1.0 * TRIG_RES as f32) as usize % TRIG_RES;
    COS_LOOKUP[index]
}

/// Fast tangent using lookup table
pub fn fast_tan(angle: f32) -> f32 {
    let sin_val = fast_sin(angle);
    let cos_val = fast_cos(angle);
    if cos_val.abs() < f32::EPSILON {
        if sin_val > 0.0 {
            f32::INFINITY
        } else {
            f32::NEG_INFINITY
        }
    } else {
        sin_val / cos_val
    }
}

/// Fast arc cosine using lookup table
pub fn fast_arccos(x: f32) -> f32 {
    let clamped = x.clamp(-1.0, 1.0);
    let index = ((clamped + 1.0) * 0.5 * 1023.0) as usize;
    let index = index.min(1023);
    ARCCOS_LOOKUP[index]
}

/// Fast arc sine using arc cosine
pub fn fast_arcsin(x: f32) -> f32 {
    PI * 0.5 - fast_arccos(x)
}

/// Fast arc tangent using built-in atan2 (for simplicity)
pub fn fast_arctan2(y: f32, x: f32) -> f32 {
    y.atan2(x)
}

/// Convert degrees to radians
pub fn deg_to_rad(degrees: f32) -> f32 {
    degrees * DEG2RAD
}

/// Convert radians to degrees
pub fn rad_to_deg(radians: f32) -> f32 {
    radians / DEG2RAD
}

/// Normalize angle to [0, 2π)
pub fn normalize_angle(angle: f32) -> f32 {
    let mut result = angle % TAU;
    if result < 0.0 {
        result += TAU;
    }
    result
}

/// Normalize angle to [-π, π)
pub fn normalize_angle_signed(angle: f32) -> f32 {
    let mut result = angle % TAU;
    if result > PI {
        result -= TAU;
    } else if result < -PI {
        result += TAU;
    }
    result
}

/// Fixed point sine (12-bit fractional)
pub fn int_sin(angle: i32) -> i32 {
    let normalized_angle = angle % INT_TWOPI;
    let index = ((normalized_angle * TRIG_RES as i32) / INT_TWOPI) as usize % TRIG_RES;
    (SIN_LOOKUP[index] * INT_ONE as f32) as i32
}

/// Fixed point cosine (12-bit fractional)
pub fn int_cos(angle: i32) -> i32 {
    int_sin(INT_HALFPI - angle)
}

/// Distance between two points
pub fn distance(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    (dx * dx + dy * dy).sqrt()
}

/// Fast distance estimation (less accurate but faster)
pub fn fast_distance(x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    let max_val = dx.max(dy);
    let min_val = dx.min(dy);
    max_val + min_val * 0.25
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::{PI, TAU};

    #[test]
    fn test_fast_trig_functions() {
        // Test sine
        assert!((fast_sin(0.0) - 0.0).abs() < 0.01);
        assert!((fast_sin(PI / 2.0) - 1.0).abs() < 0.01);
        assert!((fast_sin(PI) - 0.0).abs() < 0.01);

        // Test cosine
        assert!((fast_cos(0.0) - 1.0).abs() < 0.01);
        assert!((fast_cos(PI / 2.0) - 0.0).abs() < 0.01);
        assert!((fast_cos(PI) - (-1.0)).abs() < 0.01);

        // Test tangent
        assert!((fast_tan(0.0) - 0.0).abs() < 0.01);
        assert!((fast_tan(PI / 4.0) - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_angle_normalization() {
        assert!((normalize_angle(TAU + 1.0) - 1.0).abs() < f32::EPSILON);
        assert!((normalize_angle(-1.0) - (TAU - 1.0)).abs() < f32::EPSILON);

        assert!((normalize_angle_signed(PI + 1.0) - (-PI + 1.0)).abs() < f32::EPSILON);
        assert!((normalize_angle_signed(-PI - 1.0) - (PI - 1.0)).abs() < f32::EPSILON);
    }

    #[test]
    fn test_distance_functions() {
        assert!((distance(0.0, 0.0, 3.0, 4.0) - 5.0).abs() < f32::EPSILON);

        let fast_dist = fast_distance(0.0, 0.0, 3.0, 4.0);
        assert!(fast_dist > 4.0 && fast_dist < 6.0); // Should be approximate
    }

    #[test]
    fn test_deg_rad_conversion() {
        assert!((deg_to_rad(180.0) - PI).abs() < f32::EPSILON);
        assert!((rad_to_deg(PI) - 180.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_fixed_point_trig() {
        let sin_val = int_sin(INT_HALFPI); // sin(π/2)
        assert!((sin_val as f32 / INT_ONE as f32 - 1.0).abs() < 0.01);

        let cos_val = int_cos(0); // cos(0)
        assert!((cos_val as f32 / INT_ONE as f32 - 1.0).abs() < 0.01);
    }
}
