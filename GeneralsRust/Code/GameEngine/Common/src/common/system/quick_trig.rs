////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: quick_trig.rs ////////////////////////////////////////////////////////
// Author: Mark Lorenzen (adapted by srj)
// Desc:   Fast trig functions using lookup tables
///////////////////////////////////////////////////////////////////////////////

use std::f32::consts::PI;

pub type Real = f32;

/// Quarter circle constant
pub const QUARTER_CIRCLE: Real = PI / 2.0;

/// Quick sine lookup table
pub static QUICK_SIN_TABLE: [Real; 129] = [
    0.00000, 0.01237, 0.02473, 0.03710, 0.04945, 0.06180, 0.07414, 0.08647, 0.09879, 0.11109,
    0.12337, 0.13563, 0.14788, 0.16010, 0.17229, 0.18446, 0.19661, 0.20872, 0.22080, 0.23284,
    0.24485, 0.25683, 0.26876, 0.28065, 0.29250, 0.30431, 0.31607, 0.32778, 0.33944, 0.35104,
    0.36260, 0.37410, 0.38554, 0.39692, 0.40824, 0.41950, 0.43070, 0.44183, 0.45289, 0.46388,
    0.47480, 0.48565, 0.49643, 0.50712, 0.51774, 0.52829, 0.53875, 0.54913, 0.55942, 0.56963,
    0.57975, 0.58978, 0.59973, 0.60958, 0.61934, 0.62900, 0.63857, 0.64804, 0.65741, 0.66668,
    0.67584, 0.68491, 0.69387, 0.70272, 0.71147, 0.72010, 0.72863, 0.73705, 0.74535, 0.75354,
    0.76161, 0.76957, 0.77741, 0.78513, 0.79273, 0.80020, 0.80756, 0.81479, 0.82190, 0.82888,
    0.83574, 0.84247, 0.84907, 0.85554, 0.86187, 0.86808, 0.87415, 0.88009, 0.88590, 0.89157,
    0.89710, 0.90250, 0.90775, 0.91287, 0.91785, 0.92269, 0.92739, 0.93195, 0.93636, 0.94063,
    0.94476, 0.94874, 0.95257, 0.95626, 0.95981, 0.96321, 0.96646, 0.96956, 0.97251, 0.97532,
    0.97798, 0.98048, 0.98284, 0.98505, 0.98710, 0.98901, 0.99076, 0.99236, 0.99381, 0.99511,
    0.99625, 0.99725, 0.99809, 0.99878, 0.99931, 0.99969, 0.99992, 1.00000, 0.99992,
];

/// Quick tangent lookup table
pub static QUICK_TAN_TABLE: [Real; 129] = [
    0.00000, 0.00787, 0.01575, 0.02363, 0.03151, 0.03939, 0.04728, 0.05517, 0.06308, 0.07099,
    0.07890, 0.08683, 0.09477, 0.10272, 0.11068, 0.11866, 0.12666, 0.13466, 0.14269, 0.15073,
    0.15880, 0.16688, 0.17498, 0.18311, 0.19126, 0.19943, 0.20763, 0.21586, 0.22412, 0.23240,
    0.24071, 0.24906, 0.25744, 0.26585, 0.27430, 0.28279, 0.29131, 0.29987, 0.30847, 0.31712,
    0.32581, 0.33454, 0.34332, 0.35214, 0.36102, 0.36994, 0.37892, 0.38795, 0.39704, 0.40618,
    0.41539, 0.42465, 0.43398, 0.44337, 0.45282, 0.46234, 0.47194, 0.48160, 0.49134, 0.50115,
    0.51104, 0.52101, 0.53106, 0.54120, 0.55143, 0.56174, 0.57214, 0.58264, 0.59324, 0.60393,
    0.61473, 0.62563, 0.63664, 0.64777, 0.65900, 0.67035, 0.68183, 0.69342, 0.70515, 0.71700,
    0.72899, 0.74112, 0.75339, 0.76581, 0.77838, 0.79110, 0.80398, 0.81703, 0.83025, 0.84363,
    0.85720, 0.87096, 0.88490, 0.89904, 0.91338, 0.92793, 0.94269, 0.95767, 0.97288, 0.98833,
    1.00401, 1.01995, 1.03615, 1.05261, 1.06935, 1.08637, 1.10368, 1.12130, 1.13924, 1.15749,
    1.17609, 1.19503, 1.21433, 1.23400, 1.25406, 1.27452, 1.29540, 1.31670, 1.33845, 1.36067,
    1.38336, 1.40656, 1.43027, 1.45453, 1.47935, 1.50475, 1.53076, 1.55741, 1.58471,
];

