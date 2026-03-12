//! Watcom C++ compatibility helpers mirroring WWLib `WATCOM.H`.

use core::f64::consts;

pub const M_E: f64 = consts::E;
pub const M_LOG2E: f64 = consts::LOG2_E;
pub const M_LOG10E: f64 = consts::LOG10_E;
pub const M_LN2: f64 = consts::LN_2;
pub const M_LN10: f64 = consts::LN_10;
pub const M_PI: f64 = consts::PI;
pub const M_PI_2: f64 = consts::FRAC_PI_2;
pub const M_PI_4: f64 = consts::FRAC_PI_4;
pub const M_1_PI: f64 = consts::FRAC_1_PI;
pub const M_2_PI: f64 = consts::FRAC_2_PI;
pub const M_1_SQRTPI: f64 = 0.564189583547756286948;
pub const M_2_SQRTPI: f64 = 1.12837916709551257390;
pub const M_SQRT2: f64 = consts::SQRT_2;
pub const M_SQRT_2: f64 = consts::FRAC_1_SQRT_2;