/// Quick magnitude estimation without square root
/// NOTE: This is a very rough estimate, and may be off by 10% or more,
/// so use it only when you don't need accuracy
pub fn qmag(x: Real, y: Real, z: Real) -> Real {
    let mut max_v = x.abs();
    let mut med_v = y.abs();
    let mut min_v = z.abs();

    // Sort values so max_v >= med_v >= min_v
    if max_v < med_v {
        std::mem::swap(&mut max_v, &mut med_v);
    }

    if max_v < min_v {
        std::mem::swap(&mut max_v, &mut min_v);
    }

    if med_v < min_v {
        std::mem::swap(&mut med_v, &mut min_v);
    }

    med_v += min_v;
    max_v + (med_v * 0.25)
}

/// Fast sine function using lookup table
pub fn qsin(mut angle: Real) -> Real {
    let mut sign = 1.0;

    // Handle negative angles
    if angle < 0.0 {
        sign = -1.0;
        angle = -angle;
    }

    // Modulate angle into range of PI
    while angle > PI {
        angle -= PI;
        sign = -sign;
    }

    // Handle angles > PI/2
    if angle > PI / 2.0 {
        angle = PI - angle;
    }

    let table_size = QUICK_SIN_TABLE.len() as Real;
    let index = ((angle / QUARTER_CIRCLE) * table_size) as usize;

    // Clamp index to valid range
    let index = index.min(QUICK_SIN_TABLE.len() - 1);

    QUICK_SIN_TABLE[index] * sign
}

/// Fast cosine function using lookup table
pub fn qcos(angle: Real) -> Real {
    qsin(QUARTER_CIRCLE - angle)
}

/// Fast tangent function using lookup table
pub fn qtan(angle: Real) -> Real {
    let table_size = QUICK_TAN_TABLE.len() as Real;
    let index = (angle * table_size) as usize;

    // Clamp index to valid range
    let index = index.min(QUICK_TAN_TABLE.len() - 1);

    QUICK_TAN_TABLE[index]
}

/// Fast cosecant function (1/sin)
pub fn qcsc(angle: Real) -> Real {
    1.0 / qsin(angle)
}

/// Fast secant function (1/cos)
pub fn qsec(angle: Real) -> Real {
    1.0 / qcos(angle)
}

/// Fast cotangent function (1/tan)
pub fn qcot(angle: Real) -> Real {
    1.0 / qtan(angle)
}

/// Get the count of entries in the sine table
pub fn get_quick_sin_table_count() -> Real {
    QUICK_SIN_TABLE.len() as Real
}

/// Get the count of entries in the tangent table
pub fn get_quick_tan_table_count() -> Real {
    QUICK_TAN_TABLE.len() as Real
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_qmag() {
        let result = qmag(3.0, 4.0, 0.0);
        // Should be roughly 5.0 (3-4-5 triangle)
        assert!((result - 5.0).abs() < 1.0); // Within rough approximation
    }

    #[test]
    fn test_qsin() {
        // Test basic values
        assert!((qsin(0.0) - 0.0).abs() < 0.01);
        assert!((qsin(PI / 2.0) - 1.0).abs() < 0.01);
        assert!((qsin(PI) - 0.0).abs() < 0.01);
        assert!((qsin(-PI / 2.0) - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn test_qcos() {
        // Test basic values
        assert!((qcos(0.0) - 1.0).abs() < 0.01);
        assert!((qcos(PI / 2.0) - 0.0).abs() < 0.01);
        assert!((qcos(PI) - (-1.0)).abs() < 0.01);
    }

    #[test]
    fn test_trig_identities() {
        let angle = PI / 4.0;

        // sin^2 + cos^2 = 1 (approximately)
        let sin_val = qsin(angle);
        let cos_val = qcos(angle);
        let identity = sin_val * sin_val + cos_val * cos_val;
        assert!((identity - 1.0).abs() < 0.1);

        // tan = sin/cos (approximately)
        let tan_val = qtan(angle);
        let expected_tan = sin_val / cos_val;
        assert!((tan_val - expected_tan).abs() < 0.1);
    }

    #[test]
    fn test_reciprocal_functions() {
        let angle = PI / 4.0;

        // Test csc = 1/sin
        let sin_val = qsin(angle);
        let csc_val = qcsc(angle);
        assert!((sin_val * csc_val - 1.0).abs() < 0.01);

        // Test sec = 1/cos
        let cos_val = qcos(angle);
        let sec_val = qsec(angle);
        assert!((cos_val * sec_val - 1.0).abs() < 0.01);

        // Test cot = 1/tan
        let tan_val = qtan(angle);
        let cot_val = qcot(angle);
        assert!((tan_val * cot_val - 1.0).abs() < 0.1);
    }

    #[test]
    fn test_table_counts() {
        assert_eq!(get_quick_sin_table_count(), QUICK_SIN_TABLE.len() as Real);
        assert_eq!(get_quick_tan_table_count(), QUICK_TAN_TABLE.len() as Real);
    }
}
